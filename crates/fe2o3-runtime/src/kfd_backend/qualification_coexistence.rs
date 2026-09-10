//! Qualification-only observation. These digests never grant runtime authority.
use super::*;
use fe2o3_kfd::Gfx942R66NativeObservationFailureV1;
use sha2::{Digest, Sha256};

/// Address-free qualification data, not a publication or retirement capability.
/// Native digests bind retained receipts; membership digests bind their current
/// runtime owners. Neither establishes unfinished GPU work or physical overlap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdR66RetainedCustodyObservationV1 {
    pub compute: Option<[u8; 32]>,
    pub compute_membership: Option<[u8; 32]>,
    pub copy: Option<[u8; 32]>,
    pub copy_membership: Option<[u8; 32]>,
    pub copy_packets: usize,
}

/// Fixed, address-free failure stages for an immutable qualification observation.
/// These diagnostics neither grant authority nor change publication or retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdR66RetainedCustodyObservationFailureV1 {
    ShapeTerminal,
    ShapeQueueRetired,
    ShapeTerminalMemory,
    ShapeTerminalSdmaCustody,
    ShapeUnsupportedLaunchGate,
    ShapePendingCompute,
    ShapeComputePipeline,
    ShapeAuxiliaryCompute,
    ShapeCopyCount,
    ShapePublishedCopyCount,
    ShapeUnaccountedSubmission,
    ShapeUnaccountedComputeStorage,
    ShapeUnaccountedCopyStorage,
    ShapeSynchronousStorage,
    CountsQueueUnavailable,
    CountsNative(Gfx942R66NativeObservationFailureV1),
    CountsComputeMismatch,
    CountsCopyMismatch,
    ComputeExecution,
    /// The borrowed dispatch is not this backend's exact nonzero active owner.
    ComputeSubmissionMissing,
    ComputeStreamMismatch,
    /// Active ownership conflicts with another lifecycle index, including a result-table entry.
    ComputeStatus,
    ComputeAllocationCount,
    ComputeAllocationMembership,
    ComputeAllocationMissing,
    ComputeCustody,
    ComputeStorage,
    ComputeNative(Gfx942R66NativeObservationFailureV1),
    /// The borrowed copy is not this backend's exact nonzero active owner.
    CopySubmissionMissing,
    CopyIdMismatch,
    CopyStreamMismatch,
    /// Active ownership conflicts with another lifecycle index, including a result-table entry.
    CopyStatus,
    CopyPublishedRoster,
    CopyPartialCompletion,
    CopyWindowBytes,
    CopySourceAllocationMissing,
    CopyDestinationAllocationMissing,
    CopySourceCustody,
    CopyDestinationCustody,
    CopySourceStorage,
    CopyDestinationStorage,
    CopyComputeAlias,
    CopyPhase,
    CopyNonNativeOwner,
    CopyHostWrapperOffset,
    CopyDeviceWrapperOffset,
    CopySingleNative(Gfx942R66NativeObservationFailureV1),
    CopyWindowNative(Gfx942R66NativeObservationFailureV1),
    CopySourceOffset,
    CopyDestinationOffset,
    CopyBytes,
}

impl fmt::Display for KfdR66RetainedCustodyObservationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KfdR66RetainedCustodyObservationFailureV1 {}

fn membership(kind: &[u8], native: [u8; 32], coordinates: &[u64]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3.r66.runtime-retained-membership.v1\0");
    hash.update(kind);
    hash.update(native);
    for coordinate in coordinates {
        hash.update(coordinate.to_le_bytes());
    }
    hash.finalize().into()
}

impl KfdRuntimeBackendV1 {
    fn r66_runtime_roster_shape_v1(&self) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        for (rejected, failure) in [
            (self.terminal, Failure::ShapeTerminal),
            (self.queue_retired, Failure::ShapeQueueRetired),
            (self.terminal_memory.is_some(), Failure::ShapeTerminalMemory),
            (
                self.terminal_sdma_custody.is_some(),
                Failure::ShapeTerminalSdmaCustody,
            ),
            (
                !matches!(
                    self.launch_gate,
                    KfdRuntimeLaunchGateV1::ExactGfx942InplaceTransform(_)
                ),
                Failure::ShapeUnsupportedLaunchGate,
            ),
            (
                !self.pending_compute.is_empty(),
                Failure::ShapePendingCompute,
            ),
            (
                !self.compute_pipeline.is_empty(),
                Failure::ShapeComputePipeline,
            ),
            (
                self.auxiliary_compute_lanes
                    .iter()
                    .any(|lane| lane.active.is_some() || !lane.pipeline.is_empty()),
                Failure::ShapeAuxiliaryCompute,
            ),
            (self.active_sdma.len() > 1, Failure::ShapeCopyCount),
            (
                self.published_sdma_submissions.len() != self.active_sdma.len(),
                Failure::ShapePublishedCopyCount,
            ),
        ] {
            if rejected {
                return Err(failure);
            }
        }
        for (&id, record) in &self.submissions {
            if matches!(record.status, BackendPollV1::Pending)
                && self.active.as_ref().is_none_or(|active| active.id != id)
                && !self.active_sdma.contains_key(&id)
            {
                return Err(Failure::ShapeUnaccountedSubmission);
            }
        }
        for (&allocation, record) in &self.allocations {
            match record.sdma_storage {
                KfdRuntimeSdmaStorageV1::ComputeInFlight(id)
                    if !self.active.as_ref().is_some_and(|active| {
                        active.id == id && active.allocations.contains(&allocation)
                    }) =>
                {
                    return Err(Failure::ShapeUnaccountedComputeStorage);
                }
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id))
                    if !self.active_sdma.get(&id).is_some_and(|copy| {
                        copy.source == allocation || copy.destination == allocation
                    }) =>
                {
                    return Err(Failure::ShapeUnaccountedCopyStorage);
                }
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous) => {
                    return Err(Failure::ShapeSynchronousStorage);
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn r66_runtime_native_counts_v1(
        &self,
        native_counts: (usize, usize),
    ) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        if native_counts.0 != usize::from(self.active.is_some()) {
            return Err(Failure::CountsComputeMismatch);
        }
        if native_counts.1 != self.active_sdma.len() {
            return Err(Failure::CountsCopyMismatch);
        }
        Ok(())
    }

    fn r66_compute_membership_v1(
        &self,
        active: &ActiveSubmissionV1,
        allocation: u64,
    ) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        if active.id == 0
            || self
                .active
                .as_ref()
                .is_none_or(|retained| !core::ptr::eq(retained, active))
        {
            return Err(Failure::ComputeSubmissionMissing);
        }
        // Pending work lives in the move-only active owner, not the terminal
        // submission table. Any table entry would mask that owner in poll_v1.
        if self.submissions.contains_key(&active.id)
            || self.active_sdma.contains_key(&active.id)
            || self.pending_compute.contains_key(&active.id)
        {
            return Err(Failure::ComputeStatus);
        }
        let device = self
            .streams
            .get(&active.stream)
            .ok_or(Failure::ComputeStreamMismatch)?;
        if active.allocations.len() != 1 {
            return Err(Failure::ComputeAllocationCount);
        }
        if !active.allocations.contains(&allocation) {
            return Err(Failure::ComputeAllocationMembership);
        }
        let record = self
            .allocations
            .get(&allocation)
            .ok_or(Failure::ComputeAllocationMissing)?;
        if record.device != *device {
            return Err(Failure::ComputeStreamMismatch);
        }
        let owner = RuntimeAllocationCustodyOwnerV1 {
            submission: active.id,
            stream: active.stream,
            kind: RuntimeAllocationCustodyKindV1::Compute,
        };
        if !self.allocation_retains_exact_owner_v1(allocation, owner) {
            return Err(Failure::ComputeCustody);
        }
        if !matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == active.id)
        {
            return Err(Failure::ComputeStorage);
        }
        Ok(())
    }

    fn r66_copy_membership_v1(
        &self,
        id: u64,
        copy: &ActiveSdmaCopyV1,
    ) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        for (rejected, failure) in [
            (id != copy.id, Failure::CopyIdMismatch),
            (
                id == 0
                    || self
                        .active_sdma
                        .get(&id)
                        .is_none_or(|retained| !core::ptr::eq(retained, copy)),
                Failure::CopySubmissionMissing,
            ),
            (
                self.submissions.contains_key(&id)
                    || self.active.as_ref().is_some_and(|active| active.id == id)
                    || self.pending_compute.contains_key(&id),
                Failure::CopyStatus,
            ),
            (
                self.published_sdma_submissions.as_slice() != [id],
                Failure::CopyPublishedRoster,
            ),
            (copy.completed_bytes != 0, Failure::CopyPartialCompletion),
            (copy.window_bytes != copy.byte_len, Failure::CopyWindowBytes),
        ] {
            if rejected {
                return Err(failure);
            }
        }
        let device = self
            .streams
            .get(&copy.stream)
            .ok_or(Failure::CopyStreamMismatch)?;
        if self
            .active_sdma_streams
            .get(&copy.stream)
            .is_none_or(|ids| ids.iter().filter(|candidate| **candidate == id).count() != 1)
        {
            return Err(Failure::CopyStreamMismatch);
        }
        let owner = RuntimeAllocationCustodyOwnerV1 {
            submission: id,
            stream: copy.stream,
            kind: RuntimeAllocationCustodyKindV1::Sdma,
        };
        for (allocation, missing, custody, storage) in [
            (
                copy.source,
                Failure::CopySourceAllocationMissing,
                Failure::CopySourceCustody,
                Failure::CopySourceStorage,
            ),
            (
                copy.destination,
                Failure::CopyDestinationAllocationMissing,
                Failure::CopyDestinationCustody,
                Failure::CopyDestinationStorage,
            ),
        ] {
            let record = self.allocations.get(&allocation).ok_or(missing)?;
            if record.device != *device {
                return Err(Failure::CopyStreamMismatch);
            }
            if !self.allocation_retains_exact_owner_v1(allocation, owner) {
                return Err(custody);
            }
            if !matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(owner_id)) if owner_id == id)
            {
                return Err(storage);
            }
            if self
                .active
                .as_ref()
                .is_some_and(|active| active.allocations.contains(&allocation))
            {
                return Err(Failure::CopyComputeAlias);
            }
        }
        Ok(())
    }

    fn r66_copy_geometry_v1(
        copy: &ActiveSdmaCopyV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        bytes: u32,
    ) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        let (source_offset, destination_offset) = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => (host_offset, device_offset),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => (device_offset, host_offset),
        };
        if copy.source_offset != source_offset {
            return Err(Failure::CopySourceOffset);
        }
        if copy.destination_offset != destination_offset {
            return Err(Failure::CopyDestinationOffset);
        }
        if copy.byte_len != u64::from(bytes) {
            return Err(Failure::CopyBytes);
        }
        Ok(())
    }

    /// Observes one R26 compute and at most one directional copy without polling.
    /// A retained published receipt may already have completed on the GPU.
    pub fn observe_r66_retained_custody_v1(&self) -> Option<KfdR66RetainedCustodyObservationV1> {
        self.diagnose_r66_retained_custody_v1().ok()
    }

    /// Diagnoses the same retained roster without polling, publishing or releasing it.
    /// Scripted runtime shape and these failure stages never stand in for native receipts.
    pub fn diagnose_r66_retained_custody_v1(
        &self,
    ) -> Result<KfdR66RetainedCustodyObservationV1, KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        self.r66_runtime_roster_shape_v1()?;
        let queue = self.queue.as_ref().ok_or(Failure::CountsQueueUnavailable)?;
        let native_counts = queue
            .diagnose_r66_retained_counts_v1()
            .map_err(Failure::CountsNative)?;
        self.r66_runtime_native_counts_v1(native_counts)?;
        let mut observed = KfdR66RetainedCustodyObservationV1 {
            compute: None,
            compute_membership: None,
            copy: None,
            copy_membership: None,
            copy_packets: 0,
        };
        if let Some(active) = &self.active {
            let Some(ActiveComputeExecutionV1::Persistent {
                allocation,
                dispatch,
                ..
            }) = &active.execution
            else {
                return Err(Failure::ComputeExecution);
            };
            self.r66_compute_membership_v1(active, *allocation)?;
            let identity = queue
                .diagnose_r66_retained_compute_v1(dispatch)
                .map_err(Failure::ComputeNative)?;
            observed.compute = Some(identity);
            observed.compute_membership = Some(membership(
                b"compute\0",
                identity,
                &[active.id, active.stream, active.kernel, *allocation],
            ));
        }
        if let Some((&id, copy)) = self.active_sdma.iter().next() {
            let copy_observation = self.r66_copy_observation_v1(id, copy)?;
            observed.copy = copy_observation.copy;
            observed.copy_membership = copy_observation.copy_membership;
            observed.copy_packets = copy_observation.copy_packets;
        }
        Ok(observed)
    }

    pub(super) fn r66_copy_observation_v1(
        &self,
        id: u64,
        copy: &ActiveSdmaCopyV1,
    ) -> Result<KfdR66RetainedCustodyObservationV1, KfdR66RetainedCustodyObservationFailureV1> {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        let queue = self.queue.as_ref().ok_or(Failure::CountsQueueUnavailable)?;
        self.r66_copy_membership_v1(id, copy)?;
        let ActiveSdmaPhaseV1::DirectionalPublished(native) = &copy.phase else {
            return Err(Failure::CopyPhase);
        };
        let (identity, packets, direction, host_offset, device_offset, bytes) =
            match native.as_ref() {
                DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                    submission,
                    host_offset,
                    device_offset,
                } => {
                    if *host_offset != submission.host_offset() {
                        return Err(Failure::CopyHostWrapperOffset);
                    }
                    if *device_offset != submission.device_offset() {
                        return Err(Failure::CopyDeviceWrapperOffset);
                    }
                    (
                        queue
                            .diagnose_r66_retained_single_copy_v1(submission)
                            .map_err(Failure::CopySingleNative)?,
                        1,
                        submission.direction(),
                        *host_offset,
                        *device_offset,
                        submission.copy_bytes(),
                    )
                }
                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission } => (
                    queue
                        .diagnose_r66_retained_window_copy_v1(submission)
                        .map_err(Failure::CopyWindowNative)?,
                    submission.packet_count(),
                    submission.direction(),
                    submission.host_offset(),
                    submission.device_offset(),
                    submission.copy_bytes(),
                ),
                #[cfg(test)]
                _ => return Err(Failure::CopyNonNativeOwner),
            };
        Self::r66_copy_geometry_v1(copy, direction, host_offset, device_offset, bytes)?;
        let copy_membership = membership(
            b"copy\0",
            identity,
            &[
                id,
                copy.stream,
                copy.source,
                copy.destination,
                copy.source_offset,
                copy.destination_offset,
                copy.byte_len,
            ],
        );
        Ok(KfdR66RetainedCustodyObservationV1 {
            compute: None,
            compute_membership: None,
            copy: Some(identity),
            copy_membership: Some(copy_membership),
            copy_packets: packets,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualified_without_native_queue() -> KfdRuntimeBackendV1 {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942InplaceTransform(
            crate::qualification_gfx942_inplace_transform_v1::admit_gfx942_inplace_transform_qualification_v1().unwrap());
        backend
    }

    struct ScriptedR26RosterV1 {
        backend: KfdRuntimeBackendV1,
        allocation: u64,
        source: u64,
        destination: u64,
    }

    impl ScriptedR26RosterV1 {
        const COMPUTE: u64 = 101;
        const COPY: u64 = 102;

        fn new() -> Self {
            let mut backend = qualified_without_native_queue();
            let compute_stream = backend.create_stream_v1(7).unwrap();
            let copy_stream = backend.create_stream_v1(7).unwrap();
            let allocation = backend
                .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
                .unwrap();
            let source = backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 64, 8)
                .unwrap();
            let destination = backend
                .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 64, 8)
                .unwrap();
            // These are runtime-only scripted records, never fabricated native receipts.
            backend.active = Some(ActiveSubmissionV1 {
                id: Self::COMPUTE,
                stream: compute_stream,
                ordered_predecessor: None,
                deferred_ordered_predecessor_retain: false,
                kernel: 103,
                dependency_depth: 0,
                allocations: HashSet::from([allocation]),
                writebacks: Vec::new(),
                resident_descriptors: Vec::new(),
                ordinary_recipe: None,
                dispatch_shape_sha256: [0; 32],
                published_at: Instant::now(),
                performance: KfdRuntimeLaunchPerformanceV1::default(),
                execution: Some(ActiveComputeExecutionV1::ScriptedMaterialized),
            });
            backend.active_sdma.insert(
                Self::COPY,
                ActiveSdmaCopyV1 {
                    id: Self::COPY,
                    stream: copy_stream,
                    prior_stream_submission: None,
                    source,
                    destination,
                    source_offset: 8,
                    destination_offset: 16,
                    byte_len: 32,
                    completed_bytes: 0,
                    window_bytes: 32,
                    window_requests: None,
                    dependencies: Vec::new(),
                    dependency_cursor: 0,
                    dependency_depth: 0,
                    phase: ActiveSdmaPhaseV1::Ready,
                },
            );
            backend.published_sdma_submissions.push(Self::COPY);
            backend
                .active_sdma_streams
                .insert(copy_stream, VecDeque::from([Self::COPY]));
            for (submission, stream, kind, allocations) in [
                (
                    Self::COMPUTE,
                    compute_stream,
                    RuntimeAllocationCustodyKindV1::Compute,
                    vec![allocation],
                ),
                (
                    Self::COPY,
                    copy_stream,
                    RuntimeAllocationCustodyKindV1::Sdma,
                    vec![source, destination],
                ),
            ] {
                let reserved = backend.reserve_allocation_custody_v1(&allocations).unwrap();
                backend.retain_allocation_custody_v1(
                    &allocations,
                    RuntimeAllocationCustodyOwnerV1 {
                        submission,
                        stream,
                        kind,
                    },
                    reserved,
                );
                for allocation in allocations {
                    backend
                        .allocations
                        .get_mut(&allocation)
                        .unwrap()
                        .sdma_storage = match kind {
                        RuntimeAllocationCustodyKindV1::Compute => {
                            KfdRuntimeSdmaStorageV1::ComputeInFlight(submission)
                        }
                        RuntimeAllocationCustodyKindV1::Sdma => KfdRuntimeSdmaStorageV1::InFlight(
                            KfdRuntimeSdmaInFlightV1::Async(submission),
                        ),
                    };
                }
            }
            Self {
                backend,
                allocation,
                source,
                destination,
            }
        }

        fn compute_membership(&self) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
            self.backend
                .r66_compute_membership_v1(self.backend.active.as_ref().unwrap(), self.allocation)
        }

        fn copy_membership(&self) -> Result<(), KfdR66RetainedCustodyObservationFailureV1> {
            self.backend
                .r66_copy_membership_v1(Self::COPY, &self.backend.active_sdma[&Self::COPY])
        }
    }

    impl Drop for ScriptedR26RosterV1 {
        fn drop(&mut self) {
            self.backend.active = None;
            self.backend.active_sdma.clear();
            self.backend.active_sdma_streams.clear();
            self.backend.published_sdma_submissions.clear();
            self.backend.submissions.clear();
            self.backend.allocation_custody.clear();
            for record in self.backend.allocations.values_mut() {
                record.sdma_storage = KfdRuntimeSdmaStorageV1::Synthetic;
            }
        }
    }

    #[test]
    fn scripted_r26_membership_is_immutable_and_never_native_observation_authority() {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        let fixture = ScriptedR26RosterV1::new();
        let backend = &fixture.backend;
        let next_handle = backend.next_handle;
        let owners = [fixture.allocation, fixture.source, fixture.destination]
            .map(|allocation| backend.allocation_custody[&allocation].owners.clone());
        let bytes = [fixture.allocation, fixture.source, fixture.destination]
            .map(|allocation| Arc::clone(&backend.allocations[&allocation].bytes));
        for _ in 0..16 {
            assert_eq!(backend.r66_runtime_roster_shape_v1(), Ok(()));
            assert_eq!(backend.r66_runtime_native_counts_v1((1, 1)), Ok(()));
            assert_eq!(fixture.compute_membership(), Ok(()));
            assert_eq!(fixture.copy_membership(), Ok(()));
            assert_eq!(
                backend.diagnose_r66_retained_custody_v1(),
                Err(Failure::CountsQueueUnavailable)
            );
            assert_eq!(backend.observe_r66_retained_custody_v1(), None);
        }
        assert_eq!(backend.next_handle, next_handle);
        assert_eq!(
            backend.published_sdma_submissions,
            [ScriptedR26RosterV1::COPY]
        );
        assert_eq!(
            backend.active.as_ref().unwrap().id,
            ScriptedR26RosterV1::COMPUTE
        );
        assert_eq!(
            backend.active_sdma[&ScriptedR26RosterV1::COPY].completed_bytes,
            0
        );
        assert!(backend.submissions.is_empty());
        for (index, allocation) in [fixture.allocation, fixture.source, fixture.destination]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                backend.allocation_custody[&allocation].owners,
                owners[index]
            );
            assert!(Arc::ptr_eq(
                &backend.allocations[&allocation].bytes,
                &bytes[index]
            ));
        }
        assert_eq!(
            backend.r66_runtime_native_counts_v1((0, 1)),
            Err(Failure::CountsComputeMismatch)
        );
        assert_eq!(
            backend.r66_runtime_native_counts_v1((1, 0)),
            Err(Failure::CountsCopyMismatch)
        );
    }

    #[test]
    fn active_owner_membership_needs_no_pending_result_record_and_allocates_nothing() {
        use super::super::drain_capture::tests::counted;
        let fixture = ScriptedR26RosterV1::new();
        assert!(fixture.backend.submissions.is_empty());
        for _ in 0..16 {
            let (result, allocations) =
                counted(|| (fixture.compute_membership(), fixture.copy_membership()));
            assert_eq!(result, (Ok(()), Ok(())));
            assert_eq!(allocations, 0);
            assert!(fixture.backend.submissions.is_empty());
        }
    }

    #[test]
    fn active_owner_membership_rejects_foreign_and_removed_owner_borrows() {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        let mut fixture = ScriptedR26RosterV1::new();
        let foreign = ScriptedR26RosterV1::new();
        assert_eq!(
            fixture.backend.r66_compute_membership_v1(
                foreign.backend.active.as_ref().unwrap(),
                fixture.allocation,
            ),
            Err(Failure::ComputeSubmissionMissing)
        );
        assert_eq!(
            fixture.backend.r66_copy_membership_v1(
                ScriptedR26RosterV1::COPY,
                &foreign.backend.active_sdma[&ScriptedR26RosterV1::COPY],
            ),
            Err(Failure::CopySubmissionMissing)
        );
        let active = fixture.backend.active.take().unwrap();
        assert_eq!(
            fixture
                .backend
                .r66_compute_membership_v1(&active, fixture.allocation),
            Err(Failure::ComputeSubmissionMissing)
        );
        fixture.backend.active = Some(active);
        let copy = fixture
            .backend
            .active_sdma
            .remove(&ScriptedR26RosterV1::COPY)
            .unwrap();
        assert_eq!(
            fixture
                .backend
                .r66_copy_membership_v1(ScriptedR26RosterV1::COPY, &copy),
            Err(Failure::CopySubmissionMissing)
        );
        fixture
            .backend
            .active_sdma
            .insert(ScriptedR26RosterV1::COPY, copy);
        assert_eq!(fixture.compute_membership(), Ok(()));
        assert_eq!(fixture.copy_membership(), Ok(()));
    }

    #[test]
    fn active_owner_membership_rejects_every_conflicting_result_without_mutation() {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        let mut fixture = ScriptedR26RosterV1::new();
        for status in [
            BackendPollV1::Pending,
            BackendPollV1::Succeeded,
            BackendPollV1::Failed { code: 9 },
        ] {
            for compute in [true, false] {
                let (id, stream, expected) = if compute {
                    (
                        ScriptedR26RosterV1::COMPUTE,
                        fixture.backend.active.as_ref().unwrap().stream,
                        Failure::ComputeStatus,
                    )
                } else {
                    (
                        ScriptedR26RosterV1::COPY,
                        fixture.backend.active_sdma[&ScriptedR26RosterV1::COPY].stream,
                        Failure::CopyStatus,
                    )
                };
                fixture.backend.submissions.insert(
                    id,
                    SubmissionRecordV1 {
                        stream,
                        status,
                        profile_dispatch_published: false,
                    },
                );
                for _ in 0..2 {
                    let result = if compute {
                        fixture.compute_membership()
                    } else {
                        fixture.copy_membership()
                    };
                    assert_eq!(result, Err(expected));
                    assert_eq!(fixture.backend.submissions[&id].status, status);
                    assert_eq!(fixture.backend.submissions[&id].stream, stream);
                }
                fixture.backend.submissions.remove(&id);
                assert_eq!(fixture.compute_membership(), Ok(()));
                assert_eq!(fixture.copy_membership(), Ok(()));
            }
        }
    }

    #[test]
    fn scripted_r26_compute_membership_reports_one_coordinate_failures() {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        type Mutation = (Failure, fn(&mut ScriptedR26RosterV1));
        let cases: &[Mutation] = &[
            (Failure::ComputeStreamMismatch, |fixture| {
                let stream = fixture.backend.active.as_ref().unwrap().stream;
                fixture.backend.streams.remove(&stream);
            }),
            (Failure::ComputeStreamMismatch, |fixture| {
                fixture
                    .backend
                    .allocations
                    .get_mut(&fixture.allocation)
                    .unwrap()
                    .device += 1;
            }),
            (Failure::ComputeStatus, |fixture| {
                fixture.backend.submissions.insert(
                    ScriptedR26RosterV1::COMPUTE,
                    SubmissionRecordV1 {
                        stream: fixture.backend.active.as_ref().unwrap().stream,
                        status: BackendPollV1::Succeeded,
                        profile_dispatch_published: false,
                    },
                );
            }),
            (Failure::ComputeAllocationCount, |fixture| {
                fixture
                    .backend
                    .active
                    .as_mut()
                    .unwrap()
                    .allocations
                    .insert(fixture.source);
            }),
            (Failure::ComputeAllocationMembership, |fixture| {
                fixture.backend.active.as_mut().unwrap().allocations =
                    HashSet::from([fixture.source]);
            }),
            (Failure::ComputeAllocationMissing, |fixture| {
                fixture.backend.allocations.remove(&fixture.allocation);
            }),
            (Failure::ComputeCustody, |fixture| {
                fixture
                    .backend
                    .allocation_custody
                    .remove(&fixture.allocation);
            }),
            (Failure::ComputeCustody, |fixture| {
                fixture
                    .backend
                    .allocation_custody
                    .get_mut(&fixture.allocation)
                    .unwrap()
                    .owners[0]
                    .stream += 1;
            }),
            (Failure::ComputeCustody, |fixture| {
                fixture
                    .backend
                    .allocation_custody
                    .get_mut(&fixture.allocation)
                    .unwrap()
                    .owners[0]
                    .submission += 1;
            }),
            (Failure::ComputeStorage, |fixture| {
                fixture
                    .backend
                    .allocations
                    .get_mut(&fixture.allocation)
                    .unwrap()
                    .sdma_storage = KfdRuntimeSdmaStorageV1::ComputeInFlight(999);
            }),
        ];
        for &(expected, mutate) in cases {
            let mut fixture = ScriptedR26RosterV1::new();
            assert_eq!(fixture.compute_membership(), Ok(()));
            mutate(&mut fixture);
            assert_eq!(fixture.compute_membership(), Err(expected));
            assert!(fixture.backend.observe_r66_retained_custody_v1().is_none());
        }
    }

    #[test]
    fn scripted_r26_copy_membership_reports_one_coordinate_failures() {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        type Mutation = (Failure, fn(&mut ScriptedR26RosterV1));
        let cases: &[Mutation] = &[
            (Failure::CopyIdMismatch, |fixture| {
                fixture
                    .backend
                    .active_sdma
                    .get_mut(&ScriptedR26RosterV1::COPY)
                    .unwrap()
                    .id += 1;
            }),
            (Failure::CopyStreamMismatch, |fixture| {
                let stream = fixture.backend.active_sdma[&ScriptedR26RosterV1::COPY].stream;
                fixture.backend.streams.remove(&stream);
            }),
            (Failure::CopyStreamMismatch, |fixture| {
                let stream = fixture.backend.active_sdma[&ScriptedR26RosterV1::COPY].stream;
                fixture.backend.active_sdma_streams.remove(&stream);
            }),
            (Failure::CopyStreamMismatch, |fixture| {
                let stream = fixture.backend.active_sdma[&ScriptedR26RosterV1::COPY].stream;
                fixture
                    .backend
                    .active_sdma_streams
                    .get_mut(&stream)
                    .unwrap()
                    .push_back(ScriptedR26RosterV1::COPY);
            }),
            (Failure::CopyStreamMismatch, |fixture| {
                fixture
                    .backend
                    .allocations
                    .get_mut(&fixture.destination)
                    .unwrap()
                    .device += 1;
            }),
            (Failure::CopyStatus, |fixture| {
                fixture.backend.submissions.insert(
                    ScriptedR26RosterV1::COPY,
                    SubmissionRecordV1 {
                        stream: fixture.backend.active_sdma[&ScriptedR26RosterV1::COPY].stream,
                        status: BackendPollV1::Succeeded,
                        profile_dispatch_published: false,
                    },
                );
            }),
            (Failure::CopyPublishedRoster, |fixture| {
                fixture.backend.published_sdma_submissions[0] += 1;
            }),
            (Failure::CopyPartialCompletion, |fixture| {
                fixture
                    .backend
                    .active_sdma
                    .get_mut(&ScriptedR26RosterV1::COPY)
                    .unwrap()
                    .completed_bytes = 1;
            }),
            (Failure::CopyWindowBytes, |fixture| {
                fixture
                    .backend
                    .active_sdma
                    .get_mut(&ScriptedR26RosterV1::COPY)
                    .unwrap()
                    .window_bytes -= 1;
            }),
            (Failure::CopySourceAllocationMissing, |fixture| {
                fixture.backend.allocations.remove(&fixture.source);
            }),
            (Failure::CopyDestinationAllocationMissing, |fixture| {
                fixture.backend.allocations.remove(&fixture.destination);
            }),
            (Failure::CopySourceCustody, |fixture| {
                fixture.backend.allocation_custody.remove(&fixture.source);
            }),
            (Failure::CopyDestinationCustody, |fixture| {
                fixture
                    .backend
                    .allocation_custody
                    .get_mut(&fixture.destination)
                    .unwrap()
                    .owners[0]
                    .submission += 1;
            }),
            (Failure::CopySourceStorage, |fixture| {
                fixture
                    .backend
                    .allocations
                    .get_mut(&fixture.source)
                    .unwrap()
                    .sdma_storage = KfdRuntimeSdmaStorageV1::Synthetic;
            }),
            (Failure::CopyDestinationStorage, |fixture| {
                fixture
                    .backend
                    .allocations
                    .get_mut(&fixture.destination)
                    .unwrap()
                    .sdma_storage =
                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(999));
            }),
            (Failure::CopyComputeAlias, |fixture| {
                fixture
                    .backend
                    .active
                    .as_mut()
                    .unwrap()
                    .allocations
                    .insert(fixture.source);
            }),
        ];
        for &(expected, mutate) in cases {
            let mut fixture = ScriptedR26RosterV1::new();
            assert_eq!(fixture.copy_membership(), Ok(()));
            mutate(&mut fixture);
            assert_eq!(fixture.copy_membership(), Err(expected));
            assert!(fixture.backend.observe_r66_retained_custody_v1().is_none());
        }
    }

    #[test]
    fn scripted_r26_copy_geometry_reports_exact_directional_coordinates() {
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        let fixture = ScriptedR26RosterV1::new();
        let copy = &fixture.backend.active_sdma[&ScriptedR26RosterV1::COPY];
        for direction in [
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        ] {
            let check = |source, destination, bytes| {
                let (host, device) = match direction {
                    Gfx942PersistentSdmaDirectionV1::HostToDevice => (source, destination),
                    Gfx942PersistentSdmaDirectionV1::DeviceToHost => (destination, source),
                };
                KfdRuntimeBackendV1::r66_copy_geometry_v1(copy, direction, host, device, bytes)
            };
            assert_eq!(check(8, 16, 32), Ok(()));
            assert_eq!(check(9, 16, 32), Err(Failure::CopySourceOffset));
            assert_eq!(check(8, 17, 32), Err(Failure::CopyDestinationOffset));
            assert_eq!(check(8, 16, 31), Err(Failure::CopyBytes));
        }
    }

    #[test]
    fn qualified_runtime_shape_never_substitutes_for_native_authority() {
        let backend = qualified_without_native_queue();
        assert!(backend.r66_runtime_roster_shape_v1().is_ok());
        assert!(backend.observe_r66_retained_custody_v1().is_none());
        assert_eq!(
            backend.diagnose_r66_retained_custody_v1(),
            Err(KfdR66RetainedCustodyObservationFailureV1::CountsQueueUnavailable)
        );
        assert!(backend.queue.is_none());
        assert!(!backend.terminal);
    }

    #[test]
    fn runtime_shape_rejects_terminal_retired_unsupported_and_incomplete_rosters() {
        let mut backend = qualified_without_native_queue();
        backend.terminal = true;
        assert_eq!(
            backend.r66_runtime_roster_shape_v1(),
            Err(KfdR66RetainedCustodyObservationFailureV1::ShapeTerminal)
        );
        backend.terminal = false;
        backend.queue_retired = true;
        assert_eq!(
            backend.r66_runtime_roster_shape_v1(),
            Err(KfdR66RetainedCustodyObservationFailureV1::ShapeQueueRetired)
        );
        backend.queue_retired = false;
        backend.published_sdma_submissions.push(17);
        assert_eq!(
            backend.r66_runtime_roster_shape_v1(),
            Err(KfdR66RetainedCustodyObservationFailureV1::ShapePublishedCopyCount)
        );
        backend.published_sdma_submissions.clear();
        assert!(backend.r66_runtime_roster_shape_v1().is_ok());
        backend.submissions.insert(
            17,
            SubmissionRecordV1 {
                stream: 19,
                status: BackendPollV1::Pending,
                profile_dispatch_published: false,
            },
        );
        assert_eq!(
            backend.r66_runtime_roster_shape_v1(),
            Err(KfdR66RetainedCustodyObservationFailureV1::ShapeUnaccountedSubmission)
        );
        backend.submissions.clear();
        backend.launch_gate = KfdRuntimeLaunchGateV1::Production(Box::new(TestAuthorityV1));
        assert_eq!(
            backend.r66_runtime_roster_shape_v1(),
            Err(KfdR66RetainedCustodyObservationFailureV1::ShapeUnsupportedLaunchGate)
        );
        assert!(backend.observe_r66_retained_custody_v1().is_none());
    }

    #[test]
    fn unaccounted_native_storage_markers_are_not_an_empty_observation() {
        let mut backend = qualified_without_native_queue();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        use KfdR66RetainedCustodyObservationFailureV1 as Failure;
        for (storage, failure) in [
            (
                KfdRuntimeSdmaStorageV1::ComputeInFlight(17),
                Failure::ShapeUnaccountedComputeStorage,
            ),
            (
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(17)),
                Failure::ShapeUnaccountedCopyStorage,
            ),
            (
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
                Failure::ShapeSynchronousStorage,
            ),
        ] {
            backend
                .allocations
                .get_mut(&allocation)
                .unwrap()
                .sdma_storage = storage;
            assert_eq!(backend.r66_runtime_roster_shape_v1(), Err(failure));
            assert_eq!(backend.diagnose_r66_retained_custody_v1(), Err(failure));
            assert!(backend.observe_r66_retained_custody_v1().is_none());
        }
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_storage = KfdRuntimeSdmaStorageV1::Synthetic;
        backend.release_allocation_v1(allocation).unwrap();
        assert!(backend.r66_runtime_roster_shape_v1().is_ok());
    }
    #[test]
    fn membership_binds_every_coordinate_and_native_occurrence() {
        let baseline = membership(b"copy\0", [1; 32], &[7, 8, 9, 10, 11, 12, 13]);
        for index in 0..7 {
            let mut values = [7, 8, 9, 10, 11, 12, 13];
            values[index] += 1;
            assert_ne!(baseline, membership(b"copy\0", [1; 32], &values));
        }
        assert_ne!(
            baseline,
            membership(b"compute\0", [1; 32], &[7, 8, 9, 10, 11, 12, 13])
        );
        assert_ne!(
            baseline,
            membership(b"copy\0", [2; 32], &[7, 8, 9, 10, 11, 12, 13])
        );
    }
}
