use super::*;
use rustix::io::Errno;

#[test]
fn gate_interruptions_exhaust_exactly_the_prepaid_attempts() {
    assert_eq!(MAX_CHILD_GATE_ATTEMPTS, 64);
    let mut attempts = 0;
    let result = read_child_gate(|_| {
        attempts += 1;
        Err(Errno::INTR)
    });
    assert_eq!(result, Err(()));
    assert_eq!(attempts, MAX_CHILD_GATE_ATTEMPTS);
}

#[test]
fn gate_release_can_arrive_on_any_prepaid_attempt_including_the_last() {
    for release_attempt in 1..=MAX_CHILD_GATE_ATTEMPTS {
        let mut attempts = 0;
        let result = read_child_gate(|release| {
            attempts += 1;
            if attempts == release_attempt {
                *release = GATE_RELEASE_V1;
                Ok(1)
            } else {
                Err(Errno::INTR)
            }
        });
        assert_eq!(result, Ok(GATE_RELEASE_V1));
        assert_eq!(attempts, release_attempt);
    }
}

#[test]
fn gate_eof_invalid_length_or_permanent_failure_never_retries() {
    for read_result in [Ok(0), Ok(2), Err(Errno::AGAIN), Err(Errno::BADF)] {
        let mut attempts = 0;
        let result = read_child_gate(|_| {
            attempts += 1;
            read_result
        });
        assert_eq!(result, Err(()));
        assert_eq!(attempts, 1);
    }
}

#[test]
fn child_work_prepays_full_gate_signal_descriptor_and_failure_paths() {
    // Independently count the longest profile-free child path, including the
    // final failed exec followed by the failure report and exit.
    let syscall_attempts = 5 + 62 + 1 + 4 + 1 + 64 + 1 + 3 + 14 + 1 + 2;
    assert!(
        child_work(None)
            >= syscall_attempts * (1024 + 64) + 256 + CHILD_NAMESPACE_REPORT_CAPTURE_WORK
    );
}

#[test]
fn child_work_includes_the_capability_ceiling_without_wrapping() {
    let without_profile = child_work(None) as u128;
    for cap_last_cap in [0, 1, 40, 63, u32::MAX] {
        let profile = ChildProfileV1 {
            uid: 1,
            gid: 1,
            securebits: 0,
            cap_last_cap,
        };
        let profile_syscalls = 11 + 2 * (u128::from(cap_last_cap) + 1);
        let expected = without_profile + profile_syscalls * (1024 + 64);
        assert_eq!(
            child_work(Some(profile)) as u128,
            expected.min(usize::MAX as u128),
        );
    }
}

#[test]
fn observation_success_and_transient_errors_do_not_claim_reaping_or_ownership_loss() {
    // A descriptor-free record allows result handling to be tested without a
    // process, syscall, global reaper reservation, or artifact lock obligation.
    let mut cleanup = ChildCleanupV1::new(None, rustix::process::Pid::from_raw(1).unwrap(), None);
    let operation = "observe exact issuer pidfd";
    for status in [None, Some(())] {
        assert_eq!(
            child_wait_result(Some(&cleanup), operation, Ok(status)),
            Ok(status),
        );
        assert_eq!(cleanup.last_errno(), None);
    }
    for errno in [Errno::INTR, Errno::AGAIN, Errno::BADF, Errno::INVAL] {
        assert_eq!(
            child_wait_result::<()>(Some(&cleanup), operation, Err(errno)),
            Err(ChildProcessError::Io { operation, errno }),
        );
        assert_eq!(cleanup.last_errno(), None);
    }
    assert_eq!(cleanup.step(), CleanupPollV1::Quarantined);
}

#[test]
fn observation_and_consuming_wait_echild_preserve_quarantine() {
    for operation in [
        "observe exact issuer pidfd",
        "reap naturally exited issuer pidfd",
    ] {
        let mut cleanup =
            ChildCleanupV1::new(None, rustix::process::Pid::from_raw(1).unwrap(), None);
        assert_eq!(
            child_wait_result::<()>(Some(&cleanup), operation, Err(Errno::CHILD)),
            Err(ChildProcessError::Io {
                operation,
                errno: Errno::CHILD,
            }),
        );
        assert_eq!(cleanup.last_errno(), Some(Errno::CHILD));
        assert_eq!(
            child_wait_result::<()>(Some(&cleanup), operation, Ok(None)),
            Ok(None)
        );
        assert_eq!(cleanup.last_errno(), Some(Errno::CHILD));
        assert_eq!(cleanup.step(), CleanupPollV1::Quarantined);
        assert_eq!(cleanup.step(), CleanupPollV1::Quarantined);
    }
}

#[test]
fn transferred_or_completed_custody_cannot_be_observed_or_canceled_again() {
    for cleanup_poll in [
        CleanupPollV1::Pending,
        CleanupPollV1::Quarantined,
        CleanupPollV1::Reaped,
    ] {
        let mut child = IssuerChild {
            cleanup: None,
            pid: rustix::process::Pid::from_raw(1).unwrap(),
            reap_slot: None,
            cleanup_poll,
        };
        let error = ChildProcessError::State("pidfd is absent or cleanup custody was transferred");
        assert_eq!(child.check_pidfd(), Err(error));
        assert_eq!(child.observe_live(), Err(error));
        assert_eq!(child.try_reap(), Err(error));
        assert_eq!(child.cancel_once(), cleanup_poll);
        assert_eq!(child.cancel_once(), cleanup_poll);
        drop(child);
    }
}
