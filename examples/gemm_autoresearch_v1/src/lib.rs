#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! One mutable GEMM candidate and an immutable independent reference.

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;
