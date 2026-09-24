//! Pidfd-owned launch and readiness lifecycle for the protected issuer.

#[path = "process_native.rs"]
mod native;
pub use native::*;

#[path = "process_profile_report.rs"]
mod profile_report;
use profile_report::{ProfileReportRead, ReportError};

use core::ffi::{c_char, c_int, c_long, c_void};
use std::error::Error;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionServiceLaunchManifestV1, CompilerExecutionServiceReadyErrorV1,
    CompilerExecutionServiceReadyV1,
};
use fe2o3_protected_service_profile::observations::{
    CHILD_NAMESPACE_REPORT_BYTES, CHILD_NAMESPACE_REPORT_CAPTURE_WORK,
    capture_child_namespace_report_pre_exec,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceNamespaceSetV1 as NamespaceSetV1,
    ProtectedServiceProcessProfileV1 as ExactProcessProfileV1, ProtectedServiceProfileErrorV1,
    require_owned_sigchld_v1, validate_current_protected_service_profile_v1,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::SendFlags;
use rustix::pipe::{PipeFlags, pipe_with};

use crate::process_cleanup::{ChildCleanupV1, CleanupPollV1};
use crate::process_reaper::{ReapSlotV1, deferred_reaper};
use crate::process_staging::{StagedLaunchErrorV1, StagedLaunchInputV1, StagedLaunchV1};
use crate::{
    IssuerServiceCredentialProfileV1, PreparedProtectedIssuerLaunchV1,
    ProtectedIssuerLaunchPreparationErrorV1, ProtectedIssuerSupervisorErrorV1,
    ProtectedIssuerSupervisorV1,
};

const CLONE_PIDFD: u64 = 0x0000_1000;
const CLONE_CLEAR_SIGHAND: u64 = 0x0000_0001_0000_0000;
const SIGCHLD: u64 = 17;
const SIGKILL: c_int = 9;
const SIGSTOP: c_int = 19;
const SYS_CLONE3: c_long = 435;
const SYS_CLOSE_RANGE: c_long = 436;
const SYS_EXECVEAT: c_long = 322;
const SYS_RT_SIGACTION: c_long = 13;
const SYS_RT_SIGPROCMASK: c_long = 14;
const SYS_GETGROUPS: c_long = 115;
const SYS_GETRESUID: c_long = 118;
const SYS_GETRESGID: c_long = 120;
const SYS_SETFSUID: c_long = 122;
const SYS_SETFSGID: c_long = 123;
const SYS_CAPGET: c_long = 125;
const SYS_UMASK: c_long = 95;
const SYS_PRLIMIT64: c_long = 302;
const SIG_SETMASK: c_int = 2;
const KERNEL_SIGNAL_COUNT: c_int = 64;
const KERNEL_SIGSET_BYTES: usize = 8;
const AT_EMPTY_PATH: c_int = 0x1000;
const CLOSE_RANGE_CLOEXEC: u32 = 1 << 2;
const PR_GET_DUMPABLE: c_int = 3;
const PR_SET_PDEATHSIG: c_int = 1;
const PR_GET_PDEATHSIG: c_int = 2;
const PR_GET_SECUREBITS: c_int = 27;
const PR_CAPBSET_READ: c_int = 23;
const PR_GET_NO_NEW_PRIVS: c_int = 39;
const PR_CAP_AMBIENT: c_int = 47;
const PR_CAP_AMBIENT_IS_SET: c_int = 1;
const RLIMIT_CORE: c_int = 4;
const LINUX_CAPABILITY_VERSION_3: u32 = 0x2008_0522;
const GATE_RELEASE_V1: u8 = 0x5a;
const MAX_CHILD_GATE_ATTEMPTS: usize = 64;
const MAX_LAUNCH_WAIT_V1: Duration = Duration::from_secs(120);
const POLL_INTERVAL_V1: Duration = Duration::from_millis(1);
const MAX_CANCEL_POLLS_V1: usize = 1024;
const MAX_CANCEL_WAIT_V1: Duration = Duration::from_secs(2);

/// Maximum number of protected issuer children owned or awaiting deferred reaping.
pub const MAX_PROTECTED_ISSUER_PROCESSES_V1: usize = 64;

unsafe extern "C" {
    fn close(descriptor: c_int) -> c_int;
    fn dup3(old_descriptor: c_int, new_descriptor: c_int, flags: c_int) -> c_int;
    fn prctl(option: c_int, ...) -> c_int;
    fn syscall(number: c_long, ...) -> c_long;
    fn write(descriptor: c_int, bytes: *const c_void, length: usize) -> isize;
    fn _exit(status: c_int) -> !;
}

#[repr(C)]
struct CloneArgsV1 {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

#[repr(C)]
struct KernelSigactionV1 {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: u64,
}

#[repr(C)]
struct LinuxCapabilityHeaderV1 {
    version: u32,
    pid: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LinuxCapabilityDataV1 {
    effective: u32,
    permitted: u32,
    inheritable: u32,
}

#[repr(C)]
struct LinuxRlimit64V1 {
    current: u64,
    maximum: u64,
}

#[derive(Clone, Copy)]
struct ChildProfileV1 {
    uid: u32,
    gid: u32,
    securebits: u32,
    cap_last_cap: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildProcessError {
    State(&'static str),
    Io {
        operation: &'static str,
        errno: rustix::io::Errno,
    },
}

/// Stable failure launching, admitting, or terminating one protected issuer process.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedIssuerLaunchErrorV1 {
    /// Supervisor authority continuity failed before process creation.
    Supervisor(ProtectedIssuerSupervisorErrorV1),
    /// Prepared launch custody changed before process creation.
    Preparation(ProtectedIssuerLaunchPreparationErrorV1),
    /// A caller supplied a zero, overflowing, or excessively large bounded wait.
    InvalidTimeout,
    /// The supervisor or gated child does not have the exact production process profile.
    ProcessProfile(&'static str),
    /// A supervisor or child namespace differs from the captured launch namespace.
    Namespace(&'static str),
    /// The post-clone child rejected a fixed pre-exec stage.
    ChildStage(u8),
    /// The child exited before the requested lifecycle boundary.
    ChildExited(String),
    /// The child did not reach a bounded lifecycle boundary in time.
    Timeout(&'static str),
    /// Readiness ended before one exact record was received.
    ReadinessTruncated,
    /// Readiness contained bytes after the one exact record.
    ReadinessTrailingBytes,
    /// The canonical readiness record failed strict decoding.
    ReadinessProtocol(CompilerExecutionServiceReadyErrorV1),
    /// Readiness names another PID, launch manifest, or issuer policy.
    ReadinessMismatch,
    /// The bounded deferred-reaping table has no free process slot.
    ProcessCapacity,
    /// Pidfd ownership or exactly-once reaping was violated.
    InvalidProcessState(&'static str),
    /// A bounded Linux operation failed.
    Io {
        /// Operation that failed.
        operation: &'static str,
        /// Kernel or procfs error.
        source: io::Error,
    },
}

impl fmt::Display for ProtectedIssuerLaunchErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supervisor(error) => write!(formatter, "issuer supervisor changed: {error}"),
            Self::Preparation(error) => {
                write!(formatter, "prepared issuer launch changed: {error}")
            }
            Self::InvalidTimeout => formatter.write_str("invalid bounded issuer lifecycle timeout"),
            Self::ProcessProfile(reason) => {
                write!(
                    formatter,
                    "protected issuer process profile mismatch: {reason}"
                )
            }
            Self::Namespace(namespace) => {
                write!(formatter, "protected issuer {namespace} namespace changed")
            }
            Self::ChildStage(stage) => {
                write!(
                    formatter,
                    "protected issuer child rejected pre-exec stage {stage}"
                )
            }
            Self::ChildExited(detail) => write!(formatter, "protected issuer {detail}"),
            Self::Timeout(boundary) => {
                write!(formatter, "protected issuer timed out before {boundary}")
            }
            Self::ReadinessTruncated => {
                formatter.write_str("protected issuer readiness ended before one exact record")
            }
            Self::ReadinessTrailingBytes => {
                formatter.write_str("protected issuer readiness contained trailing bytes")
            }
            Self::ReadinessProtocol(error) => {
                write!(formatter, "protected issuer readiness is invalid: {error}")
            }
            Self::ReadinessMismatch => formatter.write_str(
                "protected issuer readiness names another PID, launch manifest, or policy",
            ),
            Self::ProcessCapacity => {
                formatter.write_str("protected issuer process/reaper capacity is exhausted")
            }
            Self::InvalidProcessState(reason) => {
                write!(
                    formatter,
                    "invalid protected issuer process state: {reason}"
                )
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedIssuerLaunchErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Supervisor(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::ReadinessProtocol(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::InvalidTimeout
            | Self::ProcessProfile(_)
            | Self::Namespace(_)
            | Self::ChildStage(_)
            | Self::ChildExited(_)
            | Self::Timeout(_)
            | Self::ReadinessTruncated
            | Self::ReadinessTrailingBytes
            | Self::ReadinessMismatch
            | Self::ProcessCapacity
            | Self::InvalidProcessState(_) => None,
        }
    }
}

/// Move-only custody of the exact pidfd child after the static launcher has executed.
///
/// This state has not admitted issuer readiness and grants no signing, compiler,
/// publication, loading, or GPU authority. Dropping it sends `SIGKILL` through
/// the pidfd and transfers cleanup custody to a fixed-capacity internal reaper.
/// Inconclusive cleanup retains the slot; it is not successful reaping evidence.
/// This state also retains the artifact-spawn lease until validated readiness.
/// Do not drop artifact locks on the same thread before advancing readiness or
/// transferring/canceling this child: lock release waits for that lease.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<LaunchedProtectedIssuerV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<LaunchedProtectedIssuerV1>();
/// ```
pub struct LaunchedProtectedIssuerV1 {
    process: IssuerChild,
    control: OwnedFd,
    stdout_reader: OwnedFd,
    stderr_reader: OwnedFd,
    readiness_reader: OwnedFd,
    launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    policy: CompilerExecutionIssuerPolicyV1,
}

impl fmt::Debug for LaunchedProtectedIssuerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LaunchedProtectedIssuerV1")
            .field("authority", &"pidfd-launch-custody-only")
            .field("pid", &self.pid())
            .finish_non_exhaustive()
    }
}

impl LaunchedProtectedIssuerV1 {
    /// Returns the exact child PID paired with the internally retained pidfd.
    pub fn pid(&self) -> u32 {
        self.process.pid_u32()
    }

    /// Reports whether the exact pidfd child has not yet produced an exit event.
    pub fn is_live(&self) -> Result<bool, ProtectedIssuerLaunchErrorV1> {
        self.process.is_live()
    }

    /// Consumes launch-only custody and admits one exact readiness record.
    pub fn await_readiness(
        mut self,
        timeout: Duration,
    ) -> Result<ReadyProtectedIssuerV1, ProtectedIssuerLaunchErrorV1> {
        let deadline = bounded_deadline(timeout)?;
        let readiness = await_readiness_record(
            &self.readiness_reader,
            &self.process,
            &self.launch_manifest,
            &self.policy,
            deadline,
        )?;
        if !self.process.is_live()? {
            return Err(self
                .process
                .exited_error("exited immediately after readiness"));
        }
        // Only the executed issuer emits this validated record. An exec-status EOF
        // alone can precede completion of the kernel's CLOEXEC/exit descriptor sweep.
        self.process.release_spawn_after_exec();
        let Self {
            process,
            control,
            stdout_reader,
            stderr_reader,
            readiness_reader,
            launch_manifest,
            policy,
        } = self;
        drop(readiness_reader);
        Ok(ReadyProtectedIssuerV1 {
            process,
            control,
            _stdout_reader: stdout_reader,
            _stderr_reader: stderr_reader,
            readiness,
            launch_manifest,
            policy,
        })
    }

    /// Attempts bounded cancellation; success means this owner reaped the child.
    /// On timeout or ownership loss, the internal reaper retains cleanup custody.
    pub fn cancel(mut self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        self.process.cancel_and_reap()
    }

    #[cfg(test)]
    pub(crate) fn stdout_reader_for_test(&self) -> &OwnedFd {
        &self.stdout_reader
    }

    #[cfg(test)]
    pub(crate) fn retains_spawn_lease_for_test(&self) -> bool {
        self.process.cleanup.as_ref().unwrap().retains_spawn_lease()
    }
}

/// Move-only evidence that the exact live pidfd child published matching readiness.
///
/// The value retains private stdout/stderr endpoints and process custody but
/// exposes no descriptor. Readiness is inert evidence; downstream compiler
/// authority still requires the bounded service protocol and receipt checks.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ReadyProtectedIssuerV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ReadyProtectedIssuerV1>();
/// ```
pub struct ReadyProtectedIssuerV1 {
    process: IssuerChild,
    control: OwnedFd,
    _stdout_reader: OwnedFd,
    _stderr_reader: OwnedFd,
    readiness: CompilerExecutionServiceReadyV1,
    launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    policy: CompilerExecutionIssuerPolicyV1,
}

impl fmt::Debug for ReadyProtectedIssuerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadyProtectedIssuerV1")
            .field("authority", &"live-ready-issuer-custody-only")
            .field("pid", &self.pid())
            .field("readiness", &self.readiness.identity())
            .finish_non_exhaustive()
    }
}

impl ReadyProtectedIssuerV1 {
    #[cfg(test)]
    pub(crate) fn retains_spawn_lease_for_test(&self) -> bool {
        self.process.cleanup.as_ref().unwrap().retains_spawn_lease()
    }

    /// Returns the exact pidfd-bound issuer PID.
    pub fn pid(&self) -> u32 {
        self.process.pid_u32()
    }

    /// Returns inert canonical readiness evidence without exposing process custody.
    pub const fn readiness(&self) -> &CompilerExecutionServiceReadyV1 {
        &self.readiness
    }

    /// Revalidates readiness binding and current pidfd liveness.
    pub fn revalidate(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        if !self
            .readiness
            .matches_launch(self.pid(), &self.launch_manifest, &self.policy)
        {
            return Err(ProtectedIssuerLaunchErrorV1::ReadinessMismatch);
        }
        if !self.process.is_live()? {
            return Err(self.process.exited_error("is no longer live"));
        }
        Ok(())
    }

    /// Publishes the admitted readiness record to Cargo and enters serving custody.
    pub fn publish_readiness(
        self,
        timeout: Duration,
    ) -> Result<ServingProtectedIssuerV1, ProtectedIssuerLaunchErrorV1> {
        let deadline = bounded_deadline(timeout)?;
        self.revalidate()?;
        publish_control_readiness(
            &self.control,
            self.readiness.canonical_bytes(),
            &self.process,
            deadline,
        )?;
        let Self {
            process,
            control,
            _stdout_reader,
            _stderr_reader,
            readiness,
            launch_manifest,
            policy,
        } = self;
        drop(control);
        Ok(ServingProtectedIssuerV1 {
            process,
            _stdout_reader,
            _stderr_reader,
            readiness,
            launch_manifest,
            policy,
        })
    }

    /// Attempts bounded cancellation; success means this owner reaped the child.
    /// On timeout or ownership loss, the internal reaper retains cleanup custody.
    pub fn cancel(mut self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        self.process.cancel_and_reap()
    }
}

/// Move-only custody of one ready issuer after Cargo received exact readiness.
///
/// This value owns the same pidfd child for the complete service session. It
/// exposes no descriptor, signing operation, publication authority, loading
/// authority, or GPU authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ServingProtectedIssuerV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<ServingProtectedIssuerV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_execution_supervisor::ServingProtectedIssuerV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<ServingProtectedIssuerV1>();
/// ```
pub struct ServingProtectedIssuerV1 {
    process: IssuerChild,
    _stdout_reader: OwnedFd,
    _stderr_reader: OwnedFd,
    readiness: CompilerExecutionServiceReadyV1,
    launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    policy: CompilerExecutionIssuerPolicyV1,
}

/// Inert termination of one naturally exited, exactly-once reaped issuer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedIssuerTerminationV1 {
    /// The issuer returned one ordinary process exit status.
    Exited {
        /// Exact status supplied to the Linux process-exit operation.
        status: i32,
    },
    /// The issuer was terminated by one signal.
    Signaled {
        /// Exact Linux signal number that terminated the issuer.
        signal: i32,
        /// Whether Linux reported that the terminating signal dumped core.
        core_dumped: bool,
    },
}

impl ProtectedIssuerTerminationV1 {
    /// Reports whether the issuer returned ordinary status zero.
    pub const fn succeeded(self) -> bool {
        matches!(self, Self::Exited { status: 0 })
    }

    fn from_wait_status(status: &rustix::process::WaitIdStatus) -> Result<Self, ChildProcessError> {
        if let Some(status) = status.exit_status() {
            return Ok(Self::Exited { status });
        }
        if let Some(signal) = status.terminating_signal() {
            return Ok(Self::Signaled {
                signal,
                core_dumped: status.dumped(),
            });
        }
        Err(ChildProcessError::State(
            "waitid returned a nonterminal issuer state",
        ))
    }
}

/// Inert evidence that one announced issuer exited naturally and was reaped once.
///
/// This value exposes no descriptor, signing operation, publication authority,
/// loading authority, or GPU authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExitedProtectedIssuerV1 {
    pid: u32,
    readiness: CompilerExecutionServiceReadyV1,
    termination: ProtectedIssuerTerminationV1,
}

impl ExitedProtectedIssuerV1 {
    /// Returns the PID formerly paired with the now-consumed pidfd.
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Returns the exact readiness record acknowledged before serving began.
    pub const fn readiness(&self) -> &CompilerExecutionServiceReadyV1 {
        &self.readiness
    }

    /// Returns the exact terminal state observed while reaping through the pidfd.
    pub const fn termination(&self) -> ProtectedIssuerTerminationV1 {
        self.termination
    }
}

impl fmt::Debug for ServingProtectedIssuerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServingProtectedIssuerV1")
            .field("authority", &"announced-live-issuer-custody-only")
            .field("pid", &self.pid())
            .field("readiness", &self.readiness.identity())
            .finish_non_exhaustive()
    }
}

impl ServingProtectedIssuerV1 {
    /// Returns the exact pidfd-bound issuer PID.
    pub fn pid(&self) -> u32 {
        self.process.pid_u32()
    }

    /// Returns the exact readiness record acknowledged by Cargo.
    pub const fn readiness(&self) -> &CompilerExecutionServiceReadyV1 {
        &self.readiness
    }

    /// Revalidates the acknowledged launch binding and pidfd liveness.
    pub fn revalidate(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        if !self
            .readiness
            .matches_launch(self.pid(), &self.launch_manifest, &self.policy)
        {
            return Err(ProtectedIssuerLaunchErrorV1::ReadinessMismatch);
        }
        if !self.process.is_live()? {
            return Err(self.process.exited_error("is no longer serving"));
        }
        Ok(())
    }

    /// Waits for natural issuer termination and consumes exactly-once process custody.
    ///
    /// One absolute nonzero timeout covers pidfd observation and reaping. If the
    /// boundary expires or observation fails, consuming `self` fails closed: its
    /// destructor kills the exact child through the pidfd and transfers reaping to
    /// the bounded internal reaper.
    pub fn wait_for_exit(
        mut self,
        timeout: Duration,
    ) -> Result<ExitedProtectedIssuerV1, ProtectedIssuerLaunchErrorV1> {
        let deadline = session_deadline(timeout)?;
        let pid = self.pid();
        let termination = self.process.wait_and_reap(deadline)?;
        Ok(ExitedProtectedIssuerV1 {
            pid,
            readiness: self.readiness,
            termination,
        })
    }

    /// Attempts bounded cancellation; success means this owner reaped the child.
    /// On timeout or ownership loss, the internal reaper retains cleanup custody.
    pub fn cancel(mut self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        self.process.cancel_and_reap()
    }
}

impl ProtectedIssuerSupervisorV1 {
    /// Consumes one fully prepared launch and executes its authenticated static launcher.
    ///
    /// Production launch requires the calling supervisor thread to already
    /// possess the complete locked service profile. The child inherits that
    /// profile, verifies it with direct syscalls while gated, and cannot execute
    /// the launcher until the parent checks procfs and its private report of
    /// every child namespace. The returned value is not ready issuer authority.
    pub fn launch(
        &self,
        prepared: PreparedProtectedIssuerLaunchV1,
        timeout: Duration,
    ) -> Result<LaunchedProtectedIssuerV1, ProtectedIssuerLaunchErrorV1> {
        self.launch_inner::<true>(prepared, timeout)
    }

    pub(crate) fn launch_inner<const ENFORCE_PROFILE: bool>(
        &self,
        prepared: PreparedProtectedIssuerLaunchV1,
        timeout: Duration,
    ) -> Result<LaunchedProtectedIssuerV1, ProtectedIssuerLaunchErrorV1> {
        let deadline = bounded_deadline(timeout)?;
        self.revalidate()
            .map_err(ProtectedIssuerLaunchErrorV1::Supervisor)?;
        prepared
            .revalidate(self)
            .map_err(ProtectedIssuerLaunchErrorV1::Preparation)?;

        let profile = if ENFORCE_PROFILE {
            Some(ExactProcessProfileV1::capture(self.credentials()).map_err(map_profile_error)?)
        } else {
            None
        };
        require_owned_sigchld_v1().map_err(map_profile_error)?;
        let namespaces = NamespaceSetV1::capture_self().map_err(map_profile_error)?;
        let reap_slot = deferred_reaper().reserve()?;

        let (profile_ready_reader, profile_ready_writer) =
            protected_pipe(PipeFlags::NONBLOCK, "create child-profile pipe")?;
        let (gate_reader, gate_writer) = protected_pipe(PipeFlags::empty(), "create launch gate")?;
        let (exec_status_reader, exec_status_writer) =
            protected_pipe(PipeFlags::NONBLOCK, "create exec-status pipe")?;
        let staged = StagedLaunchV1::new(
            StagedLaunchInputV1 {
                launcher: &prepared.launcher,
                issuer: &prepared.issuer,
                manifest: &prepared.static_manifest_file,
                sources: &prepared.sources,
            },
            &profile_ready_writer,
            &gate_reader,
            &exec_status_writer,
        )
        .map_err(|error| match error {
            StagedLaunchErrorV1::InvalidProcessState(reason) => {
                ProtectedIssuerLaunchErrorV1::InvalidProcessState(reason)
            }
            StagedLaunchErrorV1::Io { operation, source } => io_error(operation, source.into()),
        })?;
        let child_profile = profile.as_ref().map(|profile| ChildProfileV1 {
            uid: profile.credentials().uid(),
            gid: profile.credentials().gid(),
            securebits: profile.credentials().securebits(),
            cap_last_cap: profile.cap_last_cap(),
        });
        let expected_parent_pid = prepared.static_manifest().parent_pid();
        let parent = rustix::process::Pid::from_raw(expected_parent_pid).ok_or(
            ProtectedIssuerLaunchErrorV1::InvalidProcessState("invalid launch parent PID"),
        )?;
        let launch_manifest = prepared.service_manifest().clone();
        let policy = self.policy().clone();

        let spawn_lease = fe2o3_artifact_transaction::try_acquire_artifact_process_spawn_lease_v1()
            .map_err(|_| ProtectedIssuerLaunchErrorV1::ProcessCapacity)?;
        if Instant::now() >= deadline {
            return Err(ProtectedIssuerLaunchErrorV1::Timeout("child creation"));
        }
        let mut process = spawn_child(
            &staged,
            child_profile,
            expected_parent_pid,
            [
                profile_ready_reader.as_raw_fd(),
                gate_writer.as_raw_fd(),
                exec_status_reader.as_raw_fd(),
                profile_ready_writer.as_raw_fd(),
            ],
            reap_slot,
            spawn_lease,
        )
        .map_err(map_child_error)?;
        let pid = process.pid;
        if let Err(error) = process.validate_pidfd() {
            let _ = process.cancel_and_reap();
            return Err(error);
        }

        drop(profile_ready_writer);
        drop(gate_reader);
        drop(exec_status_writer);
        drop(staged);

        let result = (|| {
            let report = await_profile_ready(
                &profile_ready_reader,
                &exec_status_reader,
                &process,
                deadline,
            )?;
            namespaces.revalidate_self().map_err(map_profile_error)?;
            namespaces
                .require_child_report(pid, parent, report.report().map_err(map_child_error)?)
                .map_err(map_profile_error)?;
            if let Some(profile) = &profile {
                profile.revalidate_current().map_err(map_profile_error)?;
                profile.revalidate_process(pid).map_err(map_profile_error)?;
            }
            self.revalidate()
                .map_err(ProtectedIssuerLaunchErrorV1::Supervisor)?;
            prepared
                .revalidate(self)
                .map_err(ProtectedIssuerLaunchErrorV1::Preparation)?;
            if Instant::now() >= deadline {
                return Err(ProtectedIssuerLaunchErrorV1::Timeout("launch gate release"));
            }
            write_gate_release(&gate_writer)?;
            drop(gate_writer);
            await_exec_status(&exec_status_reader, &process, deadline)?;
            if !process.is_live()? {
                return Err(process.exited_error("exited immediately after launcher exec"));
            }
            Ok(())
        })();
        if let Err(error) = result {
            let _ = process.cancel_and_reap();
            return Err(error);
        }

        let PreparedProtectedIssuerLaunchV1 {
            accepted,
            stdout_reader,
            stderr_reader,
            readiness_reader,
            ..
        } = prepared;
        Ok(LaunchedProtectedIssuerV1 {
            process,
            control: accepted.into_control(),
            stdout_reader,
            stderr_reader,
            readiness_reader,
            launch_manifest,
            policy,
        })
    }
}

/// Clones and immediately adopts all child custody; callers validate the pidfd next.
/// Native callers prepay `child_work` before reaching this helper.
fn spawn_child(
    staged: &StagedLaunchV1,
    child_profile: Option<ChildProfileV1>,
    expected_parent_pid: i32,
    close_in_child: [i32; 4],
    reap_slot: ReapSlotV1<'static>,
    spawn_lease: fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseV1,
) -> Result<IssuerChild, ChildProcessError> {
    let mut pidfd_raw = -1_i32;
    let clone_arguments = CloneArgsV1 {
        flags: CLONE_PIDFD | CLONE_CLEAR_SIGHAND,
        pidfd: (&raw mut pidfd_raw).addr() as u64,
        child_tid: 0,
        parent_tid: 0,
        exit_signal: SIGCHLD,
        stack: 0,
        stack_size: 0,
        tls: 0,
        set_tid: 0,
        set_tid_size: 0,
        cgroup: 0,
    };
    // SAFETY: clone3 receives the exact 88-byte Linux ABI record and no VM/thread-sharing
    // flags. The child executes only direct syscalls over preallocated state and never
    // returns into Rust cleanup. CLONE_PIDFD installs one descriptor before parent return.
    let clone_result = unsafe {
        syscall(
            SYS_CLONE3,
            &raw const clone_arguments,
            std::mem::size_of::<CloneArgsV1>(),
        )
    };
    if clone_result < 0 {
        // SAFETY: syscall set the calling thread's libc errno on failure.
        let errno = unsafe { rustix::io::Errno::from_raw_os_error(*libc::__errno_location()) };
        // No child exists: ordinary Drop releases the unused lease and reservation.
        return Err(ChildProcessError::Io {
            operation: "clone3 protected issuer with atomic pidfd",
            errno,
        });
    }
    if clone_result == 0 {
        // SAFETY: child_exec uses only direct syscalls and preallocated state, then
        // execveat or _exit. No Rust destructor runs in the post-clone child.
        unsafe {
            child_exec(
                staged,
                child_profile,
                expected_parent_pid,
                close_in_child[0],
                close_in_child[1],
                close_in_child[2],
                close_in_child[3],
            )
        }
    }

    let raw_pid = i32::try_from(clone_result).unwrap_or_else(|_| std::process::abort());
    let pid = rustix::process::Pid::from_raw(raw_pid).unwrap_or_else(|| std::process::abort());
    // Adopt all cleanup obligations before the first fallible parent operation.
    // A missing atomic pidfd must retain both the slot and the inherited-lock lease.
    let pidfd = if pidfd_raw < 0 {
        None
    } else {
        // SAFETY: successful CLONE_PIDFD installed one newly owned descriptor.
        Some(unsafe { OwnedFd::from_raw_fd(pidfd_raw) })
    };
    let cleanup = ChildCleanupV1::new(pidfd, pid, Some(spawn_lease));
    let process = IssuerChild::new(cleanup, reap_slot);
    if pidfd_raw < 0 {
        return Err(ChildProcessError::State(
            "clone3 did not return the requested pidfd",
        ));
    }
    Ok(process)
}

/// Conservative prepayment for child_exec through execveat or failure write/exit.
///
/// Each operation reserves 1024 syscall units plus 64 scalar/field-work units;
/// 256 additional units cover stack records and fixed control flow. Count five
/// endpoint closes (including both report writers), all 64 signal slots
/// (including the two skipped signals), signal-mask
/// reset, four parent-death checks, readiness write, every gate read attempt,
/// close_range, three stdio and SOURCE_COUNT_V1 + 2 descriptor installs, execveat,
/// and failure write + exit. Fresh child namespace collection is prepaid using
/// its exported bound. A profile adds eleven fixed syscalls and two for
/// every capability in 0..=cap_last_cap, including all associated field checks.
/// Checked arithmetic fails closed to usize::MAX even for unsupported ceilings.
/// This is logical work, not a bound on syscall latency or executed launcher work.
fn child_work(profile: Option<ChildProfileV1>) -> usize {
    const OPERATION_WORK: usize = 1024 + 64;
    const FIXED_OPERATIONS: usize = 5
        + KERNEL_SIGNAL_COUNT as usize
        + 1
        + 4
        + 1
        + MAX_CHILD_GATE_ATTEMPTS
        + 1
        + 3
        + crate::launch_checks::SOURCE_COUNT_V1
        + 2
        + 1
        + 2;
    let profile_operations = match profile {
        None => Some(0),
        Some(profile) => usize::try_from(profile.cap_last_cap)
            .ok()
            .and_then(|ceiling| ceiling.checked_add(1))
            .and_then(|capabilities| capabilities.checked_mul(2))
            .and_then(|operations| operations.checked_add(11)),
    };
    profile_operations
        .and_then(|operations| operations.checked_add(FIXED_OPERATIONS))
        .and_then(|operations| operations.checked_mul(OPERATION_WORK))
        .and_then(|work| work.checked_add(256))
        .and_then(|work| work.checked_add(CHILD_NAMESPACE_REPORT_CAPTURE_WORK))
        .unwrap_or(usize::MAX)
}

fn protected_pipe(
    extra: PipeFlags,
    operation: &'static str,
) -> Result<(OwnedFd, OwnedFd), ProtectedIssuerLaunchErrorV1> {
    pipe_with(PipeFlags::CLOEXEC | extra).map_err(|source| io_error(operation, source.into()))
}

fn bounded_deadline(timeout: Duration) -> Result<Instant, ProtectedIssuerLaunchErrorV1> {
    if timeout.is_zero() || timeout > MAX_LAUNCH_WAIT_V1 {
        return Err(ProtectedIssuerLaunchErrorV1::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(ProtectedIssuerLaunchErrorV1::InvalidTimeout)
}

fn session_deadline(timeout: Duration) -> Result<Instant, ProtectedIssuerLaunchErrorV1> {
    if timeout.is_zero() {
        return Err(ProtectedIssuerLaunchErrorV1::InvalidTimeout);
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or(ProtectedIssuerLaunchErrorV1::InvalidTimeout)
}

unsafe fn child_exec(
    staged: &StagedLaunchV1,
    profile: Option<ChildProfileV1>,
    expected_parent_pid: i32,
    profile_ready_reader: c_int,
    gate_writer: c_int,
    exec_status_reader: c_int,
    original_profile_writer: c_int,
) -> ! {
    // SAFETY: every call in this block is a direct Linux syscall over preallocated storage.
    unsafe {
        close(profile_ready_reader);
        close(gate_writer);
        close(exec_status_reader);
        if close(original_profile_writer) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 12);
        }
        if normalize_signal_state() != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 1);
        }
        if arm_parent_death(expected_parent_pid) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 2);
        }
        if let Some(profile) = profile
            && validate_child_profile(profile) != 0
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 3);
        }
        let mut report = [0; CHILD_NAMESPACE_REPORT_BYTES];
        if capture_child_namespace_report_pre_exec(expected_parent_pid, &mut report).is_err() {
            child_fail(staged.exec_status_writer.as_raw_fd(), 11);
        }
        if write(
            staged.profile_ready_writer.as_raw_fd(),
            report.as_ptr().cast(),
            report.len(),
        ) != CHILD_NAMESPACE_REPORT_BYTES as isize
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 4);
        }
        if close(staged.profile_ready_writer.as_raw_fd()) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 12);
        }
        let release = match read_child_gate(|release| {
            let count = syscall(
                libc::SYS_read,
                staged.gate_reader.as_raw_fd(),
                std::ptr::from_mut(release),
                1_usize,
            );
            if count < 0 {
                Err(rustix::io::Errno::from_raw_os_error(
                    *libc::__errno_location(),
                ))
            } else {
                Ok(count as usize)
            }
        }) {
            Ok(release) => release,
            Err(()) => child_fail(staged.exec_status_writer.as_raw_fd(), 5),
        };
        if release != GATE_RELEASE_V1 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 6);
        }
        if syscall(SYS_CLOSE_RANGE, 3_u32, u32::MAX, CLOSE_RANGE_CLOEXEC) != 0 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 7);
        }
        for (target, source) in staged.stdio_sources.into_iter().enumerate() {
            if dup3(source, target as i32, 0) < 0 {
                child_fail(staged.exec_status_writer.as_raw_fd(), 8);
            }
        }
        for descriptor in &staged.descriptors {
            if dup3(descriptor.source.as_raw_fd(), descriptor.target, 0) < 0 {
                child_fail(staged.exec_status_writer.as_raw_fd(), 9);
            }
        }
        let launcher_name = c"fe2o3-static-preexec-launcher";
        let arguments = [launcher_name.as_ptr(), std::ptr::null()];
        let environment = [std::ptr::null::<c_char>()];
        syscall(
            SYS_EXECVEAT,
            staged.launcher.as_raw_fd(),
            c"".as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
            AT_EMPTY_PATH,
        );
        child_fail(staged.exec_status_writer.as_raw_fd(), 10);
    }
}

fn read_child_gate(mut read: impl FnMut(&mut u8) -> rustix::io::Result<usize>) -> Result<u8, ()> {
    let mut release = 0_u8;
    for _ in 0..MAX_CHILD_GATE_ATTEMPTS {
        match read(&mut release) {
            Ok(1) => return Ok(release),
            Err(rustix::io::Errno::INTR) => {}
            Ok(_) | Err(_) => return Err(()),
        }
    }
    Err(())
}

unsafe fn arm_parent_death(expected_parent_pid: i32) -> c_int {
    let mut observed_signal = 0_i32;
    // SAFETY: these are scalar getppid/prctl operations in the direct post-clone child.
    if unsafe { syscall(libc::SYS_getppid) } != c_long::from(expected_parent_pid)
        || unsafe { prctl(PR_SET_PDEATHSIG, SIGKILL, 0, 0, 0) } != 0
        || unsafe { syscall(libc::SYS_getppid) } != c_long::from(expected_parent_pid)
        || unsafe { prctl(PR_GET_PDEATHSIG, &raw mut observed_signal, 0, 0, 0) } != 0
        || observed_signal != SIGKILL
    {
        return -1;
    }
    0
}

unsafe fn normalize_signal_state() -> c_int {
    let default_action = KernelSigactionV1 {
        handler: 0,
        flags: 0,
        restorer: 0,
        mask: 0,
    };
    for signal in 1..=KERNEL_SIGNAL_COUNT {
        if signal == SIGKILL || signal == SIGSTOP {
            continue;
        }
        // SAFETY: x86-64 rt_sigaction consumes this exact kernel layout and 8-byte sigset.
        if unsafe {
            syscall(
                SYS_RT_SIGACTION,
                signal,
                &raw const default_action,
                std::ptr::null_mut::<KernelSigactionV1>(),
                KERNEL_SIGSET_BYTES,
            )
        } != 0
        {
            return -1;
        }
    }
    let empty_mask = 0_u64;
    // SAFETY: the x86-64 kernel sigset is one u64.
    if unsafe {
        syscall(
            SYS_RT_SIGPROCMASK,
            SIG_SETMASK,
            &raw const empty_mask,
            std::ptr::null_mut::<u64>(),
            KERNEL_SIGSET_BYTES,
        )
    } != 0
    {
        return -1;
    }
    0
}

unsafe fn validate_child_profile(profile: ChildProfileV1) -> c_int {
    let mut real_uid = u32::MAX;
    let mut effective_uid = u32::MAX;
    let mut saved_uid = u32::MAX;
    let mut real_gid = u32::MAX;
    let mut effective_gid = u32::MAX;
    let mut saved_gid = u32::MAX;
    // SAFETY: all pointers name writable scalar storage and the direct child owns this stack.
    if unsafe {
        syscall(
            SYS_GETRESUID,
            &raw mut real_uid,
            &raw mut effective_uid,
            &raw mut saved_uid,
        )
    } != 0
        || [real_uid, effective_uid, saved_uid] != [profile.uid; 3]
        || unsafe {
            syscall(
                SYS_GETRESGID,
                &raw mut real_gid,
                &raw mut effective_gid,
                &raw mut saved_gid,
            )
        } != 0
        || [real_gid, effective_gid, saved_gid] != [profile.gid; 3]
        || unsafe { syscall(SYS_SETFSUID, u32::MAX) } != c_long::from(profile.uid)
        || unsafe { syscall(SYS_SETFSGID, u32::MAX) } != c_long::from(profile.gid)
    {
        return -1;
    }
    if unsafe { syscall(SYS_GETGROUPS, 0_usize, std::ptr::null_mut::<u32>()) } != 0 {
        return -1;
    }
    let mut header = LinuxCapabilityHeaderV1 {
        version: LINUX_CAPABILITY_VERSION_3,
        pid: 0,
    };
    let mut data = [LinuxCapabilityDataV1 {
        effective: u32::MAX,
        permitted: u32::MAX,
        inheritable: u32::MAX,
    }; 2];
    if unsafe { syscall(SYS_CAPGET, &raw mut header, data.as_mut_ptr()) } != 0
        || data
            .iter()
            .any(|entry| entry.effective != 0 || entry.permitted != 0 || entry.inheritable != 0)
    {
        return -1;
    }
    for capability in 0..=profile.cap_last_cap {
        if unsafe { prctl(PR_CAPBSET_READ, capability, 0, 0, 0) } != 0
            || unsafe { prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_IS_SET, capability, 0, 0) } != 0
        {
            return -1;
        }
    }
    let mut core = LinuxRlimit64V1 {
        current: u64::MAX,
        maximum: u64::MAX,
    };
    if unsafe { prctl(PR_GET_SECUREBITS, 0, 0, 0, 0) } != profile.securebits as c_int
        || unsafe { prctl(PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        || unsafe { prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0
        || unsafe {
            syscall(
                SYS_PRLIMIT64,
                0,
                RLIMIT_CORE,
                std::ptr::null::<LinuxRlimit64V1>(),
                &raw mut core,
            )
        } != 0
        || core.current != 0
        || core.maximum != 0
        || unsafe { syscall(SYS_UMASK, 0o077_u32) } != 0o077
    {
        return -1;
    }
    0
}

unsafe fn child_fail(status: c_int, stage: u8) -> ! {
    // SAFETY: status is the child's private one-byte status pipe and stage is stack-resident.
    unsafe {
        let _ = write(status, (&raw const stage).cast(), 1);
        _exit(126)
    }
}

fn await_profile_ready(
    ready: &OwnedFd,
    exec_status: &OwnedFd,
    process: &IssuerChild,
    deadline: Instant,
) -> Result<ProfileReportRead, ProtectedIssuerLaunchErrorV1> {
    let mut report = ProfileReportRead::new();
    loop {
        if Instant::now() >= deadline {
            return Err(ProtectedIssuerLaunchErrorV1::Timeout(
                "gated child profile observation",
            ));
        }
        match report.observe(|bytes| rustix::io::read(ready, bytes)) {
            Ok(Some(())) => return Ok(report),
            Err(ReportError::Truncated) => {
                return exec_failure_or_exit(exec_status, process, "profile observation");
            }
            Ok(None) => {
                if !process.is_live()? {
                    return exec_failure_or_exit(exec_status, process, "profile observation");
                }
                if Instant::now() >= deadline {
                    return Err(ProtectedIssuerLaunchErrorV1::Timeout(
                        "gated child profile observation",
                    ));
                }
                std::thread::sleep(POLL_INTERVAL_V1);
            }
            Err(error) => return Err(map_child_error(error.into())),
        }
    }
}

fn write_gate_release(gate: &OwnedFd) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    match rustix::io::write(gate, &[GATE_RELEASE_V1]) {
        Ok(1) => Ok(()),
        Ok(_) => Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
            "launch gate accepted a partial release record",
        )),
        Err(source) => Err(io_error(
            "release protected issuer launch gate",
            source.into(),
        )),
    }
}

fn await_exec_status(
    status: &OwnedFd,
    process: &IssuerChild,
    deadline: Instant,
) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    let mut record = [0_u8; 2];
    loop {
        match rustix::io::read(status, &mut record) {
            Ok(0) => return Ok(()),
            Ok(1) => return Err(ProtectedIssuerLaunchErrorV1::ChildStage(record[0])),
            Ok(_) => {
                return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "child emitted a noncanonical exec-status record",
                ));
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                if !process.is_live()? {
                    return exec_failure_or_exit(status, process, "static-launcher exec");
                }
                if Instant::now() >= deadline {
                    return Err(ProtectedIssuerLaunchErrorV1::Timeout(
                        "authenticated static-launcher exec",
                    ));
                }
                std::thread::sleep(POLL_INTERVAL_V1);
            }
            Err(source) => return Err(io_error("read child exec status", source.into())),
        }
    }
}

fn exec_failure_or_exit<T>(
    status: &OwnedFd,
    process: &IssuerChild,
    boundary: &'static str,
) -> Result<T, ProtectedIssuerLaunchErrorV1> {
    let mut record = [0_u8; 2];
    match rustix::io::read(status, &mut record) {
        Ok(1) => Err(ProtectedIssuerLaunchErrorV1::ChildStage(record[0])),
        Ok(0) | Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
            Err(process.exited_error(&format!("exited before {boundary}")))
        }
        Ok(_) => Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
            "child emitted a noncanonical failure record",
        )),
        Err(source) => Err(io_error("read failed child status", source.into())),
    }
}

fn await_readiness_record(
    reader: &OwnedFd,
    process: &IssuerChild,
    launch: &CompilerExecutionServiceLaunchManifestV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    deadline: Instant,
) -> Result<CompilerExecutionServiceReadyV1, ProtectedIssuerLaunchErrorV1> {
    let mut bytes = [0_u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1];
    let mut used = 0_usize;
    loop {
        let result = if used < bytes.len() {
            rustix::io::read(reader, &mut bytes[used..])
        } else {
            let mut trailing = [0_u8; 1];
            match rustix::io::read(reader, &mut trailing) {
                Ok(0) => break,
                Ok(_) => return Err(ProtectedIssuerLaunchErrorV1::ReadinessTrailingBytes),
                Err(error) => Err(error),
            }
        };
        match result {
            Ok(0) => return Err(ProtectedIssuerLaunchErrorV1::ReadinessTruncated),
            Ok(count) => used += count,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                if !process.is_live()? {
                    return Err(process.exited_error("exited before readiness"));
                }
                if Instant::now() >= deadline {
                    return Err(ProtectedIssuerLaunchErrorV1::Timeout(
                        "exact issuer readiness",
                    ));
                }
                std::thread::sleep(POLL_INTERVAL_V1);
            }
            Err(source) => return Err(io_error("read protected issuer readiness", source.into())),
        }
    }
    let readiness = CompilerExecutionServiceReadyV1::decode(&bytes)
        .map_err(ProtectedIssuerLaunchErrorV1::ReadinessProtocol)?;
    if !readiness.matches_launch(process.pid_u32(), launch, policy) {
        return Err(ProtectedIssuerLaunchErrorV1::ReadinessMismatch);
    }
    Ok(readiness)
}

fn publish_control_readiness(
    control: &OwnedFd,
    bytes: &[u8],
    process: &IssuerChild,
    deadline: Instant,
) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    loop {
        match rustix::net::send(control, bytes, SendFlags::DONTWAIT | SendFlags::NOSIGNAL) {
            Ok(count) if count == bytes.len() => return Ok(()),
            Ok(_) => {
                return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "Cargo control accepted a partial readiness packet",
                ));
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
                if !process.is_live()? {
                    return Err(process.exited_error("exited before Cargo readiness publication"));
                }
                if Instant::now() >= deadline {
                    return Err(ProtectedIssuerLaunchErrorV1::Timeout(
                        "Cargo readiness publication",
                    ));
                }
                std::thread::sleep(POLL_INTERVAL_V1);
            }
            Err(source) => {
                return Err(io_error(
                    "publish protected issuer readiness to Cargo",
                    source.into(),
                ));
            }
        }
    }
}

/// Validates the complete current locked service profile before listener admission.
///
/// This check grants no process, signing, compiler, publication, load, launch, or GPU authority.
/// Production launch repeats the same profile checks and additionally gates the exact child.
pub fn validate_current_issuer_service_profile_v1(
    credentials: IssuerServiceCredentialProfileV1,
) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    validate_current_protected_service_profile_v1(credentials).map_err(map_profile_error)
}

fn map_profile_error(error: ProtectedServiceProfileErrorV1) -> ProtectedIssuerLaunchErrorV1 {
    match error {
        ProtectedServiceProfileErrorV1::ProcessProfile(reason) => {
            ProtectedIssuerLaunchErrorV1::ProcessProfile(reason)
        }
        ProtectedServiceProfileErrorV1::Namespace(namespace) => {
            ProtectedIssuerLaunchErrorV1::Namespace(namespace)
        }
        ProtectedServiceProfileErrorV1::InvalidState(reason) => {
            ProtectedIssuerLaunchErrorV1::InvalidProcessState(reason)
        }
        ProtectedServiceProfileErrorV1::Io { operation, source } => {
            ProtectedIssuerLaunchErrorV1::Io { operation, source }
        }
        _ => ProtectedIssuerLaunchErrorV1::InvalidProcessState(
            "unrecognized protected-service profile failure",
        ),
    }
}

struct IssuerChild {
    cleanup: Option<ChildCleanupV1>,
    pid: rustix::process::Pid,
    reap_slot: Option<ReapSlotV1<'static>>,
    cleanup_poll: CleanupPollV1,
}

impl IssuerChild {
    fn new(cleanup: ChildCleanupV1, reap_slot: ReapSlotV1<'static>) -> Self {
        let pid = cleanup.pid();
        Self {
            cleanup: Some(cleanup),
            pid,
            reap_slot: Some(reap_slot),
            cleanup_poll: CleanupPollV1::Pending,
        }
    }

    fn pidfd(&self) -> Result<&OwnedFd, ChildProcessError> {
        self.cleanup
            .as_ref()
            .and_then(ChildCleanupV1::pidfd)
            .ok_or(ChildProcessError::State(
                "pidfd is absent or cleanup custody was transferred",
            ))
    }

    fn release_spawn_after_exec(&mut self) {
        if let Some(cleanup) = self.cleanup.as_mut() {
            cleanup.release_spawn_after_exec();
        }
    }

    fn validate_pidfd(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        self.check_pidfd().map_err(map_child_error)
    }

    fn check_pidfd(&self) -> Result<(), ChildProcessError> {
        let pidfd = self.pidfd()?;
        let flags = rustix::io::fcntl_getfd(pidfd).map_err(|errno| ChildProcessError::Io {
            operation: "inspect clone3 pidfd",
            errno,
        })?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(ChildProcessError::State("clone3 pidfd is inheritable"));
        }
        Ok(())
    }

    fn pid_u32(&self) -> u32 {
        u32::try_from(self.pid.as_raw_pid()).expect("clone3 child PID is positive")
    }

    fn is_live(&self) -> Result<bool, ProtectedIssuerLaunchErrorV1> {
        self.observe_live().map_err(map_child_error)
    }

    fn observe_live(&self) -> Result<bool, ChildProcessError> {
        self.wait_once(
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
            "observe exact issuer pidfd",
        )
        .map(|status| status.is_none())
    }

    fn wait_once(
        &self,
        options: rustix::process::WaitIdOptions,
        operation: &'static str,
    ) -> Result<Option<rustix::process::WaitIdStatus>, ChildProcessError> {
        let pidfd = self.pidfd()?;
        let result =
            rustix::process::waitid(rustix::process::WaitId::PidFd(pidfd.as_fd()), options);
        child_wait_result(self.cleanup.as_ref(), operation, result)
    }

    /// Performs one consuming NOHANG wait without retrying or formatting diagnostics.
    fn try_reap(&mut self) -> Result<Option<ProtectedIssuerTerminationV1>, ChildProcessError> {
        let Some(status) = self.wait_once(
            rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOHANG,
            "reap naturally exited issuer pidfd",
        )?
        else {
            return Ok(None);
        };
        if matches!(
            status.raw_code(),
            libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
        ) {
            // Record the consuming terminal wait before any fallible status decoding.
            // Even an unexpected decoding failure must never provoke another wait.
            self.complete_reaped();
        }
        ProtectedIssuerTerminationV1::from_wait_status(&status).map(Some)
    }

    fn exited_error(&self, context: &str) -> ProtectedIssuerLaunchErrorV1 {
        let detail = self
            .wait_once(
                rustix::process::WaitIdOptions::EXITED
                    | rustix::process::WaitIdOptions::NOHANG
                    | rustix::process::WaitIdOptions::NOWAIT,
                "observe exact issuer pidfd",
            )
            .ok()
            .flatten()
            .map(|status| describe_exit(&status))
            .unwrap_or_else(|| context.to_owned());
        ProtectedIssuerLaunchErrorV1::ChildExited(detail)
    }

    /// Makes one shared cleanup attempt, then completes or transfers all custody.
    /// Repeated calls preserve the disposition; later Drop performs no further I/O.
    fn cancel_once(&mut self) -> CleanupPollV1 {
        let Some(cleanup) = self.cleanup.as_mut() else {
            return self.cleanup_poll;
        };
        let poll = cleanup.step();
        match poll {
            CleanupPollV1::Reaped => self.complete_reaped(),
            CleanupPollV1::Pending | CleanupPollV1::Quarantined => self.defer_cleanup(poll),
        }
        poll
    }

    fn cancel_and_reap(&mut self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        if self.cleanup.is_none() {
            return Ok(());
        }
        let deadline = Instant::now() + MAX_CANCEL_WAIT_V1;
        for _ in 0..MAX_CANCEL_POLLS_V1 {
            match self
                .cleanup
                .as_mut()
                .expect("foreground cleanup owner")
                .step()
            {
                CleanupPollV1::Reaped => {
                    self.complete_reaped();
                    return Ok(());
                }
                CleanupPollV1::Quarantined => {
                    self.defer_cleanup(CleanupPollV1::Quarantined);
                    return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "issuer cleanup ownership is uncertain; custody retained in quarantine",
                    ));
                }
                CleanupPollV1::Pending => {}
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(POLL_INTERVAL_V1);
        }
        let last_errno = self.cleanup.as_ref().and_then(ChildCleanupV1::last_errno);
        self.defer_cleanup(CleanupPollV1::Pending);
        Err(match last_errno {
            Some(source) => io_error(
                "cancel issuer; deferred cleanup retains custody",
                source.into(),
            ),
            None => ProtectedIssuerLaunchErrorV1::Timeout(
                "issuer cancellation; deferred cleanup retains custody",
            ),
        })
    }

    fn wait_and_reap(
        &mut self,
        deadline: Instant,
    ) -> Result<ProtectedIssuerTerminationV1, ProtectedIssuerLaunchErrorV1> {
        loop {
            match self.try_reap() {
                Ok(Some(termination)) => return Ok(termination),
                Ok(None)
                | Err(ChildProcessError::Io {
                    errno: rustix::io::Errno::INTR,
                    ..
                }) => {
                    wait_for_pidfd_exit(self.pidfd().map_err(map_child_error)?, deadline)?;
                }
                Err(ChildProcessError::Io {
                    errno: rustix::io::Errno::CHILD,
                    ..
                }) => {
                    return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "issuer child was reaped outside its pidfd owner",
                    ));
                }
                Err(error) => return Err(map_child_error(error)),
            }
        }
    }

    fn complete_reaped(&mut self) {
        if let Some(cleanup) = self.cleanup.as_mut() {
            cleanup.terminal_reaped();
        }
        drop(self.cleanup.take());
        self.reap_slot
            .take()
            .expect("live issuer child retains one reap slot")
            .complete();
        self.cleanup_poll = CleanupPollV1::Reaped;
    }

    fn defer_cleanup(&mut self, poll: CleanupPollV1) {
        let Some(cleanup) = self.cleanup.take() else {
            return;
        };
        let slot = self
            .reap_slot
            .take()
            .expect("live issuer child retains one reap slot");
        slot.defer(cleanup);
        self.cleanup_poll = poll;
    }
}

fn child_wait_result<T>(
    cleanup: Option<&ChildCleanupV1>,
    operation: &'static str,
    result: rustix::io::Result<Option<T>>,
) -> Result<Option<T>, ChildProcessError> {
    result.map_err(|errno| {
        if errno == rustix::io::Errno::CHILD
            && let Some(cleanup) = cleanup
        {
            cleanup.ownership_lost();
        }
        ChildProcessError::Io { operation, errno }
    })
}

fn wait_for_pidfd_exit(
    pidfd: &OwnedFd,
    deadline: Instant,
) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProtectedIssuerLaunchErrorV1::Timeout("natural issuer exit"));
        }
        let timeout = Timespec {
            tv_sec: i64::try_from(remaining.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(remaining.subsec_nanos()),
        };
        let mut descriptors = [PollFd::new(
            pidfd,
            PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
        )];
        match poll(&mut descriptors, Some(&timeout)) {
            Ok(0) => {
                return Err(ProtectedIssuerLaunchErrorV1::Timeout("natural issuer exit"));
            }
            Ok(_) => {
                let events = descriptors[0].revents();
                if events.contains(PollFlags::NVAL) {
                    return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "issuer pidfd became invalid while awaiting exit",
                    ));
                }
                if events.intersects(PollFlags::IN | PollFlags::ERR | PollFlags::HUP) {
                    return Ok(());
                }
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => {
                return Err(io_error(
                    "poll naturally exiting issuer pidfd",
                    source.into(),
                ));
            }
        }
    }
}

impl Drop for IssuerChild {
    fn drop(&mut self) {
        let _ = self.cancel_once();
    }
}

fn map_child_error(error: ChildProcessError) -> ProtectedIssuerLaunchErrorV1 {
    match error {
        ChildProcessError::State(reason) => {
            ProtectedIssuerLaunchErrorV1::InvalidProcessState(reason)
        }
        ChildProcessError::Io { operation, errno } => io_error(operation, errno.into()),
    }
}

fn describe_exit(status: &rustix::process::WaitIdStatus) -> String {
    if let Some(code) = status.exit_status() {
        format!("exited with status {code}")
    } else if let Some(signal) = status.terminating_signal() {
        format!("terminated by signal {signal}")
    } else {
        "ended without a canonical exit status".to_owned()
    }
}

fn io_error(operation: &'static str, source: io::Error) -> ProtectedIssuerLaunchErrorV1 {
    ProtectedIssuerLaunchErrorV1::Io { operation, source }
}

#[cfg(test)]
#[path = "process_mechanics_tests.rs"]
mod mechanics_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_namespace_snapshot_revalidates_without_drift() {
        let namespaces = NamespaceSetV1::capture_self().unwrap();
        namespaces.revalidate_self().unwrap();
    }

    #[test]
    fn lifecycle_timeout_is_strictly_bounded() {
        assert!(matches!(
            bounded_deadline(Duration::ZERO),
            Err(ProtectedIssuerLaunchErrorV1::InvalidTimeout)
        ));
        assert!(matches!(
            bounded_deadline(MAX_LAUNCH_WAIT_V1 + Duration::from_nanos(1)),
            Err(ProtectedIssuerLaunchErrorV1::InvalidTimeout)
        ));
        bounded_deadline(Duration::from_secs(1)).unwrap();
    }
}
