#![no_std]

#[cfg(feature = "grid_exclusive")]
use fe2o3_device::GridExclusive;
use fe2o3_device::{DisjointSlice, kernel, thread};
#[cfg(feature = "shifted")]
use fe2o3_device::{Index1D, Shifted};

#[kernel(
    typed,
    namespace = "3b396cb285ea8af892007da6ebb4e714a7f03969d1b07db3eaae179ef7100e27"
)]
#[cfg(not(any(feature = "oob", feature = "shifted", feature = "grid_exclusive")))]
pub fn copy_static(value: f32, mut output: DisjointSlice<f32>) {
    let input = [value; 64];
    let selected = input[63];
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = selected;
    }
}

#[kernel(
    typed,
    namespace = "24581859896c50358dec58673c853eed6805ed66208dc90186b0e9a7d5013117"
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
    namespace = "a2f15d56ba1c5a4d100743ffe976774c7e4b43bf37709788685b8bab5ed73570"
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
    namespace = "a7d280c45c82a757eae770bac6e64a6064ee4ce2ffbc1b5b0b7cca214e65d14d"
)]
#[cfg(feature = "grid_exclusive")]
pub fn grid_exclusive(mut output: DisjointSlice<f32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        if let Some(element) = output.get_mut_exclusive(&leader, 7) {
            *element = 1.0;
        }
    }
}
