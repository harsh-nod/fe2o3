//! Canonical inert algorithm, evaluation, candidate, and page-metadata identities.

use sha2::{Digest, Sha256};

use crate::{
    PagedGqaEffectContractV1, PagedGqaEffectErrorV1, PagedGqaNumericalErrorV1,
    PagedGqaNumericalPolicyV1, PagedGqaProfileDescriptorV1, PagedGqaProfileErrorV1,
    PagedGqaResourceContractV1, PagedKvBatchMetadataV1, PagedKvMetadataErrorV1,
    ValidatedPagedGqaProfileV1, validate_paged_gqa_effect_contract_v1,
    validate_paged_gqa_numerical_policy_v1, validate_paged_gqa_profile_v1,
    validate_paged_kv_metadata_v1,
};

/// Algorithm identity domain separator.
pub const PAGED_GQA_ALGORITHM_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.qwen3_paged_gqa_decode.algorithm.v1\0";
/// Evaluation identity domain separator.
pub const PAGED_GQA_EVALUATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.qwen3_paged_gqa_decode.evaluation.v1\0";
/// Candidate identity domain separator.
pub const PAGED_GQA_CANDIDATE_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.qwen3_paged_gqa_decode.candidate.v1\0";
/// Runtime page-metadata identity domain separator.
pub const PAGED_KV_METADATA_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.qwen3_paged_gqa_decode.page_metadata.v1\0";

/// One SHA-256 identity over canonical structural fields.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PagedGqaStructuralIdentityV1([u8; 32]);

impl PagedGqaStructuralIdentityV1 {
    /// Returns the complete digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Query/output tensor layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PagedQueryLayoutV1 {
    /// Contiguous `[request][active_token][query_head][feature]`.
    RequestTokenHeadFeature = 1,
    /// Unsupported head-major layout.
    RequestHeadTokenFeature = 2,
}

/// Physical key/value page layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PhysicalKvLayoutV1 {
    /// Contiguous `[physical_page][slot][kv_head][feature]`.
    PageSlotHeadFeature = 1,
    /// Unsupported head-major page layout.
    PageHeadSlotFeature = 2,
}

/// GQA query-to-KV mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PagedGqaHeadMappingV1 {
    /// `kv_head = query_head / query_heads_per_kv_head`.
    ContiguousQuotient = 1,
    /// Unsupported modulo mapping.
    Modulo = 2,
}

/// Query-position interpretation relative to committed/resident prefixes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PagedQueryPositionPolicyV1 {
    /// Query `j` is at `committed_tokens + j` and resident is committed plus
    /// the exact active width.
    CommittedPlusLocalToken = 1,
    /// Unsupported interpretation starting queries after resident tokens.
    ResidentPlusLocalToken = 2,
}

/// Public inert host evaluation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedGqaEvaluationDescriptorV1 {
    /// Evaluation schema version.
    pub schema_version: u16,
    /// Query/output layout.
    pub query_output_layout: PagedQueryLayoutV1,
    /// Physical K/V layout.
    pub key_value_layout: PhysicalKvLayoutV1,
    /// GQA query-head mapping.
    pub head_mapping: PagedGqaHeadMappingV1,
    /// Query-position policy.
    pub query_position: PagedQueryPositionPolicyV1,
    /// Request, query-token, then query-head vectors are visited ascending.
    pub outer_coordinates_ascending: bool,
    /// Logical keys are visited ascending through the causal query position.
    pub causal_keys_ascending: bool,
    /// QK and value features are visited ascending.
    pub features_ascending: bool,
    /// Number of maximum-context FP32 scratch arrays.
    pub context_scratch_arrays: u8,
    /// Complete output uses separate transactional staging.
    pub separate_output_staging: bool,
}

impl PagedGqaEvaluationDescriptorV1 {
    /// Returns the only admitted evaluation record.
    pub const fn exact() -> Self {
        Self {
            schema_version: 1,
            query_output_layout: PagedQueryLayoutV1::RequestTokenHeadFeature,
            key_value_layout: PhysicalKvLayoutV1::PageSlotHeadFeature,
            head_mapping: PagedGqaHeadMappingV1::ContiguousQuotient,
            query_position: PagedQueryPositionPolicyV1::CommittedPlusLocalToken,
            outer_coordinates_ascending: true,
            causal_keys_ascending: true,
            features_ascending: true,
            context_scratch_arrays: 2,
            separate_output_staging: true,
        }
    }
}

/// Evaluation-record mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaEvaluationErrorV1 {
    /// At least one evaluation field differed from the exact record.
    NonCanonical,
}

/// Validates the complete evaluation record.
pub fn validate_paged_gqa_evaluation_descriptor_v1(
    evaluation: PagedGqaEvaluationDescriptorV1,
) -> Result<(), PagedGqaEvaluationErrorV1> {
    if evaluation != PagedGqaEvaluationDescriptorV1::exact() {
        return Err(PagedGqaEvaluationErrorV1::NonCanonical);
    }
    Ok(())
}

/// Complete public record used to request an inert candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedGqaCandidateDescriptorV1 {
    /// Candidate schema version.
    pub schema_version: u16,
    /// Algorithm discriminator.
    pub algorithm: &'static str,
    /// Exact role/bucket/geometry profile.
    pub profile: PagedGqaProfileDescriptorV1,
    /// Exact numerical/order/exception policy.
    pub numerical: PagedGqaNumericalPolicyV1,
    /// Exact effect, alias, and race contract.
    pub effects: PagedGqaEffectContractV1,
    /// Exact host evaluation structure.
    pub evaluation: PagedGqaEvaluationDescriptorV1,
}

impl PagedGqaCandidateDescriptorV1 {
    /// Constructs the canonical record for one exact profile.
    pub const fn canonical(profile: PagedGqaProfileDescriptorV1) -> Self {
        Self {
            schema_version: 1,
            algorithm: "qwen3-bf16-fp32-causal-gqa-paged-decode",
            profile,
            numerical: PagedGqaNumericalPolicyV1::exact(),
            effects: PagedGqaEffectContractV1::exact(),
            evaluation: PagedGqaEvaluationDescriptorV1::exact(),
        }
    }
}

/// Fail-closed structural candidate error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaCandidateErrorV1 {
    /// Candidate schema differed.
    SchemaVersion,
    /// Algorithm discriminator differed.
    Algorithm,
    /// Profile validation failed.
    Profile(PagedGqaProfileErrorV1),
    /// Numerical-policy validation failed.
    Numerical(PagedGqaNumericalErrorV1),
    /// Effect validation failed.
    Effects(PagedGqaEffectErrorV1),
    /// Evaluation validation failed.
    Evaluation(PagedGqaEvaluationErrorV1),
}

/// Validated copyable inert paged-decode candidate.
///
/// This type owns no proof evidence, compiler graph, GPU program, artifact,
/// load handle, or launch capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuralPagedGqaDecodeCandidateV1 {
    profile: ValidatedPagedGqaProfileV1,
    evaluation: PagedGqaEvaluationDescriptorV1,
    algorithm_identity: PagedGqaStructuralIdentityV1,
    evaluation_identity: PagedGqaStructuralIdentityV1,
    candidate_identity: PagedGqaStructuralIdentityV1,
}

impl StructuralPagedGqaDecodeCandidateV1 {
    /// Returns the validated profile.
    pub const fn profile(self) -> ValidatedPagedGqaProfileV1 {
        self.profile
    }

    /// Returns checked worst-case resources.
    pub const fn resources(self) -> PagedGqaResourceContractV1 {
        self.profile.resources()
    }

    /// Returns the inert host evaluation record.
    pub const fn evaluation(self) -> PagedGqaEvaluationDescriptorV1 {
        self.evaluation
    }

    /// Returns the algorithm identity.
    pub const fn algorithm_identity(self) -> PagedGqaStructuralIdentityV1 {
        self.algorithm_identity
    }

    /// Returns the evaluation identity.
    pub const fn evaluation_identity(self) -> PagedGqaStructuralIdentityV1 {
        self.evaluation_identity
    }

    /// Returns the complete candidate identity.
    pub const fn candidate_identity(self) -> PagedGqaStructuralIdentityV1 {
        self.candidate_identity
    }
}

fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> PagedGqaStructuralIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(part);
    }
    PagedGqaStructuralIdentityV1(hasher.finalize().into())
}

fn encode_profile(profile: PagedGqaProfileDescriptorV1) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(96);
    encoded.push(profile.role.identity_tag());
    encoded.push(profile.bucket.identity_tag());
    for value in [
        profile.sequences,
        profile.active_tokens,
        profile.context_capacity_tokens,
        profile.page_tokens,
        profile.geometry.hidden_size,
        profile.geometry.query_heads,
        profile.geometry.kv_heads,
        profile.geometry.head_dimension,
        profile.geometry.query_heads_per_kv_head,
        profile.geometry.query_projection_size,
        profile.geometry.kv_projection_size,
    ] {
        encoded.extend_from_slice(&(value as u64).to_le_bytes());
    }
    encoded.push(profile.tensor_stage as u8);
    encoded
}

fn encode_evaluation(evaluation: PagedGqaEvaluationDescriptorV1) -> [u8; 11] {
    let version = evaluation.schema_version.to_le_bytes();
    [
        version[0],
        version[1],
        evaluation.query_output_layout as u8,
        evaluation.key_value_layout as u8,
        evaluation.head_mapping as u8,
        evaluation.query_position as u8,
        u8::from(evaluation.outer_coordinates_ascending),
        u8::from(evaluation.causal_keys_ascending),
        u8::from(evaluation.features_ascending),
        evaluation.context_scratch_arrays,
        u8::from(evaluation.separate_output_staging),
    ]
}

fn encode_policy(policy: PagedGqaNumericalPolicyV1) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(16);
    encoded.push(policy.bf16_input as u8);
    encoded.extend_from_slice(&policy.attention_scale_bits.to_le_bytes());
    encoded.push(u8::from(policy.qk_features_ascending_fp32));
    encoded.push(u8::from(policy.logical_keys_ascending));
    encoded.push(policy.softmax as u8);
    encoded.push(u8::from(policy.value_accumulation_ascending_fp32));
    encoded.push(u8::from(policy.reject_non_finite));
    encoded.push(u8::from(policy.allow_positive_zero_exp_underflow));
    encoded.push(u8::from(policy.bf16_output_rne));
    encoded
}

fn encode_effects(effects: PagedGqaEffectContractV1) -> [u8; 10] {
    [
        u8::from(effects.query_initialized_read_only),
        u8::from(effects.page_metadata_initialized_read_only),
        u8::from(effects.initialized_kv_prefix_read_only),
        u8::from(effects.key_value_allocations_disjoint),
        u8::from(effects.physical_page_mapping_injective),
        u8::from(effects.request_generation_bound),
        u8::from(effects.final_page_mask_enforced),
        u8::from(effects.transactional_single_writer_output),
        effects.atomics,
        effects.barriers,
    ]
}

/// Validates every candidate field and constructs inert structural identities.
pub fn admit_paged_gqa_decode_candidate_v1(
    descriptor: PagedGqaCandidateDescriptorV1,
) -> Result<StructuralPagedGqaDecodeCandidateV1, PagedGqaCandidateErrorV1> {
    if descriptor.schema_version != 1 {
        return Err(PagedGqaCandidateErrorV1::SchemaVersion);
    }
    if descriptor.algorithm != "qwen3-bf16-fp32-causal-gqa-paged-decode" {
        return Err(PagedGqaCandidateErrorV1::Algorithm);
    }
    let profile = validate_paged_gqa_profile_v1(descriptor.profile)
        .map_err(PagedGqaCandidateErrorV1::Profile)?;
    validate_paged_gqa_numerical_policy_v1(descriptor.numerical)
        .map_err(PagedGqaCandidateErrorV1::Numerical)?;
    validate_paged_gqa_effect_contract_v1(descriptor.effects)
        .map_err(PagedGqaCandidateErrorV1::Effects)?;
    validate_paged_gqa_evaluation_descriptor_v1(descriptor.evaluation)
        .map_err(PagedGqaCandidateErrorV1::Evaluation)?;

    let profile_bytes = encode_profile(descriptor.profile);
    let algorithm_identity = hash_parts(
        PAGED_GQA_ALGORITHM_IDENTITY_DOMAIN_V1,
        &[descriptor.algorithm.as_bytes(), &profile_bytes],
    );
    let evaluation_bytes = encode_evaluation(descriptor.evaluation);
    let evaluation_identity = hash_parts(
        PAGED_GQA_EVALUATION_IDENTITY_DOMAIN_V1,
        &[&algorithm_identity.bytes(), &evaluation_bytes],
    );
    let schema = descriptor.schema_version.to_le_bytes();
    let policy = encode_policy(descriptor.numerical);
    let effects = encode_effects(descriptor.effects);
    let candidate_identity = hash_parts(
        PAGED_GQA_CANDIDATE_IDENTITY_DOMAIN_V1,
        &[
            &schema,
            &algorithm_identity.bytes(),
            &evaluation_identity.bytes(),
            &policy,
            &effects,
        ],
    );
    Ok(StructuralPagedGqaDecodeCandidateV1 {
        profile,
        evaluation: descriptor.evaluation,
        algorithm_identity,
        evaluation_identity,
        candidate_identity,
    })
}

/// Validates metadata and returns a deterministic identity over metadata only.
///
/// The identity binds the candidate, request/generation boundaries, page-table
/// permutation, initialized masks, and allocation names. It does not hash or
/// authenticate query, key, or value bytes.
pub fn paged_kv_metadata_identity_v1(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    metadata: &PagedKvBatchMetadataV1,
) -> Result<PagedGqaStructuralIdentityV1, PagedKvMetadataErrorV1> {
    validate_paged_kv_metadata_v1(candidate.profile(), metadata)?;
    let mut hasher = Sha256::new();
    hasher.update(PAGED_KV_METADATA_IDENTITY_DOMAIN_V1);
    hasher.update(candidate.candidate_identity().bytes());
    hasher.update([metadata.role.identity_tag()]);
    hasher.update((metadata.page_tokens as u64).to_le_bytes());
    hasher.update((metadata.context_capacity_tokens as u64).to_le_bytes());
    hasher.update((metadata.physical_pages as u64).to_le_bytes());
    hasher.update(metadata.key_allocation.0);
    hasher.update(metadata.value_allocation.0);
    hasher.update((metadata.requests.len() as u64).to_le_bytes());
    for request in &metadata.requests {
        hasher.update(request.request_id.0);
        hasher.update(request.generation.to_le_bytes());
        hasher.update((request.committed_tokens as u64).to_le_bytes());
        hasher.update((request.resident_tokens as u64).to_le_bytes());
    }
    hasher.update((metadata.entries.len() as u64).to_le_bytes());
    for entry in &metadata.entries {
        hasher.update(entry.logical_page.to_le_bytes());
        hasher.update(entry.physical_page.to_le_bytes());
        hasher.update(entry.physical_generation.to_le_bytes());
        hasher.update(entry.request_id.0);
        hasher.update(entry.initialized_tokens.to_le_bytes());
        hasher.update(entry.initialized_mask.to_le_bytes());
    }
    Ok(PagedGqaStructuralIdentityV1(hasher.finalize().into()))
}
