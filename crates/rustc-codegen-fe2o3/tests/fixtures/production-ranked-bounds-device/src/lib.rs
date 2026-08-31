#![no_std]

#[cfg(feature = "device_math_sqrt")]
use fe2o3_device::DeviceMath;
#[cfg(any(
    feature = "grid_exclusive",
    feature = "write_only_grid_exclusive",
    feature = "barrier_divergent",
    feature = "barrier_early_return"
))]
use fe2o3_device::GridExclusive;
#[cfg(feature = "write_only_row_striped")]
use fe2o3_device::RowStriped2D;
#[cfg(feature = "shifted")]
use fe2o3_device::Shifted;
#[cfg(feature = "write_only_tiled")]
use fe2o3_device::Tiled2D;
#[cfg(any(
    feature = "write_only_grid_exclusive",
    feature = "write_only_blocked",
    feature = "write_only_blocked_dynamic_grid",
    feature = "write_only_tiled",
    feature = "write_only_row_striped"
))]
use fe2o3_device::WriteOnlyDisjointSlice;
#[cfg(any(
    feature = "barrier_after_access",
    feature = "barrier_before_access",
    feature = "barrier_divergent",
    feature = "barrier_early_return",
    feature = "barrier_loop",
    feature = "barrier_helper"
))]
use fe2o3_device::sync::syncthreads;
#[cfg(any(
    feature = "shifted",
    feature = "blocked",
    feature = "blocked_multi_lane",
    feature = "blocked_multi_block",
    feature = "blocked_multi_lane_dynamic_grid",
    feature = "write_only_blocked",
    feature = "write_only_blocked_dynamic_grid",
    feature = "write_only_tiled",
    feature = "write_only_row_striped",
    feature = "barrier_after_access",
    feature = "barrier_before_access",
    feature = "barrier_loop",
    feature = "barrier_helper"
))]
use fe2o3_device::{Blocked, Index1D};
use fe2o3_device::{DisjointSlice, kernel, thread};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(not(any(
    feature = "oob",
    feature = "debug_scalar",
    feature = "debug_long_name",
    feature = "debug_mutated_argument",
    feature = "shifted",
    feature = "grid_exclusive",
    feature = "blocked",
    feature = "blocked_multi_lane",
    feature = "blocked_multi_block",
    feature = "blocked_multi_lane_dynamic_grid",
    feature = "write_only_grid_exclusive",
    feature = "write_only_blocked",
    feature = "write_only_blocked_dynamic_grid",
    feature = "write_only_tiled",
    feature = "write_only_row_striped",
    feature = "barrier_after_access",
    feature = "barrier_before_access",
    feature = "barrier_divergent",
    feature = "barrier_early_return",
    feature = "barrier_loop",
    feature = "barrier_helper",
    feature = "device_math_sqrt"
)))]
pub fn copy_static(value: f32, mut output: DisjointSlice<f32>) {
    let input = [value; 64];
    let selected = input[63];
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = selected;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "device_math_sqrt")]
pub fn device_math_sqrt(value: f32, mut output: DisjointSlice<f32>) {
    let math = DeviceMath::current();
    let root = math.sqrt_f32(value);
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = root;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "debug_scalar")]
pub fn debug_scalar(value: f32, input: &[f32], mut output: DisjointSlice<f32>) {
    let _input_extent = input.len();
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "debug_long_name")]
pub fn debug_long_name(mut output: DisjointSlice<f32>) {
    let source_variable_name_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
        1.0;
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = source_variable_name_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "debug_mutated_argument")]
pub fn debug_mutated_argument(mut value: f32, mut output: DisjointSlice<f32>) {
    value = 2.0;
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "oob")]
#[allow(unconditional_panic)]
pub fn copy_static(value: f32, mut output: DisjointSlice<f32>) {
    let input = [value; 64];
    let selected = input[64];
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = selected;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "shifted")]
pub fn checked_shifted(mut output: DisjointSlice<f32, Shifted<Index1D, 4>>) {
    if let Some(index) = thread::index_1d().checked_shift::<4>() {
        if let Some(element) = output.get_disjoint_mut(index) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "grid_exclusive")]
pub fn grid_exclusive(mut output: DisjointSlice<f32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        if let Some(element) = output.get_mut_exclusive(&leader, 7) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
)]
#[cfg(feature = "blocked_multi_lane")]
pub fn blocked_multi_lane(mut output: DisjointSlice<f32, Blocked<Index1D, 64, 4>>) {
    if let Some(block) = thread::index_1d().checked_block::<64, 4>() {
        if let Some(element) = output.get_block_mut(&block, 3) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
)]
#[cfg(feature = "blocked_multi_block")]
pub fn blocked_multi_block(mut output: DisjointSlice<f32, Blocked<Index1D, 16, 4>>) {
    if let Some(block) = thread::index_1d().checked_block::<16, 4>() {
        if let Some(element) = output.get_block_mut(&block, 3) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "blocked_multi_lane_dynamic_grid")]
pub fn blocked_multi_lane_dynamic_grid(mut output: DisjointSlice<f32, Blocked<Index1D, 64, 4>>) {
    if let Some(block) = thread::index_1d().checked_block::<64, 4>() {
        if let Some(element) = output.get_block_mut(&block, 3) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "blocked")]
pub fn blocked(mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>) {
    if let Some(block) = thread::index_1d().checked_block::<1, 2>() {
        if let Some(element) = output.get_block_mut(&block, 1) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "write_only_grid_exclusive")]
pub fn write_only_grid_exclusive(mut output: WriteOnlyDisjointSlice<f32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        let _ = output.write_exclusive(&leader, 7, 1.0);
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
)]
#[cfg(feature = "write_only_blocked")]
pub fn write_only_blocked(mut output: WriteOnlyDisjointSlice<f32, Blocked<Index1D, 64, 4>>) {
    if let Some(block) = thread::index_1d().checked_block::<64, 4>() {
        let _ = output.write_block(&block, 3, 1.0);
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "write_only_blocked_dynamic_grid")]
pub fn write_only_blocked_dynamic_grid(
    mut output: WriteOnlyDisjointSlice<f32, Blocked<Index1D, 64, 4>>,
) {
    if let Some(block) = thread::index_1d().checked_block::<64, 4>() {
        let _ = output.write_block(&block, 3, 1.0);
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
)]
#[cfg(feature = "write_only_tiled")]
pub fn write_only_tiled(mut output: WriteOnlyDisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>) {
    if let Some(tile) = thread::index_1d().checked_tiled_2d::<64, 16, 16, 4>() {
        let _ = output.write_tiled_2d(&tile, 3, 16, 16, 16, 1.0);
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]),
)]
#[cfg(feature = "write_only_row_striped")]
pub fn write_only_row_striped(
    mut output: WriteOnlyDisjointSlice<f32, RowStriped2D<Index1D, 16, 4>>,
) {
    if let Some(stripe) = thread::index_1d().checked_row_striped_2d::<16, 4>() {
        let _ = output.write_row_striped_2d(&stripe, 3, 4, 64, 64, 1.0);
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "barrier_after_access")]
pub fn barrier_after_access(mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>) {
    if let Some(block) = thread::index_1d().checked_block::<1, 2>() {
        if let Some(element) = output.get_block_mut(&block, 1) {
            *element = 1.0;
        }
    }
    syncthreads();
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "barrier_before_access")]
pub fn barrier_before_access(mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>) {
    syncthreads();
    if let Some(block) = thread::index_1d().checked_block::<1, 2>() {
        if let Some(element) = output.get_block_mut(&block, 1) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "barrier_divergent")]
pub fn barrier_divergent(mut output: DisjointSlice<f32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        syncthreads();
        if let Some(element) = output.get_mut_exclusive(&leader, 0) {
            *element = 1.0;
        }
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "barrier_early_return")]
pub fn barrier_early_return(mut output: DisjointSlice<f32, GridExclusive>) {
    let Some(leader) = thread::grid_leader() else {
        return;
    };
    syncthreads();
    if let Some(element) = output.get_mut_exclusive(&leader, 0) {
        *element = 1.0;
    }
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(2)),
)]
#[cfg(feature = "barrier_loop")]
pub fn barrier_loop(mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>) {
    loop {
        syncthreads();
        if let Some(block) = thread::index_1d().checked_block::<1, 2>() {
            if let Some(element) = output.get_block_mut(&block, 1) {
                *element = 1.0;
                break;
            }
        }
    }
}

#[cfg(feature = "barrier_helper")]
#[inline(never)]
fn helper_barrier() {
    syncthreads();
}

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
)]
#[cfg(feature = "barrier_helper")]
pub fn barrier_helper(mut output: DisjointSlice<f32, Blocked<Index1D, 1, 2>>) {
    helper_barrier();
    if let Some(block) = thread::index_1d().checked_block::<1, 2>() {
        if let Some(element) = output.get_block_mut(&block, 1) {
            *element = 1.0;
        }
    }
}
