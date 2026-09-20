//! Actual dynamic device loops, not CPU-reference-only loops.
use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(any(feature = "loop-capture-exact", feature = "loop-capture-multi"))]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
pub fn count_to_limit(limit: u32, mut output: DisjointSlice<u32>) {
    let mut cursor = 0_u32;
    while cursor < limit {
        cursor += 1;
    }
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = cursor;
    }
}

#[cfg(any(feature = "loop-capture-renamed", feature = "loop-capture-multi"))]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
pub fn renamed_count(maximum: u32, mut destination: DisjointSlice<u32>) {
    let mut iteration = 0_u32;
    while iteration < maximum {
        iteration += 1;
    }
    if let Some(slot) = destination.get_mut(thread::index_1d()) {
        *slot = iteration;
    }
}

#[cfg(feature = "loop-capture-u64")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn wide_copy(value: u64, mut output: DisjointSlice<u64>) {
    // A no-certificate control, not a claim about admission of U64 loops.
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}

#[cfg(feature = "loop-capture-generic")]
fn identity<T>(value: T) -> T {
    value
}

#[cfg(feature = "loop-capture-generic")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(4294967295))
)]
pub fn generic_count(limit: u32, mut output: DisjointSlice<u32>) {
    let mut cursor = 0_u32;
    while cursor < limit {
        cursor += 1;
    }
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = identity::<u32>(cursor);
    }
}
