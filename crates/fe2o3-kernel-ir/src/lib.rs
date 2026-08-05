//! Target-neutral semantic kernel IR for fe2o3.
//!
//! The crate intentionally has no dependency on rustc, LLVM, or a GPU vendor.
//! Frontends construct this IR, target-independent passes verify and transform
//! it, and target backends lower it to their native representation.
//!
//! [`encode_module_v1`] and [`decode_module_v1`] provide a bounded canonical
//! wire representation. Decoding establishes wire well-formedness only;
//! consumers must call [`verify_module`] before relying on semantic invariants.

mod effect_extraction;
mod ir;
mod region_effects;
mod types;
mod verify;
mod wire;

pub use effect_extraction::*;
pub use ir::*;
pub use region_effects::*;
pub use types::*;
pub use verify::*;
pub use wire::*;
