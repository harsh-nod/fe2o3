#![forbid(unsafe_code)]

//! Safe ordinary-Rust source contract for a general conservative tiled GEMM.
//!
//! The attributed kernel is a positive source fixture. It is not currently a
//! source-lowered or executable GPU kernel. Unsupported and host paths fail
//! closed at the compiler-issued [`fe2o3_device::Gfx942TiledGemmWave64V1`]
//! capability boundary.

pub mod kernel;

/// Whether ordinary attributed safe Rust source is present and compile-tested.
pub const GENERAL_TILED_GEMM_SAFE_SOURCE_PRESENT_V1: bool = true;
/// Whether the general source currently reaches verified Kernel IR.
pub const GENERAL_TILED_GEMM_SOURCE_TO_IR_SUPPORTED_V1: bool = false;
/// Whether the general source currently lowers to a publishable GPU artifact.
pub const GENERAL_TILED_GEMM_SOURCE_LOWERING_SUPPORTED_V1: bool = false;
/// Whether the general source is authorized for protected GPU execution.
pub const GENERAL_TILED_GEMM_PROTECTED_EXECUTION_SUPPORTED_V1: bool = false;

/// Current integration boundary for the positive source fixture.
pub const GENERAL_TILED_GEMM_SOURCE_BLOCKER_V1: &str =
    "general tiled-GEMM semantic calls are not imported into verified Kernel IR";

/// Work remaining before the positive source can become execution authority.
pub const GENERAL_TILED_GEMM_SOURCE_BLOCKERS_V1: [&str; 5] = [
    GENERAL_TILED_GEMM_SOURCE_BLOCKER_V1,
    "the proof-required pipeline does not yet discharge the general source properties",
    "the safe phase intrinsics do not yet lower to gfx942 LDS, barriers, and MFMA",
    "the dynamic GEMM ABI and launch plan are not joined to protected publication",
    "the negative semantic corpus is not yet enforced by compiler diagnostics",
];
