//! Canonical in-memory form reconstructed from typed semantic SSA operations.

use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    SemanticExceptionalValueAttr, SemanticIeeeRoundingAttr, SemanticNumericalPolicyAttr,
    SemanticOverflowAttr, SemanticTypedBinaryKindAttr, SemanticTypedCastKindAttr,
    SemanticTypedCompareKindAttr, SemanticTypedScalarV1, SemanticTypedUnaryKindAttr,
};

pub const MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1: usize = 8_192;
pub const MAX_SEMANTIC_TYPED_EXPRESSION_DEPTH_V1: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SemanticNumericalContractV1 {
    pub policy: SemanticNumericalPolicyAttr,
    pub rounding: SemanticIeeeRoundingAttr,
    pub exceptional_values: SemanticExceptionalValueAttr,
}

impl SemanticNumericalContractV1 {
    pub fn validate(
        self,
        expression: &SemanticTypedExpressionV1,
    ) -> Result<(), SemanticTypedExpressionErrorV1> {
        match self.policy {
            SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
                if !expression.contains_float_semantics()
                    && self.rounding == SemanticIeeeRoundingAttr::NearestTiesToEven
                    && self.exceptional_values
                        == SemanticExceptionalValueAttr::PreserveExactBits =>
            {
                Ok(())
            }
            SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits
                if expression.contains_float_semantics()
                    && self.rounding == SemanticIeeeRoundingAttr::NearestTiesToEven
                    && self.exceptional_values
                        == SemanticExceptionalValueAttr::PreserveExactBits =>
            {
                Ok(())
            }
            _ => Err(SemanticTypedExpressionErrorV1::UnsupportedNumericalPolicy),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SemanticTypedExpressionV1 {
    Symbol {
        symbol: u32,
        scalar: SemanticTypedScalarV1,
    },
    Constant {
        scalar: SemanticTypedScalarV1,
        bits: u64,
    },
    Unary {
        operation: SemanticTypedUnaryKindAttr,
        scalar: SemanticTypedScalarV1,
        operand: Box<Self>,
    },
    Binary {
        operation: SemanticTypedBinaryKindAttr,
        scalar: SemanticTypedScalarV1,
        overflow: SemanticOverflowAttr,
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Compare {
        operation: SemanticTypedCompareKindAttr,
        operand_scalar: SemanticTypedScalarV1,
        lhs: Box<Self>,
        rhs: Box<Self>,
    },
    Select {
        scalar: SemanticTypedScalarV1,
        condition: Box<Self>,
        when_true: Box<Self>,
        when_false: Box<Self>,
    },
    Cast {
        kind: SemanticTypedCastKindAttr,
        source: SemanticTypedScalarV1,
        target: SemanticTypedScalarV1,
        operand: Box<Self>,
    },
}

impl SemanticTypedExpressionV1 {
    pub const fn scalar(&self) -> SemanticTypedScalarV1 {
        match self {
            Self::Symbol { scalar, .. }
            | Self::Constant { scalar, .. }
            | Self::Unary { scalar, .. }
            | Self::Binary { scalar, .. }
            | Self::Select { scalar, .. } => *scalar,
            Self::Compare { .. } => {
                SemanticTypedScalarV1::new(crate::SemanticScalarKindAttr::Bool, 1).unwrap()
            }
            Self::Cast { target, .. } => *target,
        }
    }

    pub fn validate(
        &self,
    ) -> Result<SemanticTypedExpressionStatsV1, SemanticTypedExpressionErrorV1> {
        let mut nodes = 0;
        let depth = self.validate_inner(1, &mut nodes)?;
        Ok(SemanticTypedExpressionStatsV1 { nodes, depth })
    }

    pub fn validate_static_domains(&self) -> Result<(), SemanticTypedExpressionErrorV1> {
        match self {
            Self::Symbol { .. } | Self::Constant { .. } => Ok(()),
            Self::Unary {
                operation,
                scalar,
                operand,
            } => {
                operand.validate_static_domains()?;
                if *operation == SemanticTypedUnaryKindAttr::Negate
                    && scalar.kind() == crate::SemanticScalarKindAttr::SignedInteger
                {
                    let Some(value) = constant_bits(operand) else {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                    };
                    if signed_value(value, scalar.bits()) == -(1_i128 << (scalar.bits() - 1)) {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
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
                if *overflow == SemanticOverflowAttr::Checked {
                    let (Some(lhs), Some(rhs)) = (constant_bits(lhs), constant_bits(rhs)) else {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                    };
                    if !checked_integer_result_in_range(*operation, *scalar, lhs, rhs) {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                    }
                }
                if matches!(
                    operation,
                    SemanticTypedBinaryKindAttr::Divide | SemanticTypedBinaryKindAttr::Remainder
                ) && scalar.is_integer()
                {
                    let Some(rhs_bits) = constant_bits(rhs) else {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                    };
                    if rhs_bits == 0 {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                    }
                    if scalar.kind() == crate::SemanticScalarKindAttr::SignedInteger
                        && signed_value(rhs_bits, scalar.bits()) == -1
                    {
                        let Some(lhs_bits) = constant_bits(lhs) else {
                            return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                        };
                        if signed_value(lhs_bits, scalar.bits()) == -(1_i128 << (scalar.bits() - 1))
                        {
                            return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                        }
                    }
                }
                if matches!(
                    operation,
                    SemanticTypedBinaryKindAttr::ShiftLeft
                        | SemanticTypedBinaryKindAttr::ShiftRight
                ) {
                    let Some(shift) = constant_bits(rhs) else {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
                    };
                    if shift >= u64::from(scalar.bits()) {
                        return Err(SemanticTypedExpressionErrorV1::IncompleteDomain);
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

    pub fn canonical_transcript_sha256(
        self: &Self,
        contract: SemanticNumericalContractV1,
    ) -> [u8; 32] {
        let mut expression_digest = Sha256::new();
        expression_digest.update(b"fe2o3/production-semantic-expression/v2\0");
        hash_expression(&mut expression_digest, self);
        let mut transcript = Sha256::new();
        transcript.update(b"fe2o3/production-semantic-expression-transcript/v2\0");
        transcript.update(expression_digest.finalize());
        match contract.policy {
            SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence => transcript.update([0]),
            SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits => transcript
                .update([
                    1,
                    contract.rounding as u8,
                    contract.exceptional_values as u8,
                ]),
        }
        transcript.finalize().into()
    }

    fn validate_inner(
        &self,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<usize, SemanticTypedExpressionErrorV1> {
        *nodes = nodes
            .checked_add(1)
            .ok_or(SemanticTypedExpressionErrorV1::ResourceLimit)?;
        if *nodes > MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1
            || depth > MAX_SEMANTIC_TYPED_EXPRESSION_DEPTH_V1
        {
            return Err(SemanticTypedExpressionErrorV1::ResourceLimit);
        }
        let child_depth = match self {
            Self::Symbol { .. } => depth,
            Self::Constant { scalar, bits } => {
                if scalar.bits() < 64 && *bits >= (1_u64 << scalar.bits()) {
                    return Err(SemanticTypedExpressionErrorV1::ConstantOutOfRange);
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
                        SemanticTypedUnaryKindAttr::Not => scalar.is_float(),
                        SemanticTypedUnaryKindAttr::Negate => {
                            !scalar.is_integer() && !scalar.is_float()
                        }
                    }
                {
                    return Err(SemanticTypedExpressionErrorV1::TypeMismatch);
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
                    SemanticTypedBinaryKindAttr::ShiftLeft
                        | SemanticTypedBinaryKindAttr::ShiftRight
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
                            SemanticTypedBinaryKindAttr::Add
                                | SemanticTypedBinaryKindAttr::Subtract
                                | SemanticTypedBinaryKindAttr::Multiply
                                | SemanticTypedBinaryKindAttr::Divide
                                | SemanticTypedBinaryKindAttr::Remainder
                        ) || *overflow != SemanticOverflowAttr::Wrapping)
                    || *overflow == SemanticOverflowAttr::Checked
                        && !matches!(
                            operation,
                            SemanticTypedBinaryKindAttr::Add
                                | SemanticTypedBinaryKindAttr::Subtract
                                | SemanticTypedBinaryKindAttr::Multiply
                        )
                {
                    return Err(SemanticTypedExpressionErrorV1::TypeMismatch);
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
                if lhs.scalar() != *operand_scalar || rhs.scalar() != *operand_scalar {
                    return Err(SemanticTypedExpressionErrorV1::TypeMismatch);
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
                if !condition.scalar().is_bool()
                    || when_true.scalar() != *scalar
                    || when_false.scalar() != *scalar
                {
                    return Err(SemanticTypedExpressionErrorV1::TypeMismatch);
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
                        SemanticTypedCastKindAttr::Integer => {
                            (!source.is_integer() && !source.is_bool())
                                || (!target.is_integer() && !target.is_bool())
                        }
                        SemanticTypedCastKindAttr::IntegerToFloat => {
                            !source.is_integer() || !target.is_float()
                        }
                        SemanticTypedCastKindAttr::FloatToFloat => {
                            !source.is_float() || !target.is_float()
                        }
                        SemanticTypedCastKindAttr::FloatToIntegerSaturating => {
                            !source.is_float() || !target.is_integer()
                        }
                    }
                {
                    return Err(SemanticTypedExpressionErrorV1::TypeMismatch);
                }
                operand.validate_inner(depth + 1, nodes)?
            }
        };
        Ok(depth.max(child_depth))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTypedExpressionStatsV1 {
    pub nodes: usize,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticTypedExpressionErrorV1 {
    ResourceLimit,
    TypeMismatch,
    ConstantOutOfRange,
    UnsupportedNumericalPolicy,
    IncompleteDomain,
}

impl fmt::Display for SemanticTypedExpressionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ResourceLimit => "typed semantic expression exceeds its node or depth bound",
            Self::TypeMismatch => {
                "typed semantic expression has an invalid type, operator, or arity"
            }
            Self::ConstantOutOfRange => {
                "typed semantic expression constant exceeds its scalar width"
            }
            Self::UnsupportedNumericalPolicy => {
                "typed semantic expression has an unsupported or mismatched numerical policy"
            }
            Self::IncompleteDomain => {
                "typed semantic operation definedness is not statically established"
            }
        })
    }
}

impl std::error::Error for SemanticTypedExpressionErrorV1 {}

fn constant_bits(expression: &SemanticTypedExpressionV1) -> Option<u64> {
    match expression {
        SemanticTypedExpressionV1::Constant { bits, .. } => Some(*bits),
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
    operation: SemanticTypedBinaryKindAttr,
    scalar: SemanticTypedScalarV1,
    lhs: u64,
    rhs: u64,
) -> bool {
    if !scalar.is_integer() {
        return false;
    }
    if scalar.kind() == crate::SemanticScalarKindAttr::SignedInteger {
        let lhs = signed_value(lhs, scalar.bits());
        let rhs = signed_value(rhs, scalar.bits());
        let result = match operation {
            SemanticTypedBinaryKindAttr::Add => lhs.checked_add(rhs),
            SemanticTypedBinaryKindAttr::Subtract => lhs.checked_sub(rhs),
            SemanticTypedBinaryKindAttr::Multiply => lhs.checked_mul(rhs),
            _ => return false,
        };
        let minimum = -(1_i128 << (scalar.bits() - 1));
        let maximum = (1_i128 << (scalar.bits() - 1)) - 1;
        result.is_some_and(|result| (minimum..=maximum).contains(&result))
    } else {
        let result = match operation {
            SemanticTypedBinaryKindAttr::Add => u128::from(lhs).checked_add(u128::from(rhs)),
            SemanticTypedBinaryKindAttr::Subtract => u128::from(lhs).checked_sub(u128::from(rhs)),
            SemanticTypedBinaryKindAttr::Multiply => u128::from(lhs).checked_mul(u128::from(rhs)),
            _ => return false,
        };
        let maximum = (1_u128 << scalar.bits()) - 1;
        result.is_some_and(|result| result <= maximum)
    }
}

fn scalar_tag(scalar: SemanticTypedScalarV1) -> [u8; 4] {
    let width = scalar.bits().to_le_bytes();
    match scalar.kind() {
        crate::SemanticScalarKindAttr::Bool => [0, 0, 0, 0],
        crate::SemanticScalarKindAttr::UnsignedInteger => [1, 0, width[0], width[1]],
        crate::SemanticScalarKindAttr::SignedInteger => [1, 1, width[0], width[1]],
        crate::SemanticScalarKindAttr::Float => [2, 0, width[0], width[1]],
    }
}

fn hash_expression(digest: &mut Sha256, expression: &SemanticTypedExpressionV1) {
    match expression {
        SemanticTypedExpressionV1::Symbol { symbol, scalar } => {
            digest.update([0]);
            digest.update(scalar_tag(*scalar));
            digest.update(symbol.to_le_bytes());
        }
        SemanticTypedExpressionV1::Constant { scalar, bits } => {
            digest.update([1]);
            digest.update(scalar_tag(*scalar));
            digest.update(bits.to_le_bytes());
        }
        SemanticTypedExpressionV1::Unary {
            operation,
            scalar,
            operand,
        } => {
            digest.update([2, *operation as u8]);
            digest.update(scalar_tag(*scalar));
            hash_expression(digest, operand);
        }
        SemanticTypedExpressionV1::Binary {
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
        SemanticTypedExpressionV1::Compare {
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
        SemanticTypedExpressionV1::Select {
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
        SemanticTypedExpressionV1::Cast {
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
