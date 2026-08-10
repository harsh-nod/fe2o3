use fe2o3_device::{LdsTile16x16, LdsUninitialized, Wave64, WaveLane};

fn write(
    tile: &mut LdsTile16x16<'_, u32, LdsUninitialized>,
    lane: &WaveLane<Wave64>,
) {
    let _ = tile.write_wave_fragment(lane, [0; 4]);
}

fn main() {}
