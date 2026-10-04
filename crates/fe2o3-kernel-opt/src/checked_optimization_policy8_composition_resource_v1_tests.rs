use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    panic::panic_any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[test]
fn wrapper_excludes_both_nested_headers_exactly() {
    assert_eq!(
        wrapper().unwrap()
            + size_of::<ReplayedPolicy7SemanticRelationV1<'_>>()
            + size_of::<CheckedCanonicalPolicy8ContinuationRelationV1<'_, '_, '_>>(),
        size_of::<ReplayedPolicy8SemanticRelationV1<'_>>()
    );
    assert!(wrapper().unwrap() >= size_of::<CanonicalPolicy8CompositionStorageV1>());
}
struct Dropped(Arc<AtomicBool>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
#[test]
fn owned_partial_receipts_drop_on_error_and_unwind_before_floor_cleanup() {
    for panic in [false, true] {
        let mut work = Work::new(9);
        let mut budget = Budget::new(&mut work, 47);
        budget.reserve_storage(17).unwrap();
        let dropped = Arc::new(AtomicBool::new(false));
        let result: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.reserve_storage(30)?;
            budget.charge_work(9)?;
            let _partial = Dropped(dropped.clone());
            if panic {
                panic!("test-only partial receipt unwind");
            }
            Err(Resource::Allocation.into())
        });
        if panic {
            assert!(matches!(result, Err(Error::Panicked)));
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Allocation))));
        }
        assert!(dropped.load(Ordering::SeqCst));
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.peak_storage(), 47);
        assert_eq!(budget.work(), 9);
    }
}
#[test]
fn foreign_or_undercut_storage_is_not_refunded_as_owned() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 50);
    budget.reserve_storage(17).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 16);
    let foreign = Box::leak(Box::new(Work::new(10)));
    let result = scoped(&mut budget, |budget| {
        *budget = Budget::new(foreign, 50);
        budget.reserve_storage(37)?;
        budget.charge_work(7)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.work(), 7);
}
struct Hostile;
impl Drop for Hostile {
    fn drop(&mut self) {
        panic!("test-only panic payload destructor");
    }
}
#[test]
fn deferred_panic_payload_destructor_observes_completed_cleanup() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 50);
    budget.reserve_storage(17).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), Error> = scoped(&mut budget, |budget| {
            budget.reserve_storage(30)?;
            budget.charge_work(7)?;
            panic_any(Hostile)
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.peak_storage(), 47);
    assert_eq!(budget.work(), 7);
}
