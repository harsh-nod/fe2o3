//! Dedicated descriptor custody and actual-parent observations, not root admission.
use super::{Budget, Error, Result, launch_io};
use rustix::{fs::OFlags, io::FdFlags, net, process::Pid};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};

pub(super) const WORK: usize = 64 * (1024 + 64);
const PARENT_WORK: usize = 8 * (1024 + 64);

fn os_error() -> Error {
    rustix::io::Errno::from_raw_os_error(
        std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO),
    )
    .into()
}

pub(super) struct Source(bool);
impl Source {
    /// Caller transfers exclusive raw FD 3 cleanup before any fallible operation.
    pub(super) unsafe fn take() -> Self {
        Self(true)
    }
    pub(super) fn validate(&self) -> Result<()> {
        // SAFETY: scalar F_GETFD permits an absent slot; no Rust owner is invented.
        let flags = unsafe { libc::fcntl(3, libc::F_GETFD) };
        if flags < 0 {
            return Err(os_error());
        }
        if flags != 0 {
            return Err(Error::Invalid("helper bootstrap was not inherited"));
        }
        Ok(())
    }
    pub(super) fn into_owned(mut self) -> Result<OwnedFd> {
        self.validate()?;
        // SAFETY: validated live descriptor, exclusive raw custody until duplication.
        let source = unsafe { BorrowedFd::borrow_raw(3) };
        let retained = rustix::io::fcntl_dupfd_cloexec(source, 256)?;
        self.0 = false;
        // SAFETY: custody is cleared before the single close; never retry close.
        if unsafe { libc::close(3) } != 0 {
            return Err(os_error());
        }
        Ok(retained)
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: exclusive raw startup custody. Linux close is never retried.
            unsafe {
                libc::close(3);
            }
        }
    }
}

/// Only before descriptor-owning admission, in the exclusive dedicated process.
pub(super) unsafe fn close_unrelated() -> Result<()> {
    for (first, last) in [(0u32, 2u32), (4, u32::MAX)] {
        // SAFETY: caller owns the complete otherwise-unused descriptor table.
        if unsafe { libc::syscall(libc::SYS_close_range, first, last, 0) } != 0 {
            return Err(os_error());
        }
    }
    Ok(())
}

pub(super) struct Parent {
    pid: Pid,
    pidfd: OwnedFd,
}
impl Parent {
    pub(super) fn capture(bootstrap: &OwnedFd) -> Result<Self> {
        let pid = rustix::process::getppid().ok_or(Error::Invalid("helper parent absent"))?;
        if pid.as_raw_pid() <= 1 {
            return Err(Error::Invalid("helper creator absent"));
        }
        validate_bootstrap(bootstrap, pid)?;
        // Open for our actual parent, not a peer-supplied PID or descriptor.
        // Recheck PPID before and after poll so parent exit/reparenting refuses.
        let pidfd = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())?;
        let parent = Self { pid, pidfd };
        parent.check()?;
        Ok(parent)
    }
    pub(super) fn pid(&self) -> u32 {
        self.pid.as_raw_pid() as u32
    }
    pub(super) fn sender(&self) -> launch_io::MessageSender {
        launch_io::MessageSender::new(self.pid.as_raw_pid(), 0, 0)
    }
    fn check(&self) -> Result<()> {
        if rustix::process::getppid() != Some(self.pid) {
            return Err(Error::Invalid("helper creator changed"));
        }
        let mut poll = libc::pollfd {
            fd: self.pidfd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized pollfd, zero timeout and no retry.
        let count = unsafe { libc::poll(&mut poll, 1, 0) };
        if count < 0 {
            return Err(os_error());
        }
        if count != 0 || poll.revents != 0 || rustix::process::getppid() != Some(self.pid) {
            return Err(Error::Invalid("helper creator is no longer live"));
        }
        Ok(())
    }
}

fn validate_bootstrap(fd: &OwnedFd, parent: Pid) -> Result<()> {
    let flags = rustix::fs::fcntl_getfl(fd)?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH;
    if rustix::io::fcntl_getfd(fd)? != FdFlags::CLOEXEC
        || flags & OFlags::ACCMODE != OFlags::RDWR
        || !flags.contains(OFlags::NONBLOCK)
        || flags.intersects(forbidden)
        || net::sockopt::socket_domain(fd)? != net::AddressFamily::UNIX
        || net::sockopt::socket_type(fd)? != net::SocketType::SEQPACKET
        || net::sockopt::socket_acceptconn(fd)?
        || !net::sockopt::socket_passcred(fd)?
    {
        return Err(Error::Invalid("invalid helper bootstrap shape"));
    }
    let unnamed = net::SocketAddrAny::from(net::SocketAddrUnix::new_unnamed());
    let peer = net::sockopt::socket_peercred(fd)?;
    if net::getsockname(fd)? != unnamed
        || net::getpeername(fd)?.as_ref() != Some(&unnamed)
        || net::sockopt::socket_error(fd)?.is_err()
        || peer.pid != parent
        || !peer.uid.is_root()
        || !peer.gid.is_root()
    {
        return Err(Error::Invalid(
            "helper bootstrap is not from actual root parent",
        ));
    }
    Ok(())
}

pub(super) struct Observer<'a, 'w> {
    pub parent: &'a Parent,
    pub budget: &'a mut Budget<'w>,
}
impl Observer<'_, '_> {
    pub(super) fn check(&mut self) -> Result<()> {
        self.budget.charge_work(PARENT_WORK)?;
        self.parent.check()
    }
}
impl launch_io::Observer for Observer<'_, '_> {
    type Error = Error;
    fn before_attempt(&mut self, boundary: launch_io::Boundary) -> Result<()> {
        self.budget.charge_work(boundary.work())?;
        self.check()
    }
    fn is_live(&mut self) -> Result<bool> {
        self.check().map(|()| true)
    }
}

#[cfg(test)]
#[path = "proof_executor_helper_io_v1_tests.rs"]
mod tests;
