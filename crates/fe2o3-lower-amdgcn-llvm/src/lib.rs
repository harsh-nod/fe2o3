//! Bounded typed `amdgcn` to `pliron_llvm::llvm` lowering for gfx942.
//!
//! This crate constructs and recursively verifies a real Pliron LLVM graph.
//! Target policy, kernel ABI, function attributes, module metadata, origins,
//! and obligations remain authoritative through the retained canonical typed
//! handoff, never through printer text or process-local arena identity.

mod graph_export;
mod graph_policy;
mod lower;
mod model;

pub use lower::lower_amdgcn_to_pliron_llvm_v1;
pub use model::*;
