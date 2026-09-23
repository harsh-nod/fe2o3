//! Shared coordination of process creation and artifact-lock descriptor release.

use std::fmt;
use std::process;
use std::sync::{Condvar, Mutex, OnceLock};

struct ArtifactProcessSpawnStateV1 {
    pid: u32,
    active_spawns: u64,
}

pub(crate) struct ArtifactProcessSpawnCoordinatorV1 {
    state: Mutex<ArtifactProcessSpawnStateV1>,
    idle: Condvar,
}

impl ArtifactProcessSpawnCoordinatorV1 {
    pub(crate) fn global() -> &'static Self {
        static COORDINATOR: OnceLock<ArtifactProcessSpawnCoordinatorV1> = OnceLock::new();
        COORDINATOR.get_or_init(|| Self {
            state: Mutex::new(ArtifactProcessSpawnStateV1 {
                pid: process::id(),
                active_spawns: 0,
            }),
            idle: Condvar::new(),
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ArtifactProcessSpawnStateV1> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let pid = process::id();
        if state.pid != pid {
            state.pid = pid;
            state.active_spawns = 0;
        }
        state
    }

    fn try_begin_spawn(
        &'static self,
    ) -> Result<ArtifactProcessSpawnLeaseV1, ArtifactProcessSpawnLeaseErrorV1> {
        let mut state = self.state();
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
        self.try_begin_spawn()
            .expect("concurrent artifact process spawn count overflowed")
    }

    pub(crate) fn release_lock_descriptors(&self, release: impl FnOnce()) {
        let mut state = self.state();
        while state.active_spawns != 0 {
            state = self
                .idle
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        // Keep the state lock held while descriptors close so a new child cannot inherit them.
        release();
    }
}

/// Fixed, allocation-free refusal to acquire an artifact process-spawn lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactProcessSpawnLeaseErrorV1 {
    /// The shared coordinator cannot count another active lease.
    CountOverflow,
}

impl fmt::Display for ArtifactProcessSpawnLeaseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountOverflow => {
                formatter.write_str("concurrent artifact process spawn count overflowed")
            }
        }
    }
}

impl std::error::Error for ArtifactProcessSpawnLeaseErrorV1 {}

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
/// Count overflow returns a fixed error without changing the count. This may wait for the
/// coordinator mutex; `try` refers to checked admission, not a nonblocking mutex acquisition.
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
/// Like the original wrapper, this panics if the active spawn count overflows.
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
