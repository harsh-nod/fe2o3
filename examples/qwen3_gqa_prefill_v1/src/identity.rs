//! Canonical inert algorithm, evaluation, and candidate identities.

use sha2::{Digest, Sha256};

use crate::{
    GqaEffectContractV1, GqaEffectErrorV1, GqaNumericalErrorV1, GqaNumericalPolicyV1,
    GqaPrefillProfileDescriptorV1, GqaProfileErrorV1, GqaResourceContractV1,
    ValidatedGqaPrefillProfileV1, validate_gqa_effect_contract_v1,
    validate_gqa_numerical_policy_v1, validate_gqa_prefill_profile_v1,
};

/// Algorithm identity domain separator.
pub const GQA_ALGORITHM_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.qwen3_gqa_prefill.algorithm.v1\0";
/// Evaluation-order identity domain separator.
pub const GQA_EVALUATION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.qwen3_gqa_prefill.evaluation.v1\0";
/// Complete candidate identity domain separator.
pub const GQA_CANDIDATE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.qwen3_gqa_prefill.candidate.v1\0";

/// One SHA-256 identity over canonical structural fields.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GqaStructuralIdentityV1([u8; 32]);

impl GqaStructuralIdentityV1 {
    /// Returns the complete digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Query/output tensor layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum QueryLayoutV1 {
    /// Contiguous `[sequence][token][query_head][feature]`.
    SequenceTokenHeadFeature = 1,
    /// Unsupported head-major layout.
    SequenceHeadTokenFeature = 2,
}

/// Key/value tensor layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum KvLayoutV1 {
    /// Contiguous `[sequence][token][kv_head][feature]`.
    SequenceTokenHeadFeature = 1,
    /// Unsupported head-major layout.
    SequenceHeadTokenFeature = 2,
}

/// Outer host iteration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VectorOrderV1 {
    /// Sequence, query token, then query head, all ascending.
    SequenceTokenQueryHeadAscending = 1,
    /// Unsupported query-head-major order.
    SequenceQueryHeadTokenAscending = 2,
}

/// GQA query-to-KV mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GqaHeadMappingV1 {
    /// `kv_head = query_head / query_heads_per_kv_head`.
    ContiguousQuotient = 1,
    /// Unsupported modulo mapping.
    Modulo = 2,
}

/// Public inert host evaluation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaEvaluationDescriptorV1 {
    /// Evaluation schema version.
    pub schema_version: u16,
    /// Query and output layout.
    pub query_output_layout: QueryLayoutV1,
    /// Key and value layout.
    pub key_value_layout: KvLayoutV1,
    /// Complete-output iteration order.
    pub vector_order: VectorOrderV1,
    /// GQA query-head mapping.
    pub head_mapping: GqaHeadMappingV1,
    /// Whether causal keys are visited from zero upward.
    pub causal_keys_ascending: bool,
    /// Whether QK features are visited from zero upward.
    pub qk_features_ascending: bool,
    /// Whether output features are visited from zero upward.
    pub output_features_ascending: bool,
    /// Number of bounded FP32 scratch arrays of token length.
    pub token_scratch_arrays: u8,
    /// Whether complete output uses a separate transactional buffer.
    pub separate_output_staging: bool,
}

impl GqaEvaluationDescriptorV1 {
    /// Returns the only admitted host evaluation record.
    pub const fn exact() -> Self {
        Self {
            schema_version: 1,
            query_output_layout: QueryLayoutV1::SequenceTokenHeadFeature,
            key_value_layout: KvLayoutV1::SequenceTokenHeadFeature,
            vector_order: VectorOrderV1::SequenceTokenQueryHeadAscending,
            head_mapping: GqaHeadMappingV1::ContiguousQuotient,
            causal_keys_ascending: true,
            qk_features_ascending: true,
            output_features_ascending: true,
            token_scratch_arrays: 2,
            separate_output_staging: true,
        }
    }
}

/// Independent evaluation-record mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaEvaluationErrorV1 {
    /// Schema version differed.
    SchemaVersion,
    /// Query/output layout differed.
    QueryOutputLayout,
    /// Key/value layout differed.
    KeyValueLayout,
    /// Outer vector order differed.
    VectorOrder,
    /// GQA head mapping differed.
    HeadMapping,
    /// Causal key order differed.
    CausalKeyOrder,
    /// QK feature order differed.
    QkFeatureOrder,
    /// Output feature order differed.
    OutputFeatureOrder,
    /// Scratch inventory differed.
    ScratchInventory,
    /// Transactional staging was removed.
    OutputStaging,
}

/// Validates every independent host evaluation axis.
pub fn validate_gqa_evaluation_descriptor_v1(
    evaluation: GqaEvaluationDescriptorV1,
) -> Result<(), GqaEvaluationErrorV1> {
    let exact = GqaEvaluationDescriptorV1::exact();
    if evaluation.schema_version != exact.schema_version {
        return Err(GqaEvaluationErrorV1::SchemaVersion);
    }
    if evaluation.query_output_layout != exact.query_output_layout {
        return Err(GqaEvaluationErrorV1::QueryOutputLayout);
    }
    if evaluation.key_value_layout != exact.key_value_layout {
        return Err(GqaEvaluationErrorV1::KeyValueLayout);
    }
    if evaluation.vector_order != exact.vector_order {
        return Err(GqaEvaluationErrorV1::VectorOrder);
    }
    if evaluation.head_mapping != exact.head_mapping {
        return Err(GqaEvaluationErrorV1::HeadMapping);
    }
    if !evaluation.causal_keys_ascending {
        return Err(GqaEvaluationErrorV1::CausalKeyOrder);
    }
    if !evaluation.qk_features_ascending {
        return Err(GqaEvaluationErrorV1::QkFeatureOrder);
    }
    if !evaluation.output_features_ascending {
        return Err(GqaEvaluationErrorV1::OutputFeatureOrder);
    }
    if evaluation.token_scratch_arrays != exact.token_scratch_arrays {
        return Err(GqaEvaluationErrorV1::ScratchInventory);
    }
    if !evaluation.separate_output_staging {
        return Err(GqaEvaluationErrorV1::OutputStaging);
    }
    Ok(())
}

/// Complete public record used to request an inert candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaCandidateDescriptorV1 {
    /// Candidate schema version.
    pub schema_version: u16,
    /// Algorithm discriminator.
    pub algorithm: &'static str,
    /// Exact role/bucket/geometry profile.
    pub profile: GqaPrefillProfileDescriptorV1,
    /// Exact numerical/order/exception policy.
    pub numerical: GqaNumericalPolicyV1,
    /// Exact logical effect contract.
    pub effects: GqaEffectContractV1,
    /// Exact host evaluation structure.
    pub evaluation: GqaEvaluationDescriptorV1,
}

impl GqaCandidateDescriptorV1 {
    /// Constructs the canonical record for one exact profile.
    pub const fn canonical(profile: GqaPrefillProfileDescriptorV1) -> Self {
        Self {
            schema_version: 1,
            algorithm: "qwen3-bf16-fp32-causal-gqa-prefill",
            profile,
            numerical: GqaNumericalPolicyV1::exact(),
            effects: GqaEffectContractV1::exact(),
            evaluation: GqaEvaluationDescriptorV1::exact(),
        }
    }
}

/// Fail-closed structural candidate error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaCandidateErrorV1 {
    /// Candidate schema differed.
    SchemaVersion,
    /// Algorithm discriminator differed.
    Algorithm,
    /// Profile validation failed.
    Profile(GqaProfileErrorV1),
    /// Numerical-policy validation failed.
    Numerical(GqaNumericalErrorV1),
    /// Effect validation failed.
    Effects(GqaEffectErrorV1),
    /// Evaluation validation failed.
    Evaluation(GqaEvaluationErrorV1),
}

/// Validated copyable inert host/model candidate.
///
/// This type owns no proof evidence, compiler graph, GPU program, artifact,
/// load handle, or launch capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralGqaPrefillCandidateV1 {
    profile: ValidatedGqaPrefillProfileV1,
    evaluation: GqaEvaluationDescriptorV1,
    algorithm_identity: GqaStructuralIdentityV1,
    evaluation_identity: GqaStructuralIdentityV1,
    candidate_identity: GqaStructuralIdentityV1,
}

impl StructuralGqaPrefillCandidateV1 {
    /// Returns the validated profile.
    pub const fn profile(self) -> ValidatedGqaPrefillProfileV1 {
        self.profile
    }

    /// Returns the checked resources.
    pub const fn resources(self) -> GqaResourceContractV1 {
        self.profile.resources()
    }

    /// Returns the inert host evaluation record.
    pub const fn evaluation(self) -> GqaEvaluationDescriptorV1 {
        self.evaluation
    }

    /// Returns the canonical algorithm identity.
    pub const fn algorithm_identity(self) -> GqaStructuralIdentityV1 {
        self.algorithm_identity
    }

    /// Returns the evaluation identity, which binds the algorithm identity.
    pub const fn evaluation_identity(self) -> GqaStructuralIdentityV1 {
        self.evaluation_identity
    }

    /// Returns the complete candidate identity.
    pub const fn candidate_identity(self) -> GqaStructuralIdentityV1 {
        self.candidate_identity
    }

    /// Reports that the copyable record grants no production authority.
    pub const fn grants_production_authority(self) -> bool {
        false
    }
}

fn hash_fields(domain: &[u8], fields: &[&[u8]]) -> GqaStructuralIdentityV1 {
    let mut hash = Sha256::new();
    hash.update(domain);
    for field in fields {
        hash.update((field.len() as u64).to_le_bytes());
        hash.update(field);
    }
    GqaStructuralIdentityV1(hash.finalize().into())
}

fn algorithm_identity(descriptor: GqaCandidateDescriptorV1) -> GqaStructuralIdentityV1 {
    let profile = descriptor.profile;
    let geometry = profile.geometry;
    let numerical = descriptor.numerical;
    hash_fields(
        GQA_ALGORITHM_IDENTITY_DOMAIN_V1,
        &[
            descriptor.algorithm.as_bytes(),
            &[profile.role.identity_tag()],
            &[profile.bucket.identity_tag()],
            &(profile.sequences as u64).to_le_bytes(),
            &(profile.active_tokens as u64).to_le_bytes(),
            &(profile.context_tokens as u64).to_le_bytes(),
            &(geometry.hidden_size as u64).to_le_bytes(),
            &(geometry.query_heads as u64).to_le_bytes(),
            &(geometry.kv_heads as u64).to_le_bytes(),
            &(geometry.head_dimension as u64).to_le_bytes(),
            &(geometry.query_heads_per_kv_head as u64).to_le_bytes(),
            &(geometry.query_projection_size as u64).to_le_bytes(),
            &(geometry.kv_projection_size as u64).to_le_bytes(),
            &[profile.tensor_stage as u8],
            &numerical.attention_scale_bits.to_le_bytes(),
            &[numerical.causal as u8],
            &[numerical.score as u8],
            &[numerical.softmax as u8],
            &[numerical.exponential as u8],
            &[numerical.value as u8],
            &[numerical.output_cast as u8],
            &[u8::from(numerical.reject_non_finite_inputs)],
            &[u8::from(numerical.reject_non_finite_intermediates)],
            &[u8::from(numerical.allow_exponential_underflow)],
        ],
    )
}

fn evaluation_identity(
    algorithm: GqaStructuralIdentityV1,
    descriptor: GqaCandidateDescriptorV1,
) -> GqaStructuralIdentityV1 {
    let evaluation = descriptor.evaluation;
    hash_fields(
        GQA_EVALUATION_IDENTITY_DOMAIN_V1,
        &[
            &algorithm.bytes(),
            &evaluation.schema_version.to_le_bytes(),
            &[evaluation.query_output_layout as u8],
            &[evaluation.key_value_layout as u8],
            &[evaluation.vector_order as u8],
            &[evaluation.head_mapping as u8],
            &[u8::from(evaluation.causal_keys_ascending)],
            &[u8::from(evaluation.qk_features_ascending)],
            &[u8::from(evaluation.output_features_ascending)],
            &[evaluation.token_scratch_arrays],
            &[u8::from(evaluation.separate_output_staging)],
        ],
    )
}

fn candidate_identity(
    algorithm: GqaStructuralIdentityV1,
    evaluation: GqaStructuralIdentityV1,
    descriptor: GqaCandidateDescriptorV1,
) -> GqaStructuralIdentityV1 {
    let effects = descriptor.effects;
    hash_fields(
        GQA_CANDIDATE_IDENTITY_DOMAIN_V1,
        &[
            &algorithm.bytes(),
            &evaluation.bytes(),
            &descriptor.schema_version.to_le_bytes(),
            &[effects.initialized_read_buffers],
            &[effects.write_buffers],
            &[u8::from(effects.read_only_inputs_may_alias)],
            &[u8::from(effects.output_is_disjoint)],
            &[u8::from(effects.output_mapping_is_total_and_injective)],
            &[u8::from(effects.independent_vectors_are_race_free)],
            &[u8::from(effects.reads_are_causal_only)],
            &[u8::from(effects.accesses_are_bounded)],
            &[u8::from(effects.output_commit_is_transactional)],
        ],
    )
}

/// Validates and identity-binds one inert host/model candidate.
pub fn validate_structural_gqa_candidate_v1(
    descriptor: GqaCandidateDescriptorV1,
) -> Result<StructuralGqaPrefillCandidateV1, GqaCandidateErrorV1> {
    if descriptor.schema_version != 1 {
        return Err(GqaCandidateErrorV1::SchemaVersion);
    }
    if descriptor.algorithm != "qwen3-bf16-fp32-causal-gqa-prefill" {
        return Err(GqaCandidateErrorV1::Algorithm);
    }
    let profile = validate_gqa_prefill_profile_v1(descriptor.profile)
        .map_err(GqaCandidateErrorV1::Profile)?;
    validate_gqa_numerical_policy_v1(descriptor.numerical)
        .map_err(GqaCandidateErrorV1::Numerical)?;
    validate_gqa_effect_contract_v1(descriptor.effects).map_err(GqaCandidateErrorV1::Effects)?;
    validate_gqa_evaluation_descriptor_v1(descriptor.evaluation)
        .map_err(GqaCandidateErrorV1::Evaluation)?;
    let algorithm_identity = algorithm_identity(descriptor);
    let evaluation_identity = evaluation_identity(algorithm_identity, descriptor);
    let candidate_identity =
        candidate_identity(algorithm_identity, evaluation_identity, descriptor);
    Ok(StructuralGqaPrefillCandidateV1 {
        profile,
        evaluation: descriptor.evaluation,
        algorithm_identity,
        evaluation_identity,
        candidate_identity,
    })
}
