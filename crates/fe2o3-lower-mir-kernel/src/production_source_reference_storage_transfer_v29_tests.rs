use super::*;

pub(super) const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

#[derive(Clone, Copy)]
pub(super) enum TransferCase {
    NestedMove,
    MovedRead,
    Reinitialize,
    ReinitializeCopied,
    ReinitializeSiblingMoved,
    SelfMoveReplacement,
    ReinitializeLoop,
    CopyBeforeMove,
    Deinitialize,
    DeinitializeRead,
    CopyBeforeDeinitialize,
    WholeDeinitialize,
    RepeatedCall,
    ConditionalMove { reverse: bool, read_moved: bool },
    ConditionalLoan { reverse: bool },
}

fn field_path(local: u32, nested: bool) -> SemanticPlaceV1 {
    if nested {
        projected(
            local,
            &[
                (SemanticProjectionKindV1::Field(0), CAPTURE),
                (SemanticProjectionKindV1::Field(0), REFERENCE),
            ],
        )
    } else {
        projected(local, &[(SemanticProjectionKindV1::Field(1), REFERENCE)])
    }
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

pub(super) fn transfer_owner(case: TransferCase) -> ProductionSemanticSsaOwnerV1 {
    try_transfer_owner(case).unwrap()
}

fn try_transfer_owner(
    case: TransferCase,
) -> Result<ProductionSemanticSsaOwnerV1, fe2o3_pliron::ProductionSemanticSsaErrorV1> {
    try_owner_with(Case::Shared, |types, functions| {
        assert_eq!(types.len(), PAIR.index() as usize);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([86; 32]),
            SemanticLayoutIdentityV1::from_sha256([86; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![CAPTURE, REFERENCE]).unwrap(),
            ),
        ));
        let mut locals = functions[2].locals().to_vec();
        locals.push(local(86, PAIR, SemanticLocalRoleV1::Temporary));
        locals.push(local(87, REFERENCE, SemanticLocalRoleV1::Temporary));
        locals.push(local(88, PAIR, SemanticLocalRoleV1::Temporary));
        let mut initial: Vec<_> = functions[2].blocks()[0]
            .statements()
            .iter()
            .filter(|statement| {
                !matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local() == SemanticLocalIdV1::from_index(0))
            })
            .cloned()
            .collect();
        let conditional_loan = matches!(case, TransferCase::ConditionalLoan { .. });
        if conditional_loan {
            initial.push(assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(3, WORD),
                },
            ));
        }
        initial.push(assign(
            place(5, PAIR),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(1, CAPTURE)),
                        SemanticOperandV1::Copy(place(
                            if conditional_loan { 4 } else { 2 },
                            REFERENCE,
                        )),
                    ],
                )
                .unwrap(),
            ),
        ));
        if conditional_loan {
            initial.push(dead(4));
        }
        if matches!(
            case,
            TransferCase::CopyBeforeMove | TransferCase::CopyBeforeDeinitialize
                | TransferCase::ReinitializeCopied
        ) {
            initial.push(assign(
                place(7, PAIR),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(5, PAIR))),
            ));
        }
        let selected = if matches!(case, TransferCase::WholeDeinitialize) {
            place(5, PAIR)
        } else {
            field_path(5, !conditional_loan)
        };
        let moving = assign(
            place(6, REFERENCE),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field_path(
                5, !conditional_loan && !matches!(case, TransferCase::ReinitializeSiblingMoved),
            ))),
        );
        let blocks = match case {
            TransferCase::ReinitializeLoop => {
                let update = assign(selected.clone(), SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(place(2, REFERENCE)),
                ));
                vec![
                    block(30, initial, goto(1)),
                    block(31, vec![moving, update], SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(3, WORD)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 1))],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        ).unwrap(),
                    }),
                    block(32, vec![
                        assign(place(4, REFERENCE), SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Copy(selected),
                        )), unit(),
                    ], SemanticTerminatorKindV1::Return),
                ]
            }
            TransferCase::ConditionalMove { reverse, .. }
            | TransferCase::ConditionalLoan { reverse } => {
                let read_moved = match case {
                    TransferCase::ConditionalMove { read_moved, .. } => read_moved,
                    _ => false,
                };
                let mut moved = vec![moving];
                moved.push(dead(6));
                let mut tail = Vec::new();
                if conditional_loan {
                    tail.push(dead(3));
                } else {
                    tail.push(assign(
                        place(4, REFERENCE),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field_path(
                            5, read_moved,
                        ))),
                    ));
                }
                tail.push(unit());
                vec![
                    block(
                        30,
                        initial,
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(place(3, WORD)),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    edge(
                                        SemanticEdgeRoleV1::SwitchValue,
                                        if reverse { 2 } else { 1 },
                                    ),
                                )],
                                edge(
                                    SemanticEdgeRoleV1::SwitchOtherwise,
                                    if reverse { 1 } else { 2 },
                                ),
                            )
                            .unwrap(),
                        },
                    ),
                    block(31, moved, goto(3)),
                    block(32, vec![], goto(3)),
                    block(33, tail, SemanticTerminatorKindV1::Return),
                ]
            }
            _ => {
                if matches!(
                    case,
                    TransferCase::Deinitialize
                        | TransferCase::DeinitializeRead
                        | TransferCase::CopyBeforeDeinitialize
                        | TransferCase::WholeDeinitialize
                ) {
                    initial.push(SemanticStatementV1::new(
                        source(),
                        SemanticStatementKindV1::Deinitialize(selected.clone()),
                    ));
                } else if !matches!(case, TransferCase::SelfMoveReplacement) {
                    initial.push(moving);
                }
                if matches!(case, TransferCase::Reinitialize | TransferCase::ReinitializeCopied
                    | TransferCase::ReinitializeSiblingMoved | TransferCase::SelfMoveReplacement)
                {
                    initial.push(assign(
                        selected.clone(),
                        SemanticRvalueKindV1::Use(if matches!(case, TransferCase::SelfMoveReplacement) {
                            SemanticOperandV1::Move(selected.clone())
                        } else { SemanticOperandV1::Copy(place(2, REFERENCE)) }),
                    ));
                }
                let read = match case {
                    TransferCase::MovedRead
                    | TransferCase::Reinitialize
                    | TransferCase::ReinitializeSiblingMoved
                    | TransferCase::SelfMoveReplacement
                    | TransferCase::DeinitializeRead => selected,
                    TransferCase::CopyBeforeMove | TransferCase::CopyBeforeDeinitialize
                    | TransferCase::ReinitializeCopied => {
                        field_path(7, true)
                    }
                    TransferCase::WholeDeinitialize => place(2, REFERENCE),
                    _ => field_path(5, false),
                };
                initial.push(assign(
                    place(4, REFERENCE),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
                ));
                initial.push(assign(
                    place(3, WORD),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                        4,
                        &[(SemanticProjectionKindV1::Dereference, WORD)],
                    ))),
                ));
                initial.push(unit());
                vec![block(30, initial, SemanticTerminatorKindV1::Return)]
            }
        };
        functions[2] = function(30, false, CAPTURE, locals, blocks);
        if matches!(case, TransferCase::RepeatedCall) {
            let worker = &functions[1];
            let mut statements = vec![SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
            )];
            statements.extend_from_slice(worker.blocks()[0].statements());
            functions[1] = function(
                20,
                false,
                WORD,
                worker.locals().to_vec(),
                vec![
                    block(
                        20,
                        statements,
                        worker.blocks()[0].terminator().kind().clone(),
                    ),
                    block(
                        21,
                        vec![],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: SemanticOperandV1::Copy(place(1, WORD)),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 2),
                                )],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, 0),
                            )
                            .unwrap(),
                        },
                    ),
                    block(22, vec![], SemanticTerminatorKindV1::Return),
                ],
            );
        }
    })
}

#[test]
fn source_partial_nested_moves_preserve_sibling_and_copied_holders_in_actual_emission() {
    for case in [
        TransferCase::NestedMove,
        TransferCase::CopyBeforeMove,
        TransferCase::Deinitialize,
        TransferCase::Reinitialize,
    ] {
        super::super::cell_emission_tests::with_cell_source_lowered(
            transfer_owner(case),
            |plan, _, emitted, _| {
                assert!(!plan.storage_snapshots.is_empty());
                assert!(
                    plan.nodes
                        .iter()
                        .any(|node| node.kind == SourceReferenceNodeKindV29::Absent)
                );
                assert_eq!(emitted.len(), plan.instances.instances().len());
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn source_static_field_updates_preserve_copies_removed_siblings_and_loop_transport() {
    for case in [TransferCase::ReinitializeCopied, TransferCase::ReinitializeSiblingMoved,
        TransferCase::SelfMoveReplacement, TransferCase::ReinitializeLoop]
    {
        let mut completed = false;
        super::super::cell_emission_tests::with_cell_source_lowered(transfer_owner(case),
            |plan, _, emitted, _| {
                assert_eq!(emitted.len(), plan.instances.instances().len());
                let helpers: Vec<_> = plan.instances.instances().iter().enumerate()
                    .filter(|(_, instance)| instance.function().index() == 2).collect();
                assert_eq!(helpers.len(), 2);
                for (ordinal, helper) in helpers {
                    let function = helper.declaration();
                    let mut updates = 0;
                    for (block_index, block) in function.blocks().iter().enumerate() {
                        for (statement_index, statement) in block.statements().iter().enumerate() {
                            if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                                && assignment.destination().local().index() == 5
                                && !assignment.destination().projections().is_empty()
                            {
                                updates += 1;
                                let site = execution_site_v29(
                                    SemanticBlockIdV1::from_index(block_index as u32),
                                    Some(statement_index as u32),
                                );
                                let occurrences = plan.instances.occurrences(
                                    plan.instances.id_at(ordinal).unwrap(),
                                ).unwrap();
                                let rows: Vec<_> = occurrences.events().iter().filter(|event|
                                    event.site() == site && event.operand() == ExecutionOperandV29::Destination
                                ).collect();
                                assert_eq!(rows.len(), 2);
                                assert!(rows.iter().all(|event| event.is_promoted()));
                                let Some(SsaResolvedEventV1::Use { variable, value: old }) = rows[0].resolved()
                                else { panic!("missing original old-holder use") };
                                let Some(SsaResolvedEventV1::Define { variable: defined, value: new }) = rows[1].resolved()
                                else { panic!("missing original new-holder definition") };
                                assert_eq!(variable.get(), 5);
                                assert_eq!(variable, defined);
                                assert_ne!(old, new);
                            }
                        }
                    }
                    assert_eq!(updates, 1);
                }
                completed = true;
                Ok(())
            },
        ).unwrap();
        assert!(completed);
    }
}

#[test]
fn source_partial_moved_leaf_is_not_readable_even_while_a_sibling_remains_live() {
    drop(try_transfer_owner(TransferCase::Reinitialize).unwrap());
    let error = try_transfer_owner(TransferCase::MovedRead)
        .err()
        .expect("moved source leaf admitted by SSA");
    assert_eq!(
        error,
        fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
            function: SemanticFunctionIdV1::from_index(2),
            block: 0,
            statement: Some(4),
            local: 5,
            violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        }
    );
}

#[test]
fn actual_deinitialize_has_an_exact_empty_span_and_preserves_independent_copied_holders() {
    for case in [
        TransferCase::Deinitialize,
        TransferCase::CopyBeforeDeinitialize,
        TransferCase::WholeDeinitialize,
    ] {
        let completed = std::cell::Cell::new(false);
        let result = super::super::cell_emission_tests::with_cell_source_lowered(
            transfer_owner(case),
            |plan, references, emitted, budget| {
                references.check(budget)?;
                let expected = (0..plan.instances.instances().len())
                    .filter_map(|ordinal| plan.instances.id_at(ordinal))
                    .filter(|&id| plan.instances.instance(id).unwrap().function().index() == 2)
                    .collect::<Vec<_>>();
                assert_eq!(expected.len(), 2, "the original root invokes both helper instances");
                let mut seen = BTreeSet::new();
                for (id, lowered) in &emitted {
                    let instance = plan.instances.instance(*id).unwrap();
                    if instance.function().index() != 2 {
                        continue;
                    }
                    assert!(expected.contains(id));
                    assert!(seen.insert(id.index()), "no original helper may be counted twice");
                    let mut removals = 0;
                    assert!(
                        instance.ssa().plan().promoted_variables().iter().any(|local| local.get() == 5)
                    );
                    assert!(instance.ssa().partial_move_certificate().work_units() > 0);
                    for (block, original) in instance.declaration().blocks().iter().enumerate() {
                        for (statement, original) in original.statements().iter().enumerate() {
                            if !matches!(original.kind(), SemanticStatementKindV1::Deinitialize(_)) {
                                continue;
                            }
                            let matching = lowered.statement_operation_spans.iter().filter(|span| {
                                span.semantic_block().index() as usize == block
                                    && span.statement_ordinal() as usize == statement
                            }).collect::<Vec<_>>();
                            assert_eq!(matching.len(), 1);
                            assert_eq!(matching[0].semantic_function(), instance.function());
                            assert_eq!(matching[0].operation_count(), 0);
                            removals += 1;
                        }
                    }
                    assert_eq!(removals, 1, "exactly one original removal per helper instance");
                }
                assert_eq!(seen.len(), expected.len());
                assert_eq!(emitted.len(), plan.instances.instances().len());
                completed.set(true);
                Ok(())
            },
        );
        assert!(
            completed.get(),
            "actual emission assertions must complete: {result:?}"
        );
        result.unwrap();
    }
}

#[test]
fn actual_deinitialized_nested_leaf_read_is_rejected_by_the_original_source_owner() {
    drop(try_transfer_owner(TransferCase::Deinitialize).unwrap());
    assert!(matches!(try_transfer_owner(TransferCase::DeinitializeRead),
        Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
            function, block: 0, statement: Some(4), local: 5,
            violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        }) if function.index() == 2));
}

#[test]
fn source_partial_join_keeps_may_loans_independent_of_must_initialization() {
    for reverse in [false, true] {
        let error = run_cells(
            transfer_owner(TransferCase::ConditionalLoan { reverse }),
            |_, _| panic!("possible loan was pruned by must-init join"),
        )
        .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference referent storage dies with a live loan",
                    ..
                }
            ),
            "{error:?}"
        );
        let error = try_transfer_owner(TransferCase::ConditionalMove {
            reverse,
            read_moved: true,
        })
        .err()
        .expect("one-predecessor move admitted as definitely initialized");
        assert_eq!(
            error,
            fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                function: SemanticFunctionIdV1::from_index(2),
                block: 3,
                statement: Some(0),
                local: 5,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            }
        );
    }
}

#[test]
fn source_partial_join_retains_readable_sibling_on_both_predecessor_orders() {
    for reverse in [false, true] {
        super::super::cell_emission_tests::with_cell_source_lowered(
            transfer_owner(TransferCase::ConditionalMove {
                reverse,
                read_moved: false,
            }),
            |plan, _, _, _| {
                assert!(!plan.storage_snapshots.is_empty());
                Ok(())
            },
        )
        .unwrap();
    }
}
