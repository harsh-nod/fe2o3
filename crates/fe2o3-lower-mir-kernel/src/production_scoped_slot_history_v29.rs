// Physical initialization with producer-anchored source-local invalidations.
// Full source/access equivalence, implicit effects and relocation remain separate.
use super::*;

#[cfg(test)]
#[path = "production_scoped_slot_history_v29_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Cell {
    slot: usize,
    index: u64,
}

type CellOrigin = OriginStateV1<Option<Cell>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EventKind {
    Read,
    Set(bool),
    KillSlot,
    Preserve,
}

#[derive(Clone, Copy, Debug)]
struct Event {
    cell: Cell,
    kind: EventKind,
    operation: usize,
    sequence: usize,
}

impl Event {
    fn reset(self, cell: Cell) -> Option<bool> {
        match self.kind {
            EventKind::KillSlot if self.cell.slot == cell.slot => Some(false),
            EventKind::Set(value) if self.cell == cell => Some(value),
            _ => None,
        }
    }
}

struct HistoryBlock {
    events: std::ops::Range<usize>,
    successors: std::ops::Range<usize>,
}

fn unsigned_error(
    error: PrivateArrayRelationErrorV1<ProductionSemanticKirErrorV1>,
) -> ProductionSemanticKirErrorV1 {
    match error {
        PrivateArrayRelationErrorV1::Work(error) => error,
        _ => invalid("scoped slot cell requires an exact unsigned constant offset"),
    }
}

fn cell_origins(
    function: &Function,
    graph: &SlotUseGraphV29<'_>,
    slots: &[ScopedSourceSlotV29],
    first_slot: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Vec<CellOrigin>> {
    let body = function.body.as_ref().ok_or_else(scoped_slot_error_v29)?;
    let count = graph.origins.len();
    let mut seeds = emission_vec_v1(count, budget)?;
    let mut resets = emission_vec_v1(count, budget)?;
    let mut definitions = emission_vec_v1(count, budget)?;
    budget.charge_work(argument_product_v1(count, 3)?)?;
    for origin in &graph.origins {
        seeds.push(match origin {
            Origin::Exact(None) => CellOrigin::Exact(None),
            Origin::Exact(Some(_)) | Origin::Pending => CellOrigin::Pending,
            Origin::Unknown => CellOrigin::Unknown,
        });
    }
    resets.resize(count, false);
    definitions.resize(count, None);
    for (block_ordinal, block) in body.blocks.iter().enumerate() {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        for (operation, row) in block.operations.iter().enumerate() {
            budget.charge_work(row.results.len())?;
            for result in &row.results {
                definitions[graph.value(result.id, budget)?] =
                    Some(PrivateArrayPhysicalLocationV1 {
                        block_ordinal,
                        block: block.id,
                        operation,
                    });
            }
        }
    }
    budget.charge_work(slots.len())?;
    for (ordinal, slot) in slots.iter().enumerate() {
        seeds[graph.value(slot.origin.pointer, budget)?] = CellOrigin::Exact(Some(Cell {
            slot: argument_sum_v1(&[first_slot, ordinal])?,
            index: 0,
        }));
    }
    for (_, block) in &graph.blocks {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        for operation in &block.operations {
            let OperationKind::GetElementPointer { base, offset } = operation.kind else {
                continue;
            };
            let Some(slot_index) = graph.exact(base, budget)? else {
                continue;
            };
            let slot = slot_index
                .checked_sub(first_slot)
                .and_then(|i| slots.get(i))
                .ok_or_else(scoped_slot_error_v29)?;
            if base != slot.origin.pointer {
                return Err(invalid(
                    "scoped slot chained offset needs exact cell evidence",
                ));
            }
            let location = definitions[graph.value(offset, budget)?]
                .ok_or_else(|| invalid("scoped slot offset has no constant definition"))?;
            let Type::Scalar(scalar) = graph.ty(offset, budget)? else {
                return Err(invalid("scoped slot offset is not a scalar"));
            };
            let index =
                private_array_unsigned_operation_v1(body, location, offset, *scalar, budget)
                    .map_err(unsigned_error)?;
            let result = graph.value(graph.result(operation)?.id, budget)?;
            seeds[result] = CellOrigin::Exact(Some(Cell {
                slot: slot_index,
                index,
            }));
            // This exact GEP has been checked against the actual allocation and
            // constant. Its cell is not the unchanged cell of its base.
            resets[result] = true;
        }
    }
    let mut edges = 0;
    graph.dependencies(budget, |_, target, budget| {
        budget.charge_work(1)?;
        if !resets[target] {
            edges = argument_sum_v1(&[edges, 1])?;
        }
        Ok(())
    })?;
    let mut work = OriginWorkV1::new(count, edges, budget).map_err(origin_error)?;
    for seed in seeds {
        work.seed_next(seed, budget).map_err(origin_error)?;
    }
    graph.dependencies(budget, |source, target, budget| {
        budget.charge_work(1)?;
        if !resets[target] {
            work.add_link(source, target, budget)
                .map_err(origin_error)?;
        }
        Ok(())
    })?;
    work.solve(budget).map_err(origin_error)
}

fn checked_cell(
    pointer: ValueId,
    access: &MemoryAccess,
    graph: &SlotUseGraphV29<'_>,
    origins: &[CellOrigin],
    slots: &[ScopedSourceSlotV29],
    first_slot: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Cell> {
    let CellOrigin::Exact(Some(cell)) = origins[graph.value(pointer, budget)?] else {
        return Err(invalid("scoped slot access has ambiguous cell provenance"));
    };
    budget.charge_work(7)?;
    let slot = cell
        .slot
        .checked_sub(first_slot)
        .and_then(|i| slots.get(i))
        .ok_or_else(scoped_slot_error_v29)?;
    let offset = cell
        .index
        .checked_mul(slot.element.size)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let end = offset
        .checked_add(slot.element.size)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if cell.index >= slot.length
        || end > slot.bytes
        || !access.alignment.is_power_of_two()
        || access.alignment > slot.element.alignment
        || offset % u64::from(access.alignment) != 0
    {
        return Err(invalid(
            "scoped slot access exceeds its cell bounds or alignment",
        ));
    }
    Ok(cell)
}

fn block_index(
    graph: &SlotUseGraphV29<'_>,
    block: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<usize> {
    budget.charge_work(call_splice_search_work_v1(graph.blocks.len()))?;
    graph
        .blocks
        .binary_search_by_key(&block, |(id, _)| *id)
        .map_err(|_| invalid("scoped slot history CFG target is missing"))
}

fn check_history(
    blocks: &[HistoryBlock],
    successors: &[usize],
    events: &[Event],
    cells: &[Cell],
    entry: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    budget.charge_work(argument_sum_v1(&[
        3,
        blocks.len(),
        successors.len(),
        cells.len(),
    ])?)?;
    if entry >= blocks.len()
        || cells.windows(2).any(|pair| pair[0] >= pair[1])
        || successors.iter().any(|&target| target >= blocks.len())
    {
        return Err(invalid("scoped slot history roster is inconsistent"));
    }
    let (mut event_end, mut edge_end) = (0, 0);
    for block in blocks {
        if block.events.start != event_end
            || block.successors.start != edge_end
            || events.get(block.events.clone()).is_none()
            || successors.get(block.successors.clone()).is_none()
        {
            return Err(invalid("scoped slot history block ranges are inconsistent"));
        }
        budget.charge_work(block.events.len())?;
        if events[block.events.clone()].windows(2).any(|pair| {
            (pair[0].operation, pair[0].sequence) >= (pair[1].operation, pair[1].sequence)
        }) {
            return Err(invalid("scoped slot history event order is inconsistent"));
        }
        event_end = block.events.end;
        edge_end = block.successors.end;
    }
    if event_end != events.len() || edge_end != successors.len() {
        return Err(invalid("scoped slot history roster is incomplete"));
    }
    for event in events {
        budget.charge_work(call_splice_search_work_v1(cells.len()))?;
        if cells.binary_search(&event.cell).is_err() {
            return Err(invalid("scoped slot history event has no observed cell"));
        }
    }
    let nodes = argument_sum_v1(&[blocks.len(), events.len()])?;
    for &cell in cells {
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.charge_work(events.len())?;
            let mut edges = successors.len();
            for event in events {
                if event.reset(cell).is_none() {
                    edges = argument_sum_v1(&[edges, 1])?;
                }
            }
            let mut work = OriginWorkV1::new(nodes, edges, budget).map_err(origin_error)?;
            for block in 0..blocks.len() {
                work.seed_next(
                    if block == entry {
                        OriginStateV1::Exact(false)
                    } else {
                        OriginStateV1::Pending
                    },
                    budget,
                )
                .map_err(origin_error)?;
            }
            budget.charge_work(events.len())?;
            for event in events {
                let seed = event
                    .reset(cell)
                    .map(OriginStateV1::Exact)
                    .unwrap_or(OriginStateV1::Pending);
                work.seed_next(seed, budget).map_err(origin_error)?;
            }
            for (index, block) in blocks.iter().enumerate() {
                budget.charge_work(argument_sum_v1(&[
                    1,
                    block.events.len(),
                    block.successors.len(),
                ])?)?;
                let mut previous = index;
                for event_index in block.events.clone() {
                    let node = argument_sum_v1(&[blocks.len(), event_index])?;
                    let event = events[event_index];
                    if event.reset(cell).is_none() {
                        work.add_link(previous, node, budget)
                            .map_err(origin_error)?;
                    }
                    previous = node;
                }
                for &target in &successors[block.successors.clone()] {
                    work.add_link(previous, target, budget)
                        .map_err(origin_error)?;
                }
            }
            let states = work.solve(budget).map_err(origin_error)?;
            budget.charge_work(events.len())?;
            for (index, event) in events.iter().enumerate() {
                if event.cell == cell
                    && event.kind == EventKind::Read
                    && states[blocks.len() + index] != OriginStateV1::Exact(true)
                {
                    return Err(invalid(
                        "scoped slot read is not initialized in its fresh physical activation",
                    ));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn check(
    function: &Function,
    graph: &SlotUseGraphV29<'_>,
    slots: &[ScopedSourceSlotV29],
    first_slot: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    check_with_source_kills(function, graph, slots, first_slot, &[], budget)
}

pub(super) fn check_with_source_kills(
    function: &Function,
    graph: &SlotUseGraphV29<'_>,
    slots: &[ScopedSourceSlotV29],
    first_slot: usize,
    anchors: &[ScopedMemoryAnchorV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    let body = function.body.as_ref().ok_or_else(scoped_slot_error_v29)?;
    let entry = body.blocks.first().ok_or_else(scoped_slot_error_v29)?.id;
    budget.charge_work(slots.len())?;
    if slots
        .iter()
        .any(|slot| slot.allocation.block_ordinal != 0 || slot.allocation.block != entry)
    {
        return Err(invalid("scoped slot allocation is outside the fresh entry"));
    }
    let origins = cell_origins(function, graph, slots, first_slot, budget)?;
    let (mut operations, mut edges) = (0, 0);
    for (_, block) in &graph.blocks {
        budget.charge_work(1)?;
        operations = argument_sum_v1(&[operations, block.operations.len()])?;
        block
            .terminator
            .as_ref()
            .ok_or_else(scoped_slot_error_v29)?
            .try_visit_edges_v1(|target, _| {
                budget.charge_work(1)?;
                if target == entry {
                    return Err(invalid("scoped slot allocation block is reentered"));
                }
                edges = argument_sum_v1(&[edges, 1])?;
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
    }
    let capacity = argument_sum_v1(&[operations, anchors.len()])?;
    let mut events = emission_vec_v1(capacity, budget)?;
    let mut cells = emission_vec_v1(capacity, budget)?;
    let mut blocks = emission_vec_v1(graph.blocks.len(), budget)?;
    let mut successors = emission_vec_v1(edges, budget)?;
    let mut kills = emission_vec_v1(anchors.len(), budget)?;
    budget.charge_work(anchors.len())?;
    for (sequence, row) in anchors.iter().enumerate() {
        let ScopedMemoryAnchorKindV29::Kill { local, .. } = row.kind else {
            continue;
        };
        budget.charge_work(scoped_initialization_search_work_v29(slots.len()))?;
        let slot = slots
            .binary_search_by_key(&local, |slot| slot.origin.local)
            .map_err(|_| scoped_memory_error_v29())?;
        kills.push((
            row.block,
            Event {
                cell: Cell {
                    slot: argument_sum_v1(&[first_slot, slot])?,
                    index: 0,
                },
                kind: EventKind::KillSlot,
                operation: row.position,
                sequence,
            },
        ));
    }
    call_splice_sort_work_v1(argument_product_v1(kills.len(), 3)?, budget).map_err(graph_error)?;
    kills.sort_unstable_by_key(|(block, event)| (*block, event.operation, event.sequence));
    let mut next_kill = 0;
    for (_, block) in &graph.blocks {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        let start = events.len();
        for (operation, row) in block.operations.iter().enumerate() {
            let (pointer, access, kind) = match &row.kind {
                OperationKind::Load { pointer, access }
                | OperationKind::GuardedLoad {
                    pointer, access, ..
                } => (*pointer, access, EventKind::Read),
                OperationKind::Store {
                    pointer, access, ..
                } => (*pointer, access, EventKind::Set(true)),
                OperationKind::GuardedStore {
                    pointer, access, ..
                } => (*pointer, access, EventKind::Preserve),
                _ => continue,
            };
            if graph.exact(pointer, budget)?.is_none() {
                continue;
            }
            let cell = checked_cell(pointer, access, graph, &origins, slots, first_slot, budget)?;
            events.push(Event {
                cell,
                kind,
                operation,
                sequence: usize::MAX,
            });
            cells.push(cell);
        }
        let kill_start = next_kill;
        while let Some(&(id, event)) = kills.get(next_kill) {
            budget.charge_work(2)?;
            if id != block.id {
                break;
            }
            if event.operation > block.operations.len() {
                return Err(scoped_memory_error_v29());
            }
            events.push(event);
            cells.push(event.cell);
            next_kill = argument_sum_v1(&[next_kill, 1])?;
        }
        if next_kill != kill_start {
            let slice = &mut events[start..];
            call_splice_sort_work_v1(argument_product_v1(slice.len(), 2)?, budget)
                .map_err(graph_error)?;
            slice.sort_unstable_by_key(|event| (event.operation, event.sequence));
        }
        let edge_start = successors.len();
        block
            .terminator
            .as_ref()
            .ok_or_else(scoped_slot_error_v29)?
            .try_visit_edges_v1(|target, _| {
                successors.push(block_index(graph, target, budget)?);
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
        blocks.push(HistoryBlock {
            events: start..events.len(),
            successors: edge_start..successors.len(),
        });
    }
    if next_kill != kills.len() {
        return Err(scoped_memory_error_v29());
    }
    call_splice_sort_work_v1(cells.len(), budget).map_err(graph_error)?;
    cells.sort_unstable();
    budget.charge_work(cells.len())?;
    cells.dedup();
    let entry = block_index(graph, entry, budget)?;
    check_history(&blocks, &successors, &events, &cells, entry, budget)
}
