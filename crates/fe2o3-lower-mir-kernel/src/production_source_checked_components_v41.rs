fn source_checked_component_headers_v41() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        &'a OriginalEntryIndexV20<'a, 'a>,
        &'a ProductionSourceScalarLeavesV18<'a>,
        &'a SemanticFunctionDeclV1,
        &'a SemanticPlaceV1,
        &'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        [&'a SemanticOperandV1; 2],
        [&'a SemanticTypeDeclV1; 3],
        &'a [SemanticTypeIdV1],
        OriginalEntryDefinitionRowV20,
        EntryValueV20,
        EntrySiteV20,
        EntryOperandV20,
        [ProductionSourceSsaEndpointV36<'a, 'a>; 3],
        [Option<usize>; 2],
        [SourceOwnedResultV18<Option<usize>>; 2],
        [fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1; 2],
        [usize; 8],
        [SemanticTypeIdV1; 3],
        CheckedBinaryOperator,
        ProductionSemanticScalarTypeV2,
        ProductionSemanticScalarTypeV2,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        [&'a (); 6],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        checked_arithmetic_headers_v41::<ProductionSemanticExpressionV2>(),
    ])
}

fn source_checked_operator_v41(operation: SemanticCheckedBinaryOpV1) -> CheckedBinaryOperator {
    match operation {
        SemanticCheckedBinaryOpV1::Add => CheckedBinaryOperator::Add,
        SemanticCheckedBinaryOpV1::Subtract => CheckedBinaryOperator::Subtract,
        SemanticCheckedBinaryOpV1::Multiply => CheckedBinaryOperator::Multiply,
    }
}

impl OriginalEntryIndexV20<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn checked_component_expression_v41(
        &self,
        leaves: &ProductionSourceScalarLeavesV18<'_>,
        instance: usize,
        ty: SemanticTypeIdV1,
        scalar: ProductionSemanticScalarTypeV2,
        mut site: EntrySiteV20,
        mut role: EntryOperandV20,
        mut place: &SemanticPlaceV1,
        mut depth: usize,
        remaining: &mut usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSemanticExpressionV2> {
        self.check(budget)?;
        budget.reserve_storage(source_checked_component_headers_v41()?)?;
        let function = leaves.original_function(instance, budget)?;
        let (function_id, _) = self
            .source
            .source
            .instance(leaves.leaves.root, instance, budget)?;
        let semantic = self.source.source.source_semantic(budget)?;
        let [projection] = place.projections() else {
            return self
                .source
                .source
                .missing("checked source component requires one original tuple field");
        };
        let SemanticProjectionKindV1::Field(field @ (0 | 1)) = projection.kind() else {
            return self
                .source
                .source
                .missing("checked source component field is not a value or overflow result");
        };
        if projection.result_type() != ty || place.ty() != ty {
            return self
                .source
                .source
                .missing("checked source component result type differs");
        }
        let tuple_ty = function
            .locals()
            .get(place.local().index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "checked source holder absent",
            ))?
            .ty();
        let Some(SemanticTypeShapeV1::Tuple(tuple)) = semantic
            .types()
            .get(tuple_ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return self
                .source
                .source
                .missing("checked source component holder is not a tuple");
        };
        let [integer, boolean] = tuple.fields() else {
            return self
                .source
                .source
                .missing("checked source component tuple has a different field count");
        };
        let integer_scalar = kir_semantic_scalar_v1(
            &lower_scalar_type(semantic.types(), *integer).map_err(source_emission_error_v18)?,
        )
        .filter(|s| checked_arithmetic_scalar_v41(*s).is_some())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "checked source component integer width is not modeled",
        ))?;
        if !matches!(
            semantic
                .types()
                .get(boolean.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        ) || tuple.fields()[field as usize] != ty
            || scalar
                != if field == 0 {
                    integer_scalar
                } else {
                    ProductionSemanticScalarTypeV2::Bool
                }
        {
            return self
                .source
                .source
                .missing("checked source component scalar type differs");
        }
        let mut projected = true;
        loop {
            budget.charge_work(16)?;
            if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || *remaining == 0 {
                return self
                    .source
                    .source
                    .missing("checked source component exceeds expression bounds");
            }
            depth += 1;
            *remaining -= 1;
            if !std::ptr::eq(leaves.leaves.relation, self.source)
                || !scoped_object_original_place_v29(function, site, role)
                    .is_some_and(|p| std::ptr::eq(p, place))
                || function
                    .locals()
                    .get(place.local().index() as usize)
                    .map(|l| l.ty())
                    != Some(tuple_ty)
                || (!projected && (place.ty() != tuple_ty || !place.projections().is_empty()))
            {
                return self
                    .source
                    .source
                    .missing("checked source component original holder or occurrence differs");
            }
            let value =
                self.promoted_use(function_id, site, role, place.local().index(), budget)?;
            let definition = self.definition(function_id, value, budget)?;
            let OriginalEntryDefinitionV20::Assignment { block, statement } = definition.origin
            else {
                return self.source.source.missing("checked source component requires an original assignment, not an argument or return");
            };
            let Some(SemanticStatementKindV1::Assign(assignment)) = function
                .blocks()
                .get(block as usize)
                .and_then(|b| b.statements().get(statement as usize))
                .map(|s| s.kind())
            else {
                return self
                    .source
                    .source
                    .missing("checked source component original assignment absent");
            };
            if definition.local != place.local().index()
                || assignment.destination().local().index() != definition.local
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != tuple_ty
                || assignment.value().result_type() != tuple_ty
            {
                return self
                    .source
                    .source
                    .missing("checked source component original destination differs");
            }
            site = EntrySiteV20::Statement {
                block: fe2o3_mir_model::SsaBlockIdV1::new(block),
                statement,
            };
            let checked = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(holder) | SemanticOperandV1::Move(holder),
                ) => {
                    place = holder;
                    role = EntryOperandV20::RvalueOperand(0);
                    projected = false;
                    continue;
                }
                SemanticRvalueKindV1::CheckedBinary(checked) => checked,
                _ => {
                    return self.source.source.missing(
                        "checked source component has a different original tuple producer",
                    );
                }
            };
            if semantic_operand_type(checked.left()) != *integer
                || semantic_operand_type(checked.right()) != *integer
            {
                return self.source.source.missing(
                    "checked source component operands differ from the tuple integer type",
                );
            }
            let operator = source_checked_operator_v41(checked.operation());
            let endpoint =
                self.source
                    .ssa_typed_endpoint_v36(leaves.leaves.root, instance, value, budget)?;
            if endpoint.source_function(budget)? != function_id
                || endpoint.source_local(budget)?.index() != definition.local
                || endpoint.source_type(budget)? != tuple_ty
                || endpoint.carrier_shape(budget)?
                    != (ProductionSourceSsaCarrierShapeV37::Aggregate { components: 2 })
            {
                return self
                    .source
                    .source
                    .missing("checked source component retained tuple carrier differs");
            }
            let first = endpoint.component(0, budget)?;
            let second = endpoint.component(1, budget)?;
            let first = first.original_definition(budget)?.ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "checked source value carrier is absent",
                ),
            )?;
            let second = second.original_definition(budget)?.ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "checked source overflow carrier is absent",
                ),
            )?;
            let first = self
                .source
                .inventory
                .definitions()
                .get(first)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let second = self
                .source
                .inventory
                .definitions()
                .get(second)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let (
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                    operation: a,
                    result: 0,
                },
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                    operation: b,
                    result: 1,
                },
            ) = (first.coordinate, second.coordinate)
            else {
                return self.source.source.missing(
                    "checked source components do not name distinct exact result ordinals",
                );
            };
            let actual = source_operation_row_v18(self.source.inventory, a, budget)?.operation;
            if a != b
                || !matches!(actual.kind, OperationKind::Binary { op: BinaryOp::Checked(op), .. } if op == operator)
                || actual.results.len() != 2
                || kir_semantic_scalar_v1(first.ty) != Some(integer_scalar)
                || second.ty != &Type::BOOL
            {
                return self
                    .source
                    .source
                    .missing("checked source component canonical operator or result type differs");
            }
            let operands = [checked.left(), checked.right()];
            let next = depth
                .checked_add(CHECKED_ARITHMETIC_DEPTH_V41)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if field == 0 {
                budget.reserve_storage(argument_product_v1(
                    2,
                    size_of::<ProductionSemanticExpressionV2>(),
                )?)?;
                let lhs = self.private_expression_v22(
                    leaves,
                    instance,
                    *integer,
                    integer_scalar,
                    OriginalPrivateInputV22::Operand {
                        site,
                        role: EntryOperandV20::RvalueOperand(0),
                        operand: operands[0],
                    },
                    next,
                    remaining,
                    budget,
                )?;
                let rhs = self.private_expression_v22(
                    leaves,
                    instance,
                    *integer,
                    integer_scalar,
                    OriginalPrivateInputV22::Operand {
                        site,
                        role: EntryOperandV20::RvalueOperand(1),
                        operand: operands[1],
                    },
                    next,
                    remaining,
                    budget,
                )?;
                return Ok(ProductionSemanticExpressionV2::Binary {
                    operation: checked_arithmetic_operator_v41(operator),
                    scalar: integer_scalar,
                    overflow: ProductionOverflowContractV2::Checked,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                });
            }
            let mut emit = |node, budget: &mut ArgumentBudgetV1<'_>| {
                budget.charge_work(1)?;
                if *remaining == 0 {
                    return self
                        .source
                        .source
                        .missing("checked overflow expression exceeds node bounds");
                }
                *remaining -= 1;
                Ok(match node {
                    CheckedArithmeticNodeV41::Operand(index) => self.private_expression_v22(
                        leaves,
                        instance,
                        *integer,
                        integer_scalar,
                        OriginalPrivateInputV22::Operand {
                            site,
                            role: EntryOperandV20::RvalueOperand(index as u32),
                            operand: operands[index],
                        },
                        next,
                        remaining,
                        budget,
                    )?,
                    CheckedArithmeticNodeV41::Constant { scalar, bits } => {
                        ProductionSemanticExpressionV2::Constant { scalar, bits }
                    }
                    CheckedArithmeticNodeV41::Binary {
                        operation,
                        scalar,
                        lhs,
                        rhs,
                    } => {
                        budget.reserve_storage(argument_product_v1(
                            2,
                            size_of::<ProductionSemanticExpressionV2>(),
                        )?)?;
                        ProductionSemanticExpressionV2::Binary {
                            operation,
                            scalar,
                            overflow: ProductionOverflowContractV2::Wrapping,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        }
                    }
                    CheckedArithmeticNodeV41::Compare {
                        operation,
                        scalar,
                        lhs,
                        rhs,
                    } => {
                        budget.reserve_storage(argument_product_v1(
                            2,
                            size_of::<ProductionSemanticExpressionV2>(),
                        )?)?;
                        ProductionSemanticExpressionV2::Compare {
                            operation,
                            operand_scalar: scalar,
                            lhs: Box::new(lhs),
                            rhs: Box::new(rhs),
                        }
                    }
                    CheckedArithmeticNodeV41::Select {
                        condition,
                        when_true,
                        when_false,
                    } => {
                        budget.reserve_storage(argument_product_v1(
                            3,
                            size_of::<ProductionSemanticExpressionV2>(),
                        )?)?;
                        ProductionSemanticExpressionV2::Select {
                            scalar: ProductionSemanticScalarTypeV2::Bool,
                            condition: Box::new(condition),
                            when_true: Box::new(when_true),
                            when_false: Box::new(when_false),
                        }
                    }
                })
            };
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of_val(&emit),
                std::mem::align_of_val(&emit),
            ])?)?;
            return checked_overflow_expression_v41(
                operator,
                integer_scalar,
                &mut |node| emit(node, budget),
                || {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "checked overflow expression recipe differs",
                    )
                },
            );
        }
    }
}
