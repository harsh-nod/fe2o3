//! Canonical evidence for the eight-pass production semantic middle end.
//!
//! V5 is a new immutable wire domain. It preserves V4 decoding unchanged while
//! adding hierarchical ownership, exact coverage accounting, and reconciliation
//! of retained typed recipes with their live PLIRON commitments. The evidence
//! represents internal analyses and MIR operator-congruence obligations only.

use std::{error::Error, fmt, ops::Range};

use fe2o3_kernel_analysis::{KernelCheckPassKindV1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2};
use sha2::{Digest, Sha256};

use super::middle_end_evidence_v4::{
    derive_ranked_kernel_identity, revalidated_source_semantic_identity,
};
use super::{
    ProductionMiddleEndEvidenceCodecErrorV4, ProductionRankedKernelErrorV1,
    ProductionRankedKernelLoweringInputV1, ProductionSemanticMirOwnerV1,
    ProductionTypedSemanticObligationSummaryV2, typed_semantic_commitment_reconciliation_v2,
    typed_semantic_obligation_summary_v2,
};

const MAGIC_V5: [u8; 8] = *b"F2MEV5\0\0";
const VERSION_V5: u16 = 5;
const FLAGS_V5: u16 = 0;
const ASSURANCE_INTERNAL_CHECKS_ONLY_V5: u8 = 1;
const SEMANTIC_OWNER_REVALIDATED_V5: u8 = 1;
const CLEAN_STATUS_V5: u8 = 1;
const PASS_COUNT_V5: usize = 8;
const PASS_RECORD_BYTES_V5: usize = 10;
const SHA256_BYTES: usize = 32;
const COVERAGE_COUNTERS_V5: usize = 4;
const SEMANTIC_COUNTERS_V5: usize = 6;
const TYPED_SUMMARY_COUNTERS_V5: usize = 10;
const RECONCILIATION_COUNTERS_V5: usize = 2;
const IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V5\0";

/// Stable V5 wire domain, distinct from the frozen V4 domain.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5: &[u8] =
    b"fe2o3.production-middle-end-evidence.v5";

/// Fixed policy: internal analyses and MIR operator congruence only.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5: &[u8] =
    b"fe2o3.internal-analysis-and-mir-operator-congruence-obligations-only.v5";

/// Aggregate hard limit for one canonical V5 record.
pub const MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5: usize = 4 * 1024 * 1024;

const FIXED_RECORD_BYTES_V5: usize = MAGIC_V5.len()
    + 2
    + 2
    + 8
    + 4
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len()
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len()
    + 1
    + 1
    + 2
    + SHA256_BYTES
    + SHA256_BYTES
    + 4
    + 1
    + PASS_COUNT_V5 * PASS_RECORD_BYTES_V5
    + COVERAGE_COUNTERS_V5 * 8
    + SEMANTIC_COUNTERS_V5 * 8
    + TYPED_SUMMARY_COUNTERS_V5 * 8
    + RECONCILIATION_COUNTERS_V5 * 8
    + SHA256_BYTES
    + SHA256_BYTES;

/// Maximum deterministic ranked-IR bytes in one complete record.
pub const MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5: usize =
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 - FIXED_RECORD_BYTES_V5;

/// Assurance represented by V5. It is deliberately not a correctness proof.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionMiddleEndAssuranceV5 {
    InternalAnalysesAndMirOperatorCongruenceObligationsOnly,
}

/// One pass in the immutable V5 production order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionMiddleEndEvidencePassV5 {
    TensorLayout,
    MemoryBounds,
    AtomicLegality,
    RaceFreedom,
    HierarchicalOwnership,
    BarrierConvergence,
    WorkgroupMemory,
    SemanticRefinement,
}

impl ProductionMiddleEndEvidencePassV5 {
    const fn tag(self) -> u8 {
        match self {
            Self::TensorLayout => 1,
            Self::MemoryBounds => 2,
            Self::AtomicLegality => 3,
            Self::RaceFreedom => 4,
            Self::HierarchicalOwnership => 8,
            Self::BarrierConvergence => 5,
            Self::WorkgroupMemory => 6,
            Self::SemanticRefinement => 7,
        }
    }

    const fn from_analysis(pass: KernelCheckPassKindV1) -> Option<Self> {
        match pass {
            KernelCheckPassKindV1::TensorLayout => Some(Self::TensorLayout),
            KernelCheckPassKindV1::MemoryBounds => Some(Self::MemoryBounds),
            KernelCheckPassKindV1::AtomicLegality => Some(Self::AtomicLegality),
            KernelCheckPassKindV1::RaceFreedom => Some(Self::RaceFreedom),
            KernelCheckPassKindV1::HierarchicalOwnership => Some(Self::HierarchicalOwnership),
            KernelCheckPassKindV1::BarrierConvergence => Some(Self::BarrierConvergence),
            KernelCheckPassKindV1::WorkgroupMemory => Some(Self::WorkgroupMemory),
            KernelCheckPassKindV1::SemanticRefinement => Some(Self::SemanticRefinement),
            KernelCheckPassKindV1::Structural | KernelCheckPassKindV1::ControlFlow => None,
        }
    }
}

/// Exact pass order serialized by every V5 record.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5: [ProductionMiddleEndEvidencePassV5;
    PASS_COUNT_V5] = [
    ProductionMiddleEndEvidencePassV5::TensorLayout,
    ProductionMiddleEndEvidencePassV5::MemoryBounds,
    ProductionMiddleEndEvidencePassV5::AtomicLegality,
    ProductionMiddleEndEvidencePassV5::RaceFreedom,
    ProductionMiddleEndEvidencePassV5::HierarchicalOwnership,
    ProductionMiddleEndEvidencePassV5::BarrierConvergence,
    ProductionMiddleEndEvidencePassV5::WorkgroupMemory,
    ProductionMiddleEndEvidencePassV5::SemanticRefinement,
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndPassSuccessV5 {
    pass: ProductionMiddleEndEvidencePassV5,
}

impl ProductionMiddleEndPassSuccessV5 {
    const fn new(pass: ProductionMiddleEndEvidencePassV5) -> Self {
        Self { pass }
    }

    pub const fn pass(self) -> ProductionMiddleEndEvidencePassV5 {
        self.pass
    }

    pub const fn is_clean(self) -> bool {
        true
    }

    pub const fn finding_count(self) -> u32 {
        0
    }

    pub const fn grants_compiler_refinement_authority(self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(self) -> bool {
        false
    }
}

const PASS_SUCCESSES_V5: [ProductionMiddleEndPassSuccessV5; PASS_COUNT_V5] = [
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::TensorLayout),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::MemoryBounds),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::AtomicLegality),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::RaceFreedom),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::HierarchicalOwnership),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::BarrierConvergence),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::WorkgroupMemory),
    ProductionMiddleEndPassSuccessV5::new(ProductionMiddleEndEvidencePassV5::SemanticRefinement),
];

/// Exact declared/proved coverage counts from the ownership report.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndCoverageSummaryV5 {
    total_view_declared: u64,
    total_view_proved: u64,
    collective_contributions_declared: u64,
    collective_contributions_proved: u64,
}

impl ProductionMiddleEndCoverageSummaryV5 {
    pub const fn total_view_declared(self) -> u64 {
        self.total_view_declared
    }

    pub const fn total_view_proved(self) -> u64 {
        self.total_view_proved
    }

    pub const fn collective_contributions_declared(self) -> u64 {
        self.collective_contributions_declared
    }

    pub const fn collective_contributions_proved(self) -> u64 {
        self.collective_contributions_proved
    }

    /// A clean zero-count report is never promoted into a coverage claim.
    pub const fn has_non_vacuous_total_view_proof(self) -> bool {
        self.total_view_declared != 0 && self.total_view_declared == self.total_view_proved
    }

    /// Participation is not a proof of a collective operator or final value.
    pub const fn has_non_vacuous_collective_contribution_proof(self) -> bool {
        self.collective_contributions_declared != 0
            && self.collective_contributions_declared == self.collective_contributions_proved
    }

    pub const fn grants_collective_value_authority(self) -> bool {
        false
    }
}

/// Exact non-vacuous obligation counts from the semantic pass.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndSemanticSummaryV5 {
    reference_obligations_declared: u64,
    reference_obligations_proved: u64,
    effect_contracts_declared: u64,
    effect_contracts_proved: u64,
    collective_contracts_declared: u64,
    collective_contracts_proved: u64,
}

impl ProductionMiddleEndSemanticSummaryV5 {
    pub const fn reference_obligations_declared(self) -> u64 {
        self.reference_obligations_declared
    }

    pub const fn reference_obligations_proved(self) -> u64 {
        self.reference_obligations_proved
    }

    pub const fn effect_contracts_declared(self) -> u64 {
        self.effect_contracts_declared
    }

    pub const fn effect_contracts_proved(self) -> u64 {
        self.effect_contracts_proved
    }

    pub const fn collective_contracts_declared(self) -> u64 {
        self.collective_contracts_declared
    }

    pub const fn collective_contracts_proved(self) -> u64 {
        self.collective_contracts_proved
    }

    pub const fn has_non_vacuous_reference_proof(self) -> bool {
        self.reference_obligations_declared != 0
            && self.reference_obligations_declared == self.reference_obligations_proved
    }

    pub const fn has_non_vacuous_effect_proof(self) -> bool {
        self.effect_contracts_declared != 0
            && self.effect_contracts_declared == self.effect_contracts_proved
    }

    pub const fn has_non_vacuous_collective_value_proof(self) -> bool {
        self.collective_contracts_declared != 0
            && self.collective_contracts_declared == self.collective_contracts_proved
    }

    pub const fn grants_target_or_hardware_value_authority(self) -> bool {
        false
    }
}

/// Exact typed-recipe/live-PLIRON commitment reconciliation retained by V5.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndTypedSemanticReconciliationV5 {
    recipe_expression_roots: u64,
    pliron_commitment_roots: u64,
    ordered_commitments_sha256: [u8; SHA256_BYTES],
}

impl ProductionMiddleEndTypedSemanticReconciliationV5 {
    pub const fn recipe_expression_roots(self) -> u64 {
        self.recipe_expression_roots
    }

    pub const fn pliron_commitment_roots(self) -> u64 {
        self.pliron_commitment_roots
    }

    pub const fn ordered_commitments_sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.ordered_commitments_sha256
    }

    pub const fn is_exact(self) -> bool {
        self.recipe_expression_roots == self.pliron_commitment_roots
    }

    pub const fn grants_arithmetic_interpretation_authority(self) -> bool {
        false
    }

    pub const fn grants_target_value_authority(self) -> bool {
        false
    }
}

/// Domain-separated identity of one exact canonical V5 record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndEvidenceIdentityV5 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl ProductionMiddleEndEvidenceIdentityV5 {
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        if self.byte_len != bytes.len() as u64 || bytes.len() < SHA256_BYTES {
            return false;
        }
        let terminal = bytes.len() - SHA256_BYTES;
        bytes[terminal..] == self.sha256
            && derive_evidence_identity_v5(&bytes[..terminal]) == Some(self.sha256)
    }
}

#[derive(Eq, PartialEq)]
pub struct InertProductionMiddleEndEvidenceV5 {
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir_range: Range<usize>,
    coverage: ProductionMiddleEndCoverageSummaryV5,
    semantics: ProductionMiddleEndSemanticSummaryV5,
    typed_summary: ProductionTypedSemanticObligationSummaryV2,
    reconciliation: ProductionMiddleEndTypedSemanticReconciliationV5,
    identity: ProductionMiddleEndEvidenceIdentityV5,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionMiddleEndEvidenceV5 {
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV5> {
        if bytes.len() > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::TooLarge {
                actual: bytes.len(),
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
            });
        }
        let mut reader = ReaderV5::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != VERSION_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::UnsupportedVersion(
                version,
            ));
        }
        let flags = reader.u16()?;
        if flags != FLAGS_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::UnsupportedFlags(
                flags,
            ));
        }
        let declared_len = reader.u64()?;
        let declared_len_usize = usize::try_from(declared_len)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::InvalidLength(declared_len))?;
        if declared_len_usize > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::TooLarge {
                actual: declared_len_usize,
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
            });
        }
        match declared_len_usize.cmp(&bytes.len()) {
            std::cmp::Ordering::Greater => {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::Truncated);
            }
            std::cmp::Ordering::Less => {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::TrailingBytes);
            }
            std::cmp::Ordering::Equal => {}
        }
        if declared_len_usize < FIXED_RECORD_BYTES_V5 + 1 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidLength(
                declared_len,
            ));
        }
        if reader.u32()? != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonzeroReserved);
        }
        let domain_len = usize::from(reader.u16()?);
        if domain_len != PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len()
            || reader.take(domain_len)? != PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidDomain);
        }
        let policy_len = usize::from(reader.u16()?);
        if policy_len != PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len()
            || reader.take(policy_len)? != PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPolicy);
        }
        if reader.u8()? != ASSURANCE_INTERNAL_CHECKS_ONLY_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidAssurance);
        }
        if reader.u8()? != SEMANTIC_OWNER_REVALIDATED_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::SemanticOwnerNotRevalidated);
        }
        if reader.u16()? != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonzeroReserved);
        }
        let source_semantic_identity = reader.fixed::<SHA256_BYTES>()?;
        if source_semantic_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroSemanticIdentity);
        }
        let ranked_kernel_identity = reader.fixed::<SHA256_BYTES>()?;
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroRankedKernelIdentity);
        }
        let ranked_ir_len = usize::try_from(reader.u32()?).map_err(|_| {
            ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge {
                actual: usize::MAX,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
            }
        })?;
        if ranked_ir_len > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge {
                actual: ranked_ir_len,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
            });
        }
        let ranked_ir_start = reader.offset();
        let ranked_ir_bytes = reader.take(ranked_ir_len)?;
        validate_ranked_ir_v5(ranked_ir_bytes)?;
        let ranked_ir_end = reader.offset();

        let pass_count = reader.u8()?;
        if usize::from(pass_count) != PASS_COUNT_V5 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassCount(
                pass_count,
            ));
        }
        for (index, expected) in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5
            .iter()
            .copied()
            .enumerate()
        {
            let actual = reader.u8()?;
            if actual != expected.tag() {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassOrder {
                    index,
                    expected,
                    actual,
                });
            }
            if reader.u8()? != CLEAN_STATUS_V5 || reader.u32()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::PassNotClean(
                    expected,
                ));
            }
            if reader.u8()? != 0 || reader.u8()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::AuthorityClaim(
                    expected,
                ));
            }
            if reader.u16()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonzeroReserved);
            }
        }

        let coverage = ProductionMiddleEndCoverageSummaryV5 {
            total_view_declared: reader.u64()?,
            total_view_proved: reader.u64()?,
            collective_contributions_declared: reader.u64()?,
            collective_contributions_proved: reader.u64()?,
        };
        validate_coverage_v5(coverage)?;
        let semantics = ProductionMiddleEndSemanticSummaryV5 {
            reference_obligations_declared: reader.u64()?,
            reference_obligations_proved: reader.u64()?,
            effect_contracts_declared: reader.u64()?,
            effect_contracts_proved: reader.u64()?,
            collective_contracts_declared: reader.u64()?,
            collective_contracts_proved: reader.u64()?,
        };
        validate_semantics_v5(semantics)?;
        let typed_summary = ProductionTypedSemanticObligationSummaryV2 {
            expression_roots: u64_to_usize(reader.u64()?)?,
            expression_nodes: u64_to_usize(reader.u64()?)?,
            arithmetic_operations: u64_to_usize(reader.u64()?)?,
            comparisons: u64_to_usize(reader.u64()?)?,
            selects: u64_to_usize(reader.u64()?)?,
            casts: u64_to_usize(reader.u64()?)?,
            checked_operations: u64_to_usize(reader.u64()?)?,
            statically_discharged_domain_roots: u64_to_usize(reader.u64()?)?,
            exact_bitvector_operator_congruence_roots: u64_to_usize(reader.u64()?)?,
            exact_ieee_operator_congruence_roots: u64_to_usize(reader.u64()?)?,
        };
        validate_typed_summary_v5(typed_summary)?;
        let reconciliation = ProductionMiddleEndTypedSemanticReconciliationV5 {
            recipe_expression_roots: reader.u64()?,
            pliron_commitment_roots: reader.u64()?,
            ordered_commitments_sha256: reader.fixed::<SHA256_BYTES>()?,
        };
        validate_reconciliation_v5(typed_summary, reconciliation)?;

        let terminal_offset = reader.offset();
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if declared_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroIdentity);
        }
        if !reader.is_empty() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::TrailingBytes);
        }
        if derive_evidence_identity_v5(&bytes[..terminal_offset]) != Some(declared_identity) {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::IdentityMismatch);
        }
        let ranked_ir = std::str::from_utf8(ranked_ir_bytes)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::InvalidRankedIrUtf8)?;
        let reconstructed = encode_record_v5(
            source_semantic_identity,
            ranked_kernel_identity,
            ranked_ir,
            coverage,
            semantics,
            typed_summary,
            reconciliation,
        )?;
        if reconstructed.canonical_bytes.as_ref() != bytes {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonCanonical);
        }
        Ok(Self {
            source_semantic_identity,
            ranked_kernel_identity,
            ranked_ir_range: ranked_ir_start..ranked_ir_end,
            coverage,
            semantics,
            typed_summary,
            reconciliation,
            identity: reconstructed.identity,
            canonical_bytes: reconstructed.canonical_bytes,
        })
    }

    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV5 {
        ProductionMiddleEndAssuranceV5::InternalAnalysesAndMirOperatorCongruenceObligationsOnly
    }

    pub const fn policy(&self) -> &'static [u8] {
        PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5
    }

    pub const fn source_semantic_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.source_semantic_identity
    }

    pub const fn ranked_kernel_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.ranked_kernel_identity
    }

    pub fn ranked_ir(&self) -> &str {
        std::str::from_utf8(&self.canonical_bytes[self.ranked_ir_range.clone()])
            .expect("validated V5 ranked IR remains UTF-8")
    }

    pub const fn pass_successes(
        &self,
    ) -> &'static [ProductionMiddleEndPassSuccessV5; PASS_COUNT_V5] {
        &PASS_SUCCESSES_V5
    }

    pub const fn coverage_summary(&self) -> ProductionMiddleEndCoverageSummaryV5 {
        self.coverage
    }

    pub const fn semantic_summary(&self) -> ProductionMiddleEndSemanticSummaryV5 {
        self.semantics
    }

    pub const fn typed_semantic_summary(&self) -> ProductionTypedSemanticObligationSummaryV2 {
        self.typed_summary
    }

    pub const fn typed_semantic_reconciliation(
        &self,
    ) -> ProductionMiddleEndTypedSemanticReconciliationV5 {
        self.reconciliation
    }

    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV5 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    pub const fn claims_verus_verification(&self) -> bool {
        false
    }

    pub const fn claims_full_arithmetic_correctness(&self) -> bool {
        false
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub const fn grants_target_value_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertProductionMiddleEndEvidenceV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionMiddleEndEvidenceV5")
            .field("identity", &self.identity)
            .field("coverage", &self.coverage)
            .field("semantics", &self.semantics)
            .field("typed_summary", &self.typed_summary)
            .field("reconciliation", &self.reconciliation)
            .finish_non_exhaustive()
    }
}

/// Move-only V5 evidence constructed while both owners and the PLIRON graph live.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionMiddleEndEvidenceV5;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionMiddleEndEvidenceV5>();
/// ```
#[must_use = "dropping V5 middle-end evidence abandons the live-produced stage record"]
pub struct ProductionMiddleEndEvidenceV5 {
    inert: InertProductionMiddleEndEvidenceV5,
}

impl ProductionMiddleEndEvidenceV5 {
    pub fn try_new(
        semantic: &ProductionSemanticMirOwnerV1,
        ranked: &ProductionRankedKernelLoweringInputV1,
        deterministic_ranked_ir: &str,
    ) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV5> {
        validate_ranked_ir_v5(deterministic_ranked_ir.as_bytes())?;
        let source_semantic_identity = revalidated_source_semantic_identity(semantic)
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::HistoricalV4)?;
        ranked
            .revalidate_structure()
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::RankedKernel)?;
        validate_v5_live_reports(ranked)?;
        let observed = ranked.ownership_report().coverage_summary();
        let coverage = ProductionMiddleEndCoverageSummaryV5 {
            total_view_declared: usize_to_u64(observed.total_view_declared())?,
            total_view_proved: usize_to_u64(observed.total_view_proved())?,
            collective_contributions_declared: usize_to_u64(
                observed.collective_contributions_declared(),
            )?,
            collective_contributions_proved: usize_to_u64(
                observed.collective_contributions_proved(),
            )?,
        };
        validate_coverage_v5(coverage)?;
        let semantic_report = ranked.semantic_report();
        let effect_report = semantic_report.effect_refinement();
        let semantics = ProductionMiddleEndSemanticSummaryV5 {
            reference_obligations_declared: usize_to_u64(
                semantic_report.reference_obligation_count(),
            )?,
            reference_obligations_proved: usize_to_u64(
                semantic_report.proved_reference_obligation_count(),
            )?,
            effect_contracts_declared: usize_to_u64(effect_report.contract_count())?,
            effect_contracts_proved: usize_to_u64(effect_report.proved_contract_count())?,
            collective_contracts_declared: usize_to_u64(
                semantic_report.collective_contract_count(),
            )?,
            collective_contracts_proved: usize_to_u64(
                semantic_report.proved_collective_contract_count(),
            )?,
        };
        validate_semantics_v5(semantics)?;
        let typed_summary = typed_semantic_obligation_summary_v2(ranked.kernel())
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::RankedKernel)?;
        validate_typed_summary_v5(typed_summary)?;
        let observed_reconciliation = typed_semantic_commitment_reconciliation_v2(ranked)
            .map_err(ProductionMiddleEndEvidenceCodecErrorV5::RankedKernel)?;
        let reconciliation = ProductionMiddleEndTypedSemanticReconciliationV5 {
            recipe_expression_roots: usize_to_u64(
                observed_reconciliation.recipe_expression_roots(),
            )?,
            pliron_commitment_roots: usize_to_u64(
                observed_reconciliation.pliron_commitment_roots(),
            )?,
            ordered_commitments_sha256: *observed_reconciliation.ordered_commitments_sha256(),
        };
        validate_reconciliation_v5(typed_summary, reconciliation)?;
        let ranked_kernel_identity = derive_ranked_kernel_identity(ranked);
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroRankedKernelIdentity);
        }
        let encoded = encode_record_v5(
            source_semantic_identity,
            ranked_kernel_identity,
            deterministic_ranked_ir,
            coverage,
            semantics,
            typed_summary,
            reconciliation,
        )?;
        Ok(Self {
            inert: InertProductionMiddleEndEvidenceV5 {
                source_semantic_identity,
                ranked_kernel_identity,
                ranked_ir_range: encoded.ranked_ir_range,
                coverage,
                semantics,
                typed_summary,
                reconciliation,
                identity: encoded.identity,
                canonical_bytes: encoded.canonical_bytes,
            },
        })
    }

    pub const fn as_inert(&self) -> &InertProductionMiddleEndEvidenceV5 {
        &self.inert
    }

    pub fn into_inert(self) -> InertProductionMiddleEndEvidenceV5 {
        self.inert
    }

    pub fn ranked_ir(&self) -> &str {
        self.inert.ranked_ir()
    }

    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV5 {
        self.inert.assurance()
    }

    pub const fn policy(&self) -> &'static [u8] {
        self.inert.policy()
    }

    pub const fn source_semantic_identity(&self) -> &[u8; SHA256_BYTES] {
        self.inert.source_semantic_identity()
    }

    pub const fn ranked_kernel_identity(&self) -> &[u8; SHA256_BYTES] {
        self.inert.ranked_kernel_identity()
    }

    pub const fn pass_successes(
        &self,
    ) -> &'static [ProductionMiddleEndPassSuccessV5; PASS_COUNT_V5] {
        self.inert.pass_successes()
    }

    pub const fn coverage_summary(&self) -> ProductionMiddleEndCoverageSummaryV5 {
        self.inert.coverage_summary()
    }

    pub const fn semantic_summary(&self) -> ProductionMiddleEndSemanticSummaryV5 {
        self.inert.semantic_summary()
    }

    pub const fn typed_semantic_summary(&self) -> ProductionTypedSemanticObligationSummaryV2 {
        self.inert.typed_semantic_summary()
    }

    pub const fn typed_semantic_reconciliation(
        &self,
    ) -> ProductionMiddleEndTypedSemanticReconciliationV5 {
        self.inert.typed_semantic_reconciliation()
    }

    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV5 {
        self.inert.identity()
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        self.inert.canonical_bytes()
    }

    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    pub const fn claims_verus_verification(&self) -> bool {
        false
    }

    pub const fn claims_full_arithmetic_correctness(&self) -> bool {
        false
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub const fn grants_target_value_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for ProductionMiddleEndEvidenceV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionMiddleEndEvidenceV5")
            .field("identity", &self.identity())
            .field("assurance", &self.inert.assurance())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum ProductionMiddleEndEvidenceCodecErrorV5 {
    HistoricalV4(ProductionMiddleEndEvidenceCodecErrorV4),
    RankedKernel(ProductionRankedKernelErrorV1),
    ReportPassOrderMismatch {
        index: usize,
    },
    ReportNotClean(ProductionMiddleEndEvidencePassV5),
    ReportAuthorityClaim(ProductionMiddleEndEvidencePassV5),
    CounterOverflow,
    InvalidCoverageSummary,
    InvalidSemanticSummary,
    InvalidTypedSemanticSummary,
    InvalidTypedSemanticReconciliation,
    EmptyRankedIr,
    RankedIrTooLarge {
        actual: usize,
        limit: usize,
    },
    InvalidRankedIrUtf8,
    NonCanonicalRankedIrByte {
        offset: usize,
    },
    RankedIrMissingFinalNewline,
    TooLarge {
        actual: usize,
        limit: usize,
    },
    InvalidMagic,
    UnsupportedVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength(u64),
    Truncated,
    TrailingBytes,
    NonzeroReserved,
    InvalidDomain,
    InvalidPolicy,
    InvalidAssurance,
    SemanticOwnerNotRevalidated,
    ZeroSemanticIdentity,
    ZeroRankedKernelIdentity,
    InvalidPassCount(u8),
    InvalidPassOrder {
        index: usize,
        expected: ProductionMiddleEndEvidencePassV5,
        actual: u8,
    },
    PassNotClean(ProductionMiddleEndEvidencePassV5),
    AuthorityClaim(ProductionMiddleEndEvidencePassV5),
    ZeroIdentity,
    IdentityMismatch,
    NonCanonical,
    AllocationFailed,
}

impl fmt::Display for ProductionMiddleEndEvidenceCodecErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HistoricalV4(error) => {
                write!(formatter, "historical V4 boundary failed: {error}")
            }
            Self::RankedKernel(error) => write!(formatter, "ranked kernel failed: {error}"),
            Self::ReportPassOrderMismatch { index } => {
                write!(
                    formatter,
                    "V5 report pass order differs at position {index}"
                )
            }
            Self::ReportNotClean(pass) | Self::PassNotClean(pass) => {
                write!(formatter, "V5 pass {pass:?} is not exactly clean")
            }
            Self::ReportAuthorityClaim(pass) | Self::AuthorityClaim(pass) => {
                write!(
                    formatter,
                    "V5 pass {pass:?} contains a forbidden authority claim"
                )
            }
            Self::CounterOverflow => formatter.write_str("V5 evidence counter overflow"),
            Self::InvalidCoverageSummary => {
                formatter.write_str("V5 coverage summary is inconsistent")
            }
            Self::InvalidSemanticSummary => {
                formatter.write_str("V5 semantic obligation summary is inconsistent")
            }
            Self::InvalidTypedSemanticSummary => {
                formatter.write_str("V5 typed semantic summary is inconsistent")
            }
            Self::InvalidTypedSemanticReconciliation => {
                formatter.write_str("V5 typed semantic reconciliation is inconsistent")
            }
            Self::EmptyRankedIr => formatter.write_str("deterministic ranked IR is empty"),
            Self::RankedIrTooLarge { actual, limit } => {
                write!(formatter, "ranked IR byte length {actual} exceeds {limit}")
            }
            Self::InvalidRankedIrUtf8 => formatter.write_str("ranked IR is not valid UTF-8"),
            Self::NonCanonicalRankedIrByte { offset } => {
                write!(
                    formatter,
                    "ranked IR has a noncanonical byte at offset {offset}"
                )
            }
            Self::RankedIrMissingFinalNewline => {
                formatter.write_str("ranked IR must end in a newline")
            }
            Self::TooLarge { actual, limit } => {
                write!(formatter, "V5 record length {actual} exceeds {limit}")
            }
            Self::InvalidMagic => formatter.write_str("invalid V5 evidence magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported V5 evidence version {version}")
            }
            Self::UnsupportedFlags(flags) => write!(formatter, "unsupported V5 flags {flags}"),
            Self::InvalidLength(length) => write!(formatter, "invalid V5 length {length}"),
            Self::Truncated => formatter.write_str("truncated V5 evidence"),
            Self::TrailingBytes => formatter.write_str("trailing V5 evidence bytes"),
            Self::NonzeroReserved => formatter.write_str("V5 reserved field is nonzero"),
            Self::InvalidDomain => formatter.write_str("invalid V5 evidence domain"),
            Self::InvalidPolicy => formatter.write_str("invalid V5 evidence policy"),
            Self::InvalidAssurance => formatter.write_str("invalid V5 assurance"),
            Self::SemanticOwnerNotRevalidated => {
                formatter.write_str("V5 semantic owner revalidation fact is absent")
            }
            Self::ZeroSemanticIdentity => formatter.write_str("V5 semantic identity is zero"),
            Self::ZeroRankedKernelIdentity => formatter.write_str("V5 ranked identity is zero"),
            Self::InvalidPassCount(count) => write!(formatter, "invalid V5 pass count {count}"),
            Self::InvalidPassOrder { index, .. } => {
                write!(formatter, "noncanonical V5 pass at position {index}")
            }
            Self::ZeroIdentity => formatter.write_str("V5 evidence identity is zero"),
            Self::IdentityMismatch => formatter.write_str("V5 evidence identity mismatch"),
            Self::NonCanonical => formatter.write_str("V5 evidence is not canonical"),
            Self::AllocationFailed => formatter.write_str("V5 evidence allocation failed"),
        }
    }
}

impl Error for ProductionMiddleEndEvidenceCodecErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::HistoricalV4(error) => Some(error),
            Self::RankedKernel(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_v5_live_reports(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    let report = ranked.production_pipeline_report();
    if report.pass_order() != &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::ReportPassOrderMismatch { index: 0 });
    }
    let facts = [
        (
            report.tensor_layout().pass(),
            report.tensor_layout().is_clean(),
            report.tensor_layout().findings().len(),
            report
                .tensor_layout()
                .grants_compiler_refinement_authority(),
            report.tensor_layout().grants_artifact_or_launch_authority(),
        ),
        (
            report.bounds().pass(),
            report.bounds().is_clean(),
            report.bounds().findings().len(),
            report.bounds().grants_compiler_refinement_authority(),
            report.bounds().grants_artifact_or_launch_authority(),
        ),
        (
            report.atomics().pass(),
            report.atomics().is_clean(),
            report.atomics().findings().len(),
            report.atomics().grants_compiler_refinement_authority(),
            report.atomics().grants_artifact_or_launch_authority(),
        ),
        (
            report.race().pass(),
            report.race().is_clean(),
            report.race().findings().len(),
            report.race().grants_compiler_refinement_authority(),
            report.race().grants_artifact_or_launch_authority(),
        ),
        (
            report.ownership().pass(),
            report.ownership().is_clean(),
            report.ownership().findings().len(),
            report.ownership().grants_compiler_refinement_authority(),
            report.ownership().grants_artifact_or_launch_authority(),
        ),
        (
            report.barriers().pass(),
            report.barriers().is_clean(),
            report.barriers().findings().len(),
            report.barriers().grants_compiler_refinement_authority(),
            report.barriers().grants_artifact_or_launch_authority(),
        ),
        (
            report.workgroup().pass(),
            report.workgroup().is_clean(),
            report.workgroup().findings().len(),
            report.workgroup().grants_compiler_refinement_authority(),
            report.workgroup().grants_artifact_or_launch_authority(),
        ),
        (
            report.semantics().pass(),
            report.semantics().is_clean(),
            report.semantics().findings().len(),
            report.semantics().grants_compiler_refinement_authority(),
            report.semantics().grants_artifact_or_launch_authority(),
        ),
    ];
    for (index, (actual, clean, findings, compiler_authority, artifact_authority)) in
        facts.into_iter().enumerate()
    {
        let expected = PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5[index];
        if ProductionMiddleEndEvidencePassV5::from_analysis(actual) != Some(expected) {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ReportPassOrderMismatch { index });
        }
        if !clean || findings != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ReportNotClean(
                expected,
            ));
        }
        if compiler_authority || artifact_authority {
            return Err(ProductionMiddleEndEvidenceCodecErrorV5::ReportAuthorityClaim(expected));
        }
    }
    Ok(())
}

fn validate_coverage_v5(
    summary: ProductionMiddleEndCoverageSummaryV5,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    if summary.total_view_declared != summary.total_view_proved
        || summary.collective_contributions_declared != summary.collective_contributions_proved
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidCoverageSummary);
    }
    Ok(())
}

fn validate_semantics_v5(
    summary: ProductionMiddleEndSemanticSummaryV5,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    if summary.reference_obligations_declared != summary.reference_obligations_proved
        || summary.effect_contracts_declared != summary.effect_contracts_proved
        || summary.collective_contracts_declared != summary.collective_contracts_proved
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidSemanticSummary);
    }
    Ok(())
}

fn validate_typed_summary_v5(
    summary: ProductionTypedSemanticObligationSummaryV2,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    let contract_roots = summary
        .exact_bitvector_operator_congruence_roots
        .checked_add(summary.exact_ieee_operator_congruence_roots)
        .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticSummary)?;
    if contract_roots != summary.expression_roots
        || summary.statically_discharged_domain_roots > summary.expression_roots
        || summary.arithmetic_operations > summary.expression_nodes
        || summary.comparisons > summary.expression_nodes
        || summary.selects > summary.expression_nodes
        || summary.casts > summary.expression_nodes
        || summary.checked_operations > summary.arithmetic_operations
        || (summary.expression_roots == 0 && summary.expression_nodes != 0)
        || (summary.expression_roots != 0 && summary.expression_nodes < summary.expression_roots)
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticSummary);
    }
    Ok(())
}

fn validate_reconciliation_v5(
    summary: ProductionTypedSemanticObligationSummaryV2,
    reconciliation: ProductionMiddleEndTypedSemanticReconciliationV5,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    let roots = usize_to_u64(summary.expression_roots)?;
    if reconciliation.recipe_expression_roots != roots
        || reconciliation.pliron_commitment_roots != roots
        || reconciliation.ordered_commitments_sha256 == [0; SHA256_BYTES]
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticReconciliation);
    }
    Ok(())
}

fn validate_ranked_ir_v5(bytes: &[u8]) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV5> {
    if bytes.is_empty() {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::EmptyRankedIr);
    }
    if bytes.len() > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge {
            actual: bytes.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5,
        });
    }
    std::str::from_utf8(bytes)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::InvalidRankedIrUtf8)?;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' && !(b' '..=b'~').contains(&byte) {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV5::NonCanonicalRankedIrByte { offset },
            );
        }
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrMissingFinalNewline);
    }
    Ok(())
}

struct EncodedRecordV5 {
    canonical_bytes: Box<[u8]>,
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV5,
}

fn encode_record_v5(
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir: &str,
    coverage: ProductionMiddleEndCoverageSummaryV5,
    semantics: ProductionMiddleEndSemanticSummaryV5,
    typed_summary: ProductionTypedSemanticObligationSummaryV2,
    reconciliation: ProductionMiddleEndTypedSemanticReconciliationV5,
) -> Result<EncodedRecordV5, ProductionMiddleEndEvidenceCodecErrorV5> {
    validate_ranked_ir_v5(ranked_ir.as_bytes())?;
    validate_coverage_v5(coverage)?;
    validate_semantics_v5(semantics)?;
    validate_typed_summary_v5(typed_summary)?;
    validate_reconciliation_v5(typed_summary, reconciliation)?;
    if source_semantic_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroSemanticIdentity);
    }
    if ranked_kernel_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::ZeroRankedKernelIdentity);
    }
    let total_len = FIXED_RECORD_BYTES_V5.checked_add(ranked_ir.len()).ok_or(
        ProductionMiddleEndEvidenceCodecErrorV5::TooLarge {
            actual: usize::MAX,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
        },
    )?;
    if total_len > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::TooLarge {
            actual: total_len,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5,
        });
    }
    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(total_len)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::AllocationFailed)?;
    canonical.extend_from_slice(&MAGIC_V5);
    canonical.extend_from_slice(&VERSION_V5.to_le_bytes());
    canonical.extend_from_slice(&FLAGS_V5.to_le_bytes());
    canonical.extend_from_slice(&(total_len as u64).to_le_bytes());
    canonical.extend_from_slice(&0_u32.to_le_bytes());
    canonical
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len() as u16).to_le_bytes());
    canonical.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5);
    canonical
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len() as u16).to_le_bytes());
    canonical.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5);
    canonical.push(ASSURANCE_INTERNAL_CHECKS_ONLY_V5);
    canonical.push(SEMANTIC_OWNER_REVALIDATED_V5);
    canonical.extend_from_slice(&0_u16.to_le_bytes());
    canonical.extend_from_slice(&source_semantic_identity);
    canonical.extend_from_slice(&ranked_kernel_identity);
    canonical.extend_from_slice(&(ranked_ir.len() as u32).to_le_bytes());
    let ranked_ir_start = canonical.len();
    canonical.extend_from_slice(ranked_ir.as_bytes());
    let ranked_ir_end = canonical.len();
    canonical.push(PASS_COUNT_V5 as u8);
    for pass in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5 {
        canonical.push(pass.tag());
        canonical.push(CLEAN_STATUS_V5);
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        canonical.push(0);
        canonical.push(0);
        canonical.extend_from_slice(&0_u16.to_le_bytes());
    }
    for counter in [
        coverage.total_view_declared,
        coverage.total_view_proved,
        coverage.collective_contributions_declared,
        coverage.collective_contributions_proved,
    ] {
        canonical.extend_from_slice(&counter.to_le_bytes());
    }
    for counter in [
        semantics.reference_obligations_declared,
        semantics.reference_obligations_proved,
        semantics.effect_contracts_declared,
        semantics.effect_contracts_proved,
        semantics.collective_contracts_declared,
        semantics.collective_contracts_proved,
    ] {
        canonical.extend_from_slice(&counter.to_le_bytes());
    }
    for counter in typed_summary_counters_v5(typed_summary)? {
        canonical.extend_from_slice(&counter.to_le_bytes());
    }
    canonical.extend_from_slice(&reconciliation.recipe_expression_roots.to_le_bytes());
    canonical.extend_from_slice(&reconciliation.pliron_commitment_roots.to_le_bytes());
    canonical.extend_from_slice(&reconciliation.ordered_commitments_sha256);
    let evidence_sha256 = derive_evidence_identity_v5(&canonical)
        .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::ZeroIdentity)?;
    canonical.extend_from_slice(&evidence_sha256);
    if canonical.len() != total_len {
        return Err(ProductionMiddleEndEvidenceCodecErrorV5::NonCanonical);
    }
    Ok(EncodedRecordV5 {
        canonical_bytes: canonical.into_boxed_slice(),
        ranked_ir_range: ranked_ir_start..ranked_ir_end,
        identity: ProductionMiddleEndEvidenceIdentityV5 {
            sha256: evidence_sha256,
            byte_len: total_len as u64,
        },
    })
}

fn typed_summary_counters_v5(
    summary: ProductionTypedSemanticObligationSummaryV2,
) -> Result<[u64; TYPED_SUMMARY_COUNTERS_V5], ProductionMiddleEndEvidenceCodecErrorV5> {
    Ok([
        usize_to_u64(summary.expression_roots)?,
        usize_to_u64(summary.expression_nodes)?,
        usize_to_u64(summary.arithmetic_operations)?,
        usize_to_u64(summary.comparisons)?,
        usize_to_u64(summary.selects)?,
        usize_to_u64(summary.casts)?,
        usize_to_u64(summary.checked_operations)?,
        usize_to_u64(summary.statically_discharged_domain_roots)?,
        usize_to_u64(summary.exact_bitvector_operator_congruence_roots)?,
        usize_to_u64(summary.exact_ieee_operator_congruence_roots)?,
    ])
}

fn usize_to_u64(value: usize) -> Result<u64, ProductionMiddleEndEvidenceCodecErrorV5> {
    u64::try_from(value).map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::CounterOverflow)
}

fn u64_to_usize(value: u64) -> Result<usize, ProductionMiddleEndEvidenceCodecErrorV5> {
    usize::try_from(value).map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::CounterOverflow)
}

fn derive_evidence_identity_v5(preimage: &[u8]) -> Option<[u8; SHA256_BYTES]> {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V5);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    let identity: [u8; SHA256_BYTES] = digest.finalize().into();
    (identity != [0; SHA256_BYTES]).then_some(identity)
}

struct ReaderV5<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV5<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionMiddleEndEvidenceCodecErrorV5> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::Truncated)?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV5::Truncated)?;
        self.offset = end;
        Ok(result)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionMiddleEndEvidenceCodecErrorV5> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV5::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionMiddleEndEvidenceCodecErrorV5> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IR: &str = "func @evidence_v5 {\n  kernel.return\n}\n";

    #[derive(Clone, Copy)]
    struct Layout {
        domain: usize,
        policy: usize,
        pass_count: usize,
        passes: usize,
        coverage: usize,
        semantics: usize,
        typed: usize,
        reconciliation: usize,
        identity: usize,
    }

    fn summary() -> ProductionTypedSemanticObligationSummaryV2 {
        ProductionTypedSemanticObligationSummaryV2 {
            expression_roots: 1,
            expression_nodes: 3,
            arithmetic_operations: 1,
            comparisons: 0,
            selects: 0,
            casts: 0,
            checked_operations: 0,
            statically_discharged_domain_roots: 1,
            exact_bitvector_operator_congruence_roots: 1,
            exact_ieee_operator_congruence_roots: 0,
        }
    }

    fn specimen_with_coverage(coverage: ProductionMiddleEndCoverageSummaryV5) -> EncodedRecordV5 {
        encode_record_v5(
            [1; SHA256_BYTES],
            [2; SHA256_BYTES],
            IR,
            coverage,
            ProductionMiddleEndSemanticSummaryV5 {
                reference_obligations_declared: 1,
                reference_obligations_proved: 1,
                effect_contracts_declared: 1,
                effect_contracts_proved: 1,
                collective_contracts_declared: 2,
                collective_contracts_proved: 2,
            },
            summary(),
            ProductionMiddleEndTypedSemanticReconciliationV5 {
                recipe_expression_roots: 1,
                pliron_commitment_roots: 1,
                ordered_commitments_sha256: [3; SHA256_BYTES],
            },
        )
        .unwrap()
    }

    fn specimen() -> EncodedRecordV5 {
        specimen_with_coverage(ProductionMiddleEndCoverageSummaryV5 {
            total_view_declared: 1,
            total_view_proved: 1,
            collective_contributions_declared: 2,
            collective_contributions_proved: 2,
        })
    }

    fn u16_at(bytes: &[u8], offset: usize) -> usize {
        usize::from(u16::from_le_bytes(
            bytes[offset..offset + 2].try_into().unwrap(),
        ))
    }

    fn u32_at(bytes: &[u8], offset: usize) -> usize {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
    }

    fn layout(bytes: &[u8]) -> Layout {
        let mut offset = 8 + 2 + 2 + 8 + 4;
        let domain_len = u16_at(bytes, offset);
        offset += 2;
        let domain = offset;
        offset += domain_len;
        let policy_len = u16_at(bytes, offset);
        offset += 2;
        let policy = offset;
        offset += policy_len + 1 + 1 + 2 + SHA256_BYTES + SHA256_BYTES;
        let ir_len = u32_at(bytes, offset);
        offset += 4 + ir_len;
        let pass_count = offset;
        offset += 1;
        let passes = offset;
        offset += PASS_COUNT_V5 * PASS_RECORD_BYTES_V5;
        let coverage = offset;
        offset += COVERAGE_COUNTERS_V5 * 8;
        let semantics = offset;
        offset += SEMANTIC_COUNTERS_V5 * 8;
        let typed = offset;
        offset += TYPED_SUMMARY_COUNTERS_V5 * 8;
        let reconciliation = offset;
        offset += RECONCILIATION_COUNTERS_V5 * 8 + SHA256_BYTES;
        let identity = offset;
        assert_eq!(identity + SHA256_BYTES, bytes.len());
        Layout {
            domain,
            policy,
            pass_count,
            passes,
            coverage,
            semantics,
            typed,
            reconciliation,
            identity,
        }
    }

    fn reidentify(bytes: &mut [u8]) {
        let terminal = bytes.len() - SHA256_BYTES;
        let identity = derive_evidence_identity_v5(&bytes[..terminal]).unwrap();
        bytes[terminal..].copy_from_slice(&identity);
    }

    #[test]
    fn strict_round_trip_binds_eight_passes_coverage_and_typed_reconciliation() {
        let encoded = specimen();
        let decoded = InertProductionMiddleEndEvidenceV5::decode(&encoded.canonical_bytes).unwrap();
        assert_eq!(decoded.ranked_ir(), IR);
        assert_eq!(decoded.pass_successes().len(), 8);
        assert_eq!(
            decoded.pass_successes().map(|success| success.pass()),
            PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V5
        );
        assert!(
            decoded
                .coverage_summary()
                .has_non_vacuous_total_view_proof()
        );
        assert!(
            decoded
                .coverage_summary()
                .has_non_vacuous_collective_contribution_proof()
        );
        assert_eq!(decoded.typed_semantic_summary(), summary());
        assert!(decoded.typed_semantic_reconciliation().is_exact());
        assert!(decoded.semantic_summary().has_non_vacuous_reference_proof());
        assert!(decoded.semantic_summary().has_non_vacuous_effect_proof());
        assert!(
            decoded
                .semantic_summary()
                .has_non_vacuous_collective_value_proof()
        );
        assert_eq!(
            decoded
                .typed_semantic_reconciliation()
                .recipe_expression_roots(),
            1
        );
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
        assert_eq!(
            *decoded.identity().sha256(),
            [
                255, 192, 5, 110, 140, 193, 119, 236, 249, 72, 104, 148, 57, 153, 181, 97, 27, 202,
                29, 242, 235, 152, 238, 202, 198, 37, 125, 56, 137, 41, 211, 38,
            ]
        );
        assert!(!decoded.authenticates_producer());
        assert!(!decoded.claims_full_arithmetic_correctness());
        assert!(!decoded.grants_compiler_refinement_authority());
        assert!(!decoded.grants_artifact_or_launch_authority());
        assert!(!decoded.grants_target_value_authority());
        assert!(!decoded.grants_publication_authority());
        assert!(!decoded.grants_load_authority());
    }

    #[test]
    fn zero_coverage_counts_never_become_vacuous_proof_claims() {
        let encoded = specimen_with_coverage(ProductionMiddleEndCoverageSummaryV5::default());
        let decoded = InertProductionMiddleEndEvidenceV5::decode(&encoded.canonical_bytes).unwrap();
        assert!(
            !decoded
                .coverage_summary()
                .has_non_vacuous_total_view_proof()
        );
        assert!(
            !decoded
                .coverage_summary()
                .has_non_vacuous_collective_contribution_proof()
        );
        assert!(
            !decoded
                .coverage_summary()
                .grants_collective_value_authority()
        );
    }

    #[test]
    fn pass_order_status_and_authority_mutations_fail_closed() {
        let encoded = specimen();
        let wire = layout(&encoded.canonical_bytes);

        let mut wrong_order = encoded.canonical_bytes.to_vec();
        wrong_order[wire.passes] = ProductionMiddleEndEvidencePassV5::MemoryBounds.tag();
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&wrong_order),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassOrder { index: 0, .. })
        ));

        let mut dirty = encoded.canonical_bytes.to_vec();
        dirty[wire.passes + 1] = 0;
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&dirty),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::PassNotClean(_))
        ));

        let mut authority = encoded.canonical_bytes.to_vec();
        authority[wire.passes + 6] = 1;
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&authority),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::AuthorityClaim(_))
        ));
    }

    #[test]
    fn coverage_typed_and_reconciliation_mismatches_fail_before_identity() {
        let encoded = specimen();
        let wire = layout(&encoded.canonical_bytes);

        let mut coverage = encoded.canonical_bytes.to_vec();
        coverage[wire.coverage + 8..wire.coverage + 16].copy_from_slice(&0_u64.to_le_bytes());
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&coverage),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidCoverageSummary)
        ));

        let mut semantics = encoded.canonical_bytes.to_vec();
        semantics[wire.semantics + 5 * 8..wire.semantics + 6 * 8]
            .copy_from_slice(&0_u64.to_le_bytes());
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&semantics),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidSemanticSummary)
        ));

        let mut typed = encoded.canonical_bytes.to_vec();
        typed[wire.typed + 8 * 8..wire.typed + 9 * 8].copy_from_slice(&0_u64.to_le_bytes());
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&typed),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticSummary)
        ));

        let mut reconciliation = encoded.canonical_bytes.to_vec();
        reconciliation[wire.reconciliation + 8..wire.reconciliation + 16]
            .copy_from_slice(&0_u64.to_le_bytes());
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&reconciliation),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidTypedSemanticReconciliation)
        ));
    }

    #[test]
    fn canonical_domain_policy_count_and_identity_mutations_fail_closed() {
        let encoded = specimen();
        let wire = layout(&encoded.canonical_bytes);
        let cases = [
            (
                wire.domain,
                ProductionMiddleEndEvidenceCodecErrorV5::InvalidDomain,
            ),
            (
                wire.policy,
                ProductionMiddleEndEvidenceCodecErrorV5::InvalidPolicy,
            ),
        ];
        for (offset, expected) in cases {
            let mut mutation = encoded.canonical_bytes.to_vec();
            mutation[offset] ^= 1;
            let actual = InertProductionMiddleEndEvidenceV5::decode(&mutation).unwrap_err();
            assert_eq!(actual.to_string(), expected.to_string());
        }

        let mut count = encoded.canonical_bytes.to_vec();
        count[wire.pass_count] = 7;
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&count),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::InvalidPassCount(7))
        ));

        let mut identity = encoded.canonical_bytes.to_vec();
        identity[wire.identity] ^= 1;
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&identity),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::IdentityMismatch)
        ));

        let mut canonical_change = encoded.canonical_bytes.to_vec();
        canonical_change[wire.coverage] ^= 1;
        canonical_change[wire.coverage + 8] ^= 1;
        reidentify(&mut canonical_change);
        let changed = InertProductionMiddleEndEvidenceV5::decode(&canonical_change).unwrap();
        assert_ne!(changed.identity(), encoded.identity);
    }

    #[test]
    fn every_truncation_trailing_data_and_aggregate_limit_fail_closed() {
        let encoded = specimen();
        for length in 0..encoded.canonical_bytes.len() {
            assert!(
                InertProductionMiddleEndEvidenceV5::decode(&encoded.canonical_bytes[..length])
                    .is_err()
            );
        }
        let mut trailing = encoded.canonical_bytes.to_vec();
        trailing.push(0);
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&trailing),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::TrailingBytes)
        ));
        let oversized = vec![0_u8; MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5 + 1];
        assert!(matches!(
            InertProductionMiddleEndEvidenceV5::decode(&oversized),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::TooLarge { .. })
        ));
    }

    #[test]
    fn exact_maximum_record_is_accepted_and_one_more_ranked_byte_is_rejected() {
        let mut maximum_ir = " ".repeat(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V5);
        maximum_ir.replace_range(maximum_ir.len() - 1.., "\n");
        let maximum = encode_record_v5(
            [1; SHA256_BYTES],
            [2; SHA256_BYTES],
            &maximum_ir,
            ProductionMiddleEndCoverageSummaryV5::default(),
            ProductionMiddleEndSemanticSummaryV5::default(),
            summary(),
            ProductionMiddleEndTypedSemanticReconciliationV5 {
                recipe_expression_roots: 1,
                pliron_commitment_roots: 1,
                ordered_commitments_sha256: [3; SHA256_BYTES],
            },
        )
        .unwrap();
        assert_eq!(
            maximum.canonical_bytes.len(),
            MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V5
        );
        assert!(InertProductionMiddleEndEvidenceV5::decode(&maximum.canonical_bytes).is_ok());

        maximum_ir.insert(maximum_ir.len() - 1, ' ');
        assert!(matches!(
            encode_record_v5(
                [1; SHA256_BYTES],
                [2; SHA256_BYTES],
                &maximum_ir,
                ProductionMiddleEndCoverageSummaryV5::default(),
                ProductionMiddleEndSemanticSummaryV5::default(),
                summary(),
                ProductionMiddleEndTypedSemanticReconciliationV5 {
                    recipe_expression_roots: 1,
                    pliron_commitment_roots: 1,
                    ordered_commitments_sha256: [3; SHA256_BYTES],
                },
            ),
            Err(ProductionMiddleEndEvidenceCodecErrorV5::RankedIrTooLarge { .. })
        ));
    }
}
