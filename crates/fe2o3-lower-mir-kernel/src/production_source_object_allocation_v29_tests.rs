#[test]
fn row_keyed_object_query_refuses_scalar_foreign_and_inactive_source_rows_stickily() {
    for fault in 0..7 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(active_shared_target_owner_v18, &mut budget);
        let reached = std::cell::Cell::new(false);
        let first = std::cell::RefCell::new(None);
        let result = prepared.with_source_consumer_v18(&mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                    assert!(!source.instance_active(0, 2, budget)?);
                    assert!(source.instance_active(0, 3, budget)?);
                    let original = relation.attachments.iter().find(|row| {
                        row.key.root == 0 && row.key.instance == 3
                            && row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                            && row.key.field == TileAttachmentFieldV29::MemoryPosition
                            && matches!(row.location, TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(_)))
                    }).expect("the real later helper must have an actual scalar memory row");
                    let ProductionSourceOperationV18::Operation(operation) =
                        relation.mapped_source_operation(original.location, budget)?
                        else { panic!("original memory position must be an operation"); };
                    let (mut root, mut instance, mut row) = (0, 3, original.key.row);
                    let mut expected = operation;
                    match fault {
                        0 => {}
                        1 => root = usize::MAX,
                        2 => expected.block.function.0 = u32::MAX,
                        3 => row = usize::MAX,
                        4 => instance = usize::MAX,
                        5 => instance = 2,
                        6 => expected.operation = u32::MAX,
                        _ => unreachable!(),
                    }
                    let error = match relation.retained_object_payload_at_v29(root, instance, row, expected, budget) {
                        Err(error) => error,
                        Ok(_) => panic!("a scalar/foreign/inactive row acquired typed payload authority"),
                    };
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
                    let text = format!("{error:?}");
                    let stopped = budget.work();
                    assert_eq!(format!("{:?}", source.root_count(budget).unwrap_err()), text);
                    assert!(relation.retained_object_payload_at_v29(0, 3, original.key.row, operation, budget).is_err());
                    assert_eq!(budget.work(), stopped, "sticky refusal must stop valid retries before work");
                    *first.borrow_mut() = Some(text);
                    reached.set(true);
                    // Ignoring both errors must not allow outer success.
                    Ok(())
                })
            }))
        });
        assert!(reached.get());
        assert_eq!(
            format!("{:?}", result.unwrap_err()),
            first.into_inner().unwrap()
        );
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(budget.failed_storage(), None);
    }
}

#[test]
fn row_keyed_object_query_checks_the_original_ledger_before_spending_work() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_v18(&mut budget);
    let reached = std::cell::Cell::new(false);
    let denied_floor = std::cell::Cell::new(None);
    let result = prepared.with_source_consumer_v18(
        &mut budget,
        |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        let original = relation
                            .attachments
                            .iter()
                            .find(|row| {
                                row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                                    && row.key.field == TileAttachmentFieldV29::MemoryPosition
                                    && matches!(
                                        row.location,
                                        TileAttachmentLocationV29::Origin(
                                            TileScalarSourceV29::Operation(_)
                                        )
                                    )
                            })
                            .expect("actual original scalar memory row");
                        let ProductionSourceOperationV18::Operation(operation) =
                            relation.mapped_source_operation(original.location, budget)?
                        else {
                            panic!("actual original operation");
                        };
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
                        denied_floor.set(Some(budget.storage()));
                        assert!(matches!(
                            relation.retained_object_payload_at_v29(
                                original.key.root,
                                original.key.instance,
                                original.key.row,
                                operation,
                                &mut foreign
                            ),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                        let stopped = budget.work();
                        assert!(matches!(
                            relation.retained_object_payload_at_v29(
                                original.key.root,
                                original.key.instance,
                                original.key.row,
                                operation,
                                budget
                            ),
                            Err(ProductionSourceOwnedViewErrorV18::Resource(
                                ArgumentResourceV1::Accounting
                            ))
                        ));
                        assert_eq!(budget.work(), stopped);
                        reached.set(true);
                        Ok(())
                    })
                })
            })
        },
    );
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(Some(budget.storage()), denied_floor.get());
    assert!(
        budget.storage() > MODULE_FLOOR,
        "denied custody cannot authorize containing refunds"
    );
}

#[test]
fn row_keyed_object_query_has_an_independent_one_work_entry_boundary() {
    for available in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_v18(&mut budget);
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(
            &mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            |relation, budget| {
                                let original = relation
                                    .attachments
                                    .iter()
                                    .find(|row| {
                                        row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                                            && row.key.field
                                                == TileAttachmentFieldV29::MemoryPosition
                                            && matches!(
                                                row.location,
                                                TileAttachmentLocationV29::Origin(
                                                    TileScalarSourceV29::Operation(_)
                                                )
                                            )
                                    })
                                    .expect("actual scalar row");
                                let ProductionSourceOperationV18::Operation(operation) =
                                    relation.mapped_source_operation(original.location, budget)?
                                else {
                                    panic!("actual scalar operation");
                                };
                                budget.charge_work(MODULE_LIMIT - budget.work() - available)?;
                                let before = budget.work();
                                let floor = budget.storage();
                                let error = match relation.retained_object_payload_at_v29(
                                    usize::MAX,
                                    original.key.instance,
                                    original.key.row,
                                    operation,
                                    budget,
                                ) {
                                    Err(error) => error,
                                    Ok(_) => {
                                        panic!("foreign root cannot produce an Object payload")
                                    }
                                };
                                if available == 0 {
                                    assert!(matches!(
                                        error,
                                        ProductionSourceOwnedViewErrorV18::Resource(
                                            ArgumentResourceV1::Work(_)
                                        )
                                    ));
                                } else {
                                    assert!(matches!(
                                        error,
                                        ProductionSourceOwnedViewErrorV18::Binding("root ordinal")
                                    ));
                                }
                                assert_eq!(budget.work() - before, available);
                                assert_eq!(
                                    budget.storage(),
                                    floor,
                                    "the row query owns no additional arena or header"
                                );
                                reached.set(true);
                                Err(error)
                            },
                        )
                    })
                })
            },
        );
        assert!(reached.get());
        assert!(result.is_err());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn allocation_identity_order_preserves_every_source_role_without_sentinel_aliases() {
    let roles = [
        ExecutionOperandV29::RvalueOperand(0),
        ExecutionOperandV29::RvalueOperand(u32::MAX),
        ExecutionOperandV29::RvaluePlace,
        ExecutionOperandV29::Destination,
        ExecutionOperandV29::StoreValue,
        ExecutionOperandV29::StoreDestination,
        ExecutionOperandV29::AtomicAddress,
        ExecutionOperandV29::AtomicValue,
        ExecutionOperandV29::AtomicExpected,
        ExecutionOperandV29::AtomicReplacement,
        ExecutionOperandV29::AtomicDestination,
        ExecutionOperandV29::StatementPlace,
        ExecutionOperandV29::Assume,
        ExecutionOperandV29::StorageLive,
        ExecutionOperandV29::StorageDead,
        ExecutionOperandV29::CallArgument(0),
        ExecutionOperandV29::CallArgument(u32::MAX),
        ExecutionOperandV29::CallDestinationAddress,
        ExecutionOperandV29::TailCallArgument(0),
        ExecutionOperandV29::TailCallArgument(u32::MAX),
        ExecutionOperandV29::SwitchDiscriminant,
        ExecutionOperandV29::DropPlace,
        ExecutionOperandV29::AssertCondition,
        ExecutionOperandV29::AssertMessage(0),
        ExecutionOperandV29::AssertMessage(u32::MAX),
        ExecutionOperandV29::ReturnValue,
        ExecutionOperandV29::ElidedBorrowDestination,
    ];
    let sites = [
        ExecutionSiteV29::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
            statement: 0,
        },
        ExecutionSiteV29::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
            statement: u32::MAX,
        },
        ExecutionSiteV29::Terminator {
            block: fe2o3_mir_model::SsaBlockIdV1::new(0),
        },
        ExecutionSiteV29::Terminator {
            block: fe2o3_mir_model::SsaBlockIdV1::new(u32::MAX),
        },
    ];
    let mut identities = Vec::new();
    for local in [0, u32::MAX] {
        identities.push(ScopedAllocationIdentityV29::LegacyLocal(local));
        for generation in [0, u32::MAX] {
            identities.push(ScopedAllocationIdentityV29::OriginalObject { local, generation });
            identities.push(ScopedAllocationIdentityV29::EntryValue {
                local,
                argument: generation,
            });
        }
    }
    for site in sites {
        for ordinal in [0, u32::MAX] {
            identities.push(ScopedAllocationIdentityV29::CallResultDestination { site, ordinal });
            identities.push(ScopedAllocationIdentityV29::ReturnDestination { site, ordinal });
            for role in roles {
                identities.push(ScopedAllocationIdentityV29::OperandSnapshot {
                    site,
                    role,
                    ordinal,
                });
            }
        }
    }
    let unique: BTreeSet<_> = identities.iter().copied().collect();
    assert_eq!(unique.len(), identities.len());
    for left in &identities {
        for right in &identities {
            assert_eq!(left.cmp(right).is_eq(), left == right);
            assert_eq!(left.cmp(right), right.cmp(left).reverse());
        }
        assert_eq!(
            left.legacy_local().is_ok(),
            matches!(left, ScopedAllocationIdentityV29::LegacyLocal(_))
        );
        assert_eq!(
            left.original_local().is_some(),
            matches!(
                left,
                ScopedAllocationIdentityV29::LegacyLocal(_)
                    | ScopedAllocationIdentityV29::OriginalObject { .. }
            )
        );
    }
}

#[test]
fn object_storage_has_no_scalar_array_or_legacy_local_authority() {
    let schema = fe2o3_kernel_ir::StorageLayoutIdV1(17);
    let storage = SemanticRetainedStorageV29::Object {
        cell: 3,
        schema,
        bytes: 48,
        alignment: 16,
    };
    assert_eq!(storage.value_type(), Type::StorageObject(schema));
    assert_eq!(storage.alignment(), 16);
    assert!(storage.scalar_array().is_err());
    let physical = ScopedSlotRepresentationV29::Object {
        schema,
        bytes: 48,
        alignment: 16,
    };
    assert_eq!(physical.bytes(), 48);
    assert_eq!(physical.count(), None);
    assert!(physical.scalar_array().is_err());
    assert!(
        ScopedAllocationIdentityV29::OriginalObject {
            local: 0,
            generation: 0
        }
        .legacy_local()
        .is_err()
    );
}

#[test]
fn typed_alloca_shape_checks_both_result_and_allocation_representation() {
    let pointer = ValueId(91);
    let schema = fe2o3_kernel_ir::StorageLayoutIdV1(17);
    let original = Operation::effect_free(
        ValueDef::new(
            pointer,
            Type::pointer(
                Type::StorageObject(schema),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::StorageObject(schema),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 16,
        },
    );
    for mutation in 0..11 {
        let mut operation = original.clone();
        match mutation {
            0 => {}
            1 => operation.results.clear(),
            2 => operation.results.push(operation.results[0].clone()),
            3 => operation.results[0].id = ValueId(92),
            4 => {
                operation.results[0].ty = Type::pointer(
                    Type::StorageObject(schema),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
            }
            5 => {
                operation.results[0].ty = Type::pointer(
                    Type::StorageObject(schema),
                    AddressSpace::Private,
                    AccessMode::ReadOnly,
                )
            }
            6 => {
                operation.results[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                )
            }
            7 => {
                if let OperationKind::Alloca { alignment, .. } = &mut operation.kind {
                    *alignment = 8;
                }
            }
            8 => {
                if let OperationKind::Alloca { count, .. } = &mut operation.kind {
                    *count = Some(ValueId(5));
                }
            }
            9 => {
                if let OperationKind::Alloca { element, .. } = &mut operation.kind {
                    *element = Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(18));
                }
            }
            10 => {
                if let OperationKind::Alloca { address_space, .. } = &mut operation.kind {
                    *address_space = AddressSpace::Global;
                }
            }
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(9);
        let mut budget = ArgumentBudgetV1::new(&mut work, 73);
        budget.reserve_storage(73).unwrap();
        let checked = check_scoped_object_alloca_v29(&operation, pointer, schema, 16, &mut budget);
        assert_eq!(checked.is_ok(), mutation == 0, "mutation {mutation}");
        assert_eq!(budget.work(), 9);
        assert_eq!(budget.storage(), 73);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
    let mut budget = ArgumentBudgetV1::new(&mut work, 73);
    budget.reserve_storage(73).unwrap();
    assert!(matches!(
        check_scoped_object_alloca_v29(&original, pointer, schema, 16, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert_eq!(budget.storage(), 73);
}

mod source_emission_tests {
    use super::*;
    include!("production_source_object_emission_v29_tests.rs");
}

#[test]
fn retained_object_slot_clone_prepays_real_map_headers_and_each_insertion() {
    type Slots = BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>;
    let slots: Slots = [1, 7]
        .into_iter()
        .map(|generation| {
            (
                ScopedAllocationIdentityV29::OriginalObject {
                    local: 3,
                    generation,
                },
                SemanticRetainedLocalSlotV1 {
                    pointer: ValueId(generation),
                    semantic_type: SemanticTypeIdV1::from_index(2),
                    storage: SemanticRetainedStorageV29::Object {
                        cell: generation as usize,
                        schema: fe2o3_kernel_ir::StorageLayoutIdV1(19),
                        bytes: 48,
                        alignment: 16,
                    },
                },
            )
        })
        .collect();
    let header = std::mem::size_of::<Slots>()
        + 2 * std::mem::size_of::<Result<Slots, ProductionSemanticKirErrorV1>>();
    let entry = 2
        * 32
        * std::mem::size_of::<(
            ScopedAllocationIdentityV29,
            SemanticRetainedLocalSlotV1,
            usize,
        )>();
    // Each of the first two insertions pays two tree levels *16, row3,
    // and representation1. Object schemas contain no cloned recursive Type.
    let exact_work = 72;
    for mode in 0..5 {
        let work_limit = exact_work - usize::from(mode == 1);
        let storage_limit = match mode {
            2 => header - 1,
            3 => header + entry - 1,
            4 => header + 2 * entry - 1,
            _ => header + 2 * entry,
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        let result = clone_retained_local_slots_v29(&slots, &mut budget);
        assert_eq!(result.is_ok(), mode == 0);
        let expected = match mode {
            0 => (72, header + 2 * entry),
            1 => (71, header + 2 * entry),
            2 => (0, 0),
            3 => (32, header),
            4 => (68, header + entry),
            _ => unreachable!(),
        };
        assert_eq!((budget.work(), budget.storage()), expected);
        if let Ok(cloned) = result {
            assert_eq!(cloned.len(), 2);
            for (identity, original) in &slots {
                let actual = &cloned[identity];
                assert_eq!(actual.pointer, original.pointer);
                assert_eq!(actual.semantic_type, original.semantic_type);
                assert!(
                    matches!(actual.storage, SemanticRetainedStorageV29::Object {
                    cell, schema: fe2o3_kernel_ir::StorageLayoutIdV1(19), bytes: 48, alignment: 16,
                } if cell == original.pointer.0 as usize)
                );
            }
        }
    }
}

#[test]
fn unused_typed_allocations_still_require_final_object_completion() {
    let schema = fe2o3_kernel_ir::StorageLayoutIdV1(3);
    let object = Operation::effect_free(
        ValueDef::new(
            ValueId(0),
            Type::pointer(
                Type::StorageObject(schema),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::StorageObject(schema),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    );
    assert!(scoped_object_requires_completion_v29(&object));
    let tag = Operation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U128)),
        OperationKind::Storage(ScopedObjectOperationV29::ReadDiscriminant {
            address: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Private, 1),
        }),
    );
    assert!(scoped_object_requires_completion_v29(&tag));
    let scalar = Operation::effect_free(
        ValueDef::new(
            ValueId(0),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    );
    assert!(!scoped_object_requires_completion_v29(&scalar));
}
