// Constructor payload views retain the original enum type, but their schema is
// a variant row. Authenticate that generated view without making it a type row.
fn scoped_storage_constructor_headers_v44() -> Result<usize, ArgumentResourceV1> {
    source_reference_emission_headers_v29::<(
        [SemanticProjectionV1; 1],
        source_storage_v29::SourceSelectedProjectionV29,
        bool,
    )>()
}

fn scoped_storage_constructor_schema_v44(
    plan: &SourceReferencePlanV29<'_, '_>,
    anchors: &ScopedMemoryAnchorsV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    (|| {
    plan.check_owner(plan.instances, budget)?;
    budget.charge_work(3)?;
    if endpoint.root_type != endpoint.projected_type
        || endpoint.root_schema == endpoint.projected_schema
        || endpoint.path.count != 1
    {
        return Ok(false);
    }
    anchors.check_object_ledger(budget)?;
    budget.source_reference_reserve_v29(plan, scoped_storage_constructor_headers_v44()?)?;
    budget.charge_work(12)?;
    let ScopedObjectSourceV29::Place {
        site,
        role: ExecutionOperandV29::Destination,
        local,
        prefix: 0,
    } = endpoint.source else {
        return Err(scoped_object_error_v29());
    };
    let ScopedObjectIdentityV29::Local { instance, local: object_local, .. } = endpoint.object else {
        return Err(scoped_object_error_v29());
    };
    if instance != anchors.subject.instance || object_local != local || endpoint.source_path.count != 0 {
        return Err(scoped_object_error_v29());
    }
    let original = plan.instances.instance(instance).ok_or_else(scoped_object_error_v29)?;
    if anchors.subject.function != original.function()
        || anchors.subject.source != ExecutionCallSourceV29::from_instances(plan.instances, budget)?
    {
        return Err(scoped_object_error_v29());
    }
    let function = original.declaration();
    let Some(SemanticStatementKindV1::Assign(assignment)) = scoped_source_statement_v29(function, site) else {
        return Err(scoped_object_error_v29());
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return Err(scoped_object_error_v29());
    };
    let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() else {
        return Err(scoped_object_error_v29());
    };
    if assignment.destination().local() != local
        || !assignment.destination().projections().is_empty()
        || assignment.destination().ty() != endpoint.root_type
        || assignment.value().result_type() != endpoint.root_type
        || function.locals().get(local.index() as usize).map(|local| local.ty()) != Some(endpoint.root_type)
        || !matches!(plan.instances.owner().source_semantic().types()
            .get(endpoint.root_type.index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Enum { variants, .. })
                if variants.get(*variant as usize).is_some_and(|row| !row.is_uninhabited()))
        || !matches!(anchors.object_path(endpoint.path, budget)?,
            [ScopedObjectComponentV29::View { projection: ScopedObjectViewProjectionV29::Variant(actual), ty }]
            if actual == variant && *ty == endpoint.root_type)
    {
        return Err(scoped_object_error_v29());
    }
    let layouts = plan.storage_root.as_ref().ok_or_else(scoped_object_error_v29)?
        .source_layouts(plan.instances, budget)?;
    let projection = [SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(*variant), endpoint.root_type)
        .map_err(|_| ArgumentResourceV1::Accounting)?];
    let mut visited = false;
    let projected = layouts.visit_selected_components(
        plan.instances.owner(), endpoint.root_type, endpoint.root_schema, &projection, budget,
        |component, budget| {
            budget.charge_work(6)?;
            if visited || component.source_type != endpoint.root_type
                || component.source_schema != endpoint.root_schema
                || component.result_type != endpoint.projected_type
                || component.result_schema != Some(endpoint.projected_schema)
                || component.kind != (source_storage_v29::SourceSelectedComponentKindV29::Variant {
                    original: *variant, physical: true,
                })
            {
                return Err(scoped_object_error_v29());
            }
            visited = true;
            Ok(())
        },
    )?;
    if !visited || projected != (source_storage_v29::SourceSelectedProjectionV29::Payload {
        ty: endpoint.projected_type, schema: endpoint.projected_schema, variant: *variant,
    }) {
        return Err(scoped_object_error_v29());
    }
    Ok(true)
    })().inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
