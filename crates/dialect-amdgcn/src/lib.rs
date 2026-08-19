//! Compatibility facade for fe2o3's Pliron-independent AMDGPU target model.
//!
//! The implementation remains available under its historical `dialect_amdgcn`
//! crate name while `dialect-amdgcn` is reserved for a future Pliron dialect.

mod pliron_llvm_v1;

pub use fe2o3_amdgcn_model::*;
pub use pliron_llvm_v1::*;
