//! Same-device readback provenance, separate from scalar peers and typed launches.

use super::peer_custody::ScalarPeerDependencyV1;
use super::peer_reconciliation::DirectedPeerStateV1;
use super::*;

pub(super) struct SameDeviceCopyRootV1 {
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) backend_stream: u64,
    pub(super) source: ContextReadSourceV1,
    pub(super) destination: ContextReadSourceV1,
    pub(super) producer: ScalarPeerDependencyV1,
    pub(super) dependencies: Vec<ScalarPeerDependencyV1>,
    pub(super) backend_submission: Option<u64>,
    pub(super) dependencies_held: bool,
    pub(super) state: DirectedPeerStateV1,
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn prepare_same_device_copy_custody_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<Option<SameDeviceCopyRootV1>, RuntimeValidationErrorV1> {
        if self.versions.is_none() {
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
        if source.region.access != RuntimeAccessV1::Read
            || destination.region.access != RuntimeAccessV1::Write
            || source.record.kind != RuntimeMemoryKindV1::DeviceLocal
            || destination.record.kind != RuntimeMemoryKindV1::HostVisible
        {
            return Ok(None);
        }
        let selected = dependencies.iter().find_map(|event| {
            let submission = self.events[event].submission;
            let record = &self.submissions[&submission];
            self.scalar_peer_copies
                .get(&submission)
                .filter(|peer| {
                    peer.directed.is_none()
                        && record.status == RuntimeCompletionStatusV1::Pending
                        && peer.covers_input_v1(source)
                })
                .map(|_| submission)
        });
        let Some(selected) = selected else {
            return Ok(None);
        };
        let dependencies = self.prepare_dependency_roster_v1(dependencies)?;
        let mut depth = 1;
        for (index, dependency) in dependencies.iter().enumerate() {
            if index != 0 && dependencies[index - 1].submission == dependency.submission {
                return Err(RuntimeValidationErrorV1::DuplicateDependency);
            }
            self.check_operation_custody_v1(dependency.submission)?;
            let record = self.submissions[&dependency.submission];
            if dependency.device != source.record.device {
                return Err(RuntimeValidationErrorV1::WrongDevice);
            }
            match record.status {
                RuntimeCompletionStatusV1::Succeeded if record.quiescent => {}
                RuntimeCompletionStatusV1::Pending
                    if !record.quiescent
                        && record.scalar_peer_copy
                        && !record.directed_peer_copy
                        && !record.producer_launch
                        && !record.same_device_copy => {}
                _ => return Err(RuntimeValidationErrorV1::ContextReserved),
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
        let producer = *dependencies
            .iter()
            .find(|dependency| dependency.submission == selected)
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        let stream_record = *self.unheld_stream_v1(stream)?;
        self.same_device_copies
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        Ok(Some(SameDeviceCopyRootV1 {
            stream,
            backend_stream: stream_record.backend_stream,
            source,
            destination,
            producer,
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

    pub(super) fn begin_same_device_copy_custody_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        root: SameDeviceCopyRootV1,
    ) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            assert!(
                !self.same_device_copies.contains_key(&id),
                "fresh copy root"
            );
            assert!(
                self.same_device_copies.len() < self.same_device_copies.capacity(),
                "preallocated copy root"
            );
            self.same_device_copies.insert(id, root);
            for dependency in &self.same_device_copies[&id].dependencies {
                let producer = self
                    .submissions
                    .get_mut(&dependency.submission)
                    .expect("preflighted producer");
                producer.dependency_retains = producer
                    .dependency_retains
                    .checked_add(1)
                    .expect("preflighted copy retain count");
            }
        }));
        if let Err(payload) = result {
            self.quarantine_after_async_command_panic_v1();
            std::panic::resume_unwind(payload);
        }
    }

    pub(super) fn validate_same_device_copy_custody_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        let record = self.submissions.get(&id);
        let Some(root) = self.same_device_copies.get(&id) else {
            return if record.is_some_and(|record| record.same_device_copy) {
                Err(invalid)
            } else {
                Ok(())
            };
        };
        if self.versions.is_none()
            || self.scalar_peer_copies.contains_key(&id)
            || self.producer_launches.contains_key(&id)
            || id.context_generation != self.context_generation
            || root.stream.context_generation != self.context_generation
            || root.backend_stream == 0
            || root.source.region.allocation == root.destination.region.allocation
            || root.source.record.device != root.destination.record.device
            || root.source.region.access != RuntimeAccessV1::Read
            || root.destination.region.access != RuntimeAccessV1::Write
            || root.source.record.kind != RuntimeMemoryKindV1::DeviceLocal
            || root.destination.record.kind != RuntimeMemoryKindV1::HostVisible
            || root.source.region.byte_len != root.destination.region.byte_len
            || root.dependencies.is_empty()
            || root.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || root.state.depth < 2
            || root.state.depth > MAX_RUNTIME_DEPENDENCIES_V1
            || root.state.cursor > root.dependencies.len()
            || root.state.terminal == Some(BackendPollV1::Pending)
            || root.state.terminal.is_none() && root.state.cursor != 0
        {
            return Err(invalid);
        }
        match record {
            Some(record)
                if record.same_device_copy
                    && !record.scalar_peer_copy
                    && !record.directed_peer_copy
                    && !record.producer_launch
                    && root.backend_submission == Some(record.backend_submission)
                    && record.stream == root.stream
                    && record.device == root.source.record.device
                    && root.dependencies_held != record.quiescent => {}
            None if root.backend_submission.is_none() && root.dependencies_held => {}
            _ => return Err(invalid),
        }
        if !root.dependencies_held {
            return Ok(());
        }
        if self.streams.get(&root.stream).is_none_or(|stream| {
            stream.backend_stream != root.backend_stream
                || stream.device != root.source.record.device
        }) {
            return Err(invalid);
        }
        for endpoint in [root.source, root.destination] {
            if endpoint.region.allocation.context_generation != self.context_generation
                || endpoint.region.byte_len == 0
                || endpoint
                    .region
                    .byte_offset
                    .checked_add(endpoint.region.byte_len)
                    .is_none_or(|end| end > endpoint.record.byte_len)
                || self.allocations.get(&endpoint.region.allocation) != Some(&endpoint.record)
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
        let mut ordinals = [false; MAX_RUNTIME_DEPENDENCIES_V1];
        let mut previous = None;
        let mut depth = 1;
        for (index, dependency) in root.dependencies.iter().enumerate() {
            self.validate_completion_parent_rank_v1(id, dependency.submission, root.state.depth)?;
            let producer = self
                .submissions
                .get(&dependency.submission)
                .ok_or(invalid)?;
            if previous.is_some_and(|previous| previous >= dependency.submission)
                || dependency.ordinal >= root.dependencies.len()
                || ordinals[dependency.ordinal]
                || dependency.event.context_generation != self.context_generation
                || dependency.backend_event == 0
                || dependency.submission.context_generation != self.context_generation
                || dependency.submission.local >= id.local
                || dependency.device != root.source.record.device
                || producer.device != dependency.device
                || producer.stream != dependency.stream
                || producer.backend_submission != dependency.backend_submission
                || producer.dependency_retains == 0
                || !self
                    .backend_submissions
                    .contains(&dependency.backend_submission)
                || producer.status == RuntimeCompletionStatusV1::Succeeded && !producer.quiescent
                || index < root.state.cursor
                    && producer.status != RuntimeCompletionStatusV1::Succeeded
            {
                return Err(invalid);
            }
            if producer.scalar_peer_copy && !producer.directed_peer_copy {
                self.validate_scalar_peer_custody_v1(dependency.submission)?;
            } else if producer.status != RuntimeCompletionStatusV1::Succeeded || !producer.quiescent
            {
                return Err(invalid);
            }
            let parent = self
                .completion_parent_depth_v1(dependency.submission)
                .map_err(|_| invalid)?;
            if parent == 0 || parent >= root.state.depth {
                return Err(invalid);
            }
            depth = depth.max(parent + 1);
            ordinals[dependency.ordinal] = true;
            previous = Some(dependency.submission);
        }
        if depth != root.state.depth
            || !root.dependencies.contains(&root.producer)
            || self
                .scalar_peer_copies
                .get(&root.producer.submission)
                .is_none_or(|producer| {
                    producer.directed.is_some() || !producer.covers_input_v1(root.source)
                })
        {
            return Err(invalid);
        }
        Ok(())
    }

    pub(super) fn release_same_device_copy_dependencies_v1(&mut self, id: RuntimeSubmissionIdV1) {
        if let Some(root) = self.same_device_copies.get_mut(&id)
            && root.dependencies_held
        {
            for dependency in &root.dependencies {
                self.submissions
                    .get_mut(&dependency.submission)
                    .expect("retained copy producer")
                    .dependency_retains -= 1;
            }
            root.dependencies_held = false;
        }
    }
}
