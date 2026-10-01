// This query rejoins an existing original raw-access row. It does not create a
// pointer origin, extend a lifetime, or substitute a physical slot for a cell.
type SourceStaticRawHolderFrameV42<'a> = (
    &'a SemanticFunctionDeclV1,
    &'a [SemanticTypeDeclV1],
    &'a [SemanticProjectionV1],
    &'a [SemanticProjectionV1],
    std::slice::Iter<'a, SemanticProjectionV1>,
    &'a SemanticProjectionV1,
    &'a SemanticTypeDeclV1,
    &'a fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
    SemanticTypeIdV1,
    SourceReferenceRawAccessKeyV29,
    SourceReferenceRawAccessV29,
    SourceReferenceRawHolderV29,
    Option<SourceReferenceRawAccessV29>,
    std::ops::Range<usize>,
);

fn source_static_raw_holder_headers_v42() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_emission_headers_v29::<SourceStaticRawHolderFrameV42<'_>>()?,
        source_reference_emission_headers_v29::<SourceReferenceRawAccessV29>()?,
    ])
}

fn source_static_raw_holder_v42(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    crossing: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceReferenceRawAccessV29, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let mut result = None;
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(source_static_raw_holder_headers_v42()?)?;
        budget.charge_work(8)?;
        let original = plan
            .instances
            .instance(site.instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let types = plan.instances.owner().source_semantic().types();
        let path = place
            .projections()
            .get(..crossing)
            .ok_or_else(scoped_object_error_v29)?;
        if crossing.checked_add(1) != Some(place.projections().len())
            || place.projections()[crossing].kind() != SemanticProjectionKindV1::Dereference
        {
            return Err(scoped_object_pending_v29());
        }
        let mut ty = original
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(scoped_object_error_v29)?
            .ty();
        for projection in path {
            budget.charge_work(6)?;
            let declaration = types
                .get(ty.index() as usize)
                .ok_or_else(scoped_object_error_v29)?;
            ty = match (declaration.shape(), projection.kind()) {
                (
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    SemanticProjectionKindV1::Field(field),
                ) => *fields
                    .fields()
                    .get(field as usize)
                    .ok_or_else(scoped_object_error_v29)?,
                (
                    SemanticTypeShapeV1::Array { element, length },
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                ) => {
                    source_static_constant_index_v29(*length, offset, minimum_length, from_end)?;
                    *element
                }
                _ => return Err(scoped_object_pending_v29()),
            };
            if ty != projection.result_type() {
                return Err(scoped_object_error_v29());
            }
        }
        budget.charge_work(8)?;
        let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(scoped_object_error_v29());
        };
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointee() != place.ty()
            || place.projections()[crossing].result_type() != place.ty()
            || (access == SourceReferenceAccessV29::Write
                && pointer.mutability() != SemanticMutabilityV1::Mutable)
        {
            return Err(scoped_object_error_v29());
        }
        let key = (
            source_reference_access_key_v29(site, place, access),
            crossing,
        );
        charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
        let raw = **plan
            .raw_accesses
            .get(&key)
            .ok_or_else(scoped_object_error_v29)?;
        let holder = raw.holder;
        let retained = plan
            .projections
            .get(holder.first..argument_sum_v1(&[holder.first, holder.count])?)
            .ok_or_else(scoped_object_error_v29)?;
        budget.charge_work(argument_sum_v1(&[16, path.len()])?)?;
        if raw.site != site
            || raw.source != place as *const SemanticPlaceV1 as usize
            || raw.access != access
            || raw.crossing != access
            || raw.projection != crossing
            || raw.pointee != place.ty()
            || raw.ty != place.ty()
            || holder.instance != site.instance
            || holder.local != place.local()
            || holder.parent.is_some()
            || holder.shared_path
            || holder.selector_source.is_some()
            || retained != path || !plan.nodes.get(holder.node).is_some_and(|node| {
            node.ty == ty
                && matches!(node.kind, SourceReferenceNodeKindV29::Address(set) if set == raw.set)
        }) {
            return Err(scoped_object_error_v29());
        }
        result = Some(raw);
        Ok(())
    })
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    result.ok_or(ArgumentResourceV1::Accounting.into())
}
