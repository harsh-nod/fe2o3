use super::*;
use crate::production_pipeline::with_projected_execution_source_v29;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    ProductionContextRootErrorV29 as HandoffError, with_checked_execution_source_v29,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

impl RetainedContextEntriesV29 {
    pub(crate) fn projection_test_fixture_v29() -> (AdmittedInertSemanticMirV1, Self) {
        let semantic = fixture_roots(Mutation::None, 4);
        let receipt = scope_tests::complete(&semantic);
        (semantic, receipt)
    }
}

#[test]
fn source_projection_restores_only_its_backing_on_success_failure_and_unwind() {
    let semantic = fixture_roots(Mutation::None, 4);
    let receipt = scope_tests::complete(&semantic);
    let storage = projection_storage(&receipt);
    for mode in 0..3 {
        let mut work = Work::new(10_000);
        let mut budget = VisitBudget::new(&mut work, storage + 18);
        budget.reserve_storage(7).unwrap();
        let source = receipt
            .materialization_source_v29(&semantic, &mut budget)
            .unwrap()
            .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_projected_execution_source_v29(&source, &mut budget, |input, budget| {
                assert_eq!(budget.storage(), storage + 7);
                assert_eq!(input.roots.len(), 4);
                assert_eq!(input.classes.len(), source.classes().len());
                assert_eq!(input.events.len(), source.events().len());
                for (projected, retained) in input.roots.iter().zip(source.roots()) {
                    assert!(std::ptr::eq(
                        projected.helper_arguments,
                        retained.helper_operands()
                    ));
                }
                budget.reserve_storage(11)?;
                match mode {
                    0 => Ok(42),
                    1 => Err(HandoffError::Arguments),
                    _ => panic!("consumer panic after retaining output"),
                }
            })
        }));
        match mode {
            0 => assert_eq!(result.unwrap(), Ok(42)),
            1 => assert_eq!(result.unwrap(), Err(HandoffError::Arguments)),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 18);
        assert_eq!(budget.peak_storage(), storage + 18);
        assert!(budget.failed_storage().is_none());
    }
}

#[test]
fn source_projection_obeys_exact_storage_and_work_limits() {
    let semantic = fixture_roots(Mutation::None, 4);
    let receipt = scope_tests::complete(&semantic);
    let storage = projection_storage(&receipt);
    let mut work = Work::new(10_000);
    let mut budget = VisitBudget::new(&mut work, storage + 7);
    let source = receipt
        .materialization_source_v29(&semantic, &mut budget)
        .unwrap()
        .unwrap();
    let before = budget.work();
    with_projected_execution_source_v29(&source, &mut budget, |_, _| Ok(())).unwrap();
    let exact = budget.work() - before;
    for capacity in [0, storage - 1, storage] {
        let mut work = Work::new(10_000);
        let mut budget = VisitBudget::new(&mut work, capacity + 7);
        budget.reserve_storage(7).unwrap();
        let mut visits = 0;
        let result = with_projected_execution_source_v29(&source, &mut budget, |_, _| {
            visits += 1;
            Ok(())
        });
        if capacity == storage {
            result.unwrap();
            assert_eq!(visits, 1);
        } else {
            assert!(matches!(
                result,
                Err(HandoffError::Resource(Resource::Storage { .. }))
            ));
            assert_eq!(visits, 0);
        }
        assert_eq!(budget.storage(), 7);
    }
    for limit in [0, exact - 1, exact] {
        let mut work = Work::new(limit);
        let mut budget = VisitBudget::new(&mut work, storage + 7);
        budget.reserve_storage(7).unwrap();
        let result = with_projected_execution_source_v29(&source, &mut budget, |_, _| Ok(()));
        if limit == exact {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(HandoffError::Resource(Resource::Work { .. }))
            ));
        }
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn complete_census_preflights_late_root_arguments_before_any_consumer() {
    let semantic = fixture_roots(Mutation::None, 4);
    let receipt = scope_tests::complete(&semantic);
    let launch = launch_roster(&semantic);
    let owner = ssa_owner(semantic);
    let mut work = Work::new(10_000);
    let mut budget = VisitBudget::new(&mut work, projection_storage(&receipt) + 7);
    budget.reserve_storage(7).unwrap();
    let source = receipt
        .materialization_source_v29(owner.source_semantic(), &mut budget)
        .unwrap()
        .unwrap();
    let error = with_projected_execution_source_v29(&source, &mut budget, |input, budget| {
        let mut changed = input.roots.to_vec();
        changed.last_mut().unwrap().helper_arguments = &[];
        with_checked_execution_source_v29(
            &owner,
            &launch,
            fe2o3_lower_mir_kernel::ProductionExecutionSourceInputV29 {
                roots: &changed,
                ..input
            },
            budget,
            |_, _| panic!("late malformed root followed an already consumed root"),
        )
    })
    .unwrap_err();
    assert_eq!(error, HandoffError::Arguments);
    assert_eq!(budget.storage(), 7);
}

#[test]
fn source_projection_and_root_visits_never_charge_or_refund_a_replaced_ledger() {
    let semantic = fixture_roots(Mutation::None, 4);
    let receipt = scope_tests::complete(&semantic);
    let launch = launch_roster(&semantic);
    let owner = ssa_owner(semantic);
    for nested in [false, true] {
        let mut work = Work::new(10_000);
        let mut budget = VisitBudget::new(&mut work, projection_storage(&receipt) + 7);
        budget.reserve_storage(7).unwrap();
        let source = receipt
            .materialization_source_v29(owner.source_semantic(), &mut budget)
            .unwrap()
            .unwrap();
        let mut foreign = Some(VisitBudget::new(
            Box::leak(Box::new(Work::new(10_000))),
            101,
        ));
        foreign.as_mut().unwrap().charge_work(5).unwrap();
        foreign.as_mut().unwrap().reserve_storage(101).unwrap();
        let mut visits = 0;
        let error = with_projected_execution_source_v29(&source, &mut budget, |input, budget| {
            if nested {
                with_checked_execution_source_v29(&owner, &launch, input, budget, |_, budget| {
                    visits += 1;
                    let replacement = foreign.take().expect("foreign ledger reached another root");
                    let _ = std::mem::replace(budget, replacement);
                    Ok(())
                })
            } else {
                visits += 1;
                let _ = std::mem::replace(budget, foreign.take().unwrap());
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error, HandoffError::Resource(Resource::Accounting));
        assert_eq!(visits, 1);
        assert_eq!(budget.work(), 5);
        assert_eq!(budget.storage(), 101);
        assert_eq!(budget.peak_storage(), 101);
        assert!(budget.failed_storage().is_none());
    }
}
