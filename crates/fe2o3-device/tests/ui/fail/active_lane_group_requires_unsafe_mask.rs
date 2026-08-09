use fe2o3_device::{Wave64, WaveLane};

fn active_group(lane: WaveLane<Wave64>, mask: u64) {
    let _ = lane.into_active_lane_group(mask);
}

fn main() {}
