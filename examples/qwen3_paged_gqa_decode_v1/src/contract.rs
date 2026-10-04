//! Exact B3 profile, page-table, numerical, effect, and resource contracts.

use std::collections::HashSet;

/// Exact Qwen3 attention head dimension.
pub const QWEN3_HEAD_DIMENSION_V1: usize = 128;
/// Exact target query-head count.
pub const QWEN3_TARGET_QUERY_HEADS_V1: usize = 32;
/// Exact draft query-head count.
pub const QWEN3_DRAFT_QUERY_HEADS_V1: usize = 16;
/// Exact target and draft key/value-head count.
pub const QWEN3_KV_HEADS_V1: usize = 8;
/// Exact M1 KV page size in tokens.
pub const M1_KV_PAGE_TOKENS_V1: usize = 16;
/// Exact B3 decode/speculative context capacity.
pub const M1_CONTEXT_CAPACITY_TOKENS_V1: usize = 8_192;
/// Exact logical pages per request.
pub const M1_PAGES_PER_REQUEST_V1: usize = 512;
/// Maximum physical pages for 32 B3 requests.
pub const M1_MAX_PHYSICAL_PAGES_V1: usize = 16_384;
/// Exact FP32 attention scale bits for `1 / sqrt(128)`.
pub const QWEN3_ATTENTION_SCALE_BITS_V1: u32 = 0x3db5_04f3;
/// Largest admitted query or output element count.
pub const MAX_PAGED_GQA_QUERY_ELEMENTS_V1: u64 = 163_840;
/// Largest admitted key or value physical-pool element count.
pub const MAX_PAGED_GQA_KV_ELEMENTS_V1: u64 = 268_435_456;
/// Largest admitted causal query/key-pair count.
pub const MAX_PAGED_GQA_CAUSAL_PAIRS_V1: u64 = 10_483_200;

/// Exact Qwen3 model role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3AttentionRoleV1 {
    /// Qwen3-8B target geometry.
    Target8B = 1,
    /// Qwen3-0.6B draft geometry.
    Draft06B = 2,
}

impl Qwen3AttentionRoleV1 {
    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Closed M1 B3 paged-decode bucket set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum B3PagedDecodeBucketV1 {
    /// Ordinary decode, one request, one active token.
    DecodeS1C8192 = 1,
    /// Ordinary decode, eight requests, one active token each.
    DecodeS8C8192 = 2,
    /// Ordinary decode, 32 requests, one active token each.
    DecodeS32C8192 = 3,
    /// Speculative S1K4: target width five, draft width four.
    SpecS1K4C8192 = 4,
    /// Speculative S8K4: target width five, draft width four.
    SpecS8K4C8192 = 5,
    /// Speculative S1K8: target width nine, draft width eight.
    SpecS1K8C8192 = 6,
    /// Speculative S1K16: target width 17, draft width 16.
    SpecS1K16C8192 = 7,
}

/// All and only admitted paged-decode buckets, in stable order.
pub const B3_PAGED_DECODE_BUCKETS_V1: [B3PagedDecodeBucketV1; 7] = [
    B3PagedDecodeBucketV1::DecodeS1C8192,
    B3PagedDecodeBucketV1::DecodeS8C8192,
    B3PagedDecodeBucketV1::DecodeS32C8192,
    B3PagedDecodeBucketV1::SpecS1K4C8192,
    B3PagedDecodeBucketV1::SpecS8K4C8192,
    B3PagedDecodeBucketV1::SpecS1K8C8192,
    B3PagedDecodeBucketV1::SpecS1K16C8192,
];

impl B3PagedDecodeBucketV1 {
    /// Returns the exact independent request count.
    pub const fn sequences(self) -> usize {
        match self {
            Self::DecodeS8C8192 | Self::SpecS8K4C8192 => 8,
            Self::DecodeS32C8192 => 32,
            _ => 1,
        }
    }

    /// Returns the exact active-token width for the role.
    pub const fn active_tokens(self, role: Qwen3AttentionRoleV1) -> usize {
        match self {
            Self::DecodeS1C8192 | Self::DecodeS8C8192 | Self::DecodeS32C8192 => 1,
            Self::SpecS1K4C8192 | Self::SpecS8K4C8192 => match role {
                Qwen3AttentionRoleV1::Target8B => 5,
                Qwen3AttentionRoleV1::Draft06B => 4,
            },
            Self::SpecS1K8C8192 => match role {
                Qwen3AttentionRoleV1::Target8B => 9,
                Qwen3AttentionRoleV1::Draft06B => 8,
            },
            Self::SpecS1K16C8192 => match role {
                Qwen3AttentionRoleV1::Target8B => 17,
                Qwen3AttentionRoleV1::Draft06B => 16,
            },
        }
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Public inert exact attention geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Qwen3GqaGeometryV1 {
    /// Hidden width before projection.
    pub hidden_size: usize,
    /// Projected query-head count.
    pub query_heads: usize,
    /// Projected key/value-head count.
    pub kv_heads: usize,
    /// Feature count per head.
    pub head_dimension: usize,
    /// Consecutive query heads sharing one KV head.
    pub query_heads_per_kv_head: usize,
    /// Query projection width.
    pub query_projection_size: usize,
    /// Key/value projection width.
    pub kv_projection_size: usize,
}

impl Qwen3GqaGeometryV1 {
    /// Returns the only admitted geometry for a role.
    pub const fn exact(role: Qwen3AttentionRoleV1) -> Self {
        match role {
            Qwen3AttentionRoleV1::Target8B => Self {
                hidden_size: 4_096,
                query_heads: QWEN3_TARGET_QUERY_HEADS_V1,
                kv_heads: QWEN3_KV_HEADS_V1,
                head_dimension: QWEN3_HEAD_DIMENSION_V1,
                query_heads_per_kv_head: 4,
                query_projection_size: 4_096,
                kv_projection_size: 1_024,
            },
            Qwen3AttentionRoleV1::Draft06B => Self {
                hidden_size: 1_024,
                query_heads: QWEN3_DRAFT_QUERY_HEADS_V1,
                kv_heads: QWEN3_KV_HEADS_V1,
                head_dimension: QWEN3_HEAD_DIMENSION_V1,
                query_heads_per_kv_head: 2,
                query_projection_size: 2_048,
                kv_projection_size: 1_024,
            },
        }
    }
}

/// Exact semantic stage of the tensors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PagedGqaTensorStageV1 {
    /// Q/K are projected, normalized, and rotary encoded; V is projected; K/V
    /// are already in K3 pages; output precedes output projection.
    PostProjectionQkNormRopePagedKvToPreOutputProjection = 1,
    /// Unsupported tensors before Q/K normalization and RoPE.
    ProjectedBeforeQkNormAndRope = 2,
}

/// Complete role-and-bucket profile record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedGqaProfileDescriptorV1 {
    /// Exact model role.
    pub role: Qwen3AttentionRoleV1,
    /// Exact B3 bucket.
    pub bucket: B3PagedDecodeBucketV1,
    /// Independent request count.
    pub sequences: usize,
    /// Active query tokens per request.
    pub active_tokens: usize,
    /// Exact logical context capacity.
    pub context_capacity_tokens: usize,
    /// Exact physical page size.
    pub page_tokens: usize,
    /// Exact target/draft geometry.
    pub geometry: Qwen3GqaGeometryV1,
    /// Exact tensor-stage boundary.
    pub tensor_stage: PagedGqaTensorStageV1,
}

impl PagedGqaProfileDescriptorV1 {
    /// Constructs the canonical record for a closed role/bucket pair.
    pub const fn canonical(role: Qwen3AttentionRoleV1, bucket: B3PagedDecodeBucketV1) -> Self {
        Self {
            role,
            bucket,
            sequences: bucket.sequences(),
            active_tokens: bucket.active_tokens(role),
            context_capacity_tokens: M1_CONTEXT_CAPACITY_TOKENS_V1,
            page_tokens: M1_KV_PAGE_TOKENS_V1,
            geometry: Qwen3GqaGeometryV1::exact(role),
            tensor_stage:
                PagedGqaTensorStageV1::PostProjectionQkNormRopePagedKvToPreOutputProjection,
        }
    }
}

/// Exact profile mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaProfileErrorV1 {
    /// Sequence count differed from the bucket.
    Sequences,
    /// Active width differed from the role/bucket.
    ActiveTokens,
    /// Context capacity differed from C8192.
    ContextCapacity,
    /// Page size differed from P16.
    PageTokens,
    /// Hidden size differed.
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
    /// Tensor stage differed.
    TensorStage,
    /// Checked resource arithmetic overflowed.
    ResourceArithmeticOverflow,
    /// Derived resources exceeded the finite B3 envelope.
    ResourceLimit,
}

/// Checked logical resource bounds for one profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedGqaResourceContractV1 {
    /// BF16 query elements.
    pub query_elements: u64,
    /// BF16 output elements.
    pub output_elements: u64,
    /// Exact complete page-table entries.
    pub page_table_entries: u64,
    /// Maximum physical pages.
    pub physical_pages: u64,
    /// BF16 key elements and, separately, value elements.
    pub kv_elements_each: u64,
    /// Maximum causal query/key pairs.
    pub causal_pairs: u64,
    /// FP32 QK scalar products.
    pub qk_multiplications: u64,
    /// FP32 weighted-V scalar products.
    pub value_multiplications: u64,
    /// Host exponential evaluations.
    pub exponential_evaluations: u64,
    /// FP32 output divisions.
    pub output_divisions: u64,
    /// Two maximum-context FP32 scratch arrays.
    pub vector_scratch_bytes: u64,
    /// Transactional BF16 output staging bytes.
    pub transactional_output_bytes: u64,
}

fn checked_mul(left: u64, right: usize) -> Result<u64, PagedGqaProfileErrorV1> {
    left.checked_mul(
        u64::try_from(right).map_err(|_| PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?,
    )
    .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)
}

fn derive_resources(
    descriptor: PagedGqaProfileDescriptorV1,
) -> Result<PagedGqaResourceContractV1, PagedGqaProfileErrorV1> {
    let sequences = u64::try_from(descriptor.sequences)
        .map_err(|_| PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let active = u64::try_from(descriptor.active_tokens)
        .map_err(|_| PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let heads = u64::try_from(descriptor.geometry.query_heads)
        .map_err(|_| PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let query_elements = checked_mul(
        checked_mul(
            sequences
                .checked_mul(active)
                .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?,
            descriptor.geometry.query_heads,
        )?,
        descriptor.geometry.head_dimension,
    )?;
    let page_table_entries = checked_mul(sequences, M1_PAGES_PER_REQUEST_V1)?;
    let kv_elements_each = checked_mul(
        checked_mul(
            checked_mul(page_table_entries, descriptor.page_tokens)?,
            descriptor.geometry.kv_heads,
        )?,
        descriptor.geometry.head_dimension,
    )?;
    let context = u64::try_from(descriptor.context_capacity_tokens)
        .map_err(|_| PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let first_keys = context
        .checked_sub(active)
        .and_then(|value| value.checked_add(1))
        .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let pair_sum = active
        .checked_mul(first_keys)
        .and_then(|value| {
            active
                .checked_mul(active.saturating_sub(1))
                .and_then(|triangle| triangle.checked_div(2))
                .and_then(|triangle| value.checked_add(triangle))
        })
        .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let causal_pairs = sequences
        .checked_mul(heads)
        .and_then(|value| value.checked_mul(pair_sum))
        .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let qk_multiplications = checked_mul(causal_pairs, descriptor.geometry.head_dimension)?;
    let value_multiplications = qk_multiplications;
    let output_divisions = query_elements;
    let vector_scratch_bytes = context
        .checked_mul(2)
        .and_then(|value| value.checked_mul(4))
        .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    let transactional_output_bytes = query_elements
        .checked_mul(2)
        .ok_or(PagedGqaProfileErrorV1::ResourceArithmeticOverflow)?;
    Ok(PagedGqaResourceContractV1 {
        query_elements,
        output_elements: query_elements,
        page_table_entries,
        physical_pages: page_table_entries,
        kv_elements_each,
        causal_pairs,
        qk_multiplications,
        value_multiplications,
        exponential_evaluations: causal_pairs,
        output_divisions,
        vector_scratch_bytes,
        transactional_output_bytes,
    })
}

/// Validated inert profile with checked resource bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPagedGqaProfileV1 {
    descriptor: PagedGqaProfileDescriptorV1,
    resources: PagedGqaResourceContractV1,
}

impl ValidatedPagedGqaProfileV1 {
    /// Returns the exact descriptor.
    pub const fn descriptor(self) -> PagedGqaProfileDescriptorV1 {
        self.descriptor
    }

    /// Returns checked worst-case resources.
    pub const fn resources(self) -> PagedGqaResourceContractV1 {
        self.resources
    }
}

/// Validates all exact role, bucket, geometry, stage, and resource fields.
pub fn validate_paged_gqa_profile_v1(
    descriptor: PagedGqaProfileDescriptorV1,
) -> Result<ValidatedPagedGqaProfileV1, PagedGqaProfileErrorV1> {
    let exact = PagedGqaProfileDescriptorV1::canonical(descriptor.role, descriptor.bucket);
    if descriptor.sequences != exact.sequences {
        return Err(PagedGqaProfileErrorV1::Sequences);
    }
    if descriptor.active_tokens != exact.active_tokens {
        return Err(PagedGqaProfileErrorV1::ActiveTokens);
    }
    if descriptor.context_capacity_tokens != exact.context_capacity_tokens {
        return Err(PagedGqaProfileErrorV1::ContextCapacity);
    }
    if descriptor.page_tokens != exact.page_tokens {
        return Err(PagedGqaProfileErrorV1::PageTokens);
    }
    if descriptor.geometry.hidden_size != exact.geometry.hidden_size {
        return Err(PagedGqaProfileErrorV1::HiddenSize);
    }
    if descriptor.geometry.query_heads != exact.geometry.query_heads {
        return Err(PagedGqaProfileErrorV1::QueryHeads);
    }
    if descriptor.geometry.kv_heads != exact.geometry.kv_heads {
        return Err(PagedGqaProfileErrorV1::KvHeads);
    }
    if descriptor.geometry.head_dimension != exact.geometry.head_dimension {
        return Err(PagedGqaProfileErrorV1::HeadDimension);
    }
    if descriptor.geometry.query_heads_per_kv_head != exact.geometry.query_heads_per_kv_head {
        return Err(PagedGqaProfileErrorV1::GqaGroupSize);
    }
    if descriptor.geometry.query_projection_size != exact.geometry.query_projection_size {
        return Err(PagedGqaProfileErrorV1::QueryProjection);
    }
    if descriptor.geometry.kv_projection_size != exact.geometry.kv_projection_size {
        return Err(PagedGqaProfileErrorV1::KvProjection);
    }
    if descriptor.tensor_stage != exact.tensor_stage {
        return Err(PagedGqaProfileErrorV1::TensorStage);
    }
    let resources = derive_resources(descriptor)?;
    if resources.query_elements > MAX_PAGED_GQA_QUERY_ELEMENTS_V1
        || resources.output_elements > MAX_PAGED_GQA_QUERY_ELEMENTS_V1
        || resources.kv_elements_each > MAX_PAGED_GQA_KV_ELEMENTS_V1
        || resources.causal_pairs > MAX_PAGED_GQA_CAUSAL_PAIRS_V1
        || resources.physical_pages > M1_MAX_PHYSICAL_PAGES_V1 as u64
    {
        return Err(PagedGqaProfileErrorV1::ResourceLimit);
    }
    Ok(ValidatedPagedGqaProfileV1 {
        descriptor,
        resources,
    })
}

/// BF16 input interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Bf16InputPolicyV1 {
    /// Decode all finite BF16 bit patterns exactly into host FP32.
    ExactFiniteBits = 1,
    /// Unsupported flush-to-zero interpretation.
    FlushSubnormals = 2,
}

/// Softmax algorithm and exception policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SoftmaxPolicyV1 {
    /// Ascending two-pass max-subtracted host FP32 softmax.
    StableAscendingTwoPass = 1,
    /// Unsupported unstabilized one-pass softmax.
    UnstableOnePass = 2,
}

/// Complete numerical and evaluation-order policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedGqaNumericalPolicyV1 {
    /// Input decode policy.
    pub bf16_input: Bf16InputPolicyV1,
    /// Exact attention scale bits.
    pub attention_scale_bits: u32,
    /// QK features are multiplied and accumulated in ascending order in FP32.
    pub qk_features_ascending_fp32: bool,
    /// Logical keys are visited in ascending order.
    pub logical_keys_ascending: bool,
    /// Softmax and exception policy.
    pub softmax: SoftmaxPolicyV1,
    /// Weighted-value products and sums use FP32 in ascending-key order.
    pub value_accumulation_ascending_fp32: bool,
    /// Every logically read input and intermediate must be finite.
    pub reject_non_finite: bool,
    /// Exponential underflow to positive zero is admitted.
    pub allow_positive_zero_exp_underflow: bool,
    /// Output converts to BF16 round-to-nearest, ties-to-even.
    pub bf16_output_rne: bool,
}

impl PagedGqaNumericalPolicyV1 {
    /// Returns the only admitted policy.
    pub const fn exact() -> Self {
        Self {
            bf16_input: Bf16InputPolicyV1::ExactFiniteBits,
            attention_scale_bits: QWEN3_ATTENTION_SCALE_BITS_V1,
            qk_features_ascending_fp32: true,
            logical_keys_ascending: true,
            softmax: SoftmaxPolicyV1::StableAscendingTwoPass,
            value_accumulation_ascending_fp32: true,
            reject_non_finite: true,
            allow_positive_zero_exp_underflow: true,
            bf16_output_rne: true,
        }
    }
}

/// Numerical-policy mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaNumericalErrorV1 {
    /// At least one field differed from the exact policy.
    NonCanonical,
}

/// Validates the complete numerical policy.
pub fn validate_paged_gqa_numerical_policy_v1(
    policy: PagedGqaNumericalPolicyV1,
) -> Result<(), PagedGqaNumericalErrorV1> {
    if policy != PagedGqaNumericalPolicyV1::exact() {
        return Err(PagedGqaNumericalErrorV1::NonCanonical);
    }
    Ok(())
}

/// Complete logical effect, alias, and race contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedGqaEffectContractV1 {
    /// Query is initialized and read-only.
    pub query_initialized_read_only: bool,
    /// Page metadata is initialized and read-only.
    pub page_metadata_initialized_read_only: bool,
    /// Only initialized logical K/V prefixes are read.
    pub initialized_kv_prefix_read_only: bool,
    /// Key and value allocations are disjoint.
    pub key_value_allocations_disjoint: bool,
    /// Physical pages are injective across the batch.
    pub physical_page_mapping_injective: bool,
    /// Request identity and generation are checked for every page.
    pub request_generation_bound: bool,
    /// Final-page initialized masks are enforced.
    pub final_page_mask_enforced: bool,
    /// Output has one logical writer and is separately staged.
    pub transactional_single_writer_output: bool,
    /// Host model uses no atomics.
    pub atomics: u8,
    /// Host model uses no barriers.
    pub barriers: u8,
}

impl PagedGqaEffectContractV1 {
    /// Returns the only admitted effect contract.
    pub const fn exact() -> Self {
        Self {
            query_initialized_read_only: true,
            page_metadata_initialized_read_only: true,
            initialized_kv_prefix_read_only: true,
            key_value_allocations_disjoint: true,
            physical_page_mapping_injective: true,
            request_generation_bound: true,
            final_page_mask_enforced: true,
            transactional_single_writer_output: true,
            atomics: 0,
            barriers: 0,
        }
    }
}

/// Effect-contract mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaEffectErrorV1 {
    /// At least one field differed from the exact contract.
    NonCanonical,
}

/// Validates every effect, alias, and race premise.
pub fn validate_paged_gqa_effect_contract_v1(
    effects: PagedGqaEffectContractV1,
) -> Result<(), PagedGqaEffectErrorV1> {
    if effects != PagedGqaEffectContractV1::exact() {
        return Err(PagedGqaEffectErrorV1::NonCanonical);
    }
    Ok(())
}

/// Stable nonzero request identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PagedKvRequestIdV1(pub [u8; 16]);

impl PagedKvRequestIdV1 {
    /// Returns whether at least one identity byte is nonzero.
    pub fn is_present(self) -> bool {
        self.0.iter().any(|byte| *byte != 0)
    }
}

/// Stable nonzero physical allocation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PagedKvAllocationIdV1(pub [u8; 16]);

impl PagedKvAllocationIdV1 {
    /// Returns whether at least one identity byte is nonzero.
    pub fn is_present(self) -> bool {
        self.0.iter().any(|byte| *byte != 0)
    }
}

/// Per-request stable and tentative prefix boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedKvRequestV1 {
    /// Stable request identity.
    pub request_id: PagedKvRequestIdV1,
    /// Nonzero request/page-table generation.
    pub generation: u64,
    /// Stable prefix accepted by the engine.
    pub committed_tokens: usize,
    /// Initialized prefix, including the exact current active-token suffix.
    pub resident_tokens: usize,
}

/// One K3-compatible logical-to-physical page-table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedKvPageEntryV1 {
    /// Exact zero-based logical page within the request.
    pub logical_page: u16,
    /// Physical page in the batch pool.
    pub physical_page: u32,
    /// Exact live request generation.
    pub physical_generation: u64,
    /// Exact request owner.
    pub request_id: PagedKvRequestIdV1,
    /// Initialized prefix count in this page.
    pub initialized_tokens: u16,
    /// Low-bit initialization mask, including the exact final-page tail.
    pub initialized_mask: u16,
}

/// Complete read-only batch page metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagedKvBatchMetadataV1 {
    /// Role-typed target or draft cache namespace.
    pub role: Qwen3AttentionRoleV1,
    /// Exact P16 page size.
    pub page_tokens: usize,
    /// Exact C8192 logical capacity.
    pub context_capacity_tokens: usize,
    /// Exact finite physical page count.
    pub physical_pages: usize,
    /// Stable key allocation identity.
    pub key_allocation: PagedKvAllocationIdV1,
    /// Stable value allocation identity.
    pub value_allocation: PagedKvAllocationIdV1,
    /// Requests in canonical batch order.
    pub requests: Vec<PagedKvRequestV1>,
    /// Flat `[request][logical_page]` table.
    pub entries: Vec<PagedKvPageEntryV1>,
}

/// Page-table, request, generation, prefix, mask, or alias failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedKvMetadataErrorV1 {
    /// Cache role differed from the profile.
    RoleMismatch,
    /// Page size or context capacity differed.
    BucketMismatch,
    /// Physical page count was not exact or exceeded the bound.
    PhysicalPageCount,
    /// Key or value allocation identity was absent.
    MissingAllocationIdentity,
    /// Key and value named the same allocation.
    AllocationAlias,
    /// Request count differed from the bucket.
    RequestCount,
    /// A request identity was absent or duplicated.
    RequestIdentity,
    /// A request generation was zero.
    MissingGeneration,
    /// Committed plus active did not equal resident.
    ResidentBoundary,
    /// A committed or resident boundary exceeded C8192.
    ContextBounds,
    /// Page-table length differed from `requests * 512`.
    EntryCount,
    /// Logical page order was not exact within a request.
    LogicalPageOrder,
    /// A physical page index was outside the exact pool.
    PhysicalPageOutOfBounds,
    /// Physical pages aliased or did not cover the pool exactly once.
    PhysicalPageAlias,
    /// Page owner differed from its request.
    StaleRequest,
    /// Page generation differed from its request.
    StaleGeneration,
    /// Initialized-token count did not encode the exact resident prefix.
    InitializedPrefix,
    /// Initialized mask did not encode the exact resident prefix/tail.
    InitializedMask,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// Bounded validation scratch allocation failed.
    AllocationFailure,
}

/// Validated inert page-metadata summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPagedKvMetadataV1 {
    physical_pages: usize,
    entries: usize,
}

impl ValidatedPagedKvMetadataV1 {
    /// Returns the exact physical page count.
    pub const fn physical_pages(self) -> usize {
        self.physical_pages
    }

    /// Returns the exact page-table entry count.
    pub const fn entries(self) -> usize {
        self.entries
    }
}

/// Returns the canonical low-bit mask for an initialized P16 prefix.
pub const fn p16_initialized_mask_v1(tokens: usize) -> Option<u16> {
    match tokens {
        0 => Some(0),
        1..=15 => Some((1_u16 << tokens) - 1),
        16 => Some(u16::MAX),
        _ => None,
    }
}

/// Validates exact batch shape, request/generation binding, initialized
/// prefixes, final-page masks, and an injective fragmented page permutation.
pub fn validate_paged_kv_metadata_v1(
    profile: ValidatedPagedGqaProfileV1,
    metadata: &PagedKvBatchMetadataV1,
) -> Result<ValidatedPagedKvMetadataV1, PagedKvMetadataErrorV1> {
    let descriptor = profile.descriptor();
    if metadata.role != descriptor.role {
        return Err(PagedKvMetadataErrorV1::RoleMismatch);
    }
    if metadata.page_tokens != M1_KV_PAGE_TOKENS_V1
        || metadata.context_capacity_tokens != M1_CONTEXT_CAPACITY_TOKENS_V1
    {
        return Err(PagedKvMetadataErrorV1::BucketMismatch);
    }
    let expected_pages = descriptor
        .sequences
        .checked_mul(M1_PAGES_PER_REQUEST_V1)
        .ok_or(PagedKvMetadataErrorV1::ArithmeticOverflow)?;
    if metadata.physical_pages != expected_pages
        || metadata.physical_pages > M1_MAX_PHYSICAL_PAGES_V1
    {
        return Err(PagedKvMetadataErrorV1::PhysicalPageCount);
    }
    if !metadata.key_allocation.is_present() || !metadata.value_allocation.is_present() {
        return Err(PagedKvMetadataErrorV1::MissingAllocationIdentity);
    }
    if metadata.key_allocation == metadata.value_allocation {
        return Err(PagedKvMetadataErrorV1::AllocationAlias);
    }
    if metadata.requests.len() != descriptor.sequences {
        return Err(PagedKvMetadataErrorV1::RequestCount);
    }
    let expected_entries = expected_pages;
    if metadata.entries.len() != expected_entries {
        return Err(PagedKvMetadataErrorV1::EntryCount);
    }

    let mut requests = HashSet::new();
    requests
        .try_reserve(metadata.requests.len())
        .map_err(|_| PagedKvMetadataErrorV1::AllocationFailure)?;
    let mut physical_pages = HashSet::new();
    physical_pages
        .try_reserve(metadata.entries.len())
        .map_err(|_| PagedKvMetadataErrorV1::AllocationFailure)?;
    for (request_index, request) in metadata.requests.iter().enumerate() {
        if !request.request_id.is_present() || !requests.insert(request.request_id) {
            return Err(PagedKvMetadataErrorV1::RequestIdentity);
        }
        if request.generation == 0 {
            return Err(PagedKvMetadataErrorV1::MissingGeneration);
        }
        let expected_resident = request
            .committed_tokens
            .checked_add(descriptor.active_tokens)
            .ok_or(PagedKvMetadataErrorV1::ArithmeticOverflow)?;
        if request.resident_tokens != expected_resident {
            return Err(PagedKvMetadataErrorV1::ResidentBoundary);
        }
        if request.committed_tokens > descriptor.context_capacity_tokens
            || request.resident_tokens > descriptor.context_capacity_tokens
        {
            return Err(PagedKvMetadataErrorV1::ContextBounds);
        }
        let table_start = request_index
            .checked_mul(M1_PAGES_PER_REQUEST_V1)
            .ok_or(PagedKvMetadataErrorV1::ArithmeticOverflow)?;
        for logical_page in 0..M1_PAGES_PER_REQUEST_V1 {
            let entry = metadata
                .entries
                .get(table_start + logical_page)
                .ok_or(PagedKvMetadataErrorV1::EntryCount)?;
            if usize::from(entry.logical_page) != logical_page {
                return Err(PagedKvMetadataErrorV1::LogicalPageOrder);
            }
            let physical_page = usize::try_from(entry.physical_page)
                .map_err(|_| PagedKvMetadataErrorV1::PhysicalPageOutOfBounds)?;
            if physical_page >= metadata.physical_pages {
                return Err(PagedKvMetadataErrorV1::PhysicalPageOutOfBounds);
            }
            if !physical_pages.insert(entry.physical_page) {
                return Err(PagedKvMetadataErrorV1::PhysicalPageAlias);
            }
            if entry.request_id != request.request_id {
                return Err(PagedKvMetadataErrorV1::StaleRequest);
            }
            if entry.physical_generation != request.generation {
                return Err(PagedKvMetadataErrorV1::StaleGeneration);
            }
            let page_start = logical_page
                .checked_mul(M1_KV_PAGE_TOKENS_V1)
                .ok_or(PagedKvMetadataErrorV1::ArithmeticOverflow)?;
            let initialized = request
                .resident_tokens
                .saturating_sub(page_start)
                .min(M1_KV_PAGE_TOKENS_V1);
            if usize::from(entry.initialized_tokens) != initialized {
                return Err(PagedKvMetadataErrorV1::InitializedPrefix);
            }
            if Some(entry.initialized_mask) != p16_initialized_mask_v1(initialized) {
                return Err(PagedKvMetadataErrorV1::InitializedMask);
            }
        }
    }
    if physical_pages.len() != metadata.physical_pages {
        return Err(PagedKvMetadataErrorV1::PhysicalPageAlias);
    }
    Ok(ValidatedPagedKvMetadataV1 {
        physical_pages: metadata.physical_pages,
        entries: metadata.entries.len(),
    })
}

/// Maps a query head to the exact shared KV head.
pub fn paged_gqa_kv_head_for_query_v1(
    profile: ValidatedPagedGqaProfileV1,
    query_head: usize,
) -> Option<usize> {
    let geometry = profile.descriptor().geometry;
    if query_head >= geometry.query_heads {
        return None;
    }
    query_head.checked_div(geometry.query_heads_per_kv_head)
}

/// Maps one logical token to a physical page/slot after full metadata validation.
pub fn paged_kv_physical_token_v1(
    profile: ValidatedPagedGqaProfileV1,
    metadata: &PagedKvBatchMetadataV1,
    request: usize,
    logical_token: usize,
) -> Option<(usize, usize)> {
    validate_paged_kv_metadata_v1(profile, metadata).ok()?;
    let request_record = metadata.requests.get(request)?;
    if logical_token >= request_record.resident_tokens {
        return None;
    }
    let logical_page = logical_token.checked_div(M1_KV_PAGE_TOKENS_V1)?;
    let slot = logical_token.checked_rem(M1_KV_PAGE_TOKENS_V1)?;
    let table_index = request
        .checked_mul(M1_PAGES_PER_REQUEST_V1)?
        .checked_add(logical_page)?;
    let entry = metadata.entries.get(table_index)?;
    if entry.initialized_mask & (1_u16 << slot) == 0 {
        return None;
    }
    Some((usize::try_from(entry.physical_page).ok()?, slot))
}
