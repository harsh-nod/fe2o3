use super::*;

#[test]
fn pending_completed_bounds_include_load_and_scheduler_uncertainty() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Pending);
    value.post_read(20, 80, 40, false);
    value.pause(80, 50_080);
    value.read(50_090, 50_100, SignalState::Completed);
    value.post_read(50_100, 50_120, 0, true);
    value.retirement(50_120, 50_140);
    assert!(value.valid);
    assert_eq!(value.completion_bracket_ns(), Some([10, 50_100]));
    assert_eq!(value.max_inter_read_ns, 50_090);
    assert_eq!(value.read_ns, 20);
    assert_eq!(value.pause_ns, 50_000);
    assert_eq!(value.post_read_ns, 80);
    assert_eq!(value.currentness_ns, 40);
    assert_eq!(value.retirement_ns, 20);
    assert_eq!(value.ready_ns, Some(50_120));
}

#[test]
fn first_read_complete_has_no_pending_lower_bound() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Completed);
    assert_eq!(value.completion_bracket_ns(), Some([0, 20]));
    assert_eq!(value.last_pending, None);
    assert_eq!(value.max_inter_read_ns, 0);
}

#[test]
fn signal_completion_is_not_ring_readiness_or_retirement() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Pending);
    value.read(30, 40, SignalState::Completed);
    value.post_read(40, 50, 0, false);
    value.pause(50, 100);
    value.read(110, 120, SignalState::Completed);
    value.post_read(120, 130, 0, true);
    assert_eq!(value.completion_bracket_ns(), Some([10, 40]));
    assert_eq!(value.ready_ns, Some(130));
    assert_eq!(value.completed_reads, 2);
    assert_eq!(value.retirement_ns, 0);
}

#[test]
fn no_completed_read_has_no_completion_bracket() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Pending);
    assert_eq!(value.completion_bracket_ns(), None);
}

#[test]
fn pending_after_completion_invalidates_without_replacing_frozen_bounds() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Pending);
    value.read(30, 40, SignalState::Completed);
    value.read(50, 60, SignalState::Pending);
    assert!(!value.valid);
    assert_eq!(value.last_pending.unwrap().before_ns, 10);
    assert_eq!(value.completion_bracket_ns(), None);
}

#[test]
fn unexpected_signal_is_not_success() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Unexpected);
    assert_eq!(value.unexpected_reads, 1);
    assert_eq!(value.first_completed, None);
    assert!(!value.valid);
}

#[test]
fn reversed_and_overlapping_intervals_are_invalid() {
    for (before, after) in [(19, 30), (30, 29)] {
        let mut value = Observation::default();
        value.read(10, 20, SignalState::Pending);
        value.pause(before, after);
        assert!(!value.valid);
        assert_eq!(value.pauses, 0);
    }
}

#[test]
fn currentness_is_nested_in_post_read_not_additive() {
    let mut value = Observation::default();
    value.read(10, 20, SignalState::Pending);
    value.post_read(20, 30, 11, false);
    assert!(!value.valid);
}

#[test]
fn ready_without_signal_and_retirement_without_ready_are_invalid() {
    let mut value = Observation::default();
    value.post_read(10, 20, 0, true);
    assert!(!value.valid);
    let mut value = Observation::default();
    value.retirement(10, 20);
    assert!(!value.valid);
}

#[test]
fn overflow_marks_invalid_without_panicking_or_wrapping() {
    let mut value = Observation::default();
    value.reads = u64::MAX;
    value.read(10, 20, SignalState::Pending);
    assert!(!value.valid);
    assert_eq!(value.reads, u64::MAX);
}

#[test]
fn pauses_measure_actual_elapsed_not_requested_duration() {
    let mut value = Observation::default();
    value.pause(10, 250_010);
    value.pause(250_020, 300_020);
    assert_eq!(value.pauses, 2);
    assert_eq!(value.pause_ns, 300_000);
    assert_eq!(value.max_pause_ns, 250_000);
}
