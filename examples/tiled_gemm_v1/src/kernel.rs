//! Ordinary Rust source for the fixed Slice 1 LDS tiled GEMM.
//!
//! The kernel algorithm below is real attributed Rust source, not explanatory
//! pseudocode and not a `macro_rules!` expansion. The current frontend cannot
//! yet issue the two BF16 LDS allocations consumed by the algorithm, so the
//! private acquisition boundary panics. This keeps ordinary Cargo builds and
//! unsupported device compilation fail-closed until that compiler operation is
//! implemented and authenticated.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{
    Bf16, Bf16MfmaFragment, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, LdsTile16x16,
    Wave64, WaveLane, kernel, sync, thread,
};

/// Exact workgroup dimensions required by the Slice 1 source contract.
pub const LDS_SLICE1_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Number of BF16 elements in each XOR4-staged operand tile.
pub const LDS_SLICE1_OPERAND_ELEMENTS_V1: usize = 16 * 16;
/// Number of bytes occupied by each XOR4-staged BF16 operand tile.
pub const LDS_SLICE1_OPERAND_BYTES_V1: usize = LDS_SLICE1_OPERAND_ELEMENTS_V1 * 2;
/// Total LDS bytes required for the separate A and transposed-B tiles.
pub const LDS_SLICE1_TOTAL_BYTES_V1: usize = 2 * LDS_SLICE1_OPERAND_BYTES_V1;

/// Whether the current source frontend can lower this kernel through LDS.
///
/// This remains false until authenticated lowering can issue two independent
/// `LdsTile16x16<Bf16>` capabilities for one `gfx942:xnack-` WG64 execution.
pub const LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1: bool = false;

/// Current fail-closed reason for the Slice 1 source lowering boundary.
pub const LDS_SLICE1_SOURCE_BLOCKER_V1: &str =
    "the frontend does not lower compiler-issued BF16 LdsTile16x16 allocations";

/// Complete current compiler worklist before this source can become executable.
pub const LDS_SLICE1_SOURCE_BLOCKERS_V1: [&str; 4] = [
    LDS_SLICE1_SOURCE_BLOCKER_V1,
    "the frontend does not authenticate WaveLane::from_raw as the current gfx942 wave64 lane",
    "the frontend does not lower sync::syncthreads to a convergent workgroup barrier",
    "the collected tiled GEMM path admits only the direct-global no-LDS canonical graph",
];

/// Computes one fixed `16x16x16` BF16 GEMM tile through XOR4-staged LDS.
///
/// `a` and `b` must each contain exactly 256 row-major BF16 bit patterns and
/// `c` must contain exactly 256 FP32 output elements. One `gfx942:xnack-`
/// wave64 workgroup cooperatively stages both operands, executes one
/// `V_MFMA_F32_16X16X16_BF16` from a zero accumulator, and gives each lane
/// exclusive ownership of four output elements.
///
/// The function is compiler-facing source today, but is not executable GPU
/// authority: [`LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1`] is false and the
/// missing compiler-issued LDS allocation traps at the private acquisition
/// boundary before any output is written.
#[kernel(
    typed,
    namespace = "67100a64733dabbac624aac230d3ca79ccea4cc307c45ee64d41f3362bc16bbb",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn tiled_gemm_lds_slice1(a: &[u16], b: &[u16], mut c: DisjointSlice<f32>) {
    let lane_index = thread::index_1d().get();
    if lane_index >= 64 || a.len() != 256 || b.len() != 256 || c.len() != 256 {
        fe2o3_device::trap();
        return;
    }

    let lane_column = lane_index % 16;
    let depth_base = (lane_index / 16) * 4;
    let a_row_base = lane_column * 16;

    let a_global = Bf16MfmaFragment::from_bits([
        a[a_row_base + depth_base],
        a[a_row_base + depth_base + 1],
        a[a_row_base + depth_base + 2],
        a[a_row_base + depth_base + 3],
    ]);
    let b_global = Bf16MfmaFragment::from_bits([
        b[depth_base * 16 + lane_column],
        b[(depth_base + 1) * 16 + lane_column],
        b[(depth_base + 2) * 16 + lane_column],
        b[(depth_base + 3) * 16 + lane_column],
    ]);

    // SAFETY: the exact source contract fixes one gfx942 wave64 workgroup, and
    // lane_index is checked above. Backend authentication remains required.
    let lane = unsafe { WaveLane::<Wave64>::from_raw(lane_index as u32) }
        .expect("the checked Slice 1 lane is in wave64");

    // This is the first fail-closed unsupported operation. It panics before
    // any C write until the frontend can issue two disjoint 512-byte LDS tiles.
    let (mut a_lds, mut b_lds) = acquire_bf16_lds_tiles_v1();

    // SAFETY: every lane owns four distinct XOR4 locations in each separate
    // tile. B's lane fragment is staged transposed as (column, depth).
    let a_staged = unsafe { a_lds.write_mfma_fragment(&lane, a_global) };
    let b_staged = unsafe { b_lds.write_mfma_fragment(&lane, b_global) };
    if !a_staged || !b_staged {
        fe2o3_device::trap();
        return;
    }

    // SAFETY: all 64 physical lanes execute this call in uniform control flow
    // after writing their four A and four B elements.
    unsafe { sync::syncthreads() };

    // SAFETY: the preceding convergent barrier follows complete, disjoint
    // initialization of all 256 elements in both tiles.
    let a_lds = unsafe { a_lds.assume_init() };
    let b_lds = unsafe { b_lds.assume_init() };
    let lhs = a_lds
        .read_mfma_fragment(lane_index)
        .expect("the checked wave64 lane has one A fragment");
    let rhs = b_lds
        .read_mfma_fragment(lane_index)
        .expect("the checked wave64 lane has one B fragment");

    // SAFETY: the compiler must issue this capability only for the exact
    // gfx942:xnack- wave64 profile, and every lane calls the MFMA uniformly.
    let matrix = unsafe { DeviceMatrix::from_compiler() };
    let result =
        unsafe { matrix.multiply_accumulate(lhs, rhs, F32AccumulatorFragment::ZERO) }.into_values();

    // SAFETY: (lane, component) maps bijectively to all 256 C coordinates.
    if let Some(output) = unsafe { c.get_mut_at(depth_base * 16 + lane_column) } {
        *output = result[0];
    }
    if let Some(output) = unsafe { c.get_mut_at((depth_base + 1) * 16 + lane_column) } {
        *output = result[1];
    }
    if let Some(output) = unsafe { c.get_mut_at((depth_base + 2) * 16 + lane_column) } {
        *output = result[2];
    }
    if let Some(output) = unsafe { c.get_mut_at((depth_base + 3) * 16 + lane_column) } {
        *output = result[3];
    }
}

fn acquire_bf16_lds_tiles_v1<'workgroup>() -> (
    LdsTile16x16<'workgroup, Bf16>,
    LdsTile16x16<'workgroup, Bf16>,
) {
    panic!("{LDS_SLICE1_SOURCE_BLOCKER_V1}")
}
