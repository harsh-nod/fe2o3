// Physical initialization with producer-anchored source-local invalidations.
// Full source/access equivalence, implicit effects and relocation remain separate.
use super::*;
include!("production_scoped_slot_index_history_v29.rs");
include!("production_scoped_static_object_history_v29.rs");

#[cfg(test)]
#[path = "production_scoped_slot_history_v29_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "production_scoped_slot_selector_history_v29_tests.rs"]
mod selector_tests;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Cell {
    slot: usize,
    index: CellIndex,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CellIndex {
    Literal(u64),
    Selected(usize),
    Whole,
}

type CellOrigin = OriginStateV1<Option<Cell>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EventKind {
    Read,
    FailureRead,
    Set(bool),
    KillSlot,
    FailureKillSlot,
    ForgetSelector,
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
            EventKind::KillSlot | EventKind::FailureKillSlot if self.cell.slot == cell.slot => {
                Some(false)
            }
            EventKind::Set(false)
                if self.cell.slot == cell.slot
                    && (self.cell == cell
                        || matches!(self.cell.index, CellIndex::Selected(_))
                        || matches!(cell.index, CellIndex::Selected(_))) =>
            {
                Some(false)
            }
            EventKind::ForgetSelector if self.cell == cell => Some(false),
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
    selected: Option<&mut selectors::Context<'_, '_, '_, '_>>,
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
    let selected = if let Some(selected) = selected {
        selected.prepare(graph, &definitions, slots, first_slot, budget)?;
        Some(selected)
    } else {
        None
    };
    budget.charge_work(slots.len())?;
    for (ordinal, slot) in slots.iter().enumerate() {
        seeds[graph.value(slot.origin.pointer, budget)?] = CellOrigin::Exact(Some(Cell {
            slot: argument_sum_v1(&[first_slot, ordinal])?,
            index: CellIndex::Literal(0),
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
            let Type::Scalar(scalar) = graph.ty(offset, budget)? else {
                return Err(invalid("scoped slot offset is not a scalar"));
            };
            let pointer = graph.result(operation)?.id;
            let index = if let Some(selected) = selected.as_ref()
                && let Some(index) = selected.cell(pointer, slot_index, base, offset, budget)?
            {
                CellIndex::Selected(index)
            } else {
                let location = definitions[graph.value(offset, budget)?]
                    .ok_or_else(|| invalid("scoped slot offset has no constant definition"))?;
                CellIndex::Literal(
                    private_array_unsigned_operation_v1(body, location, offset, *scalar, budget)
                        .map_err(unsigned_error)?,
                )
            };
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
    block: BlockId,
    operation: usize,
    writing: bool,
    selected: Option<&selectors::Context<'_, '_, '_, '_>>,
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
    let scalar = slot.scalar_array()?;
    let index = match cell.index {
        CellIndex::Literal(index) => index,
        CellIndex::Selected(key) => {
            selected.ok_or_else(scoped_slot_error_v29)?.access(
                pointer, key, cell.slot, block, operation, writing, access, slot, budget,
            )?;
            return Ok(cell);
        }
        CellIndex::Whole => return Err(scoped_slot_error_v29()),
    };
    let offset = index
        .checked_mul(scalar.element.size)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let end = offset
        .checked_add(scalar.element.size)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    if index >= scalar.length
        || end > scalar.bytes
        || !access.alignment.is_power_of_two()
        || access.alignment > scalar.element.alignment
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

fn check_history_roster(
    blocks: &[HistoryBlock],
    successors: &[usize],
    events: &[Event],
    cells: &[Cell],
    entry: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<usize> {
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
    argument_sum_v1(&[blocks.len(), events.len()]).map_err(Into::into)
}

fn history_reset(
    event: Event,
    cell: Cell,
    node: usize,
    whole: Option<&[bool]>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Option<bool>> {
    if let Some(whole) = whole {
        budget.charge_work(1)?;
        if *whole.get(node).ok_or_else(scoped_slot_error_v29)? {
            return Ok(Some(true));
        }
    }
    Ok(event.reset(cell))
}

// The same finite bool solver serves literal cells and source-bound selectors.
// Whole coverage is an independently proved reset, never an additional incoming
// fact that could erase the selected-store/whole-store branch correlation.
#[allow(clippy::too_many_arguments)]
fn with_history_cell(
    blocks: &[HistoryBlock],
    successors: &[usize],
    events: &[Event],
    cell: Cell,
    entry: usize,
    nodes: usize,
    whole: Option<&[bool]>,
    budget: &mut ArgumentBudgetV1<'_>,
    inspect: impl FnOnce(&[OriginStateV1<bool>], &mut ArgumentBudgetV1<'_>) -> UseResult<()>,
) -> UseResult<()> {
    with_canonical_call_scratch_v1(budget, |budget| {
        if let Some(whole) = whole {
            budget.charge_work(argument_sum_v1(&[1, blocks.len()])?)?;
            if whole.len() != nodes {
                return Err(invalid("scoped slot whole-coverage roster changed"));
            }
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Option<usize>>(),
            std::mem::size_of::<usize>(),
            std::mem::size_of::<bool>(),
        ])?)?;
        budget.charge_work(events.len())?;
        let mut edges = successors.len();
        if let Some(whole) = whole {
            budget.charge_work(successors.len())?;
            edges = successors.iter().filter(|&&target| !whole[target]).count();
        }
        for (index, event) in events.iter().enumerate() {
            if history_reset(*event, cell, blocks.len() + index, whole, budget)?.is_none() {
                edges = argument_sum_v1(&[edges, 1])?;
            }
        }
        let mut work = OriginWorkV1::new(nodes, edges, budget).map_err(origin_error)?;
        for block in 0..blocks.len() {
            work.seed_next(
                if whole.is_some_and(|whole| whole[block]) {
                    OriginStateV1::Exact(true)
                } else if block == entry {
                    OriginStateV1::Exact(false)
                } else {
                    OriginStateV1::Pending
                },
                budget,
            )
            .map_err(origin_error)?;
        }
        budget.charge_work(events.len())?;
        for (index, event) in events.iter().enumerate() {
            let seed = history_reset(*event, cell, blocks.len() + index, whole, budget)?
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
            let mut failure_previous = None;
            for event_index in block.events.clone() {
                let node = argument_sum_v1(&[blocks.len(), event_index])?;
                let event = events[event_index];
                let failure = matches!(
                    event.kind,
                    EventKind::FailureRead | EventKind::FailureKillSlot
                );
                let predecessor = if failure {
                    failure_previous.unwrap_or(previous)
                } else {
                    previous
                };
                if history_reset(event, cell, node, whole, budget)?.is_none() {
                    work.add_link(predecessor, node, budget)
                        .map_err(origin_error)?;
                }
                if failure {
                    failure_previous = Some(node);
                } else {
                    previous = node;
                }
            }
            for &target in &successors[block.successors.clone()] {
                if let Some(whole) = whole {
                    budget.charge_work(1)?;
                    if whole[target] {
                        continue;
                    }
                }
                work.add_link(previous, target, budget)
                    .map_err(origin_error)?;
            }
        }
        let states = work.solve(budget).map_err(origin_error)?;
        budget.charge_work(events.len())?;
        for (index, event) in events.iter().enumerate() {
            if event.cell == cell
                && matches!(event.kind, EventKind::Read | EventKind::FailureRead)
                && states[blocks.len() + index] != OriginStateV1::Exact(true)
            {
                return Err(invalid(
                    "scoped slot read is not initialized in its fresh physical activation",
                ));
            }
        }
        inspect(&states, budget)
    })
}

fn check_history(
    blocks: &[HistoryBlock],
    successors: &[usize],
    events: &[Event],
    cells: &[Cell],
    entry: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    let nodes = check_history_roster(blocks, successors, events, cells, entry, budget)?;
    for &cell in cells {
        with_history_cell(
            blocks,
            successors,
            events,
            cell,
            entry,
            nodes,
            None,
            budget,
            |_, _| Ok(()),
        )?;
    }
    Ok(())
}

// The expanded raw route supplies only independently source/physical-joined
// scalar accesses and original invalidation gaps. This reuses the same history
// equations as ordinary retained slots; it is not a source initialization seed.
pub(super) fn check_expanded_scalar_addresses_v29(
    function: &Function,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    if !graph.projections.is_empty() {
        return check_expanded_static_object_history_v29(
            function,
            graph,
            slots,
            accesses,
            kills,
            &[],
            budget,
        );
    }
    let body = function.body.as_ref().ok_or_else(scoped_slot_error_v29)?;
    let mut blocks = emission_vec_v1(graph.blocks.len(), budget)?;
    let mut cells = emission_vec_v1(slots.len(), budget)?;
    let capacity = argument_sum_v1(&[accesses.len(), kills.len()])?;
    let mut events = emission_vec_v1(capacity, budget)?;
    let mut edge_count = 0;
    for (_, block) in &graph.blocks {
        budget.charge_work(1)?;
        block
            .terminator
            .as_ref()
            .ok_or_else(scoped_slot_error_v29)?
            .try_visit_edges_v1(|_, arguments| {
                budget.charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                edge_count = argument_sum_v1(&[edge_count, 1])?;
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
    }
    let mut successors = emission_vec_v1(edge_count, budget)?;
    for (index, slot) in slots.iter().enumerate() {
        budget.charge_work(3)?;
        match slot.representation {
            ScopedSlotRepresentationV29::ScalarArray(scalar)
                if scalar.length == 1 && scalar.bytes == scalar.element.size => {}
            ScopedSlotRepresentationV29::Object { schema, .. }
                if graph
                    .object_layouts
                    .get(schema.0 as usize)
                    .is_some_and(|layout| {
                        matches!(
                            layout.value,
                            SourceStaticObjectValueV29::Scalar(_)
                                | SourceStaticObjectValueV29::Pointer(_)
                        )
                    }) => {}
            _ => {
                return Err(invalid(
                    "expanded scalar history needs the exact whole scalar cell",
                ));
            }
        }
        cells.push(Cell {
            slot: index,
            index: CellIndex::Literal(0),
        });
    }
    let (mut access, mut kill) = (0, 0);
    for (_, block) in &graph.blocks {
        let first = events.len();
        for gap in 0..=block.operations.len() {
            budget.charge_work(1)?;
            while let Some(row) = kills.get(kill)
                && (row.block, row.gap) == (block.id, gap)
            {
                budget.charge_work(1)?;
                events.push(Event {
                    cell: Cell {
                        slot: row.slot,
                        index: CellIndex::Literal(0),
                    },
                    kind: EventKind::KillSlot,
                    operation: gap,
                    sequence: events.len(),
                });
                kill += 1;
            }
            if let Some(row) = accesses.get(access)
                && (row.block, row.operation) == (block.id, gap)
            {
                budget.charge_work(1)?;
                let kind = match block.operations.get(gap).map(|operation| &operation.kind) {
                    Some(
                        OperationKind::Load { .. }
                        | OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. }),
                    ) => EventKind::Read,
                    Some(
                        OperationKind::Store { .. }
                        | OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. }),
                    ) => EventKind::Set(true),
                    _ => return Err(scoped_slot_error_v29()),
                };
                events.push(Event {
                    cell: Cell {
                        slot: row.slot,
                        index: CellIndex::Literal(0),
                    },
                    kind,
                    operation: gap,
                    sequence: events.len(),
                });
                access += 1;
            }
        }
        let edge = successors.len();
        block
            .terminator
            .as_ref()
            .ok_or_else(scoped_slot_error_v29)?
            .try_visit_edges_v1(|target, arguments| {
                budget.charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                successors.push(graph.block(target, budget)?);
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
        blocks.push(HistoryBlock {
            events: first..events.len(),
            successors: edge..successors.len(),
        });
    }
    if access != accesses.len()
        || kill != kills.len()
        || events.len() != capacity
        || successors.len() != edge_count
    {
        return Err(scoped_slot_error_v29());
    }
    let entry = graph.block(
        body.blocks.first().ok_or_else(scoped_slot_error_v29)?.id,
        budget,
    )?;
    let result = check_history(&blocks, &successors, &events, &cells, entry, budget);
    let owned = argument_sum_v1(&[
        argument_product_v1(blocks.capacity(), std::mem::size_of::<HistoryBlock>())?,
        argument_product_v1(successors.capacity(), std::mem::size_of::<usize>())?,
        argument_product_v1(events.capacity(), std::mem::size_of::<Event>())?,
        argument_product_v1(cells.capacity(), std::mem::size_of::<Cell>())?,
    ])?;
    drop((blocks, successors, events, cells));
    let cleanup = budget.release_storage(owned);
    match result {
        Ok(()) => cleanup.map_err(Into::into),
        Err(error) => {
            let _ = cleanup;
            Err(error)
        }
    }
}

pub(super) fn check_expanded_scalar_addresses_with_failures_v29(
    function: &Function,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    failures: &[SourceIndexFailureV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    if failures.is_empty() {
        check_expanded_scalar_addresses_v29(function, graph, slots, accesses, kills, budget)
    } else {
        check_expanded_static_object_history_v29(
            function, graph, slots, accesses, kills, failures, budget,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn check_sparse_history(
    blocks: &[HistoryBlock],
    successors: &[usize],
    events: &[Event],
    cells: &[Cell],
    entry: usize,
    extents: &[(usize, u64)],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    let nodes = check_history_roster(blocks, successors, events, cells, entry, budget)?;
    budget.charge_work(extents.len())?;
    if extents.windows(2).any(|rows| rows[0].0 >= rows[1].0) {
        return Err(invalid("scoped slot history extents are not unique"));
    }
    let mut first = 0;
    while first < cells.len() {
        budget.charge_work(1)?;
        let slot = cells[first].slot;
        let mut end = first;
        while end < cells.len() && cells[end].slot == slot {
            budget.charge_work(1)?;
            end = argument_sum_v1(&[end, 1])?;
        }
        let group = &cells[first..end];
        budget.charge_work(argument_sum_v1(&[
            group.len(),
            call_splice_search_work_v1(extents.len()),
        ])?)?;
        let extent = extents
            .binary_search_by_key(&slot, |row| row.0)
            .ok()
            .map(|index| extents[index].1)
            .filter(|&length| length != 0)
            .ok_or_else(|| invalid("scoped slot history has no exact extent"))?;
        let mut next = 0_u64;
        let mut complete = true;
        let mut extended = false;
        for cell in group {
            match cell.index {
                CellIndex::Literal(index) => {
                    if index >= extent {
                        return Err(invalid("scoped history literal exceeds its source extent"));
                    }
                    complete &= index == next;
                    next = index.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                CellIndex::Selected(_) | CellIndex::Whole => extended = true,
            }
        }
        complete &= next == extent;
        if !extended {
            for &cell in group {
                with_history_cell(
                    blocks,
                    successors,
                    events,
                    cell,
                    entry,
                    nodes,
                    None,
                    budget,
                    |_, _| Ok(()),
                )?;
            }
            first = end;
            continue;
        }
        with_canonical_call_scratch_v1(budget, |budget| {
            // Coverage is over existing history nodes, never the numeric extent.
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<Vec<bool>>(),
                std::mem::size_of::<Result<Vec<bool>, ProductionSemanticKirErrorV1>>(),
            ])?)?;
            let mut whole = emission_vec_v1(nodes, budget)?;
            budget.charge_work(nodes)?;
            whole.resize(nodes, complete);
            for &cell in group {
                budget.charge_work(1)?;
                if !matches!(cell.index, CellIndex::Literal(_)) {
                    continue;
                }
                with_history_cell(
                    blocks,
                    successors,
                    events,
                    cell,
                    entry,
                    nodes,
                    None,
                    budget,
                    |states, budget| {
                        if complete {
                            budget.charge_work(nodes)?;
                            for (covered, state) in whole.iter_mut().zip(states) {
                                *covered &= *state == OriginStateV1::Exact(true);
                            }
                        }
                        Ok(())
                    },
                )?;
            }
            for &cell in group {
                budget.charge_work(1)?;
                match cell.index {
                    CellIndex::Literal(_) => {}
                    CellIndex::Selected(_) => {
                        with_history_cell(
                            blocks,
                            successors,
                            events,
                            cell,
                            entry,
                            nodes,
                            Some(&whole),
                            budget,
                            |_, _| Ok(()),
                        )?;
                    }
                    CellIndex::Whole => {
                        budget.charge_work(events.len())?;
                        for (index, event) in events.iter().enumerate() {
                            if event.cell == cell
                                && matches!(event.kind, EventKind::Read | EventKind::FailureRead)
                                && !whole[blocks.len() + index]
                            {
                                return Err(invalid(
                                    "scoped whole-slot read lacks complete physical initialization",
                                ));
                            }
                        }
                    }
                }
            }
            Ok(())
        })?;
        first = end;
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
    check_with_source_selectors(function, graph, slots, first_slot, anchors, None, budget)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_with_source_selectors(
    function: &Function,
    graph: &SlotUseGraphV29<'_>,
    slots: &[ScopedSourceSlotV29],
    first_slot: usize,
    anchors: &[ScopedMemoryAnchorV29],
    source: Option<selectors::Source<'_, '_, '_, '_>>,
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
    let mut selected = source
        .map(|source| selectors::Context::new(source, function, budget))
        .transpose()?;
    let origins = cell_origins(
        function,
        graph,
        slots,
        first_slot,
        selected.as_mut(),
        budget,
    )?;
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
    let resets = selected
        .as_ref()
        .map(|selected| selected.resets())
        .unwrap_or(&[]);
    let capacity = argument_sum_v1(&[operations, anchors.len(), resets.len()])?;
    let mut events = emission_vec_v1(capacity, budget)?;
    let mut cells = emission_vec_v1(capacity, budget)?;
    let mut blocks = emission_vec_v1(graph.blocks.len(), budget)?;
    let mut successors = emission_vec_v1(edges, budget)?;
    let mut kills = emission_vec_v1(capacity - operations, budget)?;
    budget.charge_work(anchors.len())?;
    let mut sequence = 0_usize;
    for &(block, slot, key) in resets {
        budget.charge_work(1)?;
        kills.push((
            block,
            Event {
                cell: Cell {
                    slot,
                    index: CellIndex::Selected(key),
                },
                kind: EventKind::ForgetSelector,
                operation: 0,
                sequence,
            },
        ));
        sequence = argument_sum_v1(&[sequence, 1])?;
    }
    for row in anchors {
        let (local, read) = match row.kind {
            ScopedMemoryAnchorKindV29::Kill { local, .. } => (local, false),
            ScopedMemoryAnchorKindV29::FailureRead { local, .. } => (local, true),
            ScopedMemoryAnchorKindV29::Access { .. } => continue,
            ScopedMemoryAnchorKindV29::Object(_) => return Err(scoped_object_pending_v29()),
        };
        budget.charge_work(scoped_initialization_search_work_v29(slots.len()))?;
        let slot = match slots
            .binary_search_by_key(&ScopedAllocationIdentityV29::LegacyLocal(local), |slot| {
                slot.origin.identity
            }) {
            Ok(slot) => slot,
            Err(_) if read => continue,
            Err(_) => return Err(scoped_memory_error_v29()),
        };
        let failure = matches!(
            row.source.map(|frame| frame.role),
            Some(Some(ScopedMemoryRoleV29::Operand(
                ExecutionOperandV29::AssertMessage(_)
            )))
        );
        if read && !failure {
            return Err(scoped_memory_error_v29());
        }
        // A whole diagnostic is one coverage query, not a numeric-extent-sized
        // list of fabricated scalar accesses.
        budget.charge_work(1)?;
        kills.push((
            row.block,
            Event {
                cell: Cell {
                    slot: argument_sum_v1(&[first_slot, slot])?,
                    index: if read {
                        CellIndex::Whole
                    } else {
                        CellIndex::Literal(0)
                    },
                },
                kind: if read {
                    EventKind::FailureRead
                } else if failure {
                    EventKind::FailureKillSlot
                } else {
                    EventKind::KillSlot
                },
                operation: row.position,
                sequence,
            },
        ));
        sequence = argument_sum_v1(&[sequence, 1])?;
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
            let cell = checked_cell(
                pointer,
                access,
                block.id,
                operation,
                matches!(kind, EventKind::Set(_) | EventKind::Preserve),
                selected.as_ref(),
                graph,
                &origins,
                slots,
                first_slot,
                budget,
            )?;
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
    budget.charge_work(cells.len())?;
    if cells
        .iter()
        .any(|cell| !matches!(cell.index, CellIndex::Literal(_)))
    {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<(usize, u64)>>(),
            std::mem::size_of::<Result<Vec<(usize, u64)>, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut extents = emission_vec_v1(slots.len(), budget)?;
        for (index, slot) in slots.iter().enumerate() {
            budget.charge_work(1)?;
            extents.push((
                argument_sum_v1(&[first_slot, index])?,
                slot.scalar_array()?.length,
            ));
        }
        check_sparse_history(
            &blocks,
            &successors,
            &events,
            &cells,
            entry,
            &extents,
            budget,
        )
    } else {
        check_history(&blocks, &successors, &events, &cells, entry, budget)
    }
}
