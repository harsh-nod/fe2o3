#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-width row-softmax V1 host model and proof-facing contract.
//!
//! This crate is deliberately independent of the compiler, descriptor,
//! finalizer, and runtime crates. It supplies a finite executable reference
//! model for one unmasked row of 64 finite `f32` values and a separate output
//! allocation. The accompanying Verus source proves a mathematical model's
//! index, address-disjointness, and conditional arithmetic invariants. It does not prove
//! IEEE-754 arithmetic, an exponential implementation, compiler refinement,
//! or any property of an HSACO artifact.

mod numerical_contract;

pub use numerical_contract::{
    GFX942_OCML_COMPARISON_POLICY_V1, HOST_ORACLE_EXPONENTIAL_V1, MAX_ROW_ELEMENTS_V1,
    RowSoftmaxOracleStateV1, SoftmaxComparisonErrorV1, SoftmaxComparisonPolicyV1,
    SoftmaxContractErrorV1, SoftmaxExponentialV1, compare_row_softmax_v1, row_softmax_oracle_v1,
};

/// Number of conceptual input and output elements in row-softmax V1.
pub const ROW_ELEMENTS_V1: usize = 64;

/// Number of bytes occupied by one conceptual row of `f32` values.
pub const ROW_BYTES_V1: usize = ROW_ELEMENTS_V1 * core::mem::size_of::<f32>();

/// V1 has no mask: every one of the 64 positions participates.
pub const MASK_POLICY_V1: &str = "unmasked: all 64 positions participate";

/// V1 has no empty-row case because its shape is fixed at 64 elements.
pub const EMPTY_ROW_POLICY_V1: &str = "not representable: the fixed row has 64 elements";

/// Input validation failures for the executable host reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceErrorV1 {
    /// At least one input is NaN or infinite.
    NonFiniteInput {
        /// Index of the rejected value.
        index: usize,
    },
    /// A supposedly stable intermediate unexpectedly became non-finite.
    NonFiniteIntermediate,
}

/// Observable finite state from the executable host reference algorithm.
#[derive(Clone, Debug, PartialEq)]
pub struct FiniteAlgorithmStateV1 {
    /// Maximum found by the fixed 64-step scan.
    pub maximum: f32,
    /// Stable exponential weights, one for each input element.
    pub weights: [f32; ROW_ELEMENTS_V1],
    /// Sequential sum of the 64 weights.
    pub denominator: f32,
}

/// Conceptual memory accesses assigned to one logical lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaneAccessV1 {
    /// Logical lane and row index, in `0..64`.
    pub lane: usize,
    /// Input element read by this lane.
    pub input_index: usize,
    /// Private scratch element assigned to this lane in the address model.
    pub scratch_index: usize,
    /// Output element written by this lane after reduction.
    pub output_index: usize,
}

/// Returns the identity ownership map for one active row-softmax lane.
pub const fn lane_access_v1(lane: usize) -> Option<LaneAccessV1> {
    if lane < ROW_ELEMENTS_V1 {
        Some(LaneAccessV1 {
            lane,
            input_index: lane,
            scratch_index: lane,
            output_index: lane,
        })
    } else {
        None
    }
}

/// Computes the finite host reference into a distinct output object.
///
/// This is test and specification scaffolding, not a GPU kernel. It uses the
/// platform Rust `f32::exp`, sequential `f32` addition, and division. The
/// Verus proof does not refine or authenticate these operations.
pub fn row_softmax_reference_v1(
    input: &[f32; ROW_ELEMENTS_V1],
    output: &mut [f32; ROW_ELEMENTS_V1],
) -> Result<FiniteAlgorithmStateV1, ReferenceErrorV1> {
    for (index, value) in input.iter().enumerate() {
        if !value.is_finite() {
            return Err(ReferenceErrorV1::NonFiniteInput { index });
        }
    }

    let mut maximum = input[0];
    for value in &input[1..] {
        maximum = maximum.max(*value);
    }

    let mut weights = [0.0_f32; ROW_ELEMENTS_V1];
    let mut denominator = 0.0_f32;
    for (index, value) in input.iter().enumerate() {
        let weight = (*value - maximum).exp();
        weights[index] = weight;
        denominator += weight;
    }
    if !maximum.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
        return Err(ReferenceErrorV1::NonFiniteIntermediate);
    }

    for index in 0..ROW_ELEMENTS_V1 {
        output[index] = weights[index] / denominator;
    }
    if output.iter().any(|value| !value.is_finite()) {
        return Err(ReferenceErrorV1::NonFiniteIntermediate);
    }

    Ok(FiniteAlgorithmStateV1 {
        maximum,
        weights,
        denominator,
    })
}
