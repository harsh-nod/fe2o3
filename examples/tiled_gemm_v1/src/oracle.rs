//! General scalar arithmetic and validated bitwise evidence for tiled GEMM V1.

use core::fmt;
use std::hint::black_box;

use fe2o3_device::Bf16;

use crate::contract::ShapeV1;
use crate::inputs::BF16_INPUT_PATTERN_V1;

/// Input validation failed before general arithmetic evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArithmeticOracleErrorV1 {
    /// For nonempty output, `A` did not have exactly `M*K` BF16 elements.
    WrongALength {
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// For nonempty output, `B` did not have exactly `K*N` BF16 elements.
    WrongBLength {
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
}

impl fmt::Display for ArithmeticOracleErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongALength { expected, actual } => {
                write!(formatter, "A requires {expected} elements, got {actual}")
            }
            Self::WrongBLength { expected, actual } => {
                write!(formatter, "B requires {expected} elements, got {actual}")
            }
        }
    }
}

impl std::error::Error for ArithmeticOracleErrorV1 {}

/// Operand containing an invalid V1 evidence input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceOperandV1 {
    /// Left-hand matrix `A`.
    A,
    /// Right-hand matrix `B`.
    B,
}

impl fmt::Display for EvidenceOperandV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::A => "A",
            Self::B => "B",
        })
    }
}

/// Input admission failed for the finite pinned V1 evidence corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceInputErrorV1 {
    /// An operand did not have its exact checked row-major extent.
    WrongLength {
        /// Rejected operand.
        operand: EvidenceOperandV1,
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// A NaN encoding was rejected.
    NaNEncoding {
        /// Rejected operand.
        operand: EvidenceOperandV1,
        /// Row-major element index.
        index: usize,
        /// Exact rejected BF16 bits.
        bits: u16,
    },
    /// An infinity encoding was rejected.
    InfinityEncoding {
        /// Rejected operand.
        operand: EvidenceOperandV1,
        /// Row-major element index.
        index: usize,
        /// Exact rejected BF16 bits.
        bits: u16,
    },
    /// A subnormal encoding was rejected.
    SubnormalEncoding {
        /// Rejected operand.
        operand: EvidenceOperandV1,
        /// Row-major element index.
        index: usize,
        /// Exact rejected BF16 bits.
        bits: u16,
    },
    /// A finite, nonsubnormal encoding was not in the pinned generator alphabet.
    OutsidePinnedAlphabet {
        /// Rejected operand.
        operand: EvidenceOperandV1,
        /// Row-major element index.
        index: usize,
        /// Exact rejected BF16 bits.
        bits: u16,
    },
}

impl fmt::Display for EvidenceInputErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength {
                operand,
                expected,
                actual,
            } => write!(
                formatter,
                "evidence operand {operand} requires {expected} elements, got {actual}"
            ),
            Self::NaNEncoding {
                operand,
                index,
                bits,
            } => write!(
                formatter,
                "evidence operand {operand}[{index}] is NaN BF16 encoding 0x{bits:04x}"
            ),
            Self::InfinityEncoding {
                operand,
                index,
                bits,
            } => write!(
                formatter,
                "evidence operand {operand}[{index}] is infinite BF16 encoding 0x{bits:04x}"
            ),
            Self::SubnormalEncoding {
                operand,
                index,
                bits,
            } => write!(
                formatter,
                "evidence operand {operand}[{index}] is subnormal BF16 encoding 0x{bits:04x}"
            ),
            Self::OutsidePinnedAlphabet {
                operand,
                index,
                bits,
            } => write!(
                formatter,
                "evidence operand {operand}[{index}] BF16 encoding 0x{bits:04x} is outside the pinned V1 alphabet"
            ),
        }
    }
}

impl std::error::Error for EvidenceInputErrorV1 {}

/// Unforgeable validated inputs for the finite pinned V1 evidence corpus.
///
/// Safe construction is possible only through [`validate_evidence_inputs_v1`].
///
/// ```compile_fail
/// use fe2o3_tiled_gemm_v1::{ShapeV1, ValidatedEvidenceInputsV1};
/// let shape = ShapeV1::checked(1, 1, 1).unwrap();
/// let _forged = ValidatedEvidenceInputsV1 {
///     shape,
///     a: &[],
///     b: &[],
/// };
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ValidatedEvidenceInputsV1<'a> {
    shape: ShapeV1,
    a: &'a [Bf16],
    b: &'a [Bf16],
}

impl<'a> ValidatedEvidenceInputsV1<'a> {
    /// Returns the checked shape bound to these inputs.
    pub const fn shape(self) -> ShapeV1 {
        self.shape
    }

    /// Returns the validated row-major `A` elements.
    pub const fn a(self) -> &'a [Bf16] {
        self.a
    }

    /// Returns the validated row-major `B` elements.
    pub const fn b(self) -> &'a [Bf16] {
        self.b
    }
}

fn validate_evidence_operand_v1(
    operand: EvidenceOperandV1,
    expected: usize,
    values: &[Bf16],
) -> Result<(), EvidenceInputErrorV1> {
    if values.len() != expected {
        return Err(EvidenceInputErrorV1::WrongLength {
            operand,
            expected,
            actual: values.len(),
        });
    }

    for (index, value) in values.iter().copied().enumerate() {
        let bits = value.to_bits();
        if value.is_nan() {
            return Err(EvidenceInputErrorV1::NaNEncoding {
                operand,
                index,
                bits,
            });
        }
        if value.is_infinite() {
            return Err(EvidenceInputErrorV1::InfinityEncoding {
                operand,
                index,
                bits,
            });
        }
        if value.is_subnormal() {
            return Err(EvidenceInputErrorV1::SubnormalEncoding {
                operand,
                index,
                bits,
            });
        }
        if !BF16_INPUT_PATTERN_V1.contains(&bits) {
            return Err(EvidenceInputErrorV1::OutsidePinnedAlphabet {
                operand,
                index,
                bits,
            });
        }
    }
    Ok(())
}

/// Validates exact lengths and every BF16 encoding before evidence evaluation.
///
/// Only values in [`BF16_INPUT_PATTERN_V1`] are admitted. NaNs, infinities,
/// subnormals, negative zero, and all other encodings fail closed. Empty output
/// requires exact zero-length operands even though the general arithmetic
/// oracle permits unused slices.
pub fn validate_evidence_inputs_v1<'a>(
    shape: ShapeV1,
    a: &'a [Bf16],
    b: &'a [Bf16],
) -> Result<ValidatedEvidenceInputsV1<'a>, EvidenceInputErrorV1> {
    validate_evidence_operand_v1(EvidenceOperandV1::A, shape.a_elements(), a)?;
    validate_evidence_operand_v1(EvidenceOperandV1::B, shape.b_elements(), b)?;
    Ok(ValidatedEvidenceInputsV1 { shape, a, b })
}

#[inline(never)]
fn fp32_product_v1(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

#[inline(never)]
fn fp32_sum_v1(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

/// Evaluates the general V1 arithmetic recurrence for `C=A*B`.
///
/// Each BF16 value widens exactly to FP32. For every output `(row, column)`,
/// accumulation starts at FP32 positive zero and visits `depth=0..K` in
/// increasing order. Multiplication and addition are separate FP32 operations;
/// fused contraction is not part of this oracle.
///
/// This function intentionally accepts every BF16 encoding and is not the V1
/// bitwise-evidence admission path. Use [`validate_evidence_inputs_v1`] followed
/// by [`tiled_gemm_evidence_oracle_v1`] before making a finite-corpus bitwise
/// reproducibility claim. Empty output returns an empty vector without
/// inspecting operand elements or requiring operand lengths.
pub fn tiled_gemm_arithmetic_oracle_v1(
    shape: ShapeV1,
    a: &[Bf16],
    b: &[Bf16],
) -> Result<Vec<f32>, ArithmeticOracleErrorV1> {
    if shape.is_empty_output() {
        return Ok(Vec::new());
    }
    if a.len() != shape.a_elements() {
        return Err(ArithmeticOracleErrorV1::WrongALength {
            expected: shape.a_elements(),
            actual: a.len(),
        });
    }
    if b.len() != shape.b_elements() {
        return Err(ArithmeticOracleErrorV1::WrongBLength {
            expected: shape.b_elements(),
            actual: b.len(),
        });
    }

    let mut output = vec![f32::from_bits(0); shape.c_elements()];
    for row in 0..shape.m() {
        for column in 0..shape.n() {
            let mut accumulator = f32::from_bits(0);
            for depth in 0..shape.k() {
                let left = a[shape.a_index(row, depth).expect("bounded A coordinate")].to_f32();
                let right = b[shape.b_index(depth, column).expect("bounded B coordinate")].to_f32();
                let product = fp32_product_v1(left, right);
                accumulator = fp32_sum_v1(accumulator, product);
            }
            let index = shape
                .c_index(row, column)
                .expect("bounded output coordinate");
            output[index] = accumulator;
        }
    }
    Ok(output)
}

/// Evaluates bitwise host evidence for validated finite-corpus inputs.
///
/// The result is reproducible for the pinned BF16 evidence alphabet and exact
/// increasing-`k` host recurrence. It does not claim MFMA numerical equivalence,
/// compiler-to-machine refinement, or GPU execution evidence.
pub fn tiled_gemm_evidence_oracle_v1(inputs: ValidatedEvidenceInputsV1<'_>) -> Vec<f32> {
    tiled_gemm_arithmetic_oracle_v1(inputs.shape, inputs.a, inputs.b)
        .expect("validated evidence lengths must satisfy the arithmetic oracle")
}
