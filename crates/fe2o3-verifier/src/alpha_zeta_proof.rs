//! Exact source-proof records for the bounded gfx942 alpha/zeta profile.
//!
//! These records close substitution gaps around existing source-model proofs.
//! They do not establish IEEE-754 or compiler-to-machine refinement and grant
//! no proof, load, or launch authority.

use std::collections::BTreeSet;
use std::fmt;

use fe2o3_artifacts::DigestAlgorithm;

use crate::{
    Digest, MeasuredToolIdentity, ProofCapsuleFreshnessIdentityV1, ProofCapsuleV1, ProofOutcome,
    ProofProperty, ProofTargetIdentity, Text, VerificationModelIdentity,
};

pub const GFX942_ALPHA_ZETA_PROOF_DOMAIN_V1: [u8; 8] = *b"FE2AZPI\0";
pub const GFX942_ALPHA_ZETA_REVIEW_DOMAIN_V1: [u8; 8] = *b"FE2AZRV\0";
pub const GFX942_ALPHA_ZETA_SET_DOMAIN_V1: [u8; 8] = *b"FE2AZPS\0";
pub const GFX942_ALPHA_ZETA_PROOF_VERSION_V1: u16 = 1;
pub const GFX942_ALPHA_ZETA_MODEL_VERSION_V1: &str = "gfx942-alpha-zeta-source-v1";
pub const MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1: u64 = 1024 * 1024;
pub const MAX_GFX942_ALPHA_ZETA_REVIEW_RECORDS_V1: usize = 4096;

pub const ALPHA_ZETA_SHARED_BODY_PATH_V1: &str = "examples/verus_vecadd/src/two_kernel_bodies.rs";
pub const ALPHA_ZETA_PERMISSION_MODEL_PATH_V1: &str = "examples/verus_vecadd/verus/vecadd.rs";
pub const ALPHA_ZETA_PROOF_HARNESS_PATH_V1: &str = "examples/verus_vecadd/verus/two_kernel.rs";

pub const GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1: [ProofProperty; 5] = [
    ProofProperty::Bounds,
    ProofProperty::AddressOverflowFreedom,
    ProofProperty::Initialization,
    ProofProperty::RaceFreedom,
    ProofProperty::FunctionalCorrectness,
];

const REQUIRED_SOURCE_PATHS: [&str; 3] = [
    ALPHA_ZETA_SHARED_BODY_PATH_V1,
    ALPHA_ZETA_PERMISSION_MODEL_PATH_V1,
    ALPHA_ZETA_PROOF_HARNESS_PATH_V1,
];
const DEPENDENCY_NAMES: [&str; 3] = ["permission-model", "proof-harness", "shared-body"];

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

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AlphaZetaSourceFileIdentityV1 {
    path: Text,
    byte_len: u64,
    digest: Digest,
}

impl AlphaZetaSourceFileIdentityV1 {
    pub fn measure(path: &str, bytes: &[u8]) -> Result<Self, AlphaZetaProofErrorV1> {
        if !REQUIRED_SOURCE_PATHS.contains(&path) {
            return Err(AlphaZetaProofErrorV1::UnexpectedSourcePath);
        }
        if bytes.is_empty() || bytes.len() as u64 > MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1 {
            return Err(AlphaZetaProofErrorV1::SourceLengthOutOfRange {
                max: MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1,
            });
        }
        Ok(Self {
            path: Text::new("alpha/zeta source path", path)
                .map_err(AlphaZetaProofErrorV1::Model)?,
            byte_len: bytes.len() as u64,
            digest: sha256(bytes),
        })
    }

    pub const fn path(&self) -> &Text {
        &self.path
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    pub const fn digest(&self) -> Digest {
        self.digest
    }

    pub fn matches(&self, path: &str, bytes: &[u8]) -> bool {
        self.path.as_str() == path
            && self.byte_len == bytes.len() as u64
            && self.digest == sha256(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AlphaZetaProofSourcesV1 {
    files: Vec<AlphaZetaSourceFileIdentityV1>,
    source_tree_identity: Digest,
    dependency_tree_identity: Digest,
}

impl AlphaZetaProofSourcesV1 {
    pub fn new(
        mut files: Vec<AlphaZetaSourceFileIdentityV1>,
    ) -> Result<Self, AlphaZetaProofErrorV1> {
        files.sort_unstable_by(|left, right| left.path.cmp(&right.path));
        if files.len() != REQUIRED_SOURCE_PATHS.len() {
            return Err(AlphaZetaProofErrorV1::IncompleteSourceSet);
        }
        if files.windows(2).any(|pair| pair[0].path == pair[1].path) {
            return Err(AlphaZetaProofErrorV1::DuplicateSourcePath);
        }
        let mut actual = files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        let mut required = REQUIRED_SOURCE_PATHS.to_vec();
        actual.sort_unstable();
        required.sort_unstable();
        if actual != required {
            return Err(AlphaZetaProofErrorV1::IncompleteSourceSet);
        }
        Ok(Self {
            source_tree_identity: source_identity(b"FE2AZST\0", &files),
            dependency_tree_identity: source_identity(b"FE2AZDT\0", &files),
            files,
        })
    }

    pub fn files(&self) -> &[AlphaZetaSourceFileIdentityV1] {
        &self.files
    }

    pub const fn source_tree_identity(&self) -> Digest {
        self.source_tree_identity
    }

    pub const fn dependency_tree_identity(&self) -> Digest {
        self.dependency_tree_identity
    }

    pub fn validate_file(&self, path: &str, bytes: &[u8]) -> Result<(), AlphaZetaProofErrorV1> {
        let file = self
            .files
            .iter()
            .find(|file| file.path.as_str() == path)
            .ok_or(AlphaZetaProofErrorV1::UnexpectedSourcePath)?;
        if file.matches(path, bytes) {
            Ok(())
        } else {
            Err(AlphaZetaProofErrorV1::SourceMutation)
        }
    }
}

/// Canonical input identity for one exact alpha or zeta source proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942AlphaZetaProofInputV1 {
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
        if target.source_tree_digest != sources.source_tree_identity {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: "source tree",
            });
        }
        if target.crate_graph_digest != sources.dependency_tree_identity {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch {
                field: "dependency tree",
            });
        }
        if target.effects_contract_digest != effects_identity {
            return Err(AlphaZetaProofErrorV1::IdentityMismatch { field: "effects" });
        }
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
        bytes.extend_from_slice(&(self.sources.files.len() as u16).to_le_bytes());
        for file in &self.sources.files {
            put_text(&mut bytes, file.path.as_str());
            bytes.extend_from_slice(&file.byte_len.to_le_bytes());
            put_digest(&mut bytes, file.digest);
        }
        for identity in [
            self.sources.source_tree_identity,
            self.sources.dependency_tree_identity,
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

pub fn record_reviewed_alpha_zeta_execution_v1(
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
        .files
        .iter()
        .map(|file| file.digest)
        .collect::<BTreeSet<_>>();
    let actual_dependencies = target
        .dependencies()
        .iter()
        .map(|dependency| dependency.identity())
        .collect::<BTreeSet<_>>();
    let actual_names = target
        .dependencies()
        .iter()
        .map(|dependency| dependency.name().as_str())
        .collect::<BTreeSet<_>>();
    if actual_dependencies != expected_dependencies
        || actual_names != DEPENDENCY_NAMES.into_iter().collect()
    {
        return Err(AlphaZetaProofErrorV1::DependencySubstitution);
    }

    let policy = proof.policy();
    if policy.claimed_verifier() != &input.verus || policy.claimed_solver() != &input.solver {
        return Err(AlphaZetaProofErrorV1::ToolSubstitution);
    }
    if policy.model() != &input.model {
        return Err(AlphaZetaProofErrorV1::ModelSubstitution);
    }
    if !policy.approved_axioms().allowed().is_empty() || !policy.requested_axioms().is_empty() {
        return Err(AlphaZetaProofErrorV1::AxiomSubstitution);
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
        source_tree_identity: input.sources.source_tree_identity,
        dependency_tree_identity: input.sources.dependency_tree_identity,
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

fn source_identity(domain: &[u8; 8], files: &[AlphaZetaSourceFileIdentityV1]) -> Digest {
    let mut bytes = Vec::with_capacity(16 + files.len() * 320);
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(&GFX942_ALPHA_ZETA_PROOF_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&(files.len() as u16).to_le_bytes());
    for file in files {
        put_text(&mut bytes, file.path.as_str());
        bytes.extend_from_slice(&file.byte_len.to_le_bytes());
        put_digest(&mut bytes, file.digest);
    }
    sha256(&bytes)
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
    SourceLengthOutOfRange { max: u64 },
    DuplicateSourcePath,
    IncompleteSourceSet,
    SourceMutation,
    ZeroIdentity { field: &'static str },
    IdentityMismatch { field: &'static str },
    UnexpectedTool,
    UnexpectedModel,
    NonceCollision,
    DependencySubstitution,
    ToolSubstitution,
    ModelSubstitution,
    AxiomSubstitution,
    PropertySubstitution,
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
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity is zero"),
            Self::IdentityMismatch { field } => write!(formatter, "{field} identity differs"),
            Self::UnexpectedTool => formatter.write_str("unexpected alpha/zeta proof tool"),
            Self::UnexpectedModel => formatter.write_str("unexpected alpha/zeta proof model"),
            Self::NonceCollision => formatter.write_str("proof-set and proof nonces collide"),
            Self::DependencySubstitution => formatter.write_str("proof dependency substitution"),
            Self::ToolSubstitution => formatter.write_str("proof tool substitution"),
            Self::ModelSubstitution => formatter.write_str("proof model substitution"),
            Self::AxiomSubstitution => formatter.write_str("proof axiom substitution"),
            Self::PropertySubstitution => formatter.write_str("proof property substitution"),
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
