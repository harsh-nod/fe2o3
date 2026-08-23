use std::fmt;

use crate::kernel_ir_v6_identity::RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity;

/// Current production policy version committed by a V4 lowering configuration.
pub const LOWERING_POLICY_VERSION_V4: u64 = 2;
/// Maximum canonical semantic-MIR length representable by V4 lineage.
pub const MAX_CANONICAL_SEMANTIC_MIR_BYTES_V4: u64 = 128 * 1024 * 1024;
/// Maximum canonical Kernel-IR length representable by V4 lineage.
pub const MAX_CANONICAL_KERNEL_IR_BYTES_V4: u64 = 16 * 1024 * 1024;

/// One stage charged by canonical lineage admission work accounting.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LineageWorkStageV4 {
    /// Input bytes and decoded record traversal.
    Parse,
    /// Structural validation traversal and bitmap initialization.
    StructuralValidation,
    /// Canonical encoding, including decoder re-encoding.
    CanonicalEncoding,
}

/// Exact measured work consumed by one canonical lineage operation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineageWorkV4 {
    parse: u64,
    structural_validation: u64,
    canonical_encoding: u64,
    total: u64,
}

impl LineageWorkV4 {
    /// Returns input parsing and decoded-record work.
    pub const fn parse(self) -> u64 {
        self.parse
    }

    /// Returns structural traversal and bitmap-initialization work.
    pub const fn structural_validation(self) -> u64 {
        self.structural_validation
    }

    /// Returns canonical encoding or re-encoding work.
    pub const fn canonical_encoding(self) -> u64 {
        self.canonical_encoding
    }

    /// Returns the single checked aggregate work total.
    pub const fn total(self) -> u64 {
        self.total
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkBudgetErrorV4 {
    Overflow {
        stage: LineageWorkStageV4,
    },
    LimitExceeded {
        stage: LineageWorkStageV4,
        actual: u64,
        max: u64,
    },
}

pub(crate) struct WorkBudgetV4 {
    work: LineageWorkV4,
    max: u64,
}

impl WorkBudgetV4 {
    pub(crate) const fn new(max: u64) -> Self {
        Self {
            work: LineageWorkV4 {
                parse: 0,
                structural_validation: 0,
                canonical_encoding: 0,
                total: 0,
            },
            max,
        }
    }

    pub(crate) const fn unbounded() -> Self {
        Self::new(u64::MAX)
    }

    pub(crate) fn charge(
        &mut self,
        stage: LineageWorkStageV4,
        amount: u64,
    ) -> Result<(), WorkBudgetErrorV4> {
        let stage_total = match stage {
            LineageWorkStageV4::Parse => &mut self.work.parse,
            LineageWorkStageV4::StructuralValidation => &mut self.work.structural_validation,
            LineageWorkStageV4::CanonicalEncoding => &mut self.work.canonical_encoding,
        };
        *stage_total = stage_total
            .checked_add(amount)
            .ok_or(WorkBudgetErrorV4::Overflow { stage })?;
        self.work.total = self
            .work
            .total
            .checked_add(amount)
            .ok_or(WorkBudgetErrorV4::Overflow { stage })?;
        if self.work.total > self.max {
            return Err(WorkBudgetErrorV4::LimitExceeded {
                stage,
                actual: self.work.total,
                max: self.max,
            });
        }
        Ok(())
    }

    pub(crate) const fn work(&self) -> LineageWorkV4 {
        self.work
    }
}

/// Distinguishes an identity's canonical artifact kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ArtifactKindV4 {
    /// Canonical semantic MIR.
    SemanticMir,
    /// Canonical Kernel IR.
    KernelIr,
}

/// Closed canonical semantic-MIR wire versions understood by V4 lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SemanticMirCanonicalWireVersionV4 {
    /// Legacy canonical semantic MIR V2.
    V2 = 2,
    /// Production canonical semantic MIR V3.
    V3 = 3,
}

/// Closed canonical Kernel-IR wire versions understood by V4 lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum KernelIrCanonicalWireVersionV4 {
    /// Legacy canonical Kernel IR V5.
    V5 = 5,
    /// Production canonical Kernel IR V6.
    V6 = 6,
}

/// Identity scheme for canonical semantic MIR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticMirIdentitySchemeV4 {
    /// Raw SHA-256 of the exact canonical semantic-MIR bytes.
    RawCanonicalSha256,
}

/// Identity scheme for canonical Kernel IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelIrIdentitySchemeV4 {
    /// Frozen verified-canonical KIR V5 SHA-256 policy V1 construction.
    VerifiedCanonicalKernelIrV5Sha256PolicyV1,
    /// Frozen verified-canonical KIR V6 SHA-256 policy V1 construction.
    VerifiedCanonicalKernelIrV6Sha256PolicyV1,
}

/// Opaque digest bytes and exact byte length of one canonical artifact.
///
/// This is an inert content claim supplied by a producer. This crate does not
/// possess either preimage and therefore does not authenticate the claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct CanonicalArtifactIdentityV4 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl CanonicalArtifactIdentityV4 {
    /// Constructs a nonzero digest and nonzero canonical byte length.
    fn new(
        kind: ArtifactKindV4,
        digest: [u8; 32],
        canonical_length: u64,
    ) -> Result<Self, LineageValidationErrorV4> {
        if digest == [0; 32] {
            return Err(LineageValidationErrorV4::ZeroArtifactIdentity { kind });
        }
        if canonical_length == 0 {
            return Err(LineageValidationErrorV4::ZeroCanonicalLength { kind });
        }
        let max = match kind {
            ArtifactKindV4::SemanticMir => MAX_CANONICAL_SEMANTIC_MIR_BYTES_V4,
            ArtifactKindV4::KernelIr => MAX_CANONICAL_KERNEL_IR_BYTES_V4,
        };
        if canonical_length > max {
            return Err(LineageValidationErrorV4::CanonicalLengthLimitExceeded {
                kind,
                actual: canonical_length,
                max,
            });
        }
        Ok(Self {
            digest,
            canonical_length,
        })
    }

    /// Returns the scheme-specific digest bytes.
    const fn digest(self) -> [u8; 32] {
        self.digest
    }

    /// Returns the exact claimed canonical byte length.
    const fn canonical_length(self) -> u64 {
        self.canonical_length
    }
}

/// Typed raw-canonical semantic-MIR identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalSemanticMirIdentityV4 {
    scheme: SemanticMirIdentitySchemeV4,
    wire_version: SemanticMirCanonicalWireVersionV4,
    artifact: CanonicalArtifactIdentityV4,
}

impl CanonicalSemanticMirIdentityV4 {
    /// Constructs a semantic-MIR content identity.
    pub fn new(
        wire_version: SemanticMirCanonicalWireVersionV4,
        raw_canonical_sha256: [u8; 32],
        canonical_length: u64,
    ) -> Result<Self, LineageValidationErrorV4> {
        Ok(Self {
            scheme: SemanticMirIdentitySchemeV4::RawCanonicalSha256,
            wire_version,
            artifact: CanonicalArtifactIdentityV4::new(
                ArtifactKindV4::SemanticMir,
                raw_canonical_sha256,
                canonical_length,
            )?,
        })
    }

    /// Returns the explicit semantic-MIR identity scheme.
    pub const fn scheme(self) -> SemanticMirIdentitySchemeV4 {
        self.scheme
    }

    /// Returns the canonical semantic-MIR wire version.
    pub const fn wire_version(self) -> SemanticMirCanonicalWireVersionV4 {
        self.wire_version
    }

    /// Returns raw SHA-256 of the exact canonical semantic-MIR bytes.
    pub const fn raw_canonical_sha256(self) -> [u8; 32] {
        self.artifact.digest()
    }

    /// Returns the exact canonical semantic-MIR byte length.
    pub const fn canonical_length(self) -> u64 {
        self.artifact.canonical_length()
    }
}

/// Typed domain/policy canonical Kernel-IR identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKernelIrIdentityV4 {
    scheme: KernelIrIdentitySchemeV4,
    wire_version: KernelIrCanonicalWireVersionV4,
    artifact: CanonicalArtifactIdentityV4,
}

impl CanonicalKernelIrIdentityV4 {
    /// Constructs a production V6 identity from exact typed recomputation.
    pub fn new_v6(
        identity: RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity,
    ) -> Result<Self, LineageValidationErrorV4> {
        Ok(Self {
            scheme: KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1,
            wire_version: KernelIrCanonicalWireVersionV4::V6,
            artifact: CanonicalArtifactIdentityV4::new(
                ArtifactKindV4::KernelIr,
                identity.digest(),
                identity.canonical_length(),
            )?,
        })
    }

    /// Constructs an inert legacy V5 identity claim under its exact scheme.
    ///
    /// This compatibility constructor does not recompute or authenticate the
    /// supplied digest and is never production-eligible.
    pub fn new_legacy_v5_claimed_sha256_policy_v1(
        claimed_digest: [u8; 32],
        canonical_length: u64,
    ) -> Result<Self, LineageValidationErrorV4> {
        Ok(Self {
            scheme: KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV5Sha256PolicyV1,
            wire_version: KernelIrCanonicalWireVersionV4::V5,
            artifact: CanonicalArtifactIdentityV4::new(
                ArtifactKindV4::KernelIr,
                claimed_digest,
                canonical_length,
            )?,
        })
    }

    pub(crate) fn from_wire(
        scheme: KernelIrIdentitySchemeV4,
        wire_version: KernelIrCanonicalWireVersionV4,
        claimed_digest: [u8; 32],
        canonical_length: u64,
    ) -> Result<Self, LineageValidationErrorV4> {
        let scheme_matches_version = matches!(
            (scheme, wire_version),
            (
                KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV5Sha256PolicyV1,
                KernelIrCanonicalWireVersionV4::V5
            ) | (
                KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1,
                KernelIrCanonicalWireVersionV4::V6
            )
        );
        if !scheme_matches_version {
            return Err(
                LineageValidationErrorV4::KernelIrIdentitySchemeVersionMismatch {
                    scheme,
                    wire_version,
                },
            );
        }
        Ok(Self {
            scheme,
            wire_version,
            artifact: CanonicalArtifactIdentityV4::new(
                ArtifactKindV4::KernelIr,
                claimed_digest,
                canonical_length,
            )?,
        })
    }

    /// Returns the explicit Kernel-IR identity scheme.
    pub const fn scheme(self) -> KernelIrIdentitySchemeV4 {
        self.scheme
    }

    /// Returns the canonical Kernel-IR wire version.
    pub const fn wire_version(self) -> KernelIrCanonicalWireVersionV4 {
        self.wire_version
    }

    /// Returns the scheme-specific digest carried by this inert lineage claim.
    ///
    /// This does not return a recomputation token: decoded bytes may carry any
    /// nonzero digest, and an external owner must recompute the exact scheme
    /// over the retained artifact before comparing it with this value.
    pub const fn claimed_scheme_digest(self) -> [u8; 32] {
        self.artifact.digest()
    }

    pub(crate) const fn scheme_digest(self) -> [u8; 32] {
        self.claimed_scheme_digest()
    }

    /// Returns the exact canonical Kernel-IR byte length.
    pub const fn canonical_length(self) -> u64 {
        self.artifact.canonical_length()
    }
}

/// Closed target selected by the current production lowering policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LoweringTargetV4 {
    /// AMD CDNA3 gfx942.
    AmdGpuGfx942,
}

/// How ranked bounds checks were handled by lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RankedBoundsPolicyV4 {
    /// Retain generic bounds checks in Kernel IR.
    RetainGenericChecks,
    /// Discharge checks only from separately validated ranked-lowering input.
    DischargeWithValidatedRankedInput,
}

/// Policy for declarations backing referenced f32 operations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum F32IntrinsicDeclarationPolicyV4 {
    /// Emit exactly the declarations referenced by semantic bodies.
    DeclareReferencedIntrinsics,
}

/// Policy for declarations backing diagnostic trap operations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticTrapDeclarationPolicyV4 {
    /// Emit exactly the declarations referenced by synthetic diagnostic blocks.
    DeclareReferencedTraps,
}

/// Required relational validation policy outside this inert crate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CorrespondenceValidationPolicyV4 {
    /// Require exhaustive typed traversal of operations, operands, results,
    /// types, metadata, block parameters, terminators, and CFG edges.
    ExhaustiveTypedTraversal,
}

/// Closed checked-arithmetic refinement required by lowering policy V2.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedArithmeticRefinementPolicyV4 {
    /// Semantic MIR V3 checked Add/Sub/Mul to one Kernel IR V6 checked op.
    ///
    /// The complete normative contract and frozen policy vector are documented
    /// in `CHECKED_ARITHMETIC_REFINEMENT.md`. This tag records an obligation;
    /// only the named downstream move-only owner gate may establish it.
    SemanticMirV3ToKernelIrV6CheckedV1,
    /// No checked-arithmetic refinement claim for legacy V2-to-V5 data.
    ///
    /// This tag is accepted only with [`LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5`]
    /// and can never satisfy a production owner gate.
    LegacyInertNoRefinementAuthority,
}

/// Frozen byte vector defining checked-arithmetic refinement policy V1.
///
/// Fields are fixed-width little-endian where wider than one byte. In order:
/// magic, policy version, semantic/KIR versions, target, Add/Sub/Mul tags,
/// widths, signednesses, index mapping and width, result order, operand order,
/// projection handling, assertion independence, and span attribution.
pub const CHECKED_ARITHMETIC_REFINEMENT_POLICY_VECTOR_V4: &[u8] = &[
    b'F', b'E', b'2', b'O', b'3', b'C', b'A', b'1', // magic
    1, 0, // checked-arithmetic policy V1
    3, 0, // semantic MIR V3
    6, 0, // Kernel IR V6
    0, // gfx942
    3, 0, 1, 2, // Add, Sub, Mul
    5, 8, 0, 16, 0, 32, 0, 64, 0, 128, 0, // widths
    2, 0, 1, // unsigned, signed
    0, 64, 0, // usize/isize use target pointer width, 64
    2, 0, 1, // wrapped T, bool overflow
    0, // evaluate lhs then rhs, once each
    0, // preserve projection order; do not fuse projections
    0, // overflow assertion is independent
    0, // complete contiguous source-statement operation attribution
];

/// Required external owner gate for checked-arithmetic semantic refinement.
pub const CHECKED_ARITHMETIC_EXTERNAL_OWNER_GATE_V4: &str =
    "semantic-mir-v3-kernel-ir-v6-checked-arithmetic-owner-gate-v1";

/// Artifact-version pair admitted by a V4 lowering policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LineagePolicyModeV4 {
    /// Production semantic MIR V3 to Kernel IR V6.
    ProductionSemanticMirV3ToKernelIrV6,
    /// Legacy semantic MIR V2 to Kernel IR V5, retained as inert data only.
    LegacyInertSemanticMirV2ToKernelIrV5,
}

/// A resource bounded by the lowering invocation retained in lineage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LineageResourceV4 {
    /// Semantic functions.
    SemanticFunctions,
    /// Kernel-IR functions, including declarations.
    KirFunctions,
    /// Exported kernel records.
    Kernels,
    /// Semantic and synthetic Kernel-IR blocks.
    Blocks,
    /// Semantic statements.
    Statements,
    /// Kernel-IR operations.
    Operations,
    /// Validation or lowering work units.
    Work,
}

/// Exact resource limits applied by the represented lowering invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweringResourceLimitsV4 {
    max_semantic_functions: u64,
    max_kir_functions: u64,
    max_kernels: u64,
    max_blocks: u64,
    max_statements: u64,
    max_operations: u64,
    max_work: u64,
}

impl LoweringResourceLimitsV4 {
    /// Constructs explicit nonzero lowering limits.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_semantic_functions: u64,
        max_kir_functions: u64,
        max_kernels: u64,
        max_blocks: u64,
        max_statements: u64,
        max_operations: u64,
        max_work: u64,
    ) -> Result<Self, LineageValidationErrorV4> {
        let value = Self {
            max_semantic_functions,
            max_kir_functions,
            max_kernels,
            max_blocks,
            max_statements,
            max_operations,
            max_work,
        };
        value.validate()?;
        Ok(value)
    }

    /// Returns the configured maximum for a resource.
    pub const fn limit(self, resource: LineageResourceV4) -> u64 {
        match resource {
            LineageResourceV4::SemanticFunctions => self.max_semantic_functions,
            LineageResourceV4::KirFunctions => self.max_kir_functions,
            LineageResourceV4::Kernels => self.max_kernels,
            LineageResourceV4::Blocks => self.max_blocks,
            LineageResourceV4::Statements => self.max_statements,
            LineageResourceV4::Operations => self.max_operations,
            LineageResourceV4::Work => self.max_work,
        }
    }

    fn validate(self) -> Result<(), LineageValidationErrorV4> {
        for resource in [
            LineageResourceV4::SemanticFunctions,
            LineageResourceV4::KirFunctions,
            LineageResourceV4::Kernels,
            LineageResourceV4::Blocks,
            LineageResourceV4::Statements,
            LineageResourceV4::Operations,
            LineageResourceV4::Work,
        ] {
            if self.limit(resource) == 0 {
                return Err(LineageValidationErrorV4::ZeroLimit { resource });
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn from_wire_unvalidated(
        max_semantic_functions: u64,
        max_kir_functions: u64,
        max_kernels: u64,
        max_blocks: u64,
        max_statements: u64,
        max_operations: u64,
        max_work: u64,
    ) -> Self {
        Self {
            max_semantic_functions,
            max_kir_functions,
            max_kernels,
            max_blocks,
            max_statements,
            max_operations,
            max_work,
        }
    }
}

impl Default for LoweringResourceLimitsV4 {
    fn default() -> Self {
        Self {
            max_semantic_functions: 1_024,
            max_kir_functions: 2_048,
            max_kernels: 1_024,
            max_blocks: 16_384,
            max_statements: 1_048_576,
            max_operations: 4_194_304,
            max_work: 16_777_216,
        }
    }
}

/// Exact lowering policy and resource configuration retained in lineage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoweringConfigurationV4 {
    policy_version: u64,
    mode: LineagePolicyModeV4,
    target: LoweringTargetV4,
    ranked_bounds: RankedBoundsPolicyV4,
    f32_intrinsics: F32IntrinsicDeclarationPolicyV4,
    diagnostic_traps: DiagnosticTrapDeclarationPolicyV4,
    correspondence: CorrespondenceValidationPolicyV4,
    checked_arithmetic: CheckedArithmeticRefinementPolicyV4,
    limits: LoweringResourceLimitsV4,
}

impl LoweringConfigurationV4 {
    /// Constructs a V4 lowering configuration.
    pub fn new(
        ranked_bounds: RankedBoundsPolicyV4,
        limits: LoweringResourceLimitsV4,
    ) -> Result<Self, LineageValidationErrorV4> {
        limits.validate()?;
        Ok(Self {
            policy_version: LOWERING_POLICY_VERSION_V4,
            mode: LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6,
            target: LoweringTargetV4::AmdGpuGfx942,
            ranked_bounds,
            f32_intrinsics: F32IntrinsicDeclarationPolicyV4::DeclareReferencedIntrinsics,
            diagnostic_traps: DiagnosticTrapDeclarationPolicyV4::DeclareReferencedTraps,
            correspondence: CorrespondenceValidationPolicyV4::ExhaustiveTypedTraversal,
            checked_arithmetic:
                CheckedArithmeticRefinementPolicyV4::SemanticMirV3ToKernelIrV6CheckedV1,
            limits,
        })
    }

    /// Constructs explicitly non-production legacy V2-to-V5 configuration.
    ///
    /// The resulting lineage remains inert and must be rejected by every
    /// production owner-bearing validator.
    pub fn legacy_inert(
        ranked_bounds: RankedBoundsPolicyV4,
        limits: LoweringResourceLimitsV4,
    ) -> Result<Self, LineageValidationErrorV4> {
        let mut value = Self::new(ranked_bounds, limits)?;
        value.mode = LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5;
        value.checked_arithmetic =
            CheckedArithmeticRefinementPolicyV4::LegacyInertNoRefinementAuthority;
        Ok(value)
    }

    /// Returns the lowering-policy version.
    pub const fn policy_version(self) -> u64 {
        self.policy_version
    }

    /// Returns the exact production or legacy-inert artifact pair policy.
    pub const fn mode(self) -> LineagePolicyModeV4 {
        self.mode
    }

    /// Returns the exact lowering target.
    pub const fn target(self) -> LoweringTargetV4 {
        self.target
    }

    /// Returns the ranked-bounds handling policy.
    pub const fn ranked_bounds(self) -> RankedBoundsPolicyV4 {
        self.ranked_bounds
    }

    /// Returns the f32 declaration policy.
    pub const fn f32_intrinsics(self) -> F32IntrinsicDeclarationPolicyV4 {
        self.f32_intrinsics
    }

    /// Returns the diagnostic declaration policy.
    pub const fn diagnostic_traps(self) -> DiagnosticTrapDeclarationPolicyV4 {
        self.diagnostic_traps
    }

    /// Returns the required relational correspondence policy.
    pub const fn correspondence(self) -> CorrespondenceValidationPolicyV4 {
        self.correspondence
    }

    /// Returns the exact checked-arithmetic refinement obligation.
    pub const fn checked_arithmetic(self) -> CheckedArithmeticRefinementPolicyV4 {
        self.checked_arithmetic
    }

    /// Returns the exact represented lowering limits.
    pub const fn limits(self) -> LoweringResourceLimitsV4 {
        self.limits
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_wire(
        policy_version: u64,
        mode: LineagePolicyModeV4,
        target: LoweringTargetV4,
        ranked_bounds: RankedBoundsPolicyV4,
        f32_intrinsics: F32IntrinsicDeclarationPolicyV4,
        diagnostic_traps: DiagnosticTrapDeclarationPolicyV4,
        correspondence: CorrespondenceValidationPolicyV4,
        checked_arithmetic: CheckedArithmeticRefinementPolicyV4,
        limits: LoweringResourceLimitsV4,
    ) -> Result<Self, LineageValidationErrorV4> {
        if policy_version != LOWERING_POLICY_VERSION_V4 {
            return Err(LineageValidationErrorV4::UnsupportedPolicyVersion(
                policy_version,
            ));
        }
        Ok(Self {
            policy_version,
            mode,
            target,
            ranked_bounds,
            f32_intrinsics,
            diagnostic_traps,
            correspondence,
            checked_arithmetic,
            limits,
        })
    }
}

/// Closed f32 declaration identities used by Kernel IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum F32IntrinsicV4 {
    /// Square root.
    Sqrt = 0,
    /// Fused multiply-add.
    FusedMultiplyAdd = 1,
    /// Floor.
    Floor = 2,
    /// Ceiling.
    Ceil = 3,
    /// Truncation.
    Truncate = 4,
    /// Round to nearest, ties to even.
    RoundTiesEven = 5,
    /// Sine.
    Sin = 6,
    /// Cosine.
    Cos = 7,
    /// Base-e exponential.
    Exp = 8,
    /// Base-2 exponential.
    Exp2 = 9,
    /// Natural logarithm.
    Ln = 10,
    /// Base-2 logarithm.
    Log2 = 11,
    /// Base-10 logarithm.
    Log10 = 12,
}

/// Closed diagnostic declaration identities used by Kernel IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticTrapKindV4 {
    /// Runtime assertion failure trap.
    RuntimeAssertFailure,
}

/// Classification of one Kernel-IR function record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FunctionClassificationV4 {
    /// A function body lowered from one semantic-MIR function.
    SemanticBody {
        /// Exact source semantic-function ordinal.
        semantic_function_ordinal: u64,
        /// Exact source semantic-block count.
        semantic_block_count: u64,
    },
    /// A declaration backing one referenced f32 intrinsic.
    F32IntrinsicDeclaration(F32IntrinsicV4),
    /// A declaration backing one referenced diagnostic trap.
    DiagnosticTrapDeclaration(DiagnosticTrapKindV4),
}

/// A contiguous operation interval within one Kernel-IR block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OperationSpanV4 {
    first_operation_ordinal: u64,
    operation_count: u64,
}

impl OperationSpanV4 {
    const fn new(first_operation_ordinal: u64, operation_count: u64) -> Self {
        Self {
            first_operation_ordinal,
            operation_count,
        }
    }

    /// Returns the first operation ordinal.
    pub const fn first_operation_ordinal(self) -> u64 {
        self.first_operation_ordinal
    }

    /// Returns the operation count.
    pub const fn operation_count(self) -> u64 {
        self.operation_count
    }
}

/// Operation span emitted by one semantic-MIR statement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatementOperationSpanV4 {
    statement_ordinal: u64,
    operations: OperationSpanV4,
}

impl StatementOperationSpanV4 {
    const fn new(statement_ordinal: u64, operations: OperationSpanV4) -> Self {
        Self {
            statement_ordinal,
            operations,
        }
    }

    /// Returns the source statement ordinal.
    pub const fn statement_ordinal(self) -> u64 {
        self.statement_ordinal
    }

    /// Returns the exact emitted operation interval.
    pub const fn operations(self) -> OperationSpanV4 {
        self.operations
    }
}

/// Iterator deriving statement ordinals and cumulative operation starts.
#[derive(Clone, Debug)]
pub struct StatementOperationSpansV4<'a> {
    counts: &'a [u64],
    statement_ordinal: u64,
    first_operation_ordinal: u64,
}

impl<'a> StatementOperationSpansV4<'a> {
    const fn new(counts: &'a [u64]) -> Self {
        Self {
            counts,
            statement_ordinal: 0,
            first_operation_ordinal: 0,
        }
    }
}

impl Iterator for StatementOperationSpansV4<'_> {
    type Item = StatementOperationSpanV4;

    fn next(&mut self) -> Option<Self::Item> {
        let (&operation_count, remaining) = self.counts.split_first()?;
        let span = StatementOperationSpanV4::new(
            self.statement_ordinal,
            OperationSpanV4::new(self.first_operation_ordinal, operation_count),
        );
        self.statement_ordinal = self
            .statement_ordinal
            .checked_add(1)
            .expect("validated statement count fits u64");
        self.first_operation_ordinal = self
            .first_operation_ordinal
            .checked_add(operation_count)
            .expect("validated statement operation counts do not overflow");
        self.counts = remaining;
        Some(span)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.counts.len(), Some(self.counts.len()))
    }
}

impl std::iter::FusedIterator for StatementOperationSpansV4<'_> {}

/// Operation span emitted while lowering one semantic-MIR terminator.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TerminatorOperationSpanV4 {
    operations: OperationSpanV4,
}

impl TerminatorOperationSpanV4 {
    const fn new(operations: OperationSpanV4) -> Self {
        Self { operations }
    }

    /// Returns the exact emitted operation interval.
    pub const fn operations(self) -> OperationSpanV4 {
        self.operations
    }
}

/// Closed rule responsible for one synthetic Kernel-IR block.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SyntheticBlockRuleV4 {
    /// One diagnostic trap followed by an unreachable terminator.
    RuntimeAssertFailureTrap,
}

/// Classification and local correspondence content for one Kernel-IR block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BlockClassificationV4 {
    /// A block corresponding to one semantic-MIR block.
    SemanticBlock {
        /// Exact source semantic-block ordinal.
        semantic_block_ordinal: u64,
        /// Operation count for every statement in implicit ordinal order.
        statement_operation_counts: Vec<u64>,
        /// Operations emitted while lowering the terminator.
        terminator_operation_count: u64,
    },
    /// A block introduced by a closed synthetic lowering rule.
    SyntheticBlock {
        /// Exact synthetic rule.
        rule: SyntheticBlockRuleV4,
    },
}

/// One Kernel-IR block in exact function block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockRecordV4 {
    kir_block_ordinal: u64,
    operation_count: u64,
    classification: BlockClassificationV4,
}

impl BlockRecordV4 {
    /// Constructs a semantic block record.
    pub fn semantic(
        kir_block_ordinal: u64,
        operation_count: u64,
        semantic_block_ordinal: u64,
        statement_operation_counts: Vec<u64>,
        terminator_operation_count: u64,
    ) -> Result<Self, LineageValidationErrorV4> {
        u64::try_from(statement_operation_counts.len()).map_err(|_| {
            LineageValidationErrorV4::LengthOverflow {
                context: "statement operation counts",
            }
        })?;
        let mut covered = 0_u64;
        for count in &statement_operation_counts {
            covered = covered.checked_add(*count).ok_or(
                LineageValidationErrorV4::ArithmeticOverflow {
                    resource: LineageResourceV4::Operations,
                },
            )?;
        }
        covered = covered.checked_add(terminator_operation_count).ok_or(
            LineageValidationErrorV4::ArithmeticOverflow {
                resource: LineageResourceV4::Operations,
            },
        )?;
        if covered != operation_count {
            return Err(LineageValidationErrorV4::BlockOperationCoverageMismatch {
                kir_block_ordinal,
                expected: operation_count,
                actual: covered,
            });
        }
        Ok(Self::semantic_from_wire_unchecked(
            kir_block_ordinal,
            operation_count,
            semantic_block_ordinal,
            statement_operation_counts,
            terminator_operation_count,
        ))
    }

    pub(crate) fn semantic_from_wire_unchecked(
        kir_block_ordinal: u64,
        operation_count: u64,
        semantic_block_ordinal: u64,
        statement_operation_counts: Vec<u64>,
        terminator_operation_count: u64,
    ) -> Self {
        Self {
            kir_block_ordinal,
            operation_count,
            classification: BlockClassificationV4::SemanticBlock {
                semantic_block_ordinal,
                statement_operation_counts,
                terminator_operation_count,
            },
        }
    }

    /// Constructs a synthetic block record.
    pub fn synthetic(
        kir_block_ordinal: u64,
        operation_count: u64,
        rule: SyntheticBlockRuleV4,
    ) -> Result<Self, LineageValidationErrorV4> {
        let expected = match rule {
            SyntheticBlockRuleV4::RuntimeAssertFailureTrap => 1,
        };
        if operation_count != expected {
            return Err(
                LineageValidationErrorV4::InvalidSyntheticBlockOperationCount {
                    kir_block_ordinal,
                    expected,
                    actual: operation_count,
                },
            );
        }
        Ok(Self::synthetic_from_wire_unchecked(
            kir_block_ordinal,
            operation_count,
            rule,
        ))
    }

    pub(crate) const fn synthetic_from_wire_unchecked(
        kir_block_ordinal: u64,
        operation_count: u64,
        rule: SyntheticBlockRuleV4,
    ) -> Self {
        Self {
            kir_block_ordinal,
            operation_count,
            classification: BlockClassificationV4::SyntheticBlock { rule },
        }
    }

    /// Returns the exact Kernel-IR block ordinal.
    pub const fn kir_block_ordinal(&self) -> u64 {
        self.kir_block_ordinal
    }

    /// Returns the complete operation count of the Kernel-IR block.
    pub const fn operation_count(&self) -> u64 {
        self.operation_count
    }

    /// Returns the block classification.
    pub const fn classification(&self) -> &BlockClassificationV4 {
        &self.classification
    }

    /// Derives statement ordinals and cumulative operation spans on access.
    pub fn statement_operation_spans(&self) -> Option<StatementOperationSpansV4<'_>> {
        match &self.classification {
            BlockClassificationV4::SemanticBlock {
                statement_operation_counts,
                ..
            } => Some(StatementOperationSpansV4::new(statement_operation_counts)),
            BlockClassificationV4::SyntheticBlock { .. } => None,
        }
    }

    /// Derives the terminator's cumulative operation span on access.
    pub fn terminator_operation_span(&self) -> Option<TerminatorOperationSpanV4> {
        let BlockClassificationV4::SemanticBlock {
            statement_operation_counts,
            terminator_operation_count,
            ..
        } = &self.classification
        else {
            return None;
        };
        let first = statement_operation_counts
            .iter()
            .try_fold(0_u64, |total, count| total.checked_add(*count))
            .expect("validated statement operation counts do not overflow");
        Some(TerminatorOperationSpanV4::new(OperationSpanV4::new(
            first,
            *terminator_operation_count,
        )))
    }
}

/// One Kernel-IR function in exact module function order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionRecordV4 {
    kir_function_ordinal: u64,
    classification: FunctionClassificationV4,
    blocks: Vec<BlockRecordV4>,
}

impl FunctionRecordV4 {
    /// Constructs a semantic function-body record.
    pub fn semantic_body(
        kir_function_ordinal: u64,
        semantic_function_ordinal: u64,
        semantic_block_count: u64,
        blocks: Vec<BlockRecordV4>,
    ) -> Self {
        Self {
            kir_function_ordinal,
            classification: FunctionClassificationV4::SemanticBody {
                semantic_function_ordinal,
                semantic_block_count,
            },
            blocks,
        }
    }

    /// Constructs an f32 intrinsic declaration record.
    pub const fn f32_intrinsic_declaration(
        kir_function_ordinal: u64,
        intrinsic: F32IntrinsicV4,
    ) -> Self {
        Self {
            kir_function_ordinal,
            classification: FunctionClassificationV4::F32IntrinsicDeclaration(intrinsic),
            blocks: Vec::new(),
        }
    }

    /// Constructs a diagnostic trap declaration record.
    pub const fn diagnostic_trap_declaration(
        kir_function_ordinal: u64,
        trap: DiagnosticTrapKindV4,
    ) -> Self {
        Self {
            kir_function_ordinal,
            classification: FunctionClassificationV4::DiagnosticTrapDeclaration(trap),
            blocks: Vec::new(),
        }
    }

    /// Returns the exact Kernel-IR function ordinal.
    pub const fn kir_function_ordinal(&self) -> u64 {
        self.kir_function_ordinal
    }

    /// Returns the function classification.
    pub const fn classification(&self) -> FunctionClassificationV4 {
        self.classification
    }

    /// Returns blocks in exact Kernel-IR function order.
    pub fn blocks(&self) -> &[BlockRecordV4] {
        &self.blocks
    }
}

/// Mapping for one exported Kernel-IR kernel record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KernelRecordV4 {
    kernel_ordinal: u64,
    semantic_function_ordinal: u64,
    kir_function_ordinal: u64,
}

impl KernelRecordV4 {
    /// Constructs an exported-kernel mapping.
    pub const fn new(
        kernel_ordinal: u64,
        semantic_function_ordinal: u64,
        kir_function_ordinal: u64,
    ) -> Self {
        Self {
            kernel_ordinal,
            semantic_function_ordinal,
            kir_function_ordinal,
        }
    }

    /// Returns the exact Kernel-IR kernel ordinal.
    pub const fn kernel_ordinal(self) -> u64 {
        self.kernel_ordinal
    }

    /// Returns the source semantic-function ordinal.
    pub const fn semantic_function_ordinal(self) -> u64 {
        self.semantic_function_ordinal
    }

    /// Returns the corresponding Kernel-IR function ordinal.
    pub const fn kir_function_ordinal(self) -> u64 {
        self.kir_function_ordinal
    }
}

/// Exact aggregate counts committed by canonical V4 lineage.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LineageTotalsV4 {
    semantic_functions: u64,
    kir_functions: u64,
    kernels: u64,
    semantic_blocks: u64,
    synthetic_blocks: u64,
    statements: u64,
    terminators: u64,
    operations: u64,
}

impl LineageTotalsV4 {
    /// Returns the semantic-function count.
    pub const fn semantic_functions(self) -> u64 {
        self.semantic_functions
    }

    /// Returns the complete Kernel-IR function count.
    pub const fn kir_functions(self) -> u64 {
        self.kir_functions
    }

    /// Returns the exported-kernel count.
    pub const fn kernels(self) -> u64 {
        self.kernels
    }

    /// Returns the semantic-block count.
    pub const fn semantic_blocks(self) -> u64 {
        self.semantic_blocks
    }

    /// Returns the synthetic-block count.
    pub const fn synthetic_blocks(self) -> u64 {
        self.synthetic_blocks
    }

    /// Returns the semantic-statement count.
    pub const fn statements(self) -> u64 {
        self.statements
    }

    /// Returns the semantic-terminator span count.
    pub const fn terminators(self) -> u64 {
        self.terminators
    }

    /// Returns the complete Kernel-IR operation count.
    pub const fn operations(self) -> u64 {
        self.operations
    }

    pub(crate) const fn from_wire(values: [u64; 8]) -> Self {
        Self {
            semantic_functions: values[0],
            kir_functions: values[1],
            kernels: values[2],
            semantic_blocks: values[3],
            synthetic_blocks: values[4],
            statements: values[5],
            terminators: values[6],
            operations: values[7],
        }
    }

    pub(crate) fn total_blocks(self) -> Option<u64> {
        self.semantic_blocks.checked_add(self.synthetic_blocks)
    }
}

/// Complete typed V4 lineage content before canonical encoding.
///
/// Validation is structural and accounting-only. It does not compare MIR and
/// KIR operations; the configured exhaustive typed traversal must be performed
/// by a later owner-bearing integration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineageModelV4 {
    semantic_mir: CanonicalSemanticMirIdentityV4,
    kernel_ir: CanonicalKernelIrIdentityV4,
    configuration: LoweringConfigurationV4,
    totals: LineageTotalsV4,
    functions: Vec<FunctionRecordV4>,
    kernels: Vec<KernelRecordV4>,
}

impl LineageModelV4 {
    /// Constructs and structurally validates complete inert lineage content.
    pub fn new(
        semantic_mir: CanonicalSemanticMirIdentityV4,
        kernel_ir: CanonicalKernelIrIdentityV4,
        configuration: LoweringConfigurationV4,
        functions: Vec<FunctionRecordV4>,
        kernels: Vec<KernelRecordV4>,
    ) -> Result<Self, LineageValidationErrorV4> {
        let mut budget = WorkBudgetV4::unbounded();
        Self::new_with_budget(
            semantic_mir,
            kernel_ir,
            configuration,
            functions,
            kernels,
            &mut budget,
        )
    }

    pub(crate) fn new_with_budget(
        semantic_mir: CanonicalSemanticMirIdentityV4,
        kernel_ir: CanonicalKernelIrIdentityV4,
        configuration: LoweringConfigurationV4,
        functions: Vec<FunctionRecordV4>,
        kernels: Vec<KernelRecordV4>,
        budget: &mut WorkBudgetV4,
    ) -> Result<Self, LineageValidationErrorV4> {
        let mut value = Self {
            semantic_mir,
            kernel_ir,
            configuration,
            totals: LineageTotalsV4::default(),
            functions,
            kernels,
        };
        value.totals = value.derive_and_validate_totals(budget)?;
        Ok(value)
    }

    pub(crate) fn from_wire_with_budget(
        semantic_mir: CanonicalSemanticMirIdentityV4,
        kernel_ir: CanonicalKernelIrIdentityV4,
        configuration: LoweringConfigurationV4,
        declared_totals: LineageTotalsV4,
        functions: Vec<FunctionRecordV4>,
        kernels: Vec<KernelRecordV4>,
        budget: &mut WorkBudgetV4,
    ) -> Result<Self, LineageValidationErrorV4> {
        let value = Self::new_with_budget(
            semantic_mir,
            kernel_ir,
            configuration,
            functions,
            kernels,
            budget,
        )?;
        charge_validation_work(budget, 8)?;
        if value.totals != declared_totals {
            return Err(totals_mismatch(declared_totals, value.totals));
        }
        Ok(value)
    }

    /// Returns the canonical semantic-MIR content claim.
    pub const fn semantic_mir(&self) -> CanonicalSemanticMirIdentityV4 {
        self.semantic_mir
    }

    /// Returns the canonical Kernel-IR content claim.
    pub const fn kernel_ir(&self) -> CanonicalKernelIrIdentityV4 {
        self.kernel_ir
    }

    /// Returns the exact lowering configuration.
    pub const fn configuration(&self) -> LoweringConfigurationV4 {
        self.configuration
    }

    /// Returns exact derived aggregate counts.
    pub const fn totals(&self) -> LineageTotalsV4 {
        self.totals
    }

    /// Returns functions in exact Kernel-IR order.
    pub fn functions(&self) -> &[FunctionRecordV4] {
        &self.functions
    }

    /// Returns exported kernels in exact Kernel-IR kernel order.
    pub fn kernels(&self) -> &[KernelRecordV4] {
        &self.kernels
    }

    /// Revalidates all structural, coverage, and resource invariants.
    pub fn revalidate(&self) -> Result<(), LineageValidationErrorV4> {
        let mut budget = WorkBudgetV4::unbounded();
        self.revalidate_with_budget(&mut budget)
    }

    pub(crate) fn revalidate_with_budget(
        &self,
        budget: &mut WorkBudgetV4,
    ) -> Result<(), LineageValidationErrorV4> {
        let totals = self.derive_and_validate_totals(budget)?;
        charge_validation_work(budget, 8)?;
        if totals != self.totals {
            return Err(totals_mismatch(self.totals, totals));
        }
        Ok(())
    }

    fn derive_and_validate_totals(
        &self,
        budget: &mut WorkBudgetV4,
    ) -> Result<LineageTotalsV4, LineageValidationErrorV4> {
        charge_validation_work(budget, 7)?;
        self.configuration.limits.validate()?;
        self.validate_artifact_policy()?;
        let mut totals = LineageTotalsV4 {
            kir_functions: length_u64("functions", self.functions.len())?,
            kernels: length_u64("kernels", self.kernels.len())?,
            ..LineageTotalsV4::default()
        };

        let mut semantic_function_count = 0_u64;
        let mut f32_declarations = 0_u16;
        let mut diagnostic_declarations = 0_u8;
        for (function_index, function) in self.functions.iter().enumerate() {
            charge_validation_work(budget, 1)?;
            let expected = length_u64("function ordinal", function_index)?;
            if function.kir_function_ordinal != expected {
                return Err(LineageValidationErrorV4::NonCanonicalOrdinal {
                    context: "Kernel IR function",
                    expected,
                    actual: function.kir_function_ordinal,
                });
            }
            match function.classification {
                FunctionClassificationV4::SemanticBody {
                    semantic_function_ordinal,
                    semantic_block_count,
                } => {
                    if semantic_function_ordinal != semantic_function_count {
                        return Err(LineageValidationErrorV4::NonCanonicalOrdinal {
                            context: "semantic function",
                            expected: semantic_function_count,
                            actual: semantic_function_ordinal,
                        });
                    }
                    semantic_function_count = checked_add(
                        LineageResourceV4::SemanticFunctions,
                        semantic_function_count,
                        1,
                    )?;
                    self.validate_semantic_function(
                        function,
                        semantic_function_ordinal,
                        semantic_block_count,
                        &mut totals,
                        budget,
                    )?;
                }
                FunctionClassificationV4::F32IntrinsicDeclaration(intrinsic) => {
                    if !function.blocks.is_empty() {
                        return Err(LineageValidationErrorV4::DeclarationHasBlocks {
                            kir_function_ordinal: function.kir_function_ordinal,
                        });
                    }
                    let bit = 1_u16 << (intrinsic as u8);
                    if f32_declarations & bit != 0 {
                        return Err(LineageValidationErrorV4::DuplicateF32Declaration(intrinsic));
                    }
                    f32_declarations |= bit;
                }
                FunctionClassificationV4::DiagnosticTrapDeclaration(trap) => {
                    if !function.blocks.is_empty() {
                        return Err(LineageValidationErrorV4::DeclarationHasBlocks {
                            kir_function_ordinal: function.kir_function_ordinal,
                        });
                    }
                    let bit = match trap {
                        DiagnosticTrapKindV4::RuntimeAssertFailure => 1,
                    };
                    if diagnostic_declarations & bit != 0 {
                        return Err(LineageValidationErrorV4::DuplicateDiagnosticDeclaration(
                            trap,
                        ));
                    }
                    diagnostic_declarations |= bit;
                }
            }
        }
        totals.semantic_functions = semantic_function_count;
        let has_diagnostic_declaration = diagnostic_declarations != 0;
        let has_synthetic_diagnostic_reference = totals.synthetic_blocks != 0;
        if has_diagnostic_declaration != has_synthetic_diagnostic_reference {
            return Err(LineageValidationErrorV4::DeclarationReferenceMismatch {
                context: "runtime-assert diagnostic trap",
                declaration_present: has_diagnostic_declaration,
                reference_present: has_synthetic_diagnostic_reference,
            });
        }
        self.validate_kernels(semantic_function_count, budget)?;
        self.enforce_limits(totals, budget)?;
        Ok(totals)
    }

    fn validate_artifact_policy(&self) -> Result<(), LineageValidationErrorV4> {
        let accepted = match self.configuration.mode {
            LineagePolicyModeV4::ProductionSemanticMirV3ToKernelIrV6 => {
                self.semantic_mir.wire_version == SemanticMirCanonicalWireVersionV4::V3
                    && self.kernel_ir.wire_version == KernelIrCanonicalWireVersionV4::V6
                    && self.kernel_ir.scheme
                        == KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV6Sha256PolicyV1
                    && self.configuration.checked_arithmetic
                        == CheckedArithmeticRefinementPolicyV4::SemanticMirV3ToKernelIrV6CheckedV1
            }
            LineagePolicyModeV4::LegacyInertSemanticMirV2ToKernelIrV5 => {
                self.semantic_mir.wire_version == SemanticMirCanonicalWireVersionV4::V2
                    && self.kernel_ir.wire_version == KernelIrCanonicalWireVersionV4::V5
                    && self.kernel_ir.scheme
                        == KernelIrIdentitySchemeV4::VerifiedCanonicalKernelIrV5Sha256PolicyV1
                    && self.configuration.checked_arithmetic
                        == CheckedArithmeticRefinementPolicyV4::LegacyInertNoRefinementAuthority
            }
        };
        if accepted {
            Ok(())
        } else {
            Err(LineageValidationErrorV4::ArtifactVersionPolicyMismatch {
                mode: self.configuration.mode,
                semantic: self.semantic_mir.wire_version,
                kernel_ir: self.kernel_ir.wire_version,
            })
        }
    }

    fn validate_semantic_function(
        &self,
        function: &FunctionRecordV4,
        semantic_function_ordinal: u64,
        semantic_block_count: u64,
        totals: &mut LineageTotalsV4,
        budget: &mut WorkBudgetV4,
    ) -> Result<(), LineageValidationErrorV4> {
        let block_count = length_u64("function blocks", function.blocks.len())?;
        let mut kir_seen = fallible_bitmap(block_count, "Kernel IR block identities", budget)?;
        let mut semantic_seen =
            fallible_bitmap(semantic_block_count, "semantic block identities", budget)?;
        let mut observed_semantic_blocks = 0_u64;

        for block in &function.blocks {
            charge_validation_work(budget, 1)?;
            mark_unique(&mut kir_seen, block.kir_block_ordinal, "Kernel IR block")?;
            totals.operations = checked_add(
                LineageResourceV4::Operations,
                totals.operations,
                block.operation_count,
            )?;
            match &block.classification {
                BlockClassificationV4::SemanticBlock {
                    semantic_block_ordinal,
                    statement_operation_counts,
                    terminator_operation_count,
                } => {
                    mark_unique(
                        &mut semantic_seen,
                        *semantic_block_ordinal,
                        "semantic block",
                    )?;
                    observed_semantic_blocks =
                        checked_add(LineageResourceV4::Blocks, observed_semantic_blocks, 1)?;
                    totals.semantic_blocks =
                        checked_add(LineageResourceV4::Blocks, totals.semantic_blocks, 1)?;
                    totals.terminators =
                        checked_add(LineageResourceV4::Blocks, totals.terminators, 1)?;
                    totals.statements = checked_add(
                        LineageResourceV4::Statements,
                        totals.statements,
                        length_u64("statement spans", statement_operation_counts.len())?,
                    )?;
                    validate_semantic_block_counts(
                        semantic_function_ordinal,
                        *semantic_block_ordinal,
                        block.operation_count,
                        statement_operation_counts,
                        *terminator_operation_count,
                        budget,
                    )?;
                }
                BlockClassificationV4::SyntheticBlock { rule } => {
                    totals.synthetic_blocks =
                        checked_add(LineageResourceV4::Blocks, totals.synthetic_blocks, 1)?;
                    validate_synthetic_block(
                        semantic_function_ordinal,
                        block.kir_block_ordinal,
                        block.operation_count,
                        *rule,
                    )?;
                }
            }
        }
        if observed_semantic_blocks != semantic_block_count {
            return Err(LineageValidationErrorV4::SemanticBlockCountMismatch {
                semantic_function_ordinal,
                declared: semantic_block_count,
                actual: observed_semantic_blocks,
            });
        }
        Ok(())
    }

    fn validate_kernels(
        &self,
        semantic_function_count: u64,
        budget: &mut WorkBudgetV4,
    ) -> Result<(), LineageValidationErrorV4> {
        let mut seen_semantic =
            fallible_bitmap(semantic_function_count, "kernel semantic functions", budget)?;
        let kir_count = length_u64("functions", self.functions.len())?;
        let mut seen_kir = fallible_bitmap(kir_count, "kernel Kernel IR functions", budget)?;
        for (index, kernel) in self.kernels.iter().enumerate() {
            charge_validation_work(budget, 1)?;
            let expected = length_u64("kernel ordinal", index)?;
            if kernel.kernel_ordinal != expected {
                return Err(LineageValidationErrorV4::NonCanonicalOrdinal {
                    context: "kernel",
                    expected,
                    actual: kernel.kernel_ordinal,
                });
            }
            mark_unique(
                &mut seen_semantic,
                kernel.semantic_function_ordinal,
                "kernel semantic function",
            )?;
            mark_unique(
                &mut seen_kir,
                kernel.kir_function_ordinal,
                "kernel Kernel IR function",
            )?;
            let Some(function) = self
                .functions
                .get(to_usize(kernel.kir_function_ordinal, "kernel function")?)
            else {
                return Err(LineageValidationErrorV4::OrdinalOutOfRange {
                    context: "kernel Kernel IR function",
                    ordinal: kernel.kir_function_ordinal,
                    count: kir_count,
                });
            };
            match function.classification {
                FunctionClassificationV4::SemanticBody {
                    semantic_function_ordinal,
                    ..
                } if semantic_function_ordinal == kernel.semantic_function_ordinal => {}
                _ => {
                    return Err(LineageValidationErrorV4::KernelFunctionMismatch {
                        kernel_ordinal: kernel.kernel_ordinal,
                    });
                }
            }
        }
        Ok(())
    }

    fn enforce_limits(
        &self,
        totals: LineageTotalsV4,
        budget: &mut WorkBudgetV4,
    ) -> Result<(), LineageValidationErrorV4> {
        charge_validation_work(budget, 7)?;
        let limits = self.configuration.limits;
        let blocks = totals
            .total_blocks()
            .ok_or(LineageValidationErrorV4::ArithmeticOverflow {
                resource: LineageResourceV4::Blocks,
            })?;
        for (resource, actual) in [
            (
                LineageResourceV4::SemanticFunctions,
                totals.semantic_functions,
            ),
            (LineageResourceV4::KirFunctions, totals.kir_functions),
            (LineageResourceV4::Kernels, totals.kernels),
            (LineageResourceV4::Blocks, blocks),
            (LineageResourceV4::Statements, totals.statements),
            (LineageResourceV4::Operations, totals.operations),
        ] {
            let limit = limits.limit(resource);
            if actual > limit {
                return Err(LineageValidationErrorV4::LimitExceeded {
                    resource,
                    actual,
                    limit,
                });
            }
        }
        let work = totals
            .kir_functions
            .checked_add(totals.kernels)
            .and_then(|value| value.checked_add(blocks))
            .and_then(|value| value.checked_add(totals.statements))
            .and_then(|value| value.checked_add(totals.terminators))
            .ok_or(LineageValidationErrorV4::ArithmeticOverflow {
                resource: LineageResourceV4::Work,
            })?;
        let work_limit = limits.limit(LineageResourceV4::Work);
        if work > work_limit {
            return Err(LineageValidationErrorV4::LimitExceeded {
                resource: LineageResourceV4::Work,
                actual: work,
                limit: work_limit,
            });
        }
        Ok(())
    }
}

fn validate_semantic_block_counts(
    semantic_function_ordinal: u64,
    semantic_block_ordinal: u64,
    block_operation_count: u64,
    statement_operation_counts: &[u64],
    terminator_operation_count: u64,
    budget: &mut WorkBudgetV4,
) -> Result<(), LineageValidationErrorV4> {
    let mut next_operation = 0_u64;
    for operation_count in statement_operation_counts {
        charge_validation_work(budget, 1)?;
        next_operation = next_operation.checked_add(*operation_count).ok_or(
            LineageValidationErrorV4::ArithmeticOverflow {
                resource: LineageResourceV4::Operations,
            },
        )?;
        if next_operation > block_operation_count {
            return Err(LineageValidationErrorV4::OperationCoverageMismatch {
                semantic_function_ordinal,
                block_ordinal: semantic_block_ordinal,
                expected: block_operation_count,
                actual: next_operation,
            });
        }
    }
    charge_validation_work(budget, 1)?;
    next_operation = next_operation
        .checked_add(terminator_operation_count)
        .ok_or(LineageValidationErrorV4::ArithmeticOverflow {
            resource: LineageResourceV4::Operations,
        })?;
    if next_operation != block_operation_count {
        return Err(LineageValidationErrorV4::OperationCoverageMismatch {
            semantic_function_ordinal,
            block_ordinal: semantic_block_ordinal,
            expected: block_operation_count,
            actual: next_operation,
        });
    }
    Ok(())
}

fn validate_synthetic_block(
    semantic_function_ordinal: u64,
    kir_block_ordinal: u64,
    block_operation_count: u64,
    rule: SyntheticBlockRuleV4,
) -> Result<(), LineageValidationErrorV4> {
    let expected_count = match rule {
        SyntheticBlockRuleV4::RuntimeAssertFailureTrap => 1,
    };
    if block_operation_count != expected_count {
        return Err(LineageValidationErrorV4::InvalidSyntheticBlock {
            semantic_function_ordinal,
            kir_block_ordinal,
            rule,
        });
    }
    Ok(())
}

fn fallible_bitmap(
    count: u64,
    context: &'static str,
    budget: &mut WorkBudgetV4,
) -> Result<Vec<bool>, LineageValidationErrorV4> {
    charge_validation_work(budget, count)?;
    let count = to_usize(count, context)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| LineageValidationErrorV4::AllocationFailed { context })?;
    values.resize(count, false);
    Ok(values)
}

fn charge_validation_work(
    budget: &mut WorkBudgetV4,
    amount: u64,
) -> Result<(), LineageValidationErrorV4> {
    budget
        .charge(LineageWorkStageV4::StructuralValidation, amount)
        .map_err(LineageValidationErrorV4::from_work_budget)
}

fn mark_unique(
    seen: &mut [bool],
    ordinal: u64,
    context: &'static str,
) -> Result<(), LineageValidationErrorV4> {
    let count = length_u64(context, seen.len())?;
    let index = to_usize(ordinal, context)?;
    let Some(slot) = seen.get_mut(index) else {
        return Err(LineageValidationErrorV4::OrdinalOutOfRange {
            context,
            ordinal,
            count,
        });
    };
    if *slot {
        return Err(LineageValidationErrorV4::DuplicateOrdinal { context, ordinal });
    }
    *slot = true;
    Ok(())
}

fn checked_add(
    resource: LineageResourceV4,
    left: u64,
    right: u64,
) -> Result<u64, LineageValidationErrorV4> {
    left.checked_add(right)
        .ok_or(LineageValidationErrorV4::ArithmeticOverflow { resource })
}

fn length_u64(context: &'static str, length: usize) -> Result<u64, LineageValidationErrorV4> {
    u64::try_from(length).map_err(|_| LineageValidationErrorV4::LengthOverflow { context })
}

fn to_usize(value: u64, context: &'static str) -> Result<usize, LineageValidationErrorV4> {
    usize::try_from(value).map_err(|_| LineageValidationErrorV4::LengthOverflow { context })
}

/// Structural or resource failure in an inert V4 lineage model.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LineageValidationErrorV4 {
    /// A canonical artifact identity used the reserved all-zero digest.
    ZeroArtifactIdentity {
        /// Artifact kind.
        kind: ArtifactKindV4,
    },
    /// A canonical artifact claimed a zero-byte preimage.
    ZeroCanonicalLength {
        /// Artifact kind.
        kind: ArtifactKindV4,
    },
    /// A referenced canonical artifact exceeded its V4 storage policy.
    CanonicalLengthLimitExceeded {
        /// Artifact kind.
        kind: ArtifactKindV4,
        /// Claimed canonical bytes.
        actual: u64,
        /// V4 maximum canonical bytes.
        max: u64,
    },
    /// The lowering policy version is unsupported.
    UnsupportedPolicyVersion(u64),
    /// Artifact wire versions did not match the selected closed policy pair.
    ArtifactVersionPolicyMismatch {
        /// Selected production or legacy-inert policy.
        mode: LineagePolicyModeV4,
        /// Semantic-MIR wire version.
        semantic: SemanticMirCanonicalWireVersionV4,
        /// Kernel-IR wire version.
        kernel_ir: KernelIrCanonicalWireVersionV4,
    },
    /// A KIR identity scheme was paired with the wrong KIR wire version.
    KernelIrIdentitySchemeVersionMismatch {
        /// Exact identity scheme tag.
        scheme: KernelIrIdentitySchemeV4,
        /// Supplied KIR wire version.
        wire_version: KernelIrCanonicalWireVersionV4,
    },
    /// Structural validation work accounting overflowed.
    WorkOverflow {
        /// Stage at which accounting overflowed.
        stage: LineageWorkStageV4,
    },
    /// Structural validation exceeded its shared admission budget.
    WorkLimitExceeded {
        /// Stage that consumed the first rejected unit.
        stage: LineageWorkStageV4,
        /// Aggregate work after charging the rejected unit.
        actual: u64,
        /// Shared aggregate maximum.
        max: u64,
    },
    /// A represented lowering limit was zero.
    ZeroLimit {
        /// Affected resource.
        resource: LineageResourceV4,
    },
    /// A represented lowering resource exceeded its exact limit.
    LimitExceeded {
        /// Affected resource.
        resource: LineageResourceV4,
        /// Observed count.
        actual: u64,
        /// Configured maximum.
        limit: u64,
    },
    /// Resource accounting overflowed.
    ArithmeticOverflow {
        /// Affected resource.
        resource: LineageResourceV4,
    },
    /// A host length conversion overflowed.
    LengthOverflow {
        /// Stable field name.
        context: &'static str,
    },
    /// A bounded validation allocation failed.
    AllocationFailed {
        /// Stable allocation name.
        context: &'static str,
    },
    /// An ordinal did not match required canonical order.
    NonCanonicalOrdinal {
        /// Stable ordinal kind.
        context: &'static str,
        /// Required ordinal.
        expected: u64,
        /// Observed ordinal.
        actual: u64,
    },
    /// An identity ordinal was outside its declared collection.
    OrdinalOutOfRange {
        /// Stable ordinal kind.
        context: &'static str,
        /// Observed ordinal.
        ordinal: u64,
        /// Collection size.
        count: u64,
    },
    /// An identity ordinal appeared more than once.
    DuplicateOrdinal {
        /// Stable ordinal kind.
        context: &'static str,
        /// Duplicated ordinal.
        ordinal: u64,
    },
    /// A declaration incorrectly contained blocks.
    DeclarationHasBlocks {
        /// Kernel-IR function ordinal.
        kir_function_ordinal: u64,
    },
    /// An f32 intrinsic declaration appeared more than once.
    DuplicateF32Declaration(F32IntrinsicV4),
    /// A diagnostic declaration appeared more than once.
    DuplicateDiagnosticDeclaration(DiagnosticTrapKindV4),
    /// A semantic body's block count was incomplete or excessive.
    SemanticBlockCountMismatch {
        /// Source semantic-function ordinal.
        semantic_function_ordinal: u64,
        /// Declared semantic-block count.
        declared: u64,
        /// Observed semantic-block count.
        actual: u64,
    },
    /// Source spans did not exactly cover a Kernel-IR block's operations.
    OperationCoverageMismatch {
        /// Source semantic-function ordinal.
        semantic_function_ordinal: u64,
        /// Relevant block ordinal.
        block_ordinal: u64,
        /// Complete block operation count.
        expected: u64,
        /// Covered operation endpoint.
        actual: u64,
    },
    /// A standalone semantic block did not exactly cover its operations.
    BlockOperationCoverageMismatch {
        /// Kernel-IR block ordinal.
        kir_block_ordinal: u64,
        /// Complete block operation count.
        expected: u64,
        /// Sum of statement and terminator operation counts.
        actual: u64,
    },
    /// A standalone synthetic block had the wrong closed-rule operation count.
    InvalidSyntheticBlockOperationCount {
        /// Kernel-IR block ordinal.
        kir_block_ordinal: u64,
        /// Count required by the synthetic rule.
        expected: u64,
        /// Supplied count.
        actual: u64,
    },
    /// A synthetic block did not match its closed structural rule.
    InvalidSyntheticBlock {
        /// Owning source semantic-function ordinal.
        semantic_function_ordinal: u64,
        /// Kernel-IR block ordinal.
        kir_block_ordinal: u64,
        /// Claimed synthetic rule.
        rule: SyntheticBlockRuleV4,
    },
    /// A kernel did not map to the claimed semantic function body.
    KernelFunctionMismatch {
        /// Kernel ordinal.
        kernel_ordinal: u64,
    },
    /// A declared wire total differed from its exact reconstructed value.
    DeclaredTotalsMismatch {
        /// Stable total name.
        context: &'static str,
        /// Value from the canonical header.
        declared: u64,
        /// Value reconstructed from records.
        actual: u64,
    },
    /// A closed declaration's presence disagreed with structural references.
    DeclarationReferenceMismatch {
        /// Stable declaration kind.
        context: &'static str,
        /// Whether the declaration was present.
        declaration_present: bool,
        /// Whether a structural reference was present.
        reference_present: bool,
    },
}

impl LineageValidationErrorV4 {
    pub(crate) const fn from_work_budget(error: WorkBudgetErrorV4) -> Self {
        match error {
            WorkBudgetErrorV4::Overflow { stage } => Self::WorkOverflow { stage },
            WorkBudgetErrorV4::LimitExceeded { stage, actual, max } => {
                Self::WorkLimitExceeded { stage, actual, max }
            }
        }
    }
}

fn totals_mismatch(declared: LineageTotalsV4, actual: LineageTotalsV4) -> LineageValidationErrorV4 {
    for (context, declared, actual) in [
        (
            "semantic functions",
            declared.semantic_functions,
            actual.semantic_functions,
        ),
        (
            "Kernel IR functions",
            declared.kir_functions,
            actual.kir_functions,
        ),
        ("kernels", declared.kernels, actual.kernels),
        (
            "semantic blocks",
            declared.semantic_blocks,
            actual.semantic_blocks,
        ),
        (
            "synthetic blocks",
            declared.synthetic_blocks,
            actual.synthetic_blocks,
        ),
        ("statements", declared.statements, actual.statements),
        ("terminators", declared.terminators, actual.terminators),
        ("operations", declared.operations, actual.operations),
    ] {
        if declared != actual {
            return LineageValidationErrorV4::DeclaredTotalsMismatch {
                context,
                declared,
                actual,
            };
        }
    }
    unreachable!("caller established that totals differ")
}

impl fmt::Display for LineageValidationErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroArtifactIdentity { kind } => write!(formatter, "{kind:?} identity is zero"),
            Self::ZeroCanonicalLength { kind } => {
                write!(formatter, "{kind:?} canonical length is zero")
            }
            Self::CanonicalLengthLimitExceeded { kind, actual, max } => write!(
                formatter,
                "{kind:?} canonical length {actual} exceeds V4 limit {max}"
            ),
            Self::UnsupportedPolicyVersion(version) => {
                write!(formatter, "unsupported lowering policy version {version}")
            }
            Self::ArtifactVersionPolicyMismatch {
                mode,
                semantic,
                kernel_ir,
            } => write!(
                formatter,
                "artifact versions {semantic:?} -> {kernel_ir:?} do not match policy {mode:?}"
            ),
            Self::KernelIrIdentitySchemeVersionMismatch {
                scheme,
                wire_version,
            } => write!(
                formatter,
                "Kernel IR identity scheme {scheme:?} does not match {wire_version:?}"
            ),
            Self::WorkOverflow { stage } => {
                write!(formatter, "{stage:?} work accounting overflowed")
            }
            Self::WorkLimitExceeded { stage, actual, max } => write!(
                formatter,
                "{stage:?} raised aggregate work to {actual}, exceeding {max}"
            ),
            Self::ZeroLimit { resource } => write!(formatter, "{resource:?} limit is zero"),
            Self::LimitExceeded {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "{resource:?} count {actual} exceeds represented limit {limit}"
            ),
            Self::ArithmeticOverflow { resource } => {
                write!(formatter, "{resource:?} accounting overflowed")
            }
            Self::LengthOverflow { context } => {
                write!(formatter, "{context} length does not fit this host")
            }
            Self::AllocationFailed { context } => {
                write!(formatter, "{context} allocation failed")
            }
            Self::NonCanonicalOrdinal {
                context,
                expected,
                actual,
            } => write!(
                formatter,
                "{context} ordinal {actual} is not canonical; expected {expected}"
            ),
            Self::OrdinalOutOfRange {
                context,
                ordinal,
                count,
            } => write!(
                formatter,
                "{context} ordinal {ordinal} is outside collection of {count}"
            ),
            Self::DuplicateOrdinal { context, ordinal } => {
                write!(formatter, "duplicate {context} ordinal {ordinal}")
            }
            Self::DeclarationHasBlocks {
                kir_function_ordinal,
            } => write!(
                formatter,
                "Kernel IR declaration {kir_function_ordinal} contains blocks"
            ),
            Self::DuplicateF32Declaration(intrinsic) => {
                write!(formatter, "duplicate f32 declaration {intrinsic:?}")
            }
            Self::DuplicateDiagnosticDeclaration(trap) => {
                write!(formatter, "duplicate diagnostic declaration {trap:?}")
            }
            Self::SemanticBlockCountMismatch {
                semantic_function_ordinal,
                declared,
                actual,
            } => write!(
                formatter,
                "semantic function {semantic_function_ordinal} declares {declared} blocks but records {actual}"
            ),
            Self::OperationCoverageMismatch {
                semantic_function_ordinal,
                block_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "semantic function {semantic_function_ordinal} block {block_ordinal} covers operations through {actual}, expected {expected}"
            ),
            Self::BlockOperationCoverageMismatch {
                kir_block_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "Kernel IR block {kir_block_ordinal} covers {actual} operations, expected {expected}"
            ),
            Self::InvalidSyntheticBlockOperationCount {
                kir_block_ordinal,
                expected,
                actual,
            } => write!(
                formatter,
                "synthetic Kernel IR block {kir_block_ordinal} has {actual} operations, expected {expected}"
            ),
            Self::InvalidSyntheticBlock {
                semantic_function_ordinal,
                kir_block_ordinal,
                rule,
            } => write!(
                formatter,
                "semantic function {semantic_function_ordinal} Kernel IR block {kir_block_ordinal} does not match synthetic rule {rule:?}"
            ),
            Self::KernelFunctionMismatch { kernel_ordinal } => write!(
                formatter,
                "kernel {kernel_ordinal} does not map to its claimed semantic function body"
            ),
            Self::DeclaredTotalsMismatch {
                context,
                declared,
                actual,
            } => write!(
                formatter,
                "declared {context} total {declared} differs from reconstructed total {actual}"
            ),
            Self::DeclarationReferenceMismatch {
                context,
                declaration_present,
                reference_present,
            } => write!(
                formatter,
                "{context} declaration presence {declaration_present} differs from reference presence {reference_present}"
            ),
        }
    }
}

impl std::error::Error for LineageValidationErrorV4 {}
