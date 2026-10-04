fn optimized_compiler_enum_memory_v55(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<PendingCompilerEnumMemoryV55> {
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Usage,
    };
    original.check(budget)?;
    let relation = original.correspondence;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let owner = relation.source.root_row(original.root)?;
    let input_function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    );
    let output_function =
        optimized_source_root_function_v18(relation, optimized, original.root, budget)?.coordinate;
    let output = optimized.output_inventory(budget)?;
    let pending = &original.pending.compiler_enum;
    let mut mapped = PendingCompilerEnumMemoryV55 {
        allocations: emission_vec_v1(pending.allocations.len(), budget)
            .map_err(immutable_memory_error_v29)?,
        accesses: emission_vec_v1(pending.accesses.len(), budget)
            .map_err(immutable_memory_error_v29)?,
    };
    for row in &pending.allocations {
        let index = relation
            .inventory
            .definition_index_for_value(input_function, row.origin.pointer, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "compiler allocation input definition",
            ))?;
        let definition = &relation.inventory.definitions()[index];
        let Definition::Result {
            operation: input,
            result: 0,
        } = definition.coordinate
        else {
            return relation
                .source
                .missing("compiler allocation input is not an operation result");
        };
        check_enum_spill_alloca_v55(
            &row.origin,
            source_operation_row_v18(relation.inventory, input, budget)?.operation,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        let at = match optimized.operation(input, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                return relation
                    .source
                    .missing("compiler allocation needs checked memory-elimination transport");
            }
        };
        if at.block.function != output_function {
            return relation
                .source
                .missing("compiler allocation changed output function");
        }
        let actual = Definition::Result {
            operation: at,
            result: 0,
        };
        let mut found = false;
        for descendant in optimized.definition_descendants(definition.coordinate, budget)? {
            budget.charge_work(2)?;
            if descendant.output == actual {
                if found {
                    return relation
                        .source
                        .missing("compiler allocation duplicate descendant");
                }
                found = true;
            }
        }
        if !found {
            return relation
                .source
                .missing("compiler allocation missing descendant");
        }
        let pointer = optimized_source_definition_row_v18(output, actual, budget)?
            .value
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "compiler allocation output value",
            ))?;
        retain_source_enum_spill_v48(row.instance, &row.origin, &mut mapped.allocations, budget)
            .map_err(immutable_memory_error_v29)?;
        let retained = mapped
            .allocations
            .last_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        retained.origin.pointer = pointer;
        check_enum_spill_alloca_v55(
            &retained.origin,
            source_operation_row_v18(output, at, budget)?.operation,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
    }
    for row in &pending.accesses {
        let input =
            source_input_operation_v18(relation, original.root, row.block, row.operation, budget)?;
        let at = match optimized.operation(input, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                // Generic scalar rewrite lineage is not a dead-Store proof.
                return relation
                    .source
                    .missing("compiler Store needs checked memory-elimination transport");
            }
        };
        if at.block.function != output_function {
            return relation
                .source
                .missing("compiler Store changed output function");
        }
        let ScopedCompilerEnumRoleV55::Store {
            site,
            source,
            value,
        } = row.record.role
        else {
            return relation
                .source
                .missing("compiler Store original payload role");
        };
        let mut actual = [row.record.pointer, value];
        for (operand, old) in [row.record.pointer, value].into_iter().enumerate() {
            let (usage, new) = optimized_source_actual_operand_v18(
                relation,
                optimized,
                Usage::OperationOperand {
                    operation: input,
                    operand: operand as u32,
                },
                at,
                budget,
            )?;
            if usage.coordinate
                != (Usage::OperationOperand {
                    operation: at,
                    operand: operand as u32,
                })
                || !optimized_source_value_descends_v18(
                    relation,
                    optimized,
                    input_function,
                    old,
                    output_function,
                    new,
                    budget,
                )?
            {
                return relation
                    .source
                    .missing("compiler Store operand lineage changed");
            }
            actual[operand] = new;
        }
        let block = source_block_row_v18(output, at.block, budget)?.block.id;
        if mapped.accesses.len() == mapped.accesses.capacity() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        mapped.accesses.push(PendingCompilerEnumAccessV55 {
            block,
            operation: at.operation as usize,
            record: ScopedCompilerEnumAccessV55 {
                pointer: actual[0],
                role: ScopedCompilerEnumRoleV55::Store {
                    site,
                    source,
                    value: actual[1],
                },
                ..row.record
            },
            ..*row
        });
    }
    call_splice_sort_work_v1(mapped.allocations.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(immutable_memory_error_v29)?;
    mapped
        .allocations
        .sort_unstable_by_key(|row| row.origin.pointer);
    call_splice_sort_work_v1(mapped.accesses.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(immutable_memory_error_v29)?;
    mapped
        .accesses
        .sort_unstable_by_key(|row| (row.block, row.operation));
    Ok(mapped)
}
