//! Fixed inherited listener admission and one-session service dispatch.

use std::cell::Cell;
use std::error::Error;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1,
    COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1, COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::OFlags;
use rustix::net::{
    AddressFamily, SocketAddrAny, SocketAddrUnix, SocketFlags, SocketType, accept_with, listen,
};

use crate::{
    ExitedProtectedIssuerV1, ProtectedIssuerSessionErrorV1, ProtectedIssuerSessionTimeoutsV1,
    ProtectedIssuerSupervisorErrorV1, ProtectedIssuerSupervisorV1,
};

const MAX_ACCEPT_TIMEOUT_V1: Duration = Duration::from_secs(120);
const SERVICE_ACCEPT_OBSERVATION_V1: Duration = Duration::from_secs(1);
const LISTENER_BACKLOG_V1: i32 = crate::MAX_PROTECTED_ISSUER_PROCESSES_V1 as i32;
const PERMISSION_AND_SPECIAL_BITS: u32 = 0o7777;
const ROOT_ID_V1: u32 = 0;

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
    /// The configured worker count is zero or exceeds protected process capacity.
    InvalidWorkerCount,
    /// One fixed service worker could not be created.
    WorkerSpawn(io::Error),
    /// One fixed service worker panicked or its bounded completion channel broke.
    WorkerFailed,
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
            Self::InvalidWorkerCount => {
                formatter.write_str("invalid protected issuer service worker count")
            }
            Self::WorkerSpawn(error) => write!(formatter, "cannot spawn issuer worker: {error}"),
            Self::WorkerFailed => formatter.write_str("protected issuer service worker failed"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedIssuerServiceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Supervisor(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::WorkerSpawn(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::InvalidListener(_)
            | Self::InvalidAcceptTimeout
            | Self::AcceptTimeout
            | Self::InvalidWorkerCount
            | Self::WorkerFailed => None,
        }
    }
}

/// Validated fixed concurrency for the protected issuer service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerServiceWorkerCountV1(usize);

impl ProtectedIssuerServiceWorkerCountV1 {
    /// Constructs a nonzero worker count no larger than protected process capacity.
    pub const fn new(workers: usize) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        if workers == 0 || workers > crate::MAX_PROTECTED_ISSUER_PROCESSES_V1 {
            return Err(ProtectedIssuerServiceErrorV1::InvalidWorkerCount);
        }
        Ok(Self(workers))
    }

    /// Returns the exact fixed worker count.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// One inert completed or rejected session reported by the fixed worker pool.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerSessionOutcomeV1 {
    /// One session crossed every lifecycle stage and was exactly once reaped.
    Completed(ExitedProtectedIssuerV1),
    /// One accepted connection failed closed at its reported lifecycle stage.
    Rejected(ProtectedIssuerSessionErrorV1),
}

/// Aggregate bounded-worker service activity observed before graceful shutdown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerServiceReportV1 {
    completed: u64,
    rejected: u64,
}

impl ProtectedIssuerServiceReportV1 {
    /// Returns sessions that completed and were exactly once reaped.
    pub const fn completed(self) -> u64 {
        self.completed
    }

    /// Returns accepted connections that failed closed.
    pub const fn rejected(self) -> u64 {
        self.rejected
    }
}

/// Cloneable, authority-free request for the fixed service workers to stop accepting.
#[derive(Clone, Debug)]
pub struct ProtectedIssuerServiceShutdownV1 {
    requested: Arc<AtomicBool>,
}

impl ProtectedIssuerServiceShutdownV1 {
    /// Requests graceful stop; active sessions retain their configured bounds and custody.
    pub fn request(&self) {
        self.requested.store(true, Ordering::Release);
    }

    /// Reports whether graceful stop has been requested.
    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
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
    shutdown: Arc<AtomicBool>,
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
        let credentials = supervisor.credentials();
        Self::bind_with_policy(
            supervisor,
            listener,
            timeouts,
            Path::new(COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1),
            ListenerFilesystemPolicyV1::production(credentials),
        )
    }

    fn bind_with_policy(
        supervisor: ProtectedIssuerSupervisorV1,
        listener: OwnedFd,
        timeouts: ProtectedIssuerSessionTimeoutsV1,
        expected_path: &Path,
        filesystem_policy: ListenerFilesystemPolicyV1,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        supervisor
            .revalidate()
            .map_err(ProtectedIssuerServiceErrorV1::Supervisor)?;
        let bound =
            BoundProtectedIssuerSocketV1::admit(listener, expected_path, filesystem_policy)?;
        let listener = bound.activate()?;
        supervisor
            .revalidate()
            .map_err(ProtectedIssuerServiceErrorV1::Supervisor)?;
        Ok(Self {
            supervisor,
            listener,
            timeouts,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Returns an authority-free handle that can request graceful accept-loop shutdown.
    pub fn shutdown_handle(&self) -> ProtectedIssuerServiceShutdownV1 {
        ProtectedIssuerServiceShutdownV1 {
            requested: Arc::clone(&self.shutdown),
        }
    }

    /// Runs the sole fixed-capacity production accept loop until graceful shutdown.
    ///
    /// Each fixed worker accepts directly from the retained listener and invokes only the
    /// complete [`ProtectedIssuerSupervisorV1::run_session`] operation. The callback runs on the
    /// owner thread and receives inert completion or stage-typed rejection values. Session
    /// failures do not stop admission; listener, supervisor, channel, or worker failures do.
    pub fn run<Observe>(
        self,
        workers: ProtectedIssuerServiceWorkerCountV1,
        mut observe: Observe,
    ) -> Result<ProtectedIssuerServiceReportV1, ProtectedIssuerServiceErrorV1>
    where
        Observe: FnMut(ProtectedIssuerSessionOutcomeV1),
    {
        self.revalidate()?;
        let worker_count = workers.get();
        let (completion_sender, completion_receiver) = mpsc::sync_channel(
            worker_count
                .checked_mul(2)
                .expect("worker count is bounded"),
        );
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_count);
            for index in 0..worker_count {
                let sender = completion_sender.clone();
                let service = &self;
                let worker = std::thread::Builder::new()
                    .name(format!("fe2o3-issuer-worker-{index}"))
                    .spawn_scoped(scope, move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            service.run_worker(&sender)
                        }));
                        if result.is_err() {
                            service.shutdown.store(true, Ordering::Release);
                            let _ = sender.send(WorkerEventV1::Failed);
                        }
                        let _ = sender.send(WorkerEventV1::Stopped);
                    });
                match worker {
                    Ok(handle) => handles.push(handle),
                    Err(source) => {
                        self.shutdown.store(true, Ordering::Release);
                        drop(completion_sender);
                        for handle in handles {
                            let _ = handle.join();
                        }
                        return Err(ProtectedIssuerServiceErrorV1::WorkerSpawn(source));
                    }
                }
            }
            drop(completion_sender);

            let mut report = ProtectedIssuerServiceReportV1 {
                completed: 0,
                rejected: 0,
            };
            let mut stopped = 0_usize;
            let mut fatal = None;
            while stopped < worker_count {
                let event = completion_receiver
                    .recv()
                    .map_err(|_| ProtectedIssuerServiceErrorV1::WorkerFailed)?;
                match event {
                    WorkerEventV1::Outcome(ProtectedIssuerSessionOutcomeV1::Completed(exited)) => {
                        report.completed = report
                            .completed
                            .checked_add(1)
                            .ok_or(ProtectedIssuerServiceErrorV1::WorkerFailed)?;
                        observe(ProtectedIssuerSessionOutcomeV1::Completed(exited));
                    }
                    WorkerEventV1::Outcome(ProtectedIssuerSessionOutcomeV1::Rejected(error)) => {
                        report.rejected = report
                            .rejected
                            .checked_add(1)
                            .ok_or(ProtectedIssuerServiceErrorV1::WorkerFailed)?;
                        observe(ProtectedIssuerSessionOutcomeV1::Rejected(error));
                    }
                    WorkerEventV1::Fatal(error) => {
                        self.shutdown.store(true, Ordering::Release);
                        if fatal.is_none() {
                            fatal = Some(error);
                        }
                    }
                    WorkerEventV1::Failed => {
                        self.shutdown.store(true, Ordering::Release);
                        if fatal.is_none() {
                            fatal = Some(ProtectedIssuerServiceErrorV1::WorkerFailed);
                        }
                    }
                    WorkerEventV1::Stopped => stopped += 1,
                }
            }
            for handle in handles {
                if handle.join().is_err() && fatal.is_none() {
                    fatal = Some(ProtectedIssuerServiceErrorV1::WorkerFailed);
                }
            }
            if let Some(error) = fatal {
                return Err(error);
            }
            self.revalidate()?;
            Ok(report)
        })
    }

    fn run_worker(&self, sender: &SyncSender<WorkerEventV1>) {
        while !self.shutdown.load(Ordering::Acquire) {
            match self.serve_one(SERVICE_ACCEPT_OBSERVATION_V1) {
                Ok(exited) => {
                    if sender
                        .send(WorkerEventV1::Outcome(
                            ProtectedIssuerSessionOutcomeV1::Completed(exited),
                        ))
                        .is_err()
                    {
                        self.shutdown.store(true, Ordering::Release);
                    }
                }
                Err(ProtectedIssuerServiceErrorV1::AcceptTimeout) => {}
                Err(ProtectedIssuerServiceErrorV1::Session(error)) => {
                    if sender
                        .send(WorkerEventV1::Outcome(
                            ProtectedIssuerSessionOutcomeV1::Rejected(error),
                        ))
                        .is_err()
                    {
                        self.shutdown.store(true, Ordering::Release);
                    }
                }
                Err(error) => {
                    self.shutdown.store(true, Ordering::Release);
                    let _ = sender.send(WorkerEventV1::Fatal(error));
                }
            }
        }
    }

    fn serve_one(
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
        let credentials = supervisor.credentials();
        Self::bind_with_policy(
            supervisor,
            listener,
            timeouts,
            expected_path,
            ListenerFilesystemPolicyV1::fixture(credentials),
        )
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

enum WorkerEventV1 {
    Outcome(ProtectedIssuerSessionOutcomeV1),
    Fatal(ProtectedIssuerServiceErrorV1),
    Failed,
    Stopped,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ListenerFilesystemPolicyV1 {
    parent_owner: u32,
    parent_group: u32,
    parent_mode: u32,
    socket_owner: u32,
    socket_group: u32,
    socket_mode: u32,
}

impl ListenerFilesystemPolicyV1 {
    pub(super) const fn production(credentials: crate::IssuerServiceCredentialProfileV1) -> Self {
        Self {
            parent_owner: ROOT_ID_V1,
            parent_group: ROOT_ID_V1,
            parent_mode: COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1,
            socket_owner: ROOT_ID_V1,
            socket_group: credentials.gid(),
            socket_mode: COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1,
        }
    }

    #[cfg(test)]
    pub(super) const fn fixture(credentials: crate::IssuerServiceCredentialProfileV1) -> Self {
        Self {
            parent_owner: credentials.uid(),
            parent_group: credentials.gid(),
            parent_mode: 0o700,
            socket_owner: credentials.uid(),
            socket_group: credentials.gid(),
            socket_mode: COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1,
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProtectedIssuerSocketStateV1 {
    Bound,
    Listening,
}

struct ProtectedIssuerSocketCustodyV1 {
    descriptor: OwnedFd,
    expected_path: PathBuf,
    filesystem_policy: ListenerFilesystemPolicyV1,
    descriptor_snapshot: SocketSnapshotV1,
    path_snapshot: SocketSnapshotV1,
    parent_snapshot: SocketSnapshotV1,
}

impl ProtectedIssuerSocketCustodyV1 {
    fn admit(
        descriptor: OwnedFd,
        expected_path: &Path,
        filesystem_policy: ListenerFilesystemPolicyV1,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        if !expected_path.is_absolute() {
            return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                "expected pathname is not absolute",
            ));
        }
        let descriptor_snapshot = snapshot_descriptor(&descriptor)?;
        let path_snapshot = snapshot_path(expected_path, filesystem_policy)?;
        let parent_snapshot = snapshot_parent(expected_path, filesystem_policy)?;
        let socket = Self {
            descriptor,
            expected_path: expected_path.to_owned(),
            filesystem_policy,
            descriptor_snapshot,
            path_snapshot,
            parent_snapshot,
        };
        socket.revalidate()?;
        Ok(socket)
    }

    fn revalidate(&self) -> Result<ProtectedIssuerSocketStateV1, ProtectedIssuerServiceErrorV1> {
        let state = validate_socket_shape(&self.descriptor, &self.expected_path)?;
        if snapshot_descriptor(&self.descriptor)? != self.descriptor_snapshot
            || snapshot_path(&self.expected_path, self.filesystem_policy)? != self.path_snapshot
            || snapshot_parent(&self.expected_path, self.filesystem_policy)? != self.parent_snapshot
        {
            return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                "descriptor or pathname identity changed",
            ));
        }
        Ok(state)
    }

    fn revalidate_clone(
        &self,
        descriptor: &OwnedFd,
    ) -> Result<ProtectedIssuerSocketStateV1, ProtectedIssuerServiceErrorV1> {
        let state = validate_socket_shape(descriptor, &self.expected_path)?;
        if snapshot_descriptor(&descriptor)? != self.descriptor_snapshot
            || snapshot_path(&self.expected_path, self.filesystem_policy)? != self.path_snapshot
            || snapshot_parent(&self.expected_path, self.filesystem_policy)? != self.parent_snapshot
        {
            return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                "deployment clone changed descriptor or pathname identity",
            ));
        }
        Ok(state)
    }
}

/// Root-retained custody of the exact socket admitted before service activation.
///
/// The observed kernel state may advance once from bound to listening after the deployed
/// supervisor activates a clone. Filesystem identity remains fixed across that transition.
pub(super) struct ProvisionedProtectedIssuerSocketV1 {
    socket: ProtectedIssuerSocketCustodyV1,
    observed_state: Cell<ProtectedIssuerSocketStateV1>,
}

impl ProvisionedProtectedIssuerSocketV1 {
    pub(super) fn admit(
        descriptor: OwnedFd,
        expected_path: &Path,
        filesystem_policy: ListenerFilesystemPolicyV1,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        let socket =
            ProtectedIssuerSocketCustodyV1::admit(descriptor, expected_path, filesystem_policy)?;
        require_socket_state(socket.revalidate()?, ProtectedIssuerSocketStateV1::Bound)?;
        Ok(Self {
            socket,
            observed_state: Cell::new(ProtectedIssuerSocketStateV1::Bound),
        })
    }

    pub(super) fn revalidate(&self) -> Result<(), ProtectedIssuerServiceErrorV1> {
        self.observe_state().map(|_| ())
    }

    pub(super) fn try_clone_for_deployment(
        &self,
    ) -> Result<OwnedFd, ProtectedIssuerServiceErrorV1> {
        require_socket_state(self.observe_state()?, ProtectedIssuerSocketStateV1::Bound)?;
        let descriptor = rustix::io::fcntl_dupfd_cloexec(&self.socket.descriptor, 0)
            .map_err(|source| io_error("clone protected issuer bound socket", source.into()))?;
        require_socket_state(
            self.socket.revalidate_clone(&descriptor)?,
            ProtectedIssuerSocketStateV1::Bound,
        )?;
        require_socket_state(self.observe_state()?, ProtectedIssuerSocketStateV1::Bound)?;
        Ok(descriptor)
    }

    fn observe_state(&self) -> Result<ProtectedIssuerSocketStateV1, ProtectedIssuerServiceErrorV1> {
        let observed = self.socket.revalidate()?;
        match (self.observed_state.get(), observed) {
            (ProtectedIssuerSocketStateV1::Bound, ProtectedIssuerSocketStateV1::Listening) => {
                self.observed_state
                    .set(ProtectedIssuerSocketStateV1::Listening);
            }
            (ProtectedIssuerSocketStateV1::Listening, ProtectedIssuerSocketStateV1::Bound) => {
                return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                    "listener activation state regressed",
                ));
            }
            _ => {}
        }
        Ok(observed)
    }
}

struct BoundProtectedIssuerSocketV1 {
    socket: ProtectedIssuerSocketCustodyV1,
}

impl BoundProtectedIssuerSocketV1 {
    fn admit(
        descriptor: OwnedFd,
        expected_path: &Path,
        filesystem_policy: ListenerFilesystemPolicyV1,
    ) -> Result<Self, ProtectedIssuerServiceErrorV1> {
        let socket =
            ProtectedIssuerSocketCustodyV1::admit(descriptor, expected_path, filesystem_policy)?;
        require_socket_state(socket.revalidate()?, ProtectedIssuerSocketStateV1::Bound)?;
        Ok(Self { socket })
    }

    fn activate(self) -> Result<ProtectedIssuerListenerV1, ProtectedIssuerServiceErrorV1> {
        require_socket_state(
            self.socket.revalidate()?,
            ProtectedIssuerSocketStateV1::Bound,
        )?;
        listen(&self.socket.descriptor, LISTENER_BACKLOG_V1)
            .map_err(|source| io_error("activate protected issuer listener", source.into()))?;
        let listener = ProtectedIssuerListenerV1 {
            socket: self.socket,
        };
        listener.revalidate()?;
        Ok(listener)
    }
}

pub(super) struct ProtectedIssuerListenerV1 {
    socket: ProtectedIssuerSocketCustodyV1,
}

impl ProtectedIssuerListenerV1 {
    pub(super) fn revalidate(&self) -> Result<(), ProtectedIssuerServiceErrorV1> {
        require_socket_state(
            self.socket.revalidate()?,
            ProtectedIssuerSocketStateV1::Listening,
        )
    }

    fn accept(&self, timeout: Duration) -> Result<OwnedFd, ProtectedIssuerServiceErrorV1> {
        if timeout.is_zero() || timeout > MAX_ACCEPT_TIMEOUT_V1 {
            return Err(ProtectedIssuerServiceErrorV1::InvalidAcceptTimeout);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(ProtectedIssuerServiceErrorV1::InvalidAcceptTimeout)?;
        loop {
            wait_for_listener(&self.socket.descriptor, deadline)?;
            match accept_with(
                &self.socket.descriptor,
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

fn validate_socket_shape(
    descriptor: &OwnedFd,
    expected_path: &Path,
) -> Result<ProtectedIssuerSocketStateV1, ProtectedIssuerServiceErrorV1> {
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
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "endpoint is not a Unix SOCK_SEQPACKET socket",
        ));
    }
    let state = if rustix::net::sockopt::socket_acceptconn(descriptor)
        .map_err(|source| io_error("inspect issuer listener state", source.into()))?
    {
        ProtectedIssuerSocketStateV1::Listening
    } else {
        ProtectedIssuerSocketStateV1::Bound
    };
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
        Ok(()) => Ok(state),
        Err(_) => Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener has a pending socket error",
        )),
    }
}

fn require_socket_state(
    observed: ProtectedIssuerSocketStateV1,
    expected: ProtectedIssuerSocketStateV1,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    if observed == expected {
        return Ok(());
    }
    let reason = match expected {
        ProtectedIssuerSocketStateV1::Bound => {
            "endpoint is not a bound, non-listening Unix SOCK_SEQPACKET socket"
        }
        ProtectedIssuerSocketStateV1::Listening => {
            "endpoint is not a listening Unix SOCK_SEQPACKET socket"
        }
    };
    Err(ProtectedIssuerServiceErrorV1::InvalidListener(reason))
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

fn snapshot_path(
    path: &Path,
    policy: ListenerFilesystemPolicyV1,
) -> Result<SocketSnapshotV1, ProtectedIssuerServiceErrorV1> {
    let snapshot = SocketSnapshotV1::from_stat(
        rustix::fs::lstat(path)
            .map_err(|source| io_error("inspect issuer listener pathname", source.into()))?,
    );
    require_socket_path_snapshot(snapshot, policy)?;
    require_absent_path_attributes(path, "inspect issuer listener pathname attributes")?;
    Ok(snapshot)
}

fn snapshot_parent(
    path: &Path,
    policy: ListenerFilesystemPolicyV1,
) -> Result<SocketSnapshotV1, ProtectedIssuerServiceErrorV1> {
    let parent = path
        .parent()
        .ok_or(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener pathname has no parent directory",
        ))?;
    let snapshot =
        SocketSnapshotV1::from_stat(rustix::fs::lstat(parent).map_err(|source| {
            io_error("inspect issuer listener parent directory", source.into())
        })?);
    if snapshot.mode & libc::S_IFMT != libc::S_IFDIR
        || snapshot.mode & PERMISSION_AND_SPECIAL_BITS != policy.parent_mode
        || snapshot.owner != policy.parent_owner
        || snapshot.group != policy.parent_group
        || snapshot.links == 0
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener parent directory has the wrong type, owner, group, mode, or link count",
        ));
    }
    require_absent_path_attributes(parent, "inspect issuer listener parent attributes")?;
    Ok(snapshot)
}

fn require_socket_snapshot(
    snapshot: SocketSnapshotV1,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    if snapshot.mode & libc::S_IFMT != libc::S_IFSOCK || snapshot.links == 0 {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener descriptor is not a linked Unix socket",
        ));
    }
    Ok(())
}

fn require_socket_path_snapshot(
    snapshot: SocketSnapshotV1,
    policy: ListenerFilesystemPolicyV1,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    if snapshot.mode & libc::S_IFMT != libc::S_IFSOCK
        || snapshot.mode & PERMISSION_AND_SPECIAL_BITS != policy.socket_mode
        || snapshot.owner != policy.socket_owner
        || snapshot.group != policy.socket_group
        || snapshot.links == 0
    {
        return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
            "listener pathname has the wrong type, owner, group, mode, or link count",
        ));
    }
    Ok(())
}

fn require_absent_path_attributes(
    path: &Path,
    operation: &'static str,
) -> Result<(), ProtectedIssuerServiceErrorV1> {
    for attribute in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        let mut byte = 0_u8;
        match rustix::fs::lgetxattr(path, attribute, std::slice::from_mut(&mut byte)) {
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP) => {}
            Ok(_) | Err(rustix::io::Errno::RANGE) => {
                return Err(ProtectedIssuerServiceErrorV1::InvalidListener(
                    "listener path has a forbidden capability or POSIX ACL",
                ));
            }
            Err(source) => {
                return Err(io_error(operation, source.into()));
            }
        }
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
