use fe2o3_device::{
    BF16_F32_MFMA_M, BF16_F32_MFMA_N, BF16_F32_MFMA_REDUCTION, BF16_F32_MFMA_WAVE_LANES,
    Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMatrix, F32AccumulatorFragment,
    MfmaRowMajorXor4,
};
use fe2o3_tiled_gemm_v1::contract::{TILE_K_V1, TILE_M_V1, TILE_N_V1, WAVE_LANES_V1};
use fe2o3_tiled_gemm_v1::kernel_face::accumulate_fragment_v1;

#[test]
fn host_contract_reuses_the_device_matrix_profile_exactly() {
    assert_eq!(TILE_M_V1 as usize, BF16_F32_MFMA_M);
    assert_eq!(TILE_N_V1 as usize, BF16_F32_MFMA_N);
    assert_eq!(TILE_K_V1 as usize, BF16_F32_MFMA_REDUCTION);
    assert_eq!(WAVE_LANES_V1 as usize, BF16_F32_MFMA_WAVE_LANES);
}

#[test]
fn kernel_boundary_has_only_the_existing_fragment_signature() {
    let _: for<'wave> fn(
        &DeviceMatrix,
        Bf16MfmaAFragment<'wave, MfmaRowMajorXor4>,
        Bf16MfmaBFragment<'wave, MfmaRowMajorXor4>,
        F32AccumulatorFragment<'wave>,
    ) -> F32AccumulatorFragment<'wave> = accumulate_fragment_v1;
}

#[test]
fn source_contains_no_handwritten_intrinsic_or_host_emulation() {
    let source = include_str!("../src/kernel_face.rs");
    assert!(source.contains("matrix.multiply_accumulate(lhs, rhs, accumulator)"));
    for forbidden in [
        "llvm.amdgcn",
        "asm!",
        "global_asm!",
        "from_compiler()",
        "for lane in",
        "mul_add(",
    ] {
        assert!(!source.contains(forbidden), "found `{forbidden}`");
    }
}
