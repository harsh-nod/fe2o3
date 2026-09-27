// Exact issued-pointer transport is separate from source Index/BoundsCheck
// recipes. This installer grants no native memory or read-from authority.
#[cfg(test)]
include!("production_optimized_source_issued_role_queries_v18_tests.rs");

struct IssuedRoleOutputV18 {
    input: [SliceOperation; 4],
    output: [SliceOperation; 4],
    physical: SourceIssuedPhysicalV29,
    root_input: ValueId,
    receiver: ValueId,
}

fn issued_output_definition_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: SliceDefinition,
    output: SliceDefinition,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    let mut found = false;
    for row in optimized.definition_descendants(input, budget)? {
        budget.charge_work(2)?;
        if row.output == output {
            if found || row.kind != Kind::Retained {
                return original.source.missing("issued output definition is not uniquely retained");
            }
            found = true;
        }
    }
    if !found { return original.source.missing("issued output definition lost its original producer"); }
    Ok(())
}

fn issued_output_operand_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: SliceOperation,
    output: SliceOperation,
    operand: u32,
    expected: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Usage;
    let actual = optimized.operand(Usage::OperationOperand { operation: input, operand }, budget)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output operand was removed"))?;
    let inventory = optimized.output_inventory(budget)?;
    let definition = optimized_source_definition_row_v18(inventory, actual.definition, budget)?;
    budget.charge_work(3)?;
    if actual.coordinate != (Usage::OperationOperand { operation: output, operand })
        || definition.value != Some(expected) {
        return original.source.missing("issued output operand changed occurrence or value");
    }
    Ok(())
}

fn issued_output_issuer_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    row: &PendingSourceIssuedIssuerV29,
    function: &CanonicalKirFunctionRefV1<'_>,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<IssuedRoleOutputV18>> {
    let input = scoped_raw_admission_v29::source_issued_tail_locations_v18(original, root, row, budget)?;
    let mut output = input;
    for (source, target) in input.iter().zip(&mut output) {
        let ProductionOptimizedSourceOperationV18::Retained { output, .. } = optimized.operation(*source, budget)?
            else { return Ok(None); };
        budget.charge_work(1)?;
        if output.block.function != function.coordinate {
            return original.source.missing("issued output tail changed root");
        }
        *target = output;
    }
    let inventory = optimized.output_inventory(budget)?;
    let length = optimized_source_operation_row_v18(inventory, output[0], budget)?.operation;
    let compare = optimized_source_operation_row_v18(inventory, output[1], budget)?.operation;
    let data = optimized_source_operation_row_v18(inventory, output[2], budget)?.operation;
    let address = optimized_source_operation_row_v18(inventory, output[3], budget)?.operation;
    let ([length_result], [present], [data_result], [pointer]) =
        (length.results.as_slice(), compare.results.as_slice(), data.results.as_slice(), address.results.as_slice())
        else { return original.source.missing("issued output tail result census"); };
    let OperationKind::SliceLength { slice: receiver } = length.kind
        else { return original.source.missing("issued output tail length opcode"); };
    let OperationKind::Compare { lhs: index, .. } = compare.kind
        else { return original.source.missing("issued output tail comparison opcode"); };
    let physical = SourceIssuedPhysicalV29 {
        present: present.id, pointer: pointer.id, element: row.element, access: row.access,
    };
    check_source_issued_tail_v29(physical, receiver, index, length, compare, data, address, budget)
        .map_err(source_emission_error_v18)?;
    for (ordinal, value) in [length_result.id, present.id, data_result.id, pointer.id].into_iter().enumerate() {
        issued_output_definition_v18(original, optimized,
            SliceDefinition::Result { operation: input[ordinal], result: 0 },
            SliceDefinition::Result { operation: output[ordinal], result: 0 }, budget)?;
        budget.charge_work(1)?;
        if inventory.definition_for_value(function.coordinate, value, budget)
            .map_err(source_pointer_inventory_error_v18)?.is_none_or(|definition|
                definition.coordinate != (SliceDefinition::Result { operation: output[ordinal], result: 0 })) {
            return original.source.missing("issued output actual result differs");
        }
    }
    for (operation, operand, value) in [(0, 0, receiver), (1, 0, index),
        (1, 1, length_result.id), (2, 0, receiver), (3, 0, data_result.id), (3, 1, index)] {
        issued_output_operand_v18(original, optimized, input[operation], output[operation], operand, value, budget)?;
    }
    let body = function.function.body.as_ref()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output root body"))?;
    let root_input = *body.parameters.get(row.root_parameter)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output root parameter"))?;
    let input_root = original.source.root(root, budget)?.1;
    let input_function = original.inventory.functions().get(input_root)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued original root coordinate"))?.coordinate;
    let original_parameter = original.inventory.definition_for_value(input_function, row.root_input, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued original root parameter definition"))?;
    let output_parameter = inventory.definition_for_value(function.coordinate, root_input, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output root parameter definition"))?;
    issued_output_definition_v18(original, optimized, original_parameter.coordinate, output_parameter.coordinate, budget)?;
    budget.charge_work(6)?;
    if !matches!(function.function.signature.parameters.get(row.root_parameter), Some(Type::Slice(slice))
        if slice.address_space == AddressSpace::Global && slice.access == row.access
            && *slice.element == Type::Scalar(row.element))
        || actual.value(root_input, budget).map_err(source_emission_error_v18)?.input != Some(row.root_parameter)
        || *actual.value(index, budget).map_err(source_emission_error_v18)?.ty != Type::INDEX {
        return original.source.missing("issued output root or index type differs");
    }
    Ok(Some(IssuedRoleOutputV18 { input, output, physical, root_input, receiver }))
}

fn install_optimized_issued_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    roles: &mut [DescriptorSourceRoleRowV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    optimized.check_exact_original_v18(original, budget)?;
    optimized_source_endpoints_v18(original, optimized, budget)?;
    let floor = budget.storage();
    let retained = scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        original.source.retain_construction(|| {
            budget.reserve_storage(issued_output_headers_v18()?)?;
            install_optimized_issued_roles_inner_v18(original, optimized, root, leaves, roles, budget)?;
            budget.storage().checked_sub(floor).ok_or(ArgumentResourceV1::Accounting.into())
        })
    })?;
    original.retain_query(budget.release_storage(retained).map_err(Into::into))
}

fn install_optimized_issued_roles_inner_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    roles: &mut [DescriptorSourceRoleRowV18],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Usage;
    let Some(rows) = scoped_raw_admission_v29::checked_issued_source_rows_v18(original, root, budget)?
        else { return Ok(()); };
    if rows.issuers.is_empty() { return Ok(()); }
    let function = optimized_source_root_function_v18(original, optimized, root, budget)?;
    let output = optimized.output_inventory(budget)?;
    let actual = SourceIssuedActualV29::from_function(function.function, budget).map_err(source_emission_error_v18)?;
    let mut guards = source_issued_guards_v29(function.function, &actual, budget).map_err(source_emission_error_v18)?;
    call_splice_sort_work_v1(guards.len(), budget).map_err(source_address_call_error_v29).map_err(source_emission_error_v18)?;
    guards.sort_unstable_by_key(|row| (row.present, row.block, row.edge));
    let mut facts = emission_vec_v1(rows.issuers.len(), budget).map_err(source_emission_error_v18)?;
    let mut order = emission_vec_v1(rows.issuers.len(), budget).map_err(source_emission_error_v18)?;
    for (index, issuer) in rows.issuers.iter().enumerate() {
        let fact = issued_output_issuer_v18(original, optimized, root, issuer, function, &actual, budget)?;
        budget.charge_work(2)?;
        facts.push(fact);
        order.push(index);
    }
    call_splice_sort_work_v1(order.len(), budget).map_err(source_address_call_error_v29).map_err(source_emission_error_v18)?;
    order.sort_unstable_by_key(|&index| (rows.issuers[index].instance.index(), rows.issuers[index].definition));
    let mut accesses = emission_vec_v1(rows.accesses.len(), budget).map_err(source_emission_error_v18)?;
    for row in &rows.accesses {
        charge_execution_cfg_lookup_v29(order.len(), budget).map_err(source_emission_error_v18)?;
        let index = order.binary_search_by_key(&(row.instance.index(), row.issuer), |&index|
            (rows.issuers[index].instance.index(), rows.issuers[index].definition))
            .map_err(|_| ProductionSourceOwnedViewErrorV18::Binding("issued output original issuer index"))?;
        let Some(issuer) = &facts[order[index]] else { continue; };
        let [position] = original.attachment_range(TileAttachmentKeyV29 {
            root, family: TileAttachmentFamilyV29::MemoryAnchor, instance: row.instance.index(), row: row.anchor,
            field: TileAttachmentFieldV29::MemoryPosition, component: 0, part: 0,
        }, budget)? else { return original.source.missing("issued output original access position"); };
        let ProductionSourceOperationV18::Operation(input) = original.mapped_source_operation(position.location, budget)?
            else { return original.source.missing("issued output original access coordinate"); };
        let transported = optimized.scalar_memory_access(root, input, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output scalar access receipt"))?;
        let Some(coordinate) = transported.output() else { continue; };
        budget.charge_work(4)?;
        if transported.instance() != row.instance.index() || transported.input() != input
            || coordinate.block.function != function.coordinate
            || !matches!((row.writing, transported.payload()),
                (true, Some(ProductionOptimizedSourcePayloadV18::Store { .. }))
                | (false, Some(ProductionOptimizedSourcePayloadV18::Load { .. }))) {
            return original.source.missing("issued output access payload or instance differs");
        }
        if !row.writing {
            let operation = optimized_source_operation_row_v18(original.inventory, input, budget)?.operation;
            let [result] = operation.results.as_slice()
                else { return original.source.missing("issued original read result census"); };
            let Some(leaf) = leaves.original.leaves.find([1, result.id.0 as usize, 0], budget)? else { continue; };
            if leaf.operation != input || leaf.value != result.id {
                return original.source.missing("issued original read scalar leaf differs");
            }
        }
        let operation = optimized_source_operation_row_v18(output, coordinate, budget)?.operation;
        let access = source_address_value_access_v29(operation).map_err(source_emission_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output memory opcode"))?;
        budget.charge_work(6)?;
        if access.object || access.pointer != issuer.physical.pointer || access.writing != row.writing
            || access.access != row.access || access.access.volatile
            || *actual.value(access.value, budget).map_err(source_emission_error_v18)?.ty != Type::Scalar(issuer.physical.element) {
            return original.source.missing("issued output memory attributes or pointer differ");
        }
        let guard = original.inventory.block_for_id(input.block.function, row.guard_block, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued original success guard block"))?;
        let condition = optimized.operand(Usage::TerminatorOperand { block: guard.coordinate, operand: 0 }, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output success condition removed"))?;
        let Usage::TerminatorOperand { block, operand: 0 } = condition.coordinate
            else { return original.source.missing("issued output success condition use changed"); };
        if block.function != function.coordinate {
            return original.source.missing("issued output success guard changed root");
        }
        let guard = optimized_source_block_row_v18(output, block, budget)?.block;
        let selector = optimized_source_definition_row_v18(output, condition.definition, budget)?.value
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding("issued output success condition definition"))?;
        budget.charge_work(3)?;
        if !matches!(guard.terminator.as_ref(),
            Some(Terminator::ConditionalBranch { condition, .. }) if *condition == selector)
            && !matches!(guard.terminator.as_ref(), Some(Terminator::Switch { selector: value, .. }
                | Terminator::IntegerSwitch { selector: value, .. }) if *value == selector) {
            return original.source.missing("issued output success condition differs from actual terminator");
        }
        if actual.present(selector, budget).map_err(source_emission_error_v18)? != Some(issuer.physical.present) {
            return original.source.missing("issued output success guard changed issuer condition");
        }
        charge_execution_cfg_lookup_v29(guards.len(), budget).map_err(source_emission_error_v18)?;
        if guards.binary_search_by_key(&(issuer.physical.present, guard.id, row.guard_edge),
            |guard| (guard.present, guard.block, guard.edge)).is_err() {
            return original.source.missing("issued output success edge changed polarity");
        }
        let target = optimized_source_block_row_v18(output, coordinate.block, budget)?.block.id;
        accesses.push((guard.id, row.guard_edge, target));
        install_descriptor_role_v18(original, optimized, roles, input, coordinate, row.instance.index(), None,
            if row.writing { DescriptorSourceRoleV18::Write } else { DescriptorSourceRoleV18::Read }, budget)?;
    }
    let mut valid = true;
    budget.charge_work(argument_sum_v1(&[facts.len(), accesses.len()])?)?;
    fe2o3_kernel_ir::with_function_control_flow_v1(function.function, Default::default(), budget, |view| {
        for fact in facts.iter().flatten() {
            if fact.receiver != fact.root_input && view.unique_value_origin(fact.receiver)? != Some(fact.root_input) {
                valid = false;
            }
        }
        for &(guard, edge, access) in &accesses {
            if !view.success_edge_dominates(guard, edge, access)? { valid = false; }
        }
        Ok(())
    }).map_err(|error| match error {
        fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
        _ => ProductionSourceOwnedViewErrorV18::Binding("issued output CFG query refused"),
    })?;
    if !valid { return original.source.missing("issued output root transport or success dominance differs"); }
    for (index, fact) in facts.iter().enumerate() {
        let Some(fact) = fact else { continue; };
        for (ordinal, role) in [(0, DescriptorSourceRoleV18::Length), (2, DescriptorSourceRoleV18::Data),
            (3, DescriptorSourceRoleV18::Address)] {
            install_descriptor_role_v18(original, optimized, roles, fact.input[ordinal], fact.output[ordinal],
                rows.issuers[index].instance.index(), None, role, budget)?;
        }
    }
    Ok(())
}

fn issued_output_headers_v18() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[size_of::<T>(), argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?])
    }
    argument_sum_v1(&[
        scoped_raw_admission_v29::source_issued_replay_headers_v18()?,
        h::<Option<&PendingSourceIssuedRolesV29>>()?, h::<&PendingSourceIssuedRolesV29>()?,
        argument_product_v1(3, h::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?)?,
        h::<SliceOperation>()?, h::<Option<SliceOperation>>()?,
        h::<IssuedRoleOutputV18>()?, h::<Option<IssuedRoleOutputV18>>()?,
        h::<&IssuedRoleOutputV18>()?, h::<Option<&IssuedRoleOutputV18>>()?,
        h::<&Option<IssuedRoleOutputV18>>()?, h::<Option<&Option<IssuedRoleOutputV18>>>()?,
        h::<Vec<Option<IssuedRoleOutputV18>>>()?,
        size_of::<Result<Vec<Option<IssuedRoleOutputV18>>, ProductionSemanticKirErrorV1>>(),
        h::<ProductionOptimizedSourceOperationV18>()?,
        h::<ProductionOptimizedSourceMemoryAccessV18<'_>>()?,
        h::<Option<ProductionOptimizedSourceMemoryAccessV18<'_>>>()?,
        h::<Option<ProductionOptimizedSourcePayloadV18>>()?,
        h::<ProductionOptimizedSourcePayloadV18>()?,
        h::<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>()?,
        h::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>()?,
        h::<fe2o3_kernel_ir::CanonicalKirUseCoordinateV1>()?,
        h::<&SourceScalarLeafRowV18>()?, h::<Option<&SourceScalarLeafRowV18>>()?,
        h::<[usize; 3]>()?,
        argument_product_v1(2, h::<SliceDefinition>()?)?,
        argument_product_v1(2, h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()?)?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>>()?,
        size_of::<Result<Option<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>, CanonicalKirInventoryErrorV1>>(),
        h::<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>()?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>>()?,
        size_of::<Result<Option<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>, CanonicalKirInventoryErrorV1>>(),
        h::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>()?,
        h::<std::slice::Iter<'_, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>>()?,
        h::<&fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>()?,
        h::<Option<&fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>>()?,
        h::<std::slice::Iter<'_, Option<IssuedRoleOutputV18>>>()?,
        h::<std::iter::Flatten<std::slice::Iter<'_, Option<IssuedRoleOutputV18>>>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, Option<IssuedRoleOutputV18>>>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, PendingSourceIssuedIssuerV29>>>()?,
        h::<Option<(usize, &PendingSourceIssuedIssuerV29)>>()?,
        h::<(usize, &PendingSourceIssuedIssuerV29)>()?,
        h::<Option<(usize, &Option<IssuedRoleOutputV18>)>>()?,
        h::<(usize, &Option<IssuedRoleOutputV18>)>()?,
        h::<std::iter::Zip<std::slice::Iter<'_, SliceOperation>, std::slice::IterMut<'_, SliceOperation>>>()?,
        h::<Option<(&SliceOperation, &mut SliceOperation)>>()?,
        h::<(&SliceOperation, &mut SliceOperation)>()?,
        h::<&SliceOperation>()?, h::<&mut SliceOperation>()?,
        // The original and mapped coordinate arrays coexist until publication.
        h::<[SliceOperation; 4]>()?,
        argument_product_v1(4, h::<&[fe2o3_kernel_ir::ValueDef]>()?)?,
        h::<[ValueId; 4]>()?, h::<std::iter::Enumerate<std::array::IntoIter<ValueId, 4>>>()?,
        h::<Option<(usize, ValueId)>>()?, h::<(usize, ValueId)>()?,
        h::<[(usize, u32, ValueId); 6]>()?, h::<std::array::IntoIter<(usize, u32, ValueId), 6>>()?,
        h::<Option<(usize, u32, ValueId)>>()?, h::<(usize, u32, ValueId)>()?,
        h::<[(usize, DescriptorSourceRoleV18); 3]>()?,
        h::<std::array::IntoIter<(usize, DescriptorSourceRoleV18), 3>>()?,
        h::<Option<(usize, DescriptorSourceRoleV18)>>()?, h::<(usize, DescriptorSourceRoleV18)>()?,
        h::<Option<&Terminator>>()?,
        // Two nested installer captures borrow this fixed argument roster.
        argument_product_v1(2, size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>, usize,
            &ProductionOptimizedSourceScalarLeavesV18<'_>,
            &mut [DescriptorSourceRoleRowV18], usize,
        )>())?,
        size_of::<(&Vec<Option<IssuedRoleOutputV18>>, &Vec<(BlockId, usize, BlockId)>, &mut bool)>(),
        // Sort/search closures reuse one borrowed roster and one copied key.
        size_of::<(&PendingSourceIssuedRolesV29, (usize, SsaValueV1))>(),
        size_of::<(ValueId, BlockId, usize)>(),
    ])
}
