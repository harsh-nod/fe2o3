// Original static selections are hypotheses compared with independently solved
// actual Project equations. They never become provenance seeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceObjectProjectV29 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    source: SourceStaticObjectLocationV29,
    projected: SourceStaticObjectLocationV29,
}

fn source_static_object_expected_location_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1> {
    with_source_static_object_location_query_v29(instances, plan, budget, |budget| {
        source_static_object_expected_location_inner_v29(
            instances, plan, source_index, slots, endpoint, budget,
        )
    })
}

fn with_source_static_object_location_query_v29<'work>(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'work>,
    query: impl FnOnce(&mut ArgumentBudgetV1<'work>)
        -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1>,
) -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let mut location = None;
    with_canonical_call_scratch_v1(budget, |budget| {
        location = Some(query(budget)?);
        // Only Copy coordinates escape. Query values and their prepaid return
        // envelopes die before the unit-returning scratch scope is refunded.
        Ok(())
    }).inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    location.ok_or(ArgumentResourceV1::Accounting.into())
}

fn source_static_object_expected_location_inner_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1> {
    let ScopedObjectIdentityV29::Local { instance, .. } = endpoint.object else {
        // The existing whole-cell raw-holder proof remains responsible for
        // references. Projected reference/holder paths are not admitted here.
        if endpoint.path.count != 0 || endpoint.root_schema != endpoint.projected_schema {
            return Err(scoped_object_pending_v29());
        }
        return Ok(None);
    };
    if matches!(endpoint.source, ScopedObjectSourceV29::EntryArgument { .. }) {
        source_index.sidecar(instance, budget)?;
        let slot = source_address_object_entry_v29(instances, plan, slots, instance, endpoint, budget)?;
        return Ok(Some(SourceStaticObjectLocationV29 { slot, offset: 0, schema: Some(endpoint.root_schema) }));
    }
    if let ScopedObjectSourceV29::ProjectionIndex(read) = endpoint.source {
        source_index.sidecar(instance, budget)?;
        let slot = source_address_object_index_v29(instances, plan, slots, instance, read, endpoint, budget)?;
        return Ok(Some(SourceStaticObjectLocationV29 { slot, offset: 0, schema: Some(endpoint.root_schema) }));
    }
    let (site, role, prefix, generated_operand) = match endpoint.source {
        ScopedObjectSourceV29::Place {
            site, role, prefix, ..
        } => (site, role, prefix, None),
        ScopedObjectSourceV29::AggregateComponent {
            site,
            operand,
            variant: None,
            ..
        } => (site, ExecutionOperandV29::Destination, 0, Some(operand)),
        _ => return Err(scoped_object_pending_v29()),
    };
    source_index.sidecar(instance, budget)?;
    let original = instances
        .instance(instance)
        .ok_or_else(scoped_object_error_v29)?
        .declaration();
    let place = scoped_object_original_place_v29(original, site, role)
        .ok_or_else(scoped_object_error_v29)?;
    let generated = if let Some(operand) = generated_operand {
        let Some(SemanticStatementKindV1::Assign(assignment)) =
            scoped_source_statement_v29(original, site)
        else {
            return Err(scoped_object_error_v29());
        };
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return Err(scoped_object_error_v29());
        };
        let types = instances.owner().source_semantic().types();
        budget.charge_work(10)?;
        let declaration = types
            .get(place.ty().index() as usize)
            .ok_or_else(scoped_object_error_v29)?;
        require_ordinary_execution_representation_v29(declaration)?;
        let fields = match (declaration.shape(), aggregate.kind()) {
            (SemanticTypeShapeV1::Tuple(fields), SemanticAggregateKindV1::Tuple)
            | (SemanticTypeShapeV1::Aggregate(fields), SemanticAggregateKindV1::Aggregate) => {
                fields.fields()
            }
            _ => return Err(scoped_object_pending_v29()),
        };
        let ty = *fields
            .get(operand as usize)
            .ok_or_else(scoped_object_error_v29)?;
        if !place.projections().is_empty()
            || fields.len() != aggregate.operands().len()
            || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1
            || endpoint.path.count != 1
            || aggregate
                .operands()
                .get(operand as usize)
                .map(|operand| operand.ty())
                != Some(ty)
            || endpoint.projected_type != ty
            || endpoint.source_path.count != 0
            || !matches!(
                types
                    .get(ty.index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            )
        {
            return Err(scoped_object_error_v29());
        }
        Some(
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(operand), ty)
                .map_err(|_| ArgumentResourceV1::Accounting)?,
        )
    } else {
        None
    };
    let (block, statement) = scoped_memory_site_key_v29(site);
    let site = SourceReferenceSiteV29 {
        instance,
        block: SemanticBlockIdV1::from_index(block),
        statement: statement.map(|row| row as usize),
    };
    let access = source_reference_raw_original_access_v29(original, site, place, budget)?
        .ok_or_else(scoped_object_error_v29)?;
    if !matches!(
        access,
        SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
    ) {
        return Err(scoped_object_pending_v29());
    }
    let slot = source_address_object_direct_v29(
        instances, plan, slots, site, place, access, endpoint, budget,
    )?;
    let path = if let Some(projection) = &generated {
        std::slice::from_ref(projection)
    } else {
        place
            .projections()
            .get(..prefix as usize)
            .ok_or_else(scoped_object_error_v29)?
    };
    if path.len() > 1 || endpoint.path.count != path.len() {
        return Err(scoped_object_pending_v29());
    }
    budget.charge_work(path.len())?;
    if path
        .iter()
        .any(|projection| !matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
    {
        return Err(scoped_object_pending_v29());
    }
    let layouts = plan
        .storage_root
        .as_ref()
        .ok_or(ArgumentResourceV1::Accounting)?
        .source_layouts(instances, budget)?;
    let mut offset = 0u64;
    let mut ty = endpoint.root_type;
    let mut schema = endpoint.root_schema;
    let selected = layouts.visit_selected_components(
        instances.owner(),
        ty,
        schema,
        path,
        budget,
        |component, budget| {
            budget.charge_work(6)?;
            if component.source_type != ty || component.source_schema != schema {
                return Err(scoped_object_error_v29());
            }
            let displacement = match component.kind {
                source_storage_v29::SourceSelectedComponentKindV29::Field {
                    byte_offset, ..
                } => byte_offset,
                _ => return Err(scoped_object_pending_v29()),
            };
            offset = offset
                .checked_add(displacement)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            ty = component.result_type;
            schema = component
                .result_schema
                .ok_or_else(scoped_object_error_v29)?;
            Ok(())
        },
    )?;
    if ty != endpoint.projected_type
        || schema != endpoint.projected_schema
        || !matches!(selected, source_storage_v29::SourceSelectedProjectionV29::Physical { ty: t, schema: s } if t == ty && s == schema)
    {
        return Err(scoped_object_error_v29());
    }
    if !path.is_empty()
        && !matches!(
            instances
                .owner()
                .source_semantic()
                .types()
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        )
    {
        return Err(scoped_object_pending_v29());
    }
    Ok(Some(SourceStaticObjectLocationV29 {
        slot,
        offset,
        schema: Some(schema),
    }))
}

fn check_source_static_object_projects_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<PendingSourceObjectProjectV29>, ProductionSemanticKirErrorV1> {
    let mut projects = emission_vec_v1(graph.projections.len(), budget)?;
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(std::mem::size_of::<(Vec<bool>, Vec<(usize, u32)>)>())?;
        let mut seen = emission_vec_v1(graph.projections.len(), budget)?;
        let mut original_keys = emission_vec_v1(graph.projections.len(), budget)?;
        budget.charge_work(graph.projections.len())?;
        seen.resize(graph.projections.len(), false);
        for sidecar in &source_index.pending.sidecars.rows {
            let instance = sidecar
                .source_call_instance
                .ok_or_else(scoped_object_error_v29)?;
            let original = instances
                .instance(instance)
                .ok_or_else(scoped_object_error_v29)?
                .declaration();
            let occurrences = instances
                .occurrences(instance)
                .ok_or_else(scoped_object_error_v29)?;
            let anchors = sidecar
                .scoped_memory_anchors
                .as_ref()
                .ok_or_else(scoped_object_error_v29)?;
            for (ordinal, row) in anchors.rows.iter().enumerate() {
                budget.charge_work(1)?;
                if !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                    continue;
                }
                let payload = anchors.object_payload(row, budget)?;
                let ScopedObjectOperationV29::Project { base, .. } = payload.operation else {
                    continue;
                };
                let ScopedObjectRoleV29::Project { source, projected } = payload.role else {
                    return Err(scoped_object_error_v29());
                };
                let actual = source_address_original_operation_v29(
                    source_index.pending,
                    graph,
                    instance,
                    row.block,
                    row.position,
                    budget,
                )?;
                payload.check_operation(actual, budget)?;
                anchors.check_object_source(
                    original,
                    &occurrences,
                    ordinal,
                    row,
                    payload,
                    budget,
                )?;
                let (site, role, component) = match projected.source {
                    ScopedObjectSourceV29::AggregateComponent {
                        site,
                        operand,
                        variant: None,
                        ..
                    } => (site, ExecutionOperandV29::Destination, operand),
                    ScopedObjectSourceV29::Place {
                        site,
                        role,
                        prefix: 1,
                        ..
                    } => (site, role, u32::MAX),
                    _ => return Err(scoped_object_pending_v29()),
                };
                let place = scoped_object_original_place_v29(original, site, role)
                    .ok_or_else(scoped_object_error_v29)?;
                let (block, statement) = scoped_memory_site_key_v29(site);
                let site = SourceReferenceSiteV29 {
                    instance,
                    block: SemanticBlockIdV1::from_index(block),
                    statement: statement.map(|ordinal| ordinal as usize),
                };
                let access =
                    source_reference_raw_original_access_v29(original, site, place, budget)?
                        .ok_or_else(scoped_object_error_v29)?;
                if !matches!(
                    access,
                    SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
                ) {
                    return Err(scoped_object_pending_v29());
                }
                charge_execution_cfg_lookup_v29(plan.access_sites.len(), budget)?;
                let access_ordinal = *plan
                    .access_sites
                    .get(&source_reference_access_key_v29(site, place, access))
                    .ok_or_else(scoped_object_error_v29)?;
                let source = source_static_object_expected_location_v29(
                    instances,
                    plan,
                    source_index,
                    slots,
                    source,
                    budget,
                )?
                .ok_or_else(scoped_object_pending_v29)?;
                let projected = source_static_object_expected_location_v29(
                    instances,
                    plan,
                    source_index,
                    slots,
                    projected,
                    budget,
                )?
                .ok_or_else(scoped_object_pending_v29)?;
                let result = payload.result.ok_or_else(scoped_object_error_v29)?;
                if graph.object_location(base, budget)? != source
                    || graph.object_location(result, budget)? != projected
                {
                    return Err(scoped_object_error_v29());
                }
                let node = graph.value(result, budget)?;
                budget.charge_work(call_splice_search_work_v1(graph.projections.len()))?;
                let index = graph
                    .projections
                    .binary_search_by_key(&node, |row| row.0)
                    .map_err(|_| scoped_object_error_v29())?;
                if std::mem::replace(&mut seen[index], true) {
                    return Err(scoped_object_error_v29());
                }
                original_keys.push((access_ordinal, component));
                projects.push(PendingSourceObjectProjectV29 {
                    instance,
                    anchor: ordinal,
                    source,
                    projected,
                });
            }
        }
        budget.charge_work(seen.len())?;
        if seen.iter().any(|seen| !seen) {
            return Err(scoped_object_error_v29());
        }
        call_splice_sort_work_v1(argument_product_v1(original_keys.len(), 2)?, budget)
            .map_err(source_address_call_error_v29)?;
        original_keys.sort_unstable();
        budget.charge_work(original_keys.len())?;
        if original_keys.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err(scoped_object_error_v29());
        }
        Ok(())
    })?;
    Ok(projects)
}
