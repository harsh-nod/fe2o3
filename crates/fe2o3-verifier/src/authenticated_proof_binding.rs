use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifacts::{
    DigestAlgorithm, ExecutableCodeObjectVersionV1, ManifestV1,
    MeasuredToolIdentity as ArtifactMeasuredToolIdentity, PayloadDigest,
    ProofExecutableBindingError, ProofExecutableBindingV1, ProofMatchError, ProofMatchPolicy,
};

use crate::{
    ArtifactRecordConversionError, AuthenticatedExecutionError,
    AuthenticatedVerusExecutionEvidenceV1, BoundExecutionPayloadV1, Digest,
    ExecutableMeasurementV1, ExecutableRole, PersistentFreshnessIdentityV1,
    PersistentFreshnessLedgerErrorV1, PersistentFreshnessReceiptV1,
    PersistentProofFreshnessLedgerV1, ReviewedInvocationIdentityV1, VerifierPolicy,
    canonical_invocation_digest, convert_to_artifact_proof_record,
};

/// Domain and schema version for measured proof-to-executable bridge identities.
pub const AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2APXB\0";
pub const PERSISTENT_AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2PPXB\0";
pub const AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1: u16 = 1;

/// Independent policies and finalized artifact identities admitted by the bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedProofExecutablePolicyV1 {
    verifier_policy: VerifierPolicy,
    proof_match_policy: ProofMatchPolicy,
    manifest: ManifestV1,
    finalized_code_object_digest: PayloadDigest,
    code_object_version: ExecutableCodeObjectVersionV1,
    compiler: ArtifactMeasuredToolIdentity,
    artifact_producer: ArtifactMeasuredToolIdentity,
    binding_digest_algorithm: DigestAlgorithm,
}

impl AuthenticatedProofExecutablePolicyV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        verifier_policy: VerifierPolicy,
        proof_match_policy: ProofMatchPolicy,
        manifest: ManifestV1,
        finalized_code_object_digest: PayloadDigest,
        code_object_version: ExecutableCodeObjectVersionV1,
        compiler: ArtifactMeasuredToolIdentity,
        artifact_producer: ArtifactMeasuredToolIdentity,
        binding_digest_algorithm: DigestAlgorithm,
    ) -> Self {
        Self {
            verifier_policy,
            proof_match_policy,
            manifest,
            finalized_code_object_digest,
            code_object_version,
            compiler,
            artifact_producer,
            binding_digest_algorithm,
        }
    }

    pub const fn verifier_policy(&self) -> &VerifierPolicy {
        &self.verifier_policy
    }

    pub const fn proof_match_policy(&self) -> &ProofMatchPolicy {
        &self.proof_match_policy
    }

    pub const fn manifest(&self) -> &ManifestV1 {
        &self.manifest
    }

    pub const fn finalized_code_object_digest(&self) -> PayloadDigest {
        self.finalized_code_object_digest
    }

    pub const fn code_object_version(&self) -> ExecutableCodeObjectVersionV1 {
        self.code_object_version
    }

    pub const fn compiler(&self) -> &ArtifactMeasuredToolIdentity {
        &self.compiler
    }

    pub const fn artifact_producer(&self) -> &ArtifactMeasuredToolIdentity {
        &self.artifact_producer
    }

    pub const fn binding_digest_algorithm(&self) -> DigestAlgorithm {
        self.binding_digest_algorithm
    }
}

/// One process-local replay ledger for authenticated proof executions.
///
/// The ledger is deliberately neither `Clone` nor serializable. A successful
/// bridge consumes both the challenge and transcript identity. Production
/// callers that require replay protection across restart should use
/// `PersistentProofFreshnessLedgerV1` and
/// `bind_authenticated_proof_executable_persistent_v1` instead.
///
/// ```compile_fail
/// let ledger = fe2o3_verifier::AuthenticatedExecutionFreshnessV1::new();
/// let _duplicate = ledger.clone();
/// ```
#[derive(Debug, Default)]
pub struct AuthenticatedExecutionFreshnessV1 {
    consumed_challenges: BTreeSet<Digest>,
    consumed_transcripts: BTreeSet<Digest>,
}

impl AuthenticatedExecutionFreshnessV1 {
    pub const fn new() -> Self {
        Self {
            consumed_challenges: BTreeSet::new(),
            consumed_transcripts: BTreeSet::new(),
        }
    }

    pub fn consumed_count(&self) -> usize {
        debug_assert_eq!(
            self.consumed_challenges.len(),
            self.consumed_transcripts.len()
        );
        self.consumed_challenges.len()
    }

    fn check(
        &self,
        challenge: Digest,
        transcript: Digest,
    ) -> Result<(), AuthenticatedProofExecutableBindingError> {
        if self.consumed_challenges.contains(&challenge) {
            return Err(AuthenticatedProofExecutableBindingError::ChallengeReplay);
        }
        if self.consumed_transcripts.contains(&transcript) {
            return Err(AuthenticatedProofExecutableBindingError::TranscriptReplay);
        }
        Ok(())
    }

    fn consume(&mut self, challenge: Digest, transcript: Digest) {
        let challenge_was_new = self.consumed_challenges.insert(challenge);
        let transcript_was_new = self.consumed_transcripts.insert(transcript);
        debug_assert!(challenge_was_new && transcript_was_new);
    }
}

/// Canonical digest and exact byte length of one retained execution payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedPayloadIdentityV1 {
    byte_len: u64,
    digest: Digest,
}

impl AuthenticatedPayloadIdentityV1 {
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn digest(self) -> Digest {
        self.digest
    }
}

/// Every measured and retained identity from the sealed Verus transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedProofExecutionIdentityV1 {
    challenge: Digest,
    canonical_invocation_digest: Digest,
    policy_digest: Digest,
    request_digest: Digest,
    verus: ExecutableMeasurementV1,
    solver: ExecutableMeasurementV1,
    evidence_recorder: ExecutableMeasurementV1,
    stdout: AuthenticatedPayloadIdentityV1,
    stderr: AuthenticatedPayloadIdentityV1,
    result: AuthenticatedPayloadIdentityV1,
    transcript_digest: Digest,
}

impl AuthenticatedProofExecutionIdentityV1 {
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

    pub const fn stdout(&self) -> AuthenticatedPayloadIdentityV1 {
        self.stdout
    }

    pub const fn stderr(&self) -> AuthenticatedPayloadIdentityV1 {
        self.stderr
    }

    pub const fn result(&self) -> AuthenticatedPayloadIdentityV1 {
        self.result
    }

    pub const fn transcript_digest(&self) -> Digest {
        self.transcript_digest
    }
}

/// Inert evidence joining one measured Verus transaction to one exact
/// `ProofExecutableBindingV1`.
///
/// Construction is private to the fail-closed bridge. The complete measured
/// evidence is retained so auditors can recover the exact request/result and
/// stdout/stderr transcript bytes. This type still grants no load or launch
/// authority.
///
/// ```compile_fail
/// # fn cannot_launch(binding: fe2o3_verifier::AuthenticatedProofExecutableBindingV1) {
/// binding.launch();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedProofExecutableBindingV1 {
    execution_evidence: AuthenticatedVerusExecutionEvidenceV1,
    execution_identity: AuthenticatedProofExecutionIdentityV1,
    executable_binding: ProofExecutableBindingV1,
    binding_identity: Digest,
}

impl AuthenticatedProofExecutableBindingV1 {
    pub const fn version(&self) -> u16 {
        AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1
    }

    pub const fn execution_evidence(&self) -> &AuthenticatedVerusExecutionEvidenceV1 {
        &self.execution_evidence
    }

    pub const fn execution_identity(&self) -> &AuthenticatedProofExecutionIdentityV1 {
        &self.execution_identity
    }

    pub const fn executable_binding(&self) -> &ProofExecutableBindingV1 {
        &self.executable_binding
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub fn validate_against(
        &self,
        actual: &Self,
    ) -> Result<(), AuthenticatedProofExecutableBindingError> {
        let expected = &self.execution_identity;
        let actual_execution = &actual.execution_identity;
        require_equal(expected.challenge, actual_execution.challenge, "challenge")?;
        require_equal(
            expected.canonical_invocation_digest,
            actual_execution.canonical_invocation_digest,
            "canonical invocation",
        )?;
        require_equal(
            expected.policy_digest,
            actual_execution.policy_digest,
            "verifier policy",
        )?;
        require_equal(
            expected.request_digest,
            actual_execution.request_digest,
            "sealed request",
        )?;
        require_equal(
            &expected.verus,
            &actual_execution.verus,
            "Verus measurement",
        )?;
        require_equal(
            &expected.solver,
            &actual_execution.solver,
            "solver measurement",
        )?;
        require_equal(
            &expected.evidence_recorder,
            &actual_execution.evidence_recorder,
            "recorder measurement",
        )?;
        require_equal(
            expected.stdout,
            actual_execution.stdout,
            "stdout transcript",
        )?;
        require_equal(
            expected.stderr,
            actual_execution.stderr,
            "stderr transcript",
        )?;
        require_equal(
            expected.result,
            actual_execution.result,
            "result transcript",
        )?;
        require_equal(
            expected.transcript_digest,
            actual_execution.transcript_digest,
            "execution transcript",
        )?;
        self.executable_binding
            .validate_against(&actual.executable_binding)
            .map_err(AuthenticatedProofExecutableBindingError::ExecutableBinding)?;
        require_equal(
            self.binding_identity,
            actual.binding_identity,
            "authenticated binding identity",
        )
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Canonical identity of a proof/executable binding and its durable freshness
/// receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PersistentlyFreshProofExecutableIdentityV1 {
    proof_binding_identity: Digest,
    consumed_execution: PersistentFreshnessIdentityV1,
    ledger_namespace: Digest,
    ledger_generation: u64,
    ledger_state_identity: Digest,
    binding_identity: Digest,
}

impl PersistentlyFreshProofExecutableIdentityV1 {
    pub const fn proof_binding_identity(self) -> Digest {
        self.proof_binding_identity
    }

    pub const fn consumed_execution(self) -> PersistentFreshnessIdentityV1 {
        self.consumed_execution
    }

    pub const fn ledger_namespace(self) -> Digest {
        self.ledger_namespace
    }

    pub const fn ledger_generation(self) -> u64 {
        self.ledger_generation
    }

    pub const fn ledger_state_identity(self) -> Digest {
        self.ledger_state_identity
    }

    pub const fn binding_identity(self) -> Digest {
        self.binding_identity
    }
}

/// Non-clone evidence that an exact authenticated proof/executable binding was
/// durably consumed by one named freshness ledger state.
///
/// Construction is private to the persistent bridge. The receipt and canonical
/// identity remain attached so downstream APIs can require persistent
/// freshness at the type boundary. This value grants no runtime authority.
///
/// ```compile_fail
/// # fn duplicate(value: fe2o3_verifier::PersistentlyFreshProofExecutableBindingV1) {
/// let _copy = value.clone();
/// # }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct PersistentlyFreshProofExecutableBindingV1 {
    proof_binding: AuthenticatedProofExecutableBindingV1,
    freshness_receipt: PersistentFreshnessReceiptV1,
    identity: PersistentlyFreshProofExecutableIdentityV1,
}

impl PersistentlyFreshProofExecutableBindingV1 {
    pub const fn version(&self) -> u16 {
        AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1
    }

    pub const fn proof_binding(&self) -> &AuthenticatedProofExecutableBindingV1 {
        &self.proof_binding
    }

    pub const fn freshness_receipt(&self) -> PersistentFreshnessReceiptV1 {
        self.freshness_receipt
    }

    pub const fn identity(&self) -> PersistentlyFreshProofExecutableIdentityV1 {
        self.identity
    }

    pub const fn binding_identity(&self) -> Digest {
        self.identity.binding_identity
    }

    pub fn validate_against(
        &self,
        actual: &Self,
    ) -> Result<(), AuthenticatedProofExecutableBindingError> {
        self.proof_binding.validate_against(&actual.proof_binding)?;
        require_equal(
            self.freshness_receipt,
            actual.freshness_receipt,
            "persistent freshness receipt",
        )?;
        require_equal(
            self.identity,
            actual.identity,
            "persistent authenticated binding identity",
        )
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Authenticates, policy-matches, and binds one exact measured proof execution.
///
/// The evidence is consumed, all retained bytes and identities are recomputed,
/// and freshness is consumed only after the complete executable binding has
/// succeeded. Neither this function nor its output grants runtime authority.
pub fn bind_authenticated_proof_executable_v1(
    evidence: AuthenticatedVerusExecutionEvidenceV1,
    policy: &AuthenticatedProofExecutablePolicyV1,
    freshness: &mut AuthenticatedExecutionFreshnessV1,
) -> Result<AuthenticatedProofExecutableBindingV1, AuthenticatedProofExecutableBindingError> {
    validate_authenticated_binding_input(&evidence, policy)?;
    freshness.check(evidence.challenge(), evidence.transcript_digest())?;

    let binding = finish_authenticated_proof_executable_binding(evidence, policy)?;
    freshness.consume(
        binding.execution_identity.challenge,
        binding.execution_identity.transcript_digest,
    );
    Ok(binding)
}

/// Authenticates and binds one proof only after durably consuming its exact
/// challenge, transcript, and sealed-result identities.
///
/// All proof, policy, result, and executable checks complete before the ledger
/// transaction begins. The durable transaction completes before this function
/// returns the inert binding. An I/O failure after intent publication may
/// conservatively consume the evidence; recovery will never make it replayable.
pub fn bind_authenticated_proof_executable_persistent_v1(
    evidence: AuthenticatedVerusExecutionEvidenceV1,
    policy: &AuthenticatedProofExecutablePolicyV1,
    freshness: &mut PersistentProofFreshnessLedgerV1,
) -> Result<PersistentlyFreshProofExecutableBindingV1, AuthenticatedProofExecutableBindingError> {
    validate_authenticated_binding_input(&evidence, policy)?;
    let binding = finish_authenticated_proof_executable_binding(evidence, policy)?;
    let receipt = freshness.consume_authenticated_execution(binding.execution_identity())?;
    Ok(persistently_fresh_binding(binding, receipt))
}

fn persistently_fresh_binding(
    proof_binding: AuthenticatedProofExecutableBindingV1,
    freshness_receipt: PersistentFreshnessReceiptV1,
) -> PersistentlyFreshProofExecutableBindingV1 {
    let execution = &proof_binding.execution_identity;
    let consumed_execution = freshness_receipt.identity();
    debug_assert_eq!(consumed_execution.challenge(), execution.challenge());
    debug_assert_eq!(
        consumed_execution.transcript(),
        execution.transcript_digest()
    );
    debug_assert_eq!(consumed_execution.result(), execution.result().digest());
    let binding_identity = persistent_binding_identity(&proof_binding, freshness_receipt);
    let identity = PersistentlyFreshProofExecutableIdentityV1 {
        proof_binding_identity: proof_binding.binding_identity,
        consumed_execution,
        ledger_namespace: freshness_receipt.namespace(),
        ledger_generation: freshness_receipt.generation(),
        ledger_state_identity: freshness_receipt.state_identity(),
        binding_identity,
    };
    PersistentlyFreshProofExecutableBindingV1 {
        proof_binding,
        freshness_receipt,
        identity,
    }
}

fn persistent_binding_identity(
    proof_binding: &AuthenticatedProofExecutableBindingV1,
    receipt: PersistentFreshnessReceiptV1,
) -> Digest {
    let consumed = receipt.identity();
    let mut bytes = Vec::with_capacity(8 + 4 + 32 * 6 + 8);
    bytes.extend_from_slice(&PERSISTENT_AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1);
    bytes.extend_from_slice(&AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(proof_binding.binding_identity.as_bytes());
    bytes.extend_from_slice(consumed.challenge().as_bytes());
    bytes.extend_from_slice(consumed.transcript().as_bytes());
    bytes.extend_from_slice(consumed.result().as_bytes());
    bytes.extend_from_slice(receipt.namespace().as_bytes());
    bytes.extend_from_slice(&receipt.generation().to_le_bytes());
    bytes.extend_from_slice(receipt.state_identity().as_bytes());
    sha256(&bytes)
}

fn validate_authenticated_binding_input(
    evidence: &AuthenticatedVerusExecutionEvidenceV1,
    policy: &AuthenticatedProofExecutablePolicyV1,
) -> Result<(), AuthenticatedProofExecutableBindingError> {
    if policy.binding_digest_algorithm != DigestAlgorithm::Sha256 {
        return Err(AuthenticatedProofExecutableBindingError::UnsupportedDigestAlgorithm);
    }
    validate_authenticated_execution(evidence, &policy.verifier_policy)
}

fn finish_authenticated_proof_executable_binding(
    evidence: AuthenticatedVerusExecutionEvidenceV1,
    policy: &AuthenticatedProofExecutablePolicyV1,
) -> Result<AuthenticatedProofExecutableBindingV1, AuthenticatedProofExecutableBindingError> {
    let plan = evidence.invocation_plan();
    let reviewed = ReviewedInvocationIdentityV1::new(
        plan.request().correlation_id(),
        evidence.canonical_invocation_digest(),
    );
    let artifact_evidence = convert_to_artifact_proof_record(plan, evidence.result(), reviewed)?;
    let matched = policy.proof_match_policy.match_record(
        artifact_evidence.record().clone(),
        policy.binding_digest_algorithm,
    )?;
    let executable_binding = matched.bind_finalized_executable_v1(
        &policy.manifest,
        policy.finalized_code_object_digest,
        policy.code_object_version,
        &policy.compiler,
        &policy.artifact_producer,
        policy.binding_digest_algorithm,
    )?;

    let execution_identity = execution_identity(&evidence);
    let binding_identity = sha256(&canonical_binding_bytes(
        &execution_identity,
        &executable_binding,
    ));
    Ok(AuthenticatedProofExecutableBindingV1 {
        execution_evidence: evidence,
        execution_identity,
        executable_binding,
        binding_identity,
    })
}

fn validate_authenticated_execution(
    evidence: &AuthenticatedVerusExecutionEvidenceV1,
    policy: &VerifierPolicy,
) -> Result<(), AuthenticatedProofExecutableBindingError> {
    if evidence
        .challenge()
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(AuthenticatedProofExecutableBindingError::InvalidChallenge);
    }
    require_digest(
        sha256(&evidence.to_canonical_bytes()),
        evidence.transcript_digest(),
        AuthenticatedProofExecutableBindingError::TranscriptDigestMismatch,
    )?;

    let plan = evidence.invocation_plan();
    require_digest(
        canonical_invocation_digest(plan),
        evidence.canonical_invocation_digest(),
        AuthenticatedProofExecutableBindingError::InvocationDigestMismatch,
    )?;
    require_digest(
        sha256(plan.request_bytes()),
        evidence.request_digest(),
        AuthenticatedProofExecutableBindingError::RequestDigestMismatch,
    )?;
    require_digest(
        sha256(&policy.to_canonical_bytes()),
        evidence.policy_digest(),
        AuthenticatedProofExecutableBindingError::PolicyDigestMismatch,
    )?;
    require_equal(
        plan.tools(),
        policy.expected_tools(),
        "invocation policy tools",
    )?;
    require_equal(
        plan.request().configuration(),
        policy.expected_configuration(),
        "proof configuration policy",
    )?;
    require_equal(
        plan.request().model(),
        policy.expected_model(),
        "verification model policy",
    )?;
    if plan.timeout_seconds() == 0 || plan.timeout_seconds() > policy.max_timeout_seconds() {
        return Err(AuthenticatedProofExecutableBindingError::IdentityMismatch {
            field: "timeout policy",
        });
    }
    if policy
        .axiom_policy()
        .validate(plan.request().trusted_items())
        .is_err()
    {
        return Err(AuthenticatedProofExecutableBindingError::IdentityMismatch {
            field: "trusted-item policy",
        });
    }

    for (measurement, role, expected) in [
        (
            evidence.verus(),
            ExecutableRole::Verus,
            policy.expected_tools().verifier(),
        ),
        (
            evidence.solver(),
            ExecutableRole::Solver,
            policy.expected_tools().solver(),
        ),
        (
            evidence.evidence_recorder(),
            ExecutableRole::EvidenceRecorder,
            policy.expected_tools().evidence_recorder(),
        ),
    ] {
        if measurement.role() != role {
            return Err(AuthenticatedProofExecutableBindingError::ExecutableRoleMismatch { role });
        }
        if measurement.identity() != expected {
            return Err(
                AuthenticatedProofExecutableBindingError::ExecutableIdentityMismatch { role },
            );
        }
        if measurement.byte_len() == 0 {
            return Err(AuthenticatedProofExecutableBindingError::EmptyExecutable { role });
        }
    }

    for (payload, field) in [
        (evidence.stdout(), "stdout"),
        (evidence.stderr(), "stderr"),
        (evidence.result_bytes(), "result"),
    ] {
        if sha256(payload.bytes()) != payload.digest() {
            return Err(AuthenticatedProofExecutableBindingError::PayloadDigestMismatch { field });
        }
    }

    let reparsed = evidence.revalidate_authenticated_result()?;
    if &reparsed != evidence.result() {
        return Err(AuthenticatedProofExecutableBindingError::ResultIdentityMismatch);
    }
    Ok(())
}

fn execution_identity(
    evidence: &AuthenticatedVerusExecutionEvidenceV1,
) -> AuthenticatedProofExecutionIdentityV1 {
    AuthenticatedProofExecutionIdentityV1 {
        challenge: evidence.challenge(),
        canonical_invocation_digest: evidence.canonical_invocation_digest(),
        policy_digest: evidence.policy_digest(),
        request_digest: evidence.request_digest(),
        verus: evidence.verus().clone(),
        solver: evidence.solver().clone(),
        evidence_recorder: evidence.evidence_recorder().clone(),
        stdout: payload_identity(evidence.stdout()),
        stderr: payload_identity(evidence.stderr()),
        result: payload_identity(evidence.result_bytes()),
        transcript_digest: evidence.transcript_digest(),
    }
}

fn payload_identity(payload: &BoundExecutionPayloadV1) -> AuthenticatedPayloadIdentityV1 {
    AuthenticatedPayloadIdentityV1 {
        byte_len: u64::try_from(payload.bytes().len())
            .expect("bounded execution payload length fits u64"),
        digest: payload.digest(),
    }
}

fn canonical_binding_bytes(
    execution: &AuthenticatedProofExecutionIdentityV1,
    executable: &ProofExecutableBindingV1,
) -> Vec<u8> {
    let mut writer = IdentityWriter::new();
    for digest in [
        execution.challenge,
        execution.canonical_invocation_digest,
        execution.policy_digest,
        execution.request_digest,
    ] {
        writer.digest(digest);
    }
    for measurement in [
        &execution.verus,
        &execution.solver,
        &execution.evidence_recorder,
    ] {
        writer.measurement(measurement);
    }
    for payload in [execution.stdout, execution.stderr, execution.result] {
        writer.u64(payload.byte_len);
        writer.digest(payload.digest);
    }
    writer.digest(execution.transcript_digest);
    writer.payload_digest(executable.proof_record_digest());
    writer.payload_digest(executable.binding_identity());
    writer.bytes
}

struct IdentityWriter {
    bytes: Vec<u8>,
}

impl IdentityWriter {
    fn new() -> Self {
        let mut bytes = Vec::with_capacity(768);
        bytes.extend_from_slice(&AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1);
        bytes.extend_from_slice(&AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        Self { bytes }
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn digest(&mut self, value: Digest) {
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        let algorithm = match value.algorithm() {
            DigestAlgorithm::Sha256 => 1,
            _ => 0,
        };
        self.bytes.push(algorithm);
        self.bytes.extend_from_slice(value.bytes().as_bytes());
    }

    fn measurement(&mut self, value: &ExecutableMeasurementV1) {
        self.bytes.push(match value.role() {
            ExecutableRole::Verus => 1,
            ExecutableRole::Solver => 2,
            ExecutableRole::EvidenceRecorder => 3,
        });
        let identity = value.identity();
        self.text(identity.name().as_str());
        self.text(identity.version().as_str());
        self.digest(identity.executable_digest());
        self.digest(identity.configuration_digest());
        self.u64(value.byte_len());
    }
}

fn require_digest(
    actual: Digest,
    expected: Digest,
    error: AuthenticatedProofExecutableBindingError,
) -> Result<(), AuthenticatedProofExecutableBindingError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error)
    }
}

fn require_equal<T: PartialEq>(
    expected: T,
    actual: T,
    field: &'static str,
) -> Result<(), AuthenticatedProofExecutableBindingError> {
    if expected == actual {
        Ok(())
    } else {
        Err(AuthenticatedProofExecutableBindingError::IdentityMismatch { field })
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthenticatedProofExecutableBindingError {
    UnsupportedDigestAlgorithm,
    InvalidChallenge,
    ChallengeReplay,
    TranscriptReplay,
    TranscriptDigestMismatch,
    InvocationDigestMismatch,
    PolicyDigestMismatch,
    RequestDigestMismatch,
    ExecutableRoleMismatch { role: ExecutableRole },
    ExecutableIdentityMismatch { role: ExecutableRole },
    EmptyExecutable { role: ExecutableRole },
    PayloadDigestMismatch { field: &'static str },
    ResultIdentityMismatch,
    IdentityMismatch { field: &'static str },
    AuthenticatedResult(AuthenticatedExecutionError),
    ArtifactRecord(ArtifactRecordConversionError),
    ProofMatch(ProofMatchError),
    ExecutableBinding(ProofExecutableBindingError),
    PersistentFreshness(PersistentFreshnessLedgerErrorV1),
}

impl fmt::Display for AuthenticatedProofExecutableBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedDigestAlgorithm => {
                formatter.write_str("authenticated proof binding requires SHA-256")
            }
            Self::InvalidChallenge => formatter.write_str("execution challenge is invalid"),
            Self::ChallengeReplay => {
                formatter.write_str("execution challenge was already consumed")
            }
            Self::TranscriptReplay => {
                formatter.write_str("execution transcript was already consumed")
            }
            Self::TranscriptDigestMismatch => {
                formatter.write_str("execution transcript digest does not match retained bytes")
            }
            Self::InvocationDigestMismatch => {
                formatter.write_str("canonical invocation digest does not match")
            }
            Self::PolicyDigestMismatch => {
                formatter.write_str("verifier policy digest does not match")
            }
            Self::RequestDigestMismatch => {
                formatter.write_str("sealed request digest does not match")
            }
            Self::ExecutableRoleMismatch { role } => {
                write!(
                    formatter,
                    "measured executable role {role:?} does not match"
                )
            }
            Self::ExecutableIdentityMismatch { role } => {
                write!(
                    formatter,
                    "measured executable identity {role:?} does not match policy"
                )
            }
            Self::EmptyExecutable { role } => {
                write!(formatter, "measured executable {role:?} is empty")
            }
            Self::PayloadDigestMismatch { field } => {
                write!(
                    formatter,
                    "{field} payload digest does not match retained bytes"
                )
            }
            Self::ResultIdentityMismatch => {
                formatter.write_str("reparsed result does not match authenticated result")
            }
            Self::IdentityMismatch { field } => write!(formatter, "{field} does not match"),
            Self::AuthenticatedResult(error) => {
                write!(formatter, "cannot revalidate authenticated result: {error}")
            }
            Self::ArtifactRecord(error) => {
                write!(formatter, "cannot construct artifact proof record: {error}")
            }
            Self::ProofMatch(error) => write!(formatter, "proof policy rejected result: {error}"),
            Self::ExecutableBinding(error) => {
                write!(formatter, "cannot bind proof to executable: {error}")
            }
            Self::PersistentFreshness(error) => {
                write!(formatter, "cannot persist proof freshness: {error}")
            }
        }
    }
}

impl std::error::Error for AuthenticatedProofExecutableBindingError {}

impl From<AuthenticatedExecutionError> for AuthenticatedProofExecutableBindingError {
    fn from(value: AuthenticatedExecutionError) -> Self {
        Self::AuthenticatedResult(value)
    }
}

impl From<ArtifactRecordConversionError> for AuthenticatedProofExecutableBindingError {
    fn from(value: ArtifactRecordConversionError) -> Self {
        Self::ArtifactRecord(value)
    }
}

impl From<ProofMatchError> for AuthenticatedProofExecutableBindingError {
    fn from(value: ProofMatchError) -> Self {
        Self::ProofMatch(value)
    }
}

impl From<ProofExecutableBindingError> for AuthenticatedProofExecutableBindingError {
    fn from(value: ProofExecutableBindingError) -> Self {
        Self::ExecutableBinding(value)
    }
}

impl From<PersistentFreshnessLedgerErrorV1> for AuthenticatedProofExecutableBindingError {
    fn from(value: PersistentFreshnessLedgerErrorV1) -> Self {
        Self::PersistentFreshness(value)
    }
}
