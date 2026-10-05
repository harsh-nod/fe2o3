fn with_control_instances(
    owner: &ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    production_call_instances_v1::with_production_call_instances_v1(
        owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consume(
                instances, budget,
            ))
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(budget.storage(), 23);
}

#[test]
fn cursor_control_matches_original_facts_without_compressing_coordinates() {
    for exit in [Exit::Loop, Exit::Unreachable, Exit::MaybeReturn] {
        let owner = no_return_owner(exit, false, false);
        with_control_instances(&owner, |instances, budget| {
            assert_eq!(instances.instances().len(), 5);
            for index in 0..instances.instances().len() {
                let id = instances.id_at(index).unwrap();
                let active = instances.instance_reachable(id).unwrap();
                let floor = budget.storage();
                let result =
                    with_execution_availability_v29(instances, id, budget, |cursor, budget| {
                        let original = instances.instance(id).unwrap();
                        assert_eq!(cursor.visited.len(), original.declaration().blocks().len());
                        assert_eq!(
                            cursor.claimed.len(),
                            instances.occurrences(id).unwrap().events().len()
                        );
                        assert_eq!(
                            cursor.control.may_return(budget)?,
                            instances.instance_may_return(id).unwrap()
                        );
                        for index in 0..cursor.visited.len() {
                            let block = SemanticBlockIdV1::from_index(index as u32);
                            let reachable = instances.block_reachable(id, block).unwrap();
                            assert_eq!(
                                cursor.source_block_reachable_v29(block, budget)?,
                                reachable
                            );
                            if !reachable {
                                assert!(cursor.events.blocks[index].is_empty());
                                assert!(cursor.cfg.ranges[index].is_empty());
                                assert_eq!(cursor.cfg.incoming[index], 0);
                            }
                        }
                        for &index in &cursor.events.required {
                            let event = &cursor.occurrences.events()[index];
                            let block = SemanticBlockIdV1::from_index(
                                execution_event_block_v29(event.site()).get(),
                            );
                            assert_eq!(instances.block_reachable(id, block), Some(true));
                        }
                        assert!(cursor.claimed.iter().all(|claimed| !claimed));
                        assert!(cursor.visited.iter().all(|visited| !visited));
                        Ok(())
                    });
                assert_eq!(result.is_ok(), active, "instance {index}: {result:?}");
                assert_eq!(budget.storage(), floor);
            }
            Ok(())
        });
    }
}

#[test]
fn cursor_control_removes_only_dead_normal_edges_not_shared_targets_or_unwind() {
    use production_call_instances_v1::ProductionCallControlV1;
    for (exit, bypass, unwind) in [
        (Exit::Loop, false, false),
        (Exit::Loop, true, false),
        (Exit::Unreachable, false, true),
        (Exit::MaybeReturn, true, false),
    ] {
        let owner = no_return_owner(exit, bypass, unwind);
        with_control_instances(&owner, |instances, budget| {
            with_execution_availability_v29(
                instances,
                instances.root(),
                budget,
                |cursor, budget| {
                    let expected = cursor
                        .occurrences
                        .successors()
                        .iter()
                        .filter(|edge| {
                            let block = SemanticBlockIdV1::from_index(edge.id().source().get());
                            instances.block_reachable(instances.root(), block) == Some(true)
                                && (edge.edge().role() != SemanticEdgeRoleV1::CallReturn
                                    || instances.call_control(ProductionCallOccurrenceV1 {
                                        caller: instances.root(),
                                        block,
                                    }) == Some(ProductionCallControlV1::MayReturn))
                        })
                        .map(|edge| (edge.id(), edge.edge().target().index() as usize))
                        .collect::<Vec<_>>();
                    let actual = cursor
                        .cfg
                        .edges
                        .iter()
                        .map(|edge| (edge.id, edge.target))
                        .collect::<Vec<_>>();
                    assert_eq!(actual, expected);
                    assert!(cursor.cfg.edges.iter().all(|edge| !edge.claimed));
                    if matches!(exit, Exit::Loop) && bypass {
                        assert!(cursor.source_block_reachable_v29(
                            SemanticBlockIdV1::from_index(2),
                            budget
                        )?);
                        assert_eq!(cursor.cfg.incoming[2], 1);
                    }
                    if unwind {
                        assert_eq!(cursor.cfg.edges.len(), 1);
                        let kept = cursor.cfg.edges[0].id;
                        let original = cursor
                            .occurrences
                            .successors()
                            .iter()
                            .find(|edge| edge.id() == kept)
                            .unwrap();
                        assert_eq!(original.edge().role(), SemanticEdgeRoleV1::CallUnwind);
                        assert_eq!(cursor.cfg.incoming[2], 1);
                    }
                    Ok(())
                },
            )
        });
    }
}

#[test]
fn cursor_control_final_census_requires_exact_live_block_coverage() {
    let owner = no_return_owner(Exit::Loop, false, false);
    with_control_instances(&owner, |instances, budget| {
        for add_dead_visit in [false, true] {
            with_execution_availability_v29(
                instances,
                instances.root(),
                budget,
                |mut cursor, budget| {
                    assert!(cursor.finish(budget).is_err());
                    assert!(!cursor.events.finished);
                    let entry = cursor.function.entry();
                    cursor.begin_block(entry, budget)?;
                    assert!(cursor.events.pending.is_empty());
                    cursor.finish_block(budget)?;
                    if add_dead_visit {
                        // A forged visit to an original dead suffix must not satisfy the census.
                        cursor.visited[1] = true;
                        assert!(cursor.finish(budget).is_err());
                        assert!(!cursor.events.finished);
                    } else {
                        cursor.finish(budget)?;
                        assert!(cursor.events.finished);
                        assert!(cursor.claimed.iter().all(|claimed| !claimed));
                    }
                    Ok(())
                },
            )?;
        }
        Ok(())
    });
}

#[test]
fn cursor_control_authenticates_instance_call_ledger_and_dead_entry() {
    let owner = no_return_owner(Exit::Loop, false, false);
    with_control_instances(&owner, |instances, budget| {
        with_execution_availability_v29(
            instances,
            instances.root(),
            budget,
            |mut cursor, budget| {
                let block = cursor.function.entry();
                let SemanticTerminatorKindV1::Call(call) = cursor.function.blocks()
                    [block.index() as usize]
                    .terminator()
                    .kind()
                else {
                    unreachable!()
                };
                assert_eq!(
                    cursor.source_call_control_v29(block, call, budget)?,
                    production_call_instances_v1::ProductionCallControlV1::NoNormalReturn
                );
                assert!(
                    cursor
                        .source_call_control_v29(block, &call.clone(), budget)
                        .is_err()
                );
                assert!(
                    cursor
                        .control
                        .check_source(
                            cursor.source,
                            cursor.function,
                            cursor.ssa,
                            instances.id_at(1).unwrap(),
                            budget
                        )
                        .is_err()
                );
                assert!(
                    cursor
                        .source_block_reachable_v29(SemanticBlockIdV1::from_index(u32::MAX), budget)
                        .is_err()
                );
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
                assert!(
                    cursor
                        .source_block_reachable_v29(block, &mut foreign)
                        .is_err()
                );
                assert!(
                    cursor
                        .begin_block(SemanticBlockIdV1::from_index(1), budget)
                        .is_err()
                );
                assert!(cursor.visited.iter().all(|visited| !visited));
                assert!(cursor.block.is_none());
                let original = cursor.source;
                cursor.source.root = SemanticFunctionIdV1::from_index(1);
                assert!(cursor.source_block_reachable_v29(block, budget).is_err());
                cursor.source = original;
                assert!(cursor.source_block_reachable_v29(block, budget)?);
                Ok(())
            },
        )
    });
}

#[test]
fn cursor_control_reference_cfg_requires_states_only_for_original_live_blocks() {
    for exit in [Exit::Loop, Exit::Unreachable] {
        let owner = no_return_owner(exit, false, false);
        with_control_instances(&owner, |instances, budget| {
            let floor = budget.storage();
            with_source_reference_storage_plan_v29(
                instances,
                SourceReferenceStorageV29::ScalarCells,
                budget,
                |plan, budget| {
                    let references = SourceReferenceEmissionV29::new(plan, budget)?;
                    let result = (|| {
                        for index in 0..instances.instances().len() {
                            let id = instances.id_at(index).unwrap();
                            if instances.instance_reachable(id) == Some(false) {
                                continue;
                            }
                            with_source_reference_availability_v29(
                                instances,
                                id,
                                Some(&references),
                                budget,
                                |cursor, budget| {
                                    for block in 0..cursor.visited.len() {
                                        let active = cursor.source_block_reachable_v29(
                                            SemanticBlockIdV1::from_index(block as u32),
                                            budget,
                                        )?;
                                        if !active {
                                            assert!(cursor.cfg.ranges[block].is_empty());
                                            assert!(cursor.events.blocks[block].is_empty());
                                        }
                                    }
                                    Ok(())
                                },
                            )?;
                        }
                        Ok(())
                    })();
                    let cleanup = references.abort_scope(instances, budget);
                    result.and(cleanup)
                },
            )?;
            // abort_scope leaves its two caller result headers paid until the
            // surrounding scope has consumed them and destroyed all owners.
            let headers = 2 * std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>();
            assert_eq!(budget.storage(), floor + headers);
            budget.release_storage(headers)?;
            Ok(())
        });
    }
}

#[test]
fn cursor_control_exact_and_one_short_limits_preserve_outer_floor() {
    let owner = no_return_owner(Exit::Loop, false, false);
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                    with_execution_availability_v29(
                        instances,
                        instances.root(),
                        budget,
                        |mut cursor, budget| {
                            cursor.begin_block(cursor.function.entry(), budget)?;
                            cursor.finish_block(budget)?;
                            cursor.finish(budget)
                        },
                    ),
                )
            },
        );
        assert_eq!(budget.storage(), 23);
        (result, budget.work(), budget.peak_storage())
    };
    let (result, work, storage) = run(usize::MAX, usize::MAX);
    result.unwrap().unwrap();
    run(work, storage).0.unwrap().unwrap();
    for (work, storage) in [(work - 1, storage), (work, storage - 1)] {
        let result = run(work, storage).0;
        assert!(result.is_err() || result.unwrap().is_err());
    }
}
