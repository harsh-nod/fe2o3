#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceCheckedResultV44 {
    values: [ValueId; 2],
    scalar: ScalarType,
    operands: [ValueId; 2],
}

fn source_checked_result_headers_v44() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<Option<SourceCheckedResultV44>>(),
        argument_product_v1(
            2,
            size_of::<Result<Option<SourceCheckedResultV44>, ProductionSemanticKirErrorV1>>(),
        )?,
        size_of::<[usize; 2]>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<[usize; 2]>>())?,
        size_of::<[&SemanticValueBindingV1; 2]>(),
        size_of::<[SemanticTypeIdV1; 2]>(),
        argument_product_v1(6, size_of::<Type>())?,
        argument_product_v1(12, size_of::<usize>())?,
    ])
}

fn retain_source_checked_result_v44(
    types: &[SemanticTypeDeclV1],
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    binding: &SemanticValueBindingV1,
    operands: Option<[ValueId; 2]>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceCheckedResultV44>, ProductionSemanticKirErrorV1> {
    let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
        return Ok(None);
    };
    budget.charge_work(2)?;
    // This captures a locator even for retained destinations; no SSA promotion
    // is inferred from the presence of these two actual results.
    let expected =
        checked_binary_result_type(types, checked.left().ty(), assignment.value().result_type())
            .map_err(|_| execution_archive_error_v29())?;
    let SemanticValueBindingV1::Aggregate(fields) = binding else {
        return Err(execution_archive_error_v29());
    };
    let [
        SemanticValueBindingV1::Value {
            id: value,
            ty: value_type,
        },
        SemanticValueBindingV1::Value {
            id: overflow,
            ty: overflow_type,
        },
    ] = fields.as_slice()
    else {
        return Err(execution_archive_error_v29());
    };
    budget.charge_work(8)?;
    if !invocation_equal_types_v1(value_type, &expected, budget)? || overflow_type != &Type::BOOL {
        return Err(execution_archive_error_v29());
    }
    let Type::Scalar(scalar) = expected else {
        return Err(execution_archive_error_v29());
    };
    Ok(Some(SourceCheckedResultV44 {
        values: [*value, *overflow],
        scalar,
        operands: operands.ok_or_else(execution_archive_error_v29)?,
    }))
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn checked_assignment_definitions_v44(
        &self,
        root: usize,
        instance: usize,
        site: ExecutionSiteV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<[usize; 2]> {
        let result = (|| {
            self.query(budget)?;
            let ExecutionSiteV29::Statement { block, statement } = site else {
                return self
                    .source
                    .missing("checked result requires an original assignment");
            };
            let (row, assignment) = self.assignment_result_row_v30(
                root,
                instance,
                SemanticBlockIdV1::from_index(block.get()),
                statement,
                budget,
            )?;
            let saved = row
                .checked
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "checked result pair was not retained",
                ))?;
            let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
                return self
                    .source
                    .missing("checked result original opcode differs");
            };
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(self.source.root_row(root)?.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let mut definitions = [0; 2];
            for (ordinal, value) in saved.values.iter().enumerate() {
                budget.charge_work(3)?;
                definitions[ordinal] = self
                    .inventory
                    .definition_index_for_value(function, *value, budget)
                    .map_err(source_pointer_inventory_error_v18)?
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "checked result definition is absent",
                    ))?;
            }
            let first = &self.inventory.definitions()[definitions[0]];
            let second = &self.inventory.definitions()[definitions[1]];
            let (
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                    operation: left,
                    result: 0,
                },
                fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                    operation: right,
                    result: 1,
                },
            ) = (first.coordinate, second.coordinate)
            else {
                return self
                    .source
                    .missing("checked results require both exact operation ordinals");
            };
            let operation = source_operation_row_v18(self.inventory, left, budget)?.operation;
            if left != right
                || first.ty != &Type::Scalar(saved.scalar)
                || second.ty != &Type::BOOL
                || operation.results.len() != 2
                || !matches!(operation.kind, OperationKind::Binary { op: BinaryOp::Checked(operator), lhs, rhs }
                    if operator == lower_checked_binary(checked.operation()) && [lhs, rhs] == saved.operands)
            {
                return self
                    .source
                    .missing("checked result operation or scalar type differs");
            }
            Ok(definitions)
        })();
        self.retain_query(result)
    }
}
