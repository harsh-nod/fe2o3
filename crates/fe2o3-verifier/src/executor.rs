use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::{
    InvocationPlan, MAX_RESULT_BYTES, ProofResultV1, RecorderTermination, ResultError,
    parse_recorder_result,
};

pub const MAX_CAPTURE_BYTES: usize = 1024 * 1024;
const POLL_INTERVAL: Duration = Duration::from_millis(2);
const CAPTURE_SHUTDOWN_GRACE: Duration = Duration::from_millis(100);

pub(crate) fn spawn_artifact_coordinated_child(command: &mut Command) -> io::Result<Child> {
    fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
}

#[cfg(test)]
pub(crate) fn output_artifact_coordinated_child(
    command: &mut Command,
) -> io::Result<std::process::Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn_artifact_coordinated_child(command)?.wait_with_output()
}

#[cfg(test)]
pub(crate) fn status_artifact_coordinated_child(command: &mut Command) -> io::Result<ExitStatus> {
    spawn_artifact_coordinated_child(command)?.wait()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}

impl ExecutionLimits {
    pub fn new(max_stdout_bytes: usize, max_stderr_bytes: usize) -> Result<Self, ExecutionError> {
        for (stream, limit) in [
            (OutputStream::Stdout, max_stdout_bytes),
            (OutputStream::Stderr, max_stderr_bytes),
        ] {
            if limit == 0 || limit > MAX_CAPTURE_BYTES {
                return Err(ExecutionError::without_output(
                    ExecutionErrorKind::CaptureLimitOutOfRange {
                        stream,
                        max: MAX_CAPTURE_BYTES,
                    },
                ));
            }
        }
        Ok(Self {
            max_stdout_bytes,
            max_stderr_bytes,
        })
    }

    pub const fn max_stdout_bytes(self) -> usize {
        self.max_stdout_bytes
    }

    pub const fn max_stderr_bytes(self) -> usize {
        self.max_stderr_bytes
    }
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_stdout_bytes: 64 * 1024,
            max_stderr_bytes: 64 * 1024,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProcessOutput {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl ProcessOutput {
    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionSuccess {
    result: ProofResultV1,
    output: ProcessOutput,
}

impl ExecutionSuccess {
    pub const fn result(&self) -> &ProofResultV1 {
        &self.result
    }

    pub const fn output(&self) -> &ProcessOutput {
        &self.output
    }
}

/// Executes the evidence recorder described by `plan` without shell interpretation.
///
/// The child receives exactly the planned argv, an empty environment, `/` as its
/// working directory, and null stdin. Only a bounded result file accepted by
/// `parse_recorder_result` can produce a recorder result record; stdout is never
/// parsed as a result. Parsing does not establish that a verifier or solver ran.
pub fn execute_recorder(
    plan: &InvocationPlan,
    limits: ExecutionLimits,
) -> Result<ExecutionSuccess, ExecutionError> {
    validate_execution_paths(plan)?;
    let _request_guard = materialize_request(plan)?;

    if Path::new(plan.result_file())
        .try_exists()
        .map_err(|error| {
            ExecutionError::io(
                ExecutionStage::InspectResultPath,
                error,
                ProcessOutput::default(),
            )
        })?
    {
        return Err(ExecutionError::without_output(
            ExecutionErrorKind::ResultPathAlreadyExists,
        ));
    }

    let mut command = Command::new(plan.command().program());
    command
        .args(plan.command().arguments())
        .env_clear()
        .current_dir(std::path::MAIN_SEPARATOR_STR)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = spawn_artifact_coordinated_child(&mut command).map_err(|error| {
        ExecutionError::new(
            ExecutionErrorKind::SpawnFailed(error.kind()),
            ProcessOutput::default(),
        )
    })?;
    let output = supervise_child(child, plan.timeout_seconds(), limits)?;
    let result_bytes = read_bounded_result(plan.result_file(), &output)?;
    let result = parse_recorder_result(&result_bytes, plan, RecorderTermination::Exited(0))
        .map_err(|error| {
            ExecutionError::new(ExecutionErrorKind::InvalidEnvelope(error), output.clone())
        })?;
    Ok(ExecutionSuccess { result, output })
}

fn validate_execution_paths(plan: &InvocationPlan) -> Result<(), ExecutionError> {
    for (field, value) in [
        (ExecutionPath::RecorderProgram, plan.command().program()),
        (ExecutionPath::VerifierProgram, plan.verifier_program()),
        (ExecutionPath::SolverProgram, plan.solver_program()),
        (ExecutionPath::RequestFile, plan.request_file()),
        (ExecutionPath::ResultFile, plan.result_file()),
    ] {
        if !Path::new(value).is_absolute() {
            return Err(ExecutionError::without_output(
                ExecutionErrorKind::PathNotAbsolute { field },
            ));
        }
    }
    if plan.request_file() == plan.result_file() {
        return Err(ExecutionError::without_output(
            ExecutionErrorKind::RequestResultPathAlias,
        ));
    }
    Ok(())
}

fn materialize_request(plan: &InvocationPlan) -> Result<RequestFileGuard<'_>, ExecutionError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(plan.request_file())
        .map_err(|error| {
            ExecutionError::io(
                ExecutionStage::CreateRequest,
                error,
                ProcessOutput::default(),
            )
        })?;
    let guard = RequestFileGuard(Path::new(plan.request_file()));
    file.write_all(plan.request_bytes()).map_err(|error| {
        ExecutionError::io(
            ExecutionStage::WriteRequest,
            error,
            ProcessOutput::default(),
        )
    })?;
    file.flush().map_err(|error| {
        ExecutionError::io(
            ExecutionStage::WriteRequest,
            error,
            ProcessOutput::default(),
        )
    })?;
    Ok(guard)
}

pub(crate) fn supervise_child(
    child: Child,
    timeout_seconds: u32,
    limits: ExecutionLimits,
) -> Result<ProcessOutput, ExecutionError> {
    let mut child = ChildGuard(Some(child));
    let stdout = child
        .as_mut()
        .stdout
        .take()
        .ok_or_else(|| ExecutionError::without_output(ExecutionErrorKind::MissingPipe))?;
    let stderr = child
        .as_mut()
        .stderr
        .take()
        .ok_or_else(|| ExecutionError::without_output(ExecutionErrorKind::MissingPipe))?;
    let stdout = CaptureTask::spawn(stdout, limits.max_stdout_bytes);
    let stderr = CaptureTask::spawn(stderr, limits.max_stderr_bytes);
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
        .ok_or_else(|| ExecutionError::without_output(ExecutionErrorKind::DeadlineOverflow))?;

    let mut forced = None;
    let status = loop {
        if stdout.exceeded() {
            forced = Some(ExecutionErrorKind::OutputTooLarge {
                stream: OutputStream::Stdout,
                max: limits.max_stdout_bytes,
            });
        } else if stderr.exceeded() {
            forced = Some(ExecutionErrorKind::OutputTooLarge {
                stream: OutputStream::Stderr,
                max: limits.max_stderr_bytes,
            });
        } else if Instant::now() >= deadline {
            forced = Some(ExecutionErrorKind::TimedOut);
        }

        if forced.is_some() {
            let _ = child.as_mut().kill();
            break child.as_mut().wait().map_err(|error| {
                ExecutionError::io(
                    ExecutionStage::WaitForRecorder,
                    error,
                    ProcessOutput::default(),
                )
            })?;
        }
        if let Some(status) = child.as_mut().try_wait().map_err(|error| {
            ExecutionError::io(
                ExecutionStage::WaitForRecorder,
                error,
                ProcessOutput::default(),
            )
        })? {
            break status;
        }
        thread::sleep(POLL_INTERVAL);
    };
    child.0 = None;

    let capture_deadline = if forced.is_some() {
        Instant::now()
            .checked_add(CAPTURE_SHUTDOWN_GRACE)
            .unwrap_or(deadline)
    } else {
        deadline
    };
    while (!stdout.is_finished() || !stderr.is_finished()) && Instant::now() < capture_deadline {
        thread::sleep(POLL_INTERVAL);
    }
    if !stdout.is_finished() || !stderr.is_finished() {
        return Err(ExecutionError::without_output(
            forced.unwrap_or(ExecutionErrorKind::TimedOut),
        ));
    }

    let stdout_exceeded = stdout.exceeded();
    let stderr_exceeded = stderr.exceeded();
    let stdout = stdout.finish(OutputStream::Stdout)?;
    let stderr = stderr.finish(OutputStream::Stderr)?;
    let output = ProcessOutput { stdout, stderr };

    if let Some(kind) = forced {
        return Err(ExecutionError::new(kind, output));
    }
    if stdout_exceeded {
        return Err(ExecutionError::new(
            ExecutionErrorKind::OutputTooLarge {
                stream: OutputStream::Stdout,
                max: limits.max_stdout_bytes,
            },
            output,
        ));
    }
    if stderr_exceeded {
        return Err(ExecutionError::new(
            ExecutionErrorKind::OutputTooLarge {
                stream: OutputStream::Stderr,
                max: limits.max_stderr_bytes,
            },
            output,
        ));
    }

    let termination = termination(status);
    if termination != RecorderTermination::Exited(0) {
        let kind = match termination {
            RecorderTermination::Exited(code) => ExecutionErrorKind::Exited(code),
            RecorderTermination::Signaled(signal) => ExecutionErrorKind::Signaled(signal),
            RecorderTermination::TimedOut => ExecutionErrorKind::TimedOut,
        };
        return Err(ExecutionError::new(kind, output));
    }

    Ok(output)
}

fn read_bounded_result(path: &str, output: &ProcessOutput) -> Result<Vec<u8>, ExecutionError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ExecutionError::io(ExecutionStage::ReadResult, error, output.clone()))?;
    if !metadata.file_type().is_file() {
        return Err(ExecutionError::new(
            ExecutionErrorKind::ResultNotRegularFile,
            output.clone(),
        ));
    }
    if metadata.len() > MAX_RESULT_BYTES as u64 {
        return Err(ExecutionError::new(
            ExecutionErrorKind::InvalidEnvelope(ResultError::TooLarge {
                max: MAX_RESULT_BYTES,
            }),
            output.clone(),
        ));
    }
    let file = File::open(path)
        .map_err(|error| ExecutionError::io(ExecutionStage::ReadResult, error, output.clone()))?;
    let mut bytes = Vec::with_capacity(MAX_RESULT_BYTES.min(8192));
    file.take((MAX_RESULT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| ExecutionError::io(ExecutionStage::ReadResult, error, output.clone()))?;
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(ExecutionError::new(
            ExecutionErrorKind::InvalidEnvelope(ResultError::TooLarge {
                max: MAX_RESULT_BYTES,
            }),
            output.clone(),
        ));
    }
    Ok(bytes)
}

fn termination(status: ExitStatus) -> RecorderTermination {
    if let Some(code) = status.code() {
        return RecorderTermination::Exited(code);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        RecorderTermination::Signaled(status.signal().unwrap_or(0))
    }
    #[cfg(not(unix))]
    {
        RecorderTermination::Signaled(0)
    }
}

struct CaptureTask {
    handle: JoinHandle<io::Result<Vec<u8>>>,
    exceeded: Arc<AtomicBool>,
}

impl CaptureTask {
    fn spawn<R>(mut reader: R, max: usize) -> Self
    where
        R: Read + Send + 'static,
    {
        let exceeded = Arc::new(AtomicBool::new(false));
        let thread_exceeded = Arc::clone(&exceeded);
        let handle = thread::spawn(move || {
            let mut retained = Vec::with_capacity(max.min(8192));
            let mut buffer = [0_u8; 8192];
            loop {
                let count = reader.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                let remaining = max.saturating_sub(retained.len());
                retained.extend_from_slice(&buffer[..count.min(remaining)]);
                if count > remaining {
                    thread_exceeded.store(true, Ordering::Release);
                }
            }
            Ok(retained)
        });
        Self { handle, exceeded }
    }

    fn exceeded(&self) -> bool {
        self.exceeded.load(Ordering::Acquire)
    }

    fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    fn finish(self, stream: OutputStream) -> Result<Vec<u8>, ExecutionError> {
        match self.handle.join() {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(error)) => Err(ExecutionError::io(
                ExecutionStage::CaptureOutput { stream },
                error,
                ProcessOutput::default(),
            )),
            Err(_) => Err(ExecutionError::without_output(
                ExecutionErrorKind::CaptureThreadPanicked { stream },
            )),
        }
    }
}

struct ChildGuard(Option<Child>);

impl ChildGuard {
    fn as_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("child guard is populated")
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct RequestFileGuard<'a>(&'a Path);

impl Drop for RequestFileGuard<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionPath {
    RecorderProgram,
    VerifierProgram,
    SolverProgram,
    RequestFile,
    ResultFile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionStage {
    InspectResultPath,
    CreateRequest,
    WriteRequest,
    WaitForRecorder,
    ReadResult,
    CaptureOutput { stream: OutputStream },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExecutionErrorKind {
    CaptureLimitOutOfRange {
        stream: OutputStream,
        max: usize,
    },
    PathNotAbsolute {
        field: ExecutionPath,
    },
    RequestResultPathAlias,
    ResultPathAlreadyExists,
    ResultNotRegularFile,
    SpawnFailed(io::ErrorKind),
    Io {
        stage: ExecutionStage,
        kind: io::ErrorKind,
    },
    MissingPipe,
    DeadlineOverflow,
    OutputTooLarge {
        stream: OutputStream,
        max: usize,
    },
    Exited(i32),
    Signaled(i32),
    TimedOut,
    InvalidEnvelope(ResultError),
    CaptureThreadPanicked {
        stream: OutputStream,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionError {
    kind: ExecutionErrorKind,
    output: ProcessOutput,
}

impl ExecutionError {
    pub(crate) fn from_spawn(kind: io::ErrorKind) -> Self {
        Self::without_output(ExecutionErrorKind::SpawnFailed(kind))
    }

    pub(crate) fn new(kind: ExecutionErrorKind, output: ProcessOutput) -> Self {
        Self { kind, output }
    }

    pub(crate) fn without_output(kind: ExecutionErrorKind) -> Self {
        Self::new(kind, ProcessOutput::default())
    }

    pub(crate) fn io(stage: ExecutionStage, error: io::Error, output: ProcessOutput) -> Self {
        Self::new(
            ExecutionErrorKind::Io {
                stage,
                kind: error.kind(),
            },
            output,
        )
    }

    pub const fn kind(&self) -> &ExecutionErrorKind {
        &self.kind
    }

    pub const fn output(&self) -> &ProcessOutput {
        &self.output
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "evidence recorder execution failed: {:?}",
            self.kind
        )
    }
}

impl std::error::Error for ExecutionError {}
