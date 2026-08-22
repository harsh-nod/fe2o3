#![no_std]

#[cfg(feature = "grid_exclusive")]
use fe2o3_device::GridExclusive;
use fe2o3_device::{DisjointSlice, kernel, thread};
#[cfg(feature = "shifted")]
use fe2o3_device::{Index1D, Shifted};

#[kernel(
    typed,
    namespace = "a130f76c1071b4a38d95d4243086735784efd11000f30a04ed4beaf1e6ef67d6"
)]
#[cfg(not(any(
    feature = "oob",
    feature = "shifted",
    feature = "grid_exclusive",
    feature = "production_safe",
    feature = "production_oob"
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
    namespace = "2071184a44bd1eb8257e5623dff35c629e517cfb6aaffb641f7c3ff32f573fc9"
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
    namespace = "a1c8d65df28d93d3567e3d99058431b0ada97a60637c83b5375953a36e5a6972"
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
    namespace = "0a5525deac37d1126cd5a1a817512d66b7338591f913c397afffb26bc027fa01"
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
