#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]

//! Fixed-shape Rust teaching kernels corresponding to the gfx950 advanced
//! attention HIP fixture.
//!
//! The attributed functions are real ordinary Rust source with independent
//! safe CPU references. They deliberately use conservative scalar algorithms
//! and carry no claim of compiler lowering, gfx950 ISA selection, hardware
//! execution, proof, performance, or model equivalence.

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;

/// Channels written by the recurrent and residual-mixing profiles.
pub const CHANNELS_V1: usize = 16;
/// History taps consumed by each KDA/GDN recurrence step.
pub const KDA_TAPS_V1: usize = 3;
/// Tokens in the fixed prefill recurrence.
pub const PREFILL_TOKENS_V1: usize = 8;
/// Tokens in the sparse and compressed-hybrid attention fixtures.
pub const ATTENTION_TOKENS_V1: usize = 16;
/// Reduction depth of each quantized attention score.
pub const HEAD_DIMENSION_V1: usize = 128;
/// Blocks considered by indexed sparse attention.
pub const SPARSE_BLOCKS_V1: usize = 4;
/// Tokens in each sparse-attention block.
pub const TOKENS_PER_BLOCK_V1: usize = 4;
/// Blocks retained by the content selector.
pub const SELECTED_BLOCKS_V1: usize = 2;
/// Tokens retained after block and token ranking.
pub const SELECTED_TOKENS_V1: usize = 3;
/// Residual depths, branches, and streams in the bounded mixing profiles.
pub const MIXING_STREAMS_V1: usize = 4;
/// Sinkhorn row/column normalization iterations.
pub const SINKHORN_ITERATIONS_V1: usize = 3;

/// Exact workgroup dimensions declared by every teaching kernel.
pub const GFX950_ADVANCED_ATTENTION_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Exact grid dimensions declared by every teaching kernel.
pub const GFX950_ADVANCED_ATTENTION_GRID_V1: [u32; 3] = [1, 1, 1];
/// Whether an authenticated MIR-to-Kernel-IR lowering profile exists.
pub const GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1: bool = false;
/// Current boundary of the Rust-first source package.
pub const GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1: &str = "the seven fixed-shape attributed kernels have no authenticated MIR-to-Kernel-IR profiles; gfx950 tensor/transpose lowering, finalization, runtime execution, proof, and performance evidence are pending";
