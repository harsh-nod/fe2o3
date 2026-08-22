//! Ordinary Rust source for the fixed Slice 1 LDS tiled GEMM.
//!
//! The kernel algorithm below is real attributed Rust source, not explanatory
//! pseudocode and not a `macro_rules!` expansion. The two BF16 LDS tiles are
//! issued through a compiler-only intrinsic. Host execution and every
//! unsupported compiler path remain fail-closed.

#![allow(missing_docs)] // Generated typed-kernel modules lack rustdoc in V1.

use fe2o3_device::{
    Bf16MfmaFragment, Blocked, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, Index1D,
    Wave64, WaveLane, gfx942_lds_bf16_tile_pair_m16x16_v1,
    gfx942_publish_lds_bf16_tile_pair_m16x16_v1, kernel, thread,
};

/// Exact workgroup dimensions required by the Slice 1 source contract.
pub const LDS_SLICE1_WORKGROUP_V1: [u32; 3] = [64, 1, 1];
/// Number of BF16 elements in each XOR4-staged operand tile.
pub const LDS_SLICE1_OPERAND_ELEMENTS_V1: usize = 16 * 16;
/// Number of bytes occupied by each XOR4-staged BF16 operand tile.
pub const LDS_SLICE1_OPERAND_BYTES_V1: usize = LDS_SLICE1_OPERAND_ELEMENTS_V1 * 2;
/// Total LDS bytes required for the separate A and transposed-B tiles.
pub const LDS_SLICE1_TOTAL_BYTES_V1: usize = 2 * LDS_SLICE1_OPERAND_BYTES_V1;

/// Whether the attributed Rust source reaches the verified canonical LDS IR.
pub const LDS_SLICE1_SOURCE_TO_IR_SUPPORTED_V1: bool = true;

/// Whether the current source frontend lowers this kernel through LLVM/HSACO.
///
/// Source authentication and the source-to-IR correspondence are implemented,
/// but descriptor publication and dedicated LLVM lowering are not joined yet.
pub const LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1: bool = false;

/// Current fail-closed reason for the Slice 1 source lowering boundary.
pub const LDS_SLICE1_SOURCE_BLOCKER_V1: &str =
    "the source-to-IR receipt stops before compiler descriptor construction";

/// Complete current compiler worklist before this source can become executable.
pub const LDS_SLICE1_SOURCE_BLOCKERS_V1: [&str; 4] = [
    LDS_SLICE1_SOURCE_BLOCKER_V1,
    "the authenticated source path is not joined to the dedicated upstream-LLVM LDS lowering",
    "the reviewed source-to-IR correspondence is not a compiler-refinement proof",
    "protected Worker V2 publication, HSACO load, and launch remain fail-closed",
];

/// Computes one fixed `16x16x16` BF16 GEMM tile through XOR4-staged LDS.
///
/// `a` and `b` must each contain exactly 256 row-major BF16 bit patterns and
/// `c` must contain exactly 256 FP32 output elements. One `gfx942:xnack-`
/// wave64 workgroup cooperatively stages both operands, executes one
/// `V_MFMA_F32_16X16X16_BF16` from a zero accumulator, and gives each lane
/// exclusive ownership of four output elements.
///
/// The attributed source is authenticated and selects the verified canonical
/// Kernel IR. It is not executable GPU authority:
/// [`LDS_SLICE1_SOURCE_LOWERING_SUPPORTED_V1`] remains false and compilation
/// stops before descriptor publication or LLVM lowering.
#[kernel(
    typed,
    namespace = "c09558e16157fec495e78bc32a23b082213fa4a6ddabe48445a54cb3de591295",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn tiled_gemm_lds_slice1(
    a: &[u16],
    b: &[u16],
    mut c: DisjointSlice<f32, Blocked<Index1D, 16, 4>>,
) {
    let thread_index = thread::index_1d();
    let lane_index = thread_index.get();
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

    let lane = WaveLane::<Wave64>::current();

    let (mut a_lds, mut b_lds) = gfx942_lds_bf16_tile_pair_m16x16_v1();

    a_lds.write_mfma_fragment(&lane, a_global);
    b_lds.write_mfma_fragment(&lane, b_global);

    let (a_lds, b_lds) = gfx942_publish_lds_bf16_tile_pair_m16x16_v1(a_lds, b_lds);
    let lhs = a_lds.read_mfma_fragment(&lane);
    let rhs = b_lds.read_mfma_fragment(&lane);

    let matrix = DeviceMatrix::current();
    let result = matrix
        .multiply_accumulate(lhs, rhs, F32AccumulatorFragment::ZERO)
        .into_values();
    let Some(output_block) = thread_index.checked_block::<16, 4>() else {
        fe2o3_device::trap();
        return;
    };

    if let Some(output) = c.get_block_mut(&output_block, 0) {
        *output = result[0];
    }
    if let Some(output) = c.get_block_mut(&output_block, 1) {
        *output = result[1];
    }
    if let Some(output) = c.get_block_mut(&output_block, 2) {
        *output = result[2];
    }
    if let Some(output) = c.get_block_mut(&output_block, 3) {
        *output = result[3];
    }
}
