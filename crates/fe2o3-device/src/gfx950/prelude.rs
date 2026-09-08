//! Canonical capability imports for gfx950 kernel source.

pub use super::{
    GFX950_LOW_PRECISION_CONTRACT_VERSION_V1, GFX950_MFMA_K, GFX950_MFMA_M, GFX950_MFMA_N,
    GFX950_MFMA_OPERAND_DWORDS, GFX950_MFMA_WAVE_LANES, GFX950_WAVE16_WIDTH,
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAFragment, Gfx950Fp4MfmaBFragment,
    Gfx950Fp8E4M3, Gfx950Fp8MfmaAFragment, Gfx950Fp8MfmaBFragment, Gfx950LdsTransposeTile,
    Gfx950MatrixViewError, Gfx950MfmaFormat, Gfx950MfmaFragment, Gfx950MfmaOperandA,
    Gfx950MfmaOperandB, Gfx950Subgroup, Gfx950TransposePublishTransition, Gfx950TransposePublished,
    Gfx950TransposeStaged, Gfx950TransposeUninitialized, GlobalGfx950Fp4MfmaAMatrix,
    GlobalGfx950Fp4MfmaBMatrix, GlobalGfx950Fp8MfmaAMatrix, GlobalGfx950Fp8MfmaBMatrix,
    GlobalGfx950MfmaMatrix, PolicyGfx950Matrix,
};
