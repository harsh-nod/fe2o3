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
        original_shared_capture_headers_v26()?,
        size_of::<OriginalPrivateInputV22<'_>>(),
        3 * size_of::<ProductionSemanticExpressionV2>(),
        size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>(),
        size_of::<OriginalEntryDefinitionRowV20>(),
        size_of::<ProductionSourceScalarArgumentV18<'_>>(),
        size_of::<ProductionSemanticScalarTypeV2>(),
        size_of::<ProductionSemanticBinaryOpV2>(),
        size_of::<SemanticUnaryOpV1>(),
        size_of::<fe2o3_pliron::ProductionSemanticUnaryOpV2>(),
        size_of::<Option<fe2o3_pliron::ProductionSemanticUnaryOpV2>>(),
        size_of::<Type>(),
        size_of::<Result<Type, ProductionSemanticKirErrorV1>>(),
        12 * size_of::<usize>(),
        8 * size_of::<&()>(),
    ])?;
    argument_sum_v1(&[
        argument_product_v1(MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1, frame)?,
        issued_discriminant_query_headers_v31()?,
        source_call_return_headers_v32()?,
        source_helper_expression_headers_v33()?,
    ])
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

fn private_unary_v39(
    operation: SemanticUnaryOpV1,
    scalar: ProductionSemanticScalarTypeV2,
) -> Option<fe2o3_pliron::ProductionSemanticUnaryOpV2> {
    use ProductionSemanticScalarTypeV2 as Scalar;
    use fe2o3_pliron::ProductionSemanticUnaryOpV2 as Out;
    match (operation, scalar) {
        (
            SemanticUnaryOpV1::Not,
            Scalar::Bool
            | Scalar::Integer {
                bits: 8 | 16 | 32 | 64,
                ..
            },
        ) => Some(Out::Not),
        (
            SemanticUnaryOpV1::Negate,
            Scalar::Integer {
                signed: true,
                bits: 8 | 16 | 32 | 64,
            }
            | Scalar::Float { bits: 32 | 64 },
        ) => Some(Out::Negate),
        _ => None,
    }
}

fn store_source_expression_headers_v23() -> Result<usize, ArgumentResourceV1> {
    // Query arguments, parsed original input, and the returned expression stay
    // live together. The actual query closure is measured at the call site.
    argument_sum_v1(&[
        6 * size_of::<&()>(),
        size_of::<ScopedMemoryStoreSourceV29>(),
        size_of::<OriginalPrivateInputV22<'_>>(),
        size_of::<ProductionSourceScalarInputV18<'_>>(),
        size_of::<SourceOwnedResultV18<ProductionSourceScalarInputV18<'_>>>(),
        size_of::<SourceOwnedResultV18<(usize, &SemanticFunctionDeclV1)>>(),
        size_of::<SourceOwnedResultV18<&ProductionSourceScalarLeavesV18<'_>>>(),
        size_of::<ProductionSemanticScalarTypeV2>(),
        size_of::<SourceOwnedResultV18<ProductionSemanticScalarTypeV2>>(),
        size_of::<SemanticTypeIdV1>(),
        size_of::<ProductionSemanticExpressionV2>(),
        size_of::<SourceOwnedResultV18<ProductionSemanticExpressionV2>>(),
        2 * size_of::<SourceOwnedResultV18<()>>(),
        4 * size_of::<usize>(),
    ])
}

impl OriginalEntryIndexV20<'_, '_> {
    fn store_source_expression_v23(
        &self,
        leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
        request: &ProductionOptimizedSourceScalarStoreV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.check(budget)?;
        let query = |budget: &mut ArgumentBudgetV1<'_>| {
            let original = leaves.original_leaves(budget)?;
            let (instance, function) = request.original(budget)?;
            budget.charge_work(3)?;
            if !std::ptr::eq(original.leaves.relation, self.source)
                || !std::ptr::eq(request.leaves, leaves)
            {
                return self
                    .source
                    .source
                    .missing("global source expression changed retained context");
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
                    .missing("global source expression requires a fixed integer scalar");
            }
            let (ty, input) = match request.original.source {
                ScopedMemoryStoreSourceV29::Operand { site, role, ty, .. } => {
                    let operand = scoped_source_operand_v29(function, site, role).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "global source expression original operand absent",
                        ),
                    )?;
                    (
                        ty,
                        OriginalPrivateInputV22::Operand {
                            site,
                            role,
                            operand,
                        },
                    )
                }
                ScopedMemoryStoreSourceV29::Assignment {
                    site: ExecutionSiteV29::Statement { block, statement },
                    ty,
                } => {
                    let Some(SemanticStatementKindV1::Assign(assignment)) = function
                        .blocks()
                        .get(block.get() as usize)
                        .and_then(|row| row.statements().get(statement as usize))
                        .map(|row| row.kind())
                    else {
                        return self
                            .source
                            .source
                            .missing("global source expression original assignment absent");
                    };
                    (
                        ty,
                        OriginalPrivateInputV22::Rvalue {
                            block: block.get(),
                            statement,
                            value: assignment.value(),
                        },
                    )
                }
                ScopedMemoryStoreSourceV29::EntryArgument { ty, .. } => {
                    let ProductionSourceScalarInputV18::EntryArgument { argument } =
                        request.input_for(function, budget)?
                    else {
                        return self
                            .source
                            .source
                            .missing("source Store entry argument changed its original input");
                    };
                    (ty, OriginalPrivateInputV22::Argument(argument))
                }
                _ => {
                    return self
                        .source
                        .source
                        .missing("global source expression unsupported original input");
                }
            };
            let mut remaining = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
            // Every symbol follows an original occurrence or caller edge;
            // candidate definitions never supply original-source facts.
            self.private_expression_v22(
                original,
                instance,
                ty,
                scalar,
                input,
                0,
                &mut remaining,
                budget,
            )
        };
        let headers = self.source.retain_query(
            store_source_expression_headers_v23()
                .and_then(|fixed| argument_sum_v1(&[fixed, std::mem::size_of_val(&query)]))
                .map_err(Into::into),
        )?;
        self.source
            .retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
        // The caller's existing expression scratch owns these headers and all
        // Box backing until after it has dropped the returned tree.
        self.source.retain_query(query(budget))
    }

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
                    if matches!(place.projections(), [projection]
                        if matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
                    {
                        return self.checked_component_expression_v41(
                            leaves, instance, ty, scalar, site, role, place, next, remaining,
                            budget,
                        );
                    }
                    return self.captured_shared_reference_expression_v26(
                        leaves, instance, ty, scalar, site, role, place, next, remaining, budget,
                    );
                }
                let value =
                    self.promoted_use(function_id, site, role, place.local().index(), budget)?;
                if let EntryValueV20::BlockArgument { block, variable } = value {
                    return leaves
                        .boundary_expression_v31(instance, block, variable, ty, scalar, budget);
                }
                let definition = self.definition(function_id, value, budget)?;
                if definition.local != place.local().index() {
                    return self
                        .source
                        .source
                        .missing("private source expression SSA local differs");
                }
                let input = match definition.origin {
                    OriginalEntryDefinitionV20::CallReturn { block, edge } => {
                        return self.private_call_expression_v33(
                            leaves,
                            instance,
                            function_id,
                            definition,
                            value,
                            block,
                            edge,
                            ty,
                            scalar,
                            next,
                            remaining,
                            budget,
                        );
                    }
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
                    SemanticRvalueKindV1::Discriminant(place)
                        if !leaves.leaves.presences.rows.is_empty() =>
                    {
                        self.issued_discriminant_v31(
                            leaves,
                            instance,
                            function_id,
                            function,
                            site,
                            value,
                            place,
                            ty,
                            scalar,
                            next,
                            remaining,
                            budget,
                        )
                    }
                    SemanticRvalueKindV1::Load(load)
                        if load.volatility() == SemanticVolatilityV1::NonVolatile =>
                    {
                        let expression = leaves
                            .original_place(instance, function, load.source(), budget)?
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "source expression explicit read lacks its captured occurrence",
                            ))?;
                        if expression.scalar() != scalar {
                            return self
                                .source
                                .source
                                .missing("source expression explicit read scalar differs");
                        }
                        Ok(expression)
                    }
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
                    } if lower_compare(*operation).is_some() => {
                        let source = self.source.source.source_semantic(budget)?;
                        let operand_ty = semantic_operand_type(left);
                        let operand_scalar = kir_semantic_scalar_v1(
                            &lower_scalar_type(source.types(), operand_ty)
                                .map_err(source_emission_error_v18)?,
                        )
                        .filter(|scalar| {
                            matches!(
                                scalar,
                                ProductionSemanticScalarTypeV2::Bool
                                    | ProductionSemanticScalarTypeV2::Integer {
                                        bits: 8 | 16 | 32 | 64,
                                        ..
                                    }
                            )
                        })
                        .ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "source SSA comparison operand type is unsupported",
                            ),
                        )?;
                        if scalar != ProductionSemanticScalarTypeV2::Bool
                            || semantic_operand_type(right) != operand_ty
                        {
                            return self
                                .source
                                .source
                                .missing("source SSA comparison types differ");
                        }
                        budget.reserve_storage(argument_product_v1(
                            2,
                            size_of::<ProductionSemanticExpressionV2>(),
                        )?)?;
                        let lhs = self.private_expression_v22(
                            leaves,
                            instance,
                            operand_ty,
                            operand_scalar,
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
                            operand_ty,
                            operand_scalar,
                            OriginalPrivateInputV22::Operand {
                                site,
                                role: EntryOperandV20::RvalueOperand(1),
                                operand: right,
                            },
                            next,
                            remaining,
                            budget,
                        )?;
                        Ok(ProductionSemanticExpressionV2::Compare {
                            operation: normalize_kir_comparison_v1(
                                lower_compare(*operation).ok_or(
                                    ProductionSourceOwnedViewErrorV18::Binding(
                                        "source SSA comparison operator differs",
                                    ),
                                )?,
                            ),
                            operand_scalar,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        })
                    }
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
                    SemanticRvalueKindV1::Unary { operation, operand } => {
                        if *operation == SemanticUnaryOpV1::PointerMetadata {
                            return leaves.leaves.descriptor_length_expression_v40(
                                instance, block, statement, value, scalar, budget,
                            );
                        }
                        budget.charge_work(6)?;
                        let source = self.source.source.source_semantic(budget)?;
                        if !function.blocks().get(block as usize)
                            .and_then(|row| row.statements().get(statement as usize))
                            .is_some_and(|row| matches!(row.kind(), SemanticStatementKindV1::Assign(assignment)
                                if std::ptr::eq(assignment.value(), value)))
                            || semantic_operand_type(operand) != ty
                            || kir_semantic_scalar_v1(&lower_scalar_type(source.types(), ty)
                                .map_err(source_emission_error_v18)?) != Some(scalar)
                        {
                            return self.source.source.missing(
                                "private source unary expression owner or scalar type differs",
                            );
                        }
                        let operation = private_unary_v39(*operation, scalar).ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "private source unary expression operator or scalar type unsupported",
                            ),
                        )?;
                        budget.reserve_storage(size_of::<ProductionSemanticExpressionV2>())?;
                        let operand = self.private_expression_v22(
                            leaves, instance, ty, scalar,
                            OriginalPrivateInputV22::Operand {
                                site, role: EntryOperandV20::RvalueOperand(0), operand,
                            },
                            next, remaining, budget,
                        )?;
                        // Preserve the actual operator. Overflow/assert control
                        // remains a separate original-source obligation.
                        Ok(ProductionSemanticExpressionV2::Unary {
                            operation, scalar, operand: Box::new(operand),
                        })
                    }
                    SemanticRvalueKindV1::Cast { .. } => self.source.source.missing(
                        "private source expression unsupported original cast derivation",
                    ),
                    SemanticRvalueKindV1::CheckedBinary(_) => self.source.source.missing(
                        "private source expression unsupported original checked-binary derivation",
                    ),
                    SemanticRvalueKindV1::UncheckedBinary(_) => self.source.source.missing(
                        "private source expression unsupported original unchecked-binary derivation",
                    ),
                    SemanticRvalueKindV1::Borrow { .. } => self.source.source.missing(
                        "private source expression unsupported original borrow derivation",
                    ),
                    SemanticRvalueKindV1::AddressOf { .. } => self.source.source.missing(
                        "private source expression unsupported original address-of derivation",
                    ),
                    SemanticRvalueKindV1::Length(_) => leaves.leaves.descriptor_length_expression_v40(
                        instance, block, statement, value, scalar, budget,
                    ),
                    SemanticRvalueKindV1::Discriminant(_) => self.source.source.missing(
                        "private source expression unsupported original discriminant derivation",
                    ),
                    SemanticRvalueKindV1::Aggregate(_) => self.source.source.missing(
                        "private source expression unsupported original aggregate derivation",
                    ),
                    SemanticRvalueKindV1::Load(_) => self.source.source.missing(
                        "private source expression unsupported original volatile-load derivation",
                    ),
                }
            }
        }
    }
}

include!("production_source_helper_expression_v33.rs");
include!("production_source_private_shared_capture_v26.rs");
include!("production_source_checked_components_v41.rs");

// Only the private write profile uses these total fixed-width bit-vector
// identities. Checked arithmetic permits only neutral operations that cannot
// overflow at any input. This normalizes the value expression, not the separate
// overflow result or its control effects. Floating operators remain unchanged.
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
    match *overflow {
        ProductionOverflowContractV2::Wrapping => (),
        ProductionOverflowContractV2::Checked => {
            if !matches!(
                *operation,
                ProductionSemanticBinaryOpV2::Add
                    | ProductionSemanticBinaryOpV2::Subtract
                    | ProductionSemanticBinaryOpV2::Multiply
            ) {
                return Some(());
            }
        }
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
