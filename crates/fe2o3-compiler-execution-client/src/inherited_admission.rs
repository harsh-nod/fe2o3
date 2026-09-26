//! Fixed-slot retention shared by all client families, without protocol decoding.
use crate::{COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientErrorV1};
use std::{
    io,
    os::fd::{FromRawFd, OwnedFd},
};

pub(super) fn retain_inherited_peer() -> Result<OwnedFd, CompilerExecutionClientErrorV1> {
    let child_fd = COMPILER_EXECUTION_SERVICE_CHILD_FD_V1;
    // SAFETY: F_GETFD inspects only the fixed scalar descriptor.
    let flags = unsafe { libc::fcntl(child_fd, libc::F_GETFD) };
    if flags < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::EBADF) {
            return Err(CompilerExecutionClientErrorV1::MissingInheritedPeer);
        }
        // SAFETY: consume the canonical slot if the kernel still considers it present.
        let _ = unsafe { libc::close(child_fd) };
        return Err(CompilerExecutionClientErrorV1::Descriptor(error));
    }
    if flags & libc::FD_CLOEXEC != 0 {
        // SAFETY: a present but inadmissible canonical descriptor is consumed exactly once.
        if unsafe { libc::close(child_fd) } != 0 {
            return Err(CompilerExecutionClientErrorV1::Descriptor(
                io::Error::last_os_error(),
            ));
        }
        return Err(CompilerExecutionClientErrorV1::InheritedPeerCloseOnExec);
    }
    // SAFETY: F_DUPFD_CLOEXEC consumes one scalar descriptor and returns a distinct owned
    // descriptor on success.
    let retained = unsafe { libc::fcntl(child_fd, libc::F_DUPFD_CLOEXEC, 3) };
    if retained < 0 {
        let error = io::Error::last_os_error();
        // SAFETY: failure to retain does not release the canonical descriptor.
        let _ = unsafe { libc::close(child_fd) };
        return Err(CompilerExecutionClientErrorV1::Descriptor(error));
    }
    // SAFETY: close consumes only the scalar inherited slot and reports absence through EBADF.
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
    pub(super) fn new() -> Self {
        Self(true)
    }

    pub(super) fn retain(mut self) -> Result<OwnedFd, CompilerExecutionClientErrorV1> {
        // The shared helper owns all consumption paths once entered. Do not close
        // the fixed number again after it has been released and could be reused.
        self.0 = false;
        retain_inherited_peer()
    }
}

impl Drop for PendingInheritedPeer {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: this call consumes only the reserved child slot, including
            // on resource denial. Never retry close or inspect an uncharged input.
            let _ = unsafe { libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1) };
        }
    }
}
