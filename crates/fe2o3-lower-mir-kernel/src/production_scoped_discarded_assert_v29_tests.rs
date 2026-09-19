use super::*;
use crate::production_semantic_kir_v1::scoped_slot_uses_v29;

const LIMIT: usize = 10_000_000;

fn check_assertion(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    folded: bool,
) {
    let source = instances.instance(instance).unwrap().declaration();
    let SemanticTerminatorKindV1::Assert {
        condition, message, ..
    } = source.blocks()[0].terminator().kind()
    else {
        panic!("expected assertion");
    };
    let SemanticAssertMessageV1::BoundsCheck { length, index } = message else {
        panic!("expected bounds diagnostics");
    };
    let mut expected = Vec::new();
    for (role, operand) in [
        (ExecutionOperandV29::AssertCondition, condition),
        (ExecutionOperandV29::AssertMessage(0), length),
        (ExecutionOperandV29::AssertMessage(1), index),
    ] {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            panic!("expected local operand");
        };
        assert_eq!(
            place.local().index(),
            match role {
                ExecutionOperandV29::AssertCondition => 4,
                ExecutionOperandV29::AssertMessage(0) => 2,
                ExecutionOperandV29::AssertMessage(1) => 3,
                _ => unreachable!(),
            }
        );
        if matches!(operand, SemanticOperandV1::Move(_)) {
            expected.push((role, place.local().index()));
        }
    }
    let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
    let span = lowered
        .terminator_operation_spans
        .iter()
        .find(|span| span.semantic_block.index() == 0)
        .unwrap();
    let block = lowered
        .function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| block.id == span.kernel_ir_block)
        .unwrap();
    let start = span.first_operation_ordinal as usize;
    let end = start + span.operation_count as usize;
    let actual_memory: Vec<_> = block.operations[start..end]
        .iter()
        .filter(|op| scoped_memory_pointer_v29(&op.kind).is_some())
        .collect();
    assert_eq!(actual_memory.len(), usize::from(!folded));
    if !folded {
        let condition_pointer = lowered
            .scoped_slot_origins
            .as_ref()
            .unwrap()
            .iter()
            .find(|slot| slot.local == 4)
            .unwrap()
            .pointer;
        assert!(
            matches!(actual_memory[0].kind, OperationKind::Load { pointer, .. }
            if pointer == condition_pointer)
        );
    }
    let rows: Vec<_> = anchors
        .rows
        .iter()
        .filter(|row| {
            row.source.is_some_and(|frame| {
                frame.site == execution_site_v29(SemanticBlockIdV1::from_index(0), None)
            })
        })
        .collect();
    assert_eq!(rows.len(), expected.len() + usize::from(!folded));
    let mut kills = Vec::new();
    for row in rows {
        match row.kind {
            ScopedMemoryAnchorKindV29::Access { .. } => {
                assert!(!folded);
                assert_eq!(
                    row.source.unwrap().role,
                    Some(ScopedMemoryRoleV29::Operand(
                        ExecutionOperandV29::AssertCondition
                    ))
                );
                assert_eq!(row.position, start);
            }
            ScopedMemoryAnchorKindV29::Kill {
                event,
                local,
                cause,
            } => {
                assert_eq!(cause, ScopedMemoryKillV29::Move);
                assert_eq!(row.position, start + usize::from(!folded));
                let occurrences = instances.occurrences(instance).unwrap();
                let original = &occurrences.events()[event];
                assert_eq!(original.role(), ExecutionEventV29::MoveKill);
                assert_eq!(original.site(), row.source.unwrap().site);
                assert_eq!(
                    Some(ScopedMemoryRoleV29::Operand(original.operand())),
                    row.source.unwrap().role
                );
                kills.push((original.operand(), local));
            }
        }
    }
    assert_eq!(kills, expected);
    assert_eq!(end, start + usize::from(!folded));
}

fn observe_runtime(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let mut helpers = 0;
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        helpers += 1;
        assert_eq!(slots.slots[item.slots.clone()].len(), 3);
        check_assertion(
            instances,
            item.instance,
            emitted[item.instance.index()].as_ref().unwrap(),
            false,
        );
    }
    assert_eq!(helpers, 2);
    let floor = budget.storage();
    let result = scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    );
    assert_eq!(budget.storage(), floor);
    result?;
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn runtime_discarded_moves_invalidate_alias_reads_without_diagnostic_loads() {
    for move_condition in [false, true] {
        for move_message in [false, true] {
            for reinitialize in [false, true] {
                let fixture = ScopedFixture::AssertionSlots {
                    move_condition,
                    move_message,
                    reinitialize,
                };
                let (result, _, _) = run(false, fixture, observe_runtime, LIMIT, LIMIT);
                assert_eq!(OBSERVED.get(), 1, "{fixture:?}: {result:?}");
                if reinitialize || !(move_condition || move_message) {
                    assert!(is_stopped(&result), "{fixture:?}: {result:?}");
                } else {
                    assert!(
                        matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                function: 3,
                                detail: "scoped slot read is not initialized in its fresh physical activation",
                                ..
                            })
                        ),
                        "{fixture:?}: {result:?}"
                    );
                }
            }
        }
    }
}

fn observe_folded(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        let floor = budget.storage();
        with_canonical_call_scratch_v1(budget, |budget| {
            let instance = item.instance;
            let row = instances.instance(instance).unwrap();
            let mut plan = execution_instance_plan_v29(
                instances,
                instance,
                FunctionId::new("folded_assert_probe"),
                item.placement,
                budget,
            )?;
            assert_eq!(plan.parameter_declarations.len(), 1);
            assert_eq!(plan.parameter_declarations[0].1, 1);
            emission_push_v1(
                &mut plan.parameter_local_bindings,
                PlannedParameterLocalBindingV1::Direct {
                    local: 1,
                    value: plan.parameter_values[0],
                    ty: plan.parameter_types[0].clone(),
                },
                budget,
            )?;
            let mut producer = ExecutionLifecycleProducerV29::new(
                source,
                instances,
                instance,
                item.placement,
                budget,
            )?;
            let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
            let lowered =
                with_execution_availability_v29(instances, instance, budget, |cursor, budget| {
                    lower_one_semantic_function_with_calls_v29(
                        instances.owner().source_semantic(),
                        &plan,
                        row.ssa(),
                        &BTreeMap::new(),
                        &BTreeMap::new(),
                        None,
                        // Exercise the private folded emitter, not admission of a folding proof.
                        BTreeSet::from([0]),
                        1,
                        false,
                        1024,
                        None,
                        &mut private,
                        None,
                        budget,
                        item.placement,
                        Some(cursor),
                        None,
                        Some(&mut producer),
                    )
                })?;
            check_assertion(instances, instance, &lowered, true);
            Ok(())
        })?;
        assert_eq!(budget.storage(), floor);
    }
    observe_runtime(source, instances, emitted, slots, budget)
}

#[test]
fn folded_conditions_and_diagnostics_preserve_same_gap_move_order_without_loads() {
    for move_condition in [false, true] {
        for move_message in [false, true] {
            let fixture = ScopedFixture::AssertionSlots {
                move_condition,
                move_message,
                reinitialize: true,
            };
            let (result, _, _) = run(false, fixture, observe_folded, LIMIT, LIMIT);
            assert_eq!(OBSERVED.get(), 1, "{fixture:?}: {result:?}");
            assert!(is_stopped(&result), "{fixture:?}: {result:?}");
        }
    }
}

#[test]
fn discarded_move_recording_keeps_exact_work_and_storage_failures() {
    let fixture = ScopedFixture::AssertionSlots {
        move_condition: true,
        move_message: true,
        reinitialize: true,
    };
    let (result, work, storage) = run(false, fixture, observe_runtime, LIMIT, LIMIT);
    assert!(is_stopped(&result), "{result:?}");
    assert!(is_stopped(
        &run(false, fixture, observe_runtime, work, storage).0
    ));
    let error = run(false, fixture, observe_runtime, work - 1, storage)
        .0
        .unwrap_err();
    assert_resource(&error, true);
    let error = run(false, fixture, observe_runtime, work, storage - 1)
        .0
        .unwrap_err();
    assert_resource(&error, false);
}
