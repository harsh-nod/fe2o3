//! Exact Qwen3 geometry, B3 prefill, numerical, effect, and resource contracts.

/// Exact Qwen3 attention head dimension.
pub const QWEN3_HEAD_DIMENSION_V1: usize = 128;
/// Exact FP32 attention scale bits for `1 / sqrt(128)`.
pub const QWEN3_ATTENTION_SCALE_BITS_V1: u32 = 0x3db5_04f3;
/// Largest admitted prefill sequence length.
pub const MAX_GQA_PREFILL_TOKENS_V1: usize = 2_048;
/// Largest admitted query/output element count.
pub const MAX_GQA_QUERY_ELEMENTS_V1: u64 = 8_388_608;
/// Largest admitted key or value element count.
pub const MAX_GQA_KV_ELEMENTS_V1: u64 = 2_097_152;
/// Largest admitted `(sequence, query-head, query-token, key-token)` count.
pub const MAX_GQA_CAUSAL_PAIRS_V1: u64 = 67_141_632;

/// Exact Qwen3 model role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3AttentionRoleV1 {
    /// Qwen3-8B target attention geometry.
    Target8B = 1,
    /// Qwen3-0.6B draft attention geometry.
    Draft06B = 2,
}

impl Qwen3AttentionRoleV1 {
    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Closed M1 B3 prefill bucket set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum B3PrefillBucketV1 {
    /// One sequence with 128 active/context tokens.
    S1T128 = 1,
    /// Eight independent sequences with 128 active/context tokens each.
    S8T128 = 2,
    /// One sequence with 512 active/context tokens.
    S1T512 = 3,
    /// One sequence with 2048 active/context tokens.
    S1T2048 = 4,
}

/// All and only admitted prefill buckets, in stable order.
pub const B3_PREFILL_BUCKETS_V1: [B3PrefillBucketV1; 4] = [
    B3PrefillBucketV1::S1T128,
    B3PrefillBucketV1::S8T128,
    B3PrefillBucketV1::S1T512,
    B3PrefillBucketV1::S1T2048,
];

impl B3PrefillBucketV1 {
    /// Returns the exact sequence count.
    pub const fn sequences(self) -> usize {
        match self {
            Self::S8T128 => 8,
            _ => 1,
        }
    }

    /// Returns active and context tokens per sequence.
    pub const fn tokens(self) -> usize {
        match self {
            Self::S1T128 | Self::S8T128 => 128,
            Self::S1T512 => 512,
            Self::S1T2048 => 2_048,
        }
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Public inert exact-geometry record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3GqaGeometryV1 {
    /// Transformer hidden width before Q/K/V projection.
    pub hidden_size: usize,
    /// Projected query-head count.
    pub query_heads: usize,
    /// Projected key/value-head count.
    pub kv_heads: usize,
    /// Feature count per query/key/value head.
    pub head_dimension: usize,
    /// Consecutive query heads sharing one KV head.
    pub query_heads_per_kv_head: usize,
    /// Query projection width.
    pub query_projection_size: usize,
    /// Key and value projection width.
    pub kv_projection_size: usize,
}

impl Qwen3GqaGeometryV1 {
    /// Returns the only admitted geometry for a role.
    pub const fn exact(role: Qwen3AttentionRoleV1) -> Self {
        match role {
            Qwen3AttentionRoleV1::Target8B => Self {
                hidden_size: 4_096,
                query_heads: 32,
                kv_heads: 8,
                head_dimension: QWEN3_HEAD_DIMENSION_V1,
                query_heads_per_kv_head: 4,
                query_projection_size: 4_096,
                kv_projection_size: 1_024,
            },
            Qwen3AttentionRoleV1::Draft06B => Self {
                hidden_size: 1_024,
                query_heads: 16,
                kv_heads: 8,
                head_dimension: QWEN3_HEAD_DIMENSION_V1,
                query_heads_per_kv_head: 2,
                query_projection_size: 2_048,
                kv_projection_size: 1_024,
            },
        }
    }
}

/// Exact semantic stage of the Q/K/V inputs and attention output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GqaTensorStageV1 {
    /// Q/K/V are projected, Q/K are normalized and rotary encoded, and output
    /// is the pre-output-projection attention tensor.
    PostProjectionQkNormRopeToPreOutputProjection = 1,
    /// Unsupported pre-normalization and pre-RoPE Q/K tensors.
    ProjectedBeforeQkNormAndRope = 2,
}

/// Complete role-and-bucket profile record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaPrefillProfileDescriptorV1 {
    /// Exact model role.
    pub role: Qwen3AttentionRoleV1,
    /// Exact B3 prefill bucket.
    pub bucket: B3PrefillBucketV1,
    /// Independent sequence count.
    pub sequences: usize,
    /// Active tokens per sequence.
    pub active_tokens: usize,
    /// Context tokens per sequence; equal to active tokens for prefill.
    pub context_tokens: usize,
    /// Exact target/draft attention geometry.
    pub geometry: Qwen3GqaGeometryV1,
    /// Boundary around projection, Q/K normalization, RoPE, and output projection.
    pub tensor_stage: GqaTensorStageV1,
}

impl GqaPrefillProfileDescriptorV1 {
    /// Constructs the canonical record for a closed role/bucket pair.
    pub const fn canonical(role: Qwen3AttentionRoleV1, bucket: B3PrefillBucketV1) -> Self {
        Self {
            role,
            bucket,
            sequences: bucket.sequences(),
            active_tokens: bucket.tokens(),
            context_tokens: bucket.tokens(),
            geometry: Qwen3GqaGeometryV1::exact(role),
            tensor_stage: GqaTensorStageV1::PostProjectionQkNormRopeToPreOutputProjection,
        }
    }
}

/// Exact profile mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaProfileErrorV1 {
    /// Sequence count differed from the named bucket.
    Sequences,
    /// Active-token count differed.
    ActiveTokens,
    /// Context-token count differed or was not equal to active tokens.
    ContextTokens,
    /// Hidden width differed from the role.
    HiddenSize,
    /// Query-head count differed.
    QueryHeads,
    /// KV-head count differed.
    KvHeads,
    /// Head dimension differed.
    HeadDimension,
    /// GQA group size differed.
    GqaGroupSize,
    /// Query projection width differed.
    QueryProjection,
    /// KV projection width differed.
    KvProjection,
    /// Q/K/V semantic stage differed.
    TensorStage,
    /// Checked resource arithmetic overflowed.
    ResourceArithmeticOverflow,
    /// Derived work exceeded the exact B3 ceiling.
    ResourceLimit,
}

/// Checked logical storage and operation bounds for one profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaResourceContractV1 {
    /// BF16 query elements.
    pub query_elements: u64,
    /// BF16 key elements and, separately, value elements.
    pub kv_elements_each: u64,
    /// BF16 output elements, equal to query elements.
    pub output_elements: u64,
    /// Total BF16 input payload bytes for Q, K, and V.
    pub input_payload_bytes: u64,
    /// Transactional BF16 output payload bytes.
    pub output_payload_bytes: u64,
    /// Causal query/key pairs across sequences and query heads.
    pub causal_pairs: u64,
    /// FP32 QK scalar products.
    pub qk_multiplications: u64,
    /// FP32 weighted-V scalar products.
    pub value_multiplications: u64,
    /// Host exponential evaluations, one per causal score.
    pub exponential_evaluations: u64,
    /// FP32 output divisions, one per output element.
    pub output_divisions: u64,
    /// Host score and weight scratch payload for one output vector.
    pub vector_scratch_bytes: u64,
    /// Logical transactional host scratch payload including output staging.
    pub transactional_scratch_bytes: u64,
}

fn checked_mul(left: u64, right: usize) -> Result<u64, GqaProfileErrorV1> {
    left.checked_mul(
        u64::try_from(right).map_err(|_| GqaProfileErrorV1::ResourceArithmeticOverflow)?,
    )
    .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)
}

fn derive_resources(
    descriptor: GqaPrefillProfileDescriptorV1,
) -> Result<GqaResourceContractV1, GqaProfileErrorV1> {
    let geometry = descriptor.geometry;
    let sequences = u64::try_from(descriptor.sequences)
        .map_err(|_| GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let query_heads = u64::try_from(geometry.query_heads)
        .map_err(|_| GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let sequence_tokens = checked_mul(sequences, descriptor.active_tokens)?;
    let query_elements = checked_mul(
        checked_mul(sequence_tokens, geometry.query_heads)?,
        geometry.head_dimension,
    )?;
    let kv_elements_each = checked_mul(
        checked_mul(sequence_tokens, geometry.kv_heads)?,
        geometry.head_dimension,
    )?;
    let tokens = u64::try_from(descriptor.active_tokens)
        .map_err(|_| GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let triangular = tokens
        .checked_mul(
            tokens
                .checked_add(1)
                .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?,
        )
        .and_then(|value| value.checked_div(2))
        .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let causal_pairs = triangular
        .checked_mul(sequences)
        .and_then(|value| value.checked_mul(query_heads))
        .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    if query_elements > MAX_GQA_QUERY_ELEMENTS_V1
        || kv_elements_each > MAX_GQA_KV_ELEMENTS_V1
        || causal_pairs > MAX_GQA_CAUSAL_PAIRS_V1
    {
        return Err(GqaProfileErrorV1::ResourceLimit);
    }
    let qk_multiplications = checked_mul(causal_pairs, geometry.head_dimension)?;
    let value_multiplications = qk_multiplications;
    let input_payload_bytes = query_elements
        .checked_add(
            kv_elements_each
                .checked_mul(2)
                .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?,
        )
        .and_then(|elements| elements.checked_mul(2))
        .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let output_payload_bytes = query_elements
        .checked_mul(2)
        .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let vector_scratch_bytes = tokens
        .checked_mul(8)
        .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let transactional_scratch_bytes = output_payload_bytes
        .checked_add(vector_scratch_bytes)
        .ok_or(GqaProfileErrorV1::ResourceArithmeticOverflow)?;
    Ok(GqaResourceContractV1 {
        query_elements,
        kv_elements_each,
        output_elements: query_elements,
        input_payload_bytes,
        output_payload_bytes,
        causal_pairs,
        qk_multiplications,
        value_multiplications,
        exponential_evaluations: causal_pairs,
        output_divisions: query_elements,
        vector_scratch_bytes,
        transactional_scratch_bytes,
    })
}

/// Validated exact profile and checked resources.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedGqaPrefillProfileV1 {
    descriptor: GqaPrefillProfileDescriptorV1,
    resources: GqaResourceContractV1,
}

impl ValidatedGqaPrefillProfileV1 {
    /// Returns the validated public record.
    pub const fn descriptor(self) -> GqaPrefillProfileDescriptorV1 {
        self.descriptor
    }

    /// Returns the checked logical resource bounds.
    pub const fn resources(self) -> GqaResourceContractV1 {
        self.resources
    }
}

/// Validates all independent axes of one exact profile.
pub fn validate_gqa_prefill_profile_v1(
    descriptor: GqaPrefillProfileDescriptorV1,
) -> Result<ValidatedGqaPrefillProfileV1, GqaProfileErrorV1> {
    let exact = GqaPrefillProfileDescriptorV1::canonical(descriptor.role, descriptor.bucket);
    if descriptor.sequences != exact.sequences {
        return Err(GqaProfileErrorV1::Sequences);
    }
    if descriptor.active_tokens != exact.active_tokens {
        return Err(GqaProfileErrorV1::ActiveTokens);
    }
    if descriptor.context_tokens != exact.context_tokens
        || descriptor.context_tokens != descriptor.active_tokens
    {
        return Err(GqaProfileErrorV1::ContextTokens);
    }
    if descriptor.geometry.hidden_size != exact.geometry.hidden_size {
        return Err(GqaProfileErrorV1::HiddenSize);
    }
    if descriptor.geometry.query_heads != exact.geometry.query_heads {
        return Err(GqaProfileErrorV1::QueryHeads);
    }
    if descriptor.geometry.kv_heads != exact.geometry.kv_heads {
        return Err(GqaProfileErrorV1::KvHeads);
    }
    if descriptor.geometry.head_dimension != exact.geometry.head_dimension {
        return Err(GqaProfileErrorV1::HeadDimension);
    }
    if descriptor.geometry.query_heads_per_kv_head != exact.geometry.query_heads_per_kv_head {
        return Err(GqaProfileErrorV1::GqaGroupSize);
    }
    if descriptor.geometry.query_projection_size != exact.geometry.query_projection_size {
        return Err(GqaProfileErrorV1::QueryProjection);
    }
    if descriptor.geometry.kv_projection_size != exact.geometry.kv_projection_size {
        return Err(GqaProfileErrorV1::KvProjection);
    }
    if descriptor.tensor_stage != exact.tensor_stage {
        return Err(GqaProfileErrorV1::TensorStage);
    }
    let resources = derive_resources(descriptor)?;
    Ok(ValidatedGqaPrefillProfileV1 {
        descriptor,
        resources,
    })
}

/// Causal mask policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CausalPolicyV1 {
    /// Each query token attends to keys `0..=query_token` in its sequence.
    LowerTriangleInclusive = 1,
    /// Unsupported non-causal policy retained for hostile tests.
    Unmasked = 2,
}

/// QK score evaluation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ScorePolicyV1 {
    /// Ascending-feature FP32 products/sums, then one FP32 scale multiply.
    AscendingFeatureFp32ThenScale = 1,
    /// Unsupported scale-before-reduction policy.
    ScaleBeforeReduction = 2,
}

/// Stable softmax evaluation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SoftmaxPolicyV1 {
    /// Materialize causal scores, then ascending max, exp/sum, and weights.
    TwoPassStableAscendingKeysFp32 = 1,
    /// Unsupported online recurrence.
    OnlineFp32 = 2,
}

/// Host exponential named by the FP32 model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExponentialPolicyV1 {
    /// Pinned Rust host `f32::exp`, with no target correspondence claim.
    RustStdF32HostSurrogate = 1,
    /// Unsupported target OCML exponential.
    OcmlExpF32 = 2,
}

/// Weighted-value accumulation policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ValuePolicyV1 {
    /// Ascending-key FP32 multiply/add, then FP32 denominator division.
    AscendingKeysFp32ThenDivide = 1,
    /// Unsupported contracted accumulation.
    ContractedFma = 2,
}

/// Output storage conversion policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AttentionOutputCastV1 {
    /// BF16 round-to-nearest, ties-to-even.
    Bf16RoundToNearestTiesEven = 1,
    /// Unsupported truncation.
    Bf16Truncate = 2,
}

/// Complete intended host numerical/order/exception record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaNumericalPolicyV1 {
    /// Exact binary32 attention scale.
    pub attention_scale_bits: u32,
    /// Exact causal mask.
    pub causal: CausalPolicyV1,
    /// QK evaluation and scale order.
    pub score: ScorePolicyV1,
    /// Stable softmax order.
    pub softmax: SoftmaxPolicyV1,
    /// Host exponential surrogate.
    pub exponential: ExponentialPolicyV1,
    /// Weighted-V evaluation order.
    pub value: ValuePolicyV1,
    /// BF16 output conversion.
    pub output_cast: AttentionOutputCastV1,
    /// Whether every physical Q/K/V input must be finite.
    pub reject_non_finite_inputs: bool,
    /// Whether every FP32 intermediate must be finite.
    pub reject_non_finite_intermediates: bool,
    /// Whether exponential underflow to positive zero is allowed.
    pub allow_exponential_underflow: bool,
}

impl GqaNumericalPolicyV1 {
    /// Returns the only admitted host numerical policy.
    pub const fn exact() -> Self {
        Self {
            attention_scale_bits: QWEN3_ATTENTION_SCALE_BITS_V1,
            causal: CausalPolicyV1::LowerTriangleInclusive,
            score: ScorePolicyV1::AscendingFeatureFp32ThenScale,
            softmax: SoftmaxPolicyV1::TwoPassStableAscendingKeysFp32,
            exponential: ExponentialPolicyV1::RustStdF32HostSurrogate,
            value: ValuePolicyV1::AscendingKeysFp32ThenDivide,
            output_cast: AttentionOutputCastV1::Bf16RoundToNearestTiesEven,
            reject_non_finite_inputs: true,
            reject_non_finite_intermediates: true,
            allow_exponential_underflow: true,
        }
    }
}

/// Independent numerical-policy mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaNumericalErrorV1 {
    /// Scale bits differed.
    Scale,
    /// Causal policy differed.
    Causal,
    /// Score evaluation differed.
    Score,
    /// Softmax evaluation differed.
    Softmax,
    /// Exponential provider differed.
    Exponential,
    /// Value evaluation differed.
    Value,
    /// Output cast differed.
    OutputCast,
    /// Physical non-finite inputs were not rejected.
    InputExceptionPolicy,
    /// Non-finite intermediates were not rejected.
    IntermediateExceptionPolicy,
    /// Stable exponential underflow policy differed.
    UnderflowPolicy,
}

/// Validates every independent numerical-policy axis.
pub fn validate_gqa_numerical_policy_v1(
    policy: GqaNumericalPolicyV1,
) -> Result<(), GqaNumericalErrorV1> {
    let exact = GqaNumericalPolicyV1::exact();
    if policy.attention_scale_bits != exact.attention_scale_bits {
        return Err(GqaNumericalErrorV1::Scale);
    }
    if policy.causal != exact.causal {
        return Err(GqaNumericalErrorV1::Causal);
    }
    if policy.score != exact.score {
        return Err(GqaNumericalErrorV1::Score);
    }
    if policy.softmax != exact.softmax {
        return Err(GqaNumericalErrorV1::Softmax);
    }
    if policy.exponential != exact.exponential {
        return Err(GqaNumericalErrorV1::Exponential);
    }
    if policy.value != exact.value {
        return Err(GqaNumericalErrorV1::Value);
    }
    if policy.output_cast != exact.output_cast {
        return Err(GqaNumericalErrorV1::OutputCast);
    }
    if !policy.reject_non_finite_inputs {
        return Err(GqaNumericalErrorV1::InputExceptionPolicy);
    }
    if !policy.reject_non_finite_intermediates {
        return Err(GqaNumericalErrorV1::IntermediateExceptionPolicy);
    }
    if !policy.allow_exponential_underflow {
        return Err(GqaNumericalErrorV1::UnderflowPolicy);
    }
    Ok(())
}

/// Explicit host/model memory, initialization, alias, race, and commit record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaEffectContractV1 {
    /// Q, K, and V are initialized read-only BF16 inputs.
    pub initialized_read_buffers: u8,
    /// O is one BF16 write-only output.
    pub write_buffers: u8,
    /// Immutable input buffers may alias one another.
    pub read_only_inputs_may_alias: bool,
    /// Output must be disjoint from every input.
    pub output_is_disjoint: bool,
    /// Every output coordinate has one logical owner.
    pub output_mapping_is_total_and_injective: bool,
    /// Distinct output vectors have no conflicting writes.
    pub independent_vectors_are_race_free: bool,
    /// Keys after the query token are never read by evaluation.
    pub reads_are_causal_only: bool,
    /// All indexing arithmetic is checked against validated extents.
    pub accesses_are_bounded: bool,
    /// Complete output publishes only after all vectors succeed.
    pub output_commit_is_transactional: bool,
}

impl GqaEffectContractV1 {
    /// Returns the only admitted effect contract.
    pub const fn exact() -> Self {
        Self {
            initialized_read_buffers: 3,
            write_buffers: 1,
            read_only_inputs_may_alias: true,
            output_is_disjoint: true,
            output_mapping_is_total_and_injective: true,
            independent_vectors_are_race_free: true,
            reads_are_causal_only: true,
            accesses_are_bounded: true,
            output_commit_is_transactional: true,
        }
    }
}

/// Independent effect-contract mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaEffectErrorV1 {
    /// Read inventory differed.
    ReadInventory,
    /// Write inventory differed.
    WriteInventory,
    /// Immutable alias policy differed.
    ReadAlias,
    /// Output disjointness was removed.
    OutputAlias,
    /// Unique logical ownership was removed.
    OutputOwnership,
    /// Independent-vector race exclusion was removed.
    RaceFreedom,
    /// Causal read restriction was removed.
    CausalReads,
    /// Bounds enforcement was removed.
    Bounds,
    /// Transactional output was removed.
    TransactionalCommit,
}

/// Validates every independent effect-contract axis.
pub fn validate_gqa_effect_contract_v1(
    effects: GqaEffectContractV1,
) -> Result<(), GqaEffectErrorV1> {
    let exact = GqaEffectContractV1::exact();
    if effects.initialized_read_buffers != exact.initialized_read_buffers {
        return Err(GqaEffectErrorV1::ReadInventory);
    }
    if effects.write_buffers != exact.write_buffers {
        return Err(GqaEffectErrorV1::WriteInventory);
    }
    if effects.read_only_inputs_may_alias != exact.read_only_inputs_may_alias {
        return Err(GqaEffectErrorV1::ReadAlias);
    }
    if !effects.output_is_disjoint {
        return Err(GqaEffectErrorV1::OutputAlias);
    }
    if !effects.output_mapping_is_total_and_injective {
        return Err(GqaEffectErrorV1::OutputOwnership);
    }
    if !effects.independent_vectors_are_race_free {
        return Err(GqaEffectErrorV1::RaceFreedom);
    }
    if !effects.reads_are_causal_only {
        return Err(GqaEffectErrorV1::CausalReads);
    }
    if !effects.accesses_are_bounded {
        return Err(GqaEffectErrorV1::Bounds);
    }
    if !effects.output_commit_is_transactional {
        return Err(GqaEffectErrorV1::TransactionalCommit);
    }
    Ok(())
}

/// Returns a checked contiguous Q/O index for layout
/// `[sequence][token][query_head][feature]`.
pub fn gqa_query_index_v1(
    profile: ValidatedGqaPrefillProfileV1,
    sequence: usize,
    token: usize,
    query_head: usize,
    feature: usize,
) -> Option<usize> {
    let descriptor = profile.descriptor();
    let geometry = descriptor.geometry;
    if sequence >= descriptor.sequences
        || token >= descriptor.active_tokens
        || query_head >= geometry.query_heads
        || feature >= geometry.head_dimension
    {
        return None;
    }
    sequence
        .checked_mul(descriptor.active_tokens)?
        .checked_add(token)?
        .checked_mul(geometry.query_heads)?
        .checked_add(query_head)?
        .checked_mul(geometry.head_dimension)?
        .checked_add(feature)
}

/// Returns a checked contiguous K/V index for layout
/// `[sequence][token][kv_head][feature]`.
pub fn gqa_kv_index_v1(
    profile: ValidatedGqaPrefillProfileV1,
    sequence: usize,
    token: usize,
    kv_head: usize,
    feature: usize,
) -> Option<usize> {
    let descriptor = profile.descriptor();
    let geometry = descriptor.geometry;
    if sequence >= descriptor.sequences
        || token >= descriptor.active_tokens
        || kv_head >= geometry.kv_heads
        || feature >= geometry.head_dimension
    {
        return None;
    }
    sequence
        .checked_mul(descriptor.active_tokens)?
        .checked_add(token)?
        .checked_mul(geometry.kv_heads)?
        .checked_add(kv_head)?
        .checked_mul(geometry.head_dimension)?
        .checked_add(feature)
}

/// Maps a valid query head to the exact grouped-query KV head.
pub fn gqa_kv_head_for_query_v1(
    profile: ValidatedGqaPrefillProfileV1,
    query_head: usize,
) -> Option<usize> {
    let geometry = profile.descriptor().geometry;
    if query_head >= geometry.query_heads {
        return None;
    }
    Some(query_head / geometry.query_heads_per_kv_head)
}

/// Returns whether a key participates in inclusive causal prefill attention.
pub fn gqa_key_participates_v1(
    profile: ValidatedGqaPrefillProfileV1,
    query_token: usize,
    key_token: usize,
) -> bool {
    let tokens = profile.descriptor().active_tokens;
    query_token < tokens && key_token < tokens && key_token <= query_token
}
