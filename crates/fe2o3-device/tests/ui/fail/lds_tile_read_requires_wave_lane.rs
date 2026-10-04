use fe2o3_device::{LdsInitialized, LdsTile16x16};

fn read_with_raw_lane(tile: &LdsTile16x16<'_, u32, LdsInitialized>) {
    let _ = tile.read_wave_fragment(0);
}

fn main() {
    let _ = read_with_raw_lane;
}
