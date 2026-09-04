//! Transactional FP32 host reference and independent `f64` vector oracle.

use crate::{
    Bf16ConversionErrorV1, Bf16V1, QWEN3_ATTENTION_SCALE_BITS_V1, ValidatedGqaPrefillProfileV1,
    gqa_kv_head_for_query_v1, gqa_kv_index_v1, gqa_query_index_v1,
};

/// Logical tensor named by a host-model error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaTensorV1 {
    /// Query tensor.
    Query,
    /// Key tensor.
    Key,
    /// Value tensor.
    Value,
    /// Attention output tensor.
    Output,
}

/// FP32 stage that produced a rejected intermediate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaArithmeticStageV1 {
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

/// Fail-closed reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GqaReferenceErrorV1 {
    /// One physical tensor length differed from the exact profile extent.
    WrongLength {
        /// Tensor with the wrong length.
        tensor: GqaTensorV1,
        /// Required element count.
        expected: usize,
        /// Observed element count.
        actual: usize,
    },
    /// A physical input tensor contained NaN or infinity.
    NonFiniteInput {
        /// Input tensor.
        tensor: GqaTensorV1,
        /// Contiguous element index.
        index: usize,
    },
    /// Requested vector coordinate was outside the validated profile.
    CoordinateOutOfRange,
    /// Checked internal indexing unexpectedly failed.
    IndexingFailure,
    /// FP32 evaluation produced a rejected intermediate.
    NonFiniteIntermediate {
        /// Sequence coordinate.
        sequence: usize,
        /// Query-token coordinate.
        query_token: usize,
        /// Query-head coordinate.
        query_head: usize,
        /// Causal key-token coordinate, when applicable.
        key_token: Option<usize>,
        /// Feature coordinate, when applicable.
        feature: Option<usize>,
        /// Rejected arithmetic stage.
        stage: GqaArithmeticStageV1,
    },
    /// Bounded scratch allocation failed.
    AllocationFailure,
}

/// Summary of one complete FP32 host evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GqaReferenceStateV1 {
    /// Completely evaluated output vectors.
    pub output_vectors: usize,
    /// Smallest stable softmax denominator.
    pub minimum_denominator: f32,
    /// Largest stable softmax denominator.
    pub maximum_denominator: f32,
}

/// Independent idealized `f64` result for one output vector.
#[derive(Clone, Debug, PartialEq)]
pub struct GqaF64VectorOracleV1 {
    /// Maximum causal scaled score.
    pub maximum_score: f64,
    /// Sum of stable causal weights.
    pub denominator: f64,
    /// One head-dimension output vector.
    pub output: Vec<f64>,
}

/// Borrowed immutable Q/K/V input tensors.
#[derive(Clone, Copy, Debug)]
pub struct GqaInputV1<'a> {
    /// Query tensor.
    pub query: &'a [Bf16V1],
    /// Key tensor.
    pub key: &'a [Bf16V1],
    /// Value tensor.
    pub value: &'a [Bf16V1],
}

/// One output-vector coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GqaVectorCoordinateV1 {
    /// Independent sequence coordinate.
    pub sequence: usize,
    /// Query-token coordinate.
    pub query_token: usize,
    /// Query-head coordinate.
    pub query_head: usize,
}

struct ScratchF32<'a> {
    scores: &'a mut [f32],
    weights: &'a mut [f32],
}

fn allocate_filled<T: Clone>(length: usize, value: T) -> Result<Vec<T>, GqaReferenceErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| GqaReferenceErrorV1::AllocationFailure)?;
    result.resize(length, value);
    Ok(result)
}

fn exact_lengths(
    profile: ValidatedGqaPrefillProfileV1,
) -> Result<(usize, usize), GqaReferenceErrorV1> {
    let query = usize::try_from(profile.resources().query_elements)
        .map_err(|_| GqaReferenceErrorV1::IndexingFailure)?;
    let kv = usize::try_from(profile.resources().kv_elements_each)
        .map_err(|_| GqaReferenceErrorV1::IndexingFailure)?;
    Ok((query, kv))
}

fn check_length(
    tensor: GqaTensorV1,
    actual: usize,
    expected: usize,
) -> Result<(), GqaReferenceErrorV1> {
    if actual != expected {
        return Err(GqaReferenceErrorV1::WrongLength {
            tensor,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_finite(tensor: GqaTensorV1, values: &[Bf16V1]) -> Result<(), GqaReferenceErrorV1> {
    if let Some((index, _)) = values
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(GqaReferenceErrorV1::NonFiniteInput { tensor, index });
    }
    Ok(())
}

fn validate_inputs(
    profile: ValidatedGqaPrefillProfileV1,
    input: GqaInputV1<'_>,
) -> Result<(usize, usize), GqaReferenceErrorV1> {
    let (query_elements, kv_elements) = exact_lengths(profile)?;
    check_length(GqaTensorV1::Query, input.query.len(), query_elements)?;
    check_length(GqaTensorV1::Key, input.key.len(), kv_elements)?;
    check_length(GqaTensorV1::Value, input.value.len(), kv_elements)?;
    check_finite(GqaTensorV1::Query, input.query)?;
    check_finite(GqaTensorV1::Key, input.key)?;
    check_finite(GqaTensorV1::Value, input.value)?;
    Ok((query_elements, kv_elements))
}

fn validate_coordinate(
    profile: ValidatedGqaPrefillProfileV1,
    coordinate: GqaVectorCoordinateV1,
) -> Result<(), GqaReferenceErrorV1> {
    let descriptor = profile.descriptor();
    if coordinate.sequence >= descriptor.sequences
        || coordinate.query_token >= descriptor.active_tokens
        || coordinate.query_head >= descriptor.geometry.query_heads
    {
        return Err(GqaReferenceErrorV1::CoordinateOutOfRange);
    }
    Ok(())
}

fn intermediate_error(
    coordinate: GqaVectorCoordinateV1,
    key_token: Option<usize>,
    feature: Option<usize>,
    stage: GqaArithmeticStageV1,
) -> GqaReferenceErrorV1 {
    GqaReferenceErrorV1::NonFiniteIntermediate {
        sequence: coordinate.sequence,
        query_token: coordinate.query_token,
        query_head: coordinate.query_head,
        key_token,
        feature,
        stage,
    }
}

fn compute_weights_f32(
    profile: ValidatedGqaPrefillProfileV1,
    input: GqaInputV1<'_>,
    coordinate: GqaVectorCoordinateV1,
    scratch: &mut ScratchF32<'_>,
) -> Result<f32, GqaReferenceErrorV1> {
    let descriptor = profile.descriptor();
    let geometry = descriptor.geometry;
    let kv_head = gqa_kv_head_for_query_v1(profile, coordinate.query_head)
        .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
    let scale = f32::from_bits(QWEN3_ATTENTION_SCALE_BITS_V1);

    for (key_token, score_slot) in scratch
        .scores
        .iter_mut()
        .enumerate()
        .take(coordinate.query_token + 1)
    {
        let mut dot = 0.0_f32;
        for feature in 0..geometry.head_dimension {
            let q_index = gqa_query_index_v1(
                profile,
                coordinate.sequence,
                coordinate.query_token,
                coordinate.query_head,
                feature,
            )
            .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
            let k_index =
                gqa_kv_index_v1(profile, coordinate.sequence, key_token, kv_head, feature)
                    .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
            let product = input.query[q_index].to_f32() * input.key[k_index].to_f32();
            if !product.is_finite() {
                return Err(intermediate_error(
                    coordinate,
                    Some(key_token),
                    Some(feature),
                    GqaArithmeticStageV1::QkProduct,
                ));
            }
            dot += product;
            if !dot.is_finite() {
                return Err(intermediate_error(
                    coordinate,
                    Some(key_token),
                    Some(feature),
                    GqaArithmeticStageV1::QkSum,
                ));
            }
        }
        let score = dot * scale;
        if !score.is_finite() {
            return Err(intermediate_error(
                coordinate,
                Some(key_token),
                None,
                GqaArithmeticStageV1::ScoreScale,
            ));
        }
        *score_slot = score;
    }

    let active_scores = &scratch.scores[..=coordinate.query_token];
    let mut maximum = active_scores[0];
    for score in &active_scores[1..] {
        maximum = maximum.max(*score);
    }
    let mut denominator = 0.0_f32;
    for (key_token, weight_slot) in scratch
        .weights
        .iter_mut()
        .enumerate()
        .take(coordinate.query_token + 1)
    {
        let weight = (scratch.scores[key_token] - maximum).exp();
        if !weight.is_finite() || weight < 0.0 {
            return Err(intermediate_error(
                coordinate,
                Some(key_token),
                None,
                GqaArithmeticStageV1::Exponential,
            ));
        }
        *weight_slot = weight;
        denominator += weight;
        if !denominator.is_finite() || denominator <= 0.0 {
            return Err(intermediate_error(
                coordinate,
                Some(key_token),
                None,
                GqaArithmeticStageV1::Denominator,
            ));
        }
    }
    if denominator < 1.0 {
        return Err(intermediate_error(
            coordinate,
            Some(coordinate.query_token),
            None,
            GqaArithmeticStageV1::Denominator,
        ));
    }
    Ok(denominator)
}

fn compute_vector_f32(
    profile: ValidatedGqaPrefillProfileV1,
    input: GqaInputV1<'_>,
    coordinate: GqaVectorCoordinateV1,
    scratch: &mut ScratchF32<'_>,
    output: &mut [Bf16V1],
) -> Result<f32, GqaReferenceErrorV1> {
    validate_coordinate(profile, coordinate)?;
    let geometry = profile.descriptor().geometry;
    if output.len() != geometry.head_dimension {
        return Err(GqaReferenceErrorV1::WrongLength {
            tensor: GqaTensorV1::Output,
            expected: geometry.head_dimension,
            actual: output.len(),
        });
    }
    let kv_head = gqa_kv_head_for_query_v1(profile, coordinate.query_head)
        .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
    let denominator = compute_weights_f32(profile, input, coordinate, scratch)?;

    for (feature, output_slot) in output.iter_mut().enumerate() {
        let mut numerator = 0.0_f32;
        for (key_token, weight) in scratch
            .weights
            .iter()
            .enumerate()
            .take(coordinate.query_token + 1)
        {
            let v_index =
                gqa_kv_index_v1(profile, coordinate.sequence, key_token, kv_head, feature)
                    .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
            let product = *weight * input.value[v_index].to_f32();
            if !product.is_finite() {
                return Err(intermediate_error(
                    coordinate,
                    Some(key_token),
                    Some(feature),
                    GqaArithmeticStageV1::ValueProduct,
                ));
            }
            numerator += product;
            if !numerator.is_finite() {
                return Err(intermediate_error(
                    coordinate,
                    Some(key_token),
                    Some(feature),
                    GqaArithmeticStageV1::ValueSum,
                ));
            }
        }
        let result = numerator / denominator;
        if !result.is_finite() {
            return Err(intermediate_error(
                coordinate,
                None,
                Some(feature),
                GqaArithmeticStageV1::OutputDivision,
            ));
        }
        *output_slot = Bf16V1::from_f32_rne(result).map_err(|error| match error {
            Bf16ConversionErrorV1::NonFiniteInput | Bf16ConversionErrorV1::NonFiniteOutput => {
                intermediate_error(
                    coordinate,
                    None,
                    Some(feature),
                    GqaArithmeticStageV1::OutputCast,
                )
            }
        })?;
    }
    Ok(denominator)
}

/// Evaluates one exact causal GQA output vector transactionally.
///
/// Every physical Q/K/V element is preflighted as finite, including future
/// tokens not read by the requested causal vector. QK products and ascending
/// feature sums use FP32, followed by the exact FP32 scale. Causal scores are
/// materialized, maximum and exponential weights are evaluated in ascending
/// key order, and the FP32 denominator is accumulated in that order.
/// Weighted V products/sums use ascending key order, followed by FP32 division
/// and BF16 round-to-nearest, ties-to-even. Exponential underflow to positive
/// zero is allowed; all other non-finite inputs/intermediates reject. The
/// output is unchanged on error. Finite BF16 subnormals and signed zeros are
/// admitted and decoded exactly; this host model does not impose flush-to-zero.
///
/// Rust host arithmetic and `f32::exp` are testing semantics only. This does
/// not establish IEEE, OCML, compiler, ISA, or machine correspondence.
pub fn gqa_prefill_reference_vector_v1(
    profile: ValidatedGqaPrefillProfileV1,
    input: GqaInputV1<'_>,
    coordinate: GqaVectorCoordinateV1,
    output: &mut [Bf16V1],
) -> Result<f32, GqaReferenceErrorV1> {
    validate_inputs(profile, input)?;
    validate_coordinate(profile, coordinate)?;
    let descriptor = profile.descriptor();
    check_length(
        GqaTensorV1::Output,
        output.len(),
        descriptor.geometry.head_dimension,
    )?;
    let mut scores = allocate_filled(descriptor.active_tokens, 0.0_f32)?;
    let mut weights = allocate_filled(descriptor.active_tokens, 0.0_f32)?;
    let mut staged = allocate_filled(descriptor.geometry.head_dimension, Bf16V1::default())?;
    let mut scratch = ScratchF32 {
        scores: &mut scores,
        weights: &mut weights,
    };
    let denominator = compute_vector_f32(profile, input, coordinate, &mut scratch, &mut staged)?;
    output.copy_from_slice(&staged);
    Ok(denominator)
}

/// Evaluates the complete exact profile into a separate transactional output.
///
/// The implementation retains one score vector, one weight vector, and the
/// staged BF16 output. It never allocates a quadratic score tensor. The
/// checked resource record exposes the exact potentially large quadratic work
/// count for each B3 bucket. This host routine is a semantic reference, not a
/// performance implementation or GPU fallback.
pub fn gqa_prefill_reference_v1(
    profile: ValidatedGqaPrefillProfileV1,
    input: GqaInputV1<'_>,
    output: &mut [Bf16V1],
) -> Result<GqaReferenceStateV1, GqaReferenceErrorV1> {
    let (query_elements, _) = validate_inputs(profile, input)?;
    check_length(GqaTensorV1::Output, output.len(), query_elements)?;
    let descriptor = profile.descriptor();
    let geometry = descriptor.geometry;
    let mut staged = allocate_filled(query_elements, Bf16V1::default())?;
    let mut scores = allocate_filled(descriptor.active_tokens, 0.0_f32)?;
    let mut weights = allocate_filled(descriptor.active_tokens, 0.0_f32)?;
    let mut minimum_denominator = f32::INFINITY;
    let mut maximum_denominator = 0.0_f32;

    for sequence in 0..descriptor.sequences {
        for query_token in 0..descriptor.active_tokens {
            for query_head in 0..geometry.query_heads {
                let coordinate = GqaVectorCoordinateV1 {
                    sequence,
                    query_token,
                    query_head,
                };
                let first_output =
                    gqa_query_index_v1(profile, sequence, query_token, query_head, 0)
                        .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
                let end_output = first_output
                    .checked_add(geometry.head_dimension)
                    .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
                let mut scratch = ScratchF32 {
                    scores: &mut scores,
                    weights: &mut weights,
                };
                let denominator = compute_vector_f32(
                    profile,
                    input,
                    coordinate,
                    &mut scratch,
                    staged
                        .get_mut(first_output..end_output)
                        .ok_or(GqaReferenceErrorV1::IndexingFailure)?,
                )?;
                minimum_denominator = minimum_denominator.min(denominator);
                maximum_denominator = maximum_denominator.max(denominator);
            }
        }
    }

    output.copy_from_slice(&staged);
    Ok(GqaReferenceStateV1 {
        output_vectors: descriptor.sequences * descriptor.active_tokens * geometry.query_heads,
        minimum_denominator,
        maximum_denominator,
    })
}

/// Computes an independent idealized `f64` oracle for one output vector.
///
/// It evaluates the mathematical causal GQA relation with sequential host
/// `f64` operations and the ideal `1/sqrt(128)` scale. It does not reproduce
/// FP32 rounding, the pinned FP32 scale bits, or BF16 output conversion. It is
/// a differential oracle, not a proof of real arithmetic or machine behavior.
pub fn gqa_prefill_f64_vector_oracle_v1(
    profile: ValidatedGqaPrefillProfileV1,
    input: GqaInputV1<'_>,
    coordinate: GqaVectorCoordinateV1,
) -> Result<GqaF64VectorOracleV1, GqaReferenceErrorV1> {
    validate_inputs(profile, input)?;
    validate_coordinate(profile, coordinate)?;
    let descriptor = profile.descriptor();
    let geometry = descriptor.geometry;
    let kv_head = gqa_kv_head_for_query_v1(profile, coordinate.query_head)
        .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
    let mut scores = allocate_filled(coordinate.query_token + 1, 0.0_f64)?;
    let scale = 1.0_f64 / (geometry.head_dimension as f64).sqrt();

    for (key_token, score) in scores.iter_mut().enumerate() {
        let mut dot = 0.0_f64;
        for feature in 0..geometry.head_dimension {
            let q_index = gqa_query_index_v1(
                profile,
                coordinate.sequence,
                coordinate.query_token,
                coordinate.query_head,
                feature,
            )
            .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
            let k_index =
                gqa_kv_index_v1(profile, coordinate.sequence, key_token, kv_head, feature)
                    .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
            dot +=
                f64::from(input.query[q_index].to_f32()) * f64::from(input.key[k_index].to_f32());
        }
        *score = dot * scale;
    }
    let maximum_score = scores
        .iter()
        .copied()
        .reduce(f64::max)
        .ok_or(GqaReferenceErrorV1::CoordinateOutOfRange)?;
    let mut weights = allocate_filled(coordinate.query_token + 1, 0.0_f64)?;
    let mut denominator = 0.0_f64;
    for (score, weight) in scores.iter().zip(&mut weights) {
        *weight = (*score - maximum_score).exp();
        denominator += *weight;
    }
    if !denominator.is_finite() || denominator < 1.0 {
        return Err(intermediate_error(
            coordinate,
            None,
            None,
            GqaArithmeticStageV1::Denominator,
        ));
    }
    let mut output = allocate_filled(geometry.head_dimension, 0.0_f64)?;
    for (feature, output_slot) in output.iter_mut().enumerate() {
        let mut numerator = 0.0_f64;
        for (key_token, weight) in weights.iter().enumerate() {
            let v_index =
                gqa_kv_index_v1(profile, coordinate.sequence, key_token, kv_head, feature)
                    .ok_or(GqaReferenceErrorV1::IndexingFailure)?;
            numerator += *weight * f64::from(input.value[v_index].to_f32());
        }
        *output_slot = numerator / denominator;
        if !output_slot.is_finite() {
            return Err(intermediate_error(
                coordinate,
                None,
                Some(feature),
                GqaArithmeticStageV1::OutputDivision,
            ));
        }
    }
    Ok(GqaF64VectorOracleV1 {
        maximum_score,
        denominator,
        output,
    })
}
