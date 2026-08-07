#![doc = include_str!("../README.md")]

mod api;
mod dispatch;
mod environment;
mod lifecycle;
mod sys;

pub use environment::{HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
pub use lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelV1};

/// Whether this build found reviewed HSA and HIP headers and runtime libraries.
pub const HSA_RUNTIME_AVAILABLE: bool = cfg!(fe2o3_hsa_runtime);
