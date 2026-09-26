fn scalar_field_layouts_v29() -> Vec<fe2o3_kernel_ir::StorageLayoutV1> {
    use fe2o3_kernel_ir::{
        StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1,
    };
    vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: Kind::Record(vec![
                StorageFieldV1 {
                    offset: 0,
                    layout: Id(0),
                },
                StorageFieldV1 {
                    offset: 4,
                    layout: Id(0),
                },
            ].into()),
        },
    ]
}

fn scalar_field_equations_v29(
    sibling: bool,
) -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<SourceAddressAccessV29>,
) {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    let (mut function, mut slots, _) = typed_currentness_fixture();
    slots.truncate(1);
    slots[0].origin.source = ScopedAllocationSourceV29::OriginalObject {
        cell: 0,
        schema: Id(1),
    };
    slots[0].representation = ScopedSlotRepresentationV29::Object {
        schema: Id(1),
        bytes: 8,
        alignment: 4,
    };
    let mut root = allocation(A, Type::StorageObject(Id(1)));
    let OperationKind::Alloca { alignment, .. } = &mut root.kind else {
        unreachable!()
    };
    *alignment = 4;
    let project = |result, field| {
        Operation::effect_free(
            ValueDef::new(
                result,
                Type::pointer(
                    Type::StorageObject(Id(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Storage(ScopedObjectOperationV29::Project {
                base: A,
                step: ScopedObjectProjectionV29::Field(field),
            }),
        )
    };
    function.body.as_mut().unwrap().blocks[0].operations = vec![
        root,
        project(EXPOSED_A, 0),
        project(EXPOSED_B, 1),
        Operation::new(
            vec![],
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: EXPOSED_A,
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(LOADED, Type::Scalar(ScalarType::U32)),
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                address: if sibling { EXPOSED_B } else { EXPOSED_A },
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    (
        function,
        slots,
        vec![
            SourceAddressAccessV29 {
                block: BlockId(77),
                operation: 3,
                slot: 0,
            },
            SourceAddressAccessV29 {
                block: BlockId(77),
                operation: 4,
                slot: 0,
            },
        ],
    )
}

fn run_scalar_field_history_v29(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    let layouts = scalar_field_layouts_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            function, slots, None, accesses, &layouts, budget,
        )?
        .solve(slots, accesses, kills, budget)?;
        assert_eq!(
            graph.object_location(EXPOSED_A, budget)?,
            SourceStaticObjectLocationV29 {
                slot: 0,
                offset: 0,
                schema: Some(fe2o3_kernel_ir::StorageLayoutIdV1(0))
            }
        );
        assert_eq!(
            graph.object_location(EXPOSED_B, budget)?,
            SourceStaticObjectLocationV29 {
                slot: 0,
                offset: 4,
                schema: Some(fe2o3_kernel_ir::StorageLayoutIdV1(0))
            }
        );
        scoped_slot_uses_v29::check_expanded_scalar_addresses_v29(
            function, &graph, slots, accesses, kills, budget,
        )?;
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn static_field_equations_keep_sibling_initialization_and_slot_kills_distinct() {
    let (function, slots, accesses) = scalar_field_equations_v29(false);
    let (positive, _, _, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
    positive.unwrap();
    assert!(completed);
    let (sibling, slots, accesses) = scalar_field_equations_v29(true);
    unsupported(run_scalar_field_history_v29(&sibling, &slots, &accesses, &[], LIMIT, LIMIT).0);
    let killed = [SourceAddressKillV29 {
        block: BlockId(77),
        gap: 4,
        slot: 0,
    }];
    unsupported(
        run_scalar_field_history_v29(&function, &slots, &accesses, &killed, LIMIT, LIMIT).0,
    );
    let mut early_read = function.clone();
    early_read.body.as_mut().unwrap().blocks[0]
        .operations
        .swap(3, 4);
    unsupported(run_scalar_field_history_v29(&early_read, &slots, &accesses, &[], LIMIT, LIMIT).0);
}

#[test]
fn static_field_equations_join_all_reachable_predecessor_histories() {
    for second_write in [false, true] {
        let (mut function, slots, _) = scalar_field_equations_v29(false);
        let entry = &mut function.body.as_mut().unwrap().blocks[0];
        let read = entry.operations.pop().unwrap();
        let write = entry.operations.pop().unwrap();
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(78),
            then_arguments: vec![],
            else_target: BlockId(79),
            else_arguments: vec![],
        });
        let mut left = block(78);
        left.operations.push(write.clone());
        left.terminator = Some(Terminator::Branch {
            target: BlockId(80),
            arguments: vec![],
        });
        let mut right = block(79);
        if second_write {
            right.operations.push(write);
        }
        right.terminator = Some(Terminator::Branch {
            target: BlockId(80),
            arguments: vec![],
        });
        let mut join = block(80);
        join.operations.push(read);
        function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .extend([left, right, join]);
        let mut accesses = vec![SourceAddressAccessV29 {
            block: BlockId(78),
            operation: 0,
            slot: 0,
        }];
        if second_write {
            accesses.push(SourceAddressAccessV29 {
                block: BlockId(79),
                operation: 0,
                slot: 0,
            });
        }
        accesses.push(SourceAddressAccessV29 {
            block: BlockId(80),
            operation: 0,
            slot: 0,
        });
        let (result, _, _, completed) =
            run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
        if second_write {
            result.unwrap();
            assert!(completed);
        } else {
            unsupported(result);
            assert!(!completed);
        }
    }
}

#[test]
fn static_field_solver_and_history_have_exact_measured_resource_boundaries() {
    let (function, slots, accesses) = scalar_field_equations_v29(false);
    let (result, work, storage, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], LIMIT, LIMIT);
    result.unwrap();
    assert!(completed);
    let (result, _, _, completed) =
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work, storage);
    result.unwrap();
    assert!(completed);
    assert!(matches!(
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        run_scalar_field_history_v29(&function, &slots, &accesses, &[], work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn static_field_transfer_has_independent_constant_work_and_no_storage() {
    use fe2o3_kernel_ir::StorageLayoutIdV1 as Id;
    let layouts = scalar_field_layouts_v29();
    for limit in [7, 6] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_static_object_transfer_v29(
            &layouts,
            Id(1),
            ScopedObjectProjectionV29::Field(1),
            &mut budget,
        );
        if limit == 7 {
            assert_eq!(
                result.unwrap(),
                SourceStaticObjectTransferV29::Project {
                    parent: Id(1),
                    child: Id(0),
                    offset: 4,
                    parent_bytes: 8,
                    child_bytes: 4
                }
            );
            assert_eq!(budget.work(), 7);
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
    let (_, slots, _) = scalar_field_equations_v29(false);
    let transfer = SourceStaticObjectTransferV29::Project {
        parent: Id(1),
        child: Id(0),
        offset: 4,
        parent_bytes: 8,
        child_bytes: 4,
    };
    for limit in [6, 5] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_static_object_apply_v29(
            transfer,
            Some(SourceStaticObjectLocationV29 {
                slot: 0,
                offset: 0,
                schema: Some(Id(1)),
            }),
            &slots,
            &mut budget,
        );
        if limit == 6 {
            assert_eq!(
                result.unwrap(),
                Some(SourceStaticObjectLocationV29 {
                    slot: 0,
                    offset: 4,
                    schema: Some(Id(0))
                })
            );
            assert_eq!(budget.work(), 6);
        } else {
            assert!(matches!(
                result,
                Err(origin_worklist_v1::OriginWorkErrorV1::Resource(
                    ArgumentResourceV1::Work(_)
                ))
            ));
        }
        assert_eq!(budget.peak_storage(), 0);
    }
}

#[test]
fn static_field_geometry_refuses_unknown_schema_nested_pointer_and_array_steps() {
    use fe2o3_kernel_ir::{StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind};
    let mut layouts = scalar_field_layouts_v29();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    for (schema, step) in [
        (Id(99), ScopedObjectProjectionV29::Field(0)),
        (Id(1), ScopedObjectProjectionV29::Field(2)),
        (Id(1), ScopedObjectProjectionV29::ArrayIndex(ValueId(0))),
    ] {
        unsupported(
            source_static_object_transfer_v29(&layouts, schema, step, &mut budget).map(|_| ()),
        );
    }
    let Kind::Record(fields) = &mut layouts[1].kind else {
        unreachable!()
    };
    fields[0].layout = Id(1);
    unsupported(
        source_static_object_transfer_v29(
            &layouts,
            Id(1),
            ScopedObjectProjectionV29::Field(0),
            &mut budget,
        )
        .map(|_| ()),
    );
    let (_, _, pointer_layouts) = typed_currentness_fixture();
    layouts.push(pointer_layouts[1].clone());
    let Kind::Record(fields) = &mut layouts[1].kind else {
        unreachable!()
    };
    fields[0].layout = Id(2);
    unsupported(
        source_static_object_transfer_v29(
            &layouts,
            Id(1),
            ScopedObjectProjectionV29::Field(0),
            &mut budget,
        )
        .map(|_| ()),
    );
    let (_, slots, _) = scalar_field_equations_v29(false);
    let transfer = SourceStaticObjectTransferV29::Project {
        parent: Id(1),
        child: Id(0),
        offset: 4,
        parent_bytes: 8,
        child_bytes: 4,
    };
    for location in [
        None,
        Some(SourceStaticObjectLocationV29 {
            slot: 0,
            offset: 0,
            schema: Some(Id(0)),
        }),
        Some(SourceStaticObjectLocationV29 {
            slot: 0,
            offset: 4,
            schema: Some(Id(1)),
        }),
        Some(SourceStaticObjectLocationV29 {
            slot: 1,
            offset: 0,
            schema: Some(Id(1)),
        }),
    ] {
        assert!(matches!(
            source_static_object_apply_v29(transfer, location, &slots, &mut budget),
            Err(origin_worklist_v1::OriginWorkErrorV1::Shape)
        ));
    }
    assert_eq!(budget.peak_storage(), 0);
}
