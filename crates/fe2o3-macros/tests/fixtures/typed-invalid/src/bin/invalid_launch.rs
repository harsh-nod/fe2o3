use gpu_device::kernel;

#[kernel(launch(required = [0, 1, 1]))]
fn zero_dimension() {}

#[kernel(launch(required = [64, 2, 1], max = [32, 2, 1]))]
fn conflicting_dimensions() {}

#[kernel(launch(
    required = [64, 1, 1],
    min_workgroups_per_compute_unit = 2
))]
fn occupancy_without_maximum() {}

#[kernel(launch(max = [64, 1, 1], max = [64, 1, 1]))]
fn duplicate_maximum() {}

#[kernel(typed, launch(max = [256, 1, 1]))]
pub fn general_maximum_only(value: u32) {
    let _ = value;
}

#[kernel(typed, launch(required = [128, 1, 1]))]
pub fn general_wrong_block(value: u32) {
    let _ = value;
}

fn main() {}
