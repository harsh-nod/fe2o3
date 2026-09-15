//! Safety over every CFG edge, not a termination or convergence proof.
use super::*;

pub(super) fn reaches(cfg: &IndexedControlFlow, from: Point, to: Point, b: &mut Budget) -> Result<bool> {
    b.work(1)?;
    if from.block == to.block && from.operation < to.operation { return Ok(true); }
    walk(cfg, from.block, Some(to.block), None, b)
}

pub(super) fn closes_every_exit(cfg: &IndexedControlFlow, begin: Point, end: Point, b: &mut Budget) -> Result<()> {
    if !before(cfg, begin, end, b)? { return Err(Error::NonDominatingUse); }
    if begin.block == end.block { return Ok(()); }
    walk(cfg, begin.block, None, Some(end.block), b)?;
    Ok(())
}

// A loop containing only shared reads may execute zero or more times with the
// same live operands. Consuming operations in cycles are rejected separately.
fn walk(cfg: &IndexedControlFlow, start: BlockId, target: Option<BlockId>,
    stop: Option<BlockId>, b: &mut Budget) -> Result<bool> {
    let mut seen = b.reserve::<bool>(cfg.block_count())?;
    let mut queue = b.reserve::<BlockId>(cfg.block_count())?;
    b.work(cfg.block_count())?;
    seen.resize(cfg.block_count(), false);
    let first = cfg.block_position(start).ok_or(Error::MissingProducer)?;
    seen[first] = true;
    queue.push(start);
    let mut cursor = 0;
    let mut found = false;
    while let Some(block) = queue.get(cursor).copied() {
        b.work(1)?;
        cursor += 1;
        let edges = cfg.outgoing_edges(block).ok_or(Error::MissingProducer)?;
        if edges.is_empty() && stop.is_some() { return Err(Error::MissingEnd); }
        for edge in edges {
            b.work(1)?;
            let next = cfg.edge_target(edge).ok_or(Error::MissingProducer)?;
            if Some(next) == target { found = true; break; }
            if Some(next) == stop { continue; }
            let i = cfg.block_position(next).ok_or(Error::MissingProducer)?;
            if !seen[i] {
                seen[i] = true;
                if queue.len() == queue.capacity() { return Err(Error::StorageLimit); }
                queue.push(next);
            }
        }
        if found { break; }
    }
    b.release_vec(queue);
    b.release_vec(seen);
    Ok(found)
}
