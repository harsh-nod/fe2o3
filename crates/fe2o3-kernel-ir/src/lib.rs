//! Target-neutral semantic kernel IR for fe2o3.
//!
//! The crate intentionally has no dependency on rustc, LLVM, or a GPU vendor.
//! Frontends construct this IR, target-independent passes verify and transform
//! it, and target backends lower it to their native representation.
//!
//! [`encode_module_v1`] and [`decode_module_v1`] preserve the original bounded
//! canonical wire representation. [`encode_module_v2`] adds synchronization,
//! LDS, and exact wave-width records; [`decode_module_v2`] accepts canonical V1
//! and V2 bytes. Decoding establishes wire well-formedness only; consumers must
//! call [`verify_module`] before relying on semantic invariants.

mod effect_extraction;
mod formal_memory_obligations;
mod ir;
mod region_effects;
mod types;
mod verify;
mod wire;

pub use effect_extraction::*;
pub use formal_memory_obligations::*;
pub use ir::*;
pub use region_effects::*;
pub use types::*;
pub use verify::*;
pub use wire::*;
