//! Measured, sealed execution of an external evidence recorder.
//!
//! This module launches only the recorder snapshot. It measures and seals the
//! caller-policy-selected verifier and solver images, then passes their paths
//! and digests to the recorder. It does not observe either image being run and
//! does not establish that Verus, a solver, or any proof toolchain executed.
//! A `proved` result is the recorder's authenticated report, not an
//! independently authenticated proof result.

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

use crate::executor::{spawn_artifact_coordinated_child, supervise_child};
use crate::{
    Digest, ExecutionError, ExecutionLimits, InvocationPaths, InvocationPlan, MAX_PATH_BYTES,
    MAX_RESULT_BYTES, MeasuredToolIdentity, PlanError, ProcessOutput, ProofRequestV1,
    ProofResultV1, RecorderTermination, ResultError, VerifierPolicy, build_invocation_plan,
    canonical_invocation_digest, parse_recorder_result,
};

pub const MAX_EXECUTABLE_BYTES: u64 = 1024 * 1024 * 1024;
// Legacy V1 wire marker. It authenticates recorder output, not Verus execution.
const AUTH_RESULT_MAGIC: &str = "FE2O3-VERUS-AUTH-RESULT-V1";
const AUTH_TRANSCRIPT_MAGIC: &[u8; 8] = b"FE2O3VXE";
const RANDOM_SOURCE: &str = "/dev/urandom";
const CANONICAL_REQUEST_PATH: &str = "/fe2o3-authenticated/request-v1";
const CANONICAL_RESULT_PATH: &str = "/fe2o3-authenticated/result-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableRole {
    /// Image claimed by policy to be Verus; measured but not launched here.
    Verus,
    /// Image claimed by policy to be the solver; measured but not launched here.
    Solver,
    /// Measured recorder image that this module actually launches.
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

/// Source paths measured for one recorder invocation.
///
/// `claimed_verifier` and `claimed_solver` are sealed and passed to the
/// recorder, but this API does not execute them. The policy that supplies their
/// expected identities is caller-selected and is not a trust root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasuredRecorderInputsV1 {
    claimed_verifier: String,
    claimed_solver: String,
    recorder: String,
}

impl MeasuredRecorderInputsV1 {
    pub fn new(
        claimed_verifier: impl Into<String>,
        claimed_solver: impl Into<String>,
        recorder: impl Into<String>,
    ) -> Result<Self, AuthenticatedExecutionError> {
        Ok(Self {
            claimed_verifier: checked_program_path(ExecutableRole::Verus, claimed_verifier.into())?,
            claimed_solver: checked_program_path(ExecutableRole::Solver, claimed_solver.into())?,
            recorder: checked_program_path(ExecutableRole::EvidenceRecorder, recorder.into())?,
        })
    }

    pub fn claimed_verifier(&self) -> &str {
        &self.claimed_verifier
    }

    pub fn claimed_solver(&self) -> &str {
        &self.claimed_solver
    }

    pub fn recorder(&self) -> &str {
        &self.recorder
    }

    #[deprecated(note = "use claimed_verifier(); this path is not executed by this API")]
    pub fn verus(&self) -> &str {
        self.claimed_verifier()
    }

    #[deprecated(note = "use claimed_solver(); this path is not executed by this API")]
    pub fn solver(&self) -> &str {
        self.claimed_solver()
    }

    #[deprecated(note = "use recorder(); only the recorder is executed by this API")]
    pub fn evidence_recorder(&self) -> &str {
        self.recorder()
    }
}

/// Compatibility name for [`MeasuredRecorderInputsV1`].
///
/// The claimed verifier and solver paths are not executed by this API.
#[deprecated(
    note = "use MeasuredRecorderInputsV1; verifier and solver images are not executed here"
)]
pub type AuthenticatedExecutionProgramsV1 = MeasuredRecorderInputsV1;

/// Identity and size of bytes copied into an immutable executable snapshot.
///
/// A measurement says nothing about whether the snapshot was executed. Check
/// [`ExecutableRole`] and the producing API's execution semantics separately.
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

/// Authenticated output from one measured recorder execution.
///
/// The type has no public constructor. Its transcript commits to the immutable
/// recorder snapshot, claimed verifier and solver snapshots, caller-selected
/// policy, request, fresh challenge, recorder stdout/stderr, and strict result
/// envelope. Only the recorder is launched. This type does not establish that
/// the claimed verifier or solver ran, and `ProofOutcome::Proved` means only
/// that the recorder reported that outcome. It grants no proof, module-load,
/// or kernel-launch authority.
///
/// ```compile_fail
/// # fn cannot_launch(evidence: fe2o3_verifier::AuthenticatedRecorderOutputV1) {
/// evidence.launch();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedRecorderOutputV1 {
    invocation_plan: InvocationPlan,
    challenge: Digest,
    canonical_invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    claimed_verifier: ExecutableMeasurementV1,
    claimed_solver: ExecutableMeasurementV1,
    recorder: ExecutableMeasurementV1,
    stdout: BoundExecutionPayloadV1,
    stderr: BoundExecutionPayloadV1,
    result_bytes: BoundExecutionPayloadV1,
    result: ProofResultV1,
    transcript_digest: Digest,
}

impl AuthenticatedRecorderOutputV1 {
    pub const fn invocation_plan(&self) -> &InvocationPlan {
        &self.invocation_plan
    }

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

    pub const fn claimed_verifier(&self) -> &ExecutableMeasurementV1 {
        &self.claimed_verifier
    }

    pub const fn claimed_solver(&self) -> &ExecutableMeasurementV1 {
        &self.claimed_solver
    }

    pub const fn recorder(&self) -> &ExecutableMeasurementV1 {
        &self.recorder
    }

    #[deprecated(note = "use claimed_verifier(); this image was not executed by this API")]
    pub const fn verus(&self) -> &ExecutableMeasurementV1 {
        self.claimed_verifier()
    }

    #[deprecated(note = "use claimed_solver(); this image was not executed by this API")]
    pub const fn solver(&self) -> &ExecutableMeasurementV1 {
        self.claimed_solver()
    }

    #[deprecated(note = "use recorder(); this is the image executed by this API")]
    pub const fn evidence_recorder(&self) -> &ExecutableMeasurementV1 {
        self.recorder()
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

    pub const fn recorder_report(&self) -> &ProofResultV1 {
        &self.result
    }

    #[deprecated(note = "use recorder_report(); the result is a recorder claim")]
    pub const fn result(&self) -> &ProofResultV1 {
        self.recorder_report()
    }

    pub const fn authenticates_claimed_verifier_execution(&self) -> bool {
        false
    }

    pub const fn authenticates_claimed_solver_execution(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn transcript_digest(&self) -> Digest {
        self.transcript_digest
    }

    /// Canonical descriptive transcript. Parsing these bytes cannot recreate
    /// this authenticated type; only the measured recorder path can construct
    /// it. The transcript does not show that the claimed verifier or solver ran.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        canonical_transcript_bytes(
            self.challenge,
            self.canonical_invocation_digest,
            self.policy_digest,
            self.request_digest,
            [&self.claimed_verifier, &self.claimed_solver, &self.recorder],
            [&self.stdout, &self.stderr, &self.result_bytes],
        )
    }

    pub(crate) fn revalidate_recorder_report(
        &self,
    ) -> Result<ProofResultV1, AuthenticatedExecutionError> {
        parse_authenticated_recorder_report(
            self.result_bytes.bytes(),
            &self.invocation_plan,
            AuthenticatedResultBindings {
                challenge: self.challenge,
                invocation_digest: self.canonical_invocation_digest,
                policy_digest: self.policy_digest,
                request_digest: self.request_digest,
                verus_digest: self.claimed_verifier.identity.executable_digest(),
                solver_digest: self.claimed_solver.identity.executable_digest(),
                recorder_digest: self.recorder.identity.executable_digest(),
            },
        )
    }
}

/// Compatibility name for [`AuthenticatedRecorderOutputV1`].
///
/// Despite the legacy name, the value authenticates only recorder execution.
#[deprecated(note = "use AuthenticatedRecorderOutputV1; this API executes only the recorder")]
pub type AuthenticatedVerusExecutionEvidenceV1 = AuthenticatedRecorderOutputV1;

/// Measures three images and executes only the evidence recorder.
///
/// Each image is copied into a sealed anonymous file while SHA-256 is computed
/// and compared with the caller-selected policy. The recorder snapshot is
/// launched with the sealed claimed-verifier and claimed-solver paths as
/// arguments. This process does not observe the recorder launching either
/// image and therefore does not authenticate that Verus, a solver, or a proof
/// toolchain ran. The returned result is an authenticated recorder report.
pub fn execute_authenticated_recorder(
    request: ProofRequestV1,
    inputs: MeasuredRecorderInputsV1,
    timeout_seconds: u32,
    policy: &VerifierPolicy,
    limits: ExecutionLimits,
) -> Result<AuthenticatedRecorderOutputV1, AuthenticatedExecutionError> {
    let challenge = random_challenge()?;
    execute_authenticated_recorder_with_challenge(
        request,
        inputs,
        timeout_seconds,
        policy,
        limits,
        challenge,
    )
}

/// Compatibility wrapper for [`execute_authenticated_recorder`].
///
/// Despite the legacy name, this function launches only the recorder and does
/// not authenticate that Verus or a solver ran.
#[deprecated(note = "use execute_authenticated_recorder; this function executes only the recorder")]
pub fn execute_authenticated_verus(
    request: ProofRequestV1,
    inputs: MeasuredRecorderInputsV1,
    timeout_seconds: u32,
    policy: &VerifierPolicy,
    limits: ExecutionLimits,
) -> Result<AuthenticatedRecorderOutputV1, AuthenticatedExecutionError> {
    execute_authenticated_recorder(request, inputs, timeout_seconds, policy, limits)
}

fn execute_authenticated_recorder_with_challenge(
    request: ProofRequestV1,
    inputs: MeasuredRecorderInputsV1,
    timeout_seconds: u32,
    policy: &VerifierPolicy,
    limits: ExecutionLimits,
    challenge: Digest,
) -> Result<AuthenticatedRecorderOutputV1, AuthenticatedExecutionError> {
    if challenge.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(AuthenticatedExecutionError::InvalidChallenge);
    }

    let claimed_verifier = SealedExecutable::measure(
        ExecutableRole::Verus,
        inputs.claimed_verifier(),
        policy.expected_tools().verifier(),
    )?;
    let claimed_solver = SealedExecutable::measure(
        ExecutableRole::Solver,
        inputs.claimed_solver(),
        policy.expected_tools().solver(),
    )?;
    let recorder = SealedExecutable::measure(
        ExecutableRole::EvidenceRecorder,
        inputs.recorder(),
        policy.expected_tools().evidence_recorder(),
    )?;

    let plan = build_invocation_plan(
        request,
        policy.expected_tools().clone(),
        InvocationPaths::new(
            inputs.claimed_verifier,
            inputs.claimed_solver,
            inputs.recorder,
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
        verus_digest: claimed_verifier.measurement.identity.executable_digest(),
        solver_digest: claimed_solver.measurement.identity.executable_digest(),
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
            &claimed_verifier.proc_path(),
            "--solver",
            &claimed_solver.proc_path(),
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

    let child = spawn_artifact_coordinated_child(&mut command).map_err(|error| {
        AuthenticatedExecutionError::Execution(ExecutionError::from_spawn(error.kind()))
    })?;
    let output = supervise_child(child, timeout_seconds, limits)
        .map_err(AuthenticatedExecutionError::Execution)?;
    let result_bytes = result_file.seal_and_read(MAX_RESULT_BYTES)?;
    let result = parse_authenticated_recorder_report(&result_bytes, &plan, bindings)?;

    Ok(build_evidence(EvidenceParts {
        invocation_plan: plan,
        challenge,
        canonical_invocation_digest: invocation_digest,
        policy_digest,
        request_digest,
        claimed_verifier: claimed_verifier.measurement,
        claimed_solver: claimed_solver.measurement,
        recorder: recorder.measurement,
        output,
        result_bytes,
        result,
    }))
}

struct EvidenceParts {
    invocation_plan: InvocationPlan,
    challenge: Digest,
    canonical_invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    claimed_verifier: ExecutableMeasurementV1,
    claimed_solver: ExecutableMeasurementV1,
    recorder: ExecutableMeasurementV1,
    output: ProcessOutput,
    result_bytes: Vec<u8>,
    result: ProofResultV1,
}

fn build_evidence(parts: EvidenceParts) -> AuthenticatedRecorderOutputV1 {
    let stdout = BoundExecutionPayloadV1::new(parts.output.stdout().to_vec());
    let stderr = BoundExecutionPayloadV1::new(parts.output.stderr().to_vec());
    let result_bytes = BoundExecutionPayloadV1::new(parts.result_bytes);
    let transcript = canonical_transcript_bytes(
        parts.challenge,
        parts.canonical_invocation_digest,
        parts.policy_digest,
        parts.request_digest,
        [
            &parts.claimed_verifier,
            &parts.claimed_solver,
            &parts.recorder,
        ],
        [&stdout, &stderr, &result_bytes],
    );
    AuthenticatedRecorderOutputV1 {
        invocation_plan: parts.invocation_plan,
        challenge: parts.challenge,
        canonical_invocation_digest: parts.canonical_invocation_digest,
        policy_digest: parts.policy_digest,
        request_digest: parts.request_digest,
        claimed_verifier: parts.claimed_verifier,
        claimed_solver: parts.claimed_solver,
        recorder: parts.recorder,
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

fn parse_authenticated_recorder_report(
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
        "authenticated recorder execution requires Linux memfd sealing",
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
        "authenticated recorder execution requires Linux memfd sealing",
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
    /// Legacy wire field containing the claimed verifier image digest.
    Verus,
    /// Legacy wire field containing the claimed solver image digest.
    Solver,
    /// Wire field containing the recorder image digest that was executed.
    EvidenceRecorder,
}

/// Failure while parsing or binding an authenticated recorder result envelope.
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

/// Failure while measuring inputs or executing the sealed recorder snapshot.
///
/// Verifier and solver roles in these errors concern measurements only; this
/// execution path does not launch either image.
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
        write!(
            formatter,
            "authenticated recorder execution failed: {self:?}"
        )
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
        assert!(parse_authenticated_recorder_report(&valid, &plan, expected).is_ok());

        let mut malformed = valid.clone();
        malformed[0] = b'X';
        assert_eq!(
            parse_authenticated_recorder_report(&malformed, &plan, expected),
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
                parse_authenticated_recorder_report(&substituted, &plan, expected),
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
            parse_authenticated_recorder_report(uppercase.as_bytes(), &plan, expected),
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
            parse_authenticated_recorder_report(&replay, &plan, replay_target),
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
            parse_authenticated_recorder_report(leading_zero.as_bytes(), &plan, expected),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::MalformedLength
            ))
        );
        let reordered = valid.replacen("challenge=", "policy=", 1);
        assert_eq!(
            parse_authenticated_recorder_report(reordered.as_bytes(), &plan, expected),
            Err(AuthenticatedExecutionError::Result(
                AuthenticatedResultError::UnexpectedField {
                    expected: "challenge"
                }
            ))
        );
    }

    #[test]
    fn every_authenticated_result_truncation_and_trailing_byte_is_rejected() {
        let plan = plan();
        let expected = bindings(70);
        let valid = auth_envelope(expected, &proof_payload());
        assert!(parse_authenticated_recorder_report(&valid, &plan, expected).is_ok());

        for prefix_len in 0..valid.len() {
            assert!(
                parse_authenticated_recorder_report(&valid[..prefix_len], &plan, expected).is_err(),
                "truncated prefix of {prefix_len} bytes was accepted"
            );
        }

        for trailing in [0_u8, b'\n', b'x', 0xff] {
            let mut changed = valid.clone();
            changed.push(trailing);
            assert!(
                parse_authenticated_recorder_report(&changed, &plan, expected).is_err(),
                "trailing byte {trailing:#04x} was accepted"
            );
        }
    }
}
