#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Dynamic multi-row softmax through the workload-neutral production pipeline.

pub mod kernel;

pub const ROW_SOFTMAX_SAFE_SOURCE_PRESENT_V1: bool = true;
pub const ROW_SOFTMAX_SOURCE_LOWERING_SUPPORTED_V1: bool = true;
pub const ROW_SOFTMAX_QUALIFICATION_EXECUTION_SUPPORTED_V1: bool = true;
