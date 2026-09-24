use super::*;

#[test]
fn active_policy_rejects_timestamp_wrappers_but_default_still_admits_them() {
    assert!(
        Ordered64WaitPolicy::ActivePoll10msV1
            .require_timestamp_compatibility()
            .is_err()
    );
    assert!(
        Ordered64WaitPolicy::default()
            .require_timestamp_compatibility()
            .is_ok()
    );
}

#[test]
fn diagnostic_policy_requires_profile_only_for_ordinary_ordered64() {
    let active = Ordered64WaitPolicy::ActivePoll10msV1;
    assert!(active.require_profile(true, false).is_err());
    assert!(active.require_profile(true, true).is_ok());
    assert!(active.require_profile(false, false).is_ok());
    assert!(
        Ordered64WaitPolicy::default()
            .require_profile(true, false)
            .is_ok()
    );
}

#[test]
fn default_policy_never_creates_an_active_window() {
    let expired = Instant::now();
    for ordered64 in [false, true] {
        assert!(
            Ordered64WaitPolicy::default()
                .start(ordered64, expired)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn explicit_policy_only_applies_to_ordered64() {
    let deadline = Instant::now() + Duration::from_secs(60);
    assert!(
        Ordered64WaitPolicy::ActivePoll10msV1
            .start(false, deadline)
            .unwrap()
            .is_none()
    );
    assert!(
        Ordered64WaitPolicy::ActivePoll10msV1
            .start(true, deadline)
            .unwrap()
            .is_some()
    );
}

#[test]
fn spins_stop_exactly_at_the_fixed_ten_millisecond_boundary() {
    let now = Instant::now();
    let mut wait = ActivePollWait::new(now, now + Duration::from_secs(1)).unwrap();
    assert_eq!(wait.next_action(now).unwrap(), PauseAction::Spin);
    assert_eq!(
        wait.next_action(now + ACTIVE_WINDOW - Duration::from_nanos(1))
            .unwrap(),
        PauseAction::Spin
    );
    assert_eq!(
        wait.next_action(now + ACTIVE_WINDOW).unwrap(),
        PauseAction::Sleep(FALLBACK_SLEEP)
    );
    assert_eq!((wait.spin_pauses, wait.fallback_sleeps), (2, 1));
}

#[test]
fn repeated_pending_polls_cannot_extend_the_active_window() {
    let now = Instant::now();
    let mut wait = ActivePollWait::new(now, now + Duration::from_secs(1)).unwrap();
    for millis in [1, 5, 9] {
        assert_eq!(
            wait.next_action(now + Duration::from_millis(millis))
                .unwrap(),
            PauseAction::Spin
        );
    }
    for millis in [10, 11, 100] {
        assert_eq!(
            wait.next_action(now + Duration::from_millis(millis))
                .unwrap(),
            PauseAction::Sleep(FALLBACK_SLEEP)
        );
    }
    assert_eq!(wait.active_until, now + ACTIVE_WINDOW);
}

#[test]
fn a_short_original_deadline_caps_the_active_window() {
    let now = Instant::now();
    let deadline = now + Duration::from_micros(100);
    let mut wait = ActivePollWait::new(now, deadline).unwrap();
    assert_eq!(wait.active_until, deadline);
    assert!(wait.next_action(deadline).is_err());
    assert_eq!((wait.spin_pauses, wait.fallback_sleeps), (0, 0));
}

#[test]
fn fallback_sleep_is_clipped_to_remaining_original_deadline() {
    let now = Instant::now();
    let tail = Duration::from_micros(7);
    let mut wait = ActivePollWait::new(now, now + ACTIVE_WINDOW + tail).unwrap();
    assert_eq!(
        wait.next_action(now + ACTIVE_WINDOW).unwrap(),
        PauseAction::Sleep(tail)
    );
}

#[test]
fn expired_start_and_pause_fail_without_counting_an_action() {
    let now = Instant::now();
    assert!(ActivePollWait::new(now, now).is_err());
    let deadline = now + Duration::from_secs(1);
    let mut wait = ActivePollWait::new(now, deadline).unwrap();
    assert!(
        wait.next_action(deadline + Duration::from_nanos(1))
            .is_err()
    );
    assert_eq!((wait.spin_pauses, wait.fallback_sleeps), (0, 0));
}

#[test]
fn successful_batch_counters_separate_fallback_from_no_fallback() {
    let now = Instant::now();
    let mut counters = ActivePollCounters::default();
    let immediate = ActivePollWait::new(now, now + Duration::from_secs(1)).unwrap();
    counters.record_completed(&immediate).unwrap();
    let mut fallback = ActivePollWait::new(now, now + Duration::from_secs(1)).unwrap();
    fallback.next_action(now).unwrap();
    fallback.next_action(now + ACTIVE_WINDOW).unwrap();
    counters.record_completed(&fallback).unwrap();
    assert_eq!(counters.completed_batches, 2);
    assert_eq!(counters.completed_without_fallback, 1);
    assert_eq!(counters.completed_after_fallback, 1);
    assert_eq!((counters.spin_pauses, counters.fallback_sleeps), (1, 1));
}

#[test]
fn counter_overflow_does_not_partially_commit_a_success() {
    let now = Instant::now();
    let mut wait = ActivePollWait::new(now, now + Duration::from_secs(1)).unwrap();
    wait.next_action(now).unwrap();
    let mut counters = ActivePollCounters {
        spin_pauses: u64::MAX,
        ..ActivePollCounters::default()
    };
    let before = counters;
    assert!(counters.record_completed(&wait).is_err());
    assert_eq!(counters, before);
}

#[test]
fn legacy_report_is_empty_and_explicit_report_has_exact_schema() {
    let mut legacy = Vec::new();
    Ordered64WaitPolicy::default()
        .write_terminal_report(7, ActivePollCounters::default(), &mut legacy)
        .unwrap();
    assert!(legacy.is_empty());
    let mut active = Vec::new();
    Ordered64WaitPolicy::ActivePoll10msV1
        .write_terminal_report(7, ActivePollCounters::default(), &mut active)
        .unwrap();
    assert_eq!(active.iter().filter(|byte| **byte == b'\n').count(), 1);
    let value: serde_json::Value = serde_json::from_slice(&active).unwrap();
    assert_eq!(value["schema"], "Fe2o3Ordered64WaitPolicyDiagnosticV1");
    assert_eq!(value["policy"], "ActivePoll10msV1");
    assert_eq!(value["worker_pid"], std::process::id());
    assert_eq!(value["device_unique_id"], 7);
    assert_eq!(value["active_window_ns"], 10_000_000_u64);
    assert_eq!(value["fallback_sleep_ns"], 50_000_u64);
    assert_eq!(value["closed_cleanly"], true);
    assert_eq!(value["counters"].as_object().unwrap().len(), 5);
}

#[test]
fn terminal_report_write_failure_is_not_success() {
    struct Fail;
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected report failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(
        Ordered64WaitPolicy::ActivePoll10msV1
            .write_terminal_report(7, ActivePollCounters::default(), Fail)
            .is_err()
    );
}

#[test]
fn pause_counter_overflow_keeps_the_original_window_and_totals() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(1);
    let mut wait = ActivePollWait::new(now, deadline).unwrap();
    wait.spin_pauses = u64::MAX;
    assert!(wait.next_action(now).is_err());
    assert_eq!(wait.spin_pauses, u64::MAX);
    assert_eq!(wait.fallback_sleeps, 0);
    wait.fallback_sleeps = u64::MAX;
    assert!(wait.next_action(now + ACTIVE_WINDOW).is_err());
    assert_eq!(wait.fallback_sleeps, u64::MAX);
    assert_eq!(wait.active_until, now + ACTIVE_WINDOW);
    assert_eq!(wait.deadline, deadline);
}

#[test]
fn maximum_terminal_record_is_bounded_and_flush_failure_is_an_error() {
    let counters = ActivePollCounters {
        completed_batches: u64::MAX,
        spin_pauses: u64::MAX,
        fallback_sleeps: 0,
        completed_without_fallback: u64::MAX,
        completed_after_fallback: 0,
    };
    let mut record = Vec::new();
    Ordered64WaitPolicy::ActivePoll10msV1
        .write_terminal_report(u64::MAX, counters, &mut record)
        .unwrap();
    assert!(record.len() <= 1024);
    assert_eq!(record.iter().filter(|byte| **byte == b'\n').count(), 1);

    struct FlushFailure(Vec<u8>);
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("injected terminal flush failure"))
        }
    }
    let mut output = FlushFailure(Vec::new());
    assert!(
        Ordered64WaitPolicy::ActivePoll10msV1
            .write_terminal_report(u64::MAX, counters, &mut output)
            .is_err()
    );
    assert_eq!(output.0, record);
}
