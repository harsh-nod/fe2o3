//! Fixed-slot retention shared by all client families, without protocol decoding.
use crate::{COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientErrorV1};
use std::{
    io,
    os::fd::{FromRawFd, OwnedFd},
};

/// Consumes the uniquely transferred fixed slot, including on refusal.
///
/// # Safety
/// The caller transfers FD 195 with no other owner or outstanding borrow. No
/// thread, handler, or foreign code may close, replace, or acquire ownership of
/// that slot during this call. If absent, it must remain absent until return.
/// The same transfer must not be consumed again, including after failure.
pub(super) unsafe fn retain_inherited_peer() -> Result<OwnedFd, CompilerExecutionClientErrorV1> {
    let child_fd = COMPILER_EXECUTION_SERVICE_CHILD_FD_V1;
    // SAFETY: F_GETFD inspects only the fixed scalar descriptor.
    let flags = unsafe { libc::fcntl(child_fd, libc::F_GETFD) };
    if flags < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EBADF) {
            return Err(CompilerExecutionClientErrorV1::MissingInheritedPeer);
        }
        // SAFETY: the caller transferred exclusive custody even on flag-query
        // failure. Consume that slot once; never retry close on an error.
        let _ = unsafe { libc::close(child_fd) };
        return Err(CompilerExecutionClientErrorV1::Descriptor(error));
    }
    if flags & libc::FD_CLOEXEC != 0 {
        // SAFETY: the caller transferred sole ownership even for invalid flags.
        if unsafe { libc::close(child_fd) } != 0 {
            return Err(CompilerExecutionClientErrorV1::Descriptor(
                io::Error::last_os_error(),
            ));
        }
        return Err(CompilerExecutionClientErrorV1::InheritedPeerCloseOnExec);
    }
    // SAFETY: the caller keeps the source exclusively owned and live. Duplication
    // returns a distinct owned descriptor without consuming the source.
    let retained = unsafe { libc::fcntl(child_fd, libc::F_DUPFD_CLOEXEC, 3) };
    if retained < 0 {
        let error = io::Error::last_os_error();
        // SAFETY: failed duplication leaves the exclusively transferred slot
        // owned here. Close it once, including on an interrupted close.
        let _ = unsafe { libc::close(child_fd) };
        return Err(CompilerExecutionClientErrorV1::Descriptor(error));
    }
    // SAFETY: consume the caller's exclusively transferred slot once. The private
    // duplicate is distinct, and the canonical number must not be closed again.
    let close_result = unsafe { libc::close(child_fd) };
    if close_result != 0 {
        let error = io::Error::last_os_error();
        // SAFETY: `retained` is the distinct descriptor returned by F_DUPFD_CLOEXEC.
        unsafe { libc::close(retained) };
        return Err(CompilerExecutionClientErrorV1::Descriptor(error));
    }
    // SAFETY: successful F_DUPFD_CLOEXEC returned unique ownership and the error path above
    // closed it before returning.
    let retained = unsafe { OwnedFd::from_raw_fd(retained) };
    Ok(retained)
}

/// A native quota refusal must consume the canonical slot without inspecting it.
pub(super) struct PendingInheritedPeer(bool);

impl PendingInheritedPeer {
    /// # Safety
    /// Transfer the fixed slot under retain_inherited_peer's ownership contract,
    /// which must hold until this guard is consumed or dropped. An absent slot
    /// must remain unallocated for that lifetime. Drop consumes even unpaid input.
    pub(super) unsafe fn new() -> Self {
        Self(true)
    }

    pub(super) fn retain(mut self) -> Result<OwnedFd, CompilerExecutionClientErrorV1> {
        // The shared helper owns all consumption paths once entered. Do not close
        // the fixed number again after it has been released and could be reused.
        self.0 = false;
        // SAFETY: construction transferred exclusive slot custody to this guard;
        // disarming transfers that same custody to the consuming helper exactly once.
        unsafe { retain_inherited_peer() }
    }
}

impl Drop for PendingInheritedPeer {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: the unsafe constructor transferred exclusive slot custody
            // through this drop, including resource denial. Never retry close.
            let _ = unsafe { libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1) };
        }
    }
}
