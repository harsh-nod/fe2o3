//! Transactional FP32 paged host reference and contiguous differential oracle.

use std::mem::size_of;

use crate::{
    Bf16ConversionErrorV1, Bf16V1, M1_KV_PAGE_TOKENS_V1, M1_PAGES_PER_REQUEST_V1,
    PagedGqaStructuralIdentityV1, PagedKvBatchMetadataV1, PagedKvMetadataErrorV1,
    QWEN3_ATTENTION_SCALE_BITS_V1, StructuralPagedGqaDecodeCandidateV1,
    paged_gqa_kv_head_for_query_v1, paged_kv_metadata_identity_v1, validate_paged_kv_metadata_v1,
};

/// Logical tensor named by a reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaTensorV1 {
    /// Query tensor.
    Query,
    /// Physical key pool.
    Key,
    /// Physical value pool.
    Value,
    /// Transactional attention output.
    Output,
    /// One-vector contiguous key oracle input.
    ContiguousKey,
    /// One-vector contiguous value oracle input.
    ContiguousValue,
}

/// FP32 stage that produced a rejected intermediate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaArithmeticStageV1 {
    /// One decoded BF16 Q*K product.
    QkProduct,
    /// Ascending-feature dot accumulation.
    QkSum,
    /// Post-dot attention-scale multiply.
    ScoreScale,
    /// Stable host exponential.
    Exponential,
    /// Ascending-key denominator accumulation.
    Denominator,
    /// One weight*V product.
    ValueProduct,
    /// Ascending-key weighted-value accumulation.
    ValueSum,
    /// Final FP32 denominator division.
    OutputDivision,
    /// BF16 output conversion.
    OutputCast,
}

/// Fail-closed paged reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagedGqaReferenceErrorV1 {
    /// Page metadata validation failed before any data read.
    Metadata(PagedKvMetadataErrorV1),
    /// One tensor length differed from the exact extent.
    WrongLength {
        /// Tensor with the wrong length.
        tensor: PagedGqaTensorV1,
        /// Required element count.
        expected: usize,
        /// Observed element count.
        actual: usize,
    },
    /// Key and value physical slice ranges overlap.
    KeyValuePhysicalAlias,
    /// An input value that is logically read was NaN or infinity.
    NonFiniteInput {
        /// Input tensor.
        tensor: PagedGqaTensorV1,
        /// Physical or contiguous element index.
        index: usize,
    },
    /// A requested vector coordinate was outside the exact profile/request.
    CoordinateOutOfRange,
    /// A logical read could not map to an initialized physical coordinate.
    InvalidPagedRead,
    /// Checked internal indexing unexpectedly failed.
    IndexingFailure,
    /// FP32 evaluation produced a rejected intermediate.
    NonFiniteIntermediate {
        /// Request coordinate.
        request: usize,
        /// Active-token coordinate.
        local_query: usize,
        /// Query-head coordinate.
        query_head: usize,
        /// Logical key-token coordinate, when applicable.
        logical_key: Option<usize>,
        /// Feature coordinate, when applicable.
        feature: Option<usize>,
        /// Rejected arithmetic stage.
        stage: PagedGqaArithmeticStageV1,
    },
    /// Bounded allocation failed.
    AllocationFailure,
}

impl From<PagedKvMetadataErrorV1> for PagedGqaReferenceErrorV1 {
    fn from(value: PagedKvMetadataErrorV1) -> Self {
        Self::Metadata(value)
    }
}

/// Borrowed immutable paged attention input tensors.
#[derive(Clone, Copy, Debug)]
pub struct PagedGqaInputV1<'a> {
    /// Contiguous query tensor.
    pub query: &'a [Bf16V1],
    /// Physical paged key pool.
    pub key: &'a [Bf16V1],
    /// Physical paged value pool.
    pub value: &'a [Bf16V1],
}

/// Summary of one complete transactional host evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagedGqaReferenceStateV1 {
    /// Completely evaluated output vectors.
    pub output_vectors: usize,
    /// Smallest stable softmax denominator.
    pub minimum_denominator: f32,
    /// Largest stable softmax denominator.
    pub maximum_denominator: f32,
    /// Deterministic identity of candidate plus page metadata, not tensor data.
    pub metadata_identity: PagedGqaStructuralIdentityV1,
}

fn allocate_filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, PagedGqaReferenceErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| PagedGqaReferenceErrorV1::AllocationFailure)?;
    result.resize(length, value);
    Ok(result)
}

fn exact_lengths(
    candidate: StructuralPagedGqaDecodeCandidateV1,
) -> Result<(usize, usize), PagedGqaReferenceErrorV1> {
    let query = usize::try_from(candidate.resources().query_elements)
        .map_err(|_| PagedGqaReferenceErrorV1::IndexingFailure)?;
    let kv = usize::try_from(candidate.resources().kv_elements_each)
        .map_err(|_| PagedGqaReferenceErrorV1::IndexingFailure)?;
    Ok((query, kv))
}

fn check_length(
    tensor: PagedGqaTensorV1,
    actual: usize,
    expected: usize,
) -> Result<(), PagedGqaReferenceErrorV1> {
    if actual != expected {
        return Err(PagedGqaReferenceErrorV1::WrongLength {
            tensor,
            expected,
            actual,
        });
    }
    Ok(())
}

fn slice_byte_range(values: &[Bf16V1]) -> Option<(usize, usize)> {
    let start = values.as_ptr() as usize;
    let bytes = values.len().checked_mul(size_of::<Bf16V1>())?;
    Some((start, start.checked_add(bytes)?))
}

fn slices_overlap(left: &[Bf16V1], right: &[Bf16V1]) -> Option<bool> {
    let (left_start, left_end) = slice_byte_range(left)?;
    let (right_start, right_end) = slice_byte_range(right)?;
    Some(left_start < right_end && right_start < left_end)
}

fn query_index(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    request: usize,
    local_query: usize,
    query_head: usize,
    feature: usize,
) -> Option<usize> {
    let profile = candidate.profile().descriptor();
    if request >= profile.sequences
        || local_query >= profile.active_tokens
        || query_head >= profile.geometry.query_heads
        || feature >= profile.geometry.head_dimension
    {
        return None;
    }
    request
        .checked_mul(profile.active_tokens)?
        .checked_add(local_query)?
        .checked_mul(profile.geometry.query_heads)?
        .checked_add(query_head)?
        .checked_mul(profile.geometry.head_dimension)?
        .checked_add(feature)
}

fn physical_kv_index(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    metadata: &PagedKvBatchMetadataV1,
    request: usize,
    logical_token: usize,
    kv_head: usize,
    feature: usize,
) -> Option<usize> {
    let profile = candidate.profile().descriptor();
    if request >= profile.sequences
        || logical_token >= metadata.requests.get(request)?.resident_tokens
        || kv_head >= profile.geometry.kv_heads
        || feature >= profile.geometry.head_dimension
    {
        return None;
    }
    let logical_page = logical_token.checked_div(M1_KV_PAGE_TOKENS_V1)?;
    let slot = logical_token.checked_rem(M1_KV_PAGE_TOKENS_V1)?;
    let entry_index = request
        .checked_mul(M1_PAGES_PER_REQUEST_V1)?
        .checked_add(logical_page)?;
    let entry = metadata.entries.get(entry_index)?;
    if entry.initialized_mask & (1_u16 << slot) == 0 {
        return None;
    }
    usize::try_from(entry.physical_page)
        .ok()?
        .checked_mul(M1_KV_PAGE_TOKENS_V1)?
        .checked_add(slot)?
        .checked_mul(profile.geometry.kv_heads)?
        .checked_add(kv_head)?
        .checked_mul(profile.geometry.head_dimension)?
        .checked_add(feature)
}

fn intermediate_error(
    request: usize,
    local_query: usize,
    query_head: usize,
    logical_key: Option<usize>,
    feature: Option<usize>,
    stage: PagedGqaArithmeticStageV1,
) -> PagedGqaReferenceErrorV1 {
    PagedGqaReferenceErrorV1::NonFiniteIntermediate {
        request,
        local_query,
        query_head,
        logical_key,
        feature,
        stage,
    }
}

struct VectorContext {
    request: usize,
    local_query: usize,
    query_head: usize,
    query_position: usize,
    kv_head: usize,
}

fn compute_paged_vector(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    metadata: &PagedKvBatchMetadataV1,
    input: PagedGqaInputV1<'_>,
    context: VectorContext,
    scores: &mut [f32],
    weights: &mut [f32],
    destination: &mut [Bf16V1],
) -> Result<f32, PagedGqaReferenceErrorV1> {
    let profile = candidate.profile().descriptor();
    let key_count = context
        .query_position
        .checked_add(1)
        .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
    if scores.len() < key_count || weights.len() < key_count {
        return Err(PagedGqaReferenceErrorV1::IndexingFailure);
    }
    let scale = f32::from_bits(QWEN3_ATTENTION_SCALE_BITS_V1);
    for (logical_key, score_slot) in scores.iter_mut().enumerate().take(key_count) {
        let mut dot = 0.0_f32;
        for feature in 0..profile.geometry.head_dimension {
            let q_index = query_index(
                candidate,
                context.request,
                context.local_query,
                context.query_head,
                feature,
            )
            .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
            let k_index = physical_kv_index(
                candidate,
                metadata,
                context.request,
                logical_key,
                context.kv_head,
                feature,
            )
            .ok_or(PagedGqaReferenceErrorV1::InvalidPagedRead)?;
            let query = input.query[q_index];
            let key = input.key[k_index];
            if !query.is_finite() {
                return Err(PagedGqaReferenceErrorV1::NonFiniteInput {
                    tensor: PagedGqaTensorV1::Query,
                    index: q_index,
                });
            }
            if !key.is_finite() {
                return Err(PagedGqaReferenceErrorV1::NonFiniteInput {
                    tensor: PagedGqaTensorV1::Key,
                    index: k_index,
                });
            }
            let product = query.to_f32() * key.to_f32();
            if !product.is_finite() {
                return Err(intermediate_error(
                    context.request,
                    context.local_query,
                    context.query_head,
                    Some(logical_key),
                    Some(feature),
                    PagedGqaArithmeticStageV1::QkProduct,
                ));
            }
            dot += product;
            if !dot.is_finite() {
                return Err(intermediate_error(
                    context.request,
                    context.local_query,
                    context.query_head,
                    Some(logical_key),
                    Some(feature),
                    PagedGqaArithmeticStageV1::QkSum,
                ));
            }
        }
        let score = dot * scale;
        if !score.is_finite() {
            return Err(intermediate_error(
                context.request,
                context.local_query,
                context.query_head,
                Some(logical_key),
                None,
                PagedGqaArithmeticStageV1::ScoreScale,
            ));
        }
        *score_slot = score;
    }
    let maximum = scores[..key_count]
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let mut denominator = 0.0_f32;
    for (score, weight) in scores[..key_count].iter().zip(&mut weights[..key_count]) {
        let evaluated = (*score - maximum).exp();
        if !evaluated.is_finite() || evaluated < 0.0 {
            return Err(intermediate_error(
                context.request,
                context.local_query,
                context.query_head,
                None,
                None,
                PagedGqaArithmeticStageV1::Exponential,
            ));
        }
        *weight = evaluated;
        denominator += evaluated;
        if !denominator.is_finite() {
            return Err(intermediate_error(
                context.request,
                context.local_query,
                context.query_head,
                None,
                None,
                PagedGqaArithmeticStageV1::Denominator,
            ));
        }
    }
    if !(denominator.is_finite() && denominator > 0.0) {
        return Err(intermediate_error(
            context.request,
            context.local_query,
            context.query_head,
            None,
            None,
            PagedGqaArithmeticStageV1::Denominator,
        ));
    }
    for (feature, output) in destination.iter_mut().enumerate() {
        let mut numerator = 0.0_f32;
        for (logical_key, weight) in weights[..key_count].iter().copied().enumerate() {
            let v_index = physical_kv_index(
                candidate,
                metadata,
                context.request,
                logical_key,
                context.kv_head,
                feature,
            )
            .ok_or(PagedGqaReferenceErrorV1::InvalidPagedRead)?;
            let value = input.value[v_index];
            if !value.is_finite() {
                return Err(PagedGqaReferenceErrorV1::NonFiniteInput {
                    tensor: PagedGqaTensorV1::Value,
                    index: v_index,
                });
            }
            let product = weight * value.to_f32();
            if !product.is_finite() {
                return Err(intermediate_error(
                    context.request,
                    context.local_query,
                    context.query_head,
                    Some(logical_key),
                    Some(feature),
                    PagedGqaArithmeticStageV1::ValueProduct,
                ));
            }
            numerator += product;
            if !numerator.is_finite() {
                return Err(intermediate_error(
                    context.request,
                    context.local_query,
                    context.query_head,
                    Some(logical_key),
                    Some(feature),
                    PagedGqaArithmeticStageV1::ValueSum,
                ));
            }
        }
        let value = numerator / denominator;
        if !value.is_finite() {
            return Err(intermediate_error(
                context.request,
                context.local_query,
                context.query_head,
                None,
                Some(feature),
                PagedGqaArithmeticStageV1::OutputDivision,
            ));
        }
        *output = Bf16V1::from_f32_rne(value).map_err(|error| {
            let _ = match error {
                Bf16ConversionErrorV1::NonFiniteInput | Bf16ConversionErrorV1::NonFiniteOutput => {
                    error
                }
            };
            intermediate_error(
                context.request,
                context.local_query,
                context.query_head,
                None,
                Some(feature),
                PagedGqaArithmeticStageV1::OutputCast,
            )
        })?;
    }
    Ok(denominator)
}

/// Evaluates the complete exact paged batch and publishes output transactionally.
///
/// K/V pages must already contain the exact K3-produced resident prefix. This
/// function performs no RoPE or KV write and creates no compiler or runtime
/// authority.
pub fn qwen3_paged_gqa_decode_reference_v1(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    metadata: &PagedKvBatchMetadataV1,
    input: PagedGqaInputV1<'_>,
    output: &mut [Bf16V1],
) -> Result<PagedGqaReferenceStateV1, PagedGqaReferenceErrorV1> {
    validate_paged_kv_metadata_v1(candidate.profile(), metadata)?;
    let metadata_identity = paged_kv_metadata_identity_v1(candidate, metadata)?;
    let (query_elements, kv_elements) = exact_lengths(candidate)?;
    check_length(PagedGqaTensorV1::Query, input.query.len(), query_elements)?;
    check_length(PagedGqaTensorV1::Key, input.key.len(), kv_elements)?;
    check_length(PagedGqaTensorV1::Value, input.value.len(), kv_elements)?;
    check_length(PagedGqaTensorV1::Output, output.len(), query_elements)?;
    if slices_overlap(input.key, input.value).ok_or(PagedGqaReferenceErrorV1::IndexingFailure)? {
        return Err(PagedGqaReferenceErrorV1::KeyValuePhysicalAlias);
    }

    let profile = candidate.profile().descriptor();
    let mut staged = allocate_filled(query_elements, Bf16V1::default())?;
    let mut scores = allocate_filled(profile.context_capacity_tokens, 0.0_f32)?;
    let mut weights = allocate_filled(profile.context_capacity_tokens, 0.0_f32)?;
    let mut minimum_denominator = f32::INFINITY;
    let mut maximum_denominator = 0.0_f32;
    let mut output_vectors = 0_usize;
    for request in 0..profile.sequences {
        let request_record = metadata
            .requests
            .get(request)
            .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
        for local_query in 0..profile.active_tokens {
            let query_position = request_record
                .committed_tokens
                .checked_add(local_query)
                .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
            for query_head in 0..profile.geometry.query_heads {
                let kv_head = paged_gqa_kv_head_for_query_v1(candidate.profile(), query_head)
                    .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
                let output_start = query_index(candidate, request, local_query, query_head, 0)
                    .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
                let output_end = output_start
                    .checked_add(profile.geometry.head_dimension)
                    .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
                let denominator = compute_paged_vector(
                    candidate,
                    metadata,
                    input,
                    VectorContext {
                        request,
                        local_query,
                        query_head,
                        query_position,
                        kv_head,
                    },
                    &mut scores,
                    &mut weights,
                    staged
                        .get_mut(output_start..output_end)
                        .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?,
                )?;
                minimum_denominator = minimum_denominator.min(denominator);
                maximum_denominator = maximum_denominator.max(denominator);
                output_vectors = output_vectors
                    .checked_add(1)
                    .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
            }
        }
    }
    output.copy_from_slice(&staged);
    Ok(PagedGqaReferenceStateV1 {
        output_vectors,
        minimum_denominator,
        maximum_denominator,
        metadata_identity,
    })
}

/// Evaluates one vector from contiguous logical K/V as a differential oracle.
///
/// `query` has exactly 128 elements. K/V use
/// `[resident_token][kv_head][feature]` for the selected request. This oracle
/// shares the declared FP32 order but not the page-table coordinate path.
pub fn qwen3_contiguous_gqa_decode_vector_v1(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    request: &crate::PagedKvRequestV1,
    local_query: usize,
    query_head: usize,
    query: &[Bf16V1],
    key: &[Bf16V1],
    value: &[Bf16V1],
) -> Result<Vec<Bf16V1>, PagedGqaReferenceErrorV1> {
    let profile = candidate.profile().descriptor();
    if local_query >= profile.active_tokens
        || query_head >= profile.geometry.query_heads
        || request.resident_tokens
            != request
                .committed_tokens
                .checked_add(profile.active_tokens)
                .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?
        || request.resident_tokens > profile.context_capacity_tokens
    {
        return Err(PagedGqaReferenceErrorV1::CoordinateOutOfRange);
    }
    check_length(
        PagedGqaTensorV1::Query,
        query.len(),
        profile.geometry.head_dimension,
    )?;
    let kv_elements = request
        .resident_tokens
        .checked_mul(profile.geometry.kv_heads)
        .and_then(|value| value.checked_mul(profile.geometry.head_dimension))
        .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
    check_length(PagedGqaTensorV1::ContiguousKey, key.len(), kv_elements)?;
    check_length(PagedGqaTensorV1::ContiguousValue, value.len(), kv_elements)?;
    let kv_head = paged_gqa_kv_head_for_query_v1(candidate.profile(), query_head)
        .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
    let query_position = request
        .committed_tokens
        .checked_add(local_query)
        .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
    let key_count = query_position
        .checked_add(1)
        .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
    let mut scores = allocate_filled(key_count, 0.0_f32)?;
    let mut weights = allocate_filled(key_count, 0.0_f32)?;
    let scale = f32::from_bits(QWEN3_ATTENTION_SCALE_BITS_V1);
    for (logical_key, score) in scores.iter_mut().enumerate() {
        let mut dot = 0.0_f32;
        for (feature, query_value) in query.iter().copied().enumerate() {
            let index = logical_key
                .checked_mul(profile.geometry.kv_heads)
                .and_then(|value| value.checked_add(kv_head))
                .and_then(|value| value.checked_mul(profile.geometry.head_dimension))
                .and_then(|value| value.checked_add(feature))
                .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
            if !query_value.is_finite() {
                return Err(PagedGqaReferenceErrorV1::NonFiniteInput {
                    tensor: PagedGqaTensorV1::Query,
                    index: feature,
                });
            }
            if !key[index].is_finite() {
                return Err(PagedGqaReferenceErrorV1::NonFiniteInput {
                    tensor: PagedGqaTensorV1::ContiguousKey,
                    index,
                });
            }
            dot += query_value.to_f32() * key[index].to_f32();
            if !dot.is_finite() {
                return Err(intermediate_error(
                    0,
                    local_query,
                    query_head,
                    Some(logical_key),
                    Some(feature),
                    PagedGqaArithmeticStageV1::QkSum,
                ));
            }
        }
        *score = dot * scale;
        if !score.is_finite() {
            return Err(intermediate_error(
                0,
                local_query,
                query_head,
                Some(logical_key),
                None,
                PagedGqaArithmeticStageV1::ScoreScale,
            ));
        }
    }
    let maximum = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut denominator = 0.0_f32;
    for (score, weight) in scores.iter().zip(&mut weights) {
        *weight = (*score - maximum).exp();
        denominator += *weight;
    }
    if !(denominator.is_finite() && denominator > 0.0) {
        return Err(intermediate_error(
            0,
            local_query,
            query_head,
            None,
            None,
            PagedGqaArithmeticStageV1::Denominator,
        ));
    }
    let mut output = allocate_filled(profile.geometry.head_dimension, Bf16V1::default())?;
    for (feature, destination) in output.iter_mut().enumerate() {
        let mut numerator = 0.0_f32;
        for (logical_key, weight) in weights.iter().copied().enumerate() {
            let index = logical_key
                .checked_mul(profile.geometry.kv_heads)
                .and_then(|value| value.checked_add(kv_head))
                .and_then(|value| value.checked_mul(profile.geometry.head_dimension))
                .and_then(|value| value.checked_add(feature))
                .ok_or(PagedGqaReferenceErrorV1::IndexingFailure)?;
            if !value[index].is_finite() {
                return Err(PagedGqaReferenceErrorV1::NonFiniteInput {
                    tensor: PagedGqaTensorV1::ContiguousValue,
                    index,
                });
            }
            numerator += weight * value[index].to_f32();
            if !numerator.is_finite() {
                return Err(intermediate_error(
                    0,
                    local_query,
                    query_head,
                    Some(logical_key),
                    Some(feature),
                    PagedGqaArithmeticStageV1::ValueSum,
                ));
            }
        }
        *destination = Bf16V1::from_f32_rne(numerator / denominator).map_err(|_| {
            intermediate_error(
                0,
                local_query,
                query_head,
                None,
                Some(feature),
                PagedGqaArithmeticStageV1::OutputCast,
            )
        })?;
    }
    Ok(output)
}
