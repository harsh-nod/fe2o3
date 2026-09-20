use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, rc::Rc};

#[test]
fn promotion_wrapper_counts_the_actual_prefix_and_tail_once() {
    let direct = promotion_header::<
        ProductionOwnedCommutativeContinuationV1,
        ProductionOwnedPrivateCellPromotionContinuationV1,
    >()
    .unwrap();
    let erased = promotion_header::<
        ProductionOwnedUnitLocalCommutativeContinuationV1,
        ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    >()
    .unwrap();
    let fields = size_of::<Box<[FormalMemoryObligations]>>() + size_of::<usize>();
    assert!(direct >= fields && erased >= fields);
    assert_eq!(
        direct + size_of::<ProductionOwnedCommutativeContinuationV1>() + size_of::<PromotionTail>(),
        size_of::<ProductionOwnedPrivateCellPromotionContinuationV1>()
    );
    assert_eq!(
        erased
            + size_of::<ProductionOwnedUnitLocalCommutativeContinuationV1>()
            + size_of::<PromotionTail>(),
        size_of::<ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1>()
    );
}

struct Dropped(Rc<Cell<bool>>);
impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

#[test]
fn promotion_scope_drops_failed_candidates_and_preserves_live_accounting() {
    for panics in [false, true] {
        let mut work = Work::new(13);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 83);
        budget.reserve_storage(47).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let dropped = Rc::new(Cell::new(false));
        let result: PResult<()> = promotion_scoped(40, &mut budget, |budget, binding| {
            let _owner = Dropped(dropped.clone());
            budget.reserve_storage(36)?;
            budget.charge_work(13)?;
            binding.check(budget)?;
            if panics {
                std::panic::panic_any(689_u32);
            }
            Err(refused("private-cell promotion", "test late source denial").into())
        });
        assert!(dropped.get() && result.is_err());
        assert_eq!(budget.storage(), 47);
        assert_eq!(budget.peak_storage(), 83);
        assert_eq!(budget.work(), 13);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn promotion_scope_refuses_underpaid_and_undercut_floors_without_invented_credit() {
    let mut work = Work::new(7);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 83);
    budget.reserve_storage(47).unwrap();
    let entered = Cell::new(false);
    let denied: PResult<()> = promotion_scoped(48, &mut budget, |_, _| {
        entered.set(true);
        Ok(())
    });
    assert!(matches!(
        denied,
        Err(PError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert!(!entered.get());
    assert_eq!(budget.work(), 0);
    let result: PResult<()> = promotion_scoped(47, &mut budget, |budget, _| {
        budget.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(PError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(budget.storage(), 46);
}

#[test]
fn promotion_scope_refuses_a_foreign_ledger_on_success_error_and_panic() {
    for mode in 0..3 {
        let mut work = Work::new(11);
        let mut other_work = Work::new(17);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 89);
        let mut foreign = AssertOriginBudgetV1::new(&mut other_work, 89);
        budget.reserve_storage(47).unwrap();
        foreign.reserve_storage(47).unwrap();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let dropped = Rc::new(Cell::new(false));
        let result: PResult<Dropped> = promotion_scoped(47, &mut budget, |budget, _| {
            budget.charge_work(11)?;
            budget.reserve_storage(31)?;
            std::mem::swap(budget, &mut foreign);
            if mode == 1 {
                return Err(PError::Panicked);
            }
            if mode == 2 {
                std::panic::panic_any(689_u32);
            }
            Ok(Dropped(dropped.clone()))
        });
        assert!(matches!(
            result,
            Err(PError::Resource(AssertOriginResourceV1::Accounting))
        ));
        if mode == 0 {
            assert!(dropped.get());
        }
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert_eq!((budget.storage(), budget.work()), (47, 0));
        assert_eq!((foreign.storage(), foreign.work()), (78, 11));
    }
}

impl ProductionOwnedUnitLocalCommutativeContinuationV1 {
    pub(crate) fn exercise_private_cell_prefix_refusal_v1(
        mut self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        assert_eq!(self.data.kernels.len(), 2);
        assert_ne!(self.data.kernels[0], self.data.kernels[1]);
        self.data.kernels.swap(0, 1);
        let floor = budget.storage();
        let before = budget.work();
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(CError::Admission(E::Formal(
                crate::ProductionFormalMemoryErrorV1::ObligationMismatch
            )))
        ));
        let replay_work = budget.work() - before;
        let before = budget.work();
        let result = self.continue_private_cell_promotion_v1(budget);
        assert!(matches!(
            result,
            Err(PError::Prefix(ref error)) if matches!(error.as_ref(),
                CError::Admission(E::Formal(crate::ProductionFormalMemoryErrorV1::ObligationMismatch)))
        ));
        assert_eq!(
            budget.work() - before,
            replay_work,
            "failure precedes the promotion service"
        );
        assert_eq!(budget.storage(), floor);
    }
}

impl ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1 {
    pub(crate) fn exercise_private_cell_report_order_v1(
        &mut self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        assert_eq!(self.data.kernels.len(), 2);
        assert_ne!(self.data.kernels[0], self.data.kernels[1]);
        let floor = budget.storage();
        self.data.kernels.swap(0, 1);
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(PError::Admission(E::Formal(
                crate::ProductionFormalMemoryErrorV1::ObligationMismatch
            )))
        ));
        assert_eq!(budget.storage(), floor);
        self.data.kernels.swap(0, 1);
        self.verify_equivalence(budget).unwrap();
    }
}

impl ProductionOwnedPrivateCellPromotionContinuationV1 {
    pub(crate) fn exercise_private_cell_foreign_tail_refusal_v1(
        &mut self,
        foreign: &ProductionOwnedCommutativeContinuationV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) {
        assert_ne!(
            foreign.output().canonical().identity(),
            self.prefix.output().canonical().identity()
        );
        let tail = prepare_promotion(foreign.output(), budget).unwrap();
        let added = tail.retained_storage();
        budget.reserve_storage(added).unwrap();
        let original = std::mem::replace(&mut self.data.tail, tail);
        let previous_added = self.data.added;
        self.data.added = promotion_added(
            &self.data,
            promotion_header::<ProductionOwnedCommutativeContinuationV1, Self>().unwrap(),
        )
        .unwrap();
        let floor = budget.storage();
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(PError::Continuation(
                fe2o3_kernel_opt::OwnedPrivateCellPromotionErrorV1::ForeignInput
            ))
        ));
        assert_eq!(budget.storage(), floor);
        drop(std::mem::replace(&mut self.data.tail, original));
        self.data.added = previous_added;
        budget.release_storage(added).unwrap();
        self.verify_equivalence(budget).unwrap();
    }
}
