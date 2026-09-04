use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CopyNonOverlappingContract, Function,
    Kernel, KernelIrEncodeError, LaunchDomain, LaunchExtent, MAX_TEXT_BYTES_V1, MemoryElementType,
    MemoryIntrinsicOperation, MemoryLayout, Module, Operation, OperationKind,
    PointerDistanceContract, PointerDistanceKind, PointerDistanceUnit, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VolatileAccessContract, encode_module_v10, verify_module,
};
use fe2o3_kernel_opt::{
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2,
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_POLICY_VERSION_V2,
    KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE,
    KernelIrPlironOptimizationByteLimitV2, KernelIrPlironOptimizationErrorV2,
    KernelIrPlironOptimizationLimitsV2, KernelIrPlironOptimizationPolicyV2,
    MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2, optimize_kernel_ir_module_at_epoch_v2,
    optimize_kernel_ir_module_v2, optimize_production_kernel_ir_module_v2,
    production_kernel_ir_pliron_optimization_limits_v2,
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

fn memory_intrinsic_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let source = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let destination = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let element = MemoryElementType::Scalar(ScalarType::U32);
    let layout = MemoryLayout::new(4, 4);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::I64)),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
                pointer: ValueId(0),
                origin: ValueId(0),
                kind: PointerDistanceKind::Signed,
                unit: PointerDistanceUnit::Elements,
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), scalar.clone()),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(0),
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(1),
                value: ValueId(3),
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(0),
                destination: ValueId(1),
                count: ValueId(2),
                element,
                source_address_space: AddressSpace::Global,
                destination_address_space: AddressSpace::Global,
                layout,
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "memory_impl",
        Signature::new(vec![source, destination, Type::INDEX, scalar], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    let mut module = Module::new("pliron-v2-memory-intrinsics");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "memory",
        "memory_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn v10_memory_intrinsics_round_trip_through_the_production_optimizer() {
    let input = memory_intrinsic_module();
    let input_bytes = encode_module_v10(&input).unwrap();
    let optimized = optimize_production_kernel_ir_module_v2(&input).unwrap();

    assert_eq!(optimized.module(), &input);
    assert_eq!(optimized.canonical().canonical_bytes(), input_bytes);
    assert!(!optimized.report().changed());
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
    let input_bytes = encode_module_v10(&input).unwrap();

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
fn production_entry_uses_the_fixed_v2_pipeline() {
    let input = optimizable_module();
    let production = optimize_production_kernel_ir_module_v2(&input).unwrap();
    let configured =
        optimize_kernel_ir_module_v2(&input, KernelIrPlironOptimizationLimitsV2::default())
            .unwrap();

    assert_eq!(production.module(), configured.module());
    assert_eq!(production.canonical(), configured.canonical());
    assert_eq!(
        production.report().limits(),
        production_kernel_ir_pliron_optimization_limits_v2()
    );
    assert_eq!(
        configured.report().limits(),
        KernelIrPlironOptimizationLimitsV2::default()
    );
    assert_eq!(production.report().bridge(), configured.report().bridge());
    assert_eq!(production.report().pliron(), configured.report().pliron());
    assert_eq!(production.report().passes(), configured.report().passes());
    assert_eq!(
        production.report().policy(),
        KernelIrPlironOptimizationPolicyV2::ProductionV1
    );
    assert_eq!(
        configured.report().policy(),
        KernelIrPlironOptimizationPolicyV2::Configurable
    );
    assert_eq!(production.report().passes().len(), 7);
    assert_eq!(
        KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_POLICY_VERSION_V2,
        1
    );
    assert_eq!(
        production
            .report()
            .passes()
            .iter()
            .map(|pass| pass.pliron().pass().name())
            .collect::<Vec<_>>(),
        KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
            .iter()
            .map(|pass| pass.name())
            .collect::<Vec<_>>()
    );
    assert!(production.report().is_production_replay_compatible());
    assert!(!configured.report().is_production_replay_compatible());
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
    let input_length = encode_module_v10(&input).unwrap().len();
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
            "canonical V10 input requires {input_length} bytes but the limit is {}",
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
fn only_the_closed_production_report_is_admitted_by_production_v4_replay() {
    const {
        assert!(KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE);
    }
    let production = optimize_production_kernel_ir_module_v2(&optimizable_module()).unwrap();
    let configured = optimize_kernel_ir_module_v2(
        &optimizable_module(),
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();
    let custom_limits = KernelIrPlironOptimizationLimitsV2::new(
        MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2 - 1,
        MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2 - 1,
        ShellLimits::default(),
        PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    let custom = optimize_kernel_ir_module_v2(&optimizable_module(), custom_limits).unwrap();
    let nonzero_epoch = optimize_kernel_ir_module_at_epoch_v2(
        &optimizable_module(),
        1,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();

    assert!(production.report().is_production_replay_compatible());
    assert!(!configured.report().is_production_replay_compatible());
    assert!(!custom.report().is_production_replay_compatible());
    assert!(!nonzero_epoch.report().is_production_replay_compatible());
    assert_eq!(custom.report().limits(), custom_limits);
    assert_eq!(
        production.report().limits(),
        production_kernel_ir_pliron_optimization_limits_v2()
    );
    let production_limits = production_kernel_ir_pliron_optimization_limits_v2();
    assert_eq!(production_limits.max_input_canonical_bytes(), 16_777_216);
    assert_eq!(production_limits.max_output_canonical_bytes(), 16_777_216);
    assert_eq!(production_limits.shell().max_dialects(), 32);
    assert_eq!(production_limits.shell().max_passes(), 64);
    assert_eq!(production_limits.shell().max_diagnostic_bytes(), 512);
    assert_eq!(production_limits.pliron().max_passes(), 256);
    assert_eq!(production_limits.pliron().max_graph_work(), 16_384);
    assert_eq!(production_limits.pliron().max_work_units(), 12_636_160);
}
