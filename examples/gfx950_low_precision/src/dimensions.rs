pub const GFX950_WORKGROUP: [u32; 3] = [256, 1, 1];
pub const GFX950_GRID: [u32; 3] = [4, 1, 1];
pub const GFX950_BATCHES: usize = 16;
pub const GEMM_M: usize = 16;
pub const GEMM_N: usize = 16;
pub const GEMM_K: usize = 128;
pub const ATTENTION_TOKENS: usize = 16;
pub const VALUE_COLUMNS: usize = 16;
