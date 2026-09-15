//! Nonobserving lane-field transport through exact single-leaf carriers.
//! Unknown consumers, loads, stores and dereferences reject; this issues no lane.
use super::*;
use matrix_access_borrow::{MatrixAccessBorrow, shared_pointee};

#[derive(Clone, Copy)]
pub(super) struct ClosedProjection {
    pub(super) parent: u32,
    pub(super) reference: SemanticTypeIdV1,
    pub(super) owned: SemanticTypeIdV1,
}

pub(super) fn sites(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    facts: &BTreeMap<usize, MatrixAccessBorrow<'_>>,
    budget: &mut Budget,
) -> Result<BTreeMap<SemanticTransparentBorrowSiteV1, ClosedProjection>, ProductionSemanticSsaErrorV1> {
    let Some(types) = types else { return Ok(BTreeMap::new()) };
    let mut by_subgroup = BTreeMap::new();
    let mut lanes = BTreeSet::new();
    for fact in facts.values().copied() {
        budget.charge(8 + 2 * (usize::BITS - by_subgroup.len().leading_zeros()) as usize)?;
        let (reference, owned) = fact.pairs()[0];
        if by_subgroup.insert(reference, (owned, fact.lane())).is_some_and(|old| old != (owned, fact.lane())) {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        lanes.insert(fact.lane());
    }
    if lanes.is_empty() { return Ok(BTreeMap::new()) }
    let mut pairs = BTreeMap::new();
    for index in 0..types.len() {
        budget.charge(4 + (usize::BITS - lanes.len().leading_zeros()) as usize)?;
        let id = SemanticTypeIdV1::from_index(index as u32);
        if let Some(owned) = shared_pointee(types, id).filter(|owned| lanes.contains(owned)) {
            budget.charge(3)?;
            pairs.insert(id, owned);
        }
    }
    let routes = math_capture_flow_v1::Routes::from_pairs(function, types, &pairs, &BTreeSet::new(), budget)?;
    let mut candidates = Vec::new();
    let mut by_reference = BTreeMap::new();
    let mut duplicates = BTreeSet::new();
    let mut roots = BTreeMap::new();
    for (block, body) in function.blocks().iter().enumerate() {
        budget.charge(1)?;
        for (statement, source) in body.statements().iter().enumerate() {
            budget.charge(2)?;
            let SemanticStatementKindV1::Assign(a) = source.kind() else { continue };
            if !a.destination().projections().is_empty() || a.destination().ty() != a.value().result_type() { continue }
            let reference = a.destination().ty();
            let Some(&owned) = routes.owned(reference) else { continue };
            let site = SemanticTransparentBorrowSiteV1 { block: block as u32, statement: statement as u32 };
            let projection = match a.value().kind() {
                SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place } => {
                    let parent = function.locals().get(place.local().index() as usize);
                    let expected = parent.and_then(|local| by_subgroup.get(&local.ty()));
                    match (place.projections(), parent, expected) {
                        ([deref, field], Some(parent), Some(&(subgroup, lane)))
                            if deref.kind() == SemanticProjectionKindV1::Dereference
                                && deref.result_type() == subgroup
                                && field.kind() == SemanticProjectionKindV1::Field(0)
                                && field.result_type() == lane && place.ty() == lane && owned == lane =>
                            Some((place, ClosedProjection { parent: place.local().index(), reference: parent.ty(), owned: subgroup })),
                        _ => None,
                    }
                }
                _ => None,
            };
            let (place, source_reference) = if let Some((place, relation)) = projection {
                budget.charge(4)?;
                roots.insert(candidates.len(), relation);
                (place, None)
            } else if let Some(place) = routes.source(a, budget)? {
                (place, Some(place.local().index()))
            } else if let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = a.value().kind() {
                if !place.projections().is_empty() || place.ty() != reference || !pairs.contains_key(&reference) { continue }
                (place, Some(place.local().index()))
            } else { continue };
            budget.charge(12 + (usize::BITS - by_reference.len().leading_zeros()) as usize)?;
            let local = a.destination().local().index();
            let index = candidates.len();
            candidates.push(SemanticBorrowCandidateV1 {
                site, source_local: place.local().index(), source_type: owned,
                source_reference, value_alias: source_reference.is_some(), source_kind: SemanticBorrowCandidateSourceV1::Direct, consumers: 0, intrinsic_consumer: false,
                valid: function.locals().get(local as usize).is_some_and(|decl| decl.ty() == reference
                    && decl.role() != SemanticLocalRoleV1::Return),
            });
            if duplicates.contains(&local) { candidates[index].valid = false }
            else if let Some(previous) = by_reference.insert(local, index) {
                candidates[previous].valid = false;
                candidates[index].valid = false;
                by_reference.remove(&local);
                duplicates.insert(local);
            }
        }
    }
    if roots.is_empty() { return Ok(BTreeMap::new()) }
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, source) in body.statements().iter().enumerate() {
            budget.charge(2)?;
            lane_work_v1::statement(source.kind(), budget)?;
            let site = SemanticTransparentBorrowSiteV1 { block: block as u32, statement: statement as u32 };
            if let SemanticStatementKindV1::Assign(a) = source.kind() {
                let candidate = by_reference.get(&a.destination().local().index())
                    .is_some_and(|index| candidates[*index].site == site);
                if candidate && routes.source(a, budget)?.is_some() {
                    invalidate_reference_uses_in_statement_v1(source.kind(), site, &by_reference, &mut candidates);
                    routes.invalidate_siblings(a, &by_reference, &mut candidates, budget)?;
                    continue;
                }
                if routes.disjoint_field_use(a, budget)? {
                    invalidate_reference_place_v1(a.destination(), &by_reference, &mut candidates);
                    continue;
                }
            }
            invalidate_reference_uses_in_statement_v1(source.kind(), site, &by_reference, &mut candidates);
        }
        // Empty callable roster deliberately permits no terminal use of this
        // passive reference, including a recognized lane terminal.
        if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() {
            budget.charge(call.arguments().len().saturating_mul(2).saturating_add(2))?;
        } else { budget.charge(2)? }
        lane_work_v1::terminator(body.terminator().kind(), budget)?;
        validate_reference_uses_in_terminator_v1(body.terminator().kind(), &[], &by_reference, &mut candidates);
    }
    budget.charge(candidates.len().saturating_mul(2))?;
    let mut children = vec![Vec::new(); candidates.len()];
    for (index, candidate) in candidates.iter().enumerate() {
        if let Some(parent) = candidate.source_reference.and_then(|local| by_reference.get(&local)) {
            children[*parent].push(index);
        }
    }
    let mut result = BTreeMap::new();
    for (root, relation) in roots {
        if borrow_components_v1::members(&candidates, &children, root, budget, |_| true)?.is_ok() {
            budget.charge(4)?;
            result.insert(candidates[root].site, relation);
        }
    }
    Ok(result)
}
