//! Canonical inert records joining proof inputs, dependencies, and results.
//!
//! A capsule is pre-envelope descriptive evidence. It deliberately has no
//! Worker V2 envelope identity, so an envelope can embed the capsule without a
//! hash cycle. A later external publication receipt may bind the capsule and
//! envelope identities together.
//!
//! Its digest detects accidental mutation, while comparison against identities
//! supplied by a trusted caller detects substitution. Neither operation
//! authenticates a compiler, establishes source-to-machine-code refinement,
//! durably consumes freshness, or grants load or launch authority.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifacts::{DigestAlgorithm, MAX_CODE_OBJECT_BYTES};

use crate::{
    AxiomPolicy, CorrelationId, Digest, MeasuredToolIdentity, ModelError,
    PersistentlyFreshProofExecutableBindingV1, ProofOutcome, ProofProperty, ProofTargetIdentity,
    Text, TrustedItem, VerificationModelIdentity, VerifierPolicy,
};

/// Canonical pre-envelope V1 wire marker. The earlier envelope-dependent draft
/// was never published and has no compatibility status.
pub const PROOF_CAPSULE_MAGIC_V1: [u8; 8] = *b"FE2PCP1\0";
pub const PROOF_CAPSULE_VERSION_V1: u16 = 1;
pub const MAX_PROOF_CAPSULE_BYTES_V1: usize = 128 * 1024;
pub const MAX_PROOF_CAPSULE_DEPENDENCIES_V1: usize = 128;
pub const MAX_PROOF_CAPSULE_FEATURES_V1: usize = 128;
pub const MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1: usize = crate::MAX_RESULT_BYTES;
pub const MAX_PROOF_CAPSULE_FINALIZED_HSACO_BYTES_V1: usize = MAX_CODE_OBJECT_BYTES;

const HEADER_BYTES: usize = 16;
const LAST_FIELD_TAG: u16 = 18;
const MIN_IDENTIFIER_WIRE_BYTES: usize = 3;
const MIN_NAMED_DIGEST_WIRE_BYTES: usize = MIN_IDENTIFIER_WIRE_BYTES + 32;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProofCapsuleDependencyV1 {
    name: Text,
    identity: Digest,
}

impl ProofCapsuleDependencyV1 {
    pub fn new(
        name: impl Into<String>,
        identity: Digest,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        require_nonzero(identity, "dependency identity")?;
        Ok(Self {
            name: Text::identifier("dependency name", name)
                .map_err(ProofCapsuleBuildErrorV1::Model)?,
            identity,
        })
    }

    pub const fn name(&self) -> &Text {
        &self.name
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofCapsulePayloadIdentityV1 {
    byte_len: u64,
    digest: Digest,
}

impl ProofCapsulePayloadIdentityV1 {
    pub fn sealed_result(byte_len: u64, digest: Digest) -> Result<Self, ProofCapsuleBuildErrorV1> {
        Self::new_bounded(
            byte_len,
            digest,
            "sealed proof result",
            MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1,
        )
    }

    pub fn finalized_hsaco(
        byte_len: u64,
        digest: Digest,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        Self::new_bounded(
            byte_len,
            digest,
            "finalized HSACO",
            MAX_PROOF_CAPSULE_FINALIZED_HSACO_BYTES_V1,
        )
    }

    fn new_bounded(
        byte_len: u64,
        digest: Digest,
        field: &'static str,
        max: usize,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        if byte_len == 0 {
            return Err(ProofCapsuleBuildErrorV1::PayloadLengthOutOfRange {
                field,
                value: byte_len,
                max,
            });
        }
        if byte_len > max as u64 {
            return Err(ProofCapsuleBuildErrorV1::PayloadLengthOutOfRange {
                field,
                value: byte_len,
                max,
            });
        }
        require_nonzero(digest, "payload identity")?;
        Ok(Self { byte_len, digest })
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn digest(self) -> Digest {
        self.digest
    }
}

/// Exact pre-envelope proof target plus dependency, feature, ABI, launch,
/// machine-effect, artifact, and finalized-payload identities.
///
/// An envelope identity is intentionally unavailable, preventing a hash cycle
/// when `WorkerV2LoadEnvelopeV1` embeds these bytes. A later external
/// publication receipt may bind the resulting capsule and envelope identities.
///
/// ```compile_fail
/// # fn no_envelope_dependency(target: &fe2o3_verifier::ProofCapsuleTargetV1) {
/// let _ = target.envelope_identity();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCapsuleTargetV1 {
    proof_target: ProofTargetIdentity,
    dependencies: Vec<ProofCapsuleDependencyV1>,
    features: Vec<Text>,
    abi_identity: Digest,
    launch_identity: Digest,
    machine_effect_evidence_identity: Digest,
    finalized_payload: ProofCapsulePayloadIdentityV1,
    artifact_identity: Digest,
}

impl ProofCapsuleTargetV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        proof_target: ProofTargetIdentity,
        mut dependencies: Vec<ProofCapsuleDependencyV1>,
        mut features: Vec<Text>,
        abi_identity: Digest,
        launch_identity: Digest,
        machine_effect_evidence_identity: Digest,
        finalized_payload: ProofCapsulePayloadIdentityV1,
        artifact_identity: Digest,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        for (field, identity) in [
            ("kernel identity", proof_target.kernel_id),
            ("source identity", proof_target.source_tree_digest),
            (
                "dependency closure identity",
                proof_target.crate_graph_digest,
            ),
            ("feature closure identity", proof_target.environment_digest),
            ("ABI identity", abi_identity),
            ("effects identity", proof_target.effects_contract_digest),
            ("launch identity", launch_identity),
            (
                "model target identity",
                proof_target.functional_specification_digest,
            ),
            (
                "machine-effect evidence identity",
                machine_effect_evidence_identity,
            ),
            ("artifact identity", artifact_identity),
        ] {
            require_nonzero(identity, field)?;
        }
        for identity in proof_target.digests() {
            require_nonzero(identity, "proof target identity")?;
        }
        validate_payload_bound(
            finalized_payload,
            "finalized HSACO",
            MAX_PROOF_CAPSULE_FINALIZED_HSACO_BYTES_V1,
        )?;
        canonicalize_dependencies(&mut dependencies)?;
        canonicalize_features(&mut features)?;
        Ok(Self {
            proof_target,
            dependencies,
            features,
            abi_identity,
            launch_identity,
            machine_effect_evidence_identity,
            finalized_payload,
            artifact_identity,
        })
    }

    pub const fn proof_target(&self) -> ProofTargetIdentity {
        self.proof_target
    }

    pub fn dependencies(&self) -> &[ProofCapsuleDependencyV1] {
        &self.dependencies
    }

    pub fn features(&self) -> &[Text] {
        &self.features
    }

    pub const fn abi_identity(&self) -> Digest {
        self.abi_identity
    }

    pub const fn effects_identity(&self) -> Digest {
        self.proof_target.effects_contract_digest
    }

    pub const fn launch_identity(&self) -> Digest {
        self.launch_identity
    }

    pub const fn machine_effect_evidence_identity(&self) -> Digest {
        self.machine_effect_evidence_identity
    }

    pub const fn finalized_payload(&self) -> ProofCapsulePayloadIdentityV1 {
        self.finalized_payload
    }

    pub const fn artifact_identity(&self) -> Digest {
        self.artifact_identity
    }
}

/// Exact verification model, measured Verus and solver identities, approved
/// axiom policy, requested axioms, and requested proof properties.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCapsulePolicyV1 {
    model: VerificationModelIdentity,
    verus: MeasuredToolIdentity,
    solver: MeasuredToolIdentity,
    approved_axioms: AxiomPolicy,
    requested_axioms: Vec<TrustedItem>,
    requested_properties: Vec<ProofProperty>,
}

impl ProofCapsulePolicyV1 {
    pub fn new(
        model: VerificationModelIdentity,
        verus: MeasuredToolIdentity,
        solver: MeasuredToolIdentity,
        approved_axioms: AxiomPolicy,
        mut requested_axioms: Vec<TrustedItem>,
        mut requested_properties: Vec<ProofProperty>,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        validate_model_and_tool_identities(&model, [&verus, &solver])?;
        for item in approved_axioms.allowed() {
            require_nonzero(item.contract_digest(), "approved axiom contract identity")?;
        }
        canonicalize_trusted_items(&mut requested_axioms, "requested axioms")?;
        approved_axioms
            .validate(&requested_axioms)
            .map_err(ProofCapsuleBuildErrorV1::Model)?;
        canonicalize_properties(&mut requested_properties, false)?;
        Ok(Self {
            model,
            verus,
            solver,
            approved_axioms,
            requested_axioms,
            requested_properties,
        })
    }

    pub const fn model(&self) -> &VerificationModelIdentity {
        &self.model
    }

    pub const fn verus(&self) -> &MeasuredToolIdentity {
        &self.verus
    }

    pub const fn solver(&self) -> &MeasuredToolIdentity {
        &self.solver
    }

    pub const fn approved_axioms(&self) -> &AxiomPolicy {
        &self.approved_axioms
    }

    pub fn requested_axioms(&self) -> &[TrustedItem] {
        &self.requested_axioms
    }

    pub fn requested_properties(&self) -> &[ProofProperty] {
        &self.requested_properties
    }
}

/// Exact persistent proof identity copied from the existing one-history
/// freshness bridge. The persistent binding identity commits the receipt's
/// previous state, so this projection does not replace or weaken that bridge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProofCapsuleFreshnessIdentityV1 {
    proof_binding_identity: Digest,
    challenge: Digest,
    transcript: Digest,
    result: Digest,
    ledger_namespace: Digest,
    previous_ledger_state_identity: Digest,
    ledger_generation: u64,
    ledger_state_identity: Digest,
    persistent_binding_identity: Digest,
}

impl ProofCapsuleFreshnessIdentityV1 {
    pub fn project_from_persistent(binding: &PersistentlyFreshProofExecutableBindingV1) -> Self {
        let identity = binding.identity();
        let receipt = binding.freshness_receipt();
        let consumed = identity.consumed_execution();
        Self {
            proof_binding_identity: identity.proof_binding_identity(),
            challenge: consumed.challenge(),
            transcript: consumed.transcript(),
            result: consumed.result(),
            ledger_namespace: identity.ledger_namespace(),
            previous_ledger_state_identity: receipt.previous_state_identity(),
            ledger_generation: identity.ledger_generation(),
            ledger_state_identity: identity.ledger_state_identity(),
            persistent_binding_identity: identity.binding_identity(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_inert(
        proof_binding_identity: Digest,
        challenge: Digest,
        transcript: Digest,
        result: Digest,
        ledger_namespace: Digest,
        previous_ledger_state_identity: Digest,
        ledger_generation: u64,
        ledger_state_identity: Digest,
        persistent_binding_identity: Digest,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        for (field, identity) in [
            ("proof binding identity", proof_binding_identity),
            ("freshness challenge", challenge),
            ("freshness transcript", transcript),
            ("freshness result", result),
            ("ledger namespace", ledger_namespace),
            (
                "previous ledger state identity",
                previous_ledger_state_identity,
            ),
            ("ledger state identity", ledger_state_identity),
            ("persistent binding identity", persistent_binding_identity),
        ] {
            require_nonzero(identity, field)?;
        }
        if ledger_generation == 0 {
            return Err(ProofCapsuleBuildErrorV1::ZeroLedgerGeneration);
        }
        Ok(Self {
            proof_binding_identity,
            challenge,
            transcript,
            result,
            ledger_namespace,
            previous_ledger_state_identity,
            ledger_generation,
            ledger_state_identity,
            persistent_binding_identity,
        })
    }

    pub const fn proof_binding_identity(self) -> Digest {
        self.proof_binding_identity
    }

    pub const fn challenge(self) -> Digest {
        self.challenge
    }

    pub const fn transcript(self) -> Digest {
        self.transcript
    }

    pub const fn result(self) -> Digest {
        self.result
    }

    pub const fn ledger_namespace(self) -> Digest {
        self.ledger_namespace
    }

    pub const fn previous_ledger_state_identity(self) -> Digest {
        self.previous_ledger_state_identity
    }

    pub const fn ledger_generation(self) -> u64 {
        self.ledger_generation
    }

    pub const fn ledger_state_identity(self) -> Digest {
        self.ledger_state_identity
    }

    pub const fn persistent_binding_identity(self) -> Digest {
        self.persistent_binding_identity
    }
}

/// Correlation and exact identities from one sealed verifier execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCapsuleExecutionV1 {
    correlation_id: CorrelationId,
    canonical_invocation_identity: Digest,
    policy_identity: Digest,
    request_identity: Digest,
    challenge: Digest,
    transcript_identity: Digest,
    sealed_result: ProofCapsulePayloadIdentityV1,
    freshness: Option<ProofCapsuleFreshnessIdentityV1>,
}

impl ProofCapsuleExecutionV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new_inert(
        correlation_id: CorrelationId,
        canonical_invocation_identity: Digest,
        policy_identity: Digest,
        request_identity: Digest,
        challenge: Digest,
        transcript_identity: Digest,
        sealed_result: ProofCapsulePayloadIdentityV1,
        freshness: Option<ProofCapsuleFreshnessIdentityV1>,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        if correlation_id.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(ProofCapsuleBuildErrorV1::ZeroCorrelation);
        }
        for (field, identity) in [
            (
                "canonical invocation identity",
                canonical_invocation_identity,
            ),
            ("policy identity", policy_identity),
            ("request identity", request_identity),
            ("execution challenge", challenge),
            ("execution transcript identity", transcript_identity),
        ] {
            require_nonzero(identity, field)?;
        }
        if let Some(freshness) = freshness {
            if freshness.challenge != challenge {
                return Err(ProofCapsuleBuildErrorV1::FreshnessMismatch { field: "challenge" });
            }
            if freshness.transcript != transcript_identity {
                return Err(ProofCapsuleBuildErrorV1::FreshnessMismatch {
                    field: "transcript",
                });
            }
            if freshness.result != sealed_result.digest {
                return Err(ProofCapsuleBuildErrorV1::FreshnessMismatch { field: "result" });
            }
        }
        validate_payload_bound(
            sealed_result,
            "sealed proof result",
            MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1,
        )?;
        Ok(Self {
            correlation_id,
            canonical_invocation_identity,
            policy_identity,
            request_identity,
            challenge,
            transcript_identity,
            sealed_result,
            freshness,
        })
    }

    fn project_from_persistent(
        binding: &PersistentlyFreshProofExecutableBindingV1,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        let proof = binding.proof_binding();
        let evidence = proof.execution_evidence();
        let execution = proof.execution_identity();
        let freshness = ProofCapsuleFreshnessIdentityV1::project_from_persistent(binding);
        Ok(Self {
            correlation_id: evidence.result().correlation_id(),
            canonical_invocation_identity: execution.canonical_invocation_digest(),
            policy_identity: execution.policy_digest(),
            request_identity: execution.request_digest(),
            challenge: execution.challenge(),
            transcript_identity: execution.transcript_digest(),
            sealed_result: ProofCapsulePayloadIdentityV1::sealed_result(
                execution.result().byte_len(),
                execution.result().digest(),
            )?,
            freshness: Some(freshness),
        })
    }

    pub const fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    pub const fn canonical_invocation_identity(&self) -> Digest {
        self.canonical_invocation_identity
    }

    pub const fn policy_identity(&self) -> Digest {
        self.policy_identity
    }

    pub const fn request_identity(&self) -> Digest {
        self.request_identity
    }

    pub const fn challenge(&self) -> Digest {
        self.challenge
    }

    pub const fn transcript_identity(&self) -> Digest {
        self.transcript_identity
    }

    pub const fn sealed_result(&self) -> ProofCapsulePayloadIdentityV1 {
        self.sealed_result
    }

    pub const fn freshness(&self) -> Option<ProofCapsuleFreshnessIdentityV1> {
        self.freshness
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCapsuleResultV1 {
    outcome: ProofOutcome,
    proved_properties: Vec<ProofProperty>,
}

impl ProofCapsuleResultV1 {
    pub fn new(
        outcome: ProofOutcome,
        mut proved_properties: Vec<ProofProperty>,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        canonicalize_properties(&mut proved_properties, outcome != ProofOutcome::Proved)?;
        if outcome != ProofOutcome::Proved && !proved_properties.is_empty() {
            return Err(ProofCapsuleBuildErrorV1::ClaimsOnIncompleteProof);
        }
        Ok(Self {
            outcome,
            proved_properties,
        })
    }

    pub const fn outcome(&self) -> ProofOutcome {
        self.outcome
    }

    pub fn proved_properties(&self) -> &[ProofProperty] {
        &self.proved_properties
    }
}

/// Versioned, bounded, canonical, and inert proof input/result record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCapsuleV1 {
    target: ProofCapsuleTargetV1,
    policy: ProofCapsulePolicyV1,
    execution: ProofCapsuleExecutionV1,
    result: ProofCapsuleResultV1,
    identity: Digest,
}

impl ProofCapsuleV1 {
    pub fn new_inert(
        target: ProofCapsuleTargetV1,
        policy: ProofCapsulePolicyV1,
        execution: ProofCapsuleExecutionV1,
        result: ProofCapsuleResultV1,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        validate_outcome(&policy, &execution, &result)?;
        let mut capsule = Self {
            target,
            policy,
            execution,
            result,
            identity: Digest::from_bytes([0; 32]),
        };
        capsule.identity = sha256(&capsule.identity_input_bytes());
        if capsule.to_bytes().len() > MAX_PROOF_CAPSULE_BYTES_V1 {
            return Err(ProofCapsuleBuildErrorV1::TooLarge {
                max: MAX_PROOF_CAPSULE_BYTES_V1,
            });
        }
        Ok(capsule)
    }

    /// Projects a proved, inert capsule from an existing persistent binding.
    ///
    /// The policy digest, proof target, model, measured Verus/solver identities,
    /// requested properties, and requested axioms are rejoined here. Additional
    /// target axes remain inert caller inputs and require a future production
    /// authenticator and compiler-refinement evidence.
    pub fn project_inert_from_persistently_fresh(
        target: ProofCapsuleTargetV1,
        verifier_policy: &VerifierPolicy,
        binding: &PersistentlyFreshProofExecutableBindingV1,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        let proof = binding.proof_binding();
        let evidence = proof.execution_evidence();
        let plan = evidence.invocation_plan();
        let proof_result = evidence.result();
        if sha256(&verifier_policy.to_canonical_bytes()) != evidence.policy_digest() {
            return Err(ProofCapsuleBuildErrorV1::PolicyIdentityMismatch);
        }
        if target.proof_target != proof_result.target() {
            return Err(ProofCapsuleBuildErrorV1::ProofTargetMismatch);
        }
        let bound_finalized = proof
            .executable_binding()
            .executable()
            .finalized_code_object_digest();
        // ProofExecutableBindingV1 retains the finalized occurrence digest but
        // not its byte length. The capsule length is bounded here; a later
        // artifact/envelope join must validate the exact occurrence length.
        require_bound_finalized_payload(target.finalized_payload, bound_finalized)?;
        if verifier_policy.expected_tools() != plan.tools()
            || verifier_policy.expected_model() != proof_result.model()
        {
            return Err(ProofCapsuleBuildErrorV1::VerifierPolicyMismatch);
        }
        let policy = ProofCapsulePolicyV1::new(
            proof_result.model().clone(),
            proof_result.tools().verifier().clone(),
            proof_result.tools().solver().clone(),
            verifier_policy.axiom_policy().clone(),
            proof_result.trusted_items().to_vec(),
            plan.request().properties().to_vec(),
        )?;
        let execution = ProofCapsuleExecutionV1::project_from_persistent(binding)?;
        let result = ProofCapsuleResultV1::new(
            proof_result.outcome(),
            proof_result.proved_properties().to_vec(),
        )?;
        Self::new_inert(target, policy, execution, result)
    }

    pub const fn version(&self) -> u16 {
        PROOF_CAPSULE_VERSION_V1
    }

    pub const fn target(&self) -> &ProofCapsuleTargetV1 {
        &self.target
    }

    pub const fn policy(&self) -> &ProofCapsulePolicyV1 {
        &self.policy
    }

    pub const fn execution(&self) -> &ProofCapsuleExecutionV1 {
        &self.execution
    }

    pub const fn result(&self) -> &ProofCapsuleResultV1 {
        &self.result
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    /// Confirms this schema is deliberately independent of a Worker V2
    /// envelope. A later external publication receipt may bind both identities.
    pub const fn is_pre_envelope_evidence(&self) -> bool {
        true
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.identity_input_bytes();
        put_field(&mut bytes, 18);
        put_digest(&mut bytes, self.identity);
        debug_assert!(bytes.len() <= MAX_PROOF_CAPSULE_BYTES_V1);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProofCapsuleDecodeErrorV1> {
        decode_capsule(bytes)
    }

    fn identity_input_bytes(&self) -> Vec<u8> {
        let mut writer = CapsuleWriter::new();
        writer.header();
        writer.field(1);
        writer.proof_target(self.target.proof_target);
        writer.field(2);
        writer.dependencies(&self.target.dependencies);
        writer.field(3);
        writer.features(&self.target.features);
        writer.field(4);
        writer.digest(self.target.abi_identity);
        writer.field(5);
        writer.digest(self.target.launch_identity);
        writer.field(6);
        writer.model(&self.policy.model);
        writer.field(7);
        writer.tool(&self.policy.verus);
        writer.field(8);
        writer.tool(&self.policy.solver);
        writer.field(9);
        writer.trusted_items(self.policy.approved_axioms.allowed());
        writer.field(10);
        writer.trusted_items(&self.policy.requested_axioms);
        writer.field(11);
        writer.properties(&self.policy.requested_properties);
        writer.field(12);
        writer.execution(&self.execution);
        writer.field(13);
        writer.result(&self.result);
        writer.field(14);
        writer.digest(self.target.machine_effect_evidence_identity);
        writer.field(15);
        writer.payload(self.target.finalized_payload);
        writer.field(16);
        writer.digest(self.target.artifact_identity);
        writer.field(17);
        writer.freshness(self.execution.freshness);
        writer.finish_with_total_len()
    }
}

/// Expected context for one exact persistent freshness identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofCapsuleFreshnessExpectationV1 {
    identity: ProofCapsuleFreshnessIdentityV1,
}

impl ProofCapsuleFreshnessExpectationV1 {
    pub const fn new(identity: ProofCapsuleFreshnessIdentityV1) -> Self {
        Self { identity }
    }

    pub fn project_from_persistent(binding: &PersistentlyFreshProofExecutableBindingV1) -> Self {
        Self::new(ProofCapsuleFreshnessIdentityV1::project_from_persistent(
            binding,
        ))
    }

    pub const fn identity(self) -> ProofCapsuleFreshnessIdentityV1 {
        self.identity
    }
}

/// Identities supplied by a trusted caller for exact capsule comparison.
///
/// Constructing this value does not authenticate its inputs. Production code
/// must obtain them from authenticated compiler, artifact, and
/// rollback-resistant freshness state. Future runtime authority must revalidate
/// and durably consume the proof against the live ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofCapsuleExpectationV1 {
    capsule_identity: Digest,
    artifact_identity: Digest,
    freshness: Option<ProofCapsuleFreshnessExpectationV1>,
}

impl ProofCapsuleExpectationV1 {
    pub fn new(
        capsule_identity: Digest,
        artifact_identity: Digest,
        freshness: Option<ProofCapsuleFreshnessExpectationV1>,
    ) -> Result<Self, ProofCapsuleBuildErrorV1> {
        require_nonzero(capsule_identity, "expected capsule identity")?;
        require_nonzero(artifact_identity, "expected artifact identity")?;
        Ok(Self {
            capsule_identity,
            artifact_identity,
            freshness,
        })
    }

    pub const fn capsule_identity(self) -> Digest {
        self.capsule_identity
    }

    pub const fn artifact_identity(self) -> Digest {
        self.artifact_identity
    }

    pub const fn freshness(self) -> Option<ProofCapsuleFreshnessExpectationV1> {
        self.freshness
    }
}

/// Process-local duplicate detection for canonical, expected capsules.
///
/// This diagnostic set is intentionally not serializable. Recording a capsule
/// here does not consume its persistent receipt and cannot preserve durable
/// single-use freshness. Future authority must revalidate and durably consume
/// against the live `PersistentProofFreshnessLedgerV1` and a rollback-resistant
/// production root.
#[derive(Debug, Default)]
pub struct ProcessLocalProofCapsuleDuplicateDetectorV1 {
    recorded_capsules: BTreeSet<Digest>,
    recorded_correlations: BTreeSet<CorrelationId>,
    recorded_challenges: BTreeSet<Digest>,
    recorded_transcripts: BTreeSet<Digest>,
    recorded_results: BTreeSet<Digest>,
    recorded_persistent_bindings: BTreeSet<Digest>,
}

impl ProcessLocalProofCapsuleDuplicateDetectorV1 {
    pub const fn new() -> Self {
        Self {
            recorded_capsules: BTreeSet::new(),
            recorded_correlations: BTreeSet::new(),
            recorded_challenges: BTreeSet::new(),
            recorded_transcripts: BTreeSet::new(),
            recorded_results: BTreeSet::new(),
            recorded_persistent_bindings: BTreeSet::new(),
        }
    }

    pub fn recorded_count(&self) -> usize {
        self.recorded_capsules.len()
    }

    /// Canonically parses, compares expected identities, and records local
    /// duplicates. The returned capsule remains cloneable inert evidence; this
    /// method does not consume or revalidate a live persistent ledger receipt.
    pub fn parse_validate_and_record(
        &mut self,
        bytes: &[u8],
        expected: ProofCapsuleExpectationV1,
    ) -> Result<ProofCapsuleV1, ProofCapsuleContextErrorV1> {
        let capsule = ProofCapsuleV1::from_bytes(bytes)?;
        validate_expectation(&capsule, expected)?;
        if self.recorded_capsules.contains(&capsule.identity) {
            return Err(ProofCapsuleContextErrorV1::CapsuleDuplicate);
        }
        if self
            .recorded_correlations
            .contains(&capsule.execution.correlation_id)
            || self
                .recorded_challenges
                .contains(&capsule.execution.challenge)
            || self
                .recorded_transcripts
                .contains(&capsule.execution.transcript_identity)
            || self
                .recorded_results
                .contains(&capsule.execution.sealed_result.digest)
        {
            return Err(ProofCapsuleContextErrorV1::ExecutionDuplicate);
        }
        if let Some(freshness) = capsule.execution.freshness
            && self
                .recorded_persistent_bindings
                .contains(&freshness.persistent_binding_identity)
        {
            return Err(ProofCapsuleContextErrorV1::PersistentProofDuplicate);
        }

        let capsule_was_new = self.recorded_capsules.insert(capsule.identity);
        let correlation_was_new = self
            .recorded_correlations
            .insert(capsule.execution.correlation_id);
        let challenge_was_new = self.recorded_challenges.insert(capsule.execution.challenge);
        let transcript_was_new = self
            .recorded_transcripts
            .insert(capsule.execution.transcript_identity);
        let result_was_new = self
            .recorded_results
            .insert(capsule.execution.sealed_result.digest);
        debug_assert!(
            capsule_was_new
                && correlation_was_new
                && challenge_was_new
                && transcript_was_new
                && result_was_new
        );
        if let Some(freshness) = capsule.execution.freshness {
            let persistent_was_new = self
                .recorded_persistent_bindings
                .insert(freshness.persistent_binding_identity);
            debug_assert!(persistent_was_new);
        }
        Ok(capsule)
    }
}

fn validate_expectation(
    capsule: &ProofCapsuleV1,
    expected: ProofCapsuleExpectationV1,
) -> Result<(), ProofCapsuleContextErrorV1> {
    if capsule.target.artifact_identity != expected.artifact_identity {
        return Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::Artifact,
        });
    }
    validate_expected_freshness(capsule.execution.freshness, expected.freshness)?;
    if capsule.identity != expected.capsule_identity {
        return Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::Capsule,
        });
    }
    Ok(())
}

fn validate_expected_freshness(
    actual: Option<ProofCapsuleFreshnessIdentityV1>,
    expected: Option<ProofCapsuleFreshnessExpectationV1>,
) -> Result<(), ProofCapsuleContextErrorV1> {
    let (actual, expected) = match (actual, expected) {
        (Some(actual), Some(expected)) => (actual, expected.identity),
        (None, None) => return Ok(()),
        (Some(_), None) | (None, Some(_)) => {
            return Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
                field: ProofCapsuleIdentityFieldV1::Freshness,
            });
        }
    };
    if actual.ledger_namespace != expected.ledger_namespace {
        return Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::LedgerNamespace,
        });
    }
    if actual.ledger_generation < expected.ledger_generation {
        return Err(ProofCapsuleContextErrorV1::StaleLedgerGeneration {
            expected: expected.ledger_generation,
            actual: actual.ledger_generation,
        });
    }
    if actual.ledger_generation > expected.ledger_generation {
        return Err(ProofCapsuleContextErrorV1::UnexpectedLedgerGeneration {
            expected: expected.ledger_generation,
            actual: actual.ledger_generation,
        });
    }
    for (field, actual, expected) in [
        (
            ProofCapsuleIdentityFieldV1::PreviousLedgerState,
            actual.previous_ledger_state_identity,
            expected.previous_ledger_state_identity,
        ),
        (
            ProofCapsuleIdentityFieldV1::LedgerState,
            actual.ledger_state_identity,
            expected.ledger_state_identity,
        ),
        (
            ProofCapsuleIdentityFieldV1::ProofBinding,
            actual.proof_binding_identity,
            expected.proof_binding_identity,
        ),
        (
            ProofCapsuleIdentityFieldV1::Challenge,
            actual.challenge,
            expected.challenge,
        ),
        (
            ProofCapsuleIdentityFieldV1::Transcript,
            actual.transcript,
            expected.transcript,
        ),
        (
            ProofCapsuleIdentityFieldV1::Result,
            actual.result,
            expected.result,
        ),
        (
            ProofCapsuleIdentityFieldV1::PersistentBinding,
            actual.persistent_binding_identity,
            expected.persistent_binding_identity,
        ),
    ] {
        if actual != expected {
            return Err(ProofCapsuleContextErrorV1::IdentitySubstitution { field });
        }
    }
    Ok(())
}

fn decode_capsule(bytes: &[u8]) -> Result<ProofCapsuleV1, ProofCapsuleDecodeErrorV1> {
    if bytes.len() > MAX_PROOF_CAPSULE_BYTES_V1 {
        return Err(ProofCapsuleDecodeErrorV1::TooLarge {
            max: MAX_PROOF_CAPSULE_BYTES_V1,
        });
    }
    if bytes.len() < HEADER_BYTES {
        return Err(ProofCapsuleDecodeErrorV1::Truncated);
    }
    let mut reader = CapsuleReader::new(bytes);
    if reader.array::<8>()? != PROOF_CAPSULE_MAGIC_V1 {
        return Err(ProofCapsuleDecodeErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != PROOF_CAPSULE_VERSION_V1 {
        return Err(ProofCapsuleDecodeErrorV1::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(ProofCapsuleDecodeErrorV1::UnsupportedFlags(flags));
    }
    let total_len = reader.length_u32(MAX_PROOF_CAPSULE_BYTES_V1)?;
    if total_len > bytes.len() {
        return Err(ProofCapsuleDecodeErrorV1::Truncated);
    }
    if total_len < bytes.len() {
        return Err(ProofCapsuleDecodeErrorV1::TrailingBytes);
    }

    reader.field(1)?;
    let proof_target = reader.proof_target()?;
    reader.field(2)?;
    let dependencies = reader.dependencies()?;
    reader.field(3)?;
    let features = reader.features()?;
    reader.field(4)?;
    let abi_identity = reader.digest()?;
    reader.field(5)?;
    let launch_identity = reader.digest()?;
    reader.field(6)?;
    let model = reader.model()?;
    reader.field(7)?;
    let verus = reader.tool()?;
    reader.field(8)?;
    let solver = reader.tool()?;
    reader.field(9)?;
    let approved_axioms = reader.trusted_items("approved axioms")?;
    reader.field(10)?;
    let requested_axioms = reader.trusted_items("requested axioms")?;
    reader.field(11)?;
    let requested_properties = reader.properties(false)?;
    reader.field(12)?;
    let execution_parts = reader.execution()?;
    reader.field(13)?;
    let result = reader.result()?;
    reader.field(14)?;
    let machine_effect_evidence_identity = reader.digest()?;
    reader.field(15)?;
    let finalized_payload = reader.payload(
        "finalized HSACO",
        MAX_PROOF_CAPSULE_FINALIZED_HSACO_BYTES_V1,
    )?;
    reader.field(16)?;
    let artifact_identity = reader.digest()?;
    reader.field(17)?;
    let freshness = reader.freshness()?;

    let identity_input_len = reader.consumed_len();
    reader.field(18)?;
    let encoded_identity = reader.digest()?;
    if !reader.is_empty() {
        return Err(ProofCapsuleDecodeErrorV1::TrailingBytes);
    }
    if sha256(&bytes[..identity_input_len]) != encoded_identity {
        return Err(ProofCapsuleDecodeErrorV1::IdentityMismatch);
    }

    let target = ProofCapsuleTargetV1::new(
        proof_target,
        dependencies,
        features,
        abi_identity,
        launch_identity,
        machine_effect_evidence_identity,
        finalized_payload,
        artifact_identity,
    )?;
    let policy = ProofCapsulePolicyV1::new(
        model,
        verus,
        solver,
        AxiomPolicy::allow_list(approved_axioms).map_err(ProofCapsuleBuildErrorV1::Model)?,
        requested_axioms,
        requested_properties,
    )?;
    let execution = ProofCapsuleExecutionV1::new_inert(
        execution_parts.correlation_id,
        execution_parts.canonical_invocation_identity,
        execution_parts.policy_identity,
        execution_parts.request_identity,
        execution_parts.challenge,
        execution_parts.transcript_identity,
        execution_parts.sealed_result,
        freshness,
    )?;
    let capsule = ProofCapsuleV1::new_inert(target, policy, execution, result)?;
    if capsule.identity != encoded_identity || capsule.to_bytes() != bytes {
        return Err(ProofCapsuleDecodeErrorV1::NonCanonical);
    }
    Ok(capsule)
}

fn validate_outcome(
    policy: &ProofCapsulePolicyV1,
    execution: &ProofCapsuleExecutionV1,
    result: &ProofCapsuleResultV1,
) -> Result<(), ProofCapsuleBuildErrorV1> {
    match result.outcome {
        ProofOutcome::Proved => {
            if result.proved_properties != policy.requested_properties {
                return Err(ProofCapsuleBuildErrorV1::IncompleteProof);
            }
            if execution.freshness.is_none() {
                return Err(ProofCapsuleBuildErrorV1::MissingPersistentFreshness);
            }
        }
        ProofOutcome::Failed | ProofOutcome::TimedOut => {
            if !result.proved_properties.is_empty() {
                return Err(ProofCapsuleBuildErrorV1::ClaimsOnIncompleteProof);
            }
            if execution.freshness.is_some() {
                return Err(ProofCapsuleBuildErrorV1::UnexpectedPersistentFreshness);
            }
        }
    }
    Ok(())
}

fn validate_model_and_tool_identities(
    model: &VerificationModelIdentity,
    tools: [&MeasuredToolIdentity; 2],
) -> Result<(), ProofCapsuleBuildErrorV1> {
    require_nonzero(model.axioms_digest(), "model axioms identity")?;
    for tool in tools {
        require_nonzero(tool.executable_digest(), "tool executable identity")?;
        require_nonzero(tool.configuration_digest(), "tool configuration identity")?;
    }
    Ok(())
}

fn canonicalize_dependencies(
    dependencies: &mut [ProofCapsuleDependencyV1],
) -> Result<(), ProofCapsuleBuildErrorV1> {
    if dependencies.len() > MAX_PROOF_CAPSULE_DEPENDENCIES_V1 {
        return Err(ProofCapsuleBuildErrorV1::TooManyItems {
            field: "dependencies",
            max: MAX_PROOF_CAPSULE_DEPENDENCIES_V1,
        });
    }
    dependencies.sort_unstable_by(|left, right| left.name.cmp(&right.name));
    if dependencies
        .windows(2)
        .any(|pair| pair[0].name == pair[1].name)
    {
        return Err(ProofCapsuleBuildErrorV1::DuplicateItem {
            field: "dependency name",
        });
    }
    Ok(())
}

fn canonicalize_features(features: &mut [Text]) -> Result<(), ProofCapsuleBuildErrorV1> {
    if features.len() > MAX_PROOF_CAPSULE_FEATURES_V1 {
        return Err(ProofCapsuleBuildErrorV1::TooManyItems {
            field: "features",
            max: MAX_PROOF_CAPSULE_FEATURES_V1,
        });
    }
    features.sort_unstable();
    if features.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProofCapsuleBuildErrorV1::DuplicateItem { field: "feature" });
    }
    Ok(())
}

fn canonicalize_trusted_items(
    items: &mut [TrustedItem],
    field: &'static str,
) -> Result<(), ProofCapsuleBuildErrorV1> {
    if items.len() > crate::MAX_TRUSTED_ITEMS {
        return Err(ProofCapsuleBuildErrorV1::TooManyItems {
            field,
            max: crate::MAX_TRUSTED_ITEMS,
        });
    }
    items.sort_unstable();
    if items
        .windows(2)
        .any(|pair| pair[0].name() == pair[1].name())
    {
        return Err(ProofCapsuleBuildErrorV1::DuplicateItem { field });
    }
    for item in items {
        require_nonzero(item.contract_digest(), "axiom contract identity")?;
    }
    Ok(())
}

fn canonicalize_properties(
    properties: &mut [ProofProperty],
    allow_empty: bool,
) -> Result<(), ProofCapsuleBuildErrorV1> {
    if properties.len() > crate::MAX_PROPERTIES || (!allow_empty && properties.is_empty()) {
        return Err(ProofCapsuleBuildErrorV1::PropertyCountOutOfRange {
            min: usize::from(!allow_empty),
            max: crate::MAX_PROPERTIES,
        });
    }
    properties.sort_unstable();
    if properties.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ProofCapsuleBuildErrorV1::DuplicateItem {
            field: "proof property",
        });
    }
    Ok(())
}

fn require_nonzero(identity: Digest, field: &'static str) -> Result<(), ProofCapsuleBuildErrorV1> {
    if identity.as_bytes().iter().all(|byte| *byte == 0) {
        Err(ProofCapsuleBuildErrorV1::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn validate_payload_bound(
    payload: ProofCapsulePayloadIdentityV1,
    field: &'static str,
    max: usize,
) -> Result<(), ProofCapsuleBuildErrorV1> {
    if payload.byte_len == 0 || payload.byte_len > max as u64 {
        Err(ProofCapsuleBuildErrorV1::PayloadLengthOutOfRange {
            field,
            value: payload.byte_len,
            max,
        })
    } else {
        Ok(())
    }
}

fn require_bound_finalized_payload(
    capsule: ProofCapsulePayloadIdentityV1,
    bound: fe2o3_artifacts::PayloadDigest,
) -> Result<(), ProofCapsuleBuildErrorV1> {
    if bound.algorithm() != DigestAlgorithm::Sha256
        || bound.bytes().as_bytes() != capsule.digest.as_bytes()
    {
        Err(ProofCapsuleBuildErrorV1::FinalizedPayloadMismatch)
    } else {
        Ok(())
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn put_field(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn put_digest(bytes: &mut Vec<u8>, value: Digest) {
    bytes.extend_from_slice(value.as_bytes());
}

struct CapsuleWriter {
    bytes: Vec<u8>,
}

impl CapsuleWriter {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(4096),
        }
    }

    fn header(&mut self) {
        self.bytes.extend_from_slice(&PROOF_CAPSULE_MAGIC_V1);
        self.u16(PROOF_CAPSULE_VERSION_V1);
        self.u16(0);
        self.u32(0);
    }

    fn finish_with_total_len(mut self) -> Vec<u8> {
        let total_len = self
            .bytes
            .len()
            .checked_add(2 + 32)
            .expect("bounded capsule length cannot overflow");
        let total_len = u32::try_from(total_len).expect("bounded capsule length fits u32");
        self.bytes[12..16].copy_from_slice(&total_len.to_le_bytes());
        self.bytes
    }

    fn field(&mut self, value: u16) {
        self.u16(value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn digest(&mut self, value: Digest) {
        put_digest(&mut self.bytes, value);
    }

    fn text(&mut self, value: &Text) {
        self.u16(value.as_str().len() as u16);
        self.bytes.extend_from_slice(value.as_str().as_bytes());
    }

    fn payload(&mut self, value: ProofCapsulePayloadIdentityV1) {
        self.u64(value.byte_len);
        self.digest(value.digest);
    }

    fn proof_target(&mut self, value: ProofTargetIdentity) {
        for digest in value.digests() {
            self.digest(digest);
        }
    }

    fn dependencies(&mut self, values: &[ProofCapsuleDependencyV1]) {
        self.u16(values.len() as u16);
        for value in values {
            self.text(&value.name);
            self.digest(value.identity);
        }
    }

    fn features(&mut self, values: &[Text]) {
        self.u16(values.len() as u16);
        for value in values {
            self.text(value);
        }
    }

    fn model(&mut self, value: &VerificationModelIdentity) {
        self.text(value.version());
        self.digest(value.axioms_digest());
    }

    fn tool(&mut self, value: &MeasuredToolIdentity) {
        self.text(value.name());
        self.text(value.version());
        self.digest(value.executable_digest());
        self.digest(value.configuration_digest());
    }

    fn trusted_items(&mut self, values: &[TrustedItem]) {
        self.u16(values.len() as u16);
        for value in values {
            self.text(value.name());
            self.digest(value.contract_digest());
        }
    }

    fn properties(&mut self, values: &[ProofProperty]) {
        self.u16(values.len() as u16);
        for value in values {
            self.u8(property_tag(*value));
        }
    }

    fn execution(&mut self, value: &ProofCapsuleExecutionV1) {
        self.bytes
            .extend_from_slice(value.correlation_id.as_bytes());
        self.digest(value.canonical_invocation_identity);
        self.digest(value.policy_identity);
        self.digest(value.request_identity);
        self.digest(value.challenge);
        self.digest(value.transcript_identity);
        self.payload(value.sealed_result);
    }

    fn result(&mut self, value: &ProofCapsuleResultV1) {
        self.u8(outcome_tag(value.outcome));
        self.properties(&value.proved_properties);
    }

    fn freshness(&mut self, value: Option<ProofCapsuleFreshnessIdentityV1>) {
        match value {
            None => self.u8(0),
            Some(value) => {
                self.u8(1);
                self.digest(value.proof_binding_identity);
                self.digest(value.challenge);
                self.digest(value.transcript);
                self.digest(value.result);
                self.digest(value.ledger_namespace);
                self.digest(value.previous_ledger_state_identity);
                self.u64(value.ledger_generation);
                self.digest(value.ledger_state_identity);
                self.digest(value.persistent_binding_identity);
            }
        }
    }
}

struct ExecutionParts {
    correlation_id: CorrelationId,
    canonical_invocation_identity: Digest,
    policy_identity: Digest,
    request_identity: Digest,
    challenge: Digest,
    transcript_identity: Digest,
    sealed_result: ProofCapsulePayloadIdentityV1,
}

struct CapsuleReader<'a> {
    original_len: usize,
    remaining: &'a [u8],
    last_field: u16,
}

impl<'a> CapsuleReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            original_len: bytes.len(),
            remaining: bytes,
            last_field: 0,
        }
    }

    const fn consumed_len(&self) -> usize {
        self.original_len - self.remaining.len()
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    const fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProofCapsuleDecodeErrorV1> {
        if self.remaining.len() < count {
            return Err(ProofCapsuleDecodeErrorV1::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProofCapsuleDecodeErrorV1> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, ProofCapsuleDecodeErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProofCapsuleDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProofCapsuleDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProofCapsuleDecodeErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn length_u32(&mut self, max: usize) -> Result<usize, ProofCapsuleDecodeErrorV1> {
        let value = u64::from(self.u32()?);
        if value > max as u64 {
            Err(ProofCapsuleDecodeErrorV1::LengthOutOfRange { value, max })
        } else {
            Ok(value as usize)
        }
    }

    fn field(&mut self, expected: u16) -> Result<(), ProofCapsuleDecodeErrorV1> {
        let actual = self.u16()?;
        if actual == 0 || actual > LAST_FIELD_TAG {
            return Err(ProofCapsuleDecodeErrorV1::UnknownField(actual));
        }
        if actual <= self.last_field {
            return Err(ProofCapsuleDecodeErrorV1::DuplicateField(actual));
        }
        if actual != expected {
            return Err(ProofCapsuleDecodeErrorV1::NonCanonicalFieldOrder { expected, actual });
        }
        self.last_field = actual;
        Ok(())
    }

    fn digest(&mut self) -> Result<Digest, ProofCapsuleDecodeErrorV1> {
        Ok(Digest::from_bytes(self.array()?))
    }

    fn text(&mut self, field: &'static str) -> Result<Text, ProofCapsuleDecodeErrorV1> {
        let len = usize::from(self.u16()?);
        if len == 0 || len > crate::MAX_TEXT_BYTES {
            return Err(ProofCapsuleDecodeErrorV1::TextLengthOutOfRange {
                field,
                max: crate::MAX_TEXT_BYTES,
            });
        }
        let bytes = self.take(len)?;
        let value = std::str::from_utf8(bytes)
            .map_err(|_| ProofCapsuleDecodeErrorV1::InvalidUtf8 { field })?;
        Text::new(field, value.to_owned())
            .map_err(ProofCapsuleBuildErrorV1::Model)
            .map_err(ProofCapsuleDecodeErrorV1::Build)
    }

    fn identifier(&mut self, field: &'static str) -> Result<Text, ProofCapsuleDecodeErrorV1> {
        let value = self.text(field)?;
        Text::identifier(field, value.as_str().to_owned())
            .map_err(ProofCapsuleBuildErrorV1::Model)
            .map_err(ProofCapsuleDecodeErrorV1::Build)
    }

    fn payload(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<ProofCapsulePayloadIdentityV1, ProofCapsuleDecodeErrorV1> {
        ProofCapsulePayloadIdentityV1::new_bounded(self.u64()?, self.digest()?, field, max)
            .map_err(Into::into)
    }

    fn reserve_collection<T>(
        &self,
        count: usize,
        minimum_item_bytes: usize,
        field: &'static str,
    ) -> Result<Vec<T>, ProofCapsuleDecodeErrorV1> {
        let minimum = count
            .checked_mul(minimum_item_bytes)
            .ok_or(ProofCapsuleDecodeErrorV1::CollectionMinimumBytesOverflow { field })?;
        let remaining = self.remaining_len();
        if minimum > remaining {
            return Err(ProofCapsuleDecodeErrorV1::TruncatedCollection {
                field,
                minimum,
                remaining,
            });
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| ProofCapsuleDecodeErrorV1::CollectionAllocationFailed { field, count })?;
        Ok(values)
    }

    fn proof_target(&mut self) -> Result<ProofTargetIdentity, ProofCapsuleDecodeErrorV1> {
        Ok(ProofTargetIdentity {
            kernel_id: self.digest()?,
            instance_digest: self.digest()?,
            source_tree_digest: self.digest()?,
            crate_graph_digest: self.digest()?,
            executable_digest: self.digest()?,
            environment_digest: self.digest()?,
            artifact_selection_digest: self.digest()?,
            artifact_contract_digest: self.digest()?,
            memory_contract_digest: self.digest()?,
            effects_contract_digest: self.digest()?,
            type_layout_digest: self.digest()?,
            capability_semantics_digest: self.digest()?,
            functional_specification_digest: self.digest()?,
        })
    }

    fn dependencies(&mut self) -> Result<Vec<ProofCapsuleDependencyV1>, ProofCapsuleDecodeErrorV1> {
        let count = usize::from(self.u16()?);
        if count > MAX_PROOF_CAPSULE_DEPENDENCIES_V1 {
            return Err(ProofCapsuleDecodeErrorV1::CountOutOfRange {
                field: "dependencies",
                value: count,
                max: MAX_PROOF_CAPSULE_DEPENDENCIES_V1,
            });
        }
        let mut values =
            self.reserve_collection(count, MIN_NAMED_DIGEST_WIRE_BYTES, "dependencies")?;
        for _ in 0..count {
            let name = self.identifier("dependency name")?;
            let identity = self.digest()?;
            values.push(ProofCapsuleDependencyV1::new(name.as_str(), identity)?);
        }
        require_canonical_dependency_order(&values)?;
        Ok(values)
    }

    fn features(&mut self) -> Result<Vec<Text>, ProofCapsuleDecodeErrorV1> {
        let count = usize::from(self.u16()?);
        if count > MAX_PROOF_CAPSULE_FEATURES_V1 {
            return Err(ProofCapsuleDecodeErrorV1::CountOutOfRange {
                field: "features",
                value: count,
                max: MAX_PROOF_CAPSULE_FEATURES_V1,
            });
        }
        let mut values = self.reserve_collection(count, MIN_IDENTIFIER_WIRE_BYTES, "features")?;
        for _ in 0..count {
            values.push(self.identifier("feature")?);
        }
        require_canonical_text_order(&values, "features")?;
        Ok(values)
    }

    fn model(&mut self) -> Result<VerificationModelIdentity, ProofCapsuleDecodeErrorV1> {
        let version = self.text("verification model version")?;
        VerificationModelIdentity::new(version.as_str(), self.digest()?)
            .map_err(ProofCapsuleBuildErrorV1::Model)
            .map_err(Into::into)
    }

    fn tool(&mut self) -> Result<MeasuredToolIdentity, ProofCapsuleDecodeErrorV1> {
        let name = self.text("tool name")?;
        let version = self.text("tool version")?;
        MeasuredToolIdentity::new(
            name.as_str(),
            version.as_str(),
            self.digest()?,
            self.digest()?,
        )
        .map_err(ProofCapsuleBuildErrorV1::Model)
        .map_err(Into::into)
    }

    fn trusted_items(
        &mut self,
        field: &'static str,
    ) -> Result<Vec<TrustedItem>, ProofCapsuleDecodeErrorV1> {
        let count = usize::from(self.u16()?);
        if count > crate::MAX_TRUSTED_ITEMS {
            return Err(ProofCapsuleDecodeErrorV1::CountOutOfRange {
                field,
                value: count,
                max: crate::MAX_TRUSTED_ITEMS,
            });
        }
        let mut values = self.reserve_collection(count, MIN_NAMED_DIGEST_WIRE_BYTES, field)?;
        for _ in 0..count {
            let name = self.identifier("trusted item name")?;
            values.push(
                TrustedItem::new(name.as_str(), self.digest()?)
                    .map_err(ProofCapsuleBuildErrorV1::Model)?,
            );
        }
        require_canonical_trusted_order(&values, field)?;
        Ok(values)
    }

    fn properties(
        &mut self,
        allow_empty: bool,
    ) -> Result<Vec<ProofProperty>, ProofCapsuleDecodeErrorV1> {
        let count = usize::from(self.u16()?);
        if count > crate::MAX_PROPERTIES || (!allow_empty && count == 0) {
            return Err(ProofCapsuleDecodeErrorV1::CountOutOfRange {
                field: "proof properties",
                value: count,
                max: crate::MAX_PROPERTIES,
            });
        }
        let mut values = self.reserve_collection(count, 1, "proof properties")?;
        for _ in 0..count {
            values.push(parse_property_tag(self.u8()?)?);
        }
        require_canonical_property_order(&values)?;
        Ok(values)
    }

    fn execution(&mut self) -> Result<ExecutionParts, ProofCapsuleDecodeErrorV1> {
        Ok(ExecutionParts {
            correlation_id: CorrelationId::from_bytes(self.array()?),
            canonical_invocation_identity: self.digest()?,
            policy_identity: self.digest()?,
            request_identity: self.digest()?,
            challenge: self.digest()?,
            transcript_identity: self.digest()?,
            sealed_result: self.payload(
                "sealed proof result",
                MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1,
            )?,
        })
    }

    fn result(&mut self) -> Result<ProofCapsuleResultV1, ProofCapsuleDecodeErrorV1> {
        let outcome = parse_outcome_tag(self.u8()?)?;
        let properties = self.properties(outcome != ProofOutcome::Proved)?;
        ProofCapsuleResultV1::new(outcome, properties).map_err(Into::into)
    }

    fn freshness(
        &mut self,
    ) -> Result<Option<ProofCapsuleFreshnessIdentityV1>, ProofCapsuleDecodeErrorV1> {
        match self.u8()? {
            0 => Ok(None),
            1 => Ok(Some(ProofCapsuleFreshnessIdentityV1::new_inert(
                self.digest()?,
                self.digest()?,
                self.digest()?,
                self.digest()?,
                self.digest()?,
                self.digest()?,
                self.u64()?,
                self.digest()?,
                self.digest()?,
            )?)),
            tag => Err(ProofCapsuleDecodeErrorV1::UnknownFreshnessKind(tag)),
        }
    }
}

fn require_canonical_dependency_order(
    values: &[ProofCapsuleDependencyV1],
) -> Result<(), ProofCapsuleDecodeErrorV1> {
    for pair in values.windows(2) {
        match pair[0].name.cmp(&pair[1].name) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(ProofCapsuleDecodeErrorV1::DuplicateItem {
                    field: "dependency name",
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(ProofCapsuleDecodeErrorV1::NonCanonicalCollection {
                    field: "dependencies",
                });
            }
        }
    }
    Ok(())
}

fn require_canonical_text_order(
    values: &[Text],
    field: &'static str,
) -> Result<(), ProofCapsuleDecodeErrorV1> {
    for pair in values.windows(2) {
        match pair[0].cmp(&pair[1]) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(ProofCapsuleDecodeErrorV1::DuplicateItem { field });
            }
            std::cmp::Ordering::Greater => {
                return Err(ProofCapsuleDecodeErrorV1::NonCanonicalCollection { field });
            }
        }
    }
    Ok(())
}

fn require_canonical_trusted_order(
    values: &[TrustedItem],
    field: &'static str,
) -> Result<(), ProofCapsuleDecodeErrorV1> {
    for pair in values.windows(2) {
        match pair[0].name().cmp(pair[1].name()) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(ProofCapsuleDecodeErrorV1::DuplicateItem { field });
            }
            std::cmp::Ordering::Greater => {
                return Err(ProofCapsuleDecodeErrorV1::NonCanonicalCollection { field });
            }
        }
    }
    Ok(())
}

fn require_canonical_property_order(
    values: &[ProofProperty],
) -> Result<(), ProofCapsuleDecodeErrorV1> {
    for pair in values.windows(2) {
        match pair[0].cmp(&pair[1]) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => {
                return Err(ProofCapsuleDecodeErrorV1::DuplicateItem {
                    field: "proof property",
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(ProofCapsuleDecodeErrorV1::NonCanonicalCollection {
                    field: "proof properties",
                });
            }
        }
    }
    Ok(())
}

const fn property_tag(value: ProofProperty) -> u8 {
    match value {
        ProofProperty::Bounds => 1,
        ProofProperty::AddressOverflowFreedom => 2,
        ProofProperty::MemorySafety => 3,
        ProofProperty::Initialization => 4,
        ProofProperty::RaceFreedom => 5,
        ProofProperty::LaunchValidity => 6,
        ProofProperty::FunctionalCorrectness => 7,
    }
}

fn parse_property_tag(value: u8) -> Result<ProofProperty, ProofCapsuleDecodeErrorV1> {
    match value {
        1 => Ok(ProofProperty::Bounds),
        2 => Ok(ProofProperty::AddressOverflowFreedom),
        3 => Ok(ProofProperty::MemorySafety),
        4 => Ok(ProofProperty::Initialization),
        5 => Ok(ProofProperty::RaceFreedom),
        6 => Ok(ProofProperty::LaunchValidity),
        7 => Ok(ProofProperty::FunctionalCorrectness),
        tag => Err(ProofCapsuleDecodeErrorV1::UnknownProperty(tag)),
    }
}

const fn outcome_tag(value: ProofOutcome) -> u8 {
    match value {
        ProofOutcome::Proved => 1,
        ProofOutcome::Failed => 2,
        ProofOutcome::TimedOut => 3,
    }
}

fn parse_outcome_tag(value: u8) -> Result<ProofOutcome, ProofCapsuleDecodeErrorV1> {
    match value {
        1 => Ok(ProofOutcome::Proved),
        2 => Ok(ProofOutcome::Failed),
        3 => Ok(ProofOutcome::TimedOut),
        tag => Err(ProofCapsuleDecodeErrorV1::UnknownOutcome(tag)),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofCapsuleBuildErrorV1 {
    Model(ModelError),
    TooLarge {
        max: usize,
    },
    TooManyItems {
        field: &'static str,
        max: usize,
    },
    PropertyCountOutOfRange {
        min: usize,
        max: usize,
    },
    DuplicateItem {
        field: &'static str,
    },
    ZeroIdentity {
        field: &'static str,
    },
    ZeroCorrelation,
    ZeroLedgerGeneration,
    PayloadLengthOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    FreshnessMismatch {
        field: &'static str,
    },
    ClaimsOnIncompleteProof,
    IncompleteProof,
    MissingPersistentFreshness,
    UnexpectedPersistentFreshness,
    PolicyIdentityMismatch,
    ProofTargetMismatch,
    FinalizedPayloadMismatch,
    VerifierPolicyMismatch,
}

impl fmt::Display for ProofCapsuleBuildErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => write!(formatter, "invalid proof capsule model: {error}"),
            Self::TooLarge { max } => write!(formatter, "proof capsule exceeds {max} bytes"),
            Self::TooManyItems { field, max } => {
                write!(formatter, "{field} exceeds the limit of {max}")
            }
            Self::PropertyCountOutOfRange { min, max } => {
                write!(formatter, "proof property count must be in {min}..={max}")
            }
            Self::DuplicateItem { field } => write!(formatter, "duplicate {field}"),
            Self::ZeroIdentity { field } => write!(formatter, "{field} must not be zero"),
            Self::ZeroCorrelation => formatter.write_str("correlation ID must not be zero"),
            Self::ZeroLedgerGeneration => formatter.write_str("ledger generation must not be zero"),
            Self::PayloadLengthOutOfRange { field, value, max } => {
                write!(formatter, "{field} length {value} must be in 1..={max}")
            }
            Self::FreshnessMismatch { field } => {
                write!(
                    formatter,
                    "persistent freshness {field} does not match execution"
                )
            }
            Self::ClaimsOnIncompleteProof => {
                formatter.write_str("failed or timed-out result carries proved-property claims")
            }
            Self::IncompleteProof => {
                formatter.write_str("proved result does not claim exactly every requested property")
            }
            Self::MissingPersistentFreshness => {
                formatter.write_str("proved result lacks persistent one-history freshness")
            }
            Self::UnexpectedPersistentFreshness => formatter
                .write_str("failed or timed-out result must not claim persistent proof freshness"),
            Self::PolicyIdentityMismatch => {
                formatter.write_str("verifier policy identity does not match sealed execution")
            }
            Self::ProofTargetMismatch => {
                formatter.write_str("capsule proof target does not match proof result")
            }
            Self::FinalizedPayloadMismatch => formatter
                .write_str("capsule finalized payload does not match proof/executable binding"),
            Self::VerifierPolicyMismatch => formatter
                .write_str("verifier policy model or tool identities do not match execution"),
        }
    }
}

impl std::error::Error for ProofCapsuleBuildErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofCapsuleDecodeErrorV1 {
    TooLarge {
        max: usize,
    },
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    LengthOutOfRange {
        value: u64,
        max: usize,
    },
    CountOutOfRange {
        field: &'static str,
        value: usize,
        max: usize,
    },
    CollectionMinimumBytesOverflow {
        field: &'static str,
    },
    TruncatedCollection {
        field: &'static str,
        minimum: usize,
        remaining: usize,
    },
    CollectionAllocationFailed {
        field: &'static str,
        count: usize,
    },
    TextLengthOutOfRange {
        field: &'static str,
        max: usize,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    Truncated,
    TrailingBytes,
    UnknownField(u16),
    DuplicateField(u16),
    NonCanonicalFieldOrder {
        expected: u16,
        actual: u16,
    },
    DuplicateItem {
        field: &'static str,
    },
    NonCanonicalCollection {
        field: &'static str,
    },
    UnknownProperty(u8),
    UnknownOutcome(u8),
    UnknownFreshnessKind(u8),
    IdentityMismatch,
    NonCanonical,
    Build(ProofCapsuleBuildErrorV1),
}

impl fmt::Display for ProofCapsuleDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "proof capsule exceeds {max} bytes"),
            Self::InvalidMagic => formatter.write_str("invalid proof capsule magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unsupported proof capsule version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported proof capsule flags {flags:#x}")
            }
            Self::LengthOutOfRange { value, max } => {
                write!(formatter, "proof capsule length {value} exceeds {max}")
            }
            Self::CountOutOfRange { field, value, max } => {
                write!(formatter, "{field} count {value} exceeds {max}")
            }
            Self::CollectionMinimumBytesOverflow { field } => {
                write!(formatter, "{field} minimum encoded length overflowed")
            }
            Self::TruncatedCollection {
                field,
                minimum,
                remaining,
            } => write!(
                formatter,
                "{field} requires at least {minimum} bytes but only {remaining} remain"
            ),
            Self::CollectionAllocationFailed { field, count } => {
                write!(formatter, "cannot reserve {count} {field} entries")
            }
            Self::TextLengthOutOfRange { field, max } => {
                write!(formatter, "{field} text length must be in 1..={max}")
            }
            Self::InvalidUtf8 { field } => write!(formatter, "{field} is not UTF-8"),
            Self::Truncated => formatter.write_str("proof capsule is truncated"),
            Self::TrailingBytes => formatter.write_str("proof capsule has trailing bytes"),
            Self::UnknownField(field) => write!(formatter, "unknown proof capsule field {field}"),
            Self::DuplicateField(field) => {
                write!(formatter, "duplicate proof capsule field {field}")
            }
            Self::NonCanonicalFieldOrder { expected, actual } => write!(
                formatter,
                "proof capsule field {actual} appears where field {expected} is required"
            ),
            Self::DuplicateItem { field } => write!(formatter, "duplicate {field}"),
            Self::NonCanonicalCollection { field } => {
                write!(formatter, "{field} are not in canonical order")
            }
            Self::UnknownProperty(tag) => write!(formatter, "unknown proof property tag {tag}"),
            Self::UnknownOutcome(tag) => write!(formatter, "unknown proof outcome tag {tag}"),
            Self::UnknownFreshnessKind(tag) => {
                write!(formatter, "unknown proof freshness kind {tag}")
            }
            Self::IdentityMismatch => formatter.write_str("proof capsule identity mismatch"),
            Self::NonCanonical => formatter.write_str("proof capsule is not canonical"),
            Self::Build(error) => write!(formatter, "invalid proof capsule: {error}"),
        }
    }
}

impl std::error::Error for ProofCapsuleDecodeErrorV1 {}

impl From<ProofCapsuleBuildErrorV1> for ProofCapsuleDecodeErrorV1 {
    fn from(value: ProofCapsuleBuildErrorV1) -> Self {
        Self::Build(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofCapsuleIdentityFieldV1 {
    Capsule,
    Artifact,
    Freshness,
    LedgerNamespace,
    PreviousLedgerState,
    LedgerState,
    ProofBinding,
    Challenge,
    Transcript,
    Result,
    PersistentBinding,
}

impl ProofCapsuleIdentityFieldV1 {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Capsule => "capsule",
            Self::Artifact => "artifact",
            Self::Freshness => "freshness kind",
            Self::LedgerNamespace => "ledger namespace",
            Self::PreviousLedgerState => "previous ledger state",
            Self::LedgerState => "ledger state",
            Self::ProofBinding => "proof binding",
            Self::Challenge => "challenge",
            Self::Transcript => "transcript",
            Self::Result => "result",
            Self::PersistentBinding => "persistent binding",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofCapsuleContextErrorV1 {
    Decode(ProofCapsuleDecodeErrorV1),
    IdentitySubstitution { field: ProofCapsuleIdentityFieldV1 },
    StaleLedgerGeneration { expected: u64, actual: u64 },
    UnexpectedLedgerGeneration { expected: u64, actual: u64 },
    CapsuleDuplicate,
    ExecutionDuplicate,
    PersistentProofDuplicate,
}

impl fmt::Display for ProofCapsuleContextErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => write!(formatter, "cannot decode proof capsule: {error}"),
            Self::IdentitySubstitution { field } => {
                write!(formatter, "{} identity substitution", field.as_str())
            }
            Self::StaleLedgerGeneration { expected, actual } => write!(
                formatter,
                "stale ledger generation {actual}; expected {expected}"
            ),
            Self::UnexpectedLedgerGeneration { expected, actual } => write!(
                formatter,
                "unexpected future ledger generation {actual}; expected {expected}"
            ),
            Self::CapsuleDuplicate => {
                formatter.write_str("proof capsule was already recorded in this process")
            }
            Self::ExecutionDuplicate => {
                formatter.write_str("proof execution was already recorded in this process")
            }
            Self::PersistentProofDuplicate => {
                formatter.write_str("persistent proof was already recorded in this process")
            }
        }
    }
}

impl std::error::Error for ProofCapsuleContextErrorV1 {}

impl From<ProofCapsuleDecodeErrorV1> for ProofCapsuleContextErrorV1 {
    fn from(value: ProofCapsuleDecodeErrorV1) -> Self {
        Self::Decode(value)
    }
}

#[cfg(test)]
mod capsule_regression_tests {
    use fe2o3_artifacts::{DigestBytes, PayloadDigest};

    use super::*;

    #[test]
    fn persistent_projection_rejects_finalized_payload_substitution() {
        let capsule =
            ProofCapsulePayloadIdentityV1::finalized_hsaco(4096, Digest::from_bytes([0x41; 32]))
                .unwrap();
        let substituted =
            PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([0x42; 32]));
        let exact =
            PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([0x41; 32]));

        assert_eq!(require_bound_finalized_payload(capsule, exact), Ok(()));
        assert_eq!(
            require_bound_finalized_payload(capsule, substituted),
            Err(ProofCapsuleBuildErrorV1::FinalizedPayloadMismatch)
        );
    }
}
