//! Select End-restored values only at an original, checked mutable borrow.
//! The source owner's SSA value itself is never rebound to a later KIR value.
use super::*;
use fe2o3_mir_model::SemanticExpandedStatementOriginV1;
use fe2o3_pliron::ProductionSemanticSsaSourceSiteV1 as Site;

#[derive(Clone, Debug)]
pub(super) struct OriginalBorrow {
    pub(super) phase: usize,
    pub(super) lease: Option<usize>,
    pub(super) site: Site,
    pub(super) assignment: fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    pub(super) source: SsaValueV1,
    pub(super) reference: SsaValueV1,
}

fn original_borrow_count(
    phases: usize,
    work: &mut usize,
    mut leases_at: impl FnMut(usize) -> usize,
) -> PhaseResult<usize> {
    let mut count = 0usize;
    for index in 0..phases {
        spend(work, 1)?;
        count = count
            .checked_add(1)
            .and_then(|count| count.checked_add(leases_at(index)))
            .ok_or_else(|| rejected("phase original borrow count overflow"))?;
    }
    Ok(count)
}

pub(super) fn collect(
    checked: &CheckedRows<'_>,
    work: &mut usize,
) -> PhaseResult<Vec<OriginalBorrow>> {
    let count = original_borrow_count(checked.input.phases.len(), work, |index| {
        checked.input.phases[index].leases.len()
    })?;
    let mut output = reserve(count, work)?;
    for (phase_index, phase) in checked.input.phases.iter().enumerate() {
        spend(work, 1)?;
        let owner = checked.binding(phase.owner, work)?;
        let wrapper = checked.binding(phase.wrapper, work)?;
        let borrow = original(
            checked,
            phase_index,
            None,
            phase.begin,
            phase.converted_owner,
            phase.owner_reference,
            owner.destination(),
            owner.caller_instance(),
            work,
        )?;
        if borrow.site.block() != wrapper.expanded_call_block()
            || !matches!(wrapper.arguments().first(), Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place))
                if place == borrow.assignment.destination())
        {
            return Err(rejected(
                "phase owner borrow no longer feeds the exact wrapper",
            ));
        }
        output.push(borrow);
        for (lease_index, lease) in phase.leases.iter().enumerate() {
            let allocation = checked.binding(lease.storage_conversion, work)?;
            output.push(original(
                checked,
                phase_index,
                Some(lease_index),
                lease.borrowed_at,
                lease.allocation,
                lease.root_reference,
                allocation.destination(),
                allocation.caller_instance(),
                work,
            )?);
        }
    }
    for (index, selected) in output.iter().enumerate() {
        for prior in &output[..index] {
            spend(work, 1)?;
            if selected.site == prior.site || selected.reference == prior.reference {
                return Err(rejected("phase emission reused an original mutable borrow"));
            }
        }
    }
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
fn original(
    checked: &CheckedRows<'_>,
    phase: usize,
    lease: Option<usize>,
    point: (SsaBlockIdV1, u32),
    source: SsaValueV1,
    reference: SsaValueV1,
    expected: &SemanticPlaceV1,
    instance: SemanticCallInstanceIdV1,
    work: &mut usize,
) -> PhaseResult<OriginalBorrow> {
    let site = checked
        .query
        .event_site(point.0, point.1, &mut || spend(work, 1).is_ok())
        .map_err(|_| rejected("phase borrow lost its original source event"))?;
    let statement = site
        .statement()
        .ok_or_else(|| rejected("phase borrow is not an original assignment"))?;
    let view = checked
        .owner
        .execution_view_for_root(checked.input.root)
        .ok_or_else(|| rejected("phase borrow lost the checked root view"))?;
    let origin = view
        .block_origins()
        .get(site.block().index() as usize)
        .ok_or_else(|| rejected("phase borrow block is absent"))?;
    if origin.instance() != instance
        || !matches!(
            origin.statements().get(statement as usize),
            Some(SemanticExpandedStatementOriginV1::Source { .. })
        )
    {
        return Err(rejected(
            "phase mutable borrow changed its original owner instance",
        ));
    }
    let Some(SemanticStatementKindV1::Assign(assignment)) = view
        .body()
        .blocks()
        .get(site.block().index() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .map(|statement| statement.kind())
    else {
        return Err(rejected("phase mutable borrow assignment is absent"));
    };
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Mutable,
        place,
    } = assignment.value().kind()
    else {
        return Err(rejected(
            "phase restoration requires the original mutable borrow",
        ));
    };
    if place != expected
        || !place.projections().is_empty()
        || !assignment.destination().projections().is_empty()
        || assignment.value().result_type() != assignment.destination().ty()
    {
        return Err(rejected("phase restoration substituted its source place"));
    }
    let types = checked.owner.source_semantic().types();
    if !matches!(types.get(assignment.destination().ty().index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer))
            if pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Mutable
                && pointer.metadata() == SemanticPointerMetadataV1::None
                && pointer.pointee() == place.ty())
    {
        return Err(rejected(
            "phase restoration changed the exact reference type",
        ));
    }
    checked.check_event(
        PhaseEmissionBoundaryV1 {
            site,
            event: point.1,
            variable: fe2o3_mir_model::SsaVariableIdV1::new(
                assignment.destination().local().index(),
            ),
            value: reference,
            kind: PhaseBoundaryKindV1::Define,
        },
        work,
    )?;
    let actual = checked
        .query
        .borrow_place_use(site, place, &mut || spend(work, 1).is_ok())
        .map_err(|_| rejected("phase restoration has no exact promoted source use"))?;
    if actual != source || source == reference {
        return Err(rejected(
            "phase restoration changed its original owner SSA value",
        ));
    }
    // Both cloned projection vectors are empty; the descriptor's inline
    // storage was charged by the caller's exact-capacity reservation.
    spend(work, 1)?;
    Ok(OriginalBorrow {
        phase,
        lease,
        site,
        assignment: assignment.clone(),
        source,
        reference,
    })
}

#[cfg(test)]
#[path = "borrow_census_tests.rs"]
mod census_tests;
