use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, verify_module,
};
use fe2o3_kernel_opt::{
    KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V5,
    KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V5,
    KernelIrTargetNeutralOptimizationErrorV5, ProductionTransformationV1,
    optimize_production_kernel_ir_module_v5,
};

fn module(recursive: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let output = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), scalar.clone()),
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs: ValueId(0),
            rhs: ValueId(0),
        },
    ));
    if recursive {
        helper_block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(2), scalar.clone()),
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![ValueId(1)],
            },
        ));
    }
    helper_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![scalar.clone()], vec![scalar.clone()]),
        vec![ValueId(0)],
        vec![helper_block],
    );

    let mut root = BasicBlock::new(BlockId(0));
    root.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar.clone()),
            OperationKind::Call {
                callee: "helper".into(),
                arguments: vec![ValueId(0)],
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
    root.terminator = Some(Terminator::Return { values: vec![] });
    let kernel = Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![scalar, output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![root],
    );
    let mut module = Module::new("optimization-v5");
    module.functions.extend([kernel, helper]);
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

#[test]
fn expanded_policy_is_fixed_and_covers_the_new_coherent_families() {
    assert_eq!(
        KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V5,
        [
            ProductionTransformationV1::ScalarReplacementOfAggregates,
            ProductionTransformationV1::SecondarySsaPromotion,
            ProductionTransformationV1::HelperInlining,
            ProductionTransformationV1::InterproceduralCleanup,
        ]
    );
}

#[test]
fn expanded_policy_replays_exact_identity_and_epoch_chain() {
    let input = module(false);
    let output = optimize_production_kernel_ir_module_v5(&input).unwrap();
    assert_eq!(
        output.report().policy_version(),
        KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V5
    );
    assert!(output.report().is_exact_fixed_policy_replay());
    assert!(output.report().changed());
    assert!(!output.report().grants_semantic_preservation_authority());
    assert_eq!(output.module().functions.len(), 1);
    assert!(output.report().transformations().iter().any(|record| {
        record.transformation() == ProductionTransformationV1::HelperInlining && record.changed()
    }));
    assert!(output.report().transformations().iter().any(|record| {
        record.transformation() == ProductionTransformationV1::InterproceduralCleanup
            && record.changed()
    }));
    let operations = &output.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert!(!operations.iter().any(|operation| matches!(
        operation.kind,
        OperationKind::Call { .. } | OperationKind::Binary { .. }
    )));
}

#[test]
fn expanded_policy_is_deterministic_across_fresh_owner_sessions() {
    let input = module(false);
    let first = optimize_production_kernel_ir_module_v5(&input).unwrap();
    for _ in 0..8 {
        assert_eq!(
            optimize_production_kernel_ir_module_v5(&input).unwrap(),
            first
        );
    }
}

#[test]
fn recursive_scc_stops_the_expanded_policy() {
    assert!(matches!(
        optimize_production_kernel_ir_module_v5(&module(true)),
        Err(KernelIrTargetNeutralOptimizationErrorV5::Transform(_))
    ));
}
