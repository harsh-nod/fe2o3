#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_acquire_v1"]
fn proof_acquire_gfx942_tiled_gemm_wave64_v1(k: u32) -> ProofSensitiveGeneralGemmWave64V1 {
    let _ = phase_count(k);
    unreachable!("proof-sensitive GEMM authority requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_stage_v1"]
fn proof_stage_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    a_bits: [u16; 4],
    b_bits: [u16; 4],
) {
    let _ = (context, a_bits, b_bits);
    unreachable!("proof-sensitive GEMM staging requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_publish_v1"]
fn proof_publish_gfx942_tiled_gemm_wave64_v1(context: &mut ProofSensitiveGeneralGemmWave64V1) {
    let _ = context;
    unreachable!("proof-sensitive GEMM publish requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_mfma_v1"]
fn proof_mfma_gfx942_tiled_gemm_wave64_v1(context: &mut ProofSensitiveGeneralGemmWave64V1) {
    let _ = context;
    unreachable!("proof-sensitive GEMM MFMA requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_reuse_v1"]
fn proof_reuse_gfx942_tiled_gemm_wave64_v1(context: &mut ProofSensitiveGeneralGemmWave64V1) {
    let _ = context;
    unreachable!("proof-sensitive GEMM reuse requires authenticated compiler analysis")
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_store_v1"]
fn proof_store_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    c: &mut DisjointSlice<f32>,
    m: u32,
    n: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
) {
    let _ = (context, c, m, n, ldc, alpha, beta);
    unreachable!("proof-sensitive GEMM stores require authenticated compiler analysis")
}
