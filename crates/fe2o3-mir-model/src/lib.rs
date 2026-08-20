mod control_flow;
mod executable;
mod executable_wire;
mod mem2reg;
mod semantic_constant;
mod semantic_memory;
pub mod semantic_mir_v1;
mod semantic_type;
pub mod semantic_type_v2;

pub use control_flow::{
    MAX_MIR_CONTROL_FLOW_WORK_UNITS, MIR_CONTROL_FLOW_WORK_UNITS_PER_BLOCK, MirControlFlowAnalysis,
    MirControlFlowEdge, MirControlFlowError, analyze_mir_control_flow,
};
pub use executable::{
    EXECUTABLE_MIR_VERSION, GFX942_POINTER_ABIS, GFX942_TARGET_CPU, GFX942_TARGET_DATA_LAYOUT,
    GFX942_TARGET_FEATURES, GFX942_TARGET_TRIPLE, MAX_EXECUTABLE_ADDRESS_SPACE,
    MAX_EXECUTABLE_BLOCK_PARAMETERS, MAX_EXECUTABLE_BLOCKS, MAX_EXECUTABLE_CALL_ARGUMENTS,
    MAX_EXECUTABLE_CALLABLES, MAX_EXECUTABLE_EDGE_ARGUMENTS, MAX_EXECUTABLE_FIELDS,
    MAX_EXECUTABLE_FUNCTIONS, MAX_EXECUTABLE_IDENTITY_BYTES, MAX_EXECUTABLE_LOCALS,
    MAX_EXECUTABLE_PROJECTIONS, MAX_EXECUTABLE_SOURCE_FILE_BYTES, MAX_EXECUTABLE_STATEMENTS,
    MAX_EXECUTABLE_STATEMENTS_PER_BLOCK, MAX_EXECUTABLE_SWITCH_TARGETS, MAX_EXECUTABLE_TYPE_DEPTH,
    MAX_EXECUTABLE_TYPE_ITEMS, MAX_EXECUTABLE_TYPE_NODES, MAX_EXECUTABLE_TYPES,
    MAX_EXECUTABLE_VARIANTS, MirAggregateKind, MirAssertMessage, MirAuthorizedDeviceImport,
    MirBasicBlock, MirBinaryOp, MirBlockId, MirBlockParameter, MirBody, MirBodyForm, MirCall,
    MirCallAuthority, MirCallReturn, MirCallSignature, MirCallable, MirCallee, MirCastKind,
    MirConstant, MirConstantValue, MirEdge, MirExecutableModule, MirExecutableTarget,
    MirExecutableTargetProfile, MirExecutableValidationError, MirExecutableVersion,
    MirExternalCallRegistry, MirExternalCallReturn, MirExternalCallSignature, MirFunction,
    MirIntrinsic, MirLocalDecl, MirLocalId, MirLocalKind, MirOperand, MirPlace, MirPointerAbi,
    MirProjection, MirRvalue, MirSourceSpan, MirStatement, MirStatementKind, MirTerminator,
    MirTerminatorKind, MirTypeId, MirUnaryOp, MirUnwindAction, MirValueId,
    ValidatedMirExecutableModule,
};
pub use executable_wire::{
    MAX_EXECUTABLE_WIRE_BYTES, MirExecutableDecodeError, MirExecutableSemanticDigestV1,
};
pub use mem2reg::{
    MAX_MEM2REG_LIVENESS_STORAGE_ITEMS, MAX_MEM2REG_LIVENESS_WORK_UNITS, MAX_MEM2REG_OUTPUT_ITEMS,
    MirMem2RegError, MirMem2RegFunctionReport, MirMem2RegFunctionResourceReport, MirMem2RegReport,
    MirMem2RegResourceReport, promote_module_to_ssa, promote_module_to_ssa_with_registry,
    promote_module_to_ssa_with_registry_and_resources, promote_module_to_ssa_with_resources,
};
pub use semantic_constant::{
    MAX_CONSTANT_ALLOCATION_BYTES, MAX_CONSTANT_ALLOCATIONS, MAX_CONSTANT_GRAPH_DEPTH,
    MAX_CONSTANT_IDENTITY_BYTES, MAX_CONSTANT_RELOCATIONS, MAX_CONSTANT_TOTAL_BYTES,
    MAX_CONSTANT_WIRE_BYTES, MirAlignment, MirAllocationId, MirAllocationOrigin, MirByteOffset,
    MirConstantAllocation, MirConstantDecodeError, MirConstantIdentity, MirConstantRepresentation,
    MirConstantValidationError, MirInitializedMask, MirMemoryIdentity, MirPointerProvenance,
    MirPointerRelocation, MirPointerWidth, MirPromotedIdentity, MirSemanticConstantPool,
    MirStaticIdentity, MirSymbolIdentity,
};
pub use semantic_memory::{
    MAX_MEMORY_OPERATION_WIRE_BYTES, MirCopyNonOverlappingContract, MirElementCount,
    MirMemoryAccessContract, MirMemoryContractDecodeError, MirMemoryContractValidationError,
    MirMemoryPermission, MirOperationProvenance, MirOverlapContract, MirPointerDistanceContract,
    MirPointerDistanceResult, MirPointerDistanceUnit, MirPointerOperandContract,
    MirProvenanceRegion, MirSemanticMemoryOperation, MirVolatileAccessContract,
};
pub use semantic_type::{
    MirAddressSpace, MirAggregateLayout, MirEnumEncoding, MirEnumType, MirField, MirLayout,
    MirMutability, MirPadding, MirScalarType, MirSemanticType, MirStructType, MirTypeKind,
    MirTypeValidationError, MirVariant,
};
pub use semantic_type_v2::{
    PointerMetadataV2, ScalarValidityRangeV2, SemanticEnumEncodingV2, SemanticFieldV2,
    SemanticMutabilityV2, SemanticNichePathComponentV2, SemanticNicheSourceV2, SemanticScalarV2,
    SemanticTypeGraphBudgetsV2, SemanticTypeGraphBuilderV2, SemanticTypeGraphErrorV2,
    SemanticTypeGraphV2, SemanticTypeKindV2, SemanticTypeLayoutV2, SemanticTypeNodeIdV2,
    SemanticTypeNodeV2, SemanticVariantV2, UntrustedSemanticTypeGraphEncodingV2,
};
