//! Compile with rustc --test and matching cached fe2o3_mir_model/fe2o3_kernel_ir
//! dependencies. This does not compile the parent compiler crate or run Cargo.

extern crate fe2o3_kernel_ir;
extern crate fe2o3_mir_model;

#[path = "math/contract.rs"]
mod contract;
#[path = "../../../production_ranked_projection_v1/numerical_policy_v1/legacy_math.rs"]
mod legacy_math;
