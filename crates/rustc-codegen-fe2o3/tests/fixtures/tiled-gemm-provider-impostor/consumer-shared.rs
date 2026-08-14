use matrix_provider::{Bf16MfmaFragment, DeviceMatrix, F32AccumulatorFragment};

const FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

#[unsafe(export_name = "fe2o3_kernel_tiled_gemm_provider_impostor")]
pub fn kernel(lhs: Bf16MfmaFragment, rhs: Bf16MfmaFragment, accumulator: F32AccumulatorFragment) {
    let matrix = unsafe { DeviceMatrix::from_compiler() };
    let _result = unsafe { matrix.multiply_accumulate(lhs, rhs, accumulator) };
}

#[used]
static __fe2o3_kernel_registration_tiled_gemm_provider_impostor: (
    u64,
    u16,
    u16,
    &'static str,
    &'static str,
    fn(Bf16MfmaFragment, Bf16MfmaFragment, F32AccumulatorFragment),
) = (
    0x4e52_4b33_4f32_4546,
    1,
    1,
    "tiled_gemm_provider_impostor",
    "tiled_gemm_provider_impostor",
    kernel,
);

#[used]
static __fe2o3_kernel_frontend_contract_v1_tiled_gemm_provider_impostor: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    fn(Bf16MfmaFragment, Bf16MfmaFragment, F32AccumulatorFragment),
) = (
    0x4146_4b33_4f32_4546,
    1,
    1,
    "tiled_gemm_provider_impostor",
    FRONTEND_CONTRACT,
    kernel,
);

fn main() {}
