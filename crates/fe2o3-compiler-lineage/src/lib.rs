#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod capability_evidence_v1;
mod capsule;
mod error;
mod instruction_selection_correspondence_v1;
mod machine_refinement_receipt_v1;
mod multi_root_correspondence_payload_v2;
mod multi_root_correspondence_payload_v3;
mod multi_root_proof_lineage_v3;
mod multi_root_proof_roster_v2;
mod multi_root_proof_roster_v3;
mod multi_root_target_lineage_v2;
mod post_llvm_stage_custody_v1;
mod proof_binding;
mod proof_binding_v4;
mod proof_owner_v5;
mod receipt;
mod semantic_to_llvm_v3;
mod structured_kir_to_llvm_v1;
mod target_lineage_v3;

pub use capability_evidence_v1::{
    EXPANDED_SOURCE_EVIDENCE_MAGIC_V2, ExpandedSourceRootV2,
    INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_MAGIC_V1,
    INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_VERSION_V1,
    InertCapabilityRefinementReceiptIdentityV1, InertCapabilityRefinementReceiptKindV1,
    InertCapabilityRefinementReceiptV1, InertExpandedSourceEvidenceV2,
    InertMultiRootStaticCapabilityEvidenceAssociationV1,
    InertStaticCapabilityEvidenceAssociationErrorV1,
    InertStaticCapabilityEvidenceAssociationIdentityV1,
    InertStaticCapabilityEvidenceAssociationInputsV1, InertStaticCapabilityEvidenceAssociationV1,
    MAX_CAPABILITY_REFINEMENT_RECEIPT_BYTES_V1,
    MAX_INERT_MULTI_ROOT_STATIC_CAPABILITY_EVIDENCE_BYTES_V1,
    MAX_INERT_STATIC_CAPABILITY_EVIDENCE_ASSOCIATION_BYTES_V1,
};
pub use capsule::{
    INERT_PRODUCTION_SEMANTIC_CAPSULE_MAGIC_V3, INERT_PRODUCTION_SEMANTIC_CAPSULE_VERSION_V3,
    InertProductionSemanticCapsuleIdentityV3, InertProductionSemanticCapsuleV3,
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_DECODE_OWNED_BYTES_V3,
    OrderedInertSemanticLineageReceiptsV3,
};
pub use error::{LineageDecodeErrorV3, LineageErrorV3};
pub use instruction_selection_correspondence_v1::*;
pub use machine_refinement_receipt_v1::*;
pub use multi_root_correspondence_payload_v2::{
    MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V2, MULTI_ROOT_CORRESPONDENCE_PAYLOAD_POLICY_V2,
    MULTI_ROOT_CORRESPONDENCE_PAYLOAD_VERSION_V2, MultiRootCorrespondenceBlockV2,
    MultiRootCorrespondenceFunctionRoleV2, MultiRootCorrespondenceFunctionV2,
    MultiRootCorrespondenceParameterV2, MultiRootCorrespondencePayloadErrorV2,
    MultiRootCorrespondencePayloadV2, MultiRootCorrespondenceStatementV2,
    MultiRootCorrespondenceSyntheticRuleV2, MultiRootCorrespondenceSyntheticV2,
    MultiRootCorrespondenceTerminatorV2,
};
pub use multi_root_correspondence_payload_v3::{
    MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3, MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V3,
    MultiRootCorrespondenceInputsV3, MultiRootCorrespondencePayloadErrorV3,
    MultiRootCorrespondencePayloadV3, MultiRootInductionKindV3,
};
pub use multi_root_proof_lineage_v3::{
    InertMultiRootProofLineageIdentityV3, InertMultiRootProofLineageV3,
    MAX_MULTI_ROOT_PROOF_LINEAGE_BYTES_V3, MULTI_ROOT_PROOF_LINEAGE_VERSION_V3,
    MultiRootProofLineageErrorV3,
};
pub use multi_root_proof_roster_v2::{
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V2, MULTI_ROOT_PROOF_ROSTER_POLICY_V2,
    MULTI_ROOT_PROOF_ROSTER_VERSION_V2, MultiRootCanonicalKirVersionV2,
    MultiRootNeutralKirIdentityV2, MultiRootProofRosterErrorV2, MultiRootProofRosterInputsV2,
    MultiRootProofRosterKindV2, MultiRootProofRosterRootInputV2, MultiRootProofRosterRootV2,
    MultiRootProofRosterTranscriptV2,
};
pub use multi_root_proof_roster_v3::{
    MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3, MULTI_ROOT_PROOF_ROSTER_POLICY_V3,
    MULTI_ROOT_PROOF_ROSTER_VERSION_V3, MultiRootCanonicalKirVersionV3,
    MultiRootNeutralKirIdentityV3, MultiRootProofRosterErrorV3, MultiRootProofRosterInputsV3,
    MultiRootProofRosterKindV3, MultiRootProofRosterRootInputV3, MultiRootProofRosterRootV3,
    MultiRootProofRosterTranscriptV3,
};
pub use multi_root_target_lineage_v2::{
    MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V2, MULTI_ROOT_TARGET_BINDING_MAGIC_V2,
    MULTI_ROOT_TARGET_BINDING_VERSION_V2, MultiRootTargetBindingInputsV2,
    MultiRootTargetBindingTranscriptV2, MultiRootTargetWorkgroupInputV2,
    MultiRootTargetWorkgroupV2,
};
pub use post_llvm_stage_custody_v1::{
    CheckedPostLlvmStageContentsV1, ExactCompilerStageContentIdentityV1,
    ExactLlvmPassOccurrenceContentsV1, ExactProductionLlvmPhaseContentsV1,
    FIXED_PRODUCTION_LLVM_PHASES_V1, FixedProductionLlvmPhaseV1, LlvmPassInvocationV1,
    LlvmPassIrUnitV1, LlvmPassOccurrenceCustodyV1, MAX_LLVM_PASS_INVOCATIONS_V1,
    MAX_LLVM_PASS_NESTING_DEPTH_V1, PostLlvmOccurrenceAuthenticationV1,
    PostLlvmPipelineOccurrenceTranscriptIdentityV1, PostLlvmPipelineOccurrenceTranscriptPartsV1,
    PostLlvmPipelineOccurrenceTranscriptV1, PostLlvmStageCustodyErrorV1,
    PostLlvmStageCustodyIdentityV1, PostLlvmStageCustodyPartsV1, PostLlvmStageCustodyV1,
    ProductionLlvmPhaseCustodyV1, check_exact_fixed_production_pipeline_contents_v1,
    check_exact_post_llvm_pipeline_occurrence_v1, check_exact_post_llvm_stage_contents_v1,
    decode_post_llvm_stage_custody_identity_v1,
};
pub use proof_binding::{
    INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3, INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3,
    InertLineageContentIdentityV3, InertProofBindingAssociationErrorV3,
    InertProofBindingAssociationInputsV3, InertProofBindingAssociationV3,
    MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V3,
};
pub use proof_binding_v4::{
    INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V4, INERT_PROOF_BINDING_ASSOCIATION_VERSION_V4,
    InertProofBindingAssociationErrorV4, InertProofBindingAssociationInputsV4,
    InertProofBindingAssociationV4, MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4,
    MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4,
};
pub use proof_owner_v5::{
    INERT_COMPILER_PROOF_OWNER_MAGIC_V5, INERT_COMPILER_PROOF_OWNER_VERSION_V5,
    InertCanonicalKernelIrV13ReceiptErrorV5, InertCanonicalKernelIrV13ReceiptIdentityV5,
    InertCanonicalKernelIrV13ReceiptV5, InertCompilerProofOwnerErrorV5,
    InertCompilerProofOwnerIdentityV5, InertCompilerProofOwnerInputsV5, InertCompilerProofOwnerV5,
    MAX_INERT_CANONICAL_KERNEL_IR_V13_RECEIPT_BYTES_V5, MAX_INERT_COMPILER_PROOF_OWNER_BYTES_V5,
};
pub use receipt::{
    InertAbiReceiptIdentityV3, InertAbiReceiptV3, InertAmdgpuLoweringReceiptIdentityV3,
    InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirIdentityV3,
    InertCanonicalSemanticMirReceiptV3, InertDataLayoutReceiptIdentityV3, InertDataLayoutReceiptV3,
    InertExportManifestReceiptIdentityV3, InertExportManifestReceiptV3,
    InertFinalCompilerModuleCommitmentIdentityV3, InertFinalCompilerModuleCommitmentReceiptV3,
    InertFormalMemoryReceiptIdentityV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptIdentityV3,
    InertKernelIrReceiptV3, InertMiddleEndReceiptIdentityV3, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptIdentityV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingReceiptIdentityV3, InertProofBindingReceiptV3,
    InertRustcIdentityInventoryReceiptIdentityV3, InertRustcIdentityInventoryReceiptV3,
    InertRustcPreflightPlanReceiptIdentityV3, InertRustcPreflightPlanReceiptV3,
    InertSemanticToLlvmReceiptIdentityV3, InertSemanticToLlvmReceiptV3,
    InertTargetBindingReceiptIdentityV3, InertTargetBindingReceiptV3,
    MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
};
pub use semantic_to_llvm_v3::{
    INERT_SEMANTIC_TO_LLVM_ASSOCIATION_MAGIC_V3, INERT_SEMANTIC_TO_LLVM_ASSOCIATION_VERSION_V3,
    InertSemanticToLlvmAssociationErrorV3, InertSemanticToLlvmAssociationInputsV3,
    InertSemanticToLlvmAssociationV3, InertSemanticToLlvmContentIdentityV3,
    MAX_INERT_SEMANTIC_TO_LLVM_ASSOCIATION_BYTES_V3,
};
pub use structured_kir_to_llvm_v1::{
    MAX_STRUCTURED_KIR_TO_LLVM_BLOCKS_V1, MAX_STRUCTURED_KIR_TO_LLVM_EDGES_V1,
    MAX_STRUCTURED_KIR_TO_LLVM_OPERATIONS_V1, MAX_STRUCTURED_KIR_TO_LLVM_VALUES_V1,
    StructuredKirBlockLoweringV1, StructuredKirControlEdgeLoweringV1, StructuredKirOperationKindV1,
    StructuredKirOperationLoweringV1, StructuredKirToLlvmDerivationErrorV1,
    StructuredKirToLlvmDerivationIdentityV1, StructuredKirToLlvmDerivationPartsV1,
    StructuredKirToLlvmDerivationV1, StructuredKirValueCarrierV1, StructuredKirValueLoweringV1,
    StructuredKirValueTypeV1, StructuredLlvmNumericalPolicyV1, StructuredLlvmOpcodeV1,
    StructuredLlvmTargetV1,
};
pub use target_lineage_v3::{
    ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3, DataLayoutTranscriptInputsV3,
    DataLayoutTranscriptV3, MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
    ProductionTargetLineageErrorV3, SemanticToLlvmAssociationInputsV3,
    SemanticToLlvmAssociationTranscriptV3, TargetBindingTranscriptInputsV3,
    TargetBindingTranscriptV3, TargetLineageClaimV3, TargetLineageIdentityV3,
    canonical_semantic_target_layout_transcript_v1, derive_semantic_target_layout_identity_v1,
};
