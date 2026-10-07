use super::*;
use crate::persistent_compute::{
    BoundedPersistentComputeAttachmentV1, PersistentComputeCancellationCustodyV1,
};
use crate::shared_memory::{local_mapping_for_persistent_sdma_test, xgmi_mapping_for_sdma_test};

fn owner(id: u64) -> Gfx942PersistentDeviceAllocationV1 {
    match Gfx942PersistentDeviceAllocationV1::from_exact_two_device_peer_mapping(
        xgmi_mapping_for_sdma_test(id),
    ) {
        Ok(owner) => owner,
        Err(_) => panic!("test fixture is an exact complete peer mapping"),
    }
}

fn request(
    operation: Gfx942PersistentOperationV1,
    offset: u64,
    byte_len: u64,
) -> Gfx942PersistentUseRequestV1 {
    Gfx942PersistentUseRequestV1::new(operation, offset, byte_len).unwrap()
}

fn settle(
    owner: &mut Gfx942PersistentDeviceAllocationV1,
    lease: Gfx942PersistentUseLeaseV1<Gfx942PersistentReservedV1>,
) -> Gfx942PersistentDependencyFrontierV1 {
    let lease = owner.prepare(lease).unwrap();
    let lease = owner.publish(lease).unwrap();
    let lease = owner.complete(lease).unwrap();
    owner.settle(lease).unwrap()
}

#[test]
fn persistent_owner_and_queue_layouts_keep_ledgers_out_of_inline_custody() {
    assert!(std::mem::size_of::<Gfx942PersistentDeviceAllocationV1>() <= 512);
    let ledger_bytes =
        std::mem::size_of::<[Option<LedgerRecordV1>; GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1]>();
    let state_bytes = std::mem::size_of::<PersistentOwnerStateV1>();
    assert!(state_bytes <= ledger_bytes + 1024);
    assert!(std::mem::size_of::<PersistentComputeCancellationCustodyV1>() <= 8 * 1024);
    assert!(std::mem::size_of::<BoundedPersistentComputeAttachmentV1>() <= 32 * 1024);
    // Capacity configuration and table-credit custody add fixed metadata;
    // neither the 64-slot nor the 1024-slot table may be inline here.
    let bytes = std::mem::size_of::<crate::ComputeAqlQueueSessionV1>();
    assert!(bytes <= 41 * 1024, "queue inline bytes: {bytes}");
    eprintln!(
        "owner_inline={} owner_heap={state_bytes} ledger={ledger_bytes} queue_inline={bytes}",
        std::mem::size_of::<Gfx942PersistentDeviceAllocationV1>()
    );
    assert!(std::mem::size_of::<fe2o3_resource_accounting::HostMetadataTableV1<u64>>() <= 64);
}

#[test]
fn ledger_storage_survives_owner_moves_and_transitions() {
    for mut allocation in [
        Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            local_mapping_for_persistent_sdma_test(31),
        ),
        owner(32),
    ] {
        let binding = allocation.binding;
        let ledger = allocation.state.ledger.as_ptr();
        assert_eq!(
            allocation.state.ledger.len(),
            GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1
        );
        let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 8);
        let reserved = allocation.reserve(read, None).unwrap();
        allocation = std::hint::black_box(allocation);
        assert_eq!(allocation.state.ledger.as_ptr(), ledger);
        assert_eq!(allocation.live_use_count(), 1);
        let (error, recovered) = allocation.try_into_native().unwrap_err();
        assert_eq!(error, Gfx942PersistentUseErrorV1::OutstandingUses);
        allocation = recovered;
        assert_eq!(allocation.state.ledger.as_ptr(), ledger);
        let prepared = allocation.prepare(reserved).unwrap();
        allocation.cancel_prepared(prepared).unwrap();

        let write = request(Gfx942PersistentOperationV1::ComputeReadWrite, 0, 8);
        let reserved = allocation.reserve(write, None).unwrap();
        let frontier = settle(&mut allocation, reserved);
        assert_eq!(allocation.retained_settled_use_count(), 1);
        allocation = std::hint::black_box(allocation);
        allocation.retire_settled_frontier(frontier).unwrap();
        assert_eq!(allocation.state.ledger.as_ptr(), ledger);
        assert!(allocation.state.ledger.iter().all(Option::is_none));
        let native_binding = match allocation.try_into_native().unwrap() {
            Gfx942PersistentNativeAllocationV1::Local(lease) => lease.storage_identity(),
            Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(mapping) => {
                mapping.lease().storage_identity()
            }
        };
        assert_eq!(native_binding, binding);
    }
}

#[test]
fn exact_peer_form_and_operation_access_are_checked() {
    let mut owner = owner(1);
    assert_eq!(
        owner.mapping_form(),
        Gfx942PersistentMappingFormV1::ExactTwoDevicePeer { gpu_ids: [7, 9] }
    );
    let request = request(Gfx942PersistentOperationV1::PeerMappedSource, 0, 8);
    assert_eq!(request.owner(), Gfx942PersistentUseOwnerV1::PeerMapped);
    assert_eq!(request.access(), Gfx942PersistentAccessV1::Read);
    assert!(owner.reserve(request, None).is_ok());
    assert!(
        Gfx942PersistentUseRequestV1::new(Gfx942PersistentOperationV1::ComputeRead, u64::MAX, 2)
            .is_err()
    );
}

#[test]
fn capacity_and_allocation_bounds_are_preflighted() {
    let mut owner = owner(2);
    let outside = request(Gfx942PersistentOperationV1::ComputeRead, 4095, 2);
    assert_eq!(
        owner.reserve(outside, None).unwrap_err().error(),
        Gfx942PersistentUseErrorV1::InvalidRange
    );
    let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 1);
    let mut leases = Vec::new();
    for _ in 0..GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1 {
        leases.push(owner.reserve(read, None).unwrap());
    }
    assert_eq!(
        owner.reserve(read, None).unwrap_err().error(),
        Gfx942PersistentUseErrorV1::Capacity
    );
    assert_eq!(leases.len(), GFX942_MAX_PERSISTENT_ALLOCATION_USES_V1);
}

#[test]
fn owner_and_generation_substitution_recover_move_only_custody() {
    let mut first = owner(3);
    let mut second = owner(4);
    let lease = first
        .reserve(
            request(Gfx942PersistentOperationV1::ComputeRead, 0, 8),
            None,
        )
        .unwrap();
    let failure = second.prepare(lease).unwrap_err();
    assert_eq!(
        failure.error(),
        Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration
    );
    let (_, mut lease) = failure.into_parts();
    lease.generation += 1;
    let failure = first.prepare(lease).unwrap_err();
    assert_eq!(
        failure.error(),
        Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration
    );
}

#[test]
fn reads_coexist_writers_conflict_and_disjoint_writers_coexist() {
    let mut owner = owner(5);
    let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 16);
    let first = owner.reserve(read, None).unwrap();
    let second = owner.reserve(read, None).unwrap();
    let writer = request(Gfx942PersistentOperationV1::LocalSdmaDestination, 8, 4);
    assert_eq!(
        owner.reserve(writer, None).unwrap_err().error(),
        Gfx942PersistentUseErrorV1::OverlappingWriterActive
    );
    let disjoint = request(Gfx942PersistentOperationV1::LocalSdmaDestination, 32, 4);
    let disjoint = owner.reserve(disjoint, None).unwrap();
    owner.cancel_reserved(first).unwrap();
    owner.cancel_reserved(second).unwrap();
    owner.cancel_reserved(disjoint).unwrap();
}

#[test]
fn overlapping_writer_requires_exact_current_successful_frontier() {
    let mut owner = owner(6);
    let first = owner
        .reserve(
            request(Gfx942PersistentOperationV1::ComputeRead, 0, 16),
            None,
        )
        .unwrap();
    let frontier = settle(&mut owner, first);
    let writer = request(Gfx942PersistentOperationV1::ComputeWrite, 0, 16);
    assert_eq!(
        owner.reserve(writer, None).unwrap_err().error(),
        Gfx942PersistentUseErrorV1::DependencyRequired
    );
    let mut other = self::owner(7);
    let other_lease = other
        .reserve(
            request(Gfx942PersistentOperationV1::ComputeRead, 0, 16),
            None,
        )
        .unwrap();
    let other_frontier = settle(&mut other, other_lease);
    assert_eq!(
        owner
            .reserve(writer, Some(&other_frontier))
            .unwrap_err()
            .error(),
        Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency
    );
    let writer = owner.reserve(writer, Some(&frontier)).unwrap();
    let newer = settle(&mut owner, writer);
    assert!(newer.through_sequence() > frontier.through_sequence());
    let read = request(Gfx942PersistentOperationV1::ComputeRead, 0, 16);
    assert_eq!(
        owner.reserve(read, Some(&frontier)).unwrap_err().error(),
        Gfx942PersistentUseErrorV1::StaleOrSubstitutedDependency
    );
    assert!(owner.reserve(read, Some(&newer)).is_ok());
}

#[test]
fn prepublication_cancel_reclaims_slots() {
    let mut owner = owner(8);
    let request = request(Gfx942PersistentOperationV1::ComputeRead, 0, 8);
    let reserved = owner.reserve(request, None).unwrap();
    owner.cancel_reserved(reserved).unwrap();
    let reserved = owner.reserve(request, None).unwrap();
    let prepared = owner.prepare(reserved).unwrap();
    owner.cancel_prepared(prepared).unwrap();
    assert_eq!(owner.live_use_count(), 0);
    assert_eq!(owner.retained_settled_use_count(), 0);
}

#[test]
fn timeout_retains_exact_published_custody() {
    let mut owner = owner(9);
    let request = request(Gfx942PersistentOperationV1::LocalSdmaSource, 12, 8);
    let lease = owner.reserve(request, None).unwrap();
    let lease = owner.prepare(lease).unwrap();
    let lease = owner.publish(lease).unwrap();
    let timeout = owner.observe_timeout(lease).unwrap();
    assert_eq!(timeout.request(), request);
    assert_eq!(owner.live_use_count(), 1);
    let lease = owner.complete(timeout.into_published()).unwrap();
    let _frontier = owner.settle(lease).unwrap();
}

#[test]
fn indeterminate_publication_quarantines_and_blocks_release() {
    let mut owner = owner(10);
    let lease = owner
        .reserve(
            request(Gfx942PersistentOperationV1::PeerMappedDestination, 0, 8),
            None,
        )
        .unwrap();
    let lease = owner.prepare(lease).unwrap();
    let lease = owner.publish(lease).unwrap();
    owner
        .quarantine_published(
            lease,
            Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
        )
        .unwrap();
    assert_eq!(
        owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
    );
    let (error, owner) = owner.try_into_native().unwrap_err();
    assert_eq!(error, Gfx942PersistentUseErrorV1::Quarantined);
    assert_eq!(owner.live_use_count(), 1);
}

#[test]
fn caller_reported_currentness_loss_quarantines_without_claiming_observation() {
    let mut owner = owner(12);
    owner.quarantine_for_caller_reported_currentness_loss();
    assert_eq!(
        owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
    );
    let (error, _) = owner.try_into_native().unwrap_err();
    assert_eq!(error, Gfx942PersistentUseErrorV1::Quarantined);
}

#[test]
fn prepared_indeterminate_use_is_quarantined_without_fake_publication() {
    let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        local_mapping_for_persistent_sdma_test(13),
    );
    let reserved = owner
        .reserve(
            request(Gfx942PersistentOperationV1::LocalSdmaDestination, 0, 8),
            None,
        )
        .unwrap();
    let prepared = owner.prepare(reserved).unwrap();
    owner
        .quarantine_prepared(
            prepared,
            Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
        )
        .unwrap();
    assert_eq!(
        owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate)
    );
    let (error, _) = owner.try_into_native().unwrap_err();
    assert_eq!(error, Gfx942PersistentUseErrorV1::Quarantined);
}

#[test]
fn sdma_detach_restore_requires_exact_local_native_identity() {
    let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        local_mapping_for_persistent_sdma_test(14),
    );
    let lease = owner.detach_local_native_for_sdma().unwrap();
    assert!(!owner.local_native_is_attached_for_sdma());
    let foreign = local_mapping_for_persistent_sdma_test(15);
    let (error, foreign) = owner.restore_local_native_from_sdma(foreign).unwrap_err();
    assert_eq!(error, Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
    assert!(!owner.local_native_is_attached_for_sdma());
    owner.restore_local_native_from_sdma(lease).unwrap();
    assert!(owner.local_native_is_attached_for_sdma());
    assert_eq!(foreign.layout().requested_bytes(), owner.byte_len());
}

#[test]
fn compute_detach_cancel_and_completion_restore_the_exact_mapping() {
    let mut owner = Gfx942PersistentDeviceAllocationV1::from_local_mapping(
        local_mapping_for_persistent_sdma_test(16),
    );
    let request = request(Gfx942PersistentOperationV1::ComputeReadWrite, 0, 4096);
    let reserved = owner.reserve(request, None).unwrap();
    let prepared = owner.prepare(reserved).unwrap();
    let lease = owner.detach_local_native_for_compute(&prepared).unwrap();
    let foreign = local_mapping_for_persistent_sdma_test(17);
    let (error, _foreign) = owner
        .restore_local_native_from_cancelled_compute(&prepared, foreign)
        .unwrap_err();
    assert_eq!(error, Gfx942PersistentUseErrorV1::WrongOwnerOrGeneration);
    owner
        .restore_local_native_from_cancelled_compute(&prepared, lease)
        .unwrap();
    owner.cancel_prepared(prepared).unwrap();

    let reserved = owner.reserve(request, None).unwrap();
    let prepared = owner.prepare(reserved).unwrap();
    let lease = owner.detach_local_native_for_compute(&prepared).unwrap();
    let published = owner.publish(prepared).unwrap();
    let completed = owner.complete(published).unwrap();
    owner
        .restore_local_native_from_compute(&completed, lease)
        .unwrap();
    let _frontier = owner.settle(completed).unwrap();
    assert!(owner.local_native_is_attached_for_sdma());
}

#[test]
fn release_requires_settlement_and_returns_exact_native_owner() {
    let mut owner = owner(11);
    let lease = owner
        .reserve(
            request(Gfx942PersistentOperationV1::ComputeRead, 0, 8),
            None,
        )
        .unwrap();
    let (error, mut owner) = owner.try_into_native().unwrap_err();
    assert_eq!(error, Gfx942PersistentUseErrorV1::OutstandingUses);
    let frontier = settle(&mut owner, lease);
    assert_eq!(owner.retained_settled_use_count(), 1);
    owner.retire_settled_frontier(frontier).unwrap();
    assert_eq!(owner.retained_settled_use_count(), 0);
    assert!(matches!(
        owner.try_into_native().unwrap(),
        Gfx942PersistentNativeAllocationV1::ExactTwoDevicePeer(_)
    ));
}
