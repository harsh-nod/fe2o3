use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_verifier::{
    AxiomPolicy, Configuration, ConfigurationEntry, CorrelationId, Digest, ExecutionErrorKind,
    ExecutionLimits, ExecutionPath, ExecutionStage, ExecutionTools, InvocationPaths,
    MAX_CAPTURE_BYTES, MAX_RESULT_BYTES, MeasuredToolIdentity, OutputStream, ProofOutcome,
    ProofProperty, ProofRequestV1, ProofTargetIdentity, ResultError, VerificationModelIdentity,
    VerifierPolicy, build_invocation_plan, execute_recorder,
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn target() -> ProofTargetIdentity {
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
    }
}

fn configuration() -> Configuration {
    Configuration::new(vec![ConfigurationEntry::new("solver", "z3").unwrap()]).unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new("gpu-model-v1", digest(20)).unwrap()
}

fn tool(name: &str, seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, "1.0.0", digest(seed), digest(seed + 1)).unwrap()
}

fn tools() -> ExecutionTools {
    ExecutionTools::new(
        tool("verus", 30),
        tool("z3", 32),
        tool("fe2o3-recorder", 34),
    )
}

fn plan(temp: &TempDirectory, mode: &str, timeout_seconds: u32) -> fe2o3_verifier::InvocationPlan {
    plan_with_program(
        temp,
        mode,
        timeout_seconds,
        env!("CARGO_BIN_EXE_fe2o3-verifier-test-recorder"),
    )
}

fn plan_with_program(
    temp: &TempDirectory,
    mode: &str,
    timeout_seconds: u32,
    recorder: &str,
) -> fe2o3_verifier::InvocationPlan {
    let request = ProofRequestV1::new(
        CorrelationId::from_bytes([50; 16]),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
        vec![],
    )
    .unwrap();
    let tools = tools();
    let policy = VerifierPolicy::new(
        tools.clone(),
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        10,
    )
    .unwrap();
    build_invocation_plan(
        request,
        tools,
        InvocationPaths::new(
            format!("/fixture/mode/{mode}"),
            "/fixture/solver",
            recorder,
            temp.path()
                .join("request with spaces;literal.bin")
                .display()
                .to_string(),
            temp.path()
                .join("result with spaces;literal.txt")
                .display()
                .to_string(),
        )
        .unwrap(),
        timeout_seconds,
        &policy,
    )
    .unwrap()
}

#[test]
fn successful_execution_is_hermetic_and_strictly_parsed() {
    let temp = TempDirectory::new();
    let plan = plan(&temp, "success", 2);
    let success = execute_recorder(&plan, ExecutionLimits::default()).unwrap();
    assert_eq!(success.result().outcome(), ProofOutcome::Proved);
    assert_eq!(success.result().target(), target());
    assert_eq!(success.result().tools(), &tools());
    assert_eq!(
        success.result().recorder_reported_properties(),
        &[ProofProperty::Bounds, ProofProperty::RaceFreedom]
    );
    assert!(success.output().stdout().is_empty());
    assert!(success.output().stderr().is_empty());
    assert!(!Path::new(plan.request_file()).exists());
}

#[test]
fn failed_envelope_is_evidence_without_proof_claims() {
    let temp = TempDirectory::new();
    let success = execute_recorder(&plan(&temp, "failed", 2), ExecutionLimits::default()).unwrap();
    assert_eq!(success.result().outcome(), ProofOutcome::Failed);
    assert!(success.result().recorder_reported_properties().is_empty());
}

#[test]
fn exit_signal_timeout_and_spawn_failures_are_distinct() {
    let temp = TempDirectory::new();
    let error = execute_recorder(&plan(&temp, "exit", 2), ExecutionLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &ExecutionErrorKind::Exited(17));
    assert_eq!(error.output().stderr(), b"bounded failure");

    let temp = TempDirectory::new();
    let error =
        execute_recorder(&plan(&temp, "signal", 2), ExecutionLimits::default()).unwrap_err();
    assert!(matches!(error.kind(), ExecutionErrorKind::Signaled(signal) if *signal > 0));

    let temp = TempDirectory::new();
    let error =
        execute_recorder(&plan(&temp, "timeout", 1), ExecutionLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &ExecutionErrorKind::TimedOut);

    let temp = TempDirectory::new();
    let missing = temp.path().join("missing-recorder");
    let error = execute_recorder(
        &plan_with_program(&temp, "success", 2, &missing.display().to_string()),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &ExecutionErrorKind::SpawnFailed(io::ErrorKind::NotFound)
    );
}

#[test]
fn inherited_descendant_pipes_cannot_extend_the_deadline() {
    let temp = TempDirectory::new();
    let started = std::time::Instant::now();
    let error = execute_recorder(
        &plan(&temp, "inherited-pipe", 1),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &ExecutionErrorKind::TimedOut);
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
}

#[test]
fn stdout_and_stderr_are_independently_bounded() {
    let limits = ExecutionLimits::new(1024, 2048).unwrap();
    for (mode, stream, max) in [
        ("stdout-oversize", OutputStream::Stdout, 1024),
        ("stderr-oversize", OutputStream::Stderr, 2048),
    ] {
        let temp = TempDirectory::new();
        let error = execute_recorder(&plan(&temp, mode, 5), limits).unwrap_err();
        assert_eq!(
            error.kind(),
            &ExecutionErrorKind::OutputTooLarge { stream, max }
        );
        assert!(error.output().stdout().len() <= limits.max_stdout_bytes());
        assert!(error.output().stderr().len() <= limits.max_stderr_bytes());
    }

    assert!(matches!(
        ExecutionLimits::new(0, 1).unwrap_err().kind(),
        ExecutionErrorKind::CaptureLimitOutOfRange {
            stream: OutputStream::Stdout,
            max: MAX_CAPTURE_BYTES,
        }
    ));
}

#[test]
fn result_bytes_are_bounded_and_must_be_utf8() {
    let temp = TempDirectory::new();
    let error = execute_recorder(
        &plan(&temp, "nonutf8-result", 2),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &ExecutionErrorKind::InvalidEnvelope(ResultError::InvalidUtf8)
    );

    let temp = TempDirectory::new();
    let error = execute_recorder(
        &plan(&temp, "result-oversize", 2),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &ExecutionErrorKind::InvalidEnvelope(ResultError::TooLarge {
            max: MAX_RESULT_BYTES,
        })
    );

    let temp = TempDirectory::new();
    let error = execute_recorder(
        &plan(&temp, "result-directory", 2),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &ExecutionErrorKind::ResultNotRegularFile);
}

#[test]
fn raw_stdout_is_never_interpreted_as_a_proof() {
    let temp = TempDirectory::new();
    let error = execute_recorder(
        &plan(&temp, "stdout-envelope", 2),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &ExecutionErrorKind::Io {
            stage: ExecutionStage::ReadResult,
            kind: io::ErrorKind::NotFound,
        }
    );
    assert!(
        error
            .output()
            .stdout()
            .starts_with(b"FE2O3-VERIFIER-RESULT-V1\n")
    );
}

#[test]
fn correlation_and_envelope_adversaries_fail_closed() {
    for (mode, expected) in [
        (
            "wrong-correlation",
            ExecutionErrorKind::InvalidEnvelope(ResultError::CorrelationMismatch),
        ),
        (
            "malformed",
            ExecutionErrorKind::InvalidEnvelope(ResultError::MalformedEnvelope),
        ),
    ] {
        let temp = TempDirectory::new();
        let error =
            execute_recorder(&plan(&temp, mode, 2), ExecutionLimits::default()).unwrap_err();
        assert_eq!(error.kind(), &expected);
    }

    let temp = TempDirectory::new();
    let success = execute_recorder(
        &plan(&temp, "nonutf8-stdout", 2),
        ExecutionLimits::default(),
    )
    .unwrap();
    assert_eq!(success.result().outcome(), ProofOutcome::Proved);
    assert_eq!(success.output().stdout(), &[0xff, 0xfe]);
}

#[test]
fn execution_rejects_ambiguous_paths_and_stale_results() {
    let temp = TempDirectory::new();
    let relative = plan_with_program(&temp, "success", 2, "relative-recorder");
    let error = execute_recorder(&relative, ExecutionLimits::default()).unwrap_err();
    assert_eq!(
        error.kind(),
        &ExecutionErrorKind::PathNotAbsolute {
            field: ExecutionPath::RecorderProgram,
        }
    );

    let temp = TempDirectory::new();
    let plan = plan(&temp, "success", 2);
    fs::write(plan.result_file(), b"stale proved result").unwrap();
    let error = execute_recorder(&plan, ExecutionLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &ExecutionErrorKind::ResultPathAlreadyExists);
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-verifier-executor-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
