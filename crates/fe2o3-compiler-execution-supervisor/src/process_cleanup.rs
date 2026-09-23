//! Finite cleanup mechanics shared by foreground and deferred child custody.
//!
//! The caller supplies exclusive consuming-wait ownership and retains this record
//! in its existing reserved slot until terminal reaping. This module creates no
//! worker, reserves no slot, and supplies no persistent native ledger accounting.

use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};

use fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions};

/// Disposition of one finite cleanup attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub(crate) enum CleanupPollV1 {
    /// Retain the record and its slot for a later cleanup attempt.
    Pending,
    /// An exact consuming terminal wait succeeded; the caller may retire custody.
    Reaped,
    /// Retain custody and capacity without further signaling or waiting.
    Quarantined,
}

/// Move-only custody of one child and its inherited artifact-spawn obligation.
///
/// Transfer the whole value to the existing deferred table on uncertainty. Drop
/// deliberately preserves unresolved resources; it is not a cleanup service or
/// a replacement for retaining a reachable, accounted record in that table.
#[must_use]
pub(crate) struct ChildCleanupV1 {
    pid: Pid,
    custody: CleanupCustodyV1<OwnedFd, ArtifactProcessSpawnLeaseV1>,
}

impl ChildCleanupV1 {
    /// Adopts the exact child immediately after clone, before fallible work.
    ///
    /// Any pidfd must identify `pid`, and the caller must hold exclusive wait
    /// ownership. A missing atomic clone pidfd quarantines the record: the scalar
    /// PID alone cannot establish safe signal ownership after an external reap.
    pub(crate) fn new(
        pidfd: Option<OwnedFd>,
        pid: Pid,
        spawn_lease: Option<ArtifactProcessSpawnLeaseV1>,
    ) -> Self {
        Self {
            pid,
            custody: CleanupCustodyV1::new(pidfd, spawn_lease),
        }
    }

    pub(crate) fn pid(&self) -> Pid {
        self.pid
    }

    /// Borrows the exact pidfd without transferring consuming-wait ownership.
    pub(crate) fn pidfd(&self) -> Option<&OwnedFd> {
        self.custody.pidfd.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn retains_spawn_lease(&self) -> bool {
        self.custody.spawn_lease.is_some()
    }

    /// Returns the most recent syscall errno, retained across successful calls.
    ///
    /// This includes signal `SRCH`. Missing-pidfd quarantine invents no errno.
    /// A recorded ownership loss reports `CHILD` before the next cleanup step.
    pub(crate) fn last_errno(&self) -> Option<Errno> {
        self.custody.last_errno()
    }

    /// Records `ECHILD` from a foreground observation or consuming wait.
    ///
    /// The next cleanup step quarantines without signaling or waiting. This
    /// shared-reference notification releases no resources and preserves an
    /// already-confirmed terminal reap.
    pub(crate) fn ownership_lost(&self) {
        self.custody.ownership_lost();
    }

    /// Releases inherited-lock custody only after the caller verifies exec.
    ///
    /// Gate release, a signal result, and an inconclusive wait are insufficient.
    /// This leaves child cleanup and wait ownership unchanged.
    pub(crate) fn release_spawn_after_exec(&mut self) {
        self.custody.release_spawn_after_exec();
    }

    /// Records the caller's successful exact consuming terminal wait.
    ///
    /// The caller must have consumed an exited, killed, or core-dumped status
    /// for this child under exclusive wait ownership. A `NOWAIT` observation,
    /// nonterminal status, `ECHILD`, or a signal result does not qualify. Call
    /// this immediately after the wait, before dropping or completing the slot.
    pub(crate) fn terminal_reaped(&mut self) {
        self.custody.terminal_reaped();
    }

    /// Attempts at most one pidfd KILL followed by one consuming NOHANG wait.
    ///
    /// There is no retry loop, allocation, or raw-PID fallback. Pending and
    /// quarantined records retain their descriptor and any unverified spawn
    /// lease. Terminal reaping releases that lease but leaves descriptor and
    /// slot retirement to the caller. Repeated terminal calls perform no I/O.
    pub(crate) fn step(&mut self) -> CleanupPollV1 {
        self.custody.step(&mut PidfdCleanupSyscallsV1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupPhaseV1 {
    KillRequired,
    AwaitingExit,
    Quarantined,
    Reaped,
}

// Generic resource ownership lets fake schedules exercise the same lease and
// descriptor retention paths without constructing OS descriptors or children.
struct CleanupCustodyV1<F, L> {
    pidfd: Option<F>,
    spawn_lease: Option<L>,
    phase: CleanupPhaseV1,
    last_errno: Option<Errno>,
    ownership_lost: AtomicBool,
}

impl<F, L> CleanupCustodyV1<F, L> {
    fn new(pidfd: Option<F>, spawn_lease: Option<L>) -> Self {
        let phase = if pidfd.is_some() {
            CleanupPhaseV1::KillRequired
        } else {
            CleanupPhaseV1::Quarantined
        };
        Self {
            pidfd,
            spawn_lease,
            phase,
            last_errno: None,
            ownership_lost: AtomicBool::new(false),
        }
    }

    fn last_errno(&self) -> Option<Errno> {
        if self.ownership_lost.load(Ordering::Acquire) {
            Some(Errno::CHILD)
        } else {
            self.last_errno
        }
    }

    fn ownership_lost(&self) {
        if self.phase != CleanupPhaseV1::Reaped {
            self.ownership_lost.store(true, Ordering::Release);
        }
    }

    fn release_spawn_after_exec(&mut self) {
        drop(self.spawn_lease.take());
    }

    fn terminal_reaped(&mut self) {
        self.phase = CleanupPhaseV1::Reaped;
        drop(self.spawn_lease.take());
    }

    fn step(&mut self, syscalls: &mut impl CleanupSyscallsV1<F>) -> CleanupPollV1 {
        if self.phase == CleanupPhaseV1::Reaped {
            return CleanupPollV1::Reaped;
        }
        if self.ownership_lost.load(Ordering::Acquire) {
            self.phase = CleanupPhaseV1::Quarantined;
            self.last_errno = Some(Errno::CHILD);
            return CleanupPollV1::Quarantined;
        }
        if self.phase == CleanupPhaseV1::Quarantined {
            return CleanupPollV1::Quarantined;
        }
        let Some(pidfd) = self.pidfd.as_ref() else {
            self.phase = CleanupPhaseV1::Quarantined;
            return CleanupPollV1::Quarantined;
        };
        if self.phase == CleanupPhaseV1::KillRequired {
            match syscalls.kill(pidfd) {
                Ok(()) => self.phase = CleanupPhaseV1::AwaitingExit,
                Err(errno) => {
                    self.last_errno = Some(errno);
                    if errno == Errno::SRCH {
                        self.phase = CleanupPhaseV1::AwaitingExit;
                    }
                }
            }
        }
        match syscalls.wait_exited_nohang(pidfd) {
            Ok(Some(CleanupWaitV1::Terminal)) => {
                self.terminal_reaped();
                CleanupPollV1::Reaped
            }
            Ok(None | Some(CleanupWaitV1::Nonterminal)) => CleanupPollV1::Pending,
            Err(errno) => {
                self.last_errno = Some(errno);
                if errno == Errno::CHILD {
                    self.ownership_lost();
                    self.phase = CleanupPhaseV1::Quarantined;
                    CleanupPollV1::Quarantined
                } else {
                    CleanupPollV1::Pending
                }
            }
        }
    }
}

impl<F, L> Drop for CleanupCustodyV1<F, L> {
    fn drop(&mut self) {
        if self.phase != CleanupPhaseV1::Reaped {
            // An owner dropped before transfer must not falsely release the
            // pre-exec obligation. This fail-closed leak cannot provide progress.
            std::mem::forget(self.pidfd.take());
            std::mem::forget(self.spawn_lease.take());
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupWaitV1 {
    Terminal,
    Nonterminal,
}

impl CleanupWaitV1 {
    fn from_wait_code(code: i32) -> Self {
        match code {
            libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED => Self::Terminal,
            _ => Self::Nonterminal,
        }
    }
}

trait CleanupSyscallsV1<F> {
    fn kill(&mut self, pidfd: &F) -> rustix::io::Result<()>;
    fn wait_exited_nohang(&mut self, pidfd: &F) -> rustix::io::Result<Option<CleanupWaitV1>>;
}

struct PidfdCleanupSyscallsV1;

impl CleanupSyscallsV1<OwnedFd> for PidfdCleanupSyscallsV1 {
    fn kill(&mut self, pidfd: &OwnedFd) -> rustix::io::Result<()> {
        rustix::process::pidfd_send_signal(pidfd, Signal::KILL)
    }

    fn wait_exited_nohang(&mut self, pidfd: &OwnedFd) -> rustix::io::Result<Option<CleanupWaitV1>> {
        rustix::process::waitid(
            WaitId::PidFd(pidfd.as_fd()),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG,
        )
        .map(|status| status.map(|status| CleanupWaitV1::from_wait_code(status.raw_code())))
    }
}

#[cfg(test)]
#[path = "process_cleanup_tests.rs"]
mod tests;
