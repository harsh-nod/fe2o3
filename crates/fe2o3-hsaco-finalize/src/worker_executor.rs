//! Bounded execution of a measured direct LLVM worker.
//!
//! This module is intentionally Linux-only. It captures a regular native ELF through an
//! `O_NOFOLLOW` descriptor, checks its declared content identity, copies it into an anonymous
//! `memfd`, seals that image against mutation, and executes only the retained `/proc/self/fd`
//! reference. Unsupported platforms fail closed.
//!
//! Sealing removes pathname-replacement and source-inode mutation races after capture. It does
//! not pin the ELF interpreter, shared libraries, kernel, procfs mount, or other files opened by
//! the worker. The expected build identities are authenticated inputs to this API, but the
//! worker's report is still not remote attestation. Process-group termination and bounded Linux
//! `/proc` descendant snapshots cover ordinary descendants; a hostile process that escapes both
//! before observation requires a stronger external containment boundary such as a delegated
//! cgroup. Returned values are inert evidence and grant no publication, loading, or launch
//! authority.

use std::{error::Error, fmt, path::Path, time::Duration};

#[cfg(target_os = "linux")]
use std::{io, path::PathBuf, process::ExitStatus};

use crate::{
    CompilerHandoffWorkerRequestV2, ContentIdentityV1, MAX_WORKER_RESPONSE_BYTES,
    MAX_WORKER_TOOLCHAIN_ID_BYTES, WorkerEvidenceClassV1, WorkerEvidenceClassV2,
    WorkerProtocolError, WorkerRequestV1, WorkerRequestV2, WorkerResponseV1, WorkerResponseV2,
};
use fe2o3_artifact_transaction::{BuildAttempt, CompilerModuleHandoffIdentityV1};

/// Maximum bytes accepted for a selected native worker executable.
pub const MAX_WORKER_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
/// Default wall-clock execution limit.
pub const DEFAULT_WORKER_TIMEOUT: Duration = Duration::from_secs(120);
/// Maximum configurable wall-clock execution limit.
pub const MAX_WORKER_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// Default bound for unauthenticated worker stderr.
pub const DEFAULT_WORKER_STDERR_BYTES: usize = 16 * 1024;
/// Maximum configurable stderr capture bound.
pub const MAX_WORKER_STDERR_BYTES: usize = 64 * 1024;
/// The complete deterministic child environment. No parent variables are inherited.
pub const WORKER_ENVIRONMENT_ALLOWLIST_V1: &[(&str, &str)] =
    &[("LANG", "C"), ("LC_ALL", "C"), ("TZ", "UTC")];

/// Authenticated identities expected for one worker and its LLVM build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerMeasurementV1 {
    executable: ContentIdentityV1,
    worker_build_identity: String,
    llvm_build_identity: String,
}

impl WorkerMeasurementV1 {
    pub fn new(
        executable: ContentIdentityV1,
        worker_build_identity: impl Into<String>,
        llvm_build_identity: impl Into<String>,
    ) -> Result<Self, WorkerExecutionError> {
        let worker_build_identity = worker_build_identity.into();
        let llvm_build_identity = llvm_build_identity.into();
        if executable.byte_len() == 0
            || executable.byte_len() > MAX_WORKER_EXECUTABLE_BYTES
            || !valid_build_identity(&worker_build_identity)
            || !valid_build_identity(&llvm_build_identity)
        {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::InvalidMeasurement,
            ));
        }
        Ok(Self {
            executable,
            worker_build_identity,
            llvm_build_identity,
        })
    }

    pub const fn executable(&self) -> ContentIdentityV1 {
        self.executable
    }

    pub fn worker_build_identity(&self) -> &str {
        &self.worker_build_identity
    }

    pub fn llvm_build_identity(&self) -> &str {
        &self.llvm_build_identity
    }
}

/// Resource limits for one worker execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerExecutionLimitsV1 {
    timeout: Duration,
    stdout_bytes: usize,
    stderr_bytes: usize,
}

impl WorkerExecutionLimitsV1 {
    pub fn new(
        timeout: Duration,
        stdout_bytes: usize,
        stderr_bytes: usize,
    ) -> Result<Self, WorkerExecutionError> {
        if timeout.is_zero()
            || timeout > MAX_WORKER_TIMEOUT
            || stdout_bytes == 0
            || stdout_bytes > MAX_WORKER_RESPONSE_BYTES
            || stderr_bytes > MAX_WORKER_STDERR_BYTES
        {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::InvalidLimits,
            ));
        }
        Ok(Self {
            timeout,
            stdout_bytes,
            stderr_bytes,
        })
    }

    pub const fn timeout(self) -> Duration {
        self.timeout
    }

    pub const fn stdout_bytes(self) -> usize {
        self.stdout_bytes
    }

    pub const fn stderr_bytes(self) -> usize {
        self.stderr_bytes
    }
}

impl Default for WorkerExecutionLimitsV1 {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_WORKER_TIMEOUT,
            stdout_bytes: MAX_WORKER_RESPONSE_BYTES,
            stderr_bytes: DEFAULT_WORKER_STDERR_BYTES,
        }
    }
}

/// Portable description of an unsuccessful child termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerTerminationV1 {
    Exit(i32),
    Signal(i32),
    Unknown,
}

/// Stable category for a fail-closed executor error.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerExecutionErrorKind {
    UnsupportedPlatform,
    InvalidMeasurement,
    InvalidLimits,
    OpenWorker,
    WorkerNotRegular,
    WorkerNotExecutable,
    WorkerNotNativeElf,
    WorkerChangedDuringCapture,
    WorkerIdentityMismatch {
        expected: ContentIdentityV1,
        actual: ContentIdentityV1,
    },
    PreparePinnedImage,
    Spawn,
    ConfigurePipe,
    WriteRequest,
    RequestWriteIncomplete,
    ReadStdout,
    ReadStderr,
    Timeout,
    StdoutLimitExceeded,
    StderrLimitExceeded,
    Wait,
    ProcessTreeNotQuiescent,
    ExitFailure(WorkerTerminationV1),
    UnexpectedStderr,
    DecodeResponse(WorkerProtocolError),
    RequestIdentityMismatch,
    WorkerBuildIdentityMismatch,
    LlvmBuildIdentityMismatch,
    OutputLimitExceeded,
}

/// Executor failure plus bounded process output useful for diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerExecutionError {
    kind: Box<WorkerExecutionErrorKind>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    detail: Option<String>,
}

impl WorkerExecutionError {
    fn plain(kind: WorkerExecutionErrorKind) -> Self {
        Self {
            kind: Box::new(kind),
            stdout: Vec::new(),
            stderr: Vec::new(),
            detail: None,
        }
    }

    #[cfg(target_os = "linux")]
    fn io(kind: WorkerExecutionErrorKind, error: io::Error) -> Self {
        Self {
            kind: Box::new(kind),
            stdout: Vec::new(),
            stderr: Vec::new(),
            detail: Some(error.to_string()),
        }
    }

    #[cfg(target_os = "linux")]
    fn process(kind: WorkerExecutionErrorKind, capture: &ProcessCapture) -> Self {
        Self {
            kind: Box::new(kind),
            stdout: capture.stdout.bytes.clone(),
            stderr: capture.stderr.bytes.clone(),
            detail: None,
        }
    }

    #[cfg(target_os = "linux")]
    fn timeout(capture: &ProcessCapture, request_len: usize) -> Self {
        Self {
            kind: Box::new(WorkerExecutionErrorKind::Timeout),
            stdout: capture.stdout.bytes.clone(),
            stderr: capture.stderr.bytes.clone(),
            detail: Some(format!(
                "request_written={}/{} stdout_bytes={} stdout_eof={} stderr_bytes={} stderr_eof={}",
                capture.request_written,
                request_len,
                capture.stdout.bytes.len(),
                capture.stdout.eof,
                capture.stderr.bytes.len(),
                capture.stderr.eof,
            )),
        }
    }

    pub const fn kind(&self) -> &WorkerExecutionErrorKind {
        &self.kind
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

impl fmt::Display for WorkerExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "direct LLVM worker execution failed: {:?}",
            self.kind
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}

impl Error for WorkerExecutionError {}

#[cfg(target_os = "linux")]
struct ProcessCapture {
    status: ExitStatus,
    request_written: usize,
    stdout: Capture,
    stderr: Capture,
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    eof: bool,
    overflow: bool,
}

/// A verified response and worker measurement with no artifact authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertWorkerExecutionV1 {
    worker_executable: ContentIdentityV1,
    response: WorkerResponseV1,
}

impl InertWorkerExecutionV1 {
    pub const fn worker_executable(&self) -> ContentIdentityV1 {
        self.worker_executable
    }

    pub const fn response(&self) -> &WorkerResponseV1 {
        &self.response
    }

    /// Classifies the retained response as generic link evidence.
    pub const fn evidence_class(&self) -> WorkerEvidenceClassV1 {
        WorkerEvidenceClassV1::GenericLink
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// A sealed V2 response bound to the measured worker with no artifact authority.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) struct InertWorkerExecutionV2 {
    worker_executable: ContentIdentityV1,
    response: WorkerResponseV2,
}

/// Verified Worker V2 execution retained with its exact build attempt and one-shot handoff.
///
/// The worker response is measured and request-bound, but this remains inert evidence. In
/// particular, it cannot publish, load, or launch the returned bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InertCompilerHandoffExecutionV2 {
    attempt: BuildAttempt,
    handoff_identity: CompilerModuleHandoffIdentityV1,
    execution: InertWorkerExecutionV2,
}

impl InertCompilerHandoffExecutionV2 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.handoff_identity
    }

    pub const fn worker_executable(&self) -> ContentIdentityV1 {
        self.execution.worker_executable()
    }

    pub const fn response(&self) -> &WorkerResponseV2 {
        self.execution.response()
    }

    pub const fn evidence_class(&self) -> WorkerEvidenceClassV2 {
        WorkerEvidenceClassV2::CompilerFfiLink
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[allow(dead_code)]
impl InertWorkerExecutionV2 {
    pub const fn worker_executable(&self) -> ContentIdentityV1 {
        self.worker_executable
    }

    pub const fn response(&self) -> &WorkerResponseV2 {
        &self.response
    }

    pub const fn evidence_class(&self) -> WorkerEvidenceClassV2 {
        WorkerEvidenceClassV2::CompilerFfiLink
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn valid_build_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_WORKER_TOOLCHAIN_ID_BYTES
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
}

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeSet,
        fs::{File, Metadata},
        io::{Read, Seek, SeekFrom, Write},
        os::{
            fd::AsRawFd,
            unix::{fs::MetadataExt, process::CommandExt},
        },
        process::{Child, Command, Stdio},
        thread,
        time::Instant,
    };

    use rustix::{
        fs::{MemfdFlags, Mode, OFlags, SealFlags},
        process::{Pid, Signal, kill_process, kill_process_group},
    };

    const IO_CHUNK_BYTES: usize = 64 * 1024;
    const POLL_INTERVAL: Duration = Duration::from_millis(2);
    const DESCENDANT_SCAN_INTERVAL: Duration = Duration::from_millis(25);
    const DRAIN_GRACE: Duration = Duration::from_millis(200);
    const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
        .union(SealFlags::GROW)
        .union(SealFlags::SHRINK)
        .union(SealFlags::SEAL);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Snapshot {
        device: u64,
        inode: u64,
        mode: u32,
        size: u64,
        modified_seconds: i64,
        modified_nanoseconds: i64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
    }

    impl Snapshot {
        fn from_metadata(metadata: &Metadata) -> Self {
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                size: metadata.len(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
            }
        }
    }

    /// One immutable Linux `memfd` executable image retained through all executions.
    ///
    /// The source pathname is never reopened after capture. The image is content-checked and
    /// sealed before construction succeeds. This does not pin the ELF interpreter, shared
    /// libraries, procfs implementation, or files the worker later opens; callers that require a
    /// closed native trust chain must provide an external mount/cgroup sandbox and independently
    /// measure those dependencies.
    pub struct PinnedWorkerV1 {
        image: File,
        descriptor_path: PathBuf,
        measurement: WorkerMeasurementV1,
        snapshot: Snapshot,
    }

    impl fmt::Debug for PinnedWorkerV1 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("PinnedWorkerV1")
                .field("measurement", &self.measurement)
                .finish_non_exhaustive()
        }
    }

    impl PinnedWorkerV1 {
        /// Captures `path` once and requires its exact declared content identity.
        ///
        /// The final path component is opened with `O_NOFOLLOW`. The sole exception is an exact
        /// `/proc/self/fd/N` spelling whose referred memfd already has all required seals. That
        /// path supports an externally retained executable without reopening its mutable source.
        /// Parent-directory resolution of ordinary paths may race, but substituting a different
        /// object cannot pass the authenticated content identity unless it has the same bytes.
        /// The immutable captured image, rather than the pathname, is used by [`Self::execute`].
        pub fn open(
            path: impl AsRef<Path>,
            measurement: WorkerMeasurementV1,
        ) -> Result<Self, WorkerExecutionError> {
            let path = path.as_ref();
            let retained_descriptor = retained_descriptor_number(path);
            let mut open_flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
            if retained_descriptor.is_none() {
                open_flags |= OFlags::NOFOLLOW;
            }
            let fd = rustix::fs::open(path, open_flags, Mode::empty()).map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error.into())
            })?;
            let mut source = File::from(fd);
            if retained_descriptor.is_some()
                && rustix::fs::fcntl_get_seals(&source).map_err(|error| {
                    WorkerExecutionError::io(
                        WorkerExecutionErrorKind::PreparePinnedImage,
                        error.into(),
                    )
                })? != REQUIRED_SEALS
            {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::PreparePinnedImage,
                ));
            }
            let initial_metadata = source.metadata().map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error)
            })?;
            if !initial_metadata.is_file() {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerNotRegular,
                ));
            }
            let initial = Snapshot::from_metadata(&initial_metadata);
            if initial.mode & 0o111 == 0 {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerNotExecutable,
                ));
            }
            if initial.size != measurement.executable.byte_len() {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerIdentityMismatch {
                        expected: measurement.executable,
                        actual: ContentIdentityV1::from_parts([0; 32], initial.size),
                    },
                ));
            }

            let (image, snapshot, digest) = capture_and_seal(&mut source)?;
            let actual = ContentIdentityV1::from_parts(digest, snapshot.size);
            if actual != measurement.executable {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerIdentityMismatch {
                        expected: measurement.executable,
                        actual,
                    },
                ));
            }
            if Snapshot::from_metadata(&source.metadata().map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error)
            })?) != initial
            {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerChangedDuringCapture,
                ));
            }

            let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", image.as_raw_fd()));
            validate_image(&image, &descriptor_path, snapshot)?;
            Ok(Self {
                image,
                descriptor_path,
                measurement,
                snapshot,
            })
        }

        pub const fn measurement(&self) -> &WorkerMeasurementV1 {
            &self.measurement
        }

        /// Runs one canonical request under fixed environment and process resource bounds.
        ///
        /// A successful return verifies the exact request identity, declared LLVM build,
        /// executable measurement, worker response identity, output digest and output bound. It
        /// remains inert and cannot publish or load the output. Ordinary descendants are killed
        /// through an isolated process group plus bounded `/proc` discovery; hostile descendants
        /// that escape both before observation are outside this API's containment claim.
        pub fn execute(
            &self,
            request: &WorkerRequestV1,
            limits: WorkerExecutionLimitsV1,
        ) -> Result<InertWorkerExecutionV1, WorkerExecutionError> {
            if request.llvm_build_identity() != self.measurement.llvm_build_identity {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::LlvmBuildIdentityMismatch,
                ));
            }
            validate_image(&self.image, &self.descriptor_path, self.snapshot)?;

            let mut command = Command::new(&self.descriptor_path);
            command
                .arg0("fe2o3-llvm-link-worker")
                .env_clear()
                .envs(WORKER_ENVIRONMENT_ALLOWLIST_V1.iter().copied())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0);
            let mut child = command.spawn().map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::Spawn, error)
            })?;
            let capture = supervise(&mut child, request.canonical_bytes(), limits)?;

            if !capture.status.success() {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::ExitFailure(termination(capture.status)),
                    &capture,
                ));
            }
            if capture.request_written != request.canonical_bytes().len() {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::RequestWriteIncomplete,
                    &capture,
                ));
            }
            if !capture.stderr.bytes.is_empty() {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::UnexpectedStderr,
                    &capture,
                ));
            }
            let response = WorkerResponseV1::decode(&capture.stdout.bytes).map_err(|error| {
                WorkerExecutionError::process(
                    WorkerExecutionErrorKind::DecodeResponse(error),
                    &capture,
                )
            })?;
            if !response.binds_request(request) {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::RequestIdentityMismatch,
                    &capture,
                ));
            }
            if response.worker_build_identity() != self.measurement.worker_build_identity {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::WorkerBuildIdentityMismatch,
                    &capture,
                ));
            }
            if response.output().is_some_and(|output| {
                output.bytes().len() as u64 > request.output_constraints().max_bytes()
                    || !output.identity().matches(output.bytes())
            }) {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::OutputLimitExceeded,
                    &capture,
                ));
            }
            Ok(InertWorkerExecutionV1 {
                worker_executable: self.measurement.executable,
                response,
            })
        }

        /// Runs one sealed compiler-FFI V2 request under the V1 supervisor limits.
        ///
        /// The request must bind this exact executable, worker build, and LLVM
        /// build. The response must use the V2 domain and echo the exact request
        /// and compiler-envelope identities. The result remains inert.
        #[allow(dead_code)]
        pub(crate) fn execute_v2(
            &self,
            request: &WorkerRequestV2,
            limits: WorkerExecutionLimitsV1,
        ) -> Result<InertWorkerExecutionV2, WorkerExecutionError> {
            if request.llvm_build_identity() != self.measurement.llvm_build_identity {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::LlvmBuildIdentityMismatch,
                ));
            }
            if request.worker_build_identity() != self.measurement.worker_build_identity {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerBuildIdentityMismatch,
                ));
            }
            if request.worker_executable() != self.measurement.executable {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerIdentityMismatch {
                        expected: request.worker_executable(),
                        actual: self.measurement.executable,
                    },
                ));
            }
            validate_image(&self.image, &self.descriptor_path, self.snapshot)?;

            let mut command = Command::new(&self.descriptor_path);
            command
                .arg0("fe2o3-llvm-link-worker")
                .env_clear()
                .envs(WORKER_ENVIRONMENT_ALLOWLIST_V1.iter().copied())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0);
            let mut child = command.spawn().map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::Spawn, error)
            })?;
            let capture = supervise(&mut child, request.canonical_bytes(), limits)?;

            if !capture.status.success() {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::ExitFailure(termination(capture.status)),
                    &capture,
                ));
            }
            if capture.request_written != request.canonical_bytes().len() {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::RequestWriteIncomplete,
                    &capture,
                ));
            }
            if !capture.stderr.bytes.is_empty() {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::UnexpectedStderr,
                    &capture,
                ));
            }
            let response = WorkerResponseV2::decode_for_request(&capture.stdout.bytes, request)
                .map_err(|error| {
                    WorkerExecutionError::process(
                        WorkerExecutionErrorKind::DecodeResponse(error),
                        &capture,
                    )
                })?;
            if response.worker_build_identity() != self.measurement.worker_build_identity {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::WorkerBuildIdentityMismatch,
                    &capture,
                ));
            }
            if response.output().is_some_and(|output| {
                output.bytes().len() as u64 > request.output_constraints().max_bytes()
                    || !output.identity().matches(output.bytes())
                    || output.request_identity() != request.identity()
                    || output.compiler_envelope_identity() != request.compiler_envelope_identity()
            }) {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::OutputLimitExceeded,
                    &capture,
                ));
            }
            Ok(InertWorkerExecutionV2 {
                worker_executable: self.measurement.executable,
                response,
            })
        }

        /// Executes a Worker V2 request that consumed one exact build-attempt handoff.
        pub fn execute_compiler_handoff_v2(
            &self,
            request: &CompilerHandoffWorkerRequestV2,
            limits: WorkerExecutionLimitsV1,
        ) -> Result<InertCompilerHandoffExecutionV2, WorkerExecutionError> {
            let execution = self.execute_v2(request.sealed_request(), limits)?;
            Ok(InertCompilerHandoffExecutionV2 {
                attempt: request.attempt(),
                handoff_identity: request.handoff_identity(),
                execution,
            })
        }
    }

    fn retained_descriptor_number(path: &Path) -> Option<i32> {
        let text = path.to_str()?;
        let suffix = text.strip_prefix("/proc/self/fd/")?;
        if suffix.is_empty()
            || !suffix.bytes().all(|byte| byte.is_ascii_digit())
            || (suffix.len() > 1 && suffix.starts_with('0'))
        {
            return None;
        }
        let descriptor = suffix.parse::<i32>().ok()?;
        (descriptor >= 3 && text == format!("/proc/self/fd/{descriptor}")).then_some(descriptor)
    }

    fn capture_and_seal(
        source: &mut File,
    ) -> Result<(File, Snapshot, [u8; 32]), WorkerExecutionError> {
        let initial = Snapshot::from_metadata(&source.metadata().map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error)
        })?);
        if initial.size == 0 || initial.size > MAX_WORKER_EXECUTABLE_BYTES {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::InvalidMeasurement,
            ));
        }
        let fd = rustix::fs::memfd_create(
            "fe2o3-llvm-link-worker",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error.into())
        })?;
        let mut image = File::from(fd);
        let mut hasher = Sha256::new();
        let mut magic = Vec::with_capacity(4);
        let mut copied = 0_u64;
        let mut buffer = [0_u8; IO_CHUNK_BYTES];
        while copied < initial.size {
            let needed = usize::try_from((initial.size - copied).min(IO_CHUNK_BYTES as u64))
                .expect("bounded chunk fits usize");
            let read = read_retry(source, &mut buffer[..needed]).map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error)
            })?;
            if read == 0 {
                return Err(WorkerExecutionError::plain(
                    WorkerExecutionErrorKind::WorkerChangedDuringCapture,
                ));
            }
            if magic.len() < 4 {
                let take = (4 - magic.len()).min(read);
                magic.extend_from_slice(&buffer[..take]);
            }
            image.write_all(&buffer[..read]).map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error)
            })?;
            hasher.update(&buffer[..read]);
            copied += read as u64;
        }
        if read_retry(source, &mut buffer[..1]).map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error)
        })? != 0
        {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::WorkerChangedDuringCapture,
            ));
        }
        if magic != b"\x7fELF" {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::WorkerNotNativeElf,
            ));
        }
        if Snapshot::from_metadata(&source.metadata().map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::OpenWorker, error)
        })?) != initial
        {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::WorkerChangedDuringCapture,
            ));
        }
        rustix::fs::fcntl_add_seals(
            &image,
            SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
        )
        .and_then(|()| rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL))
        .map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error.into())
        })?;
        image.seek(SeekFrom::Start(0)).map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error)
        })?;
        let snapshot = Snapshot::from_metadata(&image.metadata().map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error)
        })?);
        Ok((image, snapshot, hasher.finalize().into()))
    }

    fn validate_image(
        image: &File,
        descriptor_path: &Path,
        expected: Snapshot,
    ) -> Result<(), WorkerExecutionError> {
        let seals = rustix::fs::fcntl_get_seals(image).map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error.into())
        })?;
        let descriptor = Snapshot::from_metadata(&image.metadata().map_err(|error| {
            WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error)
        })?);
        let procfs =
            Snapshot::from_metadata(&std::fs::metadata(descriptor_path).map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::PreparePinnedImage, error)
            })?);
        if seals != REQUIRED_SEALS || descriptor != expected || procfs != expected {
            return Err(WorkerExecutionError::plain(
                WorkerExecutionErrorKind::PreparePinnedImage,
            ));
        }
        Ok(())
    }

    fn supervise(
        child: &mut Child,
        request: &[u8],
        limits: WorkerExecutionLimitsV1,
    ) -> Result<ProcessCapture, WorkerExecutionError> {
        let stdin = child.stdin.take().expect("worker stdin is piped");
        let mut stdout_pipe = child.stdout.take().expect("worker stdout is piped");
        let mut stderr_pipe = child.stderr.take().expect("worker stderr is piped");
        make_nonblocking(&stdin)
            .and_then(|()| make_nonblocking(&stdout_pipe))
            .and_then(|()| make_nonblocking(&stderr_pipe))
            .map_err(|error| {
                terminate_process_tree(child, &BTreeSet::new());
                let _ = child.wait();
                WorkerExecutionError::io(WorkerExecutionErrorKind::ConfigurePipe, error)
            })?;
        let mut stdin = Some(stdin);

        let started = Instant::now();
        let mut request_written = 0;
        let mut stdout = Capture::default();
        let mut stderr = Capture::default();
        let mut descendants_seen = BTreeSet::new();
        let mut next_descendant_scan = Instant::now();
        let status = loop {
            if Instant::now() >= next_descendant_scan {
                descendants_seen.extend(descendants(child.id()));
                next_descendant_scan = Instant::now() + DESCENDANT_SCAN_INTERVAL;
            }
            if let Some(pipe) = stdin.as_mut() {
                match pipe.write(&request[request_written..]) {
                    Ok(0) => {
                        let capture = partial_capture(request_written, stdout, stderr);
                        terminate_process_tree(child, &descendants_seen);
                        let _ = child.wait();
                        return Err(WorkerExecutionError::process(
                            WorkerExecutionErrorKind::WriteRequest,
                            &capture,
                        ));
                    }
                    Ok(written) => request_written += written,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {}
                    Err(error) => {
                        let capture = partial_capture(request_written, stdout, stderr);
                        terminate_process_tree(child, &descendants_seen);
                        let _ = child.wait();
                        return Err(WorkerExecutionError {
                            kind: Box::new(WorkerExecutionErrorKind::WriteRequest),
                            stdout: capture.stdout.bytes,
                            stderr: capture.stderr.bytes,
                            detail: Some(error.to_string()),
                        });
                    }
                }
                if request_written == request.len() {
                    stdin = None;
                }
            }
            drain(&mut stdout_pipe, &mut stdout, limits.stdout_bytes).map_err(|error| {
                terminate_process_tree(child, &descendants_seen);
                let _ = child.wait();
                WorkerExecutionError::io(WorkerExecutionErrorKind::ReadStdout, error)
            })?;
            drain(&mut stderr_pipe, &mut stderr, limits.stderr_bytes).map_err(|error| {
                terminate_process_tree(child, &descendants_seen);
                let _ = child.wait();
                WorkerExecutionError::io(WorkerExecutionErrorKind::ReadStderr, error)
            })?;
            if stdout.overflow {
                let capture = partial_capture(request_written, stdout, stderr);
                terminate_process_tree(child, &descendants_seen);
                let _ = child.wait();
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::StdoutLimitExceeded,
                    &capture,
                ));
            }
            if stderr.overflow {
                let capture = partial_capture(request_written, stdout, stderr);
                terminate_process_tree(child, &descendants_seen);
                let _ = child.wait();
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::StderrLimitExceeded,
                    &capture,
                ));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= limits.timeout => {
                    let capture = partial_capture(request_written, stdout, stderr);
                    terminate_process_tree(child, &descendants_seen);
                    let _ = child.wait();
                    return Err(WorkerExecutionError::timeout(&capture, request.len()));
                }
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(error) => {
                    terminate_process_tree(child, &descendants_seen);
                    let _ = child.wait();
                    return Err(WorkerExecutionError::io(
                        WorkerExecutionErrorKind::Wait,
                        error,
                    ));
                }
            }
        };

        drop(stdin);
        descendants_seen.extend(descendants(child.id()));
        terminate_process_tree(child, &descendants_seen);
        let deadline = Instant::now() + DRAIN_GRACE;
        while (!stdout.eof || !stderr.eof) && Instant::now() < deadline {
            drain(&mut stdout_pipe, &mut stdout, limits.stdout_bytes).map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::ReadStdout, error)
            })?;
            drain(&mut stderr_pipe, &mut stderr, limits.stderr_bytes).map_err(|error| {
                WorkerExecutionError::io(WorkerExecutionErrorKind::ReadStderr, error)
            })?;
            if stdout.overflow {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::StdoutLimitExceeded,
                    &partial_capture(request_written, stdout, stderr),
                ));
            }
            if stderr.overflow {
                return Err(WorkerExecutionError::process(
                    WorkerExecutionErrorKind::StderrLimitExceeded,
                    &partial_capture(request_written, stdout, stderr),
                ));
            }
            if !stdout.eof || !stderr.eof {
                thread::sleep(POLL_INTERVAL);
            }
        }
        let capture = partial_capture(request_written, stdout, stderr);
        if !capture.stdout.eof || !capture.stderr.eof {
            return Err(WorkerExecutionError::process(
                WorkerExecutionErrorKind::ProcessTreeNotQuiescent,
                &capture,
            ));
        }
        Ok(ProcessCapture { status, ..capture })
    }

    fn partial_capture(request_written: usize, stdout: Capture, stderr: Capture) -> ProcessCapture {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        ProcessCapture {
            status: ExitStatus::from_raw(0),
            request_written,
            stdout,
            stderr,
        }
    }

    fn make_nonblocking(fd: &impl std::os::fd::AsFd) -> io::Result<()> {
        let flags = rustix::fs::fcntl_getfl(fd)?;
        rustix::fs::fcntl_setfl(fd, flags | OFlags::NONBLOCK)?;
        Ok(())
    }

    fn drain<R: Read>(reader: &mut R, capture: &mut Capture, limit: usize) -> io::Result<()> {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    capture.eof = true;
                    return Ok(());
                }
                Ok(read) => {
                    let remaining = limit.saturating_sub(capture.bytes.len());
                    capture
                        .bytes
                        .extend_from_slice(&buffer[..read.min(remaining)]);
                    if read > remaining {
                        capture.overflow = true;
                        return Ok(());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn terminate_process_tree(child: &mut Child, known_descendants: &BTreeSet<u32>) {
        let root = Pid::from_child(child);
        let _ = kill_process_group(root, Signal::KILL);
        for raw in known_descendants.iter().rev() {
            if let Ok(raw) = i32::try_from(*raw)
                && let Some(pid) = Pid::from_raw(raw)
            {
                let _ = kill_process(pid, Signal::KILL);
            }
        }
        let _ = child.kill();
    }

    fn descendants(root: u32) -> BTreeSet<u32> {
        const MAX_DESCENDANTS: usize = 4096;
        let mut found = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(parent) = pending.pop() {
            if found.len() >= MAX_DESCENDANTS {
                break;
            }
            let path = format!("/proc/{parent}/task/{parent}/children");
            let Ok(children) = std::fs::read_to_string(path) else {
                continue;
            };
            for child in children.split_ascii_whitespace() {
                if let Ok(child) = child.parse::<u32>()
                    && child != root
                    && found.insert(child)
                {
                    pending.push(child);
                }
            }
        }
        found
    }

    fn termination(status: ExitStatus) -> WorkerTerminationV1 {
        if let Some(code) = status.code() {
            return WorkerTerminationV1::Exit(code);
        }
        use std::os::unix::process::ExitStatusExt;
        status
            .signal()
            .map_or(WorkerTerminationV1::Unknown, WorkerTerminationV1::Signal)
    }

    fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match reader.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                result => return result,
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub use platform::PinnedWorkerV1;

#[cfg(not(target_os = "linux"))]
/// Fail-closed placeholder on platforms without the Linux sealed-descriptor strategy.
#[derive(Debug)]
pub struct PinnedWorkerV1;

#[cfg(not(target_os = "linux"))]
impl PinnedWorkerV1 {
    pub fn open(
        _path: impl AsRef<Path>,
        _measurement: WorkerMeasurementV1,
    ) -> Result<Self, WorkerExecutionError> {
        Err(WorkerExecutionError::plain(
            WorkerExecutionErrorKind::UnsupportedPlatform,
        ))
    }

    pub fn execute(
        &self,
        _request: &WorkerRequestV1,
        _limits: WorkerExecutionLimitsV1,
    ) -> Result<InertWorkerExecutionV1, WorkerExecutionError> {
        Err(WorkerExecutionError::plain(
            WorkerExecutionErrorKind::UnsupportedPlatform,
        ))
    }

    pub fn execute_compiler_handoff_v2(
        &self,
        _request: &CompilerHandoffWorkerRequestV2,
        _limits: WorkerExecutionLimitsV1,
    ) -> Result<InertCompilerHandoffExecutionV2, WorkerExecutionError> {
        Err(WorkerExecutionError::plain(
            WorkerExecutionErrorKind::UnsupportedPlatform,
        ))
    }

    #[allow(dead_code)]
    pub(crate) fn execute_v2(
        &self,
        _request: &WorkerRequestV2,
        _limits: WorkerExecutionLimitsV1,
    ) -> Result<InertWorkerExecutionV2, WorkerExecutionError> {
        Err(WorkerExecutionError::plain(
            WorkerExecutionErrorKind::UnsupportedPlatform,
        ))
    }
}

#[cfg(all(test, target_os = "linux"))]
mod configured_v2_tests {
    use super::*;
    use crate::{
        ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
        MultiInputLinkPlanV1, ProvenanceNodeV1, WorkerInputKindV1, WorkerInputV1,
        WorkerOutputConstraintsV1, WorkerStageV1,
        construct_worker_request_v2_from_consumed_handoff,
        execute_reproducible_first_build_worker_v2,
    };
    use fe2o3_artifact_transaction::{
        BuildInvocation, BuildSession, ProducerIdentity, begin_build_attempt,
        consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
    };
    use fe2o3_compiler_ffi::{
        CodeObjectVersion as CompilerCodeObjectVersion, CompilerFfiContractV1,
        CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
        CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
        CompilerModuleSymbolRoleV1, DeviceTargetV1 as CompilerDeviceTargetV1,
    };
    use fe2o3_kernel_descriptor::DeviceTargetV1;
    use reserved_fe2o3_symbols::{
        DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
        DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    const TARGET: &str = "gfx942";
    const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
    const OCML_ABI: &str = "C(f32[size=4,align=4])->f32[size=4,align=4]";
    const OCML_MODULE: &[u8] = br#"target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_sin_f32(float)

define float @ocml_entry(float %value) {
entry:
  %result = call float @__ocml_sin_f32(float %value)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
"#;
    const OCML_OMITTED_IMPORT_MODULE: &[u8] = br#"target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_sin_f32(float)
declare float @__ocml_sqrt_f32(float)

define float @ocml_entry(float %value) {
entry:
  %sin = call float @__ocml_sin_f32(float %value)
  %result = call float @__ocml_sqrt_f32(float %sin)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
"#;
    const OCML_MODULE_ASM_IMPORT_MODULE: &[u8] = br#"target triple = "amdgcn-amd-amdhsa"
module asm ".long hidden_runtime_import"

declare float @__ocml_sin_f32(float)

define float @ocml_entry(float %value) {
entry:
  %result = call float @__ocml_sin_f32(float %value)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
"#;
    const OCML_FUNCTION_ASM_IMPORT_MODULE: &[u8] = br#"target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_sin_f32(float)

define float @ocml_entry(float %value) {
entry:
  call void asm sideeffect ".long __ocml_sqrt_f32", ""()
  %result = call float @__ocml_sin_f32(float %value)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
"#;
    const OCML_LOCAL_MODULE_ASM_IMPORT_MODULE: &[u8] = br#"target triple = "amdgcn-amd-amdhsa"
module asm ".local local_hidden_import"
module asm ".long local_hidden_import"

declare float @__ocml_sin_f32(float)

define float @ocml_entry(float %value) {
entry:
  %result = call float @__ocml_sin_f32(float %value)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
"#;
    const OCML_LOCAL_FUNCTION_ASM_IMPORT_MODULE: &[u8] = br#"target triple = "amdgcn-amd-amdhsa"

declare float @__ocml_sin_f32(float)

define float @ocml_entry(float %value) {
entry:
  call void asm sideeffect ".local local_hidden_import\0A.long local_hidden_import", ""()
  %result = call float @__ocml_sin_f32(float %value)
  ret float %result
}

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"amdhsa_code_object_version", i32 500}
"#;

    fn required_env(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| panic!("missing configured test variable {name}"))
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-native-v2-handoff-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn ffi_contract(
        symbol: &str,
        direction: DeviceFfiDirectionV1,
        role: CompilerFfiLinkRoleV1,
        semantic_byte: u8,
        abi: &str,
    ) -> CompilerFfiContractV1 {
        let semantic_identity = [semantic_byte; 32];
        let semantic_text = semantic_identity
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let direction_tag = match direction {
            DeviceFfiDirectionV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
            DeviceFfiDirectionV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
        };
        let fields = DeviceFfiContractFieldsV1 {
            direction: direction_tag,
            symbol,
            calling_convention: "C",
            code_object_version: 5,
            target: TARGET,
            physical_abi: abi,
            effects: "none",
            semantic_identity: &semantic_text,
        };
        CompilerFfiContractV1::new(
            derive_device_ffi_contract_id_v1(fields),
            direction,
            role,
            CompilerDeviceTargetV1::parse(TARGET).unwrap(),
            CompilerCodeObjectVersion::V5,
            CompilerFfiSourceOwnerV1::new(
                "native_fixture",
                &format!("native_fixture::{symbol}"),
                [semantic_byte; 16],
                &format!("_RINvNtCs1234_14native_fixture{symbol}"),
            )
            .unwrap(),
            symbol,
            abi,
            "none",
            semantic_identity,
        )
        .unwrap()
    }

    fn make_ocml_handoff_request(
        module: &[u8],
        expected_output: &[u8],
        measurement: &WorkerMeasurementV1,
        invocation_byte: u8,
    ) -> (TestDirectory, CompilerHandoffWorkerRequestV2) {
        let mut envelope = CompilerFfiEnvelopeBuilderV1::new(
            CompilerDeviceTargetV1::parse(TARGET).unwrap(),
            CompilerCodeObjectVersion::V5,
            2,
        )
        .unwrap();
        envelope
            .push(ffi_contract(
                "__ocml_sin_f32",
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                0x51,
                OCML_ABI,
            ))
            .unwrap();
        envelope
            .push(ffi_contract(
                "ocml_entry",
                DeviceFfiDirectionV1::Export,
                CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
                0x52,
                OCML_ABI,
            ))
            .unwrap();
        let manifest = CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::DeviceFfiExport, "ocml_entry"),
            (
                CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
                "__ocml_sin_f32",
            ),
        ])
        .unwrap();
        let handoff = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            CompilerDeviceTargetV1::parse(TARGET).unwrap(),
            CompilerCodeObjectVersion::V5,
            envelope.finish().unwrap(),
            manifest,
            module,
        )
        .unwrap();

        let directory = TestDirectory::new();
        let producer = ProducerIdentity::from_codegen(
            "ocml_native_fixture",
            Some(std::path::Path::new("src/lib.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory.0,
            &producer,
            BuildInvocation::from_bytes([invocation_byte; 32]),
            BuildSession::from_bytes([invocation_byte.wrapping_add(1); 16]),
        )
        .unwrap();
        publish_compiler_module_handoff_v1(
            &directory.0,
            &producer,
            attempt,
            handoff.canonical_bytes(),
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();

        let target = DeviceTargetV1::parse(TARGET).unwrap();
        let compiler_module =
            WorkerInputV1::new(WorkerInputKindV1::LlvmTextIr, module.to_vec()).unwrap();
        let input = LinkInputV1::new(compiler_module.identity(), target);
        let output_identity = ContentIdentityV1::calculate(expected_output);
        let plan = MultiInputLinkPlanV1::canonicalized(
            target,
            vec![input],
            vec![
                LinkOptionV1::new("code-object-version", "5").unwrap(),
                LinkOptionV1::new("opt-level", "3").unwrap(),
                LinkOptionV1::new("strip-debug", "true").unwrap(),
                LinkOptionV1::new("verify-each", "true").unwrap(),
            ],
            LinkOutputV1::new(output_identity, target),
            vec![
                ProvenanceNodeV1::new(input.identity(), vec![]).unwrap(),
                ProvenanceNodeV1::new(output_identity, vec![input.identity()]).unwrap(),
            ],
        )
        .unwrap();
        let input_kinds = LinkInputKindClosureV1::new(&plan, vec![compiler_module.kind()]).unwrap();
        let request = construct_worker_request_v2_from_consumed_handoff(
            &plan,
            measurement,
            consumed,
            vec![],
            &input_kinds,
            WorkerOutputConstraintsV1::new(output_identity.byte_len()).unwrap(),
        )
        .unwrap();
        (directory, request)
    }

    #[test]
    #[ignore = "requires the explicitly configured native LLVM/LLD worker"]
    fn configured_cpp_worker_v2_round_trip() {
        let worker_path = required_env("FE2O3_LLVM_LINK_WORKER");
        let worker_bytes = fs::read(&worker_path).unwrap();
        let measurement = WorkerMeasurementV1::new(
            ContentIdentityV1::calculate(&worker_bytes),
            required_env("FE2O3_LLVM_LINK_WORKER_BUILD_ID"),
            required_env("FE2O3_LLVM_BUILD_ID"),
        )
        .unwrap();
        let pinned = PinnedWorkerV1::open(worker_path, measurement).unwrap();
        let module_bytes = fs::read(required_env("FE2O3_LLVM_V2_MODULE")).unwrap();
        let external_provider = WorkerInputV1::new(
            WorkerInputKindV1::AmdGpuRelocatable,
            fs::read(required_env("FE2O3_LLVM_V2_PROVIDER")).unwrap(),
        )
        .unwrap();
        let expected_output = fs::read(required_env("FE2O3_LLVM_V2_EXPECTED_OUTPUT")).unwrap();

        let mut envelope = CompilerFfiEnvelopeBuilderV1::new(
            CompilerDeviceTargetV1::parse(TARGET).unwrap(),
            CompilerCodeObjectVersion::V5,
            2,
        )
        .unwrap();
        envelope
            .push(ffi_contract(
                "object_helper",
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                0x41,
                ABI,
            ))
            .unwrap();
        envelope
            .push(ffi_contract(
                "mixed_entry",
                DeviceFfiDirectionV1::Export,
                CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
                0x42,
                ABI,
            ))
            .unwrap();
        let symbol_manifest = CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::DeviceFfiExport, "mixed_entry"),
            (
                CompilerModuleSymbolRoleV1::UnresolvedExternalImport,
                "object_helper",
            ),
        ])
        .unwrap();
        let handoff = CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmBitcode,
            CompilerDeviceTargetV1::parse(TARGET).unwrap(),
            CompilerCodeObjectVersion::V5,
            envelope.finish().unwrap(),
            symbol_manifest,
            &module_bytes,
        )
        .unwrap();
        let directory = TestDirectory::new();
        let producer = ProducerIdentity::from_codegen(
            "native_fixture",
            Some(std::path::Path::new("src/lib.rs")),
        )
        .unwrap();
        let attempt = begin_build_attempt(
            &directory.0,
            &producer,
            BuildInvocation::from_bytes([0x91; 32]),
            BuildSession::from_bytes([0x92; 16]),
        )
        .unwrap();
        let receipt = publish_compiler_module_handoff_v1(
            &directory.0,
            &producer,
            attempt,
            handoff.canonical_bytes(),
        )
        .unwrap();
        let consumed =
            consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
        let evidence = execute_reproducible_first_build_worker_v2(
            consumed,
            &pinned,
            vec![external_provider],
            vec![
                LinkOptionV1::new("code-object-version", "5").unwrap(),
                LinkOptionV1::new("opt-level", "3").unwrap(),
                LinkOptionV1::new("strip-debug", "true").unwrap(),
                LinkOptionV1::new("verify-each", "true").unwrap(),
            ],
            WorkerOutputConstraintsV1::new(4 * 1024 * 1024).unwrap(),
            WorkerExecutionLimitsV1::new(
                Duration::from_secs(120),
                MAX_WORKER_RESPONSE_BYTES,
                DEFAULT_WORKER_STDERR_BYTES,
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(evidence.output_bytes(), expected_output);
        assert_eq!(evidence.attempt(), attempt);
        assert_eq!(evidence.handoff_identity(), receipt.identity());
        assert_eq!(
            evidence.candidate().evidence_class(),
            WorkerEvidenceClassV2::CompilerFfiLink
        );
        assert_eq!(
            evidence.authorized().evidence_class(),
            WorkerEvidenceClassV2::CompilerFfiLink
        );
        assert!(!evidence.grants_publication_authority());
        assert!(!evidence.grants_load_authority());
        assert!(!evidence.grants_launch_authority());
    }

    #[test]
    #[ignore = "requires the explicitly configured gfx942 OCML worker"]
    fn configured_cpp_worker_v2_resolves_measured_ocml_without_request_provider() {
        let worker_path = required_env("FE2O3_LLVM_LINK_WORKER");
        let worker_bytes = fs::read(&worker_path).unwrap();
        let measurement = WorkerMeasurementV1::new(
            ContentIdentityV1::calculate(&worker_bytes),
            required_env("FE2O3_LLVM_LINK_WORKER_BUILD_ID"),
            required_env("FE2O3_LLVM_BUILD_ID"),
        )
        .unwrap();
        let pinned = PinnedWorkerV1::open(worker_path, measurement.clone()).unwrap();
        let expected_output = fs::read(required_env("FE2O3_LLVM_V2_OCML_EXPECTED_OUTPUT")).unwrap();

        let (_directory, request) =
            make_ocml_handoff_request(OCML_MODULE, &expected_output, &measurement, 0xa1);
        let attempt = request.attempt();
        let handoff_identity = request.handoff_identity();
        let execution = pinned
            .execute_compiler_handoff_v2(
                &request,
                WorkerExecutionLimitsV1::new(
                    Duration::from_secs(120),
                    MAX_WORKER_RESPONSE_BYTES,
                    DEFAULT_WORKER_STDERR_BYTES,
                )
                .unwrap(),
            )
            .unwrap();

        assert_eq!(execution.attempt(), attempt);
        assert_eq!(execution.handoff_identity(), handoff_identity);
        assert_eq!(execution.worker_executable(), measurement.executable());
        let response = execution.response();
        assert_eq!(response.stage(), WorkerStageV1::Complete);
        assert_eq!(
            response.worker_build_identity(),
            measurement.worker_build_identity()
        );
        assert_eq!(response.request_id(), request.sealed_request().request_id());
        assert_eq!(
            response.request_identity(),
            request.sealed_request().identity()
        );
        assert_eq!(
            response.compiler_envelope_identity(),
            request.sealed_request().compiler_envelope_identity()
        );
        assert!(response.diagnostics().iter().any(|diagnostic| {
            diagnostic
                == "device_library.check=identity status=ok provider=gfx942-ocml-v1 \
roots=[__ocml_sin_f32] files=4"
        }));
        let output = response.output().expect("OCML link did not produce output");
        assert!(output.identity().matches(output.bytes()));
        assert_eq!(output.bytes(), expected_output);
        let inspected = fe2o3_hsaco::inspect(output.bytes()).unwrap();
        assert_eq!(inspected.target().to_string(), TARGET);
        assert_eq!(
            inspected.code_object_version(),
            fe2o3_hsaco::CodeObjectVersion::V5
        );
        assert!(!execution.grants_publication_authority());
        assert!(!execution.grants_load_authority());
        assert!(!execution.grants_launch_authority());
    }

    #[test]
    #[ignore = "requires the explicitly configured gfx942 OCML worker"]
    fn configured_cpp_worker_v2_rejects_omitted_measured_ocml_import() {
        let worker_path = required_env("FE2O3_LLVM_LINK_WORKER");
        let worker_bytes = fs::read(&worker_path).unwrap();
        let measurement = WorkerMeasurementV1::new(
            ContentIdentityV1::calculate(&worker_bytes),
            required_env("FE2O3_LLVM_LINK_WORKER_BUILD_ID"),
            required_env("FE2O3_LLVM_BUILD_ID"),
        )
        .unwrap();
        let pinned = PinnedWorkerV1::open(worker_path, measurement.clone()).unwrap();
        let expected_output = fs::read(required_env("FE2O3_LLVM_V2_OCML_EXPECTED_OUTPUT")).unwrap();
        let (_directory, request) = make_ocml_handoff_request(
            OCML_OMITTED_IMPORT_MODULE,
            &expected_output,
            &measurement,
            0xb1,
        );

        let execution = pinned
            .execute_compiler_handoff_v2(
                &request,
                WorkerExecutionLimitsV1::new(
                    Duration::from_secs(120),
                    MAX_WORKER_RESPONSE_BYTES,
                    DEFAULT_WORKER_STDERR_BYTES,
                )
                .unwrap(),
            )
            .unwrap();
        let response = execution.response();
        assert_eq!(response.stage(), WorkerStageV1::InputValidation);
        assert!(response.output().is_none());
        assert_eq!(
            response.diagnostics(),
            &["compiler-module import manifest mismatch: \
omitted=[__ocml_sqrt_f32] extra=[]"]
        );
    }

    #[test]
    #[ignore = "requires the explicitly configured gfx942 OCML worker"]
    fn configured_cpp_worker_v2_rejects_inline_assembly_imports_before_ocml_linking() {
        let worker_path = required_env("FE2O3_LLVM_LINK_WORKER");
        let worker_bytes = fs::read(&worker_path).unwrap();
        let measurement = WorkerMeasurementV1::new(
            ContentIdentityV1::calculate(&worker_bytes),
            required_env("FE2O3_LLVM_LINK_WORKER_BUILD_ID"),
            required_env("FE2O3_LLVM_BUILD_ID"),
        )
        .unwrap();
        let pinned = PinnedWorkerV1::open(worker_path, measurement.clone()).unwrap();
        let expected_output = fs::read(required_env("FE2O3_LLVM_V2_OCML_EXPECTED_OUTPUT")).unwrap();

        for (module, invocation_byte, omitted) in [
            (OCML_MODULE_ASM_IMPORT_MODULE, 0xc1, "hidden_runtime_import"),
            (OCML_FUNCTION_ASM_IMPORT_MODULE, 0xd1, "__ocml_sqrt_f32"),
            (
                OCML_LOCAL_MODULE_ASM_IMPORT_MODULE,
                0xe1,
                "local_hidden_import",
            ),
            (
                OCML_LOCAL_FUNCTION_ASM_IMPORT_MODULE,
                0xf1,
                "local_hidden_import",
            ),
        ] {
            let (_directory, request) =
                make_ocml_handoff_request(module, &expected_output, &measurement, invocation_byte);
            let execution = pinned
                .execute_compiler_handoff_v2(
                    &request,
                    WorkerExecutionLimitsV1::new(
                        Duration::from_secs(120),
                        MAX_WORKER_RESPONSE_BYTES,
                        DEFAULT_WORKER_STDERR_BYTES,
                    )
                    .unwrap(),
                )
                .unwrap();
            let response = execution.response();
            assert_eq!(response.stage(), WorkerStageV1::InputValidation);
            assert!(response.output().is_none());
            assert_eq!(
                response.diagnostics(),
                &[format!(
                    "compiler-module object import manifest mismatch: omitted=[{omitted}] extra=[]"
                )]
            );
        }
    }
}
