#![no_std]

#[allow(unused_imports)]
use fe2o3_device::{DisjointSlice, kernel, thread};

#[allow(unused_macros)]
macro_rules! integer_body {
    ($left:ident, $right:ident, $output:ident) => {{
        let lane = thread::index_1d();
        if let Some(output) = $output.get_mut(lane) {
            *output = if $left < $right {
                $left ^ $right
            } else {
                $left & $right
            };
        }
    }};
}

#[cfg(feature = "integer-i8")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_i8(left: i8, right: i8, mut output: DisjointSlice<i8>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-i16")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_i16(left: i16, right: i16, mut output: DisjointSlice<i16>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-i32")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_i32(left: i32, right: i32, mut output: DisjointSlice<i32>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-i64")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_i64(left: i64, right: i64, mut output: DisjointSlice<i64>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-u8")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_u8(left: u8, right: u8, mut output: DisjointSlice<u8>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-u16")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_u16(left: u16, right: u16, mut output: DisjointSlice<u16>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-u32")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_u32(left: u32, right: u32, mut output: DisjointSlice<u32>) {
    integer_body!(left, right, output);
}

#[cfg(feature = "integer-u64")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn integer_u64(left: u64, right: u64, mut output: DisjointSlice<u64>) {
    integer_body!(left, right, output);
}

#[allow(unused_macros)]
macro_rules! float_body {
    ($left:ident, $right:ident, $output:ident) => {{
        let lane = thread::index_1d();
        let value = $left + $right;
        if let Some(output) = $output.get_mut(lane) {
            *output = value;
        }
    }};
}

#[cfg(feature = "float-f32")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn float_f32(left: f32, right: f32, mut output: DisjointSlice<f32>) {
    float_body!(left, right, output);
}

#[cfg(feature = "float-f64")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn float_f64(left: f64, right: f64, mut output: DisjointSlice<f64>) {
    float_body!(left, right, output);
}

#[cfg(feature = "switch-u32")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(integer_switches(u32))
)]
pub fn switch_u32(selector: u32, mut output: DisjointSlice<u32>) {
    let selected = match selector {
        0 => 11,
        1 | 2 => 23,
        7 => 71,
        _ => fe2o3_device::trap(),
    };
    if let Some(output) = output.get_mut(thread::index_1d()) {
        *output = selected;
    }
}

#[cfg(feature = "bounds-output")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn bounds_output(value: u32, mut output: DisjointSlice<u32>) {
    if let Some(output) = output.get_mut(thread::index_1d()) {
        *output = value;
    }
}

#[cfg(feature = "atomic-u32")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn atomic_u32(target: fe2o3_device::DeviceGlobalMutPtr<u32>) {
    use fe2o3_device::atomic::Ordering;

    let target = target.as_atomic();
    let _ = target.fetch_add(1, Ordering::Relaxed);
}

#[cfg(feature = "unsupported-memory")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn unsupported_memory(source: &[u32], mut destination: DisjointSlice<u32>) {
    let destination_index = thread::index_1d().into_disjoint();
    let distance = fe2o3_device::memory::offset_from(source, 1, 0);
    let value = fe2o3_device::memory::volatile_load(source, 0);
    fe2o3_device::memory::volatile_store(&mut destination, &destination_index, value);
    fe2o3_device::memory::copy_one_nonoverlapping(source, 0, &mut destination, &destination_index);
    let _ = distance;
}

#[cfg(feature = "unsupported-i128")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn unsupported_i128(value: i128, mut output: DisjointSlice<i64>) {
    if let Some(output) = output.get_mut(thread::index_1d()) {
        *output = value as i64;
    }
}
