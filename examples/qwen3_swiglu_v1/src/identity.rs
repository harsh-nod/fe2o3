//! Domain-separated inert identities for the structural foundation.

use sha2::{Digest, Sha256};

use crate::{
    GFX942_PROCESSOR_V1, GFX942_TARGET_FEATURES_V1, SwiGluBufferBindingV1,
    ValidatedSwiGluCandidateV1, ValidatedSwiGluProfileV1,
};

const PROFILE_DOMAIN_V1: &[u8] = b"fe2o3.qwen3.swiglu.profile.v1";
const ALGORITHM_DOMAIN_V1: &[u8] = b"fe2o3.qwen3.swiglu.algorithm.v1";
const SCHEDULE_DOMAIN_V1: &[u8] = b"fe2o3.qwen3.swiglu.schedule.v1";
const CANDIDATE_DOMAIN_V1: &[u8] = b"fe2o3.qwen3.swiglu.candidate.v1";

/// Exact 32-byte inert identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct SwiGluIdentityV1([u8; 32]);

impl SwiGluIdentityV1 {
    /// Returns the complete digest bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Complete set of identities for one validated inert candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SwiGluIdentityBundleV1 {
    /// Exact role/bucket/shape identity.
    pub profile: SwiGluIdentityV1,
    /// Exact BF16/FP32 evaluation-order identity.
    pub algorithm: SwiGluIdentityV1,
    /// Exact gfx942 structural schedule identity.
    pub schedule: SwiGluIdentityV1,
    /// Aggregate invocation candidate identity.
    pub candidate: SwiGluIdentityV1,
}

fn update_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(
        u64::try_from(bytes.len())
            .expect("bounded canonical identity field fits u64")
            .to_le_bytes(),
    );
    hasher.update(bytes);
}

fn update_u64(hasher: &mut Sha256, value: usize) {
    update_field(
        hasher,
        &u64::try_from(value)
            .expect("validated B3 dimension fits u64")
            .to_le_bytes(),
    );
}

fn finish(hasher: Sha256) -> SwiGluIdentityV1 {
    SwiGluIdentityV1(hasher.finalize().into())
}

/// Computes the exact validated role/bucket/shape identity.
pub fn swiglu_profile_identity_v1(profile: ValidatedSwiGluProfileV1) -> SwiGluIdentityV1 {
    let descriptor = profile.descriptor();
    let mut hasher = Sha256::new();
    update_field(&mut hasher, PROFILE_DOMAIN_V1);
    update_field(&mut hasher, &[descriptor.role.identity_tag()]);
    update_field(&mut hasher, &[descriptor.bucket.identity_tag()]);
    for value in [
        descriptor.sequences,
        descriptor.active_tokens,
        descriptor.rows,
        descriptor.hidden_size,
        descriptor.intermediate_size,
    ] {
        update_u64(&mut hasher, value);
    }
    finish(hasher)
}

/// Computes the exact evaluation-order identity.
pub fn swiglu_algorithm_identity_v1() -> SwiGluIdentityV1 {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, ALGORITHM_DOMAIN_V1);
    update_field(&mut hasher, b"gate=bf16_to_f32");
    update_field(&mut hasher, b"up=bf16_to_f32");
    update_field(
        &mut hasher,
        b"sigmoid=gate>=0?1_f32/(1_f32+exp_f32(-gate)):exp_f32(gate)/(1_f32+exp_f32(gate))",
    );
    update_field(&mut hasher, b"silu=gate*sigmoid");
    update_field(&mut hasher, b"activated=bf16_rne(silu*up)");
    update_field(&mut hasher, b"reject_non_finite=true");
    finish(hasher)
}

/// Computes the exact gfx942 structural schedule identity.
pub fn swiglu_schedule_identity_v1(candidate: ValidatedSwiGluCandidateV1) -> SwiGluIdentityV1 {
    let schedule = candidate.descriptor().schedule;
    let resources = candidate.profile().resources();
    let mut hasher = Sha256::new();
    update_field(&mut hasher, SCHEDULE_DOMAIN_V1);
    update_field(&mut hasher, GFX942_PROCESSOR_V1.as_bytes());
    update_field(&mut hasher, GFX942_TARGET_FEATURES_V1.as_bytes());
    update_field(&mut hasher, &schedule.threads_per_workgroup.to_le_bytes());
    update_field(&mut hasher, &[schedule.elements_per_thread]);
    update_field(&mut hasher, &schedule.lds_bytes_per_workgroup.to_le_bytes());
    update_field(&mut hasher, &[schedule.barriers_per_workgroup]);
    update_u64(&mut hasher, resources.workgroups);
    update_u64(&mut hasher, resources.elements);
    finish(hasher)
}

fn update_binding(hasher: &mut Sha256, binding: SwiGluBufferBindingV1) {
    update_field(hasher, &binding.allocation_id.to_le_bytes());
    update_field(hasher, &binding.generation.to_le_bytes());
    update_field(hasher, &binding.byte_offset.to_le_bytes());
    update_field(hasher, &binding.byte_len.to_le_bytes());
}

/// Computes all exact inert identities for a validated candidate.
pub fn swiglu_identity_bundle_v1(candidate: ValidatedSwiGluCandidateV1) -> SwiGluIdentityBundleV1 {
    let profile = swiglu_profile_identity_v1(candidate.profile());
    let algorithm = swiglu_algorithm_identity_v1();
    let schedule = swiglu_schedule_identity_v1(candidate);
    let descriptor = candidate.descriptor();
    let mut hasher = Sha256::new();
    update_field(&mut hasher, CANDIDATE_DOMAIN_V1);
    update_field(&mut hasher, profile.as_bytes());
    update_field(&mut hasher, algorithm.as_bytes());
    update_field(&mut hasher, schedule.as_bytes());
    update_binding(&mut hasher, descriptor.gate);
    update_binding(&mut hasher, descriptor.up);
    update_binding(&mut hasher, descriptor.activated);
    SwiGluIdentityBundleV1 {
        profile,
        algorithm,
        schedule,
        candidate: finish(hasher),
    }
}
