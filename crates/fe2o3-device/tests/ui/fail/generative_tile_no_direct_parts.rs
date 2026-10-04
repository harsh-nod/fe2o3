use fe2o3_device::MaskedTile1D;

fn bypass(value: MaskedTile1D<'_, u32, 1, 1, ()>) {
    let _ = value.into_parts();
}

fn main() {}
