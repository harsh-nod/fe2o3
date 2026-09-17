//! Versioned internal-analysis evidence with explicit static publication facts.
//! Decoded bytes remain inert and never authorize compilation or a launch.

use super::middle_end_evidence_v5::{CommonMiddleEndFactsV1, PASS_SUCCESSES_V5, ReaderV5};
use super::{
    ProductionMiddleEndAssuranceV5, ProductionMiddleEndCoverageSummaryV5,
    ProductionMiddleEndEvidenceCodecErrorV5, ProductionMiddleEndPassSuccessV5,
    ProductionMiddleEndSemanticSummaryV5, ProductionMiddleEndTypedSemanticReconciliationV5,
    ProductionRankedKernelLoweringInputV1, ProductionRankedOperationV1,
    ProductionSemanticMirOwnerV1, ProductionTypedSemanticObligationSummaryV2,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, ops::Range};

const MAGIC_V6: [u8; 8] = *b"F2MEV6\0\0";
const IDENTITY_DOMAIN_V6: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V6\0";
pub const PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V6: &[u8] =
    b"fe2o3.production-middle-end-evidence.v6";
pub const PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V6: &[u8] =
    b"fe2o3.internal-analysis-mir-congruence-and-static-publication-obligations-only.v6";
pub const MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V6: usize = 4 * 1024 * 1024;
const HEADER_BYTES_V6: usize = 32
    + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V6.len()
    + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V6.len();
const COMMON_BODY_BYTES_V6: usize = 32 + 32 + 4 + 1 + 8 * 10 + 4 * 8 + 6 * 8 + 10 * 8 + 2 * 8 + 32;
const PUBLICATION_BYTES_V6: usize = 88;
const LIVE_GRAPH_IDENTITY_BYTES_V6: usize = 32;
const FIXED_BYTES_V6: usize = HEADER_BYTES_V6
    + COMMON_BODY_BYTES_V6
    + LIVE_GRAPH_IDENTITY_BYTES_V6
    + PUBLICATION_BYTES_V6
    + 32;
pub const MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V6: usize =
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V6 - FIXED_BYTES_V6;

/// Exact ranked operation coordinates, not source strings or physical addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMiddleEndPublicationSiteV6 {
    block: u32,
    operation: u32,
}
impl ProductionMiddleEndPublicationSiteV6 {
    pub const fn block(self) -> u32 {
        self.block
    }
    pub const fn operation(self) -> u32 {
        self.operation
    }
}

/// A replay-derived safety proof, not progress or concrete runtime evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMiddleEndPublicationSummaryV6 {
    payload_origin: u64,
    flags_origin: u64,
    sites: [ProductionMiddleEndPublicationSiteV6; 6],
    maximum_invocations: u32,
    potentially_conflicting_cell_pairs: u32,
    discharged_cell_pairs: u32,
    unresolved_cell_pairs: u32,
}
impl ProductionMiddleEndPublicationSummaryV6 {
    pub const fn payload_origin(self) -> u64 {
        self.payload_origin
    }
    pub const fn flags_origin(self) -> u64 {
        self.flags_origin
    }
    /// Payload write, READY, REQUEST, acquire, guard, payload read.
    pub const fn sites(&self) -> &[ProductionMiddleEndPublicationSiteV6; 6] {
        &self.sites
    }
    pub const fn maximum_invocations(self) -> u32 {
        self.maximum_invocations
    }
    pub const fn potentially_conflicting_cell_pairs(self) -> u32 {
        self.potentially_conflicting_cell_pairs
    }
    pub const fn discharged_cell_pairs(self) -> u32 {
        self.discharged_cell_pairs
    }
    pub const fn unresolved_cell_pairs(self) -> u32 {
        self.unresolved_cell_pairs
    }
    pub const fn grants_artifact_or_launch_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMiddleEndEvidenceIdentityV6 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl ProductionMiddleEndEvidenceIdentityV6 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && bytes.len() >= 32
            && bytes[bytes.len() - 32..] == self.sha256
            && identity_v6(&bytes[..bytes.len() - 32]) == self.sha256
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct InertProductionMiddleEndEvidenceV6 {
    facts: CommonMiddleEndFactsV1,
    live_ranked_graph_identity: [u8; 32],
    publication: Option<ProductionMiddleEndPublicationSummaryV6>,
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV6,
    canonical_bytes: Box<[u8]>,
}
impl InertProductionMiddleEndEvidenceV6 {
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV6> {
        decode_record_v6(bytes)
    }
    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV5 {
        ProductionMiddleEndAssuranceV5::InternalAnalysesAndMirOperatorCongruenceObligationsOnly
    }
    pub const fn policy(&self) -> &'static [u8] {
        PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V6
    }
    pub const fn source_semantic_identity(&self) -> &[u8; 32] {
        &self.facts.source_semantic_identity
    }
    pub const fn ranked_kernel_identity(&self) -> &[u8; 32] {
        &self.facts.ranked_kernel_identity
    }
    /// Exact live PLIRON graph identity, distinct from the ranked recipe hash.
    pub const fn live_ranked_graph_identity(&self) -> &[u8; 32] {
        &self.live_ranked_graph_identity
    }
    pub fn ranked_ir(&self) -> &str {
        std::str::from_utf8(&self.canonical_bytes[self.ranked_ir_range.clone()])
            .expect("validated V6 text")
    }
    pub const fn pass_successes(&self) -> &'static [ProductionMiddleEndPassSuccessV5; 8] {
        &PASS_SUCCESSES_V5
    }
    pub const fn coverage_summary(&self) -> ProductionMiddleEndCoverageSummaryV5 {
        self.facts.coverage
    }
    pub const fn semantic_summary(&self) -> ProductionMiddleEndSemanticSummaryV5 {
        self.facts.semantics
    }
    pub const fn typed_semantic_summary(&self) -> ProductionTypedSemanticObligationSummaryV2 {
        self.facts.typed_summary
    }
    pub const fn typed_semantic_reconciliation(
        &self,
    ) -> ProductionMiddleEndTypedSemanticReconciliationV5 {
        self.facts.reconciliation
    }
    pub const fn static_publication(&self) -> Option<&ProductionMiddleEndPublicationSummaryV6> {
        self.publication.as_ref()
    }
    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV6 {
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

#[derive(Debug)]
pub struct ProductionMiddleEndEvidenceV6 {
    inert: InertProductionMiddleEndEvidenceV6,
}
impl ProductionMiddleEndEvidenceV6 {
    pub fn try_new(
        semantic: &ProductionSemanticMirOwnerV1,
        ranked: &ProductionRankedKernelLoweringInputV1,
        ir: &str,
    ) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV6> {
        let facts = CommonMiddleEndFactsV1::revalidate(semantic, ranked)?;
        let publication = publication_from_owner_v6(ranked)?;
        let live_ranked_graph_identity = ranked.exact_graph_identity().digest();
        Ok(Self {
            inert: encode_record_v6(facts, ir, live_ranked_graph_identity, publication)?,
        })
    }
    pub const fn as_inert(&self) -> &InertProductionMiddleEndEvidenceV6 {
        &self.inert
    }
    pub fn into_inert(self) -> InertProductionMiddleEndEvidenceV6 {
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
    pub const fn source_semantic_identity(&self) -> &[u8; 32] {
        self.inert.source_semantic_identity()
    }
    pub const fn ranked_kernel_identity(&self) -> &[u8; 32] {
        self.inert.ranked_kernel_identity()
    }
    /// Exact live PLIRON graph identity, independently retained from the recipe.
    pub const fn live_ranked_graph_identity(&self) -> &[u8; 32] {
        self.inert.live_ranked_graph_identity()
    }
    pub const fn pass_successes(&self) -> &'static [ProductionMiddleEndPassSuccessV5; 8] {
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
    pub const fn static_publication(&self) -> Option<&ProductionMiddleEndPublicationSummaryV6> {
        self.inert.static_publication()
    }
    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV6 {
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

#[derive(Debug)]
pub enum ProductionMiddleEndEvidenceCodecErrorV6 {
    Common(ProductionMiddleEndEvidenceCodecErrorV5),
    InvalidPublication,
    PublicationReportMismatch,
}
impl From<ProductionMiddleEndEvidenceCodecErrorV5> for ProductionMiddleEndEvidenceCodecErrorV6 {
    fn from(error: ProductionMiddleEndEvidenceCodecErrorV5) -> Self {
        Self::Common(error)
    }
}
impl fmt::Display for ProductionMiddleEndEvidenceCodecErrorV6 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Common(error) => write!(formatter, "V6 common evidence: {error}"),
            Self::InvalidPublication => {
                formatter.write_str("invalid exact static-publication evidence")
            }
            Self::PublicationReportMismatch => formatter
                .write_str("ranked publication operations and replay-derived proof disagree"),
        }
    }
}
impl Error for ProductionMiddleEndEvidenceCodecErrorV6 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Common(error) => Some(error),
            _ => None,
        }
    }
}

include!("middle_end_evidence_v6_codec.rs");
include!("middle_end_evidence_v6_publication.rs");

#[cfg(test)]
#[path = "middle_end_evidence_v6_tests.rs"]
mod tests;
