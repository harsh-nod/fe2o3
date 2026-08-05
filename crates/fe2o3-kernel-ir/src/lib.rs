//! Target-neutral semantic kernel IR for fe2o3.
//!
//! The crate intentionally has no dependency on rustc, LLVM, or a GPU vendor.
//! Frontends construct this IR, target-independent passes verify and transform
//! it, and target backends lower it to their native representation.

mod ir;
mod types;
mod verify;

pub use ir::*;
pub use types::*;
pub use verify::*;
