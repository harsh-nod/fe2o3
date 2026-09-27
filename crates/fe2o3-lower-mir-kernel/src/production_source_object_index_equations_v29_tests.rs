// Physical equations only. Original typed ProjectionIndex authority is tested
// separately by the same-candidate source fixture.
fn object_index_equation_module_v29(mut module: Module) -> Module {
    use fe2o3_kernel_ir::{StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1};
    assert!(module.storage_layouts.is_empty());
    module.storage_layouts.push(StorageLayoutV1 {
        size: 4, alignment: 4, kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
    });
    for block in &mut module.functions[0].body.as_mut().unwrap().blocks {
        for operation in &mut block.operations {
            match operation.kind {
                OperationKind::Alloca { ref mut element, .. } => {
                    assert_eq!(*element, Type::Scalar(ScalarType::U32));
                    *element = Type::StorageObject(StorageLayoutIdV1(0));
                    operation.results[0].ty = Type::pointer(element.clone(), AddressSpace::Private, AccessMode::ReadWrite);
                }
                OperationKind::Load { pointer, access } => operation.kind = OperationKind::Storage(
                    ScopedObjectOperationV29::ReadValue { address: pointer, access }),
                OperationKind::Store { pointer, value, access } => operation.kind = OperationKind::Storage(
                    ScopedObjectOperationV29::WriteValue { address: pointer, value, access }),
                _ => {},
            }
        }
    }
    module
}

#[test]
fn typed_scalar_index_equations_need_all_reaching_writes_and_exact_load_identity() {
    for (initialized, mixed, expected) in [(true, false, true), (false, false, false), (true, true, false)] {
        let original = object_index_equation_module_v29(module(initialized, false, false, mixed));
        with_model(&original, |query, inventory, budget| {
            let load = coordinate(inventory, 90, 0, budget);
            assert_eq!(query.check_bound(0, load, FIRST, 4, budget).unwrap(), expected);
            let stored = budget.storage();
            assert_eq!(query.check_bound(0, load, FIRST, 4, budget).unwrap(), expected);
            assert_eq!(budget.storage(), stored);
            assert!(matches!(query.check_bound(0, load, INDEX, 4, budget),
                Err(ProductionSemanticKirErrorV1::Unsupported { function: 0,
                    detail: "source raw address differs from its actual formation or memory history", .. })));
        });
    }
}

#[test]
fn typed_scalar_index_history_cannot_supply_missing_shared_memory_versions() {
    let original = object_index_equation_module_v29(module(true, false, false, false));
    with_model(&original, |query, inventory, budget| {
        let load = coordinate(inventory, 90, 0, budget);
        assert!(query.check_bound(0, load, FIRST, 4, budget).unwrap());
    });
    with_model_or_error_versions(&original, false, |query, inventory, budget| {
        let load = coordinate(inventory, 90, 0, budget);
        let error = query.unwrap().check_bound(0, load, FIRST, 4, budget).unwrap_err();
        assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
            function: 0, detail: "source raw address differs from its actual formation or memory history", ..
        }), "{error:?}");
    });
}

#[test]
fn typed_index_holder_must_be_a_whole_scalar_not_a_same_sized_record() {
    use fe2o3_kernel_ir::{StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1};
    let mut original = object_index_equation_module_v29(module(true, false, false, false));
    with_model(&original, |query, inventory, budget| {
        let load = coordinate(inventory, 90, 0, budget);
        assert!(query.check_bound(0, load, FIRST, 4, budget).unwrap());
    });
    original.storage_layouts.push(StorageLayoutV1 {
        size: 4, alignment: 4,
        kind: Kind::Record(vec![StorageFieldV1 { offset: 0, layout: Id(0) }].into()),
    });
    let entry = &mut original.functions[0].body.as_mut().unwrap().blocks[0];
    let mut record = entry.operations[0].clone();
    let OperationKind::Alloca { element, .. } = &mut record.kind else { unreachable!() };
    *element = Type::StorageObject(Id(1));
    record.results[0] = ValueDef { id: ValueId(13),
        ty: Type::pointer(element.clone(), AddressSpace::Private, AccessMode::ReadWrite) };
    entry.operations.insert(1, record);
    with_model(&original, |query, inventory, budget| {
        let load = coordinate(inventory, 90, 0, budget);
        assert!(query.check_bound(0, load, FIRST, 4, budget).unwrap());
        let error = query.check_bound(1, load, FIRST, 4, budget).unwrap_err();
        assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
            function: 0, detail: "source raw address differs from its actual formation or memory history", ..
        }), "{error:?}");
    });
}

#[test]
fn typed_scalar_index_copy_cycle_cannot_supply_its_own_initial_value() {
    for initialized in [false, true] {
        let mut original = module(initialized, true, true, false);
        let body = original.functions[0].body.as_mut().unwrap();
        let mut allocation = body.blocks[0].operations[0].clone();
        allocation.results[0].id = ValueId(13);
        body.blocks[0].operations.insert(1, allocation);
        let latch = body.blocks.iter_mut().find(|block| block.id == BlockId(110)).unwrap();
        let mut save = write(FIRST);
        let OperationKind::Store { pointer, .. } = &mut save.kind else { unreachable!() };
        *pointer = ValueId(13);
        let mut copied = load(ValueId(40));
        let OperationKind::Load { pointer, .. } = &mut copied.kind else { unreachable!() };
        *pointer = ValueId(13);
        latch.operations = vec![save, copied, write(ValueId(40))];
        let original = object_index_equation_module_v29(original);
        with_model(&original, |query, inventory, budget| {
            let load = coordinate(inventory, 90, 0, budget);
            assert_eq!(query.check_bound(0, load, FIRST, 4, budget).unwrap(), initialized);
        });
    }
}

#[test]
fn typed_scalar_index_equations_do_not_treat_unknown_writes_as_disjoint() {
    for space in [AddressSpace::Global, AddressSpace::Private, AddressSpace::Generic] {
        let mut original = object_index_equation_module_v29(module(true, false, false, false));
        let function = &mut original.functions[0];
        function.signature.parameters.push(Type::pointer(Type::Scalar(ScalarType::U32),
            space, AccessMode::ReadWrite));
        let body = function.body.as_mut().unwrap();
        body.parameters.push(ValueId(2));
        body.blocks[0].operations.push(Operation::new(vec![], OperationKind::Store {
            pointer: ValueId(2), value: ValueId(0), access: MemoryAccess::new(space, 4),
        }));
        with_model_or_error(&original, |query, inventory, budget| {
            let load = coordinate(inventory, 90, 0, budget);
            let result = query.and_then(|query| query.check_bound(0, load, FIRST, 4, budget));
            if space == AddressSpace::Global {
                assert_eq!(result.unwrap(), true);
            } else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    detail: "source raw address differs from its actual formation or memory history", ..
                })), "unknown {space:?}: {result:?}");
            }
        });
    }
}

#[test]
fn typed_scalar_index_bounds_do_not_survive_a_holder_restart_or_skipped_recheck() {
    for recheck in [false, true] {
        let original = object_index_equation_module_v29(module(true, true, recheck, false));
        with_model(&original, |query, inventory, budget| {
            let guard = SourceIndexGuardLocationV29 {
                source: PendingSourceIndexGuardV29 {
                    instance: ProductionCallInstanceIdV1(0), assertion: SemanticBlockIdV1::from_index(0),
                    condition_event: 0, comparison_event: 0, load_anchor: 0, slot: 0,
                    value: FIRST, scalar: ScalarType::U32, length: 4, condition: CONDITION,
                    block: BlockId(90), success: BlockId(100), failure: BlockId(120),
                },
                load: coordinate(inventory, 90, 0, budget),
            };
            let target = coordinate(inventory, 100, 0, budget);
            assert_eq!(query.test_guard_bound_v29(&guard, target, &[], &[], budget).unwrap(), recheck);
            let restart = [SourceAddressLifetimeV29 {
                block: BlockId(100), gap: 0, sequence: 0, slot: 0, live: true,
            }];
            assert!(!query.test_guard_bound_v29(&guard, target, &[], &restart, budget).unwrap());
            let kill = [SourceAddressKillV29 { block: BlockId(100), gap: 0, slot: 0 }];
            assert!(!query.test_guard_bound_v29(&guard, target, &kill, &[], budget).unwrap());
        });
    }
}
