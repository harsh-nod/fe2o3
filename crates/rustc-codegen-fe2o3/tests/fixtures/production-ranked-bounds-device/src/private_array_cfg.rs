use fe2o3_device::{DisjointSlice, kernel, thread};

#[cfg(feature = "private_array_cross_block")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_array_cross_block(
    a: u32,
    b: u32,
    c: u32,
    selector: u64,
    rounds: u32,
    mut output: DisjointSlice<u32>,
) {
    let mut values = [a, b, c];
    if rounds != 0 {
        values = [c, b, a];
    }
    let selected = values[0].wrapping_add(values[selector as usize]);
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = selected;
    }
}

#[cfg(feature = "private_array_loop")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_array_loop(
    a: u32,
    b: u32,
    c: u32,
    selector: u64,
    rounds: u32,
    mut output: DisjointSlice<u32>,
) {
    let mut values = [a, b, c];
    let mut step = 0;
    while step < rounds {
        let head = values[0];
        values = [head.wrapping_add(b), b, c];
        step = step.wrapping_add(1);
    }
    let selected = values[0].wrapping_add(values[selector as usize]);
    if let Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = selected;
    }
}
