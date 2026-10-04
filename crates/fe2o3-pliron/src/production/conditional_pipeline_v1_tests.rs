#[cfg(feature = "internal-proof-staging")]
mod source259_consuming_pipeline {
    use super::{source259_execution::conditional_effect::effect_owner, *};
    use crate::production_analysis as pa;
    type Limits = ProductionAnalysisResourceLimitsV1;

    fn checked() -> ProductionConditionalPipelineAnalysisV1 {
        effect_owner(0).check_pipeline_v1().unwrap()
    }

    fn bounds_resource_case(case: Option<pa::ConditionalBoundsResourceCaseV1>) {
        for reads in [None, Some(&[][..])] {
            let pending = effect_owner(0);
            let mut resources =
                ProductionAnalysisResourceContractV1::new(Limits::production_hard_ceiling());
            resources
                .admit_retained(
                    Phase::PipelineVerification,
                    pending.analysis._analyses.resource_upper_bound(),
                )
                .unwrap();
            let input = pending
                .prepare_pipeline_subject_with_budget_v1(&mut resources, None, reads)
                .unwrap();
            assert_eq!(
                input.conditional_reads().map(<[_]>::len),
                reads.map(<[_]>::len)
            );
            match case {
                None => pa::test_conditional_bounds_composition_v1(&input),
                Some(case) => {
                    let error = pa::test_capture_rejection_v1(
                        &input,
                        resources.remaining(Phase::PipelineVerification).unwrap(),
                        pa::CaptureFaultV1::BoundsBorrowResources(case),
                    );
                    assert!(matches!(
                        error,
                        pa::PipelineErrorV1::ConditionalValidation(
                            pa::conditional_validation_v1::ErrorV1::Family {
                                position: usize::MAX
                            }
                        )
                    ));
                }
            }
        }
    }

    #[test]
    fn conditional_bounds_resources_composition_preserves_ordinary_and_omits_reborrowed_producer() {
        bounds_resource_case(None);
    }

    #[test]
    fn conditional_bounds_resources_foreign_manager_never_debits_either_ledger() {
        bounds_resource_case(Some(pa::ConditionalBoundsResourceCaseV1::ForeignManager));
    }

    #[test]
    fn conditional_bounds_resources_borrow_exact_and_one_short_report_validation_work() {
        use pa::ConditionalBoundsResourceCaseV1::*;
        for case in [Exact, ManagerWorkShort, ReceiptWorkShort] {
            bounds_resource_case(Some(case));
        }
    }

    #[test]
    fn conditional_bounds_slot_rejects_omission_and_foreign_custody() {
        use pa::{CaptureFaultV1 as Fault, conditional_validation_v1::ErrorV1 as Error};
        let first = effect_owner(0);
        let other = effect_owner(0);
        let mut resources =
            ProductionAnalysisResourceContractV1::new(Limits::production_hard_ceiling());
        for pending in [&first, &other] {
            resources
                .admit_retained(
                    Phase::PipelineVerification,
                    pending.analysis._analyses.resource_upper_bound(),
                )
                .unwrap();
        }
        let input = first.prepare_pipeline_subject_v1(&mut resources).unwrap();
        let foreign = other.prepare_pipeline_subject_v1(&mut resources).unwrap();
        for (fault, expected) in [
            (
                Fault::MissingBounds,
                Error::Order {
                    expected: 1,
                    observed: 2,
                },
            ),
            (Fault::ForeignBoundsManager, Error::Ledger),
            (
                Fault::ForeignBoundsSubject(foreign.pending_subject()),
                Error::Subject,
            ),
        ] {
            let error = pa::test_capture_rejection_v1(
                &input,
                resources.remaining(Phase::PipelineVerification).unwrap(),
                fault,
            );
            let pa::PipelineErrorV1::ConditionalValidation(actual) = error else {
                panic!("wrong bounds refusal: {error:?}")
            };
            assert_eq!(actual, expected);
        }
    }

    fn replay_failure(fault: replay_test_v1::Fault) -> ProductionConditionalPipelineErrorV1 {
        let pending = effect_owner(0);
        let function = pending.analysis.payload.function();
        let epoch = pending.analysis._mutation_epoch;
        let original = pending.analysis._analyses.resource_upper_bound();
        let selected = pending.selections()[0];
        let scope = replay_test_v1::install(fault);
        let error = pending
            .check_pipeline_v1()
            .err()
            .expect("replay fault succeeded");
        drop(scope);
        let pending = error.pending_analysis();
        assert_eq!(pending.analysis.payload.function(), function);
        assert_eq!(pending.analysis._mutation_epoch, epoch);
        assert_eq!(pending.analysis._analyses.resource_upper_bound(), original);
        assert_eq!(pending.selections(), &[selected]);
        assert_eq!(pending.pending_pipeline_checks().len(), 9);
        assert_eq!(
            pending
                .legacy_report()
                .coverage_summary()
                .total_view_proved(),
            0
        );
        let first = error.analysis.first.as_ref().unwrap();
        assert_eq!(first.report.test_pass_count(), 9);
        assert!(!first.report.independently_validated());
        let f = error.analysis.observations[0].unwrap();
        let g = error.analysis.observations[1].unwrap();
        assert_eq!(f.observation.current, first.resource_upper_bound);
        assert_eq!(f.observation.current, f.observation.committed);
        assert_eq!(f.observation.first_denial, None);
        assert!(!f.observation.caught_panic);
        assert_eq!(
            g.floor,
            f.floor
                .checked_then_retain(f.observation.current, Phase::PipelineVerification)
                .unwrap()
        );
        error
    }

    #[test]
    fn consuming_owner_retains_first_report_after_replay_quota_or_panic() {
        use replay_test_v1::Fault;
        for fault in [Fault::WorkShort, Fault::Panic] {
            let error = replay_failure(fault);
            let history = error.analysis.observations[1].unwrap();
            let state = history.observation;
            assert_eq!(
                error.analysis.resources,
                history
                    .floor
                    .checked_then_retain(state.current, Phase::PipelineVerification)
                    .unwrap()
            );
            assert!(error.analysis.replay.is_none());
            assert!(error.analysis.input.is_none());
            match fault {
                Fault::WorkShort => {
                    assert!(matches!(
                        error.failure,
                        ConditionalPipelineFailureV1::Pipeline(_)
                    ));
                    assert_eq!(state.first_denial.unwrap().resource, "work upper bound");
                    assert!(state.current.work_upper_bound() > 0);
                    assert!(
                        state.current.work_upper_bound()
                            < error
                                .analysis
                                .first
                                .as_ref()
                                .unwrap()
                                .resource_upper_bound
                                .work_upper_bound()
                    );
                    assert!(!state.caught_panic);
                    assert!(!error.analysis.caught_panic);
                    assert!(!error.pending_analysis()._session.poisoned);
                }
                Fault::Panic => {
                    assert!(matches!(
                        error.failure,
                        ConditionalPipelineFailureV1::CaughtPanic
                    ));
                    assert_eq!(state.current, Bound::default());
                    assert_eq!(state.first_denial, None);
                    assert!(state.caught_panic);
                    assert!(error.analysis.caught_panic);
                    assert!(error.pending_analysis()._session.poisoned);
                }
                _ => unreachable!(),
            }
            drop(error);
            drop(checked());
        }
    }

    #[test]
    fn consuming_owner_retains_both_reports_on_replay_mismatch() {
        use replay_test_v1::Fault;
        for fault in [Fault::Resources, Fault::Payload] {
            let error = replay_failure(fault);
            let first = error.analysis.first.as_ref().unwrap();
            let replay = error.analysis.replay.as_ref().unwrap();
            let history = error.analysis.observations[1].unwrap();
            assert_eq!(history.observation.current, first.resource_upper_bound);
            assert_eq!(history.observation.committed, first.resource_upper_bound);
            assert_eq!(history.observation.first_denial, None);
            assert!(!history.observation.caught_panic);
            assert!(!error.analysis.caught_panic);
            assert!(!error.pending_analysis()._session.poisoned);
            let expected = history
                .floor
                .checked_then_retain(history.observation.current, Phase::PipelineVerification)
                .unwrap()
                .checked_then_retain(
                    conditional_report_comparison_bound_v1(first.resource_upper_bound).unwrap(),
                    Phase::PipelineVerification,
                )
                .unwrap();
            assert_eq!(error.analysis.resources, expected);
            assert!(replay.resource_upper_bound.retained_storage_upper_bound() > 0);
            match fault {
                Fault::Resources => {
                    assert!(matches!(
                        error.failure,
                        ConditionalPipelineFailureV1::ReplayResources
                    ));
                    assert_eq!(
                        replay.resource_upper_bound.work_upper_bound(),
                        first.resource_upper_bound.work_upper_bound() + 1
                    );
                    assert_eq!(replay.report, first.report);
                }
                Fault::Payload => {
                    assert!(matches!(
                        error.failure,
                        ConditionalPipelineFailureV1::ReplayPayload
                    ));
                    assert_eq!(replay.resource_upper_bound, first.resource_upper_bound);
                    assert_ne!(replay.report, first.report);
                }
                _ => unreachable!(),
            }
            drop(error);
            drop(checked());
        }
    }

    #[test]
    fn consuming_replay_fault_scope_resets_before_consumption_on_unwind() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _scope = replay_test_v1::install(replay_test_v1::Fault::Panic);
            panic!("unwind before consuming constructor");
        }));
        assert!(result.is_err());
        drop(checked());
    }

    #[test]
    fn consuming_owner_retains_original_pending_and_releases_only_replay_storage() {
        let pending = effect_owner(0);
        let function = pending.analysis.payload.function();
        let epoch = pending.analysis._mutation_epoch;
        let original = pending.analysis._analyses.resource_upper_bound();
        let selected = pending.selections()[0];
        let owner = pending.check_pipeline_v1().unwrap();
        assert_eq!(owner.pending.analysis.payload.function(), function);
        assert_eq!(owner.pending.analysis._mutation_epoch, epoch);
        assert_eq!(
            owner.pending.analysis._analyses.resource_upper_bound(),
            original
        );
        assert_eq!(owner.pending.selections(), &[selected]);
        assert!(std::ptr::eq(
            owner.kernel().unwrap(),
            owner.pending_analysis().kernel().unwrap()
        ));
        assert!(owner.input.is_some());
        assert!(owner.replay.is_none());
        assert!(!owner.caught_panic);
        let first = owner.first.as_ref().unwrap();
        assert_eq!(first.report.test_pass_count(), 9);
        assert!(!first.report.independently_validated());
        for state in owner.observations {
            let state = state.unwrap().observation;
            assert_eq!(state.current, first.resource_upper_bound);
            assert_eq!(state.current, state.committed);
            assert_eq!(state.first_denial, None);
            assert!(!state.caught_panic);
        }
        let q = owner.observations[0].unwrap().floor;
        let h = first.resource_upper_bound;
        let phase = Phase::PipelineVerification;
        assert_eq!(
            owner.observations[1].unwrap().floor,
            q.checked_then_retain(h, phase).unwrap()
        );
        let expected = q
            .checked_then_retain(h, phase)
            .unwrap()
            .checked_then_retain(h, phase)
            .unwrap()
            .checked_then_retain(conditional_report_comparison_bound_v1(h).unwrap(), phase)
            .unwrap()
            .checked_then_replace_retained(
                h.retained_storage_upper_bound(),
                Bound::default(),
                phase,
            )
            .unwrap();
        assert_eq!(owner.resources, expected);
        assert_eq!(owner.pending.pending_pipeline_checks().len(), 9);
        assert_eq!(
            owner
                .pending
                .legacy_report()
                .coverage_summary()
                .total_view_proved(),
            0
        );
    }

    #[test]
    fn consuming_owner_exact_and_one_short_resource_limits() {
        let reference = checked();
        let bound = reference.resources;
        drop(reference);
        let exact = effect_owner(0)
            .check_pipeline_with_limits_v1(Limits::new(
                bound.work_upper_bound(),
                bound.peak_storage_upper_bound(),
            ))
            .unwrap();
        assert_eq!(exact.resources, bound);
        drop(exact);
        for work_short in [true, false] {
            let error = effect_owner(0)
                .check_pipeline_with_limits_v1(Limits::new(
                    bound.work_upper_bound() - usize::from(work_short),
                    bound.peak_storage_upper_bound() - usize::from(!work_short),
                ))
                .err()
                .expect("one-short consuming call succeeded");
            assert!(matches!(
                error.failure,
                ConditionalPipelineFailureV1::Resource(_)
            ));
            assert!(error.analysis.replay.is_none());
            assert!(!error.analysis.caught_panic);
            assert_eq!(error.pending_analysis().pending_pipeline_checks().len(), 9);
            if work_short {
                assert!(error.analysis.first.is_some());
                assert!(error.analysis.observations[0].is_some());
                assert!(error.analysis.observations[1].is_none());
                assert!(error.analysis.resources.work_upper_bound() < bound.work_upper_bound());
            }
        }
    }

    #[test]
    fn consuming_owner_preserves_semantic_error_and_pending_diagnostics() {
        for case in 1..=5 {
            let pending = effect_owner(case);
            let original = pending.analysis._analyses.resource_upper_bound();
            let error = pending
                .check_pipeline_v1()
                .err()
                .expect("invalid semantics succeeded");
            assert!(matches!(
                error.failure,
                ConditionalPipelineFailureV1::Pipeline(pa::PipelineErrorV1::ConditionalSemantic(_))
            ));
            let state = error.analysis.observations[0].unwrap().observation;
            let floor = error.analysis.observations[0].unwrap().floor;
            assert_eq!(
                error.analysis.resources,
                floor
                    .checked_then_retain(state.current, Phase::PipelineVerification)
                    .unwrap()
            );
            assert!(state.current.work_upper_bound() > 0);
            assert_eq!(state.first_denial, None);
            assert!(!state.caught_panic);
            assert!(error.analysis.resources.work_upper_bound() > original.work_upper_bound());
            assert!(error.analysis.first.is_none());
            assert!(error.analysis.replay.is_none());
            assert_eq!(
                error
                    .pending_analysis()
                    .analysis
                    ._analyses
                    .resource_upper_bound(),
                original
            );
        }
    }

    #[test]
    fn consuming_owner_preserves_preparation_failure_and_caught_panic() {
        let pending = effect_owner(0);
        let p = pending.analysis._analyses.resource_upper_bound();
        drop(
            pending
                .analysis
                .payload
                .function()
                .deref_mut(&pending._session.inner.context),
        );
        let stale = pending
            .check_pipeline_v1()
            .err()
            .expect("stale owner succeeded");
        assert!(matches!(
            stale.failure,
            ConditionalPipelineFailureV1::Session(ProductionSessionErrorV1::RankedGraphChanged)
        ));
        assert_eq!(stale.analysis.observations, [None; 2]);
        assert!(stale.analysis.resources.work_upper_bound() > p.work_upper_bound());
        assert_eq!(
            stale
                .pending_analysis()
                .analysis
                ._analyses
                .resource_upper_bound(),
            p
        );
        drop(stale);
        let pending = effect_owner(0);
        pa::panic_next_production_analysis_for_test_v1();
        let error = pending
            .check_pipeline_v1()
            .err()
            .expect("injected panic succeeded");
        assert!(matches!(
            error.failure,
            ConditionalPipelineFailureV1::CaughtPanic
        ));
        assert!(
            error.analysis.observations[0]
                .unwrap()
                .observation
                .caught_panic
        );
        assert!(error.analysis.caught_panic);
        let history = error.analysis.observations[0].unwrap();
        assert_eq!(
            error.analysis.resources,
            history
                .floor
                .checked_then_retain(history.observation.current, Phase::PipelineVerification)
                .unwrap()
        );
        assert!(error.pending_analysis()._session.poisoned);
        assert!(error.analysis.first.is_none());
    }
}
