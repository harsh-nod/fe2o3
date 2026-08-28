#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Dynamic fused attention through the workload-neutral production pipeline.

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;

pub const FLASH_ATTENTION_SAFE_SOURCE_PRESENT_V1: bool = true;
pub const FLASH_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1: bool = false;
pub const FLASH_ATTENTION_QUALIFICATION_EXECUTION_SUPPORTED_V1: bool = false;
pub const FLASH_ATTENTION_SOURCE_LOWERING_BLOCKER_V1: &str =
    "FE2O3-RACE-002: dynamic-launch checked-tiled ownership is not yet proved";
