use scoped_root_tests::fixtures::{FreshReturnCaseV1, fresh_tile_return_owner_v1};

mod no_normal_abi_tests {
    use super::*;
    include!("production_execution_no_return_abi_v1_tests.rs");
}

mod completion_tests {
    use super::*;
    include!("production_execution_return_completion_v1_tests.rs");
}

mod continuation_tests {
    use super::*;
    include!("production_call_continuation_v1_tests.rs");
}

thread_local! {
    static FRESH_SOURCE_OBSERVED_V1: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}

fn observe_actual_fresh_returns_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut identities = Vec::new();
    let mut exits = 0;
    for (ordinal, original) in instances.instances().iter().enumerate() {
        if original.function().index() < 4 {
            continue;
        }
        let id = instances.id_at(ordinal).unwrap();
        assert_eq!(instances.instance_reachable(id), Some(true));
        let lowered = emitted[ordinal].as_ref().unwrap();
        assert_eq!(lowered.source_call_instance, Some(id));
        let archive = lowered.execution_observation.as_ref().unwrap();
        let incoming = instances.incoming(id).unwrap();
        let caller_id = incoming.occurrence().caller;
        let caller = emitted[caller_id.index()].as_ref().unwrap();
        let call_destination = incoming.source().destination().unwrap();
        let caller_edge =
            SsaEdgeIdV1::new(SsaBlockIdV1::new(incoming.occurrence().block.index()), 0);
        let caller_definitions = instances
            .instance(caller_id)
            .unwrap()
            .ssa()
            .plan()
            .edge_definitions(caller_edge)
            .unwrap();
        let definition = caller_definitions
            .iter()
            .find(|row| row.variable().get() == call_destination.place().local().index())
            .unwrap();
        let installed = caller
            .execution_observation
            .as_ref()
            .unwrap()
            .lookup_original_v29(instances, caller_id, definition.value(), budget)?;
        let installed = one_nominal_leaf(installed);
        assert_eq!(
            installed.role,
            SemanticExecutionRoleV29::LaneFragmentU32 {
                lanes: 64,
                elements: 2
            }
        );
        assert!(installed.workgroup.is_some());
        assert!(
            instances
                .instance(installed.identity.producer.caller)
                .unwrap()
                .function()
                .index()
                >= 4
        );
        assert_ne!(installed.identity.producer.caller, caller_id);
        assert!(lowered.next_value <= caller.next_value);
        let occurrences = instances.occurrences(id).unwrap();
        let mut observations = Vec::new();
        for event in occurrences.events().iter().filter(|event| {
            event.operand() == ExecutionOperandV29::ReturnValue
                && event.role() == ExecutionEventV29::BaseUse
        }) {
            let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
                panic!("actual ReturnValue use")
            };
            assert_eq!(variable.get(), 0);
            assert!(event.is_promoted() && event.is_reachable());
            let returned = archive.lookup_original_v29(instances, id, value, budget)?;
            assert_eq!(one_nominal_leaf(returned), installed);
            let ExecutionSiteV29::Terminator { block } = event.site() else {
                panic!("return terminator")
            };
            let source = SemanticBlockIdV1::from_index(block.get());
            assert_eq!(instances.block_reachable(id, source), Some(true));
            let mapping = lowered
                .blocks
                .iter()
                .find(|row| row.semantic_block == source)
                .unwrap();
            let physical = lowered
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|row| row.id == mapping.kernel_ir_block)
                .unwrap();
            let Some(Terminator::Return { values }) = &physical.terminator else {
                panic!("physical Return")
            };
            let direct = execution_cfg_nominal_kind_v29(
                instances.owner().source_semantic().types(),
                original.declaration().abi().source_output_type(),
            )
            .unwrap()
            .is_some();
            assert_eq!(
                values.len(),
                usize::from(!direct),
                "fresh identity is not a physical ABI scalar"
            );
            observations.push(ExecutionIdentityReturnSlotV1 {
                exit: ExecutionIdentityReturnExitV1 {
                    block: source,
                    local: SemanticLocalIdV1::from_index(0),
                },
                observation: Some(ExecutionIdentityReturnObservationV1 {
                    source,
                    target: mapping.kernel_ir_block,
                    values: values.clone(),
                    nominal: vec![Some(ExecutionCfgLeafV29::Owned(installed.clone()))],
                }),
            });
        }
        assert!(!observations.is_empty());
        observations.sort_unstable_by_key(|row| row.exit.block.index());
        exits += observations.len();
        let mut witness = ExecutionIdentityReturnWitnessV1 {
            expected: vec![Some(ExecutionCfgLeafV29::Owned(installed.clone()))],
            observations,
        };
        let floor = budget.storage();
        witness.finish(id, original.function(), lowered, instances, budget)?;
        assert_eq!(budget.storage(), floor);
        let last = witness.observations.pop().unwrap();
        assert!(
            witness
                .finish(id, original.function(), lowered, instances, budget)
                .is_err()
        );
        witness.observations.push(last);
        let saved = witness.observations[0].exit.block;
        witness.observations[0].exit.block = SemanticBlockIdV1::from_index(u32::MAX);
        assert!(
            witness
                .finish(id, original.function(), lowered, instances, budget)
                .is_err()
        );
        witness.observations[0].exit.block = saved;
        assert!(
            witness
                .finish(caller_id, original.function(), lowered, instances, budget)
                .is_err()
        );
        witness.finish(id, original.function(), lowered, instances, budget)?;
        assert_eq!(budget.storage(), floor);
        identities.push(installed.identity);
    }
    assert!(identities.len() >= 2);
    let unique: std::collections::BTreeSet<_> = identities
        .iter()
        .map(|identity| {
            (
                identity.producer.caller.index(),
                identity.producer.block.index(),
                identity.value.0,
            )
        })
        .collect();
    assert_eq!(
        unique.len(),
        2,
        "the two original repeated calls must own distinct actual producers"
    );
    FRESH_SOURCE_OBSERVED_V1.set((identities.len(), exits));
    Ok(())
}

#[test]
fn actual_fresh_fragment_returns_cover_repeated_branch_loop_nested_and_mixed_source() {
    for case in [
        FreshReturnCaseV1::Straight,
        FreshReturnCaseV1::TwoExits,
        FreshReturnCaseV1::Loop,
        FreshReturnCaseV1::Nested,
        FreshReturnCaseV1::Mixed,
    ] {
        FRESH_SOURCE_OBSERVED_V1.set((0, 0));
        let (result, _, _) = run_profiled_suffix_owner(
            || fresh_tile_return_owner_v1(case),
            observe_actual_fresh_returns_v1,
            LIMIT,
            LIMIT,
            |output, _, _| {
                let coordinates = &output.pending.coordinates;
                assert_eq!(
                    coordinates.sources.rows.len(),
                    if case == FreshReturnCaseV1::Nested {
                        9
                    } else {
                        7
                    }
                );
                assert_eq!(coordinates.seeds.rows.len(), coordinates.sources.rows.len());
                assert_eq!(
                    output.pending.sidecars.rows.len(),
                    coordinates.sources.rows.len()
                );
                assert!(coordinates.anchors.rows.iter().all(|row| row.removed));
                assert!(
                    coordinates
                        .returns
                        .rows
                        .iter()
                        .filter(|row| row.source.semantic_function.index() >= 4)
                        .count()
                        >= 2
                );
                for (ordinal, row) in output.pending.sidecars.rows.iter().enumerate() {
                    assert_eq!(
                        row.source_call_instance,
                        Some(coordinates.sources.rows[ordinal].instance)
                    );
                    assert!(row.execution_observation.is_some());
                }
                Ok(())
            },
        );
        assert!(result.is_ok(), "{case:?}: {result:?}");
        let (helpers, exits) = FRESH_SOURCE_OBSERVED_V1.get();
        assert_eq!(
            helpers,
            if case == FreshReturnCaseV1::Nested {
                4
            } else {
                2
            }
        );
        assert_eq!(
            exits,
            if matches!(
                case,
                FreshReturnCaseV1::Nested | FreshReturnCaseV1::TwoExits
            ) {
                4
            } else {
                2
            }
        );
    }
}

fn observe_fresh_reborrow_returns_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut borrows = Vec::new();
    for (ordinal, original) in instances.instances().iter().enumerate() {
        if original.function().index() != 4 {
            continue;
        }
        let id = instances.id_at(ordinal).unwrap();
        let output = emitted[ordinal].as_ref().unwrap();
        let incoming = instances.incoming(id).unwrap();
        let caller_id = incoming.occurrence().caller;
        let caller = emitted[caller_id.index()].as_ref().unwrap();
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(incoming.occurrence().block.index()), 0);
        let definitions = instances
            .instance(caller_id)
            .unwrap()
            .ssa()
            .plan()
            .edge_definitions(edge)
            .unwrap();
        let [definition] = definitions else {
            panic!("one returned reference definition")
        };
        let binding = caller
            .execution_observation
            .as_ref()
            .unwrap()
            .lookup_original_v29(instances, caller_id, definition.value(), budget)?;
        let SemanticValueBindingV1::ExecutionBorrow(binding) = binding else {
            panic!("actual returned nominal reborrow")
        };
        assert_eq!(binding.occurrence.instance, id);
        assert_eq!(binding.occurrence.block.index(), 0);
        assert_eq!(binding.occurrence.statement, 0);
        assert_eq!(binding.destination_local.index(), 0);
        assert_eq!(binding.source_local.index(), 2);
        assert_eq!(binding.kind, SemanticBorrowKindV1::Shared);
        assert_eq!(binding.borrowed.role, SemanticExecutionRoleV29::Workgroup);
        assert_ne!(binding.borrowed.identity.producer.caller, id);
        assert!(binding.parent.is_some());
        let occurrences = instances.occurrences(id).unwrap();
        let returns: Vec<_> = occurrences
            .events()
            .iter()
            .filter(|row| {
                row.operand() == ExecutionOperandV29::ReturnValue
                    && row.role() == ExecutionEventV29::BaseUse
            })
            .collect();
        let [returned] = returns.as_slice() else {
            panic!("one original return source use")
        };
        let Some(SsaResolvedEventV1::Use { variable, value }) = returned.resolved() else {
            panic!("resolved return")
        };
        assert_eq!(variable.get(), 0);
        assert!(returned.is_promoted() && returned.is_reachable());
        let archived = output
            .execution_observation
            .as_ref()
            .unwrap()
            .lookup_original_v29(instances, id, value, budget)?;
        assert!(
            matches!(archived, SemanticValueBindingV1::ExecutionBorrow(actual) if actual == binding)
        );
        let mapping = &output.blocks[0];
        assert_eq!(mapping.semantic_block.index(), 0);
        let physical = output
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|row| row.id == mapping.kernel_ir_block)
            .unwrap();
        let Some(Terminator::Return { values }) = &physical.terminator else {
            panic!("actual return")
        };
        assert!(
            values.is_empty(),
            "nominal reborrows have no physical scalar ABI output"
        );
        let witness = ExecutionIdentityReturnWitnessV1 {
            expected: vec![Some(ExecutionCfgLeafV29::Borrow(binding.clone()))],
            observations: vec![ExecutionIdentityReturnSlotV1 {
                exit: ExecutionIdentityReturnExitV1 {
                    block: mapping.semantic_block,
                    local: SemanticLocalIdV1::from_index(0),
                },
                observation: Some(ExecutionIdentityReturnObservationV1 {
                    source: mapping.semantic_block,
                    target: mapping.kernel_ir_block,
                    values: vec![],
                    nominal: vec![Some(ExecutionCfgLeafV29::Borrow(binding.clone()))],
                }),
            }],
        };
        witness.finish(id, original.function(), output, instances, budget)?;
        borrows.push(binding.clone());
    }
    assert_eq!(borrows.len(), 2);
    assert_ne!(borrows[0].occurrence, borrows[1].occurrence);
    assert_eq!(borrows[0].borrowed, borrows[1].borrowed);
    assert_ne!(borrows[0].parent, borrows[1].parent);
    FRESH_SOURCE_OBSERVED_V1.set((2, 2));
    Ok(())
}

#[test]
fn actual_fresh_reborrows_keep_child_occurrence_and_original_ancestor_identity() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::Borrowed),
        observe_fresh_reborrow_returns_v1,
        LIMIT,
        LIMIT,
        |_, _, _| Ok(()),
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (2, 2));
}

fn observe_no_normal_stack_v1(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(emitted.len(), 7);
    let shared_target = instances.owner().source_semantic().functions()[3]
        .blocks()
        .len()
        == 8;
    let active = emitted.iter().flatten().count();
    assert_eq!(active, if shared_target { 7 } else { 5 });
    assert_eq!(slots.instances.len(), active);
    let mut no_normal = 0;
    for (ordinal, row) in emitted.iter().enumerate() {
        let id = instances.id_at(ordinal).unwrap();
        assert_eq!(instances.instance_reachable(id), Some(row.is_some()));
        let Some(row) = row else { continue };
        let archive = row.execution_observation.as_ref().unwrap();
        assert_eq!(row.source_call_instance, Some(id));
        let original = instances.instance(id).unwrap();
        for mapping in &row.blocks {
            assert_eq!(
                instances.block_reachable(id, mapping.semantic_block),
                Some(true)
            );
        }
        for call in instances
            .calls(id)
            .unwrap()
            .iter()
            .filter(|call| call.child().is_some())
        {
            let control = instances.call_control(call.occurrence()).unwrap();
            if control == ProductionCallControlV1::Unreachable {
                assert!(
                    row.call_returns
                        .sites
                        .rows
                        .iter()
                        .all(|anchor| anchor.semantic_block != call.occurrence().block)
                );
                continue;
            }
            if control == ProductionCallControlV1::MayReturn {
                assert!(shared_target);
                assert!(
                    row.call_returns
                        .sites
                        .rows
                        .iter()
                        .any(|anchor| anchor.semantic_block == call.occurrence().block
                            && matches!(anchor.kind, SemanticKirCallReturnKindV1::Call { .. }))
                );
                continue;
            }
            assert_eq!(control, ProductionCallControlV1::NoNormalReturn);
            assert_eq!(
                instances.instance_may_return(call.child().unwrap()),
                Some(false)
            );
            let anchor = row
                .call_returns
                .sites
                .rows
                .iter()
                .find(|anchor| anchor.semantic_block == call.occurrence().block)
                .unwrap();
            let SemanticKirCallReturnKindV1::NoNormalReturnCall { call_operation, .. } =
                anchor.kind
            else {
                panic!("no fabricated normal call anchor")
            };
            assert_eq!(anchor.components(), CallComponentSpanV1::EMPTY);
            let mapping = row
                .blocks
                .iter()
                .find(|mapping| mapping.semantic_block == anchor.semantic_block)
                .unwrap();
            let block = row
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == mapping.kernel_ir_block)
                .unwrap();
            assert_eq!(block.operations.len(), call_operation as usize + 1);
            assert!(matches!(block.terminator, Some(Terminator::Unreachable)));
            let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(call.occurrence().block.index()), 0);
            for definition in original.ssa().plan().edge_definitions(edge).unwrap() {
                assert!(
                    archive
                        .lookup_original_v29(instances, id, definition.value(), budget)
                        .is_err()
                );
            }
            let mut control_checked = false;
            with_execution_availability_v29(instances, id, budget, |mut cursor, budget| {
                // Exercise the archive's direct edge gate using genuine source
                // occurrence/control authority. This is not a seeded lowering
                // or permission to fabricate a current source result.
                cursor.block = Some(edge.source());
                let definitions = original.ssa().plan().edge_definitions(edge).unwrap();
                assert!(!definitions.is_empty());
                for definition in definitions {
                    let site = ExecutionArchiveDefinitionSiteV29::Edge {
                        edge,
                        local: definition.variable().get(),
                    };
                    assert!(matches!(
                        cursor.check_archive_definition_v29(definition.value(), site, budget),
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "execution archive differs from its original source definition",
                            ..
                        })
                    ));
                }
                if shared_target {
                    let live = SsaEdgeIdV1::new(SsaBlockIdV1::new(3), 0);
                    cursor.block = Some(live.source());
                    let definitions = original.ssa().plan().edge_definitions(live).unwrap();
                    assert!(!definitions.is_empty());
                    for definition in definitions {
                        cursor.check_archive_definition_v29(
                            definition.value(),
                            ExecutionArchiveDefinitionSiteV29::Edge {
                                edge: live,
                                local: definition.variable().get(),
                            },
                            budget,
                        )?;
                    }
                }
                control_checked = true;
                Ok(())
            })?;
            assert!(control_checked);
            if shared_target {
                assert_eq!(original.function().index(), 3);
                assert_eq!(call.occurrence().block.index(), 1);
                let target = call.source().destination().unwrap().edge().target();
                assert_eq!(target.index(), 4);
                assert_eq!(instances.block_reachable(id, target), Some(true));
                assert!(
                    row.blocks
                        .iter()
                        .any(|mapping| mapping.semantic_block == target)
                );
                let live_edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(3), 0);
                for definition in original.ssa().plan().edge_definitions(live_edge).unwrap() {
                    archive.lookup_original_v29(instances, id, definition.value(), budget)?;
                }
            }
            no_normal += 1;
        }
    }
    assert_eq!(no_normal, if shared_target { 2 } else { 4 });
    let floor = budget.storage();
    let index = execution_emitted_index_v1(instances, emitted.iter().flatten(), budget)?;
    for (ordinal, row) in index.iter().enumerate() {
        assert_eq!(row.is_some(), emitted[ordinal].is_some());
        if let Some(row) = row {
            assert!(std::ptr::eq(*row, emitted[ordinal].as_ref().unwrap()));
        }
    }
    let bytes = std::mem::size_of::<Vec<Option<&LoweredFunctionResultV1>>>()
        + index.capacity() * std::mem::size_of::<Option<&LoweredFunctionResultV1>>();
    drop(index);
    budget.release_storage(bytes)?;
    assert_eq!(budget.storage(), floor);
    assert!(
        execution_emitted_index_v1(instances, emitted.iter().flatten().skip(1), budget).is_err()
    );
    assert_eq!(budget.storage(), floor);
    assert!(
        execution_emitted_index_v1(
            instances,
            emitted
                .iter()
                .flatten()
                .chain(emitted.iter().flatten().take(1)),
            budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), floor);
    // Vector creation pays three fixed work units, then each original slot is
    // initialized and checked once; each active row plus iterator exhaustion
    // pays four. Inactive rows remain original-ID holes, not compact ordinals.
    let count = instances.instances().len();
    let exact_work = 3 + 2 * count + 4 * (active + 1);
    let exact_storage = std::mem::size_of::<Vec<Option<&LoweredFunctionResultV1>>>()
        + count * std::mem::size_of::<Option<&LoweredFunctionResultV1>>();
    for (work_limit, storage_limit, succeeds) in [
        (exact_work, 31 + exact_storage, true),
        (exact_work - 1, 31 + exact_storage, false),
        (exact_work, 31 + exact_storage - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut query = ArgumentBudgetV1::new(&mut work, storage_limit);
        query.reserve_storage(31)?;
        let result = execution_emitted_index_v1(instances, emitted.iter().flatten(), &mut query);
        assert_eq!(result.is_ok(), succeeds);
        if let Ok(rows) = result {
            assert_eq!(rows.len(), count);
            assert_eq!(rows.iter().flatten().count(), active);
            drop(rows);
            query.release_storage(exact_storage)?;
        }
        assert_eq!(query.storage(), 31);
        assert_eq!(
            query.failed_storage(),
            (storage_limit < 31 + exact_storage).then_some(31 + exact_storage)
        );
        drop(query);
        if storage_limit == 31 + exact_storage {
            assert_eq!(
                work.work(),
                if succeeds {
                    exact_work
                } else {
                    exact_work - count
                }
            );
        }
        assert_eq!(
            work.failed_work(),
            (work_limit < exact_work).then_some(exact_work)
        );
    }
    FRESH_SOURCE_OBSERVED_V1.set((active, no_normal));
    Err(unsupported(
        0,
        None,
        None,
        "fresh no-normal source observed before lifecycle admission",
    ))
}

#[test]
fn actual_no_normal_children_preserve_inactive_absence_and_never_install_return_values() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::NoNormalReturn),
        observe_no_normal_stack_v1,
        LIMIT,
        LIMIT,
        |_, _, _| panic!("the observer deliberately stops before lifecycle admission"),
    );
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "fresh no-normal source observed before lifecycle admission",
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (5, 4));
}

#[test]
fn actual_no_normal_edge_does_not_disable_another_predecessors_shared_target() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::SharedTarget),
        observe_no_normal_stack_v1,
        LIMIT,
        LIMIT,
        |_, _, _| panic!("the observer stops before divergent lifecycle admission"),
    );
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "fresh no-normal source observed before lifecycle admission",
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (7, 2));
}

fn observe_no_normal_pending_v1(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let result = observe_no_normal_stack_v1(source, instances, emitted, slots, budget);
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "fresh no-normal source observed before lifecycle admission",
                ..
            })
        ),
        "all source checks must reach the explicit observer stop: {result:?}"
    );
    Ok(())
}

#[test]
fn pending_no_normal_expansion_uses_compact_original_ids_without_claiming_executable_lifecycle() {
    FRESH_SOURCE_OBSERVED_V1.set((0, 0));
    let mut completed = false;
    let (result, _, _) = run_profiled_suffix_owner(
        || fresh_tile_return_owner_v1(FreshReturnCaseV1::NoNormalReturn),
        observe_no_normal_pending_v1,
        LIMIT,
        LIMIT,
        |output, _, _| {
            let pending = &output.pending;
            assert_eq!(pending.coordinates.sources.rows.len(), 7);
            assert_eq!(pending.coordinates.seeds.rows.len(), 5);
            assert_eq!(pending.sidecars.rows.len(), 5);
            assert!(pending.coordinates.returns.rows.is_empty());
            assert_eq!(pending.coordinates.anchors.rows.len(), 4);
            assert!(
                pending
                    .coordinates
                    .anchors
                    .rows
                    .iter()
                    .all(|row| row.removed)
            );
            assert_eq!(
                pending
                    .coordinates
                    .controls
                    .rows
                    .iter()
                    .filter(|row| matches!(
                        row.origin,
                        InstanceControlOriginV1::ParameterPreheader { .. }
                    ))
                    .count(),
                4
            );
            assert_eq!(
                pending
                    .coordinates
                    .controls
                    .rows
                    .iter()
                    .filter(|row| matches!(row.origin, InstanceControlOriginV1::CallEntry { .. }))
                    .count(),
                4
            );
            for sidecar in &pending.sidecars.rows {
                let id = sidecar.source_call_instance.unwrap();
                assert_eq!(pending.coordinates.sources.rows[id.index()].instance, id);
                assert!(sidecar.execution_observation.is_some());
            }
            assert!(
                pending
                    .function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .all(|block| !matches!(block.terminator, Some(Terminator::Return { .. })))
            );
            completed = true;
            Ok(())
        },
    );
    assert!(
        result.is_ok(),
        "pending source correspondence only: {result:?}"
    );
    assert!(completed);
    assert_eq!(FRESH_SOURCE_OBSERVED_V1.get(), (5, 4));
}
