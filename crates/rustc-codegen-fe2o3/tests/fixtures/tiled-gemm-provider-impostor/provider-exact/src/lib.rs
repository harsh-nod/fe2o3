#![no_std]
#![feature(rustc_attrs)]
#![allow(internal_features)]

use core::marker::PhantomData;

#[derive(Clone, Copy)]
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_v1"]
pub struct Bf16(u16);

#[derive(Clone, Copy)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_fragment_v1"]
pub struct Bf16MfmaFragment([Bf16; 4]);

#[derive(Clone, Copy)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_v1"]
pub struct F32AccumulatorFragment([f32; 4]);

#[rustc_diagnostic_item = "fe2o3_device_matrix_context_v1"]
pub struct DeviceMatrix(PhantomData<*mut ()>);

impl DeviceMatrix {
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_matrix_context_current_v1"]
    pub fn current() -> Self {
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
