use super::*;

fn store_owner(
    explicit: bool,
    field: bool,
    count: usize,
    read_value: bool,
    volatility: SemanticVolatilityV1,
    atomic: Option<SemanticAtomicAccessV1>,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        let original = &functions[2];
        let old = original.blocks()[0].statements();
        assert_eq!(old.len(), 4);
        assert!(
            matches!(old[2].kind(), SemanticStatementKindV1::Assign(assignment)
            if !assignment.destination().projections().is_empty())
        );
        let destination = if field {
            projected(
                1,
                &[
                    (SemanticProjectionKindV1::Field(0), REFERENCE),
                    (SemanticProjectionKindV1::Dereference, WORD),
                ],
            )
        } else {
            projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)])
        };
        let mut statements = old[..2].to_vec();
        for ordinal in 0..count {
            let value = if read_value {
                SemanticOperandV1::Copy(projected(
                    2,
                    &[(SemanticProjectionKindV1::Dereference, WORD)],
                ))
            } else {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    WORD,
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(17 + ordinal as u128, 8).unwrap(),
                    ),
                ))
            };
            statements.push(if explicit {
                statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    destination.clone(),
                    value,
                    volatility,
                    atomic,
                )))
            } else {
                assert_eq!(volatility, SemanticVolatilityV1::NonVolatile);
                assert!(atomic.is_none());
                assign(destination.clone(), SemanticRvalueKindV1::Use(value))
            });
        }
        statements.push(old[3].clone());
        functions[2] = function(
            30,
            false,
            CAPTURE,
            original.locals().to_vec(),
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

fn store_role(explicit: bool) -> ExecutionOperandV29 {
    if explicit {
        ExecutionOperandV29::StoreDestination
    } else {
        ExecutionOperandV29::Destination
    }
}

fn destination_events(cursor: &ExecutionAvailabilityV29<'_>, explicit: bool) -> Vec<usize> {
    cursor
        .occurrences
        .events()
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            event.operand() == store_role(explicit) && event.role() == ExecutionEventV29::BaseUse
        })
        .map(|(index, _)| index)
        .collect()
}

#[test]
fn projected_store_destinations_consume_each_original_use_and_emit_each_write() {
    for explicit in [false, true] {
        for field in [false, true] {
            for count in [1, 2] {
                for read_value in [false, true] {
                    let mut observed = BTreeSet::new();
                    cell_emission_tests::with_cell_source_lowered_cursor(
                        store_owner(
                            explicit,
                            field,
                            count,
                            read_value,
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        ),
                        |cursor| {
                            if cursor.function_id.index() != 2 {
                                return;
                            }
                            assert!(observed.insert(cursor.instance.index()));
                            let events = destination_events(cursor, explicit);
                            assert_eq!(events.len(), count);
                            for (ordinal, index) in events.into_iter().enumerate() {
                                let event = &cursor.occurrences.events()[index];
                                let local = if field { 1 } else { 2 };
                                assert_eq!(
                                    event.site(),
                                    execution_site_v29(
                                        SemanticBlockIdV1::from_index(0),
                                        Some(ordinal as u32 + 2),
                                    )
                                );
                                assert_eq!(event.event().variable().get(), local);
                                assert!(event.is_reachable() && event.is_promoted());
                                assert!(matches!(event.resolved(),
                                    Some(SsaResolvedEventV1::Use { variable, .. })
                                    if variable.get() == local));
                                assert_eq!(
                                    cursor.function.locals()[local as usize].ty(),
                                    if field { CAPTURE } else { REFERENCE }
                                );
                                if read_value {
                                    let value = &cursor.occurrences.events()[index - 1];
                                    assert_eq!(value.site(), event.site());
                                    assert_eq!(
                                        value.operand(),
                                        if explicit {
                                            ExecutionOperandV29::StoreValue
                                        } else {
                                            ExecutionOperandV29::RvalueOperand(0)
                                        }
                                    );
                                    assert_eq!(value.role(), ExecutionEventV29::BaseUse);
                                    assert_eq!(value.event().variable().get(), 2);
                                }
                            }
                        },
                        |plan, emission, emitted, budget| {
                            assert_eq!(plan.cells.rows.len(), 2);
                            let mut backings = BTreeSet::new();
                            let mut helpers = BTreeSet::new();
                            for (cell_index, cell) in plan.cells.rows.iter().enumerate() {
                                let worker = &emitted
                                    .iter()
                                    .find(|(id, _)| *id == cell.instance)
                                    .unwrap()
                                    .1;
                                let slot = worker
                                    .scoped_slot_origins
                                    .as_ref()
                                    .unwrap()
                                    .iter()
                                    .find(|slot| slot.legacy_local().unwrap() == cell.local.index())
                                    .unwrap();
                                assert!(backings.insert(slot.pointer));
                                let child = plan.instances.calls(cell.instance).unwrap()[0]
                                    .child()
                                    .unwrap();
                                assert!(helpers.insert(child.index()));
                                let helper =
                                    &emitted.iter().find(|(id, _)| *id == child).unwrap().1;
                                let body = helper.function.body.as_ref().unwrap();
                                assert_eq!(body.parameters.len(), 1);
                                let pointer = body.parameters[0];
                                let operations: Vec<_> = body
                                    .blocks
                                    .iter()
                                    .flat_map(|block| &block.operations)
                                    .collect();
                                assert_eq!(
                                    operations
                                        .iter()
                                        .filter(|operation| matches!(
                                            operation.kind,
                                            OperationKind::Store { pointer: actual, .. }
                                                if actual == pointer
                                        ))
                                        .count(),
                                    count
                                );
                                assert_eq!(
                                    operations
                                        .iter()
                                        .filter(|operation| matches!(
                                            operation.kind,
                                            OperationKind::Load { pointer: actual, .. }
                                                if actual == pointer
                                        ))
                                        .count(),
                                    1 + if read_value { count } else { 0 }
                                );
                                assert!(!operations.iter().any(|operation| matches!(
                                    operation.kind,
                                    OperationKind::Alloca { .. }
                                )));
                                let writes: Vec<_> = plan
                                    .accesses
                                    .iter()
                                    .zip(&emission.cell_accesses)
                                    .filter(|(row, _)| {
                                        row.key.site.instance == child
                                            && row.key.access == SourceReferenceAccessV29::Write
                                    })
                                    .collect();
                                assert_eq!(writes.len(), count);
                                let mut sites = BTreeSet::new();
                                for (row, claim) in writes {
                                    let claim = claim.get().unwrap();
                                    assert_eq!(claim.cell, cell_index);
                                    assert_eq!(claim.pointer, pointer);
                                    assert!(claim.operation.is_some());
                                    assert!(sites.insert(row.key.site.statement.unwrap()));
                                }
                                assert_eq!(sites, (2..2 + count).collect::<BTreeSet<_>>());
                                plan.check_owner(plan.instances, budget)?;
                            }
                            assert_eq!(helpers.len(), 2);
                            let mut module = Module::new("source_reference_store_component");
                            module.functions = emitted
                                .iter()
                                .map(|(_, row)| row.function.clone())
                                .collect();
                            module.kernels.push(Kernel::new(
                                "source_reference_component",
                                "source_reference_component",
                                LaunchDomain::D1 {
                                    x: LaunchExtent::Static(64),
                                },
                            ));
                            verify_module(&module).unwrap();
                            Ok(())
                        },
                    )
                    .unwrap();
                    assert_eq!(observed, BTreeSet::from([3, 4]));
                }
            }
        }
    }
}

#[test]
fn projected_store_destinations_reject_missing_duplicate_and_unindexed_uses() {
    for explicit in [false, true] {
        for field in [false, true] {
            cell_emission_tests::with_cell_source_lowered(
                store_owner(
                    explicit,
                    field,
                    2,
                    true,
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
                |_, _, _, _| Ok(()),
            )
            .unwrap();
            for instance in [3, 4] {
                for ordinal in 0..2 {
                    for mutation in 0..3 {
                        let entered = std::cell::Cell::new(false);
                        let result = cell_emission_tests::with_cell_source_lowered_cursor(
                            store_owner(
                                explicit,
                                field,
                                2,
                                true,
                                SemanticVolatilityV1::NonVolatile,
                                None,
                            ),
                            |cursor| {
                                if cursor.function_id.index() != 2
                                    || cursor.instance.index() != instance
                                {
                                    return;
                                }
                                assert!(!entered.replace(true));
                                let events = destination_events(cursor, explicit);
                                assert_eq!(events.len(), 2);
                                let event = events[ordinal];
                                match mutation {
                                    0 => cursor.skipped_event = Some(event),
                                    1 => cursor.claimed[event] = true,
                                    2 => {
                                        let before = cursor.index.len();
                                        cursor.index.retain(|row| row.index != event);
                                        assert_eq!(cursor.index.len() + 1, before);
                                    }
                                    _ => unreachable!(),
                                }
                            },
                            |_, _, _, _| panic!("invalid store event reached completed emission"),
                        );
                        assert!(entered.get());
                        assert!(
                            matches!(result,
                                Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                                if detail == "execution availability differs from its source SSA instance"
                            ),
                            "explicit={explicit}, field={field}, instance={instance}, store={ordinal}, mutation={mutation}: {result:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn projected_store_event_repair_does_not_admit_ordered_reference_writes() {
    for field in [false, true] {
        for atomic in [false, true] {
            let entered = std::cell::Cell::new(false);
            let result = cell_emission_tests::with_cell_source_lowered_cursor(
                store_owner(
                    true,
                    field,
                    2,
                    false,
                    if atomic {
                        SemanticVolatilityV1::NonVolatile
                    } else {
                        SemanticVolatilityV1::Volatile
                    },
                    atomic.then_some(SemanticAtomicAccessV1::new(
                        SemanticAtomicOrderingV1::Release,
                        SemanticAtomicScopeV1::SingleThread,
                    )),
                ),
                |_| entered.set(true),
                |_, _, _, _| panic!("ordered store reached scalar-cell emission"),
            );
            assert!(!entered.get());
            assert!(
                matches!(result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                    if detail == "source reference ordered store requires checked addressable effects"
                ),
                "field={field}, atomic={atomic}: {result:?}"
            );
        }
    }
}
