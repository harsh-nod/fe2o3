use fe2o3_device::{DisjointRowStripe2D, Index1D, Tiled2D, WriteOnlyDisjointSlice};

fn mismatch(
    output: &mut WriteOnlyDisjointSlice<u32, Tiled2D<Index1D, 64, 16, 16, 4>>,
    stripe: &DisjointRowStripe2D<Index1D, 16, 4>,
) {
    let _ = output.write_tiled_2d(stripe, 0, 1, 1, 1, 1);
}

fn main() {}
