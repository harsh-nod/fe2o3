//! Borrowed native custody checks for primary persistent compute and the exact
//! directional SDMA pair. No result survives a mutation or authorizes retirement.

use arrayvec::ArrayVec;
use fe2o3_runtime_model::{
    R66DeviceDomainV1, R66DeviceStorageV1, r66_device_storage_rosters_disjoint_v1,
};

use super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1;
use super::*;

/// A fixed observation rejection stage, not an admission or retirement result.
/// Variants disclose no native addresses and inspecting them does not poll work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942R66NativeObservationFailureV1 {
    PrimaryProfile,
    SdmaOwnerUnavailable,
    SdmaRetainedRoster,
    ComputeAttachmentShape,
    ComputeAttachmentTerminal,
    ComputeAttachmentQueue,
    ComputeAttachmentGeneration,
    ComputeAttachmentState,
    ComputeUnavailable,
    ComputeIdentityUnavailable,
    ComputeReceiptBinding,
    ComputeQuarantined,
    ComputeLiveUses,
    ComputeSettledUses,
    ComputeNativeCustody,
    ComputePoolGeneration,
    ComputeExtent,
    ComputeStorageIdentity,
    ComputeCurrentness,
    ComputeDomain,
    DispatchOwnerUnavailable,
    DispatchRoster,
    PublishedReceipt,
    CopyTicketRoster,
    CopyRetainedReceipt,
    CopyCurrentness,
    CopyQuarantined,
    CopyStorageIdentity,
    CopyPoolGeneration,
    CopyExtent,
    CopyHostIdentity,
    CopyGeometry,
}

fn diagnose_r66_compute_attachment_v1(
    attachment: Option<&BoundedPersistentComputeAttachmentV1>,
    queue: QueueKeyV1,
    next_generation: u64,
) -> Result<usize, Gfx942R66NativeObservationFailureV1> {
    use Gfx942R66NativeObservationFailureV1 as Failure;
    let Some(attachment) = attachment else {
        return Ok(0);
    };
    if !attachment.is_single() {
        return Err(Failure::ComputeAttachmentShape);
    }
    if attachment.terminal_custody.is_some() {
        return Err(Failure::ComputeAttachmentTerminal);
    }
    if attachment.binding.queue != queue {
        return Err(Failure::ComputeAttachmentQueue);
    }
    if attachment.binding.attachment_generation.checked_add(1) != Some(next_generation) {
        return Err(Failure::ComputeAttachmentGeneration);
    }
    if !matches!(
        attachment.entries[0].state,
        PersistentComputeUseStateV1::Published(_)
    ) {
        return Err(Failure::ComputeAttachmentState);
    }
    Ok(1)
}

fn diagnose_r66_compute_entry_v1(
    entry: &PersistentComputeAttachmentEntryV1,
) -> Result<Gfx942DeviceMemoryIdentityV1, Gfx942R66NativeObservationFailureV1> {
    use Gfx942R66NativeObservationFailureV1 as Failure;
    let identity = entry
        .storage_identity
        .ok_or(Failure::ComputeIdentityUnavailable)?;
    if entry.allocation.owner.quarantine_reason().is_some() {
        return Err(Failure::ComputeQuarantined);
    }
    if entry.allocation.owner.live_use_count() != 1 {
        return Err(Failure::ComputeLiveUses);
    }
    if entry.allocation.owner.retained_settled_use_count() != 0 {
        return Err(Failure::ComputeSettledUses);
    }
    if entry.allocation.owner.local_native_for_sdma().is_some() {
        return Err(Failure::ComputeNativeCustody);
    }
    if entry.allocation.attachment.pool_generation == 0 {
        return Err(Failure::ComputePoolGeneration);
    }
    if entry.allocation.byte_len() == 0
        || entry.allocation.byte_len() != entry.allocation.physical_byte_len()
        || entry.allocation.owner.byte_len() != entry.allocation.physical_byte_len()
    {
        return Err(Failure::ComputeExtent);
    }
    if entry.allocation.attachment.storage_identity
        != Gfx942SdmaBufferStorageIdentityV1::Device(identity)
    {
        return Err(Failure::ComputeStorageIdentity);
    }
    Ok(identity)
}

impl ComputeAqlQueueSessionV1 {
    /// Observes retained publication custody only; does not poll GPU completion.
    pub fn observe_r66_retained_counts_v1(&self) -> Option<(usize, usize)> {
        self.diagnose_r66_retained_counts_v1().ok()
    }

    /// Diagnoses retained custody without changing queue state or observing completion.
    pub fn diagnose_r66_retained_counts_v1(
        &self,
    ) -> Result<(usize, usize), Gfx942R66NativeObservationFailureV1> {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        if !self.coexistence_primary_profile_v1() {
            return Err(Failure::PrimaryProfile);
        }
        let copies = self
            .sdma
            .as_ref()
            .ok_or(Failure::SdmaOwnerUnavailable)?
            .compute_coexistence_endpoints_v1(self.key)
            .ok_or(Failure::SdmaRetainedRoster)?
            .len();
        let compute = diagnose_r66_compute_attachment_v1(
            self.persistent_compute.as_ref(),
            self.key,
            self.next_persistent_compute_generation,
        )?;
        Ok((compute, copies))
    }

    /// Address-free identity of the exact retained native dispatch receipt.
    pub fn observe_r66_retained_compute_v1(
        &self,
        receipt: &Gfx942PersistentComputeDispatchV1,
    ) -> Option<[u8; 32]> {
        self.diagnose_r66_retained_compute_v1(receipt).ok()
    }

    /// Explains failure of the exact compute observation; does not grant authority.
    pub fn diagnose_r66_retained_compute_v1(
        &self,
        receipt: &Gfx942PersistentComputeDispatchV1,
    ) -> Result<[u8; 32], Gfx942R66NativeObservationFailureV1> {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        use sha2::{Digest, Sha256};
        if self.diagnose_r66_retained_counts_v1()?.0 != 1 {
            return Err(Failure::ComputeUnavailable);
        }
        let attachment = self
            .persistent_compute
            .as_ref()
            .ok_or(Failure::ComputeUnavailable)?;
        let entry = attachment
            .single_entry()
            .ok_or(Failure::ComputeAttachmentShape)?;
        if entry.storage_identity.is_none() {
            return Err(Failure::ComputeIdentityUnavailable);
        }
        if receipt.binding != attachment.binding {
            return Err(Failure::ComputeReceiptBinding);
        }
        let identity = diagnose_r66_compute_entry_v1(entry)?;
        if !self.directional_persistent_sdma_attachment_is_current(&entry.allocation.attachment) {
            return Err(Failure::ComputeCurrentness);
        }
        let dispatch = self
            .dispatch
            .as_ref()
            .ok_or(Failure::DispatchOwnerUnavailable)?;
        if !dispatch.persistent_device_roster_matches_v1(&[identity]) {
            return Err(Failure::DispatchRoster);
        }
        let mut hash = Sha256::new();
        hash.update(b"fe2o3.r66.compute-attachment-observation.v1\0");
        let facts = identity
            .coexistence_facts_v1()
            .ok_or(Failure::ComputeDomain)?;
        for coordinate in [
            facts.allocation_id,
            facts.generation,
            facts.physical_device,
            facts.device_generation,
            facts.vm_id,
            entry.allocation.attachment.pool_generation,
            entry.allocation.byte_len(),
            entry.allocation.physical_byte_len(),
        ] {
            hash.update(coordinate.to_le_bytes());
        }
        hash.update(receipt.binding.attachment_generation.to_le_bytes());
        hash.update(
            dispatch
                .retained_published_batch_observation_v1(&receipt.batch)
                .ok_or(Failure::PublishedReceipt)?,
        );
        Ok(hash.finalize().into())
    }

    /// Address-free identity of an exact retained directional single receipt.
    pub fn observe_r66_retained_single_copy_v1(
        &self,
        receipt: &Gfx942DirectionalPersistentSdmaSubmissionV1,
    ) -> Option<[u8; 32]> {
        self.diagnose_r66_retained_single_copy_v1(receipt).ok()
    }

    /// Diagnoses one retained directional receipt without polling or mutation.
    pub fn diagnose_r66_retained_single_copy_v1(
        &self,
        receipt: &Gfx942DirectionalPersistentSdmaSubmissionV1,
    ) -> Result<[u8; 32], Gfx942R66NativeObservationFailureV1> {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        self.diagnose_r66_retained_counts_v1()?;
        let observed = self
            .sdma
            .as_ref()
            .ok_or(Failure::SdmaOwnerUnavailable)?
            .observe_directional_retained_request_v1(self.key, &[receipt.ticket])
            .ok_or(Failure::CopyRetainedReceipt)?;
        self.diagnose_r66_copy_observation_v1(
            &receipt.allocation,
            receipt.host_binding,
            receipt.direction,
            receipt.host_offset,
            receipt.device_offset,
            receipt.copy_bytes,
            observed,
        )
    }

    /// Address-free identity of every ticket in a retained directional window.
    pub fn observe_r66_retained_window_copy_v1(
        &self,
        receipt: &Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    ) -> Option<[u8; 32]> {
        self.diagnose_r66_retained_window_copy_v1(receipt).ok()
    }

    /// Diagnoses every ticket in a retained window without polling or mutation.
    pub fn diagnose_r66_retained_window_copy_v1(
        &self,
        receipt: &Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    ) -> Result<[u8; 32], Gfx942R66NativeObservationFailureV1> {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        self.diagnose_r66_retained_counts_v1()?;
        if receipt.tickets.len() != receipt.packet_count {
            return Err(Failure::CopyTicketRoster);
        }
        let observed = self
            .sdma
            .as_ref()
            .ok_or(Failure::SdmaOwnerUnavailable)?
            .observe_directional_retained_request_v1(self.key, &receipt.tickets)
            .ok_or(Failure::CopyRetainedReceipt)?;
        self.diagnose_r66_copy_observation_v1(
            &receipt.allocation,
            receipt.host_binding,
            receipt.direction,
            receipt.host_offset,
            receipt.device_offset,
            receipt.copy_bytes,
            observed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn diagnose_r66_copy_observation_v1(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
        host_binding: Gfx942PersistentDirectionalSdmaHostBindingV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        host_offset: u64,
        device_offset: u64,
        copy_bytes: u32,
        observed: crate::sdma::RetainedDirectionalSdmaObservationV1<'_>,
    ) -> Result<[u8; 32], Gfx942R66NativeObservationFailureV1> {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        let (host, device, actual_host_offset, actual_device_offset) = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => (
                observed.source,
                observed.destination,
                observed.source_offset,
                observed.destination_offset,
            ),
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => (
                observed.destination,
                observed.source,
                observed.destination_offset,
                observed.source_offset,
            ),
        };
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment) {
            return Err(Failure::CopyCurrentness);
        }
        if allocation.owner.quarantine_reason().is_some() {
            return Err(Failure::CopyQuarantined);
        }
        if allocation.attachment.storage_identity != device.storage_identity() {
            return Err(Failure::CopyStorageIdentity);
        }
        if allocation.attachment.pool_generation != device.pool_generation() {
            return Err(Failure::CopyPoolGeneration);
        }
        if allocation.physical_byte_len() != device.physical_bytes()
            || allocation.byte_len() != device.requested_bytes()
        {
            return Err(Failure::CopyExtent);
        }
        if !host_binding.matches(host) {
            return Err(Failure::CopyHostIdentity);
        }
        if host_offset != actual_host_offset
            || device_offset != actual_device_offset
            || copy_bytes != observed.copy_bytes
        {
            return Err(Failure::CopyGeometry);
        }
        Ok(observed.identity)
    }

    fn coexistence_domain_v1(&self) -> R66DeviceDomainV1 {
        R66DeviceDomainV1 {
            physical_device: self.key.vm.device.physical.0,
            device_generation: self.key.vm.device.generation.0,
            vm_id: self.key.vm.id.0,
        }
    }

    fn coexistence_primary_profile_v1(&self) -> bool {
        !self.terminal_poisoned
            && self.key == self.compute_lane_session
            && self.striped_sdma.is_none()
            && auxiliary_compute_lanes_are_quiescent_v1(&self.auxiliary_compute_lanes)
    }

    fn coexistence_allocation_facts_v1(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Option<R66DeviceStorageV1> {
        if !self.directional_persistent_sdma_attachment_is_current(&allocation.attachment)
            || allocation.attachment.pool_generation == 0
            || allocation.byte_len() == 0
            || allocation.byte_len() > allocation.physical_byte_len()
            || allocation.owner.byte_len() != allocation.physical_byte_len()
            || allocation.owner.quarantine_reason().is_some()
        {
            return None;
        }
        let native = allocation.owner.local_native_for_sdma()?;
        if allocation.attachment.storage_identity
            != Gfx942SdmaBufferStorageIdentityV1::Device(native.storage_identity())
            || native.layout().requested_bytes() != allocation.physical_byte_len()
        {
            return None;
        }
        native.storage_identity().coexistence_facts_v1()
    }

    pub(super) fn persistent_inputs_coexist_with_directional_sdma_v1(
        &self,
        inputs: &[&Gfx942PersistentComputeInputV1],
    ) -> bool {
        if !self.coexistence_primary_profile_v1() || self.persistent_compute.is_some() {
            return false;
        }
        let Some(copy_devices) = self
            .sdma
            .as_ref()
            .and_then(|sdma| sdma.compute_coexistence_endpoints_v1(self.key))
        else {
            return false;
        };
        let mut compute = ArrayVec::<R66DeviceStorageV1, MAX_DISPATCH_DATA_LEASES_V1>::new();
        for input in inputs {
            let allocation = match input {
                Gfx942PersistentComputeInputV1::Uninitialized(allocation)
                | Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) => {
                    allocation
                }
                Gfx942PersistentComputeInputV1::Initialized(ready) => &ready.allocation,
            };
            if allocation.owner.live_use_count() != 0
                || allocation.owner.retained_settled_use_count() != 0
                || allocation.byte_len() != allocation.physical_byte_len()
            {
                return false;
            }
            let Some(identity) = self.coexistence_allocation_facts_v1(allocation) else {
                return false;
            };
            if compute.try_push(identity).is_err() {
                return false;
            }
        }
        r66_device_storage_rosters_disjoint_v1(
            self.coexistence_domain_v1(),
            &compute,
            &copy_devices,
        )
    }

    pub(super) fn directional_sdma_coexists_with_persistent_compute_v1(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
        host: &Gfx942SdmaBufferV1,
    ) -> bool {
        let Some(attachment) = &self.persistent_compute else {
            return true;
        };
        if !self.coexistence_primary_profile_v1()
            || attachment.terminal_custody.is_some()
            || attachment.binding.queue != self.key
            || attachment.binding.attachment_generation.checked_add(1)
                != Some(self.next_persistent_compute_generation)
            || (!attachment.is_single() && !attachment.is_three())
        {
            return false;
        }
        let Some(sdma) = self.sdma.as_ref() else {
            return false;
        };
        let Some(mut copy_devices) = sdma.compute_coexistence_endpoints_v1(self.key) else {
            return false;
        };
        if !sdma.compute_coexistence_host_is_current_v1(self.key, host) {
            return false;
        }
        let Some(copy_device) = self.coexistence_allocation_facts_v1(allocation) else {
            return false;
        };
        if copy_devices.try_push(copy_device).is_err() {
            return false;
        }
        let mut identities =
            ArrayVec::<Gfx942DeviceMemoryIdentityV1, MAX_DISPATCH_DATA_LEASES_V1>::new();
        let mut compute = ArrayVec::<R66DeviceStorageV1, MAX_DISPATCH_DATA_LEASES_V1>::new();
        for entry in &attachment.entries {
            if entry.allocation.owner.quarantine_reason().is_some()
                || !self
                    .directional_persistent_sdma_attachment_is_current(&entry.allocation.attachment)
                || entry.allocation.attachment.pool_generation == 0
                || entry.allocation.byte_len() == 0
                || entry.allocation.byte_len() != entry.allocation.physical_byte_len()
                || entry.allocation.owner.byte_len() != entry.allocation.physical_byte_len()
                || entry.allocation.owner.local_native_for_sdma().is_some()
                || entry.allocation.owner.live_use_count() != 1
                || entry.allocation.owner.retained_settled_use_count() != 0
                || !matches!(
                    entry.state,
                    PersistentComputeUseStateV1::Prepared(_)
                        | PersistentComputeUseStateV1::Published(_)
                        | PersistentComputeUseStateV1::Completed(_)
                        | PersistentComputeUseStateV1::Recycled(_)
                )
            {
                return false;
            }
            let Some(identity) = entry.storage_identity else {
                return false;
            };
            if entry.allocation.attachment.storage_identity
                != Gfx942SdmaBufferStorageIdentityV1::Device(identity)
            {
                return false;
            }
            let Some(facts) = identity.coexistence_facts_v1() else {
                return false;
            };
            identities.push(identity);
            compute.push(facts);
        }
        self.dispatch
            .as_ref()
            .is_some_and(|dispatch| dispatch.persistent_device_roster_matches_v1(&identities))
            && r66_device_storage_rosters_disjoint_v1(
                self.coexistence_domain_v1(),
                &compute,
                &copy_devices,
            )
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{prepared_persistent_compute_cancellation_fixture, test_queue_key};
    use super::*;

    fn published_fixture() -> (ComputeAqlQueueSessionV1, Gfx942DeviceMemoryIdentityV1) {
        let queue = test_queue_key(0x6601, 1);
        let (mut session, _prepared, identity) =
            prepared_persistent_compute_cancellation_fixture(queue, 0x6602, Some([0x26; 32]), None);
        let entry = &mut session.persistent_compute.as_mut().unwrap().entries[0];
        assert!(publish_persistent_compute_entries_v1([entry]));
        (session, identity)
    }

    #[test]
    fn r66_prepared_and_published_compute_shape_are_distinct_without_native_authority() {
        let queue = test_queue_key(0x6601, 1);
        let (mut session, _prepared, identity) =
            prepared_persistent_compute_cancellation_fixture(queue, 0x6602, Some([0x26; 32]), None);
        assert_eq!(
            diagnose_r66_compute_attachment_v1(session.persistent_compute.as_ref(), queue, 2),
            Err(Gfx942R66NativeObservationFailureV1::ComputeAttachmentState)
        );
        let entry = &mut session.persistent_compute.as_mut().unwrap().entries[0];
        assert_eq!(diagnose_r66_compute_entry_v1(entry), Ok(identity));
        assert!(publish_persistent_compute_entries_v1([entry]));
        let before = session.completion_owner.state_snapshot_for_test();
        for _ in 0..8 {
            let attachment = session.persistent_compute.as_ref().unwrap();
            assert_eq!(
                diagnose_r66_compute_attachment_v1(Some(attachment), queue, 2),
                Ok(1)
            );
            assert_eq!(
                diagnose_r66_compute_entry_v1(&attachment.entries[0]),
                Ok(identity)
            );
            assert!(matches!(
                attachment.entries[0].state,
                PersistentComputeUseStateV1::Published(_)
            ));
            assert_eq!(attachment.entries[0].allocation.owner.live_use_count(), 1);
            assert_eq!(
                attachment.entries[0]
                    .allocation
                    .owner
                    .retained_settled_use_count(),
                0
            );
            assert!(
                attachment.entries[0]
                    .allocation
                    .owner
                    .local_native_for_sdma()
                    .is_none()
            );
            // This fixture holds real ledger tokens but no live SDMA rings or dispatch authority.
            assert_eq!(
                session.diagnose_r66_retained_counts_v1(),
                Err(Gfx942R66NativeObservationFailureV1::SdmaOwnerUnavailable)
            );
            assert_eq!(session.observe_r66_retained_counts_v1(), None);
            assert_eq!(session.completion_owner.state_snapshot_for_test(), before);
            assert!(!session.terminal_poisoned);
            assert_eq!(session.next_persistent_compute_generation, 2);
            assert_eq!(
                session
                    .persistent_compute_test_release
                    .as_ref()
                    .unwrap()
                    .1
                    .len(),
                1
            );
        }
    }

    #[test]
    fn r66_attachment_diagnostics_reject_one_coordinate_changes() {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        assert_eq!(
            diagnose_r66_compute_attachment_v1(None, test_queue_key(1, 1), 1),
            Ok(0)
        );
        for (coordinate, expected) in [
            (0, Failure::ComputeAttachmentShape),
            (1, Failure::ComputeAttachmentTerminal),
            (2, Failure::ComputeAttachmentQueue),
            (3, Failure::ComputeAttachmentGeneration),
            (4, Failure::ComputeAttachmentState),
        ] {
            let (mut session, _) = published_fixture();
            let attachment = session.persistent_compute.as_mut().unwrap();
            match coordinate {
                0 => {
                    attachment.entries.pop();
                }
                1 => {
                    attachment.terminal_custody =
                        Some(PersistentComputeTerminalNativeCustodyV1::Attached)
                }
                2 => attachment.binding.queue = test_queue_key(0x6603, 1),
                3 => attachment.binding.attachment_generation = u64::MAX,
                4 => attachment.entries[0].state = PersistentComputeUseStateV1::Quarantined,
                _ => unreachable!(),
            }
            for _ in 0..2 {
                assert_eq!(
                    diagnose_r66_compute_attachment_v1(
                        session.persistent_compute.as_ref(),
                        session.key,
                        2
                    ),
                    Err(expected)
                );
            }
        }
    }

    #[test]
    fn r66_compute_entry_diagnostics_reject_one_coordinate_changes() {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        for (coordinate, expected) in [
            (0, Failure::ComputeIdentityUnavailable),
            (1, Failure::ComputeQuarantined),
            (2, Failure::ComputePoolGeneration),
            (3, Failure::ComputeExtent),
            (4, Failure::ComputeExtent),
            (5, Failure::ComputeStorageIdentity),
        ] {
            let (mut session, _) = published_fixture();
            let entry = &mut session.persistent_compute.as_mut().unwrap().entries[0];
            match coordinate {
                0 => entry.storage_identity = None,
                1 => entry
                    .allocation
                    .owner
                    .quarantine_for_caller_reported_currentness_loss(),
                2 => entry.allocation.attachment.pool_generation = 0,
                3 => entry.allocation.attachment.logical_bytes = 0,
                4 => entry.allocation.attachment.logical_bytes -= 1,
                5 => {
                    let (foreign, _) =
                        crate::sdma::persistent_sdma_buffers_for_test(session.key, 0x6603);
                    entry.allocation.attachment.storage_identity = foreign.storage_identity();
                }
                _ => unreachable!(),
            }
            for _ in 0..2 {
                assert_eq!(diagnose_r66_compute_entry_v1(entry), Err(expected));
                assert!(matches!(
                    entry.state,
                    PersistentComputeUseStateV1::Published(_)
                ));
                assert_eq!(entry.allocation.owner.live_use_count(), 1);
            }
        }
    }

    #[test]
    fn r66_profile_failure_remains_distinct_from_missing_native_owner() {
        let (mut session, _) = published_fixture();
        session.compute_lane_session = test_queue_key(0x6604, 1);
        assert_eq!(
            session.diagnose_r66_retained_counts_v1(),
            Err(Gfx942R66NativeObservationFailureV1::PrimaryProfile)
        );
        assert_eq!(session.observe_r66_retained_counts_v1(), None);
        assert!(!session.terminal_poisoned);
        session.compute_lane_session = session.key;
        session.terminal_poisoned = true;
        assert_eq!(
            session.diagnose_r66_retained_counts_v1(),
            Err(Gfx942R66NativeObservationFailureV1::PrimaryProfile)
        );
    }

    #[test]
    fn r66_compute_custody_diagnostics_distinguish_live_settled_and_local_storage() {
        use Gfx942R66NativeObservationFailureV1 as Failure;
        let (mut session, _) = published_fixture();
        let entry = &mut session.persistent_compute.as_mut().unwrap().entries[0];
        let PersistentComputeUseStateV1::Published(published) =
            std::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined)
        else {
            panic!("fixture is published");
        };
        let completed = entry.allocation.owner.complete(published).unwrap();
        let frontier = entry.allocation.owner.settle(completed).unwrap();
        assert_eq!(
            diagnose_r66_compute_entry_v1(entry),
            Err(Failure::ComputeLiveUses)
        );
        let request = Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::ComputeReadWrite,
            0,
            entry.allocation.byte_len(),
        )
        .unwrap();
        let reserved = entry
            .allocation
            .owner
            .reserve(request, Some(&frontier))
            .unwrap();
        let prepared = entry.allocation.owner.prepare(reserved).unwrap();
        entry.state = PersistentComputeUseStateV1::Published(
            entry.allocation.owner.publish(prepared).unwrap(),
        );
        for _ in 0..2 {
            assert_eq!(
                diagnose_r66_compute_entry_v1(entry),
                Err(Failure::ComputeSettledUses)
            );
            assert_eq!(entry.allocation.owner.live_use_count(), 1);
            assert_eq!(entry.allocation.owner.retained_settled_use_count(), 1);
        }

        let (mut session, _) = published_fixture();
        let data = session
            .persistent_compute_test_release
            .as_mut()
            .unwrap()
            .1
            .pop()
            .unwrap();
        let super::super::super::dispatch_binding::DispatchDataInputStorageV1::Device(lease) =
            data.into_parts().storage
        else {
            panic!("fixture retains the detached device lease");
        };
        let entry = &mut session.persistent_compute.as_mut().unwrap().entries[0];
        entry
            .allocation
            .owner
            .restore_local_native_from_sdma(lease)
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                diagnose_r66_compute_entry_v1(entry),
                Err(Failure::ComputeNativeCustody)
            );
            assert_eq!(entry.allocation.owner.live_use_count(), 1);
            assert_eq!(entry.allocation.owner.retained_settled_use_count(), 0);
            assert!(entry.allocation.owner.local_native_for_sdma().is_some());
        }
    }
}
