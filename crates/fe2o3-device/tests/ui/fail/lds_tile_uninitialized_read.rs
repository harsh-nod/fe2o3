use fe2o3_device::{LdsTile16x16, LdsUninitialized};

fn read_before_init(tile: &LdsTile16x16<'_, u32, LdsUninitialized>) {
    let _ = tile.read_wave_fragment(0);
}

fn main() {}
