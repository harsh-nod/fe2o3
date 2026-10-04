/// Number of MoE tokens.
pub const TOKENS: usize = 16;
/// Independent wave-owned systems instances in one launch.
pub const SYSTEM_BATCHES: usize = 16;
/// Independent 256-element combine batches in one launch.
pub const COMBINE_BATCHES: usize = 4;
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
