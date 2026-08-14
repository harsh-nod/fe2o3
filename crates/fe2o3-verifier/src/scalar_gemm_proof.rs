//! Exact, bounded proof-profile review for Scalar GEMM V1 on gfx942.
//!
//! This module detects substitution and process-local replay across an inert
//! proof capsule. It does not authenticate Verus or Z3 execution, establish
//! compiler refinement, or grant proof, load, or launch authority.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifacts::DigestAlgorithm;

use crate::{
    Digest, MAX_PROOF_CAPSULE_DEPENDENCIES_V1, MAX_TRUSTED_ITEMS, MeasuredToolIdentity,
    ProofCapsuleDependencyV1, ProofCapsuleFreshnessIdentityV1, ProofCapsuleV1, ProofOutcome,
    ProofProperty, ProofTargetIdentity, TrustedItem, VerificationModelIdentity,
};

pub const SCALAR_GEMM_PROOF_DOMAIN_V1: [u8; 8] = *b"FE2SGP1\0";
pub const SCALAR_GEMM_PROOF_REVIEW_DOMAIN_V1: [u8; 8] = *b"FE2SGR1\0";
pub const SCALAR_GEMM_PROOF_VERSION_V1: u16 = 1;
pub const SCALAR_GEMM_PROOF_SOURCE_PATH_V1: &str = "verus/scalar_gemm_v1.rs";
pub const SCALAR_GEMM_PROOF_MODEL_VERSION_V1: &str = "scalar-gemm-source-v1";
pub const SCALAR_GEMM_PROOF_TARGET_V1: &str = "gfx942:xnack-";
pub const MAX_SCALAR_GEMM_PROOF_SOURCE_BYTES_V1: usize = 256 * 1024;
pub const MAX_SCALAR_GEMM_PROOF_REVIEWS_V1: usize = 4096;

pub const SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1: [ProofProperty; 7] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::MemorySafety,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::LaunchValidity,
    ProofProperty::FunctionalCorrectness,
];

/// Identity of the one fixed Verus proof source admitted by this profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmProofSourceV1 {
    byte_len: u64,
    content_identity: Digest,
    identity: Digest,
}

impl ScalarGemmProofSourceV1 {
    pub fn measure(bytes: &[u8]) -> Result<Self, ScalarGemmProofErrorV1> {
        if bytes.is_empty() || bytes.len() > MAX_SCALAR_GEMM_PROOF_SOURCE_BYTES_V1 {
            return Err(ScalarGemmProofErrorV1::SourceLengthOutOfRange {
                max: MAX_SCALAR_GEMM_PROOF_SOURCE_BYTES_V1,
            });
        }
        let byte_len = bytes.len() as u64;
        let content_identity = sha256(bytes);
        let mut identity_bytes = Vec::with_capacity(96);
        identity_bytes.extend_from_slice(b"FE2SGS1\0");
        put_text(&mut identity_bytes, SCALAR_GEMM_PROOF_SOURCE_PATH_V1);
        identity_bytes.extend_from_slice(&byte_len.to_le_bytes());
        put_digest(&mut identity_bytes, content_identity);
        Ok(Self {
            byte_len,
            content_identity,
            identity: sha256(&identity_bytes),
        })
    }

    pub const fn path(&self) -> &'static str {
        SCALAR_GEMM_PROOF_SOURCE_PATH_V1
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn content_identity(&self) -> Digest {
        self.content_identity
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }
}

/// Exact expectations for one Scalar GEMM V1 proof capsule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarGemmProofProfileV1 {
    source: ScalarGemmProofSourceV1,
    proof_target: ProofTargetIdentity,
    dependencies: Vec<ProofCapsuleDependencyV1>,
    abi_identity: Digest,
    effects_identity: Digest,
    launch_identity: Digest,
    machine_effect_evidence_identity: Digest,
    finalized_artifact_identity: Digest,
    artifact_identity: Digest,
    verus: MeasuredToolIdentity,
    solver: MeasuredToolIdentity,
    model: VerificationModelIdentity,
    trusted_items: Vec<TrustedItem>,
    transcript_identity: Digest,
    result_identity: Digest,
    identity: Digest,
}

impl ScalarGemmProofProfileV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn seal(
        source: ScalarGemmProofSourceV1,
        proof_target: ProofTargetIdentity,
        mut dependencies: Vec<ProofCapsuleDependencyV1>,
        abi_identity: Digest,
        effects_identity: Digest,
        launch_identity: Digest,
        machine_effect_evidence_identity: Digest,
        finalized_artifact_identity: Digest,
        artifact_identity: Digest,
        verus: MeasuredToolIdentity,
        solver: MeasuredToolIdentity,
        model: VerificationModelIdentity,
        mut trusted_items: Vec<TrustedItem>,
        transcript_identity: Digest,
        result_identity: Digest,
    ) -> Result<Self, ScalarGemmProofErrorV1> {
        for (field, identity) in [
            ("ABI", abi_identity),
            ("effects", effects_identity),
            ("launch", launch_identity),
            ("machine-effect evidence", machine_effect_evidence_identity),
            ("finalized artifact", finalized_artifact_identity),
            ("artifact", artifact_identity),
            ("Verus transcript", transcript_identity),
            ("proof result", result_identity),
            ("Verus executable", verus.executable_digest()),
            ("Verus configuration", verus.configuration_digest()),
            ("solver executable", solver.executable_digest()),
            ("solver configuration", solver.configuration_digest()),
            ("model axioms", model.axioms_digest()),
        ] {
            require_nonzero(identity, field)?;
        }
        for identity in proof_target.digests() {
            require_nonzero(identity, "proof target")?;
        }
        if proof_target.source_tree_digest != source.identity() {
            return Err(ScalarGemmProofErrorV1::IdentityMismatch {
                field: "scalar proof source",
            });
        }
        if proof_target.effects_contract_digest != effects_identity {
            return Err(ScalarGemmProofErrorV1::IdentityMismatch { field: "effects" });
        }
        if verus.name().as_str() != "verus" || solver.name().as_str() != "z3" {
            return Err(ScalarGemmProofErrorV1::UnexpectedTool);
        }
        if model.version().as_str() != SCALAR_GEMM_PROOF_MODEL_VERSION_V1 {
            return Err(ScalarGemmProofErrorV1::UnexpectedModel);
        }
        canonicalize_dependencies(&mut dependencies)?;
        canonicalize_trusted_items(&mut trusted_items)?;

        let mut profile = Self {
            source,
            proof_target,
            dependencies,
            abi_identity,
            effects_identity,
            launch_identity,
            machine_effect_evidence_identity,
            finalized_artifact_identity,
            artifact_identity,
            verus,
            solver,
            model,
            trusted_items,
            transcript_identity,
            result_identity,
            identity: Digest::from_bytes([0; 32]),
        };
        profile.identity = sha256(&profile.identity_bytes());
        Ok(profile)
    }

    pub const fn source(&self) -> ScalarGemmProofSourceV1 {
        self.source
    }

    pub const fn proof_target(&self) -> ProofTargetIdentity {
        self.proof_target
    }

    pub fn dependencies(&self) -> &[ProofCapsuleDependencyV1] {
        &self.dependencies
    }

    pub const fn abi_identity(&self) -> Digest {
        self.abi_identity
    }

    pub const fn effects_identity(&self) -> Digest {
        self.effects_identity
    }

    pub const fn launch_identity(&self) -> Digest {
        self.launch_identity
    }

    pub const fn finalized_artifact_identity(&self) -> Digest {
        self.finalized_artifact_identity
    }

    pub const fn artifact_identity(&self) -> Digest {
        self.artifact_identity
    }

    pub const fn transcript_identity(&self) -> Digest {
        self.transcript_identity
    }

    pub const fn result_identity(&self) -> Digest {
        self.result_identity
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn required_properties(&self) -> &[ProofProperty; 7] {
        &SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1
    }

    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn has_complete_source_closure(&self) -> bool {
        false
    }

    pub const fn has_complete_verifier_runtime_closure(&self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1024);
        bytes.extend_from_slice(&SCALAR_GEMM_PROOF_DOMAIN_V1);
        bytes.extend_from_slice(&SCALAR_GEMM_PROOF_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        put_text(&mut bytes, SCALAR_GEMM_PROOF_TARGET_V1);
        put_text(&mut bytes, self.source.path());
        bytes.extend_from_slice(&self.source.byte_len.to_le_bytes());
        put_digest(&mut bytes, self.source.content_identity);
        put_digest(&mut bytes, self.source.identity);
        for identity in self.proof_target.digests() {
            put_digest(&mut bytes, identity);
        }
        bytes.extend_from_slice(&(self.dependencies.len() as u16).to_le_bytes());
        for dependency in &self.dependencies {
            put_text(&mut bytes, dependency.name().as_str());
            put_digest(&mut bytes, dependency.identity());
        }
        for identity in [
            self.abi_identity,
            self.effects_identity,
            self.launch_identity,
            self.machine_effect_evidence_identity,
            self.finalized_artifact_identity,
            self.artifact_identity,
        ] {
            put_digest(&mut bytes, identity);
        }
        put_tool(&mut bytes, &self.verus);
        put_tool(&mut bytes, &self.solver);
        put_text(&mut bytes, self.model.version().as_str());
        put_digest(&mut bytes, self.model.axioms_digest());
        bytes.extend_from_slice(&(self.trusted_items.len() as u16).to_le_bytes());
        for item in &self.trusted_items {
            put_text(&mut bytes, item.name().as_str());
            put_digest(&mut bytes, item.contract_digest());
        }
        for property in SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1 {
            bytes.push(property_tag(property));
        }
        put_digest(&mut bytes, self.transcript_identity);
        put_digest(&mut bytes, self.result_identity);
        bytes
    }
}

/// Caller expectations for one exact review decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarGemmProofReviewV1 {
    profile_identity: Digest,
    capsule_identity: Digest,
    freshness: ProofCapsuleFreshnessIdentityV1,
    reviewer_policy_identity: Digest,
    review_nonce: Digest,
}

impl ScalarGemmProofReviewV1 {
    pub fn new(
        profile_identity: Digest,
        capsule_identity: Digest,
        freshness: ProofCapsuleFreshnessIdentityV1,
        reviewer_policy_identity: Digest,
        review_nonce: Digest,
    ) -> Result<Self, ScalarGemmProofErrorV1> {
        for (field, identity) in [
            ("reviewed profile", profile_identity),
            ("reviewed capsule", capsule_identity),
            ("reviewer policy", reviewer_policy_identity),
            ("review nonce", review_nonce),
        ] {
            require_nonzero(identity, field)?;
        }
        Ok(Self {
            profile_identity,
            capsule_identity,
            freshness,
            reviewer_policy_identity,
            review_nonce,
        })
    }
}

/// Bounded process-local duplicate detector layered over persistent freshness.
#[derive(Debug)]
pub struct ScalarGemmProofReviewLedgerV1 {
    max_records: usize,
    profile_identities: BTreeSet<Digest>,
    capsule_identities: BTreeSet<Digest>,
    proof_bindings: BTreeSet<Digest>,
    challenges: BTreeSet<Digest>,
    transcripts: BTreeSet<Digest>,
    results: BTreeSet<Digest>,
    persistent_bindings: BTreeSet<Digest>,
    review_nonces: BTreeSet<Digest>,
}

impl ScalarGemmProofReviewLedgerV1 {
    pub const fn new() -> Self {
        Self {
            max_records: MAX_SCALAR_GEMM_PROOF_REVIEWS_V1,
            profile_identities: BTreeSet::new(),
            capsule_identities: BTreeSet::new(),
            proof_bindings: BTreeSet::new(),
            challenges: BTreeSet::new(),
            transcripts: BTreeSet::new(),
            results: BTreeSet::new(),
            persistent_bindings: BTreeSet::new(),
            review_nonces: BTreeSet::new(),
        }
    }

    pub fn with_max_records(max_records: usize) -> Result<Self, ScalarGemmProofErrorV1> {
        if max_records == 0 || max_records > MAX_SCALAR_GEMM_PROOF_REVIEWS_V1 {
            return Err(ScalarGemmProofErrorV1::ReviewCapacityOutOfRange);
        }
        Ok(Self {
            max_records,
            ..Self::new()
        })
    }

    pub fn recorded_count(&self) -> usize {
        self.profile_identities.len()
    }

    fn check(
        &self,
        profile: &ScalarGemmProofProfileV1,
        capsule: &ProofCapsuleV1,
        freshness: ProofCapsuleFreshnessIdentityV1,
        review_nonce: Digest,
    ) -> Result<(), ScalarGemmProofErrorV1> {
        if self.recorded_count() >= self.max_records {
            return Err(ScalarGemmProofErrorV1::ReviewCapacityReached);
        }
        if self.profile_identities.contains(&profile.identity)
            || self.capsule_identities.contains(&capsule.identity())
            || self
                .proof_bindings
                .contains(&freshness.proof_binding_identity())
            || self.challenges.contains(&freshness.challenge())
            || self.transcripts.contains(&freshness.transcript())
            || self.results.contains(&freshness.result())
            || self
                .persistent_bindings
                .contains(&freshness.persistent_binding_identity())
            || self.review_nonces.contains(&review_nonce)
        {
            return Err(ScalarGemmProofErrorV1::Replay);
        }
        Ok(())
    }

    fn record(
        &mut self,
        profile: &ScalarGemmProofProfileV1,
        capsule: &ProofCapsuleV1,
        freshness: ProofCapsuleFreshnessIdentityV1,
        review_nonce: Digest,
    ) {
        self.profile_identities.insert(profile.identity);
        self.capsule_identities.insert(capsule.identity());
        self.proof_bindings
            .insert(freshness.proof_binding_identity());
        self.challenges.insert(freshness.challenge());
        self.transcripts.insert(freshness.transcript());
        self.results.insert(freshness.result());
        self.persistent_bindings
            .insert(freshness.persistent_binding_identity());
        self.review_nonces.insert(review_nonce);
    }
}

impl Default for ScalarGemmProofReviewLedgerV1 {
    fn default() -> Self {
        Self::new()
    }
}

/// Linear, inert record that one capsule matched one exact scalar profile.
#[derive(Debug, Eq, PartialEq)]
pub struct ReviewedScalarGemmProofV1 {
    profile_identity: Digest,
    capsule_identity: Digest,
    source_identity: Digest,
    proof_target: ProofTargetIdentity,
    transcript_identity: Digest,
    result_identity: Digest,
    finalized_artifact_identity: Digest,
    artifact_identity: Digest,
    freshness: ProofCapsuleFreshnessIdentityV1,
    reviewer_policy_identity: Digest,
    review_nonce: Digest,
    identity: Digest,
}

impl ReviewedScalarGemmProofV1 {
    pub const fn profile_identity(&self) -> Digest {
        self.profile_identity
    }

    pub const fn source_identity(&self) -> Digest {
        self.source_identity
    }

    pub const fn proof_target(&self) -> ProofTargetIdentity {
        self.proof_target
    }

    pub const fn transcript_identity(&self) -> Digest {
        self.transcript_identity
    }

    pub const fn result_identity(&self) -> Digest {
        self.result_identity
    }

    pub const fn finalized_artifact_identity(&self) -> Digest {
        self.finalized_artifact_identity
    }

    pub const fn artifact_identity(&self) -> Digest {
        self.artifact_identity
    }

    pub const fn freshness(&self) -> ProofCapsuleFreshnessIdentityV1 {
        self.freshness
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn reported_properties(&self) -> &[ProofProperty; 7] {
        &SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1
    }

    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn has_complete_source_closure(&self) -> bool {
        false
    }

    pub const fn has_complete_verifier_runtime_closure(&self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

pub fn review_scalar_gemm_proof_v1(
    profile: &ScalarGemmProofProfileV1,
    capsule: &ProofCapsuleV1,
    review: ScalarGemmProofReviewV1,
    ledger: &mut ScalarGemmProofReviewLedgerV1,
) -> Result<ReviewedScalarGemmProofV1, ScalarGemmProofErrorV1> {
    if review.profile_identity != profile.identity {
        return Err(ScalarGemmProofErrorV1::IdentityMismatch {
            field: "reviewed profile",
        });
    }
    if review.capsule_identity != capsule.identity() {
        return Err(ScalarGemmProofErrorV1::IdentityMismatch {
            field: "reviewed capsule",
        });
    }

    let target = capsule.target();
    if target.features().len() != 1 || target.features()[0].as_str() != SCALAR_GEMM_PROOF_TARGET_V1
    {
        return Err(ScalarGemmProofErrorV1::TargetProfileSubstitution);
    }
    if target.proof_target() != profile.proof_target {
        return Err(ScalarGemmProofErrorV1::IdentityMismatch {
            field: "proof target",
        });
    }
    if target.dependencies() != profile.dependencies {
        return Err(ScalarGemmProofErrorV1::DependencySubstitution);
    }
    for (field, expected, actual) in [
        ("ABI", profile.abi_identity, target.abi_identity()),
        (
            "effects",
            profile.effects_identity,
            target.effects_identity(),
        ),
        ("launch", profile.launch_identity, target.launch_identity()),
        (
            "machine-effect evidence",
            profile.machine_effect_evidence_identity,
            target.machine_effect_evidence_identity(),
        ),
        (
            "finalized artifact",
            profile.finalized_artifact_identity,
            target.finalized_payload_identity(),
        ),
        (
            "artifact",
            profile.artifact_identity,
            target.artifact_identity(),
        ),
    ] {
        if expected != actual {
            return Err(ScalarGemmProofErrorV1::IdentityMismatch { field });
        }
    }

    let policy = capsule.policy();
    if policy.claimed_verifier() != &profile.verus || policy.claimed_solver() != &profile.solver {
        return Err(ScalarGemmProofErrorV1::ToolSubstitution);
    }
    if policy.model() != &profile.model {
        return Err(ScalarGemmProofErrorV1::ModelSubstitution);
    }
    if policy.approved_axioms().allowed() != profile.trusted_items
        || policy.requested_axioms() != profile.trusted_items
    {
        return Err(ScalarGemmProofErrorV1::TrustedInventorySubstitution);
    }
    if capsule.result().outcome() != ProofOutcome::Proved {
        return Err(ScalarGemmProofErrorV1::ProofOutcomeSubstitution);
    }
    if policy.requested_properties() != SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1
        || capsule.result().reported_properties() != SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1
    {
        return Err(ScalarGemmProofErrorV1::PropertySubstitution);
    }

    let execution = capsule.execution();
    if execution.transcript_identity() != profile.transcript_identity {
        return Err(ScalarGemmProofErrorV1::IdentityMismatch {
            field: "Verus transcript",
        });
    }
    if execution.sealed_result().digest() != profile.result_identity {
        return Err(ScalarGemmProofErrorV1::IdentityMismatch {
            field: "proof result",
        });
    }
    let freshness = execution
        .freshness()
        .ok_or(ScalarGemmProofErrorV1::MissingPersistentFreshness)?;
    if freshness != review.freshness {
        return Err(ScalarGemmProofErrorV1::FreshnessSubstitution);
    }
    ledger.check(profile, capsule, freshness, review.review_nonce)?;

    let mut record = ReviewedScalarGemmProofV1 {
        profile_identity: profile.identity,
        capsule_identity: capsule.identity(),
        source_identity: profile.source.identity,
        proof_target: profile.proof_target,
        transcript_identity: profile.transcript_identity,
        result_identity: profile.result_identity,
        finalized_artifact_identity: profile.finalized_artifact_identity,
        artifact_identity: profile.artifact_identity,
        freshness,
        reviewer_policy_identity: review.reviewer_policy_identity,
        review_nonce: review.review_nonce,
        identity: Digest::from_bytes([0; 32]),
    };
    record.identity = reviewed_identity(&record);
    ledger.record(profile, capsule, freshness, review.review_nonce);
    Ok(record)
}

fn canonicalize_dependencies(
    dependencies: &mut [ProofCapsuleDependencyV1],
) -> Result<(), ScalarGemmProofErrorV1> {
    if dependencies.len() > MAX_PROOF_CAPSULE_DEPENDENCIES_V1 {
        return Err(ScalarGemmProofErrorV1::DependencyCapacity);
    }
    dependencies.sort();
    if dependencies
        .windows(2)
        .any(|pair| pair[0].name() == pair[1].name())
    {
        return Err(ScalarGemmProofErrorV1::DuplicateDependency);
    }
    Ok(())
}

fn canonicalize_trusted_items(
    trusted_items: &mut [TrustedItem],
) -> Result<(), ScalarGemmProofErrorV1> {
    if trusted_items.len() > MAX_TRUSTED_ITEMS {
        return Err(ScalarGemmProofErrorV1::TrustedItemCapacity);
    }
    trusted_items.sort();
    if trusted_items
        .windows(2)
        .any(|pair| pair[0].name() == pair[1].name())
    {
        return Err(ScalarGemmProofErrorV1::DuplicateTrustedItem);
    }
    for item in trusted_items.iter() {
        require_nonzero(item.contract_digest(), "trusted item contract")?;
    }
    Ok(())
}

fn reviewed_identity(record: &ReviewedScalarGemmProofV1) -> Digest {
    let mut bytes = Vec::with_capacity(640);
    bytes.extend_from_slice(&SCALAR_GEMM_PROOF_REVIEW_DOMAIN_V1);
    bytes.extend_from_slice(&SCALAR_GEMM_PROOF_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    for identity in [
        record.profile_identity,
        record.capsule_identity,
        record.source_identity,
        record.transcript_identity,
        record.result_identity,
        record.finalized_artifact_identity,
        record.artifact_identity,
        record.freshness.proof_binding_identity(),
        record.freshness.challenge(),
        record.freshness.transcript(),
        record.freshness.result(),
        record.freshness.ledger_namespace(),
        record.freshness.previous_ledger_state_identity(),
        record.freshness.ledger_state_identity(),
        record.freshness.persistent_binding_identity(),
        record.reviewer_policy_identity,
        record.review_nonce,
    ] {
        put_digest(&mut bytes, identity);
    }
    for identity in record.proof_target.digests() {
        put_digest(&mut bytes, identity);
    }
    bytes.extend_from_slice(&record.freshness.ledger_generation().to_le_bytes());
    sha256(&bytes)
}

fn put_tool(bytes: &mut Vec<u8>, tool: &MeasuredToolIdentity) {
    put_text(bytes, tool.name().as_str());
    put_text(bytes, tool.version().as_str());
    put_digest(bytes, tool.executable_digest());
    put_digest(bytes, tool.configuration_digest());
}

fn put_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u16).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn put_digest(bytes: &mut Vec<u8>, value: Digest) {
    bytes.extend_from_slice(value.as_bytes());
}

const fn property_tag(property: ProofProperty) -> u8 {
    match property {
        ProofProperty::Bounds => 1,
        ProofProperty::AddressOverflowFreedom => 2,
        ProofProperty::MemorySafety => 3,
        ProofProperty::Initialization => 4,
        ProofProperty::RaceFreedom => 5,
        ProofProperty::LaunchValidity => 6,
        ProofProperty::FunctionalCorrectness => 7,
    }
}

fn require_nonzero(identity: Digest, field: &'static str) -> Result<(), ScalarGemmProofErrorV1> {
    if identity.as_bytes().iter().all(|byte| *byte == 0) {
        Err(ScalarGemmProofErrorV1::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScalarGemmProofErrorV1 {
    SourceLengthOutOfRange { max: usize },
    ZeroIdentity { field: &'static str },
    IdentityMismatch { field: &'static str },
    UnexpectedTool,
    UnexpectedModel,
    DependencyCapacity,
    DuplicateDependency,
    TrustedItemCapacity,
    DuplicateTrustedItem,
    TargetProfileSubstitution,
    DependencySubstitution,
    ToolSubstitution,
    ModelSubstitution,
    TrustedInventorySubstitution,
    PropertySubstitution,
    ProofOutcomeSubstitution,
    MissingPersistentFreshness,
    FreshnessSubstitution,
    Replay,
    ReviewCapacityOutOfRange,
    ReviewCapacityReached,
}

impl fmt::Display for ScalarGemmProofErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceLengthOutOfRange { max } => {
                write!(formatter, "scalar proof source length must be in 1..={max}")
            }
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity is zero"),
            Self::IdentityMismatch { field } => write!(formatter, "{field} identity differs"),
            Self::UnexpectedTool => formatter.write_str("unexpected scalar proof tool"),
            Self::UnexpectedModel => formatter.write_str("unexpected scalar proof model"),
            Self::DependencyCapacity => {
                formatter.write_str("scalar proof dependency set exceeds its bound")
            }
            Self::DuplicateDependency => formatter.write_str("duplicate scalar proof dependency"),
            Self::TrustedItemCapacity => {
                formatter.write_str("scalar proof trusted-item set exceeds its bound")
            }
            Self::DuplicateTrustedItem => formatter.write_str("duplicate scalar trusted item"),
            Self::TargetProfileSubstitution => {
                formatter.write_str("scalar proof target profile substitution")
            }
            Self::DependencySubstitution => {
                formatter.write_str("scalar proof dependency substitution")
            }
            Self::ToolSubstitution => formatter.write_str("scalar proof tool substitution"),
            Self::ModelSubstitution => formatter.write_str("scalar proof model substitution"),
            Self::TrustedInventorySubstitution => {
                formatter.write_str("scalar proof trusted-item inventory substitution")
            }
            Self::PropertySubstitution => formatter.write_str("scalar proof property substitution"),
            Self::ProofOutcomeSubstitution => {
                formatter.write_str("scalar proof outcome is not proved")
            }
            Self::MissingPersistentFreshness => {
                formatter.write_str("scalar proof lacks persistent freshness")
            }
            Self::FreshnessSubstitution => {
                formatter.write_str("scalar proof freshness substitution")
            }
            Self::Replay => formatter.write_str("reviewed scalar proof replay"),
            Self::ReviewCapacityOutOfRange => formatter.write_str("review capacity out of range"),
            Self::ReviewCapacityReached => formatter.write_str("review capacity reached"),
        }
    }
}

impl std::error::Error for ScalarGemmProofErrorV1 {}
