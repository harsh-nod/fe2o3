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
    let _issued = SelectedIssuedPayloadObserverV30::install(0);
    selected_aggregate_actual_consumer_joins_nonempty_selected_graph_after_live_private_promotion();
    assert!(SELECTED_PAYLOAD_SEEN_V30.get() > 0);
    assert!(SELECTED_ISSUED_PAYLOAD_SEEN_V30.get() > 0);
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
        let _issued = SelectedIssuedPayloadObserverV30::install(0);
        run_selected_memory_owner(
            selected_aggregate_owner_v30(),
            work,
            storage,
            |original, _| {
                let rows = retained_selected_memory(original);
                assert_eq!(rows.selected.len(), 2);
                assert_eq!(rows.selected.iter().filter(|row| row.writing).count(), 1);
                assert!(SELECTED_PAYLOAD_SEEN_V30.get() > 0);
                assert!(SELECTED_ISSUED_PAYLOAD_SEEN_V30.get() > 0);
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

thread_local! {
    static SELECTED_ISSUED_PAYLOAD_MODE_V30: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SELECTED_ISSUED_PAYLOAD_SEEN_V30: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct SelectedIssuedPayloadObserverV30(Option<SourceIssuedMemoryPayloadObserverV30>);

impl SelectedIssuedPayloadObserverV30 {
    fn install(mode: u8) -> Self {
        SELECTED_ISSUED_PAYLOAD_MODE_V30.set(mode);
        SELECTED_ISSUED_PAYLOAD_SEEN_V30.set(0);
        SOURCE_ISSUED_MEMORY_PAYLOAD_QUERY_FAULT_V30.set(0);
        Self(
            SOURCE_ISSUED_MEMORY_PAYLOAD_OBSERVER_V30
                .replace(Some(selected_issued_payload_observer_v30)),
        )
    }
}

impl Drop for SelectedIssuedPayloadObserverV30 {
    fn drop(&mut self) {
        SOURCE_ISSUED_MEMORY_PAYLOAD_QUERY_FAULT_V30.set(0);
        SOURCE_ISSUED_MEMORY_PAYLOAD_OBSERVER_V30.set(self.0);
    }
}

fn selected_issued_payload_observer_v30(
    original: &SourceIssuedOriginalV29<'_, '_, '_>,
    anchor: usize,
    row: &ScopedMemoryAnchorV29,
    operation: &Operation,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // This hook is reached only after the real original/source pointer checks.
    // Mutations below target the later Memory payload boundary itself.
    let floor = budget.storage();
    let mode = SELECTED_ISSUED_PAYLOAD_MODE_V30.get();
    let result = match mode {
        0 => {
            check_source_issued_memory_payload_v30(original, anchor, row, operation, actual, budget)
        }
        1 => check_source_issued_memory_payload_v30(
            original,
            usize::MAX,
            row,
            operation,
            actual,
            budget,
        ),
        2 => {
            let foreign = *row;
            assert!(!std::ptr::eq(&foreign, row));
            check_source_issued_memory_payload_v30(
                original, anchor, &foreign, operation, actual, budget,
            )
        }
        3 => {
            let foreign = operation.clone();
            assert!(!std::ptr::eq(&foreign, operation));
            check_source_issued_memory_payload_v30(original, anchor, row, &foreign, actual, budget)
        }
        4 => {
            let ScopedMemoryAnchorKindV29::Access {
                payload: Some(ScopedMemoryPayloadV29::Store { value, .. }),
                ..
            } = row.kind
            else {
                panic!("genuine Memory-fed Store");
            };
            let prior = actual.value(value, budget)?.operation.unwrap().clone();
            let mut foreign = SourceIssuedActualV29 {
                values: actual.values.clone(),
                root_arguments: actual.root_arguments.clone(),
            };
            foreign
                .values
                .iter_mut()
                .find(|row| row.id == value)
                .unwrap()
                .operation = Some(&prior);
            check_source_issued_memory_payload_v30(
                original, anchor, row, operation, &foreign, budget,
            )
        }
        5..=9 => {
            SOURCE_ISSUED_MEMORY_PAYLOAD_QUERY_FAULT_V30.set(mode - 4);
            let result = check_source_issued_memory_payload_v30(
                original, anchor, row, operation, actual, budget,
            );
            SOURCE_ISSUED_MEMORY_PAYLOAD_QUERY_FAULT_V30.set(0);
            result
        }
        10..=13 => {
            check_source_issued_memory_payload_v30(
                original, anchor, row, operation, actual, budget,
            )?;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut scratch = ArgumentBudgetV1::new(&mut work, 0);
            let resource = match mode {
                10 => Some(scratch.charge_work(1).unwrap_err()),
                11 => Some(scratch.reserve_storage(1).unwrap_err()),
                _ => None,
            };
            let selected = match resource {
                Some(resource) => Err(resource.into()),
                None if mode == 12 => Err(source_issued_error_v29()),
                None => Ok(()),
            };
            // Real underflow leaves the budget unchanged. This targets the
            // exact production settlement helper without fabricating custody.
            let result =
                settle_source_issued_memory_payload_v30(original, selected, usize::MAX, budget);
            match resource {
                Some(expected) => {
                    assert!(
                        matches!(&result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(actual)) if *actual == expected)
                    );
                    assert_eq!(original.semantic.failure.get(), Some(expected));
                }
                None if mode == 12 => {
                    assert!(matches!(
                        &result,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "source issued pointer differs from its original issuer or actual guard",
                            ..
                        })
                    ));
                    assert_eq!(
                        original.semantic.failure.get(),
                        Some(ArgumentResourceV1::Accounting)
                    );
                }
                None => {
                    assert!(matches!(
                        &result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!(
                        original.semantic.failure.get(),
                        Some(ArgumentResourceV1::Accounting)
                    );
                }
            }
            result
        }
        _ => unreachable!(),
    };
    assert_eq!(budget.storage(), floor, "known helper credit must settle");
    SELECTED_ISSUED_PAYLOAD_SEEN_V30.set(SELECTED_ISSUED_PAYLOAD_SEEN_V30.get() + 1);
    if mode != 0 {
        assert!(
            result.is_err(),
            "changed issued Memory boundary mode {mode} was accepted"
        );
    }
    result
}

#[test]
fn selected_issued_memory_payload_refuses_foreign_rows_operations_and_read_queries_at_consumer() {
    for mode in 1..=9 {
        let _observer = SelectedIssuedPayloadObserverV30::install(mode);
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
        assert!(result.is_err());
        assert!(
            !reached.get(),
            "hostile issued Memory query reached source consumer"
        );
        assert!(
            SELECTED_ISSUED_PAYLOAD_SEEN_V30.get() > 0,
            "mutation did not reach issued Memory boundary"
        );
    }
}

#[test]
fn selected_issued_memory_payload_header_has_independent_coexisting_field_oracle() {
    macro_rules! field {
        ($ty:ty) => {
            std::mem::size_of::<$ty>()
                + 2 * std::mem::size_of::<Result<$ty, ProductionSemanticKirErrorV1>>()
        };
    }
    let expected = field!(ScopedMemoryPayloadV29)
        + field!(SourceAddressValueAccessV29)
        + field!(SourceIssuedActualValueV29<'_>)
        + field!(Option<SourceAddressValueAccessV29>)
        + field!(&ScopedMemoryAnchorsV29)
        + field!(&ScopedMemoryAnchorV29)
        + field!(&Operation)
        + field!(Type)
        + field!(ScopedMemoryOccurrenceV29)
        + field!(ExecutionSiteV29)
        + field!(ExecutionOperandV29)
        + field!(SemanticTypeIdV1)
        + field!(ValueId)
        + field!(())
        + field!(Result<(), ProductionSemanticKirErrorV1>)
        + 12 * std::mem::size_of::<&()>()
        + 8 * std::mem::size_of::<usize>();
    assert_eq!(
        source_issued_memory_payload_headers_v30().unwrap(),
        expected
    );
}

#[test]
fn selected_issued_memory_payload_settlement_preserves_first_refusal_and_retains_cleanup_poison() {
    for mode in 10..=13 {
        let _observer = SelectedIssuedPayloadObserverV30::install(mode);
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
        assert!(result.is_err());
        assert!(!reached.get());
        assert!(
            SELECTED_ISSUED_PAYLOAD_SEEN_V30.get() > 0,
            "settlement control was not exercised"
        );
    }
}
