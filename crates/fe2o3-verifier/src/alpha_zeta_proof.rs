//! Exact source-proof records for the bounded gfx942 alpha/zeta profile.
//!
//! These records close substitution gaps around existing source-model proofs.
//! They do not establish IEEE-754 or compiler-to-machine refinement and grant
//! no proof, load, or launch authority.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifact_transaction::TargetIdentityV1;
use fe2o3_artifacts::{
    AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    DigestAlgorithm, Endianness, ExecutableCodeObjectVersionV1, IdentityText, LaunchContract,
    Mutability, PayloadDigest, PointerWidth, ProofTargetIdentity as ArtifactProofTargetIdentity,
    ScalarType, TargetIdentity,
};
use fe2o3_kernel_descriptor::DeviceTargetV1;

use crate::{
    AlphaZetaProofSourcesV1, Configuration, Digest, MeasuredToolIdentity,
    PersistentlyFreshProofExecutableBindingV1, ProofCapsuleFreshnessIdentityV1, ProofCapsuleV1,
    ProofOutcome, ProofProperty, ProofTargetIdentity, VerificationModelIdentity,
};

pub const GFX942_ALPHA_ZETA_PROOF_DOMAIN_V1: [u8; 8] = *b"FE2AZPI\0";
pub const GFX942_ALPHA_ZETA_REVIEW_DOMAIN_V1: [u8; 8] = *b"FE2AZRV\0";
pub const GFX942_ALPHA_ZETA_SET_DOMAIN_V1: [u8; 8] = *b"FE2AZPS\0";
pub const GFX942_ALPHA_ZETA_PROOF_VERSION_V1: u16 = 2;
pub const GFX942_ALPHA_ZETA_MODEL_VERSION_V1: &str = "gfx942-alpha-zeta-source-v1";
pub const MAX_GFX942_ALPHA_ZETA_REVIEW_RECORDS_V1: usize = 4096;
pub const GFX942_XNACK_MINUS_TARGET_V1: &str = "gfx942:xnack-";

pub const GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1: [ProofProperty; 5] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::FunctionalCorrectness,
];

/// The artifact proof-binding V1 schema requires these additional envelope
/// claims. Only the five entries in `GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1`
/// are established by the alpha/zeta Verus source harness.
pub const GFX942_ALPHA_ZETA_AUTHENTICATED_PROPERTIES_V1: [ProofProperty; 7] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::MemorySafety,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::LaunchValidity,
    ProofProperty::FunctionalCorrectness,
];

const AMDGPU_TRIPLE_V1: &str = "amdgcn-amd-amdhsa";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942AlphaZetaKernelV1 {
    Alpha,
    Zeta,
}

impl Gfx942AlphaZetaKernelV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Zeta => "zeta",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Alpha => 1,
            Self::Zeta => 2,
        }
    }
}

/// Shared compiler, artifact-manifest, and publication identity for the exact
/// `gfx942:xnack-` alpha/zeta profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942XnackMinusTargetIdentityV1 {
    device: DeviceTargetV1,
    artifact: TargetIdentity,
    publication: TargetIdentityV1,
}

impl Gfx942XnackMinusTargetIdentityV1 {
    pub fn canonical() -> Self {
        let device = DeviceTargetV1::parse(GFX942_XNACK_MINUS_TARGET_V1)
            .expect("the fixed alpha/zeta target is canonical");
        let artifact = TargetIdentity::new(
            IdentityText::new(AMDGPU_TRIPLE_V1).expect("fixed target triple is valid"),
            IdentityText::new(GFX942_XNACK_MINUS_TARGET_V1)
                .expect("fixed target architecture is valid"),
            PointerWidth::Bits64,
            Endianness::Little,
            vec![Capability::AmdWave],
        )
        .expect("fixed artifact target is canonical");
        let publication = canonical_publication_target(&artifact);
        Self {
            device,
            artifact,
            publication,
        }
    }

    pub const fn device(&self) -> DeviceTargetV1 {
        self.device
    }

    pub const fn artifact(&self) -> &TargetIdentity {
        &self.artifact
    }

    pub const fn publication(&self) -> TargetIdentityV1 {
        self.publication
    }
}

/// Canonical input identity for one exact alpha or zeta source proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942AlphaZetaProofInputV1 {
    kernel: Gfx942AlphaZetaKernelV1,
    sources: AlphaZetaProofSourcesV1,
    canonical_target: Gfx942XnackMinusTargetIdentityV1,
    target: ProofTargetIdentity,
    abi_identity: Digest,
    effects_identity: Digest,
    launch_identity: Digest,
    verus: MeasuredToolIdentity,
    solver: MeasuredToolIdentity,
    model: VerificationModelIdentity,
    proof_set_nonce: Digest,
    proof_nonce: Digest,
    identity: Digest,
}

impl Gfx942AlphaZetaProofInputV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn seal(
        kernel: Gfx942AlphaZetaKernelV1,
        sources: AlphaZetaProofSourcesV1,
        target: ProofTargetIdentity,
        abi_identity: Digest,
        effects_identity: Digest,
        launch_identity: Digest,
        verus: MeasuredToolIdentity,
        solver: MeasuredToolIdentity,
        model: VerificationModelIdentity,
        proof_set_nonce: Digest,
        proof_nonce: Digest,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        for (field, identity) in [
            ("ABI", abi_identity),
            ("effects", effects_identity),
            ("launch", launch_identity),
            ("proof-set nonce", proof_set_nonce),
            ("proof nonce", proof_nonce),
            ("Verus executable", verus.executable_digest()),
            ("Verus configuration", verus.configuration_digest()),
            ("solver executable", solver.executable_digest()),
            ("solver configuration", solver.configuration_digest()),
            ("model axioms", model.axioms_digest()),
        ] {
            require_nonzero(identity, field)?;
        }
        for identity in target.digests() {
            require_nonzero(identity, "proof target")?;
        }
        if target.source_tree_digest != sources.source_tree_identity() {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: "source tree",
            });
        }
        if target.crate_graph_digest != sources.dependency_tree_identity() {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: "dependency tree",
            });
        }
        if target.effects_contract_digest != effects_identity {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch { field: "effects" });
        }
        let canonical_target = Gfx942XnackMinusTargetIdentityV1::canonical();
        if verus.name().as_str() != "verus" || solver.name().as_str() != "z3" {
            return Err(AlphaZetaProofErrorV1::UnexpectedTool);
        }
        if model.version().as_str() != GFX942_ALPHA_ZETA_MODEL_VERSION_V1 {
            return Err(AlphaZetaProofErrorV1::UnexpectedModel);
        }
        if proof_set_nonce == proof_nonce {
            return Err(AlphaZetaProofErrorV1::NonceCollision);
        }

        let mut input = Self {
            kernel,
            sources,
            canonical_target,
            target,
            abi_identity,
            effects_identity,
            launch_identity,
            verus,
            solver,
            model,
            proof_set_nonce,
            proof_nonce,
            identity: Digest::from_bytes([0; 32]),
        };
        input.identity = sha256(&input.identity_bytes());
        Ok(input)
    }

    pub const fn kernel(&self) -> Gfx942AlphaZetaKernelV1 {
        self.kernel
    }
    pub const fn sources(&self) -> &AlphaZetaProofSourcesV1 {
        &self.sources
    }
    pub const fn canonical_target(&self) -> &Gfx942XnackMinusTargetIdentityV1 {
        &self.canonical_target
    }
    pub const fn target(&self) -> ProofTargetIdentity {
        self.target
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
    pub const fn verus(&self) -> &MeasuredToolIdentity {
        &self.verus
    }
    pub const fn solver(&self) -> &MeasuredToolIdentity {
        &self.solver
    }
    pub const fn model(&self) -> &VerificationModelIdentity {
        &self.model
    }
    pub const fn proof_set_nonce(&self) -> Digest {
        self.proof_set_nonce
    }
    pub const fn proof_nonce(&self) -> Digest {
        self.proof_nonce
    }
    pub const fn identity(&self) -> Digest {
        self.identity
    }
    pub const fn requested_properties(&self) -> &[ProofProperty; 5] {
        &GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1
    }
    pub const fn proves_ieee_f32_refinement(&self) -> bool {
        false
    }
    pub const fn proves_compiler_to_machine_refinement(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub const fn has_complete_source_closure(&self) -> bool {
        false
    }
    pub const fn has_complete_verifier_runtime_closure(&self) -> bool {
        false
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = self.identity_bytes();
        put_digest(&mut bytes, self.identity);
        bytes
    }

    fn identity_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1024);
        bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_DOMAIN_V1);
        bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.push(self.kernel.tag());
        put_text(&mut bytes, &self.canonical_target.device.to_string());
        put_text(&mut bytes, self.canonical_target.artifact.triple().as_str());
        put_text(
            &mut bytes,
            self.canonical_target.artifact.architecture().as_str(),
        );
        bytes.push(1);
        bytes.push(0);
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&7_u16.to_le_bytes());
        bytes.extend_from_slice(self.canonical_target.publication.as_bytes());
        let manifest = self.sources.to_canonical_manifest_bytes();
        bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&manifest);
        for identity in [
            self.sources.source_tree_identity(),
            self.sources.dependency_tree_identity(),
            self.sources.trusted_inventory().identity(),
        ] {
            put_digest(&mut bytes, identity);
        }
        for identity in self.target.digests() {
            put_digest(&mut bytes, identity);
        }
        for identity in [
            self.abi_identity,
            self.effects_identity,
            self.launch_identity,
        ] {
            put_digest(&mut bytes, identity);
        }
        put_tool(&mut bytes, &self.verus);
        put_tool(&mut bytes, &self.solver);
        put_text(&mut bytes, self.model.version().as_str());
        put_digest(&mut bytes, self.model.axioms_digest());
        for property in GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1 {
            bytes.push(property_tag(property));
        }
        put_digest(&mut bytes, self.proof_set_nonce);
        put_digest(&mut bytes, self.proof_nonce);
        bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaZetaExecutionReviewV1 {
    input_identity: Digest,
    proof_capsule_identity: Digest,
    freshness: ProofCapsuleFreshnessIdentityV1,
    reviewer_policy_identity: Digest,
    review_nonce: Digest,
}

impl AlphaZetaExecutionReviewV1 {
    pub fn new(
        input_identity: Digest,
        proof_capsule_identity: Digest,
        freshness: ProofCapsuleFreshnessIdentityV1,
        reviewer_policy_identity: Digest,
        review_nonce: Digest,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        for (field, identity) in [
            ("reviewed input", input_identity),
            ("reviewed proof capsule", proof_capsule_identity),
            ("reviewer policy", reviewer_policy_identity),
            ("review nonce", review_nonce),
        ] {
            require_nonzero(identity, field)?;
        }
        Ok(Self {
            input_identity,
            proof_capsule_identity,
            freshness,
            reviewer_policy_identity,
            review_nonce,
        })
    }

    pub const fn reviewer_policy_identity(self) -> Digest {
        self.reviewer_policy_identity
    }
}

/// Process-local replay rejection layered over the persistent freshness
/// identity already carried by the proof capsule.
#[derive(Debug)]
pub struct AlphaZetaReviewLedgerV1 {
    max_records: usize,
    input_identities: BTreeSet<Digest>,
    proof_nonces: BTreeSet<Digest>,
    challenges: BTreeSet<Digest>,
    transcripts: BTreeSet<Digest>,
    results: BTreeSet<Digest>,
    persistent_bindings: BTreeSet<Digest>,
    review_nonces: BTreeSet<Digest>,
}

impl AlphaZetaReviewLedgerV1 {
    pub const fn new() -> Self {
        Self {
            max_records: MAX_GFX942_ALPHA_ZETA_REVIEW_RECORDS_V1,
            input_identities: BTreeSet::new(),
            proof_nonces: BTreeSet::new(),
            challenges: BTreeSet::new(),
            transcripts: BTreeSet::new(),
            results: BTreeSet::new(),
            persistent_bindings: BTreeSet::new(),
            review_nonces: BTreeSet::new(),
        }
    }

    pub fn with_max_records(max_records: usize) -> Result<Self, AlphaZetaProofErrorV1> {
        if max_records == 0 || max_records > MAX_GFX942_ALPHA_ZETA_REVIEW_RECORDS_V1 {
            return Err(AlphaZetaProofErrorV1::ReviewCapacityOutOfRange);
        }
        Ok(Self {
            max_records,
            ..Self::new()
        })
    }

    pub fn recorded_count(&self) -> usize {
        self.input_identities.len()
    }

    fn check(
        &self,
        input: &Gfx942AlphaZetaProofInputV1,
        freshness: ProofCapsuleFreshnessIdentityV1,
        review_nonce: Digest,
    ) -> Result<(), AlphaZetaProofErrorV1> {
        if self.recorded_count() >= self.max_records {
            return Err(AlphaZetaProofErrorV1::ReviewCapacityReached);
        }
        if self.input_identities.contains(&input.identity)
            || self.proof_nonces.contains(&input.proof_nonce)
            || self.challenges.contains(&freshness.challenge())
            || self.transcripts.contains(&freshness.transcript())
            || self.results.contains(&freshness.result())
            || self
                .persistent_bindings
                .contains(&freshness.persistent_binding_identity())
            || self.review_nonces.contains(&review_nonce)
        {
            return Err(AlphaZetaProofErrorV1::Replay);
        }
        Ok(())
    }

    fn record(
        &mut self,
        input: &Gfx942AlphaZetaProofInputV1,
        freshness: ProofCapsuleFreshnessIdentityV1,
        review_nonce: Digest,
    ) {
        self.input_identities.insert(input.identity);
        self.proof_nonces.insert(input.proof_nonce);
        self.challenges.insert(freshness.challenge());
        self.transcripts.insert(freshness.transcript());
        self.results.insert(freshness.result());
        self.persistent_bindings
            .insert(freshness.persistent_binding_identity());
        self.review_nonces.insert(review_nonce);
    }
}

impl Default for AlphaZetaReviewLedgerV1 {
    fn default() -> Self {
        Self::new()
    }
}

/// Linear record of one exact reviewer decision over a fresh recorder report.
///
/// The reviewer policy is caller-supplied identity. This record does not
/// authenticate a reviewer, Verus, the solver, or compiler refinement.
#[derive(Debug, Eq, PartialEq)]
pub struct ReviewedAlphaZetaExecutionV1 {
    kernel: Gfx942AlphaZetaKernelV1,
    input_identity: Digest,
    source_tree_identity: Digest,
    dependency_tree_identity: Digest,
    proof_set_nonce: Digest,
    proof_nonce: Digest,
    verus: MeasuredToolIdentity,
    solver: MeasuredToolIdentity,
    model: VerificationModelIdentity,
    freshness: ProofCapsuleFreshnessIdentityV1,
    reviewer_policy_identity: Digest,
    review_nonce: Digest,
    identity: Digest,
}

impl ReviewedAlphaZetaExecutionV1 {
    pub const fn kernel(&self) -> Gfx942AlphaZetaKernelV1 {
        self.kernel
    }
    pub const fn input_identity(&self) -> Digest {
        self.input_identity
    }
    pub const fn proof_set_nonce(&self) -> Digest {
        self.proof_set_nonce
    }
    pub const fn freshness(&self) -> ProofCapsuleFreshnessIdentityV1 {
        self.freshness
    }
    pub const fn reviewer_policy_identity(&self) -> Digest {
        self.reviewer_policy_identity
    }
    pub const fn identity(&self) -> Digest {
        self.identity
    }
    pub const fn reported_properties(&self) -> &[ProofProperty; 5] {
        &GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
    pub const fn proves_ieee_f32_refinement(&self) -> bool {
        false
    }
    pub const fn proves_compiler_to_machine_refinement(&self) -> bool {
        false
    }
}

/// Records caller-assembled capsule evidence for tests and diagnostics only.
/// `ProofCapsuleV1::new_inert` values are accepted here, so this function and
/// its output can never satisfy an authoritative authenticated-binding boundary.
pub fn record_descriptive_alpha_zeta_execution_v1(
    input: &Gfx942AlphaZetaProofInputV1,
    proof: &ProofCapsuleV1,
    review: AlphaZetaExecutionReviewV1,
    ledger: &mut AlphaZetaReviewLedgerV1,
) -> Result<ReviewedAlphaZetaExecutionV1, AlphaZetaProofErrorV1> {
    if review.input_identity != input.identity {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "reviewed input",
        });
    }
    if review.proof_capsule_identity != proof.identity() {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "reviewed proof capsule",
        });
    }
    let target = proof.target();
    if target.features().len() != 1 || target.features()[0].as_str() != GFX942_XNACK_MINUS_TARGET_V1
    {
        return Err(AlphaZetaProofErrorV1::TargetProfileSubstitution);
    }
    if input.target != target.proof_target() {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "proof target",
        });
    }
    for (field, expected, actual) in [
        ("ABI", input.abi_identity, target.abi_identity()),
        ("effects", input.effects_identity, target.effects_identity()),
        ("launch", input.launch_identity, target.launch_identity()),
    ] {
        if expected != actual {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch { field });
        }
    }

    let expected_dependencies = input
        .sources
        .dependency_bindings()
        .into_iter()
        .collect::<BTreeSet<_>>();
    let actual_dependencies = target
        .dependencies()
        .iter()
        .map(|dependency| (dependency.name().as_str().to_owned(), dependency.identity()))
        .collect::<BTreeSet<_>>();
    if actual_dependencies != expected_dependencies {
        return Err(AlphaZetaProofErrorV1::DependencySubstitution);
    }

    let policy = proof.policy();
    if policy.claimed_verifier() != &input.verus || policy.claimed_solver() != &input.solver {
        return Err(AlphaZetaProofErrorV1::ToolSubstitution);
    }
    if policy.model() != &input.model {
        return Err(AlphaZetaProofErrorV1::ModelSubstitution);
    }
    let expected_trusted = input.sources.trusted_inventory().trusted_items();
    if policy.approved_axioms().allowed() != expected_trusted
        || policy.requested_axioms() != expected_trusted
    {
        return Err(AlphaZetaProofErrorV1::TrustedInventorySubstitution);
    }
    if policy.requested_properties() != GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1
        || proof.result().reported_properties() != GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1
        || proof.result().outcome() != ProofOutcome::Proved
    {
        return Err(AlphaZetaProofErrorV1::PropertySubstitution);
    }
    let freshness = proof
        .execution()
        .freshness()
        .ok_or(AlphaZetaProofErrorV1::MissingPersistentFreshness)?;
    if freshness != review.freshness {
        return Err(AlphaZetaProofErrorV1::FreshnessSubstitution);
    }
    ledger.check(input, freshness, review.review_nonce)?;

    let mut record = ReviewedAlphaZetaExecutionV1 {
        kernel: input.kernel,
        input_identity: input.identity,
        source_tree_identity: input.sources.source_tree_identity(),
        dependency_tree_identity: input.sources.dependency_tree_identity(),
        proof_set_nonce: input.proof_set_nonce,
        proof_nonce: input.proof_nonce,
        verus: input.verus.clone(),
        solver: input.solver.clone(),
        model: input.model.clone(),
        freshness,
        reviewer_policy_identity: review.reviewer_policy_identity,
        review_nonce: review.review_nonce,
        identity: Digest::from_bytes([0; 32]),
    };
    record.identity = reviewed_identity(&record, proof.identity());
    ledger.record(input, freshness, review.review_nonce);
    Ok(record)
}

/// Linear, exact set containing one alpha and one zeta result.
#[derive(Debug, Eq, PartialEq)]
pub struct ReviewedAlphaZetaProofSetV1 {
    alpha: ReviewedAlphaZetaExecutionV1,
    zeta: ReviewedAlphaZetaExecutionV1,
    identity: Digest,
}

impl ReviewedAlphaZetaProofSetV1 {
    pub fn new(
        first: ReviewedAlphaZetaExecutionV1,
        second: ReviewedAlphaZetaExecutionV1,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        let (alpha, zeta) = match (first.kernel, second.kernel) {
            (Gfx942AlphaZetaKernelV1::Alpha, Gfx942AlphaZetaKernelV1::Zeta) => (first, second),
            (Gfx942AlphaZetaKernelV1::Zeta, Gfx942AlphaZetaKernelV1::Alpha) => (second, first),
            _ => return Err(AlphaZetaProofErrorV1::IncompleteKernelSet),
        };
        for equal in [
            alpha.proof_set_nonce == zeta.proof_set_nonce,
            alpha.source_tree_identity == zeta.source_tree_identity,
            alpha.dependency_tree_identity == zeta.dependency_tree_identity,
            alpha.verus == zeta.verus,
            alpha.solver == zeta.solver,
            alpha.model == zeta.model,
            alpha.reviewer_policy_identity == zeta.reviewer_policy_identity,
            alpha.freshness.ledger_namespace() == zeta.freshness.ledger_namespace(),
        ] {
            if !equal {
                return Err(AlphaZetaProofErrorV1::MixedProofSet);
            }
        }
        if alpha.input_identity == zeta.input_identity
            || alpha.proof_nonce == zeta.proof_nonce
            || alpha.freshness.proof_binding_identity() == zeta.freshness.proof_binding_identity()
            || alpha.review_nonce == zeta.review_nonce
            || alpha.freshness.challenge() == zeta.freshness.challenge()
            || alpha.freshness.transcript() == zeta.freshness.transcript()
            || alpha.freshness.result() == zeta.freshness.result()
            || alpha.freshness.persistent_binding_identity()
                == zeta.freshness.persistent_binding_identity()
        {
            return Err(AlphaZetaProofErrorV1::MixedProofSet);
        }
        let (previous, next) =
            if alpha.freshness.ledger_generation() < zeta.freshness.ledger_generation() {
                (alpha.freshness, zeta.freshness)
            } else {
                (zeta.freshness, alpha.freshness)
            };
        if previous.ledger_generation().checked_add(1) != Some(next.ledger_generation())
            || previous.ledger_state_identity() != next.previous_ledger_state_identity()
        {
            return Err(AlphaZetaProofErrorV1::MixedFreshnessHistory);
        }
        let mut bytes = Vec::with_capacity(80);
        bytes.extend_from_slice(&GFX942_ALPHA_ZETA_SET_DOMAIN_V1);
        bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        put_digest(&mut bytes, alpha.identity);
        put_digest(&mut bytes, zeta.identity);
        let identity = sha256(&bytes);
        Ok(Self {
            alpha,
            zeta,
            identity,
        })
    }

    pub const fn alpha(&self) -> &ReviewedAlphaZetaExecutionV1 {
        &self.alpha
    }
    pub const fn zeta(&self) -> &ReviewedAlphaZetaExecutionV1 {
        &self.zeta
    }
    pub const fn identity(&self) -> Digest {
        self.identity
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Independent review expectations for inert executable evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaZetaExecutableEvidenceReviewV1 {
    input_identity: Digest,
    persistent_binding_identity: Digest,
    reviewer_policy_identity: Digest,
    review_nonce: Digest,
}

impl AlphaZetaExecutableEvidenceReviewV1 {
    pub fn new(
        input_identity: Digest,
        persistent_binding_identity: Digest,
        reviewer_policy_identity: Digest,
        review_nonce: Digest,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        for (field, identity) in [
            ("executable-evidence reviewed input", input_identity),
            ("persistent proof binding", persistent_binding_identity),
            (
                "executable-evidence reviewer policy",
                reviewer_policy_identity,
            ),
            ("executable-evidence review nonce", review_nonce),
        ] {
            require_nonzero(identity, field)?;
        }
        Ok(Self {
            input_identity,
            persistent_binding_identity,
            reviewer_policy_identity,
            review_nonce,
        })
    }
}

/// Non-clone, inert review of one authenticated recorder/executable binding.
///
/// The recorder did not receive the immutable source snapshots and this lane
/// does not measure the complete Verus/compiler runtime closure. Consequently
/// this type carries no proof, load, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct ExecutableEvidenceAlphaZetaExecutionV1 {
    kernel: Gfx942AlphaZetaKernelV1,
    kernel_identity: Digest,
    input_identity: Digest,
    proof_set_nonce: Digest,
    proof_nonce: Digest,
    set_context_identity: Digest,
    proof_binding_identity: Digest,
    reviewer_policy_identity: Digest,
    review_nonce: Digest,
    binding: PersistentlyFreshProofExecutableBindingV1,
    identity: Digest,
}

impl ExecutableEvidenceAlphaZetaExecutionV1 {
    pub const fn kernel(&self) -> Gfx942AlphaZetaKernelV1 {
        self.kernel
    }

    pub const fn input_identity(&self) -> Digest {
        self.input_identity
    }

    pub const fn kernel_identity(&self) -> Digest {
        self.kernel_identity
    }

    pub const fn proof_binding_identity(&self) -> Digest {
        self.proof_binding_identity
    }

    pub const fn persistent_binding(&self) -> &PersistentlyFreshProofExecutableBindingV1 {
        &self.binding
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn has_complete_source_closure(&self) -> bool {
        false
    }

    pub const fn has_complete_verifier_runtime_closure(&self) -> bool {
        false
    }

    pub const fn recorder_consumed_immutable_source_snapshot(&self) -> bool {
        false
    }
}

/// Consumes an authenticated recorder/executable binding as inert evidence.
///
/// Unlike the descriptive `ProofCapsuleV1` path, callers cannot synthesize the
/// binding with `new_inert`. That distinction does not grant proof authority:
/// the recorder is not Verus and did not consume the retained source snapshot.
///
/// ```compile_fail
/// # use fe2o3_verifier::{
/// #     AlphaZetaExecutableEvidenceReviewV1, Gfx942AlphaZetaProofInputV1, ProofCapsuleV1,
/// #     record_inert_alpha_zeta_executable_evidence_v1,
/// # };
/// # fn inert_capsules_are_not_executable_evidence_inputs(
/// #     input: &Gfx942AlphaZetaProofInputV1,
/// #     inert: ProofCapsuleV1,
/// #     review: AlphaZetaExecutableEvidenceReviewV1,
/// # ) {
/// let _ = record_inert_alpha_zeta_executable_evidence_v1(input, inert, review);
/// # }
/// ```
pub fn record_inert_alpha_zeta_executable_evidence_v1(
    input: &Gfx942AlphaZetaProofInputV1,
    binding: PersistentlyFreshProofExecutableBindingV1,
    review: AlphaZetaExecutableEvidenceReviewV1,
) -> Result<ExecutableEvidenceAlphaZetaExecutionV1, AlphaZetaProofErrorV1> {
    if review.input_identity != input.identity {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "executable-evidence reviewed input",
        });
    }
    if review.persistent_binding_identity != binding.binding_identity() {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "persistent proof binding",
        });
    }

    let proof_binding = binding.proof_binding();
    let execution = proof_binding.execution_identity();
    let evidence = proof_binding.execution_evidence();
    let request = evidence.invocation_plan().request();
    let result = evidence.recorder_report();
    if request.target() != input.target || result.target() != input.target {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "authenticated proof target",
        });
    }
    if request.model() != &input.model || result.model() != &input.model {
        return Err(AlphaZetaProofErrorV1::ModelSubstitution);
    }
    if request.properties() != GFX942_ALPHA_ZETA_AUTHENTICATED_PROPERTIES_V1
        || result.recorder_reported_properties() != GFX942_ALPHA_ZETA_AUTHENTICATED_PROPERTIES_V1
        || result.outcome() != ProofOutcome::Proved
    {
        return Err(AlphaZetaProofErrorV1::PropertySubstitution);
    }
    let expected_trusted = input.sources.trusted_inventory().trusted_items();
    if request.trusted_items() != expected_trusted || result.trusted_items() != expected_trusted {
        return Err(AlphaZetaProofErrorV1::TrustedInventorySubstitution);
    }
    if execution.claimed_verifier().identity() != &input.verus
        || execution.claimed_solver().identity() != &input.solver
    {
        return Err(AlphaZetaProofErrorV1::ToolSubstitution);
    }
    validate_inert_configuration(input, request.configuration())?;

    let executable = proof_binding.executable_binding().executable();
    if executable.target() != input.canonical_target.artifact()
        || canonical_publication_target(executable.target()) != input.canonical_target.publication()
    {
        return Err(AlphaZetaProofErrorV1::TargetProfileSubstitution);
    }
    if executable.code_object_version() != ExecutableCodeObjectVersionV1::V6 {
        return Err(AlphaZetaProofErrorV1::UnsupportedCodeObjectVersion);
    }
    let executable_kernel = derive_executable_kernel_role(executable)?;
    if executable_kernel != input.kernel {
        return Err(AlphaZetaProofErrorV1::KernelRoleSubstitution);
    }
    let artifact_target = executable.proof_target();
    if verifier_target_from_artifact(artifact_target) != input.target {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "artifact proof target",
        });
    }
    if alpha_zeta_abi_identity_v1(executable.abi()) != input.abi_identity {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "artifact ABI",
        });
    }
    if digest_from_payload(artifact_target.source_contracts().effects_digest())
        != input.effects_identity
    {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "artifact effects contract",
        });
    }
    if alpha_zeta_launch_identity_v1(executable.launch()) != input.launch_identity {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "artifact launch contract",
        });
    }

    let proof_binding_identity = binding.identity().proof_binding_identity();
    let mut record = ExecutableEvidenceAlphaZetaExecutionV1 {
        kernel: executable_kernel,
        kernel_identity: digest_from_payload(artifact_target.artifact().kernel_id()),
        input_identity: input.identity,
        proof_set_nonce: input.proof_set_nonce,
        proof_nonce: input.proof_nonce,
        set_context_identity: inert_set_context_identity(input),
        proof_binding_identity,
        reviewer_policy_identity: review.reviewer_policy_identity,
        review_nonce: review.review_nonce,
        binding,
        identity: Digest::from_bytes([0; 32]),
    };
    record.identity = inert_executable_reviewed_identity(&record);
    Ok(record)
}

/// Inert two-kernel evidence set from one contiguous durable ledger lineage.
#[derive(Debug, Eq, PartialEq)]
pub struct InertAlphaZetaExecutableEvidenceSetV1 {
    alpha: ExecutableEvidenceAlphaZetaExecutionV1,
    zeta: ExecutableEvidenceAlphaZetaExecutionV1,
    identity: Digest,
}

impl InertAlphaZetaExecutableEvidenceSetV1 {
    pub fn new(
        first: ExecutableEvidenceAlphaZetaExecutionV1,
        second: ExecutableEvidenceAlphaZetaExecutionV1,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        let (alpha, zeta) = match (first.kernel, second.kernel) {
            (Gfx942AlphaZetaKernelV1::Alpha, Gfx942AlphaZetaKernelV1::Zeta) => (first, second),
            (Gfx942AlphaZetaKernelV1::Zeta, Gfx942AlphaZetaKernelV1::Alpha) => (second, first),
            _ => return Err(AlphaZetaProofErrorV1::IncompleteKernelSet),
        };
        let alpha_receipt = alpha.binding.freshness_receipt();
        let zeta_receipt = zeta.binding.freshness_receipt();
        if alpha.proof_set_nonce != zeta.proof_set_nonce
            || alpha.set_context_identity != zeta.set_context_identity
            || alpha.reviewer_policy_identity != zeta.reviewer_policy_identity
            || alpha_receipt.namespace() != zeta_receipt.namespace()
        {
            return Err(AlphaZetaProofErrorV1::MixedProofSet);
        }
        if alpha.input_identity == zeta.input_identity
            || alpha.proof_nonce == zeta.proof_nonce
            || alpha.proof_binding_identity == zeta.proof_binding_identity
            || alpha.review_nonce == zeta.review_nonce
            || alpha_receipt.identity().challenge() == zeta_receipt.identity().challenge()
            || alpha_receipt.identity().transcript() == zeta_receipt.identity().transcript()
            || alpha_receipt.identity().result() == zeta_receipt.identity().result()
            || alpha.binding.binding_identity() == zeta.binding.binding_identity()
        {
            return Err(AlphaZetaProofErrorV1::MixedProofSet);
        }
        let (previous, next) = if alpha_receipt.generation() < zeta_receipt.generation() {
            (alpha_receipt, zeta_receipt)
        } else {
            (zeta_receipt, alpha_receipt)
        };
        if previous.generation().checked_add(1) != Some(next.generation())
            || previous.state_identity() != next.previous_state_identity()
        {
            return Err(AlphaZetaProofErrorV1::MixedFreshnessHistory);
        }
        let mut bytes = Vec::with_capacity(80);
        bytes.extend_from_slice(&GFX942_ALPHA_ZETA_SET_DOMAIN_V1);
        bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        put_digest(&mut bytes, alpha.identity);
        put_digest(&mut bytes, zeta.identity);
        let identity = sha256(&bytes);
        Ok(Self {
            alpha,
            zeta,
            identity,
        })
    }

    pub const fn alpha(&self) -> &ExecutableEvidenceAlphaZetaExecutionV1 {
        &self.alpha
    }

    pub const fn zeta(&self) -> &ExecutableEvidenceAlphaZetaExecutionV1 {
        &self.zeta
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn canonical_publication_target(target: &TargetIdentity) -> TargetIdentityV1 {
    const DOMAIN: &[u8] = b"fe2o3.direct-link.publication-scope.manifest-claim-target.v1\0";
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(DOMAIN);
    bytes.push(1);
    put_text(&mut bytes, target.triple().as_str());
    put_text(&mut bytes, target.architecture().as_str());
    bytes.push(match target.pointer_width() {
        PointerWidth::Bits32 => 0,
        PointerWidth::Bits64 => 1,
    });
    bytes.push(match target.endianness() {
        Endianness::Little => 0,
        Endianness::Big => 1,
    });
    bytes.extend_from_slice(&(target.capabilities().len() as u16).to_le_bytes());
    for capability in target.capabilities() {
        let tag = match capability {
            Capability::Subgroup => 0,
            Capability::Ballot => 1,
            Capability::Shuffle => 2,
            Capability::WorkgroupMemory => 3,
            Capability::MatrixMultiply => 4,
            Capability::AsyncCopy => 5,
            Capability::Atomics => 6,
            Capability::AmdWave => 7,
            Capability::AmdMfma => 8,
            Capability::AmdWmma => 9,
            Capability::AmdDsPermute => 10,
        };
        bytes.extend_from_slice(&u16::to_le_bytes(tag));
    }
    TargetIdentityV1::from_bytes(*sha256(&bytes).as_bytes())
}

/// Canonical launch-contract identity used by the inert alpha/zeta executable
/// evidence join. This is distinct from the artifact's composite identity.
pub fn alpha_zeta_launch_identity_v1(launch: &LaunchContract) -> Digest {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"FE2AZLC\0");
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
    bytes.push(launch.rank());
    match launch.block_size() {
        BlockSize::Any => bytes.push(0),
        BlockSize::Exact(dimensions) => {
            bytes.push(1);
            put_dimensions(&mut bytes, dimensions);
        }
        BlockSize::AtMost(dimensions) => {
            bytes.push(2);
            put_dimensions(&mut bytes, dimensions);
        }
    }
    put_dimensions(&mut bytes, launch.max_grid());
    bytes.extend_from_slice(&launch.static_shared_memory_bytes().to_le_bytes());
    bytes.extend_from_slice(&launch.max_dynamic_shared_memory_bytes().to_le_bytes());
    sha256(&bytes)
}

/// Canonical identity of the typed artifact ABI used by the inert join.
pub fn alpha_zeta_abi_identity_v1(abi: &AbiLayout) -> Digest {
    let mut bytes = Vec::with_capacity(256 + abi.fields().len() * 128);
    bytes.extend_from_slice(b"FE2AZAB\0");
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
    bytes.push(pointer_width_tag(abi.pointer_width()));
    bytes.extend_from_slice(&abi.size().to_le_bytes());
    bytes.extend_from_slice(&abi.alignment().to_le_bytes());
    bytes.extend_from_slice(&(abi.fields().len() as u16).to_le_bytes());
    for field in abi.fields() {
        put_text(&mut bytes, field.name().as_str());
        bytes.extend_from_slice(&field.offset().to_le_bytes());
        bytes.extend_from_slice(&field.size().to_le_bytes());
        bytes.extend_from_slice(&field.alignment().to_le_bytes());
        match field.kind() {
            AbiKind::Scalar(scalar) => {
                bytes.push(0);
                bytes.push(scalar_tag(scalar));
            }
            AbiKind::Pointer {
                pointee_size,
                pointee_alignment,
            } => {
                bytes.push(1);
                bytes.extend_from_slice(&pointee_size.to_le_bytes());
                bytes.extend_from_slice(&pointee_alignment.to_le_bytes());
            }
            AbiKind::Slice {
                element_size,
                element_alignment,
            } => {
                bytes.push(2);
                bytes.extend_from_slice(&element_size.to_le_bytes());
                bytes.extend_from_slice(&element_alignment.to_le_bytes());
            }
        }
        bytes.push(mutability_tag(field.mutability()));
        bytes.push(access_tag(field.access()));
        bytes.push(address_space_tag(field.address_space()));
        bytes.extend_from_slice(field.type_identity().rust_type().bytes().as_bytes());
        bytes.extend_from_slice(field.type_identity().layout().bytes().as_bytes());
        bytes.push(ownership_tag(field.ownership()));
        bytes.push(alias_class_tag(field.alias_class()));
    }
    sha256(&bytes)
}

/// Required authenticated-request configuration bindings for one sealed input.
pub fn alpha_zeta_inert_configuration_v1(
    input: &Gfx942AlphaZetaProofInputV1,
) -> Vec<(&'static str, String)> {
    vec![
        ("alpha_zeta_input", digest_hex(input.identity)),
        ("proof_set_nonce", digest_hex(input.proof_set_nonce)),
        ("proof_nonce", digest_hex(input.proof_nonce)),
        (
            "source_manifest",
            digest_hex(input.sources.dependency_tree_identity()),
        ),
        (
            "trusted_inventory",
            digest_hex(input.sources.trusted_inventory().identity()),
        ),
    ]
}

fn validate_inert_configuration(
    input: &Gfx942AlphaZetaProofInputV1,
    configuration: &Configuration,
) -> Result<(), AlphaZetaProofErrorV1> {
    let mut expected = alpha_zeta_inert_configuration_v1(input);
    expected.sort_unstable_by_key(|(key, _)| *key);
    if configuration.entries().len() != expected.len() {
        return Err(AlphaZetaProofErrorV1::IdentityMismatch {
            field: "authenticated proof configuration",
        });
    }
    for (actual, (expected_key, expected_value)) in configuration.entries().iter().zip(expected) {
        if actual.key().as_str() != expected_key || actual.value().as_str() != expected_value {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: "authenticated proof configuration",
            });
        }
    }
    Ok(())
}

fn inert_executable_reviewed_identity(record: &ExecutableEvidenceAlphaZetaExecutionV1) -> Digest {
    let receipt = record.binding.freshness_receipt();
    let mut bytes = Vec::with_capacity(384);
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_REVIEW_DOMAIN_V1);
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.push(record.kernel.tag());
    for digest in [
        record.input_identity,
        record.kernel_identity,
        record.proof_set_nonce,
        record.proof_nonce,
        record.set_context_identity,
        record.proof_binding_identity,
        record.binding.binding_identity(),
        receipt.identity().challenge(),
        receipt.identity().transcript(),
        receipt.identity().result(),
        receipt.namespace(),
        receipt.previous_state_identity(),
        receipt.state_identity(),
        record.reviewer_policy_identity,
        record.review_nonce,
    ] {
        put_digest(&mut bytes, digest);
    }
    bytes.extend_from_slice(&receipt.generation().to_le_bytes());
    sha256(&bytes)
}

fn inert_set_context_identity(input: &Gfx942AlphaZetaProofInputV1) -> Digest {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(b"FE2AZSC\0");
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
    for digest in [
        input.sources.source_tree_identity(),
        input.sources.dependency_tree_identity(),
        input.sources.trusted_inventory().identity(),
        input.abi_identity,
        input.effects_identity,
        input.launch_identity,
        input.proof_set_nonce,
    ] {
        put_digest(&mut bytes, digest);
    }
    bytes.extend_from_slice(input.canonical_target.publication.as_bytes());
    put_tool(&mut bytes, &input.verus);
    put_tool(&mut bytes, &input.solver);
    put_text(&mut bytes, input.model.version().as_str());
    put_digest(&mut bytes, input.model.axioms_digest());
    sha256(&bytes)
}

fn derive_executable_kernel_role(
    executable: &fe2o3_artifacts::ProofExecutableSemanticIdentityV1,
) -> Result<Gfx942AlphaZetaKernelV1, AlphaZetaProofErrorV1> {
    match (
        executable.logical_name().as_str(),
        executable.export_symbol().as_str(),
    ) {
        ("alpha", "alpha.kd") => Ok(Gfx942AlphaZetaKernelV1::Alpha),
        ("zeta", "zeta.kd") => Ok(Gfx942AlphaZetaKernelV1::Zeta),
        _ => Err(AlphaZetaProofErrorV1::KernelRoleSubstitution),
    }
}

fn verifier_target_from_artifact(target: ArtifactProofTargetIdentity) -> ProofTargetIdentity {
    let artifact = target.artifact();
    let contracts = target.source_contracts();
    ProofTargetIdentity {
        kernel_id: digest_from_payload(artifact.kernel_id()),
        instance_digest: digest_from_payload(artifact.instance_digest()),
        source_tree_digest: digest_from_payload(artifact.source_tree_digest()),
        crate_graph_digest: digest_from_payload(artifact.crate_graph_digest()),
        executable_digest: digest_from_payload(artifact.executable_digest()),
        environment_digest: digest_from_payload(artifact.environment_digest()),
        artifact_selection_digest: digest_from_payload(artifact.artifact_selection_digest()),
        artifact_contract_digest: digest_from_payload(artifact.artifact_contract_digest()),
        memory_contract_digest: digest_from_payload(contracts.memory_digest()),
        effects_contract_digest: digest_from_payload(contracts.effects_digest()),
        type_layout_digest: digest_from_payload(contracts.type_layout_digest()),
        capability_semantics_digest: digest_from_payload(contracts.capability_semantics_digest()),
        functional_specification_digest: digest_from_payload(
            contracts.functional_specification_digest(),
        ),
    }
}

const fn pointer_width_tag(value: PointerWidth) -> u8 {
    match value {
        PointerWidth::Bits32 => 0,
        PointerWidth::Bits64 => 1,
    }
}

const fn scalar_tag(value: ScalarType) -> u8 {
    match value {
        ScalarType::I8 => 0,
        ScalarType::U8 => 1,
        ScalarType::I16 => 2,
        ScalarType::U16 => 3,
        ScalarType::I32 => 4,
        ScalarType::U32 => 5,
        ScalarType::I64 => 6,
        ScalarType::U64 => 7,
        ScalarType::F16 => 8,
        ScalarType::F32 => 9,
        ScalarType::F64 => 10,
    }
}

const fn mutability_tag(value: Mutability) -> u8 {
    match value {
        Mutability::Immutable => 0,
        Mutability::Mutable => 1,
    }
}

const fn access_tag(value: Access) -> u8 {
    match value {
        Access::ByValue => 0,
        Access::ReadOnly => 1,
        Access::WriteOnly => 2,
        Access::ReadWrite => 3,
    }
}

const fn address_space_tag(value: AddressSpace) -> u8 {
    match value {
        AddressSpace::Value => 0,
        AddressSpace::Global => 1,
        AddressSpace::Constant => 2,
        AddressSpace::Workgroup => 3,
        AddressSpace::Private => 4,
        AddressSpace::Generic => 5,
    }
}

const fn ownership_tag(value: ArgumentOwnership) -> u8 {
    match value {
        ArgumentOwnership::ByValue => 0,
        ArgumentOwnership::SharedBorrow => 1,
        ArgumentOwnership::UniqueBorrow => 2,
        ArgumentOwnership::RawPointer => 3,
    }
}

const fn alias_class_tag(value: AliasClass) -> u8 {
    match value {
        AliasClass::Value => 0,
        AliasClass::SharedReadOnly => 1,
        AliasClass::Exclusive => 2,
        AliasClass::SharedAtomic => 3,
        AliasClass::Unrestricted => 4,
    }
}

fn digest_from_payload(digest: PayloadDigest) -> Digest {
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn put_dimensions(bytes: &mut Vec<u8>, dimensions: fe2o3_artifacts::Dimensions) {
    bytes.extend_from_slice(&dimensions.x().to_le_bytes());
    bytes.extend_from_slice(&dimensions.y().to_le_bytes());
    bytes.extend_from_slice(&dimensions.z().to_le_bytes());
}

fn digest_hex(digest: Digest) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest.as_bytes() {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

fn reviewed_identity(record: &ReviewedAlphaZetaExecutionV1, proof_capsule: Digest) -> Digest {
    let mut bytes = Vec::with_capacity(512);
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_REVIEW_DOMAIN_V1);
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.push(record.kernel.tag());
    for identity in [
        record.input_identity,
        proof_capsule,
        record.source_tree_identity,
        record.dependency_tree_identity,
        record.proof_set_nonce,
        record.proof_nonce,
    ] {
        put_digest(&mut bytes, identity);
    }
    put_tool(&mut bytes, &record.verus);
    put_tool(&mut bytes, &record.solver);
    put_text(&mut bytes, record.model.version().as_str());
    put_digest(&mut bytes, record.model.axioms_digest());
    let freshness = record.freshness;
    for identity in [
        freshness.proof_binding_identity(),
        freshness.challenge(),
        freshness.transcript(),
        freshness.result(),
        freshness.ledger_namespace(),
        freshness.previous_ledger_state_identity(),
        freshness.ledger_state_identity(),
        freshness.persistent_binding_identity(),
        record.reviewer_policy_identity,
        record.review_nonce,
    ] {
        put_digest(&mut bytes, identity);
    }
    bytes.extend_from_slice(&freshness.ledger_generation().to_le_bytes());
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

fn require_nonzero(identity: Digest, field: &'static str) -> Result<(), AlphaZetaProofErrorV1> {
    if identity.as_bytes().iter().all(|byte| *byte == 0) {
        Err(AlphaZetaProofErrorV1::ZeroIdentity { field })
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
pub enum AlphaZetaProofErrorV1 {
    Model(crate::ModelError),
    UnexpectedSourcePath,
    SourceLengthOutOfRange {
        max: u64,
    },
    DuplicateSourcePath,
    IncompleteSourceSet,
    SourceMutation,
    SourceRoleSubstitution,
    SourceManifestMutation,
    SourceManifestCapacity,
    SourceManifestIo {
        operation: &'static str,
        path: String,
    },
    SourceManifestStructure {
        path: String,
        reason: String,
    },
    ZeroIdentity {
        field: &'static str,
    },
    IdentityMismatch {
        field: &'static str,
    },
    UnexpectedTool,
    UnexpectedModel,
    NonceCollision,
    DependencySubstitution,
    TargetProfileSubstitution,
    ToolSubstitution,
    ModelSubstitution,
    AxiomSubstitution,
    TrustedInventorySubstitution,
    PropertySubstitution,
    KernelRoleSubstitution,
    UnsupportedCodeObjectVersion,
    MissingPersistentFreshness,
    FreshnessSubstitution,
    Replay,
    ReviewCapacityOutOfRange,
    ReviewCapacityReached,
    IncompleteKernelSet,
    MixedProofSet,
    MixedFreshnessHistory,
}

impl fmt::Display for AlphaZetaProofErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) => error.fmt(formatter),
            Self::UnexpectedSourcePath => formatter.write_str("unexpected alpha/zeta source path"),
            Self::SourceLengthOutOfRange { max } => {
                write!(formatter, "alpha/zeta source length must be in 1..={max}")
            }
            Self::DuplicateSourcePath => formatter.write_str("duplicate alpha/zeta source path"),
            Self::IncompleteSourceSet => formatter.write_str("incomplete alpha/zeta source set"),
            Self::SourceMutation => formatter.write_str("alpha/zeta source bytes changed"),
            Self::SourceRoleSubstitution => formatter.write_str("alpha/zeta source role changed"),
            Self::SourceManifestMutation => {
                formatter.write_str("alpha/zeta structural source manifest changed")
            }
            Self::SourceManifestCapacity => {
                formatter.write_str("alpha/zeta structural source manifest exceeds its bound")
            }
            Self::SourceManifestIo { operation, path } => {
                write!(formatter, "cannot {operation} alpha/zeta source {path}")
            }
            Self::SourceManifestStructure { path, reason } => {
                write!(
                    formatter,
                    "invalid alpha/zeta source structure in {path}: {reason}"
                )
            }
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity is zero"),
            Self::IdentityMismatch { field } => write!(formatter, "{field} identity differs"),
            Self::UnexpectedTool => formatter.write_str("unexpected alpha/zeta proof tool"),
            Self::UnexpectedModel => formatter.write_str("unexpected alpha/zeta proof model"),
            Self::NonceCollision => formatter.write_str("proof-set and proof nonces collide"),
            Self::DependencySubstitution => formatter.write_str("proof dependency substitution"),
            Self::TargetProfileSubstitution => {
                formatter.write_str("proof target profile substitution")
            }
            Self::ToolSubstitution => formatter.write_str("proof tool substitution"),
            Self::ModelSubstitution => formatter.write_str("proof model substitution"),
            Self::AxiomSubstitution => formatter.write_str("proof axiom substitution"),
            Self::TrustedInventorySubstitution => {
                formatter.write_str("proof trusted-item inventory substitution")
            }
            Self::PropertySubstitution => formatter.write_str("proof property substitution"),
            Self::KernelRoleSubstitution => {
                formatter.write_str("alpha/zeta kernel role substitution")
            }
            Self::UnsupportedCodeObjectVersion => {
                formatter.write_str("alpha/zeta executable evidence requires code-object v6")
            }
            Self::MissingPersistentFreshness => {
                formatter.write_str("reviewed result lacks persistent freshness")
            }
            Self::FreshnessSubstitution => formatter.write_str("proof freshness substitution"),
            Self::Replay => formatter.write_str("reviewed alpha/zeta proof replay"),
            Self::ReviewCapacityOutOfRange => formatter.write_str("review capacity out of range"),
            Self::ReviewCapacityReached => formatter.write_str("review capacity reached"),
            Self::IncompleteKernelSet => {
                formatter.write_str("proof set is not one alpha and one zeta")
            }
            Self::MixedProofSet => formatter.write_str("mixed alpha/zeta proof set"),
            Self::MixedFreshnessHistory => formatter.write_str("mixed proof freshness history"),
        }
    }
}

impl std::error::Error for AlphaZetaProofErrorV1 {}
