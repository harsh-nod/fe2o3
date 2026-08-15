#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Exact source, CPU oracle, and proof-facing contract for masked Wave64 sums.
//!
//! Phase A defines one logical active-lane mask over a physically convergent
//! 64-lane wave. It includes ordinary attributed Rust kernel source, an exact
//! finite-corpus host oracle, and a Verus integer model. It does not claim
//! compiler authentication, source-to-machine correspondence, artifact
//! admission, or hardware execution.

pub mod contract;
pub mod kernel;
pub mod oracle;

pub use contract::{
    EMPTY_MASK_POLICY_V1, INACTIVE_LANE_OUTPUT_POLICY_V1, LaneOutputsV1,
    MAX_EXACT_INPUT_MAGNITUDE_V1, PHYSICAL_EXECUTION_POLICY_V1, WAVE64_LANES_V1, lane_is_active_v1,
    lane_outputs_v1,
};
pub use oracle::{
    CollectiveOutputV1, OracleErrorV1, OracleErrorV1OrMismatch, OracleStateV1, OutputMismatchV1,
    compare_wave64_collectives_v1, wave64_collectives_oracle_v1,
};
