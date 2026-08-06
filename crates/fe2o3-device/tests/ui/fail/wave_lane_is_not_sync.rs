use fe2o3_device::{Wave32, WaveLane};

fn assert_sync<T: Sync>() {}

fn main() {
    assert_sync::<WaveLane<Wave32>>();
}
