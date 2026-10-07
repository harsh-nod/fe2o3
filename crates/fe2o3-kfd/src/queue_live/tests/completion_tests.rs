use super::*;

#[test]
fn persistent_completion_wait_route_reuses_fused_poll_without_split_recycle() {
    let fixed = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let production = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let wait = fixed
        .split("pub fn wait_and_recycle_directional_persistent_fixed_dispatch_until_v1")
        .nth(1)
        .unwrap()
        .split(
            "pub fn wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1",
        )
        .next()
        .unwrap();
    assert_eq!(
        wait.matches("poll_and_recycle_directional_persistent_fixed_dispatch_v1(dispatch)")
            .count(),
        1
    );
    assert_eq!(
        wait.matches("execute_persistent_compute_wait_and_recycle_v1")
            .count(),
        1
    );
    assert!(!wait.contains("poll_directional_persistent_fixed_dispatch_v1"));
    assert!(!wait.contains("session.recycle_directional_persistent_fixed_dispatch_v1("));

    let driver = production
        .split("fn execute_persistent_compute_wait_and_recycle_v1")
        .nth(1)
        .unwrap()
        .split("fn resolve_persistent_retained_control_replay_loan_v1")
        .next()
        .unwrap();
    assert!(driver.contains("observations.saturating_add(1)"));
    assert!(driver.contains("observations == u64::MAX"));
}

#[test]
fn persistent_completion_recycle_fused_route_has_one_ordered_handoff() {
    let fixed = crate::queue::fixed_dispatch_production_source_for_tests_v1();
    let production = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let all_production = format!("{production}\n{fixed}");
    let fused = fixed
        .split("pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn recycle_directional_persistent_fixed_dispatch_v1")
        .next()
        .unwrap();
    assert_eq!(
        fused
            .matches("poll_completion_batch_with_current_handoff_retaining")
            .count(),
        1
    );
    assert_eq!(
        fused
            .matches("finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
            .count(),
        1
    );
    assert_eq!(
        fused
            .matches("Self::recycle_completion_current_handoff_retaining")
            .count(),
        1
    );
    assert_eq!(
        fused
            .matches("execute_persistent_compute_poll_and_recycle_v1")
            .count(),
        1
    );
    assert!(!fused.contains("recycle_completion_batch_retaining"));
    assert!(!fused.contains("Vec<"));
    assert!(!fused.contains("Box<"));
    assert!(!fused.contains("dyn "));

    let shared_poll = fixed
        .split("fn poll_directional_persistent_fixed_dispatch_inner_v1")
        .nth(1)
        .unwrap()
        .split("/// Polls and immediately recycles the exact three-binding dispatch.")
        .next()
        .unwrap();
    let preflight = shared_poll.find("let generation_is_current").unwrap();
    let observe = shared_poll.find("let observed =").unwrap();
    assert!(
        shared_poll[observe..].starts_with("let observed =\n            std::panic::catch_unwind")
    );
    let dispatch_completed = shared_poll.find(".mark_completed_occurrence(").unwrap();
    let allocation_completed = shared_poll
        .find("complete_persistent_compute_entries_v1([&mut attachment])")
        .unwrap();
    assert!(preflight < observe);
    assert!(observe < dispatch_completed);
    assert!(dispatch_completed < allocation_completed);

    let driver = production
        .split("fn execute_persistent_compute_poll_and_recycle_v1")
        .nth(1)
        .unwrap()
        .split("fn resolve_persistent_retained_control_replay_loan_v1")
        .next()
        .unwrap();
    let driver_poll = driver.find("let completed").unwrap();
    let midpoint = driver.find("let completion_observed_at").unwrap();
    let driver_recycle = driver.find("let recycled = recycle").unwrap();
    assert!(driver_poll < midpoint);
    assert!(midpoint < driver_recycle);

    let shared_recycle = fixed
        .split("fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
        .nth(1)
        .unwrap()
        .split("pub fn poll_and_recycle_directional_persistent_fixed_dispatch_v1")
        .next()
        .unwrap();
    let native_recycle = shared_recycle.find("let recycled =").unwrap();
    assert!(
        shared_recycle[native_recycle..]
            .starts_with("let recycled =\n            std::panic::catch_unwind")
    );
    let dispatch_recycled = shared_recycle.find(".mark_recycled_occurrence(").unwrap();
    let attachment_recycled = shared_recycle
        .find("recycle_persistent_compute_entries_v1([&mut attachment])")
        .unwrap();
    assert!(native_recycle < dispatch_recycled);
    assert!(dispatch_recycled < attachment_recycled);

    assert!(!all_production.contains("PersistentCompletionRecycleFailurePointV1"));
    assert!(!all_production.contains("persistent_completion_recycle_failure_stage_v1"));
    assert!(!all_production.contains("persistent_completion_recycle_terminal_custody_v1"));
    assert_eq!(
        shared_poll
            .matches("PersistentComputeTerminalNativeCustodyV1::Published")
            .count(),
        3
    );
    assert_eq!(
        shared_poll
            .matches("PersistentComputeTerminalNativeCustodyV1::Completed")
            .count(),
        2
    );
    assert_eq!(
        shared_recycle
            .matches("PersistentComputeTerminalNativeCustodyV1::Completed")
            .count(),
        1
    );
    assert_eq!(
        shared_recycle
            .matches("PersistentComputeTerminalNativeCustodyV1::Recycled")
            .count(),
        2
    );

    let split_poll = fixed
        .split("pub fn poll_directional_persistent_fixed_dispatch_v1")
        .nth(1)
        .unwrap()
        .split("fn finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
        .next()
        .unwrap();
    assert_eq!(
        split_poll
            .matches("poll_directional_persistent_fixed_dispatch_inner_v1")
            .count(),
        1
    );
    let split_recycle = fixed
        .split("pub fn recycle_directional_persistent_fixed_dispatch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn detach_recycled_directional_persistent_fixed_dispatch_v1")
        .next()
        .unwrap();
    assert_eq!(
        split_recycle
            .matches("finish_directional_persistent_fixed_dispatch_recycle_inner_v1")
            .count(),
        1
    );
}

#[test]
fn retained_control_replay_failure_matrix_requires_an_exact_clean_round_trip() {
    for loan_succeeded in [false, true] {
        for cancellation_succeeded in [false, true] {
            for session_healthy in [false, true] {
                let observed = classify_persistent_retained_control_replay_failure_v1(
                    PersistentRetainedControlReplayCustodyStageV1::Input,
                    loan_succeeded,
                    cancellation_succeeded,
                    session_healthy,
                );
                let expected = if loan_succeeded && cancellation_succeeded && session_healthy {
                    PersistentRetainedControlReplayDispositionV1::RetryableInput
                } else if cancellation_succeeded {
                    PersistentRetainedControlReplayDispositionV1::TerminalInput
                } else {
                    PersistentRetainedControlReplayDispositionV1::TerminalAttached
                };
                assert_eq!(observed, expected);
            }
        }
    }

    for (stage, expected) in [
        (
            PersistentRetainedControlReplayCustodyStageV1::Storage,
            PersistentRetainedControlReplayDispositionV1::TerminalStorage,
        ),
        (
            PersistentRetainedControlReplayCustodyStageV1::Data,
            PersistentRetainedControlReplayDispositionV1::TerminalData,
        ),
        (
            PersistentRetainedControlReplayCustodyStageV1::Attached,
            PersistentRetainedControlReplayDispositionV1::TerminalAttached,
        ),
    ] {
        for loan_succeeded in [false, true] {
            for cancellation_succeeded in [false, true] {
                for session_healthy in [false, true] {
                    assert_eq!(
                        classify_persistent_retained_control_replay_failure_v1(
                            stage,
                            loan_succeeded,
                            cancellation_succeeded,
                            session_healthy,
                        ),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn retained_control_replay_authority_carriers_are_move_only_and_private() {
    let source = crate::queue::live_production_source_for_tests_v1();
    for carrier in [
        "PersistentRetainedControlReplayRequestV1",
        "PersistentRetainedControlReplayDetachedV1",
        "PersistentRetainedControlReplayOutcomeV1",
    ] {
        assert!(
            source.contains(&format!("struct {carrier}"))
                || source.contains(&format!("enum {carrier}"))
        );
        assert!(!source.contains(&format!("pub struct {carrier}")));
        assert!(!source.contains(&format!("pub enum {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone)]\nstruct {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone)]\nenum {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone, Copy)]\nstruct {carrier}")));
        assert!(!source.contains(&format!("#[derive(Clone, Copy)]\nenum {carrier}")));
    }
}
