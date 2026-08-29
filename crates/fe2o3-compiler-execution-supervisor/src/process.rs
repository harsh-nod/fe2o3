//! Pidfd-owned launch and readiness lifecycle for the protected issuer.

use core::ffi::{c_char, c_int, c_long, c_void};
use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionServiceLaunchManifestV1, CompilerExecutionServiceReadyErrorV1,
    CompilerExecutionServiceReadyV1,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceProfileErrorV1, validate_current_protected_service_profile_v1,
};
use fe2o3_static_preexec_manifest::{
    PREEXEC_EXECUTABLE_FD, PREEXEC_MANIFEST_FD, PREEXEC_MAX_DESCRIPTORS, PREEXEC_SOURCE_FD_BASE,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::net::SendFlags;
use rustix::pipe::{PipeFlags, pipe_with};

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
const PROFILE_READY_V1: u8 = 0xa5;
const GATE_RELEASE_V1: u8 = 0x5a;
const MAX_PROC_STATUS_BYTES_V1: u64 = 64 * 1024;
const MAX_CAPABILITY_NUMBER_V1: u32 = 63;
const MAX_LAUNCH_WAIT_V1: Duration = Duration::from_secs(120);
const POLL_INTERVAL_V1: Duration = Duration::from_millis(1);
const REAPER_POLL_INTERVAL_V1: Duration = Duration::from_millis(10);
const REAP_SLOT_EMPTY: u8 = 0;
const REAP_SLOT_RESERVED: u8 = 1;
const REAP_SLOT_DEFERRED: u8 = 2;
const FIXED_LAUNCHER_FD_CEILING: i32 = PREEXEC_SOURCE_FD_BASE + PREEXEC_MAX_DESCRIPTORS as i32 - 1;
const STAGED_DESCRIPTOR_FLOOR: i32 = FIXED_LAUNCHER_FD_CEILING + 1;

/// Maximum number of protected issuer children owned or awaiting deferred reaping.
pub const MAX_PROTECTED_ISSUER_PROCESSES_V1: usize = 64;

const _: () = assert!(PREEXEC_MANIFEST_FD == 198);
const _: () = assert!(PREEXEC_EXECUTABLE_FD == 199);
const _: () = assert!(PREEXEC_SOURCE_FD_BASE == 200);
const _: () = assert!(STAGED_DESCRIPTOR_FLOOR == 216);

unsafe extern "C" {
    fn close(descriptor: c_int) -> c_int;
    fn close_range(first: u32, last: u32, flags: u32) -> c_int;
    fn dup3(old_descriptor: c_int, new_descriptor: c_int, flags: c_int) -> c_int;
    fn execveat(
        descriptor: c_int,
        path: *const c_char,
        arguments: *const *const c_char,
        environment: *const *const c_char,
        flags: c_int,
    ) -> c_int;
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

struct StagedDescriptorV1 {
    source: OwnedFd,
    target: i32,
}

struct StagedLaunchV1 {
    launcher: OwnedFd,
    descriptors: Vec<StagedDescriptorV1>,
    stdio_sources: [i32; 3],
    profile_ready_writer: OwnedFd,
    gate_reader: OwnedFd,
    exec_status_writer: OwnedFd,
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
/// the pidfd and transfers exactly-once reaping to a bounded internal reaper.
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
    process: ProtectedIssuerChildV1,
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
        self,
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

    /// Cancels the exact child through its pidfd and synchronously reaps it once.
    pub fn cancel(mut self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        self.process.cancel_and_reap()
    }

    #[cfg(test)]
    pub(crate) fn stdout_reader_for_test(&self) -> &OwnedFd {
        &self.stdout_reader
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
    process: ProtectedIssuerChildV1,
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

    /// Cancels the exact child through its pidfd and synchronously reaps it once.
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
    process: ProtectedIssuerChildV1,
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

    fn from_wait_status(
        status: &rustix::process::WaitIdStatus,
    ) -> Result<Self, ProtectedIssuerLaunchErrorV1> {
        if let Some(status) = status.exit_status() {
            return Ok(Self::Exited { status });
        }
        if let Some(signal) = status.terminating_signal() {
            return Ok(Self::Signaled {
                signal,
                core_dumped: status.dumped(),
            });
        }
        Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
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

    /// Cancels the exact serving child and synchronously reaps it once.
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
    /// the launcher until the parent independently checks procfs and every
    /// namespace. The returned value is not ready issuer authority.
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
            Some(ExactProcessProfileV1::capture(self.credentials())?)
        } else {
            None
        };
        require_owned_sigchld()?;
        let namespaces = NamespaceSetV1::capture_self()?;
        let reap_slot = deferred_reaper().reserve()?;

        let (profile_ready_reader, profile_ready_writer) =
            protected_pipe(PipeFlags::NONBLOCK, "create child-profile pipe")?;
        let (gate_reader, gate_writer) = protected_pipe(PipeFlags::empty(), "create launch gate")?;
        let (exec_status_reader, exec_status_writer) =
            protected_pipe(PipeFlags::NONBLOCK, "create exec-status pipe")?;
        let staged = StagedLaunchV1::new(
            &prepared,
            &profile_ready_writer,
            &gate_reader,
            &exec_status_writer,
        )?;
        let child_profile = profile.as_ref().map(ExactProcessProfileV1::child_profile);
        let expected_parent_pid = prepared.static_manifest().parent_pid();
        let launch_manifest = prepared.service_manifest().clone();
        let policy = self.policy().clone();

        fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| {
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
                return Err(io_error(
                    "clone3 protected issuer with atomic pidfd",
                    io::Error::last_os_error(),
                ));
            }
            if clone_result == 0 {
                // SAFETY: this is the post-clone child. child_exec performs direct syscalls only
                // and terminates with execveat or _exit, so no Rust destructor can run here.
                unsafe {
                    child_exec(
                        &staged,
                        child_profile,
                        expected_parent_pid,
                        profile_ready_reader.as_raw_fd(),
                        gate_writer.as_raw_fd(),
                        exec_status_reader.as_raw_fd(),
                    )
                }
            }

            let raw_pid = i32::try_from(clone_result).unwrap_or_else(|_| std::process::abort());
            let pid =
                rustix::process::Pid::from_raw(raw_pid).unwrap_or_else(|| std::process::abort());
            if pidfd_raw < 0 {
                kill_and_reap_pid_synchronously(pid);
                return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "clone3 did not return the requested pidfd",
                ));
            }
            // SAFETY: successful CLONE_PIDFD installed one newly owned descriptor in pidfd_raw.
            let pidfd = unsafe { OwnedFd::from_raw_fd(pidfd_raw) };
            let mut process = ProtectedIssuerChildV1::new(pidfd, pid, reap_slot);
            if let Err(error) = process.validate_pidfd() {
                let _ = process.cancel_and_reap();
                return Err(error);
            }

            drop(profile_ready_writer);
            drop(gate_reader);
            drop(exec_status_writer);
            drop(staged);

            let result = (|| {
                await_profile_ready(
                    &profile_ready_reader,
                    &exec_status_reader,
                    &process,
                    deadline,
                )?;
                namespaces.revalidate_self()?;
                namespaces.revalidate_child(pid)?;
                if let Some(profile) = &profile {
                    profile.revalidate_current()?;
                    profile.revalidate_child(pid)?;
                }
                self.revalidate()
                    .map_err(ProtectedIssuerLaunchErrorV1::Supervisor)?;
                prepared
                    .revalidate(self)
                    .map_err(ProtectedIssuerLaunchErrorV1::Preparation)?;
                write_gate_release(&gate_writer)?;
                drop(gate_writer);
                await_exec_status(&exec_status_reader, &process, deadline)?;
                if !process.is_live()? {
                    return Err(process.exited_error("exited immediately after launcher exec"));
                }
                Ok(())
            })();
            if let Err(error) = result {
                let cleanup = process.cancel_and_reap();
                return match cleanup {
                    Ok(()) => Err(error),
                    Err(cleanup) => Err(cleanup),
                };
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
        })
    }
}

impl StagedLaunchV1 {
    fn new(
        prepared: &PreparedProtectedIssuerLaunchV1,
        profile_ready_writer: &OwnedFd,
        gate_reader: &OwnedFd,
        exec_status_writer: &OwnedFd,
    ) -> Result<Self, ProtectedIssuerLaunchErrorV1> {
        let mut next = STAGED_DESCRIPTOR_FLOOR;
        let launcher = duplicate_above(&prepared.launcher, &mut next, "stage static launcher")?;
        let mut descriptors = Vec::with_capacity(2 + prepared.sources.len());
        descriptors.push(StagedDescriptorV1 {
            source: duplicate_above(
                &prepared.static_manifest_file,
                &mut next,
                "stage static launch manifest",
            )?,
            target: PREEXEC_MANIFEST_FD,
        });
        descriptors.push(StagedDescriptorV1 {
            source: duplicate_above(&prepared.issuer, &mut next, "stage issuer executable")?,
            target: PREEXEC_EXECUTABLE_FD,
        });
        for (index, source) in prepared.sources.iter().enumerate() {
            let target = PREEXEC_SOURCE_FD_BASE
                + i32::try_from(index).map_err(|_| {
                    ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "prepared source index exceeds i32",
                    )
                })?;
            descriptors.push(StagedDescriptorV1 {
                source: duplicate_above(source, &mut next, "stage issuer source descriptor")?,
                target,
            });
        }
        let stdio_sources = [
            descriptors[2].source.as_raw_fd(),
            descriptors[3].source.as_raw_fd(),
            descriptors[4].source.as_raw_fd(),
        ];
        let profile_ready_writer = duplicate_above(
            profile_ready_writer,
            &mut next,
            "stage child-profile writer",
        )?;
        let gate_reader = duplicate_above(gate_reader, &mut next, "stage launch-gate reader")?;
        let exec_status_writer =
            duplicate_above(exec_status_writer, &mut next, "stage exec-status writer")?;
        Ok(Self {
            launcher,
            descriptors,
            stdio_sources,
            profile_ready_writer,
            gate_reader,
            exec_status_writer,
        })
    }
}

fn duplicate_above(
    source: &impl AsFd,
    next: &mut i32,
    operation: &'static str,
) -> Result<OwnedFd, ProtectedIssuerLaunchErrorV1> {
    let duplicate = rustix::io::fcntl_dupfd_cloexec(source, *next)
        .map_err(|source| io_error(operation, source.into()))?;
    *next = duplicate.as_raw_fd().checked_add(1).ok_or(
        ProtectedIssuerLaunchErrorV1::InvalidProcessState("staged descriptor range overflowed"),
    )?;
    Ok(duplicate)
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
) -> ! {
    // SAFETY: every call in this block is a direct Linux syscall over preallocated storage.
    unsafe {
        close(profile_ready_reader);
        close(gate_writer);
        close(exec_status_reader);
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
        let ready = PROFILE_READY_V1;
        if write(
            staged.profile_ready_writer.as_raw_fd(),
            (&raw const ready).cast(),
            1,
        ) != 1
        {
            child_fail(staged.exec_status_writer.as_raw_fd(), 4);
        }
        let mut release = 0_u8;
        loop {
            let count = syscall(
                libc::SYS_read,
                staged.gate_reader.as_raw_fd(),
                &raw mut release,
                1_usize,
            );
            if count == 1 {
                break;
            }
            if count == 0 {
                child_fail(staged.exec_status_writer.as_raw_fd(), 5);
            }
            if count < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            child_fail(staged.exec_status_writer.as_raw_fd(), 5);
        }
        if release != GATE_RELEASE_V1 {
            child_fail(staged.exec_status_writer.as_raw_fd(), 6);
        }
        if close_range(3, u32::MAX, CLOSE_RANGE_CLOEXEC) != 0 {
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
        execveat(
            staged.launcher.as_raw_fd(),
            c"".as_ptr(),
            arguments.as_ptr(),
            environment.as_ptr(),
            AT_EMPTY_PATH,
        );
        child_fail(staged.exec_status_writer.as_raw_fd(), 10);
    }
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
    process: &ProtectedIssuerChildV1,
    deadline: Instant,
) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    let mut record = [0_u8; 2];
    loop {
        match rustix::io::read(ready, &mut record) {
            Ok(1) if record[0] == PROFILE_READY_V1 => return Ok(()),
            Ok(0) => return exec_failure_or_exit(exec_status, process, "profile observation"),
            Ok(_) => {
                return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                    "child emitted a noncanonical profile record",
                ));
            }
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => {
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
            Err(source) => {
                return Err(io_error("read gated child profile", source.into()));
            }
        }
    }
}

fn write_gate_release(gate: &OwnedFd) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    loop {
        match rustix::io::write(gate, &[GATE_RELEASE_V1]) {
            Ok(1) => return Ok(()),
            Ok(_) => {
                return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "launch gate accepted a partial release record",
                ));
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => {
                return Err(io_error(
                    "release protected issuer launch gate",
                    source.into(),
                ));
            }
        }
    }
}

fn await_exec_status(
    status: &OwnedFd,
    process: &ProtectedIssuerChildV1,
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

fn exec_failure_or_exit(
    status: &OwnedFd,
    process: &ProtectedIssuerChildV1,
    boundary: &'static str,
) -> Result<(), ProtectedIssuerLaunchErrorV1> {
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
    process: &ProtectedIssuerChildV1,
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
    process: &ProtectedIssuerChildV1,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcStatusProfileV1 {
    uid: [u32; 4],
    gid: [u32; 4],
    groups_empty: bool,
    capabilities_zero: bool,
    no_new_privs: u32,
    tracer_pid: u32,
    umask: u32,
}

impl ProcStatusProfileV1 {
    fn parse(bytes: &[u8]) -> Result<Self, ProtectedIssuerLaunchErrorV1> {
        let text = std::str::from_utf8(bytes).map_err(|_| {
            ProtectedIssuerLaunchErrorV1::ProcessProfile("proc status is not UTF-8")
        })?;
        let mut uid = None;
        let mut gid = None;
        let mut groups_empty = None;
        let mut capabilities = [None; 5];
        let mut no_new_privs = None;
        let mut tracer_pid = None;
        let mut umask = None;
        for line in text.lines() {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match name {
                "Uid" => set_once(&mut uid, parse_four_decimal(value)?)?,
                "Gid" => set_once(&mut gid, parse_four_decimal(value)?)?,
                "Groups" => set_once(&mut groups_empty, value.is_empty())?,
                "CapInh" => set_once(&mut capabilities[0], parse_hex_u64(value)?)?,
                "CapPrm" => set_once(&mut capabilities[1], parse_hex_u64(value)?)?,
                "CapEff" => set_once(&mut capabilities[2], parse_hex_u64(value)?)?,
                "CapBnd" => set_once(&mut capabilities[3], parse_hex_u64(value)?)?,
                "CapAmb" => set_once(&mut capabilities[4], parse_hex_u64(value)?)?,
                "NoNewPrivs" => set_once(&mut no_new_privs, parse_decimal(value)?)?,
                "TracerPid" => set_once(&mut tracer_pid, parse_decimal(value)?)?,
                "Umask" => set_once(&mut umask, parse_octal(value)?)?,
                _ => {}
            }
        }
        Ok(Self {
            uid: uid.ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "proc status lacks Uid",
            ))?,
            gid: gid.ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "proc status lacks Gid",
            ))?,
            groups_empty: groups_empty.ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "proc status lacks Groups",
            ))?,
            capabilities_zero: capabilities
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                    "proc status lacks a capability set",
                ))?
                .into_iter()
                .all(|value| value == 0),
            no_new_privs: no_new_privs.ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "proc status lacks NoNewPrivs",
            ))?,
            tracer_pid: tracer_pid.ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "proc status lacks TracerPid",
            ))?,
            umask: umask.ok_or(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "proc status lacks Umask",
            ))?,
        })
    }

    fn require(
        self,
        credentials: IssuerServiceCredentialProfileV1,
    ) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        if self.uid != [credentials.uid(); 4] {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "real, effective, saved, or filesystem UID differs",
            ));
        }
        if self.gid != [credentials.gid(); 4] {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "real, effective, saved, or filesystem GID differs",
            ));
        }
        if !self.groups_empty {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "supplementary group set is not empty",
            ));
        }
        if !self.capabilities_zero {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "a capability set is not empty",
            ));
        }
        if self.no_new_privs != 1 {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "no_new_privs is not set",
            ));
        }
        if self.tracer_pid != 0 {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "supervisor or child is traced",
            ));
        }
        if self.umask != 0o077 {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "umask is not 077",
            ));
        }
        Ok(())
    }
}

struct ExactProcessProfileV1 {
    credentials: IssuerServiceCredentialProfileV1,
    cap_last_cap: u32,
}

impl ExactProcessProfileV1 {
    fn capture(
        credentials: IssuerServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedIssuerLaunchErrorV1> {
        let profile = Self {
            credentials,
            cap_last_cap: read_cap_last_cap()?,
        };
        profile.revalidate_current()?;
        Ok(profile)
    }

    fn child_profile(&self) -> ChildProfileV1 {
        ChildProfileV1 {
            uid: self.credentials.uid(),
            gid: self.credentials.gid(),
            securebits: self.credentials.securebits(),
            cap_last_cap: self.cap_last_cap,
        }
    }

    fn revalidate_current(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        read_proc_status("/proc/self/status")?.require(self.credentials)?;
        let capabilities = rustix::thread::capabilities(None)
            .map_err(|source| io_error("inspect supervisor capabilities", source.into()))?;
        if !capabilities.effective.is_empty()
            || !capabilities.permitted.is_empty()
            || !capabilities.inheritable.is_empty()
        {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "effective, permitted, or inheritable capabilities are not empty",
            ));
        }
        let securebits = rustix::thread::capabilities_secure_bits()
            .map_err(|source| io_error("inspect supervisor securebits", source.into()))?;
        if securebits.bits() != self.credentials.securebits() {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "securebits are not exact and locked",
            ));
        }
        if !rustix::thread::no_new_privs()
            .map_err(|source| io_error("inspect supervisor no_new_privs", source.into()))?
        {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "no_new_privs is not set",
            ));
        }
        if rustix::process::dumpable_behavior()
            .map_err(|source| io_error("inspect supervisor dumpability", source.into()))?
            != rustix::process::DumpableBehavior::NotDumpable
        {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "process is dumpable",
            ));
        }
        let core = rustix::process::getrlimit(rustix::process::Resource::Core);
        if core.current != Some(0) || core.maximum != Some(0) {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "core limit is not exactly zero",
            ));
        }
        if read_cap_last_cap()? != self.cap_last_cap {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
                "kernel capability range changed",
            ));
        }
        Ok(())
    }

    fn revalidate_child(
        &self,
        pid: rustix::process::Pid,
    ) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        let path = format!("/proc/{}/status", pid.as_raw_pid());
        read_proc_status(&path)?.require(self.credentials)
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

fn read_proc_status(path: &str) -> Result<ProcStatusProfileV1, ProtectedIssuerLaunchErrorV1> {
    let file = File::open(path).map_err(|source| io_error("open proc process status", source))?;
    let mut bytes = Vec::new();
    file.take(MAX_PROC_STATUS_BYTES_V1 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error("read proc process status", source))?;
    if bytes.len() as u64 > MAX_PROC_STATUS_BYTES_V1 {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "proc status exceeds the fixed bound",
        ));
    }
    ProcStatusProfileV1::parse(&bytes)
}

fn read_cap_last_cap() -> Result<u32, ProtectedIssuerLaunchErrorV1> {
    let text = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .map_err(|source| io_error("read kernel capability ceiling", source))?;
    let value = text.trim().parse::<u32>().map_err(|_| {
        ProtectedIssuerLaunchErrorV1::ProcessProfile("kernel capability ceiling is malformed")
    })?;
    if value > MAX_CAPABILITY_NUMBER_V1 {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "kernel capability ceiling exceeds the supported 64-bit set",
        ));
    }
    Ok(value)
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), ProtectedIssuerLaunchErrorV1> {
    if slot.replace(value).is_some() {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "proc status duplicates a security field",
        ));
    }
    Ok(())
}

fn parse_four_decimal(value: &str) -> Result<[u32; 4], ProtectedIssuerLaunchErrorV1> {
    let fields = value
        .split_ascii_whitespace()
        .map(parse_decimal)
        .collect::<Result<Vec<_>, _>>()?;
    fields.try_into().map_err(|_| {
        ProtectedIssuerLaunchErrorV1::ProcessProfile("proc identity does not have four fields")
    })
}

fn parse_decimal(value: &str) -> Result<u32, ProtectedIssuerLaunchErrorV1> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "proc decimal field is malformed",
        ));
    }
    value
        .parse()
        .map_err(|_| ProtectedIssuerLaunchErrorV1::ProcessProfile("proc decimal field overflows"))
}

fn parse_hex_u64(value: &str) -> Result<u64, ProtectedIssuerLaunchErrorV1> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "proc capability field is malformed",
        ));
    }
    u64::from_str_radix(value, 16).map_err(|_| {
        ProtectedIssuerLaunchErrorV1::ProcessProfile("proc capability field overflows")
    })
}

fn parse_octal(value: &str) -> Result<u32, ProtectedIssuerLaunchErrorV1> {
    if value.is_empty() || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "proc umask field is malformed",
        ));
    }
    u32::from_str_radix(value, 8)
        .map_err(|_| ProtectedIssuerLaunchErrorV1::ProcessProfile("proc umask field overflows"))
}

const NAMESPACE_NAMES_V1: [&str; 10] = [
    "user",
    "mnt",
    "pid",
    "pid_for_children",
    "net",
    "ipc",
    "uts",
    "cgroup",
    "time",
    "time_for_children",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NamespaceIdentityV1 {
    device: u64,
    inode: u64,
}

struct NamespaceSetV1 {
    identities: [NamespaceIdentityV1; NAMESPACE_NAMES_V1.len()],
}

impl NamespaceSetV1 {
    fn capture_self() -> Result<Self, ProtectedIssuerLaunchErrorV1> {
        let identities = NAMESPACE_NAMES_V1
            .map(|name| namespace_identity(&format!("/proc/self/ns/{name}")))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| {
                ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "namespace identity cardinality changed",
                )
            })?;
        let set = Self { identities };
        set.require_children_unchanged()?;
        Ok(set)
    }

    fn revalidate_self(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        for (index, name) in NAMESPACE_NAMES_V1.iter().enumerate() {
            let observed = namespace_identity(&format!("/proc/self/ns/{name}"))?;
            if observed != self.identities[index] {
                return Err(ProtectedIssuerLaunchErrorV1::Namespace(name));
            }
        }
        self.require_children_unchanged()
    }

    fn revalidate_child(
        &self,
        pid: rustix::process::Pid,
    ) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        for (index, name) in NAMESPACE_NAMES_V1.iter().enumerate() {
            let observed = namespace_identity(&format!("/proc/{}/ns/{name}", pid.as_raw_pid()))?;
            if observed != self.identities[index] {
                return Err(ProtectedIssuerLaunchErrorV1::Namespace(name));
            }
        }
        Ok(())
    }

    fn require_children_unchanged(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        if self.identities[2] != self.identities[3] {
            return Err(ProtectedIssuerLaunchErrorV1::Namespace("pid-for-children"));
        }
        if self.identities[8] != self.identities[9] {
            return Err(ProtectedIssuerLaunchErrorV1::Namespace("time-for-children"));
        }
        Ok(())
    }
}

fn namespace_identity(path: &str) -> Result<NamespaceIdentityV1, ProtectedIssuerLaunchErrorV1> {
    let namespace = File::open(path).map_err(|source| io_error("open proc namespace", source))?;
    let stat = rustix::fs::fstat(&namespace)
        .map_err(|source| io_error("inspect proc namespace", source.into()))?;
    Ok(NamespaceIdentityV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn require_owned_sigchld() -> Result<(), ProtectedIssuerLaunchErrorV1> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::uninit();
    // SAFETY: sigaction with a null new action initializes exactly one old-action record.
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(io_error(
            "inspect supervisor SIGCHLD ownership",
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: successful sigaction initialized the record.
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL
        || action.sa_flags & (libc::SA_NOCLDWAIT | libc::SA_NOCLDSTOP) != 0
    {
        return Err(ProtectedIssuerLaunchErrorV1::ProcessProfile(
            "SIGCHLD disposition does not permit exclusive pidfd reaping",
        ));
    }
    Ok(())
}

struct ProtectedIssuerChildV1 {
    pidfd: Option<OwnedFd>,
    pid: rustix::process::Pid,
    reap_slot: Option<ReapSlotV1>,
}

impl ProtectedIssuerChildV1 {
    fn new(pidfd: OwnedFd, pid: rustix::process::Pid, reap_slot: ReapSlotV1) -> Self {
        Self {
            pidfd: Some(pidfd),
            pid,
            reap_slot: Some(reap_slot),
        }
    }

    fn validate_pidfd(&self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        let pidfd =
            self.pidfd
                .as_ref()
                .ok_or(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "clone3 pidfd was already transferred",
                ))?;
        let flags = rustix::io::fcntl_getfd(pidfd)
            .map_err(|source| io_error("inspect clone3 pidfd", source.into()))?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                "clone3 pidfd is inheritable",
            ));
        }
        Ok(())
    }

    fn pid_u32(&self) -> u32 {
        u32::try_from(self.pid.as_raw_pid()).expect("clone3 child PID is positive")
    }

    fn is_live(&self) -> Result<bool, ProtectedIssuerLaunchErrorV1> {
        let pidfd =
            self.pidfd
                .as_ref()
                .ok_or(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                    "pidfd was already transferred for reaping",
                ))?;
        match rustix::process::waitid(
            rustix::process::WaitId::PidFd(pidfd.as_fd()),
            rustix::process::WaitIdOptions::EXITED
                | rustix::process::WaitIdOptions::NOHANG
                | rustix::process::WaitIdOptions::NOWAIT,
        ) {
            Ok(None) | Err(rustix::io::Errno::INTR) => Ok(true),
            Ok(Some(_)) => Ok(false),
            Err(source) => Err(io_error("observe exact issuer pidfd", source.into())),
        }
    }

    fn exited_error(&self, context: &str) -> ProtectedIssuerLaunchErrorV1 {
        let detail = self
            .pidfd
            .as_ref()
            .and_then(|pidfd| {
                rustix::process::waitid(
                    rustix::process::WaitId::PidFd(pidfd.as_fd()),
                    rustix::process::WaitIdOptions::EXITED
                        | rustix::process::WaitIdOptions::NOHANG
                        | rustix::process::WaitIdOptions::NOWAIT,
                )
                .ok()
                .flatten()
            })
            .map(|status| describe_exit(&status))
            .unwrap_or_else(|| context.to_owned());
        ProtectedIssuerLaunchErrorV1::ChildExited(detail)
    }

    fn cancel_and_reap(&mut self) -> Result<(), ProtectedIssuerLaunchErrorV1> {
        let Some(pidfd) = self.pidfd.as_ref() else {
            return Ok(());
        };
        let signal_error =
            match rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => None,
                Err(source) => Some(io_error("pidfd-kill protected issuer", source.into())),
            };
        let wait_result = loop {
            match rustix::process::waitid(
                rustix::process::WaitId::PidFd(pidfd.as_fd()),
                rustix::process::WaitIdOptions::EXITED,
            ) {
                Ok(Some(_)) => break Ok(()),
                Ok(None) | Err(rustix::io::Errno::INTR) => {}
                Err(rustix::io::Errno::CHILD) => {
                    break Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "issuer child was reaped outside its pidfd owner",
                    ));
                }
                Err(source) => break Err(io_error("reap exact issuer pidfd", source.into())),
            }
        };
        drop(self.pidfd.take());
        self.reap_slot
            .take()
            .expect("live issuer child retains one reap slot")
            .complete();
        signal_error.map_or(wait_result, Err)
    }

    fn wait_and_reap(
        &mut self,
        deadline: Instant,
    ) -> Result<ProtectedIssuerTerminationV1, ProtectedIssuerLaunchErrorV1> {
        let status = loop {
            let pidfd =
                self.pidfd
                    .as_ref()
                    .ok_or(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "pidfd was already transferred for reaping",
                    ))?;
            match rustix::process::waitid(
                rustix::process::WaitId::PidFd(pidfd.as_fd()),
                rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOHANG,
            ) {
                Ok(Some(status)) => break status,
                Ok(None) | Err(rustix::io::Errno::INTR) => {
                    wait_for_pidfd_exit(pidfd, deadline)?;
                }
                Err(rustix::io::Errno::CHILD) => {
                    return Err(ProtectedIssuerLaunchErrorV1::InvalidProcessState(
                        "issuer child was reaped outside its pidfd owner",
                    ));
                }
                Err(source) => {
                    return Err(io_error(
                        "reap naturally exited issuer pidfd",
                        source.into(),
                    ));
                }
            }
        };
        drop(self.pidfd.take());
        self.reap_slot
            .take()
            .expect("live issuer child retains one reap slot")
            .complete();
        ProtectedIssuerTerminationV1::from_wait_status(&status)
    }

    fn finish_or_defer(&mut self) {
        let Some(pidfd) = self.pidfd.take() else {
            return;
        };
        let slot = self
            .reap_slot
            .take()
            .expect("live issuer child retains one reap slot");
        match try_reap_nonblocking(pidfd.as_fd(), self.pid) {
            ReapPollV1::Pending => slot.defer(pidfd, self.pid),
            ReapPollV1::Reaped | ReapPollV1::Lost => {
                drop(pidfd);
                slot.complete();
            }
        }
    }
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

impl Drop for ProtectedIssuerChildV1 {
    fn drop(&mut self) {
        if let Some(pidfd) = self.pidfd.as_ref() {
            let _ = rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL);
        }
        self.finish_or_defer();
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

fn kill_and_reap_pid_synchronously(pid: rustix::process::Pid) {
    let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    loop {
        match rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::empty()) {
            Ok(Some(_)) | Err(rustix::io::Errno::CHILD) => return,
            Ok(None) | Err(rustix::io::Errno::INTR) => {}
            Err(_) => return,
        }
    }
}

enum ReapPollV1 {
    Pending,
    Reaped,
    Lost,
}

fn try_reap_nonblocking(pidfd: BorrowedFd<'_>, pid: rustix::process::Pid) -> ReapPollV1 {
    match rustix::process::waitid(
        rustix::process::WaitId::PidFd(pidfd),
        rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOHANG,
    ) {
        Ok(Some(_)) => ReapPollV1::Reaped,
        Ok(None) | Err(rustix::io::Errno::INTR) => ReapPollV1::Pending,
        Err(rustix::io::Errno::CHILD) => ReapPollV1::Lost,
        Err(_) => match rustix::process::waitpid(Some(pid), rustix::process::WaitOptions::NOHANG) {
            Ok(Some(_)) => ReapPollV1::Reaped,
            Ok(None) | Err(rustix::io::Errno::INTR) => ReapPollV1::Pending,
            Err(rustix::io::Errno::CHILD) => ReapPollV1::Lost,
            Err(_) => ReapPollV1::Pending,
        },
    }
}

struct DeferredReapCellV1 {
    state: AtomicU8,
    pidfd: AtomicI32,
    pid: AtomicI32,
}

impl DeferredReapCellV1 {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(REAP_SLOT_EMPTY),
            pidfd: AtomicI32::new(-1),
            pid: AtomicI32::new(-1),
        }
    }
}

struct DeferredReaperV1 {
    cells: [DeferredReapCellV1; MAX_PROTECTED_ISSUER_PROCESSES_V1],
    thread_started: OnceLock<bool>,
}

impl DeferredReaperV1 {
    const fn new() -> Self {
        Self {
            cells: [const { DeferredReapCellV1::new() }; MAX_PROTECTED_ISSUER_PROCESSES_V1],
            thread_started: OnceLock::new(),
        }
    }

    fn reserve(&'static self) -> Result<ReapSlotV1, ProtectedIssuerLaunchErrorV1> {
        if !*self.thread_started.get_or_init(|| {
            std::thread::Builder::new()
                .name("fe2o3-issuer-reaper-v1".to_owned())
                .spawn(move || self.run())
                .is_ok()
        }) {
            return Err(ProtectedIssuerLaunchErrorV1::ProcessCapacity);
        }
        for cell in &self.cells {
            if cell
                .state
                .compare_exchange(
                    REAP_SLOT_EMPTY,
                    REAP_SLOT_RESERVED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                return Ok(ReapSlotV1 { cell, armed: true });
            }
        }
        Err(ProtectedIssuerLaunchErrorV1::ProcessCapacity)
    }

    fn run(&'static self) -> ! {
        loop {
            for cell in &self.cells {
                if cell.state.load(Ordering::Acquire) != REAP_SLOT_DEFERRED {
                    continue;
                }
                let raw_pidfd = cell.pidfd.load(Ordering::Acquire);
                let raw_pid = cell.pid.load(Ordering::Acquire);
                let Some(pid) = rustix::process::Pid::from_raw(raw_pid) else {
                    continue;
                };
                if raw_pidfd < 0 {
                    continue;
                }
                // SAFETY: the deferring owner transferred this live pidfd into the cell.
                let pidfd = unsafe { BorrowedFd::borrow_raw(raw_pidfd) };
                if !matches!(try_reap_nonblocking(pidfd, pid), ReapPollV1::Pending) {
                    // SAFETY: this cell exclusively owns raw_pidfd until this transition.
                    drop(unsafe { OwnedFd::from_raw_fd(raw_pidfd) });
                    cell.pidfd.store(-1, Ordering::Release);
                    cell.pid.store(-1, Ordering::Release);
                    cell.state.store(REAP_SLOT_EMPTY, Ordering::Release);
                }
            }
            std::thread::sleep(REAPER_POLL_INTERVAL_V1);
        }
    }
}

struct ReapSlotV1 {
    cell: &'static DeferredReapCellV1,
    armed: bool,
}

impl ReapSlotV1 {
    fn complete(mut self) {
        self.cell.pidfd.store(-1, Ordering::Release);
        self.cell.pid.store(-1, Ordering::Release);
        self.cell.state.store(REAP_SLOT_EMPTY, Ordering::Release);
        self.armed = false;
    }

    fn defer(mut self, pidfd: OwnedFd, pid: rustix::process::Pid) {
        self.cell
            .pidfd
            .store(pidfd.into_raw_fd(), Ordering::Release);
        self.cell.pid.store(pid.as_raw_pid(), Ordering::Release);
        self.cell.state.store(REAP_SLOT_DEFERRED, Ordering::Release);
        self.armed = false;
    }
}

impl Drop for ReapSlotV1 {
    fn drop(&mut self) {
        if self.armed {
            self.cell.state.store(REAP_SLOT_EMPTY, Ordering::Release);
        }
    }
}

fn deferred_reaper() -> &'static DeferredReaperV1 {
    static REAPER: DeferredReaperV1 = DeferredReaperV1::new();
    &REAPER
}

fn io_error(operation: &'static str, source: io::Error) -> ProtectedIssuerLaunchErrorV1 {
    ProtectedIssuerLaunchErrorV1::Io { operation, source }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_proc_profile_parser_accepts_only_the_exact_shape() {
        let exact = b"Name:\ttest\nUmask:\t0077\nTracerPid:\t0\nUid:\t1000\t1000\t1000\t1000\nGid:\t1001\t1001\t1001\t1001\nGroups:\t\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapBnd:\t0000000000000000\nCapAmb:\t0000000000000000\nNoNewPrivs:\t1\n";
        let credentials = IssuerServiceCredentialProfileV1::new(1000, 1001).unwrap();
        ProcStatusProfileV1::parse(exact)
            .unwrap()
            .require(credentials)
            .unwrap();

        for hostile in [
            exact.replace_bytes(b"Umask:\t0077", b"Umask:\t0022"),
            exact.replace_bytes(b"TracerPid:\t0", b"TracerPid:\t9"),
            exact.replace_bytes(
                b"Uid:\t1000\t1000\t1000\t1000",
                b"Uid:\t1000\t1000\t1000\t1002",
            ),
            exact.replace_bytes(
                b"Gid:\t1001\t1001\t1001\t1001",
                b"Gid:\t1001\t1001\t1001\t1002",
            ),
            exact.replace_bytes(b"Groups:\t\n", b"Groups:\t1001\n"),
            exact.replace_bytes(b"CapEff:\t0000000000000000", b"CapEff:\t0000000000000001"),
            exact.replace_bytes(b"NoNewPrivs:\t1", b"NoNewPrivs:\t0"),
        ] {
            assert!(
                ProcStatusProfileV1::parse(&hostile)
                    .and_then(|value| value.require(credentials))
                    .is_err()
            );
        }
    }

    trait ReplaceBytesV1 {
        fn replace_bytes(&self, from: &[u8], to: &[u8]) -> Vec<u8>;
    }

    impl ReplaceBytesV1 for [u8] {
        fn replace_bytes(&self, from: &[u8], to: &[u8]) -> Vec<u8> {
            let offset = self
                .windows(from.len())
                .position(|part| part == from)
                .expect("test field exists");
            let mut bytes = Vec::with_capacity(self.len() - from.len() + to.len());
            bytes.extend_from_slice(&self[..offset]);
            bytes.extend_from_slice(to);
            bytes.extend_from_slice(&self[offset + from.len()..]);
            bytes
        }
    }

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

    #[test]
    fn non_pidfd_wait_failure_falls_back_to_exact_child_pid() {
        let (not_pidfd, _writer) =
            protected_pipe(PipeFlags::empty(), "create fallback fixture").unwrap();
        let mut child = fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| {
            std::process::Command::new("/bin/true").spawn()
        })
        .unwrap();
        let pid = rustix::process::Pid::from_raw(i32::try_from(child.id()).unwrap()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match try_reap_nonblocking(not_pidfd.as_fd(), pid) {
                ReapPollV1::Pending if Instant::now() < deadline => {
                    std::thread::sleep(POLL_INTERVAL_V1);
                }
                ReapPollV1::Reaped => break,
                ReapPollV1::Pending => panic!("fallback waitpid did not reap the child in time"),
                ReapPollV1::Lost => panic!("fallback child was reaped outside the test"),
            }
        }
        assert!(child.wait().is_err(), "fallback waitpid must consume exit");
    }
}
