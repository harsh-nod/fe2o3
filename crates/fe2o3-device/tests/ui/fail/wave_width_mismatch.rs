use fe2o3_device::{Wave32, Wave64, WaveLane};

fn requires_wave32(_: WaveLane<Wave32>) {}

fn wrong_width(lane: WaveLane<Wave64>) {
    requires_wave32(lane);
}

fn main() {}
