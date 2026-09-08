//! Safe Rust fixed-shape gfx950 kernels.
//!
//! Read each entrypoint in five phases: validate the complete launch contract,
//! map the global thread to a Wave64-owned batch, build typed matrix views,
//! execute collective matrix or reduction operations, and publish through a
//! statically disjoint output view. Those explicit phases keep the safety
//! argument and the expected gfx950 ISA reviewable together.

#![allow(missing_docs)]

use fe2o3_device::{
    Blocked, DisjointWrite, Global, Index1D, KernelContext, KernelError, KernelResult, ReadOnly,
    StrictIeee, SubgroupWidth64, kernel,
};
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-fp4-attention",
    feature = "kernel-fp8-attention"
))]
use fe2o3_device::{Gfx950Fp4E2M1, Gfx950Fp8E4M3};

pub const GFX950_WORKGROUP: [u32; 3] = [256, 1, 1];
pub const GFX950_GRID: [u32; 3] = [4, 1, 1];
pub const GFX950_BATCHES: usize = 16;
pub const GEMM_M: usize = 16;
pub const GEMM_N: usize = 16;
pub const GEMM_K: usize = 128;
pub const ATTENTION_TOKENS: usize = 16;
pub const VALUE_COLUMNS: usize = 16;
#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-fp4-attention",
    feature = "kernel-fp8-attention"
))]
const ATTENTION_SCALE: f32 = 0.088_388_35;

#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-fp4-attention"))]
fn decode_fp4_e2m1(bits: u8) -> f32 {
    match bits & 0xf {
        0 => 0.0,
        1 => 0.5,
        2 => 1.0,
        3 => 1.5,
        4 => 2.0,
        5 => 3.0,
        6 => 4.0,
        7 => 6.0,
        8 => -0.0,
        9 => -0.5,
        10 => -1.0,
        11 => -1.5,
        12 => -2.0,
        13 => -3.0,
        14 => -4.0,
        _ => -6.0,
    }
}

#[cfg(any(
    not(target_arch = "amdgpu"),
    feature = "kernel-fp4-attention",
    feature = "kernel-fp8-attention"
))]
#[inline(always)]
fn global_load_2d_u8<Brand>(
    values: &Global<'_, u8, ReadOnly, Brand>,
    base: usize,
    row: usize,
    column: usize,
    stride: usize,
) -> u8 {
    row.checked_mul(stride)
        .and_then(|offset| base.checked_add(offset))
        .and_then(|offset| offset.checked_add(column))
        .and_then(|index| values.load(index))
        .unwrap_or(0)
}

#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-fp4-gemm"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
/// Computes 16 independent 16x16x128 packed-E2M1 GEMM tiles.
pub fn gfx950_fp4_gemm_rust(
    mut context: KernelContext<'_>,
    lhs: Global<'_, u8, ReadOnly>,
    rhs: Global<'_, u8, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
) -> KernelResult {
    // Validate all wave-shared inputs before any lane performs matrix work.
    if lhs.len() < GFX950_BATCHES * GEMM_M * GEMM_K
        || rhs.len() < GFX950_BATCHES * GEMM_K * GEMM_N
        || output.len() < GFX950_BATCHES * GEMM_M * GEMM_N
    {
        return Err(KernelError::InvalidArgument);
    }
    // Four Wave64 tiles per WG256 and four workgroups produce 16 batches.
    let index = context.invocation().index_1d();
    let batch = index.get() / 64;
    let policy = context.numerical_policy::<StrictIeee>();
    let values = context.with_workgroup(|workgroup| -> Result<[f32; 4], KernelError> {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let lhs_matrix = matrix.fp4_a_global_row_major(
                &lhs,
                batch.wrapping_mul(GEMM_M * GEMM_K),
                GEMM_M,
                GEMM_K,
                GEMM_K,
            )?;
            let lhs = lhs_matrix.load_m16k128(lane, 0, 0);
            let rhs_matrix = matrix.fp4_b_global_row_major(
                &rhs,
                batch.wrapping_mul(GEMM_K * GEMM_N),
                GEMM_K,
                GEMM_N,
                GEMM_N,
            )?;
            let rhs = rhs_matrix.load_k128n16(lane, 0, 0);
            // One native gfx950 MFMA covers the fixed K=128 reduction.
            let accumulator = matrix.fp4_zero_accumulator(lane);
            Ok(matrix
                .multiply_accumulate_fp4(lhs, rhs, accumulator)
                .into_values())
        })
    })?;
    // Each lane owns four accumulator elements; the blocked proof makes stores disjoint.
    let Some(output_block) = index.checked_block::<16, 4>() else {
        return Err(KernelError::OutOfBounds);
    };
    if !output.store_block(&output_block, 0, values[0])
        || !output.store_block(&output_block, 1, values[1])
        || !output.store_block(&output_block, 2, values[2])
        || !output.store_block(&output_block, 3, values[3])
    {
        return Err(KernelError::OutOfBounds);
    }
    Ok(())
}

#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-fp8-gemm"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
/// Computes 16 independent 16x16x128 E4M3 GEMM tiles.
pub fn gfx950_fp8_gemm_rust(
    mut context: KernelContext<'_>,
    lhs: Global<'_, u8, ReadOnly>,
    rhs: Global<'_, u8, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
) -> KernelResult {
    // Validate the launch-wide storage contract before entering MFMA code.
    if lhs.len() < GFX950_BATCHES * GEMM_M * GEMM_K
        || rhs.len() < GFX950_BATCHES * GEMM_K * GEMM_N
        || output.len() < GFX950_BATCHES * GEMM_M * GEMM_N
    {
        return Err(KernelError::InvalidArgument);
    }
    // A global Wave64 index selects one of the 16 independent output tiles.
    let index = context.invocation().index_1d();
    let batch = index.get() / 64;
    let policy = context.numerical_policy::<StrictIeee>();
    let values = context.with_workgroup(|workgroup| -> Result<[f32; 4], KernelError> {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            // Format-specific branded views keep E4M3 layout out of pointer arithmetic.
            let lhs_matrix = matrix.fp8_a_global_row_major(
                &lhs,
                batch.wrapping_mul(GEMM_M * GEMM_K),
                GEMM_M,
                GEMM_K,
                GEMM_K,
            )?;
            let lhs = lhs_matrix.load_m16k128(lane, 0, 0);
            let rhs_matrix = matrix.fp8_b_global_row_major(
                &rhs,
                batch.wrapping_mul(GEMM_K * GEMM_N),
                GEMM_K,
                GEMM_N,
                GEMM_N,
            )?;
            let rhs = rhs_matrix.load_k128n16(lane, 0, 0);
            // Accumulate the unified low-precision MFMA result in FP32.
            let accumulator = matrix.fp8_zero_accumulator(lane);
            Ok(matrix
                .multiply_accumulate_fp8(lhs, rhs, accumulator)
                .into_values())
        })
    })?;
    // Convert lane-local accumulator ownership into four proven-disjoint stores.
    let Some(output_block) = index.checked_block::<16, 4>() else {
        return Err(KernelError::OutOfBounds);
    };
    if !output.store_block(&output_block, 0, values[0])
        || !output.store_block(&output_block, 1, values[1])
        || !output.store_block(&output_block, 2, values[2])
        || !output.store_block(&output_block, 3, values[3])
    {
        return Err(KernelError::OutOfBounds);
    }
    Ok(())
}

#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-fp4-attention"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
/// Computes batched FP4 QK, stable softmax, and the FP32 PV projection.
pub fn gfx950_fp4_attention_rust(
    mut context: KernelContext<'_>,
    query: Global<'_, u8, ReadOnly>,
    key: Global<'_, u8, ReadOnly>,
    value: Global<'_, u8, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
) {
    // Invalid launch buffers abort the full wave before collective work begins.
    if query.len() < GFX950_BATCHES * ATTENTION_TOKENS * GEMM_K
        || key.len() < GFX950_BATCHES * ATTENTION_TOKENS * GEMM_K
        || value.len() < GFX950_BATCHES * ATTENTION_TOKENS * VALUE_COLUMNS
        || output.len() < GFX950_BATCHES * ATTENTION_TOKENS * VALUE_COLUMNS
    {
        fe2o3_device::trap();
    }
    // A global wave owns one head; lane modulo 16 selects the PV value column.
    let index = context.invocation().index_1d();
    let batch = index.get() / 64;
    let lane_column = index.get() % 16;
    let row_base = batch.wrapping_mul(ATTENTION_TOKENS);
    let value_base = batch.wrapping_mul(ATTENTION_TOKENS * VALUE_COLUMNS);
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let values = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        let transpose = wave16.transpose_tile::<Gfx950Fp4E2M1>();
        let staged = subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let Ok(key_matrix) = matrix.fp4_a_global_row_major(
                &key,
                0,
                GFX950_BATCHES * ATTENTION_TOKENS,
                GEMM_K,
                GEMM_K,
            ) else {
                fe2o3_device::trap();
            };
            transpose.stage_k_transposed(&key_matrix, row_base, 0)
        });
        let (workgroup, published) = staged.publish(workgroup);
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let scores = subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let Ok(query_matrix) = matrix.fp4_a_global_row_major(
                &query,
                0,
                GFX950_BATCHES * ATTENTION_TOKENS,
                GEMM_K,
                GEMM_K,
            ) else {
                fe2o3_device::trap();
            };
            let query = query_matrix.load_m16k128(lane, row_base, 0);
            let key = published.read_mfma_fragment(lane);
            let accumulator = matrix.fp4_zero_accumulator(lane);
            matrix
                .multiply_accumulate_fp4(query, key, accumulator)
                .into_values()
        });
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        // Subtract the row maximum before exponentiation to stabilize softmax.
        let maximum0 = wave16.reduce_max_f32(scores[0] * ATTENTION_SCALE);
        let maximum1 = wave16.reduce_max_f32(scores[1] * ATTENTION_SCALE);
        let maximum2 = wave16.reduce_max_f32(scores[2] * ATTENTION_SCALE);
        let maximum3 = wave16.reduce_max_f32(scores[3] * ATTENTION_SCALE);
        let probability0 = math.exp_f32(scores[0] * ATTENTION_SCALE - maximum0);
        let probability1 = math.exp_f32(scores[1] * ATTENTION_SCALE - maximum1);
        let probability2 = math.exp_f32(scores[2] * ATTENTION_SCALE - maximum2);
        let probability3 = math.exp_f32(scores[3] * ATTENTION_SCALE - maximum3);
        let normalized0 = probability0 / wave16.reduce_sum_f32(probability0);
        let normalized1 = probability1 / wave16.reduce_sum_f32(probability1);
        let normalized2 = probability2 / wave16.reduce_sum_f32(probability2);
        let normalized3 = probability3 / wave16.reduce_sum_f32(probability3);
        // Broadcast each key probability across its Wave16 row for the PV reduction.
        let mut result0 = 0.0;
        let mut result1 = 0.0;
        let mut result2 = 0.0;
        let mut result3 = 0.0;
        let value0 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            0,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 0) * value0;
        result1 += wave16.broadcast_f32(normalized1, 0) * value0;
        result2 += wave16.broadcast_f32(normalized2, 0) * value0;
        result3 += wave16.broadcast_f32(normalized3, 0) * value0;
        let value1 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            1,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 1) * value1;
        result1 += wave16.broadcast_f32(normalized1, 1) * value1;
        result2 += wave16.broadcast_f32(normalized2, 1) * value1;
        result3 += wave16.broadcast_f32(normalized3, 1) * value1;
        let value2 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            2,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 2) * value2;
        result1 += wave16.broadcast_f32(normalized1, 2) * value2;
        result2 += wave16.broadcast_f32(normalized2, 2) * value2;
        result3 += wave16.broadcast_f32(normalized3, 2) * value2;
        let value3 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            3,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 3) * value3;
        result1 += wave16.broadcast_f32(normalized1, 3) * value3;
        result2 += wave16.broadcast_f32(normalized2, 3) * value3;
        result3 += wave16.broadcast_f32(normalized3, 3) * value3;
        let value4 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            4,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 4) * value4;
        result1 += wave16.broadcast_f32(normalized1, 4) * value4;
        result2 += wave16.broadcast_f32(normalized2, 4) * value4;
        result3 += wave16.broadcast_f32(normalized3, 4) * value4;
        let value5 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            5,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 5) * value5;
        result1 += wave16.broadcast_f32(normalized1, 5) * value5;
        result2 += wave16.broadcast_f32(normalized2, 5) * value5;
        result3 += wave16.broadcast_f32(normalized3, 5) * value5;
        let value6 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            6,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 6) * value6;
        result1 += wave16.broadcast_f32(normalized1, 6) * value6;
        result2 += wave16.broadcast_f32(normalized2, 6) * value6;
        result3 += wave16.broadcast_f32(normalized3, 6) * value6;
        let value7 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            7,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 7) * value7;
        result1 += wave16.broadcast_f32(normalized1, 7) * value7;
        result2 += wave16.broadcast_f32(normalized2, 7) * value7;
        result3 += wave16.broadcast_f32(normalized3, 7) * value7;
        let value8 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            8,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 8) * value8;
        result1 += wave16.broadcast_f32(normalized1, 8) * value8;
        result2 += wave16.broadcast_f32(normalized2, 8) * value8;
        result3 += wave16.broadcast_f32(normalized3, 8) * value8;
        let value9 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            9,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 9) * value9;
        result1 += wave16.broadcast_f32(normalized1, 9) * value9;
        result2 += wave16.broadcast_f32(normalized2, 9) * value9;
        result3 += wave16.broadcast_f32(normalized3, 9) * value9;
        let value10 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            10,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 10) * value10;
        result1 += wave16.broadcast_f32(normalized1, 10) * value10;
        result2 += wave16.broadcast_f32(normalized2, 10) * value10;
        result3 += wave16.broadcast_f32(normalized3, 10) * value10;
        let value11 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            11,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 11) * value11;
        result1 += wave16.broadcast_f32(normalized1, 11) * value11;
        result2 += wave16.broadcast_f32(normalized2, 11) * value11;
        result3 += wave16.broadcast_f32(normalized3, 11) * value11;
        let value12 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            12,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 12) * value12;
        result1 += wave16.broadcast_f32(normalized1, 12) * value12;
        result2 += wave16.broadcast_f32(normalized2, 12) * value12;
        result3 += wave16.broadcast_f32(normalized3, 12) * value12;
        let value13 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            13,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 13) * value13;
        result1 += wave16.broadcast_f32(normalized1, 13) * value13;
        result2 += wave16.broadcast_f32(normalized2, 13) * value13;
        result3 += wave16.broadcast_f32(normalized3, 13) * value13;
        let value14 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            14,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 14) * value14;
        result1 += wave16.broadcast_f32(normalized1, 14) * value14;
        result2 += wave16.broadcast_f32(normalized2, 14) * value14;
        result3 += wave16.broadcast_f32(normalized3, 14) * value14;
        let value15 = decode_fp4_e2m1(global_load_2d_u8(
            &value,
            value_base,
            15,
            lane_column,
            VALUE_COLUMNS,
        ));
        result0 += wave16.broadcast_f32(normalized0, 15) * value15;
        result1 += wave16.broadcast_f32(normalized1, 15) * value15;
        result2 += wave16.broadcast_f32(normalized2, 15) * value15;
        result3 += wave16.broadcast_f32(normalized3, 15) * value15;
        [result0, result1, result2, result3]
    });
    // Publish four rows per lane through the same blocked ownership used by GEMM.
    let Some(output_block) = index.checked_block::<16, 4>() else {
        fe2o3_device::trap();
    };
    if !output.store_block(&output_block, 0, values[0])
        || !output.store_block(&output_block, 1, values[1])
        || !output.store_block(&output_block, 2, values[2])
        || !output.store_block(&output_block, 3, values[3])
    {
        fe2o3_device::trap();
    }
}

#[cfg(any(not(target_arch = "amdgpu"), feature = "kernel-fp8-attention"))]
#[kernel(
    typed,
    launch(required = [256, 1, 1], max = [256, 1, 1], max_grid = [4, 1, 1])
)]
/// Computes batched FP8 QK, stable softmax, and the FP32 PV projection.
pub fn gfx950_fp8_attention_rust(
    mut context: KernelContext<'_>,
    query: Global<'_, u8, ReadOnly>,
    key: Global<'_, u8, ReadOnly>,
    value: Global<'_, u8, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Blocked<Index1D, 16, 4>>>,
) {
    // Invalid launch buffers abort the full wave before collective work begins.
    if query.len() < GFX950_BATCHES * ATTENTION_TOKENS * GEMM_K
        || key.len() < GFX950_BATCHES * ATTENTION_TOKENS * GEMM_K
        || value.len() < GFX950_BATCHES * ATTENTION_TOKENS * VALUE_COLUMNS
        || output.len() < GFX950_BATCHES * ATTENTION_TOKENS * VALUE_COLUMNS
    {
        fe2o3_device::trap();
    }
    // A global wave owns one head; lane modulo 16 selects the PV value column.
    let index = context.invocation().index_1d();
    let batch = index.get() / 64;
    let lane_column = index.get() % 16;
    let row_base = batch.wrapping_mul(ATTENTION_TOKENS);
    let value_base = batch.wrapping_mul(ATTENTION_TOKENS * VALUE_COLUMNS);
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    let values = context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        let transpose = wave16.transpose_tile::<Gfx950Fp8E4M3>();
        let staged = subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let Ok(key_matrix) = matrix.fp8_a_global_row_major(
                &key,
                0,
                GFX950_BATCHES * ATTENTION_TOKENS,
                GEMM_K,
                GEMM_K,
            ) else {
                fe2o3_device::trap();
            };
            transpose.stage_k_transposed(&key_matrix, row_base, 0)
        });
        let (workgroup, published) = staged.publish(workgroup);
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let scores = subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let Ok(query_matrix) = matrix.fp8_a_global_row_major(
                &query,
                0,
                GFX950_BATCHES * ATTENTION_TOKENS,
                GEMM_K,
                GEMM_K,
            ) else {
                fe2o3_device::trap();
            };
            let query = query_matrix.load_m16k128(lane, row_base, 0);
            let key = published.read_mfma_fragment(lane);
            let accumulator = matrix.fp8_zero_accumulator(lane);
            matrix
                .multiply_accumulate_fp8(query, key, accumulator)
                .into_values()
        });
        let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
        // Keep maxima, exponentials, denominators, and final division in FP32.
        let maximum0 = wave16.reduce_max_f32(scores[0] * ATTENTION_SCALE);
        let maximum1 = wave16.reduce_max_f32(scores[1] * ATTENTION_SCALE);
        let maximum2 = wave16.reduce_max_f32(scores[2] * ATTENTION_SCALE);
        let maximum3 = wave16.reduce_max_f32(scores[3] * ATTENTION_SCALE);
        let probability0 = math.exp_f32(scores[0] * ATTENTION_SCALE - maximum0);
        let probability1 = math.exp_f32(scores[1] * ATTENTION_SCALE - maximum1);
        let probability2 = math.exp_f32(scores[2] * ATTENTION_SCALE - maximum2);
        let probability3 = math.exp_f32(scores[3] * ATTENTION_SCALE - maximum3);
        let normalized0 = probability0 / wave16.reduce_sum_f32(probability0);
        let normalized1 = probability1 / wave16.reduce_sum_f32(probability1);
        let normalized2 = probability2 / wave16.reduce_sum_f32(probability2);
        let normalized3 = probability3 / wave16.reduce_sum_f32(probability3);
        // Broadcast probabilities within each Wave16 row while accumulating PV.
        let mut result0 = 0.0;
        let mut result1 = 0.0;
        let mut result2 = 0.0;
        let mut result3 = 0.0;
        let bits0 = global_load_2d_u8(&value, value_base, 0, lane_column, VALUE_COLUMNS);
        let exponent0 = (bits0 >> 3_u8) & 0xf;
        let mantissa0 = bits0 & 0x7;
        let magnitude0 = if exponent0 == 0xf && mantissa0 == 0x7 {
            let nan_source = f32::from(mantissa0.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent0 == 0 {
            f32::from(mantissa0) / 512.0
        } else {
            let scale = if exponent0 < 8 {
                f32::from(1_u8.wrapping_shl(exponent0 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent0.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa0) / 8.0) * scale
        };
        let value0 = if bits0 & 0x80 == 0 {
            magnitude0
        } else {
            -magnitude0
        };
        result0 += wave16.broadcast_f32(normalized0, 0) * value0;
        result1 += wave16.broadcast_f32(normalized1, 0) * value0;
        result2 += wave16.broadcast_f32(normalized2, 0) * value0;
        result3 += wave16.broadcast_f32(normalized3, 0) * value0;
        let bits1 = global_load_2d_u8(&value, value_base, 1, lane_column, VALUE_COLUMNS);
        let exponent1 = (bits1 >> 3_u8) & 0xf;
        let mantissa1 = bits1 & 0x7;
        let magnitude1 = if exponent1 == 0xf && mantissa1 == 0x7 {
            let nan_source = f32::from(mantissa1.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent1 == 0 {
            f32::from(mantissa1) / 512.0
        } else {
            let scale = if exponent1 < 8 {
                f32::from(1_u8.wrapping_shl(exponent1 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent1.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa1) / 8.0) * scale
        };
        let value1 = if bits1 & 0x80 == 0 {
            magnitude1
        } else {
            -magnitude1
        };
        result0 += wave16.broadcast_f32(normalized0, 1) * value1;
        result1 += wave16.broadcast_f32(normalized1, 1) * value1;
        result2 += wave16.broadcast_f32(normalized2, 1) * value1;
        result3 += wave16.broadcast_f32(normalized3, 1) * value1;
        let bits2 = global_load_2d_u8(&value, value_base, 2, lane_column, VALUE_COLUMNS);
        let exponent2 = (bits2 >> 3_u8) & 0xf;
        let mantissa2 = bits2 & 0x7;
        let magnitude2 = if exponent2 == 0xf && mantissa2 == 0x7 {
            let nan_source = f32::from(mantissa2.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent2 == 0 {
            f32::from(mantissa2) / 512.0
        } else {
            let scale = if exponent2 < 8 {
                f32::from(1_u8.wrapping_shl(exponent2 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent2.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa2) / 8.0) * scale
        };
        let value2 = if bits2 & 0x80 == 0 {
            magnitude2
        } else {
            -magnitude2
        };
        result0 += wave16.broadcast_f32(normalized0, 2) * value2;
        result1 += wave16.broadcast_f32(normalized1, 2) * value2;
        result2 += wave16.broadcast_f32(normalized2, 2) * value2;
        result3 += wave16.broadcast_f32(normalized3, 2) * value2;
        let bits3 = global_load_2d_u8(&value, value_base, 3, lane_column, VALUE_COLUMNS);
        let exponent3 = (bits3 >> 3_u8) & 0xf;
        let mantissa3 = bits3 & 0x7;
        let magnitude3 = if exponent3 == 0xf && mantissa3 == 0x7 {
            let nan_source = f32::from(mantissa3.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent3 == 0 {
            f32::from(mantissa3) / 512.0
        } else {
            let scale = if exponent3 < 8 {
                f32::from(1_u8.wrapping_shl(exponent3 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent3.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa3) / 8.0) * scale
        };
        let value3 = if bits3 & 0x80 == 0 {
            magnitude3
        } else {
            -magnitude3
        };
        result0 += wave16.broadcast_f32(normalized0, 3) * value3;
        result1 += wave16.broadcast_f32(normalized1, 3) * value3;
        result2 += wave16.broadcast_f32(normalized2, 3) * value3;
        result3 += wave16.broadcast_f32(normalized3, 3) * value3;
        let bits4 = global_load_2d_u8(&value, value_base, 4, lane_column, VALUE_COLUMNS);
        let exponent4 = (bits4 >> 3_u8) & 0xf;
        let mantissa4 = bits4 & 0x7;
        let magnitude4 = if exponent4 == 0xf && mantissa4 == 0x7 {
            let nan_source = f32::from(mantissa4.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent4 == 0 {
            f32::from(mantissa4) / 512.0
        } else {
            let scale = if exponent4 < 8 {
                f32::from(1_u8.wrapping_shl(exponent4 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent4.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa4) / 8.0) * scale
        };
        let value4 = if bits4 & 0x80 == 0 {
            magnitude4
        } else {
            -magnitude4
        };
        result0 += wave16.broadcast_f32(normalized0, 4) * value4;
        result1 += wave16.broadcast_f32(normalized1, 4) * value4;
        result2 += wave16.broadcast_f32(normalized2, 4) * value4;
        result3 += wave16.broadcast_f32(normalized3, 4) * value4;
        let bits5 = global_load_2d_u8(&value, value_base, 5, lane_column, VALUE_COLUMNS);
        let exponent5 = (bits5 >> 3_u8) & 0xf;
        let mantissa5 = bits5 & 0x7;
        let magnitude5 = if exponent5 == 0xf && mantissa5 == 0x7 {
            let nan_source = f32::from(mantissa5.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent5 == 0 {
            f32::from(mantissa5) / 512.0
        } else {
            let scale = if exponent5 < 8 {
                f32::from(1_u8.wrapping_shl(exponent5 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent5.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa5) / 8.0) * scale
        };
        let value5 = if bits5 & 0x80 == 0 {
            magnitude5
        } else {
            -magnitude5
        };
        result0 += wave16.broadcast_f32(normalized0, 5) * value5;
        result1 += wave16.broadcast_f32(normalized1, 5) * value5;
        result2 += wave16.broadcast_f32(normalized2, 5) * value5;
        result3 += wave16.broadcast_f32(normalized3, 5) * value5;
        let bits6 = global_load_2d_u8(&value, value_base, 6, lane_column, VALUE_COLUMNS);
        let exponent6 = (bits6 >> 3_u8) & 0xf;
        let mantissa6 = bits6 & 0x7;
        let magnitude6 = if exponent6 == 0xf && mantissa6 == 0x7 {
            let nan_source = f32::from(mantissa6.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent6 == 0 {
            f32::from(mantissa6) / 512.0
        } else {
            let scale = if exponent6 < 8 {
                f32::from(1_u8.wrapping_shl(exponent6 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent6.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa6) / 8.0) * scale
        };
        let value6 = if bits6 & 0x80 == 0 {
            magnitude6
        } else {
            -magnitude6
        };
        result0 += wave16.broadcast_f32(normalized0, 6) * value6;
        result1 += wave16.broadcast_f32(normalized1, 6) * value6;
        result2 += wave16.broadcast_f32(normalized2, 6) * value6;
        result3 += wave16.broadcast_f32(normalized3, 6) * value6;
        let bits7 = global_load_2d_u8(&value, value_base, 7, lane_column, VALUE_COLUMNS);
        let exponent7 = (bits7 >> 3_u8) & 0xf;
        let mantissa7 = bits7 & 0x7;
        let magnitude7 = if exponent7 == 0xf && mantissa7 == 0x7 {
            let nan_source = f32::from(mantissa7.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent7 == 0 {
            f32::from(mantissa7) / 512.0
        } else {
            let scale = if exponent7 < 8 {
                f32::from(1_u8.wrapping_shl(exponent7 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent7.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa7) / 8.0) * scale
        };
        let value7 = if bits7 & 0x80 == 0 {
            magnitude7
        } else {
            -magnitude7
        };
        result0 += wave16.broadcast_f32(normalized0, 7) * value7;
        result1 += wave16.broadcast_f32(normalized1, 7) * value7;
        result2 += wave16.broadcast_f32(normalized2, 7) * value7;
        result3 += wave16.broadcast_f32(normalized3, 7) * value7;
        let bits8 = global_load_2d_u8(&value, value_base, 8, lane_column, VALUE_COLUMNS);
        let exponent8 = (bits8 >> 3_u8) & 0xf;
        let mantissa8 = bits8 & 0x7;
        let magnitude8 = if exponent8 == 0xf && mantissa8 == 0x7 {
            let nan_source = f32::from(mantissa8.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent8 == 0 {
            f32::from(mantissa8) / 512.0
        } else {
            let scale = if exponent8 < 8 {
                f32::from(1_u8.wrapping_shl(exponent8 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent8.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa8) / 8.0) * scale
        };
        let value8 = if bits8 & 0x80 == 0 {
            magnitude8
        } else {
            -magnitude8
        };
        result0 += wave16.broadcast_f32(normalized0, 8) * value8;
        result1 += wave16.broadcast_f32(normalized1, 8) * value8;
        result2 += wave16.broadcast_f32(normalized2, 8) * value8;
        result3 += wave16.broadcast_f32(normalized3, 8) * value8;
        let bits9 = global_load_2d_u8(&value, value_base, 9, lane_column, VALUE_COLUMNS);
        let exponent9 = (bits9 >> 3_u8) & 0xf;
        let mantissa9 = bits9 & 0x7;
        let magnitude9 = if exponent9 == 0xf && mantissa9 == 0x7 {
            let nan_source = f32::from(mantissa9.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent9 == 0 {
            f32::from(mantissa9) / 512.0
        } else {
            let scale = if exponent9 < 8 {
                f32::from(1_u8.wrapping_shl(exponent9 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent9.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa9) / 8.0) * scale
        };
        let value9 = if bits9 & 0x80 == 0 {
            magnitude9
        } else {
            -magnitude9
        };
        result0 += wave16.broadcast_f32(normalized0, 9) * value9;
        result1 += wave16.broadcast_f32(normalized1, 9) * value9;
        result2 += wave16.broadcast_f32(normalized2, 9) * value9;
        result3 += wave16.broadcast_f32(normalized3, 9) * value9;
        let bits10 = global_load_2d_u8(&value, value_base, 10, lane_column, VALUE_COLUMNS);
        let exponent10 = (bits10 >> 3_u8) & 0xf;
        let mantissa10 = bits10 & 0x7;
        let magnitude10 = if exponent10 == 0xf && mantissa10 == 0x7 {
            let nan_source = f32::from(mantissa10.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent10 == 0 {
            f32::from(mantissa10) / 512.0
        } else {
            let scale = if exponent10 < 8 {
                f32::from(1_u8.wrapping_shl(exponent10 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent10.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa10) / 8.0) * scale
        };
        let value10 = if bits10 & 0x80 == 0 {
            magnitude10
        } else {
            -magnitude10
        };
        result0 += wave16.broadcast_f32(normalized0, 10) * value10;
        result1 += wave16.broadcast_f32(normalized1, 10) * value10;
        result2 += wave16.broadcast_f32(normalized2, 10) * value10;
        result3 += wave16.broadcast_f32(normalized3, 10) * value10;
        let bits11 = global_load_2d_u8(&value, value_base, 11, lane_column, VALUE_COLUMNS);
        let exponent11 = (bits11 >> 3_u8) & 0xf;
        let mantissa11 = bits11 & 0x7;
        let magnitude11 = if exponent11 == 0xf && mantissa11 == 0x7 {
            let nan_source = f32::from(mantissa11.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent11 == 0 {
            f32::from(mantissa11) / 512.0
        } else {
            let scale = if exponent11 < 8 {
                f32::from(1_u8.wrapping_shl(exponent11 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent11.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa11) / 8.0) * scale
        };
        let value11 = if bits11 & 0x80 == 0 {
            magnitude11
        } else {
            -magnitude11
        };
        result0 += wave16.broadcast_f32(normalized0, 11) * value11;
        result1 += wave16.broadcast_f32(normalized1, 11) * value11;
        result2 += wave16.broadcast_f32(normalized2, 11) * value11;
        result3 += wave16.broadcast_f32(normalized3, 11) * value11;
        let bits12 = global_load_2d_u8(&value, value_base, 12, lane_column, VALUE_COLUMNS);
        let exponent12 = (bits12 >> 3_u8) & 0xf;
        let mantissa12 = bits12 & 0x7;
        let magnitude12 = if exponent12 == 0xf && mantissa12 == 0x7 {
            let nan_source = f32::from(mantissa12.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent12 == 0 {
            f32::from(mantissa12) / 512.0
        } else {
            let scale = if exponent12 < 8 {
                f32::from(1_u8.wrapping_shl(exponent12 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent12.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa12) / 8.0) * scale
        };
        let value12 = if bits12 & 0x80 == 0 {
            magnitude12
        } else {
            -magnitude12
        };
        result0 += wave16.broadcast_f32(normalized0, 12) * value12;
        result1 += wave16.broadcast_f32(normalized1, 12) * value12;
        result2 += wave16.broadcast_f32(normalized2, 12) * value12;
        result3 += wave16.broadcast_f32(normalized3, 12) * value12;
        let bits13 = global_load_2d_u8(&value, value_base, 13, lane_column, VALUE_COLUMNS);
        let exponent13 = (bits13 >> 3_u8) & 0xf;
        let mantissa13 = bits13 & 0x7;
        let magnitude13 = if exponent13 == 0xf && mantissa13 == 0x7 {
            let nan_source = f32::from(mantissa13.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent13 == 0 {
            f32::from(mantissa13) / 512.0
        } else {
            let scale = if exponent13 < 8 {
                f32::from(1_u8.wrapping_shl(exponent13 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent13.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa13) / 8.0) * scale
        };
        let value13 = if bits13 & 0x80 == 0 {
            magnitude13
        } else {
            -magnitude13
        };
        result0 += wave16.broadcast_f32(normalized0, 13) * value13;
        result1 += wave16.broadcast_f32(normalized1, 13) * value13;
        result2 += wave16.broadcast_f32(normalized2, 13) * value13;
        result3 += wave16.broadcast_f32(normalized3, 13) * value13;
        let bits14 = global_load_2d_u8(&value, value_base, 14, lane_column, VALUE_COLUMNS);
        let exponent14 = (bits14 >> 3_u8) & 0xf;
        let mantissa14 = bits14 & 0x7;
        let magnitude14 = if exponent14 == 0xf && mantissa14 == 0x7 {
            let nan_source = f32::from(mantissa14.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent14 == 0 {
            f32::from(mantissa14) / 512.0
        } else {
            let scale = if exponent14 < 8 {
                f32::from(1_u8.wrapping_shl(exponent14 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent14.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa14) / 8.0) * scale
        };
        let value14 = if bits14 & 0x80 == 0 {
            magnitude14
        } else {
            -magnitude14
        };
        result0 += wave16.broadcast_f32(normalized0, 14) * value14;
        result1 += wave16.broadcast_f32(normalized1, 14) * value14;
        result2 += wave16.broadcast_f32(normalized2, 14) * value14;
        result3 += wave16.broadcast_f32(normalized3, 14) * value14;
        let bits15 = global_load_2d_u8(&value, value_base, 15, lane_column, VALUE_COLUMNS);
        let exponent15 = (bits15 >> 3_u8) & 0xf;
        let mantissa15 = bits15 & 0x7;
        let magnitude15 = if exponent15 == 0xf && mantissa15 == 0x7 {
            let nan_source = f32::from(mantissa15.wrapping_sub(7_u8));
            nan_source / nan_source
        } else if exponent15 == 0 {
            f32::from(mantissa15) / 512.0
        } else {
            let scale = if exponent15 < 8 {
                f32::from(1_u8.wrapping_shl(exponent15 as u32)) / 128.0
            } else {
                f32::from(1_u8.wrapping_shl(exponent15.wrapping_sub(7_u8) as u32))
            };
            (1.0 + f32::from(mantissa15) / 8.0) * scale
        };
        let value15 = if bits15 & 0x80 == 0 {
            magnitude15
        } else {
            -magnitude15
        };
        result0 += wave16.broadcast_f32(normalized0, 15) * value15;
        result1 += wave16.broadcast_f32(normalized1, 15) * value15;
        result2 += wave16.broadcast_f32(normalized2, 15) * value15;
        result3 += wave16.broadcast_f32(normalized3, 15) * value15;
        [result0, result1, result2, result3]
    });
    // The blocked output mapping proves the four stores owned by each lane do not race.
    let Some(output_block) = index.checked_block::<16, 4>() else {
        fe2o3_device::trap();
    };
    if !output.store_block(&output_block, 0, values[0])
        || !output.store_block(&output_block, 1, values[1])
        || !output.store_block(&output_block, 2, values[2])
        || !output.store_block(&output_block, 3, values[3])
    {
        fe2o3_device::trap();
    }
}
