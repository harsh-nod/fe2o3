//! Inert Cargo-created application channel; creator credentials are not sender authentication.

use std::fmt;
use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};

use rustix::fs::OFlags;
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType};

use crate::{COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientProcessIdentityV1};

#[derive(Debug)]
pub enum ApplicationProofChannelErrorV1 {
    Io(rustix::io::Errno),
    Invalid(&'static str),
    Process(fe2o3_process_identity::pidfd::PidfdObservationErrorV1),
    Timeout,
}

impl From<rustix::io::Errno> for ApplicationProofChannelErrorV1 {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for ApplicationProofChannelErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "application proof channel: {error}"),
            Self::Invalid(reason) => write!(f, "invalid application proof channel: {reason}"),
            Self::Process(error) => write!(f, "application root process: {error}"),
            Self::Timeout => f.write_str("application registration deadline expired"),
        }
    }
}

impl std::error::Error for ApplicationProofChannelErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Process(error) => Some(error),
            _ => None,
        }
    }
}

impl From<fe2o3_process_identity::pidfd::PidfdObservationErrorV1>
    for ApplicationProofChannelErrorV1
{
    fn from(error: fe2o3_process_identity::pidfd::PidfdObservationErrorV1) -> Self {
        Self::Process(error)
    }
}

mod registration;
pub use registration::RegisteredApplicationProofEndpointV1;

type Result<T> = std::result::Result<T, ApplicationProofChannelErrorV1>;

fn invalid(reason: &'static str) -> ApplicationProofChannelErrorV1 {
    ApplicationProofChannelErrorV1::Invalid(reason)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EndpointSnapshot {
    object: (u64, u64, u32),
    creator: CompilerExecutionClientProcessIdentityV1,
    local: SocketAddrUnix,
    remote: SocketAddrUnix,
}

impl EndpointSnapshot {
    fn inspect(fd: impl AsFd) -> Result<Self> {
        let fd = fd.as_fd();
        let flags = rustix::fs::fcntl_getfl(fd)?;
        if rustix::io::fcntl_getfd(fd)? != rustix::io::FdFlags::CLOEXEC
            || flags & OFlags::ACCMODE != OFlags::RDWR
            || !flags.contains(OFlags::NONBLOCK)
            || flags.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT)
            || rustix::net::sockopt::socket_domain(fd)? != AddressFamily::UNIX
            || rustix::net::sockopt::socket_type(fd)? != SocketType::SEQPACKET
            || rustix::net::sockopt::socket_acceptconn(fd)?
            || !rustix::net::sockopt::socket_passcred(fd)?
        {
            return Err(invalid(
                "expected nonblocking CLOEXEC PASSCRED Unix seqpacket",
            ));
        }
        let local = SocketAddrUnix::try_from(rustix::net::getsockname(fd)?)?;
        let remote = SocketAddrUnix::try_from(
            rustix::net::getpeername(fd)?.ok_or_else(|| invalid("endpoint is disconnected"))?,
        )?;
        if local.abstract_name().is_none() || remote.abstract_name().is_none() || local == remote {
            return Err(invalid("endpoints need distinct abstract addresses"));
        }
        let credentials = rustix::net::sockopt::socket_peercred(fd)?;
        let creator = CompilerExecutionClientProcessIdentityV1::new(
            u32::try_from(credentials.pid.as_raw_nonzero().get())
                .map_err(|_| invalid("invalid creator PID"))?,
            credentials.uid.as_raw(),
            credentials.gid.as_raw(),
        )
        .map_err(|_| invalid("invalid creator credentials"))?;
        let stat = rustix::fs::fstat(fd)?;
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::Socket {
            return Err(invalid("endpoint is not a socket object"));
        }
        Ok(Self {
            object: (stat.st_dev, stat.st_ino, stat.st_mode),
            creator,
            local,
            remote,
        })
    }
}

fn current_creator(pid: u32) -> Result<CompilerExecutionClientProcessIdentityV1> {
    CompilerExecutionClientProcessIdentityV1::new(
        pid,
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .map_err(|_| invalid("invalid current process credentials"))
}

/// One prefork pair. Neither endpoint is root-authenticated or proof-authorizing.
///
/// Keep this owner alive through spawn, then immediately consume `after_spawn` even on
/// later admission failure. Do not retain an application-side alias in `Command`.
pub struct PreparedApplicationProofChannelV1 {
    child: OwnedFd,
    peer: ApplicationProofTransferPeerV1,
    setup: ApplicationProofChildSetupV1,
}

impl PreparedApplicationProofChannelV1 {
    pub fn prepare() -> Result<Self> {
        let (child, peer) = rustix::net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )?;
        let child = outside_reserved_slots(child)?;
        let peer = outside_reserved_slots(peer)?;
        for fd in [&child, &peer] {
            rustix::net::sockopt::set_socket_passcred(fd, true)?;
            // Freeze Linux's autobind addresses before either endpoint can leave Cargo.
            rustix::net::bind(fd, &SocketAddrUnix::new_unnamed())?;
        }
        let child_snapshot = EndpointSnapshot::inspect(&child)?;
        let peer_snapshot = EndpointSnapshot::inspect(&peer)?;
        if child_snapshot.creator != current_creator(std::process::id())?
            || child_snapshot.creator != peer_snapshot.creator
            || child_snapshot.local != peer_snapshot.remote
            || child_snapshot.remote != peer_snapshot.local
            || child_snapshot.object == peer_snapshot.object
        {
            return Err(invalid(
                "prepared pair creator or endpoint association differs",
            ));
        }
        let child_descriptor = child.as_raw_fd();
        let child_object = child_snapshot.object;
        Ok(Self {
            setup: ApplicationProofChildSetupV1 {
                descriptor: child.as_raw_fd(),
                snapshot: child_snapshot,
            },
            child,
            peer: ApplicationProofTransferPeerV1 {
                peer,
                snapshot: peer_snapshot,
                child_descriptor,
                child_object,
            },
        })
    }

    pub fn child_setup(&self) -> ApplicationProofChildSetupV1 {
        self.setup.clone()
    }

    /// Closes Cargo's application-side alias; the remaining peer grants no registered custody.
    pub fn after_spawn(self) -> ApplicationProofTransferPeerV1 {
        drop(self.child);
        self.peer
    }
}

fn outside_reserved_slots(fd: OwnedFd) -> Result<OwnedFd> {
    if fd.as_raw_fd() <= 2 || fd.as_raw_fd() == COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 {
        Ok(rustix::io::fcntl_dupfd_cloexec(
            &fd,
            COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 + 1,
        )?)
    } else {
        Ok(fd)
    }
}

/// Non-owning child setup facts. They cannot retain an endpoint or authenticate registration.
#[derive(Clone, Debug)]
pub struct ApplicationProofChildSetupV1 {
    descriptor: RawFd,
    snapshot: EndpointSnapshot,
}

impl ApplicationProofChildSetupV1 {
    pub const fn descriptor(&self) -> RawFd {
        self.descriptor
    }
    pub const fn descriptor_identity(&self) -> (u64, u64, u32) {
        self.snapshot.object
    }

    /// Validates the original child object and exposes it after the global CLOEXEC sweep.
    ///
    /// # Safety
    /// Call only in the single-threaded post-fork, pre-exec child, while the prepared pair
    /// still owns this descriptor and no concurrent descriptor-table mutation is possible.
    pub unsafe fn expose_before_exec(&self) -> io::Result<()> {
        // SAFETY: scalar observation rejects a stale number before making a borrowed descriptor.
        if unsafe { libc::fcntl(self.descriptor, libc::F_GETFD) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the caller guarantees unique post-fork descriptor-table access.
        let fd = unsafe { BorrowedFd::borrow_raw(self.descriptor) };
        let observed = EndpointSnapshot::inspect(fd)
            .map_err(|_| io::Error::from_raw_os_error(libc::ESTALE))?;
        if observed != self.snapshot {
            return Err(io::Error::from_raw_os_error(libc::ESTALE));
        }
        rustix::io::fcntl_setfd(fd, rustix::io::FdFlags::empty()).map_err(io::Error::from)
    }
}

/// Move-only Cargo-side peer, constructible only by consuming an original prepared pair.
///
/// ```compile_fail
/// fn cloneable<T: Clone>() {}
/// cloneable::<fe2o3_compiler_execution_client::ApplicationProofTransferPeerV1>();
/// ```
/// ```compile_fail
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<fe2o3_compiler_execution_client::ApplicationProofTransferPeerV1>();
/// ```
#[derive(Debug)]
pub struct ApplicationProofTransferPeerV1 {
    peer: OwnedFd,
    snapshot: EndpointSnapshot,
    child_descriptor: RawFd,
    child_object: (u64, u64, u32),
}

impl ApplicationProofTransferPeerV1 {
    pub fn revalidate(&self) -> Result<()> {
        if self.snapshot != EndpointSnapshot::inspect(&self.peer)?
            || self.snapshot.creator != current_creator(std::process::id())?
        {
            return Err(invalid("Cargo-side endpoint continuity changed"));
        }
        Ok(())
    }

    pub(crate) fn into_registration_descriptor(
        self,
        binding: &fe2o3_runtime_protocol::WorkerV3ApplicationRegistrationBindingV1,
    ) -> Result<OwnedFd> {
        self.revalidate()?;
        let (device, inode, mode) = self.child_object;
        let input =
            fe2o3_runtime_protocol::WorkerV3ApplicationInputOccurrenceV1::from_linux_descriptor_v1(
                4, device, inode, mode,
            )
            .map_err(|_| invalid("invalid original application endpoint occurrence"))?;
        if binding.descriptors().as_array()[3] != self.child_descriptor
            || binding.occurrence().inputs().get(3) != Some(&input)
        {
            return Err(invalid(
                "binding does not name this pair's original child endpoint",
            ));
        }
        Ok(self.peer)
    }
}

/// Claimed application endpoint retained through unload. This is transport, not a proof session.
#[derive(Debug)]
pub struct RetainedApplicationProofEndpointV1 {
    peer: OwnedFd,
    snapshot: EndpointSnapshot,
    application_pid: u32,
}

impl RetainedApplicationProofEndpointV1 {
    /// Admits an already-claimed CLOEXEC descriptor created by this application's current parent.
    /// Creator credentials do not authenticate the root process's later messages.
    pub fn admit_inherited(peer: OwnedFd) -> Result<Self> {
        let value = Self {
            snapshot: EndpointSnapshot::inspect(&peer)?,
            peer,
            application_pid: std::process::id(),
        };
        value.revalidate()?;
        Ok(value)
    }

    pub fn revalidate(&self) -> Result<()> {
        let parent =
            rustix::process::getppid().ok_or_else(|| invalid("application has no parent"))?;
        if self.application_pid != std::process::id()
            || self.snapshot.creator != current_creator(parent.as_raw_nonzero().get() as u32)?
            || self.snapshot != EndpointSnapshot::inspect(&self.peer)?
        {
            return Err(invalid("inherited endpoint or Cargo creator changed"));
        }
        Ok(())
    }

    pub const fn descriptor_identity(&self) -> (u64, u64, u32) {
        self.snapshot.object
    }
}

#[cfg(test)]
mod tests;
