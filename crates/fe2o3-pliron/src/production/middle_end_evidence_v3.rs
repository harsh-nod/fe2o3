//! Strict canonical evidence for the production semantic middle end.
//!
//! The live-produced value binds an exact semantic-MIR owner, the checked
//! ranked kernel, deterministic ranked IR, and the six fixed clean analysis
//! reports. The decoded form is deliberately named `Inert`: byte integrity is
//! not producer authentication, a refinement proof, or execution authority.

use std::{error::Error, fmt, ops::Range};

use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, IndexBinaryKindAttr, MemorySpaceAttr,
    SemanticBinaryKindAttr,
};
use fe2o3_kernel_analysis::{GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1, KernelCheckPassKindV1};
use sha2::{Digest, Sha256};

use super::{
    ProductionRankedKernelErrorV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueV1,
    ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1,
};

const MAGIC_V3: [u8; 8] = *b"F2MEV3\0\0";
const VERSION_V3: u16 = 3;
const FLAGS_V3: u16 = 0;
const ASSURANCE_INTERNAL_CHECKS_ONLY_V3: u8 = 1;
const EQUIVALENCE_REVALIDATED_V3: u8 = 1;
const CLEAN_STATUS_V3: u8 = 1;
const PASS_COUNT_V3: usize = 6;
const PASS_RECORD_BYTES_V3: usize = 10;
const SHA256_BYTES: usize = 32;
const IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V3\0";
const RANKED_KERNEL_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-RANKED-KERNEL-IDENTITY/V3\0";

/// Stable wire domain for a production middle-end evidence record.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V3: &[u8] =
    b"fe2o3.production-middle-end-evidence.v3";

/// Fixed assurance policy. It intentionally does not claim Verus verification.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3: &[u8] = b"fe2o3.internal-checks-only.v3";

/// Maximum complete record size, chosen to fit one V3 lineage receipt.
pub const MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3: usize = 4 * 1024 * 1024;

const FIXED_RECORD_BYTES_V3: usize = MAGIC_V3.len()
    + 2
    + 2
    + 8
    + 4
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V3.len()
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3.len()
    + 1
    + 1
    + 2
    + SHA256_BYTES
    + SHA256_BYTES
    + 4
    + 1
    + PASS_COUNT_V3 * PASS_RECORD_BYTES_V3
    + SHA256_BYTES;

/// Maximum deterministic ranked-IR bytes retained by one complete record.
pub const MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V3: usize =
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3 - FIXED_RECORD_BYTES_V3;

/// Assurance represented by the V3 middle-end record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionMiddleEndAssuranceV3 {
    /// fe2o3's six internal analyses completed cleanly; no Verus proof is claimed.
    InternalChecksOnly,
}

/// One pass in the fixed V3 middle-end order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionMiddleEndEvidencePassV3 {
    MemoryBounds,
    AtomicLegality,
    RaceFreedom,
    BarrierConvergence,
    WorkgroupMemory,
    SemanticRefinement,
}

impl ProductionMiddleEndEvidencePassV3 {
    const fn tag(self) -> u8 {
        match self {
            Self::MemoryBounds => 1,
            Self::AtomicLegality => 2,
            Self::RaceFreedom => 3,
            Self::BarrierConvergence => 4,
            Self::WorkgroupMemory => 5,
            Self::SemanticRefinement => 6,
        }
    }

    const fn from_analysis(pass: KernelCheckPassKindV1) -> Option<Self> {
        match pass {
            KernelCheckPassKindV1::MemoryBounds => Some(Self::MemoryBounds),
            KernelCheckPassKindV1::AtomicLegality => Some(Self::AtomicLegality),
            KernelCheckPassKindV1::RaceFreedom => Some(Self::RaceFreedom),
            KernelCheckPassKindV1::BarrierConvergence => Some(Self::BarrierConvergence),
            KernelCheckPassKindV1::WorkgroupMemory => Some(Self::WorkgroupMemory),
            KernelCheckPassKindV1::SemanticRefinement => Some(Self::SemanticRefinement),
            KernelCheckPassKindV1::Structural | KernelCheckPassKindV1::ControlFlow => None,
        }
    }
}

/// Fixed pass order encoded by every V3 record.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V3: [ProductionMiddleEndEvidencePassV3;
    PASS_COUNT_V3] = [
    ProductionMiddleEndEvidencePassV3::MemoryBounds,
    ProductionMiddleEndEvidencePassV3::AtomicLegality,
    ProductionMiddleEndEvidencePassV3::RaceFreedom,
    ProductionMiddleEndEvidencePassV3::BarrierConvergence,
    ProductionMiddleEndEvidencePassV3::WorkgroupMemory,
    ProductionMiddleEndEvidencePassV3::SemanticRefinement,
];

/// Exact success facts encoded for one mandatory pass.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndPassSuccessV3 {
    pass: ProductionMiddleEndEvidencePassV3,
}

impl ProductionMiddleEndPassSuccessV3 {
    const fn new(pass: ProductionMiddleEndEvidencePassV3) -> Self {
        Self { pass }
    }

    pub const fn pass(self) -> ProductionMiddleEndEvidencePassV3 {
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

const PASS_SUCCESSES_V3: [ProductionMiddleEndPassSuccessV3; PASS_COUNT_V3] = [
    ProductionMiddleEndPassSuccessV3::new(ProductionMiddleEndEvidencePassV3::MemoryBounds),
    ProductionMiddleEndPassSuccessV3::new(ProductionMiddleEndEvidencePassV3::AtomicLegality),
    ProductionMiddleEndPassSuccessV3::new(ProductionMiddleEndEvidencePassV3::RaceFreedom),
    ProductionMiddleEndPassSuccessV3::new(ProductionMiddleEndEvidencePassV3::BarrierConvergence),
    ProductionMiddleEndPassSuccessV3::new(ProductionMiddleEndEvidencePassV3::WorkgroupMemory),
    ProductionMiddleEndPassSuccessV3::new(ProductionMiddleEndEvidencePassV3::SemanticRefinement),
];

/// Domain-separated identity of one exact canonical V3 record.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndEvidenceIdentityV3 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl ProductionMiddleEndEvidenceIdentityV3 {
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Recomputes this inert content identity without granting authority.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        if self.byte_len != bytes.len() as u64 || bytes.len() < SHA256_BYTES {
            return false;
        }
        let terminal = bytes.len() - SHA256_BYTES;
        bytes[terminal..] == self.sha256
            && derive_evidence_identity(&bytes[..terminal]) == Some(self.sha256)
    }
}

impl fmt::Debug for ProductionMiddleEndEvidenceIdentityV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionMiddleEndEvidenceIdentityV3")
            .field("sha256", &self.sha256)
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

/// Strictly decoded, internally consistent V3 bytes.
///
/// This type is inert: decoding does not establish a live producer, replay
/// freshness, derivation correctness, Verus proof, or any execution authority.
#[derive(Eq, PartialEq)]
pub struct InertProductionMiddleEndEvidenceV3 {
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV3,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionMiddleEndEvidenceV3 {
    /// Strictly decodes one complete canonical V3 record with no fallback.
    /// All declared and aggregate bounds are checked before allocation.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV3> {
        if bytes.len() > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::TooLarge {
                actual: bytes.len(),
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3,
            });
        }

        let mut reader = ReaderV3::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != VERSION_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::UnsupportedVersion(
                version,
            ));
        }
        let flags = reader.u16()?;
        if flags != FLAGS_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::UnsupportedFlags(
                flags,
            ));
        }
        let declared_len = reader.u64()?;
        let declared_len_usize = usize::try_from(declared_len)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV3::InvalidLength(declared_len))?;
        if declared_len_usize > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::TooLarge {
                actual: declared_len_usize,
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3,
            });
        }
        if declared_len_usize > bytes.len() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::Truncated);
        }
        if declared_len_usize < bytes.len() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::TrailingBytes);
        }
        if declared_len_usize < FIXED_RECORD_BYTES_V3 + 1 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidLength(
                declared_len,
            ));
        }
        if reader.u32()? != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::NonzeroReserved);
        }

        let domain_len = usize::from(reader.u16()?);
        if domain_len != PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V3.len()
            || reader.take(domain_len)? != PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V3
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidDomain);
        }
        let policy_len = usize::from(reader.u16()?);
        if policy_len != PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3.len()
            || reader.take(policy_len)? != PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidPolicy);
        }
        let assurance = reader.u8()?;
        if assurance != ASSURANCE_INTERNAL_CHECKS_ONLY_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidAssurance(
                assurance,
            ));
        }
        if reader.u8()? != EQUIVALENCE_REVALIDATED_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::SemanticEquivalenceNotEstablished);
        }
        if reader.u16()? != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::NonzeroReserved);
        }

        let source_semantic_identity = reader.fixed::<SHA256_BYTES>()?;
        if source_semantic_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroSemanticIdentity);
        }
        let ranked_kernel_identity = reader.fixed::<SHA256_BYTES>()?;
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroRankedKernelIdentity);
        }

        let ranked_ir_len = usize::try_from(reader.u32()?).map_err(|_| {
            ProductionMiddleEndEvidenceCodecErrorV3::RankedIrTooLarge {
                actual: usize::MAX,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V3,
            }
        })?;
        if ranked_ir_len > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::RankedIrTooLarge {
                actual: ranked_ir_len,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V3,
            });
        }
        let ranked_ir_bytes = reader.take(ranked_ir_len)?;
        validate_ranked_ir(ranked_ir_bytes)?;

        let pass_count = reader.u8()?;
        if usize::from(pass_count) != PASS_COUNT_V3 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidPassCount(
                pass_count,
            ));
        }
        for (index, expected) in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V3
            .iter()
            .copied()
            .enumerate()
        {
            let actual = reader.u8()?;
            if actual != expected.tag() {
                return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidPassOrder {
                    index,
                    expected,
                    actual,
                });
            }
            let status = reader.u8()?;
            if status != CLEAN_STATUS_V3 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV3::InvalidPassStatus {
                    pass: expected,
                    actual: status,
                });
            }
            let findings = reader.u32()?;
            if findings != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV3::NonzeroFindings {
                    pass: expected,
                    actual: findings,
                });
            }
            let compiler_authority = reader.u8()?;
            let artifact_authority = reader.u8()?;
            if compiler_authority != 0 || artifact_authority != 0 {
                return Err(
                    ProductionMiddleEndEvidenceCodecErrorV3::AuthorityClaimInEncoding {
                        pass: expected,
                    },
                );
            }
            if reader.u16()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV3::NonzeroReserved);
            }
        }

        let terminal_offset = reader.offset();
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if declared_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroIdentity);
        }
        if !reader.is_empty() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::TrailingBytes);
        }
        if derive_evidence_identity(&bytes[..terminal_offset]) != Some(declared_identity) {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::IdentityMismatch);
        }

        let ranked_ir = std::str::from_utf8(ranked_ir_bytes)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV3::InvalidRankedIrUtf8)?;
        let reconstructed =
            encode_record(source_semantic_identity, ranked_kernel_identity, ranked_ir)?;
        if reconstructed.canonical_bytes.as_ref() != bytes {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::NonCanonical);
        }
        Ok(Self {
            source_semantic_identity,
            ranked_kernel_identity,
            ranked_ir_range: reconstructed.ranked_ir_range,
            identity: reconstructed.identity,
            canonical_bytes: reconstructed.canonical_bytes,
        })
    }

    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV3 {
        ProductionMiddleEndAssuranceV3::InternalChecksOnly
    }

    pub const fn policy(&self) -> &'static [u8] {
        PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3
    }

    pub const fn source_semantic_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.source_semantic_identity
    }

    pub const fn ranked_kernel_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.ranked_kernel_identity
    }

    pub fn ranked_ir(&self) -> &str {
        std::str::from_utf8(&self.canonical_bytes[self.ranked_ir_range.clone()])
            .expect("validated V3 ranked IR remains UTF-8")
    }

    pub const fn pass_successes(
        &self,
    ) -> &'static [ProductionMiddleEndPassSuccessV3; PASS_COUNT_V3] {
        &PASS_SUCCESSES_V3
    }

    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV3 {
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

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertProductionMiddleEndEvidenceV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionMiddleEndEvidenceV3")
            .field("source_semantic_identity", &self.source_semantic_identity)
            .field("ranked_kernel_identity", &self.ranked_kernel_identity)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Move-only V3 middle-end evidence constructed from the two live owners.
///
/// The value carries no authority. Its distinct type records that construction
/// revalidated the live semantic owner, ranked structure, and all six reports.
/// Strict decoding returns [`InertProductionMiddleEndEvidenceV3`] instead.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionMiddleEndEvidenceV3;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionMiddleEndEvidenceV3>();
/// ```
#[must_use = "dropping middle-end evidence abandons the live-produced stage record"]
pub struct ProductionMiddleEndEvidenceV3 {
    inert: InertProductionMiddleEndEvidenceV3,
}

impl ProductionMiddleEndEvidenceV3 {
    /// Revalidates both live owners and constructs their exact canonical record.
    pub fn try_new(
        semantic: &ProductionSemanticMirOwnerV1,
        ranked: &ProductionRankedKernelLoweringInputV1,
        deterministic_ranked_ir: &str,
    ) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV3> {
        validate_ranked_ir(deterministic_ranked_ir.as_bytes())?;
        semantic
            .verify_equivalence()
            .map_err(ProductionMiddleEndEvidenceCodecErrorV3::SemanticOwner)?;
        let source_semantic_identity = *semantic.semantic().semantic_sha256().as_bytes();
        if source_semantic_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroSemanticIdentity);
        }
        let actual_semantic_identity: [u8; SHA256_BYTES] =
            Sha256::digest(semantic.semantic().canonical_encoding()).into();
        if actual_semantic_identity != source_semantic_identity
            || semantic.locator().semantic_sha256().as_bytes() != &source_semantic_identity
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::SemanticSourceIdentityMismatch);
        }

        ranked
            .revalidate_structure()
            .map_err(ProductionMiddleEndEvidenceCodecErrorV3::RankedKernel)?;
        validate_live_reports(ranked)?;

        let ranked_kernel_identity = derive_ranked_kernel_identity(ranked);
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroRankedKernelIdentity);
        }
        let encoded = encode_record(
            source_semantic_identity,
            ranked_kernel_identity,
            deterministic_ranked_ir,
        )?;
        Ok(Self {
            inert: InertProductionMiddleEndEvidenceV3 {
                source_semantic_identity,
                ranked_kernel_identity,
                ranked_ir_range: encoded.ranked_ir_range,
                identity: encoded.identity,
                canonical_bytes: encoded.canonical_bytes,
            },
        })
    }

    pub const fn as_inert(&self) -> &InertProductionMiddleEndEvidenceV3 {
        &self.inert
    }

    pub fn into_inert(self) -> InertProductionMiddleEndEvidenceV3 {
        self.inert
    }

    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV3 {
        self.inert.assurance()
    }

    pub const fn source_semantic_identity(&self) -> &[u8; SHA256_BYTES] {
        self.inert.source_semantic_identity()
    }

    pub const fn ranked_kernel_identity(&self) -> &[u8; SHA256_BYTES] {
        self.inert.ranked_kernel_identity()
    }

    pub fn ranked_ir(&self) -> &str {
        self.inert.ranked_ir()
    }

    pub const fn pass_successes(
        &self,
    ) -> &'static [ProductionMiddleEndPassSuccessV3; PASS_COUNT_V3] {
        self.inert.pass_successes()
    }

    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV3 {
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

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for ProductionMiddleEndEvidenceV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionMiddleEndEvidenceV3")
            .field("identity", &self.identity())
            .field("assurance", &self.assurance())
            .finish_non_exhaustive()
    }
}

/// Fail-closed construction and decoding errors for the V3 codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionMiddleEndEvidenceCodecErrorV3 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    RankedKernel(ProductionRankedKernelErrorV1),
    SemanticSourceIdentityMismatch,
    ReportPassOrderMismatch {
        index: usize,
        expected: ProductionMiddleEndEvidencePassV3,
    },
    ReportNotClean {
        pass: ProductionMiddleEndEvidencePassV3,
    },
    ReportAuthorityClaim {
        pass: ProductionMiddleEndEvidencePassV3,
    },
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
    InvalidAssurance(u8),
    SemanticEquivalenceNotEstablished,
    ZeroSemanticIdentity,
    ZeroRankedKernelIdentity,
    InvalidPassCount(u8),
    InvalidPassOrder {
        index: usize,
        expected: ProductionMiddleEndEvidencePassV3,
        actual: u8,
    },
    InvalidPassStatus {
        pass: ProductionMiddleEndEvidencePassV3,
        actual: u8,
    },
    NonzeroFindings {
        pass: ProductionMiddleEndEvidencePassV3,
        actual: u32,
    },
    AuthorityClaimInEncoding {
        pass: ProductionMiddleEndEvidencePassV3,
    },
    ZeroIdentity,
    IdentityMismatch,
    NonCanonical,
    AllocationFailed,
}

impl fmt::Display for ProductionMiddleEndEvidenceCodecErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticOwner(error) => {
                write!(formatter, "semantic owner revalidation failed: {error}")
            }
            Self::RankedKernel(error) => {
                write!(formatter, "ranked kernel revalidation failed: {error}")
            }
            Self::SemanticSourceIdentityMismatch => formatter.write_str(
                "semantic source identity does not match the exact retained MIR and locator graph",
            ),
            Self::ReportPassOrderMismatch { index, .. } => write!(
                formatter,
                "live middle-end report at position {index} has the wrong pass identity"
            ),
            Self::ReportNotClean { .. } => {
                formatter.write_str("a mandatory live middle-end report is not exactly clean")
            }
            Self::ReportAuthorityClaim { .. } => {
                formatter.write_str("a live middle-end report unexpectedly claims authority")
            }
            Self::EmptyRankedIr => formatter.write_str("deterministic ranked IR is empty"),
            Self::RankedIrTooLarge { actual, limit } => {
                write!(formatter, "ranked IR byte length {actual} exceeds {limit}")
            }
            Self::InvalidRankedIrUtf8 => formatter.write_str("ranked IR is not valid UTF-8"),
            Self::NonCanonicalRankedIrByte { offset } => write!(
                formatter,
                "ranked IR contains a noncanonical byte at offset {offset}"
            ),
            Self::RankedIrMissingFinalNewline => {
                formatter.write_str("ranked IR must end in one newline-delimited record")
            }
            Self::TooLarge { actual, limit } => write!(
                formatter,
                "middle-end evidence byte length {actual} exceeds {limit}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid middle-end evidence magic"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported middle-end evidence version {version}"
            ),
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported middle-end evidence flags {flags}")
            }
            Self::InvalidLength(length) => write!(
                formatter,
                "invalid middle-end evidence byte length {length}"
            ),
            Self::Truncated => formatter.write_str("truncated middle-end evidence"),
            Self::TrailingBytes => formatter.write_str("trailing middle-end evidence bytes"),
            Self::NonzeroReserved => {
                formatter.write_str("middle-end evidence reserved field is nonzero")
            }
            Self::InvalidDomain => formatter.write_str("invalid middle-end evidence domain"),
            Self::InvalidPolicy => formatter.write_str("invalid middle-end evidence policy"),
            Self::InvalidAssurance(value) => {
                write!(formatter, "invalid middle-end assurance tag {value}")
            }
            Self::SemanticEquivalenceNotEstablished => {
                formatter.write_str("semantic equivalence success fact is absent")
            }
            Self::ZeroSemanticIdentity => formatter.write_str("source semantic identity is zero"),
            Self::ZeroRankedKernelIdentity => formatter.write_str("ranked kernel identity is zero"),
            Self::InvalidPassCount(count) => {
                write!(formatter, "invalid middle-end pass count {count}")
            }
            Self::InvalidPassOrder { index, .. } => write!(
                formatter,
                "noncanonical middle-end pass at position {index}"
            ),
            Self::InvalidPassStatus { .. } => {
                formatter.write_str("a middle-end pass status is not exactly clean")
            }
            Self::NonzeroFindings { actual, .. } => {
                write!(formatter, "a middle-end pass records {actual} findings")
            }
            Self::AuthorityClaimInEncoding { .. } => {
                formatter.write_str("middle-end evidence contains a forbidden authority claim")
            }
            Self::ZeroIdentity => formatter.write_str("middle-end evidence identity is zero"),
            Self::IdentityMismatch => formatter.write_str("middle-end evidence identity mismatch"),
            Self::NonCanonical => {
                formatter.write_str("middle-end evidence is not canonically encoded")
            }
            Self::AllocationFailed => formatter.write_str("middle-end evidence allocation failed"),
        }
    }
}

impl Error for ProductionMiddleEndEvidenceCodecErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticOwner(error) => Some(error),
            Self::RankedKernel(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct ObservedPassFactV3 {
    pass: KernelCheckPassKindV1,
    clean: bool,
    findings: usize,
    compiler_authority: bool,
    artifact_authority: bool,
}

fn validate_live_reports(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV3> {
    let facts = [
        ObservedPassFactV3 {
            pass: ranked.bounds_report().pass(),
            clean: ranked.bounds_report().is_clean(),
            findings: ranked.bounds_report().findings().len(),
            compiler_authority: ranked
                .bounds_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked.bounds_report().grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV3 {
            pass: ranked.atomic_report().pass(),
            clean: ranked.atomic_report().is_clean(),
            findings: ranked.atomic_report().findings().len(),
            compiler_authority: ranked
                .atomic_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked.atomic_report().grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV3 {
            pass: ranked.race_report().pass(),
            clean: ranked.race_report().is_clean(),
            findings: ranked.race_report().findings().len(),
            compiler_authority: ranked.race_report().grants_compiler_refinement_authority(),
            artifact_authority: ranked.race_report().grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV3 {
            pass: ranked.barrier_report().pass(),
            clean: ranked.barrier_report().is_clean(),
            findings: ranked.barrier_report().findings().len(),
            compiler_authority: ranked
                .barrier_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked
                .barrier_report()
                .grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV3 {
            pass: ranked.workgroup_report().pass(),
            clean: ranked.workgroup_report().is_clean(),
            findings: ranked.workgroup_report().findings().len(),
            compiler_authority: ranked
                .workgroup_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked
                .workgroup_report()
                .grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV3 {
            pass: ranked.semantic_report().pass(),
            clean: ranked.semantic_report().is_clean(),
            findings: ranked.semantic_report().findings().len(),
            compiler_authority: ranked
                .semantic_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked
                .semantic_report()
                .grants_artifact_or_launch_authority(),
        },
    ];
    validate_observed_pass_facts(&facts)
}

fn validate_observed_pass_facts(
    facts: &[ObservedPassFactV3; PASS_COUNT_V3],
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV3> {
    for (index, ((fact, analysis_expected), evidence_expected)) in facts
        .iter()
        .zip(GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1.iter())
        .zip(PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V3.iter())
        .enumerate()
    {
        if fact.pass != *analysis_expected
            || ProductionMiddleEndEvidencePassV3::from_analysis(fact.pass)
                != Some(*evidence_expected)
        {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV3::ReportPassOrderMismatch {
                    index,
                    expected: *evidence_expected,
                },
            );
        }
        if !fact.clean || fact.findings != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV3::ReportNotClean {
                pass: *evidence_expected,
            });
        }
        if fact.compiler_authority || fact.artifact_authority {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV3::ReportAuthorityClaim {
                    pass: *evidence_expected,
                },
            );
        }
    }
    Ok(())
}

fn validate_ranked_ir(bytes: &[u8]) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV3> {
    if bytes.is_empty() {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::EmptyRankedIr);
    }
    if bytes.len() > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V3 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::RankedIrTooLarge {
            actual: bytes.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V3,
        });
    }
    std::str::from_utf8(bytes)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV3::InvalidRankedIrUtf8)?;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' && !(b' '..=b'~').contains(&byte) {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV3::NonCanonicalRankedIrByte { offset },
            );
        }
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::RankedIrMissingFinalNewline);
    }
    Ok(())
}

struct EncodedRecordV3 {
    canonical_bytes: Box<[u8]>,
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV3,
}

fn encode_record(
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir: &str,
) -> Result<EncodedRecordV3, ProductionMiddleEndEvidenceCodecErrorV3> {
    validate_ranked_ir(ranked_ir.as_bytes())?;
    if source_semantic_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroSemanticIdentity);
    }
    if ranked_kernel_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::ZeroRankedKernelIdentity);
    }
    let total_len = FIXED_RECORD_BYTES_V3.checked_add(ranked_ir.len()).ok_or(
        ProductionMiddleEndEvidenceCodecErrorV3::TooLarge {
            actual: usize::MAX,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3,
        },
    )?;
    if total_len > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::TooLarge {
            actual: total_len,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V3,
        });
    }

    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(total_len)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV3::AllocationFailed)?;
    canonical.extend_from_slice(&MAGIC_V3);
    canonical.extend_from_slice(&VERSION_V3.to_le_bytes());
    canonical.extend_from_slice(&FLAGS_V3.to_le_bytes());
    canonical.extend_from_slice(&(total_len as u64).to_le_bytes());
    canonical.extend_from_slice(&0_u32.to_le_bytes());
    canonical
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V3.len() as u16).to_le_bytes());
    canonical.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V3);
    canonical
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3.len() as u16).to_le_bytes());
    canonical.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V3);
    canonical.push(ASSURANCE_INTERNAL_CHECKS_ONLY_V3);
    canonical.push(EQUIVALENCE_REVALIDATED_V3);
    canonical.extend_from_slice(&0_u16.to_le_bytes());
    canonical.extend_from_slice(&source_semantic_identity);
    canonical.extend_from_slice(&ranked_kernel_identity);
    canonical.extend_from_slice(&(ranked_ir.len() as u32).to_le_bytes());
    let ranked_ir_start = canonical.len();
    canonical.extend_from_slice(ranked_ir.as_bytes());
    let ranked_ir_end = canonical.len();
    canonical.push(PASS_COUNT_V3 as u8);
    for pass in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V3 {
        canonical.push(pass.tag());
        canonical.push(CLEAN_STATUS_V3);
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        canonical.push(0);
        canonical.push(0);
        canonical.extend_from_slice(&0_u16.to_le_bytes());
    }
    let evidence_sha256 = derive_evidence_identity(&canonical)
        .ok_or(ProductionMiddleEndEvidenceCodecErrorV3::ZeroIdentity)?;
    canonical.extend_from_slice(&evidence_sha256);
    if canonical.len() != total_len {
        return Err(ProductionMiddleEndEvidenceCodecErrorV3::NonCanonical);
    }
    Ok(EncodedRecordV3 {
        canonical_bytes: canonical.into_boxed_slice(),
        ranked_ir_range: ranked_ir_start..ranked_ir_end,
        identity: ProductionMiddleEndEvidenceIdentityV3 {
            sha256: evidence_sha256,
            byte_len: total_len as u64,
        },
    })
}

fn derive_evidence_identity(preimage: &[u8]) -> Option<[u8; SHA256_BYTES]> {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V3);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    let identity: [u8; SHA256_BYTES] = digest.finalize().into();
    (identity != [0; SHA256_BYTES]).then_some(identity)
}

fn derive_ranked_kernel_identity(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> [u8; SHA256_BYTES] {
    let kernel = ranked.kernel();
    let mut digest = Sha256::new();
    digest.update(RANKED_KERNEL_IDENTITY_DOMAIN_V3);
    hash_blob(&mut digest, kernel.function_name().as_bytes());
    hash_usize(&mut digest, kernel.argument_count());
    hash_usize(&mut digest, kernel.blocks().len());
    for block in kernel.blocks() {
        hash_usize(&mut digest, block.operations().len());
        for operation in block.operations() {
            hash_ranked_operation(&mut digest, operation);
        }
        hash_ranked_terminator(&mut digest, block.terminator());
    }
    digest.finalize().into()
}

fn hash_ranked_operation(digest: &mut Sha256, operation: &ProductionRankedOperationV1) {
    match operation {
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
        } => {
            digest.update([1]);
            digest.update(result.get().to_le_bytes());
            digest.update(element_width.to_le_bytes());
            digest.update([u8::from(*writable)]);
            hash_u64_slice(digest, shape);
            hash_values(digest, dynamic_extents);
        }
        ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
        } => {
            digest.update([2]);
            digest.update(result.get().to_le_bytes());
            digest.update(element_width.to_le_bytes());
            digest.update([u8::from(*writable)]);
            hash_u64_slice(digest, shape);
            hash_values(digest, dynamic_extents);
            digest.update([memory_space_tag(*memory_space)]);
        }
        ProductionRankedOperationV1::IndexConstant { result, value } => {
            digest.update([3]);
            digest.update(result.get().to_le_bytes());
            digest.update(value.to_le_bytes());
        }
        ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension,
            launch_extent,
        } => {
            digest.update([4]);
            digest.update(result.get().to_le_bytes());
            digest.update(dimension.to_le_bytes());
            digest.update(launch_extent.to_le_bytes());
        }
        ProductionRankedOperationV1::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        } => {
            digest.update([5]);
            digest.update(result.get().to_le_bytes());
            digest.update([index_binary_tag(*kind)]);
            hash_value(digest, *lhs);
            hash_value(digest, *rhs);
        }
        ProductionRankedOperationV1::Dimension {
            result,
            view,
            dimension,
        } => {
            digest.update([6]);
            digest.update(result.get().to_le_bytes());
            hash_value(digest, *view);
            digest.update(dimension.to_le_bytes());
        }
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } => {
            digest.update([7]);
            digest.update([access_kind_tag(*kind)]);
            hash_value(digest, *view);
            hash_values(digest, indices);
        }
        ProductionRankedOperationV1::AtomicAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
        } => {
            digest.update([13]);
            digest.update([access_kind_tag(*kind)]);
            digest.update([atomic_ordering_tag(*ordering)]);
            digest.update([atomic_scope_tag(*scope)]);
            hash_value(digest, *view);
            hash_values(digest, indices);
        }
        ProductionRankedOperationV1::Barrier {
            execution_scope,
            memory_scope,
            address_space,
            order,
        } => {
            digest.update([8]);
            digest.update([hierarchy_tag(*execution_scope)]);
            digest.update([memory_scope_tag(*memory_scope)]);
            digest.update([address_space_tag(*address_space)]);
            digest.update([memory_order_tag(*order)]);
        }
        ProductionRankedOperationV1::SemanticSymbol { result, symbol } => {
            digest.update([9]);
            digest.update(result.get().to_le_bytes());
            digest.update(symbol.to_le_bytes());
        }
        ProductionRankedOperationV1::SemanticConstant { result, value } => {
            digest.update([10]);
            digest.update(result.get().to_le_bytes());
            digest.update(value.to_le_bytes());
        }
        ProductionRankedOperationV1::SemanticBinary {
            result,
            kind,
            lhs,
            rhs,
        } => {
            digest.update([11]);
            digest.update(result.get().to_le_bytes());
            digest.update([semantic_binary_tag(*kind)]);
            hash_value(digest, *lhs);
            hash_value(digest, *rhs);
        }
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => {
            digest.update([12]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
        }
    }
}

fn hash_ranked_terminator(digest: &mut Sha256, terminator: &ProductionRankedTerminatorV1) {
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            digest.update([1]);
            hash_value(digest, *lhs);
            hash_value(digest, *rhs);
            digest.update(true_block.to_le_bytes());
            digest.update(false_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => {
            digest.update([4]);
            digest.update(first_block.to_le_bytes());
            digest.update(second_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::Branch { target } => {
            digest.update([2]);
            digest.update(target.to_le_bytes());
        }
        ProductionRankedTerminatorV1::Return => digest.update([3]),
    }
}

fn hash_value(digest: &mut Sha256, value: ProductionRankedValueV1) {
    match value {
        ProductionRankedValueV1::Argument(index) => {
            digest.update([1]);
            digest.update(index.to_le_bytes());
        }
        ProductionRankedValueV1::Local(identity) => {
            digest.update([2]);
            digest.update(identity.get().to_le_bytes());
        }
    }
}

fn hash_values(digest: &mut Sha256, values: &[ProductionRankedValueV1]) {
    hash_usize(digest, values.len());
    for value in values {
        hash_value(digest, *value);
    }
}

fn hash_u64_slice(digest: &mut Sha256, values: &[u64]) {
    hash_usize(digest, values.len());
    for value in values {
        digest.update(value.to_le_bytes());
    }
}

fn hash_blob(digest: &mut Sha256, bytes: &[u8]) {
    hash_usize(digest, bytes.len());
    digest.update(bytes);
}

fn hash_usize(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_le_bytes());
}

const fn access_kind_tag(value: AccessKindAttr) -> u8 {
    match value {
        AccessKindAttr::Read => 1,
        AccessKindAttr::Write => 2,
        AccessKindAttr::AtomicRead => 3,
        AccessKindAttr::AtomicWrite => 4,
        AccessKindAttr::AtomicReadModifyWrite => 5,
    }
}

const fn memory_space_tag(value: MemorySpaceAttr) -> u8 {
    match value {
        MemorySpaceAttr::Private => 1,
        MemorySpaceAttr::Workgroup => 2,
        MemorySpaceAttr::Global => 3,
    }
}

const fn atomic_ordering_tag(value: AtomicOrderingAttr) -> u8 {
    match value {
        AtomicOrderingAttr::Relaxed => 1,
        AtomicOrderingAttr::Acquire => 2,
        AtomicOrderingAttr::Release => 3,
        AtomicOrderingAttr::AcquireRelease => 4,
        AtomicOrderingAttr::SequentiallyConsistent => 5,
    }
}

const fn atomic_scope_tag(value: AtomicScopeAttr) -> u8 {
    match value {
        AtomicScopeAttr::SingleThread => 1,
        AtomicScopeAttr::Workgroup => 2,
        AtomicScopeAttr::Agent => 3,
        AtomicScopeAttr::Device => 4,
        AtomicScopeAttr::System => 5,
    }
}

const fn index_binary_tag(value: IndexBinaryKindAttr) -> u8 {
    match value {
        IndexBinaryKindAttr::Add => 1,
        IndexBinaryKindAttr::Multiply => 2,
        IndexBinaryKindAttr::Remainder => 3,
    }
}

const fn hierarchy_tag(value: HierarchyAttr) -> u8 {
    match value {
        HierarchyAttr::Grid => 1,
        HierarchyAttr::Workgroup => 2,
        HierarchyAttr::Subgroup => 3,
        HierarchyAttr::Lane => 4,
    }
}

const fn memory_scope_tag(value: MemoryScopeAttr) -> u8 {
    match value {
        MemoryScopeAttr::Subgroup => 1,
        MemoryScopeAttr::Workgroup => 2,
        MemoryScopeAttr::Device => 3,
        MemoryScopeAttr::System => 4,
    }
}

const fn address_space_tag(value: AddressSpaceAttr) -> u8 {
    match value {
        AddressSpaceAttr::Private => 1,
        AddressSpaceAttr::Workgroup => 2,
        AddressSpaceAttr::Global => 3,
        AddressSpaceAttr::Constant => 4,
    }
}

const fn memory_order_tag(value: MemoryOrderAttr) -> u8 {
    match value {
        MemoryOrderAttr::Acquire => 1,
        MemoryOrderAttr::Release => 2,
        MemoryOrderAttr::AcquireRelease => 3,
        MemoryOrderAttr::SequentiallyConsistent => 4,
    }
}

const fn semantic_binary_tag(value: SemanticBinaryKindAttr) -> u8 {
    match value {
        SemanticBinaryKindAttr::Add => 1,
        SemanticBinaryKindAttr::Multiply => 2,
    }
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

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionMiddleEndEvidenceCodecErrorV3> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV3::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV3::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionMiddleEndEvidenceCodecErrorV3> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV3::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProductionMiddleEndEvidenceCodecErrorV3> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionMiddleEndEvidenceCodecErrorV3> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionMiddleEndEvidenceCodecErrorV3> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionMiddleEndEvidenceCodecErrorV3> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_facts() -> [ObservedPassFactV3; PASS_COUNT_V3] {
        GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1.map(|pass| ObservedPassFactV3 {
            pass,
            clean: true,
            findings: 0,
            compiler_authority: false,
            artifact_authority: false,
        })
    }

    #[test]
    fn every_live_report_fact_is_fail_closed() {
        for index in 0..PASS_COUNT_V3 {
            let expected = PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V3[index];

            let mut facts = clean_facts();
            facts[index].clean = false;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(ProductionMiddleEndEvidenceCodecErrorV3::ReportNotClean { pass: expected })
            );

            let mut facts = clean_facts();
            facts[index].findings = 1;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(ProductionMiddleEndEvidenceCodecErrorV3::ReportNotClean { pass: expected })
            );

            let mut facts = clean_facts();
            facts[index].compiler_authority = true;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(
                    ProductionMiddleEndEvidenceCodecErrorV3::ReportAuthorityClaim {
                        pass: expected,
                    }
                )
            );

            let mut facts = clean_facts();
            facts[index].artifact_authority = true;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(
                    ProductionMiddleEndEvidenceCodecErrorV3::ReportAuthorityClaim {
                        pass: expected,
                    }
                )
            );
        }
    }

    #[test]
    fn live_report_order_is_exact() {
        let mut facts = clean_facts();
        facts.swap(0, 1);
        assert_eq!(
            validate_observed_pass_facts(&facts),
            Err(
                ProductionMiddleEndEvidenceCodecErrorV3::ReportPassOrderMismatch {
                    index: 0,
                    expected: ProductionMiddleEndEvidencePassV3::MemoryBounds,
                }
            )
        );
    }

    #[test]
    fn ranked_ir_lexical_form_is_canonical() {
        assert!(validate_ranked_ir(b"func @kernel {\n}\n").is_ok());
        assert_eq!(
            validate_ranked_ir(b""),
            Err(ProductionMiddleEndEvidenceCodecErrorV3::EmptyRankedIr)
        );
        assert_eq!(
            validate_ranked_ir(b"func @kernel {}"),
            Err(ProductionMiddleEndEvidenceCodecErrorV3::RankedIrMissingFinalNewline)
        );
        assert_eq!(
            validate_ranked_ir(b"func @kernel {\r\n}\n"),
            Err(ProductionMiddleEndEvidenceCodecErrorV3::NonCanonicalRankedIrByte { offset: 14 })
        );
        assert_eq!(
            validate_ranked_ir(b"func\0\n"),
            Err(ProductionMiddleEndEvidenceCodecErrorV3::NonCanonicalRankedIrByte { offset: 4 })
        );
    }

    #[test]
    fn ranked_atomic_identity_binds_kind_ordering_scope_view_and_indices() {
        fn identity(operation: &ProductionRankedOperationV1) -> [u8; SHA256_BYTES] {
            let mut digest = Sha256::new();
            hash_ranked_operation(&mut digest, operation);
            digest.finalize().into()
        }

        fn atomic(
            kind: AccessKindAttr,
            ordering: AtomicOrderingAttr,
            scope: AtomicScopeAttr,
            view: u32,
            index: u32,
        ) -> ProductionRankedOperationV1 {
            ProductionRankedOperationV1::AtomicAccess {
                kind,
                ordering,
                scope,
                view: ProductionRankedValueV1::Argument(view),
                indices: vec![ProductionRankedValueV1::Argument(index)],
            }
        }

        let base = atomic(
            AccessKindAttr::AtomicRead,
            AtomicOrderingAttr::Acquire,
            AtomicScopeAttr::Device,
            0,
            1,
        );
        let base_identity = identity(&base);
        for changed in [
            atomic(
                AccessKindAttr::AtomicReadModifyWrite,
                AtomicOrderingAttr::Acquire,
                AtomicScopeAttr::Device,
                0,
                1,
            ),
            atomic(
                AccessKindAttr::AtomicRead,
                AtomicOrderingAttr::SequentiallyConsistent,
                AtomicScopeAttr::Device,
                0,
                1,
            ),
            atomic(
                AccessKindAttr::AtomicRead,
                AtomicOrderingAttr::Acquire,
                AtomicScopeAttr::System,
                0,
                1,
            ),
            atomic(
                AccessKindAttr::AtomicRead,
                AtomicOrderingAttr::Acquire,
                AtomicScopeAttr::Device,
                2,
                1,
            ),
            atomic(
                AccessKindAttr::AtomicRead,
                AtomicOrderingAttr::Acquire,
                AtomicScopeAttr::Device,
                0,
                3,
            ),
        ] {
            assert_ne!(identity(&changed), base_identity);
        }
    }
}
