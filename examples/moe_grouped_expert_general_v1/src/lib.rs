#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Dynamic routed expert computation through the common production pipeline.

pub mod kernel;

pub const MOE_SAFE_SOURCE_PRESENT_V1: bool = true;
pub const MOE_SOURCE_LOWERING_SUPPORTED_V1: bool = true;
pub const MOE_QUALIFICATION_EXECUTION_SUPPORTED_V1: bool = true;
