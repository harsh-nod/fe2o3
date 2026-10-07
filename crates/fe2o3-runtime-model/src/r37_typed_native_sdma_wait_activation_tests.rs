use super::r37_typed_native_sdma_wait_activation::*;

fn binding(kind: R37CopyKindV1, profile: u8) -> R37WaitBindingV1 {
    let base = if profile == 0 { 0 } else { 100 };
    let submission = 13 + base;
    R37WaitBindingV1 {
        kind,
        submission,
        stream: 17 + base,
        source_allocation: 19 + base,
        destination_allocation: 23 + base,
        source_storage_generation: 29 + base,
        destination_storage_generation: 31 + base,
        restored_source_storage: 37 + base,
        restored_destination_storage: 41 + base,
        dependency_submission: 43 + base,
        dependency_retain_count: profile + 1,
        source_custody_count: profile + 2,
        destination_custody_count: profile + 3,
        stream_owner_count: profile + 4,
        published_index_frame: R37OrderedFrameV1 {
            predecessor: 11 + base,
            current: submission,
            successor: 47 + base,
        },
        stream_frame: R37OrderedFrameV1 {
            predecessor: 7 + base,
            current: submission,
            successor: 53 + base,
        },
        native_identity: R37NativeIdentityV1 {
            owner_id: 59 + base,
            request_id: 61 + base,
        },
    }
}

#[derive(Clone, Copy)]
enum LegalCase {
    Complete(R37CompletionDispositionV1),
    ExactTypedTimeout,
    NonTimeoutRetryable,
    IdentityChange(R37IdentityChangeStageV1),
    Teardown,
}

fn observation(binding: R37WaitBindingV1, case: LegalCase) -> R37NativeWaitObservationV1 {
    match case {
        LegalCase::Complete(_) => R37NativeWaitObservationV1::Complete,
        LegalCase::ExactTypedTimeout => {
            R37NativeWaitObservationV1::ExactTypedTimeout(binding.native_identity)
        }
        LegalCase::NonTimeoutRetryable => {
            R37NativeWaitObservationV1::NonTimeoutRetryable(binding.native_identity)
        }
        LegalCase::IdentityChange(stage) => R37NativeWaitObservationV1::IdentityChange {
            stage,
            returned: R37NativeIdentityV1 {
                owner_id: binding.native_identity.owner_id + 1,
                request_id: binding.native_identity.request_id,
            },
        },
        LegalCase::Teardown => R37NativeWaitObservationV1::Teardown {
            terminal_token: binding.native_identity.owner_id + 2,
        },
    }
}

fn disposition(case: LegalCase) -> R37CompletionDispositionV1 {
    match case {
        LegalCase::Complete(disposition) => disposition,
        _ => R37CompletionDispositionV1::Settle,
    }
}

#[test]
fn binding_and_observation_contracts_are_input_only_and_exact() {
    let valid = binding(R37CopyKindV1::Directional, 0);
    assert!(valid.is_valid());
    assert!(
        R37NativeWaitObservationV1::ExactTypedTimeout(valid.native_identity).is_valid_for(valid)
    );
    assert!(
        !R37NativeWaitObservationV1::ExactTypedTimeout(R37NativeIdentityV1 {
            owner_id: valid.native_identity.owner_id + 1,
            request_id: valid.native_identity.request_id,
        })
        .is_valid_for(valid)
    );
    assert!(
        !R37NativeWaitObservationV1::NonTimeoutRetryable(R37NativeIdentityV1 {
            owner_id: valid.native_identity.owner_id,
            request_id: valid.native_identity.request_id + 1,
        })
        .is_valid_for(valid)
    );
    assert!(
        !R37NativeWaitObservationV1::IdentityChange {
            stage: R37IdentityChangeStageV1::Completed,
            returned: valid.native_identity,
        }
        .is_valid_for(valid)
    );
    assert!(!R37NativeWaitObservationV1::Teardown { terminal_token: 0 }.is_valid_for(valid));

    for invalid in [
        R37WaitBindingV1 {
            submission: 0,
            ..valid
        },
        R37WaitBindingV1 {
            destination_allocation: valid.source_allocation,
            ..valid
        },
        R37WaitBindingV1 {
            dependency_retain_count: 0,
            ..valid
        },
        R37WaitBindingV1 {
            published_index_frame: R37OrderedFrameV1 {
                current: valid.submission + 1,
                ..valid.published_index_frame
            },
            ..valid
        },
    ] {
        assert!(matches!(
            R37TypedNativeSdmaWaitModelV1::new_model_only(invalid),
            Err(R37ModelErrorV1::InvalidBinding)
        ));
    }
}

#[test]
fn exhaustive_56_case_wait_domain_preserves_every_custody_rule() {
    let cases = [
        LegalCase::Complete(R37CompletionDispositionV1::Settle),
        LegalCase::Complete(R37CompletionDispositionV1::ContinuationReady),
        LegalCase::ExactTypedTimeout,
        LegalCase::NonTimeoutRetryable,
        LegalCase::IdentityChange(R37IdentityChangeStageV1::Pending),
        LegalCase::IdentityChange(R37IdentityChangeStageV1::Completed),
        LegalCase::Teardown,
    ];
    let mut checked = 0;
    for profile in [0, 1] {
        for kind in [R37CopyKindV1::Directional, R37CopyKindV1::SameDevice] {
            for deadline in [R37DeadlineClassV1::Zero, R37DeadlineClassV1::Positive] {
                for case in cases {
                    let binding = binding(kind, profile);
                    let model = R37TypedNativeSdmaWaitModelV1::new_model_only(binding).unwrap();
                    let initial = model.initial_snapshot_model_only();
                    let observation = observation(binding, case);
                    assert!(observation.is_valid_for(binding));
                    let state = model
                        .run_model_only(deadline, observation, disposition(case))
                        .unwrap();
                    checked += 1;

                    assert_eq!(state.binding, binding);
                    assert_eq!(state.native_observation_count, 1);
                    assert_eq!(state.published_index_frame, binding.published_index_frame);
                    assert_eq!(state.stream_frame, binding.stream_frame);
                    assert_eq!(
                        state.route,
                        match kind {
                            R37CopyKindV1::Directional => R37RouteV1::NativeDirectionalWait,
                            R37CopyKindV1::SameDevice => R37RouteV1::NativeSameDeviceWait,
                        }
                    );

                    match case {
                        LegalCase::Complete(R37CompletionDispositionV1::Settle) => {
                            assert_eq!(state.outcome, R37OutcomeV1::Succeeded);
                            assert!(!state.active_present);
                            assert_eq!(state.active_phase, R37ActivePhaseV1::Absent);
                            assert!(!state.published_index_retained);
                            assert_eq!(
                                state.source_storage,
                                R37StorageV1::Restored {
                                    storage_token: binding.restored_source_storage,
                                }
                            );
                            assert_eq!(
                                state.destination_storage,
                                R37StorageV1::Restored {
                                    storage_token: binding.restored_destination_storage,
                                }
                            );
                            assert_eq!(
                                state.dependency_retain_count + 1,
                                binding.dependency_retain_count
                            );
                            assert_eq!(
                                state.source_custody_count + 1,
                                binding.source_custody_count
                            );
                            assert_eq!(
                                state.destination_custody_count + 1,
                                binding.destination_custody_count
                            );
                            assert_eq!(state.stream_owner_count + 1, binding.stream_owner_count);
                            assert!(!state.stream_current_retained);
                            assert!(state.settled);
                            assert!(state.completion_recorded);
                            assert!(!state.continuation_ready);
                        }
                        LegalCase::Complete(R37CompletionDispositionV1::ContinuationReady) => {
                            assert_eq!(state.outcome, R37OutcomeV1::Pending);
                            assert!(state.active_present);
                            assert_eq!(state.active_phase, R37ActivePhaseV1::Ready);
                            assert!(!state.published_index_retained);
                            assert_eq!(
                                state.source_storage,
                                R37StorageV1::Restored {
                                    storage_token: binding.restored_source_storage,
                                }
                            );
                            assert_eq!(
                                state.destination_storage,
                                R37StorageV1::Restored {
                                    storage_token: binding.restored_destination_storage,
                                }
                            );
                            assert_eq!(
                                state.dependency_retain_count,
                                binding.dependency_retain_count
                            );
                            assert_eq!(state.source_custody_count, binding.source_custody_count);
                            assert_eq!(
                                state.destination_custody_count,
                                binding.destination_custody_count
                            );
                            assert_eq!(state.stream_owner_count, binding.stream_owner_count);
                            assert!(state.stream_current_retained);
                            assert!(!state.settled);
                            assert!(!state.completion_recorded);
                            assert!(state.continuation_ready);
                            assert_eq!(state.continuation_publication_count, 0);
                        }
                        LegalCase::ExactTypedTimeout => {
                            assert_eq!(state.outcome, R37OutcomeV1::Pending);
                            assert!(!state.terminal_poisoned);
                            assert!(state.same_operational_custody(&initial));
                            assert!(!state.settled);
                            assert!(!state.continuation_ready);
                        }
                        LegalCase::NonTimeoutRetryable => {
                            assert_eq!(state.outcome, R37OutcomeV1::Terminal);
                            assert_eq!(
                                state.native_custody,
                                R37NativeCustodyV1::TerminalPending(binding.native_identity)
                            );
                            assert!(state.terminal_preserves_in_flight_retains());
                        }
                        LegalCase::IdentityChange(stage) => {
                            let returned = match observation {
                                R37NativeWaitObservationV1::IdentityChange { returned, .. } => {
                                    returned
                                }
                                _ => unreachable!(),
                            };
                            let expected = match stage {
                                R37IdentityChangeStageV1::Pending => {
                                    R37NativeCustodyV1::TerminalPending(returned)
                                }
                                R37IdentityChangeStageV1::Completed => {
                                    R37NativeCustodyV1::TerminalCompleted(returned)
                                }
                            };
                            assert_eq!(state.outcome, R37OutcomeV1::Terminal);
                            assert_eq!(state.native_custody, expected);
                            assert!(state.terminal_preserves_in_flight_retains());
                        }
                        LegalCase::Teardown => {
                            assert_eq!(state.outcome, R37OutcomeV1::Terminal);
                            assert_eq!(
                                state.native_custody,
                                R37NativeCustodyV1::TerminalTeardown(
                                    binding.native_identity.owner_id + 2
                                )
                            );
                            assert!(state.terminal_preserves_in_flight_retains());
                        }
                    }

                    if !matches!(case, LegalCase::Complete(_)) {
                        assert!(!state.settled);
                        assert!(!state.completion_recorded);
                        assert!(!state.continuation_ready);
                        assert_eq!(state.continuation_publication_count, 0);
                    }
                    if state.terminal_poisoned {
                        assert!(!state.active_present);
                        assert!(!state.published_index_retained);
                    }
                }
            }
        }
    }
    assert_eq!(checked, 56);
}

#[test]
fn zero_deadline_still_performs_exactly_one_native_observation() {
    let binding = binding(R37CopyKindV1::SameDevice, 1);
    for case in [
        LegalCase::Complete(R37CompletionDispositionV1::Settle),
        LegalCase::Complete(R37CompletionDispositionV1::ContinuationReady),
        LegalCase::ExactTypedTimeout,
        LegalCase::NonTimeoutRetryable,
        LegalCase::IdentityChange(R37IdentityChangeStageV1::Pending),
        LegalCase::IdentityChange(R37IdentityChangeStageV1::Completed),
        LegalCase::Teardown,
    ] {
        let state = R37TypedNativeSdmaWaitModelV1::new_model_only(binding)
            .unwrap()
            .run_model_only(
                R37DeadlineClassV1::Zero,
                observation(binding, case),
                disposition(case),
            )
            .unwrap();
        assert_eq!(state.native_observation_count, 1);
    }
}

#[test]
fn matching_non_timeout_retryable_is_terminal_not_timeout() {
    let binding = binding(R37CopyKindV1::Directional, 0);
    let state = R37TypedNativeSdmaWaitModelV1::new_model_only(binding)
        .unwrap()
        .run_model_only(
            R37DeadlineClassV1::Positive,
            R37NativeWaitObservationV1::NonTimeoutRetryable(binding.native_identity),
            R37CompletionDispositionV1::Settle,
        )
        .unwrap();
    assert_eq!(state.outcome, R37OutcomeV1::Terminal);
    assert!(state.terminal_poisoned);
    assert!(!state.active_present);
    assert!(!state.published_index_retained);
}

#[test]
fn invalid_returned_identity_is_rejected_before_the_transition() {
    let binding = binding(R37CopyKindV1::Directional, 0);
    let result = R37TypedNativeSdmaWaitModelV1::new_model_only(binding)
        .unwrap()
        .run_model_only(
            R37DeadlineClassV1::Zero,
            R37NativeWaitObservationV1::ExactTypedTimeout(R37NativeIdentityV1 {
                owner_id: binding.native_identity.owner_id + 1,
                request_id: binding.native_identity.request_id,
            }),
            R37CompletionDispositionV1::Settle,
        );
    assert_eq!(result, Err(R37ModelErrorV1::InvalidObservation));
}

#[test]
fn exhaustive_eight_case_route_matrix_keeps_poll_unchanged() {
    let mut checked = 0;
    for call in [R37CallV1::Poll, R37CallV1::Wait] {
        for phase in [
            R37EntryPhaseV1::PublishedDirectional,
            R37EntryPhaseV1::PublishedSameDevice,
            R37EntryPhaseV1::Ready,
            R37EntryPhaseV1::Other,
        ] {
            let route = r37_route_model_only(call, phase);
            checked += 1;
            match (call, phase) {
                (R37CallV1::Poll, _) => assert_eq!(route, R37RouteV1::Poll),
                (R37CallV1::Wait, R37EntryPhaseV1::PublishedDirectional) => {
                    assert_eq!(route, R37RouteV1::NativeDirectionalWait)
                }
                (R37CallV1::Wait, R37EntryPhaseV1::PublishedSameDevice) => {
                    assert_eq!(route, R37RouteV1::NativeSameDeviceWait)
                }
                (R37CallV1::Wait, R37EntryPhaseV1::Ready | R37EntryPhaseV1::Other) => {
                    assert_eq!(route, R37RouteV1::LegacyWaitPoll)
                }
            }
        }
    }
    assert_eq!(checked, 8);
}

#[test]
fn owning_carriers_remain_structurally_move_only() {
    let source = include_str!("r37_typed_native_sdma_wait_activation.rs");
    let authority = source
        .split_once("struct R37PublishedAuthorityV1")
        .unwrap()
        .1
        .split_once("/// Move-only owner")
        .unwrap()
        .0;
    let owner = source
        .split_once("pub struct R37TypedNativeSdmaWaitModelV1")
        .unwrap()
        .1
        .split_once("impl R37TypedNativeSdmaWaitModelV1")
        .unwrap()
        .0;
    assert!(!authority.contains("derive(Clone"));
    assert!(!owner.contains("derive(Clone"));
}
