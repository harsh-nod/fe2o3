#![cfg_attr(target_arch = "amdgpu", no_std)]

#[cfg(not(target_arch = "amdgpu"))]
pub mod harness;
#[cfg(not(feature = "structured-tile"))]
pub mod kernel;
#[cfg(feature = "structured-tile")]
#[path = "kernel_tile.rs"]
pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod target;
