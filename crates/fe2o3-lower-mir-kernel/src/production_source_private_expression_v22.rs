use fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1 as PrivateRvalueV22;

enum OriginalPrivateInputV22<'a> {
    Argument(u32),
    Operand {
        site: EntrySiteV20,
        role: EntryOperandV20,
        operand: &'a SemanticOperandV1,
    },
    Rvalue {
        block: u32,
        statement: u32,
        value: &'a PrivateRvalueV22,
    },
}

fn original_private_expression_headers_v22() -> Result<usize, ArgumentResourceV1> {
    let frame = argument_sum_v1(&[
        size_of::<OriginalPrivateInputV22<'_>>(),
        3 * size_of::<ProductionSemanticExpressionV2>(),
        size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>(),
        size_of::<OriginalEntryDefinitionRowV20>(),
        size_of::<ProductionSourceScalarArgumentV18<'_>>(),
        size_of::<ProductionSemanticScalarTypeV2>(),
        size_of::<ProductionSemanticBinaryOpV2>(),
        12 * size_of::<usize>(),
        8 * size_of::<&()>(),
    ])?;
    argument_product_v1(MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1, frame)
}

fn private_binary_v22(operation: SemanticBinaryOpV1) -> Option<ProductionSemanticBinaryOpV2> {
    use ProductionSemanticBinaryOpV2 as Out;
    Some(match operation {
        SemanticBinaryOpV1::Add => Out::Add,
        SemanticBinaryOpV1::Subtract => Out::Subtract,
        SemanticBinaryOpV1::Multiply => Out::Multiply,
        SemanticBinaryOpV1::BitAnd => Out::BitAnd,
        SemanticBinaryOpV1::BitOr => Out::BitOr,
        SemanticBinaryOpV1::BitXor => Out::BitXor,
        _ => return None,
    })
}

impl OriginalEntryIndexV20<'_, '_> {
    fn source_write_expression_v22(
        &self,
        leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
        request: &ProductionSourceEntryWriteV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.source.retain_query((|| {
            self.check(budget)?;
            let original = leaves.original_leaves(budget)?;
            let (instance, function) = request.original(budget)?;
            budget.charge_work(3)?;
            if !std::ptr::eq(original.leaves.relation, self.source)
                || !std::ptr::eq(request.leaves, leaves)
                || !request.source_writes
            {
                return self
                    .source
                    .source
                    .missing("private source expression changed retained context");
            }
            let scalar = request.scalar(budget)?;
            if !matches!(
                scalar,
                ProductionSemanticScalarTypeV2::Integer {
                    bits: 8 | 16 | 32 | 64,
                    ..
                }
            ) {
                return self
                    .source
                    .source
                    .missing("private source expression requires a fixed integer scalar");
            }
            let input = match request.source_value_v22(budget)? {
                ScopedMemoryStoreSourceV29::Assignment {
                    site: ExecutionSiteV29::Statement { block, statement },
                    ..
                } => {
                    let Some(SemanticStatementKindV1::Assign(assignment)) = function
                        .blocks()
                        .get(block.get() as usize)
                        .and_then(|b| b.statements().get(statement as usize))
                        .map(|s| s.kind())
                    else {
                        return self
                            .source
                            .source
                            .missing("private source expression original assignment absent");
                    };
                    OriginalPrivateInputV22::Rvalue {
                        block: block.get(),
                        statement,
                        value: assignment.value(),
                    }
                }
                ScopedMemoryStoreSourceV29::Operand { site, role, .. } => {
                    let operand = scoped_source_operand_v29(function, site, role).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "private source expression original operand absent",
                        ),
                    )?;
                    OriginalPrivateInputV22::Operand {
                        site,
                        role,
                        operand,
                    }
                }
                _ => {
                    return self
                        .source
                        .source
                        .missing("private source expression unsupported original input");
                }
            };
            let mut remaining = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
            self.private_expression_v22(
                original,
                instance,
                request.row.ty,
                scalar,
                input,
                0,
                &mut remaining,
                budget,
            )
        })())
    }

    fn private_expression_v22(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        input: OriginalPrivateInputV22<'_>,
        depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        budget.charge_work(8)?;
        if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || *remaining == 0 {
            return self
                .source
                .source
                .missing("private source expression exceeds existing expression bounds");
        }
        *remaining -= 1;
        let next = depth + 1;
        let function = leaves.original_function(instance, budget)?;
        let (function_id, _) = self
            .source
            .source
            .instance(leaves.leaves.root, instance, budget)?;
        match input {
            OriginalPrivateInputV22::Argument(argument) => {
                match leaves.original_argument(instance, function, argument, budget)? {
                    ProductionSourceScalarArgumentV18::Root { argument } => {
                        Ok(ProductionSemanticExpressionV2::Symbol {
                            symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                                .checked_add(argument)
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                            scalar,
                        })
                    }
                    ProductionSourceScalarArgumentV18::Caller {
                        instance: caller,
                        function: declaration,
                        block,
                        operand,
                    } => {
                        if caller >= instance
                            || !std::ptr::eq(leaves.original_function(caller, budget)?, declaration)
                        {
                            return self
                                .source
                                .source
                                .missing("private source expression caller differs");
                        }
                        self.private_expression_v22(
                            leaves,
                            caller,
                            ty,
                            scalar,
                            OriginalPrivateInputV22::Operand {
                                site: EntrySiteV20::Terminator {
                                    block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
                                },
                                role: EntryOperandV20::CallArgument(argument),
                                operand,
                            },
                            next,
                            remaining,
                            budget,
                        )
                    }
                }
            }
            OriginalPrivateInputV22::Operand {
                site,
                role,
                operand,
            } => {
                if semantic_operand_type(operand) != ty {
                    return self
                        .source
                        .source
                        .missing("private source expression operand type changed");
                }
                let place = match operand {
                    SemanticOperandV1::Constant(constant) => {
                        let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                            return self
                                .source
                                .source
                                .missing("private source expression non-scalar constant");
                        };
                        return Ok(ProductionSemanticExpressionV2::Constant {
                            scalar,
                            bits: u64::try_from(value.bits())
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        });
                    }
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
                };
                if let Some(expression) =
                    leaves.original_place(instance, function, place, budget)?
                {
                    if expression.scalar() != scalar {
                        return self
                            .source
                            .source
                            .missing("private source expression captured read type differs");
                    }
                    return Ok(expression);
                }
                if !place.projections().is_empty() {
                    return self
                        .source
                        .source
                        .missing("private source expression uncaptured projection");
                }
                let value =
                    self.promoted_use(function_id, site, role, place.local().index(), budget)?;
                let definition = self.definition(function_id, value, budget)?;
                if definition.local != place.local().index() {
                    return self
                        .source
                        .source
                        .missing("private source expression SSA local differs");
                }
                let input = match definition.origin {
                    OriginalEntryDefinitionV20::Argument(argument) => {
                        OriginalPrivateInputV22::Argument(argument)
                    }
                    OriginalEntryDefinitionV20::Assignment { block, statement } => {
                        let Some(SemanticStatementKindV1::Assign(assignment)) = function
                            .blocks()
                            .get(block as usize)
                            .and_then(|b| b.statements().get(statement as usize))
                            .map(|s| s.kind())
                        else {
                            return self
                                .source
                                .source
                                .missing("private source expression SSA assignment absent");
                        };
                        if assignment.destination().local().index() != definition.local
                            || !assignment.destination().projections().is_empty()
                            || assignment.destination().ty() != ty
                        {
                            return self
                                .source
                                .source
                                .missing("private source expression SSA destination differs");
                        }
                        OriginalPrivateInputV22::Rvalue {
                            block,
                            statement,
                            value: assignment.value(),
                        }
                    }
                };
                self.private_expression_v22(
                    leaves, instance, ty, scalar, input, next, remaining, budget,
                )
            }
            OriginalPrivateInputV22::Rvalue {
                block,
                statement,
                value,
            } => {
                if value.result_type() != ty {
                    return self
                        .source
                        .source
                        .missing("private source expression assignment type differs");
                }
                let site = EntrySiteV20::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(block),
                    statement,
                };
                match value.kind() {
                    SemanticRvalueKindV1::Use(operand) => self.private_expression_v22(
                        leaves,
                        instance,
                        ty,
                        scalar,
                        OriginalPrivateInputV22::Operand {
                            site,
                            role: EntryOperandV20::RvalueOperand(0),
                            operand,
                        },
                        next,
                        remaining,
                        budget,
                    ),
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    } => {
                        // The existing ordinary MIR lowering retains the value
                        // of integer checked operations as a wrapping scalar.
                        // CheckedBinary is a distinct tuple-producing variant
                        // and deliberately reaches the refusal arm below.
                        let operation = private_binary_v22(*operation).ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "private source expression unsupported arithmetic contract",
                            ),
                        )?;
                        budget.reserve_storage(argument_product_v1(
                            2,
                            size_of::<ProductionSemanticExpressionV2>(),
                        )?)?;
                        let lhs = self.private_expression_v22(
                            leaves,
                            instance,
                            ty,
                            scalar,
                            OriginalPrivateInputV22::Operand {
                                site,
                                role: EntryOperandV20::RvalueOperand(0),
                                operand: left,
                            },
                            next,
                            remaining,
                            budget,
                        )?;
                        let rhs = self.private_expression_v22(
                            leaves,
                            instance,
                            ty,
                            scalar,
                            OriginalPrivateInputV22::Operand {
                                site,
                                role: EntryOperandV20::RvalueOperand(1),
                                operand: right,
                            },
                            next,
                            remaining,
                            budget,
                        )?;
                        Ok(ProductionSemanticExpressionV2::Binary {
                            operation,
                            scalar,
                            overflow: ProductionOverflowContractV2::Wrapping,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        })
                    }
                    _ => self
                        .source
                        .source
                        .missing("private source expression unsupported original derivation"),
                }
            }
        }
    }
}

// Only the private write profile uses these total fixed-width bit-vector
// identities. In particular, checked arithmetic and floating values retain
// their operators. This runs after the existing evaluator's constant folding.
fn source_private_integer_identity_v22(
    expression: &mut NormalizedScalarExpressionV1,
    depth: usize,
    charge: &mut dyn CorrelationChargeV18,
) -> Option<()> {
    charge.charge_many(8)?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return None;
    }
    match expression {
        NormalizedScalarExpressionV1::Unary { operand, .. }
        | NormalizedScalarExpressionV1::Cast { operand, .. } => {
            source_private_integer_identity_v22(&mut operand.0[0], depth + 1, charge)?;
        }
        NormalizedScalarExpressionV1::Binary { lhs, rhs, .. }
        | NormalizedScalarExpressionV1::Compare { lhs, rhs, .. } => {
            source_private_integer_identity_v22(&mut lhs.0[0], depth + 1, charge)?;
            source_private_integer_identity_v22(&mut rhs.0[0], depth + 1, charge)?;
        }
        NormalizedScalarExpressionV1::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            source_private_integer_identity_v22(&mut condition.0[0], depth + 1, charge)?;
            source_private_integer_identity_v22(&mut when_true.0[0], depth + 1, charge)?;
            source_private_integer_identity_v22(&mut when_false.0[0], depth + 1, charge)?;
        }
        _ => (),
    }
    let NormalizedScalarExpressionV1::Binary {
        operation,
        scalar,
        overflow,
        lhs,
        rhs,
    } = expression
    else {
        return Some(());
    };
    let ProductionSemanticScalarTypeV2::Integer {
        bits: width @ (8 | 16 | 32 | 64),
        ..
    } = *scalar
    else {
        return Some(());
    };
    if *overflow != ProductionOverflowContractV2::Wrapping {
        return Some(());
    }
    let neutral = match *operation {
        ProductionSemanticBinaryOpV2::Add
        | ProductionSemanticBinaryOpV2::Subtract
        | ProductionSemanticBinaryOpV2::BitOr
        | ProductionSemanticBinaryOpV2::BitXor => 0,
        ProductionSemanticBinaryOpV2::Multiply => 1,
        ProductionSemanticBinaryOpV2::BitAnd => u64::MAX >> (64 - width),
        _ => return Some(()),
    };
    let constant = |value: &NormalizedScalarExpressionV1| {
        matches!(value,
        NormalizedScalarExpressionV1::Constant { scalar: ty, bits } if *ty == *scalar && *bits == neutral)
    };
    let right = constant(&rhs.0[0]);
    let left = *operation != ProductionSemanticBinaryOpV2::Subtract && constant(&lhs.0[0]);
    if !right && !left {
        return Some(());
    }
    let placeholder = NormalizedScalarExpressionV1::Constant {
        scalar: *scalar,
        bits: 0,
    };
    let NormalizedScalarExpressionV1::Binary { lhs, rhs, .. } =
        std::mem::replace(expression, placeholder)
    else {
        unreachable!()
    };
    let selected = if right { lhs } else { rhs };
    let [value] = *selected.0;
    *expression = value;
    Some(())
}
