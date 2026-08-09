#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod control_flow_v1;
mod decode;
mod encode;
mod error;
mod kernel_contract_v1;
mod model;
mod monomorphization_dead_v1;

pub use control_flow_v1::{
    CONTROL_FLOW_CONTRACT_MAGIC_V1, CONTROL_FLOW_CONTRACT_VERSION_V1,
    CONTROL_FLOW_REGISTRATION_KIND_V1, CONTROL_FLOW_REGISTRATION_MAGIC_V1,
    CONTROL_FLOW_REGISTRATION_PREFIX_V1, CONTROL_FLOW_REGISTRATION_VERSION_V1,
    CanonicalCfgIdentityV1, ControlFlowContractV1, ControlFlowDecodeErrorV1, ControlFlowNodeIdV1,
    ControlFlowNodeKindV1, ControlFlowNodeV1, ControlFlowValidationErrorV1,
    FrontendIntegerSwitchCaseV1, FrontendIntegerSwitchTypeV1, FrontendSourceSpanV1,
    MAX_CONTROL_FLOW_CONTRACT_BYTES_V1, MAX_CONTROL_FLOW_EDGES_V1, MAX_CONTROL_FLOW_NODES_V1,
    MAX_INTEGER_SWITCH_CASES_V1, MAX_SOURCE_FILE_BYTES_V1, decode_control_flow_contract_v1,
    encode_control_flow_contract_v1,
};
pub use decode::decode_frontend_unit_v1;
pub use encode::{FRONTEND_UNIT_MAGIC_V1, FRONTEND_UNIT_VERSION_V1, encode_frontend_unit_v1};
pub use error::{DecodeError, ValidationError};
pub use kernel_contract_v1::{
    ASSEMBLY_EFFECT_ATOMIC_V1, ASSEMBLY_EFFECT_BARRIER_V1, ASSEMBLY_EFFECT_CONTROL_FLOW_V1,
    ASSEMBLY_EFFECT_READ_GLOBAL_V1, ASSEMBLY_EFFECT_READ_WORKGROUP_V1,
    ASSEMBLY_EFFECT_WRITE_GLOBAL_V1, ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1,
    ASSEMBLY_OPERAND_ADDRESS_V1, ASSEMBLY_OPERAND_IMMEDIATE_V1, ASSEMBLY_OPERAND_SGPR_V1,
    ASSEMBLY_OPERAND_VGPR_V1, ASSEMBLY_OPTION_NOMEM_V1, ASSEMBLY_OPTION_NOSTACK_V1,
    ASSEMBLY_OPTION_PRESERVES_FLAGS_V1, ASSEMBLY_OPTION_PURE_V1, ASSEMBLY_OPTION_READONLY_V1,
    FRONTEND_KERNEL_CONTRACT_MAGIC_V1, FRONTEND_KERNEL_CONTRACT_VERSION_V1, FrontendLaunchBoundsV1,
    FrontendUnsafeAssemblyDeclarationV1, FrontendUnsafeAssemblyTargetV1,
    FrontendWorkgroupDimensionsV1, KERNEL_FRONTEND_REGISTRATION_KIND_V1,
    KERNEL_FRONTEND_REGISTRATION_MAGIC_V1, KERNEL_FRONTEND_REGISTRATION_PREFIX_V1,
    KERNEL_FRONTEND_REGISTRATION_VERSION_V1, KernelFrontendContractDecodeErrorV1,
    KernelFrontendContractV1, KernelFrontendContractValidationErrorV1,
    MAX_FRONTEND_KERNEL_CONTRACT_BYTES_V1, decode_kernel_frontend_contract_v1,
    encode_kernel_frontend_contract_v1,
};
pub use model::{
    BasicBlockV1, BlockIdV1, FrontendUnitV1, FunctionIdentityV1, FunctionRoleV1,
    MAX_BLOCKS_PER_FUNCTION_V1, MAX_FUNCTION_NAME_BYTES_V1, MAX_FUNCTIONS_V1,
    MAX_PARAMETERS_PER_FUNCTION_V1, MAX_SUCCESSORS_PER_BLOCK_V1, MAX_TOTAL_BLOCKS_V1,
    MAX_UNIT_BYTES_V1, MonomorphizedFunctionV1, SourceFileIdentityV1, SourceLocationV1,
    StableTypeIdentityV1, TypedSignatureV1,
};
pub use monomorphization_dead_v1::{
    CONSTANT_FOLD_POLICY_VERSION_V1, ConstantFoldBinaryOpV1, ConstantFoldFailureV1,
    ConstantFoldInputV1, ConstantSwitchCaseV1, ConstantSwitchV1, DeadBranchContextV1,
    DeadBranchDecisionV1, FixedWidthIntegerV1, MAX_DEAD_BRANCH_DECISIONS_V1,
    MAX_DEAD_SUCCESSORS_PER_BRANCH_V1, MonomorphizationDeadEvidenceErrorV1,
    MonomorphizationDeadEvidenceIdentityV1, MonomorphizationDeadEvidenceV1, fold_binary_v1,
    prove_constant_switch_v1,
};
