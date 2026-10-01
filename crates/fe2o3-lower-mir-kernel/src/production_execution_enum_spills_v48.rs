// Compiler-private enum payload storage is not an original source allocation.
#[derive(Debug, Eq, PartialEq)]
#[cfg_attr(test, derive(Clone))]
struct ExecutionEnumSpillV48 {
    local: u32,
    source_type: SemanticTypeIdV1,
    variant: u32,
    field: u32,
    field_type: SemanticTypeIdV1,
    component: usize,
    emitted_block: BlockId,
    emitted_operation: usize,
    pointer: ValueId,
    element: Type,
    alignment: u32,
}

type ExecutionEnumSpillFrameV48<'a> = (
    &'a SemanticFunctionLoweringV1<'a, 'a>,
    &'a [BasicBlock],
    &'a [SemanticKirSyntheticOperationSpanV1],
    Option<&'a SemanticKirSyntheticOperationSpanV1>,
    &'a BasicBlock,
    &'a Operation,
    &'a SemanticTypeDeclV1,
    &'a SemanticEnumPayloadFieldStorageV1,
    &'a SemanticEnumPayloadComponentStorageV1,
    std::collections::btree_map::Iter<'a, (u32, u32, u32), SemanticEnumPayloadFieldStorageV1>,
    std::slice::Iter<'a, SemanticEnumPayloadComponentStorageV1>,
    Vec<ExecutionEnumSpillV48>,
    SemanticTypeIdV1,
    SemanticTypeIdV1,
    (&'a Type, &'a Type),
    [usize; 8],
);

fn enum_spill_types_equal_v48(
    mut left: &Type,
    mut right: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    loop {
        budget.charge_work(1)?;
        match (left, right) {
            (Type::Pointer(a), Type::Pointer(b))
                if a.address_space == b.address_space && a.access == b.access =>
            {
                (left, right) = (&a.pointee, &b.pointee);
            }
            (Type::Slice(a), Type::Slice(b))
                if a.address_space == b.address_space && a.access == b.access =>
            {
                (left, right) = (&a.element, &b.element);
            }
            (Type::Unit, Type::Unit) => return Ok(true),
            (Type::Scalar(a), Type::Scalar(b)) => return Ok(a == b),
            (Type::Vector(a), Type::Vector(b)) => return Ok(a == b),
            (Type::StorageObject(a), Type::StorageObject(b)) => return Ok(a == b),
            _ => return Ok(false),
        }
    }
}

fn capture_execution_enum_spills_v48(
    lowering: &SemanticFunctionLoweringV1<'_, '_>,
    blocks: &[BasicBlock],
    synthetic: &[SemanticKirSyntheticOperationSpanV1],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<ExecutionEnumSpillV48>, ProductionSemanticKirErrorV1> {
    let headers = source_reference_emission_headers_v29::<ExecutionEnumSpillFrameV48<'_>>()?;
    budget.reserve_storage(headers)?;
    budget.charge_work(6)?;
    let mut count = 0;
    for field in lowering.enum_payload_storage.values() {
        budget.charge_work(1)?;
        count = argument_sum_v1(&[count, field.components.len()])?;
    }
    let mut span = None;
    for candidate in synthetic {
        budget.charge_work(2)?;
        if candidate.rule != SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage {
            continue;
        }
        if span.is_some()
            || candidate.correspondence_owner != lowering.correspondence_owner
            || candidate.semantic_function != lowering.semantic_function
            || candidate.operation_count as usize != count
        {
            return Err(execution_archive_error_v29());
        }
        span = Some(candidate);
    }
    let mut rows = emission_vec_v1(count, budget)?;
    if count == 0 {
        if span.is_some() {
            return Err(execution_archive_error_v29());
        }
        budget.release_storage(headers)?;
        return Ok(rows);
    }
    if !lowering.enum_payload_allocas_emitted {
        return Err(execution_archive_error_v29());
    }
    let span = span.ok_or_else(execution_archive_error_v29)?;
    budget.charge_work(blocks.len())?;
    let block = blocks
        .iter()
        .find(|block| block.id == span.kernel_ir_block)
        .ok_or_else(execution_archive_error_v29)?;
    let first = span.first_operation_ordinal as usize;
    let end = argument_sum_v1(&[first, count])?;
    if end > block.operations.len() {
        return Err(execution_archive_error_v29());
    }
    for (&(local, variant, field), storage) in &lowering.enum_payload_storage {
        budget.charge_work(12)?;
        let source_type = lowering
            .function
            .locals()
            .get(local as usize)
            .ok_or_else(execution_archive_error_v29)?
            .ty();
        let original = lowering
            .types
            .get(source_type.index() as usize)
            .ok_or_else(execution_archive_error_v29)?;
        let SemanticTypeShapeV1::Enum { variants, .. } = original.shape() else {
            return Err(execution_archive_error_v29());
        };
        let field_type = *variants
            .get(variant as usize)
            .and_then(|variant| variant.fields().fields().get(field as usize))
            .ok_or_else(execution_archive_error_v29)?;
        charge_execution_cfg_lookup_v29(lowering.control_flow_ssa.promoted.len(), budget)?;
        let promoted = lowering
            .control_flow_ssa
            .promoted
            .get(&local)
            .ok_or_else(execution_archive_error_v29)?;
        if promoted.semantic_type != source_type
            || !promoted.transport.uses_structural_enum_transport()
        {
            return Err(execution_archive_error_v29());
        }
        for (component, storage) in storage.components.iter().enumerate() {
            budget.charge_work(12)?;
            let emitted_operation = argument_sum_v1(&[first, rows.len()])?;
            let operation = &block.operations[emitted_operation];
            let OperationKind::Alloca {
                element,
                count: None,
                address_space: AddressSpace::Private,
                alignment,
            } = &operation.kind
            else {
                return Err(execution_archive_error_v29());
            };
            let [result] = operation.results.as_slice() else {
                return Err(execution_archive_error_v29());
            };
            let Type::Pointer(pointer) = &result.ty else {
                return Err(execution_archive_error_v29());
            };
            if result.id != storage.pointer
                || *alignment != storage.alignment
                || pointer.address_space != AddressSpace::Private
                || pointer.access != AccessMode::ReadWrite
                || !enum_spill_types_equal_v48(element, &storage.kernel_type, budget)?
                || !enum_spill_types_equal_v48(&pointer.pointee, element, budget)?
            {
                return Err(execution_archive_error_v29());
            }
            if rows.len() == rows.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            rows.push(ExecutionEnumSpillV48 {
                local,
                source_type,
                variant,
                field,
                field_type,
                component,
                emitted_block: block.id,
                emitted_operation,
                pointer: result.id,
                element: emission_binding_clone_type_v1(element, budget)?,
                alignment: *alignment,
            });
        }
    }
    if rows.len() != count {
        return Err(execution_archive_error_v29());
    }
    budget.release_storage(headers)?;
    Ok(rows)
}

type ScopedEnumSpillCensusFrameV55<'a> = (
    &'a ExecutionArchiveV29,
    &'a [ExecutionEnumSpillV48],
    &'a [ScopedSourceSlotV29],
    &'a [SemanticKirSyntheticOperationSpanV1],
    Option<&'a SemanticKirSyntheticOperationSpanV1>,
    &'a BasicBlock,
    &'a Operation,
    &'a SemanticTypeDeclV1,
    (&'a Type, &'a Type),
    std::slice::Iter<'a, ExecutionEnumSpillV48>,
    std::slice::Iter<'a, ScopedSourceSlotV29>,
    std::slice::Iter<'a, SemanticKirSyntheticOperationSpanV1>,
    Vec<ValueId>,
    Option<(u32, u32, u32, usize)>,
    [usize; 12],
);

#[allow(clippy::too_many_arguments)]
// Original slots arrive after their existing source/allocation checks. Compiler
// spills must independently match the paid emission archive and actual graph.
fn check_scoped_allocation_census_v55(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    entry: BlockId,
    first: usize,
    source_slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    source_reference_emission_prepay_v29::<ScopedEnumSpillCensusFrameV55<'_>>(budget)?;
    budget.charge_work(6)?;
    if lowered.source_call_instance != Some(instance) {
        return Err(execution_archive_error_v29());
    }
    let archive = lowered
        .execution_observation
        .as_ref()
        .ok_or_else(execution_archive_error_v29)?;
    archive.check_original_v29(instances, instance, budget)?;
    let original = instances
        .instance(instance)
        .ok_or_else(execution_archive_error_v29)?;
    let spills = &archive.enum_spills;
    let mut span = None;
    for candidate in &lowered.synthetic_operation_spans {
        budget.charge_work(6)?;
        if candidate.rule != SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage {
            continue;
        }
        if span.is_some()
            || candidate.correspondence_owner != archive.subject.source.root
            || candidate.semantic_function != original.function()
            || candidate.kernel_ir_block != entry
            || candidate.first_operation_ordinal as usize != first
            || candidate.operation_count as usize != spills.len()
        {
            return Err(execution_archive_error_v29());
        }
        span = Some(candidate);
    }
    if span.is_some() != !spills.is_empty() {
        return Err(execution_archive_error_v29());
    }
    let body = lowered
        .function
        .body
        .as_ref()
        .ok_or_else(execution_archive_error_v29)?;
    budget.charge_work(body.blocks.len())?;
    let block = body
        .blocks
        .iter()
        .find(|block| block.id == entry)
        .ok_or_else(execution_archive_error_v29)?;
    let end = argument_sum_v1(&[first, spills.len()])?;
    if end > block.operations.len() {
        return Err(execution_archive_error_v29());
    }
    let total = argument_sum_v1(&[source_slots.len(), spills.len()])?;
    let mut pointers = emission_vec_v1(total, budget)?;
    for slot in source_slots {
        budget.charge_work(3)?;
        if slot.instance != instance
            || slot.allocation.block != entry
            || slot.allocation.operation >= first
        {
            return Err(execution_archive_error_v29());
        }
        pointers.push(slot.origin.pointer);
    }
    let mut previous: Option<(u32, u32, u32, usize)> = None;
    let types = instances.owner().source_semantic().types();
    for (ordinal, spill) in spills.iter().enumerate() {
        budget.charge_work(22)?;
        let key = (spill.local, spill.variant, spill.field, spill.component);
        let next_component = match previous {
            Some((local, variant, field, component))
                if (local, variant, field) == (spill.local, spill.variant, spill.field) =>
            {
                argument_sum_v1(&[component, 1])?
            }
            _ => 0,
        };
        if previous.is_some_and(|prior| prior >= key)
            || spill.component != next_component
            || spill.emitted_block != entry
            || spill.emitted_operation != argument_sum_v1(&[first, ordinal])?
            || original
                .declaration()
                .locals()
                .get(spill.local as usize)
                .map(|local| local.ty())
                != Some(spill.source_type)
        {
            return Err(execution_archive_error_v29());
        }
        previous = Some(key);
        let promoted = original.ssa().plan().promoted_variables();
        charge_execution_cfg_lookup_v29(promoted.len(), budget)?;
        if promoted
            .binary_search_by_key(&spill.local, |variable| variable.get())
            .is_err()
        {
            return Err(execution_archive_error_v29());
        }
        let Some(SemanticTypeShapeV1::Enum { variants, .. }) = types
            .get(spill.source_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(execution_archive_error_v29());
        };
        if variants
            .get(spill.variant as usize)
            .and_then(|variant| variant.fields().fields().get(spill.field as usize))
            != Some(&spill.field_type)
        {
            return Err(execution_archive_error_v29());
        }
        let operation = &block.operations[spill.emitted_operation];
        let OperationKind::Alloca {
            element,
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        } = &operation.kind
        else {
            return Err(execution_archive_error_v29());
        };
        let [result] = operation.results.as_slice() else {
            return Err(execution_archive_error_v29());
        };
        let Type::Pointer(pointer) = &result.ty else {
            return Err(execution_archive_error_v29());
        };
        if result.id != spill.pointer
            || *alignment != spill.alignment
            || pointer.address_space != AddressSpace::Private
            || pointer.access != AccessMode::ReadWrite
            || !enum_spill_types_equal_v48(element, &spill.element, budget)?
            || !enum_spill_types_equal_v48(&pointer.pointee, element, budget)?
        {
            return Err(execution_archive_error_v29());
        }
        pointers.push(spill.pointer);
    }
    // Prove the two allocation classes disjoint without rescanning each prior
    // allocation for every spill. Neither class acquires the other's authority.
    call_splice_sort_work_v1(pointers.len(), budget).map_err(source_address_call_error_v29)?;
    pointers.sort_unstable();
    budget.charge_work(pointers.len())?;
    if pointers.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(execution_archive_error_v29());
    }
    let mut actual_allocations = 0_usize;
    for block in &body.blocks {
        budget.charge_work(argument_sum_v1(&[block.operations.len(), 1])?)?;
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::Alloca { .. }) {
                actual_allocations = argument_sum_v1(&[actual_allocations, 1])?;
            }
        }
    }
    if actual_allocations != total {
        return Err(scoped_slot_error_v29());
    }
    drop(pointers);
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(())
}
