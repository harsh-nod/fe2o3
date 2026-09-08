use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, Constant, Function, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId, verify_module,
};
use fe2o3_kernel_opt::{
    TargetNeutralModuleTransformErrorV1, TargetNeutralModuleTransformLimitsV1,
    TargetNeutralModuleTransformV1, check_target_neutral_module_transform_relation_v1,
    execute_checked_scalar_cleanup_replay_v1, execute_target_neutral_module_transform_v1,
};

fn private_pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}

fn private_read_only_pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadOnly,
    )
}

fn global_pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    )
}

fn kernel_module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("target-neutral-module-transforms-v1");
    module.functions = functions;
    module.kernels.push(Kernel::new(
        "kernel",
        "kernel_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    verify_module(&module).unwrap();
    module
}

fn scalar_alloca_module(array: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    if array {
        block.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::INDEX),
                OperationKind::Constant(Constant::Index(2)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), private_pointer()),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: Some(ValueId(2)),
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::INDEX),
                OperationKind::Constant(Constant::Index(1)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(5), private_pointer()),
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(4),
                },
            ),
        ]);
    } else {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), private_pointer()),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ));
    }
    let slot = if array { ValueId(5) } else { ValueId(3) };
    block.operations.extend([
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: slot,
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: slot,
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    kernel_module(vec![Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), global_pointer()],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )])
}

fn helper_module(helper_operations: usize) -> Module {
    let mut helper_block = BasicBlock::new(BlockId(0));
    let mut last = ValueId(0);
    for index in 0..helper_operations {
        let result = ValueId(2 + index as u32);
        helper_block.operations.push(Operation::effect_free(
            ValueDef::new(result, Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: last,
                rhs: ValueId(0),
            },
        ));
        last = result;
    }
    helper_block.terminator = Some(Terminator::Return { values: vec![last] });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![helper_block],
    );

    let mut root = BasicBlock::new(BlockId(0));
    root.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    root.terminator = Some(Terminator::Return { values: vec![] });
    let kernel = Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                global_pointer(),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![root],
    );
    kernel_module(vec![kernel, helper])
}

fn scalar_identity_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    kernel_module(vec![Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), global_pointer()],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )])
}

#[test]
fn private_array_sroa_then_secondary_promotion_eliminates_the_slot() {
    let input = scalar_alloca_module(true);
    let limits = TargetNeutralModuleTransformLimitsV1::default();
    let (split, split_report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::ScalarReplacementOfAggregates,
        &input,
        limits,
    )
    .unwrap();
    assert!(split_report.changed());
    assert_eq!(split_report.inserted_operations(), 1);
    assert_eq!(split_report.edits().len(), 1);
    assert!(
        split.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Alloca { count: None, .. }))
    );

    let (promoted, promotion_report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::SecondarySsaPromotion,
        &split,
        limits,
    )
    .unwrap();
    assert!(promotion_report.changed());
    assert_eq!(promotion_report.edits().len(), 1);
    assert!(
        !promoted.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Alloca { .. }))
    );
    check_target_neutral_module_transform_relation_v1(
        TargetNeutralModuleTransformV1::SecondarySsaPromotion,
        &split,
        &promoted,
        limits,
    )
    .unwrap();
}

#[test]
fn address_escape_and_uninitialized_load_are_conservative_no_ops() {
    let mut escaped = scalar_alloca_module(false);
    escaped.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            1,
            Operation::effect_free(
                ValueDef::new(ValueId(7), private_read_only_pointer()),
                OperationKind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
                    value: ValueId(3),
                    to: private_read_only_pointer(),
                },
            ),
        );
    verify_module(&escaped).unwrap();
    let (output, report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::SecondarySsaPromotion,
        &escaped,
        TargetNeutralModuleTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(!report.changed());
    assert_eq!(output, escaped);

    let mut uninitialized = scalar_alloca_module(false);
    uninitialized.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(1);
    verify_module(&uninitialized).unwrap();
    let (output, report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::SecondarySsaPromotion,
        &uninitialized,
        TargetNeutralModuleTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(!report.changed());
    assert_eq!(output, uninitialized);
}

#[test]
fn bounded_inlining_and_interprocedural_cleanup_are_exact_and_deterministic() {
    let input = helper_module(2);
    let limits = TargetNeutralModuleTransformLimitsV1::default();
    let (inlined, report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::HelperInlining,
        &input,
        limits,
    )
    .unwrap();
    assert_eq!(report.inlined_calls(), 1);
    assert!(report.changed());
    assert_eq!(report.edits().len(), 1);
    assert!(
        !inlined.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Call { .. }))
    );
    for _ in 0..8 {
        let (repeated, repeated_report) = execute_target_neutral_module_transform_v1(
            TargetNeutralModuleTransformV1::HelperInlining,
            &input,
            limits,
        )
        .unwrap();
        assert_eq!(repeated, inlined);
        assert_eq!(repeated_report, report);
    }

    let (cleaned, cleanup) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::InterproceduralCleanup,
        &inlined,
        limits,
    )
    .unwrap();
    assert_eq!(cleanup.removed_helpers(), 1);
    assert!(!cleanup.edits().is_empty());
    assert_eq!(cleaned.functions.len(), 1);
}

#[test]
fn unused_helper_arguments_are_removed_from_definition_and_every_call() {
    let input = helper_module(1);
    let (cleaned, report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::InterproceduralCleanup,
        &input,
        TargetNeutralModuleTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(report.removed_arguments(), 1);
    let helper = &cleaned.functions[1];
    assert_eq!(helper.signature.parameters.len(), 1);
    assert_eq!(helper.body.as_ref().unwrap().parameters.len(), 1);
    let OperationKind::Call { arguments, .. } =
        &cleaned.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind
    else {
        panic!("expected retained call")
    };
    assert_eq!(arguments, &[ValueId(0)]);
}

#[test]
fn relation_checker_rejects_any_unaccounted_output_mutation() {
    let input = helper_module(1);
    let (mut expected, _) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::HelperInlining,
        &input,
        TargetNeutralModuleTransformLimitsV1::default(),
    )
    .unwrap();
    expected.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(0, 1);
    assert!(
        check_target_neutral_module_transform_relation_v1(
            TargetNeutralModuleTransformV1::HelperInlining,
            &input,
            &expected,
            TargetNeutralModuleTransformLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn inlining_rejects_growth_limit_and_recursive_sccs() {
    let input = helper_module(2);
    let limits = TargetNeutralModuleTransformLimitsV1::new(16, 1_024, 16, 1, 16).unwrap();
    assert!(matches!(
        execute_target_neutral_module_transform_v1(
            TargetNeutralModuleTransformV1::HelperInlining,
            &input,
            limits,
        ),
        Err(TargetNeutralModuleTransformErrorV1::InlineGrowthLimitExceeded { .. })
    ));

    let mut recursive = helper_module(1);
    let helper = &mut recursive.functions[1];
    helper.body.as_mut().unwrap().blocks[0].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ),
    );
    verify_module(&recursive).unwrap();
    assert_eq!(
        execute_target_neutral_module_transform_v1(
            TargetNeutralModuleTransformV1::HelperInlining,
            &recursive,
            TargetNeutralModuleTransformLimitsV1::default(),
        )
        .unwrap_err(),
        TargetNeutralModuleTransformErrorV1::RecursiveHelperGraph
    );
}

#[test]
fn module_transforms_enforce_every_resource_bound() {
    assert_eq!(
        TargetNeutralModuleTransformLimitsV1::new(0, 1, 1, 1, 1).unwrap_err(),
        TargetNeutralModuleTransformErrorV1::InvalidLimits
    );

    let input = helper_module(2);
    let function_limited = TargetNeutralModuleTransformLimitsV1::new(1, 1_024, 16, 16, 16).unwrap();
    assert!(matches!(
        execute_target_neutral_module_transform_v1(
            TargetNeutralModuleTransformV1::HelperInlining,
            &input,
            function_limited,
        ),
        Err(TargetNeutralModuleTransformErrorV1::FunctionLimitExceeded { .. })
    ));

    let operation_limited = TargetNeutralModuleTransformLimitsV1::new(16, 1, 16, 16, 16).unwrap();
    assert!(matches!(
        execute_target_neutral_module_transform_v1(
            TargetNeutralModuleTransformV1::HelperInlining,
            &input,
            operation_limited,
        ),
        Err(TargetNeutralModuleTransformErrorV1::OperationLimitExceeded { .. })
    ));

    let body_limited = TargetNeutralModuleTransformLimitsV1::new(16, 1_024, 1, 16, 16).unwrap();
    let (not_inlined, report) = execute_target_neutral_module_transform_v1(
        TargetNeutralModuleTransformV1::HelperInlining,
        &input,
        body_limited,
    )
    .unwrap();
    assert_eq!(not_inlined, input);
    assert!(!report.changed());

    let iteration_limited =
        TargetNeutralModuleTransformLimitsV1::new(16, 1_024, 16, 16, 1).unwrap();
    assert_eq!(
        execute_target_neutral_module_transform_v1(
            TargetNeutralModuleTransformV1::InterproceduralCleanup,
            &input,
            iteration_limited,
        )
        .unwrap_err(),
        TargetNeutralModuleTransformErrorV1::InterproceduralIterationLimitExceeded { limit: 1 }
    );
}

#[test]
fn scalar_cleanup_replays_checked_passes_to_a_deterministic_fixed_point() {
    let input = scalar_identity_module();
    let first = execute_checked_scalar_cleanup_replay_v1(&input, 19).unwrap();
    assert_eq!(first.iterations(), 2);
    assert_eq!(first.final_epoch(), 20);
    assert!(!first.grants_semantic_preservation_authority());
    assert!(first.pass_records().iter().any(|record| record.changed()));
    assert!(
        !first.module().functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Binary { .. }))
    );
    for _ in 0..4 {
        assert_eq!(
            execute_checked_scalar_cleanup_replay_v1(&input, 19).unwrap(),
            first
        );
    }
}
