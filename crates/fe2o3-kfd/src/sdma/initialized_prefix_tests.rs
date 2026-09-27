//! Native-neutral coverage transitions, not GPU completion evidence.

use super::*;

fn buffers(id: u64) -> (Gfx942SdmaBufferV1, Gfx942SdmaBufferV1) {
    persistent_sdma_buffers_for_test(super::tests::queue_key(7, 11, 13), id)
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
