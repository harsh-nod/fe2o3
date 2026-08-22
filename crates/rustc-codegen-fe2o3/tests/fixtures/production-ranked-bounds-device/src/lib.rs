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
    namespace = "733de473b3b81963ce2a3b6bc3b67bfc1eb309494ed6e36435de5bbab54f28f6"
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
    namespace = "426635fc4a465fce5a626e5cd0b3b287bbae25ea52f301b85663b550b63945b8"
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
    namespace = "79c5632b023afb6feb75aee40b5dd8651b6db739828a0ac9669799ca706e19a0"
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
    namespace = "5e13f6c884eb2e4fb53ae8ad285e07aebfd312c992620887c9ff32cc18207282"
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
    namespace = "e9af58d0521591b656a0bdfbd4bb0f9b27d702118d594ad63e01f30a5bcd5d82"
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
    namespace = "09db87689fbbae9d81a8f6df813c91acabe06ca91a15223a1d49d94268a85450"
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
    namespace = "6f36aa42e529752a3f46af005c5b289a215446de5ceec61fcd6d4a2f463610a3"
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
    namespace = "a39c65034cfbcc32984d8f4f8de9989637941735ef75c9d2aa3ac3a502e05ca0"
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
    namespace = "c23759cb39af0fba9c734ad713d879231dbc4e8701c6e165c5e00c23b6c02bd7"
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
    namespace = "956b6bc346a6d0b5f16d7bbc2abe4b699474b4985d61eafd7f6333bfc818e365"
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
    namespace = "dea205920007e6c762313a092e8c3245990d41a365da1c877810d50279404ebe"
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
    namespace = "90c815f83f1074e5b78c5ea1dfe21e31f02174335e2b622d890c16eaa15c5e1d"
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
    namespace = "8c442eb8bd35605ffdd9478a993befb8eb502f5fd35e5ecb58bc296e70b3d589"
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
