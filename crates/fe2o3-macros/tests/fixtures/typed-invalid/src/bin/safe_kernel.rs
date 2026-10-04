#![forbid(unsafe_code)]

use gpu_device::{KernelMarkerV1, kernel};

#[kernel]
pub fn safe_increment(value: u32) -> u32 {
    value + 1
}

#[kernel(launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn safe_launch_bounded(value: u32) -> u32 {
    value * 2
}

fn assert_marker<T: KernelMarkerV1>() {}

fn main() {
    assert_marker::<__fe2o3_kernel_marker_safe_increment>();
    assert_marker::<__fe2o3_kernel_marker_safe_launch_bounded>();
}
