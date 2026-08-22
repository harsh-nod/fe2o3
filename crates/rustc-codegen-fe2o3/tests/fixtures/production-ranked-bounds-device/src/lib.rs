#![no_std]

#[cfg(feature = "grid_exclusive")]
use fe2o3_device::GridExclusive;
#[cfg(feature = "blocked")]
use fe2o3_device::{Blocked, Index1D};
use fe2o3_device::{DisjointSlice, kernel, thread};
#[cfg(feature = "shifted")]
use fe2o3_device::{Index1D, Shifted};

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
    feature = "blocked"
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
