//! Join typed producers to original events on every completed invocation path.
use super::*;
use crate::pliron_invocation_trace::PlironInvocationTraceV1;
use dialect_kernel::ReturnOp;
use std::collections::HashMap;

pub(super) fn check(
    context: &Context,
    inventory: &BoundedPlironFunctionInventoryV1,
    reads: &[PlironProvedSemanticReadV1],
    traces: &[PlironInvocationTraceV1],
    work: &mut control_flow::Work,
) -> Result<Vec<usize>, PlironSemanticMemoryErrorV1> {
    use PlironSemanticMemoryErrorV1 as E;
    work.charge(reads.len().checked_mul(4).ok_or(E::ResourceLimit)?)?;
    let producers = reads
        .iter()
        .enumerate()
        .map(|(i, r)| (r.producer, i))
        .collect::<HashMap<_, _>>();
    let results = reads
        .iter()
        .enumerate()
        .map(|(i, r)| (r.result, i))
        .collect::<HashMap<_, _>>();
    let mut counts = vec![0usize; reads.len()];
    let mut consumed = vec![false; reads.len()];
    for trace in traces {
        let mut available = HashSet::new();
        let mut visited = HashSet::new();
        let mut expected_block = inventory.blocks().first().copied();
        let mut event_cursor = 0;
        for visit in &trace.blocks {
            work.charge(1)?;
            let block = *inventory
                .blocks()
                .get(visit.block)
                .ok_or(E::IncompleteTrace)?;
            if visit.summarized
                || expected_block != Some(block)
                || !visited.insert(block)
                || visit.events.start != event_cursor
                || visit.events.end > trace.events.len()
                || visit.events.start > visit.events.end
            {
                return Err(E::IncompleteTrace);
            }
            let arguments = block.deref(context).get_num_arguments();
            work.charge(arguments)?;
            available.extend(block.deref(context).arguments());
            for location in inventory.block_operations(visit.block) {
                let pointer = location.pointer();
                let raw = pointer.deref(context);
                work.charge(1)?;
                work.charge(raw.get_num_operands())?;
                work.charge(raw.get_num_results())?;
                for operand in raw.operands() {
                    if !available.contains(&operand) {
                        return Err(E::NonDominatingOperand {
                            site: PlironSemanticMemorySiteV1::new(
                                visit.block,
                                location.operation(),
                            ),
                        });
                    }
                    if let Some(&i) = results.get(&operand) {
                        consumed[i] = true;
                    }
                }
                if Operation::is_op::<RankedAccessOp>(pointer, context) {
                    let Some(PlironTraceEventV1::Memory {
                        location: event, ..
                    }) = trace
                        .events
                        .get(event_cursor)
                        .filter(|_| event_cursor < visit.events.end)
                    else {
                        return Err(E::IncompleteTrace);
                    };
                    if event.block != visit.block || event.operation != location.operation() {
                        return Err(E::IncompleteTrace);
                    }
                    event_cursor += 1;
                }
                if let Some(&i) = producers.get(&pointer) {
                    // Static pairing established adjacency in this same block;
                    // the preceding access just consumed its original event.
                    counts[i] = counts[i].checked_add(1).ok_or(E::ResourceLimit)?;
                }
                available.extend(raw.results());
            }
            if event_cursor != visit.events.end {
                return Err(E::IncompleteTrace);
            }
            let terminator = inventory
                .block_operations(visit.block)
                .last()
                .ok_or(E::IncompleteTrace)?
                .pointer();
            let raw = terminator.deref(context);
            expected_block = match visit.successor {
                Some(slot) if slot < raw.get_num_successors() => Some(raw.get_successor(slot)),
                None if Operation::is_op::<ReturnOp>(terminator, context) => None,
                _ => return Err(E::IncompleteTrace),
            };
        }
        if expected_block.is_some() || event_cursor != trace.events.len() {
            return Err(E::IncompleteTrace);
        }
    }
    if counts.contains(&0) {
        return Err(E::IncompleteTrace);
    }
    if consumed.contains(&false) {
        return Err(E::UnconsumedRead);
    }
    Ok(counts)
}
