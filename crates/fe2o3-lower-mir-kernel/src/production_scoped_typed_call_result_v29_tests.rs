use super::*;
use scoped_root_tests::fixtures::CALL_DESTINATIONS_AGGREGATE_OPERAND_V29;

thread_local! {
    static COMPLETED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FAULT: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
}

pub(super) fn inspect(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let item = slots
        .instances
        .iter()
        .find(|row| row.function == CALLBACK)
        .unwrap();
    let lowered = emitted[item.instance.index()].as_ref().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
    let original = instances.instance(item.instance).unwrap().declaration();
    let result_rows: Vec<_> = anchors
        .rows
        .iter()
        .filter(|row| {
            row.source
                .is_some_and(|frame| frame.role == Some(ScopedMemoryRoleV29::CallResult))
        })
        .collect();
    assert_eq!(result_rows.len(), 3);
    let mut typed = 0;
    let mut scalar = 0;
    for (ordinal, row) in result_rows.iter().enumerate() {
        let site = execution_site_v29(SemanticBlockIdV1::from_index(ordinal as u32), None);
        assert_eq!(
            row.source,
            Some(ScopedMemoryFrameV29 {
                site,
                role: Some(ScopedMemoryRoleV29::CallResult)
            })
        );
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == row.block)
            .unwrap();
        let actual = &block.operations[row.position];
        let place = scoped_source_call_destination_v29(original, site).unwrap();
        match row.kind {
            ScopedMemoryAnchorKindV29::Object(index) => {
                assert!(matches!(ordinal, 0 | 2));
                typed += 1;
                let payload = &anchors.objects[index];
                let ScopedObjectRoleV29::WriteValue { destination, value } = payload.role else {
                    panic!("actual typed result write required")
                };
                assert_eq!(
                    value,
                    ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::CallResult {
                        site,
                        ty: place.ty()
                    })
                );
                assert_eq!(destination.projected_type, place.ty());
                assert_eq!(
                    destination.source,
                    ScopedObjectSourceV29::Place {
                        site,
                        role: ExecutionOperandV29::CallDestinationAddress,
                        local: place.local(),
                        prefix: place.projections().len() as u32,
                    }
                );
                let ScopedObjectOperationV29::WriteValue {
                    address,
                    value,
                    access,
                } = payload.operation
                else {
                    panic!("write operation required")
                };
                assert_eq!(actual.kind, OperationKind::Storage(payload.operation));
                assert!(actual.results.is_empty());
                assert_eq!(
                    access,
                    MemoryAccess::new(AddressSpace::Private, if ordinal == 0 { 1 } else { 4 })
                );
                let slot = slots
                    .slots
                    .iter()
                    .find(|slot| {
                        slot.instance == item.instance
                            && slot.origin.identity.original_local() == Some(3)
                    })
                    .unwrap();
                assert!(
                    matches!(slot.representation, ScopedSlotRepresentationV29::Object { schema, .. }
                    if schema == destination.projected_schema)
                );
                if ordinal == 0 {
                    assert!(
                        matches!(destination.object, ScopedObjectIdentityV29::Reference {
                        instance, site: actual, role: ExecutionOperandV29::CallDestinationAddress,
                        dereference_prefix: 1,
                    } if instance == item.instance && actual == site)
                    );
                    let call = block.operations[..row.position]
                        .iter()
                        .rposition(|op| matches!(op.kind, OperationKind::Call { .. }))
                        .unwrap();
                    assert_eq!(row.position, call + 1);
                    assert_eq!(block.operations[call].results.len(), 1);
                    assert_eq!(value, block.operations[call].results[0].id);
                    let witness = lowered
                        .call_returns
                        .sites
                        .rows
                        .iter()
                        .find(|row| row.semantic_block.index() == 0)
                        .unwrap();
                    assert!(matches!(witness.kind, SemanticKirCallReturnKindV1::Call {
                        destination: SemanticKirCallDestinationV1::Projected { pointer, access: actual }, ..
                    } if pointer == address && actual == access));
                } else {
                    assert_eq!(address, slot.origin.pointer);
                    assert!(
                        matches!(destination.object, ScopedObjectIdentityV29::Local { instance, local, .. }
                        if instance == item.instance && local.index() == 3)
                    );
                    assert!(
                        !block
                            .operations
                            .iter()
                            .any(|op| matches!(op.kind, OperationKind::Call { .. }))
                    );
                }
            }
            ScopedMemoryAnchorKindV29::Access { pointer, .. } => {
                assert_eq!(ordinal, 1);
                scalar += 1;
                assert!(
                    matches!(actual.kind, OperationKind::Store { pointer: actual, .. } if actual == pointer)
                );
                assert_eq!(place.local().index(), 0);
                assert!(place.projections().is_empty());
            }
            _ => panic!("result effect must remain an actual memory operation"),
        }
    }
    assert_eq!((typed, scalar), (2, 1));
    let holder_retained = slots.slots.iter().any(|slot| {
        slot.instance == item.instance && slot.origin.identity.original_local() == Some(4)
    });
    let preparation: Vec<_> = anchors
        .rows
        .iter()
        .filter(|row| {
            row.source
                == Some(ScopedMemoryFrameV29 {
                    site: execution_site_v29(SemanticBlockIdV1::from_index(0), None),
                    role: Some(ScopedMemoryRoleV29::Operand(
                        ExecutionOperandV29::CallDestinationAddress,
                    )),
                })
        })
        .collect();
    assert_eq!(preparation.len(), usize::from(holder_retained));
    for row in preparation {
        let ScopedMemoryAnchorKindV29::Object(index) = row.kind else {
            panic!("retained address uses an actual typed read")
        };
        assert!(matches!(
            anchors.objects[index].operation,
            ScopedObjectOperationV29::ReadValue { .. }
        ));
        assert_eq!(row.block, result_rows[0].block);
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == row.block)
            .unwrap();
        let call = block
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::Call { .. }))
            .unwrap();
        assert!(row.position < call);
    }
    with_canonical_call_scratch_v1(budget, |budget| {
        let calls = instances.calls(item.instance).unwrap();
        let original = &calls[0];
        let callee = instances.instance(original.child().unwrap()).unwrap();
        let target = &emitted[original.child().unwrap().index()]
            .as_ref()
            .unwrap()
            .function;
        let span = lowered
            .terminator_operation_spans
            .iter()
            .find(|span| span.semantic_block.index() == 0)
            .unwrap();
        let row = lowered
            .call_returns
            .sites
            .rows
            .iter()
            .find(|row| row.semantic_block.index() == 0)
            .unwrap();
        let SemanticKirCallReturnKindV1::Call {
            arguments_first,
            call_operation,
            destination_end,
            destination,
            ..
        } = row.kind
        else {
            unreachable!()
        };
        let values =
            scoped_call_index_with_deferred_parts_v29(instances, item.instance, lowered, budget)?;
        let block = values.block(span.kernel_ir_block, budget)?;
        let successor = scoped_call_block_v29(
            lowered,
            original.source().destination().unwrap().edge().target(),
            budget,
        )?;
        // The standalone legacy checker cannot manufacture the scoped recipe.
        let refused = check_resolved_defined_call_v1(
            instances.owner().source_semantic(),
            callee.declaration(),
            &target.id,
            target,
            successor,
            block,
            original.source(),
            span.first_operation_ordinal as usize,
            (span.first_operation_ordinal + span.operation_count) as usize,
            arguments_first as usize,
            call_operation as usize,
            destination_end as usize,
            destination,
            &values,
            budget,
        );
        assert!(
            matches!(
                refused,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ),
            "{refused:?}"
        );
        Ok(())
    })?;
    let moves: Vec<_> = anchors
        .rows
        .iter()
        .filter(|row| {
            matches!(
                row.kind,
                ScopedMemoryAnchorKindV29::Kill {
                    local: 3,
                    cause: ScopedMemoryKillV29::Move,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(moves.len(), 1);
    assert_eq!(
        moves[0].source.unwrap().role,
        Some(ScopedMemoryRoleV29::Operand(
            ExecutionOperandV29::CallArgument(0)
        ))
    );
    assert!(moves[0].position < result_rows[1].position);
    let floor = budget.storage();
    check_anchors(instances, emitted, slots, budget)?;
    assert_eq!(budget.storage(), floor);
    // Source equations without the original reference plan cannot grant a typed
    // memory receipt, even after every call phase has been checked successfully.
    let refusal = scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    );
    assert!(
        matches!(
            refusal,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "original raw source requires consuming expanded physical admission",
                ..
            })
        ),
        "{refusal:?}"
    );
    assert_eq!(budget.storage(), floor);
    Ok(())
}

fn inspect_hostile(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    inspect(instances, emitted, slots, budget)?;
    let Some(fault) = FAULT.get() else {
        COMPLETED.set(true);
        return Err(unsupported(0, None, None, STOP));
    };
    let item = slots
        .instances
        .iter()
        .find(|row| row.function == CALLBACK)
        .unwrap();
    let lowered = emitted[item.instance.index()].as_ref().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
    let ordinal = anchors
        .rows
        .iter()
        .position(|row| {
            row.source
                == Some(ScopedMemoryFrameV29 {
                    site: execution_site_v29(SemanticBlockIdV1::from_index(0), None),
                    role: Some(ScopedMemoryRoleV29::CallResult),
                })
        })
        .unwrap();
    let row = anchors.rows[ordinal];
    let ScopedMemoryAnchorKindV29::Object(index) = row.kind else {
        panic!("typed anchor required")
    };
    let original = anchors.objects[index];
    let lowered = emitted[item.instance.index()].as_mut().unwrap();
    let payload = &mut lowered.scoped_memory_anchors.as_mut().unwrap().objects[index];
    let ScopedObjectRoleV29::WriteValue { destination, value } = &mut payload.role else {
        unreachable!()
    };
    match fault {
        0 => {
            destination.source = ScopedObjectSourceV29::Place {
                site: execution_site_v29(SemanticBlockIdV1::from_index(1), None),
                role: ExecutionOperandV29::CallDestinationAddress,
                local: SemanticLocalIdV1::from_index(4),
                prefix: 1,
            }
        }
        1 => destination.projected_schema = fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX),
        2 => {
            *value = ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::CallResult {
                site: execution_site_v29(SemanticBlockIdV1::from_index(1), None),
                ty: destination.projected_type,
            })
        }
        3 => {
            destination.source = ScopedObjectSourceV29::Place {
                site: execution_site_v29(SemanticBlockIdV1::from_index(0), None),
                role: ExecutionOperandV29::CallArgument(0),
                local: SemanticLocalIdV1::from_index(4),
                prefix: 1,
            }
        }
        _ => panic!("unknown hostile case"),
    }
    let floor = budget.storage();
    let refused = check_scoped_defined_call_phases_v29(instances, emitted, budget);
    assert!(
        matches!(
            refused,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ),
        "fault {fault}: {refused:?}"
    );
    assert_eq!(budget.storage(), floor);
    emitted[item.instance.index()]
        .as_mut()
        .unwrap()
        .scoped_memory_anchors
        .as_mut()
        .unwrap()
        .objects[index] = original;
    inspect(instances, emitted, slots, budget)?;
    COMPLETED.set(true);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn typed_call_results_require_original_result_endpoint_and_schema_without_minting_memory_authority()
{
    for fault in [None, Some(0), Some(1), Some(2), Some(3)] {
        FAULT.set(fault);
        COMPLETED.set(false);
        let fixture = ScopedFixture::CallDestinations {
            projected: true,
            retained_address: false,
            indexed: false,
        };
        let (result, _, _) = run(false, fixture, inspect_hostile, LIMIT, LIMIT);
        assert!(COMPLETED.get(), "fault {fault:?}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
        assert!(is_stopped(&result), "{result:?}");
    }
    FAULT.set(None);
}

fn inspect_no_normal(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let item = slots
        .instances
        .iter()
        .find(|row| row.function == CALLBACK)
        .unwrap();
    let lowered = emitted[item.instance.index()].as_ref().unwrap();
    let original = &instances.calls(item.instance).unwrap()[0];
    assert_eq!(
        instances.call_control(original.occurrence()),
        Some(ProductionCallControlV1::NoNormalReturn)
    );
    assert_eq!(
        instances.instance_may_return(original.child().unwrap()),
        Some(false)
    );
    let row = lowered
        .call_returns
        .sites
        .rows
        .iter()
        .find(|row| row.semantic_block.index() == 0)
        .unwrap();
    let SemanticKirCallReturnKindV1::NoNormalReturnCall {
        arguments_first,
        call_operation,
        destination: Some(SemanticKirCallDestinationV1::Projected { pointer, access }),
    } = row.kind
    else {
        panic!("actual prepared no-return projected call required")
    };
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
    assert_eq!(
        call_operation + 1,
        span.first_operation_ordinal + span.operation_count
    );
    assert!(arguments_first <= call_operation);
    assert!(matches!(block.terminator, Some(Terminator::Unreachable)));
    // No normal control edge does not change the declared scalar call ABI. The
    // unused SSA result remains, but there can be no result write or continuation.
    let call = &block.operations[call_operation as usize];
    let callee = &emitted[original.child().unwrap().index()]
        .as_ref()
        .unwrap()
        .function;
    assert_eq!(callee.signature.results, [Type::Scalar(ScalarType::U32)]);
    assert!(
        matches!(&call.kind, OperationKind::Call { callee: actual, .. } if actual == &callee.id)
    );
    assert_eq!(call.results.len(), 1);
    assert_eq!(call.results[0].ty, callee.signature.results[0]);
    assert_eq!(block.operations.len(), call_operation as usize + 1);
    let original_alignment = instances.owner().source_semantic().types()[U32.index() as usize]
        .layout()
        .alignment_bytes();
    assert_eq!(
        original_alignment, 4,
        "the prepared field-zero address has the original scalar alignment"
    );
    assert_eq!(
        access,
        MemoryAccess::new(AddressSpace::Private, original_alignment as u32)
    );
    let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
    let declaration = instances.instance(item.instance).unwrap().declaration();
    let source_block = &declaration.blocks()[0];
    assert_eq!(source_block.statements().len(), 5);
    let SemanticStatementKindV1::Assign(aggregate_assignment) = source_block.statements()[2].kind()
    else {
        panic!("original aggregate initialization required");
    };
    let destination = aggregate_assignment.destination();
    assert_eq!(destination.local().index(), 5);
    assert!(destination.projections().is_empty());
    assert!(
        matches!(aggregate_assignment.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
        if *aggregate.kind() == SemanticAggregateKindV1::Tuple && aggregate.operands().len() == 2)
    );
    let initialization_site = execution_site_v29(SemanticBlockIdV1::from_index(0), Some(2));
    let occurrences = instances.occurrences(item.instance).unwrap();
    let mut initialized = [false; 2];
    let mut initialization_projects = 0;
    for (ordinal, row) in anchors.rows.iter().enumerate().filter(|(_, row)| {
        row.source
            .is_some_and(|frame| frame.site == initialization_site)
    }) {
        let ScopedMemoryAnchorKindV29::Object(object) = row.kind else {
            continue;
        };
        let payload = anchors.objects[object];
        assert_eq!(row.block, block.id);
        let operation = &block.operations[row.position];
        payload.check_operation(operation, budget)?;
        anchors.check_object_source(declaration, &occurrences, ordinal, row, &payload, budget)?;
        match payload.role {
            ScopedObjectRoleV29::Project { source, projected } => {
                assert_eq!(source.root_schema, projected.root_schema);
                assert_eq!(source.path.count, 0);
                assert_eq!(projected.path.count, 1);
                let ScopedObjectSourceV29::AggregateComponent { operand, .. } = projected.source
                else {
                    panic!("original aggregate field");
                };
                assert!(
                    matches!(payload.operation, ScopedObjectOperationV29::Project {
                    step: ScopedObjectProjectionV29::Field(field), ..
                } if field == operand)
                );
                assert_eq!(
                    anchors.object_path(projected.path, budget)?,
                    [ScopedObjectComponentV29::View {
                        projection: ScopedObjectViewProjectionV29::Field(operand),
                        ty: projected.projected_type,
                    }]
                );
                initialization_projects += 1;
            }
            ScopedObjectRoleV29::WriteValue {
                destination,
                value:
                    ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::Operand {
                        site,
                        role: ExecutionOperandV29::RvalueOperand(index),
                        source,
                        ty,
                    }),
            } => {
                assert_eq!(site, initialization_site);
                assert_eq!(destination.projected_type, ty);
                assert_eq!(
                    destination.source,
                    ScopedObjectSourceV29::AggregateComponent {
                        site,
                        operand: index,
                        destination: SemanticLocalIdV1::from_index(5),
                        variant: None,
                    }
                );
                let SemanticRvalueKindV1::Aggregate(original_aggregate) =
                    aggregate_assignment.value().kind()
                else {
                    unreachable!()
                };
                match (&original_aggregate.operands()[index as usize], source) {
                    (SemanticOperandV1::Constant(_), ScopedMemoryOperandSourceV29::Constant) => {}
                    (
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                        ScopedMemoryOperandSourceV29::Place(occurrence),
                    ) => {
                        assert_eq!(index, 0);
                        assert_eq!(place.local().index(), 3);
                        assert!(place.projections().is_empty());
                        assert!(
                            matches!(occurrence, ScopedMemoryOccurrenceV29::Retained { .. }),
                            "the fixture's explicit Store retains original local3"
                        );
                        check_scoped_payload_occurrence_v29(
                            &occurrences,
                            site,
                            ExecutionOperandV29::RvalueOperand(index),
                            place,
                            occurrence,
                            budget,
                        )?;
                    }
                    _ => panic!("actual original aggregate operand recipe"),
                }
                assert!(!std::mem::replace(&mut initialized[index as usize], true));
                let ScopedObjectOperationV29::WriteValue { value, access, .. } = payload.operation
                else {
                    panic!("field write");
                };
                assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 1));
                let definition = block.operations[..row.position]
                    .iter()
                    .find(|operation| operation.results.iter().any(|result| result.id == value))
                    .unwrap();
                if index == 0 && CALL_DESTINATIONS_AGGREGATE_OPERAND_V29.get() != 0 {
                    let slot = slots
                        .slots
                        .iter()
                        .find(|slot| {
                            slot.instance == item.instance
                                && slot.origin.identity.original_local() == Some(3)
                        })
                        .unwrap();
                    let (prior, anchor, read) = match slot.representation {
                        ScopedSlotRepresentationV29::ScalarArray(_) => {
                            assert!(matches!(definition.kind, OperationKind::Load { .. }));
                            let (prior, anchor, read) =
                                anchors
                                    .rows
                                    .iter()
                                    .enumerate()
                                    .find_map(|(prior, anchor)| {
                                        let ScopedMemoryAnchorKindV29::Access {
                                            payload:
                                                Some(ScopedMemoryPayloadV29::Load { result, read }),
                                            ..
                                        } = anchor.kind
                                        else {
                                            return None;
                                        };
                                        (result == value).then_some((prior, anchor, read))
                                    })
                                    .unwrap();
                            check_scoped_payload_v29(
                                declaration,
                                &occurrences,
                                anchor,
                                definition,
                                budget,
                            )?;
                            (prior, anchor, read)
                        }
                        ScopedSlotRepresentationV29::Object { schema, .. } => {
                            assert!(matches!(
                                definition.kind,
                                OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                            ));
                            let (prior, anchor, payload) = anchors
                                .rows
                                .iter()
                                .enumerate()
                                .find_map(|(prior, anchor)| {
                                    let ScopedMemoryAnchorKindV29::Object(object) = anchor.kind
                                    else {
                                        return None;
                                    };
                                    let payload = &anchors.objects[object];
                                    (payload.result == Some(value))
                                        .then_some((prior, anchor, payload))
                                })
                                .unwrap();
                            let ScopedObjectRoleV29::ReadValue {
                                source,
                                read: ScopedObjectReadOriginV29::Original(read),
                            } = payload.role
                            else {
                                panic!("exact original retained read");
                            };
                            assert_eq!(source.root_schema, schema);
                            assert!(
                                matches!(source.object, ScopedObjectIdentityV29::Local { instance, local, .. }
                                if instance == item.instance && local.index() == 3)
                            );
                            payload.check_operation(definition, budget)?;
                            anchors.check_object_source(
                                declaration,
                                &occurrences,
                                prior,
                                anchor,
                                payload,
                                budget,
                            )?;
                            (prior, anchor, read)
                        }
                    };
                    assert!(prior < ordinal && anchor.position < row.position);
                    assert_eq!(anchor.block, row.block);
                    assert_eq!(
                        (read.site, read.role, read.prefix, read.ty),
                        (site, ExecutionOperandV29::RvalueOperand(0), 0, U32)
                    );
                    assert_eq!(source, ScopedMemoryOperandSourceV29::Place(read.occurrence));
                } else {
                    assert_eq!(
                        definition.kind,
                        OperationKind::Constant(Constant::U32(if index == 0 { 0 } else { 11 }))
                    );
                }
                let mut wrong_operand = payload;
                let ScopedObjectRoleV29::WriteValue {
                    value: wrong_origin,
                    ..
                } = &mut wrong_operand.role
                else {
                    unreachable!()
                };
                let ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::Operand {
                    role,
                    ..
                }) = wrong_origin
                else {
                    unreachable!()
                };
                *role = ExecutionOperandV29::RvalueOperand(1 - index);
                assert!(matches!(
                    anchors.check_object_source(
                        declaration,
                        &occurrences,
                        ordinal,
                        row,
                        &wrong_operand,
                        budget
                    ),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "typed object source payload differs from its actual operation",
                        ..
                    })
                ));
                for fault in 0..3 {
                    let mut forged = payload;
                    let ScopedObjectRoleV29::WriteValue { destination, .. } = &mut forged.role
                    else {
                        unreachable!()
                    };
                    match fault {
                        0 => destination.projected_type = destination.root_type,
                        1 => {
                            destination.source = ScopedObjectSourceV29::AggregateComponent {
                                site,
                                operand: index,
                                destination: SemanticLocalIdV1::from_index(3),
                                variant: None,
                            }
                        }
                        2 => {
                            destination.source = ScopedObjectSourceV29::AggregateComponent {
                                site,
                                operand: index,
                                destination: SemanticLocalIdV1::from_index(5),
                                variant: Some(0),
                            }
                        }
                        _ => unreachable!(),
                    }
                    assert!(matches!(
                        anchors.check_object_source(
                            declaration,
                            &occurrences,
                            ordinal,
                            row,
                            &forged,
                            budget
                        ),
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "typed object source payload differs from its actual operation",
                            ..
                        })
                    ));
                }
                anchors.check_object_source(
                    declaration,
                    &occurrences,
                    ordinal,
                    row,
                    &payload,
                    budget,
                )?;
                let (endpoint, source_value) =
                    source_address_object_payload_v29(anchors, row, budget)?.unwrap();
                assert_eq!(endpoint, destination);
                assert!(
                    matches!(source_value, ScopedMemoryPayloadV29::Store { value: actual, .. } if actual == value)
                );
                // This remains an inert original recipe, not checked memory.
                // The consuming pipeline still owes complete final admission.
            }
            ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::Original(read),
            } => {
                assert_ne!(CALL_DESTINATIONS_AGGREGATE_OPERAND_V29.get(), 0);
                assert!(
                    matches!(source.object, ScopedObjectIdentityV29::Local { instance, local, .. }
                    if instance == item.instance && local.index() == 3)
                );
                assert_eq!(
                    (read.site, read.role, read.prefix, read.ty),
                    (
                        initialization_site,
                        ExecutionOperandV29::RvalueOperand(0),
                        0,
                        U32
                    )
                );
                assert!(matches!(
                    read.occurrence,
                    ScopedMemoryOccurrenceV29::Retained { .. }
                ));
            }
            _ => panic!(
                "only original operand reads and actual field projects/writes belong to initialization"
            ),
        }
    }
    assert_eq!(initialization_projects, 2);
    assert_eq!(initialized, [true, true]);
    let SemanticStatementKindV1::Assign(address_assignment) = source_block.statements()[3].kind()
    else {
        panic!("original field address required");
    };
    let SemanticRvalueKindV1::AddressOf {
        place: referent, ..
    } = address_assignment.value().kind()
    else {
        panic!("original raw field address required");
    };
    assert_eq!(referent.local().index(), 5);
    assert_eq!(referent.projections().len(), 1);
    assert_eq!(
        referent.projections()[0].kind(),
        SemanticProjectionKindV1::Field(0)
    );
    let projects: Vec<_> = anchors
        .rows
        .iter()
        .filter(|row| {
            row.source.is_some_and(|frame| {
                frame.site == execution_site_v29(SemanticBlockIdV1::from_index(0), Some(3))
            }) && matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_))
        })
        .filter(|row| {
            matches!(
                block.operations[row.position].kind,
                OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::Project { .. })
            )
        })
        .collect();
    assert_eq!(projects.len(), 1);
    let project = projects[0];
    assert_eq!(project.block, block.id);
    assert!(project.position < call_operation as usize);
    let ScopedMemoryAnchorKindV29::Object(object) = project.kind else {
        unreachable!()
    };
    anchors.objects[object].check_operation(&block.operations[project.position], budget)?;
    assert!(matches!(
        block.operations[project.position].kind,
        OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::Project {
            step: fe2o3_kernel_ir::StorageProjectionV1::Field(0),
            ..
        })
    ));
    let SemanticStatementKindV1::Store(store) = source_block.statements()[4].kind() else {
        panic!("original pre-call raw store is the typed backing demand");
    };
    assert_eq!(store.destination().local().index(), 4);
    assert_eq!(store.destination().projections().len(), 1);
    assert_eq!(
        store.destination().projections()[0].kind(),
        SemanticProjectionKindV1::Dereference
    );
    assert!(
        scoped_source_place_v29(
            declaration,
            execution_site_v29(SemanticBlockIdV1::from_index(0), Some(4)),
            ExecutionOperandV29::StoreDestination,
        )
        .is_some_and(|place| std::ptr::eq(place, store.destination()))
    );
    let prewrite: Vec<_> = anchors
        .rows
        .iter()
        .filter(|row| {
            row.source
                == Some(ScopedMemoryFrameV29 {
                    site: execution_site_v29(SemanticBlockIdV1::from_index(0), Some(4)),
                    role: Some(ScopedMemoryRoleV29::Operand(
                        ExecutionOperandV29::StoreDestination,
                    )),
                })
                && matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_))
        })
        .collect();
    assert_eq!(prewrite.len(), 1);
    let write_row = prewrite[0];
    let ScopedMemoryAnchorKindV29::Object(object) = write_row.kind else {
        unreachable!()
    };
    let write = anchors.objects[object];
    assert!(
        matches!(write.role, ScopedObjectRoleV29::WriteValue { destination, .. }
        if destination.source == (ScopedObjectSourceV29::Place {
            site: execution_site_v29(SemanticBlockIdV1::from_index(0), Some(4)),
            role: ExecutionOperandV29::StoreDestination,
            local: SemanticLocalIdV1::from_index(4), prefix: 1,
        }))
    );
    assert_eq!(write_row.block, block.id);
    assert!(write_row.position < call_operation as usize);
    // The original field-zero call address retains alignment four. A raw
    // dereference write to that address conservatively carries alignment one.
    assert!(
        matches!(write.operation, ScopedObjectOperationV29::WriteValue { address, access: actual, .. }
        if address == pointer && actual == MemoryAccess::new(AddressSpace::Private, 1))
    );
    assert_eq!(
        block.operations[write_row.position].kind,
        OperationKind::Storage(write.operation)
    );
    write.check_operation(&block.operations[write_row.position], budget)?;
    let write_ordinal = anchors
        .rows
        .iter()
        .position(|row| std::ptr::eq(row, write_row))
        .unwrap();
    anchors.check_object_source(
        declaration,
        &occurrences,
        write_ordinal,
        write_row,
        &write,
        budget,
    )?;
    assert!(!anchors.rows.iter().any(|row| {
        row.source
            .is_some_and(|frame| frame.role == Some(ScopedMemoryRoleV29::CallResult))
    }));
    let floor = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        let values =
            scoped_call_index_with_deferred_parts_v29(instances, item.instance, lowered, budget)?;
        assert!(matches!(values.ty(pointer, budget)?, Type::Pointer(ty)
            if matches!(*ty.pointee, Type::StorageObject(_))));
        check_scoped_defined_call_phases_v29(instances, emitted, budget)?;
        Ok(())
    })?;
    assert_eq!(budget.storage(), floor);
    let refusal = scoped_slot_uses_v29::check_scoped_source_slot_uses_v29(
        instances, emitted, slots, 1024, budget,
    );
    assert!(
        matches!(
            refusal,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "original raw source requires consuming expanded physical admission",
                ..
            })
        ),
        "{refusal:?}"
    );
    assert_eq!(budget.storage(), floor);
    // A changed no-return interval must not hide a result effect.
    let position = lowered
        .call_returns
        .sites
        .rows
        .iter()
        .position(|row| row.semantic_block.index() == 0)
        .unwrap();
    let lowered = emitted[item.instance.index()].as_mut().unwrap();
    let saved = lowered.call_returns.sites.rows[position];
    let SemanticKirCallReturnKindV1::NoNormalReturnCall { call_operation, .. } =
        &mut lowered.call_returns.sites.rows[position].kind
    else {
        unreachable!()
    };
    *call_operation += 1;
    assert!(matches!(
        check_scoped_defined_call_phases_v29(instances, emitted, budget),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    emitted[item.instance.index()]
        .as_mut()
        .unwrap()
        .call_returns
        .sites
        .rows[position] = saved;
    check_scoped_defined_call_phases_v29(instances, emitted, budget)?;
    let child = original.child().unwrap();
    assert!(
        instances
            .instance(child)
            .unwrap()
            .ssa()
            .plan()
            .entry_arguments()
            .is_empty()
    );
    let child_output = emitted[child.index()].as_ref().unwrap();
    let entry = child_output.invocation_entry.as_ref().unwrap();
    assert!(entry.layout.preheader.is_some());
    assert!(entry.arguments.is_empty());
    assert!(entry.components.is_empty());
    assert!(entry.inputs_retained);
    assert!(!entry.inputs.is_empty());
    assert_eq!(
        entry
            .inputs
            .iter()
            .map(|row| row.parameter_count)
            .sum::<usize>(),
        child_output.function.signature.parameters.len()
    );
    let inputs = entry.inputs.clone();
    for fault in 0..3 {
        let actual = &mut emitted[child.index()]
            .as_mut()
            .unwrap()
            .invocation_entry
            .as_mut()
            .unwrap()
            .inputs;
        match fault {
            0 => actual.clear(),
            1 => actual[0].source_argument += 1,
            2 => actual[0].parameter_count += 1,
            _ => unreachable!(),
        }
        let child_output = emitted[child.index()].as_ref().unwrap();
        let raw = invocation_checked_prefix_v1(
            instances.instance(child).unwrap(),
            slots.source.root,
            child_output,
            budget,
        );
        assert!(
            matches!(
                raw,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "invocation entry differs from its original source SSA plan",
                })
            ),
            "raw invocation fault {fault}: {raw:?}"
        );
        assert_eq!(budget.storage(), floor);
        let source_rows = instance_check_source_rows_with_control_v1(
            instances,
            child,
            slots.source.root,
            child_output,
            budget,
        );
        assert!(
            matches!(source_rows, Err(InstanceCorrespondenceErrorV1::CallAnchor)),
            "source-row fault {fault}: {source_rows:?}"
        );
        assert_eq!(budget.storage(), floor);
        let phases = check_scoped_defined_call_phases_v29(instances, emitted, budget);
        assert!(
            matches!(
                phases,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution call parameters differ from their source instance",
                })
            ),
            "enclosing call-phase fault {fault}: {phases:?}"
        );
        assert_eq!(budget.storage(), floor);
        emitted[child.index()]
            .as_mut()
            .unwrap()
            .invocation_entry
            .as_mut()
            .unwrap()
            .inputs = inputs.clone();
        check_scoped_defined_call_phases_v29(instances, emitted, budget)?;
        assert_eq!(budget.storage(), floor);
    }
    with_scoped_source_test_layouts_v29(
        source,
        ProductionSemanticKirLimitsV1::default(),
        budget,
        |demands, layouts, budget| {
            let roots = source.owner.source_semantic().roots();
            budget.charge_work(roots.len())?;
            let ordinal = roots
                .iter()
                .position(|root| *root == slots.source.root)
                .ok_or(ArgumentResourceV1::Accounting)?;
            let demands = demands.root_lens(source.owner, ordinal, budget)?;
            let descriptor = source
                .kernel_argument_abi
                .map(|profile| profile.descriptor_root(source.owner, ordinal, budget))
                .transpose()?;
            source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                layouts,
                instances,
                descriptor,
                demands,
                budget,
                |references, _, budget| {
                    let floor = budget.storage();
                    let check =
                        |emitted: &[Option<LoweredFunctionResultV1>],
                         budget: &mut ArgumentBudgetV1<'_>| {
                            check_scoped_defined_call_phases_with_references_v29(
                                instances,
                                emitted,
                                Some(references),
                                budget,
                            )
                        };
                    check(emitted, budget)?;
                    let prefix = invocation_checked_prefix_v1(
                        instances.instance(child).unwrap(),
                        slots.source.root,
                        emitted[child.index()].as_ref().unwrap(),
                        budget,
                    )?;
                    let entry = emitted[child.index()]
                        .as_mut()
                        .unwrap()
                        .invocation_entry
                        .as_mut()
                        .unwrap();
                    entry.inputs_retained = false;
                    entry.inputs.clear();
                    // The descriptive prefix permits the legacy absent roster;
                    // the original-source call boundary still requires it.
                    assert_eq!(
                        invocation_checked_prefix_v1(
                            instances.instance(child).unwrap(),
                            slots.source.root,
                            emitted[child.index()].as_ref().unwrap(),
                            budget,
                        )?,
                        prefix
                    );
                    let refused = check(emitted, budget);
                    let entry = emitted[child.index()]
                        .as_mut()
                        .unwrap()
                        .invocation_entry
                        .as_mut()
                        .unwrap();
                    entry.inputs_retained = true;
                    entry.inputs = inputs.clone();
                    assert!(
                        matches!(
                            refused,
                            Err(ProductionSemanticKirErrorV1::Unsupported {
                                function: 0,
                                block: None,
                                statement: None,
                                detail: "invocation entry differs from its original source SSA plan",
                            })
                        ),
                        "forged absent input roster at original-source boundary: {refused:?}"
                    );
                    assert_eq!(budget.storage(), floor);
                    check(emitted, budget)?;
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
        },
    )?;
    assert_eq!(budget.storage(), floor);
    COMPLETED.set(true);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn typed_no_normal_call_prepares_original_address_without_result_effect_or_memory_authority() {
    struct Reset(bool, bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            scoped_root_tests::fixtures::CALL_DESTINATIONS_NO_NORMAL_V29.set(self.0);
            scoped_root_tests::ORIGINAL_TYPED_SOURCE_ENTRY_V29.set(self.1);
        }
    }
    let _reset = Reset(
        scoped_root_tests::fixtures::CALL_DESTINATIONS_NO_NORMAL_V29.replace(true),
        scoped_root_tests::ORIGINAL_TYPED_SOURCE_ENTRY_V29.replace(false),
    );
    COMPLETED.set(false);
    let fixture = ScopedFixture::CallDestinations {
        projected: true,
        retained_address: false,
        indexed: false,
    };
    let (legacy, _, _) = run(false, fixture, inspect_no_normal, LIMIT, LIMIT);
    assert!(
        matches!(
            legacy,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: STOP,
            })
        ),
        "{legacy:?}"
    );
    // This wrapper already derives the original demands/layouts. The observer
    // independently requires the unchanged final-memory-authority refusal.
    assert_eq!(OBSERVED.get(), 1);
    assert!(COMPLETED.get());
    COMPLETED.set(false);
    scoped_root_tests::ORIGINAL_TYPED_SOURCE_ENTRY_V29.set(true);
    let (result, _, _) = run(false, fixture, inspect_no_normal, LIMIT, LIMIT);
    assert!(COMPLETED.get(), "{result:?}");
    assert_eq!(OBSERVED.get(), 1);
    assert!(is_stopped(&result), "{result:?}");
}

#[test]
fn typed_aggregate_copy_and_move_operands_reach_original_no_normal_call_observer() {
    struct Reset(bool, bool, u8);
    impl Drop for Reset {
        fn drop(&mut self) {
            scoped_root_tests::fixtures::CALL_DESTINATIONS_NO_NORMAL_V29.set(self.0);
            scoped_root_tests::ORIGINAL_TYPED_SOURCE_ENTRY_V29.set(self.1);
            scoped_root_tests::fixtures::CALL_DESTINATIONS_AGGREGATE_OPERAND_V29.set(self.2);
        }
    }
    let _reset = Reset(
        scoped_root_tests::fixtures::CALL_DESTINATIONS_NO_NORMAL_V29.replace(true),
        scoped_root_tests::ORIGINAL_TYPED_SOURCE_ENTRY_V29.replace(true),
        scoped_root_tests::fixtures::CALL_DESTINATIONS_AGGREGATE_OPERAND_V29.replace(0),
    );
    for mode in [1, 2] {
        scoped_root_tests::fixtures::CALL_DESTINATIONS_AGGREGATE_OPERAND_V29.set(mode);
        COMPLETED.set(false);
        let fixture = ScopedFixture::CallDestinations {
            projected: true,
            retained_address: false,
            indexed: false,
        };
        let (result, _, _) = run(false, fixture, inspect_no_normal, LIMIT, LIMIT);
        assert!(COMPLETED.get(), "aggregate operand mode {mode}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
        assert!(is_stopped(&result), "{result:?}");
    }
}

pub(in super::super) fn complete_original_source_again(
    original: &ExecutionLifecycleSourceV29<'_>,
    retained_address: bool,
    callback_instances: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSourceOwnedViewErrorV18> {
    struct RestoreObserver(Option<ScopedSlotObserverV29>);
    impl Drop for RestoreObserver {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _observer = RestoreObserver(SCOPED_SLOT_OBSERVER_V29.replace(None));
    // This is a separate complete production re-emission of the same original
    // source, not admission of an observer-mutated candidate or its sidecars.
    let owner = scoped_root_tests::fixtures::call_destinations_owner(true, retained_address, false);
    assert_eq!(
        owner.source_semantic_sha256(),
        original.owner.source_semantic_sha256()
    );
    assert_eq!(owner.identity(), original.owner.identity());
    let launch = ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "lifecycle_fixture",
            [88; 32],
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap();
    let floor = budget.storage();
    let mut completed = false;
    let outcome = (|| -> Result<(), ProductionSourceOwnedViewErrorV18> {
        let prepared = ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
            owner,
            launch,
            original.input,
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?;
        prepared.with_source_consumer_v18(budget, |source, budget| {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        scoped_raw_admission_v29::with_checked_source_memory_v29(
                            relation, 0, None, budget,
                            |physical, budget| -> Result<(), ProductionSourceOwnedViewErrorV18> {
                                let root = source.root(0, budget)?.1;
                                let body = inventory.functions()[root].function.body.as_ref().unwrap();
                                let mut typed_results = 0;
                                let mut raw_results = 0;
                                let mut local_results = 0;
                                for (block_index, block) in body.blocks.iter().enumerate() {
                                    for (operation_index, operation) in block.operations.iter().enumerate() {
                                        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                                            address, value, access,
                                        }) = operation.kind else { continue; };
                                        let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(root as u32),
                                                block: block_index as u32,
                                            },
                                            operation: operation_index as u32,
                                        };
                                        let payload = relation.retained_object_payload_v29(0, coordinate, budget)?
                                            .expect("actual typed write keeps its original source attachment");
                                        let ScopedObjectRoleV29::WriteValue {
                                            destination,
                                            value: ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::CallResult { site, ty }),
                                        } = payload.source.role else { continue; };
                                        assert_eq!(ty, U32);
                                        assert_eq!(destination.projected_type, ty);
                                        let address_type = inventory.definition_for_value(coordinate.block.function, address, budget)
                                            .map_err(fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory)?
                                            .expect("the exact typed destination has an actual definition").ty;
                                        assert!(matches!(address_type, Type::Pointer(pointer)
                                            if pointer.pointee.as_ref() == &Type::StorageObject(destination.projected_schema)
                                                && pointer.address_space == AddressSpace::Private
                                                && pointer.access == AccessMode::ReadWrite));
                                        let value_type = inventory.definition_for_value(coordinate.block.function, value, budget)
                                            .map_err(fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory)?
                                            .expect("the original call result has an actual definition").ty;
                                        assert_eq!(value_type, &Type::Scalar(ScalarType::U32));
                                        assert!(operation.results.is_empty());
                                        assert_eq!(payload.actual.result, None);
                                        assert_eq!(payload.actual.operation, match operation.kind {
                                            OperationKind::Storage(operation) => operation,
                                            _ => unreachable!(),
                                        });
                                        let actual = physical.access(payload.instance, payload.row, coordinate, address, budget)?;
                                        match destination.object {
                                            ScopedObjectIdentityV29::Reference { instance, site: original, role, dereference_prefix } => {
                                                assert_eq!(instance.index(), payload.instance);
                                                assert_eq!(original, site);
                                                assert_eq!(site, execution_site_v29(SemanticBlockIdV1::from_index(0), None));
                                                assert_eq!(role, ExecutionOperandV29::CallDestinationAddress);
                                                assert_eq!(dereference_prefix, 1);
                                                assert_eq!(destination.source, ScopedObjectSourceV29::Place {
                                                    site, role, local: SemanticLocalIdV1::from_index(4), prefix: 1,
                                                });
                                                assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 1));
                                                let actual = actual.expect("original raw destination keeps its checked activation alternatives");
                                                assert_eq!(actual.operation_pointer(budget)?, (coordinate, address));
                                                raw_results += 1;
                                            }
                                            ScopedObjectIdentityV29::Local { instance, local, generation } => {
                                                assert_eq!(instance.index(), payload.instance);
                                                assert_eq!(local.index(), 3);
                                                assert_eq!(generation, 0);
                                                assert_eq!(site, execution_site_v29(SemanticBlockIdV1::from_index(2), None));
                                                assert_eq!(destination.source, ScopedObjectSourceV29::Place {
                                                    site, role: ExecutionOperandV29::CallDestinationAddress, local, prefix: 0,
                                                });
                                                assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 4));
                                                // Gen0 direct Object backing now has its exact
                                                // Invocation alternative, independently of raw loans.
                                                let actual = actual.expect("direct call destination keeps its checked invocation activation");
                                                assert_eq!(actual.operation_pointer(budget)?, (coordinate, address));
                                                let mut alternatives = 0;
                                                actual.visit_alternatives(budget, |owner, original_local, slot, activation, _| {
                                                    assert_eq!((owner, original_local, activation), (payload.instance, local, None));
                                                    let backing = &source.root_row(0)?.source_slots.slots[slot];
                                                    assert_eq!(backing.instance, instance);
                                                    assert_eq!(backing.origin.identity.original_local(), Some(local.index()));
                                                    alternatives += 1;
                                                    Ok(())
                                                })?;
                                                assert_eq!(alternatives, 1);
                                                local_results += 1;
                                            }
                                            _ => panic!("call results must retain their exact original Reference or Local role"),
                                        }
                                        typed_results += 1;
                                    }
                                }
                                assert_eq!(typed_results, 2 * callback_instances);
                                assert_eq!((raw_results, local_results), (callback_instances, callback_instances));
                                completed = true;
                                Ok(())
                            },
                        )
                    })
                })
            })
        })
    })();
    assert_eq!(budget.storage(), floor, "{outcome:?}");
    outcome?;
    assert!(
        completed,
        "actual final physical admission callback must complete"
    );
    Ok(())
}

pub(super) fn inspect_and_continue(
    source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    inspect(instances, emitted, slots, budget)?;
    let callbacks: Vec<_> = slots
        .instances
        .iter()
        .filter(|row| row.function == CALLBACK)
        .collect();
    let retained_address = slots.slots.iter().any(|slot| {
        callbacks.iter().any(|row| row.instance == slot.instance)
            && slot.origin.identity.original_local() == Some(4)
    });
    complete_original_source_again(source, retained_address, callbacks.len(), budget)
        .expect("same original source must complete consuming physical admission");
    COMPLETED.set(true);
    Ok(())
}

#[test]
fn typed_projected_and_whole_call_results_reach_complete_source_admission() {
    for retained_address in [false, true] {
        COMPLETED.set(false);
        let fixture = ScopedFixture::CallDestinations {
            projected: true,
            retained_address,
            indexed: false,
        };
        let (result, _, _) = run(false, fixture, inspect_and_continue, LIMIT, LIMIT);
        assert!(COMPLETED.get(), "retained={retained_address}: {result:?}");
        assert_eq!(OBSERVED.get(), 1);
        result.unwrap();
    }
}
