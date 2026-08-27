#![forbid(unsafe_code)]
#![cfg_attr(target_arch = "amdgpu", no_std)]
#![deny(missing_docs)]

//! Ordinary attributed Rust source and independent CPU references for bounded
//! gfx950 ML systems kernels.
//!
//! The Rust source is the fe2o3 tutorial implementation. The adjacent HIP
//! program remains a separate compiler and ISA-validation companion.

#[cfg(all(
    target_arch = "amdgpu",
    not(any(
        feature = "kernel-moe-route",
        feature = "kernel-moe-expert-rank",
        feature = "kernel-combine-expert-ranks",
        feature = "kernel-speculative-transaction",
        feature = "kernel-qwen-ngram-gather",
        feature = "kernel-stage-gradient-shard",
        feature = "kernel-muon-update",
    ))
))]
compile_error!("an AMDGPU build must select exactly one gfx950 advanced-systems kernel feature");

#[cfg(all(
    target_arch = "amdgpu",
    any(
        all(
            feature = "kernel-moe-route",
            any(
                feature = "kernel-moe-expert-rank",
                feature = "kernel-combine-expert-ranks",
                feature = "kernel-speculative-transaction",
                feature = "kernel-qwen-ngram-gather",
                feature = "kernel-stage-gradient-shard",
                feature = "kernel-muon-update"
            )
        ),
        all(
            feature = "kernel-moe-expert-rank",
            any(
                feature = "kernel-combine-expert-ranks",
                feature = "kernel-speculative-transaction",
                feature = "kernel-qwen-ngram-gather",
                feature = "kernel-stage-gradient-shard",
                feature = "kernel-muon-update"
            )
        ),
        all(
            feature = "kernel-combine-expert-ranks",
            any(
                feature = "kernel-speculative-transaction",
                feature = "kernel-qwen-ngram-gather",
                feature = "kernel-stage-gradient-shard",
                feature = "kernel-muon-update"
            )
        ),
        all(
            feature = "kernel-speculative-transaction",
            any(
                feature = "kernel-qwen-ngram-gather",
                feature = "kernel-stage-gradient-shard",
                feature = "kernel-muon-update"
            )
        ),
        all(
            feature = "kernel-qwen-ngram-gather",
            any(
                feature = "kernel-stage-gradient-shard",
                feature = "kernel-muon-update"
            )
        ),
        all(
            feature = "kernel-stage-gradient-shard",
            feature = "kernel-muon-update"
        ),
    )
))]
compile_error!(
    "an AMDGPU build must not select more than one gfx950 advanced-systems kernel feature"
);

pub mod kernel;
#[cfg(not(target_arch = "amdgpu"))]
pub mod reference;

/// The ordinary Rust kernel sources are present and host-checked.
pub const GFX950_ADVANCED_SYSTEMS_RUST_SOURCE_PRESENT_V1: bool = true;
/// Whether all seven source roots use the production semantic lowering surface.
pub const GFX950_ADVANCED_SYSTEMS_SOURCE_LOWERING_SUPPORTED: bool = true;
/// Boundary not established by the production source-lowering and runtime suite.
pub const GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER: &str = "the retained production extraction, finalization, ISA inspection, and gfx950 numerical runs do not establish formal compiler refinement, protected publication authority, performance, distributed-runtime behavior, or full-model behavior";

/// Number of MoE tokens.
pub const TOKENS: usize = 16;
/// MoE input width.
pub const HIDDEN: usize = 128;
/// MoE output width.
pub const OUTPUT: usize = 16;
/// Routed expert count.
pub const EXPERTS: usize = 4;
/// Routed plus shared expert count.
pub const ALL_EXPERTS: usize = 5;
/// Routes retained per token.
pub const TOP_K: usize = 2;
/// Maximum compact routes per expert.
pub const DISPATCH_CAPACITY: usize = TOKENS * TOP_K;
/// Speculative candidates.
pub const CANDIDATES: usize = 8;
/// Draft tokens per candidate.
pub const DRAFT_STEPS: usize = 4;
/// Transactional state width.
pub const STATE_WIDTH: usize = 8;
/// N-gram query count.
pub const QUERIES: usize = 8;
/// N-gram key width.
pub const NGRAM: usize = 3;
/// Hash table slot count.
pub const TABLE_SIZE: usize = 16;
/// Muon matrix dimension.
pub const MUON_DIM: usize = 4;
/// Muon matrix element count.
pub const MUON_ELEMENTS: usize = MUON_DIM * MUON_DIM;
/// Number of deterministic gradient shards.
pub const GRADIENT_SHARDS: usize = 2;
/// Newton-Schulz iteration count.
pub const MUON_ITERATIONS: usize = 5;
/// Fixed learning rate.
pub const MUON_LEARNING_RATE: f32 = 0.05;
