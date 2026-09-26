// Borrowed call/source correspondence only. This recipe has no pointer-origin,
// initializedness, lifetime, or memory-admission methods and never escapes the
// scoped phase checker. Final typed source/currentness replay remains mandatory.
#[derive(Clone, Copy)]
struct ScopedTypedCallResultRecipeV29<'a> {
    call: &'a SemanticDirectCallV1,
    operation: &'a Operation,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    access: MemoryAccess,
}

#[allow(clippy::too_many_arguments)]
fn scoped_typed_call_result_recipe_v29<'a>(
    instances: &ExecutionInstancesV29<'_>,
    original: &'a production_call_instances_v1::ProductionInstanceCallV1<'_>,
    anchors: &ScopedMemoryAnchorsV29,
    ordinal: usize,
    block: &'a BasicBlock,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedTypedCallResultRecipeV29<'a>, ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    anchors.check_object_ledger(budget)?;
    budget.charge_work(12)?;
    source_reference_emission_prepay_v29::<ScopedTypedCallResultRecipeV29<'_>>(budget)?;
    let occurrence = original.occurrence();
    let source = instances.instance(occurrence.caller).ok_or_else(mismatch)?;
    let call = original.source();
    let place = call.destination().ok_or_else(mismatch)?.place();
    let site = execution_site_v29(occurrence.block, None);
    let row = anchors.rows.get(ordinal).ok_or_else(mismatch)?;
    let operation = block.operations.get(row.position).ok_or_else(mismatch)?;
    if anchors.subject.instance != occurrence.caller
        || anchors.subject.function != source.function()
        || row.block != block.id
        || row.source != Some(ScopedMemoryFrameV29 {
            site, role: Some(ScopedMemoryRoleV29::CallResult),
        })
        || !matches!(source.declaration().blocks().get(occurrence.block.index() as usize)
            .map(|block| block.terminator().kind()),
            Some(SemanticTerminatorKindV1::Call(actual)) if std::ptr::eq(actual, call))
    { return Err(mismatch()); }
    let payload = anchors.object_payload(row, budget)?;
    payload.check_operation(operation, budget)?;
    let ScopedObjectRoleV29::WriteValue {
        destination,
        value: ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::CallResult {
            site: actual_site, ty,
        }),
    } = payload.role else { return Err(mismatch()); };
    if actual_site != site || ty != place.ty()
        || destination.projected_type != ty
        || destination.source != (ScopedObjectSourceV29::Place {
            site, role: ExecutionOperandV29::CallDestinationAddress, local: place.local(),
            prefix: u32::try_from(place.projections().len()).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        })
    { return Err(mismatch()); }
    let occurrences = instances.occurrences(occurrence.caller).ok_or_else(mismatch)?;
    anchors.check_object_source(source.declaration(), &occurrences, ordinal, row, payload, budget)?;
    if let Some(plan) = references {
        plan.check_owner(instances, budget)?;
        let layouts = plan.storage_root.as_ref().ok_or_else(mismatch)?
            .source_layouts(instances, budget)?;
        layouts.check_selected_schema(instances.owner(), destination.root_type,
            destination.root_schema, budget)?;
        layouts.check_selected_schema(instances.owner(), destination.projected_type,
            destination.projected_schema, budget)?;
    }
    let ScopedObjectOperationV29::WriteValue { access, .. } = payload.operation else {
        return Err(mismatch());
    };
    // Original reference addresses and physical projections retain the emitter's
    // conservative alignment. An unprojected local uses its natural alignment.
    let expected = match destination.object {
        ScopedObjectIdentityV29::Reference { .. } => MemoryAccess::new(access.address_space, 1),
        ScopedObjectIdentityV29::Local { .. } if destination.path.count != 0 =>
            MemoryAccess::new(AddressSpace::Private, 1),
        ScopedObjectIdentityV29::Local { .. } => memory_access_for_type(
            instances.owner().source_semantic().types(), destination.root_type, AddressSpace::Private)?,
        _ => return Err(mismatch()),
    };
    if access != expected { return Err(mismatch()); }
    Ok(ScopedTypedCallResultRecipeV29 { call, operation, schema: destination.projected_schema, access: expected })
}
