use fe2o3_device::kernel;
#[cfg(not(fe2o3_scalar_fixed_point_noop))]
use fe2o3_device::{DisjointSlice, thread};

#[cfg(not(fe2o3_scalar_fixed_point_noop))]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_checked_identity(mut output: DisjointSlice<u32>, value: u32, choose: u32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        let neutral = (value + 0) * 1;
        let result = if choose == 0 {
            neutral ^ 0
        } else {
            neutral ^ 3
        };
        *element = result;
    }
}

#[cfg(not(fe2o3_scalar_fixed_point_noop))]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_effect_then_overflow(mut output: DisjointSlice<u32>, value: u32) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 0x13579bdf;
        let result = value + 1;
        *element = result;
    }
}

#[cfg(fe2o3_scalar_fixed_point_noop)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_noop_control() {}
