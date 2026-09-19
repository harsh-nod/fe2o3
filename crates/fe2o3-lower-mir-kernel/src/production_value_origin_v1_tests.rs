use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Constant,
    Function as KirFunction, Module, Operation, OperationKind, ScalarType, Signature, Terminator,
    Type, ValueDef,
};

const LIMIT: usize = 10_000_000;
const SUBJECT: Function = Function(2);

fn slice() -> Type {
    Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    )
}

fn block(id: u32, parameter: Option<u32>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    if let Some(parameter) = parameter {
        block
            .parameters
            .push(ValueDef::new(ValueId(parameter), slice()));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn branch(target: u32, argument: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![ValueId(argument)],
    })
}

fn conditional(a: u32, a_argument: u32, b: u32, b_argument: u32) -> Option<Terminator> {
    Some(Terminator::ConditionalBranch {
        condition: ValueId(99),
        then_target: BlockId(a),
        then_arguments: vec![ValueId(a_argument)],
        else_target: BlockId(b),
        else_arguments: vec![ValueId(b_argument)],
    })
}

fn with_inventory(
    blocks: Vec<BasicBlock>,
    inspect: impl FnOnce(
        &CanonicalKirInventoryV1<'_>,
        &VerifiedCanonicalKernelIrModuleV12,
        &mut Budget<'_>,
    ),
) {
    let mut module = Module::new("whole-value-origin");
    module.functions.push(KirFunction::external_import(
        "declaration",
        Signature::new(vec![slice()], vec![]),
    ));
    module.functions.push(KirFunction::internal_helper(
        "preceding",
        Signature::new(vec![slice(), slice()], vec![]),
        vec![ValueId(4_000_000_000), ValueId(98)],
        vec![block(77, None)],
    ));
    module.functions.push(KirFunction::internal_helper(
        "origin",
        Signature::new(vec![Type::BOOL, slice(), slice()], vec![]),
        vec![ValueId(99), ValueId(98), ValueId(97)],
        blocks,
    ));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17).unwrap();
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    inspect(&inventory, &owner, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 17);
}

fn inspect(blocks: Vec<BasicBlock>, expected: &[(u32, Option<u32>)]) {
    with_inventory(blocks, |inventory, owner, budget| {
        let floor = budget.storage();
        let function = SUBJECT;
        with_whole_value_origins_v1(inventory, owner, function, budget, |origins, budget| {
            let live = budget.storage();
            assert_eq!(
                live - floor,
                origins.origins.len() * std::mem::size_of::<Origin>()
            );
            for _ in 0..2 {
                for &(value, argument) in expected {
                    let actual = origins.resolve(ValueId(value), budget).unwrap();
                    assert_eq!(
                        actual,
                        argument
                            .map(|argument| Definition::FunctionArgument { function, argument }),
                        "value %{value}"
                    );
                    assert_eq!(budget.storage(), live);
                }
            }
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn prepared_origins_query_sparse_values_without_rebuilding_the_worklist() {
    let mut entry = block(77, None);
    entry.terminator = branch(1000, 98);
    let mut blocks = vec![entry];
    for i in 0..128 {
        let mut next = block(1000 + i, Some(4_000_000_000 + i));
        if i != 127 {
            next.terminator = branch(1001 + i, 4_000_000_000 + i);
        }
        blocks.push(next);
    }
    with_inventory(blocks, |inventory, owner, budget| {
        let floor = budget.storage();
        let start = budget.work();
        with_whole_value_origins_v1(inventory, owner, SUBJECT, budget, |origins, budget| {
            assert_eq!(budget.work() - start, 1543);
            let retained = budget.storage();
            assert_eq!(retained - floor, 131 * std::mem::size_of::<Origin>());
            let peak = budget.peak_storage();
            let start = budget.work();
            for i in 0..128 {
                inventory
                    .definition_index_for_value(SUBJECT, ValueId(4_000_000_000 + i), budget)
                    .unwrap()
                    .unwrap();
            }
            let expected_query_work = budget.work() - start + 128;
            let mut previous = None;
            for _ in 0..3 {
                let start = budget.work();
                for i in 0..128 {
                    assert_eq!(
                        origins.resolve(ValueId(4_000_000_000 + i), budget).unwrap(),
                        Some(Definition::FunctionArgument {
                            function: SUBJECT,
                            argument: 1
                        })
                    );
                    assert_eq!(budget.storage(), retained);
                    assert_eq!(budget.peak_storage(), peak);
                }
                let query_work = budget.work() - start;
                assert_eq!(
                    query_work, expected_query_work,
                    "only indexed lookup and table read"
                );
                if let Some(previous) = previous {
                    assert_eq!(query_work, previous);
                }
                previous = Some(query_work);
            }
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn prepared_origins_restore_storage_on_exact_resource_and_query_failures() {
    let mut entry = block(77, None);
    entry.terminator = branch(18, 98);
    with_inventory(
        vec![entry, block(18, Some(400))],
        |inventory, owner, outer| {
            let floor = outer.storage();
            let run = |work_limit, storage_limit, missing| {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let result = with_whole_value_origins_v1(
                    inventory,
                    owner,
                    SUBJECT,
                    &mut budget,
                    |origins, budget| {
                        assert_eq!(budget.work(), 46);
                        let live = budget.storage();
                        let result =
                            origins.resolve(ValueId(if missing { 401 } else { 400 }), budget);
                        assert_eq!(budget.storage(), live);
                        result
                    },
                )
                .and_then(|result| result);
                assert_eq!(budget.storage(), floor);
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, peak) = run(LIMIT, LIMIT, false);
            assert!(result.unwrap().is_some());
            assert!(run(work, peak, false).0.unwrap().is_some());
            assert!(matches!(
                run(work - 1, peak, false).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(work, peak - 1, false).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert!(matches!(
                run(LIMIT, LIMIT, true).0,
                Err(Error::InconsistentOwner)
            ));
            assert!(matches!(
                with_whole_value_origins_v1(inventory, owner, Function(3), outer, |_, _| panic!(
                    "invalid function reached consumer"
                )),
                Err(Error::InconsistentOwner)
            ));
            assert_eq!(outer.storage(), floor);
        },
    );
}

#[test]
fn origin_consumer_temporaries_preserve_nested_results_and_the_caller_floor() {
    let mut entry = block(77, None);
    entry.terminator = branch(18, 98);
    with_inventory(vec![entry, block(18, Some(400))], |inventory, owner, _| {
        for fail in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(23).unwrap();
            let preparation_bytes = 4
                * (std::mem::size_of::<Origin>() + 2 * std::mem::size_of::<usize>() + 1)
                + 2 * std::mem::size_of::<usize>();
            let result = with_whole_value_origins_v1(
                inventory,
                owner,
                SUBJECT,
                &mut budget,
                |origins, budget| {
                    assert_eq!(budget.work(), 46);
                    assert_eq!(budget.peak_storage(), 23 + preparation_bytes);
                    assert_eq!(budget.storage(), 23 + 4 * std::mem::size_of::<Origin>());
                    assert_eq!(origins.origins.len(), 4);
                    budget.reserve_storage(1000).unwrap();
                    if fail { Err("consumer refused") } else { Ok(7) }
                },
            )
            .unwrap();
            assert_eq!(result, if fail { Err("consumer refused") } else { Ok(7) });
            assert_eq!(budget.storage(), 23);
        }
    });
}

#[test]
fn prepared_origins_are_owner_bound_and_keep_global_definition_indices_local() {
    let mut entry = block(77, None);
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(4_000_000_000), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(7)),
    ));
    entry.terminator = branch(18, 98);
    with_inventory(
        vec![entry, block(18, Some(400))],
        |inventory, owner, budget| {
            with_whole_value_origins_v1(inventory, owner, SUBJECT, budget, |origins, budget| {
                assert!(origins.definitions.start > 0);
                assert_eq!(
                    origins.resolve(ValueId(98), budget).unwrap(),
                    Some(Definition::FunctionArgument {
                        function: SUBJECT,
                        argument: 1
                    })
                );
                assert_eq!(
                    origins.resolve(ValueId(400), budget).unwrap(),
                    origins.resolve(ValueId(98), budget).unwrap()
                );
                let result = inventory
                    .definition_for_value(SUBJECT, ValueId(4_000_000_000), budget)
                    .unwrap()
                    .unwrap()
                    .coordinate;
                assert!(matches!(result, Definition::Result { .. }));
                assert_eq!(
                    origins.resolve(ValueId(4_000_000_000), budget).unwrap(),
                    Some(result)
                );
            })
            .unwrap();
            with_whole_value_origins_v1(
                inventory,
                owner,
                Function(1),
                budget,
                |origins, budget| {
                    assert_eq!(
                        origins.resolve(ValueId(4_000_000_000), budget).unwrap(),
                        Some(Definition::FunctionArgument {
                            function: Function(1),
                            argument: 0
                        })
                    );
                },
            )
            .unwrap();
            let (foreign, storage) =
                VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                    owner.module(),
                    budget,
                )
                .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let floor = budget.storage();
            assert_eq!(foreign.canonical(), owner.canonical());
            assert!(matches!(
                with_whole_value_origins_v1(inventory, &foreign, SUBJECT, budget, |_, _| panic!(
                    "foreign owner reached consumer"
                )),
                Err(Error::InconsistentOwner)
            ));
            assert_eq!(budget.storage(), floor);
            drop(foreign);
            budget.release_storage(storage.retained_storage()).unwrap();
        },
    );
}

#[test]
fn whole_carrier_transport_accepts_grounded_loops() {
    let mut entry = block(77, None);
    entry.terminator = branch(18, 98);
    let mut header = block(18, Some(400));
    header.terminator = conditional(18, 400, 19, 400);
    inspect(
        vec![entry, header, block(19, Some(500))],
        &[(98, Some(1)), (400, Some(1)), (500, Some(1))],
    );
}

#[test]
fn every_duplicate_successor_occurrence_must_carry_the_same_slice() {
    for different in [false, true] {
        let mut entry = block(u32::MAX, None);
        entry.terminator = conditional(18, 98, 18, if different { 97 } else { 98 });
        inspect(
            vec![entry, block(18, Some(400))],
            &[(400, (!different).then_some(1))],
        );
    }
}

#[test]
fn an_ungrounded_cycle_does_not_manufacture_a_slice() {
    let mut left = block(18, Some(400));
    left.terminator = branch(19, 400);
    let mut right = block(19, Some(500));
    right.terminator = branch(18, 500);
    inspect(
        vec![block(77, None), left, right, block(20, Some(600))],
        &[(400, None), (500, None), (600, None)],
    );
}

#[test]
fn an_ungrounded_incoming_cycle_contaminates_a_grounded_join() {
    let mut entry = block(77, None);
    entry.terminator = branch(20, 98);
    let mut dormant = block(18, Some(400));
    dormant.terminator = conditional(18, 400, 20, 400);
    inspect(
        vec![entry, dormant, block(20, Some(600))],
        &[(98, Some(1)), (400, None), (600, None)],
    );
}

#[test]
fn entry_parameter_retains_unknown_initial_value_despite_a_grounded_backedge() {
    let mut entry = block(77, Some(400));
    entry.terminator = conditional(77, 98, 20, 400);
    inspect(
        vec![entry, block(20, Some(500))],
        &[(400, None), (500, None)],
    );
}

#[test]
fn irreducible_cycle_requires_every_entry_to_agree() {
    for different in [false, true] {
        let mut entry = block(77, None);
        entry.terminator = conditional(18, 98, 19, if different { 97 } else { 98 });
        let mut left = block(18, Some(400));
        left.terminator = conditional(19, 400, 20, 400);
        let mut right = block(19, Some(500));
        right.terminator = conditional(18, 500, 20, 500);
        let expected = (!different).then_some(1);
        inspect(
            vec![entry, left, right, block(20, Some(600))],
            &[(400, expected), (500, expected), (600, expected)],
        );
    }
}
