// Revisited locators retain may-activation metadata, not a pointer birth or
// currentness certificate. Raw origin alternatives and expiry stay separate.
fn source_reference_raw_scalar_epoch_access_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    source: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    resolved: &SourceReferencePlaceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(18)?;
    if !matches!(
        access,
        SourceReferenceAccessV29::Address
            | SourceReferenceAccessV29::Read
            | SourceReferenceAccessV29::Write
    ) || resolved.instance != site.instance
        || !resolved.projections.is_empty()
        || resolved.loan.is_some()
        || !resolved.traversed.is_empty()
        || resolved.selector_source.is_some()
    {
        return Ok(false);
    }
    let declaration = plan
        .instances
        .instance(site.instance)
        .ok_or_else(source_reference_cfg_obligation_v29)?
        .declaration();
    let target = declaration
        .locals()
        .get(resolved.local.index() as usize)
        .ok_or_else(source_reference_cfg_obligation_v29)?;
    let types = plan.instances.owner().source_semantic().types();
    if target.ty() != source.ty()
        || !matches!(
            types
                .get(target.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        )
        || source_reference_raw_original_access_v29(declaration, site, source, budget)?
            != Some(access)
    {
        return Ok(false);
    }
    if access == SourceReferenceAccessV29::Address {
        return Ok(source.local() == resolved.local
            && source.projections().is_empty()
            && !resolved.shared_path);
    }
    let [projection] = source.projections() else {
        return Ok(false);
    };
    if projection.kind() != SemanticProjectionKindV1::Dereference
        || projection.result_type() != source.ty()
    {
        return Ok(false);
    }
    let pointer_type = declaration
        .locals()
        .get(source.local().index() as usize)
        .ok_or_else(source_reference_cfg_obligation_v29)?
        .ty();
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(pointer_type.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(false);
    };
    if pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || pointer.pointee() != source.ty()
    {
        return Ok(false);
    }
    let key = (source_reference_access_key_v29(site, source, access), 0);
    charge_execution_cfg_lookup_v29(plan.raw_accesses.len(), budget)?;
    let Some(row) = plan.raw_accesses.get(&key) else {
        return Ok(false);
    };
    budget.charge_work(12)?;
    let set = plan
        .raw_sets
        .get(row.set)
        .ok_or(ArgumentResourceV1::Accounting)?;
    Ok(row.site == site
        && row.source == source as *const SemanticPlaceV1 as usize
        && row.access == access
        && row.crossing == access
        && row.projection == 0
        && row.pointee == source.ty()
        && row.ty == source.ty()
        && set.instance == resolved.instance
        && set.local == resolved.local
        && set.ty == source.ty()
        && set.count != 0
        && set.parent.is_none()
        && set.projection_count == 0
        && resolved.shared_path == !set.mutable
        && (access != SourceReferenceAccessV29::Write || set.mutable))
}
