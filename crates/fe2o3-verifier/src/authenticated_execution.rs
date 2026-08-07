use std::fmt;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(target_os = "linux")]
use std::ffi::CString;
#[cfg(target_os = "linux")]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(target_os = "linux")]
use std::os::raw::{c_char, c_int, c_uint};

use fe2o3_artifacts::DigestAlgorithm;

use crate::executor::supervise_child;
use crate::{
    Digest, ExecutionError, ExecutionLimits, InvocationPaths, InvocationPlan, MAX_PATH_BYTES,
    MAX_RESULT_BYTES, MeasuredToolIdentity, PlanError, ProcessOutput, ProofRequestV1,
    ProofResultV1, RecorderTermination, ResultError, VerifierPolicy, build_invocation_plan,
    canonical_invocation_digest, parse_recorder_result,
};

pub const MAX_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;
const AUTH_RESULT_MAGIC: &str = "FE2O3-VERUS-AUTH-RESULT-V1";
const AUTH_TRANSCRIPT_MAGIC: &[u8; 8] = b"FE2O3VXE";
const RANDOM_SOURCE: &str = "/dev/urandom";
const CANONICAL_REQUEST_PATH: &str = "/fe2o3-authenticated/request-v1";
const CANONICAL_RESULT_PATH: &str = "/fe2o3-authenticated/result-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableRole {
    Verus,
    Solver,
    EvidenceRecorder,
}

impl ExecutableRole {
    const fn memfd_name(self) -> &'static str {
        match self {
            Self::Verus => "fe2o3-verus-v1",
            Self::Solver => "fe2o3-solver-v1",
            Self::EvidenceRecorder => "fe2o3-verus-recorder-v1",
        }
    }
}

/// Source paths for executables admitted by a trusted verifier policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedExecutionProgramsV1 {
    verus: String,
    solver: String,
    evidence_recorder: String,
}

impl AuthenticatedExecutionProgramsV1 {
    pub fn new(
        verus: impl Into<String>,
        solver: impl Into<String>,
        evidence_recorder: impl Into<String>,
    ) -> Result<Self, AuthenticatedExecutionError> {
        Ok(Self {
            verus: checked_program_path(ExecutableRole::Verus, verus.into())?,
            solver: checked_program_path(ExecutableRole::Solver, solver.into())?,
            evidence_recorder: checked_program_path(
                ExecutableRole::EvidenceRecorder,
                evidence_recorder.into(),
            )?,
        })
    }

    pub fn verus(&self) -> &str {
        &self.verus
    }

    pub fn solver(&self) -> &str {
        &self.solver
    }

    pub fn evidence_recorder(&self) -> &str {
        &self.evidence_recorder
    }
}

/// Identity and size of bytes copied into an immutable executable snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableMeasurementV1 {
    role: ExecutableRole,
    identity: MeasuredToolIdentity,
    byte_len: u64,
}

impl ExecutableMeasurementV1 {
    pub const fn role(&self) -> ExecutableRole {
        self.role
    }

    pub const fn identity(&self) -> &MeasuredToolIdentity {
        &self.identity
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// Exact bounded bytes retained by descriptive execution evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundExecutionPayloadV1 {
    bytes: Vec<u8>,
    digest: Digest,
}

impl BoundExecutionPayloadV1 {
    fn new(bytes: Vec<u8>) -> Self {
        let digest = sha256(&bytes);
        Self { bytes, digest }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }
}

/// Descriptive evidence from one measured Verus execution.
///
/// The type has no public constructor. Its transcript commits to the immutable
/// executable snapshots, policy, request, fresh challenge, stdout, stderr, and
/// strict result envelope. It grants no module-load or kernel-launch authority.
///
/// ```compile_fail
/// # fn cannot_launch(evidence: fe2o3_verifier::AuthenticatedVerusExecutionEvidenceV1) {
/// evidence.launch();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedVerusExecutionEvidenceV1 {
    challenge: Digest,
    canonical_invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    verus: ExecutableMeasurementV1,
    solver: ExecutableMeasurementV1,
    evidence_recorder: ExecutableMeasurementV1,
    stdout: BoundExecutionPayloadV1,
    stderr: BoundExecutionPayloadV1,
    result_bytes: BoundExecutionPayloadV1,
    result: ProofResultV1,
    transcript_digest: Digest,
}

impl AuthenticatedVerusExecutionEvidenceV1 {
    pub const fn challenge(&self) -> Digest {
        self.challenge
    }

    pub const fn canonical_invocation_digest(&self) -> Digest {
        self.canonical_invocation_digest
    }

    pub const fn policy_digest(&self) -> Digest {
        self.policy_digest
    }

    pub const fn request_digest(&self) -> Digest {
        self.request_digest
    }

    pub const fn verus(&self) -> &ExecutableMeasurementV1 {
        &self.verus
    }

    pub const fn solver(&self) -> &ExecutableMeasurementV1 {
        &self.solver
    }

    pub const fn evidence_recorder(&self) -> &ExecutableMeasurementV1 {
        &self.evidence_recorder
    }

    pub const fn stdout(&self) -> &BoundExecutionPayloadV1 {
        &self.stdout
    }

    pub const fn stderr(&self) -> &BoundExecutionPayloadV1 {
        &self.stderr
    }

    pub const fn result_bytes(&self) -> &BoundExecutionPayloadV1 {
        &self.result_bytes
    }

    pub const fn result(&self) -> &ProofResultV1 {
        &self.result
    }

    pub const fn transcript_digest(&self) -> Digest {
        self.transcript_digest
    }

    /// Canonical descriptive transcript. Parsing these bytes cannot recreate
    /// this authenticated type; only a measured execution can construct it.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        canonical_transcript_bytes(
            self.challenge,
            self.canonical_invocation_digest,
            self.policy_digest,
            self.request_digest,
            [&self.verus, &self.solver, &self.evidence_recorder],
            [&self.stdout, &self.stderr, &self.result_bytes],
        )
    }
}

/// Measures, snapshots, and executes the policy-approved verifier toolchain.
///
/// Caller-supplied tool identities are not accepted. Each program is copied
/// into a sealed anonymous file while SHA-256 is computed, and that same sealed
/// file is used for execution. The request is sealed before launch and the
/// result file is sealed immediately after the recorder exits.
pub fn execute_authenticated_verus(
    request: ProofRequestV1,
    programs: AuthenticatedExecutionProgramsV1,
    timeout_seconds: u32,
    policy: &VerifierPolicy,
    limits: ExecutionLimits,
) -> Result<AuthenticatedVerusExecutionEvidenceV1, AuthenticatedExecutionError> {
    let challenge = random_challenge()?;
    execute_authenticated_verus_with_challenge(
        request,
        programs,
        timeout_seconds,
        policy,
        limits,
        challenge,
    )
}

fn execute_authenticated_verus_with_challenge(
    request: ProofRequestV1,
    programs: AuthenticatedExecutionProgramsV1,
    timeout_seconds: u32,
    policy: &VerifierPolicy,
    limits: ExecutionLimits,
    challenge: Digest,
) -> Result<AuthenticatedVerusExecutionEvidenceV1, AuthenticatedExecutionError> {
    if challenge.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(AuthenticatedExecutionError::InvalidChallenge);
    }

    let verus = SealedExecutable::measure(
        ExecutableRole::Verus,
        programs.verus(),
        policy.expected_tools().verifier(),
    )?;
    let solver = SealedExecutable::measure(
        ExecutableRole::Solver,
        programs.solver(),
        policy.expected_tools().solver(),
    )?;
    let recorder = SealedExecutable::measure(
        ExecutableRole::EvidenceRecorder,
        programs.evidence_recorder(),
        policy.expected_tools().evidence_recorder(),
    )?;

    let plan = build_invocation_plan(
        request,
        policy.expected_tools().clone(),
        InvocationPaths::new(
            programs.verus,
            programs.solver,
            programs.evidence_recorder,
            CANONICAL_REQUEST_PATH,
            CANONICAL_RESULT_PATH,
        )?,
        timeout_seconds,
        policy,
    )?;
    let invocation_digest = canonical_invocation_digest(&plan);
    let policy_digest = sha256(&policy.to_canonical_bytes());
    let request_digest = sha256(plan.request_bytes());
    let request_file = SealedData::immutable("fe2o3-verus-request-v1", plan.request_bytes())?;
    let mut result_file = SealedData::mutable("fe2o3-verus-result-v1")?;

    let bindings = AuthenticatedResultBindings {
        challenge,
        invocation_digest,
        policy_digest,
        request_digest,
        verus_digest: verus.measurement.identity.executable_digest(),
        solver_digest: solver.measurement.identity.executable_digest(),
        recorder_digest: recorder.measurement.identity.executable_digest(),
    };
    let mut command = Command::new(recorder.proc_path());
    command
        .args([
            "--request",
            &request_file.proc_path(),
            "--result",
            &result_file.proc_path(),
            "--verifier",
            &verus.proc_path(),
            "--solver",
            &solver.proc_path(),
            "--timeout-seconds",
            &timeout_seconds.to_string(),
            "--auth-challenge",
            &challenge.to_hex(),
            "--auth-invocation",
            &invocation_digest.to_hex(),
            "--auth-policy",
            &policy_digest.to_hex(),
            "--auth-request",
            &request_digest.to_hex(),
            "--auth-verus",
            &bindings.verus_digest.to_hex(),
            "--auth-solver",
            &bindings.solver_digest.to_hex(),
            "--auth-recorder",
            &bindings.recorder_digest.to_hex(),
        ])
        .env_clear()
        .current_dir(std::path::MAIN_SEPARATOR_STR)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = command.spawn().map_err(|error| {
        AuthenticatedExecutionError::Execution(ExecutionError::from_spawn(error.kind()))
    })?;
    let output = supervise_child(child, timeout_seconds, limits)
        .map_err(AuthenticatedExecutionError::Execution)?;
    let result_bytes = result_file.seal_and_read(MAX_RESULT_BYTES)?;
    let result = parse_authenticated_result(&result_bytes, &plan, bindings)?;

    Ok(build_evidence(EvidenceParts {
        challenge,
        canonical_invocation_digest: invocation_digest,
        policy_digest,
        request_digest,
        verus: verus.measurement,
        solver: solver.measurement,
        evidence_recorder: recorder.measurement,
        output,
        result_bytes,
        result,
    }))
}

struct EvidenceParts {
    challenge: Digest,
    canonical_invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    verus: ExecutableMeasurementV1,
    solver: ExecutableMeasurementV1,
    evidence_recorder: ExecutableMeasurementV1,
    output: ProcessOutput,
    result_bytes: Vec<u8>,
    result: ProofResultV1,
}

fn build_evidence(parts: EvidenceParts) -> AuthenticatedVerusExecutionEvidenceV1 {
    let stdout = BoundExecutionPayloadV1::new(parts.output.stdout().to_vec());
    let stderr = BoundExecutionPayloadV1::new(parts.output.stderr().to_vec());
    let result_bytes = BoundExecutionPayloadV1::new(parts.result_bytes);
    let transcript = canonical_transcript_bytes(
        parts.challenge,
        parts.canonical_invocation_digest,
        parts.policy_digest,
        parts.request_digest,
        [&parts.verus, &parts.solver, &parts.evidence_recorder],
        [&stdout, &stderr, &result_bytes],
    );
    AuthenticatedVerusExecutionEvidenceV1 {
        challenge: parts.challenge,
        canonical_invocation_digest: parts.canonical_invocation_digest,
        policy_digest: parts.policy_digest,
        request_digest: parts.request_digest,
        verus: parts.verus,
        solver: parts.solver,
        evidence_recorder: parts.evidence_recorder,
        stdout,
        stderr,
        result_bytes,
        result: parts.result,
        transcript_digest: sha256(&transcript),
    }
}

fn canonical_transcript_bytes(
    challenge: Digest,
    invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    executables: [&ExecutableMeasurementV1; 3],
    payloads: [&BoundExecutionPayloadV1; 3],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(AUTH_TRANSCRIPT_MAGIC);
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    for digest in [challenge, invocation_digest, policy_digest, request_digest] {
        bytes.extend_from_slice(digest.as_bytes());
    }
    for executable in executables {
        bytes.push(match executable.role {
            ExecutableRole::Verus => 1,
            ExecutableRole::Solver => 2,
            ExecutableRole::EvidenceRecorder => 3,
        });
        put_text(&mut bytes, executable.identity.name().as_str());
        put_text(&mut bytes, executable.identity.version().as_str());
        bytes.extend_from_slice(executable.identity.executable_digest().as_bytes());
        bytes.extend_from_slice(executable.identity.configuration_digest().as_bytes());
        bytes.extend_from_slice(&executable.byte_len.to_le_bytes());
    }
    for payload in payloads {
        bytes.extend_from_slice(&(payload.bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(payload.digest.as_bytes());
        bytes.extend_from_slice(&payload.bytes);
    }
    bytes
}

#[derive(Clone, Copy)]
struct AuthenticatedResultBindings {
    challenge: Digest,
    invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    verus_digest: Digest,
    solver_digest: Digest,
    recorder_digest: Digest,
}

fn parse_authenticated_result(
    bytes: &[u8],
    plan: &InvocationPlan,
    expected: AuthenticatedResultBindings,
) -> Result<ProofResultV1, AuthenticatedExecutionError> {
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(AuthenticatedExecutionError::Result(
            AuthenticatedResultError::TooLarge {
                max: MAX_RESULT_BYTES,
            },
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AuthenticatedExecutionError::Result(AuthenticatedResultError::InvalidUtf8))?;
    let mut remainder = text;
    let magic = take_line(&mut remainder)?;
    if magic != AUTH_RESULT_MAGIC {
        return Err(AuthenticatedExecutionError::Result(
            AuthenticatedResultError::MalformedEnvelope,
        ));
    }
    for (field_name, field, expected_digest) in [
        (
            "challenge",
            AuthenticatedBindingField::Challenge,
            expected.challenge,
        ),
        (
            "invocation",
            AuthenticatedBindingField::Invocation,
            expected.invocation_digest,
        ),
        (
            "policy",
            AuthenticatedBindingField::Policy,
            expected.policy_digest,
        ),
        (
            "request",
            AuthenticatedBindingField::Request,
            expected.request_digest,
        ),
        (
            "verus",
            AuthenticatedBindingField::Verus,
            expected.verus_digest,
        ),
        (
            "solver",
            AuthenticatedBindingField::Solver,
            expected.solver_digest,
        ),
        (
            "recorder",
            AuthenticatedBindingField::EvidenceRecorder,
            expected.recorder_digest,
        ),
    ] {
        let line = take_line(&mut remainder)?;
        let actual = Digest::from_hex(auth_field(line, field_name)?).map_err(|_| {
            AuthenticatedExecutionError::Result(AuthenticatedResultError::MalformedDigest { field })
        })?;
        if actual != expected_digest {
            return Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::BindingMismatch { field },
            ));
        }
    }
    let result_len_text = auth_field(take_line(&mut remainder)?, "result-bytes")?;
    if result_len_text.is_empty()
        || (result_len_text.len() > 1 && result_len_text.starts_with('0'))
        || !result_len_text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(AuthenticatedExecutionError::Result(
            AuthenticatedResultError::MalformedLength,
        ));
    }
    let result_len = result_len_text.parse::<usize>().map_err(|_| {
        AuthenticatedExecutionError::Result(AuthenticatedResultError::MalformedLength)
    })?;
    if result_len != remainder.len() {
        return Err(AuthenticatedExecutionError::Result(
            AuthenticatedResultError::ResultLengthMismatch,
        ));
    }
    parse_recorder_result(remainder.as_bytes(), plan, RecorderTermination::Exited(0)).map_err(
        |error| AuthenticatedExecutionError::Result(AuthenticatedResultError::ProofResult(error)),
    )
}

fn take_line<'a>(remainder: &mut &'a str) -> Result<&'a str, AuthenticatedExecutionError> {
    let (line, rest) = remainder.split_once('\n').ok_or({
        AuthenticatedExecutionError::Result(AuthenticatedResultError::MalformedEnvelope)
    })?;
    *remainder = rest;
    Ok(line)
}

fn auth_field<'a>(
    line: &'a str,
    expected: &'static str,
) -> Result<&'a str, AuthenticatedExecutionError> {
    line.strip_prefix(expected)
        .and_then(|value| value.strip_prefix('='))
        .ok_or(AuthenticatedExecutionError::Result(
            AuthenticatedResultError::UnexpectedField { expected },
        ))
}

struct SealedExecutable {
    file: File,
    measurement: ExecutableMeasurementV1,
}

impl SealedExecutable {
    fn measure(
        role: ExecutableRole,
        path: &str,
        expected: &MeasuredToolIdentity,
    ) -> Result<Self, AuthenticatedExecutionError> {
        let mut source =
            File::open(path).map_err(|error| AuthenticatedExecutionError::ExecutableIo {
                role,
                operation: ExecutableOperation::Open,
                kind: error.kind(),
            })?;
        let metadata =
            source
                .metadata()
                .map_err(|error| AuthenticatedExecutionError::ExecutableIo {
                    role,
                    operation: ExecutableOperation::Inspect,
                    kind: error.kind(),
                })?;
        if !metadata.is_file() {
            return Err(AuthenticatedExecutionError::ExecutableNotRegular { role });
        }
        if metadata.len() == 0 || metadata.len() > MAX_EXECUTABLE_BYTES {
            return Err(AuthenticatedExecutionError::ExecutableSizeOutOfRange {
                role,
                max: MAX_EXECUTABLE_BYTES,
            });
        }

        let mut file = create_memfd(role.memfd_name()).map_err(|error| {
            AuthenticatedExecutionError::ExecutableIo {
                role,
                operation: ExecutableOperation::CreateSnapshot,
                kind: error.kind(),
            }
        })?;
        let mut exact_bytes = Vec::with_capacity(metadata.len().min(16 * 1024 * 1024) as usize);
        let mut byte_len = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let count = source.read(&mut buffer).map_err(|error| {
                AuthenticatedExecutionError::ExecutableIo {
                    role,
                    operation: ExecutableOperation::Read,
                    kind: error.kind(),
                }
            })?;
            if count == 0 {
                break;
            }
            byte_len = byte_len.checked_add(count as u64).ok_or(
                AuthenticatedExecutionError::ExecutableSizeOutOfRange {
                    role,
                    max: MAX_EXECUTABLE_BYTES,
                },
            )?;
            if byte_len > MAX_EXECUTABLE_BYTES {
                return Err(AuthenticatedExecutionError::ExecutableSizeOutOfRange {
                    role,
                    max: MAX_EXECUTABLE_BYTES,
                });
            }
            exact_bytes.extend_from_slice(&buffer[..count]);
            file.write_all(&buffer[..count]).map_err(|error| {
                AuthenticatedExecutionError::ExecutableIo {
                    role,
                    operation: ExecutableOperation::WriteSnapshot,
                    kind: error.kind(),
                }
            })?;
        }
        if byte_len == 0 {
            return Err(AuthenticatedExecutionError::ExecutableSizeOutOfRange {
                role,
                max: MAX_EXECUTABLE_BYTES,
            });
        }
        file.flush()
            .map_err(|error| AuthenticatedExecutionError::ExecutableIo {
                role,
                operation: ExecutableOperation::WriteSnapshot,
                kind: error.kind(),
            })?;
        seal(&file).map_err(|error| AuthenticatedExecutionError::ExecutableIo {
            role,
            operation: ExecutableOperation::SealSnapshot,
            kind: error.kind(),
        })?;
        let measured = sha256(&exact_bytes);
        if measured != expected.executable_digest() {
            return Err(AuthenticatedExecutionError::ExecutableDigestMismatch {
                role,
                expected: expected.executable_digest(),
                measured,
            });
        }
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            AuthenticatedExecutionError::ExecutableIo {
                role,
                operation: ExecutableOperation::Read,
                kind: error.kind(),
            }
        })?;
        Ok(Self {
            file,
            measurement: ExecutableMeasurementV1 {
                role,
                identity: expected.clone(),
                byte_len,
            },
        })
    }

    fn proc_path(&self) -> String {
        proc_fd_path(&self.file)
    }
}

struct SealedData {
    file: File,
}

impl SealedData {
    fn immutable(name: &str, bytes: &[u8]) -> Result<Self, AuthenticatedExecutionError> {
        let mut value = Self::mutable(name)?;
        value
            .file
            .write_all(bytes)
            .map_err(|error| AuthenticatedExecutionError::DataIo {
                operation: DataOperation::Write,
                kind: error.kind(),
            })?;
        value
            .file
            .flush()
            .map_err(|error| AuthenticatedExecutionError::DataIo {
                operation: DataOperation::Write,
                kind: error.kind(),
            })?;
        seal(&value.file).map_err(|error| AuthenticatedExecutionError::DataIo {
            operation: DataOperation::Seal,
            kind: error.kind(),
        })?;
        value.file.seek(SeekFrom::Start(0)).map_err(|error| {
            AuthenticatedExecutionError::DataIo {
                operation: DataOperation::Read,
                kind: error.kind(),
            }
        })?;
        Ok(value)
    }

    fn mutable(name: &str) -> Result<Self, AuthenticatedExecutionError> {
        let file = create_memfd(name).map_err(|error| AuthenticatedExecutionError::DataIo {
            operation: DataOperation::Create,
            kind: error.kind(),
        })?;
        Ok(Self { file })
    }

    fn proc_path(&self) -> String {
        proc_fd_path(&self.file)
    }

    fn seal_and_read(&mut self, max: usize) -> Result<Vec<u8>, AuthenticatedExecutionError> {
        seal(&self.file).map_err(|error| AuthenticatedExecutionError::DataIo {
            operation: DataOperation::Seal,
            kind: error.kind(),
        })?;
        self.file.seek(SeekFrom::Start(0)).map_err(|error| {
            AuthenticatedExecutionError::DataIo {
                operation: DataOperation::Read,
                kind: error.kind(),
            }
        })?;
        let mut bytes = Vec::with_capacity(max.min(8192));
        Read::by_ref(&mut self.file)
            .take((max + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| AuthenticatedExecutionError::DataIo {
                operation: DataOperation::Read,
                kind: error.kind(),
            })?;
        if bytes.len() > max {
            return Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::TooLarge { max },
            ));
        }
        Ok(bytes)
    }
}

#[cfg(target_os = "linux")]
fn create_memfd(name: &str) -> io::Result<File> {
    const MFD_ALLOW_SEALING: c_uint = 0x0002;
    let name = CString::new(name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "memfd name contains NUL"))?;
    // SAFETY: `name` is a live NUL-terminated string and the flags are a
    // documented Linux `memfd_create` value.
    let fd = unsafe { linux_memfd_create(name.as_ptr(), MFD_ALLOW_SEALING) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful `memfd_create` returns a new owned descriptor.
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    Ok(File::from(fd))
}

#[cfg(not(target_os = "linux"))]
fn create_memfd(_name: &str) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "authenticated execution requires Linux memfd sealing",
    ))
}

#[cfg(target_os = "linux")]
fn seal(file: &File) -> io::Result<()> {
    const F_ADD_SEALS: c_int = 1033;
    const ALL_IMMUTABLE_SEALS: c_int = 0x0001 | 0x0002 | 0x0004 | 0x0008;
    // SAFETY: `file` owns a live descriptor and `F_ADD_SEALS` consumes one
    // integer bitset argument.
    if unsafe { linux_fcntl(file.as_raw_fd(), F_ADD_SEALS, ALL_IMMUTABLE_SEALS) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
fn seal(_file: &File) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "authenticated execution requires Linux memfd sealing",
    ))
}

#[cfg(target_os = "linux")]
fn proc_fd_path(file: &File) -> String {
    format!("/proc/self/fd/{}", file.as_raw_fd())
}

#[cfg(not(target_os = "linux"))]
fn proc_fd_path(_file: &File) -> String {
    unreachable!("memfd creation fails before a descriptor path is needed")
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    #[link_name = "memfd_create"]
    fn linux_memfd_create(name: *const c_char, flags: c_uint) -> c_int;

    #[link_name = "fcntl"]
    fn linux_fcntl(fd: c_int, command: c_int, ...) -> c_int;
}

fn random_challenge() -> Result<Digest, AuthenticatedExecutionError> {
    let mut bytes = [0_u8; 32];
    File::open(RANDOM_SOURCE)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| AuthenticatedExecutionError::ChallengeIo(error.kind()))?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(AuthenticatedExecutionError::InvalidChallenge);
    }
    Ok(Digest::from_bytes(bytes))
}

fn checked_program_path(
    role: ExecutableRole,
    path: String,
) -> Result<String, AuthenticatedExecutionError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.chars().any(char::is_control)
        || !Path::new(&path).is_absolute()
    {
        Err(AuthenticatedExecutionError::InvalidExecutablePath { role })
    } else {
        Ok(path)
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn put_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableOperation {
    Open,
    Inspect,
    Read,
    CreateSnapshot,
    WriteSnapshot,
    SealSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DataOperation {
    Create,
    Write,
    Seal,
    Read,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthenticatedBindingField {
    Challenge,
    Invocation,
    Policy,
    Request,
    Verus,
    Solver,
    EvidenceRecorder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthenticatedResultError {
    TooLarge { max: usize },
    InvalidUtf8,
    MalformedEnvelope,
    UnexpectedField { expected: &'static str },
    MalformedDigest { field: AuthenticatedBindingField },
    BindingMismatch { field: AuthenticatedBindingField },
    MalformedLength,
    ResultLengthMismatch,
    ProofResult(ResultError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthenticatedExecutionError {
    InvalidExecutablePath {
        role: ExecutableRole,
    },
    ExecutableIo {
        role: ExecutableRole,
        operation: ExecutableOperation,
        kind: io::ErrorKind,
    },
    ExecutableNotRegular {
        role: ExecutableRole,
    },
    ExecutableSizeOutOfRange {
        role: ExecutableRole,
        max: u64,
    },
    ExecutableDigestMismatch {
        role: ExecutableRole,
        expected: Digest,
        measured: Digest,
    },
    DataIo {
        operation: DataOperation,
        kind: io::ErrorKind,
    },
    ChallengeIo(io::ErrorKind),
    InvalidChallenge,
    Plan(PlanError),
    Execution(ExecutionError),
    Result(AuthenticatedResultError),
}

impl fmt::Display for AuthenticatedExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "authenticated Verus execution failed: {self:?}")
    }
}

impl std::error::Error for AuthenticatedExecutionError {}

impl From<PlanError> for AuthenticatedExecutionError {
    fn from(value: PlanError) -> Self {
        Self::Plan(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AxiomPolicy, Configuration, ConfigurationEntry, CorrelationId, ExecutionTools,
        ProofProperty, ProofTargetIdentity, VerificationModelIdentity,
    };

    fn digest(seed: u8) -> Digest {
        Digest::from_bytes([seed; 32])
    }

    fn tool(role: ExecutableRole, seed: u8) -> ExecutableMeasurementV1 {
        let name = match role {
            ExecutableRole::Verus => "verus",
            ExecutableRole::Solver => "z3",
            ExecutableRole::EvidenceRecorder => "recorder",
        };
        ExecutableMeasurementV1 {
            role,
            identity: MeasuredToolIdentity::new(name, "1", digest(seed), digest(seed + 1)).unwrap(),
            byte_len: u64::from(seed),
        }
    }

    fn payload(bytes: &[u8]) -> BoundExecutionPayloadV1 {
        BoundExecutionPayloadV1::new(bytes.to_vec())
    }

    fn request() -> ProofRequestV1 {
        ProofRequestV1::new(
            CorrelationId::from_bytes([50; 16]),
            ProofTargetIdentity {
                kernel_id: digest(1),
                instance_digest: digest(2),
                source_tree_digest: digest(3),
                crate_graph_digest: digest(4),
                executable_digest: digest(5),
                environment_digest: digest(6),
                artifact_selection_digest: digest(7),
                artifact_contract_digest: digest(8),
                memory_contract_digest: digest(9),
                effects_contract_digest: digest(10),
                type_layout_digest: digest(11),
                capability_semantics_digest: digest(12),
                functional_specification_digest: digest(13),
            },
            Configuration::new(vec![ConfigurationEntry::new("solver", "z3").unwrap()]).unwrap(),
            VerificationModelIdentity::new("gpu-model-v1", digest(20)).unwrap(),
            vec![ProofProperty::Bounds],
            vec![],
        )
        .unwrap()
    }

    fn plan() -> InvocationPlan {
        let tools = ExecutionTools::new(
            tool(ExecutableRole::Verus, 30).identity,
            tool(ExecutableRole::Solver, 32).identity,
            tool(ExecutableRole::EvidenceRecorder, 34).identity,
        );
        let configuration = request().configuration().clone();
        let model = request().model().clone();
        let policy = VerifierPolicy::new(
            tools.clone(),
            configuration,
            model,
            AxiomPolicy::deny_all(),
            60,
        )
        .unwrap();
        build_invocation_plan(
            request(),
            tools,
            InvocationPaths::new(
                "/verus",
                "/z3",
                "/recorder",
                CANONICAL_REQUEST_PATH,
                CANONICAL_RESULT_PATH,
            )
            .unwrap(),
            10,
            &policy,
        )
        .unwrap()
    }

    fn proof_payload() -> Vec<u8> {
        b"FE2O3-VERIFIER-RESULT-V1\ncorrelation=32323232323232323232323232323232\noutcome=proved\nproperties=bounds\ntrusted=\ndiagnostic-hex=\n".to_vec()
    }

    fn auth_envelope(bindings: AuthenticatedResultBindings, payload: &[u8]) -> Vec<u8> {
        format!(
            "{AUTH_RESULT_MAGIC}\nchallenge={}\ninvocation={}\npolicy={}\nrequest={}\nverus={}\nsolver={}\nrecorder={}\nresult-bytes={}\n{}",
            bindings.challenge.to_hex(),
            bindings.invocation_digest.to_hex(),
            bindings.policy_digest.to_hex(),
            bindings.request_digest.to_hex(),
            bindings.verus_digest.to_hex(),
            bindings.solver_digest.to_hex(),
            bindings.recorder_digest.to_hex(),
            payload.len(),
            String::from_utf8_lossy(payload),
        )
        .into_bytes()
    }

    fn bindings(seed: u8) -> AuthenticatedResultBindings {
        AuthenticatedResultBindings {
            challenge: digest(seed),
            invocation_digest: digest(seed + 1),
            policy_digest: digest(seed + 2),
            request_digest: digest(seed + 3),
            verus_digest: digest(seed + 4),
            solver_digest: digest(seed + 5),
            recorder_digest: digest(seed + 6),
        }
    }

    #[test]
    fn canonical_transcript_has_a_stable_golden_digest() {
        let executables = [
            tool(ExecutableRole::Verus, 30),
            tool(ExecutableRole::Solver, 32),
            tool(ExecutableRole::EvidenceRecorder, 34),
        ];
        let payloads = [payload(b"out"), payload(b"err"), payload(b"result")];
        let bytes = canonical_transcript_bytes(
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            [&executables[0], &executables[1], &executables[2]],
            [&payloads[0], &payloads[1], &payloads[2]],
        );
        assert_eq!(
            sha256(&bytes).to_hex(),
            "717934e0ac436ed3da75dfc43bb9efcdd48a0370ecdb3e3b2a5690c58990c427"
        );
    }

    #[test]
    fn authenticated_result_rejects_malformed_and_substituted_bindings() {
        let plan = plan();
        let expected = bindings(40);
        let payload = proof_payload();
        let valid = auth_envelope(expected, &payload);
        assert!(parse_authenticated_result(&valid, &plan, expected).is_ok());

        let mut malformed = valid.clone();
        malformed[0] = b'X';
        assert_eq!(
            parse_authenticated_result(&malformed, &plan, expected),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::MalformedEnvelope
            ))
        );

        for field in [
            AuthenticatedBindingField::Challenge,
            AuthenticatedBindingField::Invocation,
            AuthenticatedBindingField::Policy,
            AuthenticatedBindingField::Request,
            AuthenticatedBindingField::Verus,
            AuthenticatedBindingField::Solver,
            AuthenticatedBindingField::EvidenceRecorder,
        ] {
            let mut substituted_bindings = expected;
            match field {
                AuthenticatedBindingField::Challenge => substituted_bindings.challenge = digest(99),
                AuthenticatedBindingField::Invocation => {
                    substituted_bindings.invocation_digest = digest(99)
                }
                AuthenticatedBindingField::Policy => {
                    substituted_bindings.policy_digest = digest(99)
                }
                AuthenticatedBindingField::Request => {
                    substituted_bindings.request_digest = digest(99)
                }
                AuthenticatedBindingField::Verus => substituted_bindings.verus_digest = digest(99),
                AuthenticatedBindingField::Solver => {
                    substituted_bindings.solver_digest = digest(99)
                }
                AuthenticatedBindingField::EvidenceRecorder => {
                    substituted_bindings.recorder_digest = digest(99)
                }
            }
            let substituted = auth_envelope(substituted_bindings, &payload);
            assert_eq!(
                parse_authenticated_result(&substituted, &plan, expected),
                Err(AuthenticatedExecutionError::Result(
                    AuthenticatedResultError::BindingMismatch { field }
                ))
            );
        }

        let uppercase = String::from_utf8(valid).unwrap().replacen(
            &expected.challenge.to_hex(),
            &"A".repeat(64),
            1,
        );
        assert_eq!(
            parse_authenticated_result(uppercase.as_bytes(), &plan, expected),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::MalformedDigest {
                    field: AuthenticatedBindingField::Challenge
                }
            ))
        );
    }

    #[test]
    fn stale_result_replay_is_rejected_by_fresh_challenge() {
        let plan = plan();
        let first = bindings(50);
        let mut replay_target = first;
        replay_target.challenge = digest(90);
        let replay = auth_envelope(first, &proof_payload());
        assert_eq!(
            parse_authenticated_result(&replay, &plan, replay_target),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::BindingMismatch {
                    field: AuthenticatedBindingField::Challenge
                }
            ))
        );
    }

    #[test]
    fn result_length_and_field_order_are_canonical() {
        let plan = plan();
        let expected = bindings(60);
        let payload = proof_payload();
        let valid = String::from_utf8(auth_envelope(expected, &payload)).unwrap();
        let leading_zero = valid.replace(
            &format!("result-bytes={}", payload.len()),
            &format!("result-bytes=0{}", payload.len()),
        );
        assert_eq!(
            parse_authenticated_result(leading_zero.as_bytes(), &plan, expected),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::MalformedLength
            ))
        );
        let reordered = valid.replacen("challenge=", "policy=", 1);
        assert_eq!(
            parse_authenticated_result(reordered.as_bytes(), &plan, expected),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::UnexpectedField {
                    expected: "challenge"
                }
            ))
        );
    }
}
