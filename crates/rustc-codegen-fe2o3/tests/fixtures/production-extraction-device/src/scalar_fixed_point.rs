use fe2o3_device::kernel;
#[cfg(any(
    all(fe2o3_scalar_fixed_point_noop, fe2o3_scalar_fixed_point_safe),
    all(fe2o3_canonical_scalar_mutating, fe2o3_canonical_scalar_noop),
    all(
        any(fe2o3_canonical_scalar_mutating, fe2o3_canonical_scalar_noop),
        any(fe2o3_scalar_fixed_point_noop, fe2o3_scalar_fixed_point_safe)
    )
))]
compile_error!("scalar source cases must be mutually exclusive");
#[cfg(not(any(
    fe2o3_scalar_fixed_point_noop,
    fe2o3_canonical_scalar_mutating,
    fe2o3_canonical_scalar_noop
)))]
use fe2o3_device::{DisjointSlice, thread};

#[cfg(not(any(
    fe2o3_scalar_fixed_point_noop,
    fe2o3_canonical_scalar_mutating,
    fe2o3_canonical_scalar_noop
)))]
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

#[cfg(not(any(
    fe2o3_scalar_fixed_point_noop,
    fe2o3_scalar_fixed_point_safe,
    fe2o3_canonical_scalar_mutating,
    fe2o3_canonical_scalar_noop
)))]
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

#[cfg(fe2o3_canonical_scalar_mutating)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_neutral_mutating(value: u32, choose: u32) {
    let first = value ^ 0;
    let second = first | 0;
    let joined = if choose == 0 {
        second ^ 0
    } else {
        second & u32::MAX
    };
    let _ = joined;
}

#[cfg(fe2o3_canonical_scalar_noop)]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_neutral_noop() {}
