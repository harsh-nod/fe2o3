use fe2o3_device::{MaskedTile1D, WorkgroupCapability};

fn load(context: &WorkgroupCapability<'_, ()>) {
    let _ = MaskedTile1D::<f32, 1, 1, ()>::load_masked(context, &[1.0], 0);
}

fn main() {}
