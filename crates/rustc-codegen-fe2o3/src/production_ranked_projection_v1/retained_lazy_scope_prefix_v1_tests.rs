//! Inert lexical/deferred-error controls, never a facts/SSA authority fixture.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::cell::Cell;
struct Meter<'a, 'w> {
    budget: &'a mut Budget<'w>,
    calls: Cell<u8>,
    storage_error: bool,
    ledger_error: bool,
}
impl ProjectedAssertionFactsV1 for Meter<'_, '_> {
    fn charge_private_array_work(&mut self, _: usize) -> Result<()> {
        panic!("lazy snapshot cannot charge work")
    }
    fn scalar_private_storage_v1(&self) -> Result<usize> {
        self.calls.set(self.calls.get() * 10 + 1);
        if self.storage_error {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "storage sentinel",
            ))
        } else {
            Ok(self.budget.storage())
        }
    }
    fn helper_value_ledger_v1(&self) -> Result<(usize, Ledger)> {
        self.calls.set(self.calls.get() * 10 + 2);
        if self.ledger_error {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ledger sentinel",
            ))
        } else {
            Ok((
                self.budget as *const _ as usize,
                self.budget.work_ledger_identity_v1(),
            ))
        }
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        panic!("no proof")
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        panic!("no proof")
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<canonical_assertion_facts_v1::ProjectedAssertionConditionV1> {
        panic!("no proof")
    }
}
#[test]
fn retained_pre_writer_lazy_storage_error_is_deferred_and_short_circuits_ledger() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut meter = Meter {
        budget: &mut budget,
        calls: Cell::new(0),
        storage_error: true,
        ledger_error: false,
    };
    let mut value = RetainedLazyScopePrefixV1::new();
    value.capture_into(&mut meter).unwrap();
    assert_eq!(meter.calls.get(), 1);
    value.before_writers().unwrap();
    assert!(matches!(
        &value.scope.as_ref().unwrap().entry,
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "storage sentinel"
        ))
    ));
    assert_eq!(meter.budget.work(), 0);
    assert_eq!(meter.budget.storage(), 0);
}
#[test]
fn retained_pre_writer_lazy_ledger_error_is_deferred_after_storage() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut meter = Meter {
        budget: &mut budget,
        calls: Cell::new(0),
        storage_error: false,
        ledger_error: true,
    };
    let mut value = RetainedLazyScopePrefixV1::new();
    value.capture_into(&mut meter).unwrap();
    assert_eq!(meter.calls.get(), 12);
    value.before_writers().unwrap();
    assert!(matches!(
        &value.scope.as_ref().unwrap().entry,
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "ledger sentinel"
        ))
    ));
}
#[test]
fn retained_pre_writer_lazy_snapshot_is_distinct_unstarted_and_one_shot() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let mut meter = Meter {
        budget: &mut budget,
        calls: Cell::new(0),
        storage_error: false,
        ledger_error: false,
    };
    let mut value = RetainedLazyScopePrefixV1::new();
    value.capture_into(&mut meter).unwrap();
    assert_eq!(meter.calls.get(), 12);
    let scope = value.scope.as_ref().unwrap();
    assert_eq!(scope.entry.as_ref().unwrap().2, 17);
    assert!(!scope.started);
    assert_eq!(scope.retained, 0);
    assert!(value.capture_into(&mut meter).is_err());
    assert_eq!(meter.calls.get(), 12);
    meter.budget.reserve_storage(19).unwrap();
    value.before_writers().unwrap();
    assert_eq!(value.scope.as_ref().unwrap().entry.as_ref().unwrap().2, 17);
}
