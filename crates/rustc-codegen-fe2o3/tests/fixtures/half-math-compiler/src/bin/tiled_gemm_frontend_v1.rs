#![allow(non_upper_case_globals)]

use fe2o3_device::{Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMatrix, F32AccumulatorFragment};

const FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

#[unsafe(export_name = "fe2o3_kernel_tiled_gemm_frontend_v1")]
pub fn kernel<'wave>(
    lhs: Bf16MfmaAFragment<'wave>,
    rhs: Bf16MfmaBFragment<'wave>,
    accumulator: F32AccumulatorFragment<'wave>,
) {
    let matrix = DeviceMatrix::current();
    let _result = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

#[used]
static __fe2o3_kernel_registration_tiled_gemm_frontend_v1: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    for<'wave> fn(
        Bf16MfmaAFragment<'wave>,
        Bf16MfmaBFragment<'wave>,
        F32AccumulatorFragment<'wave>,
    ),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "tiled_gemm_frontend_v1",
    "tiled_gemm_frontend_v1",
    kernel,
);

#[used]
static __fe2o3_kernel_frontend_contract_v1_tiled_gemm_frontend_v1: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    for<'wave> fn(
        Bf16MfmaAFragment<'wave>,
        Bf16MfmaBFragment<'wave>,
        F32AccumulatorFragment<'wave>,
    ),
) = (
    0x4146_4b33_4f32_4546,
    1,
    1,
    "tiled_gemm_frontend_v1",
    FRONTEND_CONTRACT,
    kernel,
);

fn main() {}
