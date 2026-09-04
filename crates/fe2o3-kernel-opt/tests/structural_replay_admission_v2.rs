use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, Function, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};
use fe2o3_kernel_opt::{
    KernelIrPlironOptimizationLimitsV2, KernelIrPlironStructuralReplayAdmissionErrorV2,
    admit_production_kernel_ir_structural_replay_v2, optimize_kernel_ir_module_v2,
    optimize_production_kernel_ir_module_v2,
};

fn value(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32))
}

fn optimizable_module(name: &str) -> Module {
    let mut entry = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    entry.operations = vec![
        Operation::effect_free(
            value(2),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::effect_free(
            value(3),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    ];
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let scalar = Type::Scalar(ScalarType::U32);
    let mut module = Module::new(name);
    module.functions.push(Function::definition(
        "deduplicate",
        Signature::new(vec![scalar.clone(), scalar.clone()], vec![scalar]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module
}

#[test]
fn admits_exact_closed_replay_without_claiming_semantic_refinement() {
    let input = optimizable_module("structural-replay");
    let live = optimize_production_kernel_ir_module_v2(&input).unwrap();
    assert!(live.report().changed());

    let admission =
        admit_production_kernel_ir_structural_replay_v2(&input, live.module(), live.report())
            .unwrap();
    assert!(admission.establishes_exact_closed_replay());
    assert!(admission.establishes_structural_well_formedness());
    assert!(!admission.establishes_semantic_preservation());
    assert!(!admission.grants_compiler_refinement_authority());
    assert_eq!(admission.input_digest(), live.report().input_digest());
    assert_eq!(admission.output_digest(), live.report().output_digest());
    assert_eq!(admission.report(), live.report());
}

#[test]
fn rejects_a_post_module_not_produced_by_the_replay() {
    let input = optimizable_module("post-mismatch");
    let live = optimize_production_kernel_ir_module_v2(&input).unwrap();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v2(&input, &input, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV2::OutputMismatch)
    ));
}

#[test]
fn rejects_a_report_from_another_production_execution() {
    let input = optimizable_module("report-mismatch-a");
    let live = optimize_production_kernel_ir_module_v2(&input).unwrap();
    let other_input = optimizable_module("report-mismatch-b");
    let other = optimize_production_kernel_ir_module_v2(&other_input).unwrap();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v2(&input, live.module(), other.report(),),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV2::ReportMismatch)
    ));
}

#[test]
fn rejects_a_configurable_policy_report_before_replay() {
    let input = optimizable_module("configurable-report");
    let configurable =
        optimize_kernel_ir_module_v2(&input, KernelIrPlironOptimizationLimitsV2::default())
            .unwrap();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v2(
            &input,
            configurable.module(),
            configurable.report(),
        ),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV2::NonProductionReport)
    ));
}
