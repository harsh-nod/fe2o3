use fe2o3_kernel_ir::*;
use fe2o3_kernel_opt::*;

fn pure(result: u32, kind: OperationKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(ScalarType::U32)),
        kind,
    )
}

fn optimization_module() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        pure(0, OperationKind::Constant(Constant::U32(7))),
        pure(1, OperationKind::Constant(Constant::U32(9))),
        pure(
            2,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        pure(3, OperationKind::Constant(Constant::U32(11))),
    ];
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });

    let mut unreachable = BasicBlock::new(BlockId(1));
    unreachable.operations = vec![pure(4, OperationKind::Constant(Constant::U32(13)))];
    unreachable.terminator = Some(Terminator::Unreachable);

    let function = Function::definition(
        "compute",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![entry, unreachable],
    );
    let mut module = Module::new("optimization-test");
    module.functions.push(function);
    module
}

#[test]
fn fixed_pipeline_removes_unreachable_blocks_and_transitive_dead_pure_operations() {
    let input = optimization_module();
    verify_module(&input).unwrap();

    let optimized =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap();
    verify_module(optimized.module()).unwrap();

    let body = optimized.module().functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    assert_eq!(body.blocks[0].operations.len(), 3);
    assert_eq!(
        body.blocks[0].operations[2].result_ids().next(),
        Some(ValueId(2))
    );
    assert_eq!(optimized.report().final_epoch, 2);
    assert_eq!(
        optimized
            .report()
            .passes
            .iter()
            .map(|report| report.pass)
            .collect::<Vec<_>>(),
        KERNEL_IR_OPTIMIZATION_PASS_ORDER_V1
    );
    assert_eq!(optimized.report().passes[0].mutations, 1);
    assert_eq!(optimized.report().passes[1].mutations, 1);
}

#[test]
fn a_live_pure_operation_keeps_its_transitive_dependencies() {
    let input = optimization_module();
    let optimized =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap();
    let operations = &optimized.module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations;

    assert_eq!(
        operations
            .iter()
            .flat_map(Operation::result_ids)
            .collect::<Vec<_>>(),
        vec![ValueId(0), ValueId(1), ValueId(2)]
    );
}

#[test]
fn a_noop_pipeline_does_not_advance_the_mutation_epoch() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut input = Module::new("noop");
    input.functions.push(Function::definition(
        "noop",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));

    let optimized =
        optimize_kernel_ir_module_at_epoch_v1(&input, 41, KernelIrOptimizationLimitsV1::DEFAULT)
            .unwrap();
    assert_eq!(optimized.module(), &input);
    assert_eq!(optimized.report().initial_epoch, 41);
    assert_eq!(optimized.report().final_epoch, 41);
    assert!(
        optimized
            .report()
            .passes
            .iter()
            .all(|report| !report.changed && report.input_epoch == report.output_epoch)
    );
}

#[test]
fn work_budget_exhaustion_is_fail_closed() {
    let input = optimization_module();
    let original = input.clone();
    let limits = KernelIrOptimizationLimitsV1 {
        remove_unreachable_blocks: KernelIrPassBudgetV1::new(
            0,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };

    let error = optimize_kernel_ir_module_v1(&input, limits).unwrap_err();
    assert!(matches!(
        error,
        KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
            resource: KernelIrOptimizationResourceV1::WorkUnits,
            limit: 0,
            attempted: 1,
        }
    ));
    assert_eq!(input, original);
}

#[test]
fn later_pass_failure_does_not_publish_an_earlier_pass_candidate() {
    let input = optimization_module();
    let original = input.clone();
    let limits = KernelIrOptimizationLimitsV1 {
        eliminate_dead_pure_operations: KernelIrPassBudgetV1::new(
            0,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };

    let error = optimize_kernel_ir_module_v1(&input, limits).unwrap_err();
    assert!(matches!(
        error,
        KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::EliminateDeadPureOperations,
            resource: KernelIrOptimizationResourceV1::WorkUnits,
            ..
        }
    ));
    assert_eq!(input, original);
    assert_eq!(input.functions[0].body.as_ref().unwrap().blocks.len(), 2);
}

#[test]
fn mutation_budget_exhaustion_does_not_publish_a_candidate() {
    let input = optimization_module();
    let original = input.clone();
    let limits = KernelIrOptimizationLimitsV1 {
        remove_unreachable_blocks: KernelIrPassBudgetV1::new(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
            0,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };

    let error = optimize_kernel_ir_module_v1(&input, limits).unwrap_err();
    assert!(matches!(
        error,
        KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
            resource: KernelIrOptimizationResourceV1::Mutations,
            limit: 0,
            attempted: 1,
        }
    ));
    assert_eq!(input, original);
}

#[test]
fn mutation_epoch_overflow_is_fail_closed() {
    let input = optimization_module();
    let original = input.clone();

    let error = optimize_kernel_ir_module_at_epoch_v1(
        &input,
        u64::MAX,
        KernelIrOptimizationLimitsV1::DEFAULT,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        KernelIrOptimizationErrorV1::MutationEpochOverflow {
            pass: KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
            epoch: u64::MAX,
        }
    ));
    assert_eq!(input, original);
}

#[test]
fn impure_operations_are_never_removed_when_their_results_are_dead() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32))],
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut input = Module::new("impure");
    input.functions.push(Function::definition(
        "impure",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));

    let optimized =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap();
    assert_eq!(
        optimized.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations
            .len(),
        1
    );
}

#[test]
fn repeated_runs_are_byte_for_byte_deterministic_at_the_ir_and_report_level() {
    let input = optimization_module();
    let first =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap();
    let second =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap();

    assert_eq!(first, second);
}

#[test]
fn invalid_input_is_rejected_before_the_first_pass() {
    let mut input = optimization_module();
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;

    let error =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap_err();
    assert!(matches!(
        error,
        KernelIrOptimizationErrorV1::Verification {
            pass: KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
            phase: KernelIrOptimizationVerificationPhaseV1::BeforePass,
            epoch: 0,
            ..
        }
    ));
}

#[test]
fn configured_limits_cannot_raise_hard_maxima() {
    let input = optimization_module();
    let limits = KernelIrOptimizationLimitsV1 {
        eliminate_dead_pure_operations: KernelIrPassBudgetV1::new(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1 + 1,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert_eq!(
        optimize_kernel_ir_module_v1(&input, limits),
        Err(KernelIrOptimizationErrorV1::InvalidLimit {
            pass: Some(KernelIrOptimizationPassV1::EliminateDeadPureOperations),
            resource: KernelIrOptimizationResourceV1::WorkUnits,
            requested: MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1 + 1,
            hard_maximum: MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
        })
    );
}

#[test]
fn encoded_output_admission_and_dce_storage_preflight_are_bounded() {
    let input = optimization_module();
    let encoded_bytes = encode_module_v9(&input).unwrap().len();
    let exact_byte_limits = KernelIrOptimizationLimitsV1 {
        max_module_bytes: encoded_bytes,
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    optimize_kernel_ir_module_v1(&input, exact_byte_limits).unwrap();

    let byte_limits = KernelIrOptimizationLimitsV1 {
        max_module_bytes: encoded_bytes - 1,
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert!(matches!(
        optimize_kernel_ir_module_v1(&input, byte_limits),
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            resource: KernelIrOptimizationResourceV1::CanonicalBytes,
            limit,
            attempted,
            ..
        }) if limit == u64::try_from(encoded_bytes - 1).unwrap()
            && attempted == u64::try_from(encoded_bytes).unwrap()
    ));

    let storage_limits = KernelIrOptimizationLimitsV1 {
        eliminate_dead_pure_operations: KernelIrPassBudgetV1::with_storage(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
            0,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert!(matches!(
        optimize_kernel_ir_module_v1(&input, storage_limits),
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::EliminateDeadPureOperations,
            resource: KernelIrOptimizationResourceV1::StorageItems,
            limit: 0,
            ..
        })
    ));
}

const HOSTILE_ARITY: usize = 4_096;

fn hostile_switch_module(case_count: usize) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: (0..case_count)
            .map(|value| SwitchCase {
                value: u64::try_from(value).unwrap(),
                target: BlockId(0),
                arguments: vec![],
            })
            .collect(),
        default_target: BlockId(0),
        default_arguments: vec![],
    });
    let mut module = Module::new("hostile-switch");
    module.functions.push(Function::definition(
        "fanout",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![entry],
    ));
    module
}

#[test]
fn reachability_precharges_hostile_successor_fanout_before_materialization() {
    let input = hostile_switch_module(HOSTILE_ARITY);
    verify_module(&input).unwrap();

    let work_limits = KernelIrOptimizationLimitsV1 {
        remove_unreachable_blocks: KernelIrPassBudgetV1::with_storage(
            4,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_STORAGE_ITEMS_V1,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert!(matches!(
        optimize_kernel_ir_module_v1(&input, work_limits),
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
            resource: KernelIrOptimizationResourceV1::WorkUnits,
            limit: 4,
            attempted,
        }) if attempted == u64::try_from(HOSTILE_ARITY + 5).unwrap()
    ));

    let required_storage = HOSTILE_ARITY + 3;
    let storage_limits = KernelIrOptimizationLimitsV1 {
        remove_unreachable_blocks: KernelIrPassBudgetV1::with_storage(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
            u64::try_from(required_storage - 1).unwrap(),
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert!(matches!(
        optimize_kernel_ir_module_v1(&input, storage_limits),
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::RemoveUnreachableBlocks,
            resource: KernelIrOptimizationResourceV1::StorageItems,
            limit,
            attempted,
        }) if limit == u64::try_from(required_storage - 1).unwrap()
            && attempted == u64::try_from(required_storage).unwrap()
    ));
}

fn hostile_call_module(argument_count: usize) -> Module {
    let value_type = Type::Scalar(ScalarType::U32);
    let parameter_types = vec![value_type; argument_count];
    let parameter_ids = (0..argument_count)
        .map(|value| ValueId(u32::try_from(value).unwrap()))
        .collect::<Vec<_>>();
    let import = Function::external_import("sink", Signature::new(parameter_types.clone(), vec![]));

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("sink"),
            arguments: parameter_ids.clone(),
        },
    ));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let caller = Function::definition(
        "caller",
        Signature::new(parameter_types, vec![]),
        parameter_ids,
        vec![entry],
    );

    let mut module = Module::new("hostile-call");
    module.functions.extend([import, caller]);
    module
}

#[test]
fn dce_precharges_hostile_operand_arity_before_materialization() {
    let input = hostile_call_module(HOSTILE_ARITY);
    verify_module(&input).unwrap();

    let work_limits = KernelIrOptimizationLimitsV1 {
        eliminate_dead_pure_operations: KernelIrPassBudgetV1::with_storage(
            6,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_STORAGE_ITEMS_V1,
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert!(matches!(
        optimize_kernel_ir_module_v1(&input, work_limits),
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::EliminateDeadPureOperations,
            resource: KernelIrOptimizationResourceV1::WorkUnits,
            limit: 6,
            attempted,
        }) if attempted == u64::try_from(HOSTILE_ARITY + 6).unwrap()
    ));

    let required_storage = HOSTILE_ARITY + 1;
    let storage_limits = KernelIrOptimizationLimitsV1 {
        eliminate_dead_pure_operations: KernelIrPassBudgetV1::with_storage(
            MAX_KERNEL_IR_OPTIMIZATION_WORK_UNITS_V1,
            MAX_KERNEL_IR_OPTIMIZATION_MUTATIONS_V1,
            u64::try_from(required_storage - 1).unwrap(),
        ),
        ..KernelIrOptimizationLimitsV1::DEFAULT
    };
    assert!(matches!(
        optimize_kernel_ir_module_v1(&input, storage_limits),
        Err(KernelIrOptimizationErrorV1::BudgetExceeded {
            pass: KernelIrOptimizationPassV1::EliminateDeadPureOperations,
            resource: KernelIrOptimizationResourceV1::StorageItems,
            limit,
            attempted,
        }) if limit == u64::try_from(required_storage - 1).unwrap()
            && attempted == u64::try_from(required_storage).unwrap()
    ));
}

fn hazard_module(operations: Vec<Operation>, parameters: Vec<(ValueId, Type)>) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let signature = Signature::new(
        parameters.iter().map(|(_, ty)| ty.clone()).collect(),
        vec![],
    );
    let function = Function::kernel_entry(
        "hazard_impl",
        signature,
        parameters.iter().map(|(value, _)| *value).collect(),
        vec![block],
    );
    let mut module = Module::new("optimizer-hazard");
    module.functions.push(function);
    module.kernels.push(Kernel::new(
        "hazard",
        "hazard_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

// The simulator is a host-runtime crate and cannot be a dependency of this
// compiler layer. These verified fixtures pin the same may-trap operation
// families at the optimizer's erasure boundary.
fn assert_potentially_trapping_operation_is_retained(
    input: Module,
    expected: impl Fn(&OperationKind) -> bool,
) {
    verify_module(&input).unwrap();
    let optimized =
        optimize_kernel_ir_module_v1(&input, KernelIrOptimizationLimitsV1::DEFAULT).unwrap();
    verify_module(optimized.module()).unwrap();
    let operation = optimized.module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations
        .iter()
        .find(|operation| expected(&operation.kind))
        .expect("potentially trapping operation must survive dead-code elimination");
    assert_eq!(
        classify_operation_erasability_v1(operation),
        KernelIrOperationErasabilityV1::RetainedPotentiallyObservable
    );
}

fn u32_binary_hazard(op: BinaryOp, lhs: u32, rhs: u32) -> Module {
    hazard_module(
        vec![
            pure(0, OperationKind::Constant(Constant::U32(lhs))),
            pure(1, OperationKind::Constant(Constant::U32(rhs))),
            pure(
                2,
                OperationKind::Binary {
                    op,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
        ],
        vec![],
    )
}

#[test]
fn dce_retains_division_by_zero_overflow_and_bad_shift_operations() {
    for (input, expected_op) in [
        (u32_binary_hazard(BinaryOp::Divide, 1, 0), BinaryOp::Divide),
        (u32_binary_hazard(BinaryOp::Add, u32::MAX, 1), BinaryOp::Add),
        (
            u32_binary_hazard(BinaryOp::ShiftLeft, 1, 32),
            BinaryOp::ShiftLeft,
        ),
    ] {
        assert_potentially_trapping_operation_is_retained(
            input,
            |kind| matches!(kind, OperationKind::Binary { op, .. } if *op == expected_op),
        );
    }
}

#[test]
fn dce_retains_float_to_integer_conversion() {
    let input = hazard_module(
        vec![
            Operation::effect_free(
                ValueDef::new(ValueId(0), Type::F32),
                OperationKind::Constant(Constant::F32Bits(0x7fc0_0000)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::Scalar(ScalarType::I32)),
                OperationKind::Cast {
                    kind: CastKind::FloatToInteger,
                    value: ValueId(0),
                    to: Type::Scalar(ScalarType::I32),
                },
            ),
        ],
        vec![],
    );
    assert_potentially_trapping_operation_is_retained(input, |kind| {
        matches!(
            kind,
            OperationKind::Cast {
                kind: CastKind::FloatToInteger,
                ..
            }
        )
    });
}

#[test]
fn dce_retains_gep_that_can_overflow() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let input = hazard_module(
        vec![Operation::effect_free(
            ValueDef::new(ValueId(2), pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(1),
            },
        )],
        vec![(ValueId(0), pointer), (ValueId(1), Type::INDEX)],
    );
    assert_potentially_trapping_operation_is_retained(input, |kind| {
        matches!(kind, OperationKind::GetElementPointer { .. })
    });
}
