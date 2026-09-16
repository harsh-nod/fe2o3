use fe2o3_device::{DisjointSlice, kernel, thread};

#[inline(never)]
fn increment(value: usize) -> usize {
    value.wrapping_add(3)
}

#[inline(never)]
fn combine(parts: (usize, (), usize)) -> usize {
    parts.0.wrapping_mul(5).wrapping_add(parts.2)
}

struct Token(usize);

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn slice_metadata_arguments(
    input: &[u32],
    ordinary: u64,
    mut scalar: DisjointSlice<u64>,
    mut repeated: DisjointSlice<u64>,
    mut mixed: DisjointSlice<u64>,
    mut closure: DisjointSlice<u64>,
) {
    let ordinary = ordinary as usize;
    let length = input.len();
    let a = increment(length);
    let b = combine((length, (), length));
    let c = combine((length, (), ordinary));
    let token = Token(ordinary);
    let f = move |first: usize, (): (), last: usize| {
        let moved = token;
        combine((first, (), last)).wrapping_add(moved.0)
    };
    let d = f(length, (), ordinary);
    if let Some(value) = scalar.get_mut(thread::index_1d()) {
        *value = a as u64;
    }
    if let Some(value) = repeated.get_mut(thread::index_1d()) {
        *value = b as u64;
    }
    if let Some(value) = mixed.get_mut(thread::index_1d()) {
        *value = c as u64;
    }
    if let Some(value) = closure.get_mut(thread::index_1d()) {
        *value = d as u64;
    }
}
