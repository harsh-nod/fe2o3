//! Bounded typed `amdgcn` to `pliron_llvm::llvm` lowering for gfx942.
//!
//! This crate constructs and recursively verifies a real Pliron LLVM graph. A
//! fresh owner-controlled export reconstructs canonical Handoff V2 worker input
//! from bounded live graph traversal, including graph-resident target, ABI,
//! attribute, global, CFG, and metadata policy. The retained source contributes
//! only identity-bound non-LLVM evidence and construction provenance; printer
//! text and process-local arena identity are never output authority.

mod graph_export;
mod graph_policy;
mod lower;
mod model;

#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod integration_test_support;

pub use lower::lower_amdgcn_to_pliron_llvm_v1;
pub use model::*;
