//! Profile-neutral exact CPU oracle for the fixed masked Wave64 contract.

use core::fmt;

use crate::contract::{MAX_EXACT_INPUT_MAGNITUDE_V1, WAVE64_LANES_V1, lane_is_active_v1};

/// One of the three independent output allocations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectiveOutputV1 {
    /// Per-active-lane full masked reduction.
    Reduction,
    /// Per-active-lane prefix including the lane's contribution.
    Inclusive,
    /// Per-active-lane prefix excluding the lane's contribution.
    Exclusive,
}

impl fmt::Display for CollectiveOutputV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Reduction => "reduction",
            Self::Inclusive => "inclusive",
            Self::Exclusive => "exclusive",
        })
    }
}

/// Admission failure detected before any output mutation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OracleErrorV1 {
    /// Input extent was not exactly one Wave64.
    WrongInputLength {
        /// Supplied element count.
        actual: usize,
    },
    /// An output extent was not exactly one Wave64.
    WrongOutputLength {
        /// Rejected output allocation.
        output: CollectiveOutputV1,
        /// Supplied element count.
        actual: usize,
    },
    /// An input was NaN or infinite.
    NonFiniteInput {
        /// Rejected lane.
        lane: usize,
    },
    /// A finite input was not an integer in `[-1024, 1024]`.
    OutsideExactCorpus {
        /// Rejected lane.
        lane: usize,
        /// Exact supplied binary32 value.
        value: f32,
    },
}

impl fmt::Display for OracleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongInputLength { actual } => write!(
                formatter,
                "Wave64 input requires {WAVE64_LANES_V1} elements, got {actual}"
            ),
            Self::WrongOutputLength { output, actual } => write!(
                formatter,
                "{output} output requires {WAVE64_LANES_V1} elements, got {actual}"
            ),
            Self::NonFiniteInput { lane } => {
                write!(formatter, "input lane {lane} is not finite")
            }
            Self::OutsideExactCorpus { lane, value } => write!(
                formatter,
                "input lane {lane} value {value:?} is not an integer in the exact corpus"
            ),
        }
    }
}

impl std::error::Error for OracleErrorV1 {}

/// First exact mismatch between supplied outputs and the CPU oracle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutputMismatchV1 {
    /// Output allocation containing the mismatch.
    pub output: CollectiveOutputV1,
    /// Physical lane containing the mismatch.
    pub lane: usize,
    /// Exact expected binary32 value.
    pub expected: f32,
    /// Exact observed binary32 value.
    pub actual: f32,
}

/// Deterministic state produced by the admitted exact corpus.
#[derive(Clone, Debug, PartialEq)]
pub struct OracleStateV1 {
    /// Explicit logical active-lane mask.
    pub active_mask: u64,
    /// Number of logically active lanes.
    pub active_lanes: u32,
    /// Exact sum of all active contributions.
    pub reduction: f32,
}

fn validate_lengths_and_inputs(
    input: &[f32],
    reduction: &[f32],
    inclusive: &[f32],
    exclusive: &[f32],
) -> Result<(), OracleErrorV1> {
    if input.len() != WAVE64_LANES_V1 {
        return Err(OracleErrorV1::WrongInputLength {
            actual: input.len(),
        });
    }
    for (output, values) in [
        (CollectiveOutputV1::Reduction, reduction),
        (CollectiveOutputV1::Inclusive, inclusive),
        (CollectiveOutputV1::Exclusive, exclusive),
    ] {
        if values.len() != WAVE64_LANES_V1 {
            return Err(OracleErrorV1::WrongOutputLength {
                output,
                actual: values.len(),
            });
        }
    }
    for (lane, value) in input.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(OracleErrorV1::NonFiniteInput { lane });
        }
        if value.fract() != 0.0 || value.abs() > MAX_EXACT_INPUT_MAGNITUDE_V1 {
            return Err(OracleErrorV1::OutsideExactCorpus { lane, value });
        }
    }
    Ok(())
}

fn compute_outputs(input: &[f32], active_mask: u64) -> (f32, [f32; 64], [f32; 64], [f32; 64]) {
    let contribution: [f32; WAVE64_LANES_V1] = core::array::from_fn(|lane| {
        if lane_is_active_v1(active_mask, lane) {
            input[lane]
        } else {
            0.0
        }
    });

    // Match the canonical lowering exactly: six convergent XOR exchanges,
    // with every round reading the previous round's complete lane vector.
    let mut reduction_tree = contribution;
    for distance in [1, 2, 4, 8, 16, 32] {
        let previous = reduction_tree;
        for lane in 0..WAVE64_LANES_V1 {
            reduction_tree[lane] = previous[lane] + previous[lane ^ distance];
        }
    }

    // Match the six ordered bpermute scan rounds in the canonical lowering.
    let mut inclusive_tree = contribution;
    for distance in [1, 2, 4, 8, 16, 32] {
        let previous = inclusive_tree;
        for lane in distance..WAVE64_LANES_V1 {
            inclusive_tree[lane] = previous[lane] + previous[lane - distance];
        }
    }
    let exclusive_tree: [f32; WAVE64_LANES_V1] = core::array::from_fn(|lane| {
        if lane == 0 {
            0.0
        } else {
            inclusive_tree[lane - 1]
        }
    });

    let reductions = core::array::from_fn(|lane| {
        if lane_is_active_v1(active_mask, lane) {
            reduction_tree[lane]
        } else {
            0.0
        }
    });
    let inclusive = core::array::from_fn(|lane| {
        if lane_is_active_v1(active_mask, lane) {
            inclusive_tree[lane]
        } else {
            0.0
        }
    });
    let exclusive = core::array::from_fn(|lane| {
        if lane_is_active_v1(active_mask, lane) {
            exclusive_tree[lane]
        } else {
            0.0
        }
    });
    (reduction_tree[0], reductions, inclusive, exclusive)
}

/// Computes the exact masked reduction and scans into three distinct outputs.
///
/// Every slice extent and every input value is validated before output
/// mutation. Inactive lanes receive positive zero in all outputs. Active lane
/// `i` receives the full reduction, the sum through lane `i`, and the sum
/// strictly before lane `i`, respectively. The empty mask is accepted.
pub fn wave64_collectives_oracle_v1(
    input: &[f32],
    active_mask: u64,
    reduction: &mut [f32],
    inclusive: &mut [f32],
    exclusive: &mut [f32],
) -> Result<OracleStateV1, OracleErrorV1> {
    validate_lengths_and_inputs(input, reduction, inclusive, exclusive)?;
    let (reduction_value, expected_reduction, expected_inclusive, expected_exclusive) =
        compute_outputs(input, active_mask);

    reduction.copy_from_slice(&expected_reduction);
    inclusive.copy_from_slice(&expected_inclusive);
    exclusive.copy_from_slice(&expected_exclusive);

    Ok(OracleStateV1 {
        active_mask,
        active_lanes: active_mask.count_ones(),
        reduction: reduction_value,
    })
}

/// Compares candidate outputs with the exact oracle without mutating them.
pub fn compare_wave64_collectives_v1(
    input: &[f32],
    active_mask: u64,
    reduction: &[f32],
    inclusive: &[f32],
    exclusive: &[f32],
) -> Result<(), OracleErrorV1OrMismatch> {
    validate_lengths_and_inputs(input, reduction, inclusive, exclusive)
        .map_err(OracleErrorV1OrMismatch::Admission)?;
    let (_, expected_reduction, expected_inclusive, expected_exclusive) =
        compute_outputs(input, active_mask);
    for (output, expected, actual) in [
        (
            CollectiveOutputV1::Reduction,
            &expected_reduction[..],
            reduction,
        ),
        (
            CollectiveOutputV1::Inclusive,
            &expected_inclusive[..],
            inclusive,
        ),
        (
            CollectiveOutputV1::Exclusive,
            &expected_exclusive[..],
            exclusive,
        ),
    ] {
        for lane in 0..WAVE64_LANES_V1 {
            if expected[lane].to_bits() != actual[lane].to_bits() {
                return Err(OracleErrorV1OrMismatch::Mismatch(OutputMismatchV1 {
                    output,
                    lane,
                    expected: expected[lane],
                    actual: actual[lane],
                }));
            }
        }
    }
    Ok(())
}

/// Admission error or exact candidate-output mismatch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OracleErrorV1OrMismatch {
    /// Candidate or input was outside the fixed contract.
    Admission(OracleErrorV1),
    /// Candidate differed from the exact expected output.
    Mismatch(OutputMismatchV1),
}

impl fmt::Display for OracleErrorV1OrMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => error.fmt(formatter),
            Self::Mismatch(mismatch) => write!(
                formatter,
                "{} output lane {} expected {:?}, got {:?}",
                mismatch.output, mismatch.lane, mismatch.expected, mismatch.actual
            ),
        }
    }
}

impl std::error::Error for OracleErrorV1OrMismatch {}
