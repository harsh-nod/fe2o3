//! Auxiliary reference annotation: the default manifest Vecadd has none.
//! The positive kernel includes the real manifest body's tokens unchanged.
use fe2o3_device::{DisjointSlice, kernel, thread};

include!("../../../../../../examples/vecadd/src/vecadd_body.rs");
include!("conditional_vecadd_reference.rs");

macro_rules! production_f32_add {
    ($lhs:expr, $rhs:expr) => {{ $lhs + $rhs }};
}

#[cfg_attr(
    feature = "conditional-vecadd-cpu-read",
    kernel(typed, reference = wrong_index_reference)
)]
#[cfg_attr(
    not(feature = "conditional-vecadd-cpu-read"),
    kernel(typed, reference = vecadd_reference)
)]
pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
    // This guard skips required writes even with two sufficient input extents.
    #[cfg(feature = "conditional-vecadd-input-guard")]
    if thread::index_1d().get() >= a.len() / 2 {
        return;
    }

    #[cfg(feature = "conditional-vecadd-source-argument")]
    vecadd_kernel_body!(thread, (), production_f32_add, a, a, c);
    #[cfg(not(feature = "conditional-vecadd-source-argument"))]
    vecadd_kernel_body!(thread, (), production_f32_add, a, b, c);
}
