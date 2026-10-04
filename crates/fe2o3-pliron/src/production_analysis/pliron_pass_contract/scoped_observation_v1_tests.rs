mod scoped_observation_tests {
    use super::*;
    type Bound = ProductionAnalysisResourceUpperBoundV1;
    type PreservationError = PlironPassPreservationErrorV1;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Callback {
        Success,
        AnalysisError,
        PreservationError,
        Panic,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum LimitCase {
        Exact,
        InputWork,
        InputPeak,
        OutputWork,
        OutputPeak,
        CheckpointWork,
    }

    fn case(pass: KernelCheckPassKindV1, callback: Callback, limit_case: LimitCase) {
        let hard = Limits::production_hard_ceiling();
        let position = PRODUCTION_PLIRON_PASS_CONTRACTS_V1
            .iter()
            .position(|contract| contract.pass() == pass)
            .unwrap();
        let (context, function) = fixture();
        let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
        let capture = provider
            .capture_with_resource_limits_v1(hard)
            .map_err(identity_error)
            .unwrap();
        let captured = capture.resource_upper_bound;
        drop(capture);
        let scope = scoped_progress_input_resource_upper_bound_v1().unwrap();
        let mut reference = None;
        for observed in [false, true] {
            let mut receipt = Receipt::new(Bound::default(), hard).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            phase
                .commit(owner.initial_identity_resource_upper_bound_v1())
                .unwrap();
            for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..position] {
                let old = owner
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound();
                let phase = receipt.phase(PHASE, old).unwrap();
                owner
                    .run_contiguous_pass_with_observation_v1(
                        contract.pass(),
                        hard,
                        || Ok::<(), ()>(()),
                        Some(&phase.observer(&Ok)),
                    )
                    .unwrap()
                    .unwrap();
                phase
                    .commit(owner.last_checkpoint_resource_upper_bound_v1().unwrap())
                    .unwrap();
            }
            let anchor = receipt.snapshot().committed;
            let old = owner
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            assert_eq!(old, captured.retained_storage_upper_bound());
            let capture_prefix = Bound::checked_phase(
                PHASE,
                captured.work_upper_bound() + old + 1,
                captured.retained_storage_upper_bound() + 1,
                old + captured.peak_storage_upper_bound() - captured.retained_storage_upper_bound(),
            )
            .unwrap();
            let end = Bound::checked_phase(
                PHASE,
                capture_prefix.work_upper_bound() + captured.retained_storage_upper_bound(),
                capture_prefix.retained_storage_upper_bound(),
                capture_prefix.peak_storage_upper_bound()
                    - capture_prefix.retained_storage_upper_bound(),
            )
            .unwrap();
            assert!(end.peak_storage_upper_bound() >= scope.peak_storage_upper_bound());
            let full = scope.checked_then_retain(end, PHASE).unwrap();
            let limits = ProductionAnalysisReplacementLimitsV1 {
                input: Limits::new(
                    scope.work_upper_bound() - usize::from(limit_case == LimitCase::InputWork),
                    scope.peak_storage_upper_bound()
                        - usize::from(limit_case == LimitCase::InputPeak),
                ),
                output: Limits::new(
                    if limit_case == LimitCase::OutputWork {
                        scope.work_upper_bound() - 1
                    } else {
                        full.work_upper_bound()
                            - usize::from(limit_case == LimitCase::CheckpointWork)
                    },
                    if limit_case == LimitCase::OutputPeak {
                        scope.peak_storage_upper_bound() - 1
                    } else {
                        full.peak_storage_upper_bound()
                    },
                ),
            };
            let phase = receipt.phase(PHASE, old).unwrap();
            let observer = phase.observer(&Ok);
            let calls = Cell::new(0);
            let execute = || -> Result<Result<(), &'static str>, PreservationError> {
                calls.set(calls.get() + 1);
                match callback {
                    Callback::Success => Ok(Ok(())),
                    Callback::AnalysisError => Ok(Err("analysis")),
                    Callback::PreservationError => {
                        Err(PreservationError::InvalidSessionState { detail: "callback" })
                    }
                    Callback::Panic => panic!("scoped callback panic"),
                }
            };
            let observe = observed.then_some(&observer);
            let result = match pass {
                KernelCheckPassKindV1::BarrierConvergence => owner
                    .run_scoped_barrier_with_observation_v1(
                        limits,
                        |input| {
                            let (actual_context, actual_function) = input.endpoints()?;
                            assert!(std::ptr::eq(actual_context, &context));
                            assert!(std::ptr::eq(actual_function, &function));
                            execute()
                        },
                        observe,
                    ),
                KernelCheckPassKindV1::SemanticRefinement => owner
                    .run_scoped_semantic_refinement_with_observation_v1(
                        limits,
                        |input| {
                            let (actual_context, actual_function) = input.endpoints()?;
                            assert!(std::ptr::eq(actual_context, &context));
                            assert!(std::ptr::eq(actual_function, &function));
                            execute()
                        },
                        observe,
                    ),
                _ => unreachable!(),
            };
            let record = (
                result,
                owner.certificates.clone(),
                owner.next,
                owner.pending.is_some(),
                owner.last_checkpoint_resource_upper_bound_v1(),
            );
            let completed = limit_case == LimitCase::Exact
                && matches!(callback, Callback::Success | Callback::AnalysisError);
            if completed {
                assert_eq!(owner.last_checkpoint_resource_upper_bound_v1(), Some(full));
            }
            if !observed {
                reference = Some(record);
                drop(phase);
                continue;
            }
            assert_eq!(Some(record), reference);
            let early_refusal = !matches!(limit_case, LimitCase::Exact | LimitCase::CheckpointWork);
            assert_eq!(calls.get(), usize::from(!early_refusal));
            let initial_scope = Bound::checked_phase(PHASE, 0, old, 0)
                .unwrap()
                .checked_then_retain(scope, PHASE)
                .unwrap();
            let accepted_scope = anchor
                .checked_then_replace_retained(old, initial_scope, PHASE)
                .unwrap();
            if completed {
                phase.commit(full).unwrap();
            } else {
                drop(phase);
            }
            let state = receipt.snapshot();
            let expected = if early_refusal {
                anchor
            } else {
                let local = if limit_case == LimitCase::CheckpointWork {
                    scope.checked_then_retain(capture_prefix, PHASE).unwrap()
                } else {
                    full
                };
                let after = if limit_case == LimitCase::Exact && !completed {
                    accepted_scope
                } else {
                    anchor
                        .checked_then_replace_retained(old, local, PHASE)
                        .unwrap()
                };
                let peak = after
                    .peak_storage_upper_bound()
                    .max(accepted_scope.peak_storage_upper_bound());
                let retained = if completed {
                    after.retained_storage_upper_bound()
                } else {
                    peak
                };
                Bound::checked_phase(PHASE, after.work_upper_bound(), retained, peak - retained)
                    .unwrap()
            };
            assert_eq!(state.committed, expected);
            assert_eq!(
                state.caught_panic,
                limit_case == LimitCase::Exact && callback == Callback::Panic
            );
            let denial = (limit_case != LimitCase::Exact).then_some((
                PHASE,
                if matches!(limit_case, LimitCase::InputPeak | LimitCase::OutputPeak) {
                    "peak storage upper bound"
                } else {
                    "work upper bound"
                },
            ));
            assert_eq!(
                state
                    .first_denial
                    .map(|error| (error.phase, error.resource)),
                denial
            );
        }
    }

    #[test]
    fn scoped_results_and_certificates_match_none() {
        for pass in [
            KernelCheckPassKindV1::BarrierConvergence,
            KernelCheckPassKindV1::SemanticRefinement,
        ] {
            for callback in [
                Callback::Success,
                Callback::AnalysisError,
                Callback::PreservationError,
                Callback::Panic,
            ] {
                case(pass, callback, LimitCase::Exact);
            }
        }
    }

    #[test]
    fn scoped_exact_and_short_local_limits() {
        for pass in [
            KernelCheckPassKindV1::BarrierConvergence,
            KernelCheckPassKindV1::SemanticRefinement,
        ] {
            for short in [
                LimitCase::Exact,
                LimitCase::InputWork,
                LimitCase::InputPeak,
                LimitCase::OutputWork,
                LimitCase::OutputPeak,
                LimitCase::CheckpointWork,
            ] {
                case(pass, Callback::AnalysisError, short);
            }
        }
    }

    #[test]
    fn scoped_begin_recapture_checks_global_live_snapshot_overlap() {
        let hard = Limits::production_hard_ceiling();
        let then = |a: Bound, b: Bound| a.checked_then_retain(b, PHASE).unwrap();
        for pass in [
            KernelCheckPassKindV1::BarrierConvergence,
            KernelCheckPassKindV1::SemanticRefinement,
        ] {
            for short in [false, true] {
                let (context, function) = fixture();
                let position = PRODUCTION_PLIRON_PASS_CONTRACTS_V1
                    .iter()
                    .position(|contract| contract.pass() == pass)
                    .unwrap();
                let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
                let probe = provider
                    .capture_with_resource_limits_v1(hard)
                    .map_err(identity_error)
                    .unwrap();
                let capture = probe.resource_upper_bound;
                drop(probe);

                // This real ordinary session remains live as the caller floor.
                let mut baseline = PlironPassContractSessionV1::new(
                    LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    hard,
                )
                .unwrap();
                let mut anchor = baseline.initial_identity_resource_upper_bound_v1();
                for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..position] {
                    let old = baseline
                        .lineage_identity_resource_upper_bound_v1()
                        .retained_storage_upper_bound();
                    baseline
                        .run_contiguous_pass_with_resource_limits_v1(contract.pass(), hard, || {
                            Ok::<(), ()>(())
                        })
                        .unwrap()
                        .unwrap();
                    anchor = anchor
                        .checked_then_replace_retained(
                            old,
                            baseline.last_checkpoint_resource_upper_bound_v1().unwrap(),
                            PHASE,
                        )
                        .unwrap();
                }
                let old = baseline
                    .lineage_identity_resource_upper_bound_v1()
                    .retained_storage_upper_bound();
                assert_eq!(old, capture.retained_storage_upper_bound());
                let old_bound = Bound::checked_phase(PHASE, 0, old, 0).unwrap();
                let scope = scoped_progress_input_resource_upper_bound_v1().unwrap();
                let compared = Bound::checked_phase(
                    PHASE,
                    capture.work_upper_bound() + old + capture.retained_storage_upper_bound() + 1,
                    capture.retained_storage_upper_bound(),
                    capture.peak_storage_upper_bound() - capture.retained_storage_upper_bound(),
                )
                .unwrap();
                let accepted_scope = anchor
                    .checked_then_replace_retained(
                        old,
                        then(capture, then(old_bound, scope)),
                        PHASE,
                    )
                    .unwrap();
                let full = anchor
                    .checked_then_replace_retained(
                        old,
                        then(capture, then(scope, then(old_bound, compared))),
                        PHASE,
                    )
                    .unwrap();
                assert!(
                    full.peak_storage_upper_bound() > accepted_scope.peak_storage_upper_bound()
                );
                let total = then(anchor, full);
                let limits = Limits::new(
                    total.work_upper_bound(),
                    total.peak_storage_upper_bound() - usize::from(short),
                );
                let mut receipt = Receipt::new(anchor, limits).unwrap();
                let phase = receipt.phase(PHASE, 0).unwrap();
                let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                    LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    hard,
                    Some(&phase.observer(&Ok)),
                )
                .unwrap();
                phase
                    .commit(owner.initial_identity_resource_upper_bound_v1())
                    .unwrap();
                for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..position] {
                    let replaced = owner
                        .lineage_identity_resource_upper_bound_v1()
                        .retained_storage_upper_bound();
                    let phase = receipt.phase(PHASE, replaced).unwrap();
                    owner
                        .run_contiguous_pass_with_observation_v1(
                            contract.pass(),
                            hard,
                            || Ok::<(), ()>(()),
                            Some(&phase.observer(&Ok)),
                        )
                        .unwrap()
                        .unwrap();
                    phase
                        .commit(owner.last_checkpoint_resource_upper_bound_v1().unwrap())
                        .unwrap();
                }
                assert_eq!(receipt.snapshot().committed, anchor);
                let phase = receipt.phase(PHASE, old).unwrap();
                let observer = phase.observer(&Ok);
                let retained = observer
                    .with_projection(
                        &|local| old_bound.checked_then_retain(local, PHASE),
                        |nested| provider.capture_with_resource_observation_v1(hard, Some(nested)),
                    )
                    .map_err(identity_error)
                    .unwrap();
                assert_eq!(retained.resource_upper_bound, capture);

                // Change the actual mutation-attempt epoch without changing IR bytes.
                drop(function.get_operation().deref_mut(&context));
                let called = Cell::new(false);
                let result = observer.with_projection(
                    &|local| {
                        retained
                            .resource_upper_bound
                            .checked_then_retain(local, PHASE)
                    },
                    |nested| match pass {
                        KernelCheckPassKindV1::BarrierConvergence => owner
                            .run_scoped_barrier_with_observation_v1(
                                hard,
                                |_| {
                                    called.set(true);
                                    Ok(Ok::<(), ()>(()))
                                },
                                Some(nested),
                            ),
                        KernelCheckPassKindV1::SemanticRefinement => owner
                            .run_scoped_semantic_refinement_with_observation_v1(
                                hard,
                                |_| {
                                    called.set(true);
                                    Ok(Ok::<(), ()>(()))
                                },
                                Some(nested),
                            ),
                        _ => unreachable!(),
                    },
                );
                drop(retained);
                drop(phase);
                assert!(!called.get());
                assert_eq!(owner.next, position);
                assert_eq!(owner.certificates, baseline.certificates);
                assert!(owner.pending.is_none() && owner.lineage.is_none());
                let state = receipt.snapshot();
                assert!(!state.caught_panic);
                assert_eq!(
                    state.committed.retained_storage_upper_bound(),
                    state.committed.peak_storage_upper_bound()
                );
                if short {
                    let denial = state.first_denial.unwrap();
                    assert_eq!(denial.phase, Phase::StructuralIdentity);
                    assert_eq!(denial.resource, "peak storage upper bound");
                    assert_eq!(
                        result,
                        Err(PreservationError::ResourceLimit {
                            resource: denial.resource
                        })
                    );
                    assert!(
                        state.committed.work_upper_bound() >= accepted_scope.work_upper_bound()
                    );
                    assert!(state.committed.work_upper_bound() < full.work_upper_bound());
                    assert!(
                        state.committed.peak_storage_upper_bound()
                            >= accepted_scope.peak_storage_upper_bound()
                    );
                    assert!(
                        state.committed.peak_storage_upper_bound()
                            < full.peak_storage_upper_bound()
                    );
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
                } else {
                    assert!(
                        matches!(result, Err(PreservationError::StaleMutationEpoch { pass: actual, .. }) if actual == pass)
                    );
                    assert_eq!(state.first_denial, None);
                    let held = Bound::checked_phase(
                        PHASE,
                        full.work_upper_bound(),
                        full.peak_storage_upper_bound(),
                        0,
                    )
                    .unwrap();
                    assert_eq!(state.committed, held);
                    assert_eq!(receipt.complete(), Ok(held));
                }
                limits
                    .require(PHASE, then(anchor, state.committed))
                    .unwrap();
            }
        }
    }
}
