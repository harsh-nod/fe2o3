use fe2o3_device::{
    Bf16, Bf16MfmaFragment, DeviceMatrix, F32AccumulatorFragment, LdsInitialized,
    LdsTile16x16, RowMajorXor4, Wave64, WaveLane,
};

fn type_check_matrix(
    matrix: &DeviceMatrix,
    lhs: Bf16MfmaFragment,
    rhs: Bf16MfmaFragment,
    accumulator: F32AccumulatorFragment,
) {
    let _: F32AccumulatorFragment = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn type_check_initialized_tile(
    tile: &LdsTile16x16<'_, Bf16, LdsInitialized>,
    lane: &WaveLane<Wave64>,
) {
    let _: Bf16MfmaFragment = tile.read_mfma_fragment(lane);
}

fn main() {
    let fragment = Bf16MfmaFragment::new([Bf16::ONE; 4]);
    let _: [Bf16; 4] = fragment.to_array();
    let _: Option<usize> = RowMajorXor4::physical_index(15, 15);
    let _: fn(
        &DeviceMatrix,
        Bf16MfmaFragment,
        Bf16MfmaFragment,
        F32AccumulatorFragment,
    ) = type_check_matrix;
    let _: fn(&LdsTile16x16<'_, Bf16, LdsInitialized>, &WaveLane<Wave64>) =
        type_check_initialized_tile;
}
