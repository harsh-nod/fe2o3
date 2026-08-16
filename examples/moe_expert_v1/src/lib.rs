#![forbid(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

//! Fixed source/proof vertical slice for host-scheduled MoE expert GEMM and
//! deterministic weighted combine.
//!
//! The exact profile consumes the route-ID order, permutation, inverse map,
//! capacity, and drop sentinel from `fe2o3-moe-top2-v1`. It adds explicit
//! caller-supplied route weights because the published routing ABI does not
//! produce gating weights. This crate grants no compiler, artifact, runtime,
//! protected-execution, or numerical authority.

pub mod contract;
pub mod kernel;
pub mod oracle;
pub mod pipeline;

pub use contract::*;
pub use oracle::{MoeExpertOracleV1, moe_expert_independent_oracle_v1};
pub use pipeline::{
    ExpertDispatchV1, MoeExpertExecutionV1, MoeExpertInputErrorV1,
    run_host_scheduled_moe_experts_v1,
};
