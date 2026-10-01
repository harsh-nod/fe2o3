use super::*;

fn duration_boundaries() -> impl Iterator<Item = Duration> {
    [
        0,
        1,
        2,
        u32::MAX as u64,
        u64::MAX / 2,
        u64::MAX / 2 + 1,
        u64::MAX - 1,
        u64::MAX,
    ]
    .into_iter()
    .flat_map(|seconds| {
        [
            0,
            1,
            499_999_999,
            500_000_000,
            500_000_001,
            999_999_998,
            999_999_999,
        ]
        .into_iter()
        .map(move |nanos| Duration::new(seconds, nanos))
    })
}

#[test]
fn canonical_duration_components_round_trip_full_range() {
    assert_eq!(WAIT_NANOS_PER_SECOND_V1, 1_000_000_000);
    let mut count = 0;
    for duration in duration_boundaries() {
        assert!(duration.subsec_nanos() < WAIT_NANOS_PER_SECOND_V1);
        assert_eq!(
            Duration::new(duration.as_secs(), duration.subsec_nanos()),
            duration
        );
        count += 1;
    }
    assert_eq!(count, 56);
}

#[test]
fn arithmetic_attempt_and_prefix_boundaries_match_native_policy() {
    for attempts in (0..=81).chain([u32::MAX - 1, u32::MAX]) {
        let incremented = increment_wait_attempts_v1(attempts);
        assert_eq!(incremented, attempts.saturating_add(1));
        match wait_prefix_v1(incremented) {
            WaitPrefixV1::Spin => assert!(incremented <= 64),
            WaitPrefixV1::Yield => assert!((65..=80).contains(&incremented)),
            WaitPrefixV1::Sleep => assert!(incremented > 80),
        }
    }
}

#[test]
fn sleep_selector_and_existing_duration_adapter_match_std() {
    let mut pairs = 0;
    for next in duration_boundaries() {
        let next_pair = (next.as_secs(), next.subsec_nanos());
        assert!(!wait_sleep_uses_remaining_v1(next_pair, None));
        assert_eq!(wait_sleep_duration_v1(next, None), next);
        for remaining in duration_boundaries() {
            let remaining_pair = (remaining.as_secs(), remaining.subsec_nanos());
            assert_eq!(
                wait_sleep_uses_remaining_v1(next_pair, Some(remaining_pair)),
                remaining < next
            );
            assert_eq!(
                wait_sleep_duration_v1(next, Some(remaining)),
                remaining.min(next)
            );
            pairs += 1;
        }
    }
    assert_eq!(pairs, 3136);
}

#[test]
fn pair_backoff_and_duration_adapter_match_std_full_range() {
    let mut pairs = 0;
    for next in duration_boundaries() {
        for ceiling in duration_boundaries() {
            let pair = wait_backoff_pair_v1(
                (next.as_secs(), next.subsec_nanos()),
                (ceiling.as_secs(), ceiling.subsec_nanos()),
            );
            let expected = next.saturating_mul(2).min(ceiling);
            assert!(pair.1 < WAIT_NANOS_PER_SECOND_V1);
            assert_eq!(pair, (expected.as_secs(), expected.subsec_nanos()));
            assert_eq!(wait_backoff_duration_v1(next, ceiling), expected);
            pairs += 1;
        }
    }
    assert_eq!(pairs, 3136);
}

#[test]
fn carry_saturation_and_ceiling_boundaries_are_exact() {
    let max = (u64::MAX, 999_999_999);
    for (next, expected) in [
        ((0, 0), (0, 0)),
        ((0, 499_999_999), (0, 999_999_998)),
        ((0, 500_000_000), (1, 0)),
        ((0, 500_000_001), (1, 2)),
        ((u64::MAX / 2, 499_999_999), (u64::MAX - 1, 999_999_998)),
        ((u64::MAX / 2, 500_000_000), (u64::MAX, 0)),
        ((u64::MAX / 2, 999_999_999), (u64::MAX, 999_999_998)),
        ((u64::MAX / 2 + 1, 0), max),
        (max, max),
    ] {
        assert_eq!(wait_backoff_pair_v1(next, max), expected);
    }
    for ceiling in [(0, 0), (0, 25_000), (0, 1_000_000), (1, 1), max] {
        assert_eq!(wait_backoff_pair_v1(max, ceiling), ceiling);
    }
    assert_eq!(wait_backoff_pair_v1((0, 25_000), (0, 25_000)), (0, 25_000));
    assert_eq!(
        wait_backoff_pair_v1((0, 25_000), (0, 1_000_000)),
        (0, 50_000)
    );
    assert_eq!(
        wait_backoff_pair_v1((1, 0), (0, 999_999_999)),
        (0, 999_999_999)
    );
}
