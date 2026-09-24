use std::cell::Cell;
use std::collections::VecDeque;

use super::*;

#[derive(Default)]
struct ResourceDropsV1 {
    pidfd: Cell<usize>,
    spawn_lease: Cell<usize>,
}

struct DropProbeV1<'a>(&'a Cell<usize>);

impl Drop for DropProbeV1<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

type FakeCustodyV1<'a> = CleanupCustodyV1<DropProbeV1<'a>, DropProbeV1<'a>>;

fn custody(drops: &ResourceDropsV1) -> FakeCustodyV1<'_> {
    CleanupCustodyV1::new(
        Some(DropProbeV1(&drops.pidfd)),
        Some(DropProbeV1(&drops.spawn_lease)),
    )
}

#[derive(Debug)]
enum CallV1 {
    Kill(rustix::io::Result<()>),
    Wait(rustix::io::Result<Option<CleanupWaitV1>>),
}

struct FakeSyscallsV1<'a> {
    expected_pidfd: &'a Cell<usize>,
    schedule: VecDeque<CallV1>,
    kills: usize,
    waits: usize,
}

impl<'a> FakeSyscallsV1<'a> {
    fn new(drops: &'a ResourceDropsV1, schedule: impl IntoIterator<Item = CallV1>) -> Self {
        Self {
            expected_pidfd: &drops.pidfd,
            schedule: schedule.into_iter().collect(),
            kills: 0,
            waits: 0,
        }
    }

    fn assert_finished(&self, kills: usize, waits: usize) {
        assert!(
            self.schedule.is_empty(),
            "unconsumed calls: {:?}",
            self.schedule
        );
        assert_eq!((self.kills, self.waits), (kills, waits));
    }
}

impl CleanupSyscallsV1<DropProbeV1<'_>> for FakeSyscallsV1<'_> {
    fn kill(&mut self, pidfd: &DropProbeV1<'_>) -> rustix::io::Result<()> {
        assert!(std::ptr::eq(pidfd.0, self.expected_pidfd));
        self.kills += 1;
        match self.schedule.pop_front() {
            Some(CallV1::Kill(result)) => result,
            other => panic!("unexpected kill; next scheduled call: {other:?}"),
        }
    }

    fn wait_exited_nohang(
        &mut self,
        pidfd: &DropProbeV1<'_>,
    ) -> rustix::io::Result<Option<CleanupWaitV1>> {
        assert!(std::ptr::eq(pidfd.0, self.expected_pidfd));
        self.waits += 1;
        match self.schedule.pop_front() {
            Some(CallV1::Wait(result)) => result,
            other => panic!("unexpected wait; next scheduled call: {other:?}"),
        }
    }
}

#[test]
fn pending_retains_custody_and_does_not_repeat_an_accepted_kill() {
    let drops = ResourceDropsV1::default();
    let mut owner = custody(&drops);
    let mut syscalls = FakeSyscallsV1::new(
        &drops,
        [
            CallV1::Kill(Ok(())),
            CallV1::Wait(Ok(None)),
            CallV1::Wait(Ok(None)),
        ],
    );

    for _ in 0..2 {
        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
        assert_eq!(owner.phase, CleanupPhaseV1::AwaitingExit);
        assert!(owner.pidfd.is_some());
        assert!(owner.spawn_lease.is_some());
        assert_eq!(owner.last_errno, None);
    }
    syscalls.assert_finished(1, 2);
    drop(owner);
    assert_eq!(drops.pidfd.get(), 0);
    assert_eq!(drops.spawn_lease.get(), 0);
}

#[test]
fn failed_kill_retries_only_on_a_later_step() {
    for error in [Errno::PERM, Errno::INTR, Errno::IO, Errno::BADF] {
        let drops = ResourceDropsV1::default();
        let mut owner = custody(&drops);
        let mut syscalls = FakeSyscallsV1::new(
            &drops,
            [
                CallV1::Kill(Err(error)),
                CallV1::Wait(Ok(None)),
                CallV1::Kill(Ok(())),
                CallV1::Wait(Ok(None)),
            ],
        );

        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
        assert_eq!(owner.phase, CleanupPhaseV1::KillRequired);
        assert_eq!(owner.last_errno, Some(error));
        assert_eq!((syscalls.kills, syscalls.waits), (1, 1));
        assert!(owner.spawn_lease.is_some());
        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
        assert_eq!(owner.phase, CleanupPhaseV1::AwaitingExit);
        assert_eq!(owner.last_errno, Some(error));
        syscalls.assert_finished(2, 2);
    }
}

#[test]
fn srch_stops_kill_retries_but_cannot_certify_reaping() {
    let drops = ResourceDropsV1::default();
    let mut owner = custody(&drops);
    let mut syscalls = FakeSyscallsV1::new(
        &drops,
        [
            CallV1::Kill(Err(Errno::SRCH)),
            CallV1::Wait(Ok(None)),
            CallV1::Wait(Ok(Some(CleanupWaitV1::Terminal))),
        ],
    );

    assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
    assert_eq!(owner.phase, CleanupPhaseV1::AwaitingExit);
    assert_eq!(owner.last_errno, Some(Errno::SRCH));
    assert_eq!(drops.spawn_lease.get(), 0);
    assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
    assert_eq!(drops.spawn_lease.get(), 1);
    syscalls.assert_finished(1, 2);
}

#[test]
fn interrupted_or_uncertain_wait_keeps_custody_and_signal_state() {
    for signal in [Ok(()), Err(Errno::PERM)] {
        for wait_error in [Errno::INTR, Errno::IO, Errno::INVAL, Errno::BADF] {
            let drops = ResourceDropsV1::default();
            let mut owner = custody(&drops);
            let mut syscalls = FakeSyscallsV1::new(
                &drops,
                [CallV1::Kill(signal), CallV1::Wait(Err(wait_error))],
            );

            assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
            assert_eq!(
                owner.phase,
                if signal.is_ok() {
                    CleanupPhaseV1::AwaitingExit
                } else {
                    CleanupPhaseV1::KillRequired
                }
            );
            assert_eq!(owner.last_errno, Some(wait_error));
            assert!(owner.pidfd.is_some());
            assert!(owner.spawn_lease.is_some());
            syscalls.assert_finished(1, 1);
            drop(owner);
            assert_eq!(drops.pidfd.get(), 0);
            assert_eq!(drops.spawn_lease.get(), 0);
        }
    }
}

#[test]
fn child_error_quarantines_without_later_signal_or_wait() {
    for signal in [Ok(()), Err(Errno::PERM), Err(Errno::SRCH)] {
        let drops = ResourceDropsV1::default();
        let mut owner = custody(&drops);
        let mut syscalls = FakeSyscallsV1::new(
            &drops,
            [CallV1::Kill(signal), CallV1::Wait(Err(Errno::CHILD))],
        );

        for _ in 0..3 {
            assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Quarantined);
            assert_eq!(owner.phase, CleanupPhaseV1::Quarantined);
            assert_eq!(owner.last_errno, Some(Errno::CHILD));
            assert!(owner.pidfd.is_some());
            assert!(owner.spawn_lease.is_some());
        }
        syscalls.assert_finished(1, 1);
        drop(owner);
        assert_eq!(drops.pidfd.get(), 0);
        assert_eq!(drops.spawn_lease.get(), 0);
    }
}

#[test]
fn terminal_wait_releases_lease_once_even_after_signal_failure() {
    for signal in [Ok(()), Err(Errno::PERM), Err(Errno::INTR)] {
        let drops = ResourceDropsV1::default();
        let mut owner = custody(&drops);
        let mut syscalls = FakeSyscallsV1::new(
            &drops,
            [
                CallV1::Kill(signal),
                CallV1::Wait(Ok(Some(CleanupWaitV1::Terminal))),
            ],
        );

        for _ in 0..3 {
            assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
            assert_eq!(owner.phase, CleanupPhaseV1::Reaped);
            assert!(owner.pidfd.is_some());
            assert!(owner.spawn_lease.is_none());
            assert_eq!(drops.pidfd.get(), 0);
            assert_eq!(drops.spawn_lease.get(), 1);
        }
        syscalls.assert_finished(1, 1);
        drop(owner);
        assert_eq!(drops.pidfd.get(), 1);
        assert_eq!(drops.spawn_lease.get(), 1);
    }
}

#[test]
fn only_canonical_terminal_codes_can_finish_cleanup() {
    for code in [libc::CLD_EXITED, libc::CLD_KILLED, libc::CLD_DUMPED] {
        assert_eq!(CleanupWaitV1::from_wait_code(code), CleanupWaitV1::Terminal);
    }
    for code in [
        libc::CLD_STOPPED,
        libc::CLD_CONTINUED,
        libc::CLD_TRAPPED,
        0,
        -1,
        i32::MAX,
    ] {
        assert_eq!(
            CleanupWaitV1::from_wait_code(code),
            CleanupWaitV1::Nonterminal
        );
    }

    let drops = ResourceDropsV1::default();
    let mut owner = custody(&drops);
    let mut syscalls = FakeSyscallsV1::new(
        &drops,
        [
            CallV1::Kill(Ok(())),
            CallV1::Wait(Ok(Some(CleanupWaitV1::Nonterminal))),
            CallV1::Wait(Ok(Some(CleanupWaitV1::Terminal))),
        ],
    );
    assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
    assert!(owner.spawn_lease.is_some());
    assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
    syscalls.assert_finished(1, 2);
}

#[test]
fn each_step_is_finite_even_with_repeated_interruptions() {
    let drops = ResourceDropsV1::default();
    let mut owner = custody(&drops);
    let mut syscalls = FakeSyscallsV1::new(
        &drops,
        (0..8).flat_map(|_| {
            [
                CallV1::Kill(Err(Errno::INTR)),
                CallV1::Wait(Err(Errno::INTR)),
            ]
        }),
    );

    for step in 1..=8 {
        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
        assert_eq!((syscalls.kills, syscalls.waits), (step, step));
        assert_eq!(owner.phase, CleanupPhaseV1::KillRequired);
        assert_eq!(owner.last_errno, Some(Errno::INTR));
        assert!(owner.pidfd.is_some());
        assert!(owner.spawn_lease.is_some());
    }
    syscalls.assert_finished(8, 8);
}

#[test]
fn missing_pidfd_quarantines_without_any_syscall_or_lease_release() {
    let drops = ResourceDropsV1::default();
    let mut owner: FakeCustodyV1<'_> =
        CleanupCustodyV1::new(None, Some(DropProbeV1(&drops.spawn_lease)));
    let mut syscalls = FakeSyscallsV1::new(&drops, []);

    for _ in 0..3 {
        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Quarantined);
        assert_eq!(owner.phase, CleanupPhaseV1::Quarantined);
        assert_eq!(owner.last_errno, None);
        assert!(owner.spawn_lease.is_some());
    }
    syscalls.assert_finished(0, 0);
    drop(owner);
    assert_eq!(drops.spawn_lease.get(), 0);

    let pid = Pid::from_raw(123).expect("positive fake PID");
    let mut owner = ChildCleanupV1::new(None, pid, None);
    assert_eq!(owner.pid(), pid);
    assert!(owner.pidfd().is_none());
    assert_eq!(owner.last_errno(), None);
    assert_eq!(owner.step(), CleanupPollV1::Quarantined);
}

#[test]
fn verified_exec_releases_only_spawn_custody_and_is_idempotent() {
    let drops = ResourceDropsV1::default();
    let mut owner = custody(&drops);
    owner.release_spawn_after_exec();
    owner.release_spawn_after_exec();
    assert_eq!(drops.spawn_lease.get(), 1);
    assert_eq!(drops.pidfd.get(), 0);
    assert_eq!(owner.phase, CleanupPhaseV1::KillRequired);

    let mut syscalls = FakeSyscallsV1::new(
        &drops,
        [
            CallV1::Kill(Ok(())),
            CallV1::Wait(Ok(Some(CleanupWaitV1::Terminal))),
        ],
    );
    assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
    syscalls.assert_finished(1, 1);
    drop(owner);
    assert_eq!(drops.spawn_lease.get(), 1);
    assert_eq!(drops.pidfd.get(), 1);
}

#[test]
fn natural_terminal_wait_handoff_releases_once_and_disarms_cleanup() {
    let drops = ResourceDropsV1::default();
    let mut owner = custody(&drops);
    // Simulate the caller's already-consumed exact terminal wait.
    owner.terminal_reaped();
    owner.terminal_reaped();
    assert_eq!(drops.spawn_lease.get(), 1);
    assert_eq!(drops.pidfd.get(), 0);

    let mut syscalls = FakeSyscallsV1::new(&drops, []);
    assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
    syscalls.assert_finished(0, 0);
    drop(owner);
    assert_eq!(drops.spawn_lease.get(), 1);
    assert_eq!(drops.pidfd.get(), 1);
}

#[test]
fn unresolved_drop_does_not_release_even_before_the_first_step() {
    let drops = ResourceDropsV1::default();
    drop(custody(&drops));
    assert_eq!(drops.spawn_lease.get(), 0);
    assert_eq!(drops.pidfd.get(), 0);
}

#[test]
fn moving_pending_custody_preserves_retry_intent_and_resources() {
    let drops = ResourceDropsV1::default();
    let mut foreground = Some(custody(&drops));
    let mut syscalls = FakeSyscallsV1::new(
        &drops,
        [
            CallV1::Kill(Err(Errno::PERM)),
            CallV1::Wait(Err(Errno::INTR)),
            CallV1::Kill(Ok(())),
            CallV1::Wait(Ok(Some(CleanupWaitV1::Terminal))),
        ],
    );
    assert_eq!(
        foreground.as_mut().unwrap().step(&mut syscalls),
        CleanupPollV1::Pending
    );
    let mut deferred = foreground.take();
    assert!(foreground.is_none());
    assert_eq!(drops.pidfd.get(), 0);
    assert_eq!(drops.spawn_lease.get(), 0);
    assert_eq!(deferred.as_ref().unwrap().last_errno, Some(Errno::INTR));
    assert_eq!(
        deferred.as_mut().unwrap().step(&mut syscalls),
        CleanupPollV1::Reaped
    );
    syscalls.assert_finished(2, 2);
    drop(deferred.take());
    assert_eq!(drops.pidfd.get(), 1);
    assert_eq!(drops.spawn_lease.get(), 1);
}

#[test]
fn foreground_ownership_loss_survives_transfer_without_any_cleanup_syscall() {
    let drops = ResourceDropsV1::default();
    let mut foreground = Some(custody(&drops));
    let observation = foreground.as_ref().unwrap();
    observation.ownership_lost();
    observation.ownership_lost();
    assert_eq!(observation.last_errno(), Some(Errno::CHILD));
    assert_eq!(drops.pidfd.get(), 0);
    assert_eq!(drops.spawn_lease.get(), 0);

    let mut deferred = foreground.take();
    assert!(foreground.is_none());
    let mut syscalls = FakeSyscallsV1::new(&drops, []);
    for _ in 0..3 {
        let owner = deferred.as_mut().unwrap();
        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Quarantined);
        assert_eq!(owner.last_errno(), Some(Errno::CHILD));
        assert!(owner.pidfd.is_some());
        assert!(owner.spawn_lease.is_some());
    }
    assert!(
        deferred.is_some(),
        "quarantine must retain the occupied slot"
    );
    syscalls.assert_finished(0, 0);
    drop(deferred);
    assert_eq!(drops.pidfd.get(), 0);
    assert_eq!(drops.spawn_lease.get(), 0);
}

#[test]
fn foreground_ownership_loss_suppresses_both_kill_retry_and_awaiting_exit_wait() {
    for signal in [Ok(()), Err(Errno::PERM)] {
        let drops = ResourceDropsV1::default();
        let mut owner = custody(&drops);
        let mut syscalls = FakeSyscallsV1::new(
            &drops,
            [CallV1::Kill(signal), CallV1::Wait(Err(Errno::INTR))],
        );
        assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Pending);
        assert_eq!(owner.last_errno(), Some(Errno::INTR));

        let observation = &owner;
        observation.ownership_lost();
        assert_eq!(observation.last_errno(), Some(Errno::CHILD));
        for _ in 0..3 {
            assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Quarantined);
            assert_eq!(owner.phase, CleanupPhaseV1::Quarantined);
            assert_eq!(owner.last_errno(), Some(Errno::CHILD));
            assert!(owner.pidfd.is_some());
            assert!(owner.spawn_lease.is_some());
        }
        syscalls.assert_finished(1, 1);
        drop(owner);
        assert_eq!(drops.pidfd.get(), 0);
        assert_eq!(drops.spawn_lease.get(), 0);
    }
}

#[test]
fn ownership_loss_preserves_resources_even_when_dropped_before_the_next_step() {
    let drops = ResourceDropsV1::default();
    let owner = custody(&drops);
    owner.ownership_lost();
    assert_eq!(owner.last_errno(), Some(Errno::CHILD));
    drop(owner);
    assert_eq!(drops.pidfd.get(), 0);
    assert_eq!(drops.spawn_lease.get(), 0);
}

#[test]
fn ownership_loss_after_confirmed_reaping_does_not_regress_terminal_custody() {
    for natural_wait in [false, true] {
        let drops = ResourceDropsV1::default();
        let mut owner = custody(&drops);
        if natural_wait {
            owner.terminal_reaped();
        } else {
            let mut syscalls = FakeSyscallsV1::new(
                &drops,
                [
                    CallV1::Kill(Err(Errno::PERM)),
                    CallV1::Wait(Ok(Some(CleanupWaitV1::Terminal))),
                ],
            );
            assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
            syscalls.assert_finished(1, 1);
        }
        let last_errno = owner.last_errno();
        let mut syscalls = FakeSyscallsV1::new(&drops, []);
        for _ in 0..3 {
            owner.ownership_lost();
            assert_eq!(owner.step(&mut syscalls), CleanupPollV1::Reaped);
            assert_eq!(owner.last_errno(), last_errno);
            assert_eq!(drops.spawn_lease.get(), 1);
            assert_eq!(drops.pidfd.get(), 0);
        }
        syscalls.assert_finished(0, 0);
        drop(owner);
        assert_eq!(drops.pidfd.get(), 1);
        assert_eq!(drops.spawn_lease.get(), 1);
    }
}

#[test]
fn shared_ownership_loss_notification_preserves_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ChildCleanupV1>();

    let pid = Pid::from_raw(123).expect("positive fake PID");
    let mut owner = ChildCleanupV1::new(None, pid, None);
    let observation = &owner;
    observation.ownership_lost();
    assert_eq!(observation.last_errno(), Some(Errno::CHILD));
    assert_eq!(owner.step(), CleanupPollV1::Quarantined);
    assert_eq!(owner.last_errno(), Some(Errno::CHILD));
}
