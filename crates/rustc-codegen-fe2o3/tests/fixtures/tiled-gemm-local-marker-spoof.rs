#![feature(rustc_attrs)]
#![allow(internal_features, non_upper_case_globals)]

use core::marker::PhantomData;

const FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 1,
    0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

#[derive(Clone, Copy)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_fragment_v1"]
pub struct Bf16MfmaFragment([u16; 4]);

#[derive(Clone, Copy)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_v1"]
pub struct F32AccumulatorFragment([f32; 4]);

#[rustc_diagnostic_item = "fe2o3_device_matrix_context_v1"]
pub struct DeviceMatrix(PhantomData<*mut ()>);

impl DeviceMatrix {
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_matrix_context_from_compiler_v1"]
    pub unsafe fn from_compiler() -> Self {
        unreachable!()
    }

    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_matrix_mfma_bf16_f32_m16n16k16_v1"]
    pub unsafe fn multiply_accumulate(
        &self,
        lhs: Bf16MfmaFragment,
        rhs: Bf16MfmaFragment,
        accumulator: F32AccumulatorFragment,
    ) -> F32AccumulatorFragment {
        let _ = (self, lhs, rhs);
        accumulator
    }
}

#[unsafe(export_name = "fe2o3_kernel_tiled_gemm_local_marker_spoof")]
pub fn kernel(lhs: Bf16MfmaFragment, rhs: Bf16MfmaFragment, accumulator: F32AccumulatorFragment) {
    let matrix = unsafe { DeviceMatrix::from_compiler() };
    let _result = unsafe { matrix.multiply_accumulate(lhs, rhs, accumulator) };
}

#[used]
static __fe2o3_kernel_registration_tiled_gemm_local_marker_spoof: (
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
    "tiled_gemm_local_marker_spoof",
    "tiled_gemm_local_marker_spoof",
    kernel,
);

#[used]
static __fe2o3_kernel_frontend_contract_v1_tiled_gemm_local_marker_spoof: (
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
    "tiled_gemm_local_marker_spoof",
    FRONTEND_CONTRACT,
    kernel,
);
