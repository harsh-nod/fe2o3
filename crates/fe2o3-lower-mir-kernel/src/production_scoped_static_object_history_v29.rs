// Partition only at observed access boundaries, never once per byte or once per
// declared array element. Existing all-path history equations check each segment.
include!("production_source_tag_history_v43.rs");

type SourceScalarHistoryOrderFrameV45<'a> = (
    Option<[usize; 7]>,
    [usize; 7],
    Option<&'a SourceAddressKillV29>,
    Option<&'a SourceIndexFailureV29>,
    &'a SourceIndexFailureV29,
    &'a std::ops::Range<usize>,
    [usize; 5],
);
fn check_expanded_static_object_history_v29(
    function: &Function,
    graph: &SourceAddressMemoryV29<'_>,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    failures: &[SourceIndexFailureV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> UseResult<()> {
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(std::mem::size_of::<(
            Vec<SourceAddressHistoryFootprintV43>,
            Vec<Cell>,
            Vec<std::ops::Range<usize>>,
            Vec<Event>,
            Vec<HistoryBlock>,
            Vec<usize>,
        )>())?;
        budget.reserve_storage(source_reference_emission_headers_v29::<
            SourceAddressHistoryQueryFrameV43<'_>,
        >()?)?;
        budget.reserve_storage(source_reference_emission_headers_v29::<
            SourceScalarHistoryOrderFrameV45<'_>,
        >()?)?;
        let footprint_count = argument_sum_v1(&[accesses.len(), failures.len()])?;
        let mut footprints = emission_vec_v1(footprint_count, budget)?;
        let mut cells = emission_vec_v1(
            argument_sum_v1(&[slots.len(), argument_product_v1(footprint_count, 2)?])?,
            budget,
        )?;
        for (slot, row) in slots.iter().enumerate() {
            budget.charge_work(1)?;
            if matches!(row.representation, ScopedSlotRepresentationV29::ScalarArray(scalar)
                if scalar.length != 1)
            {
                continue;
            }
            cells.push(Cell {
                slot,
                index: CellIndex::Literal(0),
            });
        }
        for row in accesses {
            budget.charge_work(3)?;
            let footprint = source_address_history_footprint_v43(graph, row, slots, budget)?;
            footprints.push(footprint);
            let Some((start, end)) = footprint.1 else {
                continue;
            };
            cells.push(Cell {
                slot: row.slot,
                index: CellIndex::Literal(start),
            });
            cells.push(Cell {
                slot: row.slot,
                index: CellIndex::Literal(end),
            });
        }
        let mut previous_range = None;
        for row in failures {
            budget.charge_work(13)?;
            let key = row.key_v45();
            if previous_range.is_some_and(|previous| previous >= key) {
                return Err(scoped_slot_error_v29());
            }
            previous_range = Some(key);
            let slot = slots.get(row.slot).ok_or_else(scoped_slot_error_v29)?;
            if row.range.start >= row.range.end || row.range.end > slot.representation.bytes() {
                return Err(scoped_slot_error_v29());
            }
            match slot.representation {
                ScopedSlotRepresentationV29::ScalarArray(scalar)
                    if scalar.length == 1
                        && scalar.bytes == scalar.element.size
                        && row.range.start == 0
                        && row.range.end == scalar.bytes
                        && matches!(
                            scalar.element.element,
                            PrivateRetainedElementFactsV1::Scalar(_)
                        ) => {}
                ScopedSlotRepresentationV29::Object { schema, bytes, .. }
                    if graph
                        .object_layouts
                        .get(schema.0 as usize)
                        .is_some_and(|layout| layout.bytes == bytes) => {}
                _ => return Err(scoped_slot_error_v29()),
            }
            footprints.push((row.slot, Some((row.range.start, row.range.end)), false));
            cells.push(Cell {
                slot: row.slot,
                index: CellIndex::Literal(row.range.start),
            });
            cells.push(Cell {
                slot: row.slot,
                index: CellIndex::Literal(row.range.end),
            });
        }
        call_splice_sort_work_v1(cells.len(), budget).map_err(source_address_call_error_v29)?;
        cells.sort_unstable();
        budget.charge_work(cells.len())?;
        cells.dedup();
        let mut ranges = emission_vec_v1(footprints.len(), budget)?;
        let mut event_count = kills.len();
        for (ordinal, &(slot, range, _)) in footprints.iter().enumerate() {
            let Some((start, end)) = range else {
                budget.charge_work(1)?;
                ranges.push(0..0);
                continue;
            };
            budget.charge_work(argument_product_v1(
                2,
                call_splice_search_work_v1(cells.len()),
            )?)?;
            let first = cells
                .binary_search(&Cell {
                    slot,
                    index: CellIndex::Literal(start),
                })
                .map_err(|_| scoped_slot_error_v29())?;
            let limit = cells
                .binary_search(&Cell {
                    slot,
                    index: CellIndex::Literal(end),
                })
                .map_err(|_| scoped_slot_error_v29())?;
            if first >= limit {
                return Err(scoped_slot_error_v29());
            }
            let repetitions = if ordinal < accesses.len() {
                1
            } else {
                1 + usize::from(failures[ordinal - accesses.len()].move_after)
            };
            event_count = argument_sum_v1(&[
                event_count,
                argument_product_v1(limit - first, repetitions)?,
            ])?;
            ranges.push(first..limit);
        }
        let mut events = emission_vec_v1(event_count, budget)?;
        let mut blocks = emission_vec_v1(graph.blocks.len(), budget)?;
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
        let (mut access, mut kill, mut failure) = (0, 0, 0);
        for (_, block) in &graph.blocks {
            let first = events.len();
            for gap in 0..=block.operations.len() {
                budget.charge_work(1)?;
                loop {
                    let next_kill = kills
                        .get(kill)
                        .filter(|row| (row.block, row.gap) == (block.id, gap));
                    let next_range = failures
                        .get(failure)
                        .filter(|row| (row.block, row.gap) == (block.id, gap));
                    if next_kill.is_none() && next_range.is_none() {
                        break;
                    }
                    budget.charge_work(7)?;
                    if let (Some(kill), Some(range)) = (next_kill, next_range)
                        && kill.source_order == range.source_order
                    {
                        return Err(scoped_slot_error_v29());
                    }
                    if let Some(row) = next_kill
                        && next_range.is_none_or(|range| row.source_order < range.source_order)
                    {
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
                        continue;
                    }
                    let row = next_range.ok_or_else(scoped_slot_error_v29)?;
                    let range = ranges
                        .get(argument_sum_v1(&[accesses.len(), failure])?)
                        .ok_or_else(scoped_slot_error_v29)?;
                    for &cell in cells.get(range.clone()).ok_or_else(scoped_slot_error_v29)? {
                        budget.charge_work(2 + usize::from(row.move_after))?;
                        events.push(Event {
                            cell,
                            kind: if row.failure_only {
                                EventKind::FailureRead
                            } else {
                                EventKind::Read
                            },
                            operation: gap,
                            sequence: events.len(),
                        });
                        if row.move_after {
                            events.push(Event {
                                cell,
                                kind: if row.failure_only {
                                    EventKind::FailureKillCell
                                } else {
                                    EventKind::Set(false)
                                },
                                operation: gap,
                                sequence: events.len(),
                            });
                        }
                    }
                    failure += 1;
                }
                if let Some(row) = accesses.get(access)
                    && (row.block, row.operation) == (block.id, gap)
                {
                    let range = ranges.get(access).ok_or_else(scoped_slot_error_v29)?;
                    for &cell in cells.get(range.clone()).ok_or_else(scoped_slot_error_v29)? {
                        budget.charge_work(1)?;
                        events.push(Event {
                            cell,
                            kind: if footprints[access].2 {
                                EventKind::Set(true)
                            } else {
                                EventKind::Read
                            },
                            operation: gap,
                            sequence: events.len(),
                        });
                    }
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
            || failure != failures.len()
            || events.len() != event_count
            || successors.len() != edge_count
        {
            return Err(scoped_slot_error_v29());
        }
        let entry = function
            .body
            .as_ref()
            .and_then(|body| body.blocks.first())
            .ok_or_else(scoped_slot_error_v29)?
            .id;
        check_history(
            &blocks,
            &successors,
            &events,
            &cells,
            graph.block(entry, budget)?,
            budget,
        )
    })
}
