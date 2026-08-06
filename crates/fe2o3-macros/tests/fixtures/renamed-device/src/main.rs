use gpu_device::{KernelMarkerV1, kernel};

#[kernel]
pub fn renamed_device(value: u32) -> u32 {
    value
}

fn assert_marker<T: KernelMarkerV1>() {}

fn main() {
    assert_marker::<__fe2o3_kernel_marker_renamed_device>();
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_device as KernelMarkerV1>::LOGICAL_NAME,
        "renamed_device"
    );
    assert_eq!(
        <__fe2o3_kernel_marker_renamed_device as KernelMarkerV1>::REGISTRATION.2,
        1
    );
}
