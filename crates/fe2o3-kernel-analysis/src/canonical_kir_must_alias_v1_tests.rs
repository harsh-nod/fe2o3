use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, CastKind, Function, Module,
    Operation, ScalarType, Signature, TargetCapability, Terminator, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12, WorkgroupMemory, WorkgroupMemoryExtent,
};

const LIMIT: usize = 10_000_000;

fn pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    )
}

fn memory(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), pointer()),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::Scalar(ScalarType::U32),
            extent: WorkgroupMemoryExtent::Static(64),
            alignment: 4,
        }),
    )
}

fn block(id: u32, parameter: Option<u32>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    if let Some(parameter) = parameter {
        block
            .parameters
            .push(ValueDef::new(ValueId(parameter), pointer()));
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

fn module(blocks: Vec<BasicBlock>, parameters: bool) -> Module {
    let mut module = Module::new("must-alias");
    let mut function = Function::internal_helper(
        "storage",
        Signature::new(
            if parameters {
                vec![Type::BOOL, pointer(), Type::INDEX]
            } else {
                vec![]
            },
            vec![],
        ),
        if parameters {
            vec![ValueId(99), ValueId(98), ValueId(97)]
        } else {
            vec![]
        },
        blocks,
    );
    function
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions.push(function);
    module
}

fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, storage.retained_storage())
}

fn inspect(module: Module, expected: &[(u32, Option<u32>)]) {
    let (owner, owner_storage) = owner(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(owner_storage + 17).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (report, storage) = CanonicalKirMustAliasV1::derive(
        &inventory,
        CanonicalKirMustAliasLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(report.belongs_to(&inventory));
    assert!(std::ptr::eq(report.inventory().owner(), &owner));
    for (value, origin) in expected {
        let definition = inventory
            .definitions()
            .iter()
            .position(|row| row.value == Some(ValueId(*value)))
            .unwrap();
        let before = budget.work();
        let actual = report
            .allocation_for_definition(definition, &mut budget)
            .unwrap();
        assert_eq!(
            actual.map(|row| row.value.unwrap().0),
            *origin,
            "value %{value}"
        );
        assert_eq!(budget.work(), before + 2);
        if let Some(actual) = actual {
            assert!(matches!(
                actual.coordinate,
                Definition::Result { result: 0, .. }
            ));
        }
    }
    assert_eq!(
        report
            .allocation_for_definition(usize::MAX, &mut budget)
            .unwrap_err(),
        CanonicalKirMustAliasErrorV1::InvalidDefinition {
            definition: usize::MAX
        }
    );
    drop(report);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn straight_line_and_pointer_selects_keep_only_one_physical_allocation() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0), memory(1)];
    for (result, a, b) in [(400, 0, 0), (401, 400, 0), (402, 0, 1), (403, 0, 98)] {
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(result), pointer()),
            OperationKind::Select {
                condition: ValueId(99),
                true_value: ValueId(a),
                false_value: ValueId(b),
            },
        ));
    }
    inspect(
        module(vec![entry], true),
        &[
            (0, Some(0)),
            (1, Some(1)),
            (98, None),
            (400, Some(0)),
            (401, Some(0)),
            (402, None),
            (403, None),
        ],
    );
}

#[test]
fn diamond_join_requires_all_incoming_allocations_to_agree() {
    for different in [false, true] {
        let mut entry = block(u32::MAX, None);
        entry.operations = vec![memory(0), memory(1)];
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(99),
            then_target: BlockId(17),
            then_arguments: vec![],
            else_target: BlockId(18),
            else_arguments: vec![],
        });
        let mut left = block(17, None);
        left.terminator = branch(19, 0);
        let mut right = block(18, None);
        right.terminator = branch(19, u32::from(different));
        inspect(
            module(vec![entry, left, right, block(19, Some(400))], true),
            &[(400, (!different).then_some(0))],
        );
    }
}

#[test]
fn duplicate_successor_occurrences_are_not_collapsed() {
    for different in [false, true] {
        let mut entry = block(77, None);
        entry.operations = vec![memory(0), memory(1)];
        entry.terminator = conditional(18, 0, 18, u32::from(different));
        inspect(
            module(vec![entry, block(18, Some(400))], true),
            &[(400, (!different).then_some(0))],
        );
    }
}

#[test]
fn grounded_loop_parameter_resolves_the_entry_allocation() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    entry.terminator = branch(18, 0);
    let mut header = block(18, Some(400));
    header.terminator = conditional(18, 400, 19, 400);
    inspect(
        module(vec![entry, header, block(19, Some(500))], true),
        &[(400, Some(0)), (500, Some(0))],
    );
}

#[test]
fn irreducible_two_entry_cycle_is_grounded_only_when_both_entries_agree() {
    for different in [false, true] {
        let mut entry = block(77, None);
        entry.operations = vec![memory(0), memory(1)];
        entry.terminator = conditional(18, 0, 19, u32::from(different));
        let mut left = block(18, Some(400));
        left.terminator = conditional(19, 400, 20, 400);
        let mut right = block(19, Some(500));
        right.terminator = conditional(18, 500, 20, 500);
        let expected = (!different).then_some(0);
        inspect(
            module(vec![entry, left, right, block(20, Some(600))], true),
            &[(400, expected), (500, expected), (600, expected)],
        );
    }
}

#[test]
fn missing_initial_ground_and_ungrounded_cycle_do_not_manufacture_an_allocation() {
    let entry = block(77, None);
    let mut left = block(18, Some(400));
    left.terminator = branch(19, 400);
    let mut right = block(19, Some(500));
    right.terminator = branch(18, 500);
    inspect(
        module(vec![entry, left, right, block(20, Some(600))], false),
        &[(400, None), (500, None), (600, None)],
    );
}

#[test]
fn unresolved_cycle_contaminates_an_otherwise_grounded_downstream_join() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    entry.terminator = branch(20, 0);
    let mut dormant = block(18, Some(400));
    dormant.terminator = conditional(18, 400, 20, 400);
    inspect(
        module(vec![entry, dormant, block(20, Some(600))], true),
        &[(0, Some(0)), (400, None), (600, None)],
    );
}

#[test]
fn entry_block_parameter_keeps_its_unknown_implicit_initial_value() {
    let mut entry = block(77, Some(400));
    entry.operations = vec![memory(0)];
    entry.terminator = conditional(77, 0, 20, 400);
    inspect(
        module(vec![entry, block(20, Some(500))], true),
        &[(400, None), (500, None)],
    );
}

#[test]
fn casts_offsets_and_calls_are_not_invented_identity_transfers() {
    let mut entry = block(77, None);
    let read_only = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadOnly,
    );
    entry.operations = vec![
        memory(0),
        Operation::effect_free(
            ValueDef::new(ValueId(400), read_only.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: ValueId(0),
                to: read_only,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(500), pointer()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(97),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(600), pointer()),
            OperationKind::Call {
                callee: "identity".into(),
                arguments: vec![ValueId(0)],
            },
        ),
    ];
    let mut module = module(vec![entry], true);
    let mut callee = block(7, None);
    callee.terminator = Some(Terminator::Return {
        values: vec![ValueId(8)],
    });
    module.functions.push(Function::internal_helper(
        "identity",
        Signature::new(vec![pointer()], vec![pointer()]),
        vec![ValueId(8)],
        vec![callee],
    ));
    inspect(
        module,
        &[
            (0, Some(0)),
            (400, None),
            (500, None),
            (600, None),
            (8, None),
        ],
    );
}

fn exact_payloads(definitions: usize, dependencies: usize) -> (usize, usize) {
    // Independent structural equation: report = inventory pointer, Vec header,
    // retained byte count; engine adds five Vec headers and three queue counters.
    // Origin is a discriminant + dense index; Link is target + next.
    let word = size_of::<usize>();
    assert_eq!(size_of::<Origin>(), 2 * word);
    assert_eq!(size_of::<Link>(), 2 * word);
    assert_eq!(size_of::<CanonicalKirMustAliasV1<'_, '_>>(), 5 * word);
    assert_eq!(size_of::<Engine<'_, '_>>(), 23 * word);
    (
        23 * word + definitions * (4 * word + 2) + dependencies * 2 * word,
        5 * word + definitions * 2 * word,
    )
}

#[test]
fn empty_and_single_allocation_have_independent_exact_work_and_storage_boundaries() {
    let empty = Module::new("empty");
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    for (source, definitions, operations, exact_work) in
        [(empty, 0, 0, 4), (module(vec![entry], false), 1, 1, 21)]
    {
        let (owner, owner_storage) = owner(&source);
        let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut setup = Budget::new(&mut setup_work, LIMIT);
        let (inventory, inventory_storage) = Inventory::derive(&owner, &mut setup).unwrap();
        assert_eq!(inventory.definitions().len(), definitions);
        assert_eq!(inventory.operations().len(), operations);
        assert_eq!(inventory.edge_arguments().len(), 0);
        // Empty: four fixed limit/census units. Nonempty: 4 + 2 operation
        // visits + five (allocation + initialization) pairs + three definition
        // visits + one enqueue + one dequeue = 21.
        let (peak, retained) = exact_payloads(definitions, 0);
        let floor = 17 + owner_storage + inventory_storage.retained_storage();
        for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + exact_work - work_under);
            let mut budget = Budget::new(&mut work, floor + peak - storage_under);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(11).unwrap();
            match CanonicalKirMustAliasV1::derive(
                &inventory,
                CanonicalKirMustAliasLimitsV1::default(),
                &mut budget,
            ) {
                Ok((report, storage)) => {
                    assert_eq!((work_under, storage_under), (0, 0));
                    assert_eq!(storage.retained_storage(), retained);
                    assert_eq!(budget.peak_storage(), floor + peak);
                    assert_eq!(budget.work(), 11 + exact_work);
                    budget.reserve_storage(retained).unwrap();
                    drop(report);
                    budget.release_storage(retained).unwrap();
                }
                Err(CanonicalKirMustAliasErrorV1::Resource(Resource::Work(_))) => {
                    assert_eq!((work_under, storage_under), (1, 0))
                }
                Err(CanonicalKirMustAliasErrorV1::Resource(Resource::Storage(_))) => {
                    assert_eq!((work_under, storage_under), (0, 1))
                }
                other => panic!("unexpected boundary result: {other:?}"),
            }
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn grounded_loop_has_exact_dependency_storage_and_work_without_calibration() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    entry.terminator = branch(18, 0);
    let mut header = block(18, Some(400));
    header.terminator = branch(18, 400);
    let (owner, owner_storage) = owner(&module(vec![entry, header], false));
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut setup).unwrap();
    assert_eq!(
        (
            inventory.definitions().len(),
            inventory.operations().len(),
            inventory.edge_arguments().len()
        ),
        (2, 1, 2)
    );
    // 4 fixed + 1 census + 5*(1+2) dense initialization + (1+2) links
    // initialization + 2 definition + 1 operation + 2*(edge visit+link)
    // + 2 initial visits + 1 seed enqueue + 2 dequeue + 2 link traversal
    // + 1 propagated enqueue + 2 closure visits = 40.
    let exact_work = 40;
    let (peak, retained) = exact_payloads(2, 2);
    let floor = 17 + owner_storage + inventory_storage.retained_storage();
    for (work_under, storage_under) in [(0, 0), (1, 0), (0, 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - work_under);
        let mut budget = Budget::new(&mut work, floor + peak - storage_under);
        budget.reserve_storage(floor).unwrap();
        let result = CanonicalKirMustAliasV1::derive(
            &inventory,
            CanonicalKirMustAliasLimitsV1::default(),
            &mut budget,
        );
        assert_eq!(result.is_ok(), work_under == 0 && storage_under == 0);
        if let Ok((report, storage)) = result {
            assert_eq!(storage.retained_storage(), retained);
            assert_eq!(budget.work(), exact_work);
            assert_eq!(budget.peak_storage(), floor + peak);
            drop(report);
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn exact_input_caps_and_other_inventory_identity_are_enforced() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    entry.terminator = branch(18, 0);
    let source = module(vec![entry, block(18, Some(400))], false);
    let (owner, owner_storage) = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17 + owner_storage).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let (other_inventory, other_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(other_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let exact = CanonicalKirMustAliasLimitsV1 {
        definitions: 2,
        operations: 1,
        edge_arguments: 1,
        dependencies: 1,
    };
    let (report, _) = CanonicalKirMustAliasV1::derive(&inventory, exact, &mut budget).unwrap();
    assert!(report.belongs_to(&inventory));
    assert!(!report.belongs_to(&other_inventory));
    drop(report);
    for limits in [
        CanonicalKirMustAliasLimitsV1 {
            definitions: 1,
            ..exact
        },
        CanonicalKirMustAliasLimitsV1 {
            operations: 0,
            ..exact
        },
        CanonicalKirMustAliasLimitsV1 {
            edge_arguments: 0,
            ..exact
        },
        CanonicalKirMustAliasLimitsV1 {
            dependencies: 0,
            ..exact
        },
    ] {
        assert!(matches!(
            CanonicalKirMustAliasV1::derive(&inventory, limits, &mut budget),
            Err(CanonicalKirMustAliasErrorV1::InputLimit { .. })
        ));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn equal_sparse_value_ids_in_distinct_functions_never_share_an_origin() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(u32::MAX)];
    entry.terminator = branch(18, u32::MAX);
    let mut source = module(vec![entry, block(18, Some(400))], false);
    let mut second = source.functions[0].clone();
    second.id = "second".into();
    source.functions.push(second);
    let (owner, owner_storage) = owner(&source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17 + owner_storage).unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let (report, receipt) = CanonicalKirMustAliasV1::derive(
        &inventory,
        CanonicalKirMustAliasLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let mut origins = Vec::new();
    for (index, row) in inventory.definitions().iter().enumerate() {
        if row.value == Some(ValueId(400)) {
            origins.push(
                report
                    .allocation_for_definition(index, &mut budget)
                    .unwrap()
                    .unwrap()
                    .coordinate,
            );
        }
    }
    assert_eq!(origins.len(), 2);
    assert_ne!(origins[0], origins[1]);
    drop(report);
    budget.release_storage(receipt.retained_storage()).unwrap();
}

#[test]
fn a_later_smaller_analysis_does_not_erase_prior_resource_failure_history() {
    let mut entry = block(77, None);
    entry.operations = vec![memory(0)];
    let (nonempty, nonempty_storage) = owner(&module(vec![entry], false));
    let (empty, empty_storage) = owner(&Module::new("empty"));
    let mut setup_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    let (large_inventory, large_storage) = Inventory::derive(&nonempty, &mut setup).unwrap();
    let (small_inventory, small_storage) = Inventory::derive(&empty, &mut setup).unwrap();
    let floor = 17
        + nonempty_storage
        + empty_storage
        + large_storage.retained_storage()
        + small_storage.retained_storage();
    let (large_peak, _) = exact_payloads(1, 0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = Budget::new(&mut work, floor + large_peak - 1);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        CanonicalKirMustAliasV1::derive(
            &large_inventory,
            CanonicalKirMustAliasLimitsV1::default(),
            &mut budget
        ),
        Err(CanonicalKirMustAliasErrorV1::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.failed_storage(), Some(floor + large_peak));
    assert_eq!(budget.storage(), floor);
    let accepted_work = budget.work();
    let (report, receipt) = CanonicalKirMustAliasV1::derive(
        &small_inventory,
        CanonicalKirMustAliasLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.work(), accepted_work + 4);
    assert_eq!(budget.failed_storage(), Some(floor + large_peak));
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(report);
    budget.release_storage(receipt.retained_storage()).unwrap();
}
