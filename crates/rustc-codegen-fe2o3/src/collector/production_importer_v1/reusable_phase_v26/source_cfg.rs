//! Bounded original-MIR control checks. Source order and block numbers are not
//! dominance; every check traverses the actual retained normal control edges.
use super::{
    PhaseResult, rejected,
    source_calls::{reserve, spend},
};
use rustc_middle::mir::{self, BasicBlock, Body, TerminatorKind, UnwindAction};

#[path = "source_cfg/acyclic.rs"]
mod acyclic;

pub(super) fn acyclic(body: &Body<'_>, work: &mut usize) -> PhaseResult<()> {
    struct Original<'a, 'tcx>(&'a Body<'tcx>);
    impl acyclic::Graph for Original<'_, '_> {
        fn node_count(&self) -> usize { self.0.basic_blocks.len() }
        fn successors(&self, node: usize) -> impl Iterator<Item = usize> {
            self.0.basic_blocks[BasicBlock::from_usize(node)]
                .terminator().successors().map(|target| target.as_usize())
        }
    }
    acyclic::check(&Original(body), work)
}

pub(super) fn normal_result_before(
    body: &Body<'_>,
    producer: BasicBlock,
    consumer: BasicBlock,
    work: &mut usize,
) -> PhaseResult<()> {
    let TerminatorKind::Call {
        target: Some(normal),
        unwind: UnwindAction::Unreachable,
        ..
    } = &body.basic_blocks[producer].terminator().kind
    else {
        return Err(rejected("phase origin lacks exact normal return edge"));
    };
    if producer == consumer
        || !reachable(body, mir::START_BLOCK, consumer, None, work)?
        || reachable(
            body,
            mir::START_BLOCK,
            consumer,
            Some((producer, *normal)),
            work,
        )?
    {
        return Err(rejected(
            "phase original normal result does not dominate its consumer",
        ));
    }
    Ok(())
}

fn reachable(
    body: &Body<'_>,
    start: BasicBlock,
    target: BasicBlock,
    excluded: Option<(BasicBlock, BasicBlock)>,
    work: &mut usize,
) -> PhaseResult<bool> {
    let mut seen = reserve(body.basic_blocks.len(), work)?;
    seen.resize(body.basic_blocks.len(), false);
    let mut queue = reserve(body.basic_blocks.len(), work)?;
    seen[start.as_usize()] = true;
    queue.push(start);
    let mut cursor = 0;
    while cursor < queue.len() {
        spend(work, 1)?;
        let block = queue[cursor];
        cursor += 1;
        if block == target {
            return Ok(true);
        }
        for successor in body.basic_blocks[block].terminator().successors() {
            spend(work, 1)?;
            if excluded == Some((block, successor)) {
                continue;
            }
            let visited = seen
                .get_mut(successor.as_usize())
                .ok_or_else(|| rejected("phase original CFG target"))?;
            if !*visited {
                *visited = true;
                queue.push(successor);
            }
        }
    }
    Ok(false)
}
