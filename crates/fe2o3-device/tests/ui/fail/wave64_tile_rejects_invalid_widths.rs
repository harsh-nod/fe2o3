use fe2o3_device::{SubgroupTile, Wave64, WaveLane};

fn zero(lane: WaveLane<Wave64>) {
    let _ = SubgroupTile::<0>::from_wave64_snapshot(&lane);
}

fn non_power_of_two(lane: WaveLane<Wave64>) {
    let _ = SubgroupTile::<3>::from_wave64_snapshot(&lane);
}

fn wider_than_wave(lane: WaveLane<Wave64>) {
    let _ = SubgroupTile::<128>::from_wave64_snapshot(&lane);
}

fn main() {}
