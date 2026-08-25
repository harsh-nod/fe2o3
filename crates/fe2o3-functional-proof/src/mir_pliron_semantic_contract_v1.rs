//! Canonical workload-neutral semantic contract between kernel MIR and PLIRON.
//!
//! Construction is deliberately structural: declarations cannot become proof
//! evidence. The production join must match every field to a live graph, its
//! authenticated MIR receipts, and its mandatory pass reports.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

pub const HARD_MAX_SEMANTIC_DOMAINS_V1: usize = 64;
pub const HARD_MAX_SEMANTIC_ROOTS_V1: usize = 8_192;
pub const HARD_MAX_SEMANTIC_LOOPS_V1: usize = 1_024;
pub const HARD_MAX_SEMANTIC_COLLECTIVES_V1: usize = 1_024;
pub const HARD_MAX_SEMANTIC_OUTPUTS_V1: usize = 4_096;

const CONTRACT_DOMAIN_V1: &[u8] = b"FE2O3/MIR-PLIRON-SEMANTIC-CONTRACT/V1\0";

/// Exact source digest of the reviewed shared Verus theorem and its four
/// workload instantiations. Runtime proof execution remains a separate receipt.
pub const MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1: DigestV1 =
    DigestV1::from_untrusted_bytes([
        0x21, 0xc5, 0xcc, 0x55, 0xf9, 0x5f, 0x4b, 0x38, 0xd6, 0x51, 0x83, 0xf5, 0x91, 0x2c, 0xc0,
        0x86, 0xd8, 0xc8, 0xff, 0x35, 0xbb, 0x81, 0x35, 0xe6, 0xd6, 0xd0, 0xd2, 0x1a, 0x3b, 0xba,
        0xae, 0xa2,
    ]);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticFiniteExtentV1 {
    Static(u64),
    Dynamic {
        symbol: u32,
        inclusive_upper_bound: u64,
    },
}

impl SemanticFiniteExtentV1 {
    pub const fn inclusive_upper_bound(self) -> u64 {
        match self {
            Self::Static(extent) => extent,
            Self::Dynamic {
                inclusive_upper_bound,
                ..
            } => inclusive_upper_bound,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticFiniteDomainV1 {
    identity: DigestV1,
    extents: Box<[SemanticFiniteExtentV1]>,
}

impl SemanticFiniteDomainV1 {
    pub fn new(
        identity: DigestV1,
        extents: Vec<SemanticFiniteExtentV1>,
    ) -> Result<Self, MirPlironSemanticContractErrorV1> {
        let domain = Self {
            identity,
            extents: extents.into_boxed_slice(),
        };
        domain.validate()?;
        Ok(domain)
    }

    pub const fn identity(&self) -> DigestV1 {
        self.identity
    }
    pub fn extents(&self) -> &[SemanticFiniteExtentV1] {
        &self.extents
    }
    pub fn maximum_cardinality(&self) -> Option<u64> {
        self.extents.iter().try_fold(1_u64, |product, extent| {
            product.checked_mul(extent.inclusive_upper_bound())
        })
    }

    fn validate(&self) -> Result<(), MirPlironSemanticContractErrorV1> {
        if self.identity.is_zero() || self.extents.is_empty() || self.extents.len() > 8 {
            return Err(MirPlironSemanticContractErrorV1::InvalidDomain);
        }
        let mut symbols = BTreeSet::new();
        for extent in &self.extents {
            match *extent {
                SemanticFiniteExtentV1::Static(0)
                | SemanticFiniteExtentV1::Dynamic {
                    inclusive_upper_bound: 0,
                    ..
                } => {
                    return Err(MirPlironSemanticContractErrorV1::InvalidDomain);
                }
                SemanticFiniteExtentV1::Dynamic { symbol, .. } if !symbols.insert(symbol) => {
                    return Err(MirPlironSemanticContractErrorV1::InvalidDomain);
                }
                _ => {}
            }
        }
        self.maximum_cardinality()
            .ok_or(MirPlironSemanticContractErrorV1::InvalidDomain)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticScalarTypeV1 {
    Boolean,
    Signed(u16),
    Unsigned(u16),
    Float(u16),
}

impl SemanticScalarTypeV1 {
    fn supported(self) -> bool {
        match self {
            Self::Boolean => true,
            Self::Signed(bits) | Self::Unsigned(bits) => matches!(bits, 8 | 16 | 32 | 64),
            Self::Float(bits) => matches!(bits, 32 | 64),
        }
    }
    fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticIeeeRoundingV1 {
    NearestTiesEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticIeeeExceptionalValueV1 {
    ExactBits,
    CanonicalNan,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticNumericalPolicyV1 {
    ExactBitVector,
    IeeeOperatorCongruence {
        rounding: SemanticIeeeRoundingV1,
        exceptional_values: SemanticIeeeExceptionalValueV1,
    },
}

impl SemanticNumericalPolicyV1 {
    fn supports(self, scalar: SemanticScalarTypeV1) -> bool {
        scalar.supported()
            && matches!(self, Self::IeeeOperatorCongruence { .. }) == scalar.is_float()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypedRootV1 {
    identity: DigestV1,
    commitment: DigestV1,
    domain: DigestV1,
    scalar: SemanticScalarTypeV1,
    numerical_policy: SemanticNumericalPolicyV1,
}

impl SemanticTypedRootV1 {
    pub fn new(
        identity: DigestV1,
        commitment: DigestV1,
        domain: DigestV1,
        scalar: SemanticScalarTypeV1,
        numerical_policy: SemanticNumericalPolicyV1,
    ) -> Result<Self, MirPlironSemanticContractErrorV1> {
        if identity.is_zero()
            || commitment.is_zero()
            || domain.is_zero()
            || !numerical_policy.supports(scalar)
        {
            return Err(MirPlironSemanticContractErrorV1::InvalidTypedRoot);
        }
        Ok(Self {
            identity,
            commitment,
            domain,
            scalar,
            numerical_policy,
        })
    }

    pub const fn identity(self) -> DigestV1 {
        self.identity
    }
    pub const fn commitment(self) -> DigestV1 {
        self.commitment
    }
    pub const fn domain(self) -> DigestV1 {
        self.domain
    }
    pub const fn scalar(self) -> SemanticScalarTypeV1 {
        self.scalar
    }
    pub const fn numerical_policy(self) -> SemanticNumericalPolicyV1 {
        self.numerical_policy
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticLoopDirectionV1 {
    Increasing,
    Decreasing,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticLoopContractV1 {
    identity: DigestV1,
    header_block: u32,
    latch_block: u32,
    exit_block: u32,
    iteration_domain: DigestV1,
    induction: DigestV1,
    lower_bound: DigestV1,
    upper_bound: DigestV1,
    step: DigestV1,
    transition: DigestV1,
    variant: DigestV1,
    direction: SemanticLoopDirectionV1,
    maximum_steps: u64,
}

impl SemanticLoopContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: DigestV1,
        header_block: u32,
        latch_block: u32,
        exit_block: u32,
        iteration_domain: DigestV1,
        induction: DigestV1,
        lower_bound: DigestV1,
        upper_bound: DigestV1,
        step: DigestV1,
        transition: DigestV1,
        variant: DigestV1,
        direction: SemanticLoopDirectionV1,
        maximum_steps: u64,
    ) -> Result<Self, MirPlironSemanticContractErrorV1> {
        if [
            identity,
            iteration_domain,
            induction,
            lower_bound,
            upper_bound,
            step,
            transition,
            variant,
        ]
        .into_iter()
        .any(DigestV1::is_zero)
            || header_block == latch_block
            || header_block == exit_block
            || latch_block == exit_block
            || maximum_steps == 0
        {
            return Err(MirPlironSemanticContractErrorV1::InvalidLoop);
        }
        Ok(Self {
            identity,
            header_block,
            latch_block,
            exit_block,
            iteration_domain,
            induction,
            lower_bound,
            upper_bound,
            step,
            transition,
            variant,
            direction,
            maximum_steps,
        })
    }

    pub const fn identity(&self) -> DigestV1 {
        self.identity
    }
    pub const fn header_block(&self) -> u32 {
        self.header_block
    }
    pub const fn latch_block(&self) -> u32 {
        self.latch_block
    }
    pub const fn exit_block(&self) -> u32 {
        self.exit_block
    }
    pub const fn iteration_domain(&self) -> DigestV1 {
        self.iteration_domain
    }
    pub const fn induction(&self) -> DigestV1 {
        self.induction
    }
    pub const fn lower_bound(&self) -> DigestV1 {
        self.lower_bound
    }
    pub const fn upper_bound(&self) -> DigestV1 {
        self.upper_bound
    }
    pub const fn step(&self) -> DigestV1 {
        self.step
    }
    pub const fn transition(&self) -> DigestV1 {
        self.transition
    }
    pub const fn variant(&self) -> DigestV1 {
        self.variant
    }
    pub const fn direction(&self) -> SemanticLoopDirectionV1 {
        self.direction
    }
    pub const fn maximum_steps(&self) -> u64 {
        self.maximum_steps
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticCollectiveKindV1 {
    FiniteFold,
    FiniteRecurrence,
    PermutationGather,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticEvaluationOrderV1 {
    SequentialAscending,
    SequentialDescending,
    Lexicographic,
    ExplicitTree,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticCoverageBindingV1 {
    TotalView,
    CollectiveContributions,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticCollectiveContractV1 {
    identity: DigestV1,
    kind: SemanticCollectiveKindV1,
    view_identity: DigestV1,
    source_domain: DigestV1,
    target_domain: DigestV1,
    actual: DigestV1,
    expected: DigestV1,
    witness0: DigestV1,
    witness1: DigestV1,
    domain_bound: u64,
    step_bound: u64,
    order: SemanticEvaluationOrderV1,
    coverage: SemanticCoverageBindingV1,
}

impl SemanticCollectiveContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        identity: DigestV1,
        kind: SemanticCollectiveKindV1,
        view_identity: DigestV1,
        source_domain: DigestV1,
        target_domain: DigestV1,
        actual: DigestV1,
        expected: DigestV1,
        witness0: DigestV1,
        witness1: DigestV1,
        domain_bound: u64,
        step_bound: u64,
        order: SemanticEvaluationOrderV1,
        coverage: SemanticCoverageBindingV1,
    ) -> Result<Self, MirPlironSemanticContractErrorV1> {
        if [
            identity,
            view_identity,
            source_domain,
            target_domain,
            actual,
            expected,
            witness0,
            witness1,
        ]
        .into_iter()
        .any(DigestV1::is_zero)
            || domain_bound == 0
            || step_bound == 0
            || step_bound > domain_bound
            || (kind == SemanticCollectiveKindV1::PermutationGather
                && source_domain == target_domain)
            || (kind != SemanticCollectiveKindV1::PermutationGather
                && source_domain != target_domain)
        {
            return Err(MirPlironSemanticContractErrorV1::InvalidCollective);
        }
        Ok(Self {
            identity,
            kind,
            view_identity,
            source_domain,
            target_domain,
            actual,
            expected,
            witness0,
            witness1,
            domain_bound,
            step_bound,
            order,
            coverage,
        })
    }

    pub const fn identity(&self) -> DigestV1 {
        self.identity
    }
    pub const fn kind(&self) -> SemanticCollectiveKindV1 {
        self.kind
    }
    pub const fn view_identity(&self) -> DigestV1 {
        self.view_identity
    }
    pub const fn source_domain(&self) -> DigestV1 {
        self.source_domain
    }
    pub const fn target_domain(&self) -> DigestV1 {
        self.target_domain
    }
    pub const fn actual(&self) -> DigestV1 {
        self.actual
    }
    pub const fn expected(&self) -> DigestV1 {
        self.expected
    }
    pub const fn witness0(&self) -> DigestV1 {
        self.witness0
    }
    pub const fn witness1(&self) -> DigestV1 {
        self.witness1
    }
    pub const fn domain_bound(&self) -> u64 {
        self.domain_bound
    }
    pub const fn step_bound(&self) -> u64 {
        self.step_bound
    }
    pub const fn order(&self) -> SemanticEvaluationOrderV1 {
        self.order
    }
    pub const fn coverage(&self) -> SemanticCoverageBindingV1 {
        self.coverage
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticOutputContractV1 {
    identity: DigestV1,
    view_identity: DigestV1,
    output_domain: DigestV1,
    actual: DigestV1,
    reference: DigestV1,
    auxiliary_roots: Box<[DigestV1]>,
}

impl SemanticOutputContractV1 {
    pub fn new(
        identity: DigestV1,
        view_identity: DigestV1,
        output_domain: DigestV1,
        actual: DigestV1,
        reference: DigestV1,
        auxiliary_roots: Vec<DigestV1>,
    ) -> Result<Self, MirPlironSemanticContractErrorV1> {
        if [identity, view_identity, output_domain, actual, reference]
            .into_iter()
            .any(DigestV1::is_zero)
            || auxiliary_roots.len() > 64
            || auxiliary_roots.iter().any(|root| root.is_zero())
        {
            return Err(MirPlironSemanticContractErrorV1::InvalidOutput);
        }
        Ok(Self {
            identity,
            view_identity,
            output_domain,
            actual,
            reference,
            auxiliary_roots: auxiliary_roots.into_boxed_slice(),
        })
    }

    pub const fn identity(&self) -> DigestV1 {
        self.identity
    }
    pub const fn view_identity(&self) -> DigestV1 {
        self.view_identity
    }
    pub const fn output_domain(&self) -> DigestV1 {
        self.output_domain
    }
    pub const fn actual(&self) -> DigestV1 {
        self.actual
    }
    pub const fn reference(&self) -> DigestV1 {
        self.reference
    }
    pub fn auxiliary_roots(&self) -> &[DigestV1] {
        &self.auxiliary_roots
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPlironSemanticContractV1 {
    safe_reference_mir: DigestV1,
    kernel_mir: DigestV1,
    pliron_evidence: DigestV1,
    domains: Box<[SemanticFiniteDomainV1]>,
    typed_roots: Box<[SemanticTypedRootV1]>,
    loops: Box<[SemanticLoopContractV1]>,
    collectives: Box<[SemanticCollectiveContractV1]>,
    outputs: Box<[SemanticOutputContractV1]>,
}

impl MirPlironSemanticContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        safe_reference_mir: DigestV1,
        kernel_mir: DigestV1,
        pliron_evidence: DigestV1,
        domains: Vec<SemanticFiniteDomainV1>,
        typed_roots: Vec<SemanticTypedRootV1>,
        loops: Vec<SemanticLoopContractV1>,
        collectives: Vec<SemanticCollectiveContractV1>,
        outputs: Vec<SemanticOutputContractV1>,
    ) -> Result<Self, MirPlironSemanticContractErrorV1> {
        let contract = Self {
            safe_reference_mir,
            kernel_mir,
            pliron_evidence,
            domains: domains.into_boxed_slice(),
            typed_roots: typed_roots.into_boxed_slice(),
            loops: loops.into_boxed_slice(),
            collectives: collectives.into_boxed_slice(),
            outputs: outputs.into_boxed_slice(),
        };
        contract.validate()?;
        Ok(contract)
    }

    pub const fn safe_reference_mir(&self) -> DigestV1 {
        self.safe_reference_mir
    }
    pub const fn kernel_mir(&self) -> DigestV1 {
        self.kernel_mir
    }
    pub const fn pliron_evidence(&self) -> DigestV1 {
        self.pliron_evidence
    }
    pub fn domains(&self) -> &[SemanticFiniteDomainV1] {
        &self.domains
    }
    pub fn typed_roots(&self) -> &[SemanticTypedRootV1] {
        &self.typed_roots
    }
    pub fn loops(&self) -> &[SemanticLoopContractV1] {
        &self.loops
    }
    pub fn collectives(&self) -> &[SemanticCollectiveContractV1] {
        &self.collectives
    }
    pub fn outputs(&self) -> &[SemanticOutputContractV1] {
        &self.outputs
    }

    pub fn canonical_sha256(&self) -> DigestV1 {
        let mut digest = Sha256::new();
        put_blob(&mut digest, CONTRACT_DOMAIN_V1);
        for subject in [
            self.safe_reference_mir,
            self.kernel_mir,
            self.pliron_evidence,
        ] {
            put_digest(&mut digest, subject);
        }
        put_u64(&mut digest, self.domains.len() as u64);
        for domain in &self.domains {
            put_digest(&mut digest, domain.identity);
            put_u64(&mut digest, domain.extents.len() as u64);
            for extent in &domain.extents {
                match extent {
                    SemanticFiniteExtentV1::Static(value) => {
                        digest.update([1]);
                        put_u64(&mut digest, *value);
                    }
                    SemanticFiniteExtentV1::Dynamic {
                        symbol,
                        inclusive_upper_bound,
                    } => {
                        digest.update([2]);
                        digest.update(symbol.to_le_bytes());
                        put_u64(&mut digest, *inclusive_upper_bound);
                    }
                }
            }
        }
        put_u64(&mut digest, self.typed_roots.len() as u64);
        for root in &self.typed_roots {
            for value in [root.identity, root.commitment, root.domain] {
                put_digest(&mut digest, value);
            }
            put_scalar(&mut digest, root.scalar);
            put_policy(&mut digest, root.numerical_policy);
        }
        put_u64(&mut digest, self.loops.len() as u64);
        for item in &self.loops {
            put_digest(&mut digest, item.identity);
            for block in [item.header_block, item.latch_block, item.exit_block] {
                digest.update(block.to_le_bytes());
            }
            for value in [
                item.iteration_domain,
                item.induction,
                item.lower_bound,
                item.upper_bound,
                item.step,
                item.transition,
                item.variant,
            ] {
                put_digest(&mut digest, value);
            }
            digest.update([match item.direction {
                SemanticLoopDirectionV1::Increasing => 1,
                SemanticLoopDirectionV1::Decreasing => 2,
            }]);
            put_u64(&mut digest, item.maximum_steps);
        }
        put_u64(&mut digest, self.collectives.len() as u64);
        for item in &self.collectives {
            put_digest(&mut digest, item.identity);
            digest.update([match item.kind {
                SemanticCollectiveKindV1::FiniteFold => 1,
                SemanticCollectiveKindV1::FiniteRecurrence => 2,
                SemanticCollectiveKindV1::PermutationGather => 3,
            }]);
            for value in [
                item.view_identity,
                item.source_domain,
                item.target_domain,
                item.actual,
                item.expected,
                item.witness0,
                item.witness1,
            ] {
                put_digest(&mut digest, value);
            }
            put_u64(&mut digest, item.domain_bound);
            put_u64(&mut digest, item.step_bound);
            digest.update([match item.order {
                SemanticEvaluationOrderV1::SequentialAscending => 1,
                SemanticEvaluationOrderV1::SequentialDescending => 2,
                SemanticEvaluationOrderV1::Lexicographic => 3,
                SemanticEvaluationOrderV1::ExplicitTree => 4,
            }]);
            digest.update([match item.coverage {
                SemanticCoverageBindingV1::TotalView => 1,
                SemanticCoverageBindingV1::CollectiveContributions => 2,
            }]);
        }
        put_u64(&mut digest, self.outputs.len() as u64);
        for item in &self.outputs {
            for value in [
                item.identity,
                item.view_identity,
                item.output_domain,
                item.actual,
                item.reference,
            ] {
                put_digest(&mut digest, value);
            }
            put_u64(&mut digest, item.auxiliary_roots.len() as u64);
            for root in &item.auxiliary_roots {
                put_digest(&mut digest, *root);
            }
        }
        DigestV1::from_untrusted_bytes(digest.finalize().into())
    }

    fn validate(&self) -> Result<(), MirPlironSemanticContractErrorV1> {
        if [
            self.safe_reference_mir,
            self.kernel_mir,
            self.pliron_evidence,
        ]
        .into_iter()
        .any(DigestV1::is_zero)
        {
            return Err(MirPlironSemanticContractErrorV1::MissingSubject);
        }
        require_count(
            "domain",
            self.domains.len(),
            1,
            HARD_MAX_SEMANTIC_DOMAINS_V1,
        )?;
        require_count(
            "typed root",
            self.typed_roots.len(),
            1,
            HARD_MAX_SEMANTIC_ROOTS_V1,
        )?;
        require_count("loop", self.loops.len(), 0, HARD_MAX_SEMANTIC_LOOPS_V1)?;
        require_count(
            "collective",
            self.collectives.len(),
            0,
            HARD_MAX_SEMANTIC_COLLECTIVES_V1,
        )?;
        require_count(
            "output",
            self.outputs.len(),
            1,
            HARD_MAX_SEMANTIC_OUTPUTS_V1,
        )?;

        let mut domains = BTreeSet::new();
        for domain in &self.domains {
            domain.validate()?;
            if !domains.insert(domain.identity) {
                return Err(MirPlironSemanticContractErrorV1::DuplicateIdentity);
            }
        }
        let roots = self
            .typed_roots
            .iter()
            .map(|root| root.identity)
            .collect::<BTreeSet<_>>();
        if roots.len() != self.typed_roots.len() {
            return Err(MirPlironSemanticContractErrorV1::DuplicateIdentity);
        }
        for root in &self.typed_roots {
            if !domains.contains(&root.domain) || !root.numerical_policy.supports(root.scalar) {
                return Err(MirPlironSemanticContractErrorV1::InvalidTypedRoot);
            }
        }
        let cardinality = |identity| {
            self.domains
                .iter()
                .find(|domain| domain.identity == identity)
                .and_then(SemanticFiniteDomainV1::maximum_cardinality)
        };
        let mut identities = BTreeSet::new();
        let mut used_roots = BTreeSet::new();
        let mut used_domains = self
            .typed_roots
            .iter()
            .map(|root| root.domain)
            .collect::<BTreeSet<_>>();
        let mut loop_edges = BTreeSet::new();
        for item in &self.loops {
            if !identities.insert(item.identity)
                || !loop_edges.insert((item.header_block, item.latch_block))
                || item.maximum_steps
                    > cardinality(item.iteration_domain)
                        .ok_or(MirPlironSemanticContractErrorV1::InvalidLoop)?
            {
                return Err(MirPlironSemanticContractErrorV1::InvalidLoop);
            }
            bind_declared_roots(
                &roots,
                &mut used_roots,
                [
                    item.induction,
                    item.lower_bound,
                    item.upper_bound,
                    item.step,
                ],
            );
            used_domains.insert(item.iteration_domain);
        }
        for item in &self.collectives {
            if !identities.insert(item.identity)
                || item.view_identity.is_zero()
                || item.domain_bound
                    != cardinality(item.source_domain)
                        .ok_or(MirPlironSemanticContractErrorV1::InvalidCollective)?
                || item.step_bound
                    > cardinality(item.source_domain)
                        .ok_or(MirPlironSemanticContractErrorV1::InvalidCollective)?
                || cardinality(item.target_domain).is_none()
                || (item.kind == SemanticCollectiveKindV1::PermutationGather
                    && cardinality(item.source_domain) != cardinality(item.target_domain))
            {
                return Err(MirPlironSemanticContractErrorV1::InvalidCollective);
            }
            bind_roots(
                &roots,
                &mut used_roots,
                [item.actual, item.expected, item.witness0, item.witness1],
            )?;
            used_domains.insert(item.source_domain);
            used_domains.insert(item.target_domain);
        }
        for item in &self.outputs {
            if !identities.insert(item.identity)
                || item.view_identity.is_zero()
                || !domains.contains(&item.output_domain)
            {
                return Err(MirPlironSemanticContractErrorV1::InvalidOutput);
            }
            bind_roots(&roots, &mut used_roots, [item.actual, item.reference])?;
            for auxiliary in &item.auxiliary_roots {
                bind_roots(&roots, &mut used_roots, [*auxiliary])?;
            }
            used_domains.insert(item.output_domain);
            let actual = root(&self.typed_roots, item.actual);
            let reference = root(&self.typed_roots, item.reference);
            if actual.domain != item.output_domain
                || reference.domain != item.output_domain
                || actual.scalar != reference.scalar
                || actual.numerical_policy != reference.numerical_policy
            {
                return Err(MirPlironSemanticContractErrorV1::InvalidOutput);
            }
        }
        if used_roots != roots {
            return Err(MirPlironSemanticContractErrorV1::UnusedTypedRoot);
        }
        if used_domains != domains {
            return Err(MirPlironSemanticContractErrorV1::UnusedDomain);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirPlironSemanticContractErrorV1 {
    MissingSubject,
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    InvalidDomain,
    InvalidTypedRoot,
    InvalidLoop,
    InvalidCollective,
    InvalidOutput,
    DuplicateIdentity,
    UnknownTypedRoot,
    UnusedTypedRoot,
    UnusedDomain,
}

impl fmt::Display for MirPlironSemanticContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSubject => formatter.write_str("semantic contract is missing a MIR or PLIRON subject"),
            Self::ResourceLimit { resource, limit, actual } => write!(formatter,
                "semantic contract {resource} count {actual} exceeds the admitted limit {limit}"),
            Self::InvalidDomain => formatter.write_str("semantic contract contains an invalid finite domain"),
            Self::InvalidTypedRoot => formatter.write_str("semantic contract contains an invalid typed root"),
            Self::InvalidLoop => formatter.write_str("semantic contract contains an invalid bounded-loop obligation"),
            Self::InvalidCollective => formatter.write_str("semantic contract contains an invalid collective obligation"),
            Self::InvalidOutput => formatter.write_str("semantic contract contains an invalid final-output obligation"),
            Self::DuplicateIdentity => formatter.write_str("semantic contract reuses an authority-relevant identity"),
            Self::UnknownTypedRoot => formatter.write_str("semantic contract refers to a typed root that it does not bind"),
            Self::UnusedTypedRoot => formatter.write_str("semantic contract contains a typed root unused by every loop, collective, and output"),
            Self::UnusedDomain => formatter.write_str("semantic contract contains a finite domain unused by every typed root, loop, collective, and output"),
        }
    }
}

impl Error for MirPlironSemanticContractErrorV1 {}

fn require_count(
    resource: &'static str,
    actual: usize,
    minimum: usize,
    limit: usize,
) -> Result<(), MirPlironSemanticContractErrorV1> {
    if actual < minimum || actual > limit {
        return Err(MirPlironSemanticContractErrorV1::ResourceLimit {
            resource,
            limit,
            actual,
        });
    }
    Ok(())
}

fn bind_roots<const N: usize>(
    roots: &BTreeSet<DigestV1>,
    used: &mut BTreeSet<DigestV1>,
    values: [DigestV1; N],
) -> Result<(), MirPlironSemanticContractErrorV1> {
    for value in values {
        if !roots.contains(&value) {
            return Err(MirPlironSemanticContractErrorV1::UnknownTypedRoot);
        }
        used.insert(value);
    }
    Ok(())
}

fn bind_declared_roots<const N: usize>(
    roots: &BTreeSet<DigestV1>,
    used: &mut BTreeSet<DigestV1>,
    values: [DigestV1; N],
) {
    for value in values {
        if roots.contains(&value) {
            used.insert(value);
        }
    }
}

fn root(roots: &[SemanticTypedRootV1], commitment: DigestV1) -> SemanticTypedRootV1 {
    *roots
        .iter()
        .find(|root| root.identity == commitment)
        .expect("typed-root membership was validated")
}

fn put_blob(digest: &mut Sha256, bytes: &[u8]) {
    put_u64(digest, bytes.len() as u64);
    digest.update(bytes);
}
fn put_digest(digest: &mut Sha256, value: DigestV1) {
    put_blob(digest, value.as_bytes());
}
fn put_u64(digest: &mut Sha256, value: u64) {
    digest.update(value.to_le_bytes());
}

fn put_scalar(digest: &mut Sha256, scalar: SemanticScalarTypeV1) {
    match scalar {
        SemanticScalarTypeV1::Boolean => digest.update([1, 1, 0]),
        SemanticScalarTypeV1::Signed(bits) => {
            digest.update([2]);
            digest.update(bits.to_le_bytes());
        }
        SemanticScalarTypeV1::Unsigned(bits) => {
            digest.update([3]);
            digest.update(bits.to_le_bytes());
        }
        SemanticScalarTypeV1::Float(bits) => {
            digest.update([4]);
            digest.update(bits.to_le_bytes());
        }
    }
}

fn put_policy(digest: &mut Sha256, policy: SemanticNumericalPolicyV1) {
    match policy {
        SemanticNumericalPolicyV1::ExactBitVector => digest.update([1, 0, 0]),
        SemanticNumericalPolicyV1::IeeeOperatorCongruence {
            rounding,
            exceptional_values,
        } => {
            digest.update([2]);
            digest.update([match rounding {
                SemanticIeeeRoundingV1::NearestTiesEven => 1,
                SemanticIeeeRoundingV1::TowardZero => 2,
                SemanticIeeeRoundingV1::TowardPositive => 3,
                SemanticIeeeRoundingV1::TowardNegative => 4,
            }]);
            digest.update([match exceptional_values {
                SemanticIeeeExceptionalValueV1::ExactBits => 1,
                SemanticIeeeExceptionalValueV1::CanonicalNan => 2,
            }]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }

    fn root(identity: u8, commitment: u8, domain: DigestV1) -> SemanticTypedRootV1 {
        SemanticTypedRootV1::new(
            digest(identity),
            digest(commitment),
            domain,
            SemanticScalarTypeV1::Unsigned(32),
            SemanticNumericalPolicyV1::ExactBitVector,
        )
        .unwrap()
    }

    fn minimal_contract(view: u8) -> MirPlironSemanticContractV1 {
        let domain = digest(10);
        MirPlironSemanticContractV1::new(
            digest(1),
            digest(2),
            digest(3),
            vec![
                SemanticFiniteDomainV1::new(domain, vec![SemanticFiniteExtentV1::Static(16)])
                    .unwrap(),
            ],
            vec![root(20, 30, domain), root(21, 30, domain)],
            vec![],
            vec![],
            vec![
                SemanticOutputContractV1::new(
                    digest(40),
                    digest(view),
                    domain,
                    digest(20),
                    digest(21),
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn distinct_ssa_roots_may_share_one_expression_commitment() {
        let contract = minimal_contract(41);
        assert_ne!(
            contract.typed_roots()[0].identity(),
            contract.typed_roots()[1].identity()
        );
        assert_eq!(
            contract.typed_roots()[0].commitment(),
            contract.typed_roots()[1].commitment()
        );
        assert_ne!(contract.canonical_sha256(), DigestV1::ZERO);
    }

    #[test]
    fn every_authority_relevant_mutation_changes_the_contract_identity() {
        let first = minimal_contract(41);
        let changed_view = minimal_contract(42);
        assert_ne!(first.canonical_sha256(), changed_view.canonical_sha256());

        let domain = digest(10);
        let changed_commitment = MirPlironSemanticContractV1::new(
            digest(1),
            digest(2),
            digest(3),
            vec![
                SemanticFiniteDomainV1::new(domain, vec![SemanticFiniteExtentV1::Static(16)])
                    .unwrap(),
            ],
            vec![root(20, 31, domain), root(21, 30, domain)],
            vec![],
            vec![],
            vec![
                SemanticOutputContractV1::new(
                    digest(40),
                    digest(41),
                    domain,
                    digest(20),
                    digest(21),
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        assert_ne!(
            first.canonical_sha256(),
            changed_commitment.canonical_sha256()
        );
    }

    #[test]
    fn unknown_unused_and_numerically_incompatible_roots_fail_closed() {
        let domain = digest(10);
        let output = |actual| {
            SemanticOutputContractV1::new(
                digest(40),
                digest(41),
                domain,
                actual,
                digest(21),
                vec![],
            )
            .unwrap()
        };
        assert_eq!(
            MirPlironSemanticContractV1::new(
                digest(1),
                digest(2),
                digest(3),
                vec![
                    SemanticFiniteDomainV1::new(domain, vec![SemanticFiniteExtentV1::Static(16)],)
                        .unwrap()
                ],
                vec![root(20, 30, domain), root(21, 30, domain)],
                vec![],
                vec![],
                vec![output(digest(99))],
            )
            .unwrap_err(),
            MirPlironSemanticContractErrorV1::UnknownTypedRoot
        );

        assert_eq!(
            SemanticTypedRootV1::new(
                digest(20),
                digest(30),
                domain,
                SemanticScalarTypeV1::Float(32),
                SemanticNumericalPolicyV1::ExactBitVector,
            )
            .unwrap_err(),
            MirPlironSemanticContractErrorV1::InvalidTypedRoot
        );
    }

    #[test]
    fn malformed_dynamic_domains_and_collective_bounds_are_rejected() {
        assert_eq!(
            SemanticFiniteDomainV1::new(
                digest(10),
                vec![
                    SemanticFiniteExtentV1::Dynamic {
                        symbol: 7,
                        inclusive_upper_bound: 16,
                    },
                    SemanticFiniteExtentV1::Dynamic {
                        symbol: 7,
                        inclusive_upper_bound: 8,
                    },
                ],
            )
            .unwrap_err(),
            MirPlironSemanticContractErrorV1::InvalidDomain
        );
        assert_eq!(
            SemanticCollectiveContractV1::new(
                digest(1),
                SemanticCollectiveKindV1::FiniteFold,
                digest(2),
                digest(3),
                digest(3),
                digest(4),
                digest(5),
                digest(6),
                digest(7),
                8,
                9,
                SemanticEvaluationOrderV1::SequentialAscending,
                SemanticCoverageBindingV1::TotalView,
            )
            .unwrap_err(),
            MirPlironSemanticContractErrorV1::InvalidCollective
        );
    }
}
