#![no_std]

#[cfg(any(
    feature = "grid_exclusive",
    feature = "barrier_divergent",
    feature = "barrier_early_return"
))]
use fe2o3_device::GridExclusive;
#[cfg(feature = "shifted")]
use fe2o3_device::Shifted;
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
    namespace = "48fb4bcf479bf2a11a1b1efd9a3bc4c3a8621c90e0678c0975f5af56ac5a0a95"
)]
#[cfg(not(any(
    feature = "oob",
    feature = "shifted",
    feature = "grid_exclusive",
    feature = "blocked",
    feature = "barrier_after_access",
    feature = "barrier_before_access",
    feature = "barrier_divergent",
    feature = "barrier_early_return",
    feature = "barrier_loop",
    feature = "barrier_helper"
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
    namespace = "470f5285b02bb6a6a44f8fc1b6eed398da1aba14f10994e6f55ab6c9d1402d76"
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
    namespace = "bc19535a7545ef14cad09ec24a34f27528a3d11ef1578f275387c73a5196a518"
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
    namespace = "985200fee1788778fd86d4c69071d74bba7399e981f45964832a71ec8ab4d83f"
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
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    namespace = "88aaf4adb87cd2b6797b984dc70210af6f35ce38286623232fe84b475664e3c9"
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
    namespace = "a5538b13de02007c2a5e2dbf0f8411b9e3fad5443c6e870fa5241170a36d8b8a"
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
    namespace = "bffc6b541ab2d7b61ac7abf7291e53b445022e06a329a222afaae0a0f0efcb46"
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
    namespace = "7dfb796b56d8d965f325ac68cf055928fbdcdd59b944a8798e4cfdf62be7e506"
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
    namespace = "510446d73bb1342ad48dfd8662d46b156b78b13255b9546996dd713edb518dde"
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
    namespace = "bf169e1721587032b7790154b762e4c075ca954ae46d4b6a290e250d1284ab57"
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
    namespace = "da4bb8bd1232ae9e51a47d988b354fd9011c1788d3ba34bc4ab0fa3f8105055e"
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
