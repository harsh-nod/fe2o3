use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function,
    Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, Signature,
    Terminator, Type, ValueDef, ValueId, verify_module,
};
use fe2o3_kernel_opt::{
    CheckedModuleTransformRelationV1, CheckedOptimizerQueryKindV1,
    KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V6,
    KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V6,
    KernelIrTargetNeutralStructuralReplayAdmissionErrorV6, ProductionTransformationV1,
    admit_production_kernel_ir_structural_replay_v6, optimize_production_kernel_ir_module_v6,
};

fn static_loop() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
    ]);
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });

    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = vec![ValueDef::new(ValueId(5), Type::INDEX)];
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(6), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(5),
            rhs: ValueId(4),
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(6),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(5)],
    });

    let mut body = BasicBlock::new(BlockId(2));
    body.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(7), Type::INDEX),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(5),
            rhs: ValueId(3),
        },
    ));
    body.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(7)],
    });

    let mut exit = BasicBlock::new(BlockId(3));
    exit.parameters = vec![ValueDef::new(ValueId(8), Type::INDEX)];
    exit.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(8),
            access: MemoryAccess::new(AddressSpace::Global, 8),
        },
    ));
    exit.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("optimization-v6");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![Type::pointer(
                Type::INDEX,
                AddressSpace::Global,
                AccessMode::WriteOnly,
            )],
            vec![],
        ),
        vec![ValueId(0)],
        vec![entry, header, body, exit],
    ));
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

fn guarded_affine_loop() -> Module {
    let mut module = static_loop();
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::INDEX);
    function.body.as_mut().unwrap().parameters.push(ValueId(1));
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations[2] = Operation::effect_free(
        ValueDef::new(ValueId(4), Type::INDEX),
        OperationKind::Constant(Constant::Index(2)),
    );
    body.blocks[0].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(9),
                rhs: ValueId(10),
            },
        ),
    ]);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(2)],
    });
    let OperationKind::Compare { rhs, .. } = &mut body.blocks[1].operations[0].kind else {
        unreachable!()
    };
    *rhs = ValueId(9);
    let mut preheader = BasicBlock::new(BlockId(4));
    preheader.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    body.blocks.insert(1, preheader);
    verify_module(&module).unwrap();
    module
}

#[test]
fn v6_loop_memory_wave_has_one_fixed_order_after_v5() {
    assert_eq!(
        KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V6,
        [
            ProductionTransformationV1::LoopCanonicalization,
            ProductionTransformationV1::InductionVariableSimplification,
            ProductionTransformationV1::LoopInvariantCodeMotion,
            ProductionTransformationV1::MemoryEffectVersioning,
            ProductionTransformationV1::MemorySimplification,
            ProductionTransformationV1::FullLoopUnrolling,
            ProductionTransformationV1::PartialLoopUnrolling,
        ]
    );
}

#[test]
fn v6_composes_exact_relations_identities_and_epochs() {
    let optimized = optimize_production_kernel_ir_module_v6(&static_loop()).unwrap();
    assert_eq!(
        optimized.report().policy_version(),
        KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V6
    );
    assert!(optimized.report().is_exact_fixed_policy_replay());
    assert!(optimized.report().changed());
    assert!(!optimized.report().grants_semantic_preservation_authority());
    assert!(optimized.report().transformations().iter().any(|record| {
        record.transformation() == ProductionTransformationV1::MemoryEffectVersioning
            && matches!(
                record.module_relation(),
                Some(CheckedModuleTransformRelationV1::LoopMemory(report))
                    if report.memory_graph().is_some()
            )
    }));
    assert!(optimized.report().transformations().iter().any(|record| {
        record.transformation() == ProductionTransformationV1::FullLoopUnrolling && record.changed()
    }));
}

#[test]
fn v6_is_deterministic_across_fresh_sessions() {
    let input = static_loop();
    let first = optimize_production_kernel_ir_module_v6(&input).unwrap();
    for _ in 0..4 {
        assert_eq!(
            optimize_production_kernel_ir_module_v6(&input).unwrap(),
            first
        );
    }
}

#[test]
fn v6_replays_the_affine_dynamic_trip_proof_at_the_unroll_boundary() {
    let optimized = optimize_production_kernel_ir_module_v6(&guarded_affine_loop()).unwrap();
    let record = optimized
        .report()
        .transformations()
        .iter()
        .find(|record| record.transformation() == ProductionTransformationV1::FullLoopUnrolling)
        .unwrap();
    assert!(record.changed());
    assert!(
        record
            .optimizer_query_replays()
            .iter()
            .any(|query| matches!(
                query.kind(),
                CheckedOptimizerQueryKindV1::DominatingIndexEqualsConstant { constant: 4, .. }
            ))
    );
}

#[test]
fn v6_structural_admission_replays_output_and_report_independently() {
    let input = static_loop();
    let live = optimize_production_kernel_ir_module_v6(&input).unwrap();
    let admission =
        admit_production_kernel_ir_structural_replay_v6(&input, live.module(), live.report())
            .unwrap();
    assert_eq!(admission.input_identity(), live.report().input_identity());
    assert_eq!(admission.output_identity(), live.report().output_identity());
    assert!(admission.establishes_exact_closed_replay());
    assert!(!admission.establishes_semantic_preservation());

    let mut hostile_output = live.module().clone();
    hostile_output.id = "hostile-output".into();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v6(&input, &hostile_output, live.report(),),
        Err(KernelIrTargetNeutralStructuralReplayAdmissionErrorV6::OutputMismatch)
    ));

    let report_for_already_optimized =
        optimize_production_kernel_ir_module_v6(live.module()).unwrap();
    assert_eq!(report_for_already_optimized.module(), live.module());
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v6(
            &input,
            live.module(),
            report_for_already_optimized.report(),
        ),
        Err(KernelIrTargetNeutralStructuralReplayAdmissionErrorV6::ReportMismatch)
    ));
}
