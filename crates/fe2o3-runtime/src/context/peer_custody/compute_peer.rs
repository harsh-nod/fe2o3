//! Exact pending compute output or segmented frame consumed by a scalar peer.

use super::*;
use crate::context::peer_reconciliation::DirectedPeerStateV1;
use crate::context::peer_segments::SegmentedPeerFrameV1;

#[cfg(test)]
thread_local! {
    static COMPUTE_PEER_VALIDATION_VISITS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

pub(in crate::context) struct ComputePeerInputV1 {
    pub(in crate::context) producer: ScalarPeerDependencyV1,
    pub(in crate::context) state: DirectedPeerStateV1,
    origin: ScalarPeerSourceV1,
    regions: [RuntimeMemoryRegionV1; 2],
    extents: [u64; 2],
    predecessor: Option<usize>,
    preserves_frame: bool,
}

enum ScalarPeerSourceV1 {
    Compute,
    SegmentedFrame(SegmentedPeerFrameV1),
}

impl ComputePeerInputV1 {
    pub(in crate::context) fn is_segmented_frame_v1(&self) -> bool {
        matches!(self.origin, ScalarPeerSourceV1::SegmentedFrame(_))
    }

    pub(in crate::context) fn matches_segmented_frame_v1(
        &self,
        root: &SegmentedPeerCopyRootV1,
        source: ContextReadSourceV1,
    ) -> bool {
        matches!(&self.origin, ScalarPeerSourceV1::SegmentedFrame(frame)
            if frame.matches_v1(root, source))
    }
}

impl ScalarPeerCopyRootV1 {
    pub(in crate::context) fn compute_predecessor_v1(&self) -> Option<&ScalarPeerDependencyV1> {
        self.compute
            .as_ref()
            .and_then(|compute| compute.predecessor)
            .and_then(|index| self.dependencies.get(index))
    }

    pub(in crate::context) fn preserves_destination_frame_v1(
        &self,
        source: ContextReadSourceV1,
    ) -> bool {
        self.compute.as_ref().is_some_and(|compute| {
            compute.preserves_frame
                && compute.regions == [self.source.region, self.destination.region]
                && compute.extents
                    == [
                        self.source.record.byte_len,
                        self.destination.record.byte_len,
                    ]
        }) && self.directed.is_none()
            && bounded_device_endpoints(self)
            && source.region.access == RuntimeAccessV1::Read
            && source.region.allocation == self.destination.region.allocation
            && source.record == self.destination.record
            && source.region.byte_len != 0
            && source
                .region
                .byte_offset
                .checked_add(source.region.byte_len)
                .is_some_and(|end| end <= self.destination.record.byte_len)
    }
}

fn bounded_device_endpoints(root: &ScalarPeerCopyRootV1) -> bool {
    root.source.region.access == RuntimeAccessV1::Read
        && root.destination.region.access == RuntimeAccessV1::Write
        && root.source.region.byte_len == root.destination.region.byte_len
        && [root.source, root.destination].iter().all(|endpoint| {
            endpoint.record.kind == RuntimeMemoryKindV1::DeviceLocal
                && endpoint.region.byte_len != 0
                && endpoint
                    .region
                    .byte_offset
                    .checked_add(endpoint.region.byte_len)
                    .is_some_and(|end| end <= endpoint.record.byte_len)
        })
}

fn exact_written_source(launch: &ProducerLaunchRootV1, source: ContextReadSourceV1) -> bool {
    let mut bindings = launch
        .bindings
        .iter()
        .filter(|binding| binding.region.allocation == source.region.allocation);
    let Some(binding) = bindings.next() else {
        return false;
    };
    bindings.next().is_none()
        && binding.record == source.record
        && binding.region.access == RuntimeAccessV1::Write
        && binding.region.byte_offset == 0
        && binding.region.byte_len == source.record.byte_len
        && launch.covers_input_v1(source)
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    #[cfg(test)]
    pub(in crate::context) fn count_compute_peer_validations_for_test_v1<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> T,
    ) -> (T, usize) {
        COMPUTE_PEER_VALIDATION_VISITS.with(|visits| visits.set(0));
        let result = operation(self);
        let visits = COMPUTE_PEER_VALIDATION_VISITS.with(|visits| visits.replace(0));
        (result, visits)
    }

    pub(in crate::context) fn compute_peer_predecessor_matches_v1(
        &self,
        root: &ScalarPeerCopyRootV1,
        dependency: &ScalarPeerDependencyV1,
    ) -> bool {
        self.scalar_peer_copies
            .get(&dependency.submission)
            .is_some_and(|previous| {
                previous.compute.as_ref().is_some_and(|compute| {
                    compute.preserves_frame && matches!(compute.origin, ScalarPeerSourceV1::Compute)
                }) && previous.directed.is_none()
                    && previous.stream == root.stream
                    && previous.destination.region.allocation == root.destination.region.allocation
                    && previous.destination.record == root.destination.record
                    && dependency.device == root.destination.record.device
            })
    }

    pub(in crate::context) fn prepare_compute_peer_custody_v1(
        &mut self,
        root: &mut ScalarPeerCopyRootV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self.versions.is_none()
            && self.backend.supports_pending_segment_frame_peer_copy_v1()
            && root.dependencies.iter().any(|dependency| {
                let record = self.submissions[&dependency.submission];
                record.status == RuntimeCompletionStatusV1::Pending
                    && record.segmented_destination == Some(root.source.region.allocation)
            })
        {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        if self.versions.is_none()
            || !(self.backend.supports_pending_compute_peer_copy_v1()
                || self.backend.supports_pending_segment_frame_peer_copy_v1())
            || !bounded_device_endpoints(root)
        {
            return Ok(());
        }
        let selected = root.dependencies.iter().find_map(|dependency| {
            if self.submissions[&dependency.submission].status != RuntimeCompletionStatusV1::Pending
            {
                return None;
            }
            if self.backend.supports_pending_compute_peer_copy_v1()
                && self
                    .producer_launches
                    .get(&dependency.submission)
                    .is_some_and(|launch| exact_written_source(launch, root.source))
            {
                return Some((*dependency, ScalarPeerSourceV1::Compute));
            }
            self.backend
                .supports_pending_segment_frame_peer_copy_v1()
                .then(|| {
                    self.segmented_peer_copies
                        .get(&dependency.submission)?
                        .destination_frame_v1(root.source)
                        .map(|frame| (*dependency, ScalarPeerSourceV1::SegmentedFrame(frame)))
                })
                .flatten()
        });
        let Some((producer, origin)) = selected else {
            return Ok(());
        };
        let mut depth = 1;
        let mut predecessor = None;
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
                if record.quiescent
                    || match &origin {
                        ScalarPeerSourceV1::Compute => !record.producer_launch,
                        ScalarPeerSourceV1::SegmentedFrame(_) => !record.segmented_peer_copy,
                    }
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
            } else if record.status != RuntimeCompletionStatusV1::Succeeded || !record.quiescent {
                if record.status != RuntimeCompletionStatusV1::Pending
                    || record.quiescent
                    || predecessor.is_some()
                    || !matches!(origin, ScalarPeerSourceV1::Compute)
                    || !self.backend.supports_ordered_compute_peer_copy_v1()
                    || !self.compute_peer_predecessor_matches_v1(root, dependency)
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                predecessor = Some(index);
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
            regions: [root.source.region, root.destination.region],
            extents: [
                root.source.record.byte_len,
                root.destination.record.byte_len,
            ],
            predecessor,
            preserves_frame: match &origin {
                ScalarPeerSourceV1::Compute => self.backend.supports_ordered_compute_peer_copy_v1(),
                ScalarPeerSourceV1::SegmentedFrame(_) => true,
            },
            origin,
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
        #[cfg(test)]
        COMPUTE_PEER_VALIDATION_VISITS.with(|visits| visits.set(visits.get() + 1));
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        if self.versions.is_none()
            || root.directed.is_some()
            || !bounded_device_endpoints(root)
            || compute.regions != [root.source.region, root.destination.region]
            || compute.extents
                != [
                    root.source.record.byte_len,
                    root.destination.record.byte_len,
                ]
            || compute.state.depth < 2
            || match &compute.origin {
                ScalarPeerSourceV1::Compute => {
                    compute.preserves_frame && !self.backend.supports_ordered_compute_peer_copy_v1()
                }
                ScalarPeerSourceV1::SegmentedFrame(frame) => {
                    !self.backend.supports_pending_segment_frame_peer_copy_v1()
                        || !frame.covers_v1(root.source)
                        || compute.predecessor.is_some()
                        || !compute.preserves_frame
                }
            }
            || compute.predecessor.is_some() && !compute.preserves_frame
            || compute.state.depth > MAX_RUNTIME_DEPENDENCIES_V1
            || compute.state.cursor > root.dependencies.len()
            || compute.state.terminal == Some(BackendPollV1::Pending)
            || compute.state.terminal.is_none() && compute.state.cursor != 0
            || !root.dependencies.contains(&compute.producer)
            || compute.predecessor.is_some_and(|index| {
                root.dependencies.get(index).is_none_or(|dependency| {
                    dependency == &compute.producer
                        || dependency.device != root.destination.record.device
                })
            })
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
                match &compute.origin {
                    ScalarPeerSourceV1::Compute => {
                        self.validate_producer_launch_custody_v1(dependency.submission)?;
                        if !record.producer_launch
                            || self
                                .producer_launches
                                .get(&dependency.submission)
                                .is_none_or(|launch| !exact_written_source(launch, root.source))
                        {
                            return Err(invalid);
                        }
                    }
                    ScalarPeerSourceV1::SegmentedFrame(frame) => {
                        self.validate_segmented_peer_custody_v1(dependency.submission)?;
                        if !record.segmented_peer_copy
                            || self
                                .segmented_peer_copies
                                .get(&dependency.submission)
                                .is_none_or(|peer| !frame.matches_v1(peer, root.source))
                        {
                            return Err(invalid);
                        }
                    }
                }
            } else if compute.predecessor == Some(index) {
                if !record.scalar_peer_copy
                    || record.directed_peer_copy
                    || !self.compute_peer_predecessor_matches_v1(root, dependency)
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
