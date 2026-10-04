use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Counted(Arc<AtomicUsize>);
impl Drop for Counted {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn original_native_formal_scope_restores_floor_on_success_error_and_panic() {
    for mode in 0..3 {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut work = Work::new(7);
        let mut budget = Budget::new(&mut work, 53);
        budget.reserve_storage(40).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(7)?;
            let value = Counted(drops.clone());
            match mode {
                0 => Ok(value),
                1 => Err(E::KernelRoster),
                _ => panic!("original N analysis test panic"),
            }
        });
        assert_eq!(budget.storage(), 40);
        assert_eq!(budget.work(), 7);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(result.is_ok(), mode == 0);
        if mode == 2 {
            assert!(matches!(result, Err(E::Panicked)));
        }
        drop(result);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn original_native_formal_scope_refuses_replaced_ledger_without_touching_it() {
    for mode in 0..3 {
        let foreign = Box::leak(Box::new(Work::new(100)));
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(40).unwrap();
        let result: Result<(), E> = scoped(&mut budget, |budget| {
            *budget = Budget::new(foreign, 100);
            budget.reserve_storage(53)?;
            budget.charge_work(7)?;
            match mode {
                0 => Ok(()),
                1 => Err(E::KernelRoster),
                _ => panic!("replaced original N ledger"),
            }
        });
        assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
        assert_eq!(budget.storage(), 53);
        assert_eq!(budget.work(), 7);
    }
}

#[test]
fn original_native_formal_scope_drops_panic_payload_after_cleanup() {
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("hostile original N panic payload destructor");
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 53);
    budget.reserve_storage(40).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), E> = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(7)?;
            std::panic::panic_any(Payload(drops.clone()));
        });
    }));
    assert!(result.is_err());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.work(), 7);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn original_native_formal_scope_exact_short_and_underfloor_are_not_reset() {
    for (work_limit, storage_limit, success) in [(7, 53, true), (6, 53, false), (7, 52, false)] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(40).unwrap();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(7)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), success);
        assert_eq!(budget.storage(), 40);
    }
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 53);
    budget.reserve_storage(40).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 39);
}
