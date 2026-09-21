use fe2o3_device::{DisjointSlice, kernel, thread};

// No reference annotation: these source tests stop at checked output, not proof
// publication. The scalar loop bound is deliberately not a SliceLength bound.
#[cfg(feature = "guarded-loop-read")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(1024))
)]
pub fn guarded_loop_read(input: &[u32], limit: usize, mut output: DisjointSlice<u32>) {
    if limit > 1024 {
        return;
    }
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 0;
        let mut cursor = 0_usize;
        while cursor < limit {
            if cursor < input.len() {
                *element = input[cursor];
            }
            cursor += 1;
        }
    }
}

#[cfg(feature = "guarded-loop-read-control")]
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn guarded_loop_read_control(_input: &[u32], _limit: usize, mut output: DisjointSlice<u32>) {
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 7;
    }
}

#[cfg(feature = "guarded-loop-read-stale")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(1024))
)]
pub fn guarded_loop_read_stale(input: &[u32], limit: usize, mut output: DisjointSlice<u32>) {
    if limit > 1024 {
        return;
    }
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 0;
        let mut cursor = 0_usize;
        if cursor < input.len() {
            while cursor < limit {
                // Safe indexing must retain its own current-index assertion.
                *element = input[cursor];
                cursor += 1;
            }
        }
    }
}

#[cfg(feature = "guarded-loop-read-different")]
#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(1024))
)]
pub fn guarded_loop_read_different(input: &[u32], limit: usize, mut output: DisjointSlice<u32>) {
    if limit > 1024 {
        return;
    }
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = 0;
        let mut cursor = 0_usize;
        while cursor < limit {
            if limit < input.len() {
                // The explicit guard cannot replace cursor's bounds assertion.
                *element = input[cursor];
            }
            cursor += 1;
        }
    }
}
