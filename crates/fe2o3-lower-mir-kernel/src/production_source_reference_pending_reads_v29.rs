// These are pending source-result claims, not physical read-from or lifetime
// proofs. Only the raw transaction may use them before its complete memory gate.
fn source_reference_pending_read_cell_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    read: ScopedMemoryReadV29,
    access: SourceReferenceAccessV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut selected = None;
    let mut accept = |node: usize, budget: &mut ArgumentBudgetV1<'_>| {
        plan.charge(4, budget)?;
        let node = plan.nodes.get(node).ok_or_else(execution_call_error_v29)?;
        let SourceReferenceNodeKindV29::Address(set) = node.kind else {
            return Err(execution_call_error_v29());
        };
        if node.ty != read.ty {
            return Err(execution_call_error_v29());
        }
        let cell = plan.raw_object_cell(set, budget)?;
        if !matches!(
            plan.cells.rows.get(cell).map(|row| row.kind),
            Some(SourceBackingKindV29::Object(_))
        ) || selected.is_some_and(|previous| previous != cell)
        {
            return Err(execution_call_error_v29());
        }
        selected = Some(cell);
        Ok(())
    };
    if (read.prefix as usize) < place.projections().len() {
        let key = (
            source_reference_access_key_v29(site, place, access),
            read.prefix as usize,
        );
        charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
        let row = plan
            .raw_accesses
            .get(&key)
            .ok_or_else(execution_call_error_v29)?;
        accept(row.holder.node, budget)?;
    } else if let Some(statement) = site.statement {
        let function = plan
            .instances
            .instance(site.instance)
            .ok_or_else(execution_call_error_v29)?
            .declaration();
        plan.charge(3, budget)?;
        let source = function
            .blocks()
            .get(site.block.index() as usize)
            .and_then(|block| block.statements().get(statement))
            .ok_or_else(execution_call_error_v29)?;
        let destination = match source.kind() {
            SemanticStatementKindV1::Assign(assignment) => assignment.destination(),
            SemanticStatementKindV1::Store(store) => store.destination(),
            _ => return Err(execution_call_error_v29()),
        };
        let key =
            source_reference_access_key_v29(site, destination, SourceReferenceAccessV29::Write);
        charge_execution_cfg_lookup_v29(plan.representation_demand_sites.len(), budget)?;
        let rows = plan
            .representation_demand_sites
            .get(&key)
            .ok_or_else(execution_call_error_v29)?;
        for &row in rows {
            plan.charge(1, budget)?;
            accept(
                plan.representation_demands
                    .get(row)
                    .ok_or_else(execution_call_error_v29)?
                    .node,
                budget,
            )?;
        }
    } else {
        let function = plan
            .instances
            .instance(site.instance)
            .ok_or_else(execution_call_error_v29)?
            .declaration();
        plan.charge(3, budget)?;
        let block = function
            .blocks()
            .get(site.block.index() as usize)
            .ok_or_else(execution_call_error_v29)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err(execution_call_error_v29());
        };
        let ExecutionOperandV29::CallArgument(ordinal) = read.role else {
            return Err(execution_call_error_v29());
        };
        plan.charge(2, budget)?;
        if !matches!(call.arguments().get(ordinal as usize),
            Some(SemanticOperandV1::Copy(original) | SemanticOperandV1::Move(original))
                if std::ptr::eq(original, place))
        {
            return Err(execution_call_error_v29());
        }
        let role = SourceReferenceBoundaryRoleV29::Argument(ordinal);
        let (_, row) = plan
            .boundary_at(site, role, budget)?
            .ok_or_else(execution_call_error_v29)?;
        accept(row.node, budget)?;
    }
    selected.ok_or_else(execution_call_error_v29)
}

fn source_reference_pending_read_seeds_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    index: &SourceReferencePointerIndexV29<'_>,
    ordinals: &[SourceReferencePointerKeyV29],
    cell_slots: &[Option<usize>],
    seeds: &mut [SourceReferencePhysicalOriginV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    for (instance, lowered) in emitted.iter().enumerate() {
        let Some(lowered) = source_reference_active_emitted_v29(plan, instance, lowered, budget)?
        else {
            continue;
        };
        with_canonical_call_scratch_v1(budget, |budget| {
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            let mut blocks = source_reference_owned_vec_v29(plan, body.blocks.len(), budget)?;
            plan.charge(body.blocks.len(), budget)?;
            blocks.extend(body.blocks.iter().map(|block| (block.id, block)));
            call_splice_sort_work_v1(blocks.len(), budget)
                .map_err(source_address_call_error_v29)?;
            blocks.sort_unstable_by_key(|row| row.0);
            plan.charge(blocks.len(), budget)?;
            if blocks.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(execution_call_error_v29());
            }
            let id = plan
                .instances
                .id_at(instance)
                .ok_or_else(execution_call_error_v29)?;
            let anchors = lowered
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            anchors.check_object_ledger(budget)?;
            plan.charge(4, budget)?;
            let original = plan
                .instances
                .instance(id)
                .ok_or_else(execution_call_error_v29)?;
            if anchors.subject.instance != id
                || anchors.subject.function != original.function()
                || anchors.subject.source
                    != ExecutionCallSourceV29::from_instances(plan.instances, budget)?
            {
                return Err(scoped_memory_error_v29());
            }
            let occurrences = plan
                .instances
                .occurrences(id)
                .ok_or_else(scoped_memory_error_v29)?;
            for (anchor, row) in anchors.rows.iter().enumerate() {
                plan.charge(2, budget)?;
                if !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                    continue;
                }
                let payload = anchors.object_payload(row, budget)?;
                let ScopedObjectRoleV29::ReadValue {
                    read: ScopedObjectReadOriginV29::Original(read),
                    ..
                } = payload.role
                else {
                    continue;
                };
                let value = payload.result.ok_or_else(scoped_object_error_v29)?;
                let Some(&(ty, definition)) = source_reference_pointer_definition_lookup_v29(
                    &index.definitions,
                    (instance, value),
                    budget,
                )?
                else {
                    continue;
                };
                let SourceReferencePointerDefinitionV29::Operation(operation, 0) = definition
                else {
                    return Err(scoped_object_error_v29());
                };
                charge_execution_cfg_lookup_v29(blocks.len(), budget)?;
                let block = blocks
                    .binary_search_by_key(&row.block, |row| row.0)
                    .map_err(|_| scoped_object_error_v29())?;
                plan.charge(2, budget)?;
                if blocks[block]
                    .1
                    .operations
                    .get(row.position)
                    .is_none_or(|placed| !std::ptr::eq(placed, operation))
                {
                    return Err(scoped_object_error_v29());
                }
                payload.check_operation(operation, budget)?;
                anchors.check_object_source(
                    original.declaration(),
                    &occurrences,
                    anchor,
                    row,
                    payload,
                    budget,
                )?;
                let place =
                    scoped_object_original_place_v29(original.declaration(), read.site, read.role)
                        .ok_or_else(scoped_object_error_v29)?;
                let (block, statement) = scoped_memory_site_key_v29(read.site);
                let site = SourceReferenceSiteV29 {
                    instance: id,
                    block: SemanticBlockIdV1::from_index(block),
                    statement: statement.map(|index| index as usize),
                };
                let access = source_reference_raw_original_access_v29(
                    original.declaration(),
                    site,
                    place,
                    budget,
                )?
                .ok_or_else(scoped_object_error_v29)?;
                let Some((expected, _)) = source_reference_place_pointer_v29(
                    plan,
                    site,
                    place,
                    read.prefix as usize,
                    access,
                    budget,
                )?
                else {
                    continue;
                };
                if !invocation_equal_types_v1(ty, &expected, budget)? {
                    return Err(scoped_object_error_v29());
                }
                let cell = source_reference_pending_read_cell_v29(
                    plan, site, place, read, access, budget,
                )?;
                let slot = cell_slots
                    .get(cell)
                    .copied()
                    .flatten()
                    .ok_or_else(execution_call_error_v29)?;
                let definition =
                    source_reference_pointer_ordinal_v29(ordinals, (instance, value), budget)?;
                plan.charge(2, budget)?;
                let seed = seeds
                    .get_mut(definition)
                    .ok_or_else(execution_call_error_v29)?;
                if *seed != origin_worklist_v1::OriginStateV1::Unknown {
                    return Err(scoped_object_error_v29());
                }
                *seed = origin_worklist_v1::OriginStateV1::Exact(Some(slot));
            }
            Ok(())
        })?;
    }
    Ok(())
}
