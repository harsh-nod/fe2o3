use fe2o3_device::{ActiveLaneGroup, SubgroupTile, Wave32, WaveLane};

fn tile(lane: WaveLane<Wave32>) {
    let _ = SubgroupTile::<32>::from_wave64_snapshot(&lane);
}

fn active(lane: WaveLane<Wave32>) {
    let _ = unsafe { ActiveLaneGroup::from_caller_asserted_snapshot(&lane, u64::MAX) };
}

fn main() {}
