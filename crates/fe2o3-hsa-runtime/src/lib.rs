#![doc = include_str!("../README.md")]

mod api;
mod dispatch;
mod environment;
mod lds_gemm_resource_observation;
mod lifecycle;
mod row_softmax_resource_observation;
mod sys;
mod wave64_collectives_resource_observation;
mod workgroup_sync_resource_observation;

#[cfg(feature = "hardware-test-hooks")]
pub use dispatch::ReviewedHsaHardwareTestBufferV1;
pub use environment::{HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
pub use lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelSetV1, ReviewedHsaKernelV1};

/// Whether this build found reviewed HSA and HIP headers and runtime libraries.
pub const HSA_RUNTIME_AVAILABLE: bool = cfg!(fe2o3_hsa_runtime);
