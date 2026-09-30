// Partition only at observed access boundaries, never once per byte or once per
// declared array element. Existing all-path history equations check each segment.
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
            Vec<(usize, u64, u64, bool)>,
            Vec<Cell>,
            Vec<std::ops::Range<usize>>,
            Vec<Event>,
            Vec<HistoryBlock>,
            Vec<usize>,
        )>())?;
        let mut footprints = emission_vec_v1(accesses.len(), budget)?;
        let mut cells = emission_vec_v1(
            argument_sum_v1(&[slots.len(), argument_product_v1(accesses.len(), 2)?])?,
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
            let operation = graph.blocks[graph.block(row.block, budget)?]
                .1
                .operations
                .get(row.operation)
                .ok_or_else(scoped_slot_error_v29)?;
            let access =
                source_address_value_access_v29(operation)?.ok_or_else(scoped_slot_error_v29)?;
            let slot = slots.get(row.slot).ok_or_else(scoped_slot_error_v29)?;
            let (start, end) = match slot.representation {
                ScopedSlotRepresentationV29::Object { .. } => {
                    if graph.object_location(access.pointer, budget)?.slot != row.slot {
                        return Err(scoped_slot_error_v29());
                    }
                    graph.object_value_range(
                        access.pointer,
                        slot,
                        graph.ty(access.value, budget)?,
                        access.access,
                        budget,
                    )?
                }
                ScopedSlotRepresentationV29::ScalarArray(scalar)
                    if scalar.length == 1 && scalar.bytes == scalar.element.size =>
                {
                    (0, scalar.bytes)
                }
                _ => return Err(invalid("static object history needs exact access ranges")),
            };
            if start >= end {
                return Err(invalid("static value access has an empty range"));
            }
            footprints.push((row.slot, start, end, access.writing));
            cells.push(Cell {
                slot: row.slot,
                index: CellIndex::Literal(start),
            });
            cells.push(Cell {
                slot: row.slot,
                index: CellIndex::Literal(end),
            });
        }
        call_splice_sort_work_v1(cells.len(), budget).map_err(source_address_call_error_v29)?;
        cells.sort_unstable();
        budget.charge_work(cells.len())?;
        cells.dedup();
        let mut ranges = emission_vec_v1(footprints.len(), budget)?;
        let mut event_count = argument_sum_v1(&[kills.len(), failures.len()])?;
        for row in failures {
            budget.charge_work(3)?;
            let slot = slots.get(row.slot).ok_or_else(scoped_slot_error_v29)?;
            let scalar = match slot.representation {
                ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                    scalar.length == 1
                        && scalar.bytes == scalar.element.size
                        && matches!(
                            scalar.element.element,
                            PrivateRetainedElementFactsV1::Scalar(_)
                        )
                }
                ScopedSlotRepresentationV29::Object { schema, .. } => graph
                    .object_layouts
                    .get(schema.0 as usize)
                    .is_some_and(|layout| {
                        matches!(layout.value, SourceStaticObjectValueV29::Scalar(_))
                    }),
            };
            if !scalar {
                return Err(invalid(
                    "failure history requires an exact whole scalar diagnostic",
                ));
            }
            event_count = argument_sum_v1(&[event_count, usize::from(row.move_after)])?;
        }
        for &(slot, start, end, _) in &footprints {
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
            event_count = argument_sum_v1(&[event_count, limit - first])?;
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
                while let Some(row) = failures.get(failure)
                    && (row.block, row.gap) == (block.id, gap)
                {
                    budget.charge_work(2 + usize::from(row.move_after))?;
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
                        events.push(Event {
                            cell,
                            kind: EventKind::FailureKillSlot,
                            operation: gap,
                            sequence: events.len(),
                        });
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
                            kind: if footprints[access].3 {
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
