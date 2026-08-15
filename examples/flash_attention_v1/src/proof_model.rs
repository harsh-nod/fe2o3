//! Executable proof-facing model for the bounded online recurrence and memory map.
//!
//! These contracts support review, mutation tests, and a future Verus proof.
//! They are ordinary Rust computations over `f64` and `usize`; they do not
//! establish source-to-model, FP32-to-real, compiler, or machine refinement.

use crate::contract::{
    FLASH_ATTENTION_HEAD_DIMENSION_V1, FLASH_ATTENTION_OUTPUT_ELEMENTS_V1,
    FLASH_ATTENTION_SEQUENCE_LENGTH_V1, FLASH_ATTENTION_WAVE_LANES_V1, lane_outputs_v1,
};

/// Precise assurance level of this Phase A model.
pub const MODEL_ASSURANCE_V1: &str =
    "executable bounded model and hostile mutation tests; not a machine-checked refinement proof";
/// Whether a machine-checked source-to-model refinement proof exists.
pub const SOURCE_MODEL_REFINEMENT_PROVED_V1: bool = false;

/// State after one non-empty prefix of an online softmax/value recurrence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnlineStateV1 {
    /// Number of key/value pairs consumed.
    pub consumed_keys: usize,
    /// Maximum score in the consumed prefix.
    pub maximum: f64,
    /// Sum of stable exponential weights relative to `maximum`.
    pub denominator: f64,
    /// Stable weighted sum for one output column.
    pub numerator: f64,
}

/// Invalid input to the bounded proof-facing online model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineModelErrorV1 {
    /// Score and value extents differ.
    LengthMismatch,
    /// The recurrence requires at least one causal key.
    EmptyPrefix,
    /// A prefix exceeded the fixed sequence length of eight.
    PrefixTooLong,
    /// A score or value was NaN or infinite.
    NonFiniteInput {
        /// Invalid prefix index.
        index: usize,
    },
    /// An online recurrence intermediate was invalid.
    NonFiniteIntermediate {
        /// Prefix index whose update failed.
        index: usize,
    },
}

/// Specific invariant violated by a supplied online state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OnlineInvariantViolationV1 {
    /// `consumed_keys` does not equal the supplied prefix extent.
    PrefixLength,
    /// The state maximum is not the maximum of the supplied prefix.
    Maximum,
    /// The stable denominator is not positive, finite, or equal to its direct form.
    Denominator,
    /// The stable numerator is not finite or equal to its direct form.
    Numerator,
}

fn validate_online_inputs(scores: &[f64], values: &[f64]) -> Result<(), OnlineModelErrorV1> {
    if scores.len() != values.len() {
        return Err(OnlineModelErrorV1::LengthMismatch);
    }
    if scores.is_empty() {
        return Err(OnlineModelErrorV1::EmptyPrefix);
    }
    if scores.len() > FLASH_ATTENTION_SEQUENCE_LENGTH_V1 {
        return Err(OnlineModelErrorV1::PrefixTooLong);
    }
    for index in 0..scores.len() {
        if !scores[index].is_finite() || !values[index].is_finite() {
            return Err(OnlineModelErrorV1::NonFiniteInput { index });
        }
    }
    Ok(())
}

/// Builds every prefix state using online max/sum rescaling.
pub fn online_trace_v1(
    scores: &[f64],
    values: &[f64],
) -> Result<Vec<OnlineStateV1>, OnlineModelErrorV1> {
    validate_online_inputs(scores, values)?;
    let mut trace = Vec::with_capacity(scores.len());
    let mut state = OnlineStateV1 {
        consumed_keys: 1,
        maximum: scores[0],
        denominator: 1.0,
        numerator: values[0],
    };
    trace.push(state);

    for index in 1..scores.len() {
        let next_maximum = state.maximum.max(scores[index]);
        let previous_weight = (state.maximum - next_maximum).exp();
        let current_weight = (scores[index] - next_maximum).exp();
        state = OnlineStateV1 {
            consumed_keys: index + 1,
            maximum: next_maximum,
            denominator: state.denominator * previous_weight + current_weight,
            numerator: state.numerator * previous_weight + values[index] * current_weight,
        };
        if !state.maximum.is_finite()
            || !state.denominator.is_finite()
            || state.denominator <= 0.0
            || !state.numerator.is_finite()
        {
            return Err(OnlineModelErrorV1::NonFiniteIntermediate { index });
        }
        trace.push(state);
    }
    Ok(trace)
}

fn close(left: f64, right: f64) -> bool {
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= 32.0 * f64::EPSILON * scale
}

/// Checks one online state against a direct stable recomputation of its prefix.
pub fn validate_online_state_v1(
    scores: &[f64],
    values: &[f64],
    state: OnlineStateV1,
) -> Result<(), OnlineInvariantViolationV1> {
    if scores.len() != values.len()
        || scores.is_empty()
        || state.consumed_keys != scores.len()
        || scores.len() > FLASH_ATTENTION_SEQUENCE_LENGTH_V1
    {
        return Err(OnlineInvariantViolationV1::PrefixLength);
    }

    let mut maximum = scores[0];
    for score in &scores[1..] {
        maximum = maximum.max(*score);
    }
    if state.maximum.to_bits() != maximum.to_bits() {
        return Err(OnlineInvariantViolationV1::Maximum);
    }

    let mut denominator = 0.0_f64;
    let mut numerator = 0.0_f64;
    for index in 0..scores.len() {
        let weight = (scores[index] - maximum).exp();
        denominator += weight;
        numerator += weight * values[index];
    }
    if !state.denominator.is_finite()
        || state.denominator <= 0.0
        || !close(state.denominator, denominator)
    {
        return Err(OnlineInvariantViolationV1::Denominator);
    }
    if !state.numerator.is_finite() || !close(state.numerator, numerator) {
        return Err(OnlineInvariantViolationV1::Numerator);
    }
    Ok(())
}

/// One complete bounded memory-access coordinate for a lane/key/feature/slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessCoordinateV1 {
    /// Physical Wave64 lane.
    pub lane: usize,
    /// Query row owned by the lane.
    pub query_row: usize,
    /// Causally admitted key/value row.
    pub key_row: usize,
    /// Q/K feature read for the dot product.
    pub feature: usize,
    /// Which one of the lane's two adjacent output columns is selected.
    pub output_slot: usize,
    /// Contiguous Q read index.
    pub q_index: usize,
    /// Contiguous K read index.
    pub k_index: usize,
    /// Contiguous V read index.
    pub v_index: usize,
    /// Contiguous O write index.
    pub output_index: usize,
}

/// Returns an access only for an in-profile lane, causal key, feature, and slot.
pub const fn access_coordinate_v1(
    lane: usize,
    key_row: usize,
    feature: usize,
    output_slot: usize,
) -> Option<AccessCoordinateV1> {
    let Some(outputs) = lane_outputs_v1(lane) else {
        return None;
    };
    if feature >= FLASH_ATTENTION_HEAD_DIMENSION_V1 || output_slot >= outputs.len() {
        return None;
    }
    let output_index = outputs[output_slot];
    let query_row = output_index / FLASH_ATTENTION_HEAD_DIMENSION_V1;
    if key_row > query_row || key_row >= FLASH_ATTENTION_SEQUENCE_LENGTH_V1 {
        return None;
    }
    let output_column = output_index % FLASH_ATTENTION_HEAD_DIMENSION_V1;
    Some(AccessCoordinateV1 {
        lane,
        query_row,
        key_row,
        feature,
        output_slot,
        q_index: query_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature,
        k_index: key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + feature,
        v_index: key_row * FLASH_ATTENTION_HEAD_DIMENSION_V1 + output_column,
        output_index,
    })
}

/// Ownership-map validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipViolationV1 {
    /// The map does not contain exactly 64 lane pairs.
    WrongLaneCount,
    /// An output index is outside `0..128`.
    OutOfBounds {
        /// Lane carrying the invalid output index.
        lane: usize,
        /// Invalid output index.
        index: usize,
    },
    /// Two lane/slot entries own the same output index.
    DuplicateWriter {
        /// Duplicated output index.
        index: usize,
    },
    /// An output index has no writer.
    MissingWriter {
        /// Unowned output index.
        index: usize,
    },
}

/// Returns the exact adjacent-pair Wave64 ownership map.
pub fn exact_ownership_map_v1() -> [[usize; 2]; FLASH_ATTENTION_WAVE_LANES_V1] {
    std::array::from_fn(|lane| lane_outputs_v1(lane).expect("lane is in Wave64"))
}

/// Checks total, in-bounds, single-writer output ownership.
pub fn validate_output_ownership_v1(ownership: &[[usize; 2]]) -> Result<(), OwnershipViolationV1> {
    if ownership.len() != FLASH_ATTENTION_WAVE_LANES_V1 {
        return Err(OwnershipViolationV1::WrongLaneCount);
    }
    let mut owner = [usize::MAX; FLASH_ATTENTION_OUTPUT_ELEMENTS_V1];
    for (lane, pair) in ownership.iter().enumerate() {
        for index in pair {
            if *index >= FLASH_ATTENTION_OUTPUT_ELEMENTS_V1 {
                return Err(OwnershipViolationV1::OutOfBounds {
                    lane,
                    index: *index,
                });
            }
            if owner[*index] != usize::MAX {
                return Err(OwnershipViolationV1::DuplicateWriter { index: *index });
            }
            owner[*index] = lane;
        }
    }
    for (index, lane) in owner.into_iter().enumerate() {
        if lane == usize::MAX {
            return Err(OwnershipViolationV1::MissingWriter { index });
        }
    }
    Ok(())
}
