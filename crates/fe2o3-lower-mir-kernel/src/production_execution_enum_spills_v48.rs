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
