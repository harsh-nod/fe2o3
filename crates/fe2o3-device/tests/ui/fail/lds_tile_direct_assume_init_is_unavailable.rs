use fe2o3_device::{Bf16, LdsTile16x16};

fn direct_assume_init_is_unavailable(tile: LdsTile16x16<'_, Bf16>) {
    let _ = tile.assume_init();
}

fn main() {}
