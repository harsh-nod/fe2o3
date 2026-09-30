// Only complete, already source-authenticated ordinary issued accesses reach
// this boundary. Generic representation alone does not create a source role.
#[derive(Clone, Copy)]
struct IssuedTransportSelectionV26 {
    input: SliceDefinition,
    output: SliceDefinition,
    kind: fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1,
}

fn select_issued_transport_definitions_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    input_casts: &[ValueId],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<IssuedTransportSelectionV26>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    type Frame<'a> = (
        Vec<IssuedTransportSelectionV26>,
        IssuedTransportSelectionV26,
        &'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1],
        &'a fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1,
        [SliceDefinition; 2],
        ProductionOptimizedSourceOperationV18,
        Kind,
        [Option<SliceOperation>; 2],
        [usize; 4],
        [&'a (); 4],
    );
    budget.reserve_storage(argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        2 * size_of::<SourceOwnedResultV18<Frame<'_>>>(),
    ])?)?;
    let mut selected =
        emission_vec_v1(input_casts.len(), budget).map_err(source_emission_error_v18)?;
    for value in input_casts {
        budget.charge_work(4)?;
        let input = original
            .inventory
            .definition_for_value(input_function, *value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued original transport result absent",
            ))?
            .coordinate;
        let SliceDefinition::Result {
            operation,
            result: 0,
        } = input
        else {
            return original
                .source
                .missing("issued original transport is not a single result");
        };
        let (kind, retained) = match optimized.operation(operation, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { input: at, output }
                if at == operation =>
            {
                (Kind::Retained, Some(output))
            }
            ProductionOptimizedSourceOperationV18::Rewritten { input: at } if at == operation => {
                (Kind::Substituted, None)
            }
            _ => {
                return original
                    .source
                    .missing("issued transport has no live checked disposition");
            }
        };
        let rows = optimized.definition_descendants(input, budget)?;
        budget.charge_work(3)?;
        let [row] = rows else {
            return original
                .source
                .missing("issued transport requires one complete checked descendant");
        };
        if row.kind != kind
            || retained.is_some_and(|operation| {
                row.output
                    != (SliceDefinition::Result {
                        operation,
                        result: 0,
                    })
            })
        {
            return original
                .source
                .missing("issued transport descendant disagrees with checked disposition");
        }
        selected.push(IssuedTransportSelectionV26 {
            input,
            output: row.output,
            kind,
        });
    }
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
fn install_issued_pointer_transport_roles_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    output_function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    output_actual: &SourceIssuedActualV29<'_>,
    transports: &[(
        SourceIssuedPointerTransportV26,
        SourceIssuedPointerTransportV26,
    )],
    roles: &mut [DescriptorSourceRoleRowV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(1)?;
    if transports.is_empty() {
        return Ok(());
    }
    type Frame<'a> = (
        SourceIssuedActualV29<'a>,
        Vec<SourceIssuedPointerTransportV26>,
        Vec<SourceIssuedPointerTransportV26>,
        Vec<ValueId>,
        Vec<ValueId>,
        Vec<bool>,
        [SliceDefinition; 4],
        [SliceOperation; 2],
        [usize; 16],
        [&'a (); 16],
        SourceOwnedResultV18<()>,
    );
    budget.reserve_storage(argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        2 * size_of::<SourceOwnedResultV18<Frame<'_>>>(),
    ])?)?;
    let input_function = original.source.root(root, budget)?.1;
    let input_function = original.inventory.functions().get(input_function).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("issued transport original root"),
    )?;
    let input_actual = SourceIssuedActualV29::from_function(input_function.function, budget)
        .map_err(source_emission_error_v18)?;
    let mut input_transports =
        emission_vec_v1(transports.len(), budget).map_err(source_emission_error_v18)?;
    let mut output_transports =
        emission_vec_v1(transports.len(), budget).map_err(source_emission_error_v18)?;
    for (input, output) in transports {
        budget.charge_work(2)?;
        input_transports.push(*input);
        output_transports.push(*output);
    }
    let input_casts = source_issued_pointer_transport_census_v26(
        input_function.function,
        &input_actual,
        &input_transports,
        budget,
    )
    .map_err(source_emission_error_v18)?;
    let output_casts = source_issued_pointer_transport_census_v26(
        output_function.function,
        output_actual,
        &output_transports,
        budget,
    )
    .map_err(source_emission_error_v18)?;
    join_issued_pointer_transport_roles_v26(
        original,
        optimized,
        input_function.coordinate,
        output_function.coordinate,
        &input_casts,
        &output_casts,
        roles,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn join_issued_pointer_transport_roles_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    input_casts: &[ValueId],
    output_casts: &[ValueId],
    roles: &mut [DescriptorSourceRoleRowV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let selections = select_issued_transport_definitions_v26(
        original,
        optimized,
        input_function,
        input_casts,
        budget,
    )?;
    join_selected_issued_pointer_transport_roles_v26(
        original,
        optimized,
        input_function,
        output_function,
        input_casts,
        output_casts,
        &selections,
        roles,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn join_selected_issued_pointer_transport_roles_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    input_casts: &[ValueId],
    output_casts: &[ValueId],
    selections: &[IssuedTransportSelectionV26],
    roles: &mut [DescriptorSourceRoleRowV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    budget.charge_work(1)?;
    if selections.len() != input_casts.len() {
        return original
            .source
            .missing("issued transport selected definition census is incomplete");
    }
    let output = optimized.output_inventory(budget)?;
    let mut seen =
        emission_vec_v1(output_casts.len(), budget).map_err(source_emission_error_v18)?;
    budget.charge_work(output_casts.len())?;
    seen.resize(output_casts.len(), false);
    let mut previous = None;
    for id in output_casts {
        budget.charge_work(2)?;
        if previous.is_some_and(|old| old >= *id) {
            return original
                .source
                .missing("issued output transport census is not unique and ordered");
        }
        previous = Some(*id);
    }
    previous = None;
    for (input_value, selection) in input_casts.iter().zip(selections) {
        budget.charge_work(10)?;
        if previous.is_some_and(|old| old >= *input_value) {
            return original
                .source
                .missing("issued input transport census is not unique and ordered");
        }
        previous = Some(*input_value);
        let input_definition = original
            .inventory
            .definition_for_value(input_function, *input_value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued original transport result absent",
            ))?;
        let SliceDefinition::Result {
            operation: input_at,
            result: 0,
        } = input_definition.coordinate
        else {
            return original
                .source
                .missing("issued original transport is not a single result");
        };
        let SliceDefinition::Result {
            operation: output_at,
            result: 0,
        } = selection.output
        else {
            return original
                .source
                .missing("issued transport selected definition is not a cast result");
        };
        let expected_kind = match optimized.operation(input_at, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { input, output }
                if input == input_at && output == output_at =>
            {
                Kind::Retained
            }
            ProductionOptimizedSourceOperationV18::Rewritten { input } if input == input_at => {
                Kind::Substituted
            }
            _ => {
                return original
                    .source
                    .missing("issued transport selection changed checked disposition");
            }
        };
        if selection.input != input_definition.coordinate
            || selection.kind != expected_kind
            || output_at.block.function != output_function
        {
            return original
                .source
                .missing("issued transport changed function or original occurrence");
        }
        issued_metadata_descendant_v18(
            original,
            optimized,
            selection.input,
            selection.output,
            Some(expected_kind),
            budget,
        )?;
        let before = source_operation_row_v18(original.inventory, input_at, budget)?.operation;
        let after = source_operation_row_v18(output, output_at, budget)?.operation;
        let (
            OperationKind::Cast {
                kind: a,
                value: x,
                to: at,
            },
            OperationKind::Cast {
                kind: b,
                value: y,
                to: bt,
            },
        ) = (&before.kind, &after.kind)
        else {
            return original
                .source
                .missing("issued transport changed cast opcode");
        };
        let ([input_result], [output_result]) =
            (before.results.as_slice(), after.results.as_slice())
        else {
            return original
                .source
                .missing("issued transport changed result census");
        };
        let input_shape = source_issued_pointer_shape_v26(at);
        let output_shape = source_issued_pointer_shape_v26(bt);
        if !matches!(
            a,
            CastKind::PointerToGeneric | CastKind::RestrictPointerAccess
        ) || a != b
            || input_result.id != *input_value
            || input_shape.is_none()
            || input_shape != output_shape
            || source_issued_pointer_shape_v26(&input_result.ty) != input_shape
            || source_issued_pointer_shape_v26(&output_result.ty) != output_shape
        {
            return original
                .source
                .missing("issued transport changed exact pointer representation");
        }
        charge_execution_cfg_lookup_v29(output_casts.len(), budget)
            .map_err(source_emission_error_v18)?;
        let index = output_casts.binary_search(&output_result.id).map_err(|_| {
            ProductionSourceOwnedViewErrorV18::Binding(
                "issued retained cast absent from final exact transport paths",
            )
        })?;
        let first = !seen[index];
        seen[index] = true;
        let input_operand = original
            .inventory
            .definition_for_value(input_function, *x, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued original transport operand absent",
            ))?;
        let output_operand = output
            .definition_for_value(output_function, *y, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued final transport operand absent",
            ))?;
        issued_metadata_descendant_v18(
            original,
            optimized,
            input_operand.coordinate,
            output_operand.coordinate,
            None,
            budget,
        )?;
        if expected_kind == Kind::Retained {
            issued_output_operand_v18(original, optimized, input_at, output_at, 0, *y, budget)?;
        }
        let role_index = descriptor_role_index_v18(roles, output_at, budget)?;
        let role = roles
            .get_mut(role_index)
            .filter(|row| row.output == output_at)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued transport final role absent",
            ))?;
        budget.charge_work(7)?;
        if (role.role.is_some() && role.role != Some(DescriptorSourceRoleV18::PointerTransport))
            || role.instance.is_some()
            || role.site.is_some()
            || role.global.is_some()
            || role.write_recipe_pending
        {
            return original
                .source
                .missing("issued transport conflicts with another source role");
        }
        if first {
            // Several checked original definitions may share this pure output.
            // Keep one original anchor only after replaying every selected row;
            // no access instance, memory site or write proof is fabricated.
            role.input = Some(input_at);
            role.role = Some(DescriptorSourceRoleV18::PointerTransport);
        }
    }
    budget.charge_work(seen.len())?;
    if seen.iter().any(|value| !*value) {
        return original
            .source
            .missing("issued final transport cast lacks an exact original occurrence");
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_issued_pointer_transport_roles_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fault: u8,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let mut completed = 0;
    original.with_global_source_expressions_v23(
        optimized,
        budget,
        &mut |root, pending, budget| {
            let floor = budget.storage();
            let checked = (|| {
                budget.reserve_storage(size_of::<(
                    Vec<DescriptorSourceRoleRowV18>,
                    Vec<ValueId>,
                    Vec<ValueId>,
                    Vec<bool>,
                    [usize; 16],
                )>())?;
                let output = optimized.output_inventory(budget)?;
                let input_function = original.inventory.functions()
                    [original.source.root(root, budget)?.1]
                    .coordinate;
                let output_function =
                    optimized_source_root_function_v18(original, optimized, root, budget)?
                        .coordinate;
                let mut rows = emission_vec_v1(pending.roles.rows.len(), budget)
                    .map_err(source_emission_error_v18)?;
                let count = pending
                    .roles
                    .rows
                    .len()
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let mut input_casts =
                    emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
                let mut output_casts =
                    emission_vec_v1(count, budget).map_err(source_emission_error_v18)?;
                for row in pending.roles.rows {
                    budget.charge_work(8)?;
                    rows.push(*row);
                    if row.role != Some(DescriptorSourceRoleV18::PointerTransport) {
                        continue;
                    }
                    let input =
                        source_operation_row_v18(original.inventory, row.input.unwrap(), budget)?
                            .operation;
                    let after = source_operation_row_v18(output, row.output, budget)?.operation;
                    assert!(matches!(
                        input.kind,
                        OperationKind::Cast {
                            kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                            ..
                        }
                    ));
                    assert!(matches!(
                        after.kind,
                        OperationKind::Cast {
                            kind: CastKind::PointerToGeneric | CastKind::RestrictPointerAccess,
                            ..
                        }
                    ));
                    assert_eq!(row.instance, None);
                    assert!(
                        row.site.is_none() && row.global.is_none() && !row.write_recipe_pending
                    );
                    input_casts.push(input.results[0].id);
                    output_casts.push(after.results[0].id);
                }
                assert!(
                    !input_casts.is_empty(),
                    "each genuine helper root needs actual transport"
                );
                call_splice_sort_work_v1(input_casts.len(), budget)
                    .map_err(source_address_call_error_v29)
                    .map_err(source_emission_error_v18)?;
                call_splice_sort_work_v1(output_casts.len(), budget)
                    .map_err(source_address_call_error_v29)
                    .map_err(source_emission_error_v18)?;
                input_casts.sort_unstable();
                output_casts.sort_unstable();
                match fault {
                    0 => (),
                    1 => {
                        input_casts.pop();
                    }
                    2 => {
                        output_casts.pop();
                    }
                    3 => output_casts.push(*output_casts.last().unwrap()),
                    4 => {
                        let function = &original.inventory.functions()[input_function.0 as usize];
                        budget.charge_work(function.operations.len())?;
                        let noncast = original.inventory.operations()[function.operations.clone()]
                            .iter()
                            .find(|row| {
                                matches!(
                                    row.operation.kind,
                                    OperationKind::GetElementPointer { .. }
                                )
                            })
                            .unwrap()
                            .operation
                            .results[0]
                            .id;
                        input_casts[0] = noncast;
                        call_splice_sort_work_v1(input_casts.len(), budget)
                            .map_err(source_address_call_error_v29)
                            .map_err(source_emission_error_v18)?;
                        input_casts.sort_unstable();
                    }
                    5 => {
                        let function = &output.functions()[output_function.0 as usize];
                        budget.charge_work(function.operations.len())?;
                        let noncast = output.operations()[function.operations.clone()]
                            .iter()
                            .find(|row| {
                                matches!(
                                    row.operation.kind,
                                    OperationKind::GetElementPointer { .. }
                                )
                            })
                            .unwrap()
                            .operation
                            .results[0]
                            .id;
                        output_casts[0] = noncast;
                        call_splice_sort_work_v1(output_casts.len(), budget)
                            .map_err(source_address_call_error_v29)
                            .map_err(source_emission_error_v18)?;
                        output_casts.sort_unstable();
                    }
                    6..=8 => (),
                    _ => unreachable!(),
                }
                if fault >= 6 {
                    let mut selections = select_issued_transport_definitions_v26(
                        original,
                        optimized,
                        input_function,
                        &input_casts,
                        budget,
                    )?;
                    match fault {
                        6 => {
                            selections.pop();
                        }
                        7 => {
                            let foreign = output
                                .functions()
                                .iter()
                                .find(|row| {
                                    row.coordinate != output_function && row.function.body.is_some()
                                })
                                .expect("genuine other root");
                            let SliceDefinition::Result { operation, result } =
                                selections[0].output
                            else {
                                panic!("cast result");
                            };
                            selections[0].output = SliceDefinition::Result {
                                operation: SliceOperation {
                                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                        function: foreign.coordinate,
                                        block: operation.block.block,
                                    },
                                    operation: operation.operation,
                                },
                                result,
                            };
                        }
                        8 => {
                            selections[0].kind =
                                fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Substituted
                        }
                        _ => unreachable!(),
                    }
                    return join_selected_issued_pointer_transport_roles_v26(
                        original,
                        optimized,
                        input_function,
                        output_function,
                        &input_casts,
                        &output_casts,
                        &selections,
                        &mut rows,
                        budget,
                    );
                }
                join_issued_pointer_transport_roles_v26(
                    original,
                    optimized,
                    input_function,
                    output_function,
                    &input_casts,
                    &output_casts,
                    &mut rows,
                    budget,
                )
            })();
            let credit = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(credit)?;
            assert_eq!(budget.storage(), floor);
            checked?;
            completed += 1;
            Ok(())
        },
    )?;
    Ok(completed)
}
