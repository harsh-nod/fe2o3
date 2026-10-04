mod begin_observation_tests {
    use super::*;

    #[test]
    fn observed_begin_rejects_restored_structure_epoch_at_exact_and_short_limits() {
        let hard = Limits::production_hard_ceiling();
        let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
        for short in [0, 1, 2] {
            let (context, function) = fixture();
            let mut ordinary = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let initial = ordinary.initial_identity_resource_upper_bound_v1();
            let old_storage = ordinary
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
            let captured = provider
                .capture_with_resource_limits_v1(hard)
                .map_err(identity_error)
                .unwrap();
            let capture = captured.resource_upper_bound;
            drop(captured);
            let compared = ProductionAnalysisResourceUpperBoundV1::checked_phase(
                PHASE,
                capture.work_upper_bound()
                    + old_storage
                    + capture.retained_storage_upper_bound()
                    + 1,
                capture.retained_storage_upper_bound(),
                capture.peak_storage_upper_bound() - capture.retained_storage_upper_bound(),
            )
            .unwrap();
            let complete_prefix = initial.checked_then_retain(compared, PHASE).unwrap();
            let total = initial.checked_then_retain(complete_prefix, PHASE).unwrap();
            let global = Limits::new(
                total.work_upper_bound(),
                total.peak_storage_upper_bound() - usize::from(short == 2),
            );
            // The unmodified ordinary session is the live caller floor.
            let mut receipt = Receipt::new(initial, global).unwrap();
            let calls = Rc::new(Calls::default());
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                FaultProvider {
                    live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    fault: Fault::Healthy,
                    calls: Rc::clone(&calls),
                },
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            phase.commit(initial).unwrap();
            // A real mutable borrow advances the epoch without changing bytes.
            drop(function.get_operation().deref_mut(&context));
            let local = Limits::new(
                compared.work_upper_bound() - usize::from(short == 1),
                capture.peak_storage_upper_bound(),
            );
            let phase = receipt.phase(PHASE, old_storage).unwrap();
            let executed = Cell::new(false);
            let result = owner.run_contiguous_pass_with_observation_v1(
                pass,
                ProductionAnalysisReplacementLimitsV1 {
                    input: local,
                    output: hard,
                },
                || {
                    executed.set(true);
                    Ok::<_, ()>(())
                },
                Some(&phase.observer(&Ok)),
            );
            drop(phase);
            assert!(!executed.get());
            assert_eq!(owner.next, 0);
            assert!(owner.pending.is_none() && owner.lineage.is_none());
            assert!(owner.certificates.is_empty());
            let state = receipt.snapshot();
            assert!(!state.caught_panic);
            assert_eq!(calls.captures.get(), 2);
            if short == 0 {
                assert!(matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::StaleMutationEpoch { .. })
                ));
                assert_eq!(calls.comparisons.get(), 1);
                assert_eq!(calls.epochs.get(), 4);
                assert_eq!(calls.snapshot_drops.get(), 2);
                assert_eq!(state.first_denial, None);
                assert_eq!(
                    state.committed.work_upper_bound(),
                    complete_prefix.work_upper_bound()
                );
                assert_eq!(
                    state.committed.peak_storage_upper_bound(),
                    complete_prefix.peak_storage_upper_bound()
                );
                assert_eq!(
                    state.committed.retained_storage_upper_bound(),
                    complete_prefix.peak_storage_upper_bound()
                );
            } else {
                let denial = state.first_denial.unwrap();
                assert_eq!(
                    denial.resource,
                    if short == 1 {
                        "work upper bound"
                    } else {
                        "peak storage upper bound"
                    }
                );
                assert_eq!(
                    result,
                    Err(PlironPassPreservationErrorV1::ResourceLimit {
                        resource: denial.resource
                    })
                );
                assert_eq!(calls.comparisons.get(), 0);
                assert_eq!(calls.epochs.get(), 3);
                assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
                if short == 1 {
                    let accepted = initial.checked_then_retain(capture, PHASE).unwrap();
                    assert_eq!(denial.phase, PHASE);
                    assert_eq!(
                        state.committed.work_upper_bound(),
                        accepted.work_upper_bound()
                    );
                    assert_eq!(
                        state.committed.peak_storage_upper_bound(),
                        accepted.peak_storage_upper_bound()
                    );
                    assert_eq!(calls.snapshot_drops.get(), 2);
                } else {
                    assert!(state.committed.work_upper_bound() > initial.work_upper_bound());
                    assert!(
                        state.committed.peak_storage_upper_bound()
                            < complete_prefix.peak_storage_upper_bound()
                    );
                }
            }
            global
                .require(
                    PHASE,
                    initial.checked_then_retain(state.committed, PHASE).unwrap(),
                )
                .unwrap();
            // None still compares with its original capture-only allowance.
            assert!(matches!(
                ordinary.run_contiguous_pass_with_resource_limits_v1::<(), ()>(
                    pass,
                    ProductionAnalysisReplacementLimitsV1 {
                        input: local,
                        output: hard
                    },
                    || panic!("stale input cannot execute"),
                ),
                Err(PlironPassPreservationErrorV1::StaleMutationEpoch { .. })
            ));
        }
    }

    #[test]
    fn observed_begin_keeps_converted_capture_denial_and_unwind() {
        let hard = Limits::production_hard_ceiling();
        let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
        for fault in [Fault::CaptureDenied, Fault::DeniedThenPanic] {
            let (context, function) = fixture();
            let calls = Rc::new(Calls::default());
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                FaultProvider {
                    live: LivePlironStructuralIdentityProviderV1::new(&context, &function),
                    fault: Fault::Healthy,
                    calls: Rc::clone(&calls),
                },
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            let initial = owner.initial_identity_resource_upper_bound_v1();
            phase.commit(initial).unwrap();
            let old_storage = owner
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            drop(function.get_operation().deref_mut(&context));
            owner.provider.fault = fault;
            let phase = receipt.phase(PHASE, old_storage).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| {
                owner.run_contiguous_pass_with_observation_v1::<(), ()>(
                    pass,
                    hard,
                    || panic!("refused capture cannot execute"),
                    Some(&phase.observer(&Ok)),
                )
            }));
            drop(phase);
            if fault == Fault::CaptureDenied {
                assert!(matches!(
                    result,
                    Ok(Err(PlironPassPreservationErrorV1::IdentityUnavailable {
                        source_code: "test-converted-resource-denial",
                        ..
                    }))
                ));
            } else {
                assert!(result.is_err());
            }
            assert_eq!(calls.captures.get(), 2);
            assert_eq!(calls.comparisons.get(), 0);
            assert_eq!(calls.snapshot_drops.get(), 1);
            assert!(owner.pending.is_none() && owner.lineage.is_none());
            let state = receipt.snapshot();
            assert_eq!(state.committed, initial);
            assert_eq!(state.caught_panic, fault == Fault::DeniedThenPanic);
            let denial = state.first_denial.unwrap();
            assert_eq!(denial.phase, Phase::StructuralIdentity);
            assert_eq!(denial.resource, "work upper bound");
            assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
        }
    }

    #[test]
    fn observed_contiguous_callback_error_checkpoints_but_panic_does_not() {
        let hard = Limits::production_hard_ceiling();
        let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass();
        for (panic, short) in [(false, false), (false, true), (true, false)] {
            let (context, function) = fixture();
            let mut ordinary = PlironPassContractSessionV1::new(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
            )
            .unwrap();
            let initial = ordinary.initial_identity_resource_upper_bound_v1();
            let old_storage = ordinary
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound();
            let limits = ProductionAnalysisReplacementLimitsV1 {
                input: hard,
                output: if short {
                    Limits::new(0, usize::MAX)
                } else {
                    hard
                },
            };
            let expected =
                ordinary.run_contiguous_pass_with_resource_limits_v1(pass, limits, || {
                    assert!(!panic, "ordinary callback panic");
                    Err::<(), _>("analysis error")
                });
            let mut receipt = Receipt::new(Default::default(), hard).unwrap();
            let phase = receipt.phase(PHASE, 0).unwrap();
            let mut owner = begin_production_pliron_pass_contract_session_with_observation_v1(
                LivePlironStructuralIdentityProviderV1::new(&context, &function),
                hard,
                Some(&phase.observer(&Ok)),
            )
            .unwrap();
            phase.commit(initial).unwrap();
            let phase = receipt.phase(PHASE, old_storage).unwrap();
            // Hold the real original identity during the allocation-free callback.
            phase
                .observer(&Ok)
                .require(
                    hard,
                    PHASE,
                    ProductionAnalysisResourceUpperBoundV1::checked_phase(PHASE, 0, old_storage, 0),
                )
                .unwrap();
            let calls = Cell::new(0);
            let actual = owner.run_contiguous_pass_with_observation_v1(
                pass,
                limits,
                || {
                    calls.set(calls.get() + 1);
                    assert!(!panic, "observed callback panic");
                    Err::<(), _>("analysis error")
                },
                Some(&phase.observer(&Ok)),
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(actual, expected);
            if !panic && !short {
                assert_eq!(actual, Ok(Err("analysis error")));
                let checkpoint = owner.last_checkpoint_resource_upper_bound_v1().unwrap();
                assert_eq!(owner.certificates, ordinary.certificates);
                phase.commit(checkpoint).unwrap();
                assert_eq!(
                    receipt.complete(),
                    Ok(initial
                        .checked_then_replace_retained(old_storage, checkpoint, PHASE)
                        .unwrap())
                );
            } else {
                drop(phase);
                let state = receipt.snapshot();
                assert_eq!(
                    state.committed.work_upper_bound(),
                    initial.work_upper_bound()
                );
                assert_eq!(
                    state.committed.peak_storage_upper_bound(),
                    initial.peak_storage_upper_bound()
                );
                assert_eq!(
                    state.committed.retained_storage_upper_bound(),
                    initial.peak_storage_upper_bound()
                );
                assert_eq!(owner.next, 0);
                assert!(owner.certificates.is_empty());
                assert_eq!(owner.pending.is_some(), panic);
                assert_eq!(receipt.snapshot().caught_panic, panic);
                assert_eq!(receipt.snapshot().first_denial.is_some(), short);
                if panic {
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::CaughtPanic));
                } else {
                    let denial = receipt.snapshot().first_denial.unwrap();
                    assert_eq!(denial.phase, PHASE);
                    assert_eq!(
                        denial.resource,
                        "remaining identity capture work upper bound"
                    );
                    assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(denial)));
                }
            }
        }
    }
}
