use fe2o3_device::{DisjointSlice, Index1D, Tiled2D, kernel, thread};

/// Each 64-lane workgroup owns an 8x8 row-major output tile.
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn fill(mut out: DisjointSlice<f32, Tiled2D<Index1D, 64, 8, 8, 1>>) {
    let length = out.len();
    let rows = length / 8 + ((length % 8 != 0) as usize);
    let Some(tile) = thread::index_1d().checked_tiled_2d::<64, 8, 8, 1>() else {
        return;
    };
    let Some(value) = out.get_tiled_2d_mut(&tile, 0, rows, 8, 8) else {
        return;
    };
    *value = 42.5;
}
