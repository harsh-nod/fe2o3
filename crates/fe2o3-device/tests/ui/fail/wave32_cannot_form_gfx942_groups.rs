use fe2o3_device::{Wave32, WaveLane};

fn tile(lane: WaveLane<Wave32>) {
    let _ = lane.into_subgroup_tile::<32>();
}

fn active(lane: WaveLane<Wave32>) {
    let _ = unsafe { lane.into_active_lane_group(u64::MAX) };
}

fn main() {}
