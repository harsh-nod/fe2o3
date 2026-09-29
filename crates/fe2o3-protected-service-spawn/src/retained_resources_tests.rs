use std::cell::Cell;
use std::mem::{align_of, size_of};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, TryLockError};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

use crate::retained_resources::{
    RetainedDependencyV2, RetainedPayload, RetainedResourceAccessErrorV2 as Access,
    RetainedResourcesV2,
};

#[derive(Debug, Eq, PartialEq)]
enum Error {
    Resource(Resource),
    Access(Access),
    Callback,
}

impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl From<Access> for Error {
    fn from(error: Access) -> Self {
        Self::Access(error)
    }
}

// Independent by-value layout witness for reference counts and the mutex payload.
#[repr(C)]
struct SharedAllocation<T> {
    _strong: AtomicUsize,
    _weak: AtomicUsize,
    _value: Mutex<T>,
}

fn observe<T: Send + 'static, R>(
    handle: &RetainedResourcesV2<T>,
    operation: impl FnOnce(&T) -> R,
) -> R {
    let mut work = Work::new(RetainedResourcesV2::<T>::ACCESS_WORK);
    let mut budget = Budget::new(
        &mut work,
        handle.retained_storage() + RetainedResourcesV2::<T>::ACCESS_SCRATCH,
    );
    budget.reserve_storage(handle.retained_storage()).unwrap();
    handle
        .with::<_, Error>(&mut budget, |value, _| Ok(operation(value)))
        .unwrap()
}

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
    assert!(observe(&handle, |value| Arc::ptr_eq(&value.0, &drops)));
    drop(handle);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn dependencies_share_original_backing_until_the_last_independent_drop() {
    for order in [[0, 1, 2, 3], [1, 0, 3, 2], [2, 3, 0, 1], [2, 0, 1, 3]] {
        let (handle, payload, drops) = tracked_pair();
        let mut work = Work::new(2 * RetainedResourcesV2::<DropWitness>::DEPENDENCY_WORK);
        let mut b = Budget::new(&mut work, 1_000_000);
        let source = handle.retained_storage();
        b.reserve_storage(source).unwrap();
        let first = handle.retain_dependency(&mut b).unwrap();
        let charge = first.retained_storage();
        assert_eq!(b.storage(), source, "output must be fully unreserved");
        assert_eq!(
            charge,
            payload.storage() + size_of::<(RetainedDependencyV2<DropWitness>, usize)>()
        );
        b.reserve_storage(charge).unwrap();
        let second = handle.retain_dependency(&mut b).unwrap();
        assert_eq!(b.storage(), source + charge);
        b.reserve_storage(second.retained_storage()).unwrap();
        assert!(Arc::ptr_eq(&handle.owner, &first.owner));
        assert!(Arc::ptr_eq(&handle.owner, &second.owner));
        assert_eq!(Arc::strong_count(&handle.owner), 4);

        let (mut handle, mut payload) = (Some(handle), Some(payload));
        let (mut first, mut second) = (Some(first), Some(second));
        for (step, owner) in order.into_iter().enumerate() {
            match owner {
                0 => {
                    drop(handle.take());
                    b.release_storage(source).unwrap();
                }
                1 => drop(payload.take()),
                2 => {
                    drop(first.take());
                    b.release_storage(charge).unwrap();
                }
                _ => {
                    // The issuer's cleanup may retire on another thread.
                    let second = second.take().unwrap();
                    std::thread::spawn(move || drop(second)).join().unwrap();
                    b.release_storage(charge).unwrap();
                }
            }
            assert_eq!(drops.load(Ordering::SeqCst), usize::from(step == 3));
        }
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn dependency_funding_is_exact_and_refusal_never_changes_refcounts() {
    type Handle = RetainedResourcesV2<DropWitness>;
    let (handle, payload, drops) = tracked_pair();
    let full = handle.retained_storage();
    let charge = payload.storage() + size_of::<(RetainedDependencyV2<DropWitness>, usize)>();
    for case in 0..5 {
        let quota = if case == 4 {
            7
        } else {
            Handle::DEPENDENCY_WORK - usize::from(case == 2)
        };
        let limit = full + charge + Handle::DEPENDENCY_SCRATCH - usize::from(case == 3);
        let floor = full - usize::from(case == 1);
        let mut work = Work::new(19 + quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(floor).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = handle.retain_dependency(&mut b);
        assert_eq!(b.storage(), floor);
        assert!(ledger == b.work_ledger_identity_v1());
        if case == 0 {
            let dependency = result.unwrap();
            assert_eq!(dependency.retained_storage(), charge);
            assert_eq!(b.work(), 19 + quota);
            assert_eq!(b.peak_storage(), limit);
            assert_eq!(Arc::strong_count(&handle.owner), 3);
            drop(dependency);
        } else {
            match (case, result) {
                (1, Err(Resource::Accounting))
                | (2 | 4, Err(Resource::Work(_)))
                | (3, Err(Resource::Storage(_))) => {}
                _ => panic!("wrong dependency refusal: {case}"),
            }
            assert_eq!(b.peak_storage(), floor);
            if case == 3 {
                assert_eq!(b.failed_storage(), Some(limit + 1));
            }
        }
        assert_eq!(Arc::strong_count(&handle.owner), 2);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn issuer_cleanup_payload_keeps_transitive_backing_after_source_retirement() {
    let (handle, compiler_payload, drops) = tracked_pair();
    let mut work = Work::new(RetainedResourcesV2::<DropWitness>::DEPENDENCY_WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(handle.retained_storage()).unwrap();
    let dependency = handle.retain_dependency(&mut b).unwrap();
    let full = dependency.retained_storage();
    b.reserve_storage(full).unwrap();
    // Model only the independent issuer slot payload, not any child authority.
    let issuer_charge =
        RetainedResourcesV2::<RetainedDependencyV2<DropWitness>>::storage_for(full).unwrap();
    b.reserve_storage(issuer_charge - full).unwrap();
    let (issuer, issuer_payload) = RetainedResourcesV2::pair(dependency, full).unwrap();
    assert!(issuer_payload.storage() >= full);
    let source = handle.retained_storage();
    drop((handle, compiler_payload));
    b.release_storage(source).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(issuer);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    std::thread::spawn(move || drop(issuer_payload))
        .join()
        .unwrap();
    b.release_storage(issuer_charge).unwrap();
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(b.storage(), 0);
}

#[test]
fn dependency_does_not_access_or_lock_poisoned_backing() {
    let (handle, payload, drops) = tracked_pair();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = handle.owner.lock().unwrap();
            panic!("poison original backing");
        }))
        .is_err()
    );
    let mut work = Work::new(RetainedResourcesV2::<DropWitness>::DEPENDENCY_WORK);
    let mut b = Budget::new(&mut work, 1_000_000);
    b.reserve_storage(handle.retained_storage()).unwrap();
    let dependency = handle.retain_dependency(&mut b).unwrap();
    assert!(Arc::ptr_eq(&handle.owner, &dependency.owner));
    drop((handle, payload));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(dependency);
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
    assert!(observe(&handle, |value| Arc::ptr_eq(&value.0, &drops)));
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
    assert_eq!(
        quoted,
        size_of::<SharedAllocation<Vec<u8>>>() + value.capacity()
    );
    let (handle, payload) = RetainedResourcesV2::pair(value, declared).unwrap();
    assert!(observe(&handle, |value| value.as_slice() == b"retained"));
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
    assert_eq!(handle.retained_storage(), usize::MAX);
    assert_eq!(
        payload.storage(),
        RetainedResourcesV2::<u8>::payload_storage(declared).unwrap()
    );
    assert!(matches!(
        RetainedResourcesV2::pair(7_u8, declared + 1),
        Err(Resource::Arithmetic)
    ));
    // This boundary leaves no representable room for the access frame.
    let mut work = Work::new(RetainedResourcesV2::<u8>::ACCESS_WORK);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(usize::MAX).unwrap();
    assert!(matches!(
        handle.with::<(), Error>(&mut budget, |_, _| panic!("unfunded access")),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert_eq!(budget.storage(), usize::MAX);
    assert!(matches!(
        handle.retain_dependency(&mut budget),
        Err(Resource::Arithmetic)
    ));
    assert_eq!(Arc::strong_count(&handle.owner), 2);
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
    assert_eq!(quote, size_of::<SharedAllocation<Aligned>>() + 23);
    let (handle, payload) = RetainedResourcesV2::pair(Aligned([1, 2, 3]), declared).unwrap();
    assert_eq!(observe(&handle, |value| value.0), [1, 2, 3]);
    assert_eq!(payload.storage(), quote);

    assert_eq!(
        RetainedResourcesV2::<u8>::payload_storage(1).unwrap(),
        size_of::<SharedAllocation<u8>>()
    );
    assert_eq!(
        RetainedResourcesV2::<()>::payload_storage(0).unwrap(),
        size_of::<SharedAllocation<()>>()
    );
    assert_eq!(size_of::<AlignedZst>(), 0);
    let (handle, payload) = RetainedResourcesV2::pair(AlignedZst, 0).unwrap();
    assert!(payload.storage() >= align_of::<AlignedZst>());
    assert_eq!(payload.storage(), size_of::<SharedAllocation<AlignedZst>>());
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

#[test]
fn nonsync_cell_access_uses_exact_original_budget_and_preserves_history() {
    type Handle = RetainedResourcesV2<Cell<usize>>;
    fn require_send_sync<T: Send + Sync>() {}
    require_send_sync::<Handle>();
    require_send_sync::<RetainedPayload>();
    let (handle, payload) = Handle::pair(Cell::new(11), size_of::<Cell<usize>>()).unwrap();
    let floor = handle.retained_storage() + 29;
    let work_limit = 13 + Handle::ACCESS_WORK + 17;
    let storage_limit = floor + Handle::ACCESS_SCRATCH + 37;
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(13).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(budget.charge_work(work_limit + 1).is_err());
    assert!(budget.reserve_storage(storage_limit + 1).is_err());
    let failed_work = budget.failed_work();
    let failed_storage = budget.failed_storage();
    let ledger = budget.work_ledger_identity_v1();
    assert_eq!(
        handle.with::<_, Error>(&mut budget, |value, b| {
            assert!(ledger == b.work_ledger_identity_v1());
            assert_eq!(b.work(), 13 + Handle::ACCESS_WORK);
            assert_eq!(b.storage(), floor + Handle::ACCESS_SCRATCH);
            assert!(matches!(
                handle.owner.try_lock(),
                Err(TryLockError::WouldBlock)
            ));
            b.charge_work(17)?;
            b.reserve_storage(37)?;
            value.set(value.get() + 1);
            Ok(value.get())
        }),
        Ok(12)
    );
    assert!(ledger == budget.work_ledger_identity_v1());
    assert_eq!(budget.work(), work_limit);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), storage_limit);
    assert_eq!(budget.failed_work(), failed_work);
    assert_eq!(budget.failed_storage(), failed_storage);
    assert!(handle.owner.try_lock().is_ok());
    drop(payload);
    assert_eq!(observe(&handle, Cell::get), 12);
}

#[test]
fn short_floor_work_or_scratch_refuses_before_callback() {
    #[derive(Clone, Copy)]
    enum Short {
        Floor,
        EntryWork,
        Work,
        Scratch,
    }
    type Handle = RetainedResourcesV2<u8>;
    let (handle, _payload) = Handle::pair(7, 1).unwrap();
    let full = handle.retained_storage();
    for short in [Short::Floor, Short::EntryWork, Short::Work, Short::Scratch] {
        let (floor, work_limit, storage_limit, charged) = match short {
            Short::Floor => (
                full - 1,
                Handle::ACCESS_WORK,
                full + Handle::ACCESS_SCRATCH,
                8,
            ),
            Short::EntryWork => (full, 7, full + Handle::ACCESS_SCRATCH, 0),
            Short::Work => (
                full,
                Handle::ACCESS_WORK - 1,
                full + Handle::ACCESS_SCRATCH,
                8,
            ),
            Short::Scratch => (
                full,
                Handle::ACCESS_WORK,
                full + Handle::ACCESS_SCRATCH - 1,
                Handle::ACCESS_WORK,
            ),
        };
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = handle.with::<(), Error>(&mut budget, |_, _| panic!("unfunded callback"));
        match short {
            Short::Floor => assert_eq!(result, Err(Error::Resource(Resource::Accounting))),
            Short::EntryWork | Short::Work => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            }
            Short::Scratch => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            }
        }
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(budget.work(), charged);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert!(!handle.owner.is_poisoned());
        assert!(handle.owner.try_lock().is_ok());
    }
}

#[test]
fn callback_error_restores_storage_and_unlocks_without_poisoning() {
    type Handle = RetainedResourcesV2<u8>;
    let (handle, _payload) = Handle::pair(7, 1).unwrap();
    let floor = handle.retained_storage();
    let mut work = Work::new(2 * Handle::ACCESS_WORK + 17);
    let mut budget = Budget::new(&mut work, floor + Handle::ACCESS_SCRATCH + 37);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        handle.with::<(), Error>(&mut budget, |_, b| {
            b.charge_work(17)?;
            b.reserve_storage(37)?;
            Err(Error::Callback)
        }),
        Err(Error::Callback)
    );
    assert_eq!(budget.storage(), floor);
    assert!(!handle.owner.is_poisoned());
    assert_eq!(
        handle.with::<_, Error>(&mut budget, |value, _| Ok(*value)),
        Ok(7)
    );
    assert_eq!(budget.work(), 2 * Handle::ACCESS_WORK + 17);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + Handle::ACCESS_SCRATCH + 37);
}

#[test]
fn callback_panic_restores_budget_and_poison_refuses_without_dropping_value() {
    type Handle = RetainedResourcesV2<DropWitness>;
    let (handle, payload, drops) = tracked_pair();
    let floor = handle.retained_storage();
    let mut work = Work::new(2 * Handle::ACCESS_WORK + 17);
    let mut budget = Budget::new(&mut work, floor + Handle::ACCESS_SCRATCH + 37);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        handle.with::<(), Error>(&mut budget, |_, b| {
            b.charge_work(17)?;
            b.reserve_storage(37)?;
            panic!("retained callback unwind");
        })
    }));
    assert!(result.is_err());
    assert!(ledger == budget.work_ledger_identity_v1());
    assert_eq!(budget.work(), Handle::ACCESS_WORK + 17);
    assert_eq!(budget.storage(), floor);
    assert!(handle.owner.is_poisoned());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(
        handle.with::<(), Error>(&mut budget, |_, _| panic!("poisoned callback")),
        Err(Error::Access(Access::Poisoned))
    );
    assert!(ledger == budget.work_ledger_identity_v1());
    assert_eq!(budget.work(), 2 * Handle::ACCESS_WORK + 17);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor + Handle::ACCESS_SCRATCH + 37);
    assert!(handle.owner.is_poisoned());
    drop(handle);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(payload);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
