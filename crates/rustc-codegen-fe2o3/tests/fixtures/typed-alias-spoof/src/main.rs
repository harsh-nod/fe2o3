#![allow(non_camel_case_types)]

use fe2o3_device::{DisjointSlice, kernel, thread};

// The macro sees the token `f32`, but rustc normalizes this alias to `f64`.
// The semantic layout extractor must reject it before artifact generation.
type f32 = f64;

#[kernel(
    typed,
    namespace = "1b3f523833ae188d124b710c48983f3911bd8be1ea82408ffc78121e843e6271"
)]
pub fn typed_alias_spoof(a: &[f32], b: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let i = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = a[i] + b[i];
    }
}

fn main() {}
