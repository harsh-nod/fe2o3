use super::*;

#[path = "production_checked_output_scalar_helper_source_abi_v1_tests.rs"]
mod source_abi_tests;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirFunctionCoordinateV1 as Coordinate,
    Function, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, Signature,
    ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const FLOOR: usize = 37;

fn helper(name: &str, ty: Type, callee: Option<&str>) -> Function {
    let mut block = BasicBlock::new(BlockId(91));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(2), ty.clone())],
        match callee {
            Some(callee) => OperationKind::Call {
                callee: callee.into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
            None => OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    Function::internal_helper(
        name,
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )
}

fn identity(name: &str, ty: Type) -> Function {
    let mut block = BasicBlock::new(BlockId(4_000_000_000));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    Function::internal_helper(
        name,
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(0)],
        vec![block],
    )
}

fn module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("scalar-helper-census");
    module.functions = functions;
    module
}

fn caller_module(scalar: ScalarType) -> Module {
    let ty = Type::Scalar(scalar);
    let mut root = helper("root", ty.clone(), Some("middle"));
    root.role = FunctionRole::KernelEntry;
    root.signature.results.clear();
    root.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return { values: vec![] });
    let mut input = module(vec![
        root,
        helper("middle", ty.clone(), Some("leaf")),
        helper("leaf", ty, None),
    ]);
    input.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    input
}

fn with_inventory<T>(
    module: Module,
    callback: impl FnOnce(&Inventory<'_>, &mut Budget<'_>) -> T,
) -> T {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, owner_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let result = callback(&inventory, &mut budget);
    // This component uses the same caller-owned cleanup boundary as its
    // production parent, after all borrowed reports have been dropped.
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}

fn expect_refusal(input: Module, detail: &'static str) {
    with_inventory(input, |inventory, budget| {
        assert!(matches!(
            check(inventory, budget),
            Err(E::Unsupported { phase: "scalar helpers", detail: actual }) if actual == detail
        ));
    });
}

#[test]
fn actual_scalar_closures_and_function_local_values_are_accepted() {
    for scalar in [
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::Index,
    ] {
        with_inventory(caller_module(scalar), |inventory, budget| {
            let checked = check(inventory, budget).unwrap();
            assert!(!checked.function(inventory, Coordinate(0), budget).unwrap());
            assert!(checked.function(inventory, Coordinate(1), budget).unwrap());
            assert!(checked.function(inventory, Coordinate(2), budget).unwrap());
            assert!(checked.call(inventory, 0, budget).unwrap());
            assert!(checked.call(inventory, 1, budget).unwrap());
            assert!(!checked.call(inventory, 2, budget).unwrap());
            assert!(!checked.call(inventory, usize::MAX, budget).unwrap());
            assert!(
                !checked
                    .function(inventory, Coordinate(u32::MAX), budget)
                    .unwrap()
            );
        });
    }
}

#[test]
fn unused_output_helper_roster_members_are_checked_without_live_root_calls() {
    let mut input = caller_module(ScalarType::U32);
    input.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    input
        .functions
        .push(identity("uncalled", Type::Scalar(ScalarType::U32)));
    with_inventory(input, |inventory, budget| {
        let checked = check(inventory, budget).unwrap();
        for ordinal in 1..4 {
            assert!(
                checked
                    .function(inventory, Coordinate(ordinal), budget)
                    .unwrap()
            );
        }
        assert!(checked.call(inventory, 0, budget).unwrap());
    });
}

#[test]
fn a_safe_called_helper_does_not_hide_an_unsafe_orphan_helper() {
    let mut input = caller_module(ScalarType::U32);
    let mut orphan = helper("uncalled", Type::Scalar(ScalarType::U32), None);
    let OperationKind::Binary { op, .. } =
        &mut orphan.body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *op = BinaryOp::Add;
    input.functions.push(orphan);
    expect_refusal(input, "closed scalar helper opcode");
}

#[test]
fn self_and_mutual_recursion_remain_incomplete_even_without_local_effects() {
    expect_refusal(
        module(vec![helper(
            "self_call",
            Type::Scalar(ScalarType::U32),
            Some("self_call"),
        )]),
        "complete empty helper closure",
    );
    expect_refusal(
        module(vec![
            helper("a", Type::Scalar(ScalarType::U32), Some("b")),
            helper("b", Type::Scalar(ScalarType::U32), Some("a")),
        ]),
        "complete empty helper closure",
    );
}

#[test]
fn an_ordinary_external_declaration_is_not_a_helper_contract() {
    let mut input = module(vec![helper(
        "h",
        Type::Scalar(ScalarType::U32),
        Some("external"),
    )]);
    input.functions.push(Function::declaration(
        "external",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 2],
            vec![Type::Scalar(ScalarType::U32)],
        ),
    ));
    expect_refusal(input, "internal scalar helper callees only");
}

#[test]
fn a_registered_complete_trap_contract_does_not_authorize_a_helper_call() {
    let trap = AmdGpuDiagnosticOperation::Trap;
    let mut block = BasicBlock::new(BlockId(13));
    block.operations.push(trap.operation(None));
    block.terminator = Some(Terminator::Unreachable);
    let mut helper =
        Function::internal_helper("h", Signature::new(vec![], vec![]), vec![], vec![block]);
    helper.required_capabilities = trap.required_capabilities();
    let mut input = module(vec![helper, trap.declaration()]);
    input.required_capabilities = trap.required_capabilities();
    with_inventory(input, |inventory, budget| {
        let (effects, receipt) = CanonicalKirCallEffectsV1::derive(inventory, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(
            effects.decision(Coordinate(0), budget).unwrap(),
            CanonicalKirCallEffectDecisionV1::CompleteEmpty
        );
        drop(effects);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            check(inventory, budget),
            Err(E::Unsupported {
                phase: "scalar helpers",
                detail: "internal scalar helper callees only",
            })
        ));
    });
}

#[test]
fn a_reserved_complete_contract_name_cannot_be_a_verified_recursive_helper() {
    let trap = AmdGpuDiagnosticOperation::Trap;
    let mut counterfeit = trap.declaration();
    let mut block = BasicBlock::new(BlockId(13));
    block.operations.push(trap.operation(None));
    block.terminator = Some(Terminator::Unreachable);
    let helper = Function::internal_helper(
        counterfeit.id.clone(),
        counterfeit.signature.clone(),
        vec![],
        vec![block],
    );
    counterfeit.role = helper.role;
    counterfeit.body = helper.body;
    let mut input = module(vec![counterfeit]);
    input.required_capabilities = trap.required_capabilities();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    assert!(Owner::from_module_ref_with_verification_budget_v12(&input, &mut budget).is_err());
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn unit_helpers_are_supported_without_inventing_a_scalar_result() {
    let mut block = BasicBlock::new(BlockId(13));
    block.terminator = Some(Terminator::Return { values: vec![] });
    with_inventory(
        module(vec![Function::internal_helper(
            "unit",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        )]),
        |inventory, budget| {
            let checked = check(inventory, budget).unwrap();
            assert!(checked.function(inventory, Coordinate(0), budget).unwrap());
        },
    );
}

#[test]
fn pointer_and_float_signatures_and_multiple_scalar_results_stay_closed() {
    for ty in [
        Type::F32,
        Type::F64,
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ] {
        expect_refusal(
            module(vec![identity("h", ty)]),
            "direct ordinary scalar signature",
        );
    }
    let mut multiple = identity("h", Type::Scalar(ScalarType::U32));
    multiple
        .signature
        .results
        .push(Type::Scalar(ScalarType::U32));
    multiple.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(0), ValueId(0)],
    });
    expect_refusal(
        module(vec![multiple]),
        "defined direct scalar or Unit result",
    );
}

#[test]
fn private_memory_is_not_reclassified_as_empty_scalar_computation() {
    let mut input = module(vec![identity("h", Type::Scalar(ScalarType::U32))]);
    input.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::new(
            vec![ValueDef::new(
                ValueId(1),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    expect_refusal(input, "ordinary scalar helper definitions");
}

#[test]
fn native_plain_integer_arithmetic_does_not_gain_helper_purity() {
    for op in [
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::ShiftLeft,
        BinaryOp::ShiftRight,
    ] {
        let mut input = module(vec![helper("h", Type::Scalar(ScalarType::U32), None)]);
        let OperationKind::Binary { op: actual, .. } =
            &mut input.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        *actual = op;
        expect_refusal(input, "closed scalar helper opcode");
    }
}

#[test]
fn identical_coordinates_in_another_inventory_do_not_transfer_facts() {
    with_inventory(caller_module(ScalarType::U32), |inventory, budget| {
        let checked = check(inventory, budget).unwrap();
        let (other, storage) = Inventory::derive(inventory.owner(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        for result in [
            checked.function(&other, Coordinate(1), budget),
            checked.call(&other, 0, budget),
        ] {
            assert!(matches!(
                result,
                Err(E::Unsupported {
                    phase: "scalar helpers",
                    detail: "same borrowed inventory",
                })
            ));
        }
        drop(other);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}

fn header() -> usize {
    std::mem::size_of::<&Inventory<'_>>() + std::mem::size_of::<Vec<u8>>()
}

#[test]
fn no_helper_path_has_fixed_storage_and_exact_nine_plus_two_functions_work() {
    for roots in [0usize, 128] {
        let mut input = Module::new("no_helpers");
        for ordinal in 0..roots {
            let mut function = identity(&format!("root{ordinal}"), Type::Scalar(ScalarType::U32));
            function.role = FunctionRole::KernelEntry;
            function.signature.results.clear();
            function.body.as_mut().unwrap().blocks[0].terminator =
                Some(Terminator::Return { values: vec![] });
            input.kernels.push(Kernel::new(
                function.id.as_str(),
                function.id.clone(),
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(1),
                },
            ));
            input.functions.push(function);
        }
        with_inventory(input, |inventory, setup| {
            let floor = setup.storage();
            let exact = 9 + 2 * roots;
            for limit in [exact - 1, exact] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, floor + header());
                budget.reserve_storage(floor).unwrap();
                {
                    let result = check(inventory, &mut budget);
                    if limit == exact {
                        let report = result.unwrap();
                        assert_eq!(report.functions.capacity(), 0);
                    } else {
                        assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                    }
                }
                assert_eq!(
                    budget.work(),
                    if limit == exact { exact } else { exact - 6 }
                );
                assert_eq!(
                    budget.storage(),
                    floor
                        + if limit == exact {
                            header()
                        } else {
                            std::mem::size_of::<&Inventory<'_>>()
                        }
                );
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(
                    work.failed_work(),
                    if limit == exact { None } else { Some(exact) }
                );
            }
        });
    }
}

#[test]
fn no_helper_report_queries_cost_three_and_preserve_failed_work() {
    with_inventory(Module::new("empty"), |inventory, setup| {
        let floor = setup.storage();
        for limit in [11, 12] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, floor + header());
            budget.reserve_storage(floor).unwrap();
            {
                let checked = check(inventory, &mut budget).unwrap();
                let result = checked.function(inventory, Coordinate(0), &mut budget);
                if limit == 12 {
                    assert!(!result.unwrap());
                } else {
                    assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                }
            }
            assert_eq!(budget.work(), if limit == 12 { 12 } else { 9 });
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                work.failed_work(),
                if limit == 12 { None } else { Some(12) }
            );
        }
    });
}

#[test]
fn empty_report_storage_has_exact_header_and_one_short_boundaries() {
    with_inventory(Module::new("empty"), |inventory, setup| {
        let floor = setup.storage();
        let pointer = std::mem::size_of::<&Inventory<'_>>();
        for limit in [floor + pointer - 1, floor + header() - 1, floor + header()] {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            {
                let result = check(inventory, &mut budget);
                if limit == floor + header() {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
                }
            }
            assert_eq!(budget.work(), if limit < floor + pointer { 3 } else { 9 });
            let retained = if limit < floor + pointer {
                0
            } else if limit < floor + header() {
                pointer
            } else {
                header()
            };
            assert_eq!(budget.storage(), floor + retained);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn actual_return_only_helper_has_exact_twenty_six_work_with_late_failure_cleanup() {
    with_inventory(
        module(vec![identity("h", Type::Scalar(ScalarType::U32))]),
        |inventory, setup| {
            let floor = setup.storage();
            // 3 header + 2 count + 6 vector + 1 initialization + 4 fresh effects
            // + 3 helper + 2 signature + 2 definition + 2 control + 1 decision.
            for limit in [25, 26] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(floor).unwrap();
                {
                    let result = check(inventory, &mut budget);
                    if limit == 26 {
                        assert!(result.is_ok());
                    } else {
                        assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
                    }
                }
                assert_eq!(budget.work(), if limit == 26 { 26 } else { 25 });
                assert!(budget.storage() > floor + header());
                budget.release_storage(budget.storage() - floor).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(
                    work.failed_work(),
                    if limit == 26 { None } else { Some(26) }
                );
            }
        },
    );
}
