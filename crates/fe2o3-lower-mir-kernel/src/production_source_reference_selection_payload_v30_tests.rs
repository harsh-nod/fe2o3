thread_local! {
    static SELECTED_PAYLOAD_MODE_V30: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SELECTED_PAYLOAD_SEEN_V30: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct SelectedPayloadObserverV30(Option<ScopedSlotObserverV29>);

impl SelectedPayloadObserverV30 {
    fn install(mode: u8) -> Self {
        SELECTED_PAYLOAD_MODE_V30.set(mode);
        SELECTED_PAYLOAD_SEEN_V30.set(0);
        Self(SCOPED_SLOT_OBSERVER_V29.replace(Some(selected_payload_observer_v30)))
    }
}

impl Drop for SelectedPayloadObserverV30 {
    fn drop(&mut self) {
        SCOPED_SLOT_OBSERVER_V29.set(self.0);
    }
}

fn selected_payload_observer_v30(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let item = slots
        .instances
        .iter()
        .find(|item| item.function.index() == 1)
        .unwrap();
    let original = instances.instance(item.instance).unwrap().declaration();
    let lowered = emitted[item.instance.index()].as_mut().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
    let (ordinal, row) = anchors
        .rows
        .iter()
        .enumerate()
        .find(|(_, row)| {
            matches!(
                row.kind,
                ScopedMemoryAnchorKindV29::Access {
                    payload: Some(ScopedMemoryPayloadV29::Store {
                        source: ScopedMemoryStoreSourceV29::Operand {
                            role: ExecutionOperandV29::StoreValue,
                            source: ScopedMemoryOperandSourceV29::Memory { .. },
                            ..
                        },
                        ..
                    }),
                    ..
                }
            )
        })
        .map(|(ordinal, row)| (ordinal, *row))
        .expect("actual external Store must retain its typed read producer");
    let ScopedMemoryAnchorKindV29::Access {
        payload:
            Some(ScopedMemoryPayloadV29::Store {
                value,
                source:
                    ScopedMemoryStoreSourceV29::Operand {
                        site,
                        role,
                        ty,
                        source: ScopedMemoryOperandSourceV29::Memory { occurrence, access },
                    },
            }),
        ..
    } = row.kind
    else {
        unreachable!()
    };
    let prior = anchors.rows[access];
    let ScopedMemoryAnchorKindV29::Object(object) = prior.kind else {
        panic!("genuine address-taken scalar must use typed ReadValue");
    };
    let payload = anchors.objects[object];
    let ScopedObjectRoleV29::ReadValue {
        source,
        read: ScopedObjectReadOriginV29::Original(read),
    } = payload.role
    else {
        panic!("typed read must retain exact original StoreValue occurrence");
    };
    assert_eq!(read.site, site);
    assert_eq!(read.role, role);
    assert_eq!(read.prefix, 0);
    assert_eq!(read.occurrence, occurrence);
    assert_eq!(source.root_type, source.projected_type);
    assert_eq!(source.source_path.count, 0);
    assert_eq!(source.path.count, 0);
    assert_eq!(payload.result, Some(value));
    assert!(matches!(
        payload.operation,
        ScopedObjectOperationV29::ReadValue { .. }
    ));
    let block = lowered
        .function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| block.id == prior.block)
        .unwrap();
    assert!(matches!(
        block.operations[prior.position].kind,
        OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
    ));
    assert!(
        matches!(block.operations[row.position].kind, OperationKind::Store { value: actual, .. } if actual == value)
    );
    let floor = budget.storage();
    let work = budget.work();
    check_scoped_payload_memory_v29(
        original, anchors, ordinal, &row, value, site, role, ty, occurrence, access, budget,
    )?;
    assert_eq!(budget.storage(), floor);
    // 12 exact source/order checks + 3 object lookup + 8 typed role checks
    // + 3 empty original-path lookup; no scan of another operation/root.
    assert_eq!(budget.work() - work, 26);
    SELECTED_PAYLOAD_SEEN_V30.set(SELECTED_PAYLOAD_SEEN_V30.get() + 1);
    match SELECTED_PAYLOAD_MODE_V30.get() {
        0 => (),
        1 => anchors.objects[object].result = Some(ValueId(u32::MAX)),
        2..=4 => {
            let ScopedObjectRoleV29::ReadValue {
                read: ScopedObjectReadOriginV29::Original(read),
                ..
            } = &mut anchors.objects[object].role
            else {
                unreachable!()
            };
            match SELECTED_PAYLOAD_MODE_V30.get() {
                2 => read.role = ExecutionOperandV29::StoreDestination,
                3 => read.prefix += 1,
                4 => read.occurrence = ScopedMemoryOccurrenceV29::Retained { event: usize::MAX },
                _ => unreachable!(),
            }
        }
        5 => {
            anchors.objects[object].role = ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::ValueTransfer,
            }
        }
        6 => anchors.rows[access].position = row.position,
        7 => anchors.rows[access].block = BlockId(u32::MAX),
        8 => {
            let ScopedMemoryAnchorKindV29::Access {
                payload:
                    Some(ScopedMemoryPayloadV29::Store {
                        source:
                            ScopedMemoryStoreSourceV29::Operand {
                                source: ScopedMemoryOperandSourceV29::Memory { access, .. },
                                ..
                            },
                        ..
                    }),
                ..
            } = &mut anchors.rows[ordinal].kind
            else {
                unreachable!()
            };
            *access = ordinal;
        }
        9 => {
            let ScopedObjectOperationV29::ReadValue { address, access } = payload.operation else {
                unreachable!()
            };
            anchors.objects[object].operation = ScopedObjectOperationV29::WriteValue {
                address,
                value,
                access,
            };
        }
        _ => unreachable!(),
    }
    Ok(())
}

#[test]
fn selected_typed_read_payload_reaches_actual_promoting_aggregate_consumer() {
    let _observer = SelectedPayloadObserverV30::install(0);
    selected_aggregate_actual_consumer_joins_nonempty_selected_graph_after_live_private_promotion();
    assert!(SELECTED_PAYLOAD_SEEN_V30.get() > 0);
}

#[test]
fn selected_typed_read_payload_rejects_changed_result_occurrence_kind_and_order() {
    for mode in 1..=9 {
        let _observer = SelectedPayloadObserverV30::install(mode);
        let reached = std::cell::Cell::new(false);
        let (result, _, _) = run_selected_memory_owner(
            selected_aggregate_owner_v30(),
            SELECTED_MEMORY_LIMIT,
            SELECTED_MEMORY_LIMIT,
            |_, _| {
                reached.set(true);
                Ok(())
            },
        );
        assert!(
            result.is_err(),
            "changed genuine typed read payload mode {mode} was admitted"
        );
        assert!(
            !reached.get(),
            "corrupt source payload reached correspondence consumer"
        );
        assert!(
            SELECTED_PAYLOAD_SEEN_V30.get() > 0,
            "mutation was not exercised"
        );
    }
}

#[test]
fn selected_typed_read_payload_has_exact_and_one_short_consumer_resources() {
    let execute = |work, storage| {
        let _observer = SelectedPayloadObserverV30::install(0);
        run_selected_memory_owner(
            selected_aggregate_owner_v30(),
            work,
            storage,
            |original, _| {
                let rows = retained_selected_memory(original);
                assert_eq!(rows.selected.len(), 2);
                assert_eq!(rows.selected.iter().filter(|row| row.writing).count(), 1);
                assert!(SELECTED_PAYLOAD_SEEN_V30.get() > 0);
                Ok(())
            },
        )
    };
    let (result, work, storage) = execute(SELECTED_MEMORY_LIMIT, SELECTED_MEMORY_LIMIT);
    result.unwrap();
    assert!(work > 0 && storage > 0);
    execute(work, storage).0.unwrap();
    assert!(execute(work - 1, storage).0.is_err());
    assert!(execute(work, storage - 1).0.is_err());
}
