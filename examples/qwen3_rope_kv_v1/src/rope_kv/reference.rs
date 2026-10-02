use super::{
    CandidateErrorV1, M1_MAX_CONTEXT_TOKENS_V1, QWEN3_HEAD_DIMENSION_V1,
    QWEN3_ROPE_HALF_DIMENSION_V1, QWEN3_ROPE_THETA_V1, Qwen3RopeKvCandidateV1,
    validate_qwen3_rope_kv_candidate_v1,
};
use std::error::Error;
use std::fmt;

/// Exact split-half pair for one rotary dimension.
#[must_use]
pub const fn qwen3_rotary_pair_v1(dimension: u16) -> Option<u16> {
    if dimension < QWEN3_ROPE_HALF_DIMENSION_V1 {
        Some(dimension + QWEN3_ROPE_HALF_DIMENSION_V1)
    } else if dimension < QWEN3_HEAD_DIMENSION_V1 {
        Some(dimension - QWEN3_ROPE_HALF_DIMENSION_V1)
    } else {
        None
    }
}

/// Returns the inverse frequency `theta^(-2*i/128)` for one split-half pair.
#[must_use]
pub fn qwen3_rotary_inverse_frequency_v1(pair_index: u16) -> Option<f64> {
    if pair_index >= QWEN3_ROPE_HALF_DIMENSION_V1 {
        return None;
    }
    let exponent = -2.0 * f64::from(pair_index) / f64::from(QWEN3_HEAD_DIMENSION_V1);
    Some(f64::from(QWEN3_ROPE_THETA_V1).powf(exponent))
}

/// Returns the exact-model angle for an admitted absolute position and pair.
#[must_use]
pub fn qwen3_rotary_angle_v1(position: u32, pair_index: u16) -> Option<f64> {
    if position >= M1_MAX_CONTEXT_TOKENS_V1 {
        return None;
    }
    qwen3_rotary_inverse_frequency_v1(pair_index).map(|frequency| f64::from(position) * frequency)
}

/// CPU RoPE model failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RopeReferenceErrorV1 {
    /// Candidate was structurally invalid.
    Candidate(CandidateErrorV1),
    /// Position extent differs from the token bucket.
    PositionCount,
    /// Query extent differs from `[tokens][query_heads][128]`.
    QueryExtent,
    /// Key extent differs from `[tokens][kv_heads][128]`.
    KeyExtent,
    /// Position is outside the M1 absolute-position domain.
    PositionOutOfBounds {
        /// Token carrying the invalid position.
        token: usize,
    },
    /// Query or key input is NaN or infinite.
    NonFiniteInput {
        /// Flattened input index.
        index: usize,
    },
    /// Host trigonometry produced a non-finite output.
    NonFiniteOutput {
        /// Flattened output index.
        index: usize,
    },
    /// Shape arithmetic overflowed.
    ArithmeticOverflow,
}

impl fmt::Display for RopeReferenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Qwen3 CPU RoPE model failure: {self:?}")
    }
}

impl Error for RopeReferenceErrorV1 {}

impl From<CandidateErrorV1> for RopeReferenceErrorV1 {
    fn from(value: CandidateErrorV1) -> Self {
        Self::Candidate(value)
    }
}

/// Rotated query and key tensors from the CPU model.
#[derive(Clone, Debug, PartialEq)]
pub struct Qwen3RopeOutputV1 {
    /// Flattened `[tokens][query_heads][128]` rotated queries.
    pub query: Vec<f64>,
    /// Flattened `[tokens][kv_heads][128]` rotated keys.
    pub key: Vec<f64>,
}

fn checked_tensor_extent(tokens: u32, heads: u16) -> Option<usize> {
    usize::try_from(tokens)
        .ok()?
        .checked_mul(usize::from(heads))?
        .checked_mul(usize::from(QWEN3_HEAD_DIMENSION_V1))
}

fn validate_rope_inputs(
    candidate: &Qwen3RopeKvCandidateV1,
    positions: &[u32],
    query: &[f64],
    key: &[f64],
) -> Result<(usize, usize), RopeReferenceErrorV1> {
    validate_qwen3_rope_kv_candidate_v1(candidate)?;
    let tokens = candidate.active_tokens.tokens();
    if positions.len() != tokens as usize {
        return Err(RopeReferenceErrorV1::PositionCount);
    }
    let query_extent = checked_tensor_extent(tokens, candidate.geometry.query_heads)
        .ok_or(RopeReferenceErrorV1::ArithmeticOverflow)?;
    let key_extent = checked_tensor_extent(tokens, candidate.geometry.kv_heads)
        .ok_or(RopeReferenceErrorV1::ArithmeticOverflow)?;
    if query.len() != query_extent {
        return Err(RopeReferenceErrorV1::QueryExtent);
    }
    if key.len() != key_extent {
        return Err(RopeReferenceErrorV1::KeyExtent);
    }
    for (token, position) in positions.iter().enumerate() {
        if *position >= M1_MAX_CONTEXT_TOKENS_V1 {
            return Err(RopeReferenceErrorV1::PositionOutOfBounds { token });
        }
    }
    for (index, value) in query.iter().chain(key).enumerate() {
        if !value.is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteInput { index });
        }
    }
    Ok((query_extent, key_extent))
}

fn tensor_base(token: usize, head: usize, heads: usize) -> usize {
    (token * heads + head) * usize::from(QWEN3_HEAD_DIMENSION_V1)
}

fn rotate_reference_dimensions(
    input: &[f64],
    output: &mut [f64],
    position: u32,
    base: usize,
) -> Result<(), RopeReferenceErrorV1> {
    for dimension in 0..QWEN3_HEAD_DIMENSION_V1 {
        let pair =
            qwen3_rotary_pair_v1(dimension).ok_or(RopeReferenceErrorV1::ArithmeticOverflow)?;
        let pair_index = dimension.min(pair);
        let angle = qwen3_rotary_angle_v1(position, pair_index)
            .ok_or(RopeReferenceErrorV1::PositionOutOfBounds { token: 0 })?;
        let (sine, cosine) = angle.sin_cos();
        let index = base + usize::from(dimension);
        let paired_index = base + usize::from(pair);
        output[index] = if dimension < QWEN3_ROPE_HALF_DIMENSION_V1 {
            input[index] * cosine - input[paired_index] * sine
        } else {
            input[index] * cosine + input[paired_index] * sine
        };
        if !output[index].is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteOutput { index });
        }
    }
    Ok(())
}

fn rotate_candidate_pairs(
    input: &[f64],
    output: &mut [f64],
    position: u32,
    base: usize,
) -> Result<(), RopeReferenceErrorV1> {
    for pair in 0..QWEN3_ROPE_HALF_DIMENSION_V1 {
        let angle = qwen3_rotary_angle_v1(position, pair)
            .ok_or(RopeReferenceErrorV1::PositionOutOfBounds { token: 0 })?;
        let (sine, cosine) = angle.sin_cos();
        let lower = base + usize::from(pair);
        let upper = lower + usize::from(QWEN3_ROPE_HALF_DIMENSION_V1);
        output[lower] = input[lower] * cosine - input[upper] * sine;
        output[upper] = input[upper] * cosine + input[lower] * sine;
        if !output[lower].is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteOutput { index: lower });
        }
        if !output[upper].is_finite() {
            return Err(RopeReferenceErrorV1::NonFiniteOutput { index: upper });
        }
    }
    Ok(())
}

type RotateHeadV1 = fn(&[f64], &mut [f64], u32, usize) -> Result<(), RopeReferenceErrorV1>;

fn rotate_tensor(
    input: &[f64],
    output: &mut [f64],
    positions: &[u32],
    heads: u16,
    rotate: RotateHeadV1,
) -> Result<(), RopeReferenceErrorV1> {
    let heads = usize::from(heads);
    for (token, position) in positions.iter().copied().enumerate() {
        for head in 0..heads {
            rotate(input, output, position, tensor_base(token, head, heads))?;
        }
    }
    Ok(())
}

/// Evaluates the dimension-oriented CPU `f64` Qwen3 RoPE reference.
///
/// This function is not an IEEE-754, BF16, FP32, OCML, compiler, or GPU
/// refinement claim.
pub fn qwen3_rope_reference_v1(
    candidate: &Qwen3RopeKvCandidateV1,
    positions: &[u32],
    query: &[f64],
    key: &[f64],
) -> Result<Qwen3RopeOutputV1, RopeReferenceErrorV1> {
    let (query_extent, key_extent) = validate_rope_inputs(candidate, positions, query, key)?;
    let mut output = Qwen3RopeOutputV1 {
        query: vec![0.0; query_extent],
        key: vec![0.0; key_extent],
    };
    rotate_tensor(
        query,
        &mut output.query,
        positions,
        candidate.geometry.query_heads,
        rotate_reference_dimensions,
    )?;
    rotate_tensor(
        key,
        &mut output.key,
        positions,
        candidate.geometry.kv_heads,
        rotate_reference_dimensions,
    )?;
    Ok(output)
}

/// Evaluates an independently indexed pair-oriented CPU `f64` candidate.
///
/// Differential equality with [`qwen3_rope_reference_v1`] validates the two
/// host algorithms only; it grants no executable GPU authority.
pub fn qwen3_rope_pair_candidate_v1(
    candidate: &Qwen3RopeKvCandidateV1,
    positions: &[u32],
    query: &[f64],
    key: &[f64],
) -> Result<Qwen3RopeOutputV1, RopeReferenceErrorV1> {
    let (query_extent, key_extent) = validate_rope_inputs(candidate, positions, query, key)?;
    let mut output = Qwen3RopeOutputV1 {
        query: vec![0.0; query_extent],
        key: vec![0.0; key_extent],
    };
    rotate_tensor(
        query,
        &mut output.query,
        positions,
        candidate.geometry.query_heads,
        rotate_candidate_pairs,
    )?;
    rotate_tensor(
        key,
        &mut output.key,
        positions,
        candidate.geometry.kv_heads,
        rotate_candidate_pairs,
    )?;
    Ok(output)
}
