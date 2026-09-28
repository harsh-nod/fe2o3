// Boundary nodes choose representations only. Their original evaluation sites
// and later physical snapshot allocations remain separate identities.
fn source_reference_object_schema_candidate_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    expected: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.charge(3, budget)?;
    let row = plan.nodes.get(node).ok_or_else(source_backing_error_v29)?;
    let types = plan.instances.owner().source_semantic().types();
    let declaration = types
        .get(expected.index() as usize)
        .ok_or_else(source_backing_error_v29)?;
    if row.ty != expected {
        return Err(source_backing_error_v29());
    }
    if execution_cfg_nominal_kind_v29(types, expected)?.is_some() {
        return Ok(false);
    }
    if (row.kind == SourceReferenceNodeKindV29::Absent || row.inactive.is_some())
        && !matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Never
        )
    {
        return Err(source_reference_error_v29(
            "boundary lacks a complete active source value",
        ));
    }
    match declaration.shape() {
        SemanticTypeShapeV1::Tuple(_)
        | SemanticTypeShapeV1::Aggregate(_)
        | SemanticTypeShapeV1::Union(_)
        | SemanticTypeShapeV1::Array { .. }
        | SemanticTypeShapeV1::Enum { .. } => Ok(true),
        SemanticTypeShapeV1::Unit
        | SemanticTypeShapeV1::Never
        | SemanticTypeShapeV1::Scalar(_)
        | SemanticTypeShapeV1::ValidityScalar(_) => Ok(false),
        SemanticTypeShapeV1::Pointer(pointer)
            if matches!(
                pointer.metadata(),
                SemanticPointerMetadataV1::None | SemanticPointerMetadataV1::SliceLength
            ) =>
        {
            Ok(false)
        }
        SemanticTypeShapeV1::Opaque
            if matches!(
                declaration.layout().backend_repr(),
                SemanticBackendReprV1::SimdVector { .. }
            ) =>
        {
            Ok(false)
        }
        SemanticTypeShapeV1::Pointer(_)
        | SemanticTypeShapeV1::Opaque
        | SemanticTypeShapeV1::Slice { .. }
        | SemanticTypeShapeV1::FunctionPointer { .. } => Err(source_reference_error_v29(
            "boundary source has no admitted value transport",
        )),
    }
}

fn source_reference_visit_schema_inputs_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    mut consume: impl FnMut(
        usize,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    for ordinal in 0..plan.boundary_values.len() {
        let source = plan.boundary_value(ordinal, budget)?;
        let ty = plan
            .nodes
            .get(source.node)
            .ok_or_else(source_backing_error_v29)?
            .ty;
        if source_reference_object_schema_candidate_v29(plan, source.node, ty, budget)? {
            consume(source.node, budget)?;
        }
    }
    if plan.entries.len() != plan.instances.instances().len()
        || plan.returns.len() != plan.entries.len()
    {
        return Err(source_backing_error_v29());
    }
    for ordinal in 0..plan.entries.len() {
        plan.charge(4, budget)?;
        let instance = plan
            .instances
            .id_at(ordinal)
            .ok_or_else(source_backing_error_v29)?;
        let function = plan
            .instances
            .instance(instance)
            .ok_or_else(source_backing_error_v29)?
            .declaration();
        let reachable = plan
            .instances
            .instance_reachable(instance)
            .ok_or_else(source_backing_error_v29)?;
        let state = plan.entries[ordinal];
        if !reachable {
            if state.is_some() || plan.returns[ordinal].is_some() {
                return Err(source_backing_error_v29());
            }
            continue;
        }
        let state = plan
            .states
            .get(state.ok_or_else(source_backing_error_v29)?)
            .ok_or_else(source_backing_error_v29)?;
        if state.len() != function.locals().len() {
            return Err(source_backing_error_v29());
        }
        for (local, declaration) in function.locals().iter().enumerate() {
            plan.charge(2, budget)?;
            if declaration.role().is_entry_argument() {
                let node = state[local].node.ok_or_else(source_backing_error_v29)?;
                if source_reference_object_schema_candidate_v29(
                    plan,
                    node,
                    declaration.ty(),
                    budget,
                )? {
                    consume(node, budget)?;
                }
            }
            if declaration.role() == SemanticLocalRoleV1::Return {
                if let Some(node) = plan.returns[ordinal] {
                    if source_reference_object_schema_candidate_v29(
                        plan,
                        node,
                        declaration.ty(),
                        budget,
                    )? {
                        consume(node, budget)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn source_reference_has_schema_inputs_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let mut present = false;
    source_reference_visit_schema_inputs_v29(plan, budget, |_, _| {
        present = true;
        Ok(())
    })?;
    Ok(present)
}

fn source_reference_seed_schema_inputs_v29(
    plan: &mut SourceReferencePlanV29<'_, '_>,
    equations: &mut SourceBackingEquationsV29,
    cells: &mut BTreeMap<(usize, u32, u32), usize>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.reserve_storage(std::mem::size_of::<Vec<usize>>())?;
    let mut inputs = Vec::new();
    source_reference_visit_schema_inputs_v29(plan, budget, |node, budget| {
        emission_push_v1(&mut inputs, node, budget)
    })?;
    // Only original node coordinates are copied. The existing equation memo
    // deduplicates repeated entry/return/boundary uses and retains all facts.
    for node in inputs {
        budget.charge_work(1)?;
        if equations
            .ensure_node(plan, cells, node, 0, budget)?
            .is_none()
        {
            return Err(source_backing_error_v29());
        }
    }
    Ok(())
}

fn source_reference_check_boundary_write_census_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    requests: &[source_storage_demands_v29::DemandV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.charge(1, budget)?;
    if requests.is_empty() && !plan.cells.rows.is_empty() {
        return Err(source_reference_error_v29(
            "object boundary local pointee lacks the original representation-write census",
        ));
    }
    Ok(())
}

fn source_reference_selected_value_schema_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    expected: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<fe2o3_kernel_ir::StorageLayoutIdV1>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        let lens = plan.storage_demands.ok_or_else(source_backing_error_v29)?;
        let (requests, _) = lens.requests(plan.instances, budget)?;
        if plan.has_storage_demands != !requests.is_empty() {
            return Err(source_backing_error_v29());
        }
        let layouts = plan
            .storage_root
            .as_ref()
            .ok_or_else(source_backing_error_v29)?
            .source_layouts(plan.instances, budget)?;
        if !source_reference_object_schema_candidate_v29(plan, node, expected, budget)? {
            return Ok(None);
        }
        let schema = plan
            .selected_storage
            .get(node)
            .copied()
            .flatten()
            .ok_or_else(|| {
                source_reference_error_v29("original object boundary has no selected schema")
            })?;
        layouts.check_selected_schema(plan.instances.owner(), expected, schema, budget)?;
        Ok(Some(schema))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_reference_boundary_schema_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    role: SourceReferenceBoundaryRoleV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<fe2o3_kernel_ir::StorageLayoutIdV1>, ProductionSemanticKirErrorV1> {
    let result = (|| {
        let (_, row) = plan
            .boundary_at(site, role, budget)?
            .ok_or_else(|| source_reference_error_v29("original value boundary is missing"))?;
        let ty = plan
            .nodes
            .get(row.node)
            .ok_or_else(source_backing_error_v29)?
            .ty;
        source_reference_selected_value_schema_v29(plan, row.node, ty, budget)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
