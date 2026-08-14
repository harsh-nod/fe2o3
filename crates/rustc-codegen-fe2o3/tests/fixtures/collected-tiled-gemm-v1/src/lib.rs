#![allow(non_upper_case_globals)]

use fe2o3_device::{
    Bf16MfmaFragment, DeviceMatrix, DisjointSlice, F32AccumulatorFragment, kernel, thread,
};

const FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0,
    0, 1, 0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

#[kernel(
    typed,
    namespace = "7eb5edda86f1edd9b886a256243b601b8a58c48b28ac8b72ba9eb5554cdb01a8"
)]
pub fn tiled_gemm_v1(a: &[u16], b: &[u16], c: &[f32], mut d: DisjointSlice<f32>) {
    let lane = thread::index_1d().get();
    let lane_column = lane % 16;
    let depth_base = (lane / 16) * 4;
    let a_row_base = lane_column * 16;

    let lhs = Bf16MfmaFragment::from_bits([
        a[a_row_base + depth_base],
        a[a_row_base + depth_base + 1],
        a[a_row_base + depth_base + 2],
        a[a_row_base + depth_base + 3],
    ]);
    let rhs = Bf16MfmaFragment::from_bits([
        b[depth_base * 16 + lane_column],
        b[(depth_base + 1) * 16 + lane_column],
        b[(depth_base + 2) * 16 + lane_column],
        b[(depth_base + 3) * 16 + lane_column],
    ]);
    let accumulator = F32AccumulatorFragment::from_values([
        c[depth_base * 16 + lane_column],
        c[(depth_base + 1) * 16 + lane_column],
        c[(depth_base + 2) * 16 + lane_column],
        c[(depth_base + 3) * 16 + lane_column],
    ]);

    let matrix = unsafe { DeviceMatrix::from_compiler() };
    let result = unsafe { matrix.multiply_accumulate(lhs, rhs, accumulator) }.into_values();

    if let Some(output) = unsafe { d.get_mut_at(depth_base * 16 + lane_column) } {
        *output = result[0];
    }
    if let Some(output) = unsafe { d.get_mut_at((depth_base + 1) * 16 + lane_column) } {
        *output = result[1];
    }
    if let Some(output) = unsafe { d.get_mut_at((depth_base + 2) * 16 + lane_column) } {
        *output = result[2];
    }
    if let Some(output) = unsafe { d.get_mut_at((depth_base + 3) * 16 + lane_column) } {
        *output = result[3];
    }
}

#[used]
static __fe2o3_kernel_frontend_contract_v1_tiled_gemm_v1: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    fn(&[u16], &[u16], &[f32], DisjointSlice<f32>),
) = (
    0x4146_4b33_4f32_4546,
    1,
    1,
    "tiled_gemm_v1",
    FRONTEND_CONTRACT,
    <__fe2o3_kernel_marker_tiled_gemm_v1 as fe2o3_device::KernelMarkerV1>::FUNCTION,
);
