//! Map retained lifecycle boundaries to real events in the one replayed plan.
use super::*;
use fe2o3_mir_model::{SemanticCallInstanceIdV1, SemanticExpandedRootV1};
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

pub(super) fn close_lease(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    closure: SemanticCallInstanceIdV1,
    local: SemanticLocalIdV1,
    result: SsaValueV1,
    defined_at: Site,
    work: &mut usize,
) -> PhaseResult<linear_events::Point> {
    let variable = SsaVariableIdV1::new(local.index());
    let definition = definition_point(query, defined_at, variable, result, work)?;
    let close = linear_events::dormant_lease(
        query.plan().plan(), variable, result, definition, &mut || spend(work, 1).is_ok(),
    ).map_err(|error| rejected(match error {
        linear_events::Error::Work => "phase dormant lease exhausted existing source work",
        linear_events::Error::Definition => "phase dormant lease definition was replaced",
        linear_events::Error::Use => "phase dormant lease escaped the complete source protocol",
        linear_events::Error::Kill => "phase dormant lease has no unique existing close",
    }))?;
    let site = query.event_site(close.block, close.event, &mut || spend(work, 1).is_ok())
        .map_err(|_| rejected("phase lease close lost its exact source event"))?;
    let statement = site.statement().ok_or_else(|| rejected("phase lease close is not an original frame end"))?;
    let origin = view.local_origins().get(local.index() as usize)
        .ok_or_else(|| rejected("phase lease close local left the original frame"))?;
    let block = view.block_origins().get(site.block().index() as usize)
        .ok_or_else(|| rejected("phase lease close block left the original frame"))?;
    spend(work, 5)?;
    if origin.instance() != closure || block.instance() != closure
        || block.function() != origin.function()
        || block.statements().get(statement as usize) != Some(&Origin::FrameStorageDead { callee: closure, local: origin.local() })
        || !matches!(query.function().blocks()[site.block().index() as usize].statements()[statement as usize].kind(),
            SemanticStatementKindV1::StorageDead(actual) if *actual == local)
    {
        return Err(rejected("phase lease close substituted the original caller frame end"));
    }
    Ok(close)
}

pub(super) fn return_transfer(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
    work: &mut usize,
) -> PhaseResult<Site> {
    let mut result = None;
    for (block, origin) in view.block_origins().iter().enumerate() {
        spend(work, 1)?;
        for (statement, marker) in origin.statements().iter().enumerate() {
            spend(work, 1)?;
            if *marker != (Origin::ReturnTransfer { callee: binding.callee_instance() }) { continue; }
            let item = &query.function().blocks()[block].statements()[statement];
            let SemanticStatementKindV1::Assign(a) = item.kind() else {
                return Err(rejected("phase exact return transfer changed statement kind"));
            };
            if origin.instance() != binding.callee_instance()
                || origin.function() != binding.contract().function()
                || a.destination() != binding.destination()
                || !matches!(a.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if p.local() == binding.callee_return() && p.ty() == binding.destination().ty() && p.projections().is_empty())
                || result.replace(Site::new(SemanticBlockIdV1::from_index(block as u32), Some(statement as u32))).is_some()
            {
                return Err(rejected("phase exact return transfer changed its source owner or value"));
            }
        }
    }
    result.ok_or_else(|| rejected("phase definition has no exact normal return transfer"))
}

pub(super) struct Transfer {
    pub input: SsaValueV1,
    pub output: SsaValueV1,
}

pub(super) fn parameter_input(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
    argument: usize,
    work: &mut usize,
) -> PhaseResult<SsaValueV1> {
    Ok(parameter_transfer(query, view, binding, argument, work)?.input)
}

pub(super) fn parameter_transfer(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
    argument: usize,
    work: &mut usize,
) -> PhaseResult<Transfer> {
    let expected = binding.arguments().get(argument).ok_or_else(|| rejected("phase argument outside exact ABI"))?;
    let parameter = binding.callee_arguments().get(argument).ok_or_else(|| rejected("phase parameter outside exact source frame"))?;
    let block = binding.expanded_call_block();
    let origin = &view.block_origins()[block.index() as usize];
    let mut result = None;
    for (statement, marker) in origin.statements().iter().enumerate() {
        spend(work, 1)?;
        if *marker != (Origin::ParameterTransfer { callee: binding.callee_instance(), argument: argument as u32 }) { continue; }
        let SemanticStatementKindV1::Assign(a) = query.function().blocks()[block.index() as usize].statements()[statement].kind() else {
            return Err(rejected("phase argument transfer changed statement kind"));
        };
        let SemanticRvalueKindV1::Use(operand) = a.value().kind() else {
            return Err(rejected("phase argument transfer lost its source operand"));
        };
        if origin.instance() != binding.caller_instance() || origin.function() != binding.caller_function()
            || a.destination().local() != *parameter || !a.destination().projections().is_empty()
            || a.destination().ty() != expected.ty() || operand != expected
        {
            return Err(rejected("phase argument transfer changed its exact source operand"));
        }
        let site = Site::new(block, Some(statement as u32));
        let used = query.operand_use(site, operand, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase argument has no exact promoted source use"))?;
        let output = value_at(query, site, SsaVariableIdV1::new(parameter.index()), Event::Define, work)?;
        if !used.belongs_to(query) || result.replace(Transfer { input: used.value(), output }).is_some() {
            return Err(rejected("phase argument has duplicate or foreign source uses"));
        }
    }
    result.ok_or_else(|| rejected("phase argument lost its exact expanded parameter transfer"))
}

pub(super) fn owner_borrow(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    view: &SemanticExpandedRootV1,
    wrapper: &SemanticExpandedDefinedCapabilityV1,
    owner: &SemanticExpandedDefinedCapabilityV1,
    converted: SsaValueV1,
    reference: SsaValueV1,
    work: &mut usize,
) -> PhaseResult<linear_events::Point> {
    let Some(SemanticOperandV1::Copy(expected) | SemanticOperandV1::Move(expected)) = wrapper.arguments().first() else {
        return Err(rejected("phase owner reference is not the retained Rust borrow"));
    };
    if !expected.projections().is_empty() {
        return Err(rejected("phase owner reference must retain its exact whole temporary"));
    }
    let block = wrapper.expanded_call_block();
    let origin = &view.block_origins()[block.index() as usize];
    let mut found = None;
    for (statement, item) in query.function().blocks()[block.index() as usize].statements().iter().enumerate() {
        spend(work, 1)?;
        if !matches!(origin.statements().get(statement), Some(Origin::Source { .. })) { continue; }
        let SemanticStatementKindV1::Assign(a) = item.kind() else { continue; };
        if a.destination() != expected { continue; }
        let SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Mutable, place } = a.value().kind() else {
            return Err(rejected("phase owner reference was redefined by a non-borrow"));
        };
        if found.is_some() || place != owner.destination() || origin.instance() != owner.caller_instance() {
            return Err(rejected("phase owner borrow substituted the original converted owner"));
        }
        let site = Site::new(block, Some(statement as u32));
        let actual = query.borrow_place_use(site, place, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase owner borrow has no exact promoted source value"))?;
        if actual != converted || value_at(query, site, SsaVariableIdV1::new(expected.local().index()), Event::Define, work)? != reference {
            return Err(rejected("phase owner borrow changed its owner or reference SSA identity"));
        }
        found = Some(definition_point(query, site, SsaVariableIdV1::new(expected.local().index()), reference, work)?);
    }
    found.ok_or_else(|| rejected("phase owner lost its exact original mutable borrow"))
}

fn definition_point(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    site: Site,
    variable: SsaVariableIdV1,
    value: SsaValueV1,
    work: &mut usize,
) -> PhaseResult<linear_events::Point> {
    event_point(query, site, variable, value, Event::Define, work)
}

pub(super) fn event_point(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    site: Site,
    variable: SsaVariableIdV1,
    value: SsaValueV1,
    kind: Event,
    work: &mut usize,
) -> PhaseResult<linear_events::Point> {
    let block = SsaBlockIdV1::new(site.block().index());
    let events = query.plan().plan().resolved_events(block)
        .ok_or_else(|| rejected("phase lease definition is unreachable"))?;
    let mut result = None;
    for (index, event) in events {
        spend(work, 1)?;
        let matches = match (kind, event) {
            (Event::Define, SsaResolvedEventV1::Define { variable: v, value: assigned })
                | (Event::Use, SsaResolvedEventV1::Use { variable: v, value: assigned })
                | (Event::Kill, SsaResolvedEventV1::Kill { variable: v, previous: Some(assigned) })
                => *v == variable && *assigned == value,
            _ => false,
        };
        if !matches { continue; }
        if query.event_site(block, *index, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase lease definition source query failed"))? != site
            || result.replace(linear_events::Point { block, event: *index }).is_some()
        {
            return Err(rejected("phase lease has no unique exact return definition"));
        }
    }
    result.ok_or_else(|| rejected("phase lease has no actual caller result definition"))
}
