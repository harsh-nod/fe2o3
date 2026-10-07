//! Persistent allocation ledger and exact lease transitions.

use super::*;

impl Gfx942PersistentDeviceAllocationV1 {
    pub fn from_local_mapping(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Self {
        let binding = lease.storage_identity();
        let byte_len = lease.layout().requested_bytes();
        Self::new(
            PersistentBackingV1::Local(Gfx942SdmaDeviceBackingV1::from_native(lease)),
            binding,
            Gfx942PersistentMappingFormV1::Local,
            byte_len,
        )
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn from_sdma_buffer(buffer: Gfx942SdmaBufferV1) -> Result<Self, Gfx942SdmaBufferV1> {
        let backing = Gfx942SdmaDeviceBackingV1::from_buffer(buffer)?;
        let lease = backing.lease().expect("attached buffer");
        let binding = lease.storage_identity();
        let byte_len = lease.layout().requested_bytes();
        Ok(Self::new(
            PersistentBackingV1::Local(backing),
            binding,
            Gfx942PersistentMappingFormV1::Local,
            byte_len,
        ))
    }

    #[allow(clippy::result_large_err)]
    pub fn from_exact_two_device_peer_mapping(
        mapping: Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<Self, Gfx942XgmiMappedDeviceMemoryV1> {
        let gpu_ids = mapping.gpu_ids();
        let Ok(gpu_ids) = <[u32; 2]>::try_from(gpu_ids) else {
            return Err(mapping);
        };
        if !mapping.is_fully_mapped() || gpu_ids[0] >= gpu_ids[1] {
            return Err(mapping);
        }
        let binding = mapping.lease().storage_identity();
        let byte_len = mapping.lease().layout().requested_bytes();
        Ok(Self::new(
            PersistentBackingV1::ExactTwoDevicePeer(mapping),
            binding,
            Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { gpu_ids },
            byte_len,
        ))
    }

    pub(super) fn new(
        native: PersistentBackingV1,
        binding: Gfx942DeviceMemoryIdentityV1,
        mapping: Gfx942PersistentMappingFormV1,
        byte_len: u64,
    ) -> Self {
        Self {
            incarnation: Rc::new(()),
            binding,
            mapping,
            byte_len,
            state: Box::new(PersistentOwnerStateV1 {
                native: Some(native),
                detached_compute: None,
                ledger: [None; GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1],
            }),
            next_generation: 1,
            next_sequence: 1,
            frontier_generation: 0,
            frontier_sequence: None,
            quarantine: None,
            thread_affinity: PhantomData,
        }
    }

    pub const fn mapping_form(&self) -> Gfx942PersistentMappingFormV1 {
        self.mapping
    }

    #[cfg(test)]
    pub(crate) fn ownership_snapshot_for_test_v1(&self) -> PersistentOwnerSnapshotForTestV1 {
        PersistentOwnerSnapshotForTestV1 {
            incarnation: Rc::as_ptr(&self.incarnation) as usize,
            local_native: self
                .local_native_for_sdma()
                .map(|lease| lease.storage_identity()),
            binding: self.binding,
            mapping: self.mapping,
            byte_len: self.byte_len,
            initialization: match self.state.native.as_ref() {
                Some(PersistentBackingV1::Local(backing)) => backing.initialization_for_test(),
                _ => None,
            },
            detached_compute: self.state.detached_compute,
            ledger_address: self.state.ledger.as_ptr() as usize,
            ledger: self.state.ledger,
            next_generation: self.next_generation,
            next_sequence: self.next_sequence,
            frontier_generation: self.frontier_generation,
            frontier_sequence: self.frontier_sequence,
            quarantine: self.quarantine,
        }
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn quarantine_reason(&self) -> Option<Gfx942PersistentQuarantineReasonV1> {
        self.quarantine
    }

    pub fn live_use_count(&self) -> usize {
        self.state
            .ledger
            .iter()
            .flatten()
            .filter(|record| record.state != LedgerStateV1::Settled)
            .count()
    }

    pub fn retained_settled_use_count(&self) -> usize {
        self.state
            .ledger
            .iter()
            .flatten()
            .filter(|record| record.state == LedgerStateV1::Settled)
            .count()
    }

    pub fn reserve(
        &mut self,
        request: Gfx942PersistentUseRequestV1,
        dependency: Option<&Gfx942PersistentDependencyFrontierV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
        Gfx942PersistentReservationFailureV1,
    > {
        let fail = |error| Gfx942PersistentReservationFailureV1 { error, request };
        if self.quarantine.is_some() {
            return Err(fail(Gfx942PersistentUseErrorV1::Quarantined));
        }
        let Some(end) = request.range.end() else {
            return Err(fail(Gfx942PersistentUseErrorV1::InvalidRange));
        };
        if request.range.byte_len == 0 || end > self.byte_len {
            return Err(fail(Gfx942PersistentUseErrorV1::InvalidRange));
        }
        if request.owner() == Gfx942PersistentUseOwnerV1::PeerMapped
            && !matches!(
                self.mapping,
                Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { .. }
            )
        {
            return Err(fail(
                Gfx942PersistentUseErrorV1::OperationRequiresPeerMapping,
            ));
        }

        let mut needs_dependency = false;
        for record in self.state.ledger.iter().flatten() {
            if !request.range.overlaps(record.request.range) {
                continue;
            }
            let hazard = request.access().writes() || record.request.access().writes();
            if !hazard {
                continue;
            }
            if record.state == LedgerStateV1::Settled {
                needs_dependency = true;
            } else {
                return Err(fail(Gfx942PersistentUseErrorV1::OverlappingWriterActive));
            }
        }
        match (needs_dependency, dependency) {
            (true, None) => return Err(fail(Gfx942PersistentUseErrorV1::DependencyRequired)),
            (false, Some(_)) => {
                return Err(fail(Gfx942PersistentUseErrorV1::DependencyNotRequired));
            }
            (true, Some(dependency))
                if !Rc::ptr_eq(&dependency.incarnation, &self.incarnation)
                    || dependency.binding != self.binding
                    || dependency.generation != self.frontier_generation
                    || Some(dependency.through_sequence) != self.frontier_sequence =>
            {
                return Err(fail(
                    Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency,
                ));
            }
            _ => {}
        }

        let Some(slot) = self.state.ledger.iter().position(Option::is_none) else {
            return Err(fail(Gfx942PersistentUseErrorV1::Capacity));
        };
        let generation = self.next_generation;
        let sequence = self.next_sequence;
        let Some(next_generation) = generation.checked_add(1) else {
            return Err(fail(Gfx942PersistentUseErrorV1::GenerationExhausted));
        };
        let Some(next_sequence) = sequence.checked_add(1) else {
            return Err(fail(Gfx942PersistentUseErrorV1::GenerationExhausted));
        };
        self.next_generation = next_generation;
        self.next_sequence = next_sequence;
        self.state.ledger[slot] = Some(LedgerRecordV1 {
            generation,
            sequence,
            request,
            state: LedgerStateV1::Reserved,
        });
        Ok(Gfx942PersistentUseLeaseV1 {
            incarnation: Rc::clone(&self.incarnation),
            binding: self.binding,
            slot: u8::try_from(slot).expect("ledger bound fits u8"),
            generation,
            sequence,
            request,
            marker: PhantomData,
            thread_affinity: PhantomData,
        })
    }

    pub fn prepare(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentReservedV1>,
    > {
        self.transition(lease, LedgerStateV1::Reserved, LedgerStateV1::Prepared)
    }

    pub fn publish(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentPreparedV1>,
    > {
        self.transition(lease, LedgerStateV1::Prepared, LedgerStateV1::Published)
    }

    pub fn complete(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    ) -> Result<
        Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentPublishedV1>,
    > {
        self.transition(lease, LedgerStateV1::Published, LedgerStateV1::Completed)
    }

    pub(super) fn transition<S: Gfx942PersistentUseStateV1, T: Gfx942PersistentUseStateV1>(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<S>,
        expected: LedgerStateV1,
        next: LedgerStateV1,
    ) -> Result<Gfx942PersistentUseLeaseV1<T>, Gfx942PersistentTransitionFailureV1<S>> {
        let result = self.validate_lease(&lease, expected);
        if let Err(error) = result {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.state.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = next;
        Ok(lease.retag())
    }

    pub fn cancel_reserved(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentReservedV1>> {
        self.cancel(lease, LedgerStateV1::Reserved)
    }

    pub fn cancel_prepared(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentPreparedV1>> {
        self.cancel(lease, LedgerStateV1::Prepared)
    }

    pub(crate) fn preflight_cancel_prepared(
        &self,
        lease: &Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        self.validate_lease(lease, LedgerStateV1::Prepared)
    }

    pub(super) fn cancel<S: Gfx942PersistentUseStateV1>(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<S>,
        expected: LedgerStateV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<S>> {
        if let Err(error) = self.validate_lease(&lease, expected) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.state.ledger[usize::from(lease.slot)] = None;
        Ok(())
    }

    pub fn observe_timeout(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
    ) -> Result<
        Gfx942PersistentTimeoutV1,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentPublishedV1>,
    > {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Published) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        Ok(Gfx942PersistentTimeoutV1 { published: lease })
    }

    pub fn settle(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    ) -> Result<
        Gfx942PersistentDependencyFrontierV1,
        Gfx942PersistentTransitionFailureV1<Gfx942PersistentCompletedV1>,
    > {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Completed) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        if self.state.ledger.iter().flatten().any(|record| {
            record.sequence < lease.sequence && record.state != LedgerStateV1::Settled
        }) {
            return Err(Gfx942PersistentTransitionFailureV1 {
                error: Gfx942PersistentUseErrorV1::EarlierUseNotSettled,
                lease,
            });
        }
        let Some(frontier_generation) = self.frontier_generation.checked_add(1) else {
            return Err(Gfx942PersistentTransitionFailureV1 {
                error: Gfx942PersistentUseErrorV1::GenerationExhausted,
                lease,
            });
        };
        self.state.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Settled;
        self.frontier_generation = frontier_generation;
        self.frontier_sequence = Some(lease.sequence);
        Ok(Gfx942PersistentDependencyFrontierV1 {
            incarnation: Rc::clone(&self.incarnation),
            binding: self.binding,
            generation: frontier_generation,
            through_sequence: lease.sequence,
            thread_affinity: PhantomData,
        })
    }

    pub(crate) fn preflight_settle(
        &self,
        lease: &Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        self.validate_lease(lease, LedgerStateV1::Completed)?;
        if self.state.ledger.iter().flatten().any(|record| {
            record.sequence < lease.sequence && record.state != LedgerStateV1::Settled
        }) {
            return Err(Gfx942PersistentUseErrorV1::EarlierUseNotSettled);
        }
        self.frontier_generation
            .checked_add(1)
            .map(|_| ())
            .ok_or(Gfx942PersistentUseErrorV1::GenerationExhausted)
    }

    /// Retires settled history after the caller has established quiescence.
    /// This is a ledger transition only and performs no device operation.
    pub fn retire_settled_frontier(
        &mut self,
        frontier: Gfx942PersistentDependencyFrontierV1,
    ) -> Result<(), Gfx942PersistentDependencyFrontierV1> {
        let current = Rc::ptr_eq(&frontier.incarnation, &self.incarnation)
            && frontier.binding == self.binding
            && frontier.generation == self.frontier_generation
            && Some(frontier.through_sequence) == self.frontier_sequence;
        let has_active = self
            .state
            .ledger
            .iter()
            .flatten()
            .any(|record| record.state != LedgerStateV1::Settled);
        if !current || has_active || self.quarantine.is_some() {
            return Err(frontier);
        }
        for slot in self.state.ledger.iter_mut() {
            if slot
                .as_ref()
                .is_some_and(|record| record.state == LedgerStateV1::Settled)
            {
                *slot = None;
            }
        }
        self.frontier_sequence = None;
        Ok(())
    }

    pub(crate) fn preflight_retire_settled_frontier(
        &self,
        frontier: &Gfx942PersistentDependencyFrontierV1,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        let current = Rc::ptr_eq(&frontier.incarnation, &self.incarnation)
            && frontier.binding == self.binding
            && frontier.generation == self.frontier_generation
            && Some(frontier.through_sequence) == self.frontier_sequence;
        let has_active = self
            .state
            .ledger
            .iter()
            .flatten()
            .any(|record| record.state != LedgerStateV1::Settled);
        if !current || has_active || self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        Ok(())
    }

    pub fn quarantine_published(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPublishedV1>,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentPublishedV1>> {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Published) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.state.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Quarantined;
        self.quarantine = Some(reason);
        Ok(())
    }

    /// Quarantines a prepared use after the native adapter crossed its point
    /// of no return without confirming publication. A prepared use must not be
    /// relabeled published merely because lower-layer custody was retained.
    pub(crate) fn quarantine_prepared(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentPreparedV1>> {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Prepared) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.state.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Quarantined;
        self.quarantine = Some(reason);
        Ok(())
    }

    /// Quarantines a completed use when signal recycle or native-authority
    /// restoration becomes indeterminate after device completion.
    pub(crate) fn quarantine_completed(
        &mut self,
        lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        reason: Gfx942PersistentQuarantineReasonV1,
    ) -> Result<(), Gfx942PersistentTransitionFailureV1<Gfx942PersistentCompletedV1>> {
        if let Err(error) = self.validate_lease(&lease, LedgerStateV1::Completed) {
            return Err(Gfx942PersistentTransitionFailureV1 { error, lease });
        }
        self.state.ledger[usize::from(lease.slot)]
            .as_mut()
            .expect("validated ledger slot")
            .state = LedgerStateV1::Quarantined;
        self.quarantine = Some(reason);
        Ok(())
    }

    /// Moves the exact local mapping into one inspected compute dispatch only
    /// while the supplied prepared compute use is current.
    pub(crate) fn detach_local_native_for_compute(
        &mut self,
        prepared: &Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, Gfx942PersistentUseErrorV1>
    {
        self.validate_lease(prepared, LedgerStateV1::Prepared)?;
        if prepared.request.owner() != Gfx942PersistentUseOwnerV1::Compute {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        let Some(PersistentBackingV1::Local(backing)) = self.state.native.as_mut() else {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        };
        let lease = backing
            .detach_for_compute()
            .ok_or(Gfx942PersistentUseErrorV1::WrongState)?;
        self.state.detached_compute = Some((prepared.slot, prepared.generation));
        Ok(lease)
    }

    /// Restores only the exact mapping returned by the completed compute use.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn restore_local_native_from_compute(
        &mut self,
        completed: &Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<
        (),
        (
            Gfx942PersistentUseErrorV1,
            Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        ),
    > {
        if let Err(error) = self.validate_lease(completed, LedgerStateV1::Completed) {
            return Err((error, lease));
        }
        if completed.request.owner() != Gfx942PersistentUseOwnerV1::Compute {
            return Err((Gfx942PersistentUseErrorV1::WrongState, lease));
        }
        if let Err(error) = self.preflight_restore_local_native_from_compute(completed, &lease) {
            return Err((error, lease));
        }
        self.restore_local_native_from_sdma(lease)
    }

    pub(crate) fn preflight_restore_local_native_from_compute(
        &self,
        completed: &Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        self.validate_lease(completed, LedgerStateV1::Completed)?;
        if completed.request.owner() != Gfx942PersistentUseOwnerV1::Compute
            || self.state.detached_compute != Some((completed.slot, completed.generation))
            || self.local_native_is_attached_for_sdma()
            || lease.storage_identity() != self.binding
            || lease.layout().requested_bytes() != self.byte_len
        {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        Ok(())
    }

    pub(crate) fn preflight_restore_completed_compute_data(
        &self,
        completed: &Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        data: &Gfx942FixedDispatchDataV1,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        let DispatchDataStorageRefV1::Device(lease) = data.storage_ref() else {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        };
        self.preflight_restore_local_native_from_compute(completed, lease)?;
        let Some(PersistentBackingV1::Local(backing)) = self.state.native.as_ref() else {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        };
        if !backing.matches_detached_compute_scope(queue, generation, logical_bytes, self.byte_len)
        {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        Ok(())
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn restore_completed_compute_data(
        &mut self,
        completed: &Gfx942PersistentUseLeaseV1<Gfx942PersistentCompletedV1>,
        data: Gfx942FixedDispatchDataV1,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
    ) -> Result<(), (Gfx942PersistentUseErrorV1, Gfx942FixedDispatchDataV1)> {
        if let Err(error) = self.preflight_restore_completed_compute_data(
            completed,
            &data,
            queue,
            generation,
            logical_bytes,
        ) {
            return Err((error, data));
        }
        self.state.native = Some(PersistentBackingV1::Local(
            Gfx942SdmaDeviceBackingV1::from_completed_compute_data(
                data,
                queue,
                generation,
                logical_bytes,
                PersistentComputeCompletionPermitV1 { _private: () },
            ),
        ));
        self.state.detached_compute = None;
        Ok(())
    }

    /// Restores a mapping detached for compute when the prepared dispatch was
    /// cancelled before native publication.
    #[allow(clippy::result_large_err)]
    pub(crate) fn restore_local_native_from_cancelled_compute(
        &mut self,
        prepared: &Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<
        (),
        (
            Gfx942PersistentUseErrorV1,
            Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        ),
    > {
        if let Err(error) = self.validate_lease(prepared, LedgerStateV1::Prepared) {
            return Err((error, lease));
        }
        if prepared.request.owner() != Gfx942PersistentUseOwnerV1::Compute {
            return Err((Gfx942PersistentUseErrorV1::WrongState, lease));
        }
        if let Err(error) =
            self.preflight_restore_local_native_from_cancelled_compute(prepared, &lease)
        {
            return Err((error, lease));
        }
        let Some(PersistentBackingV1::Local(backing)) = self.state.native.as_mut() else {
            return Err((Gfx942PersistentUseErrorV1::WrongState, lease));
        };
        backing.restore_cancelled_compute(
            lease,
            PersistentComputeCancellationPermitV1 { _private: () },
        );
        self.state.detached_compute = None;
        Ok(())
    }

    pub(crate) fn preflight_restore_local_native_from_cancelled_compute(
        &self,
        prepared: &Gfx942PersistentUseLeaseV1<Gfx942PersistentPreparedV1>,
        lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        self.validate_lease(prepared, LedgerStateV1::Prepared)?;
        if prepared.request.owner() != Gfx942PersistentUseOwnerV1::Compute
            || self.state.detached_compute != Some((prepared.slot, prepared.generation))
            || self.local_native_is_attached_for_sdma()
            || !matches!(self.state.native, Some(PersistentBackingV1::Local(_)))
            || lease.storage_identity() != self.binding
            || lease.layout().requested_bytes() != self.byte_len
        {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        Ok(())
    }

    /// Temporarily moves the exact local mapping into a queue record. The
    /// queue adapter retains this owner while the native authority is absent.
    #[cfg(test)]
    pub(crate) fn detach_local_native_for_sdma(
        &mut self,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, Gfx942PersistentUseErrorV1>
    {
        if self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::Quarantined);
        }
        if !self.local_native_is_attached_for_sdma() {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        let Some(PersistentBackingV1::Local(backing)) = self.state.native.take() else {
            unreachable!("checked attached local backing")
        };
        Ok(backing.into_native())
    }

    pub(crate) fn detach_sdma_buffer(
        &mut self,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
    ) -> Result<Gfx942SdmaBufferV1, Gfx942PersistentUseErrorV1> {
        if self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::Quarantined);
        }
        if !self.can_detach_sdma_buffer(queue, generation, logical_bytes) {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        let Some(PersistentBackingV1::Local(backing)) = self.state.native.take() else {
            unreachable!("checked attached local backing")
        };
        Ok(backing.into_buffer(queue, generation, logical_bytes))
    }

    pub(super) fn can_detach_sdma_buffer(
        &self,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
    ) -> bool {
        matches!(self.state.native.as_ref(), Some(PersistentBackingV1::Local(backing)) if backing.matches_scope(queue, generation, logical_bytes))
    }

    pub(super) fn can_restore_sdma_buffer(&self, buffer: &Gfx942SdmaBufferV1) -> bool {
        self.quarantine.is_none()
            && self.mapping == Gfx942PersistentMappingFormV1::Local
            && self.state.native.is_none()
            && buffer.storage_identity() == Gfx942SdmaBufferStorageIdentityV1::Device(self.binding)
            && buffer.physical_bytes() == self.byte_len
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn restore_sdma_buffer(
        &mut self,
        buffer: Gfx942SdmaBufferV1,
    ) -> Result<(), Gfx942SdmaBufferV1> {
        if !self.can_restore_sdma_buffer(&buffer) {
            return Err(buffer);
        }
        self.state.native = Some(PersistentBackingV1::Local(
            Gfx942SdmaDeviceBackingV1::from_buffer(buffer).expect("checked device backing"),
        ));
        Ok(())
    }

    /// Restores only the exact local mapping detached from this owner.
    #[cfg(test)]
    #[allow(clippy::result_large_err)]
    pub(crate) fn restore_local_native_from_sdma(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<
        (),
        (
            Gfx942PersistentUseErrorV1,
            Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        ),
    > {
        if self.local_native_is_attached_for_sdma()
            || self.mapping != Gfx942PersistentMappingFormV1::Local
        {
            return Err((Gfx942PersistentUseErrorV1::WrongState, lease));
        }
        if lease.storage_identity() != self.binding
            || lease.layout().requested_bytes() != self.byte_len
        {
            return Err((Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration, lease));
        }
        self.state.native = Some(PersistentBackingV1::Local(
            Gfx942SdmaDeviceBackingV1::from_native(lease),
        ));
        self.state.detached_compute = None;
        Ok(())
    }

    pub(crate) fn local_native_is_attached_for_sdma(&self) -> bool {
        self.local_native_for_sdma().is_some()
    }

    /// Initialization conversion requires retirement, not merely settled uses.
    pub(crate) fn preflight_initialized_storage_for_compute(
        &self,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
        physical_bytes: u64,
    ) -> Result<
        Option<crate::persistent_compute::Gfx942PersistentComputeStorageIneligibilityV1>,
        Gfx942PersistentUseErrorV1,
    > {
        use crate::persistent_compute::Gfx942PersistentComputeStorageIneligibilityV1 as Ineligible;
        let full = self.preflight_initialized_logical_storage(
            queue,
            generation,
            logical_bytes,
            physical_bytes,
        )?;
        // Scope and custody must be valid before any clean fallback is offered.
        Ok(if logical_bytes != physical_bytes {
            Some(Ineligible::PartialExtent)
        } else if !full {
            Some(Ineligible::IncompleteInitialization)
        } else {
            None
        })
    }

    pub(crate) fn preflight_initialized_storage_for_xgmi(
        &self,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
        physical_bytes: u64,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        if !self.preflight_initialized_logical_storage(
            queue,
            generation,
            logical_bytes,
            physical_bytes,
        )? || self.local_native_for_sdma().is_none_or(|lease| {
            lease.layout().uapi_flags()
                != fe2o3_kfd_uapi::KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
        }) {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        Ok(())
    }

    pub(super) fn preflight_initialized_logical_storage(
        &self,
        queue: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
        physical_bytes: u64,
    ) -> Result<bool, Gfx942PersistentUseErrorV1> {
        if self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::Quarantined);
        }
        if self.frontier_sequence.is_some() || self.state.ledger.iter().any(Option::is_some) {
            return Err(Gfx942PersistentUseErrorV1::OutstandingUses);
        }
        if self.mapping != Gfx942PersistentMappingFormV1::Local
            || self.state.detached_compute.is_some()
            || logical_bytes == 0
            || logical_bytes > physical_bytes
            || physical_bytes != self.byte_len
        {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        let Some(PersistentBackingV1::Local(backing)) = self.state.native.as_ref() else {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        };
        if !backing.lease().is_some_and(|lease| {
            lease.storage_identity() == self.binding
                && lease.layout().requested_bytes() == physical_bytes
        }) {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        backing
            .full_initialization_in_scope(queue, generation, logical_bytes)
            .ok_or(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration)
    }

    pub(crate) fn local_native_for_sdma(
        &self,
    ) -> Option<&Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>> {
        match self.state.native.as_ref()? {
            PersistentBackingV1::Local(backing) => backing.lease(),
            PersistentBackingV1::ExactTwoDevicePeer(_) => None,
        }
    }

    /// Records caller-reported currentness loss even when no use is published.
    /// This core does not itself observe KFD, DRM, topology, or queue state.
    pub fn quarantine_for_caller_reported_currentness_loss(&mut self) {
        self.quarantine = Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss);
    }

    /// Returns native cleanup custody only after every use is settled or
    /// cancelled. A quarantined owner is deliberately returned intact.
    #[allow(clippy::result_large_err)]
    pub fn try_into_native(
        mut self,
    ) -> Result<Gfx942PersistentNativeAllocationV1, (Gfx942PersistentUseErrorV1, Self)> {
        if self.quarantine.is_some() {
            return Err((Gfx942PersistentUseErrorV1::Quarantined, self));
        }
        if self
            .state
            .ledger
            .iter()
            .flatten()
            .any(|record| record.state != LedgerStateV1::Settled)
        {
            return Err((Gfx942PersistentUseErrorV1::OutstandingUses, self));
        }
        if self.state.native.is_none()
            || (self.mapping == Gfx942PersistentMappingFormV1::Local
                && !self.local_native_is_attached_for_sdma())
        {
            return Err((Gfx942PersistentUseErrorV1::WrongState, self));
        }
        Ok(
            match self.state.native.take().expect("checked native authority") {
                PersistentBackingV1::Local(backing) => {
                    Gfx942PersistentNativeAllocationV1::Local(backing.into_native())
                }
                PersistentBackingV1::ExactTwoDevicePeer(mapping) => {
                    Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(mapping)
                }
            },
        )
    }

    pub(super) fn validate_lease<S: Gfx942PersistentUseStateV1>(
        &self,
        lease: &Gfx942PersistentUseLeaseV1<S>,
        expected: LedgerStateV1,
    ) -> Result<(), Gfx942PersistentUseErrorV1> {
        if self.quarantine.is_some() {
            return Err(Gfx942PersistentUseErrorV1::Quarantined);
        }
        if !Rc::ptr_eq(&lease.incarnation, &self.incarnation) || lease.binding != self.binding {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        let Some(record) = self
            .state
            .ledger
            .get(usize::from(lease.slot))
            .and_then(Option::as_ref)
        else {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        };
        if record.generation != lease.generation
            || record.sequence != lease.sequence
            || record.request != lease.request
        {
            return Err(Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
        }
        if record.state != expected {
            return Err(Gfx942PersistentUseErrorV1::WrongState);
        }
        Ok(())
    }
}
