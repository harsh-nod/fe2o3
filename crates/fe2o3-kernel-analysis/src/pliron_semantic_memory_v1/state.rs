//! Private finite-trace memory model. Inputs are collected from the live graph,
//! after provenance, bounds, operation-kind and execution-order checks.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironSemanticMemoryVersionV1 {
    Initial,
    AfterWrite {
        invocation: usize,
        event: usize,
        block: usize,
        operation: usize,
    },
}

#[derive(Debug)]
pub(super) struct MemoryEvent {
    pub invocation: usize,
    pub sequence: usize,
    pub block: usize,
    pub operation: usize,
    pub allocation: u64,
    pub indices: Vec<u64>,
    pub write: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MemoryFailure {
    ResourceLimit,
    DuplicateEvent,
    Interference { first: usize, second: usize },
}

/// The bound counts logical retained words and comparison work, not allocator
/// bytes. Trace construction has its own existing step/event bound. Sorting
/// indices keeps address vectors single-owned rather than cloning dense maps.
pub(super) fn versions(
    events: &[MemoryEvent],
    limit: usize,
) -> Result<Vec<Option<PlironSemanticMemoryVersionV1>>, MemoryFailure> {
    let charge = events.iter().try_fold(0usize, |total, event| {
        total.checked_add(12usize.checked_add(event.indices.len())?)
    });
    let comparison_charge = events
        .len()
        .checked_mul(usize::BITS as usize - events.len().max(1).leading_zeros() as usize)
        .and_then(|comparisons| {
            comparisons.checked_mul(
                events
                    .iter()
                    .map(|event| event.indices.len())
                    .max()
                    .unwrap_or(1)
                    .max(1),
            )
        });
    if charge
        .and_then(|words| words.checked_add(comparison_charge?))
        .is_none_or(|words| words > limit)
    {
        return Err(MemoryFailure::ResourceLimit);
    }
    let mut ordered = (0..events.len()).collect::<Vec<_>>();
    ordered.sort_unstable_by(|&a, &b| {
        let a = &events[a];
        let b = &events[b];
        (a.allocation, &a.indices, a.invocation, a.sequence).cmp(&(
            b.allocation,
            &b.indices,
            b.invocation,
            b.sequence,
        ))
    });
    let mut result = vec![None; events.len()];
    let mut start = 0;
    while start < ordered.len() {
        let first = &events[ordered[start]];
        let mut end = start + 1;
        while end < ordered.len() {
            let event = &events[ordered[end]];
            if (event.allocation, &event.indices) != (first.allocation, &first.indices) {
                break;
            }
            end += 1;
        }
        let group = &ordered[start..end];
        if let Some(&writer) = group.iter().find(|&&index| events[index].write) {
            if let Some(&other) = group
                .iter()
                .find(|&&index| events[index].invocation != events[writer].invocation)
            {
                return Err(MemoryFailure::Interference {
                    first: writer,
                    second: other,
                });
            }
        }
        let mut version = PlironSemanticMemoryVersionV1::Initial;
        let mut previous = None;
        for &index in group {
            let event = &events[index];
            let identity = (event.invocation, event.sequence);
            if previous == Some(identity) {
                return Err(MemoryFailure::DuplicateEvent);
            }
            if previous.is_some_and(|(invocation, _)| invocation != event.invocation) {
                version = PlironSemanticMemoryVersionV1::Initial;
            }
            if event.write {
                version = PlironSemanticMemoryVersionV1::AfterWrite {
                    invocation: event.invocation,
                    event: event.sequence,
                    block: event.block,
                    operation: event.operation,
                };
            } else {
                result[index] = Some(version);
            }
            previous = Some(identity);
        }
        start = end;
    }
    Ok(result)
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
