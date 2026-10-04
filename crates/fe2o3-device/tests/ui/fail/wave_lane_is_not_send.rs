use fe2o3_device::{Wave32, WaveLane};

fn assert_send<T: Send>() {}

fn main() {
    assert_send::<WaveLane<Wave32>>();
}
