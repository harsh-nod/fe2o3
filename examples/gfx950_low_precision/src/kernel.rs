//! Safe Rust fixed-shape gfx950 kernels.

#![allow(missing_docs)]

use fe2o3_device::{
    DisjointSlice, Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp4MfmaAMatrix,
    Gfx950Fp4MfmaBMatrix, Gfx950Fp8E4M3, Gfx950Fp8MfmaAMatrix, Gfx950Fp8MfmaBMatrix,
    Gfx950LdsTransposeTile, Gfx950Matrix, Gfx950Subgroup, Gfx950TransposeUninitialized, Index1D,
    KernelError, KernelResult, Math, StridedReadView2D, Tiled2D, Wave64, WaveLane, kernel, thread,
};

pub const GFX950_WORKGROUP: [u32; 3] = [64, 1, 1];
pub const GEMM_M: usize = 16;
pub const GEMM_N: usize = 16;
pub const GEMM_K: usize = 128;
pub const ATTENTION_TOKENS: usize = 16;
pub const VALUE_COLUMNS: usize = 16;
const ATTENTION_SCALE: f32 = 0.088_388_35;

fn decode_fp4_e2m1(bits: u8) -> f32 {
    let magnitude = match bits & 0x7 {
        0 => 0.0,
        1 => 0.5,
        2 => 1.0,
        3 => 1.5,
        4 => 2.0,
        5 => 3.0,
        6 => 4.0,
        _ => 6.0,
    };
    if bits & 0x8 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn decode_fp8_e4m3(bits: u8) -> f32 {
    let exponent = (bits >> 3) & 0xf;
    let mantissa = bits & 0x7;
    if exponent == 0xf && mantissa == 0x7 {
        return f32::NAN;
    }
    let magnitude = if exponent == 0 {
        f32::from(mantissa) / 512.0
    } else {
        let scale = match exponent {
            1 => 0.015_625,
            2 => 0.031_25,
            3 => 0.062_5,
            4 => 0.125,
            5 => 0.25,
            6 => 0.5,
            7 => 1.0,
            8 => 2.0,
            9 => 4.0,
            10 => 8.0,
            11 => 16.0,
            12 => 32.0,
            13 => 64.0,
            14 => 128.0,
            _ => 256.0,
        };
        (1.0 + f32::from(mantissa) / 8.0) * scale
    };
    if bits & 0x80 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

#[kernel(
    typed,
    namespace = "a0eea3d8ddfacc67564702e483104ca51205dd96ecbaaf7133b11ba3edfc767a",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn gfx950_fp4_gemm_rust(
    lhs: &[u8],
    rhs: &[u8],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
) -> KernelResult {
    if lhs.len() < GEMM_M * GEMM_K || rhs.len() < GEMM_K * GEMM_N || output.len() < GEMM_M * GEMM_N
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let output_tile = index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let lane = WaveLane::<Wave64>::current();
    let lhs =
        Gfx950Fp4MfmaAMatrix::row_major(lhs, 0, GEMM_M, GEMM_K, GEMM_K)?.load_m16k128(&lane, 0, 0);
    let rhs =
        Gfx950Fp4MfmaBMatrix::row_major(rhs, 0, GEMM_K, GEMM_N, GEMM_N)?.load_k128n16(&lane, 0, 0);
    let accumulator = Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1>::zero(&lane);
    let values = Gfx950Matrix::current()
        .multiply_accumulate_fp4(lhs, rhs, accumulator)
        .into_values();
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 0, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[0];
    }
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 1, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[1];
    }
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 2, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[2];
    }
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 3, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[3];
    }
    Ok(())
}

#[kernel(
    typed,
    namespace = "3ea42d5446e3c6f6b5acad812e25d16ca190b39ff6618b7fdfd41b93be8a83d8",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn gfx950_fp8_gemm_rust(
    lhs: &[u8],
    rhs: &[u8],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
) -> KernelResult {
    if lhs.len() < GEMM_M * GEMM_K || rhs.len() < GEMM_K * GEMM_N || output.len() < GEMM_M * GEMM_N
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let output_tile = index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let lane = WaveLane::<Wave64>::current();
    let lhs =
        Gfx950Fp8MfmaAMatrix::row_major(lhs, 0, GEMM_M, GEMM_K, GEMM_K)?.load_m16k128(&lane, 0, 0);
    let rhs =
        Gfx950Fp8MfmaBMatrix::row_major(rhs, 0, GEMM_K, GEMM_N, GEMM_N)?.load_k128n16(&lane, 0, 0);
    let accumulator = Gfx950F32AccumulatorFragment::<Gfx950Fp8E4M3>::zero(&lane);
    let values = Gfx950Matrix::current()
        .multiply_accumulate_fp8(lhs, rhs, accumulator)
        .into_values();
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 0, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[0];
    }
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 1, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[1];
    }
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 2, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[2];
    }
    if let Some(element) = output.get_tiled_2d_mut(&output_tile, 3, GEMM_M, GEMM_N, GEMM_N) {
        *element = values[3];
    }
    Ok(())
}

#[kernel(
    typed,
    namespace = "f28a4219a206fbf0561b55682d1dad0ead21f20f90ace5d25526325c47cbb427",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16))
)]
pub fn gfx950_fp4_attention_rust(
    query: &[u8],
    key: &[u8],
    value: &[u8],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
) -> KernelResult {
    if query.len() < ATTENTION_TOKENS * GEMM_K
        || key.len() < ATTENTION_TOKENS * GEMM_K
        || value.len() < ATTENTION_TOKENS * VALUE_COLUMNS
        || output.len() < ATTENTION_TOKENS * VALUE_COLUMNS
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let lane_column = index.get() % 16;
    let output_tile = index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let lane = WaveLane::<Wave64>::current();
    let query = Gfx950Fp4MfmaAMatrix::row_major(query, 0, ATTENTION_TOKENS, GEMM_K, GEMM_K)?
        .load_m16k128(&lane, 0, 0);
    let key = Gfx950Fp4MfmaAMatrix::row_major(key, 0, ATTENTION_TOKENS, GEMM_K, GEMM_K)?;
    let key = Gfx950LdsTransposeTile::<Gfx950Fp4E2M1, Gfx950TransposeUninitialized>::current(&lane)
        .stage_k_transposed(&key, 0, 0)
        .publish()
        .read_mfma_fragment();
    let accumulator = Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1>::zero(&lane);
    let scores = Gfx950Matrix::current()
        .multiply_accumulate_fp4(query, key, accumulator)
        .into_values();
    let value = StridedReadView2D::from_shared_slice(
        value,
        0,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    )?;
    let subgroup = Gfx950Subgroup::current();
    let math = Math::current();
    let maximum0 = subgroup.reduce_max_f32::<16>(scores[0] * ATTENTION_SCALE);
    let maximum1 = subgroup.reduce_max_f32::<16>(scores[1] * ATTENTION_SCALE);
    let maximum2 = subgroup.reduce_max_f32::<16>(scores[2] * ATTENTION_SCALE);
    let maximum3 = subgroup.reduce_max_f32::<16>(scores[3] * ATTENTION_SCALE);
    let probability0 = math.exp_f32(scores[0] * ATTENTION_SCALE - maximum0);
    let probability1 = math.exp_f32(scores[1] * ATTENTION_SCALE - maximum1);
    let probability2 = math.exp_f32(scores[2] * ATTENTION_SCALE - maximum2);
    let probability3 = math.exp_f32(scores[3] * ATTENTION_SCALE - maximum3);
    let normalized0 = probability0 / subgroup.reduce_sum_f32::<16>(probability0);
    let normalized1 = probability1 / subgroup.reduce_sum_f32::<16>(probability1);
    let normalized2 = probability2 / subgroup.reduce_sum_f32::<16>(probability2);
    let normalized3 = probability3 / subgroup.reduce_sum_f32::<16>(probability3);
    let mut result0 = 0.0;
    let mut result1 = 0.0;
    let mut result2 = 0.0;
    let mut result3 = 0.0;
    let mut token = 0_usize;
    while token < ATTENTION_TOKENS {
        let value = decode_fp4_e2m1(value.load_or(token, lane_column, 0));
        result0 += subgroup.broadcast_f32::<16>(normalized0, token as u32) * value;
        result1 += subgroup.broadcast_f32::<16>(normalized1, token as u32) * value;
        result2 += subgroup.broadcast_f32::<16>(normalized2, token as u32) * value;
        result3 += subgroup.broadcast_f32::<16>(normalized3, token as u32) * value;
        token += 1;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        0,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result0;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        1,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result1;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        2,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result2;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        3,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result3;
    }
    Ok(())
}

#[kernel(
    typed,
    namespace = "171c3ff39f3d819e0f56f53c691ac70bdaa9bcf506482d5c41e7dafc05adf6f5",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(16))
)]
pub fn gfx950_fp8_attention_rust(
    query: &[u8],
    key: &[u8],
    value: &[u8],
    mut output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>,
) -> KernelResult {
    if query.len() < ATTENTION_TOKENS * GEMM_K
        || key.len() < ATTENTION_TOKENS * GEMM_K
        || value.len() < ATTENTION_TOKENS * VALUE_COLUMNS
        || output.len() < ATTENTION_TOKENS * VALUE_COLUMNS
    {
        return Err(KernelError::InvalidArgument);
    }
    let index = thread::index_1d();
    let lane_column = index.get() % 16;
    let output_tile = index
        .checked_tiled_2d::<64, 16, 16, 4>()
        .ok_or(KernelError::OutOfBounds)?;
    let lane = WaveLane::<Wave64>::current();
    let query = Gfx950Fp8MfmaAMatrix::row_major(query, 0, ATTENTION_TOKENS, GEMM_K, GEMM_K)?
        .load_m16k128(&lane, 0, 0);
    let key = Gfx950Fp8MfmaAMatrix::row_major(key, 0, ATTENTION_TOKENS, GEMM_K, GEMM_K)?;
    let key = Gfx950LdsTransposeTile::<Gfx950Fp8E4M3, Gfx950TransposeUninitialized>::current(&lane)
        .stage_k_transposed(&key, 0, 0)
        .publish()
        .read_mfma_fragment();
    let accumulator = Gfx950F32AccumulatorFragment::<Gfx950Fp8E4M3>::zero(&lane);
    let scores = Gfx950Matrix::current()
        .multiply_accumulate_fp8(query, key, accumulator)
        .into_values();
    let value = StridedReadView2D::from_shared_slice(
        value,
        0,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    )?;
    let subgroup = Gfx950Subgroup::current();
    let math = Math::current();
    let maximum0 = subgroup.reduce_max_f32::<16>(scores[0] * ATTENTION_SCALE);
    let maximum1 = subgroup.reduce_max_f32::<16>(scores[1] * ATTENTION_SCALE);
    let maximum2 = subgroup.reduce_max_f32::<16>(scores[2] * ATTENTION_SCALE);
    let maximum3 = subgroup.reduce_max_f32::<16>(scores[3] * ATTENTION_SCALE);
    let probability0 = math.exp_f32(scores[0] * ATTENTION_SCALE - maximum0);
    let probability1 = math.exp_f32(scores[1] * ATTENTION_SCALE - maximum1);
    let probability2 = math.exp_f32(scores[2] * ATTENTION_SCALE - maximum2);
    let probability3 = math.exp_f32(scores[3] * ATTENTION_SCALE - maximum3);
    let normalized0 = probability0 / subgroup.reduce_sum_f32::<16>(probability0);
    let normalized1 = probability1 / subgroup.reduce_sum_f32::<16>(probability1);
    let normalized2 = probability2 / subgroup.reduce_sum_f32::<16>(probability2);
    let normalized3 = probability3 / subgroup.reduce_sum_f32::<16>(probability3);
    let mut result0 = 0.0;
    let mut result1 = 0.0;
    let mut result2 = 0.0;
    let mut result3 = 0.0;
    let mut token = 0_usize;
    while token < ATTENTION_TOKENS {
        let value = decode_fp8_e4m3(value.load_or(token, lane_column, 0));
        result0 += subgroup.broadcast_f32::<16>(normalized0, token as u32) * value;
        result1 += subgroup.broadcast_f32::<16>(normalized1, token as u32) * value;
        result2 += subgroup.broadcast_f32::<16>(normalized2, token as u32) * value;
        result3 += subgroup.broadcast_f32::<16>(normalized3, token as u32) * value;
        token += 1;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        0,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result0;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        1,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result1;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        2,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result2;
    }
    if let Some(element) = output.get_tiled_2d_mut(
        &output_tile,
        3,
        ATTENTION_TOKENS,
        VALUE_COLUMNS,
        VALUE_COLUMNS,
    ) {
        *element = result3;
    }
    Ok(())
}
