// The recipe names an aggregate operand, not the value of an emitted index.
// Join the actual adjacent INDEX producer through immutable physical locators.
// This prerequisite does not itself complete array-subcell memory obligations.
fn check_source_array_component_index_v29(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    ordinal: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(0)?;
    if source_index.slot != std::ptr::from_ref(budget) as usize
        || source_index.ledger != budget.work_ledger_identity_v1()
        || budget.storage() < source_index.required
        || source_index.required < source_index.storage
    {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.charge_work(12)?;
    if source_index.pending.coordinates.ssa != instances.owner().identity()
        || source_index.pending.coordinates.semantic_sha256
            != *instances.owner().source_semantic_sha256()
        || source_index.pending.coordinates.root
            != instances
                .instance(instances.root())
                .ok_or_else(scoped_object_error_v29)?
                .function()
    {
        return Err(scoped_object_error_v29());
    }
    let sidecar = source_index.sidecar(instance, budget)?;
    let anchors = sidecar
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(scoped_object_error_v29)?;
    let row = anchors
        .rows
        .get(ordinal)
        .ok_or_else(scoped_object_error_v29)?;
    let payload = anchors.object_payload(row, budget)?;
    let ScopedObjectOperationV29::Project {
        step: ScopedObjectProjectionV29::ArrayIndex(index),
        ..
    } = payload.operation
    else {
        return Err(scoped_object_pending_v29());
    };
    let ScopedObjectRoleV29::Project { projected, .. } = payload.role else {
        return Err(scoped_object_error_v29());
    };
    let original = instances
        .instance(instance)
        .ok_or_else(scoped_object_error_v29)?
        .declaration();
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(scoped_object_error_v29)?;
    anchors.check_object_source(original, &occurrences, ordinal, row, payload, budget)?;
    let (site, expected) = match projected.source {
        ScopedObjectSourceV29::AggregateComponent {
            site,
            operand,
            variant: None,
            ..
        } => (site, u64::from(operand)),
        ScopedObjectSourceV29::Place {
            site,
            role,
            prefix: 1,
            ..
        } => {
            budget.charge_work(6)?;
            let place = scoped_object_original_place_v29(original, site, role)
                .ok_or_else(scoped_object_error_v29)?;
            let projection = place
                .projections()
                .first()
                .ok_or_else(scoped_object_error_v29)?;
            let SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } = projection.kind()
            else {
                return Err(scoped_object_pending_v29());
            };
            let Some(SemanticTypeShapeV1::Array { length, element }) = instances
                .owner()
                .source_semantic()
                .types()
                .get(projected.root_type.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Err(scoped_object_error_v29());
            };
            let expected =
                source_static_constant_index_v29(*length, offset, minimum_length, from_end)?;
            if *element != projection.result_type() || projected.projected_type != *element {
                return Err(scoped_object_error_v29());
            }
            (site, expected)
        }
        _ => return Err(scoped_object_pending_v29()),
    };
    let (block, statement) = scoped_memory_site_key_v29(site);
    let span = source_index.statement(
        SourceReferenceSiteV29 {
            instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|ordinal| ordinal as usize),
        },
        budget,
    )?;
    let predecessor = row
        .position
        .checked_sub(1)
        .ok_or_else(scoped_object_error_v29)?;
    budget.charge_work(10)?;
    if span.kernel_ir_block != row.block
        || predecessor < span.first_operation_ordinal as usize
        || row.position
            >= argument_sum_v1(&[
                span.first_operation_ordinal as usize,
                span.operation_count as usize,
            ])?
    {
        return Err(scoped_object_error_v29());
    }
    let actual = source_index
        .emitted
        .operation(instance, row.block, row.position, budget)?;
    payload.check_operation(actual, budget)?;
    let producer = source_index
        .emitted
        .operation(instance, row.block, predecessor, budget)?;
    let project_point = source_index
        .emitted
        .point(
            instance,
            row.block,
            u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            budget,
        )?
        .ok_or_else(scoped_object_error_v29)?;
    let producer_point = source_index
        .emitted
        .point(
            instance,
            row.block,
            u32::try_from(predecessor).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            budget,
        )?
        .ok_or_else(scoped_object_error_v29)?;
    budget.charge_work(8)?;
    if producer_point.0 != project_point.0
        || producer_point.1.checked_add(1) != Some(project_point.1)
        || !matches!(&producer.results[..], [definition] if definition.id == index && definition.ty == Type::INDEX)
        || producer.kind != OperationKind::Constant(Constant::Index(expected))
    {
        return Err(scoped_object_error_v29());
    }
    Ok(())
}

fn source_static_constant_index_v29(
    length: u64,
    offset: u64,
    minimum_length: u64,
    from_end: bool,
) -> Result<u64, ProductionSemanticKirErrorV1> {
    let index = if from_end {
        length
            .checked_sub(offset)
            .ok_or_else(scoped_object_error_v29)?
    } else {
        offset
    };
    if length < minimum_length || index >= length {
        return Err(scoped_object_error_v29());
    }
    Ok(index)
}
