use super::*;

thread_local! {
    static INDEX_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static INDEX_FINISHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_array_index(
    pending: &mut PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    _plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
    let mut components = Vec::new();
    let mut places = Vec::new();
    for (sidecar_ordinal, sidecar) in pending.sidecars.rows.iter().enumerate() {
        let instance = sidecar.source_call_instance.unwrap();
        let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
        for (ordinal, row) in anchors.rows.iter().enumerate() {
            let ScopedMemoryAnchorKindV29::Object(payload_ordinal) = row.kind else {
                continue;
            };
            let payload = &anchors.objects[payload_ordinal];
            let ScopedObjectRoleV29::Project { projected, .. } = payload.role else {
                continue;
            };
            if !matches!(
                payload.operation,
                ScopedObjectOperationV29::Project {
                    step: ScopedObjectProjectionV29::ArrayIndex(_),
                    ..
                }
            ) {
                continue;
            }
            let point = index
                .emitted
                .point(instance, row.block, row.position as u32, budget)?
                .unwrap();
            let block = pending
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .position(|block| block.id == point.0)
                .unwrap();
            let storage = budget.storage();
            check_source_array_component_index_v29(instances, &index, instance, ordinal, budget)?;
            assert_eq!(
                budget.storage(),
                storage,
                "producer join borrows existing indices"
            );
            let (is_place, operand) = match projected.source {
                ScopedObjectSourceV29::AggregateComponent { operand, .. } => (false, operand),
                ScopedObjectSourceV29::Place { prefix: 1, .. } => {
                    let (_, length, _) = SOURCE_ARRAY_CASE_V29.get();
                    (true, u32::try_from(length - 1).unwrap())
                }
                _ => panic!("unexpected array projection source"),
            };
            let selected = if is_place {
                &mut places
            } else {
                &mut components
            };
            selected.push((
                sidecar_ordinal,
                instance,
                ordinal,
                payload_ordinal,
                block,
                point.1 as usize,
                operand,
            ));
        }
    }
    let (_, length, _) = SOURCE_ARRAY_CASE_V29.get();
    assert_eq!(
        components.len(),
        4 * length as usize,
        "two helper activations and two generations"
    );
    assert_eq!(
        places.len(),
        8,
        "each generation has one indexed store and load"
    );
    index.discard(budget)?;
    assert_eq!(budget.storage(), floor);
    let raw_fault = INDEX_FAULT.get();
    let fault = if raw_fault >= 20 {
        raw_fault - 20
    } else {
        raw_fault
    };
    let selected = if raw_fault >= 20 {
        &places
    } else {
        &components
    };
    if fault == 0 {
        let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
        let (sidecar, instance, ordinal, _, _, _, _) = selected[0];
        let anchors = pending.sidecars.rows[sidecar]
            .scoped_memory_anchors
            .as_ref()
            .unwrap();
        check_source_array_component_index_v29(instances, &index, instance, ordinal, budget)?;
        let memory_access =
            source_address_object_payload_v29(anchors, &anchors.rows[ordinal], budget).unwrap();
        assert!(
            memory_access.is_none(),
            "a checked projection is not a value access"
        );
        index.discard(budget)?;
        INDEX_FINISHED.set(components.len());
        return Ok(());
    }
    let (sidecar, instance, ordinal, payload_ordinal, block, position, operand) = selected[0];
    if fault >= 10 {
        let mut index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
        let start = budget.work();
        let storage = budget.storage();
        let peak = budget.peak_storage();
        check_source_array_component_index_v29(instances, &index, instance, ordinal, budget)?;
        let query_work = budget.work() - start;
        assert!(query_work > 0);
        assert_eq!((budget.storage(), budget.peak_storage()), (storage, peak));
        match fault {
            10 | 11 => {
                let short = usize::from(fault == 11);
                budget.charge_work(MODULE_LIMIT - budget.work() - query_work + short)?;
                let query = check_source_array_component_index_v29(
                    instances, &index, instance, ordinal, budget,
                );
                assert_eq!(query.is_ok(), short == 0, "exact measured query boundary");
                let first = if let Err(error) = query {
                    error
                } else {
                    assert_eq!(budget.work(), MODULE_LIMIT);
                    budget.charge_work(1).unwrap_err().into()
                };
                assert!(matches!(
                    first,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                ));
                let before = (budget.work(), budget.storage());
                let retry = check_source_array_component_index_v29(
                    instances, &index, instance, ordinal, budget,
                )
                .unwrap_err();
                assert_eq!(format!("{first:?}"), format!("{retry:?}"));
                assert_eq!((budget.work(), budget.storage()), before);
                INDEX_FINISHED.set(components.len());
                return Err(first);
            }
            12 => {
                let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                let before = (budget.work(), budget.storage());
                let error = check_source_array_component_index_v29(
                    instances,
                    &index,
                    instance,
                    ordinal,
                    &mut foreign,
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                ));
                assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                assert_eq!((budget.work(), budget.storage()), before);
                let slot = index.slot;
                index.slot ^= 1;
                assert!(matches!(
                    check_source_array_component_index_v29(
                        instances, &index, instance, ordinal, budget
                    ),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert_eq!((budget.work(), budget.storage()), before);
                index.slot = slot;
                budget.release_storage(1)?;
                assert!(matches!(
                    check_source_array_component_index_v29(
                        instances, &index, instance, ordinal, budget
                    ),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                budget.reserve_storage(1)?;
                check_source_array_component_index_v29(
                    instances, &index, instance, ordinal, budget,
                )?;
                let foreign_owner = original_argument_pointer_array_owner_v29();
                assert_eq!(
                    foreign_owner.source_semantic_sha256(),
                    instances.owner().source_semantic_sha256()
                );
                assert_ne!(foreign_owner.identity(), instances.owner().identity());
                production_call_instances_v1::with_production_call_instances_v1(
                    &foreign_owner,
                    SemanticFunctionIdV1::from_index(0),
                    budget,
                    |foreign_instances, budget| {
                        let error = check_source_array_component_index_v29(
                            foreign_instances,
                            &index,
                            instance,
                            ordinal,
                            budget,
                        )
                        .unwrap_err();
                        assert_eq!(
                            format!("{error:?}"),
                            format!("{:?}", scoped_object_error_v29())
                        );
                        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
                    },
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        index.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        INDEX_FINISHED.set(components.len());
        return Ok(());
    }
    let saved_project =
        pending.function.body.as_ref().unwrap().blocks[block].operations[position].clone();
    let saved_producer =
        pending.function.body.as_ref().unwrap().blocks[block].operations[position - 1].clone();
    let saved_payload = pending.sidecars.rows[sidecar]
        .scoped_memory_anchors
        .as_ref()
        .unwrap()
        .objects[payload_ordinal];
    match fault {
        1 => {
            pending.function.body.as_mut().unwrap().blocks[block].operations[position - 1].kind =
                OperationKind::Constant(Constant::Index(u64::from(operand) + 1))
        }
        2 | 3 => {
            // Match the capture to the forged actual SSA operand. Recipe and
            // operation replay still succeed; only the producer join refuses.
            let replacement = if fault == 2 {
                let other = selected[1];
                pending.function.body.as_ref().unwrap().blocks[other.4].operations[other.5 - 1]
                    .results[0]
                    .id
            } else {
                ValueId(u32::MAX)
            };
            let mut operation = saved_payload.operation;
            let ScopedObjectOperationV29::Project { step, .. } = &mut operation else {
                unreachable!()
            };
            *step = ScopedObjectProjectionV29::ArrayIndex(replacement);
            pending.function.body.as_mut().unwrap().blocks[block].operations[position].kind =
                OperationKind::Storage(operation);
            pending.sidecars.rows[sidecar]
                .scoped_memory_anchors
                .as_mut()
                .unwrap()
                .objects[payload_ordinal]
                .operation = operation;
        }
        4 => {
            pending.function.body.as_mut().unwrap().blocks[block].operations[position - 1].results
                [0]
            .ty = Type::Scalar(ScalarType::U64)
        }
        5 => {
            pending.function.body.as_mut().unwrap().blocks[block].operations[position - 1].results
                [0]
            .id = ValueId(u32::MAX)
        }
        6 => {
            let producer =
                &mut pending.function.body.as_mut().unwrap().blocks[block].operations[position - 1];
            producer
                .results
                .push(ValueDef::new(ValueId(u32::MAX), Type::INDEX));
        }
        7 => {
            pending.function.body.as_mut().unwrap().blocks[block].operations[position - 1].kind =
                OperationKind::Constant(Constant::U64(u64::from(operand)))
        }
        _ => unreachable!(),
    }
    let index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
    let anchors = pending.sidecars.rows[sidecar]
        .scoped_memory_anchors
        .as_ref()
        .unwrap();
    let row = &anchors.rows[ordinal];
    let payload = &anchors.objects[payload_ordinal];
    let original = instances.instance(instance).unwrap().declaration();
    let occurrences = instances.occurrences(instance).unwrap();
    anchors.check_object_source(original, &occurrences, ordinal, row, payload, budget)?;
    payload.check_operation(
        &pending.function.body.as_ref().unwrap().blocks[block].operations[position],
        budget,
    )?;
    let error =
        check_source_array_component_index_v29(instances, &index, instance, ordinal, budget)
            .unwrap_err();
    assert_eq!(
        format!("{error:?}"),
        format!("{:?}", scoped_object_error_v29())
    );
    index.discard(budget)?;
    pending.function.body.as_mut().unwrap().blocks[block].operations[position] = saved_project;
    pending.function.body.as_mut().unwrap().blocks[block].operations[position - 1] = saved_producer;
    pending.sidecars.rows[sidecar]
        .scoped_memory_anchors
        .as_mut()
        .unwrap()
        .objects[payload_ordinal] = saved_payload;
    assert_eq!(budget.storage(), floor);
    INDEX_FINISHED.set(components.len());
    Err(error)
}

fn run_array_index(fault: u8, length: u64) {
    struct Restore(
        (u16, u64, bool),
        SourceArrayModeV29,
        Option<RootExecutionArchiveObserverV29>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
            SOURCE_ARRAY_MODE_V29.set(self.1);
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.2);
        }
    }
    let _restore = Restore(
        SOURCE_ARRAY_CASE_V29.replace((64, length, false)),
        SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::ThinPointer),
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(observe_array_index)),
    );
    INDEX_FAULT.set(fault);
    INDEX_FINISHED.set(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        scalar_payload_prepared_from_v18(original_argument_pointer_array_owner_v29, &mut budget);
    let reached = std::cell::Cell::new(false);
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            reached.set(true);
            Ok(())
        });
    let success = matches!(fault, 0 | 12 | 20 | 32);
    assert_eq!(
        INDEX_FINISHED.get(),
        4 * length as usize,
        "producer assertions must not be skipped or caught"
    );
    assert_eq!(result.is_ok(), success, "fault {fault}: {result:?}");
    assert_eq!(reached.get(), success);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn actual_array_component_producers_join_inlined_activations_and_generations_without_final_admission()
 {
    for length in [1, 3, 5] {
        run_array_index(0, length);
    }
}

#[test]
fn actual_array_component_index_literal_tampering_refuses_after_recipe_replay_succeeds() {
    run_array_index(1, 3);
}

#[test]
fn actual_array_component_index_ssa_tampering_refuses_even_with_matching_capture() {
    for fault in [2, 3] {
        run_array_index(fault, 3);
    }
}

#[test]
fn actual_array_component_index_producer_requires_exact_definition_type_arity_and_constant_kind() {
    for fault in [4, 5, 6, 7] {
        run_array_index(fault, 3);
    }
}

#[test]
fn actual_array_component_index_join_exact_short_work_is_sticky_and_storage_is_borrowed() {
    for fault in [10, 11, 12] {
        run_array_index(fault, 3);
    }
}

#[test]
fn actual_original_constant_index_producers_join_store_and_load_generations() {
    for length in [1, 3, 5] {
        run_array_index(20, length);
    }
}

#[test]
fn actual_original_constant_index_literal_and_ssa_tampering_refuse_after_recipe_replay() {
    for fault in 21..=27 {
        run_array_index(fault, 3);
    }
}

#[test]
fn actual_original_constant_index_exact_short_work_and_foreign_custody_refuse() {
    for fault in [30, 31, 32] {
        run_array_index(fault, 3);
    }
}
