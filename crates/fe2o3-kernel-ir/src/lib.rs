//! Target-neutral semantic kernel IR for fe2o3.
//!
//! The crate intentionally has no dependency on rustc, LLVM, or a GPU vendor.
//! Frontends construct this IR, target-independent passes verify and transform
//! it, and target backends lower it to their native representation.
//!
//! [`encode_module_v1`] and [`decode_module_v1`] provide a bounded canonical
//! wire representation. Decoding establishes wire well-formedness only;
//! consumers must call [`verify_module`] before relying on semantic invariants.

mod ir;
mod types;
mod verify;
mod wire;

pub use ir::*;
pub use types::*;
pub use verify::*;
pub use wire::*;
