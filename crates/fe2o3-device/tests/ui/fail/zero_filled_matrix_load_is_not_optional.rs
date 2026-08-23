#![feature(rustc_attrs)]
#![allow(internal_features)]

use fe2o3_device::{Bf16MfmaAFragment, Bf16MfmaAMatrix, Wave64, WaveLane};

fn optional_load<'data, 'wave>(
    matrix: &'data Bf16MfmaAMatrix<'data>,
    lane: &'wave WaveLane<Wave64>,
) -> Option<Bf16MfmaAFragment<'wave>> {
    matrix.load_m16k16(lane, usize::MAX, usize::MAX)
}

fn main() {}
