// Expected frames are enumerated independently of the implementation helper.
pub(in super::super) fn header_oracle_v30() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    type ReplayFields<'a> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        usize,
        &'a mut ArgumentBudgetV1<'a>,
    );
    assert_eq!(
        size_of::<SelectedReplayFrameV30<'_, '_, '_, '_>>(),
        size_of::<ReplayFields<'_>>()
    );
    h::<ReplayFields<'_>>()
        + 2 * size_of::<SourceOwnedResultV18<()>>()
        + scoped_raw_admission_v29::selected_source_rows_frame_oracle_v30()
        + h::<Writer<'_>>()
        + h::<Vec<SelectedTransportRowV30>>()
        + h::<Vec<std::ops::Range<usize>>>()
        + h::<(Vec<SelectedTransportRowV30>, Vec<std::ops::Range<usize>>)>()
        + h::<SelectedTransportRowV30>()
        + 6 * h::<SelectedDefinitionV30>()
        + h::<SelectedEdgeV30>()
        + h::<Option<Definition>>()
        + h::<&[PendingSourceSelectedAccessV30]>()
        + h::<&PendingSourceSelectedAccessV30>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()
        + h::<SourceAddressValueAccessV29>()
        + h::<Option<SourceAddressValueAccessV29>>()
        + h::<Option<(Definition, Definition)>>()
        + h::<BlockControl>()
        + h::<EdgeControl>()
        + h::<Option<OutputUse>>()
        + h::<[Option<ValueId>; 7]>()
        + h::<std::ops::Range<usize>>()
        + 6 * h::<Definition>()
        + h::<EdgeArgument>()
        + h::<Edge>()
        + h::<OpCoordinate>()
        + h::<ProductionOptimizedSourceOperationV18>()
        + h::<Option<(Block, usize, BlockControl)>>()
        + h::<usize>()
        + h::<()>()
}

pub(in super::super) fn retained_rows_oracle_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
) -> usize {
    let mut count = 0usize;
    for owner in &original.source.owner.inner.pending.roots {
        let Some(pending) = &owner.source_slots.pending_memory else {
            continue;
        };
        // Test-only original row access remains in its owning module.
        count += scoped_raw_admission_v29::selected_row_count_oracle_v30(pending);
    }
    count * size_of::<SelectedTransportRowV30>()
}

pub(in super::super) fn assert_transport_v30(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    expected_accesses: usize,
    identity: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, usize, usize)> {
    view.replay_selected_transport_v30(0, budget)?;
    assert_eq!(headers().unwrap(), header_oracle_v30());
    assert_eq!(
        view.index.selected.capacity() * size_of::<SelectedTransportRowV30>(),
        retained_rows_oracle_v30(view.original)
    );
    let mut accesses = 0;
    let mut incoming = 0;
    let mut invocations = 0;
    let mut leaves = 0;
    let mut guards = 0;
    let mut obligations = 0;
    let mut node_definitions = 0;
    for row in &view.index.selected {
        match row {
            SelectedTransportRowV30::Access {
                output_pointer,
                output_value,
                disposition,
                ..
            } => {
                accesses += 1;
                if identity {
                    assert!(
                        matches!(disposition, ProductionOptimizedSourceOperationV18::Retained { input, output } if input == output)
                    );
                    assert!(output_pointer.is_some() && output_value.is_some());
                }
            }
            SelectedTransportRowV30::Definition { role, relation } => {
                if matches!(role, SelectedDefinitionRoleV30::Node(_)) {
                    node_definitions += 1;
                }
                if identity {
                    let range = relation.outputs;
                    assert_eq!(range.len, 1);
                    assert_eq!(
                        view.checked.rows().definition_outputs[range.start as usize].output,
                        relation.input
                    );
                }
            }
            SelectedTransportRowV30::Incoming { relation, .. } => {
                incoming += 1;
                if identity {
                    assert_eq!(relation.output, Some(relation.input));
                }
            }
            SelectedTransportRowV30::Invocation { relation, .. } => {
                invocations += 1;
                if identity {
                    assert_eq!(relation.output, Some(relation.input));
                }
            }
            SelectedTransportRowV30::Leaf { .. } => leaves += 1,
            SelectedTransportRowV30::Guard { .. } => guards += 1,
            SelectedTransportRowV30::Obligation { .. } => obligations += 1,
            SelectedTransportRowV30::Node { .. } => (),
        }
    }
    assert_eq!(accesses, expected_accesses);
    if expected_accesses != 0 {
        assert!(node_definitions >= leaves && leaves >= expected_accesses * 2);
        assert!(guards >= leaves && obligations >= leaves && incoming > 0);
    } else {
        assert!(view.index.selected.is_empty());
    }
    Ok((incoming, invocations, obligations))
}

pub(in super::super) fn corrupt_transport_v30(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fault: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    view.replay_selected_transport_v30(0, budget)?;
    let floor = budget.storage();
    let mut copied = resources::vector(view.index.selected.len(), budget)?;
    budget.charge_work(view.index.selected.len() * size_of::<SelectedTransportRowV30>())?;
    copied.extend_from_slice(&view.index.selected);
    match fault {
        0 => {
            copied.pop().unwrap();
        }
        1 => {
            copied.swap(1, 2);
        }
        2 => {
            let row = copied
                .iter_mut()
                .find_map(|row| match row {
                    SelectedTransportRowV30::Incoming { relation, .. } => Some(relation),
                    _ => None,
                })
                .unwrap();
            row.input.edge.successor = row.input.edge.successor.wrapping_add(1);
        }
        3 => {
            let row = copied
                .iter_mut()
                .find_map(|row| match row {
                    SelectedTransportRowV30::Definition { relation, .. } => Some(relation),
                    _ => None,
                })
                .unwrap();
            row.outputs.len = 0;
        }
        4 => {
            let row = copied
                .iter_mut()
                .find_map(|row| match row {
                    SelectedTransportRowV30::Guard { control, .. } => Some(control),
                    _ => None,
                })
                .unwrap();
            row.executable = !row.executable;
        }
        5 => {
            let row = copied
                .iter_mut()
                .find_map(|row| match row {
                    SelectedTransportRowV30::Obligation { original, .. } => Some(original),
                    _ => None,
                })
                .unwrap();
            row.at_access = !row.at_access;
        }
        6 => {
            let row = copied
                .iter_mut()
                .find_map(|row| match row {
                    SelectedTransportRowV30::Leaf { original, .. } => Some(original),
                    _ => None,
                })
                .unwrap();
            row.node = usize::MAX;
        }
        7 => {
            let row = copied
                .iter_mut()
                .find_map(|row| match row {
                    SelectedTransportRowV30::Incoming { relation, .. } => Some(relation),
                    _ => None,
                })
                .unwrap();
            row.output = None;
        }
        _ => panic!("unknown selected transport fault"),
    }
    let result = view.original.retain_query(visit(
        view.original,
        view.checked,
        view.control,
        view.index,
        0..1,
        &mut Writer::Replay {
            rows: &copied,
            next: 0,
        },
        budget,
    ));
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized transport differs from its complete original relation"
        ))
    ));
    let retained = copied.capacity() * size_of::<SelectedTransportRowV30>();
    drop(copied);
    budget.release_storage(retained)?;
    assert_eq!(budget.storage(), floor);
    let before = budget.work();
    assert!(matches!(
        view.replay_selected_transport_v30(0, budget),
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "selected optimized transport differs from its complete original relation"
        ))
    ));
    assert_eq!(
        budget.work(),
        before,
        "first refusal is sticky before another replay"
    );
    result
}

#[test]
fn selected_optimized_transport_fixed_frames_match_independent_exact_and_short_oracles() {
    let expected = header_oracle_v30();
    assert_eq!(headers().unwrap(), expected);
    for short in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 17 + expected - short);
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(headers().unwrap());
        if short == 0 {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
            if error.actual() == 17 + expected && error.limit() == 16 + expected));
        }
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn selected_optimized_ordered_row_replay_has_linear_exact_work_and_no_scratch_growth() {
    let row = SelectedTransportRowV30::Definition {
        role: SelectedDefinitionRoleV30::AccessPointer,
        relation: SelectedDefinitionV30 {
            input: Definition::FunctionArgument {
                function: FunctionCoordinate(0),
                argument: 0,
            },
            outputs: CanonicalKirTransitionRangeV1 { start: 0, len: 1 },
        },
    };
    for count in [1usize, 64, 256, 1024] {
        let rows = vec![row; count];
        let expected = count * size_of::<SelectedTransportRowV30>() + 1;
        for short in [0, 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(expected - short);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            let mut writer = Writer::Replay {
                rows: &rows,
                next: 0,
            };
            for row in &rows {
                writer.row(*row, &mut budget).unwrap();
            }
            let result = writer.finish(&mut budget);
            if short == 0 {
                result.unwrap();
            } else {
                assert!(
                    matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(error)))
                if error.actual() == expected && error.limit() == expected - 1)
                );
            }
            assert_eq!(budget.work(), expected - short);
            assert_eq!(budget.storage(), 17);
        }
    }
}
