use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

use crate::retained_resources::{RetainedPayload, RetainedResourcesV2};

struct DropWitness(Arc<AtomicUsize>);

impl DropWitness {
    // Includes the shared observer allocation and its two reference counts.
    const STORAGE: usize = size_of::<Self>() + size_of::<[AtomicUsize; 3]>();
}

impl Drop for DropWitness {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn tracked_pair() -> (
    RetainedResourcesV2<DropWitness>,
    RetainedPayload,
    Arc<AtomicUsize>,
) {
    let drops = Arc::new(AtomicUsize::new(0));
    let (handle, payload) =
        RetainedResourcesV2::pair(DropWitness(Arc::clone(&drops)), DropWitness::STORAGE).unwrap();
    assert_eq!(Arc::strong_count(&handle.owner), 2);
    assert_eq!(Arc::strong_count(&payload.owner), 2);
    (handle, payload, drops)
}

#[test]
fn dropping_handle_retains_value_until_slot_retires() {
    let (handle, payload, drops) = tracked_pair();
    drop(handle);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(Arc::strong_count(&payload.owner), 1);
    drop(payload);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn retiring_slot_retains_value_until_handle_drops() {
    let (handle, payload, drops) = tracked_pair();
    drop(payload);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(Arc::strong_count(&handle.owner), 1);
    assert!(Arc::ptr_eq(&handle.get().0, &drops));
    drop(handle);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn unwinding_handle_preserves_slot_owner() {
    let (handle, payload, drops) = tracked_pair();
    let result = catch_unwind(move || {
        let _handle = handle;
        panic!("request unwind");
    });
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(payload);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn unwinding_slot_preserves_typed_owner() {
    let (handle, payload, drops) = tracked_pair();
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _payload = payload;
        panic!("slot unwind");
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(Arc::ptr_eq(&handle.get().0, &drops));
    drop(handle);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn unwinding_both_owners_drops_value_exactly_once() {
    let (handle, payload, drops) = tracked_pair();
    let result = catch_unwind(AssertUnwindSafe(move || {
        let _owners = (handle, payload);
        panic!("both owners unwind");
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn charges_cover_full_owned_capacity_and_request_header() {
    let mut value = Vec::<u8>::with_capacity(1024);
    value.extend_from_slice(b"retained");
    let declared = size_of::<Vec<u8>>() + value.capacity();
    let quoted = RetainedResourcesV2::<Vec<u8>>::payload_storage(declared).unwrap();
    let full = RetainedResourcesV2::<Vec<u8>>::storage_for(declared).unwrap();
    assert!(quoted >= declared + size_of::<[AtomicUsize; 2]>());
    let (handle, payload) = RetainedResourcesV2::pair(value, declared).unwrap();
    assert_eq!(handle.get().as_slice(), b"retained");
    assert_eq!(payload.storage(), quoted);
    assert_eq!(
        full,
        quoted + size_of::<(RetainedResourcesV2<Vec<u8>>, usize)>()
    );
    assert_eq!(handle.retained_storage(), full);
    let growth = full.checked_sub(declared).unwrap();
    assert_eq!(
        declared.checked_add(growth),
        Some(handle.retained_storage())
    );
}

#[test]
fn payload_quota_rejects_short_floor_and_overflow() {
    for declared in [0, 31] {
        assert_eq!(
            RetainedResourcesV2::<[u8; 32]>::payload_storage(declared),
            Err(Resource::Accounting)
        );
        assert_eq!(
            RetainedResourcesV2::<[u8; 32]>::storage_for(declared),
            Err(Resource::Accounting)
        );
    }
    assert!(RetainedResourcesV2::<[u8; 32]>::payload_storage(32).is_ok());
    assert_eq!(
        RetainedResourcesV2::<[u8; 32]>::payload_storage(usize::MAX),
        Err(Resource::Arithmetic)
    );
    assert_eq!(
        RetainedResourcesV2::<[u8; 32]>::storage_for(usize::MAX),
        Err(Resource::Arithmetic)
    );
}

#[test]
fn construction_refusals_drop_original_value_once() {
    let overhead = RetainedResourcesV2::<DropWitness>::payload_storage(DropWitness::STORAGE)
        .unwrap()
        - DropWitness::STORAGE;
    let largest_payload = usize::MAX - overhead;
    assert_eq!(
        RetainedResourcesV2::<DropWitness>::payload_storage(largest_payload),
        Ok(usize::MAX)
    );
    assert_eq!(
        RetainedResourcesV2::<DropWitness>::storage_for(largest_payload),
        Err(Resource::Arithmetic)
    );
    for (declared, expected) in [
        (size_of::<DropWitness>() - 1, Resource::Accounting),
        (usize::MAX, Resource::Arithmetic),
        (largest_payload, Resource::Arithmetic), // Only the request header overflows.
    ] {
        let drops = Arc::new(AtomicUsize::new(0));
        let result = RetainedResourcesV2::pair(DropWitness(Arc::clone(&drops)), declared);
        assert!(matches!(result, Err(error) if error == expected));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn largest_full_request_charge_is_checked_without_wrapping() {
    let overhead = RetainedResourcesV2::<u8>::payload_storage(1).unwrap() - 1;
    let declared = usize::MAX - overhead - size_of::<(RetainedResourcesV2<u8>, usize)>();
    assert_eq!(
        RetainedResourcesV2::<u8>::storage_for(declared),
        Ok(usize::MAX)
    );
    assert_eq!(
        RetainedResourcesV2::<u8>::storage_for(declared + 1),
        Err(Resource::Arithmetic)
    );
    let (handle, payload) = RetainedResourcesV2::pair(7_u8, declared).unwrap();
    assert_eq!(*handle.get(), 7);
    assert_eq!(handle.retained_storage(), usize::MAX);
    assert_eq!(
        payload.storage(),
        RetainedResourcesV2::<u8>::payload_storage(declared).unwrap()
    );
    assert!(matches!(
        RetainedResourcesV2::pair(7_u8, declared + 1),
        Err(Resource::Arithmetic)
    ));
}

#[test]
fn alignment_padding_covers_large_alignment_tail_and_zero_sized_values() {
    #[repr(align(4096))]
    struct Aligned([u8; 3]);
    #[repr(align(4096))]
    struct AlignedZst;

    let declared = size_of::<Aligned>() + 23;
    let quote = RetainedResourcesV2::<Aligned>::payload_storage(declared).unwrap();
    assert!(quote >= declared + align_of::<Aligned>());
    let (handle, payload) = RetainedResourcesV2::pair(Aligned([1, 2, 3]), declared).unwrap();
    assert_eq!(handle.get().0, [1, 2, 3]);
    assert_eq!(payload.storage(), quote);

    assert!(
        RetainedResourcesV2::<u8>::payload_storage(1).unwrap() >= size_of::<[AtomicUsize; 3]>()
    );
    assert!(
        RetainedResourcesV2::<()>::payload_storage(0).unwrap() >= size_of::<[AtomicUsize; 2]>()
    );
    assert_eq!(size_of::<AlignedZst>(), 0);
    let (handle, payload) = RetainedResourcesV2::pair(AlignedZst, 0).unwrap();
    assert!(payload.storage() >= align_of::<AlignedZst>());
    assert_eq!(
        handle.retained_storage(),
        payload.storage() + size_of::<(RetainedResourcesV2<AlignedZst>, usize)>()
    );
}

#[test]
fn debug_is_opaque_without_requiring_value_debug() {
    let (handle, payload, drops) = tracked_pair();
    assert_eq!(
        format!("{handle:?}"),
        format!(
            "RetainedResourcesV2 {{ retained_storage: {}, .. }}",
            handle.retained_storage()
        )
    );
    drop((handle, payload));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
