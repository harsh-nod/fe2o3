//! Exact pending compute output consumed by an ordinary scalar peer copy.

use super::*;
use crate::context::peer_reconciliation::DirectedPeerStateV1;

pub(in crate::context) struct ComputePeerInputV1 {
    pub(in crate::context) producer: ScalarPeerDependencyV1,
    pub(in crate::context) state: DirectedPeerStateV1,
}

fn full_device_endpoints(root: &ScalarPeerCopyRootV1) -> bool {
    root.source.region.access == RuntimeAccessV1::Read
        && root.destination.region.access == RuntimeAccessV1::Write
        && [root.source, root.destination].iter().all(|endpoint| {
            endpoint.record.kind == RuntimeMemoryKindV1::DeviceLocal
                && endpoint.region.byte_offset == 0
                && endpoint.region.byte_len != 0
                && endpoint.region.byte_len == endpoint.record.byte_len
        })
}

fn exact_written_source(launch: &ProducerLaunchRootV1, source: ContextReadSourceV1) -> bool {
    launch.covers_input_v1(source)
        && launch
            .bindings
            .iter()
            .filter(|binding| binding.region.allocation == source.region.allocation)
            .all(|binding| {
                binding.record == source.record && binding.region.access == RuntimeAccessV1::Write
            })
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(in crate::context) fn prepare_compute_peer_custody_v1(
        &mut self,
        root: &mut ScalarPeerCopyRootV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self.versions.is_none()
            || !self.backend.supports_pending_compute_peer_copy_v1()
            || !full_device_endpoints(root)
        {
            return Ok(());
        }
        let selected = root
            .dependencies
            .iter()
            .find(|dependency| {
                self.submissions[&dependency.submission].status
                    == RuntimeCompletionStatusV1::Pending
                    && self
                        .producer_launches
                        .get(&dependency.submission)
                        .is_some_and(|launch| exact_written_source(launch, root.source))
            })
            .copied();
        let Some(producer) = selected else {
            return Ok(());
        };
        let mut depth = 1;
        for (index, dependency) in root.dependencies.iter().enumerate() {
            if index > 0 && root.dependencies[index - 1].submission == dependency.submission {
                return Err(RuntimeValidationErrorV1::DuplicateDependency);
            }
            self.check_operation_custody_v1(dependency.submission)?;
            let record = self.submissions[&dependency.submission];
            if dependency == &producer {
                if dependency.device != root.source.record.device {
                    return Err(RuntimeValidationErrorV1::WrongDevice);
                }
                if record.quiescent || !record.producer_launch {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
            } else if record.status != RuntimeCompletionStatusV1::Succeeded || !record.quiescent {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            depth = depth.max(
                self.completion_parent_depth_v1(dependency.submission)?
                    .checked_add(1)
                    .ok_or(RuntimeValidationErrorV1::Capacity)?,
            );
        }
        if depth > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies);
        }
        root.compute = Some(ComputePeerInputV1 {
            producer,
            state: DirectedPeerStateV1 {
                depth,
                cursor: 0,
                terminal: None,
            },
        });
        Ok(())
    }

    pub(in crate::context) fn validate_compute_peer_custody_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        root: &ScalarPeerCopyRootV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let Some(compute) = &root.compute else {
            return Ok(());
        };
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        if self.versions.is_none()
            || root.directed.is_some()
            || !full_device_endpoints(root)
            || compute.state.depth < 2
            || compute.state.depth > MAX_RUNTIME_DEPENDENCIES_V1
            || compute.state.cursor > root.dependencies.len()
            || compute.state.terminal == Some(BackendPollV1::Pending)
            || compute.state.terminal.is_none() && compute.state.cursor != 0
            || !root.dependencies.contains(&compute.producer)
            || compute.producer.device != root.source.record.device
            || self.submissions.get(&id).is_some_and(|record| {
                record.producer_launch
                    || record.same_device_copy
                    || record.status == RuntimeCompletionStatusV1::Succeeded
                        && (!record.quiescent
                            || compute.state.terminal != Some(BackendPollV1::Succeeded)
                            || compute.state.cursor != root.dependencies.len())
            })
        {
            return Err(invalid);
        }
        if !root.dependencies_held {
            return Ok(());
        }
        for endpoint in [root.source, root.destination] {
            if self.allocations.get(&endpoint.region.allocation) != Some(&endpoint.record)
                || !self
                    .backend_allocations
                    .contains(&endpoint.record.backend_allocation)
                || !self.allocation_admission.has_expected_credit(
                    endpoint.region.allocation,
                    endpoint.record.device,
                    endpoint.record.byte_len,
                )
            {
                return Err(invalid);
            }
        }
        let mut depth = 1;
        for (index, dependency) in root.dependencies.iter().enumerate() {
            if index > 0 && root.dependencies[index - 1].submission == dependency.submission {
                return Err(invalid);
            }
            self.validate_completion_parent_rank_v1(
                id,
                dependency.submission,
                compute.state.depth,
            )?;
            let record = self
                .submissions
                .get(&dependency.submission)
                .ok_or(invalid)?;
            if dependency.submission.local >= id.local
                || record.status == RuntimeCompletionStatusV1::Succeeded && !record.quiescent
                || index < compute.state.cursor
                    && record.status != RuntimeCompletionStatusV1::Succeeded
            {
                return Err(invalid);
            }
            if dependency == &compute.producer {
                self.validate_producer_launch_custody_v1(dependency.submission)?;
                if !record.producer_launch
                    || self
                        .producer_launches
                        .get(&dependency.submission)
                        .is_none_or(|launch| !exact_written_source(launch, root.source))
                {
                    return Err(invalid);
                }
            } else if record.status != RuntimeCompletionStatusV1::Succeeded || !record.quiescent {
                return Err(invalid);
            }
            let parent = self
                .completion_parent_depth_v1(dependency.submission)
                .map_err(|_| invalid)?;
            if parent == 0 || parent >= compute.state.depth {
                return Err(invalid);
            }
            depth = depth.max(parent + 1);
        }
        if depth != compute.state.depth {
            return Err(invalid);
        }
        Ok(())
    }
}
