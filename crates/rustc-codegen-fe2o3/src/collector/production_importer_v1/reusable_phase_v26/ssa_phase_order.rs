//! Event ordering through the existing planner's complete predecessor roster.
//! Acyclic phase scope is separately checked against live source. No SSA graph
//! is rebuilt; scratch capacity and every edge query use the remaining budget.
use super::*;

pub(super) fn precedes(
    query: &ProductionSemanticSsaSourceQueryV1<'_>,
    before: linear_events::Point,
    after: linear_events::Point,
    work: &mut usize,
) -> PhaseResult<bool> {
    spend(work, 4)?;
    let plan = query.plan().plan();
    for point in [before, after] {
        let events = plan
            .resolved_events(point.block)
            .ok_or_else(|| rejected("phase order has no exact reachable event"))?;
        spend(
            work,
            1 + (usize::BITS - events.len().leading_zeros()) as usize,
        )?;
        if plan.resolved_event(point.block, point.event).is_none() {
            return Err(rejected("phase order has no exact reachable event"));
        }
    }
    if before.block == after.block {
        return Ok(before.event < after.event);
    }
    let count = query.function().blocks().len();
    let mut seen = reserve(count, work)?;
    seen.resize(count, false);
    let mut queue = reserve(count, work)?;
    seen[after.block.get() as usize] = true;
    queue.push(after.block);
    let entry = SsaBlockIdV1::new(query.function().entry().index());
    let mut cursor = 0;
    while cursor < queue.len() {
        spend(work, 1)?;
        let block = queue[cursor];
        cursor += 1;
        if block == entry {
            return Ok(false);
        }
        let predecessors = query
            .predecessor_count(block, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase order predecessor roster or remaining work"))?;
        if predecessors == 0 {
            return Err(rejected("phase order lost a reachable predecessor"));
        }
        for index in 0..predecessors {
            let edge = query
                .predecessor(block, index, &mut || spend(work, 1).is_ok())
                .map_err(|_| rejected("phase order predecessor identity or remaining work"))?
                .ok_or_else(|| rejected("phase order predecessor roster changed"))?;
            let source = edge.id().source();
            if source == before.block {
                continue;
            }
            let seen = seen
                .get_mut(source.get() as usize)
                .ok_or_else(|| rejected("phase order predecessor is outside its owner"))?;
            if !*seen {
                *seen = true;
                queue.push(source);
            }
        }
    }
    Ok(true)
}
