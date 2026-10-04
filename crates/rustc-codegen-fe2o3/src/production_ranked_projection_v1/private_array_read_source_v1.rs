use super::*;

pub(super) fn retained_read(
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
    let destination = assignment.destination();
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;
    let (place, role) = match assignment.value().kind() {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
            let Some(target) = function.locals().get(destination.local().index() as usize) else {
                return Ok(false);
            };
            if target.role() != SemanticLocalRoleV1::Temporary
                || !destination.projections().is_empty()
                || target.ty() != place.ty()
                || destination.ty() != place.ty()
            {
                return Ok(false);
            }
            (place, Role::RvalueOperand(0))
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            // Additional fixed destination/operand/type checks, at most two
            // candidates. No destination write or scalar value is authorized.
            facts.charge_private_array_work(48)?;
            let Some(element) = indexed_scalar_element(types, function, destination) else {
                return Ok(false);
            };
            let candidate = [(left, right, 0), (right, left, 1)].into_iter().find_map(
                |(indexed, scalar, ordinal)| {
                    let SemanticOperandV1::Copy(place) = indexed else {
                        return None;
                    };
                    (place.projections().len() == 1
                        && place.ty() == element
                        && whole_scalar_operand(function, scalar, element))
                    .then_some((place, Role::RvalueOperand(ordinal)))
                },
            );
            let Some(candidate) = candidate else {
                return Ok(false);
            };
            candidate
        }
        _ => return Ok(false),
    };
    let Some(element) = indexed_scalar_element(types, function, place) else {
        return Ok(false);
    };
    if source.memory_space != MemorySpaceAttr::Private
        || source.access != AccessKindAttr::Read
        || assignment.value().result_type() != element
    {
        return Ok(false);
    }
    // Both supported forms have exactly one source memory read. Its dense
    // access ordinal is zero even when it is Binary's second operand.
    // The query index is not a replacement ranked index or a value proof.
    Ok(facts.private_array_access_index_v1(site, role)?.is_some())
}

fn whole_scalar_operand(
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    element: SemanticTypeIdV1,
) -> bool {
    match operand {
        SemanticOperandV1::Constant(constant) => {
            constant.ty() == element
                && matches!(constant.value(), SemanticConstantValueV1::Scalar(_))
        }
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            place.projections().is_empty()
                && place.ty() == element
                && function
                    .locals()
                    .get(place.local().index() as usize)
                    .is_some_and(|local| local.ty() == element)
        }
    }
}

fn indexed_scalar_element(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
) -> Option<SemanticTypeIdV1> {
    let local = function.locals().get(place.local().index() as usize)?;
    let Some(SemanticTypeShapeV1::Array { element, .. }) =
        types.get(local.ty().index() as usize).map(|ty| ty.shape())
    else {
        return None;
    };
    let [projection] = place.projections() else {
        return None;
    };
    if place.ty() != *element
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
        return None;
    }
    Some(*element)
}
