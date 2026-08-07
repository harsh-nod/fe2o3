use gpu_device::{KernelMarkerV1, kernel};

#[kernel]
pub fn renamed_device(value: u32) -> u32 {
    value
}

#[kernel(launch(
    required = [256, 1, 1],
    max = [256, 1, 1],
    min_workgroups_per_compute_unit = 2
))]
pub fn launch_bounded(value: u32) -> u32 {
    value
}

#[kernel(unsafe_asm(
    target = "gfx942",
    operands(sgpr, immediate),
    options(nomem, pure, nostack),
    effects(none)
))]
pub unsafe fn assembly_declared(value: u32) -> u32 {
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
    assert_marker::<__fe2o3_kernel_marker_launch_bounded>();
    assert_marker::<__fe2o3_kernel_marker_assembly_declared>();
}
