fn optimized_source_endpoints_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.retain_query((|| {
        original.query(budget)?;
        budget.charge_work(2)?;
        if !std::ptr::eq(original.inventory, optimized.input_inventory(budget)?)
            || !std::ptr::eq(original.source, optimized.original_source(budget)?)
        {
            return original
                .source
                .missing("optimized source argument endpoint association");
        }
        Ok(())
    })())
}

fn optimized_source_value_descends_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    input: ValueId,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    output: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let input = original
            .inventory
            .definition_for_value(input_function, input, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized value has no original definition",
            ))?;
        let output = optimized
            .output_inventory(budget)?
            .definition_for_value(output_function, output, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized value has no output definition",
            ))?;
        if input.ty != output.ty {
            return Ok(false);
        }
        let mut found = false;
        for descendant in optimized.definition_descendants(input.coordinate, budget)? {
            budget.charge_work(1)?;
            if descendant.output == output.coordinate {
                if found {
                    return original
                        .source
                        .missing("optimized value repeated output descendant");
                }
                found = true;
            }
        }
        Ok(found)
    })())
}

fn optimized_source_root_function_v18<'a>(
    original: &'a ProductionSourceCorrespondenceV18<'a>,
    optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'a>> {
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let ordinal = original.source.root(root, budget)?.1;
        let input = original.inventory.functions().get(ordinal).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized source original root"),
        )?;
        // The checked transition proves a bijection of exact declarations. Use
        // the paid name index, not an assumption that function ordinals survive.
        optimized
            .output_inventory(budget)?
            .function_for_name(input.function.id.as_str(), budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized source output root",
            ))
    })())
}

fn optimized_source_block_row_v18<'a>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    coordinate: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>> {
    budget.charge_work(3)?;
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .filter(|row| row.coordinate == coordinate.function)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized block function coordinate",
        ))?;
    function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .filter(|index| *index < function.blocks.end)
        .and_then(|index| inventory.blocks().get(index))
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized block coordinate",
        ))
}

fn optimized_source_operation_row_v18<'a>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'a>> {
    let block = optimized_source_block_row_v18(inventory, coordinate.block, budget)?;
    budget.charge_work(2)?;
    block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|index| *index < block.operations.end)
        .and_then(|index| inventory.operations().get(index))
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized operation coordinate",
        ))
}

fn optimized_source_definition_row_v18<'a>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    coordinate: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    let (range, ordinal) = match coordinate {
        Definition::FunctionArgument { function, argument } => {
            budget.charge_work(3)?;
            let row = inventory
                .functions()
                .get(function.0 as usize)
                .filter(|row| row.coordinate == function)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized definition function",
                ))?;
            if argument as usize >= row.function.signature.parameters.len() {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized definition parameter",
                ));
            }
            (row.definitions.clone(), argument)
        }
        Definition::BlockArgument { block, argument } => (
            optimized_source_block_row_v18(inventory, block, budget)?
                .parameters
                .clone(),
            argument,
        ),
        Definition::Result { operation, result } => (
            optimized_source_operation_row_v18(inventory, operation, budget)?
                .results
                .clone(),
            result,
        ),
    };
    budget.charge_work(2)?;
    range
        .start
        .checked_add(ordinal as usize)
        .filter(|index| *index < range.end)
        .and_then(|index| inventory.definitions().get(index))
        .filter(|row| row.coordinate == coordinate)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized definition coordinate",
        ))
}

fn optimized_source_actual_operand_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: fe2o3_kernel_ir::CanonicalKirUseCoordinateV1,
    expected: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(fe2o3_kernel_analysis::CanonicalKirOutputUseV1, ValueId)> {
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Usage;
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let selected =
            optimized
                .operand(input, budget)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "optimized required operand removed",
                ))?;
        let Usage::OperationOperand { operation, operand } = selected.coordinate else {
            return original
                .source
                .missing("optimized operation operand became a terminator");
        };
        if operation != expected {
            return original
                .source
                .missing("optimized operand selected another output operation");
        }
        let inventory = optimized.output_inventory(budget)?;
        let row = optimized_source_operation_row_v18(inventory, operation, budget)?;
        budget.charge_work(3)?;
        let actual = row
            .operands
            .start
            .checked_add(operand as usize)
            .filter(|index| *index < row.operands.end)
            .and_then(|index| inventory.uses().get(index))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized operand census",
            ))?;
        let definition = inventory.definitions().get(actual.definition).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized operand definition"),
        )?;
        if actual.coordinate != selected.coordinate || definition.coordinate != selected.definition
        {
            return original
                .source
                .missing("optimized operand differs from actual selected use");
        }
        let value = definition
            .value
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized operand has no value",
            ))?;
        Ok((selected, value))
    })())
}
