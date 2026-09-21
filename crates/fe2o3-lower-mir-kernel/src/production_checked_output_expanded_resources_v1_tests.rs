use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, mem::size_of_val};

#[test]
fn expanded_scope_pays_its_guard_before_running_and_preserves_prior_work() {
    let floor = 59;
    let paid = size_of::<Binding>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
        + size_of::<XResult<()>>();
    for cap in [floor + paid - 1, floor + paid] {
        let mut work = Work::new(100);
        let mut budget = AssertOriginBudgetV1::new(&mut work, cap);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(7).unwrap();
        let called = Cell::new(false);
        let result = scoped(floor, &mut budget, |budget, binding| {
            called.set(true);
            binding.check(budget)?;
            budget.charge_work(3)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), cap == floor + paid);
        assert_eq!(called.get(), cap == floor + paid);
        assert_eq!(budget.work(), if called.get() { 10 } else { 7 });
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn expanded_scope_failure_panic_and_rejected_result_drop_restore_floor() {
    struct Bomb<'a>(&'a Cell<usize>);
    impl Drop for Bomb<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected expanded result");
        }
    }
    let mut work = Work::new(100);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 4096);
    budget.reserve_storage(71).unwrap();
    let error: XResult<()> = scoped(71, &mut budget, |budget, _| {
        budget.reserve_storage(101)?;
        Err(AssertOriginResourceV1::Allocation.into())
    });
    assert!(matches!(
        error,
        Err(XError::Resource(AssertOriginResourceV1::Allocation))
    ));
    assert_eq!(budget.storage(), 71);
    let panic: XResult<()> = scoped(71, &mut budget, |budget, _| {
        budget.reserve_storage(101)?;
        panic!("expanded private phase");
    });
    assert!(matches!(panic, Err(XError::Panicked)));
    assert_eq!(budget.storage(), 71);
    let drops = Cell::new(0);
    let result = scoped(71, &mut budget, |budget, _| {
        budget.release_storage(budget.storage() - 71)?;
        Ok(Bomb(&drops))
    });
    assert!(matches!(
        result,
        Err(XError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(drops.get(), 1);
    assert_eq!(budget.storage(), 71);
}

#[test]
fn expanded_scope_rejects_foreign_ledger_without_refunding_it() {
    let mut first = Work::new(100);
    let mut second = Work::new(100);
    let mut budget = AssertOriginBudgetV1::new(&mut first, 4096);
    let mut foreign = AssertOriginBudgetV1::new(&mut second, 4096);
    budget.reserve_storage(73).unwrap();
    foreign.reserve_storage(73).unwrap();
    let foreign_id = foreign.work_ledger_identity_v1();
    let result = scoped(73, &mut budget, |budget, _| {
        Ok(std::mem::replace(budget, foreign))
    });
    assert!(matches!(
        result,
        Err(XError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(budget.storage(), 73);
    assert!(budget.work_ledger_identity_v1() == foreign_id);
    assert_eq!(budget.work(), 0);
}

#[test]
fn expanded_row_storage_uses_actual_capacity_and_scope_cleans_up() {
    let mut work = Work::new(1000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 4096);
    budget.reserve_storage(79).unwrap();
    scoped(79, &mut budget, |budget, _| {
        let before = budget.storage();
        let rows = row_table(3, budget)?;
        assert_eq!(
            budget.storage() - before,
            size_of_val(&rows) + rows.capacity() * size_of::<ProductionExpandedSourceOriginV1>()
        );
        assert!(rows.capacity() >= 3);
        drop(rows);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), 79);
}

#[test]
fn expanded_error_keeps_typed_causes_and_no_authority_variant() {
    use std::error::Error as _;
    let error = XError::Resource(AssertOriginResourceV1::Arithmetic);
    assert!(error.source().unwrap().is::<AssertOriginResourceV1>());
    let error = XError::Scalar(fe2o3_kernel_opt::CheckedScalarFixedPointErrorV1::History);
    assert!(
        error
            .source()
            .unwrap()
            .is::<fe2o3_kernel_opt::CheckedScalarFixedPointErrorV1>()
    );
    assert!(XError::History.source().is_none());
}
