#![no_std]
#![feature(rustc_attrs)]
#![allow(internal_features)]
#![allow(dead_code)]

use fe2o3_device::DisjointSlice;

pub struct ProofSensitiveGeneralGemmWave64V1 {
    _sealed: (),
}

impl ProofSensitiveGeneralGemmWave64V1 {
    #[inline(always)]
    pub fn from_compiler(k: u32) -> Self {
        proof_acquire_gfx942_tiled_gemm_wave64_v1(k)
    }
}

const fn phase_count(k: u32) -> u32 {
    k / 16 + if k.is_multiple_of(16) { 0 } else { 1 }
}

// Reuses the exact reviewed terminal source and source spans. Authentication
// must still reject this different same-name package by compiled-crate identity.
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../examples/tiled_gemm_general_v1/device-api/src/proof_sensitive_terminals.rs"
));
