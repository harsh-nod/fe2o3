//! Closed typed scalar-operator transcripts emitted by independent CPU and GPU
//! projections. Equality proves operator identity/congruence at the MIR/KIR
//! boundary only; it grants no arithmetic-value or target-instruction authority.

use core::fmt;
use std::collections::BTreeSet;

use sha2::{Digest as _, Sha256};

pub const MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2: usize = 8_192;
pub const MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticScalarTypeV2 {
    Bool,
    Integer { signed: bool, bits: u16 },
    Float { bits: u16 },
}

impl ProductionSemanticScalarTypeV2 {
    pub const fn bit_width(self) -> u16 {
        match self {
            Self::Bool => 1,
            Self::Integer { bits, .. } | Self::Float { bits } => bits,
        }
    }

    pub const fn is_integer(self) -> bool {
        matches!(self, Self::Integer { .. })
    }

    pub const fn is_float(self) -> bool {
        matches!(self, Self::Float { .. })
    }

    const fn is_supported(self) -> bool {
        match self {
            Self::Bool => true,
            Self::Integer { bits, .. } => matches!(bits, 8 | 16 | 32 | 64),
            Self::Float { bits } => matches!(bits, 32 | 64),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionIeeeRoundingModeV2 {
    NearestTiesToEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionIeeeExceptionalValuePolicyV2 {
    PreserveExactBits,
    CanonicalNan,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionNumericalContractV2 {
    ExactBitVectorOperatorCongruence,
    /// Exact operator identity and congruence at the authenticated MIR/KIR
    /// boundary. This does not claim target-instruction IEEE conformance.
    ExactIeee754OperatorCongruence {
        rounding: ProductionIeeeRoundingModeV2,
        exceptional_values: ProductionIeeeExceptionalValuePolicyV2,
    },
    Relaxed,
    ErrorBounded {
        absolute_error_f64_bits: u64,
        relative_error_f64_bits: u64,
    },
}

impl ProductionNumericalContractV2 {
    pub const fn exact_for(scalar: ProductionSemanticScalarTypeV2) -> Self {
        match scalar {
            ProductionSemanticScalarTypeV2::Float { .. } => Self::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            },
            ProductionSemanticScalarTypeV2::Bool
            | ProductionSemanticScalarTypeV2::Integer { .. } => {
                Self::ExactBitVectorOperatorCongruence
            }
        }
    }

    pub const fn is_supported(self) -> bool {
        matches!(
            self,
            Self::ExactBitVectorOperatorCongruence
                | Self::ExactIeee754OperatorCongruence {
                    rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                    exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
                }
        )
    }

    pub const fn admits_scalar(self, scalar: ProductionSemanticScalarTypeV2) -> bool {
        matches!(
            (self, scalar),
            (
                Self::ExactBitVectorOperatorCongruence,
                ProductionSemanticScalarTypeV2::Bool
                    | ProductionSemanticScalarTypeV2::Integer { .. }
            ) | (
                Self::ExactIeee754OperatorCongruence {
                    rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                    exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
                },
                ProductionSemanticScalarTypeV2::Float { .. }
            )
        )
    }

    pub fn exact_for_expression(expression: &ProductionSemanticExpressionV2) -> Self {
        if expression.contains_float_semantics() {
            Self::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            }
        } else {
            Self::ExactBitVectorOperatorCongruence
        }
    }

    pub fn admits_expression(self, expression: &ProductionSemanticExpressionV2) -> bool {
        self.is_supported()
            && if expression.contains_float_semantics() {
                matches!(self, Self::ExactIeee754OperatorCongruence { .. })
            } else {
                matches!(self, Self::ExactBitVectorOperatorCongruence)
            }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionOverflowContractV2 {
    Wrapping,
    Checked,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticUnaryOpV2 {
    Not,
    Negate,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticBinaryOpV2 {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    BitXor,
    BitAnd,
    BitOr,
    ShiftLeft,
    ShiftRight,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticComparisonV2 {
    Equal,
    LessThan,
    LessOrEqual,
    NotEqual,
    GreaterOrEqual,
    GreaterThan,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticCastV2 {
    Integer,
    IntegerToFloat,
    FloatToFloat,
    /// Rust `as` conversion, including NaN-to-zero and endpoint saturation.
    FloatToIntegerSaturating,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticExpressionV2 {
    Symbol {
        symbol: u32,
        scalar: ProductionSemanticScalarTypeV2,
    },
    Constant {
        scalar: ProductionSemanticScalarTypeV2,
        bits: u64,
    },
    Unary {
        operation: ProductionSemanticUnaryOpV2,
        scalar: ProductionSemanticScalarTypeV2,
        operand: Box<Self>,
    },
    Binary {
        operation: ProductionSemanticBinaryOpV2,
        scalar: ProductionSemanticScalarTypeV2,
        overflow: ProductionOverflowContractV2,
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Compare {
        operation: ProductionSemanticComparisonV2,
        operand_scalar: ProductionSemanticScalarTypeV2,
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Select {
        scalar: ProductionSemanticScalarTypeV2,
        condition: Box<Self>,
        when_true: Box<Self>,
        when_false: Box<Self>,
    },
    Cast {
        kind: ProductionSemanticCastV2,
        source: ProductionSemanticScalarTypeV2,
        target: ProductionSemanticScalarTypeV2,
        operand: Box<Self>,
    },
}

impl ProductionSemanticExpressionV2 {
    pub const fn scalar(&self) -> ProductionSemanticScalarTypeV2 {
        match self {
            Self::Symbol { scalar, .. }
            | Self::Constant { scalar, .. }
            | Self::Unary { scalar, .. }
            | Self::Binary { scalar, .. }
            | Self::Select { scalar, .. } => *scalar,
            Self::Compare { .. } => ProductionSemanticScalarTypeV2::Bool,
            Self::Cast { target, .. } => *target,
        }
    }

    pub fn contains_float_semantics(&self) -> bool {
        if self.scalar().is_float() {
            return true;
        }
        match self {
            Self::Symbol { .. } | Self::Constant { .. } => false,
            Self::Unary { operand, .. } | Self::Cast { operand, .. } => {
                operand.contains_float_semantics()
            }
            Self::Binary { lhs, rhs, .. } | Self::Compare { lhs, rhs, .. } => {
                lhs.contains_float_semantics() || rhs.contains_float_semantics()
            }
            Self::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                condition.contains_float_semantics()
                    || when_true.contains_float_semantics()
                    || when_false.contains_float_semantics()
            }
        }
    }

    pub fn validate(
        &self,
    ) -> Result<ProductionSemanticExpressionStatsV2, ProductionSemanticExpressionErrorV2> {
        let mut nodes = 0_usize;
        let depth = self.validate_inner(1, &mut nodes)?;
        let mut stats = ProductionSemanticExpressionStatsV2 {
            nodes,
            depth,
            arithmetic_operations: 0,
            comparisons: 0,
            selects: 0,
            casts: 0,
            checked_operations: 0,
            ieee_operations: 0,
        };
        self.accumulate_stats(&mut stats);
        Ok(stats)
    }

    /// Discharges operation-definedness using only authenticated constants.
    /// Dynamic guards are intentionally not assumed by this V2 expression.
    pub fn validate_static_domains(&self) -> Result<(), ProductionSemanticExpressionErrorV2> {
        match self {
            Self::Symbol { .. } | Self::Constant { .. } => Ok(()),
            Self::Unary {
                operation,
                scalar,
                operand,
            } => {
                operand.validate_static_domains()?;
                if *operation == ProductionSemanticUnaryOpV2::Negate {
                    if let ProductionSemanticScalarTypeV2::Integer { signed: true, bits } = scalar {
                        let Some(value) = constant_bits(operand) else {
                            return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                        };
                        if signed_value(value, *bits) == -(1_i128 << (bits - 1)) {
                            return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                        }
                    }
                }
                Ok(())
            }
            Self::Binary {
                operation,
                scalar,
                overflow,
                lhs,
                rhs,
            } => {
                lhs.validate_static_domains()?;
                rhs.validate_static_domains()?;
                if *overflow == ProductionOverflowContractV2::Checked {
                    let (Some(lhs), Some(rhs)) = (constant_bits(lhs), constant_bits(rhs)) else {
                        return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                    };
                    if !checked_integer_result_in_range(*operation, *scalar, lhs, rhs) {
                        return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                    }
                }
                if matches!(
                    operation,
                    ProductionSemanticBinaryOpV2::Divide | ProductionSemanticBinaryOpV2::Remainder
                ) && scalar.is_integer()
                {
                    let Some(rhs_bits) = constant_bits(rhs) else {
                        return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                    };
                    if rhs_bits == 0 {
                        return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                    }
                    if let ProductionSemanticScalarTypeV2::Integer { signed: true, bits } = scalar {
                        let rhs = signed_value(rhs_bits, *bits);
                        if rhs == -1 {
                            let Some(lhs_bits) = constant_bits(lhs) else {
                                return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                            };
                            if signed_value(lhs_bits, *bits) == -(1_i128 << (bits - 1)) {
                                return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                            }
                        }
                    }
                }
                if matches!(
                    operation,
                    ProductionSemanticBinaryOpV2::ShiftLeft
                        | ProductionSemanticBinaryOpV2::ShiftRight
                ) {
                    let Some(shift) = constant_bits(rhs) else {
                        return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                    };
                    if shift >= u64::from(scalar.bit_width()) {
                        return Err(ProductionSemanticExpressionErrorV2::IncompleteDomain);
                    }
                }
                Ok(())
            }
            Self::Compare { lhs, rhs, .. } => {
                lhs.validate_static_domains()?;
                rhs.validate_static_domains()
            }
            Self::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                condition.validate_static_domains()?;
                when_true.validate_static_domains()?;
                when_false.validate_static_domains()
            }
            Self::Cast { operand, .. } => operand.validate_static_domains(),
        }
    }

    fn accumulate_stats(&self, stats: &mut ProductionSemanticExpressionStatsV2) {
        match self {
            Self::Symbol { .. } | Self::Constant { .. } => {}
            Self::Unary {
                scalar, operand, ..
            } => {
                stats.arithmetic_operations += 1;
                stats.ieee_operations += usize::from(scalar.is_float());
                operand.accumulate_stats(stats);
            }
            Self::Binary {
                scalar,
                overflow,
                lhs,
                rhs,
                ..
            } => {
                stats.arithmetic_operations += 1;
                stats.checked_operations +=
                    usize::from(*overflow == ProductionOverflowContractV2::Checked);
                stats.ieee_operations += usize::from(scalar.is_float());
                lhs.accumulate_stats(stats);
                rhs.accumulate_stats(stats);
            }
            Self::Compare {
                operand_scalar,
                lhs,
                rhs,
                ..
            } => {
                stats.comparisons += 1;
                stats.ieee_operations += usize::from(operand_scalar.is_float());
                lhs.accumulate_stats(stats);
                rhs.accumulate_stats(stats);
            }
            Self::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                stats.selects += 1;
                condition.accumulate_stats(stats);
                when_true.accumulate_stats(stats);
                when_false.accumulate_stats(stats);
            }
            Self::Cast {
                source,
                target,
                operand,
                ..
            } => {
                stats.casts += 1;
                stats.ieee_operations += usize::from(source.is_float() || target.is_float());
                operand.accumulate_stats(stats);
            }
        }
    }

    fn validate_inner(
        &self,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<usize, ProductionSemanticExpressionErrorV2> {
        *nodes = nodes
            .checked_add(1)
            .ok_or(ProductionSemanticExpressionErrorV2::ResourceLimit)?;
        if *nodes > MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2
            || depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2
            || !self.scalar().is_supported()
        {
            return Err(ProductionSemanticExpressionErrorV2::ResourceLimit);
        }
        let child_depth = match self {
            Self::Symbol { .. } => depth,
            Self::Constant { scalar, bits } => {
                if scalar.bit_width() < 64 && *bits >= (1_u64 << scalar.bit_width()) {
                    return Err(ProductionSemanticExpressionErrorV2::ConstantOutOfRange);
                }
                depth
            }
            Self::Unary {
                operation,
                scalar,
                operand,
            } => {
                if operand.scalar() != *scalar
                    || match operation {
                        ProductionSemanticUnaryOpV2::Not => scalar.is_float(),
                        ProductionSemanticUnaryOpV2::Negate => !matches!(
                            scalar,
                            ProductionSemanticScalarTypeV2::Integer { signed: true, .. }
                                | ProductionSemanticScalarTypeV2::Float { .. }
                        ),
                    }
                {
                    return Err(ProductionSemanticExpressionErrorV2::TypeMismatch);
                }
                operand.validate_inner(depth + 1, nodes)?
            }
            Self::Binary {
                operation,
                scalar,
                overflow,
                lhs,
                rhs,
            } => {
                let shift = matches!(
                    operation,
                    ProductionSemanticBinaryOpV2::ShiftLeft
                        | ProductionSemanticBinaryOpV2::ShiftRight
                );
                if lhs.scalar() != *scalar
                    || if shift {
                        !rhs.scalar().is_integer()
                    } else {
                        rhs.scalar() != *scalar
                    }
                    || !scalar.is_integer() && !scalar.is_float()
                    || scalar.is_float()
                        && (!matches!(
                            operation,
                            ProductionSemanticBinaryOpV2::Add
                                | ProductionSemanticBinaryOpV2::Subtract
                                | ProductionSemanticBinaryOpV2::Multiply
                                | ProductionSemanticBinaryOpV2::Divide
                                | ProductionSemanticBinaryOpV2::Remainder
                        ) || *overflow != ProductionOverflowContractV2::Wrapping)
                    || *overflow == ProductionOverflowContractV2::Checked
                        && !matches!(
                            operation,
                            ProductionSemanticBinaryOpV2::Add
                                | ProductionSemanticBinaryOpV2::Subtract
                                | ProductionSemanticBinaryOpV2::Multiply
                        )
                {
                    return Err(ProductionSemanticExpressionErrorV2::TypeMismatch);
                }
                lhs.validate_inner(depth + 1, nodes)?
                    .max(rhs.validate_inner(depth + 1, nodes)?)
            }
            Self::Compare {
                operand_scalar,
                lhs,
                rhs,
                ..
            } => {
                if lhs.scalar() != *operand_scalar
                    || rhs.scalar() != *operand_scalar
                    || !operand_scalar.is_supported()
                {
                    return Err(ProductionSemanticExpressionErrorV2::TypeMismatch);
                }
                lhs.validate_inner(depth + 1, nodes)?
                    .max(rhs.validate_inner(depth + 1, nodes)?)
            }
            Self::Select {
                scalar,
                condition,
                when_true,
                when_false,
            } => {
                if condition.scalar() != ProductionSemanticScalarTypeV2::Bool
                    || when_true.scalar() != *scalar
                    || when_false.scalar() != *scalar
                {
                    return Err(ProductionSemanticExpressionErrorV2::TypeMismatch);
                }
                condition
                    .validate_inner(depth + 1, nodes)?
                    .max(when_true.validate_inner(depth + 1, nodes)?)
                    .max(when_false.validate_inner(depth + 1, nodes)?)
            }
            Self::Cast {
                kind,
                source,
                target,
                operand,
            } => {
                if operand.scalar() != *source
                    || match kind {
                        ProductionSemanticCastV2::Integer => !matches!(
                            (source, target),
                            (
                                ProductionSemanticScalarTypeV2::Integer { .. },
                                ProductionSemanticScalarTypeV2::Integer { .. },
                            ) | (
                                ProductionSemanticScalarTypeV2::Bool,
                                ProductionSemanticScalarTypeV2::Integer { .. },
                            ) | (
                                ProductionSemanticScalarTypeV2::Bool,
                                ProductionSemanticScalarTypeV2::Bool,
                            )
                        ),
                        ProductionSemanticCastV2::IntegerToFloat => {
                            !source.is_integer() || !target.is_float()
                        }
                        ProductionSemanticCastV2::FloatToFloat => {
                            !source.is_float() || !target.is_float()
                        }
                        ProductionSemanticCastV2::FloatToIntegerSaturating => {
                            !source.is_float() || !target.is_integer()
                        }
                    }
                {
                    return Err(ProductionSemanticExpressionErrorV2::TypeMismatch);
                }
                operand.validate_inner(depth + 1, nodes)?
            }
        };
        Ok(depth.max(child_depth))
    }

    pub fn symbols(&self, output: &mut BTreeSet<u32>) {
        match self {
            Self::Symbol { symbol, .. } => {
                output.insert(*symbol);
            }
            Self::Constant { .. } => {}
            Self::Unary { operand, .. } | Self::Cast { operand, .. } => operand.symbols(output),
            Self::Binary { lhs, rhs, .. } | Self::Compare { lhs, rhs, .. } => {
                lhs.symbols(output);
                rhs.symbols(output);
            }
            Self::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                condition.symbols(output);
                when_true.symbols(output);
                when_false.symbols(output);
            }
        }
    }

    pub fn canonical_sha256(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3/production-semantic-expression/v2\0");
        hash_expression(&mut digest, self);
        digest.finalize().into()
    }

    pub fn canonical_transcript_sha256(
        &self,
        numerical_contract: ProductionNumericalContractV2,
    ) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3/production-semantic-expression-transcript/v2\0");
        digest.update(self.canonical_sha256());
        hash_numerical_contract(&mut digest, numerical_contract);
        digest.finalize().into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticExpressionStatsV2 {
    pub nodes: usize,
    pub depth: usize,
    pub arithmetic_operations: usize,
    pub comparisons: usize,
    pub selects: usize,
    pub casts: usize,
    pub checked_operations: usize,
    pub ieee_operations: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticExpressionErrorV2 {
    ResourceLimit,
    TypeMismatch,
    ConstantOutOfRange,
    UnsupportedNumericalContract,
    IncompleteDomain,
}

impl fmt::Display for ProductionSemanticExpressionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ResourceLimit => {
                "semantic expression exceeds its node, depth, or scalar-width bound"
            }
            Self::TypeMismatch => "semantic expression has an invalid typed operation",
            Self::ConstantOutOfRange => {
                "semantic expression constant exceeds its scalar bit width"
            }
            Self::UnsupportedNumericalContract => {
                "relaxed or error-bounded floating-point refinement is not implemented"
            }
            Self::IncompleteDomain => {
                "operation definedness requires an authenticated dynamic guard or a stronger range proof"
            }
        })
    }
}

fn constant_bits(expression: &ProductionSemanticExpressionV2) -> Option<u64> {
    match expression {
        ProductionSemanticExpressionV2::Constant { bits, .. } => Some(*bits),
        _ => None,
    }
}

fn signed_value(bits: u64, width: u16) -> i128 {
    let value = i128::from(bits);
    let sign = 1_i128 << (width - 1);
    if value < sign {
        value
    } else {
        value - (1_i128 << width)
    }
}

fn checked_integer_result_in_range(
    operation: ProductionSemanticBinaryOpV2,
    scalar: ProductionSemanticScalarTypeV2,
    lhs: u64,
    rhs: u64,
) -> bool {
    let ProductionSemanticScalarTypeV2::Integer { signed, bits } = scalar else {
        return false;
    };
    if signed {
        let lhs = signed_value(lhs, bits);
        let rhs = signed_value(rhs, bits);
        let result = match operation {
            ProductionSemanticBinaryOpV2::Add => lhs.checked_add(rhs),
            ProductionSemanticBinaryOpV2::Subtract => lhs.checked_sub(rhs),
            ProductionSemanticBinaryOpV2::Multiply => lhs.checked_mul(rhs),
            _ => return false,
        };
        let minimum = -(1_i128 << (bits - 1));
        let maximum = (1_i128 << (bits - 1)) - 1;
        result.is_some_and(|result| (minimum..=maximum).contains(&result))
    } else {
        let lhs = u128::from(lhs);
        let rhs = u128::from(rhs);
        let result = match operation {
            ProductionSemanticBinaryOpV2::Add => lhs.checked_add(rhs),
            ProductionSemanticBinaryOpV2::Subtract => lhs.checked_sub(rhs),
            ProductionSemanticBinaryOpV2::Multiply => lhs.checked_mul(rhs),
            _ => return false,
        };
        let maximum = (1_u128 << bits) - 1;
        result.is_some_and(|result| result <= maximum)
    }
}

impl std::error::Error for ProductionSemanticExpressionErrorV2 {}

fn scalar_tag(scalar: ProductionSemanticScalarTypeV2) -> [u8; 4] {
    match scalar {
        ProductionSemanticScalarTypeV2::Bool => [0, 0, 0, 0],
        ProductionSemanticScalarTypeV2::Integer { signed, bits } => {
            let width = bits.to_le_bytes();
            [1, u8::from(signed), width[0], width[1]]
        }
        ProductionSemanticScalarTypeV2::Float { bits } => {
            let width = bits.to_le_bytes();
            [2, 0, width[0], width[1]]
        }
    }
}

fn hash_numerical_contract(digest: &mut Sha256, contract: ProductionNumericalContractV2) {
    match contract {
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence => digest.update([0]),
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding,
            exceptional_values,
        } => digest.update([1, rounding as u8, exceptional_values as u8]),
        ProductionNumericalContractV2::Relaxed => digest.update([2]),
        ProductionNumericalContractV2::ErrorBounded {
            absolute_error_f64_bits,
            relative_error_f64_bits,
        } => {
            digest.update([3]);
            digest.update(absolute_error_f64_bits.to_le_bytes());
            digest.update(relative_error_f64_bits.to_le_bytes());
        }
    }
}

fn hash_expression(digest: &mut Sha256, expression: &ProductionSemanticExpressionV2) {
    match expression {
        ProductionSemanticExpressionV2::Symbol { symbol, scalar } => {
            digest.update([0]);
            digest.update(scalar_tag(*scalar));
            digest.update(symbol.to_le_bytes());
        }
        ProductionSemanticExpressionV2::Constant { scalar, bits } => {
            digest.update([1]);
            digest.update(scalar_tag(*scalar));
            digest.update(bits.to_le_bytes());
        }
        ProductionSemanticExpressionV2::Unary {
            operation,
            scalar,
            operand,
        } => {
            digest.update([2, *operation as u8]);
            digest.update(scalar_tag(*scalar));
            hash_expression(digest, operand);
        }
        ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            overflow,
            lhs,
            rhs,
        } => {
            digest.update([3, *operation as u8, *overflow as u8]);
            digest.update(scalar_tag(*scalar));
            hash_expression(digest, lhs);
            hash_expression(digest, rhs);
        }
        ProductionSemanticExpressionV2::Compare {
            operation,
            operand_scalar,
            lhs,
            rhs,
        } => {
            digest.update([4, *operation as u8]);
            digest.update(scalar_tag(*operand_scalar));
            hash_expression(digest, lhs);
            hash_expression(digest, rhs);
        }
        ProductionSemanticExpressionV2::Select {
            scalar,
            condition,
            when_true,
            when_false,
        } => {
            digest.update([5]);
            digest.update(scalar_tag(*scalar));
            hash_expression(digest, condition);
            hash_expression(digest, when_true);
            hash_expression(digest, when_false);
        }
        ProductionSemanticExpressionV2::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            digest.update([6, *kind as u8]);
            digest.update(scalar_tag(*source));
            digest.update(scalar_tag(*target));
            hash_expression(digest, operand);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_symbol(symbol: u32) -> ProductionSemanticExpressionV2 {
        ProductionSemanticExpressionV2::Symbol {
            symbol,
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
        }
    }

    #[test]
    fn validates_typed_arithmetic_compare_select_and_cast() {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let sum = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow: ProductionOverflowContractV2::Checked,
            lhs: Box::new(u32_symbol(1)),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 7 }),
        };
        let condition = ProductionSemanticExpressionV2::Compare {
            operation: ProductionSemanticComparisonV2::LessThan,
            operand_scalar: scalar,
            lhs: Box::new(sum.clone()),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 64 }),
        };
        let expression = ProductionSemanticExpressionV2::Select {
            scalar,
            condition: Box::new(condition),
            when_true: Box::new(sum),
            when_false: Box::new(ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 8,
                },
                target: scalar,
                operand: Box::new(ProductionSemanticExpressionV2::Constant {
                    scalar: ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 8,
                    },
                    bits: 3,
                }),
            }),
        };
        assert_eq!(expression.validate().unwrap().nodes, 11);
    }

    #[test]
    fn rejects_bad_types_and_out_of_width_constants() {
        let bad = ProductionSemanticExpressionV2::Constant {
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 8,
            },
            bits: 256,
        };
        assert_eq!(
            bad.validate(),
            Err(ProductionSemanticExpressionErrorV2::ConstantOutOfRange)
        );

        let bad = ProductionSemanticExpressionV2::Select {
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
            condition: Box::new(u32_symbol(0)),
            when_true: Box::new(u32_symbol(1)),
            when_false: Box::new(u32_symbol(2)),
        };
        assert_eq!(
            bad.validate(),
            Err(ProductionSemanticExpressionErrorV2::TypeMismatch)
        );

        let unsigned = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let bad = ProductionSemanticExpressionV2::Unary {
            operation: ProductionSemanticUnaryOpV2::Negate,
            scalar: unsigned,
            operand: Box::new(ProductionSemanticExpressionV2::Constant {
                scalar: unsigned,
                bits: 1,
            }),
        };
        assert_eq!(
            bad.validate(),
            Err(ProductionSemanticExpressionErrorV2::TypeMismatch)
        );

        let bad = ProductionSemanticExpressionV2::Cast {
            kind: ProductionSemanticCastV2::Integer,
            source: unsigned,
            target: ProductionSemanticScalarTypeV2::Bool,
            operand: Box::new(ProductionSemanticExpressionV2::Constant {
                scalar: unsigned,
                bits: 1,
            }),
        };
        assert_eq!(
            bad.validate(),
            Err(ProductionSemanticExpressionErrorV2::TypeMismatch)
        );
    }

    #[test]
    fn numerical_contracts_fail_closed_outside_exact_modes() {
        assert!(ProductionNumericalContractV2::ExactBitVectorOperatorCongruence.is_supported());
        assert!(
            ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            }
            .is_supported()
        );
        assert!(
            !ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::TowardZero,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            }
            .is_supported()
        );
        assert!(!ProductionNumericalContractV2::Relaxed.is_supported());
        assert!(
            !ProductionNumericalContractV2::ErrorBounded {
                absolute_error_f64_bits: 0,
                relative_error_f64_bits: 0,
            }
            .is_supported()
        );
    }

    #[test]
    fn semantic_mutations_change_the_canonical_identity() {
        let lhs = u32_symbol(1);
        let rhs = u32_symbol(2);
        let make = |operation| ProductionSemanticExpressionV2::Binary {
            operation,
            scalar: ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            },
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(lhs.clone()),
            rhs: Box::new(rhs.clone()),
        };
        assert_ne!(
            make(ProductionSemanticBinaryOpV2::Add).canonical_sha256(),
            make(ProductionSemanticBinaryOpV2::Subtract).canonical_sha256(),
        );
    }

    #[test]
    fn overflow_mode_substitution_changes_identity_and_cannot_mint_completion() {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let make = |overflow| ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow,
            lhs: Box::new(u32_symbol(1)),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 1 }),
        };
        let wrapping = make(ProductionOverflowContractV2::Wrapping);
        let checked = make(ProductionOverflowContractV2::Checked);
        assert_ne!(wrapping.canonical_sha256(), checked.canonical_sha256());
        assert!(wrapping.validate_static_domains().is_ok());
        assert_eq!(
            checked.validate_static_domains(),
            Err(ProductionSemanticExpressionErrorV2::IncompleteDomain),
        );
    }

    #[test]
    fn mixed_float_expressions_require_the_ieee_operator_contract() {
        let float = ProductionSemanticScalarTypeV2::Float { bits: 32 };
        let expression = ProductionSemanticExpressionV2::Compare {
            operation: ProductionSemanticComparisonV2::LessThan,
            operand_scalar: float,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                symbol: 0,
                scalar: float,
            }),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant {
                scalar: float,
                bits: 0,
            }),
        };
        assert_eq!(expression.scalar(), ProductionSemanticScalarTypeV2::Bool);
        assert!(
            !ProductionNumericalContractV2::ExactBitVectorOperatorCongruence
                .admits_expression(&expression)
        );
        assert!(matches!(
            ProductionNumericalContractV2::exact_for_expression(&expression),
            ProductionNumericalContractV2::ExactIeee754OperatorCongruence { .. }
        ));
    }

    #[test]
    fn float_to_integer_cast_binds_the_saturating_policy() {
        let source = ProductionSemanticScalarTypeV2::Float { bits: 32 };
        let target = ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 32,
        };
        let operand = Box::new(ProductionSemanticExpressionV2::Constant {
            scalar: source,
            bits: f32::NAN.to_bits().into(),
        });
        let saturating = ProductionSemanticExpressionV2::Cast {
            kind: ProductionSemanticCastV2::FloatToIntegerSaturating,
            source,
            target,
            operand,
        };
        assert!(saturating.validate().is_ok());
        assert!(saturating.validate_static_domains().is_ok());
        assert!(matches!(
            ProductionNumericalContractV2::exact_for_expression(&saturating),
            ProductionNumericalContractV2::ExactIeee754OperatorCongruence { .. }
        ));
    }

    #[test]
    fn partial_operations_require_statically_discharged_domains() {
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let constant = |bits| ProductionSemanticExpressionV2::Constant { scalar, bits };
        let binary = |operation, overflow, lhs, rhs| ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            overflow,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        };
        assert!(
            binary(
                ProductionSemanticBinaryOpV2::Add,
                ProductionOverflowContractV2::Checked,
                constant(4),
                constant(5),
            )
            .validate_static_domains()
            .is_ok()
        );
        for incomplete in [
            binary(
                ProductionSemanticBinaryOpV2::Add,
                ProductionOverflowContractV2::Checked,
                u32_symbol(0),
                constant(5),
            ),
            binary(
                ProductionSemanticBinaryOpV2::Divide,
                ProductionOverflowContractV2::Wrapping,
                constant(5),
                constant(0),
            ),
            binary(
                ProductionSemanticBinaryOpV2::ShiftLeft,
                ProductionOverflowContractV2::Wrapping,
                constant(5),
                constant(32),
            ),
        ] {
            assert_eq!(
                incomplete.validate_static_domains(),
                Err(ProductionSemanticExpressionErrorV2::IncompleteDomain)
            );
        }
    }

    #[test]
    fn ieee_policy_mutations_change_the_transcript() {
        let expression = ProductionSemanticExpressionV2::Constant {
            scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
            bits: 0x3f80_0000,
        };
        let nearest = ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
            exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
        };
        let toward_zero = ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding: ProductionIeeeRoundingModeV2::TowardZero,
            exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
        };
        assert_ne!(
            expression.canonical_transcript_sha256(nearest),
            expression.canonical_transcript_sha256(toward_zero),
        );
    }

    #[test]
    fn type_symbol_overflow_cast_and_policy_mutations_are_all_bound() {
        let u32_scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let u64_scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 64,
        };
        let binary = |symbol, scalar, overflow| ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow,
            lhs: Box::new(ProductionSemanticExpressionV2::Symbol { symbol, scalar }),
            rhs: Box::new(ProductionSemanticExpressionV2::Constant { scalar, bits: 1 }),
        };
        let base = ProductionSemanticExpressionV2::Cast {
            kind: ProductionSemanticCastV2::Integer,
            source: u32_scalar,
            target: u64_scalar,
            operand: Box::new(binary(
                1,
                u32_scalar,
                ProductionOverflowContractV2::Wrapping,
            )),
        };
        let mutations = [
            ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source: u32_scalar,
                target: u64_scalar,
                operand: Box::new(binary(
                    2,
                    u32_scalar,
                    ProductionOverflowContractV2::Wrapping,
                )),
            },
            ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source: u64_scalar,
                target: u32_scalar,
                operand: Box::new(binary(
                    1,
                    u64_scalar,
                    ProductionOverflowContractV2::Wrapping,
                )),
            },
            ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::Integer,
                source: u32_scalar,
                target: u64_scalar,
                operand: Box::new(binary(1, u32_scalar, ProductionOverflowContractV2::Checked)),
            },
            ProductionSemanticExpressionV2::Cast {
                kind: ProductionSemanticCastV2::IntegerToFloat,
                source: u32_scalar,
                target: ProductionSemanticScalarTypeV2::Float { bits: 64 },
                operand: Box::new(binary(
                    1,
                    u32_scalar,
                    ProductionOverflowContractV2::Wrapping,
                )),
            },
        ];
        let contract = ProductionNumericalContractV2::ExactBitVectorOperatorCongruence;
        for mutation in mutations {
            assert_ne!(
                base.canonical_transcript_sha256(contract),
                mutation.canonical_transcript_sha256(contract),
            );
        }
        assert_ne!(
            base.canonical_transcript_sha256(contract),
            base.canonical_transcript_sha256(
                ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
                    rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
                    exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
                },
            ),
        );
    }
}
