//! Immutable list provenance. Bounding envelopes are not scalar write coverage.

use super::*;
use crate::context::peer_custody::ScalarPeerDependencyV1;
use crate::context::peer_reconciliation::DirectedPeerStateV1;

struct SegmentedPeerPlanV1 {
    stream: RuntimeStreamIdV1,
    backend_stream: u64,
    source: ContextReadSourceV1,
    destination: ContextReadSourceV1,
    identity: IdentityDigestV1,
    origin: SegmentedPeerSourceV1,
    predecessor: Option<(ScalarPeerDependencyV1, std::sync::Arc<SegmentedPeerPlanV1>)>,
    _segments: std::sync::Arc<Vec<RuntimePeerCopySegmentV1>>,
}

pub(in crate::context) struct SegmentedPeerCopyRootV1 {
    pub(in crate::context) stream: RuntimeStreamIdV1,
    pub(in crate::context) backend_stream: u64,
    pub(in crate::context) source: ContextReadSourceV1,
    pub(in crate::context) destination: ContextReadSourceV1,
    pub(in crate::context) identity: IdentityDigestV1,
    plan: std::sync::Arc<SegmentedPeerPlanV1>,
    pub(in crate::context) origin: SegmentedPeerSourceV1,
    pub(in crate::context) predecessor: Option<ScalarPeerDependencyV1>,
    pub(in crate::context) dependencies: Vec<ScalarPeerDependencyV1>,
    pub(in crate::context) backend_submission: Option<u64>,
    pub(in crate::context) dependencies_held: bool,
    pub(in crate::context) state: DirectedPeerStateV1,
}

impl SegmentedPeerCopyRootV1 {
    pub(in crate::context) fn destination_predecessor_v1(&self) -> Option<ScalarPeerDependencyV1> {
        self.predecessor
    }

    pub(in crate::context) fn compute_producer_v1(&self) -> Option<ScalarPeerDependencyV1> {
        match self.origin {
            SegmentedPeerSourceV1::Settled => None,
            SegmentedPeerSourceV1::Compute(producer) => Some(producer),
        }
    }

    pub(in crate::context) fn preserves_destination_frame_v1(
        &self,
        source: ContextReadSourceV1,
    ) -> bool {
        self.source.region == self.plan.source.region
            && self.source.record == self.plan.source.record
            && self.destination.region == self.plan.destination.region
            && self.destination.record == self.plan.destination.record
            && self.stream == self.plan.stream
            && self.backend_stream == self.plan.backend_stream
            && self.identity == self.plan.identity
            && self.origin == self.plan.origin
            && self.predecessor
                == self
                    .plan
                    .predecessor
                    .as_ref()
                    .map(|(dependency, _)| *dependency)
            && source.record == self.destination.record
            && source.region.allocation == self.destination.region.allocation
            && source.region.access == RuntimeAccessV1::Read
            && source.region.byte_len != 0
            && source
                .region
                .byte_offset
                .checked_add(source.region.byte_len)
                .is_some_and(|end| end <= self.destination.record.byte_len)
    }
}

fn exact_source(launch: &ProducerLaunchRootV1, source: ContextReadSourceV1) -> bool {
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
    pub(in crate::context) fn segmented_predecessor_matches_v1(
        &self,
        stream: RuntimeStreamIdV1,
        destination: ContextReadSourceV1,
        dependency: ScalarPeerDependencyV1,
    ) -> bool {
        self.backend.supports_ordered_peer_copy_segments_v1()
            && self.backend.supports_peer_copy_segments_frame_v1()
            && self
                .submissions
                .get(&dependency.submission)
                .is_some_and(|record| {
                    record.segmented_peer_copy
                        && record.backend_submission == dependency.backend_submission
                        && record.stream == dependency.stream
                        && record.device == dependency.device
                })
            && self
                .segmented_peer_copies
                .get(&dependency.submission)
                .is_some_and(|previous| {
                    previous.origin == SegmentedPeerSourceV1::Settled
                        && previous.stream == stream
                        && dependency.stream == stream
                        && dependency.device == destination.record.device
                        && previous.destination.region.allocation == destination.region.allocation
                        && previous.destination.record == destination.record
                        && previous.preserves_destination_frame_v1(ContextReadSourceV1 {
                            region: RuntimeMemoryRegionV1 {
                                allocation: destination.region.allocation,
                                byte_offset: 0,
                                byte_len: destination.record.byte_len,
                                access: RuntimeAccessV1::Read,
                            },
                            record: destination.record,
                        })
                })
    }

    pub(in crate::context) fn prepare_segmented_peer_custody_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        segments: std::sync::Arc<Vec<RuntimePeerCopySegmentV1>>,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<Option<SegmentedPeerCopyRootV1>, RuntimeValidationErrorV1> {
        if self.versions.is_none()
            || !(self
                .backend
                .supports_pending_compute_peer_copy_segments_v1()
                || self.backend.supports_peer_copy_segments_frame_v1())
            || source.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
        {
            return Ok(None);
        }
        let source = ContextReadSourceV1 {
            region: source,
            record: self.allocations[&source.allocation],
        };
        let destination = ContextReadSourceV1 {
            region: destination,
            record: self.allocations[&destination.allocation],
        };
        if source.record.kind != RuntimeMemoryKindV1::DeviceLocal
            || destination.record.kind != RuntimeMemoryKindV1::DeviceLocal
        {
            return Ok(None);
        }
        let dependencies = self.prepare_dependency_roster_v1(dependencies)?;
        let producer = dependencies
            .iter()
            .find(|dependency| {
                self.submissions[&dependency.submission].status
                    == RuntimeCompletionStatusV1::Pending
                    && self
                        .producer_launches
                        .get(&dependency.submission)
                        .is_some_and(|launch| exact_source(launch, source))
            })
            .copied();
        let origin = match producer {
            Some(producer)
                if self
                    .backend
                    .supports_pending_compute_peer_copy_segments_v1() =>
            {
                SegmentedPeerSourceV1::Compute(producer)
            }
            None if self.backend.supports_peer_copy_segments_frame_v1() => {
                SegmentedPeerSourceV1::Settled
            }
            _ => return Ok(None),
        };
        let mut depth = 1;
        let mut predecessor = None;
        for (index, dependency) in dependencies.iter().enumerate() {
            if index > 0 && dependencies[index - 1].submission == dependency.submission {
                return Err(RuntimeValidationErrorV1::DuplicateDependency);
            }
            self.check_operation_custody_v1(dependency.submission)?;
            let record = self.submissions[&dependency.submission];
            if Some(*dependency) == producer {
                if dependency.device != source.record.device
                    || record.quiescent
                    || !record.producer_launch
                {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
            } else if origin == SegmentedPeerSourceV1::Settled
                && record.status == RuntimeCompletionStatusV1::Pending
                && !record.quiescent
                && self.segmented_predecessor_matches_v1(stream, destination, *dependency)
            {
                if predecessor.replace(*dependency).is_some() {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
            } else if record.status != RuntimeCompletionStatusV1::Succeeded || !record.quiescent {
                if origin == SegmentedPeerSourceV1::Settled {
                    // The legacy list still retains its controls, but this
                    // profile makes no pending-control provenance promise.
                    return Ok(None);
                }
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            depth = depth.max(
                self.completion_parent_depth_v1(dependency.submission)?
                    .checked_add(1)
                    .ok_or(RuntimeValidationErrorV1::Capacity)?,
            );
        }
        if depth > MAX_RUNTIME_DEPENDENCIES_V1 {
            if origin == SegmentedPeerSourceV1::Settled {
                return Ok(None);
            }
            return Err(RuntimeValidationErrorV1::TooManyDependencies);
        }
        self.segmented_peer_copies
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let identity = contract_identity(stream, source.region, destination.region, &segments);
        let backend_stream = self.streams[&stream].backend_stream;
        let plan = std::sync::Arc::new(SegmentedPeerPlanV1 {
            stream,
            backend_stream,
            source,
            destination,
            identity,
            origin,
            predecessor: predecessor.map(|dependency| {
                (
                    dependency,
                    std::sync::Arc::clone(&self.segmented_peer_copies[&dependency.submission].plan),
                )
            }),
            _segments: segments,
        });
        Ok(Some(SegmentedPeerCopyRootV1 {
            stream,
            backend_stream,
            identity,
            source,
            destination,
            plan,
            origin,
            predecessor,
            dependencies,
            backend_submission: None,
            dependencies_held: true,
            state: DirectedPeerStateV1 {
                depth,
                cursor: 0,
                terminal: None,
            },
        }))
    }

    pub(in crate::context) fn begin_segmented_peer_custody_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        root: SegmentedPeerCopyRootV1,
    ) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            assert!(!self.segmented_peer_copies.contains_key(&id));
            assert!(self.segmented_peer_copies.len() < self.segmented_peer_copies.capacity());
            self.segmented_peer_copies.insert(id, root);
            for dependency in &self.segmented_peer_copies[&id].dependencies {
                let producer = self
                    .submissions
                    .get_mut(&dependency.submission)
                    .expect("retained producer");
                producer.dependency_retains = producer
                    .dependency_retains
                    .checked_add(1)
                    .expect("preflighted retain");
            }
        }));
        if let Err(payload) = result {
            self.quarantine_after_async_command_panic_v1();
            std::panic::resume_unwind(payload);
        }
    }

    pub(in crate::context) fn validate_segmented_peer_custody_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        let record = self.submissions.get(&id);
        let Some(root) = self.segmented_peer_copies.get(&id) else {
            return if record.is_some_and(|record| record.segmented_peer_copy) {
                Err(invalid)
            } else {
                Ok(())
            };
        };
        if self.versions.is_none()
            || match root.origin {
                SegmentedPeerSourceV1::Settled => {
                    !self.backend.supports_peer_copy_segments_frame_v1()
                }
                SegmentedPeerSourceV1::Compute(_) => !self
                    .backend
                    .supports_pending_compute_peer_copy_segments_v1(),
            }
            || self.scalar_peer_copies.contains_key(&id)
            || self.same_device_copies.contains_key(&id)
            || self.producer_launches.contains_key(&id)
            || id.context_generation != self.context_generation
            || root.stream.context_generation != self.context_generation
            || root.backend_stream == 0
            || root.backend_stream != root.plan.backend_stream
            || root.source.record.device == root.destination.record.device
            || root.source.region.access != RuntimeAccessV1::Read
            || root.destination.region.access != RuntimeAccessV1::Write
            || root.state.depth == 0
            || root.compute_producer_v1().is_some() && root.state.depth < 2
            || root.state.depth > MAX_RUNTIME_DEPENDENCIES_V1
            || root.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || root.state.cursor > root.dependencies.len()
            || root.state.terminal == Some(BackendPollV1::Pending)
            || root.state.terminal.is_none() && root.state.cursor != 0
            || root
                .compute_producer_v1()
                .is_some_and(|producer| !root.dependencies.contains(&producer))
            || root.origin != root.plan.origin
            || root.predecessor
                != root
                    .plan
                    .predecessor
                    .as_ref()
                    .map(|(dependency, _)| *dependency)
            || root.predecessor.is_some_and(|dependency| {
                root.origin != SegmentedPeerSourceV1::Settled
                    || !self.backend.supports_ordered_peer_copy_segments_v1()
                    || root.state.depth < 2
                    || !root.dependencies.contains(&dependency)
            })
            || root.identity != root.plan.identity
            || root.stream != root.plan.stream
            || root.source.region != root.plan.source.region
            || root.source.record != root.plan.source.record
            || root.destination.region != root.plan.destination.region
            || root.destination.record != root.plan.destination.record
        {
            return Err(invalid);
        }
        match record {
            Some(record)
                if record.segmented_peer_copy
                    && !record.scalar_peer_copy
                    && !record.directed_peer_copy
                    && !record.producer_launch
                    && !record.same_device_copy
                    && root.backend_submission == Some(record.backend_submission)
                    && root.stream == record.stream
                    && root.destination.record.device == record.device
                    && root.dependencies_held != record.quiescent
                    && (record.status != RuntimeCompletionStatusV1::Succeeded
                        || record.quiescent
                            && root.state.terminal == Some(BackendPollV1::Succeeded)
                            && root.state.cursor == root.dependencies.len()) => {}
            None if root.backend_submission.is_none() && root.dependencies_held => {}
            _ => return Err(invalid),
        }
        for endpoint in [root.source, root.destination] {
            if endpoint.region.allocation.context_generation != self.context_generation
                || endpoint.record.device.context_generation != self.context_generation
                || endpoint.record.backend_allocation == 0
                || endpoint.record.kind != RuntimeMemoryKindV1::DeviceLocal
                || endpoint.region.byte_len == 0
                || endpoint
                    .region
                    .byte_offset
                    .checked_add(endpoint.region.byte_len)
                    .is_none_or(|end| end > endpoint.record.byte_len)
            {
                return Err(invalid);
            }
            if root.dependencies_held
                && (self.allocations.get(&endpoint.region.allocation) != Some(&endpoint.record)
                    || !self
                        .backend_allocations
                        .contains(&endpoint.record.backend_allocation)
                    || !self.allocation_admission.has_expected_credit(
                        endpoint.region.allocation,
                        endpoint.record.device,
                        endpoint.record.byte_len,
                    ))
            {
                return Err(invalid);
            }
        }
        if !root.dependencies_held {
            return Ok(());
        }
        if self.streams.get(&root.stream).is_none_or(|stream| {
            stream.backend_stream != root.backend_stream
                || stream.device != root.destination.record.device
        }) {
            return Err(invalid);
        }
        let mut depth = 1;
        let mut ordinals = [false; MAX_RUNTIME_DEPENDENCIES_V1];
        let mut previous = None;
        for (index, dependency) in root.dependencies.iter().enumerate() {
            self.validate_completion_parent_rank_v1(id, dependency.submission, root.state.depth)?;
            let parent = self
                .submissions
                .get(&dependency.submission)
                .ok_or(invalid)?;
            if previous.is_some_and(|prior| prior >= dependency.submission)
                || dependency.ordinal >= root.dependencies.len()
                || ordinals[dependency.ordinal]
                || dependency.event.context_generation != self.context_generation
                || dependency.backend_event == 0
                || parent.backend_submission != dependency.backend_submission
                || parent.stream != dependency.stream
                || parent.device != dependency.device
                || !self
                    .backend_submissions
                    .contains(&dependency.backend_submission)
                || parent.dependency_retains == 0
                || parent.status == RuntimeCompletionStatusV1::Succeeded && !parent.quiescent
                || index < root.state.cursor
                    && parent.status != RuntimeCompletionStatusV1::Succeeded
            {
                return Err(invalid);
            }
            previous = Some(dependency.submission);
            ordinals[dependency.ordinal] = true;
            if Some(*dependency) == root.compute_producer_v1() {
                self.validate_producer_launch_custody_v1(dependency.submission)?;
                if dependency.device != root.source.record.device
                    || !parent.producer_launch
                    || self
                        .producer_launches
                        .get(&dependency.submission)
                        .is_none_or(|launch| !exact_source(launch, root.source))
                {
                    return Err(invalid);
                }
            } else if Some(*dependency) == root.predecessor {
                if !self.segmented_predecessor_matches_v1(
                    root.stream,
                    root.destination,
                    *dependency,
                ) || root.plan.predecessor.as_ref().is_none_or(|(_, plan)| {
                    self.segmented_peer_copies
                        .get(&dependency.submission)
                        .is_none_or(|previous| !std::sync::Arc::ptr_eq(plan, &previous.plan))
                }) {
                    return Err(invalid);
                }
            } else if parent.status != RuntimeCompletionStatusV1::Succeeded || !parent.quiescent {
                return Err(invalid);
            }
            depth = depth.max(
                self.completion_parent_depth_v1(dependency.submission)?
                    .checked_add(1)
                    .ok_or(invalid)?,
            );
        }
        if depth != root.state.depth {
            return Err(invalid);
        }
        Ok(())
    }

    pub(in crate::context) fn release_segmented_peer_dependencies_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        if let Some(root) = self.segmented_peer_copies.get_mut(&id)
            && root.dependencies_held
        {
            for dependency in &root.dependencies {
                self.submissions
                    .get_mut(&dependency.submission)
                    .expect("retained producer")
                    .dependency_retains -= 1;
            }
            root.dependencies_held = false;
        }
    }
}
