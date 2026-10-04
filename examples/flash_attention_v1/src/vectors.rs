//! Deterministic finite vectors for the exact causal profile.

use crate::contract::FLASH_ATTENTION_INPUT_ELEMENTS_V1;

/// One deterministic Q/K/V corpus entry.
#[derive(Clone, Debug, PartialEq)]
pub struct DeterministicVectorV1 {
    /// Stable vector name.
    pub name: &'static str,
    /// Contiguous row-major Q tensor.
    pub q: [f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1],
    /// Contiguous row-major K tensor.
    pub k: [f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1],
    /// Contiguous row-major V tensor.
    pub v: [f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1],
}

fn nominal_v1() -> DeterministicVectorV1 {
    let mut q = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut k = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut v = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    for index in 0..FLASH_ATTENTION_INPUT_ELEMENTS_V1 {
        q[index] = ((index * 17 + 3) % 29) as f32 / 16.0 - 0.75;
        k[index] = ((index * 11 + 5) % 31) as f32 / 20.0 - 0.70;
        v[index] = ((index * 7 + 2) % 37) as f32 / 12.0 - 1.25;
    }
    DeterministicVectorV1 {
        name: "nominal-mixed-sign",
        q,
        k,
        v,
    }
}

fn all_equal_logits_v1() -> DeterministicVectorV1 {
    let q = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut k = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut v = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    for index in 0..FLASH_ATTENTION_INPUT_ELEMENTS_V1 {
        k[index] = (index % 19) as f32 - 9.0;
        v[index] = (index / 16) as f32 * 0.5 + (index % 16) as f32 * 0.03125;
    }
    DeterministicVectorV1 {
        name: "all-equal-logits",
        q,
        k,
        v,
    }
}

fn dominant_logits_v1() -> DeterministicVectorV1 {
    let mut q = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut k = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut v = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    for row in 0..8 {
        q[row * 16] = 1.0;
        k[row * 16] = row as f32 * 64.0;
        for column in 0..16 {
            v[row * 16 + column] = row as f32 * 10.0 + column as f32;
        }
    }
    DeterministicVectorV1 {
        name: "latest-causal-key-dominates",
        q,
        k,
        v,
    }
}

fn causal_weight_probe_v1() -> DeterministicVectorV1 {
    let mut q = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut k = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    let mut v = [0.0_f32; FLASH_ATTENTION_INPUT_ELEMENTS_V1];
    for row in 0..8 {
        q[row * 16 + row] = 0.5;
        k[row * 16 + row] = 0.75;
        v[row * 16 + row] = 1.0;
    }
    DeterministicVectorV1 {
        name: "causal-mask-weight-probe",
        q,
        k,
        v,
    }
}

/// Returns the complete deterministic positive corpus.
pub fn deterministic_vectors_v1() -> [DeterministicVectorV1; 4] {
    [
        nominal_v1(),
        all_equal_logits_v1(),
        dominant_logits_v1(),
        causal_weight_probe_v1(),
    ]
}
