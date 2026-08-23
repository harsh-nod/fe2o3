use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaAMatrix, Bf16MfmaBFragment, Bf16MfmaBMatrix,
    DeviceMatrix, F32AccumulatorFragment, LdsInitialized, MfmaLdsTile16x16,
    MfmaOperandA, MfmaRowMajor, MfmaRowMajorXor4, RowMajorXor4, Wave64, WaveLane,
};

fn type_check_direct_matrix<'wave>(
    matrix: &DeviceMatrix,
    lane: &'wave WaveLane<Wave64>,
    lhs_bits: &[u16],
    rhs_bits: &[u16],
) {
    let lhs = Bf16MfmaAMatrix::row_major(lhs_bits, 0, 16, 16, 16)
        .unwrap()
        .load_m16k16(lane, 0, 0)
        .unwrap();
    let rhs = Bf16MfmaBMatrix::row_major(rhs_bits, 0, 16, 16, 16)
        .unwrap()
        .load_k16n16(lane, 0, 0)
        .unwrap();
    let accumulator = F32AccumulatorFragment::zero(lane);
    let _: F32AccumulatorFragment<'wave> =
        matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn type_check_initialized_tile<'wave>(
    tile: &MfmaLdsTile16x16<'_, MfmaOperandA, LdsInitialized>,
    lane: &'wave WaveLane<Wave64>,
) {
    let _: Bf16MfmaAFragment<'wave, MfmaRowMajorXor4> =
        tile.read_mfma_fragment(lane);
}

fn type_check_direct_fragment<'wave>(
    fragment: Bf16MfmaAFragment<'wave, MfmaRowMajor>,
) {
    let _: Bf16MfmaAFragment<'wave, MfmaRowMajor> = fragment;
}

fn main() {
    let _: Option<usize> = RowMajorXor4::physical_index(15, 15);
    let _ = type_check_direct_matrix;
    let _ = type_check_initialized_tile;
    let _ = type_check_direct_fragment;
    let _: core::marker::PhantomData<Bf16MfmaBFragment<'static, MfmaRowMajor>> =
        core::marker::PhantomData;
}
