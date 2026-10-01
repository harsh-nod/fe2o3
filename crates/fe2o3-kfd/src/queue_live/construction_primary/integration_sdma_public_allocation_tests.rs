//! PUBLIC allocation uses the same constructed owners and settlement protocol.

use super::*;

fn allocate_public(
    f: &mut AllocationParent,
    bytes: u64,
) -> Result<Gfx942SdmaBufferV1, Gfx942SdmaAllocationFailureV1> {
    allocate_classified_in_place(
        f,
        SdmaAllocationRequestV1::PublicDevice {
            bytes,
            alignment: 4096,
        },
    )
}

#[test]
fn constructed_public_sdma_allocation_returns_native_public_owner_and_refunds() {
    for bytes in [1, 17, 4097] {
        let mut f = AllocationParent::new();
        let before = f
            .parent
            .engine
            .backend
            .session
            .insertion_memory_snapshot_v1();
        let buffer = allocate_public(&mut f, bytes).unwrap();
        assert_eq!(
            buffer.device_allocation_flags_v1(),
            Some(fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC.bits())
        );
        assert_eq!(buffer.requested_bytes(), bytes);
        assert_eq!(buffer.physical_bytes(), bytes.next_multiple_of(4096));
        assert!(buffer.belongs_to(f.parent.key));
        assert_eq!(buffer.pool_generation(), 1);
        assert!(f.custody.is_none() && !f.parent.poisoned);
        assert_eq!(f.outstanding, 1);
        assert_eq!(f.calls, ["loan", "retake"]);
        f.parent
            .engine
            .backend
            .session
            .insertion_assert_native_prefix_with_layout_v1(
                &before,
                DeviceInsertionPrefixV1 {
                    calls: [4, 1, 1, 0, 1],
                    phase: Some("Mapped"),
                    handle: true,
                    cpu_writable: None,
                    written: false,
                    operations: &["map_gpu"],
                },
                device_memory_layout(
                    bytes.next_multiple_of(4096),
                    4096,
                    fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap(),
                None,
            );
        f.release(buffer);
        f.shutdown();
    }
}

#[test]
fn constructed_public_sdma_allocation_retake_failures_retain_exact_public_owner() {
    for fault in [
        Fault::Error,
        Fault::Panic,
        Fault::AfterError,
        Fault::AfterPanic,
        Fault::Regression,
    ] {
        let mut f = AllocationParent::new();
        f.closing = fault;
        let result = catch_unwind(AssertUnwindSafe(|| allocate_public(&mut f, 17)));
        if matches!(fault, Fault::Panic | Fault::AfterPanic) {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().err().unwrap().disposition(),
                Gfx942SdmaAllocationDispositionV1::ProcessTeardown
            );
        }
        assert_eq!(f.outstanding, 0);
        assert!(f.parent.poisoned);
        assert_eq!(f.calls, ["loan", "retake"]);
        let SdmaAllocationCustodyV1::Device { allocation, .. } = f.custody.as_ref().unwrap() else {
            panic!("device custody required")
        };
        let snapshot = allocation.insertion_snapshot_for_test();
        assert!(snapshot.started() && snapshot.native_started() && !snapshot.failed());
        let (_, layout, mapped) = snapshot.lease().unwrap();
        assert!(mapped);
        assert_eq!(
            layout.uapi_flags(),
            fe2o3_kfd_uapi::KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC.bits()
        );
        f.no_retry();
        assert!(allocate_public(&mut f, 17).is_err());
        f.no_retry();
    }
}

#[test]
fn constructed_public_sdma_capacity_rejection_is_settled_and_retryable() {
    let mut f = AllocationParent::new();
    let before = f
        .parent
        .engine
        .backend
        .session
        .insertion_memory_snapshot_v1();
    let accounting = f.parent.engine.backend.session.observation();
    let failure = allocate_public(&mut f, (1 << 20) + 4096).err().unwrap();
    assert_eq!(
        failure.disposition(),
        Gfx942SdmaAllocationDispositionV1::RetryableCapacity
    );
    assert!(matches!(
        failure.into_error(),
        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(
            MemorySessionError::DeviceBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            )
        ))
    ));
    assert!(f.capacity_retry_is_settled());
    assert!(
        f.parent
            .engine
            .backend
            .session
            .insertion_memory_snapshot_v1()
            == before
    );
    assert!(f.parent.engine.backend.session.observation() == accounting);
    assert_eq!(f.outstanding, 0);
    let buffer = allocate_public(&mut f, 17).unwrap();
    f.release(buffer);
    f.shutdown();
}
