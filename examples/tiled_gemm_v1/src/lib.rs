#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Conservative host contract for the first gfx942 tiled BF16 GEMM slice.
//!
//! Host planning, deterministic finite-corpus inputs, validated bitwise host
//! evidence, and a general scalar FP32 recurrence are usable now. Exact target
//! and physical-ABI-bound matrix lowering, lane mapping, LDS movement, complete
//! GEMM loops, output stores, production export, GPU execution, and proofs
//! remain pending.

pub mod contract;
pub mod inputs;
pub mod kernel_face;
pub mod oracle;

pub use contract::{
    AdmittedTargetV1, EDGE_CASES_V1, EdgeCaseV1, ExpectedDecisionV1, LaunchDecisionV1,
    LaunchGeometryV1, PlanErrorV1, ShapeErrorV1, ShapeV1, TargetAdmissionErrorV1, TileOriginV1,
    admit_target_v1, exact_target_v1, plan_v1,
};
pub use inputs::{BF16_INPUT_PATTERN_V1, GeneratedInputsV1, generate_inputs_v1};
pub use oracle::{
    ArithmeticOracleErrorV1, EvidenceInputErrorV1, EvidenceOperandV1, ValidatedEvidenceInputsV1,
    tiled_gemm_arithmetic_oracle_v1, tiled_gemm_evidence_oracle_v1, validate_evidence_inputs_v1,
};
