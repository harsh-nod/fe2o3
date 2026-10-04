//! CPU references for linear operators in the pinned Ferric B3 graph.
//!
//! This package exposes no GPU compilation, scheduling, or execution capability.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[path = "../../tiled_gemm_general_v1/src/reference.rs"]
mod reference;

pub mod linear;
