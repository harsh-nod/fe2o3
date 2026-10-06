use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn genuine_private_owner_keeps_exact_bytes_roots_and_fresh_paired_nine_reports() {
    let module = fixture();
    with_checked(&module, |checked, budget| {
        let original = checked.inventory(budget).unwrap().owner();
        let bytes = original.canonical().canonical_bytes().to_vec();
        let floor = budget.storage();
        with_canonical_private_policy_checks_v1(checked, budget, |policies, budget| {
            assert!(std::ptr::eq(policies.owner(budget)?, original));
            assert_eq!(policies.owner(budget)?.canonical().canonical_bytes(), bytes);
            assert_eq!(policies.owner(budget)?.module().kernels, module.kernels);
            assert_eq!(policies.function_count(budget)?, 2);
            for function in 0..2 {
                let report = policies.report(function, budget)?;
                assert_eq!(report.paired_stage_count(), 9);
                assert_eq!(report.reports().pass_order(), &crate::production_analysis::pliron_pipeline::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2);
                assert!(report.reports().is_clean());
                assert_eq!(policies.history(function, budget)?.function(), function);
                assert!(!report.grants_artifact_or_launch_authority());
            }
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!policies.ranked_verification_is_complete());
            assert!(!policies.grants_artifact_or_launch_authority());
            let first = policies.history(0, budget)?;
            let second = policies.history(1, budget)?;
            assert_eq!(second.floor().work_upper_bound(), first.invocation().work_upper_bound());
            assert_eq!(second.floor().retained_storage_units(), first.invocation().retained_storage_units());
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn ordinary_consumer_still_refuses_the_same_retained_private_module() {
    with_checked(&fixture(), |checked, budget| {
        let error =
            with_canonical_ranked_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(error.failure(), Failure::UnsupportedGraph { .. }));
        assert_eq!(error.observation().work_upper_bound(), 0);
    });
}

#[test]
fn private_query_poison_cannot_be_ignored_or_borrow_another_ledger() {
    for foreign in [false, true] {
        with_checked(&fixture(), |checked, budget| {
            let mut other_work = Work::new(1 << 48);
            let mut other = Budget::new(&mut other_work, 1 << 32);
            let error =
                with_canonical_private_policy_checks_v1(checked, budget, |policies, budget| {
                    if foreign {
                        assert!(policies.function_count(&mut other).is_err());
                    } else {
                        assert!(policies.report(2, budget).is_err());
                    }
                    Ok(())
                })
                .unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::Resource(Resource::Accounting) | Failure::InvalidQuery { function: 2 }
            ));
            assert_eq!(other.work(), 0);
        });
    }
}

#[test]
fn callback_error_panic_and_storage_imbalance_restore_the_original_floor() {
    for mode in 0..3 {
        with_checked(&fixture(), |checked, budget| {
            let floor = budget.storage();
            let failure = with_canonical_private_policy_checks_v1(
                checked,
                budget,
                |_, budget| -> Result<(), Failure> {
                    match mode {
                        0 => Err(Failure::Callback("explicit")),
                        1 => panic!("private callback"),
                        _ => {
                            budget.reserve_storage(1)?;
                            Ok(())
                        }
                    }
                },
            )
            .unwrap_err();
            assert!(matches!(
                failure.failure(),
                Failure::Callback(_) | Failure::Panicked | Failure::Resource(Resource::Accounting)
            ));
            assert_eq!(budget.storage(), floor);
            assert!(failure.observation().work_upper_bound() > 0);
        });
    }
}

#[test]
fn analysis_work_and_storage_denial_preserve_genuine_failure_history() {
    for limits in [Limits::new(0, usize::MAX), Limits::new(usize::MAX, 0)] {
        with_checked(&fixture(), |checked, budget| {
            let floor = budget.storage();
            let error = with_private_checks(checked, budget, limits, |_, _| Ok(())).unwrap_err();
            assert!(matches!(
                error.failure(),
                Failure::AnalysisLimit { .. } | Failure::Analysis { .. }
            ));
            assert!(error.observation().first_denial().is_some());
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn actual_fixed_stage_panic_is_not_a_clean_private_report_and_next_invocation_recovers() {
    use crate::production_analysis::pliron_pipeline::panic_next_production_analysis_for_test_v1;
    with_checked(&fixture(), |checked, budget| {
        let floor = budget.storage();
        panic_next_production_analysis_for_test_v1();
        let error =
            with_canonical_private_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(error.observation().caught_panic());
        assert!(error.last_invocation().is_some());
        assert_eq!(budget.storage(), floor);
    });
    with_checked(&fixture(), |checked, budget| {
        with_canonical_private_policy_checks_v1(checked, budget, |policies, budget| {
            assert_eq!(policies.function_count(budget)?, 2);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn caller_success_cannot_hide_a_definitely_reached_callee_cycle() {
    let mut module = fixture();
    helper(&mut module).terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    with_checked(&module, |checked, budget| {
        let error =
            with_canonical_private_policy_checks_v1(checked, budget, |_, _| Ok(())).unwrap_err();
        assert!(matches!(
            error.failure(),
            Failure::Analysis { function: 1, .. }
        ));
        let last = error
            .last_invocation()
            .expect("real callee invocation recorded");
        assert_eq!(last.function(), 1);
        assert!(last.floor().work_upper_bound() > 0);
        assert!(last.invocation().work_upper_bound() > 0);
    });
}
