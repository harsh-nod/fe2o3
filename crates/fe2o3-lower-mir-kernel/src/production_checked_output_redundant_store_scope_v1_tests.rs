use super::*;
use std::{cell::Cell, rc::Rc};
struct Dropped(Rc<Cell<bool>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn redundant_store_panicking_payload_destructor_runs_only_after_floor_cleanup() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("test hostile panic payload destructor");
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 53);
    budget.reserve_storage(40).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: StoreResult<()> = store_scope(40, &mut budget, |budget| {
            budget.reserve_storage(13)?;
            budget.charge_work(7)?;
            std::panic::panic_any(Payload(dropped.clone()));
        });
    }));
    assert!(result.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.work(), 7);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn redundant_store_scope_checks_prepaid_floor_before_work_and_preserves_unrelated_storage() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(7);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 53);
    budget.reserve_storage(40).unwrap();
    let entered = Cell::new(false);
    let result: StoreResult<()> = store_scope(41, &mut budget, |_| {
        entered.set(true);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(StoreError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert!(!entered.get());
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 40);
    let result = store_scope(39, &mut budget, |budget| {
        budget.charge_work(7)?;
        budget.reserve_storage(13)?;
        Ok(17)
    });
    assert_eq!(result.unwrap(), 17);
    assert_eq!(budget.work(), 7);
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.peak_storage(), 53);
}

#[test]
fn redundant_store_scope_drops_failed_owned_scratch_and_does_not_touch_replacement_ledger() {
    for panic in [false, true] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(9);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 70);
        budget.reserve_storage(40).unwrap();
        let dropped = Rc::new(Cell::new(false));
        let marker = Dropped(dropped.clone());
        let result: StoreResult<()> = store_scope(40, &mut budget, |budget| {
            let _owned = marker;
            budget.reserve_storage(30)?;
            budget.charge_work(9)?;
            if panic {
                std::panic::panic_any(447_u32);
            }
            Err(refused("J", "test late census refusal").into())
        });
        assert!(dropped.get());
        assert_eq!(budget.storage(), 40);
        assert_eq!(budget.work(), 9);
        if panic {
            assert!(matches!(result, Err(StoreError::Panicked)));
        } else {
            assert!(matches!(
                result,
                Err(StoreError::Admission(E::Unsupported {
                    phase: "J",
                    detail: "test late census refusal"
                }))
            ));
        }
    }
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(9);
    let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(9);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 100);
    let mut other = AssertOriginBudgetV1::new(&mut other_work, 100);
    budget.reserve_storage(40).unwrap();
    other.reserve_storage(19).unwrap();
    let ledger = other.work_ledger_identity_v1();
    let result: StoreResult<()> = store_scope(40, &mut budget, |budget| {
        std::mem::swap(budget, &mut other);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(StoreError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.storage(), 19);
    assert_eq!(other.storage(), 40);
    assert_eq!(budget.work(), 0);
}
