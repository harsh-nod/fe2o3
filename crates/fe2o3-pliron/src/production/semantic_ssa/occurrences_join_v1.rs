//! Linear joins to the one actual input and its freshly constructed plan.

use super::emission::Meter;
use super::*;

pub(super) fn join(
    rows: &mut FunctionRows,
    input: &SsaConstructionInputV1,
    plan: &SsaConstructionPlanV1,
    meter: &mut Meter<'_, '_>,
) -> CaptureResult<()> {
    let function = rows.function;
    meter.require(1, function, None, || {
        rows.blocks.len() == input.blocks().len()
    })?;
    let mut event_end = 0;
    let mut successor_end = 0;
    let mut definition_end = 0;
    for (index, block) in rows.blocks.iter().enumerate() {
        meter.work(1)?;
        let id = SsaBlockIdV1::new(checked_u32(index)?);
        meter.require(7, function, Some(id), || {
            block.block == id
                && block.events.start == event_end
                && block.events.end >= block.events.start
                && block.events.end <= rows.events.len()
                && block.successors.start == successor_end
                && block.successors.end >= block.successors.start
                && block.successors.end <= rows.successors.len()
        })?;
        let actual = &input.blocks()[index];
        meter.require(2, function, Some(id), || {
            block.events.len() == actual.events().len()
                && block.successors.len() == actual.edges().len()
        })?;
        let reachable = plan.is_reachable(id);
        let resolved = plan.resolved_events(id);
        meter.require(1, function, Some(id), || resolved.is_some() == reachable)?;
        let resolved = resolved.unwrap_or(&[]);
        let mut cursor = 0;
        for (ordinal, (row, event)) in rows.events[block.events.clone()]
            .iter_mut()
            .zip(actual.events())
            .enumerate()
        {
            meter.work(1)?;
            meter.require(4, function, Some(id), || {
                row.ordinal as usize == ordinal && row.event == *event && site_block(row.site) == id
            })?;
            let promoted = input
                .promotable()
                .get(event.variable().get() as usize)
                .copied()
                .ok_or_else(|| mismatch(function, Some(id)))?;
            let value = if reachable && promoted {
                meter.work(1)?;
                let (original, value) = resolved
                    .get(cursor)
                    .ok_or_else(|| mismatch(function, Some(id)))?;
                meter.require(3, function, Some(id), || {
                    *original == row.ordinal && event_matches(*event, *value)
                })?;
                cursor += 1;
                Some(*value)
            } else {
                meter.require(1, function, Some(id), || {
                    resolved
                        .get(cursor)
                        .is_none_or(|(next, _)| *next as usize > ordinal)
                })?;
                None
            };
            meter.work(1)?;
            row.reachable = reachable;
            row.promoted = promoted;
            row.resolved = value;
        }
        meter.require(1, function, Some(id), || cursor == resolved.len())?;

        for (ordinal, (edge, actual_edge)) in rows.successors[block.successors.clone()]
            .iter()
            .zip(actual.edges())
            .enumerate()
        {
            meter.work(1)?;
            let expected = SsaEdgeIdV1::new(id, checked_u32(ordinal)?);
            meter.require(7, function, Some(id), || {
                edge.id == expected
                    && edge.edge.target().index() == actual_edge.target().get()
                    && super::super::adapter::semantic_edge_role_v1(edge.edge.role())
                        == actual_edge.role().get()
                    && edge.definitions.start == definition_end
                    && edge.definitions.end >= edge.definitions.start
                    && edge.definitions.end <= rows.edge_definitions.len()
            })?;
            meter.require(1, function, Some(id), || {
                edge.definitions.len() == actual_edge.definitions().len()
            })?;
            let definitions = plan.edge_definitions(expected);
            meter.require(1, function, Some(id), || definitions.is_some() == reachable)?;
            let definitions = definitions.unwrap_or(&[]);
            let mut cursor = 0;
            for (ordinal, (row, variable)) in rows.edge_definitions[edge.definitions.clone()]
                .iter_mut()
                .zip(actual_edge.definitions())
                .enumerate()
            {
                meter.work(1)?;
                meter.require(4, function, Some(id), || {
                    row.edge == expected
                        && row.ordinal as usize == ordinal
                        && row.variable == *variable
                })?;
                let promoted = input
                    .promotable()
                    .get(variable.get() as usize)
                    .copied()
                    .ok_or_else(|| mismatch(function, Some(id)))?;
                let value = if reachable && promoted {
                    meter.work(1)?;
                    let definition = definitions
                        .get(cursor)
                        .ok_or_else(|| mismatch(function, Some(id)))?;
                    meter.require(1, function, Some(id), || definition.variable() == *variable)?;
                    cursor += 1;
                    Some(definition.value())
                } else {
                    meter.require(1, function, Some(id), || {
                        definitions
                            .get(cursor)
                            .is_none_or(|next| next.variable() > *variable)
                    })?;
                    None
                };
                meter.work(1)?;
                row.reachable = reachable;
                row.promoted = promoted;
                row.value = value;
            }
            meter.require(1, function, Some(id), || cursor == definitions.len())?;
            definition_end = edge.definitions.end;
        }
        event_end = block.events.end;
        successor_end = block.successors.end;
    }
    meter.require(3, function, None, || {
        event_end == rows.events.len()
            && successor_end == rows.successors.len()
            && definition_end == rows.edge_definitions.len()
    })?;

    meter.require(1, function, None, || {
        rows.entries.len() == input.entry_definitions().len()
    })?;
    let definitions = plan.entry_definitions();
    let mut cursor = 0;
    for (ordinal, (row, variable)) in rows
        .entries
        .iter_mut()
        .zip(input.entry_definitions())
        .enumerate()
    {
        meter.work(1)?;
        meter.require(2, function, None, || {
            row.ordinal as usize == ordinal && row.variable == *variable
        })?;
        let promoted = input
            .promotable()
            .get(variable.get() as usize)
            .copied()
            .ok_or_else(|| mismatch(function, None))?;
        let value = if promoted {
            meter.work(1)?;
            let definition = definitions
                .get(cursor)
                .ok_or_else(|| mismatch(function, None))?;
            meter.require(1, function, None, || definition.variable() == *variable)?;
            cursor += 1;
            Some(definition.value())
        } else {
            meter.require(1, function, None, || {
                definitions
                    .get(cursor)
                    .is_none_or(|next| next.variable() > *variable)
            })?;
            None
        };
        meter.work(1)?;
        row.value = value;
    }
    meter.require(1, function, None, || cursor == definitions.len())?;
    Ok(())
}

fn site_block(site: ProductionSemanticSsaOccurrenceSiteV1) -> SsaBlockIdV1 {
    match site {
        ProductionSemanticSsaOccurrenceSiteV1::Statement { block, .. }
        | ProductionSemanticSsaOccurrenceSiteV1::Terminator { block } => block,
    }
}

fn event_matches(event: SsaEventV1, resolved: SsaResolvedEventV1) -> bool {
    match (event, resolved) {
        (SsaEventV1::Use(expected), SsaResolvedEventV1::Use { variable, .. })
        | (SsaEventV1::Define(expected), SsaResolvedEventV1::Define { variable, .. })
        | (SsaEventV1::Kill(expected), SsaResolvedEventV1::Kill { variable, .. }) => {
            expected == variable
        }
        _ => false,
    }
}
