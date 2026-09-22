use super::super::tests::{noop, owner, two_roots, with_checked, with_projection};
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Module};
use std::rc::Rc;

const STORAGE: usize = 1 << 32;
const WORK: usize = 1 << 48;

fn is_work(error: &Failure) -> bool {
    matches!(
        error,
        Failure::Resource(Resource::Work { .. })
            | Failure::Bridge(crate::KirBridgeErrorV12::Resource(Resource::Work { .. }))
    )
}

#[test]
fn empty_native_projection_has_independent_import_and_relation_failure_prefixes() {
    let (owner, owner_storage) = owner(&Module::new("empty-prefix"));
    let b = owner.canonical().canonical_bytes().len();
    // Independent empty-source census: root tree=3, slots/functions/defs=0.
    let envelope_work = 4 * 4 * 4 + b * 4 * 8 + (b + 4) * 8;
    let envelope_storage = (b + 4) * 64 + 4096;
    let digest_work = b + 12 + crate::KIR_PLIRON_BRIDGE_V12_IDENTITY_DOMAIN_V1.len();
    let header = size_of::<Projection<'_>>();
    let witness = size_of::<crate::kir_bridge_v1::NativeBridgeWitnessV1>();
    let floor = owner_storage + 31;
    let pre_schema = 2 + b + envelope_work + digest_work + 4;
    // (allowance, accepted work, peak). These constants are derived from the
    // explicit charge sequence, NOT a successful run's measured total/peak.
    let cuts = [
        (1, 0, floor),
        (2 + b - 1, 2, floor + header),
        (2 + b + envelope_work - 1, 2 + b, floor + header),
        (
            2 + b + envelope_work + digest_work - 1,
            2 + b + envelope_work,
            floor + header + envelope_storage,
        ),
        (
            pre_schema - 1,
            pre_schema - 4,
            floor + header + envelope_storage,
        ),
        (
            pre_schema + 10 - 1,
            pre_schema + 1,
            floor + header + envelope_storage + witness,
        ),
        (
            pre_schema + 10 + b + 3 - 1,
            pre_schema + 10,
            floor + header + envelope_storage + witness,
        ),
        (
            pre_schema + 10 + b + 3 + envelope_work - 1,
            pre_schema + 10 + b + 3,
            floor + header + envelope_storage + witness,
        ),
    ];
    for (allowance, accepted, peak) in cuts {
        let mut work = Work::new(allowance);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let result = protected(&mut budget, |budget| {
            let projection = Projection::import(&owner, budget)?;
            drop(projection);
            Ok(())
        });
        assert!(is_work(result.as_ref().unwrap_err()), "{result:?}");
        assert_eq!(budget.work(), accepted, "allowance={allowance}");
        assert_eq!(budget.peak_storage(), peak, "allowance={allowance}");
        assert_eq!(budget.storage(), floor);
        drop(budget);
        assert!(work.failed_work().is_some());
    }
}

#[test]
fn native_header_and_envelope_storage_boundaries_restore_unrelated_floor() {
    let (owner, owner_storage) = owner(&Module::new("empty-storage"));
    let b = owner.canonical().canonical_bytes().len();
    let h = size_of::<Projection<'_>>();
    let s = (b + 4) * 64 + 4096;
    let floor = owner_storage + 41;
    let e = 4 * 4 * 4 + b * 4 * 8 + (b + 4) * 8;
    for (limit, accepted, peak, failed) in [
        (floor + h - 1, 2, floor, floor + h),
        (floor + h + s - 1, 2 + b + e, floor + h, floor + h + s),
    ] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let result = protected(&mut budget, |budget| {
            let projection = Projection::import(&owner, budget)?;
            drop(projection);
            Ok(())
        });
        assert!(matches!(
            result,
            Err(Failure::Resource(Resource::Storage { .. })
                | Failure::Bridge(crate::KirBridgeErrorV12::Resource(Resource::Storage { .. })))
        ));
        assert_eq!(budget.work(), accepted);
        assert_eq!(budget.peak_storage(), peak);
        assert_eq!(budget.failed_storage(), Some(failed));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn ordinary_observed_zero_allowance_retains_the_real_first_denial() {
    with_checked(&noop(), |checked, budget| {
        let floor = budget.storage();
        let error = with_checks(checked, budget, Limits::new(0, 0), |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Analysis { function: 0, .. }
        ));
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert_eq!(error.observation().peak_storage_units(), 0);
        assert!(error.observation().first_denial().is_some());
        assert_eq!(
            error.last_invocation().unwrap().floor().work_upper_bound(),
            0
        );
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn real_invocation_panic_retains_zero_prefix_and_typed_refusal() {
    with_checked(&noop(), |checked, budget| {
        super::super::super::pliron_pipeline::panic_next_production_analysis_for_test_v1();
        let error =
            with_canonical_ranked_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(error.failure(), Failure::Panicked));
        assert_eq!(error.observation().work_upper_bound(), 0);
        assert_eq!(error.observation().peak_storage_units(), 0);
        assert!(error.observation().caught_panic());
        assert!(error.last_invocation().unwrap().invocation().caught_panic());
    });
}

#[test]
fn measured_terminal_work_cut_is_not_claimed_as_an_independent_prefix_oracle() {
    let complete = with_checked(&noop(), |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            policies.observation(budget)
        })
        .unwrap()
    });
    with_checked(&noop(), |checked, budget| {
        let error = with_checks(
            checked,
            budget,
            Limits::new(
                complete.work_upper_bound() - 1,
                Limits::production_hard_ceiling().max_peak_storage(),
            ),
            |_, _| Ok(()),
        )
        .unwrap_err();
        assert!(error.observation().work_upper_bound() > 0);
        assert!(error.observation().work_upper_bound() < complete.work_upper_bound());
        assert!(error.observation().first_denial().is_some());
        assert!(error.last_invocation().is_some());
    });
}

fn direct_ordinary_prefix(limits: Limits, panic_after_tensor: bool) -> InvocationObservationV1 {
    with_projection(&noop(), |projection, budget| {
        projection
            .with_function(0, budget, |context, function| {
                let _panic = panic_after_tensor.then(
                    super::super::super::pliron_pipeline::panic_after_first_production_stage_for_test_v1,
                );
                let mut receipt = InvocationReceiptV1::new(Bound::default(), limits).unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    require_production_pliron_checks_with_observation_v1(
                        context, function, limits, &mut receipt,
                    )
                }));
                assert!(result.is_err() || result.unwrap().is_err());
                receipt.snapshot()
            })
            .unwrap()
    })
}

#[test]
fn actual_interior_work_denials_match_fresh_ordinary_receipts_exactly_once() {
    let complete = with_checked(&noop(), |checked, budget| {
        with_canonical_ranked_policy_checks_v1(checked, budget, |view, budget| {
            view.observation(budget)
        })
        .unwrap()
    });
    // These cuts explore actual accepted prefixes, but are deliberately NOT
    // described as independent phase formulas. The separate ordinary call
    // checks consumer transfer/accounting, not the underlying envelope algebra.
    for work in [
        0,
        complete.work_upper_bound() / 4,
        complete.work_upper_bound() / 2,
        complete.work_upper_bound() * 3 / 4,
        complete.work_upper_bound() - 1,
    ] {
        let limits = Limits::new(work, Limits::production_hard_ceiling().max_peak_storage());
        let expected = direct_ordinary_prefix(limits, false);
        with_checked(&noop(), |checked, budget| {
            let error = with_checks(checked, budget, limits, |_, _| Ok(())).unwrap_err();
            assert_eq!(error.observation(), snapshot(expected.current, expected));
            let history = error.last_invocation().unwrap();
            assert_eq!(history.function(), 0);
            assert_eq!(
                history.floor(),
                CanonicalRankedPolicyResourceObservationV1::default()
            );
            assert_eq!(history.invocation(), error.observation());
            assert!(error.observation().first_denial().is_some());
        });
    }
}

#[test]
fn first_function_interior_panic_keeps_nonzero_real_prefix_once() {
    let limits = Limits::production_hard_ceiling();
    let expected = direct_ordinary_prefix(limits, true);
    assert!(expected.current.work_upper_bound() > 0);
    assert!(expected.current.peak_storage_upper_bound() > 0);
    assert!(expected.caught_panic);
    with_checked(&noop(), |checked, budget| {
        let _panic =
            super::super::super::pliron_pipeline::panic_after_first_production_stage_for_test_v1();
        let error = with_checks(checked, budget, limits, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Analysis {
                function: 0,
                cause: ProductionPlironPreloweringErrorV2::Preservation(
                    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1::AnalysisPanicked {
                        pass: crate::KernelCheckPassKindV1::TensorLayout,
                    }
                ),
            }
        ), "{error:?}");
        assert_eq!(error.observation(), snapshot(expected.current, expected));
        assert_eq!(
            error.last_invocation().unwrap().invocation(),
            error.observation()
        );
    });
    // The RAII selector is restored even when the invoked stage unwinds.
    with_checked(&noop(), |checked, budget| {
        with_checks(checked, budget, limits, |_, _| Ok(())).unwrap();
    });
}

#[test]
fn later_function_interior_panic_keeps_the_real_prior_report_and_prefix_once() {
    with_projection(&two_roots(), |projection, budget| {
        let mut state = AnalysisState::new(Limits::production_hard_ceiling());
        let first = projection
            .with_function(0, budget, |context, function| {
                state.invoke(0, context, function)
            })
            .unwrap()
            .unwrap();
        let first_bound = first.resource_upper_bound;
        let before = state.observation();
        assert!(first.report.is_clean());
        assert_eq!(before.work_upper_bound(), first_bound.work_upper_bound());
        let _panic =
            super::super::super::pliron_pipeline::panic_after_first_production_stage_for_test_v1();
        let error = projection
            .with_function(1, budget, |context, function| {
                state.invoke(1, context, function)
            })
            .unwrap()
            .err()
            .expect("second invocation must panic after TensorLayout");
        assert!(matches!(
            error,
            Failure::Analysis {
                function: 1,
                cause: ProductionPlironPreloweringErrorV2::Preservation(
                    crate::production_analysis::pliron_pass_contract::PlironPassPreservationErrorV1::AnalysisPanicked {
                        pass: crate::KernelCheckPassKindV1::TensorLayout,
                    }
                ),
            }
        ), "{error:?}");
        let history = state.last.unwrap();
        assert_eq!(history.function(), 1);
        assert_eq!(history.floor(), before);
        let local = history.invocation();
        assert!(local.work_upper_bound() > 0);
        assert!(local.caught_panic());
        let total = state.observation();
        assert_eq!(
            total.work_upper_bound(),
            before.work_upper_bound() + local.work_upper_bound()
        );
        assert_eq!(
            total.retained_storage_units(),
            before.retained_storage_units() + local.retained_storage_units()
        );
        assert_eq!(
            total.peak_storage_units(),
            before
                .peak_storage_units()
                .max(before.retained_storage_units() + local.peak_storage_units())
        );
        assert_eq!(first.resource_upper_bound, first_bound);
        assert_eq!(first.report.pass_order().len(), 9);
        drop(first);
        state.release_reports().unwrap();
        assert_eq!(state.observation().retained_storage_units(), 0);
        assert_eq!(
            state.observation().work_upper_bound(),
            total.work_upper_bound()
        );
        assert_eq!(
            state.observation().peak_storage_units(),
            total.peak_storage_units()
        );
    });
}

struct PaidDrop<'w> {
    budget: Budget<'w>,
    observed: Rc<Cell<usize>>,
    panic_after_observation: bool,
}
impl Drop for PaidDrop<'_> {
    fn drop(&mut self) {
        self.observed.set(self.budget.storage());
        if self.panic_after_observation {
            panic!("rejected callback owner's destructor");
        }
    }
}

#[test]
fn moved_original_budget_is_observed_paid_during_rejected_value_destruction() {
    for panic_after_observation in [false, true] {
        let mut work = Work::new(100);
        let mut foreign_work = Work::new(100);
        let mut budget = Budget::new(&mut work, 1000);
        budget.reserve_storage(73).unwrap();
        let guard = Guard::new(&budget);
        let observed = Rc::new(Cell::new(0));
        let error = guard
            .callback(&mut budget, |budget| {
                let original = std::mem::replace(budget, Budget::new(&mut foreign_work, 1000));
                Ok(PaidDrop {
                    budget: original,
                    observed: observed.clone(),
                    panic_after_observation,
                })
            })
            .err()
            .expect("replacement must reject");
        assert!(matches!(error, Failure::Resource(Resource::Accounting)));
        assert_eq!(observed.get(), 73);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn released_backing_and_extra_escaping_reservations_are_rejected_without_query() {
    for add in [false, true] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 1000);
        budget.reserve_storage(73).unwrap();
        let guard = Guard::new(&budget);
        let result = guard.callback(&mut budget, |budget| {
            if add {
                budget.reserve_storage(1)?;
            } else {
                budget.release_storage(1)?;
            }
            Ok(())
        });
        assert!(matches!(
            result,
            Err(Failure::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), if add { 74 } else { 72 });
    }
}

#[test]
fn query_work_denial_and_invalid_ordinal_preserve_the_first_error() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(73).unwrap();
    let guard = Guard::new(&budget);
    assert!(matches!(
        guard.query(&mut budget),
        Err(Failure::Resource(Resource::Work { .. }))
    ));
    assert!(matches!(
        guard.invalid(99),
        Failure::Resource(Resource::Work { .. })
    ));
    assert!(matches!(
        guard.callback(&mut budget, |_| Ok(())),
        Err(Failure::Resource(Resource::Work { .. }))
    ));
    assert_eq!(budget.storage(), 73);
}

#[test]
fn prepaid_row_capacity_is_retained_and_failed_admission_has_no_buffer() {
    let mut work = Work::new(20);
    let bytes = size_of::<Vec<u64>>() + 3 * size_of::<u64>();
    let mut budget = Budget::new(&mut work, bytes - 1);
    assert!(matches!(
        reserve_rows::<u64>(3, &mut budget),
        Err(Failure::Resource(Resource::Storage { .. }))
    ));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
    let mut work = Work::new(20);
    let mut budget = Budget::new(&mut work, 1000);
    let rows = reserve_rows::<u64>(3, &mut budget).unwrap();
    assert_eq!(
        budget.storage(),
        size_of::<Vec<u64>>() + rows.capacity() * size_of::<u64>()
    );
    let paid = budget.storage();
    drop(rows);
    budget.release_storage(paid).unwrap();
}

#[path = "canonical_ranked_checks_nine_oracle_v1_tests.rs"]
mod nine_oracle;
