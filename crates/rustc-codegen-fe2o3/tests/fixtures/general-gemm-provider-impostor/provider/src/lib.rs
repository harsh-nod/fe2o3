#![no_std]
#![feature(rustc_attrs)]
#![allow(internal_features)]

pub struct ProofSensitiveGeneralGemmWave64V1 {
    _sealed: (),
}

impl ProofSensitiveGeneralGemmWave64V1 {
    #[inline(always)]
    pub fn from_compiler(k: u32) -> Self {
        proof_acquire_gfx942_tiled_gemm_wave64_v1(k)
    }
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_acquire_v1"]
fn proof_acquire_gfx942_tiled_gemm_wave64_v1(k: u32) -> ProofSensitiveGeneralGemmWave64V1 {
    let _ = k;
    unreachable!("same-name provider has no GEMM authority")
}
