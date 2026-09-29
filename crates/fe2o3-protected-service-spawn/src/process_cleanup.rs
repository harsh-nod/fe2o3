//! Finite cleanup mechanics for protected-service child custody.
//! Consumed by native issuer and root coordinator launches through one shared pool.
//!
//! The caller supplies exclusive consuming-wait ownership and retains this record
//! in its existing reserved slot until root reaping and domain cleanup. This
//! module creates no worker, reserves no slot, and supplies no persistent ledger.

use std::os::fd::{AsFd, OwnedFd};
use std::sync::atomic::{AtomicBool, Ordering};

use fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, WaitId, WaitIdOptions};

use crate::native_cgroup::NativeCgroupDomainV1;
use crate::native_spawn::{ProtectedServiceSpawnErrorV2 as SpawnError, Result as SpawnResult};
use crate::native_user_namespace::NativeUserNamespaceV1;

/// Disposition of one finite cleanup attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum CleanupPollV1 {
    /// Retain the record and its slot for a later cleanup attempt.
    Pending,
    /// Every required root wait and retained domain cleanup completed.
    /// A domain-only rollback has no child wait obligation.
    Reaped,
    /// Retain custody and capacity without further signaling or waiting.
    Quarantined,
}

/// Move-only child, optional domain/namespace and inherited artifact-spawn custody.
///
/// Transfer the whole value to the existing deferred table on uncertainty. Drop
/// deliberately preserves unresolved resources; it is not a cleanup service or
/// a replacement for retaining a reachable, accounted record in that table.
#[must_use]
pub struct ChildCleanupV1 {
    pid: Pid,
    custody: CleanupCustodyV1<OwnedFd, ArtifactProcessSpawnLeaseV1, NativeCgroupDomainV1>,
    namespace: Option<NativeUserNamespaceV1>,
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
            namespace: None,
        }
    }

    /// Adopts the launch's concrete domain along with its exact child and lease.
    /// This mechanical transfer grants no deployment or isolation admission.
    pub(crate) fn new_with_domain(
        pidfd: Option<OwnedFd>,
        pid: Pid,
        spawn_lease: Option<ArtifactProcessSpawnLeaseV1>,
        domain: NativeCgroupDomainV1,
    ) -> Self {
        Self {
            pid,
            custody: CleanupCustodyV1::with_domain(pidfd, spawn_lease, domain),
            namespace: None,
        }
    }

    /// Retains the prepared namespace before any fallible post-clone operation.
    /// The caller supplies the same gated child/domain/pidfd from controlled clone.
    pub(crate) fn new_with_domain_and_namespace(
        pidfd: Option<OwnedFd>,
        pid: Pid,
        spawn_lease: Option<ArtifactProcessSpawnLeaseV1>,
        domain: NativeCgroupDomainV1,
        namespace: NativeUserNamespaceV1,
    ) -> Self {
        Self {
            pid,
            custody: CleanupCustodyV1::with_domain(pidfd, spawn_lease, domain),
            namespace: Some(namespace),
        }
    }

    /// Mechanical map setup only, not proof or namespace-isolation admission.
    /// The launch wrapper must prepay CONFIGURE_WORK/SCRATCH on its original
    /// ledger before clone, and keep the exact child behind its mapping gate.
    /// Errors never extract the namespace or discharge cleanup custody.
    pub(crate) fn configure_namespace(&mut self) -> SpawnResult<()> {
        self.require_namespace_setup_custody()?;
        let pidfd = self
            .custody
            .pidfd
            .as_ref()
            .ok_or(SpawnError::State("atomic native child pidfd is absent"))?;
        let result = self
            .namespace
            .as_mut()
            .ok_or(SpawnError::State("native child has no namespace custody"))?
            .configure_child(self.pid, pidfd);
        self.observe_namespace_result(result)
    }

    /// Rechecks only the exact retained child and namespace. The launch wrapper
    /// prepays REVALIDATE_WORK/SCRATCH; this creates no budget or admission.
    pub(crate) fn revalidate_namespace(&self) -> SpawnResult<()> {
        self.require_namespace_setup_custody()?;
        let pidfd = self
            .custody
            .pidfd
            .as_ref()
            .ok_or(SpawnError::State("atomic native child pidfd is absent"))?;
        let result = self
            .namespace
            .as_ref()
            .ok_or(SpawnError::State("native child has no namespace custody"))?
            .revalidate_child(self.pid, pidfd);
        self.observe_namespace_result(result)
    }

    fn observe_namespace_result(&self, result: SpawnResult<()>) -> SpawnResult<()> {
        if matches!(
            &result,
            Err(SpawnError::Io {
                source: Errno::CHILD,
                ..
            })
        ) {
            self.custody.ownership_lost();
        }
        result
    }

    fn require_namespace_setup_custody(&self) -> SpawnResult<()> {
        if self.custody.phase != CleanupPhaseV1::KillRequired
            || self.custody.domain_poll != CleanupPollV1::Pending
            || self.custody.last_errno.is_some()
            || self.custody.ownership_lost.load(Ordering::Acquire)
        {
            return Err(SpawnError::State(
                "native child cleanup already started or lost ownership",
            ));
        }
        Ok(())
    }

    /// Returns the scalar identity bound by the trusted adoption protocol.
    pub fn pid(&self) -> Pid {
        self.pid
    }

    /// Borrows the exact pidfd without transferring consuming-wait ownership.
    pub fn pidfd(&self) -> Option<&OwnedFd> {
        self.custody.pidfd.as_ref()
    }

    /// Test observation of the retained inherited-lock obligation, without releasing it.
    #[cfg(any(test, feature = "test-support"))]
    pub fn retains_spawn_lease(&self) -> bool {
        self.custody.spawn_lease.is_some()
    }

    /// Returns the most recent root-cleanup errno, retained across successful calls.
    ///
    /// This includes signal `SRCH`. Missing-pidfd quarantine invents no errno.
    /// A recorded ownership loss reports `CHILD` before the next cleanup step.
    pub fn last_errno(&self) -> Option<Errno> {
        self.custody.last_errno()
    }

    /// Records `ECHILD` from a foreground observation or consuming wait.
    ///
    /// The next cleanup step quarantines without signaling or waiting. This
    /// shared-reference notification releases no resources and preserves an
    /// already-confirmed terminal reap.
    pub fn ownership_lost(&self) {
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
    /// this immediately after the wait. A retained domain must still complete
    /// before dropping this record or completing its slot.
    pub(crate) fn terminal_reaped(&mut self) {
        self.custody.terminal_reaped();
    }

    /// Attempts at most one pidfd KILL followed by one consuming NOHANG wait.
    ///
    /// There is no retry loop, allocation, or raw-PID fallback. Pending and
    /// quarantined records retain their descriptor and any unverified spawn
    /// lease. A retained domain receives at most one additional finite step.
    /// Root reaping stops root I/O, but only complete cleanup releases an
    /// unverified spawn lease. Repeated complete calls perform no I/O.
    pub fn step(&mut self) -> CleanupPollV1 {
        self.custody.step(&mut PidfdCleanupSyscallsV1)
    }
}

impl Drop for ChildCleanupV1 {
    fn drop(&mut self) {
        if !self.custody.complete() {
            // Losing the outer owner cannot retire the namespace while domain
            // descendants may survive. Reachable progress still requires its slot.
            std::mem::forget(self.namespace.take());
        }
    }
}

/// A reserved slot can retain a created domain even when no child was created.
/// Neither variant supplies launch or isolation authority.
pub(crate) enum CleanupRecordV1 {
    Child(ChildCleanupV1),
    UnspawnedDomain(UnspawnedDomainCleanupV1),
}

impl CleanupRecordV1 {
    pub(crate) fn unspawned_domain(domain: NativeCgroupDomainV1) -> Self {
        Self::UnspawnedDomain(UnspawnedDomainCleanupV1 {
            custody: CleanupCustodyV1::unspawned_domain(domain),
        })
    }

    pub(crate) fn step(&mut self) -> CleanupPollV1 {
        match self {
            Self::Child(child) => child.step(),
            Self::UnspawnedDomain(domain) => domain.custody.step(&mut PidfdCleanupSyscallsV1),
        }
    }
}

pub(crate) struct UnspawnedDomainCleanupV1 {
    custody: CleanupCustodyV1<OwnedFd, ArtifactProcessSpawnLeaseV1, NativeCgroupDomainV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupPhaseV1 {
    /// Domain rollback before any child was created, not a fabricated root reap.
    Unspawned,
    KillRequired,
    AwaitingExit,
    Quarantined,
    /// The direct root was reaped; any retained domain is still independently owed.
    Reaped,
}

// Generic resource ownership lets fake schedules exercise the same lease and
// descriptor retention paths without constructing OS descriptors or children.
struct CleanupCustodyV1<F, L, D = ()> {
    pidfd: Option<F>,
    spawn_lease: Option<L>,
    domain: Option<D>,
    domain_poll: CleanupPollV1,
    phase: CleanupPhaseV1,
    last_errno: Option<Errno>,
    ownership_lost: AtomicBool,
}

impl<F, L, D> CleanupCustodyV1<F, L, D> {
    fn new(pidfd: Option<F>, spawn_lease: Option<L>) -> Self {
        let phase = if pidfd.is_some() {
            CleanupPhaseV1::KillRequired
        } else {
            CleanupPhaseV1::Quarantined
        };
        Self {
            pidfd,
            spawn_lease,
            domain: None,
            domain_poll: CleanupPollV1::Pending,
            phase,
            last_errno: None,
            ownership_lost: AtomicBool::new(false),
        }
    }

    fn with_domain(pidfd: Option<F>, spawn_lease: Option<L>, domain: D) -> Self {
        let mut custody = Self::new(pidfd, spawn_lease);
        custody.domain = Some(domain);
        custody
    }

    fn unspawned_domain(domain: D) -> Self {
        let mut custody = Self::with_domain(None, None, domain);
        custody.phase = CleanupPhaseV1::Unspawned;
        custody
    }

    fn root_complete(&self) -> bool {
        matches!(
            self.phase,
            CleanupPhaseV1::Reaped | CleanupPhaseV1::Unspawned
        )
    }

    fn complete(&self) -> bool {
        self.root_complete() && (self.domain.is_none() || self.domain_poll == CleanupPollV1::Reaped)
    }

    fn last_errno(&self) -> Option<Errno> {
        if self.ownership_lost.load(Ordering::Acquire) {
            Some(Errno::CHILD)
        } else {
            self.last_errno
        }
    }

    fn ownership_lost(&self) {
        if !self.root_complete() {
            self.ownership_lost.store(true, Ordering::Release);
        }
    }

    fn release_spawn_after_exec(&mut self) {
        drop(self.spawn_lease.take());
    }

    fn terminal_reaped(&mut self) {
        self.phase = CleanupPhaseV1::Reaped;
        if self.complete() {
            drop(self.spawn_lease.take());
        }
    }

    fn step(&mut self, syscalls: &mut impl CleanupSyscallsV1<F, D>) -> CleanupPollV1 {
        if self.complete() {
            return CleanupPollV1::Reaped;
        }
        if !self.root_complete() && self.ownership_lost.load(Ordering::Acquire) {
            self.phase = CleanupPhaseV1::Quarantined;
            self.last_errno = Some(Errno::CHILD);
            return CleanupPollV1::Quarantined;
        }
        if self.phase == CleanupPhaseV1::Quarantined
            || self.domain_poll == CleanupPollV1::Quarantined
        {
            return CleanupPollV1::Quarantined;
        }
        if !self.root_complete() && self.step_root(syscalls) == CleanupPollV1::Quarantined {
            return CleanupPollV1::Quarantined;
        }
        if self.domain_poll == CleanupPollV1::Pending {
            if let Some(domain) = self.domain.as_mut() {
                self.domain_poll = syscalls.step_domain(domain);
            }
        }
        if self.domain.is_some() && self.domain_poll != CleanupPollV1::Reaped {
            return self.domain_poll;
        }
        if self.complete() {
            drop(self.spawn_lease.take());
            CleanupPollV1::Reaped
        } else {
            CleanupPollV1::Pending
        }
    }

    fn step_root(&mut self, syscalls: &mut impl CleanupSyscallsV1<F, D>) -> CleanupPollV1 {
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

impl<F, L, D> Drop for CleanupCustodyV1<F, L, D> {
    fn drop(&mut self) {
        if !self.complete() {
            // An owner dropped before transfer must not falsely release the
            // pre-exec obligation. This fail-closed leak cannot provide progress.
            std::mem::forget(self.pidfd.take());
            std::mem::forget(self.spawn_lease.take());
            std::mem::forget(self.domain.take());
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

trait CleanupSyscallsV1<F, D = ()> {
    fn kill(&mut self, pidfd: &F) -> rustix::io::Result<()>;
    fn wait_exited_nohang(&mut self, pidfd: &F) -> rustix::io::Result<Option<CleanupWaitV1>>;
    fn step_domain(&mut self, domain: &mut D) -> CleanupPollV1;
}

struct PidfdCleanupSyscallsV1;

impl CleanupSyscallsV1<OwnedFd, NativeCgroupDomainV1> for PidfdCleanupSyscallsV1 {
    fn step_domain(&mut self, domain: &mut NativeCgroupDomainV1) -> CleanupPollV1 {
        domain.step()
    }
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
