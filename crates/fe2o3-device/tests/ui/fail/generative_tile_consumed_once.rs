use fe2o3_device::{LaneFragment, MaskedTile1D};

fn tile(value: MaskedTile1D<'_, u32, 1, 1, ()>) {
    let _ = value.into_fragment();
    let _ = value.into_fragment();
}

fn fragment(value: LaneFragment<'_, u32, 1, 1, ()>) {
    let _ = value.into_parts();
    let _ = value.into_parts();
}

fn main() {}
