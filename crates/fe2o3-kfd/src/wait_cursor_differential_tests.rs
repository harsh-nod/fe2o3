use super::*;

// Exact pre-extraction next_action_at body from signed 0b3ecfea5.
#[rustfmt::skip]
fn original_next_action_at(cursor: &mut MonotonicWaitV1, now: Instant) -> WaitActionV1 {
    cursor.attempts = cursor.attempts.saturating_add(1);
    if cursor
        .active_spin_until
        .is_some_and(|active_spin_until| now < active_spin_until)
    {
        return WaitActionV1::Spin;
    }
    if cursor.attempts <= SPIN_ATTEMPTS_V1 {
        return WaitActionV1::Spin;
    }
    if cursor.attempts <= SPIN_ATTEMPTS_V1 + YIELD_ATTEMPTS_V1 {
        return WaitActionV1::Yield;
    }
    let sleep = cursor
        .deadline
        .map(|deadline| deadline.saturating_duration_since(now))
        .map_or(cursor.next_sleep, |remaining| remaining.min(cursor.next_sleep));
    cursor.next_sleep = cursor.next_sleep.saturating_mul(2).min(cursor.max_sleep);
    WaitActionV1::Sleep(sleep)
}

fn copy_cursor(cursor: &MonotonicWaitV1) -> MonotonicWaitV1 {
    MonotonicWaitV1 {
        deadline: cursor.deadline,
        active_spin_until: cursor.active_spin_until,
        attempts: cursor.attempts,
        next_sleep: cursor.next_sleep,
        max_sleep: cursor.max_sleep,
    }
}

fn compare_step(cursor: &mut MonotonicWaitV1, now: Instant) -> WaitActionV1 {
    let mut original = copy_cursor(cursor);
    let expected = original_next_action_at(&mut original, now);
    let actual = cursor.next_action_at(now);
    assert_eq!(actual, expected);
    assert_eq!(cursor.attempts, original.attempts);
    assert_eq!(cursor.next_sleep, original.next_sleep);
    assert_eq!(cursor.deadline, original.deadline);
    assert_eq!(cursor.active_spin_until, original.active_spin_until);
    assert_eq!(cursor.max_sleep, original.max_sleep);
    actual
}

#[test]
fn cursor_extraction_matches_original_across_full_duration_boundaries() {
    let now = Instant::now();
    let before = now.checked_sub(Duration::from_nanos(1)).unwrap();
    let after = now.checked_add(Duration::from_nanos(1)).unwrap();
    let deadlines = [
        None,
        Some(before),
        Some(now),
        Some(after),
        Some(now.checked_add(Duration::from_micros(7)).unwrap()),
        Some(now.checked_add(Duration::from_secs(1)).unwrap()),
    ];
    let durations = [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_micros(25),
        Duration::from_millis(1),
        Duration::new(0, 999_999_999),
        Duration::new(1, 500_000_001),
        Duration::new(u64::MAX / 2, 999_999_999),
        Duration::new(u64::MAX / 2 + 1, 0),
        Duration::new(u64::MAX, 0),
        Duration::MAX,
    ];
    let mut cases = 0;
    for attempts in [0, 63, 64, 79, 80, u32::MAX - 1, u32::MAX] {
        for active_spin_until in [None, Some(before), Some(now), Some(after)] {
            for deadline in deadlines {
                for next_sleep in durations {
                    for max_sleep in durations {
                        let mut cursor = MonotonicWaitV1 {
                            deadline,
                            active_spin_until,
                            attempts,
                            next_sleep,
                            max_sleep,
                        };
                        compare_step(&mut cursor, now);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 16_800);
}

#[test]
fn cursor_extraction_keeps_the_complete_prefix_and_backoff_trace() {
    let now = Instant::now();
    let deadline = now.checked_add(Duration::from_secs(1)).unwrap();
    for ceiling in [
        Duration::ZERO,
        INITIAL_SLEEP_V1,
        MAX_SLEEP_V1,
        Duration::MAX,
    ] {
        let mut cursor = MonotonicWaitV1::until_with_sleep_ceiling(deadline, ceiling);
        for step in 1..=256 {
            let action = compare_step(&mut cursor, now);
            match step {
                1..=64 => assert_eq!(action, WaitActionV1::Spin),
                65..=80 => assert_eq!(action, WaitActionV1::Yield),
                _ => assert!(matches!(action, WaitActionV1::Sleep(_))),
            }
        }
    }
}

#[test]
fn zero_clipped_sleep_still_advances_backoff_but_active_spin_does_not() {
    let now = Instant::now();
    let mut cursor = MonotonicWaitV1 {
        deadline: Some(now),
        active_spin_until: Some(now.checked_add(Duration::from_nanos(1)).unwrap()),
        attempts: u32::MAX,
        next_sleep: INITIAL_SLEEP_V1,
        max_sleep: MAX_SLEEP_V1,
    };
    assert_eq!(compare_step(&mut cursor, now), WaitActionV1::Spin);
    assert_eq!(cursor.next_sleep, INITIAL_SLEEP_V1);
    cursor.active_spin_until = Some(now);
    assert_eq!(
        compare_step(&mut cursor, now),
        WaitActionV1::Sleep(Duration::ZERO)
    );
    assert_eq!(cursor.next_sleep, INITIAL_SLEEP_V1 * 2);
    assert_eq!(cursor.attempts, u32::MAX);
}

#[test]
fn zero_and_full_range_ceilings_keep_constructor_and_observation_order() {
    let now = Instant::now();
    for ceiling in [Duration::ZERO, Duration::MAX] {
        let cursor = MonotonicWaitV1::until_with_active_spin_floor_and_sleep_ceiling_from(
            now,
            now,
            Duration::MAX,
            ceiling,
        );
        assert_eq!(cursor.active_spin_until, Some(now));
        assert_eq!(cursor.next_sleep, INITIAL_SLEEP_V1.min(ceiling));
        for ready in [false, true] {
            let mut cursor = copy_cursor(&cursor);
            let mut observed = 0;
            assert_eq!(
                cursor.observe_until_ready(|| {
                    observed += 1;
                    Ok::<_, ()>(ready)
                }),
                Ok(ready)
            );
            assert_eq!(observed, 1);
            assert_eq!(cursor.attempts, 0);
            assert_eq!(cursor.next_sleep, INITIAL_SLEEP_V1.min(ceiling));
        }
    }
}

#[test]
fn std_duration_contract_examples_use_lossless_full_range_nanoseconds() {
    let max_ns = u128::from(u64::MAX) * 1_000_000_000 + 999_999_999;
    assert_eq!(Duration::MAX.as_nanos(), max_ns);
    for duration in [
        Duration::ZERO,
        Duration::from_nanos(1),
        INITIAL_SLEEP_V1,
        MAX_SLEEP_V1,
        Duration::new(0, 999_999_999),
        Duration::new(u64::MAX / 2, 999_999_999),
        Duration::new(u64::MAX / 2 + 1, 0),
        Duration::MAX,
    ] {
        let ns =
            u128::from(duration.as_secs()) * 1_000_000_000 + u128::from(duration.subsec_nanos());
        assert_eq!(duration.as_nanos(), ns);
        assert_eq!(duration.saturating_mul(2).as_nanos(), (ns * 2).min(max_ns));
        for ceiling in [
            Duration::ZERO,
            INITIAL_SLEEP_V1,
            MAX_SLEEP_V1,
            Duration::MAX,
        ] {
            assert_eq!(duration.min(ceiling).as_nanos(), ns.min(ceiling.as_nanos()));
        }
    }
}
