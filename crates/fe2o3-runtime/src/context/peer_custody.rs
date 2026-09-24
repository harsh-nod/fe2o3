//! Scalar operation provenance and dependency custody, not success-gated execution.

use super::*;

pub(super) struct PreparedPeerSubmissionV1 {
    pub(super) mechanism: PeerTransferMechanismV1,
    pub(super) scalar: Option<ScalarPeerCopyRootV1>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ScalarPeerDependencyV1 {
    pub(super) ordinal: usize,
    pub(super) event: RuntimeEventIdV1,
    pub(super) backend_event: u64,
    pub(super) submission: RuntimeSubmissionIdV1,
    pub(super) backend_submission: u64,
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) device: RuntimeDeviceIdV1,
}

pub(super) struct ScalarPeerCopyRootV1 {
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) backend_stream: u64,
    pub(super) source: ContextReadSourceV1,
    pub(super) destination: ContextReadSourceV1,
    // Canonical producer/event order; ordinal preserves the original backend roster.
    pub(super) dependencies: Vec<ScalarPeerDependencyV1>,
    pub(super) backend_submission: Option<u64>,
    pub(super) dependencies_held: bool,
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(super) fn prepare_scalar_peer_custody_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<ScalarPeerCopyRootV1, RuntimeValidationErrorV1> {
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies);
        }
        let stream_record = *self.unheld_stream_v1(stream)?;
        let source = ContextReadSourceV1 {
            region: source,
            record: *self
                .allocations
                .get(&source.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?,
        };
        let destination = ContextReadSourceV1 {
            region: destination,
            record: *self
                .allocations
                .get(&destination.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?,
        };
        let mut roster = Vec::new();
        roster
            .try_reserve_exact(dependencies.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        for (ordinal, id) in dependencies.iter().copied().enumerate() {
            let event = self
                .events
                .get(&id)
                .ok_or(RuntimeValidationErrorV1::UnknownEvent)?;
            let producer = self
                .submissions
                .get(&event.submission)
                .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
            self.require_ordinary_submission_v1(event.submission)?;
            if event.device != producer.device
                || !self.backend_events.contains(&event.backend_event)
                || !self
                    .backend_submissions
                    .contains(&producer.backend_submission)
            {
                return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
            }
            roster.push(ScalarPeerDependencyV1 {
                ordinal,
                event: id,
                backend_event: event.backend_event,
                submission: event.submission,
                backend_submission: producer.backend_submission,
                stream: producer.stream,
                device: producer.device,
            });
        }
        roster.sort_unstable_by_key(|dependency| (dependency.submission, dependency.event));
        for group in roster.chunk_by(|left, right| left.submission == right.submission) {
            self.submissions[&group[0].submission]
                .dependency_retains
                .checked_add(group.len())
                .ok_or(RuntimeValidationErrorV1::Capacity)?;
        }
        self.scalar_peer_copies
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        Ok(ScalarPeerCopyRootV1 {
            stream,
            backend_stream: stream_record.backend_stream,
            source,
            destination,
            dependencies: roster,
            backend_submission: None,
            dependencies_held: true,
        })
    }

    pub(super) fn begin_scalar_peer_custody_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
        root: ScalarPeerCopyRootV1,
    ) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            assert!(
                !self.scalar_peer_copies.contains_key(&id),
                "fresh scalar root"
            );
            assert!(
                self.scalar_peer_copies.len() < self.scalar_peer_copies.capacity(),
                "preallocated scalar root"
            );
            self.scalar_peer_copies.insert(id, root);
            for dependency in &self.scalar_peer_copies[&id].dependencies {
                let producer = self
                    .submissions
                    .get_mut(&dependency.submission)
                    .expect("preflighted producer");
                producer.dependency_retains = producer
                    .dependency_retains
                    .checked_add(1)
                    .expect("preflighted retain count");
            }
        }));
        if let Err(payload) = result {
            self.quarantine_after_async_command_panic_v1();
            std::panic::resume_unwind(payload);
        }
    }

    pub(super) fn validate_scalar_peer_custody_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        let record = self.submissions.get(&id);
        let Some(root) = self.scalar_peer_copies.get(&id) else {
            return if record.is_some_and(|record| record.scalar_peer_copy) {
                Err(invalid)
            } else {
                Ok(())
            };
        };
        if id.context_generation != self.context_generation
            || root.stream.context_generation != self.context_generation
            || root.backend_stream == 0
            || root.source.record.device == root.destination.record.device
            || root.source.region.byte_len != root.destination.region.byte_len
            || !matches!(
                root.source.region.access,
                RuntimeAccessV1::Read | RuntimeAccessV1::ReadWrite
            )
            || !matches!(
                root.destination.region.access,
                RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
            )
            || root.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
        {
            return Err(invalid);
        }
        for allocation in [&root.source, &root.destination] {
            if allocation.region.allocation.context_generation != self.context_generation
                || allocation.record.device.context_generation != self.context_generation
                || allocation.record.backend_allocation == 0
                || allocation.region.byte_len == 0
                || allocation
                    .region
                    .byte_offset
                    .checked_add(allocation.region.byte_len)
                    .is_none_or(|end| end > allocation.record.byte_len)
            {
                return Err(invalid);
            }
        }
        match record {
            Some(record)
                if record.scalar_peer_copy
                    && root.backend_submission == Some(record.backend_submission)
                    && root.stream == record.stream
                    && root.destination.record.device == record.device
                    && root.dependencies_held != record.quiescent => {}
            None if root.backend_submission.is_none() && root.dependencies_held => {}
            _ => return Err(invalid),
        }
        if !root.dependencies_held {
            return Ok(());
        }
        let mut ordinals = [false; MAX_RUNTIME_DEPENDENCIES_V1];
        let mut previous = None;
        for dependency in &root.dependencies {
            let key = (dependency.submission, dependency.event);
            if previous.is_some_and(|previous| previous >= key)
                || dependency.ordinal >= root.dependencies.len()
                || ordinals[dependency.ordinal]
                || dependency.event.context_generation != self.context_generation
                || dependency.backend_event == 0
                || dependency.submission.context_generation != self.context_generation
                || dependency.submission.local >= id.local
                || (dependency.device != root.source.record.device
                    && dependency.device != root.destination.record.device)
            {
                return Err(invalid);
            }
            previous = Some(key);
            ordinals[dependency.ordinal] = true;
            let producer = self
                .submissions
                .get(&dependency.submission)
                .ok_or(invalid)?;
            if producer.backend_submission != dependency.backend_submission
                || producer.stream != dependency.stream
                || producer.device != dependency.device
                || !self
                    .backend_submissions
                    .contains(&dependency.backend_submission)
            {
                return Err(invalid);
            }
        }
        for group in root
            .dependencies
            .chunk_by(|left, right| left.submission == right.submission)
        {
            if self.submissions[&group[0].submission].dependency_retains < group.len() {
                return Err(invalid);
            }
        }
        Ok(())
    }

    pub(super) fn check_scalar_peer_custody_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let result = self.validate_scalar_peer_custody_v1(id);
        if result.is_err() {
            self.quarantine_after_async_command_panic_v1();
        }
        result
    }

    pub(super) fn release_scalar_peer_dependencies_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        // Validate all decrements before mutating any count. Public events and
        // producer streams may already have been released legitimately.
        self.check_scalar_peer_custody_v1(id)?;
        let Some(root) = self.scalar_peer_copies.get_mut(&id) else {
            return Ok(());
        };
        if !root.dependencies_held {
            return Ok(());
        }
        for dependency in &root.dependencies {
            self.submissions
                .get_mut(&dependency.submission)
                .expect("retained producer")
                .dependency_retains -= 1;
        }
        root.dependencies_held = false;
        Ok(())
    }
}
