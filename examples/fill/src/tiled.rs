use fe2o3_device::{DisjointSlice, Index1D, Tiled2D, kernel, thread};

/// Each 64-lane workgroup owns an 8x8 row-major output tile.
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn fill(mut out: DisjointSlice<f32, Tiled2D<Index1D, 64, 8, 8, 1>>) {
    let length = out.len();
    let mut rows = length / 8;
    if length % 8 != 0 {
        // The quotient is at most usize::MAX / 8; expose the increment bound.
        if rows > usize::MAX - 1 {
            return;
        }
        rows += 1;
    }
    let Some(tile) = thread::index_1d().checked_tiled_2d::<64, 8, 8, 1>() else {
        return;
    };
    let Some(value) = out.get_tiled_2d_mut(&tile, 0, rows, 8, 8) else {
        return;
    };
    *value = 42.5;
}
