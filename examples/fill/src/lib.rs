#![no_std]

use fe2o3_device::{DisjointSlice, kernel, thread};

/// CPU result for one output coordinate; launch coverage is checked separately.
pub fn fill_reference(_point: usize, out: &mut f32) {
    *out = 42.5;
}

#[cfg_attr(
    not(feature = "reference-proof"),
    kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))
)]
#[cfg_attr(
    feature = "reference-proof",
    kernel(
        typed,
        reference = fill_reference,
        launch(required = [64, 1, 1], max = [64, 1, 1]),
    )
)]
pub fn fill(mut out: DisjointSlice<f32>) {
    let idx = thread::index_1d();
    let Some(value) = out.get_mut(idx) else {
        return;
    };
    *value = 42.5;
}
