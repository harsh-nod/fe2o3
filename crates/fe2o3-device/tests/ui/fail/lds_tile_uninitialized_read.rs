use fe2o3_device::{LdsTile16x16, LdsUninitialized, Wave64, WaveLane};

fn read_before_init(
    tile: &LdsTile16x16<'_, u32, LdsUninitialized>,
    lane: &WaveLane<Wave64>,
) {
    let _ = tile.read_wave_fragment(lane);
}

fn main() {}
