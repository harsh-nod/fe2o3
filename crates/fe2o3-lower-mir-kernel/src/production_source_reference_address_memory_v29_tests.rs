use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

include!("production_source_static_object_geometry_v29_tests.rs");
include!("production_source_failure_history_v29_tests.rs");
include!("production_source_tag_geometry_v43_tests.rs");

const LIMIT: usize = 20_000_000;
const FLOOR: usize = 37;
const A: ValueId = ValueId(10);
const B: ValueId = ValueId(11);
const CELL: ValueId = ValueId(12);
const EXPOSED_A: ValueId = ValueId(20);
const EXPOSED_B: ValueId = ValueId(21);
const LOADED: ValueId = ValueId(30);

#[test]
fn empty_local_domain_does_not_authorize_unknown_aliases_to_any_real_cell() {
    let mut entry = block(77);
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(EXPOSED_A, pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(1),
            },
        ),
        store(EXPOSED_A, ValueId(2), AddressSpace::Generic),
    ];
    let external = Function::internal_helper(
        "empty-local-domain-equations-only",
        Signature::new(
            vec![pointer(), Type::INDEX, Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry],
    );
    let check = |function: &Function,
                 slots: &[ScopedSourceSlotV29],
                 rows: &[SourceAddressAccessV29],
                 work_limit,
                 storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
            let graph = SourceAddressMemoryV29::prepare(function, slots, None, rows, budget)?
                .solve(slots, rows, &[], budget)?;
            assert!(slots.is_empty());
            assert_eq!(graph.exact(EXPOSED_A, budget)?, None);
            Ok::<_, ProductionSemanticKirErrorV1>(())
        });
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    };
    let (positive, work, storage) = check(&external, &[], &[], LIMIT, LIMIT);
    positive.unwrap();
    check(&external, &[], &[], work, storage).0.unwrap();
    assert!(matches!(
        check(&external, &[], &[], work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        check(&external, &[], &[], work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
    let fabricated = [SourceAddressAccessV29 {
        footprint: 0,
        block: BlockId(77),
        operation: 1,
        slot: 0,
    }];
    unsupported(check(&external, &[], &fabricated, LIMIT, LIMIT).0);
    let mut with_local = external.clone();
    with_local.body.as_mut().unwrap().blocks[0]
        .operations
        .insert(0, allocation(A, Type::Scalar(ScalarType::U32)));
    unsupported(check(&with_local, &[], &[], LIMIT, LIMIT).0);
    let local_slots = candidates(&with_local);
    assert_eq!(local_slots.len(), 1);
    unsupported(check(&with_local, &local_slots, &[], LIMIT, LIMIT).0);
}

fn pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    )
}

fn allocation(id: ValueId, element: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            id,
            Type::pointer(
                element.clone(),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    )
}

fn expose(id: ValueId, base: ValueId) -> Operation {
    Operation::effect_free(
        ValueDef::new(id, pointer()),
        OperationKind::Cast {
            kind: CastKind::PointerToGeneric,
            value: base,
            to: pointer(),
        },
    )
}

fn store(pointer: ValueId, value: ValueId, space: AddressSpace) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer,
            value,
            access: MemoryAccess::new(space, 4),
        },
    )
}

fn load() -> Operation {
    Operation::effect_free(
        ValueDef::new(LOADED, pointer()),
        OperationKind::Load {
            pointer: CELL,
            access: MemoryAccess::new(AddressSpace::Private, 8),
        },
    )
}

fn block(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn fixture() -> Function {
    let mut entry = block(77);
    entry.operations = vec![
        allocation(A, Type::Scalar(ScalarType::U32)),
        allocation(B, Type::Scalar(ScalarType::U32)),
        allocation(CELL, pointer()),
        expose(EXPOSED_A, A),
        expose(EXPOSED_B, B),
        store(CELL, EXPOSED_A, AddressSpace::Private),
        load(),
        store(LOADED, ValueId(1), AddressSpace::Generic),
    ];
    Function::internal_helper(
        "raw-address-equations-only",
        Signature::new(vec![Type::BOOL, Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    )
}

fn accesses() -> Vec<SourceAddressAccessV29> {
    vec![
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation: 5,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation: 6,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation: 7,
            slot: 0,
        },
    ]
}

// These are inert raw-KIR equation labels, not admitted original-source slots.
// The production source census must authenticate every roster row separately.
fn candidates(function: &Function) -> Vec<ScopedSourceSlotV29> {
    let mut slots = Vec::new();
    for (block_ordinal, block) in function.body.as_ref().unwrap().blocks.iter().enumerate() {
        for (operation, row) in block.operations.iter().enumerate() {
            let OperationKind::Alloca { element, .. } = &row.kind else {
                continue;
            };
            let (element, size) = match element {
                Type::Scalar(scalar) => (PrivateRetainedElementFactsV1::Scalar(*scalar), 4),
                Type::Pointer(pointer) => (
                    PrivateRetainedElementFactsV1::ThinPointer {
                        element: pointer.pointee.as_scalar().unwrap(),
                        space: pointer.address_space,
                        access: pointer.access,
                    },
                    8,
                ),
                _ => panic!("fixture supports only physical scalar cells"),
            };
            slots.push(ScopedSourceSlotV29 {
                instance: ProductionCallInstanceIdV1(0),
                origin: ScopedSlotOriginV29 {
                    identity: ScopedAllocationIdentityV29::LegacyLocal(slots.len() as u32),
                    source: ScopedAllocationSourceV29::Legacy,
                    semantic_type: SemanticTypeIdV1::from_index(0),
                    pointer: row.results[0].id,
                },
                representation: ScopedSlotRepresentationV29::ScalarArray(
                    ScopedScalarArraySlotV29 {
                        element_type: SemanticTypeIdV1::from_index(0),
                        element: PrivateRetainedSlotFactsV1 {
                            element,
                            size,
                            alignment: 8,
                        },
                        length: 1,
                        bytes: size,
                        count: None,
                    },
                ),
                allocation: PrivateArrayPhysicalLocationV1 {
                    block_ordinal,
                    block: block.id,
                    operation,
                },
            });
        }
    }
    slots
}

fn run(
    function: &Function,
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let slots = candidates(function);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut complete = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::new(function, &slots, None, accesses, kills, budget)?;
        assert_eq!(graph.exact(LOADED, budget)?, Some(0));
        drop(graph);
        complete = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    if result.is_ok() {
        assert!(complete, "inner physical assertions must complete")
    }
    (result, budget.work(), budget.peak_storage())
}

fn unsupported(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
    ));
}

fn run_currentness(
    function: &Function,
    accesses: &[SourceAddressAccessV29],
    kills: &[SourceAddressKillV29],
    lifetimes: &[SourceAddressLifetimeV29],
    births: &[SourceAddressBirthV29],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let slots = candidates(function);
    // Inert equation inputs, NOT original source lifetime/formation authority.
    // Actual admission must derive these rows from original events and source
    // recipes before this physical check, and finish the full access census.
    let initial_live = vec![true; slots.len()];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::new(function, &slots, None, accesses, kills, budget)?;
        let before = budget.storage();
        check_source_address_currentness_v29(
            function,
            &graph,
            &slots,
            accesses,
            kills,
            &initial_live,
            lifetimes,
            births,
            budget,
        )?;
        assert_eq!(
            budget.storage(),
            before,
            "closed currentness scratch must be dropped before refund"
        );
        drop(graph);
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    if result.is_ok() {
        assert!(
            completed,
            "physical currentness assertions were caught or skipped"
        );
    }
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn physical_currentness_restarts_expire_saved_registers_and_pointer_cells() {
    run_currentness(&fixture(), &accesses(), &[], &[], &[], LIMIT, LIMIT)
        .0
        .unwrap();
    for (gap, live) in [(7, false), (7, true), (6, true)] {
        unsupported(
            run_currentness(
                &fixture(),
                &accesses(),
                &[],
                &[SourceAddressLifetimeV29 {
                    block: BlockId(77),
                    gap,
                    sequence: 0,
                    slot: 0,
                    live,
                }],
                &[],
                LIMIT,
                LIMIT,
            )
            .0,
        );
    }
    // Ending a different original allocation does not kill this object's alias.
    run_currentness(
        &fixture(),
        &accesses(),
        &[],
        &[SourceAddressLifetimeV29 {
            block: BlockId(77),
            gap: 7,
            sequence: 0,
            slot: 1,
            live: false,
        }],
        &[],
        LIMIT,
        LIMIT,
    )
    .0
    .unwrap();
}

fn typed_currentness_fixture() -> (
    Function,
    Vec<ScopedSourceSlotV29>,
    Vec<fe2o3_kernel_ir::StorageLayoutV1>,
) {
    use fe2o3_kernel_ir::{
        StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1, StoragePointerV1,
    };
    let mut function = fixture();
    let mut slots = candidates(&function);
    let layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: AddressSpace::Private,
                encoded_space: AddressSpace::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
    ];
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    for (index, slot) in slots.iter_mut().enumerate() {
        let schema = Id(u32::from(index == 2));
        let layout = &layouts[schema.0 as usize];
        // Inert raw-KIR equation inputs only. The ordinary-source tests
        // separately require the authenticated original instance and cell.
        slot.origin.identity = ScopedAllocationIdentityV29::OriginalObject {
            local: index as u32,
            generation: 0,
        };
        slot.origin.source = ScopedAllocationSourceV29::OriginalObject {
            cell: index,
            schema,
        };
        slot.representation = ScopedSlotRepresentationV29::Object {
            schema,
            bytes: layout.size,
            alignment: layout.alignment,
        };
        operations[index] = allocation(slot.origin.pointer, Type::StorageObject(schema));
        let OperationKind::Alloca { alignment, .. } = &mut operations[index].kind else {
            unreachable!()
        };
        *alignment = layout.alignment;
    }
    let pointer = Type::pointer(
        Type::StorageObject(Id(0)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    operations[3] = Operation::effect_free(
        ValueDef::new(ValueId(40), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    );
    operations[4] = Operation::effect_free(
        ValueDef::new(EXPOSED_A, pointer.clone()),
        OperationKind::Select {
            condition: ValueId(40),
            true_value: A,
            false_value: A,
        },
    );
    operations[5] = Operation::new(
        vec![],
        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address: CELL,
            value: EXPOSED_A,
            access: MemoryAccess::new(AddressSpace::Private, 8),
        }),
    );
    operations[6] = Operation::effect_free(
        ValueDef::new(LOADED, pointer),
        OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
            address: CELL,
            access: MemoryAccess::new(AddressSpace::Private, 8),
        }),
    );
    operations[7] = Operation::new(
        vec![],
        OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
            address: LOADED,
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        }),
    );
    (function, slots, layouts)
}

fn run_typed_currentness(
    function: &Function,
    slots: &[ScopedSourceSlotV29],
    layouts: &[fe2o3_kernel_ir::StorageLayoutV1],
    accesses: &[SourceAddressAccessV29],
    lifetimes: &[SourceAddressLifetimeV29],
    births: &[SourceAddressBirthV29],
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let graph = SourceAddressMemoryV29::prepare_with_layouts(
            function, slots, None, accesses, layouts, budget,
        )?
        .solve(slots, accesses, &[], budget)?;
        assert_eq!(graph.exact(LOADED, budget)?, Some(0));
        let floor = budget.storage();
        check_source_address_currentness_v29(
            function,
            &graph,
            slots,
            accesses,
            &[],
            &[true; 3],
            lifetimes,
            births,
            budget,
        )?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR);
    result
}

#[test]
fn typed_cell_currentness_expires_saved_values_and_only_admits_a_fresh_birth() {
    let (mut function, slots, layouts) = typed_currentness_fixture();
    run_typed_currentness(&function, &slots, &layouts, &accesses(), &[], &[]).unwrap();
    for (gap, live) in [(7, false), (7, true), (6, true)] {
        let lifetime = [SourceAddressLifetimeV29 {
            block: BlockId(77),
            gap,
            sequence: 0,
            slot: 0,
            live,
        }];
        unsupported(run_typed_currentness(
            &function,
            &slots,
            &layouts,
            &accesses(),
            &lifetime,
            &[],
        ));
    }
    let other = [SourceAddressLifetimeV29 {
        block: BlockId(77),
        gap: 7,
        sequence: 0,
        slot: 1,
        live: false,
    }];
    run_typed_currentness(&function, &slots, &layouts, &accesses(), &other, &[]).unwrap();
    let restart = [SourceAddressLifetimeV29 {
        block: BlockId(77),
        gap: 7,
        sequence: 0,
        slot: 0,
        live: true,
    }];
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations.insert(
        7,
        Operation::effect_free(
            ValueDef::new(ValueId(22), operations[6].results[0].ty.clone()),
            OperationKind::Select {
                condition: ValueId(40),
                true_value: A,
                false_value: A,
            },
        ),
    );
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) =
        &mut operations[8].kind
    else {
        unreachable!()
    };
    *address = ValueId(22);
    let mut rows = accesses();
    rows[2].operation = 8;
    let births = [SourceAddressBirthV29 {
        block: BlockId(77),
        operation: 7,
        result: ValueId(22),
    }];
    run_typed_currentness(&function, &slots, &layouts, &rows, &restart, &births).unwrap();
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) =
        &mut function.body.as_mut().unwrap().blocks[0].operations[8].kind
    else {
        unreachable!()
    };
    *address = LOADED;
    unsupported(run_typed_currentness(
        &function, &slots, &layouts, &rows, &restart, &births,
    ));
}

#[test]
fn typed_select_birth_requires_the_original_object_not_a_saved_alias() {
    let (mut function, slots, layouts) = typed_currentness_fixture();
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations.insert(
        7,
        Operation::effect_free(
            ValueDef::new(ValueId(22), operations[6].results[0].ty.clone()),
            OperationKind::Select {
                condition: ValueId(40),
                true_value: A,
                false_value: A,
            },
        ),
    );
    let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) =
        &mut operations[8].kind
    else {
        unreachable!()
    };
    *address = ValueId(22);
    let mut rows = accesses();
    rows[2].operation = 8;
    let restart = [SourceAddressLifetimeV29 {
        block: BlockId(77),
        gap: 7,
        sequence: 0,
        slot: 0,
        live: true,
    }];
    let births = [SourceAddressBirthV29 {
        block: BlockId(77),
        operation: 7,
        result: ValueId(22),
    }];
    run_typed_currentness(&function, &slots, &layouts, &rows, &restart, &births).unwrap();
    for fault in 0..7 {
        let mut changed = function.clone();
        let operation = &mut changed.body.as_mut().unwrap().blocks[0].operations[7];
        let OperationKind::Select {
            condition,
            true_value,
            false_value,
        } = &mut operation.kind
        else {
            unreachable!()
        };
        match fault {
            0 => *false_value = B,
            1 => {
                *true_value = EXPOSED_A;
                *false_value = EXPOSED_A;
            }
            2 => {
                *true_value = LOADED;
                *false_value = LOADED;
            }
            3 => *condition = ValueId(1),
            4 => {
                operation.results[0].ty = Type::pointer(
                    Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(0)),
                    AddressSpace::Private,
                    AccessMode::ReadOnly,
                )
            }
            5 => {
                operation.results[0].ty = Type::pointer(
                    Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(1)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                )
            }
            6 => {
                operation.results[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                )
            }
            _ => unreachable!(),
        }
        unsupported(run_typed_currentness(
            &changed, &slots, &layouts, &rows, &restart, &births,
        ));
    }
    let dead = [SourceAddressLifetimeV29 {
        live: false,
        ..restart[0]
    }];
    unsupported(run_typed_currentness(
        &function, &slots, &layouts, &rows, &dead, &births,
    ));
}

#[test]
fn typed_cell_solver_rejects_changed_schema_value_type_and_access_census() {
    let (function, slots, layouts) = typed_currentness_fixture();
    run_typed_currentness(&function, &slots, &layouts, &accesses(), &[], &[]).unwrap();
    for fault in 0..4 {
        let mut function = function.clone();
        let mut rows = accesses();
        let mut layouts = layouts.clone();
        match fault {
            0 => layouts[0].kind = fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::I32),
            1 => {
                function.body.as_mut().unwrap().blocks[0].operations[6].results[0].ty =
                    Type::Scalar(ScalarType::U32)
            }
            2 => rows[2].slot = 1,
            3 => {
                rows.remove(0);
            }
            _ => unreachable!(),
        }
        unsupported(run_typed_currentness(
            &function,
            &slots,
            &layouts,
            &rows,
            &[],
            &[],
        ));
    }
}

fn private_formation_fixture() -> Function {
    let mut function = fixture();
    let private = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations[2] = allocation(CELL, private.clone());
    operations[3] = Operation::effect_free(
        ValueDef::new(ValueId(40), Type::INDEX),
        OperationKind::Constant(Constant::Index(0)),
    );
    operations[4] = Operation::effect_free(
        ValueDef::new(EXPOSED_A, private.clone()),
        OperationKind::GetElementPointer {
            base: A,
            offset: ValueId(40),
        },
    );
    operations[6].results[0].ty = private;
    operations[7] = store(LOADED, ValueId(1), AddressSpace::Private);
    function
}

#[test]
fn checked_zero_gep_distinguishes_saved_private_alias_from_reactivated_backing() {
    let mut function = private_formation_fixture();
    run(&function, &accesses(), &[], LIMIT, LIMIT).0.unwrap();
    let lifetime = [SourceAddressLifetimeV29 {
        block: BlockId(77),
        gap: 7,
        sequence: 0,
        slot: 0,
        live: true,
    }];
    unsupported(run_currentness(&function, &accesses(), &[], &lifetime, &[], LIMIT, LIMIT).0);
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    let private = operations[4].results[0].ty.clone();
    operations.insert(
        7,
        Operation::effect_free(
            ValueDef::new(ValueId(22), private),
            OperationKind::GetElementPointer {
                base: A,
                offset: ValueId(40),
            },
        ),
    );
    operations[8] = store(ValueId(22), ValueId(1), AddressSpace::Private);
    let mut rows = accesses();
    rows[2].operation = 8;
    let births = [SourceAddressBirthV29 {
        block: BlockId(77),
        operation: 7,
        result: ValueId(22),
    }];
    run_currentness(&function, &rows, &[], &lifetime, &births, LIMIT, LIMIT)
        .0
        .unwrap();
    // Same object, pointee and address space do not revive the saved value.
    function.body.as_mut().unwrap().blocks[0].operations[8] =
        store(LOADED, ValueId(1), AddressSpace::Private);
    unsupported(run_currentness(&function, &rows, &[], &lifetime, &births, LIMIT, LIMIT).0);
}

#[test]
fn zero_gep_requires_actual_index_zero_and_exact_pointer_type() {
    for mutation in 0..3 {
        let mut function = private_formation_fixture();
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => operations[3].kind = OperationKind::Constant(Constant::Index(1)),
            1 => {
                operations[3].kind = OperationKind::Constant(Constant::U64(0));
                operations[3].results[0].ty = Type::Scalar(ScalarType::U64);
            }
            2 => operations[4].results[0].ty = pointer(),
            _ => unreachable!(),
        }
        unsupported(run(&function, &accesses(), &[], LIMIT, LIMIT).0);
    }
}

#[test]
fn physical_value_kill_does_not_fabricate_referent_lifetime_end() {
    // The pointer has already been loaded. Moving/deinitializing its holder
    // cannot expire the independently live referent or that saved raw value.
    run_currentness(
        &fixture(),
        &accesses(),
        &[SourceAddressKillV29 {
            source_order: [0; 5],
            block: BlockId(77),
            gap: 7,
            slot: 2,
        }],
        &[],
        &[],
        LIMIT,
        LIMIT,
    )
    .0
    .unwrap();
}

fn activation_loop(
    fresh: bool,
) -> (
    Function,
    Vec<SourceAddressAccessV29>,
    Vec<SourceAddressBirthV29>,
) {
    let mut function = fixture();
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations.truncate(6);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(88),
        arguments: vec![],
    });
    let mut next = block(88);
    let mut rows = vec![SourceAddressAccessV29 {
        footprint: 0,
        block: BlockId(77),
        operation: 5,
        slot: 2,
    }];
    let mut births = Vec::new();
    if fresh {
        next.operations.push(expose(ValueId(22), A));
        next.operations
            .push(store(CELL, ValueId(22), AddressSpace::Private));
        rows.push(SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 1,
            slot: 2,
        });
        births.push(SourceAddressBirthV29 {
            block: BlockId(88),
            operation: 0,
            result: ValueId(22),
        });
    }
    rows.push(SourceAddressAccessV29 {
        footprint: 0,
        block: BlockId(88),
        operation: next.operations.len(),
        slot: 2,
    });
    next.operations.push(load());
    rows.push(SourceAddressAccessV29 {
        footprint: 0,
        block: BlockId(88),
        operation: next.operations.len(),
        slot: 0,
    });
    next.operations
        .push(store(LOADED, ValueId(1), AddressSpace::Generic));
    next.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(88),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    body.blocks.extend([next, block(99)]);
    (function, rows, births)
}

#[test]
fn same_static_activation_in_a_loop_requires_actual_fresh_alias_reassignment() {
    let lifecycle = [SourceAddressLifetimeV29 {
        block: BlockId(88),
        gap: 0,
        sequence: 0,
        slot: 0,
        live: true,
    }];
    let (function, rows, births) = activation_loop(true);
    run_currentness(&function, &rows, &[], &lifecycle, &births, LIMIT, LIMIT)
        .0
        .unwrap();
    let (function, rows, births) = activation_loop(false);
    unsupported(run_currentness(&function, &rows, &[], &lifecycle, &births, LIMIT, LIMIT).0);
}

#[test]
fn one_expired_alternative_in_a_same_object_join_cannot_be_dropped() {
    let (function, rows) = diamond(EXPOSED_A);
    unsupported(
        run_currentness(
            &function,
            &rows,
            &[],
            &[SourceAddressLifetimeV29 {
                block: BlockId(88),
                gap: 0,
                sequence: 0,
                slot: 0,
                live: true,
            }],
            &[],
            LIMIT,
            LIMIT,
        )
        .0,
    );
}

#[test]
fn physical_currentness_actual_equations_preserve_exact_and_one_short_limits() {
    let function = fixture();
    let rows = accesses();
    let (result, work, storage) = run_currentness(&function, &rows, &[], &[], &[], LIMIT, LIMIT);
    result.unwrap();
    assert!(work > 0 && storage > 0);
    run_currentness(&function, &rows, &[], &[], &[], work, storage)
        .0
        .unwrap();
    assert!(matches!(
        run_currentness(&function, &rows, &[], &[], &[], work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work { .. }
            )
        )
    ));
    assert!(matches!(
        run_currentness(&function, &rows, &[], &[], &[], work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage { .. }
            )
        )
    ));
}

#[test]
fn actual_pointer_store_load_and_generic_exposure_preserve_one_allocation() {
    run(&fixture(), &accesses(), &[], LIMIT, LIMIT).0.unwrap();
}

#[test]
fn target_hypotheses_cannot_seed_or_substitute_physical_pointer_origins() {
    let function = fixture();
    let mut wrong = accesses();
    wrong[2].slot = 1;
    unsupported(run(&function, &wrong, &[], LIMIT, LIMIT).0);
    let mut function = fixture();
    function.body.as_mut().unwrap().blocks[0].operations[5] =
        store(CELL, EXPOSED_B, AddressSpace::Private);
    unsupported(run(&function, &accesses(), &[], LIMIT, LIMIT).0);
    let mut missing = accesses();
    missing.remove(0);
    unsupported(run(&fixture(), &missing, &[], LIMIT, LIMIT).0);
}

#[test]
fn unwritten_and_killed_pointer_cells_do_not_gain_origins_from_read_receipts() {
    let mut function = fixture();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .remove(5);
    let reads = [
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation: 5,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation: 6,
            slot: 0,
        },
    ];
    unsupported(run(&function, &reads, &[], LIMIT, LIMIT).0);
    let kill = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(77),
        gap: 6,
        slot: 2,
    }];
    unsupported(run(&fixture(), &accesses(), &kill, LIMIT, LIMIT).0);
    let prior_kill = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(77),
        gap: 5,
        slot: 2,
    }];
    run(&fixture(), &accesses(), &prior_kill, LIMIT, LIMIT)
        .0
        .unwrap();
}

#[test]
fn wrong_or_duplicate_physical_rows_are_rejected_before_solving() {
    let mut rows = accesses();
    rows[1] = rows[0];
    unsupported(run(&fixture(), &rows, &[], LIMIT, LIMIT).0);
    let mut rows = accesses();
    rows[0].operation = 4;
    unsupported(run(&fixture(), &rows, &[], LIMIT, LIMIT).0);
    let kills = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(999),
        gap: 0,
        slot: 2,
    }];
    unsupported(run(&fixture(), &accesses(), &kills, LIMIT, LIMIT).0);
}

fn diamond(other: ValueId) -> (Function, Vec<SourceAddressAccessV29>) {
    let mut function = fixture();
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations.truncate(6);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(88),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    for (id, value) in [(88, EXPOSED_A), (99, other)] {
        let mut arm = block(id);
        arm.operations
            .push(store(CELL, value, AddressSpace::Private));
        arm.terminator = Some(Terminator::Branch {
            target: BlockId(100),
            arguments: vec![],
        });
        body.blocks.push(arm);
    }
    let mut join = block(100);
    join.operations = vec![load(), store(LOADED, ValueId(1), AddressSpace::Generic)];
    body.blocks.push(join);
    (
        function,
        vec![
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(77),
                operation: 5,
                slot: 2,
            },
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(88),
                operation: 0,
                slot: 2,
            },
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(99),
                operation: 0,
                slot: 2,
            },
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(100),
                operation: 0,
                slot: 2,
            },
            SourceAddressAccessV29 {
                footprint: 0,
                block: BlockId(100),
                operation: 1,
                slot: 0,
            },
        ],
    )
}

#[test]
fn actual_predecessors_join_same_cell_values_without_cross_product_origins() {
    let (function, rows) = diamond(EXPOSED_A);
    run(&function, &rows, &[], LIMIT, LIMIT).0.unwrap();
    let (function, rows) = diamond(EXPOSED_B);
    unsupported(run(&function, &rows, &[], LIMIT, LIMIT).0);
    let (function, rows) = diamond(EXPOSED_A);
    let killed_arm = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(99),
        gap: 1,
        slot: 2,
    }];
    unsupported(run(&function, &rows, &killed_arm, LIMIT, LIMIT).0);
}

#[test]
fn real_loop_memory_edges_recompute_writes_and_do_not_ground_an_unwritten_cycle() {
    let mut function = fixture();
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations.truncate(5);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(88),
        arguments: vec![],
    });
    let mut loop_body = block(88);
    loop_body.operations = vec![
        store(CELL, EXPOSED_A, AddressSpace::Private),
        load(),
        store(LOADED, ValueId(1), AddressSpace::Generic),
    ];
    loop_body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(88),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    body.blocks.extend([loop_body, block(99)]);
    let rows = [
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 0,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 1,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 2,
            slot: 0,
        },
    ];
    let kills = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(88),
        gap: 0,
        slot: 2,
    }];
    run(&function, &rows, &kills, LIMIT, LIMIT).0.unwrap();
    // Moving the same kill after the actual write invalidates the next Load;
    // the loop edge and a source target hypothesis cannot revive that value.
    let kills = [SourceAddressKillV29 {
        source_order: [0; 5],
        block: BlockId(88),
        gap: 1,
        slot: 2,
    }];
    unsupported(run(&function, &rows, &kills, LIMIT, LIMIT).0);
    function.body.as_mut().unwrap().blocks[1]
        .operations
        .remove(0);
    let rows = [
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 0,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 1,
            slot: 0,
        },
    ];
    unsupported(run(&function, &rows, &[], LIMIT, LIMIT).0);
}

#[test]
fn loop_carried_store_of_prior_load_requires_a_real_entry_initialization() {
    let mut function = fixture();
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations.truncate(6);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(88),
        arguments: vec![],
    });
    let mut loop_body = block(88);
    loop_body.operations = vec![
        load(),
        store(CELL, LOADED, AddressSpace::Private),
        store(LOADED, ValueId(1), AddressSpace::Generic),
    ];
    loop_body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(88),
        then_arguments: vec![],
        else_target: BlockId(99),
        else_arguments: vec![],
    });
    body.blocks.extend([loop_body, block(99)]);
    let mut rows = vec![
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(77),
            operation: 5,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 0,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 1,
            slot: 2,
        },
        SourceAddressAccessV29 {
            footprint: 0,
            block: BlockId(88),
            operation: 2,
            slot: 0,
        },
    ];
    run(&function, &rows, &[], LIMIT, LIMIT).0.unwrap();
    function.body.as_mut().unwrap().blocks[0].operations.pop();
    rows.remove(0);
    unsupported(run(&function, &rows, &[], LIMIT, LIMIT).0);
}

#[test]
fn surviving_calls_and_returns_cannot_escape_tracked_addresses() {
    let mut function = fixture();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("unmodeled-external-call"),
                arguments: vec![CELL],
            },
        ));
    unsupported(run(&function, &accesses(), &[], LIMIT, LIMIT).0);
    let mut function = fixture();
    function.signature.results.push(pointer());
    function.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![LOADED],
    });
    unsupported(run(&function, &accesses(), &[], LIMIT, LIMIT).0);
}

#[test]
fn pointer_memory_exact_and_one_short_limits_preserve_error_kind_and_drop_order() {
    let function = fixture();
    let rows = accesses();
    let (result, work, storage) = run(&function, &rows, &[], LIMIT, LIMIT);
    result.unwrap();
    run(&function, &rows, &[], work, storage).0.unwrap();
    assert!(matches!(
        run(&function, &rows, &[], work - 1, storage).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        run(&function, &rows, &[], work, storage - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}
include!("production_source_reference_logical_alias_v29_tests.rs");
