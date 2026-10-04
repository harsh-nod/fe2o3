#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_acquire_v1"]
fn proof_acquire_gfx942_tiled_gemm_wave64_v1(k: u32) -> ProofSensitiveGeneralGemmWave64V1 {
    let _ = phase_count(k);
    unreachable!("proof-sensitive GEMM authority requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_lane_v1"]
fn proof_lane_gfx942_tiled_gemm_wave64_v1(context: &ProofSensitiveGeneralGemmWave64V1) -> u32 {
    let _ = context;
    unreachable!("proof-sensitive GEMM lane identity requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_workgroup_x_v1"]
fn proof_workgroup_x_gfx942_tiled_gemm_wave64_v1(
    context: &ProofSensitiveGeneralGemmWave64V1,
) -> u32 {
    let _ = context;
    unreachable!("proof-sensitive GEMM workgroup X requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_workgroup_y_v1"]
fn proof_workgroup_y_gfx942_tiled_gemm_wave64_v1(
    context: &ProofSensitiveGeneralGemmWave64V1,
) -> u32 {
    let _ = context;
    unreachable!("proof-sensitive GEMM workgroup Y requires authenticated compiler analysis")
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_load_a_v1"]
fn proof_load_a_gfx942_tiled_gemm_wave64_v1(
    context: &ProofSensitiveGeneralGemmWave64V1,
    a: &[u16],
    row: u32,
    depth: u32,
    m: u32,
    k: u32,
    lda: u32,
) -> u16 {
    let _ = (context, a, row, depth, m, k, lda);
    unreachable!("proof-sensitive GEMM A load requires authenticated compiler analysis")
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_load_b_v1"]
fn proof_load_b_gfx942_tiled_gemm_wave64_v1(
    context: &ProofSensitiveGeneralGemmWave64V1,
    b: &[u16],
    depth: u32,
    column: u32,
    k: u32,
    n: u32,
    ldb: u32,
) -> u16 {
    let _ = (context, b, depth, column, k, n, ldb);
    unreachable!("proof-sensitive GEMM B load requires authenticated compiler analysis")
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
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_stage_value_v1"]
fn proof_stage_value_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    slot: u32,
    epoch: u32,
    depth: u32,
    k: u32,
    value: u16,
) {
    let _ = (context, slot, epoch, depth, k, value);
    unreachable!("proof-sensitive GEMM stage value requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_wait_stage_v1"]
fn proof_wait_stage_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    epoch: u32,
) {
    let _ = (context, epoch);
    unreachable!("proof-sensitive GEMM stage wait requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_read_stage_v1"]
fn proof_read_stage_gfx942_tiled_gemm_wave64_v1(
    context: &ProofSensitiveGeneralGemmWave64V1,
    slot: u32,
    epoch: u32,
) -> u16 {
    let _ = (context, slot, epoch);
    unreachable!("proof-sensitive GEMM LDS read requires authenticated compiler analysis")
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
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_mfma_value_v1"]
fn proof_mfma_value_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    lhs: u16,
    rhs: u16,
    prior: f32,
) -> f32 {
    let _ = (context, lhs, rhs, prior);
    unreachable!("proof-sensitive GEMM carried MFMA requires authenticated compiler analysis")
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

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_load_c_v1"]
fn proof_load_c_gfx942_tiled_gemm_wave64_v1(
    context: &ProofSensitiveGeneralGemmWave64V1,
    c: &DisjointSlice<f32>,
    row: u32,
    column: u32,
    m: u32,
    n: u32,
    ldc: u32,
) -> f32 {
    let _ = (context, c, row, column, m, n, ldc);
    unreachable!("proof-sensitive GEMM C load requires authenticated compiler analysis")
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_store_epilogue_v1"]
fn proof_store_epilogue_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    c: &mut DisjointSlice<f32>,
    row: u32,
    column: u32,
    m: u32,
    n: u32,
    ldc: u32,
    value: f32,
    alpha: f32,
    accumulator: f32,
    beta: f32,
    initial: f32,
) {
    let _ = (
        context,
        c,
        row,
        column,
        m,
        n,
        ldc,
        value,
        alpha,
        accumulator,
        beta,
        initial,
    );
    unreachable!("proof-sensitive GEMM epilogue store requires authenticated compiler analysis")
}
