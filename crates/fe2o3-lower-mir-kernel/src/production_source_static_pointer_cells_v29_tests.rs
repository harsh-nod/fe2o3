fn pointer_subcell_fixture_v29(
    length: u64,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<SourceAddressAccessV29>,
    Vec<fe2o3_kernel_ir::StorageLayoutV1>,
) {
    use fe2o3_kernel_ir::{StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1};
    let (mut function, mut slots, mut layouts) = typed_currentness_fixture();
    layouts.push(StorageLayoutV1 {
        size: length * 8,
        alignment: 8,
        kind: Kind::Array {
            element: Id(1),
            length,
            stride: 8,
        },
    });
    slots[2].origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 2,
        schema: Id(2),
    };
    slots[2].representation = ScopedSlotRepresentationV29::Object {
        schema: Id(2),
        bytes: length * 8,
        alignment: 8,
    };
    let scalar = |id| {
        let mut operation = allocation(id, Type::StorageObject(Id(0)));
        let OperationKind::Alloca { alignment, .. } = &mut operation.kind else {
            unreachable!()
        };
        *alignment = 4;
        operation
    };
    let project = |id, index| {
        Operation::effect_free(
            ValueDef::new(
                id,
                Type::pointer(
                    Type::StorageObject(Id(1)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: CELL,
                step: ScopedObjectProjectionV29::ArrayIndex(index),
            }),
        )
    };
    let store = |address, value| {
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address,
                value,
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        )
    };
    function.body.as_mut().unwrap().blocks[0].operations = vec![
        scalar(A),
        scalar(B),
        allocation(CELL, Type::StorageObject(Id(2))),
        Operation::effect_free(
            ValueDef::new(ValueId(40), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        project(EXPOSED_A, ValueId(40)),
        Operation::effect_free(
            ValueDef::new(ValueId(41), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        project(EXPOSED_B, ValueId(41)),
        store(EXPOSED_A, A),
        store(EXPOSED_B, B),
        Operation::effect_free(
            ValueDef::new(
                LOADED,
                Type::pointer(
                    Type::StorageObject(Id(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                address: EXPOSED_A,
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: LOADED,
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    let accesses = [7, 8, 9, 10]
        .into_iter()
        .map(|operation| SourceAddressAccessV29 {
            block: BlockId(77),
            operation,
            slot: if operation == 10 { 0 } else { 2 },
        })
        .collect();
    (function, slots, accesses, layouts)
}

fn run_pointer_subcell_fixture_v29(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    rows: &[SourceAddressAccessV29],
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    kills: &[SourceAddressKillV29],
    lifetimes: &[SourceAddressLifetimeV29],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            function, slots, None, rows, layouts, budget,
        )?
        .solve(slots, rows, kills, budget)?;
        assert_eq!(graph.pointer_subcells.len(), 2);
        assert_eq!(graph.pointer_cell_count, 2);
        assert_ne!(
            graph.pointer_cell(2, EXPOSED_A, budget)?,
            graph.pointer_cell(2, EXPOSED_B, budget)?
        );
        assert_eq!(graph.exact(LOADED, budget)?, Some(0));
        check_source_address_currentness_v29(
            function,
            &graph,
            slots,
            rows,
            kills,
            &[true; 3],
            lifetimes,
            &[],
            budget,
        )?;
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            function, &graph, slots, rows, kills, budget,
        )?;
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn observed_pointer_subcells_preserve_distinct_contents_without_extent_expansion() {
    let mut prior = None;
    for length in [2, 1_000_000] {
        let (function, slots, rows, layouts) = pointer_subcell_fixture_v29(length);
        let (result, work, storage, completed) = run_pointer_subcell_fixture_v29(
            &function,
            &slots,
            &rows,
            &layouts,
            &[],
            &[],
            LIMIT,
            LIMIT,
        );
        result.unwrap();
        assert!(completed);
        if let Some(expected) = prior {
            assert_eq!((work, storage), expected);
        }
        prior = Some((work, storage));
    }
}

#[test]
fn observed_pointer_subcells_expire_all_contents_on_kill_and_pointee_restart() {
    let (function, slots, rows, layouts) = pointer_subcell_fixture_v29(2);
    let kill = [SourceAddressKillV29 {
        block: BlockId(77),
        gap: 9,
        slot: 2,
    }];
    let (result, _, _, completed) = run_pointer_subcell_fixture_v29(
        &function,
        &slots,
        &rows,
        &layouts,
        &kill,
        &[],
        LIMIT,
        LIMIT,
    );
    unsupported(result);
    assert!(!completed);
    for slot in [0, 1, 2] {
        let lifetime = [SourceAddressLifetimeV29 {
            block: BlockId(77),
            gap: 9,
            sequence: 0,
            slot,
            live: true,
        }];
        let (result, _, _, completed) = run_pointer_subcell_fixture_v29(
            &function,
            &slots,
            &rows,
            &layouts,
            &[],
            &lifetime,
            LIMIT,
            LIMIT,
        );
        if slot == 1 {
            result.unwrap();
            assert!(completed);
        } else {
            unsupported(result);
            assert!(!completed);
        }
    }
}

#[test]
fn observed_pointer_subcells_reject_uninitialized_sibling_even_if_loaded_value_is_unused() {
    let (mut function, slots, mut rows, layouts) = pointer_subcell_fixture_v29(2);
    // Keep the second leaf address but remove its store, then read that leaf.
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations.remove(8);
    operations.pop();
    let OperationKind::Storage(ScopedObjectOperationV29::ReadValue { address, .. }) =
        &mut operations[8].kind
    else {
        unreachable!()
    };
    *address = EXPOSED_B;
    rows.truncate(2);
    rows[1].operation = 8;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let mut attempted_history = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &function, &slots, None, &rows, &layouts, budget,
        )?
        .solve(&slots, &rows, &[], budget)?;
        attempted_history = true;
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            &function,
            &graph,
            &slots,
            &rows,
            &[],
            budget,
        )
    });
    unsupported(result);
    assert!(attempted_history);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn observed_pointer_subcells_exact_and_one_short_cumulative_resources_restore_floor() {
    let (function, slots, rows, layouts) = pointer_subcell_fixture_v29(2);
    let (result, work, storage, completed) =
        run_pointer_subcell_fixture_v29(&function, &slots, &rows, &layouts, &[], &[], LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, _, _, completed) = run_pointer_subcell_fixture_v29(
        &function,
        &slots,
        &rows,
        &layouts,
        &[],
        &[],
        work,
        storage,
    );
    result.unwrap();
    assert!(completed);
    for (w, s) in [(work - 1, storage), (work, storage - 1)] {
        let (result, _, _, completed) =
            run_pointer_subcell_fixture_v29(&function, &slots, &rows, &layouts, &[], &[], w, s);
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_) | ArgumentResourceV1::Storage(_)
                )
            )
        ));
        assert!(!completed);
    }
}

#[test]
fn constant_index_transfer_has_exact_eleven_work_and_checked_offset_bounds() {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    let (_, _, _, layouts) = pointer_subcell_fixture_v29(2);
    for limit in [11, 10] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_static_object_transfer_with_index_v29(
            &layouts,
            Id(2),
            ScopedObjectProjectionV29::ArrayIndex(ValueId(40)),
            Some(1),
            &mut budget,
        );
        if limit == 11 {
            assert_eq!(
                result.unwrap(),
                SourceStaticObjectTransferV29::Project {
                    parent: Id(2),
                    child: Id(1),
                    offset: 8,
                    parent_bytes: 16,
                    child_bytes: 8
                }
            );
            assert_eq!(budget.work(), 11);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
            assert_eq!(budget.work(), 7);
            budget.charge_work(0).unwrap();
            assert!(matches!(
                budget.check_prior_denials_v1(),
                Err(ArgumentResourceV1::Work(error)) if error.actual() == 11 && error.limit() == 10
            ));
            assert_eq!(budget.work(), 7);
        }
        assert_eq!(budget.storage(), 0);
    }
    for (length, offset, minimum, end, expected) in [
        (5, 0, 5, false, Some(0)),
        (5, 1, 5, true, Some(4)),
        (5, 5, 5, true, Some(0)),
        (5, 0, 5, true, None),
        (5, 6, 5, true, None),
        (5, 1, 6, false, None),
        (5, 5, 5, false, None),
        (0, 0, 0, false, None),
    ] {
        assert_eq!(
            source_static_constant_index_v29(length, offset, minimum, end).ok(),
            expected
        );
    }
}

fn opaque_pointer_subcell_fixture_v29() -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<SourceAddressAccessV29>,
    Vec<fe2o3_kernel_ir::StorageLayoutV1>,
) {
    let (mut function, slots, mut rows, mut layouts) = pointer_subcell_fixture_v29(2);
    let raw = pointer();
    let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(layout) = &mut layouts[1].kind else {
        unreachable!()
    };
    layout.value_space = AddressSpace::Generic;
    function.signature.parameters.push(raw.clone());
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(2));
    let entry = &mut body.blocks[0];
    entry.operations.pop();
    let mut load = entry.operations.pop().unwrap();
    load.results[0].ty = raw.clone();
    for ordinal in [7, 8] {
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) =
            &mut entry.operations[ordinal].kind
        else {
            unreachable!()
        };
        *value = ValueId(2);
    }
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(78),
        arguments: vec![ValueId(2)],
    });
    let mut child = block(78);
    child.parameters.push(ValueDef::new(ValueId(3), raw));
    child.operations = vec![
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: EXPOSED_A,
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ),
        load,
    ];
    body.blocks.push(child);
    rows.truncate(2);
    rows.extend([0, 1].map(|operation| SourceAddressAccessV29 {
        block: BlockId(78),
        operation,
        slot: 2,
    }));
    (function, slots, rows, layouts)
}

#[test]
fn initialized_opaque_pointer_subcells_transport_bits_through_cfg_without_pointee_authority() {
    for fault in 0..4 {
        let (mut function, slots, rows, layouts) = opaque_pointer_subcell_fixture_v29();
        let last = &mut function.body.as_mut().unwrap().blocks[1];
        match fault {
            0 => {}
            1 => last
                .operations
                .push(store(LOADED, ValueId(1), AddressSpace::Generic)),
            2 => {
                function.signature.results.push(pointer());
                last.terminator = Some(Terminator::Return {
                    values: vec![LOADED],
                });
            }
            3 => {
                last.operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(50), Type::INDEX),
                    OperationKind::Constant(Constant::Index(0)),
                ));
                last.operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(51), pointer()),
                    OperationKind::GetElementPointer {
                        base: LOADED,
                        offset: ValueId(50),
                    },
                ));
            }
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut completed = false;
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let graph = SourceAddressMemoryV29::prepare_with_layouts(
                &function, &slots, None, &rows, &layouts, budget,
            )?
            .solve(&slots, &rows, &[], budget)?;
            assert_eq!(
                graph.origins[graph.value(LOADED, budget)?],
                SourceAddressOriginV29::Unknown
            );
            assert!(
                graph.exact(LOADED, budget).is_err(),
                "no local-domain exclusion was minted"
            );
            scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
                &function,
                &graph,
                &slots,
                &rows,
                &[],
                budget,
            )?;
            check_source_address_currentness_v29(
                &function,
                &graph,
                &slots,
                &rows,
                &[],
                &[true; 3],
                &[],
                &[],
                budget,
            )?;
            completed = true;
            Ok(())
        });
        if fault == 0 {
            result.unwrap();
            assert!(completed);
        } else {
            unsupported(result);
            assert!(!completed);
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn observed_pointer_subcells_reject_forged_index_producer_and_wrong_layout_geometry() {
    for fault in 0..6 {
        let (mut function, slots, rows, mut layouts) = pointer_subcell_fixture_v29(2);
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        match fault {
            0 => operations[3].kind = OperationKind::Constant(Constant::Index(2)),
            1 => operations[3].results[0].id = ValueId(99),
            2 => operations[3].results[0].ty = Type::Scalar(ScalarType::U64),
            3 => {
                let fe2o3_kernel_ir::StorageLayoutKindV1::Array { stride, .. } =
                    &mut layouts[2].kind
                else {
                    unreachable!()
                };
                *stride = u64::MAX;
            }
            4 => {
                let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) = &mut layouts[1].kind
                else {
                    unreachable!()
                };
                pointer.access = AccessMode::ReadOnly;
            }
            5 => {
                let fe2o3_kernel_ir::StorageLayoutKindV1::Array { stride, .. } =
                    &mut layouts[2].kind
                else {
                    unreachable!()
                };
                *stride = 4;
            }
            _ => unreachable!(),
        }
        let (result, _, _, completed) = run_pointer_subcell_fixture_v29(
            &function,
            &slots,
            &rows,
            &layouts,
            &[],
            &[],
            LIMIT,
            LIMIT,
        );
        assert!(result.is_err());
        assert!(!completed);
    }
}

#[test]
fn observed_pointer_subcell_addresses_require_one_independent_allocation_location() {
    for fault in 0..3 {
        let (mut function, slots, mut rows, layouts) = pointer_subcell_fixture_v29(2);
        let address_type = Type::pointer(
            Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(1)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        );
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        let address = ValueId(60);
        let kind = if fault == 2 {
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                address: EXPOSED_B,
                access: MemoryAccess::new(AddressSpace::Private, 8),
            })
        } else {
            OperationKind::Select {
                condition: ValueId(0),
                true_value: EXPOSED_A,
                false_value: if fault == 0 { EXPOSED_A } else { EXPOSED_B },
            }
        };
        operations.insert(
            7,
            Operation::effect_free(ValueDef::new(address, address_type), kind),
        );
        let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address: target, ..
        }) = &mut operations[8].kind
        else {
            unreachable!()
        };
        *target = address;
        for row in &mut rows {
            row.operation += 1;
        }
        if fault == 2 {
            rows.insert(
                0,
                SourceAddressAccessV29 {
                    block: BlockId(77),
                    operation: 7,
                    slot: 2,
                },
            );
        }
        let (result, _, _, completed) = run_pointer_subcell_fixture_v29(
            &function,
            &slots,
            &rows,
            &layouts,
            &[],
            &[],
            LIMIT,
            LIMIT,
        );
        if fault == 0 {
            result.unwrap();
            assert!(completed);
        } else {
            assert!(result.is_err());
            assert!(!completed);
        }
    }
}

#[test]
fn observed_pointer_subcell_initialization_cannot_be_supplied_by_a_future_store() {
    let (mut function, slots, mut rows, layouts) = pointer_subcell_fixture_v29(2);
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .swap(7, 9);
    rows[0].operation = 7;
    rows[2].operation = 9;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let mut attempted_history = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let mut no_dereference = function.clone();
        no_dereference.body.as_mut().unwrap().blocks[0]
            .operations
            .pop();
        rows.pop();
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            &no_dereference,
            &slots,
            None,
            &rows,
            &layouts,
            budget,
        )?
        .solve(&slots, &rows, &[], budget)?;
        attempted_history = true;
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            &no_dereference,
            &graph,
            &slots,
            &rows,
            &[],
            budget,
        )
    });
    unsupported(result);
    assert!(attempted_history);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn static_projection_classification_keeps_value_and_tag_effects_distinct() {
    let access = MemoryAccess::new(AddressSpace::Private, 8);
    for step in [
        ScopedObjectProjectionV29::Field(0),
        ScopedObjectProjectionV29::ArrayIndex(ValueId(40)),
    ] {
        let operation = Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::Project { base: CELL, step }),
        );
        assert!(
            source_address_value_access_v29(&operation)
                .unwrap()
                .is_none()
        );
    }
    for operation in [
        ScopedObjectOperationV29::Project {
            base: CELL,
            step: ScopedObjectProjectionV29::Variant { index: 0, access },
        },
        ScopedObjectOperationV29::Project {
            base: CELL,
            step: ScopedObjectProjectionV29::VariantForWrite { index: 0 },
        },
        ScopedObjectOperationV29::ReadDiscriminant {
            address: CELL,
            access,
        },
        ScopedObjectOperationV29::SetDiscriminant {
            address: CELL,
            variant: 0,
            access,
        },
    ] {
        let operation = Operation::new(vec![], OperationKind::Storage(operation));
        assert!(
            matches!(source_address_value_access_v29(&operation), Err(error)
            if error.to_string() == scoped_object_pending_v29().to_string())
        );
    }
    let (function, _, _, _) = pointer_subcell_fixture_v29(2);
    let operations = &function.body.as_ref().unwrap().blocks[0].operations;
    for (ordinal, writing, value) in [(7, true, A), (9, false, LOADED)] {
        let row = source_address_value_access_v29(&operations[ordinal])
            .unwrap()
            .unwrap();
        assert_eq!(
            (row.pointer, row.value, row.writing, row.object),
            (EXPOSED_A, value, writing, true)
        );
    }
}

#[test]
fn nonaccess_array_projections_still_require_full_geometry_and_complete_access_census() {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    for fault in 0..9 {
        let (mut function, slots, mut rows, layouts) = pointer_subcell_fixture_v29(2);
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        // Even a projection with no value access must authenticate its producer
        // and original layout. It never becomes an access-census row itself.
        operations.push(Operation::effect_free(
            ValueDef::new(ValueId(60), Type::INDEX),
            OperationKind::Constant(Constant::Index(if fault == 1 { 2 } else { 0 })),
        ));
        operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(61),
                Type::pointer(
                    Type::StorageObject(if fault == 4 { Id(0) } else { Id(1) }),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: CELL,
                step: if fault == 5 {
                    ScopedObjectProjectionV29::VariantForWrite { index: 0 }
                } else {
                    ScopedObjectProjectionV29::ArrayIndex(ValueId(if fault == 3 { 40 } else { 60 }))
                },
            }),
        ));
        if fault == 2 {
            operations[11].results[0].ty = Type::Scalar(ScalarType::U64);
        }
        if fault == 6 {
            rows.remove(0);
        }
        if fault == 7 {
            rows.insert(
                0,
                SourceAddressAccessV29 {
                    block: BlockId(77),
                    operation: 4,
                    slot: 2,
                },
            );
        }
        if fault == 8 {
            rows[0].slot = 0;
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut completed = false;
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let graph = SourceAddressMemoryV29::prepare_with_layouts(
                &function, &slots, None, &rows, &layouts, budget,
            )?
            .solve(&slots, &rows, &[], budget)?;
            check_source_address_currentness_v29(
                &function,
                &graph,
                &slots,
                &rows,
                &[],
                &[true; 3],
                &[],
                &[],
                budget,
            )?;
            scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
                &function,
                &graph,
                &slots,
                &rows,
                &[],
                budget,
            )?;
            completed = true;
            Ok(())
        });
        if fault == 0 {
            result.unwrap();
            assert!(completed);
        } else {
            unsupported(result);
            assert!(!completed, "fault {fault}");
        }
        assert_eq!(budget.storage(), FLOOR, "fault {fault}");
    }
}
