#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Ordinary attributed Rust source and independent CPU references for bounded
//! gfx950 ML systems kernels.
//!
//! The Rust source is the fe2o3 tutorial implementation. The adjacent HIP
//! program is a separate ISA-validation companion until the fe2o3 compiler has
//! an authenticated gfx950 low-precision lowering profile.

pub mod kernel;
pub mod reference;

/// The fixed source profile has no authenticated Rust-to-gfx950 lowering yet.
pub const GFX950_ADVANCED_SYSTEMS_SOURCE_LOWERING_SUPPORTED: bool = false;
/// The exact boundary that prevents the Rust source from claiming GPU evidence.
pub const GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER: &str = "gfx950 FP4/FP8 MFMA and gfx950 production target lowering are not authenticated by rustc-codegen-fe2o3";

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
