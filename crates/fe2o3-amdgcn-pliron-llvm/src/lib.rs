#![forbid(unsafe_code)]
#![doc = "Typed AMDGCN to dialect-only Pliron LLVM lowering."]
//!
//! This crate owns one deliberately small lowering lane. V1 accepts a single
//! scalar `f32` kernel body with the exact operation sequence
//! `load(input), fadd(addend), store(output), return`. It constructs and
//! verifies real [`pliron_llvm`] operations without enabling that crate's
//! `llvm-sys` default feature.
//!
//! The current Pliron LLVM dialect does not represent the AMDGPU calling
//! convention, target machine policy, function target attributes, or LLVM
//! module metadata on its operations. Those facts remain authoritative in the
//! canonical [`fe2o3_llvm_handoff::Gfx942HandoffV1`] returned with the dialect
//! tree. Printer output and arena pointer values are never identities.
//!
//! This lane does not compile, link, publish, load, or execute code.

mod lower;
mod model;

pub use lower::{LoweredScalarKernelV1, lower_scalar_kernel_v1};
pub use model::*;
