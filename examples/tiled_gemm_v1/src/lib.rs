#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Conservative host contract for the first gfx942 tiled BF16 GEMM slice.
//!
//! Host planning, deterministic finite-corpus inputs, validated bitwise host
//! evidence, a general scalar FP32 recurrence, and exact AMD-calculator-pinned
//! register and XOR4 LDS staging maps are usable now. An ordinary attributed
//! Rust kernel expresses one fixed LDS/MFMA phase and disjoint output stores,
//! and is authenticated to the verified fixed Kernel IR. The path fails closed
//! before compiler descriptor publication and dedicated LLVM lowering.
//! Multi-phase production export, source-driven GPU execution, and
//! machine-level proofs remain pending.

pub mod contract;
pub mod general_plan;
pub mod general_reference;
pub mod inputs;
pub mod kernel;
pub mod kernel_face;
pub mod layout;
pub mod numerical_contract;
pub mod numerical_vectors;
pub mod oracle;
pub mod semantic_corpus;

pub use contract::{
    AdmittedTargetV1, EDGE_CASES_V1, EdgeCaseV1, ExpectedDecisionV1, LaunchDecisionV1,
    LaunchGeometryV1, PlanErrorV1, ShapeErrorV1, ShapeV1, TargetAdmissionErrorV1, TileOriginV1,
    admit_target_v1, exact_target_v1, plan_v1,
};
pub use general_plan::{
    GENERAL_GEMM_PLAN_SCHEMA_V1, GENERAL_GEMM_REFERENCE_SCHEDULE_V1, GeneralGemmPlanIdentityV1,
    GeneralGemmPlanV1, GeneralGemmRequestV1, GeneralLaunchLimitErrorV1, GeneralLaunchLimitsV1,
    GeneralPlanErrorV1, GeneralPlanLimitV1, GeneralStorageExtentsV1, plan_general_gemm_v1,
};
pub use general_reference::{
    GeneralReferenceErrorV1, GeneralReferenceResultV1, GeneralReferenceTraceV1,
    execute_general_reference_v1,
};
pub use inputs::{BF16_INPUT_PATTERN_V1, GeneratedInputsV1, generate_inputs_v1};
pub use kernel::{
    LDS_SLICE1_OPERAND_BYTES_V1, LDS_SLICE1_OPERAND_ELEMENTS_V1, LDS_SLICE1_SOURCE_BLOCKER_V1,
    LDS_SLICE1_SOURCE_BLOCKERS_V1, LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1,
    LDS_SLICE1_SOURCE_TO_IR_SUPPORTED_V1, LDS_SLICE1_TOTAL_BYTES_V1, LDS_SLICE1_WORKGROUP_V1,
};
pub use layout::{
    AMD_MATRIX_CALCULATOR_A_CSV_SHA256_V1, AMD_MATRIX_CALCULATOR_ARCHITECTURE_V1,
    AMD_MATRIX_CALCULATOR_B_CSV_SHA256_V1, AMD_MATRIX_CALCULATOR_C_CSV_SHA256_V1,
    AMD_MATRIX_CALCULATOR_COMMIT_V1, AMD_MATRIX_CALCULATOR_D_CSV_SHA256_V1,
    AMD_MATRIX_CALCULATOR_INSTRUCTION_V1, AMD_MATRIX_CALCULATOR_REPOSITORY_V1,
    ARegisterCoordinateV1, ARegisterLayoutV1, AccumulatorCoordinateV1, AccumulatorRegisterLayoutV1,
    BRegisterCoordinateV1, BRegisterLayoutV1, LdsLogicalCoordinateV1, LdsPhysicalCoordinateV1,
    MFMA_LAYOUT_COMPONENTS_V1, MFMA_LAYOUT_EXTENT_V1, MFMA_LAYOUT_LANES_V1, RowMajorXor4StagingV1,
};
pub use oracle::{
    ArithmeticOracleErrorV1, EvidenceInputErrorV1, EvidenceOperandV1, ValidatedEvidenceInputsV1,
    tiled_gemm_arithmetic_oracle_v1, tiled_gemm_evidence_oracle_v1, validate_evidence_inputs_v1,
};
pub use semantic_corpus::{
    GEMM_SEMANTIC_CORPUS_SCHEMA_V1, GENERAL_GEMM_SAFE_SOURCE_MODEL_V1, GemmFailureKindV1,
    GemmRequiredPropertyV1, GemmSemanticDiagnosticV1, GemmSemanticNegativeCaseV1,
    GemmVerificationStageV1, SEMANTIC_NEGATIVE_CORPUS_V1, SemanticMutationV1,
};
