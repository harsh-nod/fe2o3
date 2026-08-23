//! Typed boundary for future exact gfx942 matrix lowering.
//!
//! This host scaffold uses the existing matrix types, but does not claim that
//! their Rust ABI or target identity is authenticated by the frontend yet.
//!
//! No safe host code can construct the compiler-issued matrix capability:
//!
//! ```compile_fail
//! use fe2o3_device::DeviceMatrix;
//! let _matrix = DeviceMatrix::default();
//! ```
//!
//! The capability is intentionally not transferable between host threads:
//!
//! ```compile_fail
//! use fe2o3_device::DeviceMatrix;
//! fn require_send<T: Send>() {}
//! require_send::<DeviceMatrix>();
//! ```

use fe2o3_device::{
    BF16_F32_MFMA_M, BF16_F32_MFMA_N, BF16_F32_MFMA_REDUCTION, BF16_F32_MFMA_WAVE_LANES,
    Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMatrix, F32AccumulatorFragment,
    MfmaRowMajorXor4,
};

use crate::contract::{TILE_K_V1, TILE_M_V1, TILE_N_V1, WAVE_LANES_V1};

const _: () = assert!(BF16_F32_MFMA_M == TILE_M_V1 as usize);
const _: () = assert!(BF16_F32_MFMA_N == TILE_N_V1 as usize);
const _: () = assert!(BF16_F32_MFMA_REDUCTION == TILE_K_V1 as usize);
const _: () = assert!(BF16_F32_MFMA_WAVE_LANES == WAVE_LANES_V1 as usize);

/// Executes exactly one existing BF16/FP32 `16x16x16` device-matrix step.
///
/// This function intentionally does not define lane-to-fragment mapping, LDS
/// data movement, GEMM loops, output stores, target binding, or physical ABI
/// validation. A future authenticated frontend must retain those obligations
/// while lowering the existing `fe2o3-device` contract.
///
/// The compiler verifies that all lanes invoke this operation in converged
/// control flow with fragments from the same matrix operation.
#[must_use]
pub fn accumulate_fragment_v1<'wave>(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaAFragment<'wave, MfmaRowMajorXor4>,
    rhs: Bf16MfmaBFragment<'wave, MfmaRowMajorXor4>,
    accumulator: F32AccumulatorFragment<'wave>,
) -> F32AccumulatorFragment<'wave> {
    matrix.multiply_accumulate(lhs, rhs, accumulator)
}
