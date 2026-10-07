use super::*;

#[test]
fn retained_control_replay_panics_keep_the_current_phase_outside_the_loan() {
    let stages = [
        RetainedReplayInjectedStageV1::MappedFacts,
        RetainedReplayInjectedStageV1::Detach,
        RetainedReplayInjectedStageV1::AuthenticatedConstruction,
        RetainedReplayInjectedStageV1::Retain,
        RetainedReplayInjectedStageV1::FinalAudit,
    ];
    for (index, stage) in stages.into_iter().enumerate() {
        let mut script = RetainedReplayScriptV1 {
            fail: Some(stage),
            panic: true,
            trace: Vec::new(),
        };
        let mut phases = RetainedReplayScriptCustodyV1::Input(RetainedReplayScriptRequestV1(0x35));
        let mut retakes = 0;
        let mut poisoned = false;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute_live_model_custody_v1(
                &mut script,
                |_| Ok::<_, ()>(()),
                |script| {
                    let _ = execute_retained_replay_script_v1(script, &mut phases);
                },
                |_, ()| {
                    retakes += 1;
                    std::panic::panic_any("secondary retake");
                },
                |_| poisoned = true,
            )
        }));
        assert_eq!(
            caught
                .unwrap_err()
                .downcast_ref::<RetainedReplayInjectedStageV1>(),
            Some(&stage)
        );
        assert_eq!(retakes, 1);
        assert!(poisoned);
        assert_eq!(script.trace, stages[..=index]);
        let outcome = phases.into_outcome(Err(stage));
        use PersistentRetainedControlReplayPipelineOutcomeV1 as Outcome;
        match (index, outcome) {
            (0 | 1, Outcome::BeforeDetach { request, .. }) => assert_eq!(request.0, 0x35),
            (2, Outcome::Storage { storage, .. }) => assert_eq!(storage.0, 0x35),
            (3, Outcome::Data { data, .. }) => assert_eq!(data.0, 0x35),
            (4, Outcome::Attached { attached, .. }) => assert_eq!(attached.0, 0x35),
            _ => panic!("panic lost the active replay phase"),
        }
    }
}

#[test]
fn persistent_bind_early_validation_retains_each_prepared_entry_on_error_and_panic() {
    for count in [1, 3] {
        for failed in 0..count {
            for panic in [false, true] {
                let entries = persistent_bind_prepared_entries_for_test(count);
                let expected: Vec<_> = entries
                    .iter()
                    .map(|entry| entry.storage_identity.unwrap())
                    .collect();
                let mut visited = 0;
                let result =
                    validate_persistent_bind_inputs_v1(&mut visited, &entries, |visited, entry| {
                        let index = *visited;
                        *visited += 1;
                        assert_eq!(
                            entry
                                .allocation
                                .owner
                                .local_native_for_sdma()
                                .unwrap()
                                .storage_identity(),
                            expected[index]
                        );
                        if index == failed {
                            if panic {
                                std::panic::panic_any(index);
                            }
                            return Err(index);
                        }
                        Ok(())
                    });
                assert_eq!(visited, failed + 1);
                if panic {
                    assert_eq!(result.unwrap_err().downcast_ref::<usize>(), Some(&failed));
                } else {
                    assert_eq!(result.unwrap(), Err(failed));
                }
                for (entry, identity) in entries.iter().zip(expected) {
                    assert!(matches!(
                        entry.state,
                        PersistentComputeUseStateV1::Prepared(_)
                    ));
                    assert_eq!(entry.allocation.owner.live_use_count(), 1);
                    assert_eq!(
                        entry
                            .allocation
                            .owner
                            .local_native_for_sdma()
                            .unwrap()
                            .storage_identity(),
                        identity
                    );
                }
            }
        }
    }
}

#[test]
fn persistent_bind_terminal_retake_dominates_successful_roster_cancellation() {
    for count in [1, 3] {
        for terminal in [false, true] {
            let mut entries = persistent_bind_prepared_entries_for_test(count);
            let expected: Vec<_> = entries
                .iter()
                .map(|entry| entry.storage_identity.unwrap())
                .collect();
            let mut session =
                persistent_compute_cancellation_test_session(test_queue_key(293, 1), None, None);
            let (operation, retake) = execute_live_model_custody_v1(
                &mut session,
                |_| Ok(()),
                |_| Err::<(), _>("early validation"),
                |_, ()| {
                    if terminal {
                        Err("closing retake")
                    } else {
                        Ok(())
                    }
                },
                |session| session.poison_terminal(),
            )
            .unwrap();
            assert_eq!(operation, Err("early validation"));
            assert_eq!(retake.is_err(), terminal);
            let cancelled = if count == 1 {
                cancel_persistent_compute_prepublication_entries_v1([&mut entries[0]])
            } else {
                let [a, b, c] = entries.as_mut_slice() else {
                    unreachable!()
                };
                cancel_persistent_compute_prepublication_entries_v1([a, b, c])
            };
            assert!(cancelled);
            assert_eq!(
                persistent_bind_retryable_v1(!session.terminal_poisoned, cancelled),
                !terminal
            );
            for (entry, identity) in entries.iter().zip(expected) {
                assert_eq!(entry.allocation.owner.live_use_count(), 0);
                assert_eq!(
                    entry
                        .allocation
                        .owner
                        .local_native_for_sdma()
                        .unwrap()
                        .storage_identity(),
                    identity
                );
            }
            assert!(session.dispatch.is_none());
        }
    }
}

#[test]
fn retained_control_replay_script_executes_the_production_pipeline_and_preserves_stage_custody() {
    let stages = [
        RetainedReplayInjectedStageV1::MappedFacts,
        RetainedReplayInjectedStageV1::Detach,
        RetainedReplayInjectedStageV1::AuthenticatedConstruction,
        RetainedReplayInjectedStageV1::Retain,
        RetainedReplayInjectedStageV1::FinalAudit,
    ];
    for (index, failed_stage) in stages.into_iter().enumerate() {
        let (outcome, trace) = run_retained_replay_script_v1(Some(failed_stage));
        assert_eq!(trace, stages[..=index]);
        match (failed_stage, outcome) {
            (
                RetainedReplayInjectedStageV1::MappedFacts | RetainedReplayInjectedStageV1::Detach,
                PersistentRetainedControlReplayPipelineOutcomeV1::BeforeDetach { request, error },
            ) => {
                assert_eq!(request.0, 0x35);
                assert_eq!(error, failed_stage);
            }
            (
                RetainedReplayInjectedStageV1::AuthenticatedConstruction,
                PersistentRetainedControlReplayPipelineOutcomeV1::Storage { storage, error },
            ) => {
                assert_eq!(storage.0, 0x35);
                assert_eq!(error, failed_stage);
            }
            (
                RetainedReplayInjectedStageV1::Retain,
                PersistentRetainedControlReplayPipelineOutcomeV1::Data { data, error },
            ) => {
                assert_eq!(data.0, 0x35);
                assert_eq!(error, failed_stage);
            }
            (
                RetainedReplayInjectedStageV1::FinalAudit,
                PersistentRetainedControlReplayPipelineOutcomeV1::Attached { attached, error },
            ) => {
                assert_eq!(attached.0, 0x35);
                assert_eq!(error, failed_stage);
            }
            _ => panic!("injected replay stage returned the wrong custody"),
        }
    }

    let (outcome, trace) = run_retained_replay_script_v1(None);
    assert_eq!(trace, stages);
    let PersistentRetainedControlReplayPipelineOutcomeV1::Ready(attached) = outcome else {
        panic!("clean replay pipeline must reach Ready")
    };
    assert_eq!(attached.0, 0x35);
}

#[test]
fn retained_control_replay_loan_resolution_distinguishes_open_retake_and_ready() {
    let unopened: PersistentRetainedControlReplayLoanResolutionV1<
        RetainedReplayScriptRequestV1,
        RetainedReplayScriptAttachedV1,
        &'static str,
    > = resolve_persistent_retained_control_replay_loan_v1(
        Some(RetainedReplayScriptRequestV1(0x41)),
        None,
        Err("loan open"),
        || "missing",
    );
    let PersistentRetainedControlReplayLoanResolutionV1::Unopened { request, error } = unopened
    else {
        panic!("unopened loan must retain input custody")
    };
    assert_eq!(request.0, 0x41);
    assert_eq!(error, "loan open");

    for loan in [Ok(()), Err("loan retake")] {
        let resolution: PersistentRetainedControlReplayLoanResolutionV1<
            RetainedReplayScriptRequestV1,
            RetainedReplayScriptAttachedV1,
            &'static str,
        > = resolve_persistent_retained_control_replay_loan_v1(
            None,
            Some(RetainedReplayScriptAttachedV1(0x42)),
            loan,
            || "missing",
        );
        let PersistentRetainedControlReplayLoanResolutionV1::Executed {
            outcome,
            retake_error,
        } = resolution
        else {
            panic!("executed loan must preserve its move-only outcome")
        };
        assert_eq!(outcome.0, 0x42);
        assert_eq!(retake_error, loan.err());
    }
}

#[test]
fn retained_control_replay_terminal_custody_variants_preserve_exact_native_stage() {
    let queue = test_queue_key(181, 1);
    let (mut storage_allocation, storage_prepared) =
        retained_replay_prepared_owner_fixture_v1(queue, 8181);
    let storage_identity = storage_allocation
        .owner
        .local_native_for_sdma()
        .unwrap()
        .storage_identity();
    let storage = storage_allocation
        .owner
        .detach_local_native_for_compute(&storage_prepared)
        .unwrap();
    let custody = PersistentComputeTerminalNativeCustodyV1::Storage(
        Gfx942SdmaBufferStorageV1::Device(storage),
    );
    assert_eq!(
        custody.stage(),
        Some(crate::Gfx942PersistentComputeTerminalStageV1::StorageDetached)
    );
    let PersistentComputeTerminalNativeCustodyV1::Storage(Gfx942SdmaBufferStorageV1::Device(
        storage,
    )) = custody
    else {
        unreachable!()
    };
    assert_eq!(storage.storage_identity(), storage_identity);

    let (mut data_allocation, data_prepared) =
        retained_replay_prepared_owner_fixture_v1(queue, 8282);
    let data_identity = data_allocation
        .owner
        .local_native_for_sdma()
        .unwrap()
        .storage_identity();
    let data = Gfx942FixedDispatchDataV1::uninitialized(
        data_allocation
            .owner
            .detach_local_native_for_compute(&data_prepared)
            .unwrap(),
    );
    let custody = PersistentComputeTerminalNativeCustodyV1::Data(
        PersistentComputeTerminalDataV1::from_one(data),
    );
    assert_eq!(
        custody.stage(),
        Some(crate::Gfx942PersistentComputeTerminalStageV1::DataDetached)
    );
    let PersistentComputeTerminalNativeCustodyV1::Data(data) = custody else {
        unreachable!()
    };
    assert_eq!(data.len(), 1);
    assert_eq!(
        data[0].sdma_storage_identity(),
        Gfx942SdmaBufferStorageIdentityV1::Device(data_identity)
    );

    assert_eq!(
        PersistentComputeTerminalNativeCustodyV1::Attached.stage(),
        Some(crate::Gfx942PersistentComputeTerminalStageV1::Attached)
    );
}

#[test]
fn retained_control_replay_cancellation_and_quarantine_preserve_prepared_authority() {
    let queue = test_queue_key(182, 1);
    let (mut exact, exact_prepared) = retained_replay_prepared_owner_fixture_v1(queue, 8383);
    let exact_sequence = exact_prepared.sequence();
    let (mut substituted, substituted_prepared) =
        retained_replay_prepared_owner_fixture_v1(queue, 8484);

    let cancellation = substituted
        .owner
        .cancel_prepared(exact_prepared)
        .expect_err("a substituted owner cannot cancel the exact replay use");
    let (_, exact_prepared) = cancellation.into_parts();
    assert_eq!(exact_prepared.sequence(), exact_sequence);
    let state = quarantine_persistent_retained_control_replay_prepared_v1(
        &mut substituted.owner,
        exact_prepared,
    );
    let PersistentComputeUseStateV1::Prepared(exact_prepared) = state else {
        panic!("failed quarantine must preserve Prepared authority")
    };
    assert_eq!(exact_prepared.sequence(), exact_sequence);
    assert_eq!(substituted.owner.quarantine_reason(), None);
    exact.owner.cancel_prepared(exact_prepared).unwrap();
    substituted
        .owner
        .cancel_prepared(substituted_prepared)
        .unwrap();

    let (mut exact, exact_prepared) = retained_replay_prepared_owner_fixture_v1(queue, 8585);
    let state =
        quarantine_persistent_retained_control_replay_prepared_v1(&mut exact.owner, exact_prepared);
    assert!(matches!(state, PersistentComputeUseStateV1::Quarantined));
    assert_eq!(
        exact.owner.quarantine_reason(),
        Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
    );
}

#[test]
fn retained_control_replay_public_input_failure_is_retryable_only_for_clean_round_trip() {
    for (id, retryable) in [(8686, true), (8787, false)] {
        let queue = test_queue_key(183, 1);
        let (mut allocation, prepared) = retained_replay_prepared_owner_fixture_v1(queue, id);
        let expected_identity = allocation.attachment.storage_identity;
        allocation.owner.cancel_prepared(prepared).unwrap();
        let failure = persistent_retained_control_replay_input_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("replay fault injection"),
            Gfx942PersistentComputeInputV1::Uninitialized(allocation),
            retryable,
        );
        let (_, custody) = failure.into_parts();
        let input = match (retryable, custody) {
            (true, Gfx942PersistentComputeBindFailureCustodyV1::Retryable(input)) => input,
            (false, Gfx942PersistentComputeBindFailureCustodyV1::ProcessTeardown(terminal)) => {
                assert!(terminal.retains_prebinding_input());
                terminal
                    .input
                    .expect("terminal input custody remains exact")
            }
            _ => panic!("public replay failure returned the wrong custody class"),
        };
        let (allocation, initialization) = input.into_parts();
        let _ = initialization.authenticated_sha256();
        let _ = initialization.is_fully_initialized();
        assert_eq!(allocation.attachment.storage_identity, expected_identity);
    }
}

#[test]
fn persistent_control_replay_uses_the_active_queue_currentness_policy() {
    let production = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let replay = production
        .split("fn bind_retained_persistent_fixed_dispatch_control_replay_v1")
        .nth(1)
        .unwrap()
        .split("pub fn bind_three_binding_directional_persistent_fixed_dispatch_v1")
        .next()
        .unwrap();
    assert_eq!(replay.matches("with_live_queue_memory_model").count(), 1);
    let mapped = replay.find("phases.mapped_facts(memory)").unwrap();
    let detach = replay.find("phases.detach()").unwrap();
    let construct = replay.find("phases.construct()").unwrap();
    let retain = replay.find("phases.retain(memory)").unwrap();
    let replay_validation = replay.find("phases.audit(memory)").unwrap();
    let phases = include_str!("../../queue_live/persistent_bind.rs");
    for operation in [
        "mapped_gfx942_device_memory_facts",
        "detach_local_native_for_compute",
        "from_authenticated_full_transfer",
        "retain_persistent_replay_data_in_place_v1",
        "validate_persistent_replay_dispatch_memory",
    ] {
        assert!(phases.contains(operation));
    }
    assert!(!phases.contains("validate_live_queue_dispatch_memory"));
    let loan_close = replay
        .find("resolve_persistent_retained_control_replay_loan_v1")
        .unwrap();
    let commit = replay
        .find("state: PersistentComputeUseStateV1::Prepared(replay.prepared)")
        .unwrap();
    assert!(mapped < detach);
    assert!(detach < construct);
    assert!(construct < retain);
    assert!(retain < replay_validation);
    assert!(replay_validation < loan_close);
    assert!(loan_close < commit);
    assert_eq!(
        replay
            .matches("restore_model_ownership_for_live_mutation")
            .count(),
        0
    );
    assert_eq!(
        replay
            .matches("retake_model_ownership_after_live_mutation")
            .count(),
        0
    );
    assert!(replay.contains("let mut request = Some(request)"));
    assert!(replay.contains("let mut phases = PersistentRetainedControlReplayCustodyV1::Empty"));
    assert!(replay.contains("let mut pipeline_result = None"));
    assert!(replay.find("let mut phases").unwrap() < replay.find("catch_unwind").unwrap());
    assert!(replay.contains("PersistentRetainedControlReplayOutcomeV1::Ready(replay)"));
    assert!(phases.contains("PersistentComputeTerminalNativeCustodyV1::Storage"));
    assert!(phases.contains("PersistentComputeTerminalNativeCustodyV1::Data"));
    assert!(replay.contains("PersistentComputeTerminalNativeCustodyV1::Attached"));
    assert!(
        replay.contains("predecessor_dispatch_generation: Some(commit.predecessor_generation)")
    );
    assert!(replay.contains("queue: self.key"));
    assert!(replay.contains("attachment_generation: commit.attachment_generation"));
    assert!(replay.contains("self.dispatch = Some(replay.dispatch)"));
    assert!(replay.contains("self.detached_data_count = 0"));
    assert!(replay.contains("self.detached_dispatch_generation = None"));
    assert!(replay.contains("self.detached_data_identities.clear()"));
    assert!(replay.contains("self.detached_next_insertion_index = None"));
    assert!(replay.contains("storage_identity: commit.storage_identity"));
    assert!(replay.contains("effect: commit.effect"));
    assert!(replay.contains("terminal_custody: None"));
    assert!(replay.contains("self.next_persistent_compute_generation ="));
    assert!(!replay.contains("prepare_persistent_fixed_dispatch_resources_v1"));

    let bind = production
        .split("pub fn bind_directional_persistent_fixed_dispatch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn submit_directional_persistent_fixed_dispatch_v1")
        .next()
        .unwrap();
    let retained = bind
        .find("if let Some(dispatch) = self.dispatch.take()")
        .unwrap();
    let replay_call = bind
        .find("bind_retained_persistent_fixed_dispatch_control_replay_v1")
        .unwrap();
    let initial_start = bind.find("validate_persistent_bind_inputs_v1(").unwrap();
    assert!(retained < replay_call);
    assert!(replay_call < initial_start);
    let initial = &bind[initial_start..];
    assert_eq!(
        initial
            .matches("with_live_queue_memory_model(|memory|")
            .count(),
        2
    );
    assert_eq!(
        initial
            .matches("with_live_queue_memory_model_custody")
            .count(),
        0
    );
    assert_eq!(
        initial
            .matches("restore_model_ownership_for_live_mutation")
            .count(),
        0
    );
    assert_eq!(
        initial
            .matches("retake_model_ownership_after_live_mutation")
            .count(),
        0
    );
    assert_eq!(
        initial
            .matches("Self::validate_persistent_bind_preparation_v1")
            .count(),
        1
    );
    assert!(initial.contains("prepare_persistent_fixed_dispatch_resources_v1"));
    assert!(!initial.contains("validate_persistent_replay_dispatch_memory"));
    assert!(!initial.contains("retain_persistent_replay_data_in_place_v1"));

    let release = production
        .split("pub fn release_retained_persistent_fixed_dispatch_control_v1")
        .last()
        .unwrap()
        .split("fn detach_recycled_fixed_dispatch_inner")
        .next()
        .unwrap();
    let sequencer = include_str!("../../queue_live/retained_control_release.rs");
    let close_audit = sequencer.find("context.check_currentness()?").unwrap();
    let consume_control = sequencer
        .find("original = context.dispatch().take()")
        .unwrap();
    assert!(close_audit < consume_control);
    assert!(sequencer.contains("catch_unwind(AssertUnwindSafe"));
    assert!(sequencer.contains("self.with_live_queue_memory_model_custody(operation)"));
    assert!(
        sequencer.find("let mut cleanup = None").unwrap()
            < sequencer.find("let result = catch_unwind").unwrap()
    );
    assert!(
        sequencer.find("retake?;").unwrap()
            < sequencer
                .find("ReturningControlCleanupCustodyV1::is_complete")
                .unwrap()
    );
    assert!(
        sequencer
            .find("*context.dispatch() = Some(dispatch)")
            .unwrap()
            < sequencer.find("context.poison(result.is_err())").unwrap()
    );
    assert!(
        sequencer.find("context.retain(root)").unwrap()
            < sequencer.find("context.poison(result.is_err())").unwrap()
    );
    assert!(release.contains("self.release_retained_control_settled_v1()"));
    assert!(
        release
            .find("self.retain_terminal_rebind_parent_v1")
            .unwrap()
            < release.find("settled.into_result()").unwrap()
    );

    let shared_memory = include_str!("../../shared_memory.rs");
    let operational = shared_memory
        .split("fn validate_persistent_replay_dispatch_memory")
        .nth(1)
        .unwrap()
        .split("fn map_device_memory")
        .next()
        .unwrap();
    assert!(operational.contains("self.check_operational_currentness()?"));
    assert!(operational.contains("validate_dispatch_device_memory_authorities"));
    assert!(!operational.contains("validate_complete_dispatch_device_memory_set"));
    assert!(!operational.contains("self.check_currentness()?"));

    let ordinary_rebind = production
        .split("pub fn bind_fixed_dispatch<const N: usize>")
        .last()
        .unwrap()
        .split("pub fn allocate_uninitialized_fixed_dispatch_data")
        .next()
        .unwrap();
    assert!(ordinary_rebind.contains("bind_fixed_dispatch_settled_v1"));
    assert!(!ordinary_rebind.contains("validate_persistent_replay_dispatch_memory"));
    let rebind = include_str!("../../queue_live/rebind.rs");
    assert!(rebind.contains("Self::validate_persistent_bind_preparation_v1"));
    assert!(!rebind.contains("validate_persistent_replay_dispatch_memory"));
    let validation = crate::queue::fixed_dispatch_production_source_for_tests_v1()
        .split("fn validate_persistent_bind_preparation_v1")
        .nth(1)
        .unwrap()
        .split("pub(super) const fn has_any_persistent_compute_attachment_v1")
        .next()
        .unwrap();
    assert!(validation.contains("preparation.completed()?"));
    assert!(validation.contains("validate_live_queue_dispatch_memory"));
    assert!(!validation.contains("validate_persistent_replay_dispatch_memory"));
}

#[test]
fn persistent_completion_recycle_driver_executes_required_paths() {
    let (pending, trace) = execute_completion_recycle_script_v1(CompletionRecycleScriptV1::Pending);
    assert!(matches!(
        pending,
        Ok(PersistentComputePollAndRecycleTransitionV1::Pending(73))
    ));
    assert_eq!(trace, ["check-a", "acquire", "check-b"]);

    let (ready, trace) = execute_completion_recycle_script_v1(CompletionRecycleScriptV1::Ready);
    assert!(matches!(
        ready,
        Ok(PersistentComputePollAndRecycleTransitionV1::Recycled {
            recycled: 73,
            completion_observed_at: 101,
        })
    ));
    assert_eq!(
        trace,
        [
            "check-a",
            "acquire",
            "check-b",
            "dispatch-completed",
            "allocation-completed",
            "midpoint",
            "reset",
            "check-c",
            "dispatch-recycled",
            "attachment-recycled",
        ]
    );

    for (script, expected_trace, expected_custody) in [
        (
            CompletionRecycleScriptV1::CompletionObservationFailure,
            &["check-a", "acquire", "observation-failure"][..],
            CompletionRecycleScriptCustodyV1::Published(73),
        ),
        (
            CompletionRecycleScriptV1::SignalResetFailure,
            &[
                "check-a",
                "acquire",
                "check-b",
                "dispatch-completed",
                "allocation-completed",
                "midpoint",
                "reset-failure",
            ][..],
            CompletionRecycleScriptCustodyV1::Completed(73),
        ),
        (
            CompletionRecycleScriptV1::ClosingCurrentnessFailure,
            &[
                "check-a",
                "acquire",
                "check-b",
                "dispatch-completed",
                "allocation-completed",
                "midpoint",
                "reset",
                "check-c-failure",
            ][..],
            CompletionRecycleScriptCustodyV1::Completed(73),
        ),
        (
            CompletionRecycleScriptV1::DispatchRecycleFailure,
            &[
                "check-a",
                "acquire",
                "check-b",
                "dispatch-completed",
                "allocation-completed",
                "midpoint",
                "reset",
                "check-c",
                "dispatch-recycle-failure",
            ][..],
            CompletionRecycleScriptCustodyV1::Recycled(73),
        ),
    ] {
        let (result, trace) = execute_completion_recycle_script_v1(script);
        let failure = match result {
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure))
                if script == CompletionRecycleScriptV1::CompletionObservationFailure =>
            {
                failure
            }
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => failure,
            _ => panic!("script returned the wrong transition"),
        };
        assert_eq!(failure.point, script);
        assert_eq!(failure.custody, expected_custody);
        assert_eq!(trace, expected_trace);
    }
}

#[test]
fn persistent_completion_recycle_driver_exhausts_failure_custody_matrix() {
    for (script, expected_custody, poll_failure) in [
        (
            CompletionRecycleScriptV1::PublishedStateFailure,
            CompletionRecycleScriptCustodyV1::Published(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::DispatchGenerationFailure,
            CompletionRecycleScriptCustodyV1::Published(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::CompletionObservationFailure,
            CompletionRecycleScriptCustodyV1::Published(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::DispatchCompletionFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::AllocationCompletionFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::SignalGenerationFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::SignalResetFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::ClosingCurrentnessFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::RecycleCurrentnessFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::RecycleInfrastructureFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::DispatchRecycleFailure,
            CompletionRecycleScriptCustodyV1::Recycled(73),
            false,
        ),
    ] {
        let (result, _) = execute_completion_recycle_script_v1(script);
        let (failure, actual_poll_failure) = match result {
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => {
                (failure, true)
            }
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                (failure, false)
            }
            Ok(_) => panic!("failure script unexpectedly succeeded"),
        };
        assert_eq!(failure.point, script);
        assert_eq!(failure.custody, expected_custody);
        assert_eq!(actual_poll_failure, poll_failure);
    }
}

#[test]
fn persistent_completion_wait_driver_counts_zero_deadline_late_and_retry_observations() {
    let (timeout, trace) = execute_completion_wait_recycle_scripts_v1(
        CompletionWaitPendingV1(73),
        [CompletionRecycleScriptV1::Pending],
        1,
    );
    let retry_pending = match timeout {
        Ok(PersistentComputeWaitAndRecycleTransitionV1::Timeout {
            pending,
            observations: 1,
        }) => pending,
        _ => panic!("first pending observation must return exact timeout custody"),
    };
    assert_eq!(trace, ["check-a", "acquire", "check-b"]);
    assert!(!trace.contains(&"midpoint"));
    assert!(!trace.contains(&"reset"));

    for (scripts, expected_observations) in [
        (vec![CompletionRecycleScriptV1::Ready], 1),
        (
            vec![
                CompletionRecycleScriptV1::Pending,
                CompletionRecycleScriptV1::Ready,
            ],
            2,
        ),
        (
            vec![
                CompletionRecycleScriptV1::Pending,
                CompletionRecycleScriptV1::Pending,
                CompletionRecycleScriptV1::Ready,
            ],
            3,
        ),
    ] {
        let (ready, trace) = execute_completion_wait_recycle_scripts_v1(
            CompletionWaitPendingV1(73),
            scripts,
            u64::MAX,
        );
        assert!(matches!(
            ready,
            Ok(PersistentComputeWaitAndRecycleTransitionV1::Recycled {
                recycled: 73,
                completion_observed_at: 101,
                observations,
            }) if observations == expected_observations
        ));
        assert_eq!(
            trace.iter().filter(|event| **event == "acquire").count(),
            expected_observations as usize
        );
        assert_eq!(trace.iter().filter(|event| **event == "reset").count(), 1);
    }

    let (retry, _) = execute_completion_wait_recycle_scripts_v1(
        retry_pending,
        [CompletionRecycleScriptV1::Ready],
        u64::MAX,
    );
    assert!(matches!(
        retry,
        Ok(PersistentComputeWaitAndRecycleTransitionV1::Recycled {
            recycled: 73,
            observations: 1,
            ..
        })
    ));
}

#[test]
fn persistent_completion_wait_driver_preserves_every_failure_route_and_custody() {
    for (script, expected_custody, poll_failure) in [
        (
            CompletionRecycleScriptV1::PublishedStateFailure,
            CompletionRecycleScriptCustodyV1::Published(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::DispatchGenerationFailure,
            CompletionRecycleScriptCustodyV1::Published(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::CompletionObservationFailure,
            CompletionRecycleScriptCustodyV1::Published(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::DispatchCompletionFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::AllocationCompletionFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            true,
        ),
        (
            CompletionRecycleScriptV1::SignalGenerationFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::SignalResetFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::ClosingCurrentnessFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::RecycleCurrentnessFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::RecycleInfrastructureFailure,
            CompletionRecycleScriptCustodyV1::Completed(73),
            false,
        ),
        (
            CompletionRecycleScriptV1::DispatchRecycleFailure,
            CompletionRecycleScriptCustodyV1::Recycled(73),
            false,
        ),
    ] {
        let (result, _) = execute_completion_wait_recycle_scripts_v1(
            CompletionWaitPendingV1(73),
            [script],
            u64::MAX,
        );
        let (failure, actual_poll_failure) = match result {
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Poll(failure)) => {
                (failure, true)
            }
            Err(PersistentComputePollAndRecycleTransitionFailureV1::Recycle(failure)) => {
                (failure, false)
            }
            Ok(_) => panic!("failure wait script unexpectedly succeeded"),
        };
        assert_eq!(failure.point, script);
        assert_eq!(failure.custody, expected_custody);
        assert_eq!(actual_poll_failure, poll_failure);
    }
}
