//! Typed boundary for the exact primitive gfx942 matrix lowering.
//!
//! The genuine `DeviceMatrix` constructor and `multiply_accumulate` diagnostic
//! items now lower in the exact gfx942 wave64 context to a verified Kernel IR
//! matrix operation. This is one primitive operation, not a complete GEMM.
//!
//! No safe host code can construct the compiler-issued matrix capability:
//!
//! ```compile_fail
//! use fe2o3_device::DeviceMatrix;
//! let _matrix = DeviceMatrix::default();
//! ```
//!
//! The matrix operation is explicitly unsafe because all wave64 lanes must
//! participate in converged control flow:
//!
//! ```compile_fail
//! use fe2o3_device::{
//!     Bf16MfmaFragment, DeviceMatrix, F32AccumulatorFragment,
//! };
//!
//! fn rejected(matrix: &DeviceMatrix) {
//!     let _ = matrix.multiply_accumulate(
//!         Bf16MfmaFragment::ZERO,
//!         Bf16MfmaFragment::ZERO,
//!         F32AccumulatorFragment::ZERO,
//!     );
//! }
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
    Bf16MfmaFragment, DeviceMatrix, F32AccumulatorFragment,
};

use crate::contract::{TILE_K_V1, TILE_M_V1, TILE_N_V1, WAVE_LANES_V1};

const _: () = assert!(BF16_F32_MFMA_M == TILE_M_V1 as usize);
const _: () = assert!(BF16_F32_MFMA_N == TILE_N_V1 as usize);
const _: () = assert!(BF16_F32_MFMA_REDUCTION == TILE_K_V1 as usize);
const _: () = assert!(BF16_F32_MFMA_WAVE_LANES == WAVE_LANES_V1 as usize);

/// Executes exactly one existing BF16/FP32 `16x16x16` device-matrix step.
///
/// This function intentionally does not define lane-to-fragment mapping,
/// LDS data movement, GEMM loops, or output stores. The exact frontend path
/// recognizes compiler-created `matrix` authority and lowers this call to the
/// verified Kernel IR matrix operation. Fragments must still satisfy the
/// existing `fe2o3-device` contract.
///
/// # Safety
///
/// All 64 lanes of one wave64 must invoke this function in converged control
/// flow. `lhs` and `rhs` must be the V1 fragments for the same reduction tile,
/// and `accumulator` must be the calling lane's four FP32 accumulators for the
/// same output tile.
#[must_use]
pub unsafe fn accumulate_fragment_v1(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaFragment,
    rhs: Bf16MfmaFragment,
    accumulator: F32AccumulatorFragment,
) -> F32AccumulatorFragment {
    // SAFETY: the caller must satisfy the exact DeviceMatrix wave64 contract.
    unsafe { matrix.multiply_accumulate(lhs, rhs, accumulator) }
}
