use super::*;

pub(super) fn retained_copy_read(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    source: &ProjectedAccessSourceV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    // Fixed site4 + assignment4 + destination12 + array12 + projection8 +
    // kind/source/ordinal8 (including the caller's duplicate-site lookup).
    // No value/CFG walk: initialization remains the independent final check's job.
    facts.charge_private_array_work(48)?;
    let Some(site) = source.semantic_site else {
        return Ok(false);
    };
    let Some(statement) = site.statement.and_then(|statement| {
        function
            .blocks()
            .get(site.block)?
            .statements()
            .get(statement)
    }) else {
        return Ok(false);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Ok(false);
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
    else {
        return Ok(false);
    };
    let destination = assignment.destination();
    let Some(target) = function.locals().get(destination.local().index() as usize) else {
        return Ok(false);
    };
    let Some(local) = function.locals().get(place.local().index() as usize) else {
        return Ok(false);
    };
    let Some(SemanticTypeShapeV1::Array { element, .. }) =
        types.get(local.ty().index() as usize).map(|ty| ty.shape())
    else {
        return Ok(false);
    };
    let [projection] = place.projections() else {
        return Ok(false);
    };
    if source.memory_space != MemorySpaceAttr::Private
        || source.access != AccessKindAttr::Read
        || target.role() != SemanticLocalRoleV1::Temporary
        || !destination.projections().is_empty()
        || target.ty() != *element
        || destination.ty() != *element
        || assignment.value().result_type() != *element
        || place.ty() != *element
        || projection.result_type() != *element
        || local.role().is_entry_argument()
        || !matches!(
            types.get(element.index() as usize).map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Scalar(_))
        )
        || !matches!(
            projection.kind(),
            SemanticProjectionKindV1::Index(_) | SemanticProjectionKindV1::ConstantIndex { .. }
        )
    {
        return Ok(false);
    }
    // This syntax has one memory operand and no memory destination. The query
    // binds its actual source role; its index is not a replacement ranked index.
    Ok(facts
        .private_array_access_index_v1(
            site,
            fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(0),
        )?
        .is_some())
}
