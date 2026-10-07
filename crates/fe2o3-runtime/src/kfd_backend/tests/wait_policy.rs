use super::*;

#[test]
fn productive_pending_polls_do_not_enter_wait_backoff() {
    let mut polls = 0_u32;
    let mut backoffs = 0_u32;
    let status = wait_with_deadline_tracking_progress_by_v1(
        Instant::now() + Duration::from_secs(1),
        || {
            polls += 1;
            Ok::<_, ()>((
                if polls == 128 {
                    BackendPollV1::Succeeded
                } else {
                    BackendPollV1::Pending
                },
                true,
            ))
        },
        |_, _, _| {
            backoffs += 1;
            true
        },
    )
    .unwrap();
    assert_eq!(status, BackendPollV1::Succeeded);
    assert_eq!(polls, 128);
    assert_eq!(backoffs, 0);
}

#[test]
fn stalled_pending_polls_still_enter_wait_backoff() {
    let mut polls = 0_u32;
    let mut backoffs = 0_u32;
    let status = wait_with_deadline_tracking_progress_by_v1(
        Instant::now() + Duration::from_secs(1),
        || {
            polls += 1;
            Ok::<_, ()>((
                if polls == 4 {
                    BackendPollV1::Succeeded
                } else {
                    BackendPollV1::Pending
                },
                false,
            ))
        },
        |_, _, _| {
            backoffs += 1;
            true
        },
    )
    .unwrap();
    assert_eq!(status, BackendPollV1::Succeeded);
    assert_eq!(backoffs, 3);
}

#[test]
fn compute_pending_wait_policy_accepts_64_consecutive_settlements_without_backoff() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut attempts = 0;
    let mut sleep = WAIT_INITIAL_SLEEP_V1;
    for prior in (1..=64).rev() {
        assert!(continue_pending_compute_wait_v1(
            prior,
            prior - 1,
            &mut attempts,
            &mut sleep,
            deadline,
            |_, _, _| panic!("logical settlement must not enter backoff"),
        ));
        assert_eq!(attempts, 0);
        assert_eq!(sleep, WAIT_INITIAL_SLEEP_V1);
    }
}

#[test]
fn compute_pending_wait_stalls_preserve_escalation_and_absolute_deadline() {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut attempts = 0;
    let mut sleep = WAIT_INITIAL_SLEEP_V1;
    for expected in 1..=64 {
        assert!(continue_pending_compute_wait_v1(
            64,
            64,
            &mut attempts,
            &mut sleep,
            deadline,
            |attempt, sleep, observed_deadline| {
                assert_eq!(attempt, expected);
                assert_eq!(observed_deadline, deadline);
                if attempt >= WAIT_SPINS_V1 + WAIT_YIELDS_V1 {
                    *sleep = sleep.saturating_mul(2).min(WAIT_MAX_SLEEP_V1);
                }
                true
            },
        ));
    }
    assert!(sleep > WAIT_INITIAL_SLEEP_V1);
    assert!(continue_pending_compute_wait_v1(
        64,
        63,
        &mut attempts,
        &mut sleep,
        deadline,
        |_, _, _| panic!("progress must reset escalated backoff"),
    ));
    assert_eq!(attempts, 0);
    assert_eq!(sleep, WAIT_INITIAL_SLEEP_V1);
    assert!(!continue_pending_compute_wait_v1(
        63,
        64,
        &mut attempts,
        &mut sleep,
        deadline,
        |attempt, sleep, observed_deadline| {
            assert_eq!(attempt, 1);
            assert_eq!(*sleep, WAIT_INITIAL_SLEEP_V1);
            assert_eq!(observed_deadline, deadline);
            false
        },
    ));
}

#[test]
fn compute_pending_wait_progress_never_renews_an_expired_deadline() {
    let deadline = Instant::now();
    for current in [0, 1, 2] {
        let mut attempts = 64;
        let mut sleep = WAIT_MAX_SLEEP_V1;
        assert!(!continue_pending_compute_wait_v1(
            1,
            current,
            &mut attempts,
            &mut sleep,
            deadline,
            |_, _, _| panic!("expired waits must stop before backoff"),
        ));
        assert_eq!(attempts, 64);
        assert_eq!(sleep, WAIT_MAX_SLEEP_V1);
    }
}

#[test]
fn compute_pending_wait_production_wiring_tracks_poll_settlement_after_native_routes() {
    let body = include_str!("../../kfd_backend.rs")
        .split("fn wait_v1(")
        .nth(1)
        .unwrap()
        .split("fn release_submission_v1")
        .next()
        .unwrap();
    let ordered_fragments = [
        "return self.wait_published_sdma_v1(",
        "self.wait_published_persistent_compute_lane_v1(lane, deadline)?",
        "let prior_reservations = self.compute_completion_reservations;",
        "let status = self.poll_v1(submission)?;",
        "if status != BackendPollV1::Pending {",
        "return Ok(status);",
        "continue_pending_compute_wait_v1(",
        "prior_reservations,",
        "self.compute_completion_reservations,",
        "&mut attempts,",
        "&mut sleep,",
        "deadline,",
        "apply_wait_backoff_v1,",
    ];
    let mut remaining = body;
    for fragment in ordered_fragments {
        let (_, tail) = remaining.split_once(fragment).unwrap();
        remaining = tail;
    }
    assert_eq!(body.matches("self.poll_v1(submission)?").count(), 1);
    assert_eq!(body.matches("continue_pending_compute_wait_v1(").count(), 1);
}

#[test]
fn peer_copy_is_explicitly_rejected() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let binding = BackendMemoryRegionV1 {
        allocation: 1,
        access: RuntimeAccessV1::Read,
        byte_offset: 0,
        byte_len: 8,
    };
    assert!(matches!(
        backend.peer_copy_v1(1, binding, binding, &[]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
}
