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
    namespace = "409a087b69c0a4c4431f5bb659606feed29b109454cfb528f220397bf0349bcb"
)]
#[cfg(not(any(
    feature = "oob",
    feature = "shifted",
    feature = "grid_exclusive",
    feature = "production_safe",
    feature = "production_oob",
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
    namespace = "d5e6306fc6d085e809eda08b970bad03531d474a128843df69f1ed181e89fa1b"
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
    namespace = "be106fd078c3d060c4116b385a8baecd9304a4b92106105e8efcddd3174011bf"
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
    namespace = "3e9e4387be6f680bd9bce54206ec008602d61c3f115631ce6711f6501379c01c"
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
    namespace = "54c58fba8acb2f20497d528eb87c83d76e6af1dfe27f1d2778ffda8d69221fa9"
)]
#[cfg(feature = "production_safe")]
pub fn copy_static(value: f32, mut output: DisjointSlice<f32>) {
    let input = [value; 64];
    let selected = input[63];
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = selected;
    }
}

#[kernel(
    typed,
    namespace = "73adbf13666463e194459888d8d7c5084f35ebdb009511ac82225994ff184b2a"
)]
#[cfg(feature = "production_oob")]
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
    namespace = "b9fe29428bdba6d62863664f0f44be15fc465334eb1fcfa52ac1bb98c48abaf2"
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
