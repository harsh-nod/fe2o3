// Final-inventory adapter for the existing sparse initialization solver. All
// ranges and pointer operands are checked before symbolic events are admitted.
include!("production_scoped_mixed_memory_history_v29.rs");

#[derive(Clone, Copy)]
struct IndexResetV29 {
    block: BlockId,
    gap: usize,
    cell: Cell,
}

fn index_history_header_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Vec<IndexResetV29>>(),
        std::mem::size_of::<Vec<Event>>(),
        std::mem::size_of::<Vec<Cell>>(),
        std::mem::size_of::<Vec<HistoryBlock>>(),
        std::mem::size_of::<Vec<usize>>(),
        std::mem::size_of::<Vec<(usize, u64)>>(),
        std::mem::size_of::<UseResult<()>>(),
    ])
}

fn index_pointer_cell_v29(
    pointer: ValueId,
    slot: usize,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    selected: &[SourceIndexLocationV29],
    memory: &scoped_index_memory_v29::SourceIndexMemoryV29<'_, '_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<Cell> {
    budget.charge_work(4)?;
    let source = slots.get(slot).ok_or_else(scoped_slot_error_v29)?;
    let scalar = source.scalar_array()?;
    if scalar.length == 0 || graph.exact(pointer, budget)? != Some(slot) {
        return Err(scoped_slot_error_v29());
    }
    budget.charge_work(call_splice_search_work_v1(selected.len()))?;
    if let Ok(index) = selected.binary_search_by_key(&pointer, |row| row.source.pointer) {
        let row = selected[index].source;
        if row.array_slot != slot
            || row.length != scalar.length
            || row.base != source.origin.pointer
        {
            return Err(scoped_slot_error_v29());
        }
        return Ok(Cell {
            slot,
            index: CellIndex::Selected(row.canonical),
        });
    }
    if pointer == source.origin.pointer || scalar.length == 1 {
        return Ok(Cell {
            slot,
            index: CellIndex::Literal(0),
        });
    }
    let operation = memory
        .definition(pointer, budget)?
        .ok_or_else(scoped_slot_error_v29)?;
    let OperationKind::GetElementPointer { base, offset } = operation.kind else {
        return Err(invalid(
            "ordinary array pointer needs its direct original element recipe",
        ));
    };
    if base != source.origin.pointer {
        return Err(scoped_slot_error_v29());
    }
    graph.check_gep_type(operation, budget)?;
    let index = memory
        .constant(offset, ScalarType::Index, budget)?
        .ok_or_else(scoped_slot_error_v29)?;
    if index >= scalar.length {
        return Err(scoped_slot_error_v29());
    }
    Ok(Cell {
        slot,
        index: CellIndex::Literal(index),
    })
}

fn check_expanded_array_history_v29<'view, 'inventory, 'graph>(
    function: &'graph Function,
    graph: &'view SourceAddressMemoryV29<'graph>,
    slots: &'view [ScopedSourceSlotV29],
    accesses: &'view [SourceAddressAccessV29],
    memory_accesses: &'view [SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    memory_kills: &[SourceAddressKillV29],
    lifetimes: &[SourceAddressLifetimeV29],
    failures: &[SourceIndexFailureV29],
    selected: &[SourceIndexLocationV29],
    guards: &[SourceIndexGuardLocationV29],
    inventory: &'inventory fe2o3_kernel_analysis::CanonicalKirInventoryV18<'graph>,
    versions: Option<&'view fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'inventory, 'graph>>,
    function_coordinate: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    if !selected.is_empty() && versions.is_none() {
        return Err(invalid(
            "selected array history requires shared memory versions",
        ));
    }
    let floor = budget.storage();
    budget.reserve_storage(index_history_header_v29()?)?;
    let mut memory = scoped_index_memory_v29::SourceIndexMemoryV29::with_optional_versions(
        inventory,
        versions,
        graph,
        function_coordinate,
        slots,
        memory_accesses,
        budget,
    )?;
    budget.charge_work(selected.len())?;
    if selected
        .windows(2)
        .any(|pair| pair[0].source.pointer >= pair[1].source.pointer)
    {
        return Err(scoped_slot_error_v29());
    }
    let mut resets = emission_vec_v1(selected.len(), budget)?;
    for selected in selected {
        memory.check_recipe(selected, guards, memory_kills, lifetimes, budget)?;
        let (block, gap) = memory.reevaluation(selected.source.original, budget)?;
        resets.push(IndexResetV29 {
            block,
            gap,
            cell: Cell {
                slot: selected.source.array_slot,
                index: CellIndex::Selected(selected.source.canonical),
            },
        });
    }
    call_splice_sort_work_v1(resets.len(), budget).map_err(source_address_call_error_v29)?;
    resets.sort_unstable_by_key(|row| (row.block, row.gap, row.cell));
    budget.charge_work(resets.len())?;
    resets.dedup_by_key(|row| (row.block, row.gap, row.cell));

    // Every array GEP, including unused ones, is a checked literal or an exact
    // source selector. A symbolic pointer cannot be saved or edge-transported
    // without a separate checked iteration relation.
    for (_, block) in &graph.blocks {
        for operation in &block.operations {
            budget.charge_work(1)?;
            if let OperationKind::GetElementPointer { base, .. } = operation.kind {
                if let Some(slot) = graph.exact(base, budget)? {
                    let [result] = operation.results.as_slice() else {
                        return Err(scoped_slot_error_v29());
                    };
                    if matches!(
                        slots[slot].representation,
                        ScopedSlotRepresentationV29::Object { .. }
                    ) {
                        // Typed subobject offsets use checked Project operations,
                        // never unproved scalar pointer arithmetic.
                        graph.check_zero_gep(operation, budget)?;
                    } else if slots[slot].scalar_array()?.length == 1 {
                        budget.charge_work(call_splice_search_work_v1(selected.len()))?;
                        let found = selected
                            .binary_search_by_key(&result.id, |row| row.source.pointer)
                            .is_ok();
                        if !found {
                            graph.check_zero_gep(operation, budget)?;
                        }
                    } else {
                        index_pointer_cell_v29(
                            result.id, slot, graph, slots, selected, &memory, budget,
                        )?;
                    }
                }
            }
            let mut operand = 0;
            operation.kind.try_visit_operands(|value| {
                budget.charge_work(1)?;
                let ordinal = operand;
                operand = argument_sum_v1(&[operand, 1])?;
                budget.charge_work(call_splice_search_work_v1(selected.len()))?;
                if selected
                    .binary_search_by_key(&value, |row| row.source.pointer)
                    .is_ok()
                    && !(ordinal == 0
                        && matches!(
                            operation.kind,
                            OperationKind::Load { .. } | OperationKind::Store { .. }
                        ))
                {
                    return Err(invalid(
                        "saved selected pointer requires exact iteration transport",
                    ));
                }
                Ok(())
            })?;
        }
        let terminator = block
            .terminator
            .as_ref()
            .ok_or_else(scoped_slot_error_v29)?;
        terminator.try_visit_edges_v1(|_, arguments| {
            for value in arguments {
                budget.charge_work(call_splice_search_work_v1(selected.len()))?;
                if selected
                    .binary_search_by_key(value, |row| row.source.pointer)
                    .is_ok()
                {
                    return Err(invalid(
                        "edge selected pointer requires exact iteration transport",
                    ));
                }
            }
            Ok::<_, ProductionSemanticKirErrorV1>(())
        })?;
    }

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
    budget.charge_work(failures.len())?;
    let failure_moves = failures.iter().filter(|row| row.move_after).count();
    let count = argument_sum_v1(&[
        accesses.len(),
        kills.len(),
        resets.len(),
        failures.len(),
        failure_moves,
    ])?;
    let mut events = emission_vec_v1(count, budget)?;
    let mut cells = emission_vec_v1(
        argument_sum_v1(&[accesses.len(), resets.len(), failures.len()])?,
        budget,
    )?;
    let mut blocks = emission_vec_v1(graph.blocks.len(), budget)?;
    let mut successors = emission_vec_v1(edge_count, budget)?;
    let mut extents = emission_vec_v1(slots.len(), budget)?;
    for (index, slot) in slots.iter().enumerate() {
        budget.charge_work(1)?;
        if let ScopedSlotRepresentationV29::ScalarArray(scalar) = slot.representation {
            extents.push((index, scalar.length));
        }
    }
    let (mut access, mut kill, mut reset, mut failure) = (0, 0, 0, 0);
    for (_, block) in &graph.blocks {
        let first = events.len();
        for gap in 0..=block.operations.len() {
            budget.charge_work(1)?;
            while let Some(row) = failures.get(failure)
                && (row.block, row.gap) == (block.id, gap)
            {
                budget.charge_work(1)?;
                if slots
                    .get(row.slot)
                    .ok_or_else(scoped_slot_error_v29)?
                    .scalar_array()?
                    .length
                    != 1
                {
                    return Err(scoped_slot_error_v29());
                }
                let cell = Cell {
                    slot: row.slot,
                    index: CellIndex::Literal(0),
                };
                events.push(Event {
                    cell,
                    kind: EventKind::FailureRead,
                    operation: gap,
                    sequence: events.len(),
                });
                if row.move_after {
                    budget.charge_work(1)?;
                    events.push(Event {
                        cell,
                        kind: EventKind::FailureKillSlot,
                        operation: gap,
                        sequence: events.len(),
                    });
                }
                cells.push(cell);
                failure += 1;
            }
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
            while let Some(row) = resets.get(reset)
                && (row.block, row.gap) == (block.id, gap)
            {
                budget.charge_work(1)?;
                events.push(Event {
                    cell: row.cell,
                    kind: EventKind::ForgetSelector,
                    operation: gap,
                    sequence: events.len(),
                });
                cells.push(row.cell);
                reset += 1;
            }
            if let Some(row) = accesses.get(access)
                && (row.block, row.operation) == (block.id, gap)
            {
                budget.charge_work(1)?;
                let (pointer, kind) = match block.operations.get(gap).map(|row| &row.kind) {
                    Some(OperationKind::Load { pointer, .. }) => (*pointer, EventKind::Read),
                    Some(OperationKind::Store { pointer, .. }) => (*pointer, EventKind::Set(true)),
                    _ => return Err(scoped_slot_error_v29()),
                };
                let cell = index_pointer_cell_v29(
                    pointer, row.slot, graph, slots, selected, &memory, budget,
                )?;
                events.push(Event {
                    cell,
                    kind,
                    operation: gap,
                    sequence: events.len(),
                });
                cells.push(cell);
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
        || reset != resets.len()
        || failure != failures.len()
        || events.len() != count
        || successors.len() != edge_count
    {
        return Err(scoped_slot_error_v29());
    }
    call_splice_sort_work_v1(cells.len(), budget).map_err(source_address_call_error_v29)?;
    cells.sort_unstable();
    budget.charge_work(cells.len())?;
    cells.dedup();
    let entry = graph.block(
        function
            .body
            .as_ref()
            .and_then(|body| body.blocks.first())
            .ok_or_else(scoped_slot_error_v29)?
            .id,
        budget,
    )?;
    check_sparse_history(
        &blocks,
        &successors,
        &events,
        &cells,
        entry,
        &extents,
        budget,
    )?;
    drop((memory, resets, events, cells, blocks, successors, extents));
    let owned = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.release_storage(owned)?;
    Ok(())
}

#[cfg(test)]
mod index_history_header_tests_v29 {
    use super::*;

    #[test]
    fn index_history_header_accounts_each_owned_vector_and_result() {
        let expected = std::mem::size_of::<Vec<IndexResetV29>>()
            + std::mem::size_of::<Vec<Event>>()
            + std::mem::size_of::<Vec<Cell>>()
            + std::mem::size_of::<Vec<HistoryBlock>>()
            + std::mem::size_of::<Vec<usize>>()
            + std::mem::size_of::<Vec<(usize, u64)>>()
            + std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
        assert_eq!(index_history_header_v29().unwrap(), expected);
    }
}
