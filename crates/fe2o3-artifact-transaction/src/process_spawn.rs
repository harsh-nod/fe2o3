//! Shared coordination of process creation and artifact-lock descriptor release.

use std::fmt;
use std::process;
use std::sync::{Condvar, Mutex, MutexGuard, TryLockError};

struct ArtifactProcessSpawnStateV1 {
    pid: u32,
    active_spawns: u64,
    active_releases: u64,
    retirement: bool,
}

impl ArtifactProcessSpawnStateV1 {
    fn reset_after_fork(&mut self) {
        let pid = process::id();
        if self.pid != pid {
            self.pid = pid;
            self.active_spawns = 0;
            self.active_releases = 0;
            self.retirement = false;
        }
    }
}

pub(crate) struct ArtifactProcessSpawnCoordinatorV1 {
    state: Mutex<ArtifactProcessSpawnStateV1>,
    idle: Condvar,
}

impl ArtifactProcessSpawnCoordinatorV1 {
    pub(crate) fn global() -> &'static Self {
        // A const initializer avoids a hidden OnceLock wait in nonblocking retirement admission.
        static COORDINATOR: ArtifactProcessSpawnCoordinatorV1 = ArtifactProcessSpawnCoordinatorV1 {
            state: Mutex::new(ArtifactProcessSpawnStateV1 {
                pid: 0,
                active_spawns: 0,
                active_releases: 0,
                retirement: false,
            }),
            idle: Condvar::new(),
        };
        &COORDINATOR
    }

    fn state(&self) -> MutexGuard<'_, ArtifactProcessSpawnStateV1> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.reset_after_fork();
        state
    }

    fn try_begin_spawn(
        &'static self,
    ) -> Result<ArtifactProcessSpawnLeaseV1, ArtifactProcessSpawnLeaseErrorV1> {
        let mut state = self.state();
        loop {
            if state.retirement {
                return Err(ArtifactProcessSpawnLeaseErrorV1::RetirementInProgress);
            }
            if state.active_releases == 0 {
                return self.admit_spawn(&mut state);
            }
            state = self
                .idle
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    fn admit_spawn(
        &'static self,
        state: &mut ArtifactProcessSpawnStateV1,
    ) -> Result<ArtifactProcessSpawnLeaseV1, ArtifactProcessSpawnLeaseErrorV1> {
        state.active_spawns = state
            .active_spawns
            .checked_add(1)
            .ok_or(ArtifactProcessSpawnLeaseErrorV1::CountOverflow)?;
        Ok(ArtifactProcessSpawnLeaseV1 {
            coordinator: self,
            origin_pid: state.pid,
        })
    }

    pub(crate) fn begin_spawn(&'static self) -> ArtifactProcessSpawnLeaseV1 {
        let mut state = self.state();
        while state.retirement || state.active_releases != 0 {
            state = self
                .idle
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        self.admit_spawn(&mut state)
            .expect("concurrent artifact process spawn count overflowed")
    }

    fn try_begin_retirement(
        &'static self,
    ) -> Result<ArtifactLockRetirementBarrierV1, ArtifactLockRetirementBarrierErrorV1> {
        let mut state = match self.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::Poisoned(error)) => error.into_inner(),
            Err(TryLockError::WouldBlock) => {
                return Err(ArtifactLockRetirementBarrierErrorV1::Busy);
            }
        };
        state.reset_after_fork();
        if state.active_spawns != 0 || state.active_releases != 0 || state.retirement {
            return Err(ArtifactLockRetirementBarrierErrorV1::Busy);
        }
        state.retirement = true;
        Ok(ArtifactLockRetirementBarrierV1 {
            coordinator: self,
            origin_pid: state.pid,
        })
    }

    pub(crate) fn release_lock_descriptors(&self, release: impl FnOnce()) {
        let mut state = self.state();
        while state.active_spawns != 0 {
            state = self
                .idle
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        // Count actual descriptor closing independently of the caller's barrier. Even if that
        // barrier is dropped on another thread, a new spawn must wait for the close to finish.
        state.active_releases = state
            .active_releases
            .checked_add(1)
            .expect("concurrent artifact lock release count overflowed");
        let _release = ArtifactLockDescriptorReleaseV1 {
            coordinator: self,
            origin_pid: state.pid,
        };
        drop(state);
        release();
    }
}

struct ArtifactLockDescriptorReleaseV1<'a> {
    coordinator: &'a ArtifactProcessSpawnCoordinatorV1,
    origin_pid: u32,
}

impl Drop for ArtifactLockDescriptorReleaseV1<'_> {
    fn drop(&mut self) {
        if self.origin_pid != process::id() {
            return;
        }
        let mut state = self.coordinator.state();
        state.active_releases = state
            .active_releases
            .checked_sub(1)
            .expect("artifact lock release count underflowed");
        if state.active_releases == 0 {
            self.coordinator.idle.notify_all();
        }
    }
}

/// Fixed, allocation-free refusal to acquire an artifact process-spawn lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactProcessSpawnLeaseErrorV1 {
    /// The shared coordinator cannot count another active lease.
    CountOverflow,
    /// An explicit artifact retirement barrier excludes new spawns. Retry after retirement.
    RetirementInProgress,
}

impl fmt::Display for ArtifactProcessSpawnLeaseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountOverflow => {
                formatter.write_str("concurrent artifact process spawn count overflowed")
            }
            Self::RetirementInProgress => {
                formatter.write_str("artifact lock retirement in progress")
            }
        }
    }
}

impl std::error::Error for ArtifactProcessSpawnLeaseErrorV1 {}

/// Fixed, allocation-free refusal to acquire an artifact lock retirement barrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactLockRetirementBarrierErrorV1 {
    /// A spawn, descriptor release, another barrier, or mutex contention prevents admission.
    Busy,
}

impl fmt::Display for ArtifactLockRetirementBarrierErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("artifact lock retirement coordinator busy")
    }
}

impl std::error::Error for ArtifactLockRetirementBarrierErrorV1 {}

/// Move-only exclusion of process creation while actual artifact lock owners are destroyed.
///
/// Admission proves only that this process's shared coordinator has no outstanding spawn
/// leases. It does not prove compiler termination, durable publication, or authority to retire
/// any owner. The caller must retain the actual owners on Busy/error/unwind until its own
/// retirement conditions hold. Keep this barrier until their destructors have returned, and
/// never invoke process creation while holding it. Ordinary lock Drop then has no spawn lease
/// to wait for. No coordinator mutex is held by this value or across descriptor destruction.
///
/// This allocation-free mechanical primitive has no resource ledger. The cleanup custodian
/// must prepay each attempt, this value's storage, and actual owner destruction on its original
/// account before acquiring it. Finish fallible accounting/validation before dropping custody;
/// dropping the barrier neither touches that account nor releases any artifact owner itself.
///
/// Moving to another thread preserves exclusion in the originating process. Drop takes only
/// the coordinator mutex for bookkeeping, never waits for a spawn/descriptor release, and
/// wakes waiting legacy spawns. This is not a wall-clock scheduling guarantee. A forked copy's
/// Drop checks its origin before touching the inherited mutex; pre-exec child code must not
/// re-enter artifact APIs.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::ArtifactLockRetirementBarrierV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ArtifactLockRetirementBarrierV1>();
/// ```
#[must_use = "retain across destruction of the actual artifact lock owners"]
pub struct ArtifactLockRetirementBarrierV1 {
    coordinator: &'static ArtifactProcessSpawnCoordinatorV1,
    origin_pid: u32,
}

impl ArtifactLockRetirementBarrierV1 {
    pub(crate) fn guards_artifact_locks(&self) -> bool {
        self.origin_pid == process::id()
            && std::ptr::eq(
                self.coordinator,
                ArtifactProcessSpawnCoordinatorV1::global(),
            )
    }
}

impl fmt::Debug for ArtifactLockRetirementBarrierV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArtifactLockRetirementBarrierV1")
            .field("origin_pid", &self.origin_pid)
            .finish_non_exhaustive()
    }
}

impl Drop for ArtifactLockRetirementBarrierV1 {
    fn drop(&mut self) {
        if self.origin_pid != process::id() {
            return;
        }
        let mut state = self.coordinator.state();
        state.retirement = false;
        self.coordinator.idle.notify_all();
    }
}

/// Tries once to exclude new process creation using the actual artifact-lock coordinator.
///
/// Unlike checked spawn-lease admission, this uses `try_lock` and never waits. Busy leaves
/// coordinator obligations unchanged. No owner is accepted, extracted, or destroyed here.
/// On success, retain the returned barrier across actual lease/token destruction outside all
/// cleanup-pool mutexes. Ordinary artifact locking/currentness checks are unchanged.
pub fn try_acquire_artifact_lock_retirement_barrier_v1()
-> Result<ArtifactLockRetirementBarrierV1, ArtifactLockRetirementBarrierErrorV1> {
    ArtifactProcessSpawnCoordinatorV1::global().try_begin_retirement()
}

/// Move-only custody of one obligation in the shared artifact process-spawn coordinator.
///
/// Acquire before creating a child. Retain this lease until confirmed successful exec has closed
/// the inherited `CLOEXEC` artifact-lock aliases, or terminal disposal has closed them. If no
/// child was created, it may be dropped immediately. A supervisor must transfer it to deferred
/// cleanup whenever returning or unwinding leaves that obligation outstanding. Moving the lease
/// to another thread in the originating process preserves the same count; dropping it releases
/// exactly that count. This grants no execution authority and is not evidence of exec or disposal.
///
/// Do not release artifact locks while retaining this lease on the same cleanup path: descriptor
/// release waits for all leases, including this one. A forked copy's Drop does nothing and checks
/// its origin before touching the inherited mutex. Pre-exec child code must still not re-enter
/// artifact APIs. Acquiring or dropping a lease can wait for the coordinator mutex; the existing
/// artifact-lock release wait is not made bounded by transferring custody.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ArtifactProcessSpawnLeaseV1>();
/// ```
#[must_use = "retain until confirmed exec/CLOEXEC or terminal disposal of the child"]
pub struct ArtifactProcessSpawnLeaseV1 {
    coordinator: &'static ArtifactProcessSpawnCoordinatorV1,
    origin_pid: u32,
}

impl fmt::Debug for ArtifactProcessSpawnLeaseV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArtifactProcessSpawnLeaseV1")
            .field("origin_pid", &self.origin_pid)
            .finish_non_exhaustive()
    }
}

impl Drop for ArtifactProcessSpawnLeaseV1 {
    fn drop(&mut self) {
        // A fork may copy a mutex held by a vanished thread. Check before even locking it.
        if self.origin_pid != process::id() {
            return;
        }
        let mut state = self.coordinator.state();
        state.active_spawns = state
            .active_spawns
            .checked_sub(1)
            .expect("artifact process spawn lease underflowed");
        if state.active_spawns == 0 {
            self.coordinator.idle.notify_all();
        }
    }
}

/// Acquires transferable spawn custody from the same coordinator used by artifact lock release.
///
/// Count overflow or an explicit retirement barrier returns a fixed error without changing
/// the count. This may wait for the coordinator mutex or ordinary descriptor closing; `try`
/// refers to checked admission, not a nonblocking mutex acquisition.
/// The caller must uphold the retention obligation of [`ArtifactProcessSpawnLeaseV1`].
pub fn try_acquire_artifact_process_spawn_lease_v1()
-> Result<ArtifactProcessSpawnLeaseV1, ArtifactProcessSpawnLeaseErrorV1> {
    ArtifactProcessSpawnCoordinatorV1::global().try_begin_spawn()
}

/// Runs one process creation operation without exposing inherited artifact-lock aliases.
///
/// On Linux, a child temporarily retains the parent's `CLOEXEC` OFD and `flock` descriptors
/// between `fork` and `exec`. Every process creation in a process that uses this crate's artifact
/// transactions must coordinate its `Command::spawn` through this function or retain an
/// [`ArtifactProcessSpawnLeaseV1`]. Artifact lock release then waits for all coordinated children
/// to exec or fail, ensuring that a child can never become the sole owner of an inherited lock
/// alias. Lock acquisition remains nonblocking and reports only genuine lock contention.
///
/// This wrapper releases its lease on return or unwind. If cleanup can outlive either, use
/// [`try_acquire_artifact_process_spawn_lease_v1`] and transfer the lease to that custodian.
/// This waits for any retirement barrier to be dropped before invoking `spawn`, preserving the
/// wrapper's return/error behavior. Like the original wrapper, count overflow panics. Do not
/// invoke it on the path responsible for dropping an outstanding retirement barrier.
pub fn with_artifact_process_spawn_v1<T, E>(spawn: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let _spawn = ArtifactProcessSpawnCoordinatorV1::global().begin_spawn();
    spawn()
}

/// Compatibility entry point for test fixtures that predate production spawn coordination.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
pub fn with_test_artifact_fork_exec_barrier_v1<T, E>(
    spawn: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    with_artifact_process_spawn_v1(spawn)
}

#[cfg(test)]
#[path = "process_spawn_tests.rs"]
mod tests;
