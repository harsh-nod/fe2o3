use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

#[test]
fn wrappers_credit_each_prefix_and_tail_exactly_once() {
    let direct = header::<
        ProductionOwnedRedundantStoreContinuationV1,
        ProductionOwnedCommutativeContinuationV1,
    >()
    .unwrap();
    let erased = header::<
        ProductionOwnedUnitLocalRedundantStoreContinuationV1,
        ProductionOwnedUnitLocalCommutativeContinuationV1,
    >()
    .unwrap();
    let fields = size_of::<Box<[FormalMemoryObligations]>>() + size_of::<usize>();
    assert!(direct >= fields && erased >= fields);
    assert_eq!(
        direct + size_of::<ProductionOwnedRedundantStoreContinuationV1>() + size_of::<Tail>(),
        size_of::<ProductionOwnedCommutativeContinuationV1>()
    );
    assert_eq!(
        erased
            + size_of::<ProductionOwnedUnitLocalRedundantStoreContinuationV1>()
            + size_of::<Tail>(),
        size_of::<ProductionOwnedUnitLocalCommutativeContinuationV1>()
    );
}

struct Dropped(Rc<Cell<bool>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn scope_drops_local_owners_on_error_and_panic_before_exact_cleanup() {
    for panic in [false, true] {
        let mut work = Work::new(11);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 79);
        budget.reserve_storage(47).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Rc::new(Cell::new(false));
        let result: CResult<()> = scoped(40, &mut budget, |budget, binding| {
            let _owner = Dropped(dropped.clone());
            budget.reserve_storage(32)?;
            budget.charge_work(11)?;
            binding.check(budget)?;
            if panic {
                std::panic::panic_any(574_u32);
            }
            Err(refused("commutative K", "test late source refusal").into())
        });
        assert!(dropped.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), 47);
        assert_eq!(budget.peak_storage(), 79);
        assert_eq!(budget.work(), 11);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn scope_prepaid_floor_success_extra_storage_and_underflow_contract() {
    let mut work = Work::new(13);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 91);
    budget.reserve_storage(59).unwrap();
    let entered = Cell::new(false);
    let denied: CResult<()> = scoped(60, &mut budget, |_, _| {
        entered.set(true);
        Ok(())
    });
    assert!(matches!(
        denied,
        Err(CError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert!(!entered.get());
    assert_eq!(budget.work(), 0);
    let value = scoped(50, &mut budget, |budget, binding| {
        budget.charge_work(13)?;
        budget.reserve_storage(32)?;
        binding.check(budget)?;
        Ok(29)
    })
    .unwrap();
    assert_eq!(value, 29);
    assert_eq!(budget.storage(), 59);
    assert_eq!(budget.work(), 13);
    let result: CResult<()> = scoped(59, &mut budget, |budget, _| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(CError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(
        budget.storage(),
        58,
        "an undercut floor is never repaired by guessing ownership"
    );
}

#[test]
fn foreign_ledger_equal_floor_is_neither_charged_nor_released_on_any_exit() {
    for mode in 0..3 {
        let mut work = Work::new(11);
        let mut other_work = Work::new(17);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 89);
        let mut foreign = AssertOriginBudgetV1::new(&mut other_work, 89);
        budget.reserve_storage(47).unwrap();
        foreign.reserve_storage(47).unwrap();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let dropped = Rc::new(Cell::new(false));
        let result: CResult<Dropped> = scoped(47, &mut budget, |budget, _| {
            budget.charge_work(11)?;
            budget.reserve_storage(31)?;
            std::mem::swap(budget, &mut foreign);
            if mode == 1 {
                return Err(CError::Panicked);
            }
            if mode == 2 {
                std::panic::panic_any(574_u32);
            }
            Ok(Dropped(dropped.clone()))
        });
        assert!(matches!(
            result,
            Err(CError::Resource(AssertOriginResourceV1::Accounting))
        ));
        if mode == 0 {
            assert!(dropped.get());
        }
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert_eq!(budget.storage(), 47);
        assert_eq!(budget.work(), 0);
        assert_eq!(foreign.storage(), 78);
        assert_eq!(foreign.work(), 11);
    }
}

#[test]
fn panicking_payload_is_destroyed_only_after_original_floor_restoration() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("hostile payload drop");
        }
    }
    let seen = Arc::new(AtomicBool::new(false));
    let mut work = Work::new(7);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 71);
    budget.reserve_storage(40).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: CResult<()> = scoped(40, &mut budget, |budget, _| {
            budget.reserve_storage(31)?;
            budget.charge_work(7)?;
            std::panic::panic_any(Payload(seen.clone()));
        });
    }));
    assert!(result.is_err() && seen.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.work(), 7);
}

impl ProductionOwnedUnitLocalCommutativeContinuationV1 {
    pub(crate) fn exercise_fresh_report_order_v1(&mut self, budget: &mut AssertOriginBudgetV1<'_>) {
        assert_eq!(self.data.kernels.len(), 2);
        assert_ne!(self.data.kernels[0], self.data.kernels[1]);
        let floor = budget.storage();
        self.data.kernels.swap(0, 1);
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(CError::Admission(E::Formal(
                crate::ProductionFormalMemoryErrorV1::ObligationMismatch
            )))
        ));
        assert_eq!(budget.storage(), floor);
        self.data.kernels.swap(0, 1);
        self.verify_equivalence(budget).unwrap();
    }
}
