use fe2o3_device::{MaskedTile1D, WorkgroupCapability};

fn load(context: &WorkgroupCapability<'_, ()>) {
    let _ = MaskedTile1D::<u32, 64, 126, ()>::load_masked(context, &[1, 2], 0);
}

fn main() {
    let _ = std::hint::black_box(load as fn(&WorkgroupCapability<'_, ()>));
}
