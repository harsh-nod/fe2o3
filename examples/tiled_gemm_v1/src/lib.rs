#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Conservative host contract for the first gfx942 tiled BF16 GEMM slice.
//!
//! Host planning, deterministic inputs, and the scalar FP32 oracle are usable
//! now. GPU compilation and dispatch remain pending frontend integration.

pub mod contract;
pub mod inputs;
pub mod kernel_face;
pub mod oracle;

pub use contract::{
    EDGE_CASES_V1, EdgeCaseV1, ExpectedDecisionV1, LaunchDecisionV1, LaunchGeometryV1, PlanErrorV1,
    ShapeErrorV1, ShapeV1, TileOriginV1, plan_v1,
};
pub use inputs::{BF16_INPUT_PATTERN_V1, GeneratedInputsV1, generate_inputs_v1};
pub use oracle::{OracleErrorV1, tiled_gemm_oracle_v1};
