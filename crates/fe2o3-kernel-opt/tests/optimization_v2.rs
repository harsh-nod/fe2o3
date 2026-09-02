use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, Function, KernelIrEncodeError, MAX_TEXT_BYTES_V1, Module,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    encode_module_v9, verify_module,
};
use fe2o3_kernel_opt::{
    KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE,
    KernelIrPlironOptimizationByteLimitV2, KernelIrPlironOptimizationErrorV2,
    KernelIrPlironOptimizationLimitsV2, MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
    optimize_kernel_ir_module_at_epoch_v2, optimize_kernel_ir_module_v2,
};
use fe2o3_pliron::{
    PlironOptimizationErrorV1, PlironOptimizationLimitsV1, PlironOptimizationPlanErrorV1,
    PlironOptimizationPlanV1, ShellLimits,
};

fn u32_value(id: u32) -> ValueDef {
    ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32))
}

fn optimizable_module() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        Operation::effect_free(
            u32_value(2),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::effect_free(
            u32_value(3),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::effect_free(
            u32_value(4),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });

    let u32_type = Type::Scalar(ScalarType::U32);
    let function = Function::definition(
        "deduplicate",
        Signature::new(vec![u32_type.clone(), u32_type.clone()], vec![u32_type]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    );
    let mut module = Module::new("pliron-v2-test");
    module.functions.push(function);
    module
}

fn limits_with_pliron(pliron: PlironOptimizationLimitsV1) -> KernelIrPlironOptimizationLimitsV2 {
    KernelIrPlironOptimizationLimitsV2::new(
        MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
        MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
        ShellLimits::default(),
        pliron,
    )
    .unwrap()
}

#[test]
fn standard_pipeline_publishes_verified_output_digests_reports_and_epochs() {
    let input = optimizable_module();
    verify_module(&input).unwrap();
    let input_bytes = encode_module_v9(&input).unwrap();

    let optimized =
        optimize_kernel_ir_module_v2(&input, KernelIrPlironOptimizationLimitsV2::default())
            .unwrap();
    verify_module(optimized.module()).unwrap();

    let report = optimized.report();
    assert!(report.changed());
    assert_ne!(optimized.module(), &input);
    assert_eq!(
        report.input_digest().canonical_bytes(),
        u64::try_from(input_bytes.len()).unwrap()
    );
    assert_eq!(
        report.output_digest().canonical_bytes(),
        u64::try_from(optimized.canonical().canonical_bytes().len()).unwrap()
    );
    assert_eq!(
        report
            .passes()
            .iter()
            .map(|pass| pass.pliron().pass())
            .collect::<Vec<_>>(),
        PlironOptimizationPlanV1::standard().passes()
    );
    assert_eq!(report.passes().len(), report.pliron().passes().len());

    let mut epoch = report.initial_epoch();
    for pass in report.passes() {
        assert_eq!(pass.input_epoch(), epoch);
        if pass.pliron().changed() {
            epoch += 1;
        }
        assert_eq!(pass.output_epoch(), epoch);
    }
    assert_eq!(report.final_epoch(), epoch);
    assert!(report.final_epoch() > report.initial_epoch());
}

#[test]
fn fresh_sessions_produce_deterministic_outputs_and_accounting() {
    let input = optimizable_module();
    let first = optimize_kernel_ir_module_at_epoch_v2(
        &input,
        19,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();
    let second = optimize_kernel_ir_module_at_epoch_v2(
        &input,
        19,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.report().initial_epoch(), 19);
}

#[test]
fn noop_pipeline_preserves_canonical_identity_and_epoch() {
    let input = Module::new("pliron-v2-noop");
    verify_module(&input).unwrap();
    let optimized = optimize_kernel_ir_module_at_epoch_v2(
        &input,
        73,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();

    assert_eq!(optimized.module(), &input);
    assert_eq!(
        optimized.report().input_digest(),
        optimized.report().output_digest()
    );
    assert!(!optimized.report().changed());
    assert_eq!(optimized.report().initial_epoch(), 73);
    assert_eq!(optimized.report().final_epoch(), 73);
    assert!(
        optimized
            .report()
            .passes()
            .iter()
            .all(|pass| !pass.pliron().changed()
                && pass.input_epoch() == 73
                && pass.output_epoch() == 73)
    );
}

#[test]
fn invalid_ir_and_bounded_encode_failure_are_rejected_before_session_publication() {
    let mut missing_terminator = Module::new("invalid");
    missing_terminator.functions.push(Function::definition(
        "invalid",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock::new(BlockId(0))],
    ));
    let original = missing_terminator.clone();
    assert!(matches!(
        optimize_kernel_ir_module_v2(
            &missing_terminator,
            KernelIrPlironOptimizationLimitsV2::default()
        ),
        Err(KernelIrPlironOptimizationErrorV2::InputCanonicalization(_))
    ));
    assert_eq!(missing_terminator, original);

    let oversized = Module::new("x".repeat(MAX_TEXT_BYTES_V1 + 1));
    assert!(matches!(
        optimize_kernel_ir_module_v2(&oversized, KernelIrPlironOptimizationLimitsV2::default()),
        Err(KernelIrPlironOptimizationErrorV2::InputEncoding(
            KernelIrEncodeError::LimitExceeded { .. }
        ))
    ));
}

#[test]
fn configured_input_and_output_admission_limits_fail_closed() {
    let input = optimizable_module();
    let original = input.clone();
    let input_length = encode_module_v9(&input).unwrap().len();
    let input_limited = KernelIrPlironOptimizationLimitsV2::new(
        input_length - 1,
        MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
        ShellLimits::default(),
        PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        optimize_kernel_ir_module_v2(&input, input_limited)
            .unwrap_err()
            .to_string(),
        format!(
            "canonical V9 input requires {input_length} bytes but the limit is {}",
            input_length - 1
        )
    );
    assert_eq!(input, original);

    let baseline =
        optimize_kernel_ir_module_v2(&input, KernelIrPlironOptimizationLimitsV2::default())
            .unwrap();
    let output_length = baseline.canonical().canonical_bytes().len();
    let output_limited = KernelIrPlironOptimizationLimitsV2::new(
        MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
        output_length - 1,
        ShellLimits::default(),
        PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    assert!(matches!(
        optimize_kernel_ir_module_v2(&input, output_limited),
        Err(KernelIrPlironOptimizationErrorV2::OutputByteLimitExceeded {
            required,
            limit
        }) if required == output_length && limit == output_length - 1
    ));
    assert_eq!(input, original);
}

#[test]
fn pass_and_graph_budgets_are_mapped_to_the_closed_executor() {
    let input = optimizable_module();
    let defaults = PlironOptimizationLimitsV1::default();
    let too_few_passes = PlironOptimizationLimitsV1::new(
        PlironOptimizationPlanV1::standard().passes().len() - 1,
        defaults.max_graph_work(),
        defaults.max_work_units(),
    )
    .unwrap();
    assert!(matches!(
        optimize_kernel_ir_module_v2(&input, limits_with_pliron(too_few_passes)),
        Err(KernelIrPlironOptimizationErrorV2::Plan(
            PlironOptimizationPlanErrorV1::TooManyPasses { .. }
        ))
    ));

    let graph_limited = PlironOptimizationLimitsV1::new(
        PlironOptimizationPlanV1::standard().passes().len(),
        1,
        defaults.max_work_units(),
    )
    .unwrap();
    let original = input.clone();
    assert!(matches!(
        optimize_kernel_ir_module_v2(&input, limits_with_pliron(graph_limited)),
        Err(KernelIrPlironOptimizationErrorV2::Optimize(
            PlironOptimizationErrorV1::GraphWorkLimitExceeded { .. }
        ))
    ));
    assert_eq!(input, original);
}

#[test]
fn epoch_overflow_discards_an_already_optimized_private_candidate() {
    let input = optimizable_module();
    let original = input.clone();
    assert!(matches!(
        optimize_kernel_ir_module_at_epoch_v2(
            &input,
            u64::MAX,
            KernelIrPlironOptimizationLimitsV2::default()
        ),
        Err(KernelIrPlironOptimizationErrorV2::EpochOverflow)
    ));
    assert_eq!(input, original);
}

#[test]
fn byte_limits_are_nonzero_hard_bounded_admission_controls() {
    assert!(matches!(
        KernelIrPlironOptimizationLimitsV2::new(
            0,
            1,
            ShellLimits::default(),
            PlironOptimizationLimitsV1::default()
        ),
        Err(KernelIrPlironOptimizationErrorV2::InvalidByteLimit {
            limit: KernelIrPlironOptimizationByteLimitV2::Input,
            ..
        })
    ));
    assert!(matches!(
        KernelIrPlironOptimizationLimitsV2::new(
            1,
            MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2 + 1,
            ShellLimits::default(),
            PlironOptimizationLimitsV1::default()
        ),
        Err(KernelIrPlironOptimizationErrorV2::InvalidByteLimit {
            limit: KernelIrPlironOptimizationByteLimitV2::Output,
            ..
        })
    ));
}

#[test]
fn v2_evidence_is_explicitly_outside_frozen_production_replay() {
    const {
        assert!(!KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE);
    }
    let optimized = optimize_kernel_ir_module_v2(
        &optimizable_module(),
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();
    assert!(!optimized.report().is_production_replay_compatible());
}
