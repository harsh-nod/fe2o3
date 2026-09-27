//! Native-neutral coverage transitions, not GPU completion evidence.

use super::*;
use crate::persistent_allocation::{
    Gfx942PersistentDeviceAllocationV1 as PersistentOwner,
    Gfx942PersistentNativeAllocationV1 as Native, Gfx942PersistentOperationV1 as Operation,
    Gfx942PersistentUseErrorV1 as UseError, Gfx942PersistentUseRequestV1 as UseRequest,
    detach_sdma_buffer_pair_v1, restore_sdma_buffer_pair_v1,
};

fn buffers(id: u64) -> (Gfx942SdmaBufferV1, Gfx942SdmaBufferV1) {
    persistent_sdma_buffers_for_test(super::tests::queue_key(7, 11, 13), id)
}

#[test]
fn initialized_prefix_direct_submit_revokes_digest_before_injected_completion() {
    use crate::shared_memory::{
        DataCleanupCustodyV1, DispatchDataReleaseV1, PreparationMemoryFixtureV1,
        SdmaResourceCleanupCustodyV1,
    };
    use sha2::{Digest, Sha256};

    struct Queue(Gfx942SdmaQueueSetV1);
    impl Drop for Queue {
        fn drop(&mut self) {
            retained_release::fixture::cleanup_set(&mut self.0);
        }
    }

    for known in [false, true] {
        let mut memory = PreparationMemoryFixtureV1::new(true);
        let key = super::tests::queue_key(7, 11, 13);
        let mut queue = Queue(retained_release::fixture::generic(&mut memory, key, None));
        let data = memory.device(known);
        let initialized = data.is_fully_initialized();
        let mut source =
            Gfx942SdmaBufferV1::from_bridge_parts(data.into_sdma_storage(), key, 1, 4096);
        if initialized {
            source.record_initialized_write(0, 4096, true);
        }
        let mut destination = Gfx942SdmaBufferV1::from_bridge_parts(
            memory.host(true).into_sdma_storage(),
            key,
            1,
            4096,
        );
        let digest: [u8; 32] = Sha256::digest([0x5a; 4096]).into();
        destination.certify_full_host_content(digest);
        let source_identity = source.storage_identity();
        let destination_identity = destination.storage_identity();
        let mut source_owner = PersistentOwner::from_sdma_buffer(source).unwrap();
        let source = source_owner.detach_sdma_buffer(key, 1, 4096).unwrap();
        memory.enable_sdma_mapped_bytes_v1();
        let ticket = queue
            .0
            .submit(&mut memory, source, 0, destination, 8, 16)
            .unwrap();
        let Gfx942SdmaQueueSetV1::Generic(owners) = &mut queue.0 else {
            unreachable!()
        };
        let owner = &mut owners[0];
        let record = owner.records[usize::from(ticket.slot)].as_ref().unwrap();
        assert_eq!(
            record.destination.certified_full_host_content_sha256(4096),
            None
        );
        assert_eq!(
            record.destination.initialized_prefix,
            if known { 4096 } else { 8 }
        );
        memory
            .overwrite_mapped_host_visible_subrange_in_current_scope(
                owner.completions.as_mut().unwrap(),
                u64::from(ticket.slot) * 8,
                &i64::from(ticket.generation).to_le_bytes(),
            )
            .unwrap();
        let SingleSdmaWaitInCurrentScopeV1::Completed(completed) = queue
            .0
            .wait_for_in_current_scope_with_final_currentness(&mut memory, ticket, Duration::ZERO)
        else {
            panic!("exact injected completion must retire the submitted record")
        };
        assert_eq!(completed.source.storage_identity(), source_identity);
        assert_eq!(
            completed.destination.storage_identity(),
            destination_identity
        );
        assert_eq!(
            completed
                .destination
                .certified_full_host_content_sha256(4096),
            None
        );
        assert_eq!(
            completed.destination.initialized_prefix,
            if known { 4096 } else { 8 }
        );
        source_owner.restore_sdma_buffer(completed.source).unwrap();
        let source = source_owner.detach_sdma_buffer(key, 1, 4096).unwrap();
        assert_eq!(source.initialized_range_is_known(0, 4096), known);
        for buffer in [source, completed.destination] {
            let mut cleanup = DataCleanupCustodyV1::from_sdma(buffer);
            memory.release_data(&mut cleanup).unwrap();
            assert!(cleanup.is_complete());
        }
        let Gfx942SdmaQueueSetV1::Generic(owners) = &mut queue.0 else {
            unreachable!()
        };
        let owner = &mut owners[0];
        assert!(owner.records.iter().all(Option::is_none));
        let mut cleanup = SdmaResourceCleanupCustodyV1::new_sdma(
            owner.completions.take().unwrap(),
            owner.control.take().unwrap().into_token(),
            owner.ring.take().unwrap().into_token(),
        );
        memory
            .primary_release_sdma_resources_v1(&mut cleanup)
            .unwrap();
        assert!(cleanup.is_complete());
        memory.primary_assert_all_released_v1();
    }
}

fn retire(request: Gfx942SdmaCopyRequestV1) -> Gfx942SdmaCompletedCopyV1 {
    Gfx942SdmaCompletedCopyV1::from_completed_record(SdmaCopyRecordV1 {
        directional_persistent: false,
        generation: 1,
        completion_value: 1,
        fence_header: SDMA_FENCE_SYSTEM_SNOOP_HEADER_V1,
        completion_observed: true,
        source: request.source,
        destination: request.destination,
        copy_bytes: request.copy_bytes,
        source_offset: request.source_offset,
        destination_offset: request.destination_offset,
    })
}

#[test]
fn initialized_prefix_request_does_not_mint_and_retirement_transfers_exact_source_range() {
    let (device, mut host) = buffers(100);
    host.record_initialized_write(0, 2048, true);
    let request = Gfx942SdmaCopyRequestV1::new(host, 512, device, 0, 1536);
    assert_eq!(request.destination.initialized_prefix, 0);
    let completed = retire(request);
    assert_eq!(completed.source.initialized_prefix, 2048);
    assert_eq!(completed.destination.initialized_prefix, 1536);

    let request =
        Gfx942SdmaCopyRequestV1::new(completed.source, 0, completed.destination, 1536, 2048);
    assert_eq!(request.destination.initialized_prefix, 1536);
    let completed = retire(request);
    assert_eq!(completed.destination.initialized_prefix, 3584);
    assert!(!completed.destination.initialized_range_is_known(0, 4096));

    let request =
        Gfx942SdmaCopyRequestV1::new(completed.source, 2048, completed.destination, 1024, 1);
    assert_eq!(request.destination.initialized_prefix, 1024);
    assert_eq!(retire(request).destination.initialized_prefix, 1024);
}

#[test]
fn initialized_prefix_known_overwrites_keep_coverage_but_not_digest() {
    let (mut device, mut host) = buffers(200);
    device.record_initialized_write(0, 4096, true);
    host.certify_full_host_content([5; 32]);
    let request = Gfx942SdmaCopyRequestV1::new(device, 0, host, 128, 256);
    assert_eq!(request.destination.initialized_prefix, 4096);
    assert_eq!(
        request.destination.certified_full_host_content_sha256(4096),
        None
    );
    assert_eq!(retire(request).destination.initialized_prefix, 4096);
}

#[test]
fn initialized_prefix_unknown_overwrites_invalidate_even_when_unpublished() {
    let (device, mut host) = buffers(300);
    host.certify_full_host_content([9; 32]);
    let request = Gfx942SdmaCopyRequestV1::new(device, 0, host, 16, 4);
    let (_, host) = request.into_buffers();
    assert_eq!(host.initialized_prefix, 16);
    assert!(host.initialized_range_is_known(0, 16));
    assert!(!host.initialized_range_is_known(16, 1));
}

#[test]
fn initialized_prefix_gaps_and_invalid_ranges_cannot_promote() {
    let (device, mut host) = buffers(400);
    host.record_initialized_write(0, 4096, true);
    let completed = retire(Gfx942SdmaCopyRequestV1::new(host, 0, device, 1024, 1024));
    assert_eq!(completed.destination.initialized_prefix, 0);
    let completed = retire(Gfx942SdmaCopyRequestV1::new(
        completed.source,
        u64::MAX,
        completed.destination,
        0,
        1,
    ));
    assert_eq!(completed.destination.initialized_prefix, 0);
}

#[test]
fn initialized_prefix_pool_resize_and_bare_reconstruction_reset() {
    let (_, mut host) = buffers(500);
    host.certify_full_host_content([1; 32]);
    host.pool_generation = u64::MAX;
    let snapshot = host.cleanup_metadata();
    assert!(host.advance_pool_generation().is_err());
    assert_eq!(host.cleanup_metadata(), snapshot);
    host.pool_generation = 1;
    host.advance_pool_generation().unwrap();
    assert_eq!(host.initialized_prefix, 0);
    host.certify_full_host_content([1; 32]);
    host.set_logical_bytes(2048);
    assert_eq!(host.initialized_prefix, 0);
    host.record_initialized_write(0, 2048, true);
    let (storage, owner, generation, logical) = host.into_bridge_parts();
    let host = Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, generation, logical);
    assert_eq!(host.initialized_prefix, 0);
}

#[test]
fn initialized_prefix_cleanup_and_wrong_kind_rejection_preserve_exact_custody() {
    let (_, mut host) = buffers(600);
    host.record_initialized_write(0, 512, true);
    let before = host.cleanup_metadata();
    let pair = crate::persistent_directional_sdma::Gfx942PersistentDirectionalSdmaPairV1 {
        host_to_device_queue_id: 12,
        device_to_host_queue_id: 13,
    };
    let host = crate::persistent_directional_sdma::promote_directional_persistent_sdma_custody_v1(
        host, pair, 1,
    )
    .unwrap_err();
    assert_eq!(host.cleanup_metadata(), before);
    let (_, metadata) = host.into_cleanup_parts();
    assert_eq!(metadata, before);
}

#[test]
fn initialized_prefix_failed_known_host_write_cannot_extend_coverage() {
    let (_, mut host) = buffers(700);
    host.record_initialized_write(0, 512, true);
    let result = host.replace_full_host_content_certificate(|_| {
        Err(Gfx942SdmaErrorV1::Contract("injected write failure"))
    });
    assert!(result.is_err());
    assert_eq!(host.initialized_prefix, 512);
    assert_eq!(host.certified_full_host_content_sha256(4096), None);
    host.replace_full_host_content_certificate(|_| Ok([8; 32]))
        .unwrap();
    assert!(host.initialized_range_is_known(0, 4096));
}

#[test]
fn initialized_prefix_raw_requests_cannot_preserve_stale_destination_digests() {
    for prepare in [false, true] {
        for known in [false, true] {
            let (mut device, mut host) = buffers(800);
            if known {
                device.record_initialized_write(0, 4096, true);
            }
            host.certify_full_host_content([3; 32]);
            // The direct submit API and private recovery paths can bypass new().
            let mut request = Gfx942SdmaCopyRequestV1 {
                source: device,
                destination: host,
                source_offset: 0,
                destination_offset: 8,
                copy_bytes: 16,
            };
            if prepare {
                request.prepare_destination_write();
                assert_eq!(
                    request.destination.certified_full_host_content_sha256(4096),
                    None
                );
                assert_eq!(
                    request.destination.initialized_prefix,
                    if known { 4096 } else { 8 }
                );
            }
            let completed = retire(request);
            assert_eq!(
                completed
                    .destination
                    .certified_full_host_content_sha256(4096),
                None
            );
            assert_eq!(
                completed.destination.initialized_prefix,
                if known { 4096 } else { 8 }
            );
        }
    }
}

#[test]
fn initialized_prefix_persistent_scope_rejection_and_raw_extraction_are_exact() {
    let (mut device, _) = buffers(900);
    device.set_logical_bytes(17);
    device.record_initialized_write(0, 17, true);
    let key = device.queue_owner();
    let generation = device.pool_generation();
    let before = device.cleanup_metadata();
    let mut owner = PersistentOwner::from_sdma_buffer(device).unwrap();
    let snapshot = owner.ownership_snapshot_for_test_v1();
    for (queue, generation, logical) in [
        (super::tests::queue_key(7, 11, 14), generation, 17),
        (key, generation + 1, 17),
        (key, generation, 4096),
        (key, generation, 0),
    ] {
        assert!(
            owner
                .detach_sdma_buffer(queue, generation, logical)
                .is_err()
        );
        assert_eq!(owner.ownership_snapshot_for_test_v1(), snapshot);
    }
    let device = owner.detach_sdma_buffer(key, generation, 17).unwrap();
    assert_eq!(device.cleanup_metadata(), before);
    assert!(device.initialized_range_is_known(0, 17));
    assert!(!device.initialized_range_is_known(0, 4096));
    let (error, mut owner) = owner.try_into_native().unwrap_err();
    assert_eq!(error, UseError::WrongState);
    owner.restore_sdma_buffer(device).unwrap();
    assert_eq!(owner.ownership_snapshot_for_test_v1(), snapshot);
    let Native::Local(lease) = owner.try_into_native().unwrap() else {
        unreachable!()
    };
    let mut owner = PersistentOwner::from_local_mapping(lease);
    assert_eq!(
        owner
            .detach_sdma_buffer(key, generation, 17)
            .unwrap()
            .initialized_prefix,
        0
    );
}

#[test]
fn initialized_prefix_directional_sequential_copies_survive_native_owner_transfers() {
    use crate::persistent_directional_sdma::{
        Gfx942PersistentDirectionalSdmaHostBindingV1 as HostBinding,
        Gfx942PersistentDirectionalSdmaPairV1 as Pair,
        promote_directional_persistent_sdma_custody_v1 as promote,
        restore_directional_persistent_sdma_request_v1 as restore,
    };
    use crate::persistent_sdma::Gfx942PersistentSdmaDirectionV1 as Direction;
    let (device, mut host) = buffers(1000);
    host.record_initialized_write(0, 4096, true);
    let (mut allocation, _) = promote(
        device,
        Pair {
            host_to_device_queue_id: 12,
            device_to_host_queue_id: 13,
        },
        1,
    )
    .unwrap();
    for (offset, len, prefix) in [(1024, 1024, 0), (0, 1024, 1024), (1024, 3072, 4096)] {
        let a = allocation.attachment;
        let host_binding = HostBinding::capture(&host, a.queue);
        let device = allocation
            .owner
            .detach_sdma_buffer(a.queue, a.pool_generation, a.logical_bytes)
            .unwrap();
        let completed = retire(Gfx942SdmaCopyRequestV1::new(
            host, offset, device, offset, len,
        ));
        let request = Gfx942SdmaCopyRequestV1 {
            source: completed.source,
            destination: completed.destination,
            source_offset: offset,
            destination_offset: offset,
            copy_bytes: len,
        };
        (allocation, host) = restore(
            allocation,
            Direction::HostToDevice,
            offset,
            offset,
            len,
            host_binding,
            request,
        )
        .unwrap_or_else(|_| panic!("exact directional restoration"));
        let device = allocation
            .owner
            .detach_sdma_buffer(a.queue, a.pool_generation, a.logical_bytes)
            .unwrap();
        assert_eq!(device.initialized_prefix, prefix);
        allocation.owner.restore_sdma_buffer(device).unwrap();
    }
    let a = allocation.attachment;
    let device = allocation
        .owner
        .detach_sdma_buffer(a.queue, a.pool_generation, a.logical_bytes)
        .unwrap();
    let completed = retire(Gfx942SdmaCopyRequestV1::new(device, 0, host, 0, 4096));
    assert!(completed.destination.initialized_range_is_known(0, 4096));
    assert_eq!(
        completed
            .destination
            .certified_full_host_content_sha256(4096),
        None
    );
}

#[test]
fn initialized_prefix_pair_transfer_is_atomic_and_unknown_writes_invalidate() {
    let (mut source, _) = buffers(1100);
    let (mut destination, _) = buffers(1200);
    source.record_initialized_write(0, 512, true);
    destination.record_initialized_write(0, 4096, true);
    let scope = (
        source.queue_owner(),
        source.pool_generation(),
        source.requested_bytes(),
    );
    let mut a = PersistentOwner::from_sdma_buffer(source).unwrap();
    let mut b = PersistentOwner::from_sdma_buffer(destination).unwrap();
    let snapshots = (
        a.ownership_snapshot_for_test_v1(),
        b.ownership_snapshot_for_test_v1(),
    );
    assert!(
        detach_sdma_buffer_pair_v1(&mut a, scope, &mut b, (scope.0, scope.1 + 1, scope.2)).is_err()
    );
    assert_eq!(
        (
            a.ownership_snapshot_for_test_v1(),
            b.ownership_snapshot_for_test_v1()
        ),
        snapshots
    );
    let (source, destination) = detach_sdma_buffer_pair_v1(&mut a, scope, &mut b, scope).unwrap();
    let metadata = (source.cleanup_metadata(), destination.cleanup_metadata());
    let snapshots = (
        a.ownership_snapshot_for_test_v1(),
        b.ownership_snapshot_for_test_v1(),
    );
    let (_, destination, source) =
        restore_sdma_buffer_pair_v1(&mut a, destination, &mut b, source).unwrap_err();
    assert_eq!(
        (
            a.ownership_snapshot_for_test_v1(),
            b.ownership_snapshot_for_test_v1()
        ),
        snapshots
    );
    assert_eq!(
        (source.cleanup_metadata(), destination.cleanup_metadata()),
        metadata
    );
    let completed = retire(Gfx942SdmaCopyRequestV1::new(
        source,
        512,
        destination,
        256,
        1,
    ));
    restore_sdma_buffer_pair_v1(&mut a, completed.source, &mut b, completed.destination).unwrap();
    let (source, destination) = detach_sdma_buffer_pair_v1(&mut a, scope, &mut b, scope).unwrap();
    assert_eq!(source.initialized_prefix, 512);
    assert_eq!(destination.initialized_prefix, 256);
}

#[test]
fn initialized_prefix_compute_cancellation_is_bound_to_detaching_use() {
    let (mut device, _) = buffers(1300);
    device.record_initialized_write(0, 4096, true);
    let scope = (
        device.queue_owner(),
        device.pool_generation(),
        device.requested_bytes(),
    );
    let mut owner = PersistentOwner::from_sdma_buffer(device).unwrap();
    let a = owner
        .reserve(UseRequest::new(Operation::ComputeRead, 0, 8).unwrap(), None)
        .unwrap();
    let a = owner.prepare(a).unwrap();
    let b = owner
        .reserve(UseRequest::new(Operation::ComputeRead, 8, 8).unwrap(), None)
        .unwrap();
    let b = owner.prepare(b).unwrap();
    let lease = owner.detach_local_native_for_compute(&a).unwrap();
    let snapshot = owner.ownership_snapshot_for_test_v1();
    assert!(owner.detach_sdma_buffer(scope.0, scope.1, scope.2).is_err());
    let (error, lease) = owner
        .restore_local_native_from_cancelled_compute(&b, lease)
        .unwrap_err();
    assert_eq!(error, UseError::WrongOwnerOrGeneration);
    assert_eq!(owner.ownership_snapshot_for_test_v1(), snapshot);
    owner
        .restore_local_native_from_cancelled_compute(&a, lease)
        .unwrap();
    owner.cancel_prepared(a).unwrap();
    owner.cancel_prepared(b).unwrap();
    let device = owner.detach_sdma_buffer(scope.0, scope.1, scope.2).unwrap();
    assert!(device.initialized_range_is_known(0, 4096));
    owner.restore_sdma_buffer(device).unwrap();
    let a = owner
        .reserve(
            UseRequest::new(Operation::ComputeReadWrite, 0, 4096).unwrap(),
            None,
        )
        .unwrap();
    let a = owner.prepare(a).unwrap();
    let lease = owner.detach_local_native_for_compute(&a).unwrap();
    let a = owner.publish(a).unwrap();
    let a = owner.complete(a).unwrap();
    owner.restore_local_native_from_compute(&a, lease).unwrap();
    let _frontier = owner.settle(a).unwrap();
    assert_eq!(
        owner
            .detach_sdma_buffer(scope.0, scope.1, scope.2)
            .unwrap()
            .initialized_prefix,
        0
    );
}

#[test]
fn initialized_prefix_bookkeeping_cannot_initialize_storage_and_pool_reset_is_exact() {
    use crate::persistent_directional_sdma::{
        Gfx942PersistentDirectionalSdmaPairV1 as Pair,
        demote_directional_persistent_sdma_custody_v1 as demote,
        promote_directional_persistent_sdma_custody_v1 as promote,
    };
    let (device, _) = buffers(1400);
    let scope = (
        device.queue_owner(),
        device.pool_generation(),
        device.requested_bytes(),
    );
    let mut owner = PersistentOwner::from_sdma_buffer(device).unwrap();
    let a = owner
        .reserve(
            UseRequest::new(Operation::LocalSdmaDestination, 0, 4096).unwrap(),
            None,
        )
        .unwrap();
    let a = owner.prepare(a).unwrap();
    let a = owner.publish(a).unwrap();
    let a = owner.complete(a).unwrap();
    let frontier = owner.settle(a).unwrap();
    owner.retire_settled_frontier(frontier).unwrap();
    let mut device = owner.detach_sdma_buffer(scope.0, scope.1, scope.2).unwrap();
    assert_eq!(device.initialized_prefix, 0);
    device.record_initialized_write(0, 4096, true);
    let pair = Pair {
        host_to_device_queue_id: 12,
        device_to_host_queue_id: 13,
    };
    let (mut allocation, _) = promote(device, pair, 1).unwrap();
    allocation.attachment.pool_generation = u64::MAX;
    let snapshot = allocation.owner.ownership_snapshot_for_test_v1();
    let (error, mut allocation) = demote(allocation, 1).unwrap_err();
    assert_eq!(error, UseError::GenerationExhausted);
    assert_eq!(allocation.owner.ownership_snapshot_for_test_v1(), snapshot);
    allocation.attachment.pool_generation = scope.1;
    let (device, _) = demote(allocation, 1).unwrap();
    assert_eq!(device.pool_generation(), scope.1 + 1);
    assert_eq!(device.initialized_prefix, 0);
    let (mut allocation, _) = promote(device, pair, 1).unwrap();
    assert_eq!(
        allocation
            .owner
            .detach_sdma_buffer(scope.0, scope.1 + 1, scope.2)
            .unwrap()
            .initialized_prefix,
        0
    );
}
