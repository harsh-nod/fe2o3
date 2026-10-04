use super::{
    CompilerExecutionChildChannelErrorV1, CompilerExecutionClientProcessIdentityV1,
    current_submitter, open_pidfd, require_close_on_exec, require_pidfd_live,
};
use std::fmt;
use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::process::Child;

/// Original child and parent process custody captured immediately after spawn, before reaping.
///
/// This value grants no compilation or execution authority. Keep the `Child` unreaped through
/// channel admission. Cleanup may retain this owner after child exit or reaping, but transfer
/// requires the original child to remain live and waitable by the original parent.
///
/// ```compile_fail
/// fn require_clone<T: Clone>() {}
/// require_clone::<fe2o3_compiler_execution_client::RetainedCompilerExecutionChildV1>();
/// ```
///
/// ```compile_fail
/// fn require_fd<T: std::os::fd::AsFd>() {}
/// require_fd::<fe2o3_compiler_execution_client::RetainedCompilerExecutionChildV1>();
/// ```
pub struct RetainedCompilerExecutionChildV1 {
    child_pidfd: OwnedFd,
    parent_pidfd: OwnedFd,
    child_pid: u32,
    submitter: CompilerExecutionClientProcessIdentityV1,
}

impl fmt::Debug for RetainedCompilerExecutionChildV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedCompilerExecutionChildV1")
            .field("child_pid", &self.child_pid)
            .field("submitter", &self.submitter)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl RetainedCompilerExecutionChildV1 {
    /// Captures both original pidfds without reaping even an already-exited child.
    pub fn capture(child: &Child) -> Result<Self, CompilerExecutionChildChannelErrorV1> {
        if child.id() == 0 || child.id() > i32::MAX as u32 {
            return Err(CompilerExecutionChildChannelErrorV1::InvalidChildPid);
        }
        require_waitable(libc::P_PID, child.id(), child.id())?;
        let submitter = current_submitter()?;
        let value = Self {
            child_pidfd: open_pidfd(child.id())?,
            parent_pidfd: open_pidfd(submitter.pid())?,
            child_pid: child.id(),
            submitter,
        };
        value.validate_custody()?;
        value.require_original_child_waitable()?;
        Ok(value)
    }

    pub const fn child_pid(&self) -> u32 {
        self.child_pid
    }

    pub const fn submitter(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.submitter
    }

    /// Checks descriptor/history custody, not a live child occurrence, during cleanup.
    pub fn validate_custody(&self) -> Result<(), CompilerExecutionChildChannelErrorV1> {
        if current_submitter()? != self.submitter {
            return Err(CompilerExecutionChildChannelErrorV1::ParentCredentialsMismatch);
        }
        require_close_on_exec(&self.child_pidfd)?;
        require_close_on_exec(&self.parent_pidfd)?;
        require_pidfd_live(&self.parent_pidfd)
    }

    pub(super) fn validate_live_transfer(
        &self,
    ) -> Result<(), CompilerExecutionChildChannelErrorV1> {
        self.validate_custody()?;
        self.require_original_child_waitable()?;
        require_pidfd_live(&self.child_pidfd)
    }

    pub(super) fn clone_for_live_transfer(
        &self,
    ) -> Result<OwnedFd, CompilerExecutionChildChannelErrorV1> {
        self.validate_live_transfer()?;
        let duplicate = rustix::io::fcntl_dupfd_cloexec(&self.child_pidfd, 3)
            .map_err(|error| CompilerExecutionChildChannelErrorV1::Descriptor(error.into()))?;
        require_close_on_exec(&duplicate)?;
        require_pidfd_live(&duplicate)?;
        self.validate_live_transfer()?;
        Ok(duplicate)
    }

    fn require_original_child_waitable(&self) -> Result<(), CompilerExecutionChildChannelErrorV1> {
        // P_PIDFD keeps the wait bound to the retained process even after numeric PID reuse.
        require_waitable(
            libc::P_PIDFD,
            self.child_pidfd.as_raw_fd() as libc::id_t,
            self.child_pid,
        )
    }

    /// Inspects the original descriptor only in cross-crate custody tests.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn test_child_pidfd(&self) -> std::os::fd::BorrowedFd<'_> {
        std::os::fd::AsFd::as_fd(&self.child_pidfd)
    }
}

fn require_waitable(
    kind: libc::idtype_t,
    id: libc::id_t,
    expected_pid: u32,
) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    // SAFETY: waitid writes one valid siginfo_t; WNOWAIT prevents consuming exit status.
    if unsafe {
        libc::waitid(
            kind,
            id,
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } != 0
    {
        return Err(CompilerExecutionChildChannelErrorV1::ChildWait(
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: successful waitid initialized the zeroed output. Zero means no exit is pending.
    let observed_pid = unsafe { info.assume_init().si_pid() };
    if observed_pid != 0 && u32::try_from(observed_pid).ok() != Some(expected_pid) {
        return Err(CompilerExecutionChildChannelErrorV1::ChildPidMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
