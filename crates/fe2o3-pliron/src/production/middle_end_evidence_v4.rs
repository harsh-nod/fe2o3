//! Strict canonical evidence for the production semantic middle end.
//!
//! The live-produced value binds an exact semantic-MIR owner, the checked
//! ranked kernel, deterministic ranked IR, and the seven fixed clean analysis
//! reports represented by this historical schema. Live construction also
//! requires the hierarchy-ownership pass in the V2 pipeline, but V4 bytes do
//! not encode that eighth report. The decoded form is deliberately named `Inert`: byte integrity is
//! not producer authentication, a refinement proof, or execution authority.

use std::{error::Error, fmt, ops::Range};

use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, IndexBinaryKindAttr, MemorySpaceAttr,
    SemanticBinaryKindAttr,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1,
    PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
};
use fe2o3_kernel_ir::{
    MatrixElement, TensorElementPackingV1, TensorFragmentLayoutV1, TensorInstructionProfileV1,
    TensorLayoutContractV1, TensorLdsSwizzleV1, TensorMultiplicityV1, TensorOperandRoleV1,
    TensorSymbolicMapV1, TensorTailMaskV1,
};
use sha2::{Digest, Sha256};

// V4 keeps every pre-existing operation tag stable. New operation variants must
// consume an unused tag so two distinct recipes can never share an identity
// prefix within the V4 domain.
const RANKED_EXECUTION_LAYOUT_TAG_V4: u8 = 15;
const RANKED_TENSOR_LAYOUT_TAG_V4: u8 = 19;
const RANKED_OWNERSHIP_CONTRACT_TAG_V4: u8 = 24;

use super::{
    ProductionRankedKernelErrorV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionRankedValueV1,
    ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1,
};

const MAGIC_V4: [u8; 8] = *b"F2MEV4\0\0";
const VERSION_V4: u16 = 4;
const FLAGS_V4: u16 = 0;
const ASSURANCE_INTERNAL_CHECKS_ONLY_V4: u8 = 1;
const EQUIVALENCE_REVALIDATED_V4: u8 = 1;
const CLEAN_STATUS_V4: u8 = 1;
const PASS_COUNT_V4: usize = 7;
const PASS_RECORD_BYTES_V4: usize = 10;
const SHA256_BYTES: usize = 32;
const IDENTITY_DOMAIN_V4: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V4\0";
const RANKED_KERNEL_IDENTITY_DOMAIN_V4: &[u8] = b"FE2O3/PRODUCTION-RANKED-KERNEL-IDENTITY/V4\0";
const FUNCTIONAL_REFINEMENT_GRAPH_IDENTITY_DOMAIN_V2: &[u8] =
    b"FE2O3/PRODUCTION-FUNCTIONAL-REFINEMENT-GRAPH/V2\0";

/// Stable wire domain for a production middle-end evidence record.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4: &[u8] =
    b"fe2o3.production-middle-end-evidence.v4";

/// Fixed assurance policy. It intentionally does not claim Verus verification.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4: &[u8] = b"fe2o3.internal-checks-only.v4";

/// Maximum complete record size, chosen to fit one V4 lineage receipt.
pub const MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4: usize = 4 * 1024 * 1024;

const FIXED_RECORD_BYTES_V4: usize = MAGIC_V4.len()
    + 2
    + 2
    + 8
    + 4
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4.len()
    + 2
    + PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4.len()
    + 1
    + 1
    + 2
    + SHA256_BYTES
    + SHA256_BYTES
    + 4
    + 1
    + PASS_COUNT_V4 * PASS_RECORD_BYTES_V4
    + SHA256_BYTES;

/// Maximum deterministic ranked-IR bytes retained by one complete record.
pub const MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4: usize =
    MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 - FIXED_RECORD_BYTES_V4;

/// Assurance represented by the V4 middle-end record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionMiddleEndAssuranceV4 {
    /// fe2o3's seven internal analyses completed cleanly; no Verus proof is claimed.
    InternalChecksOnly,
}

/// One pass in the fixed V4 middle-end order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionMiddleEndEvidencePassV4 {
    TensorLayout,
    MemoryBounds,
    AtomicLegality,
    RaceFreedom,
    BarrierConvergence,
    WorkgroupMemory,
    SemanticRefinement,
}

impl ProductionMiddleEndEvidencePassV4 {
    const fn tag(self) -> u8 {
        match self {
            Self::TensorLayout => 1,
            Self::MemoryBounds => 2,
            Self::AtomicLegality => 3,
            Self::RaceFreedom => 4,
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
            KernelCheckPassKindV1::BarrierConvergence => Some(Self::BarrierConvergence),
            KernelCheckPassKindV1::WorkgroupMemory => Some(Self::WorkgroupMemory),
            KernelCheckPassKindV1::SemanticRefinement => Some(Self::SemanticRefinement),
            KernelCheckPassKindV1::Structural
            | KernelCheckPassKindV1::ControlFlow
            | KernelCheckPassKindV1::HierarchicalOwnership => None,
        }
    }
}

/// Fixed pass order encoded by every V4 record.
pub const PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4: [ProductionMiddleEndEvidencePassV4;
    PASS_COUNT_V4] = [
    ProductionMiddleEndEvidencePassV4::TensorLayout,
    ProductionMiddleEndEvidencePassV4::MemoryBounds,
    ProductionMiddleEndEvidencePassV4::AtomicLegality,
    ProductionMiddleEndEvidencePassV4::RaceFreedom,
    ProductionMiddleEndEvidencePassV4::BarrierConvergence,
    ProductionMiddleEndEvidencePassV4::WorkgroupMemory,
    ProductionMiddleEndEvidencePassV4::SemanticRefinement,
];

/// Exact success facts encoded for one mandatory pass.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndPassSuccessV4 {
    pass: ProductionMiddleEndEvidencePassV4,
}

impl ProductionMiddleEndPassSuccessV4 {
    const fn new(pass: ProductionMiddleEndEvidencePassV4) -> Self {
        Self { pass }
    }

    pub const fn pass(self) -> ProductionMiddleEndEvidencePassV4 {
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

const PASS_SUCCESSES_V4: [ProductionMiddleEndPassSuccessV4; PASS_COUNT_V4] = [
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::TensorLayout),
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::MemoryBounds),
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::AtomicLegality),
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::RaceFreedom),
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::BarrierConvergence),
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::WorkgroupMemory),
    ProductionMiddleEndPassSuccessV4::new(ProductionMiddleEndEvidencePassV4::SemanticRefinement),
];

/// Domain-separated identity of one exact canonical V4 record.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionMiddleEndEvidenceIdentityV4 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl ProductionMiddleEndEvidenceIdentityV4 {
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

impl fmt::Debug for ProductionMiddleEndEvidenceIdentityV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionMiddleEndEvidenceIdentityV4")
            .field("sha256", &self.sha256)
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

/// Strictly decoded, internally consistent V4 bytes.
///
/// This type is inert: decoding does not establish a live producer, replay
/// freshness, derivation correctness, Verus proof, or any execution authority.
#[derive(Eq, PartialEq)]
pub struct InertProductionMiddleEndEvidenceV4 {
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV4,
    canonical_bytes: Box<[u8]>,
}

impl InertProductionMiddleEndEvidenceV4 {
    /// Strictly decodes one complete canonical V4 record with no fallback.
    /// All declared and aggregate bounds are checked before allocation.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV4> {
        if bytes.len() > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::TooLarge {
                actual: bytes.len(),
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
            });
        }

        let mut reader = ReaderV4::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != VERSION_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::UnsupportedVersion(
                version,
            ));
        }
        let flags = reader.u16()?;
        if flags != FLAGS_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::UnsupportedFlags(
                flags,
            ));
        }
        let declared_len = reader.u64()?;
        let declared_len_usize = usize::try_from(declared_len)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV4::InvalidLength(declared_len))?;
        if declared_len_usize > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::TooLarge {
                actual: declared_len_usize,
                limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
            });
        }
        if declared_len_usize > bytes.len() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::Truncated);
        }
        if declared_len_usize < bytes.len() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::TrailingBytes);
        }
        if declared_len_usize < FIXED_RECORD_BYTES_V4 + 1 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidLength(
                declared_len,
            ));
        }
        if reader.u32()? != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved);
        }

        let domain_len = usize::from(reader.u16()?);
        if domain_len != PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4.len()
            || reader.take(domain_len)? != PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidDomain);
        }
        let policy_len = usize::from(reader.u16()?);
        if policy_len != PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4.len()
            || reader.take(policy_len)? != PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4
        {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPolicy);
        }
        let assurance = reader.u8()?;
        if assurance != ASSURANCE_INTERNAL_CHECKS_ONLY_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidAssurance(
                assurance,
            ));
        }
        if reader.u8()? != EQUIVALENCE_REVALIDATED_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::SemanticEquivalenceNotEstablished);
        }
        if reader.u16()? != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved);
        }

        let source_semantic_identity = reader.fixed::<SHA256_BYTES>()?;
        if source_semantic_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroSemanticIdentity);
        }
        let ranked_kernel_identity = reader.fixed::<SHA256_BYTES>()?;
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroRankedKernelIdentity);
        }

        let ranked_ir_len = usize::try_from(reader.u32()?).map_err(|_| {
            ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
                actual: usize::MAX,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
            }
        })?;
        if ranked_ir_len > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
                actual: ranked_ir_len,
                limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
            });
        }
        let ranked_ir_bytes = reader.take(ranked_ir_len)?;
        validate_ranked_ir(ranked_ir_bytes)?;

        let pass_count = reader.u8()?;
        if usize::from(pass_count) != PASS_COUNT_V4 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassCount(
                pass_count,
            ));
        }
        for (index, expected) in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4
            .iter()
            .copied()
            .enumerate()
        {
            let actual = reader.u8()?;
            if actual != expected.tag() {
                return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassOrder {
                    index,
                    expected,
                    actual,
                });
            }
            let status = reader.u8()?;
            if status != CLEAN_STATUS_V4 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassStatus {
                    pass: expected,
                    actual: status,
                });
            }
            let findings = reader.u32()?;
            if findings != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroFindings {
                    pass: expected,
                    actual: findings,
                });
            }
            let compiler_authority = reader.u8()?;
            let artifact_authority = reader.u8()?;
            if compiler_authority != 0 || artifact_authority != 0 {
                return Err(
                    ProductionMiddleEndEvidenceCodecErrorV4::AuthorityClaimInEncoding {
                        pass: expected,
                    },
                );
            }
            if reader.u16()? != 0 {
                return Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved);
            }
        }

        let terminal_offset = reader.offset();
        let declared_identity = reader.fixed::<SHA256_BYTES>()?;
        if declared_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroIdentity);
        }
        if !reader.is_empty() {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::TrailingBytes);
        }
        if derive_evidence_identity(&bytes[..terminal_offset]) != Some(declared_identity) {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::IdentityMismatch);
        }

        let ranked_ir = std::str::from_utf8(ranked_ir_bytes)
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV4::InvalidRankedIrUtf8)?;
        let reconstructed =
            encode_record(source_semantic_identity, ranked_kernel_identity, ranked_ir)?;
        if reconstructed.canonical_bytes.as_ref() != bytes {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::NonCanonical);
        }
        Ok(Self {
            source_semantic_identity,
            ranked_kernel_identity,
            ranked_ir_range: reconstructed.ranked_ir_range,
            identity: reconstructed.identity,
            canonical_bytes: reconstructed.canonical_bytes,
        })
    }

    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV4 {
        ProductionMiddleEndAssuranceV4::InternalChecksOnly
    }

    pub const fn policy(&self) -> &'static [u8] {
        PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4
    }

    pub const fn source_semantic_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.source_semantic_identity
    }

    pub const fn ranked_kernel_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.ranked_kernel_identity
    }

    pub fn ranked_ir(&self) -> &str {
        std::str::from_utf8(&self.canonical_bytes[self.ranked_ir_range.clone()])
            .expect("validated V4 ranked IR remains UTF-8")
    }

    pub const fn pass_successes(
        &self,
    ) -> &'static [ProductionMiddleEndPassSuccessV4; PASS_COUNT_V4] {
        &PASS_SUCCESSES_V4
    }

    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV4 {
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

impl fmt::Debug for InertProductionMiddleEndEvidenceV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionMiddleEndEvidenceV4")
            .field("source_semantic_identity", &self.source_semantic_identity)
            .field("ranked_kernel_identity", &self.ranked_kernel_identity)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Move-only V4 middle-end evidence constructed from the two live owners.
///
/// The value carries no authority. Its distinct type records that construction
/// revalidated the live semantic owner, ranked structure, and all seven reports.
/// Strict decoding returns [`InertProductionMiddleEndEvidenceV4`] instead.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionMiddleEndEvidenceV4;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionMiddleEndEvidenceV4>();
/// ```
#[must_use = "dropping middle-end evidence abandons the live-produced stage record"]
pub struct ProductionMiddleEndEvidenceV4 {
    inert: InertProductionMiddleEndEvidenceV4,
}

impl ProductionMiddleEndEvidenceV4 {
    /// Revalidates both live owners and constructs their exact canonical record.
    pub fn try_new(
        semantic: &ProductionSemanticMirOwnerV1,
        ranked: &ProductionRankedKernelLoweringInputV1,
        deterministic_ranked_ir: &str,
    ) -> Result<Self, ProductionMiddleEndEvidenceCodecErrorV4> {
        validate_ranked_ir(deterministic_ranked_ir.as_bytes())?;
        let source_semantic_identity = revalidated_source_semantic_identity(semantic)?;

        ranked
            .revalidate_structure()
            .map_err(ProductionMiddleEndEvidenceCodecErrorV4::RankedKernel)?;
        validate_live_reports(ranked)?;
        if ranked.kernel().blocks().iter().any(|block| {
            block.operations().iter().any(|operation| {
                matches!(
                    operation,
                    ProductionRankedOperationV1::CollectiveSemantics { .. }
                )
            })
        }) {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV4::CollectiveSemanticsRequireNewEvidenceVersion,
            );
        }

        let ranked_kernel_identity = derive_ranked_kernel_identity(ranked);
        if ranked_kernel_identity == [0; SHA256_BYTES] {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroRankedKernelIdentity);
        }
        let encoded = encode_record(
            source_semantic_identity,
            ranked_kernel_identity,
            deterministic_ranked_ir,
        )?;
        Ok(Self {
            inert: InertProductionMiddleEndEvidenceV4 {
                source_semantic_identity,
                ranked_kernel_identity,
                ranked_ir_range: encoded.ranked_ir_range,
                identity: encoded.identity,
                canonical_bytes: encoded.canonical_bytes,
            },
        })
    }

    pub const fn as_inert(&self) -> &InertProductionMiddleEndEvidenceV4 {
        &self.inert
    }

    pub fn into_inert(self) -> InertProductionMiddleEndEvidenceV4 {
        self.inert
    }

    pub const fn assurance(&self) -> ProductionMiddleEndAssuranceV4 {
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
    ) -> &'static [ProductionMiddleEndPassSuccessV4; PASS_COUNT_V4] {
        self.inert.pass_successes()
    }

    pub const fn identity(&self) -> ProductionMiddleEndEvidenceIdentityV4 {
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

impl fmt::Debug for ProductionMiddleEndEvidenceV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionMiddleEndEvidenceV4")
            .field("identity", &self.identity())
            .field("assurance", &self.assurance())
            .finish_non_exhaustive()
    }
}

/// Fail-closed construction and decoding errors for the V4 codec.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionMiddleEndEvidenceCodecErrorV4 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    RankedKernel(ProductionRankedKernelErrorV1),
    SemanticSourceIdentityMismatch,
    CollectiveSemanticsRequireNewEvidenceVersion,
    ReportPassOrderMismatch {
        index: usize,
        expected: ProductionMiddleEndEvidencePassV4,
    },
    ReportNotClean {
        pass: ProductionMiddleEndEvidencePassV4,
    },
    ReportAuthorityClaim {
        pass: ProductionMiddleEndEvidencePassV4,
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
        expected: ProductionMiddleEndEvidencePassV4,
        actual: u8,
    },
    InvalidPassStatus {
        pass: ProductionMiddleEndEvidencePassV4,
        actual: u8,
    },
    NonzeroFindings {
        pass: ProductionMiddleEndEvidencePassV4,
        actual: u32,
    },
    AuthorityClaimInEncoding {
        pass: ProductionMiddleEndEvidencePassV4,
    },
    ZeroIdentity,
    IdentityMismatch,
    NonCanonical,
    AllocationFailed,
}

impl fmt::Display for ProductionMiddleEndEvidenceCodecErrorV4 {
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
            Self::CollectiveSemanticsRequireNewEvidenceVersion => formatter.write_str(
                "V4 cannot serialize collective semantics or hierarchical coverage; a new evidence version is required",
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

impl Error for ProductionMiddleEndEvidenceCodecErrorV4 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticOwner(error) => Some(error),
            Self::RankedKernel(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
struct ObservedPassFactV4 {
    pass: KernelCheckPassKindV1,
    clean: bool,
    findings: usize,
    compiler_authority: bool,
    artifact_authority: bool,
}

fn validate_live_reports(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV4> {
    if ranked.production_pipeline_report().pass_order()
        != &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    {
        return Err(
            ProductionMiddleEndEvidenceCodecErrorV4::ReportPassOrderMismatch {
                index: 0,
                expected: ProductionMiddleEndEvidencePassV4::TensorLayout,
            },
        );
    }
    let facts = [
        ObservedPassFactV4 {
            pass: ranked.tensor_layout_report().pass(),
            clean: ranked.tensor_layout_report().is_clean(),
            findings: ranked.tensor_layout_report().findings().len(),
            compiler_authority: ranked
                .tensor_layout_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked
                .tensor_layout_report()
                .grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV4 {
            pass: ranked.bounds_report().pass(),
            clean: ranked.bounds_report().is_clean(),
            findings: ranked.bounds_report().findings().len(),
            compiler_authority: ranked
                .bounds_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked.bounds_report().grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV4 {
            pass: ranked.atomic_report().pass(),
            clean: ranked.atomic_report().is_clean(),
            findings: ranked.atomic_report().findings().len(),
            compiler_authority: ranked
                .atomic_report()
                .grants_compiler_refinement_authority(),
            artifact_authority: ranked.atomic_report().grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV4 {
            pass: ranked.race_report().pass(),
            clean: ranked.race_report().is_clean(),
            findings: ranked.race_report().findings().len(),
            compiler_authority: ranked.race_report().grants_compiler_refinement_authority(),
            artifact_authority: ranked.race_report().grants_artifact_or_launch_authority(),
        },
        ObservedPassFactV4 {
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
        ObservedPassFactV4 {
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
        ObservedPassFactV4 {
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
    facts: &[ObservedPassFactV4; PASS_COUNT_V4],
) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV4> {
    for (index, ((fact, analysis_expected), evidence_expected)) in facts
        .iter()
        .zip(PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1.iter())
        .zip(PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4.iter())
        .enumerate()
    {
        if fact.pass != *analysis_expected
            || ProductionMiddleEndEvidencePassV4::from_analysis(fact.pass)
                != Some(*evidence_expected)
        {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV4::ReportPassOrderMismatch {
                    index,
                    expected: *evidence_expected,
                },
            );
        }
        if !fact.clean || fact.findings != 0 {
            return Err(ProductionMiddleEndEvidenceCodecErrorV4::ReportNotClean {
                pass: *evidence_expected,
            });
        }
        if fact.compiler_authority || fact.artifact_authority {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV4::ReportAuthorityClaim {
                    pass: *evidence_expected,
                },
            );
        }
    }
    Ok(())
}

fn validate_ranked_ir(bytes: &[u8]) -> Result<(), ProductionMiddleEndEvidenceCodecErrorV4> {
    if bytes.is_empty() {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::EmptyRankedIr);
    }
    if bytes.len() > MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
            actual: bytes.len(),
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
        });
    }
    std::str::from_utf8(bytes)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV4::InvalidRankedIrUtf8)?;
    for (offset, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' && !(b' '..=b'~').contains(&byte) {
            return Err(
                ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset },
            );
        }
    }
    if bytes.last() != Some(&b'\n') {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrMissingFinalNewline);
    }
    Ok(())
}

struct EncodedRecordV4 {
    canonical_bytes: Box<[u8]>,
    ranked_ir_range: Range<usize>,
    identity: ProductionMiddleEndEvidenceIdentityV4,
}

fn encode_record(
    source_semantic_identity: [u8; SHA256_BYTES],
    ranked_kernel_identity: [u8; SHA256_BYTES],
    ranked_ir: &str,
) -> Result<EncodedRecordV4, ProductionMiddleEndEvidenceCodecErrorV4> {
    validate_ranked_ir(ranked_ir.as_bytes())?;
    if source_semantic_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroSemanticIdentity);
    }
    if ranked_kernel_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroRankedKernelIdentity);
    }
    let total_len = FIXED_RECORD_BYTES_V4.checked_add(ranked_ir.len()).ok_or(
        ProductionMiddleEndEvidenceCodecErrorV4::TooLarge {
            actual: usize::MAX,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
        },
    )?;
    if total_len > MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::TooLarge {
            actual: total_len,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
        });
    }

    let mut canonical = Vec::new();
    canonical
        .try_reserve_exact(total_len)
        .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV4::AllocationFailed)?;
    canonical.extend_from_slice(&MAGIC_V4);
    canonical.extend_from_slice(&VERSION_V4.to_le_bytes());
    canonical.extend_from_slice(&FLAGS_V4.to_le_bytes());
    canonical.extend_from_slice(&(total_len as u64).to_le_bytes());
    canonical.extend_from_slice(&0_u32.to_le_bytes());
    canonical
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4.len() as u16).to_le_bytes());
    canonical.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4);
    canonical
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4.len() as u16).to_le_bytes());
    canonical.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4);
    canonical.push(ASSURANCE_INTERNAL_CHECKS_ONLY_V4);
    canonical.push(EQUIVALENCE_REVALIDATED_V4);
    canonical.extend_from_slice(&0_u16.to_le_bytes());
    canonical.extend_from_slice(&source_semantic_identity);
    canonical.extend_from_slice(&ranked_kernel_identity);
    canonical.extend_from_slice(&(ranked_ir.len() as u32).to_le_bytes());
    let ranked_ir_start = canonical.len();
    canonical.extend_from_slice(ranked_ir.as_bytes());
    let ranked_ir_end = canonical.len();
    canonical.push(PASS_COUNT_V4 as u8);
    for pass in PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4 {
        canonical.push(pass.tag());
        canonical.push(CLEAN_STATUS_V4);
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        canonical.push(0);
        canonical.push(0);
        canonical.extend_from_slice(&0_u16.to_le_bytes());
    }
    let evidence_sha256 = derive_evidence_identity(&canonical)
        .ok_or(ProductionMiddleEndEvidenceCodecErrorV4::ZeroIdentity)?;
    canonical.extend_from_slice(&evidence_sha256);
    if canonical.len() != total_len {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::NonCanonical);
    }
    Ok(EncodedRecordV4 {
        canonical_bytes: canonical.into_boxed_slice(),
        ranked_ir_range: ranked_ir_start..ranked_ir_end,
        identity: ProductionMiddleEndEvidenceIdentityV4 {
            sha256: evidence_sha256,
            byte_len: total_len as u64,
        },
    })
}

fn derive_evidence_identity(preimage: &[u8]) -> Option<[u8; SHA256_BYTES]> {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V4);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    let identity: [u8; SHA256_BYTES] = digest.finalize().into();
    (identity != [0; SHA256_BYTES]).then_some(identity)
}

pub(super) fn revalidated_source_semantic_identity(
    semantic: &ProductionSemanticMirOwnerV1,
) -> Result<[u8; SHA256_BYTES], ProductionMiddleEndEvidenceCodecErrorV4> {
    semantic
        .verify_equivalence()
        .map_err(ProductionMiddleEndEvidenceCodecErrorV4::SemanticOwner)?;
    let source_semantic_identity = *semantic.semantic().semantic_sha256().as_bytes();
    if source_semantic_identity == [0; SHA256_BYTES] {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroSemanticIdentity);
    }
    let actual_semantic_identity: [u8; SHA256_BYTES] =
        Sha256::digest(semantic.semantic().canonical_encoding()).into();
    if actual_semantic_identity != source_semantic_identity
        || semantic.locator().semantic_sha256().as_bytes() != &source_semantic_identity
    {
        return Err(ProductionMiddleEndEvidenceCodecErrorV4::SemanticSourceIdentityMismatch);
    }
    Ok(source_semantic_identity)
}

pub(super) fn derive_ranked_kernel_identity(
    ranked: &ProductionRankedKernelLoweringInputV1,
) -> [u8; SHA256_BYTES] {
    let kernel = ranked.kernel();
    let mut digest = Sha256::new();
    digest.update(RANKED_KERNEL_IDENTITY_DOMAIN_V4);
    hash_blob(&mut digest, kernel.function_name().as_bytes());
    hash_usize(&mut digest, kernel.argument_count());
    hash_usize(&mut digest, kernel.blocks().len());
    for block in kernel.blocks() {
        digest.update(block.index_argument_count().to_le_bytes());
        hash_usize(&mut digest, block.operations().len());
        for operation in block.operations() {
            hash_ranked_operation(&mut digest, operation);
        }
        hash_ranked_terminator(&mut digest, block.terminator());
    }
    digest.finalize().into()
}

/// Canonical identity of the complete ranked graph covered by a functional-refinement request.
///
/// The only normalized transition is the compiler-owned replacement of an unbound request with
/// its exactly bound requirement. Proof receipt bytes are deliberately excluded so the graph can
/// be hashed before the receipt exists. Every other operation and every CFG terminator is exact.
pub(super) fn derive_functional_refinement_graph_identity_v2(
    kernel: &super::ProductionRankedKernelV1,
) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(FUNCTIONAL_REFINEMENT_GRAPH_IDENTITY_DOMAIN_V2);
    hash_blob(&mut digest, kernel.function_name().as_bytes());
    hash_usize(&mut digest, kernel.argument_count());
    hash_usize(&mut digest, kernel.blocks().len());
    for block in kernel.blocks() {
        digest.update(block.index_argument_count().to_le_bytes());
        hash_usize(&mut digest, block.operations().len());
        for operation in block.operations() {
            hash_functional_refinement_graph_operation(&mut digest, operation);
        }
        hash_ranked_terminator(&mut digest, block.terminator());
    }
    digest.finalize().into()
}

fn hash_functional_refinement_graph_operation(
    digest: &mut Sha256,
    operation: &ProductionRankedOperationV1,
) {
    digest.update([functional_refinement_graph_operation_tag(operation)]);
    match operation {
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual,
            expected,
            subjects,
        } => {
            digest.update([250]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
            hash_functional_refinement_subjects(digest, *subjects);
        }
        ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent {
            actual,
            expected,
            proof,
        } => {
            digest.update([250]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
            hash_functional_refinement_subjects(digest, proof.binding().subjects());
        }
        ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects } => {
            digest.update([251]);
            hash_effect_refinement_contract(digest, contract);
            hash_functional_refinement_subjects(digest, *subjects);
        }
        ProductionRankedOperationV1::RequireEffectRefinement { contract, proof } => {
            digest.update([251]);
            hash_effect_refinement_contract(digest, contract);
            hash_functional_refinement_subjects(digest, proof.binding().subjects());
        }
        ProductionRankedOperationV1::RequestNumericalRefinement { contract, subjects } => {
            digest.update([252]);
            hash_numerical_refinement_contract(digest, *contract);
            hash_functional_refinement_subjects(digest, *subjects);
        }
        ProductionRankedOperationV1::RequireNumericalRefinement { contract, proof } => {
            digest.update([252]);
            hash_numerical_refinement_contract(digest, *contract);
            hash_functional_refinement_subjects(digest, proof.binding().subjects());
        }
        _ => hash_ranked_operation(digest, operation),
    }
}

fn functional_refinement_graph_operation_tag(operation: &ProductionRankedOperationV1) -> u8 {
    match operation {
        ProductionRankedOperationV1::ExecutionLayout { .. } => 1,
        ProductionRankedOperationV1::View { .. } => 2,
        ProductionRankedOperationV1::ViewInSpace { .. } => 3,
        ProductionRankedOperationV1::IndexConstant { .. } => 4,
        ProductionRankedOperationV1::IndexUnknown { .. } => 5,
        ProductionRankedOperationV1::InvocationIndex { .. } => 6,
        ProductionRankedOperationV1::IndexBinary { .. } => 7,
        ProductionRankedOperationV1::DeterministicJoin { .. } => 8,
        ProductionRankedOperationV1::CheckedTiledIndex2D { .. } => 9,
        ProductionRankedOperationV1::CheckedRowStripedIndex2D { .. } => 10,
        ProductionRankedOperationV1::Dimension { .. } => 11,
        ProductionRankedOperationV1::Access { .. } => 12,
        ProductionRankedOperationV1::ValueAccess { .. } => 26,
        ProductionRankedOperationV1::AtomicAccess { .. } => 13,
        ProductionRankedOperationV1::AtomicValueAccess { .. } => 27,
        ProductionRankedOperationV1::OwnershipContract { .. } => 14,
        ProductionRankedOperationV1::AllocationEffect { .. } => 15,
        ProductionRankedOperationV1::Barrier { .. } => 16,
        ProductionRankedOperationV1::Fence { .. } => 17,
        ProductionRankedOperationV1::TensorLayout { .. } => 18,
        ProductionRankedOperationV1::SemanticSymbol { .. } => 19,
        ProductionRankedOperationV1::SemanticConstant { .. } => 20,
        ProductionRankedOperationV1::SemanticBinary { .. } => 21,
        ProductionRankedOperationV1::SemanticExpression { .. } => 28,
        ProductionRankedOperationV1::CollectiveSemantics { .. } => 29,
        ProductionRankedOperationV1::RequireEquivalent { .. } => 22,
        ProductionRankedOperationV1::RequireReferenceEquivalent { .. } => 23,
        ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent { .. }
        | ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent { .. } => 24,
        ProductionRankedOperationV1::RequireEffectRefinement { .. }
        | ProductionRankedOperationV1::RequestEffectRefinement { .. } => 25,
        ProductionRankedOperationV1::RequireNumericalRefinement { .. }
        | ProductionRankedOperationV1::RequestNumericalRefinement { .. } => 30,
    }
}

fn hash_effect_refinement_contract(
    digest: &mut Sha256,
    contract: &super::ProductionEffectRefinementContractV2,
) {
    digest.update(contract.contract_identity().to_le_bytes());
    digest.update(contract.gpu_write_site().block().to_le_bytes());
    digest.update(contract.gpu_write_site().operation().to_le_bytes());
    digest.update(contract.reference_output_site().argument().to_le_bytes());
    digest.update(contract.reference_output_site().block().to_le_bytes());
    digest.update(contract.reference_output_site().statement().to_le_bytes());
    hash_value(digest, contract.view());
    hash_values(digest, contract.indices());
    hash_values(digest, contract.gpu_coordinates());
    hash_values(digest, contract.reference_coordinates());
    for value in [
        contract.gpu_domain(),
        contract.reference_domain(),
        contract.gpu_precondition(),
        contract.reference_precondition(),
        contract.gpu_value(),
        contract.reference_value(),
    ] {
        hash_value(digest, value);
    }
}

fn hash_numerical_refinement_contract(
    digest: &mut Sha256,
    contract: super::ProductionNumericalRefinementContractV2,
) {
    digest.update(contract.contract_identity().to_le_bytes());
    for value in [
        contract.actual(),
        contract.reference(),
        contract.domain(),
        contract.precondition(),
    ] {
        hash_value(digest, value);
    }
    digest.update(contract.absolute_error_f64_bits().to_le_bytes());
    digest.update(contract.relative_error_f64_bits().to_le_bytes());
}

fn hash_ranked_operation(digest: &mut Sha256, operation: &ProductionRankedOperationV1) {
    match operation {
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups,
        } => {
            digest.update([RANKED_EXECUTION_LAYOUT_TAG_V4]);
            digest.update(grid_identity.to_le_bytes());
            for extent in global_extents {
                digest.update(extent.to_le_bytes());
            }
            for extent in workgroup_extents {
                digest.update(extent.to_le_bytes());
            }
            digest.update(subgroup_size.to_le_bytes());
            digest.update([u8::from(*full_physical_workgroups)]);
        }
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            allocation_origin,
            noalias_class,
        } => {
            digest.update([1]);
            digest.update(result.get().to_le_bytes());
            digest.update(element_width.to_le_bytes());
            digest.update([u8::from(*writable)]);
            hash_u64_slice(digest, shape);
            hash_values(digest, dynamic_extents);
            digest.update(allocation_origin.to_le_bytes());
            digest.update(noalias_class.to_le_bytes());
        }
        ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
            allocation_origin,
            noalias_class,
        } => {
            digest.update([2]);
            digest.update(result.get().to_le_bytes());
            digest.update(element_width.to_le_bytes());
            digest.update([u8::from(*writable)]);
            hash_u64_slice(digest, shape);
            hash_values(digest, dynamic_extents);
            digest.update([memory_space_tag(*memory_space)]);
            digest.update(allocation_origin.to_le_bytes());
            digest.update(noalias_class.to_le_bytes());
        }
        ProductionRankedOperationV1::IndexConstant { result, value } => {
            digest.update([3]);
            digest.update(result.get().to_le_bytes());
            digest.update(value.to_le_bytes());
        }
        ProductionRankedOperationV1::IndexUnknown { result } => {
            digest.update([22]);
            digest.update(result.get().to_le_bytes());
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
        ProductionRankedOperationV1::DeterministicJoin {
            result,
            dependencies,
        } => {
            digest.update([17]);
            digest.update(result.get().to_le_bytes());
            hash_values(digest, dependencies);
        }
        ProductionRankedOperationV1::CheckedTiledIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        } => {
            digest.update([14]);
            digest.update(result.get().to_le_bytes());
            hash_value(digest, *invocation);
            hash_value(digest, *component);
            hash_value(digest, *rows);
            hash_value(digest, *columns);
            hash_value(digest, *row_stride);
            digest.update(lanes_per_tile.to_le_bytes());
            digest.update(tile_rows.to_le_bytes());
            digest.update(tile_columns.to_le_bytes());
            digest.update(elements_per_lane.to_le_bytes());
        }
        ProductionRankedOperationV1::CheckedRowStripedIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            lanes_per_row,
            elements_per_lane,
        } => {
            digest.update([18]);
            digest.update(result.get().to_le_bytes());
            hash_value(digest, *invocation);
            hash_value(digest, *component);
            hash_value(digest, *rows);
            hash_value(digest, *columns);
            hash_value(digest, *row_stride);
            digest.update(lanes_per_row.to_le_bytes());
            digest.update(elements_per_lane.to_le_bytes());
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
        ProductionRankedOperationV1::ValueAccess {
            kind,
            view,
            indices,
            value,
        } => {
            digest.update([28]);
            digest.update([access_kind_tag(*kind)]);
            hash_value(digest, *view);
            hash_values(digest, indices);
            hash_value(digest, *value);
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
        ProductionRankedOperationV1::AtomicValueAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
            value,
        } => {
            digest.update([29]);
            digest.update([access_kind_tag(*kind)]);
            digest.update([atomic_ordering_tag(*ordering)]);
            digest.update([atomic_scope_tag(*scope)]);
            hash_value(digest, *view);
            hash_values(digest, indices);
            hash_value(digest, *value);
        }
        ProductionRankedOperationV1::OwnershipContract {
            view,
            coverage,
            partition,
        } => {
            digest.update([RANKED_OWNERSHIP_CONTRACT_TAG_V4]);
            hash_value(digest, *view);
            digest.update([match coverage {
                dialect_kernel::OwnershipCoverageAttr::ExactView => 1,
                dialect_kernel::OwnershipCoverageAttr::ExactEffectDomain => 2,
                dialect_kernel::OwnershipCoverageAttr::TotalView => 3,
                dialect_kernel::OwnershipCoverageAttr::CollectiveContributions => 4,
            }]);
            digest.update([match partition {
                dialect_kernel::OwnershipPartitionAttr::ExactSets => 1,
                dialect_kernel::OwnershipPartitionAttr::DenseRectangles => 2,
            }]);
        }
        ProductionRankedOperationV1::AllocationEffect {
            kind,
            memory_space,
            allocation_origin,
            noalias_class,
        } => {
            digest.update([18]);
            digest.update([access_kind_tag(*kind)]);
            digest.update([memory_space_tag(*memory_space)]);
            digest.update(allocation_origin.to_le_bytes());
            digest.update(noalias_class.to_le_bytes());
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
        ProductionRankedOperationV1::Fence {
            memory_scope,
            address_space,
            order,
        } => {
            digest.update([16]);
            digest.update([memory_scope_tag(*memory_scope)]);
            digest.update([address_space_tag(*address_space)]);
            digest.update([memory_order_tag(*order)]);
        }
        ProductionRankedOperationV1::TensorLayout {
            contract,
            convergence,
            active_lanes,
        } => {
            digest.update([RANKED_TENSOR_LAYOUT_TAG_V4]);
            hash_tensor_layout_contract(digest, contract);
            digest.update([match convergence {
                dialect_kernel::TensorConvergenceAttr::UniformSubgroup => 1,
                dialect_kernel::TensorConvergenceAttr::Divergent => 2,
                dialect_kernel::TensorConvergenceAttr::UniformWorkgroup => 3,
                dialect_kernel::TensorConvergenceAttr::Opaque => 4,
            }]);
            digest.update(active_lanes.to_le_bytes());
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
        ProductionRankedOperationV1::SemanticExpression {
            result,
            expression,
            numerical_contract,
        } => {
            digest.update([28]);
            digest.update(result.get().to_le_bytes());
            digest.update(expression.canonical_transcript_sha256(*numerical_contract));
        }
        ProductionRankedOperationV1::CollectiveSemantics {
            contract,
            view,
            actual,
            expected,
            witness0,
            witness1,
        } => {
            // V4 does not gain a coverage pass from this graph entry. This
            // only commits the complete ranked recipe for proof correlation.
            digest.update([30]);
            digest.update([match contract.kind() {
                super::ProductionCollectiveSemanticKindV1::FiniteFold => 1,
                super::ProductionCollectiveSemanticKindV1::FiniteRecurrence => 2,
                super::ProductionCollectiveSemanticKindV1::PermutationGather => 3,
            }]);
            for identity in [
                contract.contract_identity(),
                contract.source_domain_identity(),
                contract.target_domain_identity(),
            ] {
                for word in identity {
                    digest.update(word.to_le_bytes());
                }
            }
            digest.update(contract.domain_bound().to_le_bytes());
            digest.update(contract.step_bound().to_le_bytes());
            digest.update([match contract.order() {
                dialect_kernel::SemanticEvaluationOrderAttr::Ascending => 1,
                dialect_kernel::SemanticEvaluationOrderAttr::Descending => 2,
                dialect_kernel::SemanticEvaluationOrderAttr::Lexicographic => 3,
                dialect_kernel::SemanticEvaluationOrderAttr::Explicit => 4,
            }]);
            digest.update([match contract.numerical_contract() {
                super::ProductionNumericalContractV2::ExactBitVectorOperatorCongruence => 1,
                super::ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
                    rounding: super::ProductionIeeeRoundingModeV2::NearestTiesToEven,
                    exceptional_values:
                        super::ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
                } => 2,
                _ => 255,
            }]);
            digest.update([match contract.coverage() {
                dialect_kernel::SemanticCoverageBindingAttr::TotalView => 1,
                dialect_kernel::SemanticCoverageBindingAttr::CollectiveContributions => 2,
            }]);
            for value in [view, actual, expected, witness0, witness1] {
                hash_value(digest, *value);
            }
        }
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => {
            digest.update([12]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
        }
        ProductionRankedOperationV1::RequireReferenceEquivalent {
            actual,
            expected,
            proof,
        } => {
            digest.update([23]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
            for identity in [
                proof.obligation_id(),
                proof.subject_id(),
                proof.model_id(),
                proof.evidence_id(),
            ] {
                for word in identity {
                    digest.update(word.to_le_bytes());
                }
            }
        }
        ProductionRankedOperationV1::RequireAuthenticatedReferenceEquivalent {
            actual,
            expected,
            proof,
        } => {
            digest.update([24]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
            digest.update(proof.receipt_identity().digest().as_bytes());
            let binding = proof.binding();
            digest.update([binding.safe_reference_kind() as u8]);
            for identity in [
                binding.safe_reference_identity(),
                binding.safe_reference_source_hash(),
                binding.safe_reference_mir_hash(),
                binding.kernel_subject_identity(),
                binding.kernel_mir_hash(),
                binding.normalized_obligation_effect_ir_hash(),
            ] {
                digest.update(identity.as_bytes());
            }
        }
        ProductionRankedOperationV1::RequireEffectRefinement { contract, proof } => {
            digest.update([25]);
            hash_effect_refinement_contract(digest, contract);
            digest.update(proof.receipt_identity().digest().as_bytes());
            digest.update(
                proof
                    .binding()
                    .normalized_obligation_effect_ir_hash()
                    .as_bytes(),
            );
        }
        ProductionRankedOperationV1::RequestAuthenticatedReferenceEquivalent {
            actual,
            expected,
            subjects,
        } => {
            digest.update([26]);
            hash_value(digest, *actual);
            hash_value(digest, *expected);
            hash_functional_refinement_subjects(digest, *subjects);
        }
        ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects } => {
            digest.update([27]);
            digest.update(contract.request_shape_hash().as_bytes());
            hash_functional_refinement_subjects(digest, *subjects);
        }
        ProductionRankedOperationV1::RequireNumericalRefinement { contract, proof } => {
            digest.update([28]);
            hash_numerical_refinement_contract(digest, *contract);
            digest.update(proof.receipt_identity().digest().as_bytes());
            digest.update(
                proof
                    .binding()
                    .normalized_obligation_effect_ir_hash()
                    .as_bytes(),
            );
        }
        ProductionRankedOperationV1::RequestNumericalRefinement { contract, subjects } => {
            digest.update([29]);
            digest.update(contract.request_shape_hash().as_bytes());
            hash_functional_refinement_subjects(digest, *subjects);
        }
    }
}

fn hash_functional_refinement_subjects(
    digest: &mut Sha256,
    subjects: fe2o3_functional_proof::FunctionalRefinementSubjectsV2,
) {
    digest.update([subjects.safe_reference_kind() as u8]);
    for identity in [
        subjects.safe_reference_identity(),
        subjects.safe_reference_source_hash(),
        subjects.safe_reference_mir_hash(),
        subjects.kernel_subject_identity(),
        subjects.kernel_mir_hash(),
    ] {
        digest.update(identity.as_bytes());
    }
}

fn hash_tensor_layout_contract(digest: &mut Sha256, contract: &TensorLayoutContractV1) {
    match contract.profile {
        TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64 => digest.update([1]),
        TensorInstructionProfileV1::IncompatibleWave32 => digest.update([2]),
        TensorInstructionProfileV1::Opaque(identity) => {
            digest.update([3]);
            digest.update(identity.to_le_bytes());
        }
    }
    digest.update(contract.subgroup_width.to_le_bytes());
    hash_tensor_fragment(digest, &contract.a);
    hash_tensor_fragment(digest, &contract.b);
    hash_tensor_fragment(digest, &contract.accumulator);
    match contract.tail_mask {
        TensorTailMaskV1::ExactPhysicalTile => digest.update([1]),
        TensorTailMaskV1::ZeroFilledPredicateInputs => digest.update([2]),
        TensorTailMaskV1::PredicateMask => digest.update([3]),
        TensorTailMaskV1::Missing => digest.update([4]),
        TensorTailMaskV1::Unsupported(code) => digest.update([5, code]),
    }
}

fn hash_tensor_fragment(digest: &mut Sha256, fragment: &TensorFragmentLayoutV1) {
    digest.update([match fragment.role {
        TensorOperandRoleV1::A => 1,
        TensorOperandRoleV1::B => 2,
        TensorOperandRoleV1::Accumulator => 3,
    }]);
    for extent in fragment.shape {
        digest.update(extent.to_le_bytes());
    }
    digest.update([match fragment.element {
        MatrixElement::Bf16 => 1,
        MatrixElement::F32 => 2,
    }]);
    digest.update([fragment.fragment_elements]);
    match fragment.mapping {
        TensorSymbolicMapV1::LaneComponentAffine {
            lane_modulus,
            lane_divisor,
            axes,
        } => {
            digest.update([1]);
            digest.update(lane_modulus.to_le_bytes());
            digest.update(lane_divisor.to_le_bytes());
            for axis in axes {
                digest.update(axis.constant.to_le_bytes());
                digest.update(axis.lane_mod_scale.to_le_bytes());
                digest.update(axis.lane_div_scale.to_le_bytes());
                digest.update(axis.component_scale.to_le_bytes());
                digest.update([u8::from(axis.tile_origin)]);
            }
        }
        TensorSymbolicMapV1::Opaque(identity) => {
            digest.update([2]);
            digest.update(identity.to_le_bytes());
        }
    }
    match fragment.multiplicity {
        TensorMultiplicityV1::Unique => digest.update([1]),
        TensorMultiplicityV1::Broadcast { factor } => digest.update([2, factor]),
    }
    match fragment.packing {
        TensorElementPackingV1::Bf16PairInI32 => digest.update([1]),
        TensorElementPackingV1::F32Scalar => digest.update([2]),
        TensorElementPackingV1::Unsupported(code) => digest.update([3, code]),
    }
    match fragment.lds_swizzle {
        TensorLdsSwizzleV1::None => digest.update([1]),
        TensorLdsSwizzleV1::Xor4 => digest.update([2]),
        TensorLdsSwizzleV1::Unsupported(code) => digest.update([3, code]),
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
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => {
            digest.update([7]);
            hash_value(digest, *lhs);
            hash_value(digest, *rhs);
            hash_values(digest, true_arguments);
            hash_values(digest, false_arguments);
            digest.update(true_block.to_le_bytes());
            digest.update(false_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::IndexEqual {
            lhs,
            rhs,
            true_block,
            false_block,
        } => {
            digest.update([8]);
            hash_value(digest, *lhs);
            hash_value(digest, *rhs);
            digest.update(true_block.to_le_bytes());
            digest.update(false_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::IndexEqualArgs {
            lhs,
            rhs,
            true_arguments,
            false_arguments,
            true_block,
            false_block,
        } => {
            digest.update([9]);
            hash_value(digest, *lhs);
            hash_value(digest, *rhs);
            hash_values(digest, true_arguments);
            hash_values(digest, false_arguments);
            digest.update(true_block.to_le_bytes());
            digest.update(false_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::AnalysisSplit {
            control_dependencies,
            first_block,
            second_block,
        } => {
            digest.update([4]);
            hash_values(digest, control_dependencies);
            digest.update(first_block.to_le_bytes());
            digest.update(second_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            control_dependencies,
            first_arguments,
            second_arguments,
            first_block,
            second_block,
        } => {
            digest.update([10]);
            hash_values(digest, control_dependencies);
            hash_values(digest, first_arguments);
            hash_values(digest, second_arguments);
            digest.update(first_block.to_le_bytes());
            digest.update(second_block.to_le_bytes());
        }
        ProductionRankedTerminatorV1::Branch { target } => {
            digest.update([2]);
            digest.update(target.to_le_bytes());
        }
        ProductionRankedTerminatorV1::BranchArgs { arguments, target } => {
            digest.update([5]);
            hash_values(digest, arguments);
            digest.update(target.to_le_bytes());
        }
        ProductionRankedTerminatorV1::BranchArgsAdd {
            value,
            step,
            target,
        } => {
            digest.update([6]);
            hash_value(digest, *value);
            hash_value(digest, *step);
            digest.update(target.to_le_bytes());
        }
        ProductionRankedTerminatorV1::BranchArgsAddAt {
            arguments,
            add_argument,
            step,
            target,
        } => {
            digest.update([11]);
            hash_values(digest, arguments);
            digest.update(add_argument.to_le_bytes());
            hash_value(digest, *step);
            digest.update(target.to_le_bytes());
        }
        ProductionRankedTerminatorV1::Return => digest.update([3]),
        ProductionRankedTerminatorV1::Trap => digest.update([12]),
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
        ProductionRankedValueV1::BlockArgument { block, argument } => {
            digest.update([3]);
            digest.update(block.to_le_bytes());
            digest.update(argument.to_le_bytes());
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
        IndexBinaryKindAttr::Divide => 4,
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

struct ReaderV4<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV4<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionMiddleEndEvidenceCodecErrorV4> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV4::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionMiddleEndEvidenceCodecErrorV4::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionMiddleEndEvidenceCodecErrorV4> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionMiddleEndEvidenceCodecErrorV4::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ProductionMiddleEndEvidenceCodecErrorV4> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionMiddleEndEvidenceCodecErrorV4> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionMiddleEndEvidenceCodecErrorV4> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionMiddleEndEvidenceCodecErrorV4> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductionRankedValueIdV1;

    fn clean_facts() -> [ObservedPassFactV4; PASS_COUNT_V4] {
        PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1.map(|pass| ObservedPassFactV4 {
            pass,
            clean: true,
            findings: 0,
            compiler_authority: false,
            artifact_authority: false,
        })
    }

    #[test]
    fn every_live_report_fact_is_fail_closed() {
        for index in 0..PASS_COUNT_V4 {
            let expected = PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4[index];

            let mut facts = clean_facts();
            facts[index].clean = false;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(ProductionMiddleEndEvidenceCodecErrorV4::ReportNotClean { pass: expected })
            );

            let mut facts = clean_facts();
            facts[index].findings = 1;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(ProductionMiddleEndEvidenceCodecErrorV4::ReportNotClean { pass: expected })
            );

            let mut facts = clean_facts();
            facts[index].compiler_authority = true;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(
                    ProductionMiddleEndEvidenceCodecErrorV4::ReportAuthorityClaim {
                        pass: expected,
                    }
                )
            );

            let mut facts = clean_facts();
            facts[index].artifact_authority = true;
            assert_eq!(
                validate_observed_pass_facts(&facts),
                Err(
                    ProductionMiddleEndEvidenceCodecErrorV4::ReportAuthorityClaim {
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
                ProductionMiddleEndEvidenceCodecErrorV4::ReportPassOrderMismatch {
                    index: 0,
                    expected: ProductionMiddleEndEvidencePassV4::TensorLayout,
                }
            )
        );
    }

    #[test]
    fn ranked_ir_lexical_form_is_canonical() {
        assert!(validate_ranked_ir(b"func @kernel {\n}\n").is_ok());
        assert_eq!(
            validate_ranked_ir(b""),
            Err(ProductionMiddleEndEvidenceCodecErrorV4::EmptyRankedIr)
        );
        assert_eq!(
            validate_ranked_ir(b"func @kernel {}"),
            Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrMissingFinalNewline)
        );
        assert_eq!(
            validate_ranked_ir(b"func @kernel {\r\n}\n"),
            Err(ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset: 14 })
        );
        assert_eq!(
            validate_ranked_ir(b"func\0\n"),
            Err(ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset: 4 })
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

    #[test]
    fn ranked_trap_identity_is_distinct_from_successful_return() {
        fn identity(terminator: &ProductionRankedTerminatorV1) -> [u8; SHA256_BYTES] {
            let mut digest = Sha256::new();
            hash_ranked_terminator(&mut digest, terminator);
            digest.finalize().into()
        }

        assert_ne!(
            identity(&ProductionRankedTerminatorV1::Return),
            identity(&ProductionRankedTerminatorV1::Trap)
        );
    }

    #[test]
    fn ranked_execution_and_tensor_layouts_have_distinct_v4_variant_tags_and_identities() {
        fn identity(operation: &ProductionRankedOperationV1) -> [u8; SHA256_BYTES] {
            let mut digest = Sha256::new();
            hash_ranked_operation(&mut digest, operation);
            digest.finalize().into()
        }

        assert_eq!(RANKED_EXECUTION_LAYOUT_TAG_V4, 15);
        assert_eq!(RANKED_TENSOR_LAYOUT_TAG_V4, 19);
        assert_eq!(RANKED_OWNERSHIP_CONTRACT_TAG_V4, 24);
        assert_ne!(RANKED_EXECUTION_LAYOUT_TAG_V4, RANKED_TENSOR_LAYOUT_TAG_V4);
        assert_ne!(
            RANKED_EXECUTION_LAYOUT_TAG_V4,
            RANKED_OWNERSHIP_CONTRACT_TAG_V4
        );
        assert_ne!(
            RANKED_TENSOR_LAYOUT_TAG_V4,
            RANKED_OWNERSHIP_CONTRACT_TAG_V4
        );

        let execution = ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [64, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        };
        let tensor = ProductionRankedOperationV1::TensorLayout {
            contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            convergence: dialect_kernel::TensorConvergenceAttr::UniformSubgroup,
            active_lanes: 64,
        };
        assert_ne!(identity(&execution), identity(&tensor));
    }

    #[test]
    fn ranked_control_and_divide_identity_bind_every_new_recipe_field() {
        fn terminator_identity(terminator: &ProductionRankedTerminatorV1) -> [u8; SHA256_BYTES] {
            let mut digest = Sha256::new();
            hash_ranked_terminator(&mut digest, terminator);
            digest.finalize().into()
        }
        fn operation_identity(operation: &ProductionRankedOperationV1) -> [u8; SHA256_BYTES] {
            let mut digest = Sha256::new();
            hash_ranked_operation(&mut digest, operation);
            digest.finalize().into()
        }

        let split = ProductionRankedTerminatorV1::AnalysisSplitArgs {
            control_dependencies: vec![ProductionRankedValueV1::Argument(2)],
            first_arguments: vec![ProductionRankedValueV1::Argument(0)],
            second_arguments: vec![ProductionRankedValueV1::Argument(1)],
            first_block: 1,
            second_block: 2,
        };
        for changed in [
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies: vec![ProductionRankedValueV1::Argument(2)],
                first_arguments: vec![],
                second_arguments: vec![
                    ProductionRankedValueV1::Argument(0),
                    ProductionRankedValueV1::Argument(1),
                ],
                first_block: 1,
                second_block: 2,
            },
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies: vec![ProductionRankedValueV1::Argument(3)],
                first_arguments: vec![ProductionRankedValueV1::Argument(1)],
                second_arguments: vec![ProductionRankedValueV1::Argument(0)],
                first_block: 1,
                second_block: 2,
            },
        ] {
            assert_ne!(terminator_identity(&split), terminator_identity(&changed));
        }

        let add_at = ProductionRankedTerminatorV1::BranchArgsAddAt {
            arguments: vec![
                ProductionRankedValueV1::Argument(0),
                ProductionRankedValueV1::Argument(1),
            ],
            add_argument: 0,
            step: ProductionRankedValueV1::Argument(2),
            target: 3,
        };
        for changed in [
            ProductionRankedTerminatorV1::BranchArgsAddAt {
                arguments: vec![
                    ProductionRankedValueV1::Argument(0),
                    ProductionRankedValueV1::Argument(1),
                ],
                add_argument: 1,
                step: ProductionRankedValueV1::Argument(2),
                target: 3,
            },
            ProductionRankedTerminatorV1::BranchArgsAddAt {
                arguments: vec![
                    ProductionRankedValueV1::Argument(0),
                    ProductionRankedValueV1::Argument(1),
                ],
                add_argument: 0,
                step: ProductionRankedValueV1::Argument(3),
                target: 3,
            },
        ] {
            assert_ne!(terminator_identity(&add_at), terminator_identity(&changed));
        }

        let divide = ProductionRankedOperationV1::IndexBinary {
            result: ProductionRankedValueIdV1::new(0),
            kind: IndexBinaryKindAttr::Divide,
            lhs: ProductionRankedValueV1::Argument(0),
            rhs: ProductionRankedValueV1::Argument(1),
        };
        let add = ProductionRankedOperationV1::IndexBinary {
            result: ProductionRankedValueIdV1::new(0),
            kind: IndexBinaryKindAttr::Add,
            lhs: ProductionRankedValueV1::Argument(0),
            rhs: ProductionRankedValueV1::Argument(1),
        };
        assert_ne!(operation_identity(&divide), operation_identity(&add));
    }
}
