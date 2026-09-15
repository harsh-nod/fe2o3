use super::*;
use dialect_kernel::{
    MAX_SEMANTIC_TYPED_EXPRESSION_DEPTH_V1, MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1,
    SemanticExceptionalValueAttr, SemanticIeeeRoundingAttr, SemanticNumericalPolicyAttr,
    SemanticOverflowAttr, SemanticScalarKindAttr, SemanticTypedBinaryKindAttr,
    SemanticTypedCastKindAttr, SemanticTypedCompareKindAttr, SemanticTypedScalarV1,
    SemanticTypedUnaryKindAttr,
};
use fe2o3_pliron::{
    PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2, PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2,
    ProductionIeeeExceptionalValuePolicyV2, ProductionIeeeRoundingModeV2,
    ProductionNumericalContractV2, ProductionOverflowContractV2, ProductionSemanticBinaryOpV2,
    ProductionSemanticCastV2, ProductionSemanticComparisonV2, ProductionSemanticExpressionV2,
    ProductionSemanticScalarTypeV2, ProductionSemanticUnaryOpV2,
};

pub(super) fn numerical(
    contract: ProductionNumericalContractV2,
) -> Result<SemanticNumericalContractV1, Failure> {
    let policy = match contract {
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence => {
            SemanticNumericalPolicyAttr::ExactBitVectorOperatorCongruence
        }
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding: ProductionIeeeRoundingModeV2::NearestTiesToEven,
            exceptional_values: ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
        } => SemanticNumericalPolicyAttr::ExactIeeeNearestTiesToEvenPreserveBits,
        _ => return Err(Failure::UnsupportedNumericalPolicy),
    };
    Ok(SemanticNumericalContractV1 {
        policy,
        rounding: SemanticIeeeRoundingAttr::NearestTiesToEven,
        exceptional_values: SemanticExceptionalValueAttr::PreserveExactBits,
    })
}

fn scalar(value: ProductionSemanticScalarTypeV2) -> Result<SemanticTypedScalarV1, Failure> {
    let kind = match value {
        ProductionSemanticScalarTypeV2::Bool => SemanticScalarKindAttr::Bool,
        ProductionSemanticScalarTypeV2::Integer { signed: true, .. } => {
            SemanticScalarKindAttr::SignedInteger
        }
        ProductionSemanticScalarTypeV2::Integer { signed: false, .. } => {
            SemanticScalarKindAttr::UnsignedInteger
        }
        ProductionSemanticScalarTypeV2::Float { .. } => SemanticScalarKindAttr::Float,
    };
    SemanticTypedScalarV1::new(kind, value.bit_width()).ok_or(Failure::InvalidTypedExpression)
}

/// Only CPU roots from the revalidated source export enter here. Symbols are
/// renamed through actual source parameters, never by final SSA coincidence.
pub(super) fn reference_rhs(
    source: &ProductionSemanticExpressionV2,
    contract: ProductionNumericalContractV2,
    parameters: &[FinalWriteParameterBindingV1],
) -> Result<SemanticTypedExpressionV1, Failure> {
    let mut remaining = MAX_SEMANTIC_TYPED_EXPRESSION_NODES_V1;
    let expression = convert(source, parameters, 1, &mut remaining)?;
    expression
        .validate()
        .map_err(|_| Failure::InvalidTypedExpression)?;
    expression
        .validate_static_domains()
        .map_err(|_| Failure::InvalidTypedExpression)?;
    numerical(contract)?
        .validate(&expression)
        .map_err(|_| Failure::UnsupportedNumericalPolicy)?;
    Ok(expression)
}

fn convert(
    expression: &ProductionSemanticExpressionV2,
    parameters: &[FinalWriteParameterBindingV1],
    depth: usize,
    remaining: &mut usize,
) -> Result<SemanticTypedExpressionV1, Failure> {
    if depth > MAX_SEMANTIC_TYPED_EXPRESSION_DEPTH_V1 || *remaining == 0 {
        return Err(Failure::ExpressionLimit);
    }
    *remaining -= 1;
    use ProductionSemanticExpressionV2 as P;
    use SemanticTypedExpressionV1 as T;
    let mut child = |value| convert(value, parameters, depth + 1, remaining).map(Box::new);
    Ok(match expression {
        P::Load(_) => return Err(Failure::UnsupportedLoad),
        P::Symbol { symbol, scalar: ty } => {
            if *symbol >= PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2 {
                return Err(Failure::UnsupportedLoad);
            }
            let argument = symbol
                .checked_sub(PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2)
                .ok_or(Failure::UnboundScalarSymbol(*symbol))?;
            let mut matches = parameters.iter().filter(|p| p.source_argument == argument);
            let binding = matches
                .next()
                .ok_or(Failure::UnboundScalarSymbol(*symbol))?;
            // The canonical scalar projection currently admits f32 parameters
            // only. Integer constants/operations still retain their own types.
            if matches.next().is_some()
                || binding.scalar_type != Type::F32
                || *ty != (ProductionSemanticScalarTypeV2::Float { bits: 32 })
            {
                return Err(Failure::UnboundScalarSymbol(*symbol));
            }
            T::Symbol {
                symbol: binding.final_ordinal,
                scalar: scalar(*ty)?,
            }
        }
        P::Constant { scalar: ty, bits } => T::Constant {
            scalar: scalar(*ty)?,
            bits: *bits,
        },
        P::Unary {
            operation,
            scalar: ty,
            operand,
        } => T::Unary {
            operation: match operation {
                ProductionSemanticUnaryOpV2::Not => SemanticTypedUnaryKindAttr::Not,
                ProductionSemanticUnaryOpV2::Negate => SemanticTypedUnaryKindAttr::Negate,
            },
            scalar: scalar(*ty)?,
            operand: child(operand)?,
        },
        P::Binary {
            operation,
            scalar: ty,
            overflow,
            lhs,
            rhs,
        } => T::Binary {
            operation: match operation {
                ProductionSemanticBinaryOpV2::Add => SemanticTypedBinaryKindAttr::Add,
                ProductionSemanticBinaryOpV2::Subtract => SemanticTypedBinaryKindAttr::Subtract,
                ProductionSemanticBinaryOpV2::Multiply => SemanticTypedBinaryKindAttr::Multiply,
                ProductionSemanticBinaryOpV2::Divide => SemanticTypedBinaryKindAttr::Divide,
                ProductionSemanticBinaryOpV2::Remainder => SemanticTypedBinaryKindAttr::Remainder,
                ProductionSemanticBinaryOpV2::BitXor => SemanticTypedBinaryKindAttr::BitXor,
                ProductionSemanticBinaryOpV2::BitAnd => SemanticTypedBinaryKindAttr::BitAnd,
                ProductionSemanticBinaryOpV2::BitOr => SemanticTypedBinaryKindAttr::BitOr,
                ProductionSemanticBinaryOpV2::ShiftLeft => SemanticTypedBinaryKindAttr::ShiftLeft,
                ProductionSemanticBinaryOpV2::ShiftRight => SemanticTypedBinaryKindAttr::ShiftRight,
            },
            scalar: scalar(*ty)?,
            overflow: match overflow {
                ProductionOverflowContractV2::Wrapping => SemanticOverflowAttr::Wrapping,
                ProductionOverflowContractV2::Checked => SemanticOverflowAttr::Checked,
            },
            lhs: child(lhs)?,
            rhs: child(rhs)?,
        },
        P::Compare {
            operation,
            operand_scalar,
            lhs,
            rhs,
        } => T::Compare {
            operation: match operation {
                ProductionSemanticComparisonV2::Equal => SemanticTypedCompareKindAttr::Equal,
                ProductionSemanticComparisonV2::NotEqual => SemanticTypedCompareKindAttr::NotEqual,
                ProductionSemanticComparisonV2::LessThan => SemanticTypedCompareKindAttr::LessThan,
                ProductionSemanticComparisonV2::LessOrEqual => {
                    SemanticTypedCompareKindAttr::LessOrEqual
                }
                ProductionSemanticComparisonV2::GreaterThan => {
                    SemanticTypedCompareKindAttr::GreaterThan
                }
                ProductionSemanticComparisonV2::GreaterOrEqual => {
                    SemanticTypedCompareKindAttr::GreaterOrEqual
                }
            },
            operand_scalar: scalar(*operand_scalar)?,
            lhs: child(lhs)?,
            rhs: child(rhs)?,
        },
        P::Select {
            scalar: ty,
            condition,
            when_true,
            when_false,
        } => T::Select {
            scalar: scalar(*ty)?,
            condition: child(condition)?,
            when_true: child(when_true)?,
            when_false: child(when_false)?,
        },
        P::Cast {
            kind,
            source,
            target,
            operand,
        } => T::Cast {
            kind: match kind {
                ProductionSemanticCastV2::Integer => SemanticTypedCastKindAttr::Integer,
                ProductionSemanticCastV2::IntegerToFloat => {
                    SemanticTypedCastKindAttr::IntegerToFloat
                }
                ProductionSemanticCastV2::FloatToFloat => SemanticTypedCastKindAttr::FloatToFloat,
                ProductionSemanticCastV2::FloatToIntegerSaturating => {
                    SemanticTypedCastKindAttr::FloatToIntegerSaturating
                }
            },
            source: scalar(*source)?,
            target: scalar(*target)?,
            operand: child(operand)?,
        },
    })
}
