use super::*;

mod native_source_queries {
    include!("canonical_kir_inventory_native_source_queries_v1_tests.rs");
}
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, CanonicalKernelIrWorkBudgetV1, CheckedBinaryOperator,
    Constant, CopyNonOverlappingContract, FunctionRole, IntegerSwitchCase, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, MemoryElementType, MemoryIntrinsicOperation, MemoryLayout, Module,
    ScalarType, Signature, ValueDef, VerificationContractKeyV12, VerificationContractOperationV12,
    WorkgroupPipelineEventKindV12,
};

fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .expect("fixture is a semantically valid exact V12 module");
    (owner, storage.retained_storage())
}

#[test]
fn definition_index_and_reference_queries_have_identical_lookup_budgets() {
    let (owner, owner_storage) = admit(&mixed_module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(owner_storage).unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let mut cases = vec![
        (FunctionCoordinate(u32::MAX), ValueId(4_000_000_000)),
        (FunctionCoordinate(2), ValueId(u32::MAX)),
    ];
    let mut kinds = std::collections::BTreeSet::new();
    for function in inventory.functions() {
        for row in &inventory.definitions()[function.definitions.clone()] {
            if let Some(value) = row.value {
                cases.push((function.coordinate, value));
                kinds.insert(match row.coordinate {
                    Definition::FunctionArgument { .. } => 0,
                    Definition::BlockArgument { .. } => 1,
                    Definition::Result { .. } => 2,
                });
            }
        }
    }
    assert_eq!(kinds.len(), 3);
    for (function, value) in cases {
        let lookup = |indexed, limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, 0);
            let result = if indexed {
                inventory
                    .definition_index_for_value(function, value, &mut budget)
                    .map(|index| index.map(|index| inventory.definitions()[index].coordinate))
            } else {
                inventory
                    .definition_for_value(function, value, &mut budget)
                    .map(|row| row.map(|row| row.coordinate))
            };
            assert_eq!(budget.storage(), 0);
            (result, budget.work())
        };
        let measured = lookup(false, 10_000);
        assert_eq!(lookup(true, 10_000), measured);
        assert_eq!(lookup(true, measured.1), measured);
        assert!(matches!(
            lookup(true, measured.1 - 1).0,
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_)))
        ));
        assert_eq!(lookup(false, measured.1 - 1), lookup(true, measured.1 - 1));
    }
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(owner_storage).unwrap();
    assert_eq!(budget.storage(), 0);
}

fn mixed_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let parameter = ValueId(4_000_000_000);
    let mut entry = BasicBlock::new(BlockId(4_000_000_000));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(91), scalar.clone()),
            OperationKind::Constant(Constant::U32(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(92), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(999), pointer.clone()),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1000), pointer),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(999),
                value: parameter,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(999),
                destination: ValueId(1000),
                count: ValueId(92),
                element: MemoryElementType::Scalar(ScalarType::U32),
                source_address_space: AddressSpace::Private,
                destination_address_space: AddressSpace::Private,
                layout: MemoryLayout::new(4, 4),
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), scalar.clone()),
            OperationKind::Load {
                pointer: ValueId(1000),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(22), scalar.clone()),
            ValueDef::new(ValueId(23), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(10),
            ValueId(91),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(72), scalar.clone()),
            OperationKind::Call {
                callee: "external".into(),
                arguments: vec![ValueId(22)],
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(23),
        then_target: BlockId(77),
        then_arguments: vec![ValueId(22)],
        else_target: BlockId(77),
        else_arguments: vec![ValueId(10)],
    });
    let mut join = BasicBlock::new(BlockId(77));
    join.parameters
        .push(ValueDef::new(ValueId(98), scalar.clone()));
    join.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(98),
        cases: vec![
            IntegerSwitchCase {
                value: Constant::U32(0),
                target: BlockId(5),
                arguments: vec![ValueId(91)],
            },
            IntegerSwitchCase {
                value: Constant::U32(1),
                target: BlockId(5),
                arguments: vec![ValueId(98)],
            },
        ],
        default_target: BlockId(77),
        default_arguments: vec![ValueId(91)],
    });
    let mut exit = BasicBlock::new(BlockId(5));
    exit.parameters
        .push(ValueDef::new(ValueId(8), scalar.clone()));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(8)],
    });
    let shared = Function::internal_helper(
        "shared",
        Signature::new(vec![scalar.clone()], vec![scalar.clone()]),
        vec![parameter],
        vec![entry, join, exit],
    );
    let root = |name: &str| {
        let mut block = BasicBlock::new(BlockId(91));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(8), scalar.clone()),
            OperationKind::Call {
                callee: "shared".into(),
                arguments: vec![ValueId(7)],
            },
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        Function::kernel_entry(
            name,
            Signature::new(vec![scalar.clone()], vec![]),
            vec![ValueId(7)],
            vec![block],
        )
    };
    let mut module = Module::new("canonical-inventory");
    module.functions = vec![
        Function::external_import(
            "external",
            Signature::new(vec![scalar.clone()], vec![scalar.clone()]),
        ),
        root("root_b"),
        shared,
        root("root_a"),
    ];
    module.kernels = ["root_a", "root_b"]
        .into_iter()
        .map(|name| {
            Kernel::new(
                name,
                name,
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(1),
                },
            )
        })
        .collect();
    module
}

#[test]
fn mixed_ssa_inventory_borrows_exact_definitions_and_preserves_every_occurrence() {
    let (owner, floor) = admit(&mixed_module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, floor + 1_000_000);
    budget.reserve_storage(floor).unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(inventory.functions().len(), 4);
    assert_eq!(inventory.blocks().len(), 5);
    assert_eq!(inventory.operations().len(), 11);
    assert_eq!(inventory.definitions().len(), 16);
    assert_eq!(inventory.effects().len(), 6);
    assert_eq!(inventory.calls().len(), 3);
    assert_eq!(inventory.edges().len(), 5);
    assert_eq!(inventory.edge_arguments().len(), 5);
    assert_eq!(inventory.kernels()[0].entry, FunctionCoordinate(3));
    assert_eq!(inventory.kernels()[1].entry, FunctionCoordinate(1));
    assert_eq!(
        inventory.functions()[0].function.role,
        FunctionRole::ExternalImport
    );
    assert!(
        inventory.definitions()[inventory.functions()[0].definitions.start]
            .value
            .is_none()
    );
    assert!(std::ptr::eq(inventory.owner(), &owner));
    assert_eq!(inventory.identity(), *owner.canonical().identity());
    for operation in inventory.operations() {
        let coordinate = operation.coordinate;
        let actual = &owner.module().functions[coordinate.block.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[coordinate.block.block as usize]
            .operations[coordinate.operation as usize];
        assert!(std::ptr::eq(operation.operation, actual));
        assert_eq!(
            operation.traps(),
            CanonicalKirBehaviorAnalysisV1::NotAnalyzed
        );
        assert_eq!(
            operation.convergence(),
            CanonicalKirBehaviorAnalysisV1::NotAnalyzed
        );
    }
    let checked = inventory
        .operations()
        .iter()
        .find(|row| row.results.len() == 2)
        .unwrap();
    assert_eq!(
        inventory.definitions()[checked.results.start].value,
        Some(ValueId(22))
    );
    assert_eq!(
        inventory.definitions()[checked.results.start + 1].value,
        Some(ValueId(23))
    );
    assert_eq!(
        inventory.definitions()[checked.results.start + 1].coordinate,
        Definition::Result {
            operation: checked.coordinate,
            result: 1,
        }
    );
    let parameter = inventory
        .definition_for_value(FunctionCoordinate(2), ValueId(4_000_000_000), &mut budget)
        .unwrap()
        .unwrap();
    let parameter_index = inventory
        .definition_index_for_value(FunctionCoordinate(2), ValueId(4_000_000_000), &mut budget)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(
        parameter,
        &inventory.definitions()[parameter_index]
    ));
    assert_eq!(
        parameter.coordinate,
        Definition::FunctionArgument {
            function: FunctionCoordinate(2),
            argument: 0
        }
    );
    assert!(
        inventory
            .definition_index_for_value(FunctionCoordinate(1), ValueId(4_000_000_000), &mut budget)
            .unwrap()
            .is_none()
    );
    for operand in inventory.uses() {
        assert_eq!(
            inventory.definitions()[operand.definition].value,
            Some(operand.value)
        );
    }
    let helper_calls = inventory
        .calls()
        .iter()
        .filter(|call| call.callee == "shared")
        .collect::<Vec<_>>();
    assert_eq!(helper_calls.len(), 2);
    assert!(
        helper_calls
            .iter()
            .all(|call| call.target == Some(FunctionCoordinate(2)))
    );
    assert_eq!(
        inventory
            .calls()
            .iter()
            .find(|call| call.callee == "external")
            .unwrap()
            .target,
        Some(FunctionCoordinate(0))
    );

    let copy = inventory
        .operations()
        .iter()
        .find(|row| {
            matches!(
                row.operation.kind,
                OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping { .. })
            )
        })
        .unwrap();
    let effects = &inventory.effects()[copy.effects.clone()];
    assert_eq!(effects.len(), 2);
    assert_eq!(
        effects[0].coordinate,
        Access {
            operation: copy.coordinate,
            effect: 0
        }
    );
    assert_eq!(
        effects[1].coordinate,
        Access {
            operation: copy.coordinate,
            effect: 1
        }
    );
    assert!(matches!(
        effects[0].effect,
        KirLocalMemoryEffectRefV1::Read(AddressSpace::Private)
    ));
    assert!(matches!(
        effects[1].effect,
        KirLocalMemoryEffectRefV1::Write(AddressSpace::Private)
    ));

    let edges = inventory.edges();
    assert_eq!(edges[0].target, edges[1].target);
    assert_ne!(edges[0].coordinate, edges[1].coordinate);
    assert_eq!(edges[0].arguments, [ValueId(22)]);
    assert_eq!(edges[1].arguments, [ValueId(10)]);
    assert_eq!(edges[2].target, edges[3].target);
    assert_ne!(edges[2].arguments, edges[3].arguments);
    assert_eq!(edges[4].coordinate.source, edges[4].target); // Exact loop backedge.
    for edge in edges {
        let block = &owner.module().functions[edge.coordinate.source.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[edge.coordinate.source.block as usize];
        let mut occurrence = 0;
        block
            .terminator
            .as_ref()
            .unwrap()
            .try_visit_edges_v1(|_, arguments| {
                if occurrence == edge.coordinate.successor {
                    assert!(std::ptr::eq(arguments.as_ptr(), edge.arguments.as_ptr()));
                }
                occurrence += 1;
                Ok::<_, ()>(())
            })
            .unwrap();
        for binding in &inventory.edge_arguments()[edge.bindings.clone()] {
            assert_eq!(
                inventory.definitions()[binding.incoming_definition].value,
                Some(binding.value)
            );
            assert!(
                matches!(inventory.definitions()[binding.target_definition].coordinate,
                Definition::BlockArgument { block, argument } if block == edge.target
                    && argument == binding.coordinate.argument)
            );
        }
    }
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn equal_bytes_do_not_substitute_another_connected_owner() {
    let source = mixed_module();
    let (first, _) = admit(&source);
    let (second, _) = admit(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let (inventory, _) = CanonicalKirInventoryV1::derive(&first, &mut budget).unwrap();
    assert_eq!(first.canonical().identity(), second.canonical().identity());
    assert!(inventory.belongs_to(&first));
    assert!(!inventory.belongs_to(&second));
}

#[test]
fn compiler_ordering_is_separate_from_local_memory_and_unanalyzed_behavior() {
    let mut block = BasicBlock::new(BlockId(99));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(11),
                kind: WorkgroupPipelineEventKindV12::Stage,
                storage: ValueId(42),
                epoch: ValueId(100),
            },
        ),
    ));
    block.terminator = Some(Terminator::Unreachable);
    let mut module = Module::new("inert-contract");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                Type::pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(42), ValueId(100)],
        vec![block],
    ));
    let (owner, _) = admit(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let (inventory, _) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    assert!(inventory.effects().is_empty());
    assert!(
        inventory.operations()[0]
            .compiler_ordering()
            .has_ordered_verification_contract()
    );
    assert_eq!(
        inventory.operations()[0].traps(),
        CanonicalKirBehaviorAnalysisV1::NotAnalyzed
    );
    assert_eq!(inventory.uses().len(), 2);
    // Key 11 remains inert: inventory creation does not authenticate a catalog.
}

#[test]
fn independently_specified_empty_budget_boundaries_restore_prefixed_floors() {
    let (owner, owner_storage) = admit(&Module::new("empty"));
    let floor = owner_storage + 7;
    let payload = size_of::<CanonicalKirInventoryV1<'_>>();
    // Empty census and fill each visit the module once; no vectors allocate,
    // no rows link, no indexes sort. Exactly two logical work units.
    for (work_allowance, storage_allowance, succeeds) in [
        (2, payload, true),
        (1, payload, false),
        (2, payload - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + work_allowance);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, floor + storage_allowance);
        budget.reserve_storage(floor).unwrap();
        let result = CanonicalKirInventoryV1::derive(&owner, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert_eq!(result.is_ok(), succeeds);
        match result {
            Ok((inventory, receipt)) => {
                assert_eq!(budget.work(), 13);
                assert_eq!(receipt.retained_storage(), payload);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                drop(inventory);
                budget.release_storage(receipt.retained_storage()).unwrap();
            }
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(error.actual(), 13);
                assert_eq!(budget.work(), 12);
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Storage(error))) => {
                assert_eq!(error.actual(), floor + payload);
                assert_eq!(budget.work(), 12);
                assert_eq!(budget.peak_storage(), floor);
            }
            other => panic!("unexpected boundary result: {other:?}"),
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn failed_large_inventory_preserves_first_failure_history_and_owner() {
    let (owner, owner_storage) = admit(&mixed_module());
    let original = *owner.canonical().identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20);
    work.charge_work(3).unwrap();
    {
        let mut budget = Budget::new(&mut work, owner_storage + 1024);
        budget.reserve_storage(owner_storage).unwrap();
        assert!(budget.charge_work(usize::MAX).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let result = CanonicalKirInventoryV1::derive(&owner, &mut budget);
        assert!(matches!(
            result,
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(_)))
        ));
        assert_eq!(budget.storage(), owner_storage);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
    assert_eq!(work.failed_work(), Some(usize::MAX));
    assert_eq!(*owner.canonical().identity(), original);
}

#[test]
fn excess_allocator_capacity_does_not_admit_extra_logical_records() {
    fn push(values: &mut Vec<u32>, admitted: usize, value: u32) -> Result<()> {
        append!(*values, admitted, value);
        Ok(())
    }
    let mut values = Vec::with_capacity(8);
    assert!(values.capacity() > 1);
    push(&mut values, 1, 42).unwrap();
    assert_eq!(
        push(&mut values, 1, 99),
        Err(CanonicalKirInventoryErrorV1::InconsistentOwner)
    );
    assert_eq!(values, [42]);
}

#[test]
fn independently_bounded_sort_failure_drops_admitted_vectors_before_floor_restore() {
    let mut module = Module::new("sort-failure");
    for name in ["z", "a"] {
        module.functions.push(Function::external_import(
            name,
            Signature::new(vec![Type::INDEX], vec![]),
        ));
    }
    let (owner, owner_storage) = admit(&module);
    let identity = *owner.canonical().identity();
    let floor = owner_storage + 7;
    let payload = size_of::<CanonicalKirInventoryV1<'_>>()
        + 2 * size_of::<CanonicalKirFunctionRefV1<'_>>()
        + 2 * size_of::<CanonicalKirDefinitionRefV1<'_>>()
        + 2 * size_of::<(&str, FunctionCoordinate)>();
    // Census 5 + allocations 3 + fill 5 + index scans 4 = 17.
    // First heap visit takes one unit. Comparing one-byte names takes two:
    // with 19 available units, that comparison must reject after accepting 18.
    let prefix = 5;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(prefix + 19);
    work.charge_work(prefix).unwrap();
    {
        let mut budget = Budget::new(&mut work, floor + payload);
        budget.reserve_storage(floor).unwrap();
        let error = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap_err();
        let CanonicalKirInventoryErrorV1::Resource(Resource::Work(error)) = error else {
            panic!("expected the first heap comparison to fail");
        };
        assert_eq!(error.actual(), prefix + 20);
        assert_eq!(budget.work(), prefix + 18);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor + payload);
        assert_eq!(budget.failed_storage(), None);
    }
    assert_eq!(work.failed_work(), Some(prefix + 20));
    assert_eq!(*owner.canonical().identity(), identity);
    assert_eq!(owner.module().functions.len(), 2);
}

#[test]
fn heapsort_and_lookup_are_sparse_and_do_not_depend_on_source_order() {
    for mut keys in [vec![9, 1, 4, 8, 0], vec![2, 2, 1], vec![], vec![u32::MAX]] {
        let mut expected = keys.clone();
        expected.sort_unstable();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
        let mut budget = Budget::new(&mut work, 0);
        heap_sort(&mut keys, &mut budget, |a, b, budget| {
            budget.charge_work(1)?;
            Ok(a.cmp(b))
        })
        .unwrap();
        assert_eq!(keys, expected);
        for needle in [0, 1, 2, 9, u32::MAX] {
            let found = find(&keys, &mut budget, |value, budget| {
                budget.charge_work(1)?;
                Ok(value.cmp(&needle))
            })
            .unwrap();
            assert_eq!(
                found.map(|index| keys[index]),
                keys.iter().find(|value| **value == needle).copied()
            );
        }
    }
}

#[test]
fn independently_specified_declaration_budget_covers_nonempty_retained_vectors() {
    let mut module = Module::new("declaration");
    module.functions.push(Function::external_import(
        "f",
        Signature::new(vec![Type::INDEX], vec![]),
    ));
    let (owner, owner_storage) = admit(&module);
    let floor = owner_storage + 13;
    let payload = size_of::<CanonicalKirInventoryV1<'_>>()
        + size_of::<CanonicalKirFunctionRefV1<'_>>()
        + size_of::<CanonicalKirDefinitionRefV1<'_>>()
        + size_of::<(&str, FunctionCoordinate)>();
    // Census 3 + three vector reservations + fill 3 + index scans 2.
    // No values exist, so no value index, comparisons, searches or swaps.
    for (allowance, bytes, succeeds) in [
        (11, payload, true),
        (10, payload, false),
        (11, payload - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(17 + allowance);
        work.charge_work(17).unwrap();
        let mut budget = Budget::new(&mut work, floor + bytes);
        budget.reserve_storage(floor).unwrap();
        let result = CanonicalKirInventoryV1::derive(&owner, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert_eq!(result.is_ok(), succeeds);
        match result {
            Ok((inventory, receipt)) => {
                assert_eq!(budget.work(), 28);
                assert_eq!(budget.peak_storage(), floor + payload);
                assert_eq!(receipt.retained_storage(), payload);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(inventory.functions().len(), 1);
                assert_eq!(inventory.definitions().len(), 1);
                assert!(inventory.definitions()[0].value.is_none());
                drop(inventory);
                budget.release_storage(receipt.retained_storage()).unwrap();
            }
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Work(error))) => {
                assert_eq!(error.actual(), 28);
                assert_eq!(budget.work(), 27);
                assert_eq!(budget.peak_storage(), floor + payload);
            }
            Err(CanonicalKirInventoryErrorV1::Resource(Resource::Storage(error))) => {
                assert_eq!(error.actual(), floor + payload);
                assert_eq!(budget.work(), 23);
                assert_eq!(
                    budget.peak_storage(),
                    floor + payload - size_of::<(&str, FunctionCoordinate)>()
                );
            }
            other => panic!("unexpected boundary result: {other:?}"),
        }
        assert_eq!(budget.storage(), floor);
    }
}
