use fe2o3_device::{Wave32, WaveLane};

fn duplicate(lane: WaveLane<Wave32>) {
    let first = lane;
    let second = lane;
    let _ = (first, second);
}

fn main() {}
