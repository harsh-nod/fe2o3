//! Retained Issue -> RustCall tuple -> closure local -> shared Bind reference.
//! Source correspondence only; the ordered loan and lifecycle owners remain
//! mandatory. No capability is reconstructed from its type or a ZST constant.
use super::*;
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedRootV1,
    SemanticExpandedTerminatorOriginV1 as TerminatorOrigin,
};

pub(super) struct Request<'a> {
    pub closure_source: &'a SemanticFunctionDeclV1,
    pub wrapper: SemanticCallInstanceIdV1,
    pub closure: SemanticCallInstanceIdV1,
    pub invoke_block: SemanticBlockIdV1,
    pub tuple_type: SemanticTypeIdV1,
    pub phase_type: SemanticTypeIdV1,
    pub issued_local: SemanticLocalIdV1,
    pub issued_value: SsaValueV1,
}

pub(super) struct Argument {
    pub local: SemanticLocalIdV1,
    pub value: SsaValueV1,
}

pub(super) fn closure_argument(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    request: Request<'_>,
    work: &mut usize,
) -> PhaseResult<Argument> {
    let frame = view
        .instances()
        .get(request.closure.index() as usize)
        .ok_or_else(|| rejected("phase closure argument has no exact expansion frame"))?;
    spend(work, 8)?;
    if !std::ptr::eq(query.function(), view.body())
        || frame.function_identity() != request.closure_source.identity()
        || frame.parent() != Some(request.wrapper)
        || frame.call_block() != Some(request.invoke_block)
        || request.closure_source.abi().extern_abi() != SemanticExternAbiV1::RustCall
        || request.closure_source.abi().source_input_types().len() != 2
        || request.closure_source.abi().source_input_types()[1] != request.tuple_type
        || request
            .closure_source
            .abi()
            .source_argument_ownership()
            .get(1)
            != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
    {
        return Err(rejected(
            "phase closure argument substituted its source ABI or caller",
        ));
    }
    let call_block = mapped_block(view, request.wrapper, request.invoke_block, work)?;
    let origin = &view.block_origins()[call_block.index() as usize];
    if origin.terminator()
        != (TerminatorOrigin::CallEntry {
            callee: request.closure,
        })
    {
        return Err(rejected("phase tuple has no exact closure call entry"));
    }
    let pack_site = source_site(view, call_block, 0, work)?;
    let pack = assignment(query, pack_site)?;
    let SemanticRvalueKindV1::Aggregate(aggregate) = pack.value().kind() else {
        return Err(rejected("phase tuple is not the original aggregate"));
    };
    let [operand @ SemanticOperandV1::Move(issued)] = aggregate.operands() else {
        return Err(rejected("phase tuple must move its one issued Workgroup"));
    };
    if aggregate.kind() != &SemanticAggregateKindV1::Tuple
        || pack.destination().ty() != request.tuple_type
        || !pack.destination().projections().is_empty()
        || issued.local() != request.issued_local
        || issued.ty() != request.phase_type
        || !issued.projections().is_empty()
    {
        return Err(rejected(
            "phase tuple substituted its issued Workgroup or layout",
        ));
    }
    moved_value(query, pack_site, operand, request.issued_value, work)?;
    let tuple = value_at(
        query,
        pack_site,
        variable(pack.destination().local()),
        Event::Define,
        work,
    )?;

    let mut transfer = None;
    for (statement, marker) in origin.statements().iter().enumerate() {
        spend(work, 1)?;
        if *marker
            != (Origin::ParameterTransfer {
                callee: request.closure,
                argument: 1,
            })
        {
            continue;
        }
        let site = Site::new(call_block, Some(statement as u32));
        let a = assignment(query, site)?;
        let SemanticRvalueKindV1::Use(operand @ SemanticOperandV1::Move(place)) = a.value().kind()
        else {
            return Err(rejected(
                "phase closure tuple parameter lost its original move",
            ));
        };
        let local = view
            .local_origins()
            .get(a.destination().local().index() as usize)
            .ok_or_else(|| rejected("phase closure tuple parameter left its original frame"))?;
        let original = request
            .closure_source
            .locals()
            .get(local.local().index() as usize)
            .ok_or_else(|| rejected("phase closure tuple parameter has no original local"))?;
        if place != pack.destination()
            || a.destination().ty() != request.tuple_type
            || !a.destination().projections().is_empty()
            || local.instance() != request.closure
            || local.function() != frame.function()
            || original.role() != SemanticLocalRoleV1::Argument(1)
            || original.ty() != request.tuple_type
        {
            return Err(rejected(
                "phase closure tuple parameter changed its exact source operand",
            ));
        }
        moved_value(query, site, operand, tuple, work)?;
        let value = value_at(
            query,
            site,
            variable(a.destination().local()),
            Event::Define,
            work,
        )?;
        if transfer.replace((a.destination().local(), value)).is_some() {
            return Err(rejected(
                "phase closure tuple parameter transfer was duplicated",
            ));
        }
    }
    let (parameter, tuple_value) = transfer
        .ok_or_else(|| rejected("phase closure lost its exact tuple parameter transfer"))?;
    let entry = mapped_block(view, request.closure, request.closure_source.entry(), work)?;
    let site = source_site(view, entry, 0, work)?;
    let projection = assignment(query, site)?;
    if !field_zero_move(projection, parameter, request.phase_type) {
        return Err(rejected(
            "phase closure lost its exact owned field-zero projection",
        ));
    }
    // A partial field move has the existing aggregate Use, not a whole-local
    // Kill. Requiring an invented Kill here would change retained MIR semantics.
    if value_at(query, site, variable(parameter), Event::Use, work)? != tuple_value {
        return Err(rejected(
            "phase closure projected a substituted tuple SSA value",
        ));
    }
    let local = projection.destination().local();
    Ok(Argument {
        local,
        value: value_at(query, site, variable(local), Event::Define, work)?,
    })
}

pub(super) fn shared_bind_reference(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
    phase: &Argument,
    phase_type: SemanticTypeIdV1,
    reference_type: SemanticTypeIdV1,
    work: &mut usize,
) -> PhaseResult<SsaValueV1> {
    let Some(SemanticOperandV1::Copy(expected) | SemanticOperandV1::Move(expected)) =
        binding.arguments().first()
    else {
        return Err(rejected(
            "phase Bind needs its actual shared reference operand",
        ));
    };
    if expected.ty() != reference_type || !expected.projections().is_empty() {
        return Err(rejected(
            "phase Bind shared reference type or place changed",
        ));
    }
    let transfer = boundaries::parameter_transfer(query, view, binding, 0, work)?;
    let block = binding.expanded_call_block();
    let origin = &view.block_origins()[block.index() as usize];
    let mut found = false;
    for (statement, item) in query.function().blocks()[block.index() as usize]
        .statements()
        .iter()
        .enumerate()
    {
        spend(work, 1)?;
        if !matches!(
            origin.statements().get(statement),
            Some(Origin::Source { .. })
        ) {
            continue;
        }
        let SemanticStatementKindV1::Assign(a) = item.kind() else {
            continue;
        };
        if a.destination() != expected {
            continue;
        }
        let SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place,
        } = a.value().kind()
        else {
            return Err(rejected(
                "phase Bind reference was replaced by a non-shared borrow",
            ));
        };
        if found
            || place.local() != phase.local
            || place.ty() != phase_type
            || !place.projections().is_empty()
            || origin.instance() != binding.caller_instance()
        {
            return Err(rejected("phase Bind borrowed a different closure phase"));
        }
        found = true;
        let site = Site::new(block, Some(statement as u32));
        if query
            .borrow_place_use(site, place, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase Bind source borrow has no exact promoted phase"))?
            != phase.value
            || value_at(query, site, variable(expected.local()), Event::Define, work)?
                != transfer.input
        {
            return Err(rejected(
                "phase Bind reference changed its exact phase SSA origin",
            ));
        }
    }
    if !found {
        return Err(rejected("phase Bind lost its original shared phase borrow"));
    }
    Ok(transfer.output)
}

fn field_zero_move(
    a: &SemanticAssignmentV1,
    parameter: SemanticLocalIdV1,
    phase: SemanticTypeIdV1,
) -> bool {
    a.destination().projections().is_empty()
        && a.destination().ty() == phase
        && a.value().result_type() == phase
        && matches!(a.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
            if p.local() == parameter && p.ty() == phase
                && matches!(p.projections(), [field] if field.kind() == SemanticProjectionKindV1::Field(0)
                    && field.result_type() == phase))
}

fn moved_value(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    site: Site,
    operand: &SemanticOperandV1,
    expected: SsaValueV1,
    work: &mut usize,
) -> PhaseResult<()> {
    let SemanticOperandV1::Move(place) = operand else {
        return Err(rejected("phase source edge is not its retained move"));
    };
    let used = query
        .operand_use(site, operand, &mut || spend(work, 1).is_ok())
        .map_err(|_| rejected("phase source move has no pointer-identical promoted use"))?;
    if !used.belongs_to(query)
        || used.value() != expected
        || value_at(query, site, variable(place.local()), Event::Kill, work)? != expected
    {
        return Err(rejected(
            "phase source move changed or reused its exact SSA value",
        ));
    }
    Ok(())
}

pub(super) fn mapped_block(
    view: &SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    block: SemanticBlockIdV1,
    work: &mut usize,
) -> PhaseResult<SemanticBlockIdV1> {
    let mut result = None;
    for (index, origin) in view.block_origins().iter().enumerate() {
        spend(work, 1)?;
        if origin.instance() == instance
            && origin.block() == block
            && result
                .replace(SemanticBlockIdV1::from_index(index as u32))
                .is_some()
        {
            return Err(rejected(
                "phase source block mapped more than once in one occurrence",
            ));
        }
    }
    result.ok_or_else(|| rejected("phase source block missing from the exact occurrence"))
}

fn source_site(
    view: &SemanticExpandedRootV1,
    block: SemanticBlockIdV1,
    statement: u32,
    work: &mut usize,
) -> PhaseResult<Site> {
    let mut result = None;
    for (index, marker) in view.block_origins()[block.index() as usize]
        .statements()
        .iter()
        .enumerate()
    {
        spend(work, 1)?;
        if *marker == (Origin::Source { statement })
            && result
                .replace(Site::new(block, Some(index as u32)))
                .is_some()
        {
            return Err(rejected("phase source statement mapped more than once"));
        }
    }
    result.ok_or_else(|| rejected("phase source statement was erased or substituted"))
}

pub(super) fn assignment<'a>(
    query: &ProductionSemanticSsaSourceQueryV1<'a>,
    site: Site,
) -> PhaseResult<&'a SemanticAssignmentV1> {
    let Some(SemanticStatementKindV1::Assign(a)) = site
        .statement()
        .and_then(|statement| {
            query
                .function()
                .blocks()
                .get(site.block().index() as usize)
                .and_then(|block| block.statements().get(statement as usize))
        })
        .map(|s| s.kind())
    else {
        return Err(rejected(
            "phase source edge is not its exact retained assignment",
        ));
    };
    Ok(a)
}

fn variable(local: SemanticLocalIdV1) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(local.index())
}

#[cfg(test)]
#[path = "ssa_phase_argument_tests.rs"]
mod tests;
