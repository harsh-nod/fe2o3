use fe2o3_device::{ActiveLaneGroup, SubgroupTile, Wave64, WaveLane};

fn escape_tile(lane: &WaveLane<Wave64>) -> SubgroupTile<'static, 32> {
    SubgroupTile::from_wave64_snapshot(lane)
}

fn escape_active(lane: &WaveLane<Wave64>) -> ActiveLaneGroup<'static> {
    unsafe { ActiveLaneGroup::from_caller_asserted_snapshot(lane, u64::MAX).unwrap() }
}

fn main() {}
