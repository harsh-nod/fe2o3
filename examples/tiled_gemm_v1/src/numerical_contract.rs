//! Profile-neutral BF16/F32 LDS GEMM numerical contract and host oracle.
//!
//! # Source semantics
//!
//! BF16 operands widen exactly by placing their 16 bits in the high half of an
//! IEEE binary32 encoding. For each logical output `(row, column)`, the source
//! oracle starts from positive FP32 zero and visits `depth = 0..K` in increasing
//! order. Every product and sum is a separate round-to-nearest-even FP32
//! operation; fused contraction is deliberately excluded. The final value is
//! evaluated as two separate products followed by one addition:
//! `alpha * accumulator + beta * C[row, column]`.
//!
//! This executable recurrence is a finite approximation of the mathematical
//! GEMM specification over the exactly widened BF16 values. A proof may reason
//! over real-number summation independently of this host evaluation order. In
//! particular, this module does not claim that an MFMA instruction uses the
//! same reduction tree as the scalar recurrence.
//!
//! # Finite hardware policy
//!
//! [`build_hardware_expectation`] admits only finite normal values and signed
//! zeros. BF16 and FP32 subnormals, NaNs, infinities, and any nonnormal nonzero
//! intermediate are rejected. Overflow is therefore unsupported rather than
//! compared after the fact. Signed zero is preserved by exact-bit comparison;
//! a bounded comparison may accept opposite zero signs when its one-ULP and
//! absolute-error bounds permit that result.
//!
//! Exact-bit policy is intended for pinned dyadic corpora whose operations are
//! known to be exactly representable. Bounded policy requires both its
//! absolute/relative envelope and ULP limit to pass. It exists for finite MFMA
//! observations whose legal reduction order differs from this scalar oracle.
//! Neither policy turns a finite test corpus into a general numerical proof.

use core::fmt;
use std::hint::black_box;

/// Stable identity of these source arithmetic semantics.
pub const SOURCE_SEMANTICS_ID: &str = "fe2o3-lds-gemm-bf16-f32-source-v1";

/// Stable identity of the strict finite hardware admission policy.
pub const FINITE_HARDWARE_POLICY_ID: &str = "fe2o3-lds-gemm-finite-normal-or-zero-v1";

/// Matrix or scalar input named by a diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericalOperand {
    /// Left-hand BF16 matrix `A`.
    A,
    /// Right-hand BF16 matrix `B`.
    B,
    /// Initial FP32 matrix `C`.
    C,
    /// FP32 product coefficient.
    Alpha,
    /// FP32 initial-output coefficient.
    Beta,
}

impl fmt::Display for NumericalOperand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::Alpha => "alpha",
            Self::Beta => "beta",
        })
    }
}

/// Checked row-major GEMM dimensions, strides, and accessed storage extents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmSpec {
    m: usize,
    n: usize,
    k: usize,
    a_stride: usize,
    b_stride: usize,
    c_stride: usize,
    a_len: usize,
    b_len: usize,
    c_len: usize,
    output_len: usize,
}

/// A GEMM shape or row stride cannot describe bounded host storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GemmSpecError {
    /// A nonempty row is shorter than its logical column count.
    StrideTooSmall {
        /// Matrix whose stride is invalid.
        operand: NumericalOperand,
        /// Minimum valid row stride.
        minimum: usize,
        /// Rejected row stride.
        actual: usize,
    },
    /// An accessed storage extent overflows `usize`.
    ExtentOverflow {
        /// Matrix whose extent overflowed.
        operand: NumericalOperand,
    },
    /// The compact logical output extent overflows `usize`.
    OutputExtentOverflow,
}

impl fmt::Display for GemmSpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrideTooSmall {
                operand,
                minimum,
                actual,
            } => write!(
                formatter,
                "{operand} row stride requires at least {minimum} elements, got {actual}"
            ),
            Self::ExtentOverflow { operand } => {
                write!(
                    formatter,
                    "{operand} accessed storage extent overflows usize"
                )
            }
            Self::OutputExtentOverflow => {
                formatter.write_str("compact logical output extent overflows usize")
            }
        }
    }
}

impl std::error::Error for GemmSpecError {}

impl GemmSpec {
    /// Checks dimensions and independent row strides for `A[M,K]`, `B[K,N]`,
    /// and `C[M,N]`.
    ///
    /// Empty matrices have zero accessed storage extent. A nonempty matrix uses
    /// exactly `(rows - 1) * stride + columns` elements, including padding
    /// between rows but not after the final logical row.
    pub fn checked(
        m: usize,
        n: usize,
        k: usize,
        a_stride: usize,
        b_stride: usize,
        c_stride: usize,
    ) -> Result<Self, GemmSpecError> {
        fn checked_extent(
            operand: NumericalOperand,
            rows: usize,
            columns: usize,
            stride: usize,
        ) -> Result<usize, GemmSpecError> {
            if rows == 0 || columns == 0 {
                return Ok(0);
            }
            if stride < columns {
                return Err(GemmSpecError::StrideTooSmall {
                    operand,
                    minimum: columns,
                    actual: stride,
                });
            }
            (rows - 1)
                .checked_mul(stride)
                .and_then(|prefix| prefix.checked_add(columns))
                .ok_or(GemmSpecError::ExtentOverflow { operand })
        }

        let a_len = checked_extent(NumericalOperand::A, m, k, a_stride)?;
        let b_len = checked_extent(NumericalOperand::B, k, n, b_stride)?;
        let c_len = checked_extent(NumericalOperand::C, m, n, c_stride)?;
        let output_len = m
            .checked_mul(n)
            .ok_or(GemmSpecError::OutputExtentOverflow)?;
        Ok(Self {
            m,
            n,
            k,
            a_stride,
            b_stride,
            c_stride,
            a_len,
            b_len,
            c_len,
            output_len,
        })
    }

    /// Returns `[M, N, K]`.
    pub const fn dimensions(self) -> [usize; 3] {
        [self.m, self.n, self.k]
    }

    /// Returns `[A row stride, B row stride, C row stride]` in elements.
    pub const fn strides(self) -> [usize; 3] {
        [self.a_stride, self.b_stride, self.c_stride]
    }

    /// Returns the exact accessed `A` storage length.
    pub const fn a_len(self) -> usize {
        self.a_len
    }

    /// Returns the exact accessed `B` storage length.
    pub const fn b_len(self) -> usize {
        self.b_len
    }

    /// Returns the exact accessed initial `C` storage length.
    pub const fn c_len(self) -> usize {
        self.c_len
    }

    /// Returns the compact logical output length `M*N`.
    pub const fn output_len(self) -> usize {
        self.output_len
    }

    const fn a_index(self, row: usize, depth: usize) -> usize {
        row * self.a_stride + depth
    }

    const fn b_index(self, depth: usize, column: usize) -> usize {
        depth * self.b_stride + column
    }

    const fn c_index(self, row: usize, column: usize) -> usize {
        row * self.c_stride + column
    }
}

/// Borrowed inputs for `C_out = alpha * A*B + beta * C_in`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GemmInputs<'a> {
    /// Exact BF16 encodings for row-major `A` storage.
    pub a_bits: &'a [u16],
    /// Exact BF16 encodings for row-major `B` storage.
    pub b_bits: &'a [u16],
    /// FP32 values for row-major initial `C` storage.
    pub c: &'a [f32],
    /// Product coefficient.
    pub alpha: f32,
    /// Initial-output coefficient.
    pub beta: f32,
}

/// Source evaluation failed before arithmetic began.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEvaluationError {
    /// A storage slice does not have the exact checked accessed extent.
    WrongLength {
        /// Matrix whose storage length is wrong.
        operand: NumericalOperand,
        /// Required accessed extent.
        expected: usize,
        /// Supplied slice length.
        actual: usize,
    },
}

impl fmt::Display for SourceEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength {
                operand,
                expected,
                actual,
            } => write!(
                formatter,
                "{operand} requires exactly {expected} accessed elements, got {actual}"
            ),
        }
    }
}

impl std::error::Error for SourceEvaluationError {}

fn validate_lengths(spec: GemmSpec, inputs: GemmInputs<'_>) -> Result<(), SourceEvaluationError> {
    for (operand, expected, actual) in [
        (NumericalOperand::A, spec.a_len, inputs.a_bits.len()),
        (NumericalOperand::B, spec.b_len, inputs.b_bits.len()),
        (NumericalOperand::C, spec.c_len, inputs.c.len()),
    ] {
        if expected != actual {
            return Err(SourceEvaluationError::WrongLength {
                operand,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

/// Widens an exact BF16 encoding to binary32 without rounding.
pub const fn widen_bf16_bits(bits: u16) -> f32 {
    f32::from_bits((bits as u32) << 16)
}

#[inline(never)]
fn fp32_product(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

#[inline(never)]
fn fp32_sum(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

fn evaluate_unchecked(spec: GemmSpec, inputs: GemmInputs<'_>) -> Vec<f32> {
    let mut output = Vec::with_capacity(spec.output_len);
    for row in 0..spec.m {
        for column in 0..spec.n {
            let mut accumulator = f32::from_bits(0);
            for depth in 0..spec.k {
                let left = widen_bf16_bits(inputs.a_bits[spec.a_index(row, depth)]);
                let right = widen_bf16_bits(inputs.b_bits[spec.b_index(depth, column)]);
                accumulator = fp32_sum(accumulator, fp32_product(left, right));
            }
            let product = fp32_product(inputs.alpha, accumulator);
            let initial = fp32_product(inputs.beta, inputs.c[spec.c_index(row, column)]);
            output.push(fp32_sum(product, initial));
        }
    }
    output
}

/// Evaluates the deterministic scalar source recurrence for all IEEE inputs.
///
/// This function preserves IEEE exceptional-value behavior and applies no
/// hardware admissibility policy. Use [`build_hardware_expectation`] before a
/// finite hardware comparison claim.
pub fn evaluate_source(
    spec: GemmSpec,
    inputs: GemmInputs<'_>,
) -> Result<Vec<f32>, SourceEvaluationError> {
    validate_lengths(spec, inputs)?;
    Ok(evaluate_unchecked(spec, inputs))
}

/// IEEE class reported by strict finite hardware admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedValueClass {
    /// A nonzero subnormal value.
    Subnormal,
    /// Positive or negative infinity.
    Infinity,
    /// A quiet or signaling NaN encoding.
    NaN,
}

impl fmt::Display for UnsupportedValueClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Subnormal => "subnormal",
            Self::Infinity => "infinity",
            Self::NaN => "NaN",
        })
    }
}

/// Arithmetic stage that produced an unsupported intermediate value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvaluationStage {
    /// One widened BF16 product.
    Product,
    /// The increasing-depth FP32 accumulation.
    Accumulation,
    /// Multiplication by `alpha`.
    AlphaScale,
    /// Multiplication of initial `C` by `beta`.
    BetaScale,
    /// Addition of the two scaled terms.
    Output,
}

impl fmt::Display for EvaluationStage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Product => "BF16 product",
            Self::Accumulation => "FP32 accumulation",
            Self::AlphaScale => "alpha scaling",
            Self::BetaScale => "beta scaling",
            Self::Output => "output addition",
        })
    }
}

/// Strict finite-hardware admission or evaluation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HardwareExpectationError {
    /// Input storage failed the source shape contract.
    Source(SourceEvaluationError),
    /// A logical BF16 matrix element is outside finite normal-or-zero policy.
    UnsupportedBf16 {
        /// Rejected matrix.
        operand: NumericalOperand,
        /// Index in the supplied strided storage.
        index: usize,
        /// Exact rejected BF16 encoding.
        bits: u16,
        /// Rejected IEEE class.
        class: UnsupportedValueClass,
    },
    /// A logical FP32 input is outside finite normal-or-zero policy.
    UnsupportedF32 {
        /// Rejected matrix or coefficient.
        operand: NumericalOperand,
        /// Index in strided storage, or zero for a scalar coefficient.
        index: usize,
        /// Exact rejected FP32 encoding.
        bits: u32,
        /// Rejected IEEE class.
        class: UnsupportedValueClass,
    },
    /// A scalar recurrence step produced a subnormal, infinity, or NaN.
    UnsupportedIntermediate {
        /// Logical output row.
        row: usize,
        /// Logical output column.
        column: usize,
        /// Reduction index for product/accumulation stages.
        depth: Option<usize>,
        /// Arithmetic stage that failed.
        stage: EvaluationStage,
        /// Exact rejected FP32 encoding.
        bits: u32,
        /// Rejected IEEE class.
        class: UnsupportedValueClass,
    },
}

impl fmt::Display for HardwareExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::UnsupportedBf16 {
                operand,
                index,
                bits,
                class,
            } => write!(
                formatter,
                "{operand}[{index}] BF16 encoding 0x{bits:04x} is unsupported {class} input"
            ),
            Self::UnsupportedF32 {
                operand,
                index,
                bits,
                class,
            } => write!(
                formatter,
                "{operand}[{index}] FP32 encoding 0x{bits:08x} is unsupported {class} input"
            ),
            Self::UnsupportedIntermediate {
                row,
                column,
                depth,
                stage,
                bits,
                class,
            } => {
                write!(
                    formatter,
                    "output ({row}, {column}) {stage} produced unsupported {class} 0x{bits:08x}"
                )?;
                if let Some(depth) = depth {
                    write!(formatter, " at depth {depth}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for HardwareExpectationError {}

impl From<SourceEvaluationError> for HardwareExpectationError {
    fn from(error: SourceEvaluationError) -> Self {
        Self::Source(error)
    }
}

fn bf16_unsupported_class(bits: u16) -> Option<UnsupportedValueClass> {
    let exponent = bits & 0x7f80;
    let fraction = bits & 0x007f;
    if exponent == 0x7f80 {
        Some(if fraction == 0 {
            UnsupportedValueClass::Infinity
        } else {
            UnsupportedValueClass::NaN
        })
    } else if exponent == 0 && fraction != 0 {
        Some(UnsupportedValueClass::Subnormal)
    } else {
        None
    }
}

fn f32_unsupported_class(value: f32) -> Option<UnsupportedValueClass> {
    if value.is_nan() {
        Some(UnsupportedValueClass::NaN)
    } else if value.is_infinite() {
        Some(UnsupportedValueClass::Infinity)
    } else if value.is_subnormal() {
        Some(UnsupportedValueClass::Subnormal)
    } else {
        None
    }
}

fn validate_bf16_logical(
    spec: GemmSpec,
    operand: NumericalOperand,
    bits: &[u16],
) -> Result<(), HardwareExpectationError> {
    let (rows, columns, stride) = match operand {
        NumericalOperand::A => (spec.m, spec.k, spec.a_stride),
        NumericalOperand::B => (spec.k, spec.n, spec.b_stride),
        _ => unreachable!("BF16 validation is limited to A and B"),
    };
    for row in 0..rows {
        for column in 0..columns {
            let index = row * stride + column;
            if let Some(class) = bf16_unsupported_class(bits[index]) {
                return Err(HardwareExpectationError::UnsupportedBf16 {
                    operand,
                    index,
                    bits: bits[index],
                    class,
                });
            }
        }
    }
    Ok(())
}

fn validate_f32_input(
    operand: NumericalOperand,
    index: usize,
    value: f32,
) -> Result<(), HardwareExpectationError> {
    if let Some(class) = f32_unsupported_class(value) {
        return Err(HardwareExpectationError::UnsupportedF32 {
            operand,
            index,
            bits: value.to_bits(),
            class,
        });
    }
    Ok(())
}

fn check_intermediate(
    value: f32,
    row: usize,
    column: usize,
    depth: Option<usize>,
    stage: EvaluationStage,
) -> Result<f32, HardwareExpectationError> {
    if let Some(class) = f32_unsupported_class(value) {
        return Err(HardwareExpectationError::UnsupportedIntermediate {
            row,
            column,
            depth,
            stage,
            bits: value.to_bits(),
            class,
        });
    }
    Ok(value)
}

fn evaluate_finite(
    spec: GemmSpec,
    inputs: GemmInputs<'_>,
) -> Result<Vec<f32>, HardwareExpectationError> {
    let mut output = Vec::with_capacity(spec.output_len);
    for row in 0..spec.m {
        for column in 0..spec.n {
            let mut accumulator = f32::from_bits(0);
            for depth in 0..spec.k {
                let left = widen_bf16_bits(inputs.a_bits[spec.a_index(row, depth)]);
                let right = widen_bf16_bits(inputs.b_bits[spec.b_index(depth, column)]);
                let product = check_intermediate(
                    fp32_product(left, right),
                    row,
                    column,
                    Some(depth),
                    EvaluationStage::Product,
                )?;
                accumulator = check_intermediate(
                    fp32_sum(accumulator, product),
                    row,
                    column,
                    Some(depth),
                    EvaluationStage::Accumulation,
                )?;
            }
            let product = check_intermediate(
                fp32_product(inputs.alpha, accumulator),
                row,
                column,
                None,
                EvaluationStage::AlphaScale,
            )?;
            let initial = check_intermediate(
                fp32_product(inputs.beta, inputs.c[spec.c_index(row, column)]),
                row,
                column,
                None,
                EvaluationStage::BetaScale,
            )?;
            output.push(check_intermediate(
                fp32_sum(product, initial),
                row,
                column,
                None,
                EvaluationStage::Output,
            )?);
        }
    }
    Ok(output)
}

/// Policy used to compare finite observed hardware values with the host oracle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComparisonPolicy {
    /// Require identical FP32 encodings, including signed zero.
    ExactBits,
    /// Require both an absolute/relative envelope and a ULP bound.
    Bounded {
        /// Nonnegative absolute-error floor.
        max_abs: f32,
        /// Nonnegative relative-error coefficient.
        max_rel: f32,
        /// Positive maximum distance in ordered finite FP32 encodings.
        max_ulps: u32,
    },
}

/// A bounded comparison policy is malformed or vacuous.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonPolicyError {
    /// Absolute tolerance is negative, subnormal, NaN, or infinite.
    InvalidAbsoluteTolerance(u32),
    /// Relative tolerance is negative, subnormal, NaN, or infinite.
    InvalidRelativeTolerance(u32),
    /// Both numeric tolerances are zero.
    ZeroNumericTolerance,
    /// ULP tolerance is zero.
    ZeroUlpTolerance,
}

impl fmt::Display for ComparisonPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAbsoluteTolerance(bits) => write!(
                formatter,
                "absolute tolerance 0x{bits:08x} must be finite, normal-or-zero, and nonnegative"
            ),
            Self::InvalidRelativeTolerance(bits) => write!(
                formatter,
                "relative tolerance 0x{bits:08x} must be finite, normal-or-zero, and nonnegative"
            ),
            Self::ZeroNumericTolerance => formatter
                .write_str("bounded comparison requires a nonzero absolute or relative tolerance"),
            Self::ZeroUlpTolerance => {
                formatter.write_str("bounded comparison requires a positive ULP tolerance")
            }
        }
    }
}

impl std::error::Error for ComparisonPolicyError {}

impl ComparisonPolicy {
    /// Creates a validated bounded policy.
    pub fn bounded(
        max_abs: f32,
        max_rel: f32,
        max_ulps: u32,
    ) -> Result<Self, ComparisonPolicyError> {
        fn valid_nonnegative_normal_or_zero(value: f32) -> bool {
            !value.is_sign_negative()
                && !value.is_nan()
                && !value.is_infinite()
                && !value.is_subnormal()
        }

        if !valid_nonnegative_normal_or_zero(max_abs) {
            return Err(ComparisonPolicyError::InvalidAbsoluteTolerance(
                max_abs.to_bits(),
            ));
        }
        if !valid_nonnegative_normal_or_zero(max_rel) {
            return Err(ComparisonPolicyError::InvalidRelativeTolerance(
                max_rel.to_bits(),
            ));
        }
        if max_abs == 0.0 && max_rel == 0.0 {
            return Err(ComparisonPolicyError::ZeroNumericTolerance);
        }
        if max_ulps == 0 {
            return Err(ComparisonPolicyError::ZeroUlpTolerance);
        }
        Ok(Self::Bounded {
            max_abs,
            max_rel,
            max_ulps,
        })
    }
}

/// A sealed expected output with its comparison policy and contract identity.
///
/// Safe callers can obtain this value only through
/// [`build_hardware_expectation`]. The comparison policy is retained inside the
/// expectation, so observation code cannot silently replace it with a weaker
/// policy.
#[derive(Clone, Debug, PartialEq)]
pub struct HardwareExpectation {
    spec: GemmSpec,
    expected: Vec<f32>,
    policy: ComparisonPolicy,
    integrity: u64,
}

impl HardwareExpectation {
    /// Returns the bound GEMM shape and strides.
    pub const fn spec(&self) -> GemmSpec {
        self.spec
    }

    /// Returns compact logical expected outputs in row-major order.
    pub fn expected(&self) -> &[f32] {
        &self.expected
    }

    /// Returns the policy sealed into this expectation.
    pub const fn policy(&self) -> ComparisonPolicy {
        self.policy
    }
}

fn integrity_hash(spec: GemmSpec, expected: &[f32], policy: ComparisonPolicy) -> u64 {
    fn append(mut hash: u64, bytes: &[u8]) -> u64 {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    let mut hash = 0xcbf2_9ce4_8422_2325;
    hash = append(hash, SOURCE_SEMANTICS_ID.as_bytes());
    hash = append(hash, FINITE_HARDWARE_POLICY_ID.as_bytes());
    for value in [
        spec.m,
        spec.n,
        spec.k,
        spec.a_stride,
        spec.b_stride,
        spec.c_stride,
        spec.a_len,
        spec.b_len,
        spec.c_len,
        spec.output_len,
    ] {
        hash = append(hash, &value.to_le_bytes());
    }
    match policy {
        ComparisonPolicy::ExactBits => {
            hash = append(hash, &[0]);
        }
        ComparisonPolicy::Bounded {
            max_abs,
            max_rel,
            max_ulps,
        } => {
            hash = append(hash, &[1]);
            hash = append(hash, &max_abs.to_bits().to_le_bytes());
            hash = append(hash, &max_rel.to_bits().to_le_bytes());
            hash = append(hash, &max_ulps.to_le_bytes());
        }
    }
    for value in expected {
        hash = append(hash, &value.to_bits().to_le_bytes());
    }
    hash
}

/// Validates inputs and evaluates one sealed finite hardware expectation.
pub fn build_hardware_expectation(
    spec: GemmSpec,
    inputs: GemmInputs<'_>,
    policy: ComparisonPolicy,
) -> Result<HardwareExpectation, HardwareExpectationError> {
    validate_lengths(spec, inputs)?;
    validate_bf16_logical(spec, NumericalOperand::A, inputs.a_bits)?;
    validate_bf16_logical(spec, NumericalOperand::B, inputs.b_bits)?;
    for row in 0..spec.m {
        for column in 0..spec.n {
            let index = spec.c_index(row, column);
            validate_f32_input(NumericalOperand::C, index, inputs.c[index])?;
        }
    }
    validate_f32_input(NumericalOperand::Alpha, 0, inputs.alpha)?;
    validate_f32_input(NumericalOperand::Beta, 0, inputs.beta)?;

    let expected = evaluate_finite(spec, inputs)?;
    let integrity = integrity_hash(spec, &expected, policy);
    Ok(HardwareExpectation {
        spec,
        expected,
        policy,
        integrity,
    })
}

/// Successful comparison summary over all compact logical outputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComparisonReport {
    /// Number of compared logical outputs.
    pub outputs: usize,
    /// Largest observed absolute error.
    pub max_abs_error: f32,
    /// Largest observed relative error, using `max(abs(expected), f32::MIN_POSITIVE)`.
    pub max_rel_error: f32,
    /// Largest ordered finite FP32 encoding distance.
    pub max_ulp_error: u32,
}

/// A sealed expectation or finite hardware observation failed comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ComparisonError {
    /// The expectation's bound contents do not match its integrity tag.
    CorruptExpectation,
    /// The observed compact output length is wrong.
    WrongObservedLength {
        /// Required logical output count.
        expected: usize,
        /// Supplied observed output count.
        actual: usize,
    },
    /// Hardware produced NaN or infinity, which no comparison policy admits.
    NonFiniteObservation {
        /// Compact row-major logical output index.
        index: usize,
        /// Exact observed FP32 encoding.
        bits: u32,
    },
    /// One observed output violates the sealed policy.
    Mismatch {
        /// Compact row-major logical output index.
        index: usize,
        /// Expected FP32 encoding.
        expected_bits: u32,
        /// Observed FP32 encoding.
        actual_bits: u32,
        /// Absolute error.
        abs_error: f32,
        /// Relative error.
        rel_error: f32,
        /// Ordered finite FP32 encoding distance.
        ulp_error: u32,
    },
}

impl fmt::Display for ComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CorruptExpectation => formatter.write_str(
                "hardware expectation contents or comparison policy do not match their integrity tag",
            ),
            Self::WrongObservedLength { expected, actual } => write!(
                formatter,
                "hardware observation requires {expected} logical outputs, got {actual}"
            ),
            Self::NonFiniteObservation { index, bits } => write!(
                formatter,
                "hardware output[{index}] is nonfinite FP32 encoding 0x{bits:08x}"
            ),
            Self::Mismatch {
                index,
                expected_bits,
                actual_bits,
                abs_error,
                rel_error,
                ulp_error,
            } => write!(
                formatter,
                "hardware output[{index}] expected 0x{expected_bits:08x}, got 0x{actual_bits:08x} (abs={abs_error}, rel={rel_error}, ulps={ulp_error})"
            ),
        }
    }
}

impl std::error::Error for ComparisonError {}

fn ordered_f32(bits: u32) -> u32 {
    if bits & 0x8000_0000 == 0 {
        bits | 0x8000_0000
    } else {
        !bits
    }
}

fn ulp_distance(left: f32, right: f32) -> u32 {
    ordered_f32(left.to_bits()).abs_diff(ordered_f32(right.to_bits()))
}

/// Compares compact logical hardware outputs using the expectation's sealed
/// exact-bit or bounded policy.
pub fn compare_hardware(
    expectation: &HardwareExpectation,
    observed: &[f32],
) -> Result<ComparisonReport, ComparisonError> {
    if integrity_hash(expectation.spec, &expectation.expected, expectation.policy)
        != expectation.integrity
    {
        return Err(ComparisonError::CorruptExpectation);
    }
    if observed.len() != expectation.expected.len() {
        return Err(ComparisonError::WrongObservedLength {
            expected: expectation.expected.len(),
            actual: observed.len(),
        });
    }

    let mut report = ComparisonReport {
        outputs: observed.len(),
        max_abs_error: 0.0,
        max_rel_error: 0.0,
        max_ulp_error: 0,
    };
    for (index, (expected, actual)) in expectation
        .expected
        .iter()
        .copied()
        .zip(observed.iter().copied())
        .enumerate()
    {
        if !actual.is_finite() {
            return Err(ComparisonError::NonFiniteObservation {
                index,
                bits: actual.to_bits(),
            });
        }
        let abs_error = (actual - expected).abs();
        let rel_error = abs_error / expected.abs().max(f32::MIN_POSITIVE);
        let ulp_error = ulp_distance(expected, actual);
        report.max_abs_error = report.max_abs_error.max(abs_error);
        report.max_rel_error = report.max_rel_error.max(rel_error);
        report.max_ulp_error = report.max_ulp_error.max(ulp_error);

        let accepted = match expectation.policy {
            ComparisonPolicy::ExactBits => actual.to_bits() == expected.to_bits(),
            ComparisonPolicy::Bounded {
                max_abs,
                max_rel,
                max_ulps,
            } => {
                actual.to_bits() == expected.to_bits()
                    || (abs_error <= max_abs + max_rel * expected.abs() && ulp_error <= max_ulps)
            }
        };
        if !accepted {
            return Err(ComparisonError::Mismatch {
                index,
                expected_bits: expected.to_bits(),
                actual_bits: actual.to_bits(),
                abs_error,
                rel_error,
                ulp_error,
            });
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_by_one() -> (GemmSpec, [u16; 1], [u16; 1], [f32; 1]) {
        (
            GemmSpec::checked(1, 1, 1, 1, 1, 1).unwrap(),
            [0x3f80],
            [0x4000],
            [3.0],
        )
    }

    #[test]
    fn hostile_expected_output_and_policy_mutations_fail_integrity() {
        let (spec, a, b, c) = one_by_one();
        let mut expectation = build_hardware_expectation(
            spec,
            GemmInputs {
                a_bits: &a,
                b_bits: &b,
                c: &c,
                alpha: 1.0,
                beta: 1.0,
            },
            ComparisonPolicy::ExactBits,
        )
        .unwrap();
        let observed = expectation.expected.clone();

        expectation.expected[0] = 6.0;
        assert_eq!(
            compare_hardware(&expectation, &observed),
            Err(ComparisonError::CorruptExpectation)
        );

        expectation.expected[0] = 5.0;
        expectation.policy = ComparisonPolicy::bounded(1.0, 1.0, u32::MAX).unwrap();
        assert_eq!(
            compare_hardware(&expectation, &observed),
            Err(ComparisonError::CorruptExpectation)
        );
    }
}
