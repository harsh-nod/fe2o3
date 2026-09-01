//! Inert, bounded V3 lineage evidence derived from live production owners.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    FORMAL_MEMORY_OBLIGATION_POLICY_V1, FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V1,
    FormalMemoryReceiptErrorV1, InertCanonicalFormalMemoryObligationReceiptV1,
};
use sha2::{Digest, Sha256};

/// Wire version for exact MIR-to-KIR correspondence lineage evidence.
pub const MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V3: u16 = 3;
/// Validation policy committed by MIR-to-KIR correspondence V3 evidence.
pub const MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V3: u16 = 1;
/// Maximum exact bytes in one correspondence evidence record.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V3: usize = 512 * 1024;
/// Maximum semantic functions represented by correspondence evidence.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3: usize = 1_024;
/// Maximum semantic blocks represented by correspondence evidence.
pub const MAX_MIR_TO_KIR_CORRESPONDENCE_BLOCKS_V3: usize = 16_384;

/// Wire version for exact formal-memory admission lineage evidence.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V3: u16 = 3;
/// Validation policy committed by formal-memory admission V3 evidence.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V3: u16 = 1;
/// Maximum exact bytes in one formal-memory admission evidence record.
///
/// This matches the V3 compiler-lineage receipt-preimage budget. The embedded
/// canonical formal-obligation receipt must fit inside this stricter envelope.
pub const MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V3: usize = 4 * 1024 * 1024;

const CORRESPONDENCE_MAGIC_V3: [u8; 8] = *b"FE2O3MC\0";
const FORMAL_MEMORY_MAGIC_V3: [u8; 8] = *b"FE2O3FA\0";
const FORMAL_OBLIGATION_RECEIPT_MAGIC_V1: [u8; 8] = *b"FE2O3FM\0";
const CORRESPONDENCE_IDENTITY_DOMAIN_V3: &[u8] =
    b"FE2O3/INERT-MIR-TO-KIR-CORRESPONDENCE-EVIDENCE/V3\0";
const FORMAL_MEMORY_IDENTITY_DOMAIN_V3: &[u8] =
    b"FE2O3/INERT-FORMAL-MEMORY-ADMISSION-EVIDENCE/V3\0";
const COMMON_HEADER_BYTES_V3: usize = 20;
const CORRESPONDENCE_HEADER_BYTES_V3: usize = COMMON_HEADER_BYTES_V3 + 32 + 32 + 4 + 4;
const CORRESPONDENCE_RECORD_BYTES_V3: usize = 16;
const FORMAL_MEMORY_HEADER_BYTES_V3: usize =
    COMMON_HEADER_BYTES_V3 + 32 + 32 + 8 + 2 + 2 + 4 + 4 + 4;

/// Exact completeness policy committed by formal-memory admission evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FormalMemoryCompletenessPolicyV3 {
    /// Require complete extraction and reject every unresolved static or
    /// inter-invocation conflict before evidence construction.
    RequireCompleteConflictFree = 1,
}

/// Exact completeness status committed by formal-memory admission evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FormalMemoryCompletenessStatusV3 {
    /// The live owner re-derived complete, conflict-free obligations.
    Complete = 1,
}

/// Identifies one exact canonical MIR-to-KIR correspondence evidence encoding.
///
/// This is a content identity only. It grants no producer, artifact, proof, or
/// launch authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirToKirCorrespondenceEvidenceIdentityV3([u8; 32]);

impl MirToKirCorrespondenceEvidenceIdentityV3 {
    /// Returns the exact content digest.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Identifies one exact canonical formal-memory admission evidence encoding.
///
/// This is a content identity only. It grants no producer, artifact, proof, or
/// launch authority.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalMemoryAdmissionEvidenceIdentityV3([u8; 32]);

impl FormalMemoryAdmissionEvidenceIdentityV3 {
    /// Returns the exact content digest.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonically ordered correspondence for one semantic block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MirToKirBlockCorrespondenceEvidenceV3 {
    semantic_function: u32,
    semantic_block: u32,
    kernel_ir_block: u32,
    source_statement_count: u32,
}

impl MirToKirBlockCorrespondenceEvidenceV3 {
    /// Constructs one exact block-correspondence record for canonical encoding.
    pub const fn from_parts(
        semantic_function: u32,
        semantic_block: u32,
        kernel_ir_block: u32,
        source_statement_count: u32,
    ) -> Self {
        Self {
            semantic_function,
            semantic_block,
            kernel_ir_block,
            source_statement_count,
        }
    }

    /// Returns the semantic function index.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }

    /// Returns the semantic block index.
    pub const fn semantic_block(&self) -> u32 {
        self.semantic_block
    }

    /// Returns the exact corresponding Kernel IR block index.
    pub const fn kernel_ir_block(&self) -> u32 {
        self.kernel_ir_block
    }

    /// Returns the source statement count covered by this rule.
    pub const fn source_statement_count(&self) -> u32 {
        self.source_statement_count
    }
}

/// Exact inert correspondence content derived from a live semantic-KIR owner.
///
/// Decoding proves canonical structure and content integrity only. Any caller
/// can decode or copy these bytes; this value intentionally carries no
/// authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalMirToKirCorrespondenceEvidenceV3 {
    canonical_bytes: Vec<u8>,
    identity: MirToKirCorrespondenceEvidenceIdentityV3,
    semantic_sha256: [u8; 32],
    canonical_kir_v5_identity: [u8; 32],
    function_count: u32,
    blocks: Box<[MirToKirBlockCorrespondenceEvidenceV3]>,
}

impl InertCanonicalMirToKirCorrespondenceEvidenceV3 {
    /// Canonically encodes already extracted V3 correspondence records.
    pub fn from_canonical_parts(
        semantic_sha256: [u8; 32],
        canonical_kir_v5_identity: [u8; 32],
        function_count: u32,
        blocks: &[MirToKirBlockCorrespondenceEvidenceV3],
    ) -> Result<Self, ProductionLineageEvidenceErrorV3> {
        let canonical_bytes = encode_correspondence(
            semantic_sha256,
            canonical_kir_v5_identity,
            function_count,
            blocks,
        )?;
        Self::decode(&canonical_bytes)
    }

    /// Strictly decodes one complete canonical V3 correspondence encoding.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionLineageEvidenceErrorV3> {
        preflight_total_bytes(
            EvidenceKindV3::MirToKirCorrespondence,
            bytes.len(),
            MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V3,
        )?;
        let mut reader = ReaderV3::new(bytes);
        decode_common_header(
            &mut reader,
            EvidenceKindV3::MirToKirCorrespondence,
            CORRESPONDENCE_MAGIC_V3,
            MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V3,
            MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V3,
        )?;
        let semantic_sha256 = reader.fixed::<32>()?;
        let canonical_kir_v5_identity = reader.fixed::<32>()?;
        require_nonzero_identity("semantic MIR SHA-256", &semantic_sha256)?;
        require_nonzero_identity(
            "canonical Kernel IR V5 identity",
            &canonical_kir_v5_identity,
        )?;

        let function_count = reader.u32()?;
        let block_count = reader.u32()?;
        let function_count_usize = usize::try_from(function_count).map_err(|_| {
            ProductionLineageEvidenceErrorV3::Overflow {
                field: "correspondence function count",
            }
        })?;
        let block_count_usize = usize::try_from(block_count).map_err(|_| {
            ProductionLineageEvidenceErrorV3::Overflow {
                field: "correspondence block count",
            }
        })?;
        enforce_count(
            "correspondence functions",
            function_count_usize,
            MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3,
        )?;
        enforce_count(
            "correspondence blocks",
            block_count_usize,
            MAX_MIR_TO_KIR_CORRESPONDENCE_BLOCKS_V3,
        )?;
        if function_count == 0 || block_count == 0 {
            return Err(ProductionLineageEvidenceErrorV3::InvalidCorrespondence(
                "function and block counts must both be nonzero",
            ));
        }
        let record_bytes = block_count_usize
            .checked_mul(CORRESPONDENCE_RECORD_BYTES_V3)
            .ok_or(ProductionLineageEvidenceErrorV3::Overflow {
                field: "correspondence record bytes",
            })?;
        if reader.remaining() != record_bytes {
            return Err(if reader.remaining() < record_bytes {
                ProductionLineageEvidenceErrorV3::Truncated
            } else {
                ProductionLineageEvidenceErrorV3::TrailingBytes
            });
        }

        // Counts and exact remaining bytes are checked before this allocation.
        let mut blocks = Vec::with_capacity(block_count_usize);
        for _ in 0..block_count_usize {
            blocks.push(MirToKirBlockCorrespondenceEvidenceV3 {
                semantic_function: reader.u32()?,
                semantic_block: reader.u32()?,
                kernel_ir_block: reader.u32()?,
                source_statement_count: reader.u32()?,
            });
        }
        reader.finish()?;
        validate_canonical_correspondence(function_count, &blocks)?;

        let reencoded = encode_correspondence(
            semantic_sha256,
            canonical_kir_v5_identity,
            function_count,
            &blocks,
        )?;
        if reencoded != bytes {
            return Err(ProductionLineageEvidenceErrorV3::NonCanonical);
        }
        let identity = MirToKirCorrespondenceEvidenceIdentityV3(canonical_identity(
            CORRESPONDENCE_IDENTITY_DOMAIN_V3,
            &reencoded,
        ));
        require_nonzero_identity("correspondence evidence identity", identity.digest())?;
        Ok(Self {
            canonical_bytes: reencoded,
            identity,
            semantic_sha256,
            canonical_kir_v5_identity,
            function_count,
            blocks: blocks.into_boxed_slice(),
        })
    }

    /// Rechecks strict decoding, re-encoding, and the retained content identity.
    pub fn revalidate(&self) -> Result<(), ProductionLineageEvidenceErrorV3> {
        let decoded = Self::decode(&self.canonical_bytes)?;
        if decoded.identity != self.identity
            || decoded.semantic_sha256 != self.semantic_sha256
            || decoded.canonical_kir_v5_identity != self.canonical_kir_v5_identity
            || decoded.function_count != self.function_count
            || decoded.blocks != self.blocks
        {
            return Err(ProductionLineageEvidenceErrorV3::IdentityMismatch);
        }
        Ok(())
    }

    /// Returns the exact canonical V3 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Consumes inert evidence and returns its exact canonical V3 bytes.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    /// Returns the exact inert evidence identity.
    pub const fn identity(&self) -> &MirToKirCorrespondenceEvidenceIdentityV3 {
        &self.identity
    }

    /// Returns the exact admitted semantic MIR SHA-256.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }

    /// Returns the identity of the exact canonical Kernel IR V5 bytes.
    pub const fn canonical_kir_v5_identity(&self) -> &[u8; 32] {
        &self.canonical_kir_v5_identity
    }

    /// Returns the number of covered semantic functions.
    pub const fn function_count(&self) -> u32 {
        self.function_count
    }

    /// Returns the complete canonically ordered block correspondence.
    pub fn blocks(&self) -> &[MirToKirBlockCorrespondenceEvidenceV3] {
        &self.blocks
    }

    /// Inert lineage content never grants artifact, proof, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl TryFrom<&[u8]> for InertCanonicalMirToKirCorrespondenceEvidenceV3 {
    type Error = ProductionLineageEvidenceErrorV3;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Exact inert formal-memory admission content derived from a live owner.
///
/// The exact canonical formal-obligation receipt is embedded without a lossy
/// projection. Decoding grants no authority and does not prove runtime
/// satisfaction of the retained obligations. A successful live formal owner
/// has no unresolved static admission failure, so V3 records that count as
/// zero; runtime alias requirements remain present in the embedded receipt.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalFormalMemoryAdmissionEvidenceV3 {
    canonical_bytes: Vec<u8>,
    identity: FormalMemoryAdmissionEvidenceIdentityV3,
    canonical_kir_v5_identity: [u8; 32],
    formal_obligation_receipt_identity: [u8; 32],
    witness_extent: u64,
    completeness_policy: FormalMemoryCompletenessPolicyV3,
    completeness_status: FormalMemoryCompletenessStatusV3,
    static_conflict_count: u32,
    inter_invocation_conflict_count: u32,
    formal_obligation_receipt_offset: usize,
}

impl InertCanonicalFormalMemoryAdmissionEvidenceV3 {
    /// Canonically encodes an admitted V3 obligation receipt and KIR identity.
    pub fn from_canonical_parts(
        canonical_kir_v5_identity: [u8; 32],
        formal_obligation_receipt_identity: [u8; 32],
        witness_extent: u64,
        formal_obligation_receipt: &[u8],
    ) -> Result<Self, ProductionLineageEvidenceErrorV3> {
        let canonical_bytes = encode_formal_memory_admission(
            canonical_kir_v5_identity,
            formal_obligation_receipt_identity,
            witness_extent,
            FormalMemoryCompletenessPolicyV3::RequireCompleteConflictFree,
            FormalMemoryCompletenessStatusV3::Complete,
            0,
            0,
            formal_obligation_receipt,
        )?;
        Self::decode(&canonical_bytes)
    }

    /// Strictly decodes one complete canonical V3 formal-admission encoding.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionLineageEvidenceErrorV3> {
        preflight_total_bytes(
            EvidenceKindV3::FormalMemoryAdmission,
            bytes.len(),
            MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V3,
        )?;
        let mut reader = ReaderV3::new(bytes);
        decode_common_header(
            &mut reader,
            EvidenceKindV3::FormalMemoryAdmission,
            FORMAL_MEMORY_MAGIC_V3,
            FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V3,
            FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V3,
        )?;
        let canonical_kir_v5_identity = reader.fixed::<32>()?;
        let formal_obligation_receipt_identity = reader.fixed::<32>()?;
        require_nonzero_identity(
            "canonical Kernel IR V5 identity",
            &canonical_kir_v5_identity,
        )?;
        require_nonzero_identity(
            "formal-obligation receipt identity",
            &formal_obligation_receipt_identity,
        )?;
        let witness_extent = reader.u64()?;
        if !is_production_witness_invocation_count(witness_extent) {
            return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
                "flattened witness invocation count does not match the production policy",
            ));
        }
        let completeness_policy = decode_completeness_policy(reader.u16()?)?;
        let completeness_status = decode_completeness_status(reader.u16()?)?;
        let static_conflict_count = reader.u32()?;
        let inter_invocation_conflict_count = reader.u32()?;
        if static_conflict_count != 0 || inter_invocation_conflict_count != 0 {
            return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
                "complete production admission must have zero conflict counts",
            ));
        }
        let receipt_len_u32 = reader.u32()?;
        let receipt_len = usize::try_from(receipt_len_u32).map_err(|_| {
            ProductionLineageEvidenceErrorV3::Overflow {
                field: "formal-obligation receipt length",
            }
        })?;
        if receipt_len == 0 {
            return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
                "formal-obligation receipt is empty",
            ));
        }
        if receipt_len
            > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V3
                .saturating_sub(FORMAL_MEMORY_HEADER_BYTES_V3)
        {
            return Err(ProductionLineageEvidenceErrorV3::LimitExceeded {
                field: "embedded formal-obligation receipt bytes",
                actual: receipt_len,
                max: MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V3 - FORMAL_MEMORY_HEADER_BYTES_V3,
            });
        }
        if reader.remaining() != receipt_len {
            return Err(if reader.remaining() < receipt_len {
                ProductionLineageEvidenceErrorV3::Truncated
            } else {
                ProductionLineageEvidenceErrorV3::TrailingBytes
            });
        }
        let receipt_offset = reader.offset();
        let receipt_bytes = reader.take(receipt_len)?;
        reader.finish()?;

        // The outer length bound is checked before allocating nested bytes.
        let receipt = InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
            receipt_bytes.to_vec(),
        )
        .map_err(ProductionLineageEvidenceErrorV3::FormalObligationReceipt)?;
        receipt
            .revalidate()
            .map_err(ProductionLineageEvidenceErrorV3::FormalObligationReceipt)?;
        if receipt.identity().digest() != &formal_obligation_receipt_identity {
            return Err(ProductionLineageEvidenceErrorV3::NestedIdentityMismatch);
        }

        let reencoded = encode_formal_memory_admission(
            canonical_kir_v5_identity,
            formal_obligation_receipt_identity,
            witness_extent,
            completeness_policy,
            completeness_status,
            static_conflict_count,
            inter_invocation_conflict_count,
            receipt.canonical_bytes(),
        )?;
        if reencoded != bytes {
            return Err(ProductionLineageEvidenceErrorV3::NonCanonical);
        }
        let identity = FormalMemoryAdmissionEvidenceIdentityV3(canonical_identity(
            FORMAL_MEMORY_IDENTITY_DOMAIN_V3,
            &reencoded,
        ));
        require_nonzero_identity("formal-memory evidence identity", identity.digest())?;
        Ok(Self {
            canonical_bytes: reencoded,
            identity,
            canonical_kir_v5_identity,
            formal_obligation_receipt_identity,
            witness_extent,
            completeness_policy,
            completeness_status,
            static_conflict_count,
            inter_invocation_conflict_count,
            formal_obligation_receipt_offset: receipt_offset,
        })
    }

    /// Rechecks strict decoding, nested receipt validity, re-encoding, and all
    /// retained identities.
    pub fn revalidate(&self) -> Result<(), ProductionLineageEvidenceErrorV3> {
        let decoded = Self::decode(&self.canonical_bytes)?;
        if decoded.identity != self.identity
            || decoded.canonical_kir_v5_identity != self.canonical_kir_v5_identity
            || decoded.formal_obligation_receipt_identity != self.formal_obligation_receipt_identity
            || decoded.witness_extent != self.witness_extent
            || decoded.completeness_policy != self.completeness_policy
            || decoded.completeness_status != self.completeness_status
            || decoded.static_conflict_count != self.static_conflict_count
            || decoded.inter_invocation_conflict_count != self.inter_invocation_conflict_count
            || decoded.formal_obligation_receipt_offset != self.formal_obligation_receipt_offset
        {
            return Err(ProductionLineageEvidenceErrorV3::IdentityMismatch);
        }
        Ok(())
    }

    /// Returns the exact canonical V3 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Consumes inert evidence and returns its exact canonical V3 bytes.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    /// Returns the exact inert evidence identity.
    pub const fn identity(&self) -> &FormalMemoryAdmissionEvidenceIdentityV3 {
        &self.identity
    }

    /// Returns the identity of the exact canonical Kernel IR V5 bytes.
    pub const fn canonical_kir_v5_identity(&self) -> &[u8; 32] {
        &self.canonical_kir_v5_identity
    }

    /// Returns the identity of the embedded exact formal-obligation receipt.
    pub const fn formal_obligation_receipt_identity(&self) -> &[u8; 32] {
        &self.formal_obligation_receipt_identity
    }

    /// Returns the embedded exact canonical formal-obligation receipt bytes.
    pub fn formal_obligation_receipt_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.formal_obligation_receipt_offset..]
    }

    /// Returns the exact flattened structural witness invocation count.
    pub const fn witness_extent(&self) -> u64 {
        self.witness_extent
    }

    /// Returns the completeness policy committed by this evidence.
    pub const fn completeness_policy(&self) -> FormalMemoryCompletenessPolicyV3 {
        self.completeness_policy
    }

    /// Returns the completeness status committed by this evidence.
    pub const fn completeness_status(&self) -> FormalMemoryCompletenessStatusV3 {
        self.completeness_status
    }

    /// Returns the unresolved static conflict count, which is zero for an
    /// admitted production owner.
    pub const fn static_conflict_count(&self) -> u32 {
        self.static_conflict_count
    }

    /// Returns the inter-invocation conflict count, which is zero for an
    /// admitted production owner.
    pub const fn inter_invocation_conflict_count(&self) -> u32 {
        self.inter_invocation_conflict_count
    }

    /// Inert lineage content never grants artifact, proof, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl TryFrom<&[u8]> for InertCanonicalFormalMemoryAdmissionEvidenceV3 {
    type Error = ProductionLineageEvidenceErrorV3;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// V3 lineage evidence category used by bounded codec diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceKindV3 {
    /// MIR-to-KIR correspondence evidence.
    MirToKirCorrespondence,
    /// Formal-memory admission evidence.
    FormalMemoryAdmission,
}

/// Failure to derive, encode, or strictly decode V3 production lineage evidence.
#[derive(Debug)]
pub enum ProductionLineageEvidenceErrorV3 {
    /// The exact nested formal-obligation receipt failed validation.
    FormalObligationReceipt(FormalMemoryReceiptErrorV1),
    /// Input exceeded a hard byte bound before decoding allocation.
    TooLarge {
        /// Evidence category.
        evidence: EvidenceKindV3,
        /// Observed bytes.
        actual: usize,
        /// Maximum accepted bytes.
        max: usize,
    },
    /// A bounded count exceeded its hard cap.
    LimitExceeded {
        /// Bounded field.
        field: &'static str,
        /// Observed count.
        actual: usize,
        /// Maximum accepted count.
        max: usize,
    },
    /// A checked size calculation overflowed.
    Overflow {
        /// Size field that overflowed.
        field: &'static str,
    },
    /// Input ended before a complete field was available.
    Truncated,
    /// Bytes remained after the exact canonical value.
    TrailingBytes,
    /// The wire magic did not match the selected evidence kind.
    InvalidMagic {
        /// Evidence category.
        evidence: EvidenceKindV3,
    },
    /// The wire version is unsupported.
    UnknownVersion {
        /// Evidence category.
        evidence: EvidenceKindV3,
        /// Rejected version.
        version: u16,
    },
    /// The codec policy is unsupported.
    UnknownPolicy {
        /// Evidence category.
        evidence: EvidenceKindV3,
        /// Rejected policy.
        policy: u16,
    },
    /// Reserved flags were nonzero.
    UnsupportedFlags(u16),
    /// A reserved field was nonzero.
    ReservedNonzero,
    /// The declared total length did not equal the supplied bytes.
    InvalidLength {
        /// Declared length.
        declared: usize,
        /// Supplied length.
        actual: usize,
    },
    /// A required content identity was all zeroes.
    ZeroIdentity {
        /// Rejected identity field.
        field: &'static str,
    },
    /// A correspondence invariant was violated.
    InvalidCorrespondence(&'static str),
    /// A formal-admission invariant was violated.
    InvalidFormalAdmission(&'static str),
    /// The formal completeness policy tag is unsupported.
    UnknownCompletenessPolicy(u16),
    /// The formal completeness status tag is unsupported.
    UnknownCompletenessStatus(u16),
    /// The nested receipt identity did not match its exact bytes.
    NestedIdentityMismatch,
    /// Decoding and structured re-encoding changed the bytes.
    NonCanonical,
    /// Retained fields or content identity changed during revalidation.
    IdentityMismatch,
}

impl fmt::Display for ProductionLineageEvidenceErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FormalObligationReceipt(error) => {
                write!(
                    formatter,
                    "canonical formal-obligation receipt failed: {error}"
                )
            }
            Self::TooLarge {
                evidence,
                actual,
                max,
            } => write!(formatter, "{evidence:?} bytes {actual} exceed limit {max}"),
            Self::LimitExceeded { field, actual, max } => {
                write!(formatter, "{field} count {actual} exceeds limit {max}")
            }
            Self::Overflow { field } => write!(formatter, "{field} size overflowed"),
            Self::Truncated => formatter.write_str("lineage evidence is truncated"),
            Self::TrailingBytes => formatter.write_str("lineage evidence has trailing bytes"),
            Self::InvalidMagic { evidence } => {
                write!(formatter, "invalid {evidence:?} wire magic")
            }
            Self::UnknownVersion { evidence, version } => {
                write!(formatter, "unsupported {evidence:?} version {version}")
            }
            Self::UnknownPolicy { evidence, policy } => {
                write!(formatter, "unsupported {evidence:?} policy {policy}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported lineage evidence flags {flags:#06x}")
            }
            Self::ReservedNonzero => {
                formatter.write_str("reserved lineage evidence field is nonzero")
            }
            Self::InvalidLength { declared, actual } => write!(
                formatter,
                "declared lineage evidence length {declared} does not equal supplied length {actual}",
            ),
            Self::ZeroIdentity { field } => write!(formatter, "{field} must be nonzero"),
            Self::InvalidCorrespondence(detail) => {
                write!(formatter, "invalid MIR-to-KIR correspondence: {detail}")
            }
            Self::InvalidFormalAdmission(detail) => {
                write!(formatter, "invalid formal-memory admission: {detail}")
            }
            Self::UnknownCompletenessPolicy(policy) => {
                write!(formatter, "unsupported formal completeness policy {policy}")
            }
            Self::UnknownCompletenessStatus(status) => {
                write!(formatter, "unsupported formal completeness status {status}")
            }
            Self::NestedIdentityMismatch => {
                formatter.write_str("formal-obligation receipt identity does not match exact bytes")
            }
            Self::NonCanonical => formatter.write_str("lineage evidence is not canonical"),
            Self::IdentityMismatch => formatter.write_str("lineage evidence identity mismatch"),
        }
    }
}

impl Error for ProductionLineageEvidenceErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FormalObligationReceipt(error) => Some(error),
            Self::TooLarge { .. }
            | Self::LimitExceeded { .. }
            | Self::Overflow { .. }
            | Self::Truncated
            | Self::TrailingBytes
            | Self::InvalidMagic { .. }
            | Self::UnknownVersion { .. }
            | Self::UnknownPolicy { .. }
            | Self::UnsupportedFlags(_)
            | Self::ReservedNonzero
            | Self::InvalidLength { .. }
            | Self::ZeroIdentity { .. }
            | Self::InvalidCorrespondence(_)
            | Self::InvalidFormalAdmission(_)
            | Self::UnknownCompletenessPolicy(_)
            | Self::UnknownCompletenessStatus(_)
            | Self::NestedIdentityMismatch
            | Self::NonCanonical
            | Self::IdentityMismatch => None,
        }
    }
}

fn validate_canonical_correspondence(
    function_count: u32,
    blocks: &[MirToKirBlockCorrespondenceEvidenceV3],
) -> Result<(), ProductionLineageEvidenceErrorV3> {
    let mut cursor = 0_usize;
    let mut previous_function = None;
    let mut covered_function_count = 0_u32;
    while let Some(first) = blocks.get(cursor) {
        let function = first.semantic_function;
        if previous_function.is_some_and(|previous| previous >= function) {
            return Err(ProductionLineageEvidenceErrorV3::InvalidCorrespondence(
                "semantic function locators are not in strictly increasing canonical order",
            ));
        }
        let mut block = 0_u32;
        while let Some(record) = blocks.get(cursor) {
            if record.semantic_function != function {
                break;
            }
            if record.semantic_block != block {
                return Err(ProductionLineageEvidenceErrorV3::InvalidCorrespondence(
                    "semantic block locators are not contiguous canonical indices",
                ));
            }
            if record.kernel_ir_block != record.semantic_block {
                return Err(ProductionLineageEvidenceErrorV3::InvalidCorrespondence(
                    "current V3 policy requires exact semantic-to-Kernel-IR block identity",
                ));
            }
            cursor += 1;
            block = block
                .checked_add(1)
                .ok_or(ProductionLineageEvidenceErrorV3::Overflow {
                    field: "semantic block index",
                })?;
        }
        if block == 0 {
            return Err(ProductionLineageEvidenceErrorV3::InvalidCorrespondence(
                "every covered semantic function must contain a block",
            ));
        }
        previous_function = Some(function);
        covered_function_count = covered_function_count.checked_add(1).ok_or(
            ProductionLineageEvidenceErrorV3::Overflow {
                field: "covered semantic function count",
            },
        )?;
    }
    if cursor != blocks.len() || covered_function_count != function_count {
        return Err(ProductionLineageEvidenceErrorV3::InvalidCorrespondence(
            "covered semantic function count differs from canonical records",
        ));
    }
    Ok(())
}

fn encode_correspondence(
    semantic_sha256: [u8; 32],
    canonical_kir_v5_identity: [u8; 32],
    function_count: u32,
    blocks: &[MirToKirBlockCorrespondenceEvidenceV3],
) -> Result<Vec<u8>, ProductionLineageEvidenceErrorV3> {
    require_nonzero_identity("semantic MIR SHA-256", &semantic_sha256)?;
    require_nonzero_identity(
        "canonical Kernel IR V5 identity",
        &canonical_kir_v5_identity,
    )?;
    validate_canonical_correspondence(function_count, blocks)?;
    enforce_count(
        "correspondence functions",
        function_count as usize,
        MAX_MIR_TO_KIR_CORRESPONDENCE_FUNCTIONS_V3,
    )?;
    enforce_count(
        "correspondence blocks",
        blocks.len(),
        MAX_MIR_TO_KIR_CORRESPONDENCE_BLOCKS_V3,
    )?;
    let exact_size = CORRESPONDENCE_HEADER_BYTES_V3
        .checked_add(
            blocks
                .len()
                .checked_mul(CORRESPONDENCE_RECORD_BYTES_V3)
                .ok_or(ProductionLineageEvidenceErrorV3::Overflow {
                    field: "correspondence record bytes",
                })?,
        )
        .ok_or(ProductionLineageEvidenceErrorV3::Overflow {
            field: "correspondence evidence bytes",
        })?;
    preflight_total_bytes(
        EvidenceKindV3::MirToKirCorrespondence,
        exact_size,
        MAX_MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_BYTES_V3,
    )?;
    let declared =
        u32::try_from(exact_size).map_err(|_| ProductionLineageEvidenceErrorV3::Overflow {
            field: "correspondence evidence length",
        })?;
    let block_count =
        u32::try_from(blocks.len()).map_err(|_| ProductionLineageEvidenceErrorV3::Overflow {
            field: "correspondence block count",
        })?;

    // Every checked bound precedes this exact allocation.
    let mut bytes = Vec::with_capacity(exact_size);
    encode_common_header(
        &mut bytes,
        CORRESPONDENCE_MAGIC_V3,
        MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_VERSION_V3,
        MIR_TO_KIR_CORRESPONDENCE_EVIDENCE_POLICY_V3,
        declared,
    );
    bytes.extend_from_slice(&semantic_sha256);
    bytes.extend_from_slice(&canonical_kir_v5_identity);
    push_u32(&mut bytes, function_count);
    push_u32(&mut bytes, block_count);
    for block in blocks {
        push_u32(&mut bytes, block.semantic_function);
        push_u32(&mut bytes, block.semantic_block);
        push_u32(&mut bytes, block.kernel_ir_block);
        push_u32(&mut bytes, block.source_statement_count);
    }
    debug_assert_eq!(bytes.len(), exact_size);
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn encode_formal_memory_admission(
    canonical_kir_v5_identity: [u8; 32],
    formal_obligation_receipt_identity: [u8; 32],
    witness_extent: u64,
    completeness_policy: FormalMemoryCompletenessPolicyV3,
    completeness_status: FormalMemoryCompletenessStatusV3,
    static_conflict_count: u32,
    inter_invocation_conflict_count: u32,
    formal_obligation_receipt: &[u8],
) -> Result<Vec<u8>, ProductionLineageEvidenceErrorV3> {
    require_nonzero_identity(
        "canonical Kernel IR V5 identity",
        &canonical_kir_v5_identity,
    )?;
    require_nonzero_identity(
        "formal-obligation receipt identity",
        &formal_obligation_receipt_identity,
    )?;
    if !is_production_witness_invocation_count(witness_extent)
        || completeness_policy != FormalMemoryCompletenessPolicyV3::RequireCompleteConflictFree
        || completeness_status != FormalMemoryCompletenessStatusV3::Complete
        || static_conflict_count != 0
        || inter_invocation_conflict_count != 0
        || formal_obligation_receipt.is_empty()
    {
        return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
            "formal admission fields do not satisfy the production policy",
        ));
    }
    validate_formal_receipt_witness(formal_obligation_receipt, witness_extent)?;
    let exact_size = FORMAL_MEMORY_HEADER_BYTES_V3
        .checked_add(formal_obligation_receipt.len())
        .ok_or(ProductionLineageEvidenceErrorV3::Overflow {
            field: "formal-memory admission evidence bytes",
        })?;
    preflight_total_bytes(
        EvidenceKindV3::FormalMemoryAdmission,
        exact_size,
        MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V3,
    )?;
    let declared =
        u32::try_from(exact_size).map_err(|_| ProductionLineageEvidenceErrorV3::Overflow {
            field: "formal-memory admission evidence length",
        })?;
    let receipt_len = u32::try_from(formal_obligation_receipt.len()).map_err(|_| {
        ProductionLineageEvidenceErrorV3::Overflow {
            field: "formal-obligation receipt length",
        }
    })?;

    // Every checked bound precedes this exact allocation.
    let mut bytes = Vec::with_capacity(exact_size);
    encode_common_header(
        &mut bytes,
        FORMAL_MEMORY_MAGIC_V3,
        FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V3,
        FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V3,
        declared,
    );
    bytes.extend_from_slice(&canonical_kir_v5_identity);
    bytes.extend_from_slice(&formal_obligation_receipt_identity);
    push_u64(&mut bytes, witness_extent);
    push_u16(&mut bytes, completeness_policy as u16);
    push_u16(&mut bytes, completeness_status as u16);
    push_u32(&mut bytes, static_conflict_count);
    push_u32(&mut bytes, inter_invocation_conflict_count);
    push_u32(&mut bytes, receipt_len);
    bytes.extend_from_slice(formal_obligation_receipt);
    debug_assert_eq!(bytes.len(), exact_size);
    Ok(bytes)
}

fn preflight_total_bytes(
    evidence: EvidenceKindV3,
    actual: usize,
    max: usize,
) -> Result<(), ProductionLineageEvidenceErrorV3> {
    if actual > max {
        return Err(ProductionLineageEvidenceErrorV3::TooLarge {
            evidence,
            actual,
            max,
        });
    }
    Ok(())
}

fn enforce_count(
    field: &'static str,
    actual: usize,
    max: usize,
) -> Result<(), ProductionLineageEvidenceErrorV3> {
    if actual > max {
        return Err(ProductionLineageEvidenceErrorV3::LimitExceeded { field, actual, max });
    }
    Ok(())
}

fn require_nonzero_identity(
    field: &'static str,
    identity: &[u8; 32],
) -> Result<(), ProductionLineageEvidenceErrorV3> {
    if identity.iter().all(|byte| *byte == 0) {
        return Err(ProductionLineageEvidenceErrorV3::ZeroIdentity { field });
    }
    Ok(())
}

fn canonical_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u32).to_le_bytes());
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn encode_common_header(
    bytes: &mut Vec<u8>,
    magic: [u8; 8],
    version: u16,
    policy: u16,
    declared: u32,
) {
    bytes.extend_from_slice(&magic);
    push_u16(bytes, version);
    push_u16(bytes, policy);
    push_u16(bytes, 0);
    push_u16(bytes, 0);
    push_u32(bytes, declared);
}

fn decode_common_header(
    reader: &mut ReaderV3<'_>,
    evidence: EvidenceKindV3,
    magic: [u8; 8],
    version: u16,
    policy: u16,
) -> Result<(), ProductionLineageEvidenceErrorV3> {
    if reader.fixed::<8>()? != magic {
        return Err(ProductionLineageEvidenceErrorV3::InvalidMagic { evidence });
    }
    let actual_version = reader.u16()?;
    if actual_version != version {
        return Err(ProductionLineageEvidenceErrorV3::UnknownVersion {
            evidence,
            version: actual_version,
        });
    }
    let actual_policy = reader.u16()?;
    if actual_policy != policy {
        return Err(ProductionLineageEvidenceErrorV3::UnknownPolicy {
            evidence,
            policy: actual_policy,
        });
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(ProductionLineageEvidenceErrorV3::UnsupportedFlags(flags));
    }
    if reader.u16()? != 0 {
        return Err(ProductionLineageEvidenceErrorV3::ReservedNonzero);
    }
    let declared = reader.u32()? as usize;
    if declared != reader.bytes.len() {
        return Err(ProductionLineageEvidenceErrorV3::InvalidLength {
            declared,
            actual: reader.bytes.len(),
        });
    }
    Ok(())
}

fn decode_completeness_policy(
    value: u16,
) -> Result<FormalMemoryCompletenessPolicyV3, ProductionLineageEvidenceErrorV3> {
    match value {
        1 => Ok(FormalMemoryCompletenessPolicyV3::RequireCompleteConflictFree),
        other => Err(ProductionLineageEvidenceErrorV3::UnknownCompletenessPolicy(
            other,
        )),
    }
}

fn decode_completeness_status(
    value: u16,
) -> Result<FormalMemoryCompletenessStatusV3, ProductionLineageEvidenceErrorV3> {
    match value {
        1 => Ok(FormalMemoryCompletenessStatusV3::Complete),
        other => Err(ProductionLineageEvidenceErrorV3::UnknownCompletenessStatus(
            other,
        )),
    }
}

fn validate_formal_receipt_witness(
    receipt: &[u8],
    witness_extent: u64,
) -> Result<(), ProductionLineageEvidenceErrorV3> {
    let mut reader = ReaderV3::new(receipt);
    if reader.fixed::<8>()? != FORMAL_OBLIGATION_RECEIPT_MAGIC_V1 {
        return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
            "embedded formal-obligation receipt magic changed after validation",
        ));
    }
    if reader.u16()? != FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V1
        || reader.u16()? != FORMAL_MEMORY_OBLIGATION_POLICY_V1
        || reader.u16()? != 0
        || reader.u16()? != 0
        || reader.u32()? as usize != receipt.len()
    {
        return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
            "embedded formal-obligation receipt header changed after validation",
        ));
    }
    for _ in 0..2 {
        let text_len = reader.u32()? as usize;
        reader.take(text_len)?;
    }
    reader.u8()?;
    reader.u8()?;
    if reader.u16()? != 0 || reader.u8()? != 1 {
        return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
            "embedded formal-obligation receipt has no exact invocation witness",
        ));
    }
    let start = reader.u64()?;
    let end_exclusive = reader.u64()?;
    if start != 0 || end_exclusive != witness_extent {
        return Err(ProductionLineageEvidenceErrorV3::InvalidFormalAdmission(
            "embedded formal-obligation receipt invocation range differs from witness extent",
        ));
    }
    Ok(())
}

const fn is_production_witness_invocation_count(count: u64) -> bool {
    count != 0
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct ReaderV3<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV3<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ProductionLineageEvidenceErrorV3> {
        let end =
            self.offset
                .checked_add(len)
                .ok_or(ProductionLineageEvidenceErrorV3::Overflow {
                    field: "decoder offset",
                })?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionLineageEvidenceErrorV3::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ProductionLineageEvidenceErrorV3> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionLineageEvidenceErrorV3::Truncated)
    }

    fn u16(&mut self) -> Result<u16, ProductionLineageEvidenceErrorV3> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u8(&mut self) -> Result<u8, ProductionLineageEvidenceErrorV3> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u32(&mut self) -> Result<u32, ProductionLineageEvidenceErrorV3> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionLineageEvidenceErrorV3> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn finish(self) -> Result<(), ProductionLineageEvidenceErrorV3> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProductionLineageEvidenceErrorV3::TrailingBytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_correspondence_round_trip_is_exact_and_rejects_trailing_bytes() {
        let evidence = InertCanonicalMirToKirCorrespondenceEvidenceV3::from_canonical_parts(
            [1; 32],
            [2; 32],
            1,
            &[MirToKirBlockCorrespondenceEvidenceV3::from_parts(
                0, 0, 0, 0,
            )],
        )
        .unwrap();
        assert_eq!(
            InertCanonicalMirToKirCorrespondenceEvidenceV3::decode(evidence.canonical_bytes())
                .unwrap(),
            evidence
        );
        let mut trailing = evidence.canonical_bytes().to_vec();
        trailing.push(0);
        assert!(matches!(
            InertCanonicalMirToKirCorrespondenceEvidenceV3::decode(&trailing),
            Err(ProductionLineageEvidenceErrorV3::InvalidLength { .. })
        ));
    }

    #[test]
    fn strict_decoders_reject_empty_inputs() {
        assert!(InertCanonicalMirToKirCorrespondenceEvidenceV3::decode(&[]).is_err());
        assert!(InertCanonicalFormalMemoryAdmissionEvidenceV3::decode(&[]).is_err());
    }
}
