#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

/// Bounded Qwen3 BF16/FP32 GEMM and GEMV compiler profiles.
pub mod gemm;

/// Bounded Qwen3 pure and explicitly residual-fused RMSNorm compiler profiles.
pub mod rmsnorm;
