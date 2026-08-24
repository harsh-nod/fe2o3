#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Safe ordinary-Rust source for the dynamic strided GEMM qualification kernel.
//!
//! The production compiler imports the attributed Rust MIR, projects and
//! verifies generic ranked-memory IR, lowers verified Kernel IR, admits formal
//! memory obligations, and emits gfx942 LLVM. The host executable separately
//! owns the documented unsafe module-load and launch boundary.

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;

/// Whether the attributed kernel contains only safe ordinary Rust.
pub const GENERAL_TILED_GEMM_SAFE_SOURCE_PRESENT_V1: bool = true;
/// Whether production compilation reaches verified Kernel IR.
pub const GENERAL_TILED_GEMM_SOURCE_TO_IR_SUPPORTED_V1: bool = true;
/// Whether production compilation reaches deterministic gfx942 LLVM.
pub const GENERAL_TILED_GEMM_SOURCE_LOWERING_SUPPORTED_V1: bool = true;
/// Whether the checked-in runner can materialize and execute a qualification HSACO.
pub const GENERAL_TILED_GEMM_QUALIFICATION_EXECUTION_SUPPORTED_V1: bool = true;
/// Whether the qualification artifact grants protected release authority.
pub const GENERAL_TILED_GEMM_PROTECTED_EXECUTION_SUPPORTED_V1: bool = false;

/// Remaining boundary between qualification execution and protected release.
pub const GENERAL_TILED_GEMM_PROTECTED_EXECUTION_BLOCKER_V1: &str = "protected Worker publication and artifact-currentness admission remain separate from the qualification runner";
