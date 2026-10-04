use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CastKind, Function,
    KernelIrEncodeError, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, verify_module,
};
use fe2o3_kernel_opt::{
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2, KernelIrPlironOptimizationErrorV2,
    KernelIrPlironOptimizationErrorV3, KernelIrPlironOptimizationLimitsV2,
    KernelIrPlironOptimizationPolicyV2, KernelIrPlironStructuralReplayAdmissionErrorV3,
    admit_production_kernel_ir_structural_replay_v3, optimize_kernel_ir_module_at_epoch_v3,
    optimize_kernel_ir_module_v2, optimize_kernel_ir_module_v3,
    optimize_production_kernel_ir_module_v2, optimize_production_kernel_ir_module_v3,
};

fn restriction(result: u32, source: ValueId, read_only: &Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), read_only.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: source,
            to: read_only.clone(),
        },
    )
}

fn pointer_restriction_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let read_write = Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let read_only = Type::pointer(scalar, AddressSpace::Private, AccessMode::ReadOnly);

    let mut live = BasicBlock::new(BlockId(0));
    live.operations = vec![restriction(1, ValueId(0), &read_only)];
    live.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });

    let mut redundant = BasicBlock::new(BlockId(0));
    redundant.operations = vec![
        restriction(1, ValueId(0), &read_only),
        restriction(2, ValueId(0), &read_only),
    ];
    redundant.terminator = Some(Terminator::Return {
        values: vec![ValueId(1), ValueId(2)],
    });

    let mut dead = BasicBlock::new(BlockId(0));
    dead.operations = vec![restriction(1, ValueId(0), &read_only)];
    dead.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("pliron-v3-pointer-restrictions");
    module.functions.extend([
        Function::internal_helper(
            "live",
            Signature::new(vec![read_write.clone()], vec![read_only.clone()]),
            vec![ValueId(0)],
            vec![live],
        ),
        Function::internal_helper(
            "redundant",
            Signature::new(vec![read_write.clone()], vec![read_only.clone(), read_only]),
            vec![ValueId(0)],
            vec![redundant],
        ),
        Function::internal_helper(
            "dead",
            Signature::new(vec![read_write], vec![]),
            vec![ValueId(0)],
            vec![dead],
        ),
    ]);
    module
}

fn scalar_module(name: &str) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut module = Module::new(name);
    module.functions.push(Function::internal_helper(
        "scalar",
        Signature::new(vec![scalar.clone(), scalar.clone()], vec![scalar]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

fn function_operations<'a>(module: &'a Module, id: &str) -> &'a [Operation] {
    &module
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations
}

#[test]
fn v11_pointer_restrictions_remain_live_and_participate_in_cse_and_dce() {
    let input = pointer_restriction_module();
    verify_module(&input).unwrap();

    let optimized = optimize_production_kernel_ir_module_v3(&input).unwrap();
    verify_module(optimized.module()).unwrap();

    assert_eq!(function_operations(optimized.module(), "live").len(), 1);
    assert!(matches!(
        function_operations(optimized.module(), "live")[0].kind,
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            ..
        }
    ));
    assert_eq!(
        function_operations(optimized.module(), "redundant").len(),
        1
    );
    assert!(matches!(
        function_operations(optimized.module(), "redundant")[0].kind,
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            ..
        }
    ));
    assert!(function_operations(optimized.module(), "dead").is_empty());

    let redundant_return = optimized.module().functions[1]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .terminator
        .as_ref()
        .unwrap();
    assert!(matches!(
        redundant_return,
        Terminator::Return { values } if values.len() == 2 && values[0] == values[1]
    ));
}

#[test]
fn v3_uses_exact_v11_and_leaves_the_v2_v10_endpoint_frozen() {
    let input = scalar_module("version-boundaries");
    let v2 = optimize_production_kernel_ir_module_v2(&input).unwrap();
    let v3 = optimize_production_kernel_ir_module_v3(&input).unwrap();

    assert_eq!(
        &v2.canonical().canonical_bytes()[8..10],
        &10_u16.to_le_bytes()
    );
    assert_eq!(
        &v3.canonical().canonical_bytes()[8..10],
        &11_u16.to_le_bytes()
    );
    assert_eq!(v2.module(), v3.module());
    assert_eq!(v2.report().pliron(), v3.report().pliron());
    assert_eq!(v2.report().passes(), v3.report().passes());
    assert_ne!(v2.report().input_digest(), v3.report().input_digest());
    assert_eq!(
        v3.report().policy(),
        KernelIrPlironOptimizationPolicyV2::ProductionV2
    );
    assert_eq!(
        v3.report()
            .passes()
            .iter()
            .map(|pass| pass.pliron().pass().name())
            .collect::<Vec<_>>(),
        KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
            .iter()
            .map(|pass| pass.name())
            .collect::<Vec<_>>()
    );

    assert!(matches!(
        optimize_kernel_ir_module_v2(
            &pointer_restriction_module(),
            KernelIrPlironOptimizationLimitsV2::default(),
        ),
        Err(KernelIrPlironOptimizationErrorV2::InputEncoding(
            KernelIrEncodeError::UnsupportedInVersion { .. }
        ))
    ));
}

#[test]
fn fresh_v3_sessions_produce_deterministic_v11_receipts() {
    let input = pointer_restriction_module();
    let first = optimize_kernel_ir_module_at_epoch_v3(
        &input,
        19,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();
    let second = optimize_kernel_ir_module_at_epoch_v3(
        &input,
        19,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.report().initial_epoch(), 19);
}

#[test]
fn invalid_v11_input_is_rejected_without_mutating_the_caller_module() {
    let mut invalid = Module::new("invalid-v11");
    invalid.functions.push(Function::internal_helper(
        "invalid",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock::new(BlockId(0))],
    ));
    let original = invalid.clone();

    assert!(matches!(
        optimize_kernel_ir_module_v3(&invalid, KernelIrPlironOptimizationLimitsV2::default()),
        Err(KernelIrPlironOptimizationErrorV3::InputCanonicalization(_))
    ));
    assert_eq!(invalid, original);
}

#[test]
fn v11_structural_replay_is_exact_and_rejects_cross_version_reports() {
    let input = pointer_restriction_module();
    let live = optimize_production_kernel_ir_module_v3(&input).unwrap();
    let admitted =
        admit_production_kernel_ir_structural_replay_v3(&input, live.module(), live.report())
            .unwrap();
    assert!(admitted.establishes_exact_closed_replay());
    assert!(admitted.establishes_structural_well_formedness());
    assert!(!admitted.establishes_semantic_preservation());
    assert!(!admitted.grants_compiler_refinement_authority());
    assert_eq!(admitted.report(), live.report());

    let scalar = scalar_module("cross-version-report");
    let v2 = optimize_production_kernel_ir_module_v2(&scalar).unwrap();
    let v3 = optimize_production_kernel_ir_module_v3(&scalar).unwrap();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v3(&scalar, v3.module(), v2.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV3::ReportMismatch)
    ));
}
