use super::queue::CanonicalKirPrivateDataflowQueueV1 as Queue;
use super::*;

#[derive(Clone, Copy)]
pub(super) enum Event {
    None,
    Reset { start: usize, length: usize },
    Write(usize),
    Read(usize),
}

// Events retain dense inventory/cell ordinals only. They are not initialization
// facts; read permission is established exclusively by the converged replay.
fn events(
    inventory: &CanonicalKirInventoryV1<'_>,
    addresses: &[Option<Address>],
    budget: &mut Budget<'_>,
) -> R<Vec<Event>> {
    let mut rows = scratch::<Event>(inventory.operations().len(), budget)?;
    for row in inventory.operations() {
        charge(budget, 4)?;
        let event = match row.operation.kind {
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            } => {
                let address = addresses[row.results.start].ok_or_else(arithmetic)?;
                Event::Reset {
                    start: address.start,
                    length: address.length,
                }
            }
            OperationKind::Load { pointer, access }
            | OperationKind::Store {
                pointer, access, ..
            } if access.address_space == AddressSpace::Private => {
                let definition = index(inventory, row.coordinate.block.function, pointer, budget)?;
                charge(budget, 3)?;
                let address = addresses[definition].ok_or_else(arithmetic)?;
                let cell = address
                    .start
                    .checked_add(address.offset)
                    .ok_or_else(arithmetic)?;
                if matches!(row.operation.kind, OperationKind::Store { .. }) {
                    Event::Write(cell)
                } else {
                    Event::Read(cell)
                }
            }
            _ => Event::None,
        };
        if rows.len() == rows.capacity() {
            return Err(arithmetic());
        }
        rows.push(event);
    }
    Ok(rows)
}

pub(super) fn row_start(block: usize, cells: usize) -> R<usize> {
    block.checked_mul(cells).ok_or_else(arithmetic)
}

pub(super) fn transfer(
    events: &[Event],
    operations: std::ops::Range<usize>,
    base: usize,
    state: &mut [Option<usize>],
    budget: &mut Budget<'_>,
    mut read: impl FnMut(usize, Option<usize>, &mut Budget<'_>) -> R<()>,
) -> R<()> {
    for ordinal in operations {
        charge(budget, 3)?;
        match events[ordinal] {
            Event::None => {}
            Event::Reset { start, length } => {
                let start = start.checked_sub(base).ok_or_else(arithmetic)?;
                let end = start.checked_add(length).ok_or_else(arithmetic)?;
                charge(budget, length)?;
                state.get_mut(start..end).ok_or_else(arithmetic)?.fill(None);
            }
            Event::Write(cell) => {
                let cell = cell.checked_sub(base).ok_or_else(arithmetic)?;
                *state.get_mut(cell).ok_or_else(arithmetic)? = Some(ordinal);
            }
            Event::Read(cell) => {
                let cell = cell.checked_sub(base).ok_or_else(arithmetic)?;
                read(ordinal, *state.get(cell).ok_or_else(arithmetic)?, budget)?;
            }
        }
    }
    Ok(())
}

pub(super) fn check(
    inventory: &CanonicalKirInventoryV1<'_>,
    addresses: &[Option<Address>],
    latest: &mut [Option<usize>],
    budget: &mut Budget<'_>,
) -> R<()> {
    let events = events(inventory, addresses, budget)?;
    let mut base = 0usize;
    for function in inventory.functions() {
        charge(budget, 3)?;
        let mut cells = 0usize;
        for ordinal in function.operations.clone() {
            charge(budget, 3)?;
            if let Event::Reset { start, length } = events[ordinal] {
                if start != base.checked_add(cells).ok_or_else(arithmetic)? {
                    return Err(arithmetic());
                }
                cells = cells.checked_add(length).ok_or_else(arithmetic)?;
            }
        }
        if cells == 0 {
            continue;
        }
        let blocks = function.blocks.len();
        if blocks == 0 {
            return Err(arithmetic());
        }
        let count = blocks.checked_mul(cells).ok_or_else(arithmetic)?;
        let mut inputs = scratch::<Option<usize>>(count, budget)?;
        let mut reached = scratch::<bool>(blocks, budget)?;
        let mut processings = scratch::<usize>(blocks, budget)?;
        let mut state = scratch::<Option<usize>>(cells, budget)?;
        let mut queue = Queue::new(blocks, budget)?;
        charge(
            budget,
            count
                .checked_add(blocks.checked_mul(2).ok_or_else(arithmetic)?)
                .and_then(|n| n.checked_add(cells))
                .ok_or_else(arithmetic)?,
        )?;
        inputs.resize(count, None);
        reached.resize(blocks, false);
        processings.resize(blocks, 0);
        state.resize(cells, None);
        reached[0] = true;
        queue.push(0, budget)?;
        let bound = cells.checked_add(1).ok_or_else(arithmetic)?;
        while let Some(block) = queue.pop(budget)? {
            charge(budget, 4)?;
            processings[block] = processings[block].checked_add(1).ok_or_else(arithmetic)?;
            if processings[block] > bound {
                return Err(arithmetic());
            }
            let start = row_start(block, cells)?;
            let end = start.checked_add(cells).ok_or_else(arithmetic)?;
            charge(budget, cells)?;
            state.copy_from_slice(&inputs[start..end]);
            let actual = function
                .blocks
                .start
                .checked_add(block)
                .ok_or_else(arithmetic)?;
            let row = &inventory.blocks()[actual];
            transfer(
                &events,
                row.operations.clone(),
                base,
                &mut state,
                budget,
                |_, _, _| Ok(()),
            )?;
            for edge in &inventory.edges()[row.edges.clone()] {
                charge(budget, 5)?;
                let target = edge.target.block as usize;
                if edge.target.function != function.coordinate || target >= blocks {
                    return Err(arithmetic());
                }
                let start = row_start(target, cells)?;
                let mut changed = !reached[target];
                for (cell, incoming) in state.iter().enumerate() {
                    charge(budget, 4)?;
                    let slot = &mut inputs[start.checked_add(cell).ok_or_else(arithmetic)?];
                    if !reached[target] {
                        *slot = *incoming;
                    } else if slot.is_some() && *slot != *incoming {
                        *slot = None;
                        changed = true;
                    }
                }
                reached[target] = true;
                if changed {
                    queue.push(target, budget)?;
                }
            }
        }
        // No provisional load result escapes the fixed point. Reconcile old
        // local anchors too, while preserving historically accepted dead reads.
        for (block, row) in inventory.blocks()[function.blocks.clone()]
            .iter()
            .enumerate()
        {
            charge(budget, 3)?;
            if !reached[block] {
                for ordinal in row.operations.clone() {
                    charge(budget, 2)?;
                    if matches!(events[ordinal], Event::Read(_)) && latest[ordinal].is_none() {
                        return Err(refused(
                            "private",
                            "cross-block Load is structurally reachable",
                        ));
                    }
                }
                continue;
            }
            let start = row_start(block, cells)?;
            let end = start.checked_add(cells).ok_or_else(arithmetic)?;
            charge(budget, cells)?;
            state.copy_from_slice(&inputs[start..end]);
            transfer(
                &events,
                row.operations.clone(),
                base,
                &mut state,
                budget,
                |ordinal, store, budget| {
                    charge(budget, 4)?;
                    let store = store.ok_or_else(|| {
                        refused("private", "Load requires one exact reaching Store")
                    })?;
                    if latest[ordinal].is_some_and(|local| local != store) {
                        return Err(refused(
                            "private",
                            "local and converged Store anchors agree",
                        ));
                    }
                    latest[ordinal] = Some(store);
                    Ok(())
                },
            )?;
        }
        base = base.checked_add(cells).ok_or_else(arithmetic)?;
    }
    Ok(())
}
