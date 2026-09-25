//! Shared existing typed conversion; inert expressions, not admission.
use super::*;
use dialect_kernel::IndexBinaryKindAttr;

/// Maps the existing CPU scalar domain without authenticating its origin.
pub fn reference_scalar_v2(
    scalar: ReferenceScalarTypeV1,
) -> Option<ProductionSemanticScalarTypeV2> {
    Some(match scalar {
        ReferenceScalarTypeV1::Bool => ProductionSemanticScalarTypeV2::Bool,
        ReferenceScalarTypeV1::U8 => ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 8,
        },
        ReferenceScalarTypeV1::U16 => ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 16,
        },
        ReferenceScalarTypeV1::U32 => ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        ReferenceScalarTypeV1::U64 | ReferenceScalarTypeV1::Usize => {
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 64,
            }
        }
        ReferenceScalarTypeV1::I8 => ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 8,
        },
        ReferenceScalarTypeV1::I16 => ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 16,
        },
        ReferenceScalarTypeV1::I32 => ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 32,
        },
        ReferenceScalarTypeV1::I64 | ReferenceScalarTypeV1::Isize => {
            ProductionSemanticScalarTypeV2::Integer {
                signed: true,
                bits: 64,
            }
        }
        ReferenceScalarTypeV1::F32 => ProductionSemanticScalarTypeV2::Float { bits: 32 },
        ReferenceScalarTypeV1::F64 => ProductionSemanticScalarTypeV2::Float { bits: 64 },
    })
}

/// Borrowed GPU expressions and optional checked source/adjusted argument rows.
#[derive(Clone, Copy)]
pub struct ReferenceGpuLoadsV2<'a> {
    /// Exact ranked kernel used to resolve index definitions.
    pub kernel: &'a ProductionRankedKernelV1,
    /// Independently projected GPU value expression.
    pub expression: &'a ProductionSemanticExpressionV2,
    /// Existing checked source mapping; ordinary conversion uses `None`.
    pub arguments: Option<&'a [fe2o3_lower_mir_kernel::ProductionConditionalSourceArgumentV1]>,
}

impl ReferenceGpuLoadsV2<'_> {
    fn allocation_origin(self, source_argument: u32) -> Result<u64, ConditionalReferenceErrorV1> {
        let adjusted = if let Some(arguments) = self.arguments {
            let mut selected = None;
            for row in arguments
                .iter()
                .filter(|row| row.source_argument() == source_argument)
            {
                if selected.is_some_and(|previous| previous != row.adjusted_argument()) {
                    return Err(ConditionalReferenceErrorV1::UnsupportedReference(
                        "CPU source argument has conflicting checked adjusted arguments",
                    ));
                }
                selected = Some(row.adjusted_argument());
            }
            selected.ok_or(ConditionalReferenceErrorV1::UnsupportedReference(
                "CPU read has no checked source-to-adjusted argument correspondence",
            ))?
        } else {
            source_argument
        };
        Ok(u64::from(adjusted) + 1)
    }
}

/// Existing typed expression conversion, not a source or formula receipt.
/// The conditional caller prepays its work and temporary storage envelope.
pub fn reference_expression_inner_checked_v2(
    effect_ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    expected: ReferenceScalarTypeV1,
    gpu_loads: Option<ReferenceGpuLoadsV2<'_>>,
) -> Result<ProductionSemanticExpressionV2, ConditionalReferenceErrorV1> {
    let expression = reference_expression_inner_v2(effect_ir, expression, gpu_loads, 0)?;
    let expected =
        reference_scalar_v2(expected).ok_or(ConditionalReferenceErrorV1::UnsupportedReference(
            "reference output scalar is outside typed semantic refinement V2",
        ))?;
    if expression.scalar() != expected {
        return Err(ConditionalReferenceErrorV1::UnsupportedReference(
            "reference output RHS type disagrees with its logical ABI",
        ));
    }
    expression
        .validate()
        .map_err(ConditionalReferenceErrorV1::SemanticExpression)?;
    Ok(expression)
}

fn reference_expression_inner_v2(
    effect_ir: &ReferenceEffectIrV1,
    expression: &ReferenceEffectExpressionV1,
    gpu_loads: Option<ReferenceGpuLoadsV2<'_>>,
    depth: usize,
) -> Result<ProductionSemanticExpressionV2, ConditionalReferenceErrorV1> {
    if depth >= fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return Err(ConditionalReferenceErrorV1::UnsupportedReference(
            "reference RHS exceeds the typed semantic expression depth bound",
        ));
    }
    match expression {
        ReferenceEffectExpressionV1::PointCoordinate { axis } => {
            Ok(ProductionSemanticExpressionV2::Symbol {
                symbol: *axis,
                scalar: ProductionSemanticScalarTypeV2::Integer {
                    signed: false,
                    bits: 64,
                },
            })
        }
        ReferenceEffectExpressionV1::KernelScalarArgument { argument } => {
            let scalar = effect_ir
                .relations
                .iter()
                .find_map(|relation| match relation {
                    ReferenceArgumentRelationV1::ScalarInput {
                        argument: actual,
                        scalar,
                    } if actual == argument => reference_scalar_v2(*scalar),
                    _ => None,
                })
                .ok_or(ConditionalReferenceErrorV1::UnsupportedReference(
                    "reference RHS scalar argument has no exact logical ABI type",
                ))?;
            let symbol = crate::portable_reference_v1::kernel_scalar_symbol_v2(*argument).ok_or(
                ConditionalReferenceErrorV1::UnsupportedReference(
                    "reference scalar argument exceeds the reserved semantic symbol namespace",
                ),
            )?;
            Ok(ProductionSemanticExpressionV2::Symbol { symbol, scalar })
        }
        ReferenceEffectExpressionV1::InputLength { .. } => {
            Err(ConditionalReferenceErrorV1::UnsupportedReference(
                "reference slice length cannot be used as an opaque semantic value",
            ))
        }
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar { scalar, bits }) => {
            let scalar = reference_scalar_v2(*scalar).ok_or(
                ConditionalReferenceErrorV1::UnsupportedReference(
                    "reference RHS constant type is unsupported",
                ),
            )?;
            let bits = u64::try_from(*bits).map_err(|_| {
                ConditionalReferenceErrorV1::UnsupportedReference(
                    "reference RHS constant exceeds 64 bits",
                )
            })?;
            Ok(ProductionSemanticExpressionV2::Constant { scalar, bits })
        }
        ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::ZeroSized) => {
            Err(ConditionalReferenceErrorV1::UnsupportedReference(
                "reference RHS is a zero-sized value rather than a scalar",
            ))
        }
        ReferenceEffectExpressionV1::InputLoad {
            reference_argument,
            index,
        } => {
            let loads_context =
                gpu_loads.ok_or(ConditionalReferenceErrorV1::UnsupportedReference(
                    "safe reference load requires an independently projected GPU expression",
                ))?;
            let (argument, element) = effect_ir
                .relations
                .iter()
                .find_map(|relation| match relation {
                    ReferenceArgumentRelationV1::SharedSliceInput { argument, element }
                        if effect_ir
                            .reference_argument_for_kernel_argument_v1(*argument)
                            .is_ok_and(|exact| exact == *reference_argument) =>
                    {
                        Some((*argument, *element))
                    }
                    _ => None,
                })
                .ok_or(ConditionalReferenceErrorV1::UnsupportedReference(
                    "safe reference load base is not one exact shared-slice input",
                ))?;
            let scalar = reference_scalar_v2(element).ok_or(
                ConditionalReferenceErrorV1::UnsupportedReference(
                    "safe reference load element type is unsupported",
                ),
            )?;
            let allocation_origin = loads_context.allocation_origin(argument)?;
            let mut loads = Vec::new();
            collect_semantic_loads_v2(loads_context.expression, &mut loads);
            let mut matches = loads.into_iter().filter(|load| {
                load.scalar == scalar
                    && load.allocation_origin == allocation_origin
                    && load.indices.len() == 1
                    && gpu_index_expression_v2(loads_context.kernel, load.indices[0], 0)
                        .is_ok_and(|gpu_index| gpu_index == **index)
            });
            let Some(load) = matches.next() else {
                return Err(ConditionalReferenceErrorV1::UnsupportedReference(
                    "safe reference load has no exact ranked GPU read with matching input, type, and index",
                ));
            };
            if matches.next().is_some() {
                return Err(ConditionalReferenceErrorV1::UnsupportedReference(
                    "safe reference load matches multiple ranked GPU reads",
                ));
            }
            Ok(ProductionSemanticExpressionV2::Load(load.clone()))
        }
        ReferenceEffectExpressionV1::Binary {
            operation,
            lhs,
            rhs,
            checked,
        } => {
            let lhs = reference_expression_inner_v2(effect_ir, lhs, gpu_loads, depth + 1)?;
            let rhs = reference_expression_inner_v2(effect_ir, rhs, gpu_loads, depth + 1)?;
            let comparison = match operation {
                ReferenceBinaryOpV1::Equal => Some(ProductionSemanticComparisonV2::Equal),
                ReferenceBinaryOpV1::LessThan => Some(ProductionSemanticComparisonV2::LessThan),
                ReferenceBinaryOpV1::LessEqual => Some(ProductionSemanticComparisonV2::LessOrEqual),
                ReferenceBinaryOpV1::NotEqual => Some(ProductionSemanticComparisonV2::NotEqual),
                ReferenceBinaryOpV1::GreaterEqual => {
                    Some(ProductionSemanticComparisonV2::GreaterOrEqual)
                }
                ReferenceBinaryOpV1::GreaterThan => {
                    Some(ProductionSemanticComparisonV2::GreaterThan)
                }
                _ => None,
            };
            if let Some(operation) = comparison {
                return Ok(ProductionSemanticExpressionV2::Compare {
                    operation,
                    operand_scalar: lhs.scalar(),
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                });
            }
            let operation = match operation {
                ReferenceBinaryOpV1::Add => ProductionSemanticBinaryOpV2::Add,
                ReferenceBinaryOpV1::Subtract => ProductionSemanticBinaryOpV2::Subtract,
                ReferenceBinaryOpV1::Multiply => ProductionSemanticBinaryOpV2::Multiply,
                ReferenceBinaryOpV1::Divide => ProductionSemanticBinaryOpV2::Divide,
                ReferenceBinaryOpV1::Remainder => ProductionSemanticBinaryOpV2::Remainder,
                ReferenceBinaryOpV1::BitXor => ProductionSemanticBinaryOpV2::BitXor,
                ReferenceBinaryOpV1::BitAnd => ProductionSemanticBinaryOpV2::BitAnd,
                ReferenceBinaryOpV1::BitOr => ProductionSemanticBinaryOpV2::BitOr,
                ReferenceBinaryOpV1::ShiftLeft => ProductionSemanticBinaryOpV2::ShiftLeft,
                ReferenceBinaryOpV1::ShiftRight => ProductionSemanticBinaryOpV2::ShiftRight,
                ReferenceBinaryOpV1::Equal
                | ReferenceBinaryOpV1::LessThan
                | ReferenceBinaryOpV1::LessEqual
                | ReferenceBinaryOpV1::NotEqual
                | ReferenceBinaryOpV1::GreaterEqual
                | ReferenceBinaryOpV1::GreaterThan => unreachable!(),
            };
            Ok(ProductionSemanticExpressionV2::Binary {
                operation,
                scalar: lhs.scalar(),
                overflow: if *checked {
                    ProductionOverflowContractV2::Checked
                } else {
                    ProductionOverflowContractV2::Wrapping
                },
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        ReferenceEffectExpressionV1::Unary { operation, operand } => {
            let operand = reference_expression_inner_v2(effect_ir, operand, gpu_loads, depth + 1)?;
            Ok(ProductionSemanticExpressionV2::Unary {
                operation: match operation {
                    ReferenceUnaryOpV1::Not => ProductionSemanticUnaryOpV2::Not,
                    ReferenceUnaryOpV1::Negate => ProductionSemanticUnaryOpV2::Negate,
                },
                scalar: operand.scalar(),
                operand: Box::new(operand),
            })
        }
        ReferenceEffectExpressionV1::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            let source = reference_scalar_v2(*source).ok_or(
                ConditionalReferenceErrorV1::UnsupportedReference(
                    "reference cast source type is unsupported",
                ),
            )?;
            let target = reference_scalar_v2(*target).ok_or(
                ConditionalReferenceErrorV1::UnsupportedReference(
                    "reference cast target type is unsupported",
                ),
            )?;
            let kind = match kind {
                ReferenceCastKindV1::Integer => ProductionSemanticCastV2::Integer,
                ReferenceCastKindV1::IntegerToFloat => ProductionSemanticCastV2::IntegerToFloat,
                ReferenceCastKindV1::FloatToFloat => ProductionSemanticCastV2::FloatToFloat,
                ReferenceCastKindV1::FloatToIntegerSaturating => {
                    ProductionSemanticCastV2::FloatToIntegerSaturating
                }
            };
            Ok(ProductionSemanticExpressionV2::Cast {
                kind,
                source,
                target,
                operand: Box::new(reference_expression_inner_v2(
                    effect_ir,
                    operand,
                    gpu_loads,
                    depth + 1,
                )?),
            })
        }
    }
}

/// Collects existing ordered expression-load occurrences without deduplication.
pub fn collect_semantic_loads_v2<'a>(
    expression: &'a ProductionSemanticExpressionV2,
    loads: &mut Vec<&'a fe2o3_pliron::ProductionSemanticLoadV2>,
) {
    match expression {
        ProductionSemanticExpressionV2::Load(load) => loads.push(load),
        ProductionSemanticExpressionV2::Symbol { .. }
        | ProductionSemanticExpressionV2::Constant { .. } => {}
        ProductionSemanticExpressionV2::Unary { operand, .. }
        | ProductionSemanticExpressionV2::Cast { operand, .. } => {
            collect_semantic_loads_v2(operand, loads);
        }
        ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }
        | ProductionSemanticExpressionV2::Compare { lhs, rhs, .. } => {
            collect_semantic_loads_v2(lhs, loads);
            collect_semantic_loads_v2(rhs, loads);
        }
        ProductionSemanticExpressionV2::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            collect_semantic_loads_v2(condition, loads);
            collect_semantic_loads_v2(when_true, loads);
            collect_semantic_loads_v2(when_false, loads);
        }
    }
}

/// Normalizes an existing ranked index in the unchanged bounded scalar domain.
pub fn gpu_index_expression_v2(
    kernel: &ProductionRankedKernelV1,
    value: ProductionRankedValueV1,
    depth: usize,
) -> Result<ReferenceEffectExpressionV1, ConditionalReferenceErrorV1> {
    if depth >= 64 {
        return Err(ConditionalReferenceErrorV1::UnsupportedGpuIndex(
            "GPU index expression exceeds the bounded normalization depth",
        ));
    }
    match value {
        ProductionRankedValueV1::Argument(argument) => {
            Ok(ReferenceEffectExpressionV1::KernelScalarArgument { argument })
        }
        ProductionRankedValueV1::BlockArgument { .. } => {
            Err(ConditionalReferenceErrorV1::UnsupportedGpuIndex(
                "GPU output coordinate depends on a block argument",
            ))
        }
        ProductionRankedValueV1::Local(identity) => {
            let mut definitions = kernel
                .blocks()
                .iter()
                .flat_map(|block| block.operations())
                .filter(|operation| operation_result_v2(operation) == Some(identity));
            let definition =
                definitions
                    .next()
                    .ok_or(ConditionalReferenceErrorV1::UnsupportedGpuIndex(
                        "GPU output coordinate has no ranked definition",
                    ))?;
            if definitions.next().is_some() {
                return Err(ConditionalReferenceErrorV1::UnsupportedGpuIndex(
                    "GPU output coordinate has multiple ranked definitions",
                ));
            }
            match definition {
                ProductionRankedOperationV1::InvocationIndex { dimension, .. } => {
                    Ok(ReferenceEffectExpressionV1::PointCoordinate { axis: *dimension })
                }
                ProductionRankedOperationV1::IndexConstant { value, .. } => Ok(
                    ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                        scalar: ReferenceScalarTypeV1::Usize,
                        bits: u128::from(*value),
                    }),
                ),
                ProductionRankedOperationV1::IndexBinary { kind, lhs, rhs, .. } => {
                    let operation = match kind {
                        IndexBinaryKindAttr::Add => ReferenceBinaryOpV1::Add,
                        IndexBinaryKindAttr::Multiply => ReferenceBinaryOpV1::Multiply,
                        IndexBinaryKindAttr::Remainder => ReferenceBinaryOpV1::Remainder,
                        IndexBinaryKindAttr::Divide => ReferenceBinaryOpV1::Divide,
                    };
                    Ok(ReferenceEffectExpressionV1::Binary {
                        operation,
                        lhs: Box::new(gpu_index_expression_v2(kernel, *lhs, depth + 1)?),
                        rhs: Box::new(gpu_index_expression_v2(kernel, *rhs, depth + 1)?),
                        checked: false,
                    })
                }
                _ => Err(ConditionalReferenceErrorV1::UnsupportedGpuIndex(
                    "GPU output coordinate uses an unsupported ranked definition",
                )),
            }
        }
    }
}

/// Returns the same scalar/index result roster used by live normalization.
pub fn operation_result_v2(
    operation: &ProductionRankedOperationV1,
) -> Option<ProductionRankedValueIdV1> {
    match operation {
        ProductionRankedOperationV1::IndexConstant { result, .. }
        | ProductionRankedOperationV1::IndexUnknown { result }
        | ProductionRankedOperationV1::InvocationIndex { result, .. }
        | ProductionRankedOperationV1::IndexBinary { result, .. }
        | ProductionRankedOperationV1::DeterministicJoin { result, .. }
        | ProductionRankedOperationV1::CheckedTiledIndex2D { result, .. }
        | ProductionRankedOperationV1::CheckedRowStripedIndex2D { result, .. }
        | ProductionRankedOperationV1::Dimension { result, .. }
        | ProductionRankedOperationV1::SemanticConstant { result, .. }
        | ProductionRankedOperationV1::SemanticSymbol { result, .. }
        | ProductionRankedOperationV1::SemanticExpression { result, .. } => Some(*result),
        _ => None,
    }
}
