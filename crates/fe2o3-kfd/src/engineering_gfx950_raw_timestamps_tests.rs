use super::*;
use std::time::Instant;

fn pending() -> PendingDispatch {
    let now = Instant::now();
    PendingDispatch {
        unique_id: 17,
        queue_epoch: 3,
        next: 12,
        started: now,
        deadline: now,
        next_currentness: now,
        wait_started: None,
        profiled: false,
        completed: true,
        raw_timestamps: Some([0x1234_5678_9abc_def0, 0x1234_5678_9abc_def9]),
    }
}

fn observation(rank: usize, host: u64) -> Gfx950EngineeringRawTimestampObservationV1 {
    completed_observation(5, rank, [17, 3, 12], &mut pending(), host).unwrap()
}

#[test]
fn timestamp_mode_is_explicit_and_cannot_fall_back() {
    assert!(require_capture_mode(false, false).is_ok());
    assert!(require_capture_mode(true, true).is_ok());
    assert!(require_capture_mode(false, true).is_err());
    assert!(require_capture_mode(true, false).is_err());
}

#[test]
fn timestamps_reject_unwritten_reversed_and_wrap_without_inventing_duration() {
    for ticks in [[0, 0], [0, 1], [1, 0], [2, 1], [u64::MAX, 1]] {
        assert!(require_ticks(ticks).is_err(), "{ticks:?}");
    }
    for ticks in [[1, 1], [1, u64::MAX], [u64::MAX, u64::MAX]] {
        assert!(require_ticks(ticks).is_ok(), "{ticks:?}");
    }
}

#[test]
fn timestamps_preserve_full_width_raw_values_and_exact_identity() {
    let value = observation(1, 99);
    assert_eq!(value.group_incarnation(), 5);
    assert_eq!(value.rank(), 1);
    assert_eq!(value.unique_id(), 17);
    assert_eq!(value.queue_epoch(), 3);
    assert_eq!(value.packet_id(), 11);
    assert_eq!(value.signal_generation(), 12);
    assert_eq!(value.start_tick(), 0x1234_5678_9abc_def0);
    assert_eq!(value.end_tick(), 0x1234_5678_9abc_def9);
    assert_eq!(value.host_elapsed_ns(), 99);
}

#[test]
fn timestamps_require_completed_exact_queue_epoch_uid_and_generation() {
    for retained in [[18, 3, 12], [17, 4, 12], [17, 3, 13]] {
        assert!(completed_observation(5, 1, retained, &mut pending(), 1).is_err());
    }
    for (incarnation, rank) in [(0, 1), (5, 8)] {
        assert!(completed_observation(incarnation, rank, [17, 3, 12], &mut pending(), 1).is_err());
    }
    let mut value = pending();
    value.completed = false;
    assert!(completed_observation(5, 1, [17, 3, 12], &mut value, 1).is_err());
    value.completed = true;
    value.next = 0;
    assert!(completed_observation(5, 1, [17, 3, 0], &mut value, 1).is_err());
}

#[test]
fn timestamps_are_consumed_once_and_missing_or_bad_writes_fail() {
    let mut value = pending();
    completed_observation(5, 1, [17, 3, 12], &mut value, 1).unwrap();
    assert!(completed_observation(5, 1, [17, 3, 12], &mut value, 1).is_err());
    for ticks in [None, Some([0, 0]), Some([3, 2])] {
        value.raw_timestamps = ticks;
        assert!(completed_observation(5, 1, [17, 3, 12], &mut value, 1).is_err());
    }
}

#[test]
fn timestamps_queue_rollover_is_a_distinct_observation_not_global_time() {
    let old = observation(0, 8);
    let mut value = pending();
    value.queue_epoch = 4;
    value.next = 1;
    value.raw_timestamps = Some([1, 1]);
    assert!(completed_observation(5, 0, [17, 3, 1], &mut value, 8).is_err());
    let new = completed_observation(5, 0, [17, 4, 1], &mut value, 8).unwrap();
    assert_ne!(old.queue_epoch(), new.queue_epoch());
    assert_eq!(new.packet_id(), 0);
    assert_eq!(new.signal_generation(), 1);
}

#[test]
fn timestamp_round_joins_completion_order_to_input_order() {
    let mut slots = [None; 8];
    slots[7] = Some(observation(7, 70));
    slots[0] = Some(observation(0, 10));
    let values = finish_round(true, &[7, 0], &[70, 10], slots).unwrap();
    assert_eq!(values.iter().map(|v| v.rank()).collect::<Vec<_>>(), [7, 0]);
    assert!(
        finish_round(false, &[0, 1], &[10, 20], [None; 8])
            .unwrap()
            .is_empty()
    );
}

#[test]
fn timestamp_round_refuses_missing_extra_duplicate_or_mismatched_observations() {
    let mut slots = [None; 8];
    slots[0] = Some(observation(0, 10));
    slots[1] = Some(observation(1, 20));
    for (ranks, elapsed) in [
        (&[0][..], &[10][..]),
        (&[0, 0], &[10, 10]),
        (&[0, 2], &[10, 20]),
        (&[0, 1], &[10, 21]),
        (&[0, 8], &[10, 20]),
        (&[], &[]),
        (&[0, 1], &[10]),
    ] {
        assert!(finish_round(true, ranks, elapsed, slots).is_err());
    }
    assert!(finish_round(false, &[0, 1], &[10, 20], slots).is_err());
    slots[0] = Some(observation(1, 10));
    assert!(finish_round(true, &[0, 1], &[10, 20], slots).is_err());
}
