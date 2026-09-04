use crate::*;

fn digest(byte: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([byte; IDENTITY_DIGEST_BYTES_V1])
}

fn device(physical: u64, generation: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(physical),
        generation: DeviceGenerationV1(generation),
    }
}

#[test]
fn best_fit_reuse_advances_generation_only_after_release() {
    let mut pool = MemoryPoolModelV1::new_model_only(
        digest(1),
        device(7, 3),
        MemoryKindV1::DeviceLocal,
        4096,
        4,
    )
    .unwrap();
    let large = pool.lease_model_only(1024, 256).unwrap();
    let large_identity = large.identity();
    pool.release_model_only(large).unwrap();
    let small = pool.lease_model_only(512, 128).unwrap();
    assert_eq!(small.identity().block_id(), large_identity.block_id());
    assert_eq!(
        small.identity().generation(),
        large_identity.generation() + 1
    );
    assert_eq!(pool.committed_bytes(), 1024);
    pool.release_model_only(small).unwrap();
    assert_eq!(pool.retained_block_count(), 1);
    assert_eq!(pool.non_reusable_block_count(), 0);
    assert!(pool.validate_global_invariants().is_ok());
}

#[test]
fn best_fit_excludes_storage_with_insufficient_alignment() {
    let mut pool = MemoryPoolModelV1::new_model_only(
        digest(6),
        device(7, 4),
        MemoryKindV1::DeviceLocal,
        4096,
        4,
    )
    .unwrap();
    let low_alignment = pool.lease_model_only(256, 64).unwrap();
    let low_id = low_alignment.identity().block_id();
    let high_alignment = pool.lease_model_only(512, 256).unwrap();
    let high_id = high_alignment.identity().block_id();
    pool.release_model_only(low_alignment).unwrap();
    pool.release_model_only(high_alignment).unwrap();
    let selected = pool.lease_model_only(128, 128).unwrap();
    assert_ne!(selected.identity().block_id(), low_id);
    assert_eq!(selected.identity().block_id(), high_id);
}

#[test]
fn in_flight_and_quarantined_storage_never_reenters_free_set() {
    let mut pool = MemoryPoolModelV1::new_model_only(
        digest(2),
        device(8, 1),
        MemoryKindV1::HostVisibleCoherent,
        4096,
        4,
    )
    .unwrap();
    let first = pool.lease_model_only(256, 64).unwrap();
    let first_id = first.identity();
    let first = pool.mark_in_flight_model_only(first).unwrap();
    let failure = pool.release_model_only(first).unwrap_err();
    assert_eq!(failure.error(), MemoryPoolErrorV1::IllegalTransition);
    let first = failure.into_lease();
    pool.quarantine_model_only(first).unwrap();
    let second = pool.lease_model_only(128, 64).unwrap();
    assert_ne!(second.identity().block_id(), first_id.block_id());
    assert_eq!(pool.trim_model_only().unwrap(), 0);
}

#[test]
fn completion_is_required_before_releasing_published_storage() {
    let mut pool = MemoryPoolModelV1::new_model_only(
        digest(3),
        device(9, 5),
        MemoryKindV1::DeviceLocal,
        1024,
        1,
    )
    .unwrap();
    let lease = pool.lease_model_only(100, 64).unwrap();
    let lease = pool.mark_in_flight_model_only(lease).unwrap();
    let lease = pool.observe_completion_model_only(lease).unwrap();
    pool.release_model_only(lease).unwrap();
    assert_eq!(pool.trim_model_only().unwrap(), 128);
    assert_eq!(pool.committed_bytes(), 0);
    assert!(pool.blocks().is_empty());
}

#[test]
fn exact_alignment_does_not_overallocate_an_extra_block() {
    let mut pool = MemoryPoolModelV1::new_model_only(
        digest(7),
        device(10, 1),
        MemoryKindV1::DeviceLocal,
        4096,
        1,
    )
    .unwrap();
    let lease = pool.lease_model_only(4096, 4096).unwrap();
    assert_eq!(pool.committed_bytes(), 4096);
    assert_eq!(pool.blocks()[0].byte_len(), 4096);
    pool.release_model_only(lease).unwrap();
}

#[test]
fn capacity_and_identity_coordinates_fail_closed() {
    assert_eq!(
        MemoryPoolModelV1::new_model_only(
            digest(0),
            device(1, 1),
            MemoryKindV1::DeviceLocal,
            1,
            1,
        )
        .unwrap_err(),
        MemoryPoolErrorV1::InvalidIdentity
    );
    assert_eq!(
        MemoryPoolModelV1::new_model_only(
            digest(1),
            device(0, 1),
            MemoryKindV1::DeviceLocal,
            1,
            1,
        )
        .unwrap_err(),
        MemoryPoolErrorV1::InvalidIdentity
    );
    assert_eq!(
        MemoryPoolModelV1::new_model_only(
            digest(1),
            device(1, 0),
            MemoryKindV1::DeviceLocal,
            1,
            1,
        )
        .unwrap_err(),
        MemoryPoolErrorV1::InvalidIdentity
    );
    let mut first = MemoryPoolModelV1::new_model_only(
        digest(4),
        device(1, 1),
        MemoryKindV1::DeviceLocal,
        64,
        1,
    )
    .unwrap();
    let mut second = MemoryPoolModelV1::new_model_only(
        digest(5),
        device(2, 1),
        MemoryKindV1::DeviceLocal,
        64,
        1,
    )
    .unwrap();
    let lease = first.lease_model_only(64, 64).unwrap();
    let failure = second.mark_in_flight_model_only(lease).unwrap_err();
    assert_eq!(failure.error(), MemoryPoolErrorV1::UnknownLease);
    let lease = failure.into_lease();
    first.release_model_only(lease).unwrap();
    assert!(matches!(
        first.lease_model_only(65, 1),
        Err(MemoryPoolErrorV1::CapacityExceeded)
    ));
}
