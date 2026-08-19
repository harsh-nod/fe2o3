#![forbid(unsafe_code)]
#![doc = "Typed AMDGCN to dialect-only Pliron LLVM lowering."]
//!
//! This crate owns one deliberately small lowering lane. V1 accepts a single
//! scalar `f32` kernel body with the exact operation sequence
//! `load(input), fadd(addend), store(output), return`. It constructs and
//! verifies real [`pliron_llvm`] operations without enabling that crate's
//! `llvm-sys` default feature.
//!
//! V2 translates executable instructions, values, alignments, strict-FP state,
//! and control flow from the live Pliron graph. The current Pliron LLVM dialect
//! does not represent the AMDGPU calling convention, target machine policy,
//! function target attributes, LLVM module metadata, origins, or obligations.
//! Those facts are combined only as a receipt- and policy-validated retained
//! [`fe2o3_llvm_handoff::Gfx942HandoffV1`] sidecar. Printer output and arena
//! pointer values are never identities.
//!
//! This lane does not compile, link, publish, load, or execute code.

mod extract_v2;
mod lower;
mod model;

pub use lower::{LoweredScalarKernelV1, lower_scalar_kernel_v1, lower_scalar_kernel_v2};
pub use model::*;
