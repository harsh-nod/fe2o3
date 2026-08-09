use fe2o3_device::{Wave64, WaveLane};

fn zero(lane: WaveLane<Wave64>) {
    let _ = lane.into_subgroup_tile::<0>();
}

fn non_power_of_two(lane: WaveLane<Wave64>) {
    let _ = lane.into_subgroup_tile::<3>();
}

fn wider_than_wave(lane: WaveLane<Wave64>) {
    let _ = lane.into_subgroup_tile::<128>();
}

fn main() {}
