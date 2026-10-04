//! Domain-separated inert algorithm and candidate identities.

use sha2::{Digest, Sha256};

use crate::{
    LogitsEffectContractV1, LogitsEffectErrorV1, LogitsNumericalErrorV1, LogitsNumericalPolicyV1,
    LogitsPlanIdentityV1, LogitsProfileDescriptorV1, LogitsProfileErrorV1,
    LogitsResourceContractV1, ValidatedLogitsProfileV1, validate_logits_effect_contract_v1,
    validate_logits_numerical_policy_v1, validate_logits_profile_v1,
};

/// Algorithm identity domain separator.
pub const LOGITS_ALGORITHM_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.qwen3_logits_projection_argmax.algorithm.v1\0";
/// Candidate identity domain separator.
pub const LOGITS_CANDIDATE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.qwen3_logits_projection_argmax.candidate.v1\0";

/// One deterministic structural SHA-256 identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogitsStructuralIdentityV1([u8; 32]);

impl LogitsStructuralIdentityV1 {
    /// Returns all digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Complete public inert candidate descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogitsCandidateDescriptorV1 {
    /// Candidate schema version.
    pub schema_version: u16,
    /// Exact algorithm discriminator.
    pub algorithm: &'static str,
    /// Exact role/bucket/row profile.
    pub profile: LogitsProfileDescriptorV1,
    /// External generated-plan identity bound into this candidate.
    pub plan_identity: LogitsPlanIdentityV1,
    /// Exact numerical and exception policy.
    pub numerical: LogitsNumericalPolicyV1,
    /// Exact effect and publication contract.
    pub effects: LogitsEffectContractV1,
}

impl LogitsCandidateDescriptorV1 {
    /// Constructs the canonical candidate record.
    pub const fn canonical(
        profile: LogitsProfileDescriptorV1,
        plan_identity: LogitsPlanIdentityV1,
    ) -> Self {
        Self {
            schema_version: 1,
            algorithm: "qwen3-bf16-fp32-logits-lowest-argmax-compact",
            profile,
            plan_identity,
            numerical: LogitsNumericalPolicyV1::exact(),
            effects: LogitsEffectContractV1::exact(),
        }
    }
}

/// Candidate admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsCandidateErrorV1 {
    /// Schema version differed.
    SchemaVersion,
    /// Algorithm discriminator differed.
    Algorithm,
    /// Profile validation failed.
    Profile(LogitsProfileErrorV1),
    /// Plan identity was absent.
    MissingPlanIdentity,
    /// Numerical validation failed.
    Numerical(LogitsNumericalErrorV1),
    /// Effect validation failed.
    Effects(LogitsEffectErrorV1),
}

/// Validated copyable inert host/model candidate.
///
/// This carries no proof, compiler, artifact, load, launch, or completion
/// authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralLogitsCandidateV1 {
    profile: ValidatedLogitsProfileV1,
    plan_identity: LogitsPlanIdentityV1,
    algorithm_identity: LogitsStructuralIdentityV1,
    candidate_identity: LogitsStructuralIdentityV1,
}

impl StructuralLogitsCandidateV1 {
    /// Returns validated profile.
    pub const fn profile(self) -> ValidatedLogitsProfileV1 {
        self.profile
    }

    /// Returns checked resources.
    pub const fn resources(self) -> LogitsResourceContractV1 {
        self.profile.resources()
    }

    /// Returns bound external plan identity.
    pub const fn plan_identity(self) -> LogitsPlanIdentityV1 {
        self.plan_identity
    }

    /// Returns algorithm identity.
    pub const fn algorithm_identity(self) -> LogitsStructuralIdentityV1 {
        self.algorithm_identity
    }

    /// Returns full candidate identity.
    pub const fn candidate_identity(self) -> LogitsStructuralIdentityV1 {
        self.candidate_identity
    }
}

fn encode_profile(profile: LogitsProfileDescriptorV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(72);
    bytes.push(profile.role.identity_tag());
    bytes.push(profile.mode as u8);
    bytes.push(profile.bucket.identity_tag());
    for value in [
        profile.sequences,
        profile.active_tokens,
        profile.rows,
        profile.hidden_size,
        profile.vocabulary_size,
        profile.speculative_k,
    ] {
        bytes.extend_from_slice(&(value as u64).to_le_bytes());
    }
    bytes
}

fn encode_numerical(policy: LogitsNumericalPolicyV1) -> [u8; 9] {
    [
        u8::from(policy.activation_bf16),
        u8::from(policy.weight_bf16),
        u8::from(policy.bias_absent),
        u8::from(policy.ascending_hidden_separate_fp32_mul_add),
        u8::from(policy.contraction_disabled),
        u8::from(policy.reject_non_finite),
        u8::from(policy.token_ids_ascending),
        u8::from(policy.replace_only_on_strict_greater),
        u8::from(policy.lowest_token_id_tie_break),
    ]
}

fn encode_effects(effects: LogitsEffectContractV1) -> [u8; 8] {
    [
        u8::from(effects.inputs_initialized_read_only),
        u8::from(effects.streamed_logit_consumption),
        u8::from(effects.unique_logit_coordinates),
        u8::from(effects.unique_compact_record_writers),
        u8::from(effects.transactional_output),
        u8::from(effects.output_disjoint_from_inputs),
        effects.atomics,
        effects.barriers,
    ]
}

fn hash(domain: &[u8], parts: &[&[u8]]) -> LogitsStructuralIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    LogitsStructuralIdentityV1(hasher.finalize().into())
}

/// Strictly validates a descriptor and constructs inert identities.
pub fn admit_logits_candidate_v1(
    descriptor: LogitsCandidateDescriptorV1,
) -> Result<StructuralLogitsCandidateV1, LogitsCandidateErrorV1> {
    if descriptor.schema_version != 1 {
        return Err(LogitsCandidateErrorV1::SchemaVersion);
    }
    if descriptor.algorithm != "qwen3-bf16-fp32-logits-lowest-argmax-compact" {
        return Err(LogitsCandidateErrorV1::Algorithm);
    }
    let profile =
        validate_logits_profile_v1(descriptor.profile).map_err(LogitsCandidateErrorV1::Profile)?;
    if !descriptor.plan_identity.is_present() {
        return Err(LogitsCandidateErrorV1::MissingPlanIdentity);
    }
    validate_logits_numerical_policy_v1(descriptor.numerical)
        .map_err(LogitsCandidateErrorV1::Numerical)?;
    validate_logits_effect_contract_v1(descriptor.effects)
        .map_err(LogitsCandidateErrorV1::Effects)?;

    let profile_bytes = encode_profile(descriptor.profile);
    let numerical = encode_numerical(descriptor.numerical);
    let algorithm_identity = hash(
        LOGITS_ALGORITHM_IDENTITY_DOMAIN_V1,
        &[descriptor.algorithm.as_bytes(), &profile_bytes, &numerical],
    );
    let schema = descriptor.schema_version.to_le_bytes();
    let effects = encode_effects(descriptor.effects);
    let candidate_identity = hash(
        LOGITS_CANDIDATE_IDENTITY_DOMAIN_V1,
        &[
            &schema,
            &algorithm_identity.bytes(),
            &descriptor.plan_identity.0,
            &effects,
        ],
    );
    Ok(StructuralLogitsCandidateV1 {
        profile,
        plan_identity: descriptor.plan_identity,
        algorithm_identity,
        candidate_identity,
    })
}
