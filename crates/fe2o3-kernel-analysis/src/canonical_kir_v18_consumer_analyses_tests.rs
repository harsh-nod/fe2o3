use super::*;
use crate::canonical_kir_inventory_v1::v18_tests::{admit, storage_module, with_inventory};
use crate::{
    CanonicalKirCallEffectDecisionV1 as Decision, CanonicalKirCallEffectErrorV1 as CallError,
    CanonicalKirCallEffectKindV1 as Effect, CanonicalKirCallEffectsV18, CanonicalKirInventoryV18,
    CanonicalKirMemorySsaErrorV1, CanonicalKirMemorySsaLimitsV1,
    CanonicalKirMemorySsaNodeV1 as MemoryNode, CanonicalKirMemorySsaV18,
};
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as Function, Module, ValueId,
};

const LIMIT: usize = 10_000_000;

#[test]
fn v18_operation_effect_query_keeps_recursive_and_external_calls_incomplete() {
    use fe2o3_kernel_ir::{Function as KirFunction, OperationKind};
    for external in [false, true] {
        let mut module = storage_module(AddressSpace::Private, false);
        if external {
            module.functions[0] =
                KirFunction::external_import("storage", module.functions[0].signature.clone());
        } else {
            let original =
                module.functions[1].body.as_mut().unwrap().blocks[0].operations[0].clone();
            let mut recursive = original.clone();
            recursive.results[0].id = ValueId(3);
            let OperationKind::Call { callee, .. } = &mut recursive.kind else {
                unreachable!()
            };
            *callee = "caller".into();
            let operations = &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations;
            operations.push(recursive);
            let mut repeated = original;
            repeated.results[0].id = ValueId(4);
            operations.push(repeated);
        }
        with_inventory(&module, |inventory, budget| {
            let floor = budget.storage();
            let (report, receipt) =
                CanonicalKirCallEffectsV18::derive_v18(inventory, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let live = budget.storage();
            assert_eq!(
                report.decision(Function(1), budget).unwrap(),
                Decision::Incomplete
            );
            let caller = &inventory.functions()[1];
            let calls = &inventory.calls()[caller.calls.clone()];
            assert_eq!(calls.len(), if external { 1 } else { 3 });
            for (ordinal, call) in calls.iter().enumerate() {
                assert_eq!(call.coordinate.block.function, Function(1));
                assert_eq!(call.coordinate.operation as usize, ordinal);
                assert_eq!(
                    call.target,
                    Some(Function(if !external && ordinal == 1 { 1 } else { 0 }))
                );
                assert_eq!(
                    report.operation_decision(call.coordinate, budget).unwrap(),
                    if external || ordinal == 1 {
                        Decision::Incomplete
                    } else {
                        Decision::CompleteNonempty
                    }
                );
                assert_eq!(budget.storage(), live);
            }
            drop(report);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn v18_operation_effect_query_uses_registered_summary_not_incomplete_declaration() {
    use fe2o3_kernel_ir::{
        Constant, F32MathFunction, FloatOperation, Operation, OperationKind, Type, ValueDef,
    };
    let mut module = storage_module(AddressSpace::Private, false);
    let sqrt = FloatOperation::F32Math {
        function: F32MathFunction::Sqrt,
        implementation: F32MathFunction::Sqrt.required_implementation(),
        arguments: vec![ValueId(50)],
    };
    module.functions.push(sqrt.declaration());
    let operations = &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations;
    operations.push(Operation::effect_free(
        ValueDef::new(ValueId(50), Type::F32),
        OperationKind::Constant(Constant::F32Bits(4.0f32.to_bits())),
    ));
    operations.push(sqrt.operation(ValueId(51)));
    with_inventory(&module, |inventory, budget| {
        let floor = budget.storage();
        let (report, receipt) = CanonicalKirCallEffectsV18::derive_v18(inventory, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let live = budget.storage();
        assert_eq!(
            report.decision(Function(2), budget).unwrap(),
            Decision::Incomplete
        );
        let calls = &inventory.calls()[inventory.functions()[1].calls.clone()];
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].target, Some(Function(0)));
        assert_eq!(calls[1].target, Some(Function(2)));
        assert!(
            calls[1]
                .operation
                .has_complete_effect_summary_with_budget_v1(budget)
                .unwrap()
        );
        assert_eq!(
            report
                .operation_decision(calls[0].coordinate, budget)
                .unwrap(),
            Decision::CompleteNonempty
        );
        assert_eq!(
            report
                .operation_decision(calls[1].coordinate, budget)
                .unwrap(),
            Decision::CompleteEmpty
        );
        assert_eq!(
            report.decision(Function(1), budget).unwrap(),
            Decision::CompleteNonempty
        );
        assert_eq!(budget.storage(), live);
        drop(report);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn v18_operation_effect_query_reuses_exact_classifier_and_callee_states() {
    with_inventory(
        &storage_module(AddressSpace::Private, false),
        |inventory, budget| {
            let floor = budget.storage();
            let (report, receipt) =
                CanonicalKirCallEffectsV18::derive_v18(inventory, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            let live = budget.storage();
            for operation in inventory.operations() {
                let expected = if matches!(
                    operation.operation.kind,
                    fe2o3_kernel_ir::OperationKind::Call { .. }
                ) || !operation.effects.is_empty()
                    || !operation.compiler_ordering().is_empty()
                {
                    Decision::CompleteNonempty
                } else {
                    Decision::CompleteEmpty
                };
                assert_eq!(
                    report
                        .operation_decision(operation.coordinate, budget)
                        .unwrap(),
                    expected
                );
                assert_eq!(budget.storage(), live);
            }
            let original = inventory.operations()[0].coordinate;
            let mut invalid = original;
            invalid.block.function = Function(u32::MAX);
            assert!(matches!(
                report.operation_decision(invalid, budget),
                Err(CallError::InvalidFunction(Function(u32::MAX)))
            ));
            invalid = original;
            invalid.block.block = u32::MAX;
            assert_eq!(
                report.operation_decision(invalid, budget),
                Err(CallError::InconsistentInventory)
            );
            invalid = original;
            invalid.operation = u32::MAX;
            assert_eq!(
                report.operation_decision(invalid, budget),
                Err(CallError::InconsistentInventory)
            );
            assert_eq!(budget.storage(), live);
            drop(report);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        },
    );
}

#[test]
fn v18_pure_operation_query_has_independent_exact_five_step_boundary() {
    let module = storage_module(AddressSpace::Private, false);
    let (owner, owner_bytes) = admit(&module);
    for available in [5, 4] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(owner_bytes).unwrap();
        let (inventory, inventory_receipt) =
            CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
        budget
            .reserve_storage(inventory_receipt.retained_storage())
            .unwrap();
        let (report, receipt) =
            CanonicalKirCallEffectsV18::derive_v18(&inventory, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let operation = inventory
            .operations()
            .iter()
            .find(|operation| {
                matches!(
                    operation.operation.kind,
                    fe2o3_kernel_ir::OperationKind::Constant(_)
                )
            })
            .unwrap()
            .coordinate;
        let padding = LIMIT
            .checked_sub(available)
            .unwrap()
            .checked_sub(budget.work())
            .unwrap();
        budget.charge_work(padding).unwrap();
        let floor = budget.storage();
        let peak = budget.peak_storage();
        let result = report.operation_decision(operation, &mut budget);
        if available == 5 {
            assert_eq!(result, Ok(Decision::CompleteEmpty));
            assert_eq!(budget.work(), LIMIT);
        } else {
            assert!(
                matches!(result, Err(CallError::Resource(Resource::Work(error))) if error.actual() == LIMIT + 1 && error.limit() == LIMIT)
            );
            assert_eq!(budget.work(), LIMIT - 4);
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), peak);
        drop(report);
        budget.release_storage(receipt.retained_storage()).unwrap();
        drop(inventory);
        budget
            .release_storage(inventory_receipt.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), owner_bytes);
    }
}

#[test]
fn v18_shared_sparse_solver_keeps_storage_values_dynamic_and_edges_distinct() {
    let module = storage_module(AddressSpace::Private, false);
    with_inventory(&module, |inventory, budget| {
        let floor = budget.storage();
        let (report, receipt) =
            CanonicalKirSparseV18::derive_v18(inventory, Default::default(), budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(report.belongs_to(inventory));
        assert!(std::ptr::eq(
            report.inventory().owner().module(),
            inventory.owner().module()
        ));
        for (value, bits) in [(13, 1), (4_000_000_000, 23), (6, 1)] {
            let index = inventory
                .definition_index_for_value(Function(0), ValueId(value), budget)
                .unwrap()
                .unwrap();
            assert!(matches!(report.value(index), Some(Value::Constant(c)) if c.bits() == bits));
        }
        for value in [900, 400, 1, 99, 44, 81, 82, 83, 84, 85, 86, 2] {
            let index = inventory
                .definition_index_for_value(Function(0), ValueId(value), budget)
                .unwrap()
                .unwrap();
            assert_eq!(report.value(index), Some(Value::Dynamic));
        }
        assert_eq!(report.edge_executable(0), Some(true));
        assert_eq!(report.edge_executable(1), Some(false));
        assert_eq!(inventory.edges().len(), 2);
        assert_eq!(report.block_executable(1), Some(true));
        drop(report);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn v18_memory_ssa_and_call_effects_preserve_every_original_storage_occurrence() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        for volatile in [false, true] {
            with_inventory(&storage_module(space, volatile), |inventory, budget| {
                let floor = budget.storage();
                let (memory, memory_storage) =
                    CanonicalKirMemorySsaV18::derive_v18(inventory, Default::default(), budget)
                        .unwrap();
                budget
                    .reserve_storage(memory_storage.retained_storage())
                    .unwrap();
                let (calls, call_storage) =
                    CanonicalKirCallEffectsV18::derive_v18(inventory, budget).unwrap();
                budget
                    .reserve_storage(call_storage.retained_storage())
                    .unwrap();
                assert!(memory.belongs_to(inventory));
                assert!(calls.belongs_to(inventory));
                for ordinal in 0..18 {
                    let coordinate = inventory.operations()[ordinal].coordinate;
                    let node = memory.operation(coordinate, budget).unwrap();
                    match ordinal {
                        6 | 14 | 16 if !volatile => {
                            assert!(
                                matches!(memory.node(node.unwrap(), budget).unwrap(), MemoryNode::Use { operation, .. } if *operation == coordinate)
                            );
                        }
                        0 | 5 | 6 | 7 | 8 | 9 | 12 | 13 | 14 | 16 => {
                            assert!(
                                matches!(memory.node(node.unwrap(), budget).unwrap(), MemoryNode::Def { operation, .. } if *operation == coordinate)
                            );
                        }
                        _ => assert_eq!(node, None),
                    }
                }
                let join = memory
                    .block_entry(inventory.blocks()[1].coordinate, budget)
                    .unwrap();
                let inputs = memory.phi_inputs(join, budget).unwrap();
                assert_eq!(inputs.len(), 2);
                assert_ne!(inputs[0].source(), inputs[1].source());
                for function in [Function(0), Function(1)] {
                    assert_eq!(
                        calls.decision(function, budget).unwrap(),
                        Decision::CompleteNonempty
                    );
                    let mut count = 0;
                    calls
                        .try_visit(function, budget, |occurrence| {
                            assert!(std::ptr::eq(occurrence.inventory(), inventory));
                            assert_eq!(occurrence.root(), function);
                            assert_eq!(occurrence.function(), Function(0));
                            assert_eq!(
                                occurrence.call_path(),
                                if function == Function(0) {
                                    &[][..]
                                } else {
                                    &[0][..]
                                }
                            );
                            let Effect::Physical(effect) = occurrence.kind() else {
                                panic!("unexpected ordering-only occurrence")
                            };
                            assert!(std::ptr::eq(effect, &inventory.effects()[count]));
                            count += 1;
                            Ok::<_, CallError>(())
                        })
                        .unwrap();
                    assert_eq!(count, 11);
                }
                drop(calls);
                budget
                    .release_storage(call_storage.retained_storage())
                    .unwrap();
                drop(memory);
                budget
                    .release_storage(memory_storage.retained_storage())
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn v18_reports_do_not_belong_to_a_second_equal_owner_inventory() {
    let module = storage_module(AddressSpace::Private, false);
    let (owner, owner_bytes) = admit(&module);
    let (other, other_bytes) = admit(&module);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(owner_bytes + other_bytes + 17)
        .unwrap();
    let floor = budget.storage();
    let (inventory, receipt) = CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (second, second_receipt) =
        CanonicalKirInventoryV18::derive_v18(&other, &mut budget).unwrap();
    budget
        .reserve_storage(second_receipt.retained_storage())
        .unwrap();
    let (sparse, sparse_receipt) =
        CanonicalKirSparseV18::derive_v18(&inventory, Default::default(), &mut budget).unwrap();
    budget
        .reserve_storage(sparse_receipt.retained_storage())
        .unwrap();
    let (memory, memory_receipt) =
        CanonicalKirMemorySsaV18::derive_v18(&inventory, Default::default(), &mut budget).unwrap();
    budget
        .reserve_storage(memory_receipt.retained_storage())
        .unwrap();
    let (calls, call_receipt) =
        CanonicalKirCallEffectsV18::derive_v18(&inventory, &mut budget).unwrap();
    budget
        .reserve_storage(call_receipt.retained_storage())
        .unwrap();
    assert!(!sparse.belongs_to(&second));
    assert!(!memory.belongs_to(&second));
    assert!(!calls.belongs_to(&second));
    drop(calls);
    budget
        .release_storage(call_receipt.retained_storage())
        .unwrap();
    drop(memory);
    budget
        .release_storage(memory_receipt.retained_storage())
        .unwrap();
    drop(sparse);
    budget
        .release_storage(sparse_receipt.retained_storage())
        .unwrap();
    drop(second);
    budget
        .release_storage(second_receipt.retained_storage())
        .unwrap();
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn v18_sparse_empty_budget_is_independently_exact_and_one_short() {
    with_inventory(&Module::new("empty-v18-analyses"), |inventory, outer| {
        let floor = outer.storage();
        let peak = floor + size_of::<Engine<'_, '_, VerifiedCanonicalKernelIrModuleV18>>();
        for (work_limit, storage_limit, success) in
            [(8, peak, true), (7, peak, false), (8, peak - 1, false)]
        {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result =
                CanonicalKirSparseV18::derive_v18(inventory, Default::default(), &mut budget);
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.storage(), floor);
            match result {
                Ok((report, receipt)) => {
                    assert_eq!(budget.work(), 8);
                    assert_eq!(budget.peak_storage(), peak);
                    assert_eq!(
                        receipt.retained_storage(),
                        size_of::<CanonicalKirSparseV18<'_, '_>>()
                    );
                    assert!(report.values().is_empty());
                    drop(report);
                }
                Err(CanonicalKirSparseErrorV1::Resource(Resource::Work(error))) => {
                    assert_eq!(error.limit(), 7);
                    assert_eq!(error.actual(), 8);
                }
                Err(CanonicalKirSparseErrorV1::Resource(Resource::Storage(error))) => {
                    assert_eq!(error.limit(), peak - 1);
                    assert_eq!(error.actual(), peak);
                }
                other => panic!("wrong independent resource failure: {other:?}"),
            }
        }
    });
}

#[test]
fn v18_analysis_limits_reject_before_claiming_complete_facts() {
    with_inventory(
        &storage_module(AddressSpace::Private, false),
        |inventory, budget| {
            let floor = budget.storage();
            let sparse = CanonicalKirSparseV18::derive_v18(
                inventory,
                CanonicalKirSparseLimitsV1 {
                    definitions: 0,
                    ..Default::default()
                },
                budget,
            );
            assert!(matches!(
                sparse,
                Err(CanonicalKirSparseErrorV1::InputLimit { .. })
            ));
            let memory = CanonicalKirMemorySsaV18::derive_v18(
                inventory,
                CanonicalKirMemorySsaLimitsV1 {
                    effects: 0,
                    ..Default::default()
                },
                budget,
            );
            assert!(matches!(
                memory,
                Err(CanonicalKirMemorySsaErrorV1::InputLimit { .. })
            ));
            assert_eq!(budget.storage(), floor);
        },
    );
}
