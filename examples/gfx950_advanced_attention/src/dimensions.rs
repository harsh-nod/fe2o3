/// Value channels written by the attention and residual-mixing profiles.
pub const CHANNELS_V1: usize = 16;
/// Key dimension of the fixed Kimi Delta Attention profile.
pub const KDA_KEY_DIMENSION_V1: usize = 16;
/// Value dimension of the fixed Kimi Delta Attention profile.
pub const KDA_VALUE_DIMENSION_V1: usize = 16;
/// FP32 elements in the logical `[K,V]` matrix state.
pub const KDA_STATE_ELEMENTS_V1: usize = KDA_KEY_DIMENSION_V1 * KDA_VALUE_DIMENSION_V1;
/// Tokens solved together by each exact WY/UT prefill chunk.
pub const KDA_CHUNK_TOKENS_V1: usize = 4;
/// Tokens in the fixed prefill profile.
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
/// Top-k token indices supplied by the DeepSeek Lightning Indexer boundary.
pub const DEEPSEEK_SPARSE_TOP_K_V1: usize = 4;
/// Invalid token sentinel accepted by the DeepSeek sparse-attention contract.
pub const DEEPSEEK_INVALID_TOKEN_V1: u32 = u32::MAX;
/// Residual depths, branches, and streams in the bounded mixing profiles.
pub const MIXING_STREAMS_V1: usize = 4;
/// Sinkhorn row/column normalization iterations.
pub const SINKHORN_ITERATIONS_V1: usize = 3;

/// Exact workgroup dimensions declared by every teaching kernel.
pub const GFX950_ADVANCED_ATTENTION_WORKGROUP_V1: [u32; 3] = [256, 1, 1];
/// Exact workgroup dimensions declared by both canonical matrix-state KDA kernels.
pub const GFX950_KDA_WORKGROUP_V2: [u32; 3] = [256, 1, 1];
/// Exact grid dimensions declared by every teaching kernel.
pub const GFX950_ADVANCED_ATTENTION_GRID_V1: [u32; 3] = [4, 1, 1];
/// Whether the eight source roots use the production semantic lowering surface.
pub const GFX950_ADVANCED_ATTENTION_SOURCE_LOWERING_SUPPORTED_V1: bool = true;
/// Boundary not established by the production source-lowering and runtime suite.
pub const GFX950_ADVANCED_ATTENTION_SOURCE_BLOCKER_V1: &str = "the retained production extraction, finalization, ISA inspection, and gfx950 numerical runs do not establish formal compiler refinement, protected publication authority, performance, or full-model behavior";
