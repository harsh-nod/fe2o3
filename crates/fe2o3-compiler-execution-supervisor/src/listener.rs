//! Fixed inherited listener admission and one-session service dispatch.

use std::error::Error;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::OFlags;
use rustix::net::{
    AddressFamily, SocketAddrAny, SocketAddrUnix, SocketFlags, SocketType, accept_with,
};

use crate::{
    ExitedProtectedIssuerV1, ProtectedIssuerSessionErrorV1, ProtectedIssuerSessionTimeoutsV1,
    ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1,
};

const MAX_ACCEPT_TIMEOUT_V1: Duration = Duration::from_secs(120);
const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;

/// Failure admitting or using the sole protected issuer service listener.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerServiceErrorV1 {
    /// The inherited listener has the wrong descriptor, transport, address, or filesystem shape.
    InvalidListener(&'static str),
    /// The accepted-control wait has an invalid caller bound.
    InvalidAcceptTimeout,
    /// No connection arrived before the absolute accept deadline.
    AcceptTimeout,
    /// Protected supervisor authority changed before accepting a session.
    Supervisor(ProtectedIssuerSupervisorErrorV1),
    /// One accepted connection failed in its exact lifecycle stage.
    Session(ProtectedIssuerSessionErrorV1),
    /// A bounded listener operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or filesystem failure.
        source: io::Error,
    },
}

impl fmt::Display for ProtectedIssuerServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidListener(reason) => {
                write!(formatter, "invalid protected issuer listener: {reason}")
            }
            Self::InvalidAcceptTimeout => {
                formatter.write_str("invalid protected issuer accept timeout")
            }
            Self::AcceptTimeout => formatter.write_str("protected issuer accept timed out"),
            Self::Supervisor(error) => write!(formatter, "issuer supervisor changed: {error}"),
            Self::Session(error) => write!(formatter, "issuer session failed: {error}"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedIssuerServiceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Supervisor(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::InvalidListener(_) | Self::InvalidAcceptTimeout | Self::AcceptTimeout => None,
        }
    }
}

/// Move-only custody of the one production listener and protected supervisor.
///
/// This value exposes no descriptor, signing operation, process authority,
/// publication authority, loading authority, or GPU authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ProtectedIssuerServiceV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerServiceV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ProtectedIssuerServiceV1>();
/// ```
pub struct ProtectedIssuerServiceV1 {
    supervisor: ProtectedIssuerSupervisorV1,
    listener: ProtectedIssuerListenerV1,
    timeouts: ProtectedIssuerSessionTimeoutsV1,
}

impl fmt::Debug for ProtectedIssuerServiceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedIssuerServiceV1")
            .field("authority", &"fixed-listener-session-dispatch-only")
            .field("timeouts", &self.timeouts)
            .finish_non_exhaustive()
    }
}

impl ProtectedIssuerServiceV1 {
    /// Admits the sole fixed production listener and consumes supervisor custody.
    pub fn bind(
        supervisor: ProtectedIssuerSupervisorV1,
        listener: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        Self::bind_at(
            supervisor,
            listener,
            timeouts,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
        )
    }

    fn bind_at(
        supervisor: ProtectedIssuerSupervisorV1,
        listener: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
        expected_path: &Path,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        supervisor
            .revalidate()
            .map_err(ProtectedIssuerServiceErrorV1::Supervisor)?;
        let listener = ProtectedIssuerListenerV1::admit(listener, expected_path)?;
        supervisor
            .revalidate()
            .map_err(ProtectedIssuerServiceErrorV1::Supervisor)?;
        Ok(Self {
            supervisor,
            listener,
            timeouts,
        })
    }

    /// Accepts and runs one complete production session without exposing its control descriptor.
    pub fn serve_one(
        &self,
        accept_timeout: Duration,
    ) -> Result<ExitedProtectedIssuerV1, ProtectedIssuerServiceErrorV1> {
        self.revalidate()?;
        let control = self.listener.accept(accept_timeout)?;
        self.revalidate()?;
        let result = self
            .supervisor
            .run_session(control, self.timeouts)
            .map_err(ProtectedIssuerServiceErrorV1::Session);
        let continuity = self.revalidate();
        match (result, continuity) {
            (Ok(exited), Ok(())) => Ok(exited),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }

    fn revalidate(&self) -> Result<(), ProtectedIssuerServiceErrorV1> {
        self.supervisor
            .revalidate()
            .map_err(ProtectedIssuerServiceErrorV1::Supervisor)?;
        self.listener.revalidate()
    }

    #[cfg(test)]
    pub(crate) fn bind_inner(
        supervisor: ProtectedIssuerSupervisorV1,
        listener: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
        expected_path: &Path,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        Self::bind_at(supervisor, listener, timeouts, expected_path)
    }

    #[cfg(test)]
    pub(crate) fn serve_one_inner<State, AfterPreparation, AfterLaunch>(
        &self,
        accept_timeout: Duration,
        after_preparation: AfterPreparation,
        after_launch: AfterLaunch,
    ) -> Result<ExitedProtectedIssuerV1, ProtectedIssuerServiceErrorV1>
    where
        AfterPreparation: FnOnce(&crate::PreparedProtectedIssuerLaunchV1) -> State,
        AfterLaunch: FnOnce(State, &crate::LaunchedProtectedIssuerV1),
    {
        self.revalidate()?;
        let control = self.listener.accept(accept_timeout)?;
        self.revalidate()?;
        let result = self
            .supervisor
            .run_session_inner::<false, _, _, _>(
                control,
                self.timeouts,
                after_preparation,
                after_launch,
            )
            .map_err(ProtectedIssuerServiceErrorV1::Session);
        let continuity = self.revalidate();
        match (result, continuity) {
            (Ok(exited), Ok(())) => Ok(exited),
            (Err(error), Ok(())) => Err(error),
            (_, Err(error)) => Err(error),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SocketSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
    group: u32,
    links: u64,
}

impl SocketSnapshotV1 {
    fn from_stat(stat: rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            owner: stat.st_uid,
            group: stat.st_gid,
            links: stat.st_nlink,
        }
    }
}

struct ProtectedIssuerListenerV1 {
    descriptor: OwnedFd,
    expected_path: PathBuf,
    descriptor_snapshot: SocketSnapshotV1,
    path_snapshot: SocketSnapshotV1,
}

impl ProtectedIssuerListenerV1 {
    fn admit(
        descriptor: OwnedFd,
        expected_path: &Path,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        if !expected_path.is_absolute() {
            return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                "expected pathname is not absolute",
            ));
        }
        let descriptor_snapshot = snapshot_descriptor(&descriptor)?;
        let path_snapshot = snapshot_path(expected_path)?;
        let listener = Self {
            descriptor,
            expected_path: expected_path.to_owned(),
            descriptor_snapshot,
            path_snapshot,
        };
        listener.revalidate()?;
        Ok(listener)
    }

    fn revalidate(&self) -> Result<(), ProtectedIssuerServiceErrorV1> {
        validate_listener_shape(&self.descriptor, &self.expected_path)?;
        if snapshot_descriptor(&self.descriptor)? != self.descriptor_snapshot
            || snapshot_path(&self.expected_path)? != self.path_snapshot
        {
            return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                "descriptor or pathname identity changed",
            ));
        }
        Ok(())
    }

    fn accept(&self, timeout: Duration) -> Result<OwnedFd, ProtectedIssuerServiceErrorV1> {
        if timeout.is_zero() || timeout > MAX_ACCEPT_TIMEOUT_V1 {
            return Err(ProtectedIssuerServiceErrorV1::InvalidAcceptTimeout);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ProtectedIssuerServiceErrorV1::InvalidAcceptTimeout)?;
        loop {
            wait_for_listener(&self.descriptor, deadline)?;
            match accept_with(
                &self.descriptor,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            ) {
                Ok(control) => {
                    self.revalidate()?;
                    return Ok(control);
                }
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {}
                Err(source) => {
                    return Err(io_error("accept protected issuer control", source.into()));
                }
            }
        }
    }
}

fn validate_listener_shape(
    descriptor: &OwnedFd,
    expected_path: &Path,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
        .map_err(|source| io_error("inspect issuer listener descriptor flags", source.into()))?;
    let status = rustix::fs::fcntl_getfl(descriptor)
        .map_err(|source| io_error("inspect issuer listener status flags", source.into()))?;
    let forbidden = OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT;
    if descriptor_flags != rustix::io::FdFlags::CLOEXEC
        || status & OFlags::ACCMODE != OFlags::RDWR
        || !status.contains(OFlags::NONBLOCK)
        || status.intersects(forbidden)
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "descriptor flags are not exact nonblocking close-on-exec custody",
        ));
    }
    if rustix::net::sockopt::socket_domain(descriptor)
        .map_err(|source| io_error("inspect issuer listener domain", source.into()))?
        != AddressFamily::UNIX
        || rustix::net::sockopt::socket_type(descriptor)
            .map_err(|source| io_error("inspect issuer listener type", source.into()))?
            != SocketType::SEQPACKET
        || !rustix::net::sockopt::socket_acceptconn(descriptor)
            .map_err(|source| io_error("inspect issuer listener state", source.into()))?
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "endpoint is not a listening Unix SOCK_SEQPACKET socket",
        ));
    }
    let expected = SocketAddrAny::from(
        SocketAddrUnix::new(expected_path)
            .map_err(|source| io_error("encode fixed issuer listener pathname", source.into()))?,
    );
    if rustix::net::getsockname(descriptor)
        .map_err(|source| io_error("inspect issuer listener pathname", source.into()))?
        != expected
        || listener_has_peer(descriptor)?
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener is not unconnected at the fixed pathname",
        ));
    }
    match rustix::net::sockopt::socket_error(descriptor)
        .map_err(|source| io_error("inspect issuer listener socket error", source.into()))?
    {
        Ok(()) => Ok(()),
        Err(_) => Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener has a pending socket error",
        )),
    }
}

fn listener_has_peer(descriptor: &OwnedFd) -> Result<bool, ProtectedIssuerServiceErrorV1> {
    match rustix::net::getpeername(descriptor) {
        Ok(peer) => Ok(peer.is_some()),
        Err(rustix::io::Errno::NOTCONN) => Ok(false),
        Err(source) => Err(io_error("inspect issuer listener peer", source.into())),
    }
}

fn snapshot_descriptor(
    descriptor: &impl AsFd,
) -> Result<SocketSnapshotV1, ProtectedIssuerServiceErrorV1> {
    let snapshot = SocketSnapshotV1::from_stat(
        rustix::fs::fstat(descriptor)
            .map_err(|source| io_error("inspect issuer listener descriptor", source.into()))?,
    );
    require_socket_snapshot(snapshot)?;
    Ok(snapshot)
}

fn snapshot_path(path: &Path) -> Result<SocketSnapshotV1, ProtectedIssuerServiceErrorV1> {
    let snapshot = SocketSnapshotV1::from_stat(
        rustix::fs::lstat(path)
            .map_err(|source| io_error("inspect issuer listener pathname", source.into()))?,
    );
    require_socket_snapshot(snapshot)?;
    Ok(snapshot)
}

fn require_socket_snapshot(
    snapshot: SocketSnapshotV1,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    if snapshot.mode & libc::S_IFMT != libc::S_IFSOCK
        || snapshot.mode & PERMISSION_AND_SPECIAL_BITS == 0
        || snapshot.links == 0
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener object is not a linked permissioned Unix socket",
        ));
    }
    Ok(())
}

fn wait_for_listener(
    listener: &OwnedFd,
    deadline: Instant,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProtectedIssuerServiceErrorV1::AcceptTimeout);
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(remaining.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(remaining.subsec_nanos()),
        };
        let mut descriptors = [PollFd::new(
            listener,
            PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
        )];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) => return Err(ProtectedIssuerServiceErrorV1::AcceptTimeout),
            Ok(_) => {
                let events = descriptors[0].revents();
                if events.contains(PollFlags::NVAL) {
                    return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                        "listener descriptor became invalid",
                    ));
                }
                if events.intersects(PollFlags::ERR | PollFlags::HUP) {
                    return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                        "listener reported an error or hangup",
                    ));
                }
                if events.contains(PollFlags::IN) {
                    return Ok(());
                }
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => return Err(io_error("poll protected issuer listener", source.into())),
        }
    }
}

fn io_error(operation: &'static str, source: io::Error) -> ProtectedIssuerServiceErrorV1 {
    ProtectedIssuerServiceErrorV1::Io { operation, source }
}
