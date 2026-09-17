//! Authority-free formal-memory custody with explicit static-publication discharge.
//! The original formal-obligation receipt is retained byte-for-byte, including
//! raw conflicts; publication does not rewrite the affine analysis result.

use crate::{
    ProductionCanonicalKernelIrIdentityV1, ProductionCanonicalKernelIrVersionV1,
    ProductionFormalMemoryOwnerV1,
};
use fe2o3_kernel_ir::InertCanonicalFormalMemoryObligationReceiptV1;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, ops::Range};

/// Canonical explicit-discharge evidence wire version.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V5: u16 = 5;
/// Closed V5 completeness policy revision.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V5: u16 = 1;
/// Maximum canonical evidence size, including the unchanged raw receipt.
pub const MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V5: usize = 4 * 1024 * 1024;
const MAGIC_V5: [u8; 8] = *b"F2FMA5\0\0";
const DOMAIN_V5: &[u8] = b"FE2O3/FORMAL-MEMORY-ADMISSION-EVIDENCE/V5\0";
const HEADER_BYTES_V5: usize = 196;
const PUBLICATION_FIXED_BYTES_V5: usize = 368;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
/// All obligations must be resolved, retaining explicit conflict discharges.
pub enum FormalMemoryCompletenessPolicyV5 {
    /// Completeness with replay-derived, individually recorded discharges.
    RequireCompleteWithExplicitDischarges = 1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
/// Admission completeness, not dispatch or allocation authority.
pub enum FormalMemoryCompletenessStatusV5 {
    /// No unresolved compiler obligations remain.
    Complete = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
/// Exact original KIR operation location.
pub struct FormalMemoryPublicationLocationV5 {
    block: u32,
    operation: u64,
}
impl FormalMemoryPublicationLocationV5 {
    /// Original block identifier.
    pub const fn block(self) -> u32 {
        self.block
    }
    /// Original operation ordinal.
    pub const fn operation(self) -> u64 {
        self.operation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One exact source, ranked and KIR event correspondence.
pub struct FormalMemoryPublicationEventV5 {
    semantic_block: u32,
    semantic_ordinal: u32,
    kir: FormalMemoryPublicationLocationV5,
    ranked: [u32; 2],
}
impl FormalMemoryPublicationEventV5 {
    /// Original source semantic block.
    pub const fn semantic_block(self) -> u32 {
        self.semantic_block
    }
    /// Effect ordinal within the authenticated terminal.
    pub const fn semantic_ordinal(self) -> u32 {
        self.semantic_ordinal
    }
    /// Exact original KIR effect.
    pub const fn kir(self) -> FormalMemoryPublicationLocationV5 {
        self.kir
    }
    /// Exact ranked block and operation ordinal.
    pub const fn ranked(self) -> [u32; 2] {
        self.ranked
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u8)]
/// Closed reasons for discharging an original affine conflict.
pub enum FormalMemoryStaticConflictReasonV5 {
    /// Distinct producer invocations address distinct payload cells.
    ProducerInjectivity = 1,
    /// The exact REQUEST/acquire protocol orders the published payload read.
    PublishedReadHappensBefore = 2,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
/// One retained original conflict and its explicit compiler discharge reason.
pub struct FormalMemoryConflictDischargeV5 {
    left: FormalMemoryPublicationLocationV5,
    right: FormalMemoryPublicationLocationV5,
    allocation: u32,
    reason: FormalMemoryStaticConflictReasonV5,
}
impl FormalMemoryConflictDischargeV5 {
    /// First original conflicting effect.
    pub const fn left(self) -> FormalMemoryPublicationLocationV5 {
        self.left
    }
    /// Second original conflicting effect.
    pub const fn right(self) -> FormalMemoryPublicationLocationV5 {
        self.right
    }
    /// Original KIR allocation parameter index.
    pub const fn allocation_parameter(self) -> u32 {
        self.allocation
    }
    /// Closed static discharge reason.
    pub const fn reason(self) -> FormalMemoryStaticConflictReasonV5 {
        self.reason
    }
}

#[derive(Debug, Eq, PartialEq)]
/// Inert exact five-effect publication proof, including zero-raw-conflict cases.
pub struct FormalMemoryStaticPublicationSummaryV5 {
    semantic_function: u32,
    selected_root: u32,
    source_arguments: [u32; 2],
    values: [u32; 6],
    payload_parameter_index: u32,
    global_extents: [u64; 3],
    workgroup_extents: [u64; 3],
    events: [FormalMemoryPublicationEventV5; 5],
    origins: [u64; 2],
    ranked_sites: [[u32; 2]; 6],
    ranked_counts: [u32; 4],
    discharges: Box<[FormalMemoryConflictDischargeV5]>,
}
impl FormalMemoryStaticPublicationSummaryV5 {
    /// Authenticated source function containing the terminals.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    /// Original selected source kernel root.
    pub const fn selected_root(&self) -> u32 {
        self.selected_root
    }
    /// Ranked payload allocation origin.
    pub const fn payload_origin(&self) -> u64 {
        self.origins[0]
    }
    /// Ranked flag allocation origin.
    pub const fn flags_origin(&self) -> u64 {
        self.origins[1]
    }
    /// Independently joined full physical global extents.
    pub const fn global_extents(&self) -> [u64; 3] {
        self.global_extents
    }
    /// Independently joined workgroup extents.
    pub const fn workgroup_extents(&self) -> [u64; 3] {
        self.workgroup_extents
    }
    /// W, READY, REQUEST, acquire, and payload-read event identities.
    pub const fn events(&self) -> &[FormalMemoryPublicationEventV5; 5] {
        &self.events
    }
    /// Exact ranked W, READY, REQUEST, acquire, guard and read sites.
    pub const fn ranked_sites(&self) -> &[[u32; 2]; 6] {
        &self.ranked_sites
    }
    /// Ranked potential pairs, distinct from the raw affine conflict count.
    pub const fn ranked_potentially_conflicting_cell_pairs(&self) -> u32 {
        self.ranked_counts[1]
    }
    /// Every raw affine conflict and its typed discharge.
    pub fn discharged_conflicts(&self) -> &[FormalMemoryConflictDischargeV5] {
        &self.discharges
    }
}

#[derive(Debug, Eq, PartialEq)]
/// Canonical compiler evidence that grants no runtime or dispatch authority.
pub struct InertCanonicalFormalMemoryAdmissionEvidenceV5 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    source_semantic_identity: [u8; 32],
    ranked_graph_identity: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    receipt_identity: [u8; 32],
    receipt_range: Range<usize>,
    witness: u64,
    raw_conflicts: u32,
    discharged_conflicts: u32,
    publication: Option<FormalMemoryStaticPublicationSummaryV5>,
}

impl InertCanonicalFormalMemoryAdmissionEvidenceV5 {
    /// Replays the live owner and retains its exact source, ranked and KIR joins.
    pub fn from_live_owner(
        owner: &ProductionFormalMemoryOwnerV1,
    ) -> Result<Self, ProductionFormalMemoryEvidenceErrorV5> {
        owner
            .verify_equivalence()
            .map_err(|error| ProductionFormalMemoryEvidenceErrorV5::LiveOwner(error.to_string()))?;
        let [kernel] = owner.kernels() else {
            return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidAdmission);
        };
        let semantic = owner.semantic_kir();
        let (ranked, _) = semantic
            .retained_static_publication_inputs_v1(kernel.obligations().kernel().as_str())
            .ok_or(ProductionFormalMemoryEvidenceErrorV5::InvalidAdmission)?;
        let source_identity = *semantic.semantic().semantic().semantic_sha256().as_bytes();
        let ranked_identity = ranked.exact_graph_identity().digest();
        let receipt = InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(
            kernel.obligations(),
        )
        .map_err(|error| ProductionFormalMemoryEvidenceErrorV5::Receipt(error.to_string()))?;
        let witness = kernel.witness_invocation_count();
        if witness == 0
            || kernel
                .witness_extents(semantic.module())
                .is_none_or(|extents| extents.contains(&0))
        {
            return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidAdmission);
        }
        let publication = publication_from_live_v5(kernel.static_publication())?;
        if publication.is_some() != ranked.race_report().static_publication().is_some() {
            return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidAdmission);
        }
        let fields = FormalFieldsV5 {
            source_identity,
            ranked_identity,
            kir: semantic.canonical_kernel_ir_identity(),
            receipt_identity: *receipt.identity().digest(),
            witness,
            raw_conflicts: u32::try_from(kernel.obligations().inter_invocation_conflicts().len())
                .map_err(|_| ProductionFormalMemoryEvidenceErrorV5::Limit)?,
            discharged: publication
                .as_ref()
                .map_or(0, |proof| proof.discharges.len() as u32),
        };
        Self::decode(&encode_v5(
            fields,
            publication.as_ref(),
            receipt.canonical_bytes(),
        )?)
    }
    /// Decodes one complete bounded canonical record, without granting authority.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionFormalMemoryEvidenceErrorV5> {
        decode_v5(bytes)
    }
    /// Rechecks canonical bytes and all retained inert fields.
    pub fn revalidate(&self) -> Result<(), ProductionFormalMemoryEvidenceErrorV5> {
        if Self::decode(&self.canonical_bytes)? != *self {
            return Err(ProductionFormalMemoryEvidenceErrorV5::InvalidIdentity);
        }
        Ok(())
    }
    /// Complete canonical V5 wire bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Consumes this record into its canonical bytes.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes.into_vec()
    }
    /// Domain-separated identity over all canonical bytes.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    /// Exact retained semantic source digest.
    pub const fn source_semantic_identity(&self) -> &[u8; 32] {
        &self.source_semantic_identity
    }
    /// Exact retained ranked graph digest.
    pub const fn ranked_graph_identity(&self) -> &[u8; 32] {
        &self.ranked_graph_identity
    }
    /// Exact retained KIR identity.
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }
    /// Identity of the unchanged raw formal-obligation receipt.
    pub const fn formal_obligation_receipt_identity(&self) -> &[u8; 32] {
        &self.receipt_identity
    }
    /// Unchanged canonical raw V1 or V2 receipt bytes.
    pub fn formal_obligation_receipt_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.receipt_range.clone()]
    }
    /// Full compiler witness invocation count.
    pub const fn witness_invocation_count(&self) -> u64 {
        self.witness
    }
    /// Closed completeness policy.
    pub const fn completeness_policy(&self) -> FormalMemoryCompletenessPolicyV5 {
        FormalMemoryCompletenessPolicyV5::RequireCompleteWithExplicitDischarges
    }
    /// Completeness of the recorded compiler obligations.
    pub const fn completeness_status(&self) -> FormalMemoryCompletenessStatusV5 {
        FormalMemoryCompletenessStatusV5::Complete
    }
    /// Unresolved static-conflict count, always zero for admitted evidence.
    pub const fn static_conflict_count(&self) -> u32 {
        0
    }
    /// Original, not discharged-away, affine conflict count.
    pub const fn inter_invocation_conflict_count(&self) -> u32 {
        self.raw_conflicts
    }
    /// Original raw affine conflict count.
    pub const fn raw_inter_invocation_conflict_count(&self) -> u32 {
        self.raw_conflicts
    }
    /// Number of individually recorded conflict discharges.
    pub const fn discharged_inter_invocation_conflict_count(&self) -> u32 {
        self.discharged_conflicts
    }
    /// Remaining unresolved conflicts, always zero for admitted evidence.
    pub const fn unresolved_inter_invocation_conflict_count(&self) -> u32 {
        0
    }
    /// Explicit publication record, including when the raw receipt has no conflicts.
    pub const fn static_publication(&self) -> Option<&FormalMemoryStaticPublicationSummaryV5> {
        self.publication.as_ref()
    }
    /// Inert evidence never grants allocation or dispatch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
/// Closed failures in live derivation or bounded canonical evidence parsing.
pub enum ProductionFormalMemoryEvidenceErrorV5 {
    /// Live owner replay failed.
    LiveOwner(String),
    /// The existing raw receipt validator rejected its bytes.
    Receipt(String),
    /// A checked byte or allocation limit was exceeded.
    Limit,
    /// Version, policy or reserved header data was invalid.
    InvalidHeader,
    /// A canonical byte length was invalid.
    InvalidLength,
    /// An identity was missing or inconsistent.
    InvalidIdentity,
    /// Completeness, witness or raw conflict records disagreed.
    InvalidAdmission,
    /// The closed publication and discharge record was inconsistent.
    InvalidPublication,
}
impl fmt::Display for ProductionFormalMemoryEvidenceErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(message) => write!(formatter, "formal owner replay failed: {message}"),
            Self::Receipt(message) => write!(formatter, "raw formal receipt failed: {message}"),
            Self::Limit => formatter.write_str("formal V5 bounded resource limit"),
            Self::InvalidHeader => formatter.write_str("invalid formal V5 header"),
            Self::InvalidLength => formatter.write_str("invalid formal V5 length"),
            Self::InvalidIdentity => formatter.write_str("invalid formal V5 identity"),
            Self::InvalidAdmission => formatter.write_str("invalid formal V5 admission"),
            Self::InvalidPublication => {
                formatter.write_str("invalid formal V5 publication discharge")
            }
        }
    }
}
impl Error for ProductionFormalMemoryEvidenceErrorV5 {}

type FormalResultV5<T> = Result<T, ProductionFormalMemoryEvidenceErrorV5>;
include!("production_formal_memory_evidence_v5_codec.rs");
include!("production_formal_memory_evidence_v5_publication.rs");
include!("production_formal_memory_evidence_v5_receipt.rs");
#[cfg(test)]
#[path = "production_formal_memory_evidence_v5_tests.rs"]
mod tests;
