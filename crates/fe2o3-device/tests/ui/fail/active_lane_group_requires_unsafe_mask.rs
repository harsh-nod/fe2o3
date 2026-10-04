use fe2o3_device::{ActiveLaneGroup, Wave64, WaveLane};

fn active_group(lane: WaveLane<Wave64>, mask: u64) {
    let _ = ActiveLaneGroup::from_caller_asserted_snapshot(&lane, mask);
}

fn main() {}
