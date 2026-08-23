//! Dependency-light, bounded, canonical, inert MIR-to-KIR lineage V4.
//!
//! This crate defines a wire-level data model only. Decoding establishes
//! canonical structure and internally consistent resource accounting. It does
//! not authenticate a producer, prove that lowering occurred, inspect MIR or
//! KIR semantics, or grant compiler, verifier, artifact, publication, load, or
//! launch authority.

#![deny(missing_docs)]

mod codec;
mod kernel_ir_v6_identity;
mod model;

pub use codec::{
    InertCanonicalMirToKirLineageV4, LineageDecodeErrorV4, LineageDecodeLimitsV4,
    LineageEncodeErrorV4, MAX_LINEAGE_BYTES_V4, MIR_TO_KIR_LINEAGE_MAGIC_V4,
    MIR_TO_KIR_LINEAGE_VERSION_V4,
};
pub use kernel_ir_v6_identity::{
    CANONICAL_KERNEL_IR_MAGIC_V6, CANONICAL_KERNEL_IR_V6_HEADER_BYTES,
    CANONICAL_KERNEL_IR_VERSION_V6, KernelIrV6IdentityPreimageError,
    RecomputedCanonicalKernelIrV6Sha256PolicyV1Identity,
    VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_DOMAIN_V1,
    VERIFIED_CANONICAL_KERNEL_IR_V6_IDENTITY_POLICY_V1,
    recompute_verified_canonical_kernel_ir_v6_sha256_policy_v1,
};
pub use model::{
    ArtifactKindV4, BlockClassificationV4, BlockRecordV4,
    CHECKED_ARITHMETIC_EXTERNAL_OWNER_GATE_V4, CHECKED_ARITHMETIC_REFINEMENT_POLICY_VECTOR_V4,
    CanonicalKernelIrIdentityV4, CanonicalSemanticMirIdentityV4,
    CheckedArithmeticRefinementPolicyV4, CorrespondenceValidationPolicyV4,
    DiagnosticTrapDeclarationPolicyV4, DiagnosticTrapKindV4, F32IntrinsicDeclarationPolicyV4,
    F32IntrinsicV4, FunctionClassificationV4, FunctionRecordV4, KernelIrCanonicalWireVersionV4,
    KernelIrIdentitySchemeV4, KernelRecordV4, LineageModelV4, LineagePolicyModeV4,
    LineageResourceV4, LineageTotalsV4, LineageValidationErrorV4, LineageWorkStageV4,
    LineageWorkV4, LoweringConfigurationV4, LoweringResourceLimitsV4, LoweringTargetV4,
    MAX_CANONICAL_KERNEL_IR_BYTES_V4, MAX_CANONICAL_SEMANTIC_MIR_BYTES_V4, OperationSpanV4,
    RankedBoundsPolicyV4, SemanticMirCanonicalWireVersionV4, SemanticMirIdentitySchemeV4,
    StatementOperationSpanV4, StatementOperationSpansV4, SyntheticBlockRuleV4,
    TerminatorOperationSpanV4,
};
