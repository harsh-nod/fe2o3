//! Definite initialization on the actual per-function CFG. Reachability is the
//! lattice top; reached inputs only lose initialized bits. Read facts are emitted
//! only by replay after convergence. Copy snapshots before its destination write.
use super::super::physical_cfg::InitializationState;
use super::super::queue::CanonicalKirPrivateDataflowQueueV1 as Queue;
use super::intervals::{Partitions, Span};
use super::*;

impl InitializationState for bool {
    const EMPTY: Self = false;
    const MAX_DESCENTS: usize = 1;
    fn written(_: usize) -> Self {
        true
    }
    fn meet(self, incoming: Self) -> Self {
        self && incoming
    }
    fn initialized(self) -> bool {
        self
    }
    fn writer(self) -> Option<usize> {
        None
    }
}

#[derive(Clone, Copy)]
enum Action {
    None,
    Reset(Span),
    Read(Span),
    Write(Span),
    Copy { source: Span, destination: Span },
    UnknownWrite,
}

fn span(range: std::ops::Range<usize>) -> Span {
    Span {
        start: range.start,
        length: range.len(),
    }
}

fn actions(events: &[Event], partitions: &Partitions, budget: &mut Budget<'_>) -> R<Vec<Action>> {
    let mut rows = scratch(events.len(), budget)?;
    for event in events {
        charge(budget, 4)?;
        rows.push(match *event {
            Event::None => Action::None,
            Event::Reset(allocation) => Action::Reset(partitions.allocations[allocation]),
            Event::Read(range) => Action::Read(span(partitions.range(range, budget)?)),
            Event::Write(range) => Action::Write(span(partitions.range(range, budget)?)),
            Event::Copy {
                source,
                destination,
            } => {
                let source = span(partitions.range(source, budget)?);
                let destination = span(partitions.range(destination, budget)?);
                if source.length != destination.length {
                    return Err(arithmetic());
                }
                Action::Copy {
                    source,
                    destination,
                }
            }
            Event::UnknownWrite => Action::UnknownWrite,
        });
    }
    Ok(rows)
}

fn local(range: Span, base: usize) -> R<std::ops::Range<usize>> {
    let start = range.start.checked_sub(base).ok_or_else(arithmetic)?;
    Ok(start..start.checked_add(range.length).ok_or_else(arithmetic)?)
}

fn transfer(
    actions: &[Action],
    operations: std::ops::Range<usize>,
    base: usize,
    state: &mut [bool],
    snapshot: &mut [bool],
    budget: &mut Budget<'_>,
    mut read: impl FnMut(usize, bool, &mut Budget<'_>) -> R<()>,
) -> R<()> {
    for ordinal in operations {
        charge(budget, 4)?;
        match actions[ordinal] {
            Action::None => {}
            Action::Reset(range) | Action::Write(range) => {
                charge(budget, range.length)?;
                let initialized = matches!(actions[ordinal], Action::Write(_));
                state
                    .get_mut(local(range, base)?)
                    .ok_or_else(arithmetic)?
                    .fill(initialized);
            }
            Action::Read(range) => {
                let mut initialized = true;
                for bit in state.get(local(range, base)?).ok_or_else(arithmetic)? {
                    charge(budget, 2)?;
                    initialized &= *bit;
                }
                read(ordinal, initialized, budget)?;
            }
            Action::Copy {
                source,
                destination,
            } => {
                charge(budget, source.length.checked_mul(2).ok_or_else(arithmetic)?)?;
                let source = state.get(local(source, base)?).ok_or_else(arithmetic)?;
                let snapshot = snapshot.get_mut(..source.len()).ok_or_else(arithmetic)?;
                snapshot.copy_from_slice(source);
                state
                    .get_mut(local(destination, base)?)
                    .ok_or_else(arithmetic)?
                    .copy_from_slice(snapshot);
            }
            Action::UnknownWrite => {
                charge(budget, state.len())?;
                state.fill(false);
            }
        }
    }
    Ok(())
}

pub(super) fn analyze(
    inventory: &Inventory<'_>,
    events: &[Event],
    partitions: &Partitions,
    facts: &mut [Fact],
    budget: &mut Budget<'_>,
) -> R<()> {
    if !partitions.complete {
        return Ok(());
    }
    let actions = actions(events, partitions, budget)?;
    let mut base = 0usize;
    for function in inventory.functions() {
        charge(budget, 4)?;
        let mut cells = 0usize;
        for ordinal in function.operations.clone() {
            charge(budget, 3)?;
            if let Action::Reset(range) = actions[ordinal] {
                if range.start != base.checked_add(cells).ok_or_else(arithmetic)? {
                    return Err(arithmetic());
                }
                cells = cells.checked_add(range.length).ok_or_else(arithmetic)?;
            }
        }
        scoped(budget, |budget| {
            let blocks = function.blocks.len();
            if blocks == 0 {
                if !function.operations.is_empty() || cells != 0 {
                    return Err(arithmetic());
                }
                return Ok(());
            }
            let count = blocks.checked_mul(cells).ok_or_else(arithmetic)?;
            let mut inputs = filled(count, false, budget)?;
            let mut reached = filled(blocks, false, budget)?;
            let mut processings = filled(blocks, 0usize, budget)?;
            let mut state = filled(cells, false, budget)?;
            let mut snapshot = filled(cells, false, budget)?;
            let mut queue = Queue::new(blocks, budget)?;
            reached[0] = true;
            queue.push(0, budget)?;
            let bound = cells.checked_add(1).ok_or_else(arithmetic)?;
            while let Some(block) = queue.pop(budget)? {
                charge(budget, 5)?;
                processings[block] = processings[block].checked_add(1).ok_or_else(arithmetic)?;
                if processings[block] > bound {
                    return Err(arithmetic());
                }
                let start = block.checked_mul(cells).ok_or_else(arithmetic)?;
                let end = start.checked_add(cells).ok_or_else(arithmetic)?;
                charge(budget, cells)?;
                state.copy_from_slice(&inputs[start..end]);
                let row = &inventory.blocks()[function.blocks.start + block];
                transfer(
                    &actions,
                    row.operations.clone(),
                    base,
                    &mut state,
                    &mut snapshot,
                    budget,
                    |_, _, _| Ok(()),
                )?;
                for edge in &inventory.edges()[row.edges.clone()] {
                    charge(budget, 6)?;
                    let target = edge.target.block as usize;
                    if edge.target.function != function.coordinate || target >= blocks {
                        return Err(arithmetic());
                    }
                    let start = target.checked_mul(cells).ok_or_else(arithmetic)?;
                    let mut changed = !reached[target];
                    for (index, incoming) in state.iter().enumerate() {
                        charge(budget, 4)?;
                        let slot = &mut inputs[start + index];
                        let merged = if reached[target] {
                            slot.meet(*incoming)
                        } else {
                            *incoming
                        };
                        changed |= merged != *slot;
                        *slot = merged;
                    }
                    reached[target] = true;
                    if changed {
                        queue.push(target, budget)?;
                    }
                }
            }
            for (block, row) in inventory.blocks()[function.blocks.clone()]
                .iter()
                .enumerate()
            {
                charge(budget, 3)?;
                if !reached[block] {
                    for ordinal in row.operations.clone() {
                        charge(budget, 2)?;
                        if facts[ordinal].kind != OperationKindV38::None {
                            facts[ordinal].require(Obligation::Reachability);
                        }
                    }
                    continue;
                }
                let start = block.checked_mul(cells).ok_or_else(arithmetic)?;
                let end = start.checked_add(cells).ok_or_else(arithmetic)?;
                charge(budget, cells)?;
                state.copy_from_slice(&inputs[start..end]);
                transfer(
                    &actions,
                    row.operations.clone(),
                    base,
                    &mut state,
                    &mut snapshot,
                    budget,
                    |ordinal, initialized, budget| {
                        charge(budget, 3)?;
                        facts[ordinal].initialized = Some(initialized);
                        if !initialized {
                            facts[ordinal].require(Obligation::Initialization);
                        }
                        Ok(())
                    },
                )?;
            }
            Ok(())
        })?;
        base = base.checked_add(cells).ok_or_else(arithmetic)?;
    }
    if base != partitions.boundaries.len() {
        return Err(arithmetic());
    }
    Ok(())
}

pub(super) fn headers() -> R<usize> {
    header_sum(&[
        h::<Action>()?,
        h::<Span>()?,
        h::<&Partitions>()?,
        h::<&[Event]>()?,
        h::<&[Action]>()?,
        h::<&mut [Fact]>()?,
        header_copies::<&mut [bool]>(3)?,
        h::<&[bool]>()?,
        h::<std::ops::Range<usize>>()?,
        h::<std::slice::Iter<'_, Event>>()?,
        h::<std::slice::Iter<'_, crate::CanonicalKirFunctionRefV1<'_>>>()?,
        h::<std::slice::Iter<'_, crate::CanonicalKirEdgeRefV1<'_>>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, crate::CanonicalKirBlockRefV1<'_>>>>()?,
        h::<std::iter::Enumerate<std::slice::Iter<'_, bool>>>()?,
        h::<&crate::CanonicalKirBlockRefV1<'_>>()?,
        h::<&crate::CanonicalKirFunctionRefV1<'_>>()?,
        header_copies::<usize>(22)?,
        header_copies::<bool>(6)?,
        h::<Option<usize>>()?,
        h::<(
            &[Action],
            std::ops::Range<usize>,
            usize,
            &mut [bool],
            &mut [bool],
            &mut Budget<'_>,
        )>()?,
        h::<(&mut [Fact],)>()?,
        h::<&mut Cleanup<'_, '_>>()?,
        h::<
            AssertUnwindSafe<(
                &mut Cleanup<'_, '_>,
                (&Inventory<'_>, &[Action], &mut [Fact], usize, usize),
            )>,
        >()?,
    ])
}
