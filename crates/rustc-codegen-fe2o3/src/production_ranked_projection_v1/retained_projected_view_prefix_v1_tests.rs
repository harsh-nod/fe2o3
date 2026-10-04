//! Inert view-backing/lexical-loan controls. No admitted source/SSA owner is forged.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
#[path = "whole_root_inert_fixtures_v1_tests.rs"]
mod fixture;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 19;
struct Meter<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for Meter<'_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0.charge_work(amount).map_err(resource)
    }
    fn scalar_private_storage_v1(&self) -> Result<usize> {
        Ok(self.0.storage())
    }
    fn helper_value_ledger_v1(
        &self,
    ) -> Result<(
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    )> {
        Ok((
            self.0 as *const _ as usize,
            self.0.work_ledger_identity_v1(),
        ))
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        panic!("inert loan is not a proof")
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        panic!("inert loan is not a proof")
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<canonical_assertion_facts_v1::ProjectedAssertionConditionV1> {
        panic!("inert loan is not a proof")
    }
}
fn trial(work_limit: usize, extra: usize) -> (bool, usize, usize, usize) {
    let function = fixture::positive();
    let types = fixture::projection_types();
    let target = fixture::target();
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR + extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut value = RetainedProjectedViewPrefixV1::new();
    let result = value.prepare_into(
        &function,
        &types,
        target,
        &mut Prep::new(&mut budget, &mut owned),
    );
    assert_eq!(budget.storage(), FLOOR + owned);
    assert!(value.source.is_some());
    assert!(value.locals.iter().all(Option::is_none));
    assert_eq!(value.queried.capacity(), 0);
    assert!(!value.loan_started && !value.loan_returned);
    assert_eq!(
        value.phase,
        if result.is_ok() {
            Phase::Prepared
        } else {
            Phase::Terminal
        }
    );
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        value.locals.len(),
        value.locals.capacity(),
    );
    assert!(
        value
            .prepare_into(
                &function,
                &types,
                target,
                &mut Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            value.locals.len(),
            value.locals.capacity()
        )
    );
    let outcome = (
        result.is_ok(),
        budget.work(),
        owned,
        value.locals.capacity(),
    );
    drop(value);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    outcome
}
#[test]
fn retained_pre_writer_views_complete_initial_backing_and_one_shot() {
    let value = trial(LIMIT, LIMIT);
    assert!(value.0);
    assert_eq!(value.1, 36);
    assert!(value.3 >= 4);
}
#[test]
fn retained_pre_writer_views_every_work_cut_keeps_partial_destination() {
    let good = trial(LIMIT, LIMIT);
    for cut in 0..good.1 {
        assert!(!trial(cut, LIMIT).0);
    }
}
#[test]
fn retained_pre_writer_views_every_storage_cut_keeps_partial_destination() {
    let good = trial(LIMIT, LIMIT);
    for cut in 0..good.2 {
        assert!(!trial(LIMIT, cut).0);
    }
}
#[test]
fn retained_pre_writer_views_foreign_source_counter_and_held_floor_refuse() {
    let function = fixture::positive();
    let other = function.clone();
    let types = fixture::projection_types();
    let copied = types.clone();
    let target = fixture::target();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut value = RetainedProjectedViewPrefixV1::new();
    value
        .prepare_into(
            &function,
            &types,
            target,
            &mut Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    assert!(
        value
            .check(&other, &types, target, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    assert!(
        value
            .check(
                &function,
                &copied,
                target,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    let mut foreign = owned;
    assert!(
        value
            .check(
                &function,
                &types,
                target,
                &Prep::new(&mut budget, &mut foreign)
            )
            .is_err()
    );
    budget.release_storage(1).unwrap();
    assert!(
        value
            .check(
                &function,
                &types,
                target,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    budget.reserve_storage(1).unwrap();
    value
        .check(
            &function,
            &types,
            target,
            &Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    drop(value);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_pre_writer_views_lexical_error_and_unwind_keep_backing_terminal() {
    // These directly constructed, test-private loans intentionally have no Shared
    // view. They test only HRTB/error/drop custody; genuine Some(view) enrollment
    // is tested separately by the real checked-source observation.
    for mode in 0..3 {
        let function = fixture::positive();
        let types = fixture::projection_types();
        let target = fixture::target();
        let mut work = Work::new(if mode == 1 { 37 } else { LIMIT });
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut value = RetainedProjectedViewPrefixV1::new();
        value
            .prepare_into(
                &function,
                &types,
                target,
                &mut Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        let before = (
            budget.work(),
            budget.storage(),
            owned,
            value.locals.capacity(),
        );
        value.loan_started = true;
        {
            let mut meter = Meter(&mut budget);
            let mut loan = ProjectedViewPrefixLoanV1 {
                backing: &mut value,
                scalar_private_singletons: &[],
                shared_value_reads: None,
                scalar_private_borrows: None,
                facts: Some(&mut meter),
            };
            let caught =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
                    loan.with_assertion_facts_v1(|facts| match mode {
                        0 => Err(Error::Incomplete("lexical error sentinel")),
                        1 => facts.charge_private_array_work(2),
                        _ => std::panic::panic_any(()),
                    })
                }));
            match mode {
                0 => assert!(matches!(
                    caught,
                    Ok(Err(Error::Incomplete("lexical error sentinel")))
                )),
                1 => assert!(matches!(
                    caught,
                    Ok(Err(Error::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Resource(Resource::Work(_))
                    )))
                )),
                _ => assert!(caught.is_err()),
            }
            assert_eq!(loan.backing.phase, Phase::Terminal);
            assert!(loan.charge_private_array_work(1).is_err());
            assert!(loan.with_assertion_facts_v1(|_| Ok(())).is_err());
            assert_eq!(loan.backing.locals.capacity(), before.3);
        }
        assert!(value.loan_returned);
        assert_eq!(value.phase, Phase::Terminal);
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                owned,
                value.locals.capacity()
            ),
            before
        );
        drop(value);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
