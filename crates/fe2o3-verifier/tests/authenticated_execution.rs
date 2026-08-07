use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_verifier::{
    AuthenticatedExecutionError, AuthenticatedExecutionProgramsV1, AxiomPolicy, Configuration,
    ConfigurationEntry, CorrelationId, Digest, ExecutableRole, ExecutionLimits, ExecutionTools,
    MeasuredToolIdentity, ProofOutcome, ProofProperty, ProofRequestV1, ProofTargetIdentity,
    VerificationModelIdentity, VerifierPolicy, execute_authenticated_verus,
};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
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

fn request() -> ProofRequestV1 {
    ProofRequestV1::new(
        CorrelationId::from_bytes([50; 16]),
        target(),
        configuration(),
        model(),
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
        vec![],
    )
    .unwrap()
}

fn tool(name: &str, executable_digest: Digest, configuration_seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(
        name,
        "test-v1",
        executable_digest,
        digest(configuration_seed),
    )
    .unwrap()
}

fn policy(executable_digest: Digest) -> VerifierPolicy {
    VerifierPolicy::new(
        ExecutionTools::new(
            tool("verus", executable_digest, 30),
            tool("z3", executable_digest, 31),
            tool("fe2o3-recorder", executable_digest, 32),
        ),
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        10,
    )
    .unwrap()
}

fn fixture() -> &'static str {
    env!("CARGO_BIN_EXE_fe2o3-verifier-test-recorder")
}

fn programs(verus: &str, solver: &str, recorder: &str) -> AuthenticatedExecutionProgramsV1 {
    AuthenticatedExecutionProgramsV1::new(verus, solver, recorder).unwrap()
}

#[test]
fn measured_execution_binds_exact_tools_policy_input_and_all_outputs() {
    let executable_bytes = fs::read(fixture()).unwrap();
    let executable_digest = sha256(&executable_bytes);
    let request = request();
    let request_digest = sha256(&request.to_canonical_bytes());
    let policy = policy(executable_digest);
    let policy_digest = sha256(&policy.to_canonical_bytes());

    let evidence = execute_authenticated_verus(
        request,
        programs(fixture(), fixture(), fixture()),
        2,
        &policy,
        ExecutionLimits::default(),
    )
    .unwrap();

    assert_eq!(evidence.request_digest(), request_digest);
    assert_eq!(evidence.policy_digest(), policy_digest);
    assert!(
        evidence
            .challenge()
            .as_bytes()
            .iter()
            .any(|byte| *byte != 0)
    );
    for (measurement, role) in [
        (evidence.verus(), ExecutableRole::Verus),
        (evidence.solver(), ExecutableRole::Solver),
        (
            evidence.evidence_recorder(),
            ExecutableRole::EvidenceRecorder,
        ),
    ] {
        assert_eq!(measurement.role(), role);
        assert_eq!(
            measurement.identity().executable_digest(),
            executable_digest
        );
        assert_eq!(measurement.byte_len(), executable_bytes.len() as u64);
    }
    assert_eq!(evidence.stdout().bytes(), b"authenticated stdout");
    assert_eq!(evidence.stderr().bytes(), b"authenticated stderr");
    assert_eq!(evidence.stdout().digest(), sha256(b"authenticated stdout"));
    assert_eq!(evidence.stderr().digest(), sha256(b"authenticated stderr"));
    assert_eq!(
        evidence.result_bytes().digest(),
        sha256(evidence.result_bytes().bytes())
    );
    assert_eq!(evidence.result().outcome(), ProofOutcome::Proved);
    assert_eq!(
        evidence.result().proved_properties(),
        &[ProofProperty::Bounds, ProofProperty::RaceFreedom]
    );
    assert_eq!(
        evidence.transcript_digest(),
        sha256(&evidence.to_canonical_bytes())
    );
}

#[test]
fn mutated_or_substituted_executable_never_reaches_the_recorder() {
    let fixture_bytes = fs::read(fixture()).unwrap();
    let expected = sha256(&fixture_bytes);
    let temp = TempDirectory::new();
    let changed = temp.path().join("changed-verus");
    let mut changed_bytes = fixture_bytes;
    changed_bytes[0] ^= 1;
    fs::write(&changed, changed_bytes).unwrap();

    let error = execute_authenticated_verus(
        request(),
        programs(changed.to_str().unwrap(), fixture(), fixture()),
        2,
        &policy(expected),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AuthenticatedExecutionError::ExecutableDigestMismatch {
            role: ExecutableRole::Verus,
            expected: policy_digest,
            measured: _
        } if policy_digest == expected
    ));

    let error = execute_authenticated_verus(
        request(),
        programs(fixture(), "/bin/true", fixture()),
        2,
        &policy(expected),
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AuthenticatedExecutionError::ExecutableDigestMismatch {
            role: ExecutableRole::Solver,
            ..
        }
    ));
}

#[test]
fn unauthenticated_caller_digest_claim_is_rejected_before_execution() {
    let forged = policy(digest(99));
    let error = execute_authenticated_verus(
        request(),
        programs(fixture(), fixture(), fixture()),
        2,
        &forged,
        ExecutionLimits::default(),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        AuthenticatedExecutionError::ExecutableDigestMismatch {
            role: ExecutableRole::Verus,
            expected,
            ..
        } if expected == digest(99)
    ));
}

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-verus-authentication-{}-{sequence}",
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
