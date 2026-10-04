use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(feature = "generic")]
#[inline(never)]
fn generic_identity<T: Copy>(value: T) -> T {
    value
}

#[cfg(feature = "generic")]
#[kernel]
pub fn generic_kernel(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = generic_identity(input[offset]);
    }
}

#[cfg(feature = "const-generic")]
#[inline(never)]
fn const_bias<const BIAS: u32>(value: f32) -> f32 {
    value + BIAS as f32
}

#[cfg(feature = "const-generic")]
#[kernel]
pub fn const_generic_kernel(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = const_bias::<7>(input[offset]);
    }
}

#[cfg(feature = "aggregate")]
#[derive(Clone, Copy)]
struct Pair {
    left: f32,
    right: f32,
}

#[cfg(feature = "aggregate")]
#[inline(never)]
fn sum_pair(pair: Pair) -> f32 {
    pair.left + pair.right
}

#[cfg(feature = "aggregate")]
#[kernel]
pub fn aggregate_field_kernel(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        let pair = Pair {
            left: input[offset],
            right: 2.0,
        };
        *value = sum_pair(pair);
    }
}

#[cfg(feature = "integer-match")]
#[inline(never)]
fn classify_integer(value: u32) -> f32 {
    match value & 3 {
        0 => 1.0,
        1 => 2.0,
        2 => 4.0,
        _ => 8.0,
    }
}

#[cfg(feature = "integer-match")]
#[kernel]
pub fn integer_match_kernel(mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let tag = index.get() as u32;
    if let Some(value) = output.get_mut(index) {
        *value = classify_integer(tag);
    }
}

#[cfg(feature = "loops")]
#[inline(never)]
fn repeat_bias(mut value: f32) -> f32 {
    let mut iteration = 0;
    while iteration < 4 {
        value += 0.5;
        iteration += 1;
    }
    value
}

#[cfg(feature = "loops")]
#[kernel]
pub fn loop_kernel(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = repeat_bias(input[offset]);
    }
}

#[cfg(feature = "cross-crate")]
#[kernel]
pub fn cross_crate_kernel(input: &[f32], mut output: DisjointSlice<f32>) {
    let index = thread::index_1d();
    let offset = index.get();
    if let Some(value) = output.get_mut(index) {
        *value = g2_semantic_helpers::cross_crate_bias(input[offset]);
    }
}

fn main() {}
