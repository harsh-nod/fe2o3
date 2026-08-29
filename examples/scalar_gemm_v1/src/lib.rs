#![cfg_attr(target_arch = "amdgpu", no_std)]

#[cfg(not(target_arch = "amdgpu"))]
pub mod harness;
pub mod kernel;
