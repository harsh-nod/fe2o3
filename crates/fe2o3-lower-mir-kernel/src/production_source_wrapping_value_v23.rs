// Ordinary MIR Binary and CheckedBinary have different result contracts even
// though the emitter uses a two-result checked KIR instruction for both.
#[derive(Clone, Copy)]
struct SourceWrappingValueV23 {
    span: usize,
    instance: usize,
    block: u32,
    statement: u32,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    value: ValueId,
    operator: CheckedBinaryOperator,
    scalar: ProductionSemanticScalarTypeV2,
}

type SourceWrappingContractV23 = (
    usize,
    u32,
    u32,
    CheckedBinaryOperator,
    ProductionSemanticScalarTypeV2,
);

fn source_wrapping_span_contract_v23(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    span: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<SourceWrappingContractV23>> {
    relation.query(budget)?;
    let row = relation
        .source
        .root_row(root)?
        .coordinates
        .spans
        .rows
        .get(span)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "wrapping scalar source span differs",
        ))?;
    budget.charge_work(10)?;
    let InstanceSpanSourceV1::Statement(site) = row.source else {
        return Ok(None);
    };
    let instance = row.instance.index();
    let source = relation.source.instance(root, instance, budget)?.0;
    if source != site.semantic_function {
        return relation
            .source
            .missing("wrapping scalar source instance differs");
    }
    let semantic = relation.source.source_semantic(budget)?;
    let original = semantic
        .functions()
        .get(source.index() as usize)
        .and_then(|function| function.blocks().get(site.semantic_block.index() as usize))
        .and_then(|block| block.statements().get(site.statement_ordinal as usize))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "wrapping scalar source assignment differs",
        ))?;
    Ok(
        source_wrapping_contract_v23(original, semantic.types()).map(|(operator, scalar)| {
            (
                instance,
                site.semantic_block.index(),
                site.statement_ordinal,
                operator,
                scalar,
            )
        }),
    )
}

fn visit_wrapping_span_v23(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    span: usize,
    instance: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    mut visit: impl FnMut(
        ProductionSourceOperationV18,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    let rows = relation.attachment_range(
        TileAttachmentKeyV29 {
            root,
            family: TileAttachmentFamilyV29::InstanceSpans,
            instance,
            row: span,
            field: TileAttachmentFieldV29::Span,
            component: 0,
            part: 0,
        },
        budget,
    )?;
    for row in rows {
        let operation = relation.mapped_source_operation(row.location, budget)?;
        visit(operation, budget)?;
    }
    Ok(())
}

fn source_wrapping_contract_v23(
    statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
    types: &[SemanticTypeDeclV1],
) -> Option<(CheckedBinaryOperator, ProductionSemanticScalarTypeV2)> {
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return None;
    };
    let SemanticRvalueKindV1::Binary { operation, .. } = assignment.value().kind() else {
        return None;
    };
    let operator = match operation {
        SemanticBinaryOpV1::Add => CheckedBinaryOperator::Add,
        SemanticBinaryOpV1::Subtract => CheckedBinaryOperator::Subtract,
        SemanticBinaryOpV1::Multiply => CheckedBinaryOperator::Multiply,
        _ => return None,
    };
    let ty = lower_scalar_type(types, assignment.value().result_type()).ok()?;
    let scalar = kir_semantic_scalar_v1(&ty)?;
    if !matches!(
        scalar,
        ProductionSemanticScalarTypeV2::Integer {
            bits: 8 | 16 | 32 | 64,
            ..
        }
    ) {
        return None;
    }
    Some((operator, scalar))
}

fn source_wrapping_result_v23(
    operation: &Operation,
    operator: CheckedBinaryOperator,
    scalar: ProductionSemanticScalarTypeV2,
) -> Option<ValueId> {
    let OperationKind::Binary {
        op: BinaryOp::Checked(actual),
        ..
    } = &operation.kind
    else {
        return None;
    };
    let [value, overflow] = operation.results.as_slice() else {
        return None;
    };
    (*actual == operator
        && kir_semantic_scalar_v1(&value.ty) == Some(scalar)
        && overflow.ty == Type::BOOL
        && value.id != overflow.id)
        .then_some(value.id)
}

fn source_scalar_overflow_query_headers_v23() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<(
            &dyn SourceScalarNormalizationV18,
            &Function,
            &ValueId,
            &ProductionOverflowContractV2,
        )>(),
        3 * size_of::<SourceOwnedResultV18<ProductionOverflowContractV2>>(),
        size_of::<Option<ProductionOverflowContractV2>>(),
    ])
}

fn source_wrapping_values_v23(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceWrappingValueV23>> {
    relation.retain_query((|| {
        relation.query(budget)?;
        // Construction temporaries remain paid until the containing leaf scope
        // drops its owners. No backing is allocated during a lookup.
        budget.reserve_storage(argument_sum_v1(&[
            size_of::<Vec<SourceWrappingValueV23>>(),
            size_of::<SourceWrappingValueV23>(),
            size_of::<Option<SourceWrappingValueV23>>(),
            size_of::<SourceWrappingContractV23>(),
            size_of::<Option<SourceWrappingContractV23>>(),
            size_of::<SourceOwnedResultV18<Option<SourceWrappingContractV23>>>(),
            size_of::<Type>(),
            size_of::<Result<Type, ProductionSemanticKirErrorV1>>(),
            size_of::<Option<ProductionSemanticScalarTypeV2>>(),
            size_of::<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>(),
            size_of::<&[SourceAttachmentV18]>(),
            size_of::<std::slice::Iter<'_, SourceAttachmentV18>>(),
            size_of::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>(),
            size_of::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>(),
            size_of::<&fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>(),
            // The two endpoint implementations and the output builder borrow
            // these fixed carriers; their vectors have separate capacity credit.
            size_of::<(
                &SourceScalarLeavesV18<'_, '_>,
                &Function,
                &ValueId,
                &ProductionOverflowContractV2,
                &mut ArgumentBudgetV1<'_>,
            )>(),
            size_of::<(
                &ProductionOptimizedSourceScalarLeavesV18<'_>,
                &Function,
                &ValueId,
                &ProductionOverflowContractV2,
                &mut ArgumentBudgetV1<'_>,
            )>(),
            size_of::<(
                &ProductionSourceScalarLeavesV18<'_>,
                &ProductionOptimizedSourceCorrespondenceV18<'_>,
                &mut ArgumentBudgetV1<'_>,
            )>(),
            2 * size_of::<fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1>(),
            size_of::<ProductionOptimizedSourceOperationV18>(),
            size_of::<TileAttachmentKeyV29>(),
            size_of::<ProductionSourceOperationV18>(),
            size_of::<(
                usize,
                usize,
                u32,
                u32,
                ValueId,
                CheckedBinaryOperator,
                ProductionSemanticScalarTypeV2,
                Option<(CheckedBinaryOperator, ProductionSemanticScalarTypeV2)>,
                &SemanticFunctionDeclV1,
                &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
                &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
                &ProductionSourceCorrespondenceV18<'_>,
                &ArgumentBudgetV1<'_>,
                &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>,
                &SourceWrappingValueV23,
                Option<&SourceWrappingValueV23>,
            )>(),
        ])?)?;
        let spans = relation.source.root_row(root)?.coordinates.spans.rows.len();
        let mut capacity = 0usize;
        for span in 0..spans {
            if source_wrapping_span_contract_v23(relation, root, span, budget)?.is_some() {
                capacity = capacity
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        let mut rows = emission_vec_v1(capacity, budget).map_err(source_emission_error_v18)?;
        for span in 0..spans {
            let Some((instance, block, statement, operator, scalar)) =
                source_wrapping_span_contract_v23(relation, root, span, budget)?
            else {
                continue;
            };
            let mut found = None;
            let visit = |mapped, budget: &mut ArgumentBudgetV1<'_>| {
                budget.charge_work(6)?;
                let ProductionSourceOperationV18::Operation(operation) = mapped else {
                    return Ok(());
                };
                let actual = source_operation_row_v18(relation.inventory, operation, budget)?;
                let Some(value) = source_wrapping_result_v23(actual.operation, operator, scalar)
                else {
                    return Ok(());
                };
                if found.is_some() {
                    return relation.source.missing(
                        "ordinary source arithmetic has multiple checked value producers",
                    );
                }
                found = Some(SourceWrappingValueV23 {
                    span,
                    instance,
                    block,
                    statement,
                    operation,
                    value,
                    operator,
                    scalar,
                });
                Ok(())
            };
            let visit_header = std::mem::size_of_val(&visit);
            budget.reserve_storage(visit_header)?;
            visit_wrapping_span_v23(relation, root, span, instance, budget, visit)?;
            budget.release_storage(visit_header)?;
            // Original unreachable/zero-operation spans grant nothing.
            if let Some(row) = found {
                budget.charge_work(1)?;
                if rows.len() == rows.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                rows.push(row);
            }
        }
        source_wrapping_sort_v23(&mut rows, budget)?;
        Ok(rows)
    })())
}

fn source_wrapping_sort_v23(
    rows: &mut [SourceWrappingValueV23],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    private_array_heapsort_v1(
        rows,
        |row| [row.value.0 as usize],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    for pair in rows.windows(2) {
        budget.charge_work(1)?;
        if pair[0].value == pair[1].value {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "ordinary source arithmetic value is ambiguous",
            ));
        }
    }
    Ok(())
}

fn source_wrapping_find_v23<'a>(
    rows: &'a [SourceWrappingValueV23],
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<&'a SourceWrappingValueV23>> {
    let first = private_array_partition_v1(
        rows,
        |row| [row.value.0 as usize],
        [value.0 as usize],
        false,
        &mut SourceCorrespondenceWorkV18(budget),
    )?;
    budget.charge_work(1)?;
    Ok(rows.get(first).filter(|row| row.value == value))
}

impl SourceScalarLeavesV18<'_, '_> {
    fn wrapping_value_v23(
        &self,
        function: &Function,
        value: ValueId,
        overflow: ProductionOverflowContractV2,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOverflowContractV2> {
        if overflow != ProductionOverflowContractV2::Checked || !self.ordinary_values {
            return Ok(overflow);
        }
        self.relation.retain_query((|| {
            self.query(budget)?;
            let root = self.relation.source.root(self.root, budget)?.1;
            let actual = self.relation.inventory.functions().get(root).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("wrapping original root"),
            )?;
            budget.charge_work(1)?;
            if !std::ptr::eq(actual.function, function) {
                return self
                    .relation
                    .source
                    .missing("wrapping scalar substituted original function");
            }
            let Some(row) = source_wrapping_find_v23(&self.wrapping, value, budget)? else {
                return Ok(overflow);
            };
            self.check_wrapping_row_v23(row, budget)?;
            Ok(ProductionOverflowContractV2::Wrapping)
        })())
    }

    fn check_wrapping_row_v23(
        &self,
        row: &SourceWrappingValueV23,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.query(budget)?;
        let contract =
            source_wrapping_span_contract_v23(self.relation, self.root, row.span, budget)?;
        if contract
            != Some((
                row.instance,
                row.block,
                row.statement,
                row.operator,
                row.scalar,
            ))
        {
            return self
                .relation
                .source
                .missing("wrapping scalar source assignment differs");
        }
        let mut found = 0usize;
        let visit = |mapped, budget: &mut ArgumentBudgetV1<'_>| {
            budget.charge_work(1)?;
            if matches!(mapped, ProductionSourceOperationV18::Operation(operation) if operation == row.operation)
            {
                found = found.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            Ok(())
        };
        let visit_header = std::mem::size_of_val(&visit);
        budget.reserve_storage(visit_header)?;
        visit_wrapping_span_v23(
            self.relation,
            self.root,
            row.span,
            row.instance,
            budget,
            visit,
        )?;
        budget.release_storage(visit_header)?;
        if found != 1 {
            return self
                .relation
                .source
                .missing("wrapping scalar original occurrence differs");
        }
        let actual = source_operation_row_v18(self.relation.inventory, row.operation, budget)?;
        budget.charge_work(6)?;
        if source_wrapping_result_v23(actual.operation, row.operator, row.scalar) != Some(row.value)
        {
            return self
                .relation
                .source
                .missing("wrapping scalar original occurrence differs");
        }
        Ok(())
    }
}

fn optimized_source_wrapping_values_v23(
    original: &ProductionSourceScalarLeavesV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceWrappingValueV23>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    let leaves = original.leaves;
    let relation = leaves.relation;
    if leaves.wrapping.is_empty() {
        return Ok(Vec::new());
    }
    relation.retain_query((|| {
        optimized_source_endpoints_v18(relation, optimized, budget)?;
        let output = optimized.output_inventory(budget)?;
        let mut rows =
            emission_vec_v1(leaves.wrapping.len(), budget).map_err(source_emission_error_v18)?;
        for source in &leaves.wrapping {
            leaves.check_wrapping_row_v23(source, budget)?;
            budget.charge_work(1)?;
            let actual = match optimized.operation(source.operation, budget)? {
                ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. }
                | ProductionOptimizedSourceOperationV18::Rewritten { .. } => continue,
            };
            let operation = source_operation_row_v18(output, actual, budget)?;
            budget.charge_work(6)?;
            let value =
                source_wrapping_result_v23(operation.operation, source.operator, source.scalar)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "wrapping scalar output operation differs",
                    ))?;
            let mut found = 0usize;
            for descendant in optimized.definition_descendants(
                Definition::Result {
                    operation: source.operation,
                    result: 0,
                },
                budget,
            )? {
                budget.charge_work(1)?;
                if descendant.output
                    == (Definition::Result {
                        operation: actual,
                        result: 0,
                    })
                {
                    found = found.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
            }
            if found != 1 {
                return relation
                    .source
                    .missing("wrapping scalar output is not the exact value descendant");
            }
            if rows.len() == rows.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            rows.push(SourceWrappingValueV23 {
                operation: actual,
                value,
                ..*source
            });
        }
        source_wrapping_sort_v23(&mut rows, budget)?;
        Ok(rows)
    })())
}
