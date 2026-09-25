use super::r38_bounded_persistent_compute_wait_recycle::*;

fn binding(profile: u8) -> R38PersistentWaitBindingV1 {
    let base = u64::from(profile) * 100;
    R38PersistentWaitBindingV1 {
        lane: profile,
        submission: 13 + base,
        stream: 17 + base,
        prior_stream_submission: 11 + base,
        allocation: 19 + base,
        allocation_storage_generation: 23 + base,
        module: 29 + base,
        dependency: 31 + base,
        event: 37 + base,
        dispatch_shape_digest: 41 + base,
        queue_occurrence: 43 + base,
        attachment_generation: 47 + base,
        dispatch_generation: 53 + base,
        completion_batch: 59 + base,
        signal_generation: 61 + base,
        next_signal_generation: 62 + base,
        module_retain_count: profile + 1,
        dependency_retain_count: profile + 2,
        event_retain_count: profile + 3,
        allocation_owner_count: profile + 4,
        completion_reservation_count: profile + 5,
        completion_midpoint: 67 + base,
    }
}

const STAGES: [R38FailureStageV1; 11] = [
    R38FailureStageV1::PublishedState,
    R38FailureStageV1::DispatchGeneration,
    R38FailureStageV1::CompletionObservation,
    R38FailureStageV1::DispatchCompletion,
    R38FailureStageV1::AllocationCompletion,
    R38FailureStageV1::SignalGeneration,
    R38FailureStageV1::SignalReset,
    R38FailureStageV1::ClosingCurrentness,
    R38FailureStageV1::RecycleCurrentness,
    R38FailureStageV1::RecycleInfrastructure,
    R38FailureStageV1::DispatchRecycle,
];

fn terminal_cases() -> [R38R36TerminalResultV1; 14] {
    let mut cases = [R38R36TerminalResultV1::Recycled; 14];
    cases[1] = R38R36TerminalResultV1::RetryablePreflight(R38RetryablePreflightV1::Poll);
    cases[2] = R38R36TerminalResultV1::RetryablePreflight(R38RetryablePreflightV1::Recycle);
    for (offset, stage) in STAGES.into_iter().enumerate() {
        let index = 3 + offset;
        cases[index] = R38R36TerminalResultV1::ProcessTeardown {
            stage,
            terminal_token: 1000 + index as u64,
        };
    }
    cases
}

fn model(profile: u8) -> R38BoundedPersistentComputeWaitRecycleModelV1 {
    R38BoundedPersistentComputeWaitRecycleModelV1::new_model_only(binding(profile)).unwrap()
}

#[test]
fn binding_limits_and_scripts_reject_out_of_domain_inputs() {
    let valid = binding(0);
    assert!(valid.is_valid());
    for invalid in [
        R38PersistentWaitBindingV1 {
            submission: 0,
            ..valid
        },
        R38PersistentWaitBindingV1 { lane: 3, ..valid },
        R38PersistentWaitBindingV1 {
            next_signal_generation: valid.signal_generation + 2,
            ..valid
        },
        R38PersistentWaitBindingV1 {
            completion_reservation_count: 0,
            ..valid
        },
    ] {
        assert_eq!(
            R38BoundedPersistentComputeWaitRecycleModelV1::new_model_only(invalid),
            Err(R38ModelErrorV1::InvalidBinding)
        );
    }

    let valid_script = R38WaitScriptV1 {
        pending_before_terminal: 0,
        terminal: R38R36TerminalResultV1::Recycled,
    };
    assert_eq!(
        model(0).run_model_only(
            true,
            R38WaitLimitsV1 {
                deadline: R38DeadlineV1::Zero,
                observation_max: 0,
            },
            valid_script,
        ),
        Err(R38ModelErrorV1::InvalidLimits)
    );
    assert_eq!(
        model(0).run_model_only(
            true,
            R38WaitLimitsV1 {
                deadline: R38DeadlineV1::Zero,
                observation_max: 1,
            },
            R38WaitScriptV1 {
                pending_before_terminal: 3,
                ..valid_script
            },
        ),
        Err(R38ModelErrorV1::InvalidScript)
    );
    assert_eq!(
        model(0).run_model_only(
            true,
            R38WaitLimitsV1 {
                deadline: R38DeadlineV1::Zero,
                observation_max: 1,
            },
            R38WaitScriptV1 {
                pending_before_terminal: 0,
                terminal: R38R36TerminalResultV1::ProcessTeardown {
                    stage: R38FailureStageV1::DispatchRecycle,
                    terminal_token: 0,
                },
            },
        ),
        Err(R38ModelErrorV1::InvalidScript)
    );
}

#[test]
fn exhaustive_756_case_model_admitted_wait_domain_has_exact_custody_and_counts() {
    let deadlines = [
        R38DeadlineV1::Zero,
        R38DeadlineV1::Positive {
            pending_observation_limit: 1,
        },
        R38DeadlineV1::Positive {
            pending_observation_limit: 2,
        },
    ];
    let mut checked = 0_u16;
    for profile in [0, 1] {
        for deadline in deadlines {
            for observation_max in [1, 2, 3] {
                for pending_before_terminal in [0, 1, 2] {
                    for terminal in terminal_cases() {
                        let limits = R38WaitLimitsV1 {
                            deadline,
                            observation_max,
                        };
                        let script = R38WaitScriptV1 {
                            pending_before_terminal,
                            terminal,
                        };
                        let initial = model(profile).initial_snapshot_model_only();
                        let state = model(profile).run_model_only(true, limits, script).unwrap();
                        checked += 1;

                        let deadline_limit = match deadline {
                            R38DeadlineV1::Zero => 1,
                            R38DeadlineV1::Positive {
                                pending_observation_limit,
                            } => pending_observation_limit,
                        };
                        let stop = observation_max.min(deadline_limit);
                        let terminal_observation = pending_before_terminal + 1;
                        let timeout = terminal_observation > stop;
                        assert_eq!(state.binding, binding(profile));
                        assert_eq!(state.route, R38RouteV1::BoundedPersistentWait);
                        assert!(state.has_exactly_one_stage_authority());
                        assert!(state.observations > 0);
                        assert!(state.observations <= observation_max);
                        assert_eq!(state.r36_composition_count, state.observations);

                        if timeout {
                            assert_eq!(state.outcome, R38OutcomeV1::Pending);
                            assert_eq!(state.observations, stop);
                            assert!(state.same_timeout_operational_custody(&initial));
                            assert_eq!(
                                state.timeout_reason,
                                Some(if observation_max <= deadline_limit {
                                    R38TimeoutReasonV1::ObservationMaximum
                                } else {
                                    R38TimeoutReasonV1::Deadline
                                })
                            );
                            assert_eq!(state.completion_midpoint, None);
                            assert!(!state.r36_poll_ready);
                            assert!(!state.r36_recycle_finished);
                            continue;
                        }

                        assert_eq!(state.observations, terminal_observation);
                        assert_eq!(state.timeout_reason, None);
                        assert!(!state.active_present);
                        assert_eq!(state.active_execution, R38ExecutionPhaseV1::Absent);
                        assert_eq!(state.active_lane, None);
                        assert_eq!(state.active_submission, None);
                        assert_eq!(state.lane_submission, None);
                        assert_eq!(state.lane_stream, Some(binding(profile).stream));
                        assert_eq!(state.allocation_storage, initial.allocation_storage);
                        assert_eq!(state.module_retain_count, initial.module_retain_count);
                        assert_eq!(
                            state.dependency_retain_count,
                            initial.dependency_retain_count
                        );
                        assert_eq!(state.event_retain_count, initial.event_retain_count);
                        assert_eq!(state.allocation_owner_count, initial.allocation_owner_count);
                        assert_eq!(
                            state.allocation_current_owner,
                            initial.allocation_current_owner
                        );
                        assert_eq!(state.stream_tail_submission, initial.stream_tail_submission);
                        assert_eq!(state.stream_current_owner, initial.stream_current_owner);
                        assert_eq!(
                            state.completion_reservation_count,
                            initial.completion_reservation_count
                        );
                        assert!(!state.submission_recorded);

                        match terminal {
                            R38R36TerminalResultV1::Recycled => {
                                assert_eq!(state.outcome, R38OutcomeV1::Recycled);
                                assert_eq!(state.custody, R38TerminalCustodyV1::Recycled);
                                assert_eq!(state.failure_stage, None);
                                assert!(!state.terminal_poisoned);
                                assert_eq!(
                                    state.completion_midpoint,
                                    Some(binding(profile).completion_midpoint)
                                );
                                assert!(state.r36_poll_ready);
                                assert!(state.r36_recycle_finished);
                            }
                            R38R36TerminalResultV1::RetryablePreflight(
                                R38RetryablePreflightV1::Poll,
                            ) => {
                                assert_eq!(state.outcome, R38OutcomeV1::Terminal);
                                assert_eq!(state.custody, R38TerminalCustodyV1::Published);
                                assert_eq!(state.failure_stage, None);
                                assert!(state.terminal_poisoned);
                                assert_eq!(state.completion_midpoint, None);
                                assert!(!state.r36_poll_ready);
                                assert!(!state.r36_recycle_finished);
                            }
                            R38R36TerminalResultV1::RetryablePreflight(
                                R38RetryablePreflightV1::Recycle,
                            ) => {
                                assert_eq!(state.outcome, R38OutcomeV1::Terminal);
                                assert_eq!(state.custody, R38TerminalCustodyV1::Completed);
                                assert_eq!(state.failure_stage, None);
                                assert!(state.terminal_poisoned);
                                assert_eq!(
                                    state.completion_midpoint,
                                    Some(binding(profile).completion_midpoint)
                                );
                                assert!(state.r36_poll_ready);
                                assert!(!state.r36_recycle_finished);
                            }
                            R38R36TerminalResultV1::ProcessTeardown {
                                stage,
                                terminal_token,
                            } => {
                                assert_eq!(state.outcome, R38OutcomeV1::Terminal);
                                assert_eq!(state.failure_stage, Some(stage));
                                assert!(state.terminal_poisoned);
                                assert_eq!(
                                    state.custody,
                                    R38TerminalCustodyV1::ProcessTeardown {
                                        stage,
                                        retained_native_stage: stage.retained_native_stage(),
                                        terminal_token,
                                    }
                                );
                                assert_eq!(
                                    state.completion_midpoint,
                                    stage
                                        .observes_ready_midpoint()
                                        .then_some(binding(profile).completion_midpoint)
                                );
                                assert_eq!(state.r36_poll_ready, stage.observes_ready_midpoint());
                                assert!(!state.r36_recycle_finished);
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 756);
}

#[test]
fn zero_deadline_observes_before_testing_the_boundary() {
    let limits = R38WaitLimitsV1 {
        deadline: R38DeadlineV1::Zero,
        observation_max: 3,
    };
    let ready = model(0)
        .run_model_only(
            true,
            limits,
            R38WaitScriptV1 {
                pending_before_terminal: 0,
                terminal: R38R36TerminalResultV1::Recycled,
            },
        )
        .unwrap();
    assert_eq!(ready.outcome, R38OutcomeV1::Recycled);
    assert_eq!(ready.observations, 1);

    let timeout = model(0)
        .run_model_only(
            true,
            limits,
            R38WaitScriptV1 {
                pending_before_terminal: 1,
                terminal: R38R36TerminalResultV1::Recycled,
            },
        )
        .unwrap();
    assert_eq!(timeout.outcome, R38OutcomeV1::Pending);
    assert_eq!(timeout.observations, 1);
    assert_eq!(timeout.timeout_reason, Some(R38TimeoutReasonV1::Deadline));
}

#[test]
fn observation_max_terminates_without_incrementing_past_the_bound() {
    let state = model(1)
        .run_model_only(
            true,
            R38WaitLimitsV1 {
                deadline: R38DeadlineV1::Positive {
                    pending_observation_limit: 2,
                },
                observation_max: 1,
            },
            R38WaitScriptV1 {
                pending_before_terminal: 2,
                terminal: R38R36TerminalResultV1::Recycled,
            },
        )
        .unwrap();
    assert_eq!(state.outcome, R38OutcomeV1::Pending);
    assert_eq!(state.observations, 1);
    assert_eq!(
        state.timeout_reason,
        Some(R38TimeoutReasonV1::ObservationMaximum)
    );
}

#[test]
fn missing_queue_retains_exact_published_authority_without_an_observation() {
    let mut checked = 0;
    for profile in [0, 1] {
        let initial = model(profile).initial_snapshot_model_only();
        let state = model(profile)
            .run_model_only(
                false,
                R38WaitLimitsV1 {
                    deadline: R38DeadlineV1::Zero,
                    observation_max: 1,
                },
                R38WaitScriptV1 {
                    pending_before_terminal: 0,
                    terminal: R38R36TerminalResultV1::Recycled,
                },
            )
            .unwrap();
        checked += 1;
        assert_eq!(state.outcome, R38OutcomeV1::Terminal);
        assert_eq!(state.custody, R38TerminalCustodyV1::Published);
        assert!(state.terminal_poisoned);
        assert!(!state.active_present);
        assert_eq!(state.active_execution, R38ExecutionPhaseV1::Absent);
        assert_eq!(state.active_lane, None);
        assert_eq!(state.active_submission, None);
        assert_eq!(state.lane_submission, None);
        assert_eq!(state.lane_stream, initial.lane_stream);
        assert_eq!(state.allocation_storage, initial.allocation_storage);
        assert_eq!(state.module_retain_count, initial.module_retain_count);
        assert_eq!(
            state.dependency_retain_count,
            initial.dependency_retain_count
        );
        assert_eq!(state.event_retain_count, initial.event_retain_count);
        assert_eq!(state.allocation_owner_count, initial.allocation_owner_count);
        assert_eq!(
            state.allocation_current_owner,
            initial.allocation_current_owner
        );
        assert_eq!(state.stream_tail_submission, initial.stream_tail_submission);
        assert_eq!(state.stream_current_owner, initial.stream_current_owner);
        assert_eq!(
            state.completion_reservation_count,
            initial.completion_reservation_count
        );
        assert!(!state.submission_recorded);
        assert_eq!(state.observations, 0);
        assert_eq!(state.r36_composition_count, 0);
        assert_eq!(state.completion_midpoint, None);
        assert!(!state.r36_poll_ready);
        assert!(!state.r36_recycle_finished);
        assert!(state.has_exactly_one_stage_authority());
    }
    assert_eq!(checked, 2);
}

#[test]
fn exhaustive_eight_case_route_matrix_keeps_poll_and_fallbacks_unchanged() {
    let mut checked = 0;
    for call in [R38CallV1::Poll, R38CallV1::Wait] {
        for phase in [
            R38EntryPhaseV1::PublishedPersistent,
            R38EntryPhaseV1::PreparedPersistent,
            R38EntryPhaseV1::Materialized,
            R38EntryPhaseV1::Other,
        ] {
            let route = r38_route_model_only(call, phase);
            checked += 1;
            match (call, phase) {
                (R38CallV1::Poll, _) => assert_eq!(route, R38RouteV1::Poll),
                (R38CallV1::Wait, R38EntryPhaseV1::PublishedPersistent) => {
                    assert_eq!(route, R38RouteV1::BoundedPersistentWait)
                }
                (
                    R38CallV1::Wait,
                    R38EntryPhaseV1::PreparedPersistent
                    | R38EntryPhaseV1::Materialized
                    | R38EntryPhaseV1::Other,
                ) => assert_eq!(route, R38RouteV1::LegacyPollWait),
            }
        }
    }
    assert_eq!(checked, 8);
}

#[test]
fn owning_carriers_remain_structurally_move_only() {
    let source = include_str!("r38_bounded_persistent_compute_wait_recycle.rs");
    let authority = source
        .split_once("struct R38PublishedAuthorityV1")
        .unwrap()
        .1
        .split_once("impl R38BoundedPersistentComputeWaitRecycleModelV1")
        .unwrap()
        .0;
    let owner = source
        .split_once("pub struct R38BoundedPersistentComputeWaitRecycleModelV1")
        .unwrap()
        .1
        .split_once("struct R38PublishedAuthorityV1")
        .unwrap()
        .0;
    for region in [authority, owner] {
        assert!(!region.contains("derive(Clone"));
        assert!(!region.contains("impl Clone"));
        assert!(!region.contains("impl Copy"));
    }
    assert!(owner.contains("published: R38PublishedAuthorityV1"));
    assert!(source.contains("pub fn run_model_only(\n        self,"));
    assert!(source.contains("constant-time closed form"));
}
