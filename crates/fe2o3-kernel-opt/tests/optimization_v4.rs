use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ExecutionCapabilityRequirementV1,
    Function, FunctionId, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, ScalarType, Signature, TargetCapability,
    Terminator, Type, ValueDef, ValueId,
};
use fe2o3_kernel_opt::{
    KernelIrPlironOptimizationLimitsV2, KernelIrPlironStructuralReplayAdmissionErrorV4,
    admit_production_kernel_ir_structural_replay_v4, optimize_kernel_ir_module_at_epoch_v4,
    optimize_production_kernel_ir_module_v4,
};

fn context(root: &str) -> KernelContextTypeV1 {
    KernelContextTypeV1::new(root, [1; 32], [2; 32], [3; 32])
}

fn source(byte: u8) -> KernelContextSourceIdentityV1 {
    KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [byte; 32])
}

fn context_module() -> Module {
    let context = context("entry");
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.terminator = Some(Terminator::Return { values: vec![] });
    let helper = Function::internal_helper(
        "helper",
        Signature::new(vec![Type::KernelContext(context.clone())], vec![]),
        vec![ValueId(0)],
        vec![helper_block],
    );

    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context,
        source(7),
    ));
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![ValueId(0)],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("context-v4");
    module.functions.extend([
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        ),
        helper,
    ]);
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module.required_capabilities = BTreeSet::from([TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        },
    )]);
    module
}

fn scalar_module() -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), ty.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), ty.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut module = Module::new("scalar-v4");
    module.functions.push(Function::internal_helper(
        "scalar",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

#[test]
fn v11_compatible_v13_graph_runs_the_closed_v13_optimizer() {
    let input = scalar_module();
    let optimized = optimize_production_kernel_ir_module_v4(&input).unwrap();
    assert_eq!(
        &optimized.canonical().canonical_bytes()[8..10],
        &fe2o3_kernel_ir::KERNEL_IR_VERSION_V13.to_le_bytes()
    );
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
fn v13_protected_facts_survive_the_native_optimizer_and_epoch_replay() {
    let input = context_module();
    let optimized = optimize_kernel_ir_module_at_epoch_v4(
        &input,
        31,
        KernelIrPlironOptimizationLimitsV2::default(),
    )
    .unwrap();
    assert_eq!(optimized.report().initial_epoch(), 31);
    assert_eq!(
        optimized.report().capability_replay().input_epoch(),
        optimized.report().initial_epoch()
    );
    assert_eq!(
        optimized.report().capability_replay().output_epoch(),
        optimized.report().final_epoch()
    );
    assert!(!optimized.report().grants_semantic_preservation_authority());
    assert_eq!(
        optimized.report().capability_pass_replays().len(),
        optimized.report().optimizer().passes().len()
    );
    assert!(
        optimized
            .report()
            .capability_pass_replays()
            .iter()
            .all(|pass| !pass.grants_semantic_preservation_authority())
    );
    let mut epoch = optimized.report().initial_epoch();
    for pass in optimized.report().capability_pass_replays() {
        assert_eq!(pass.replay().input_epoch(), epoch);
        epoch = pass.replay().output_epoch();
    }
    assert_eq!(epoch, optimized.report().final_epoch());

    let entry = &optimized.module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0];
    assert!(matches!(
        entry.operations[0].kind,
        OperationKind::KernelContextIssue(_)
    ));
    assert_eq!(
        optimized.module().required_capabilities,
        input.required_capabilities
    );
}

#[test]
fn portable_requirements_survive_optimization_of_the_same_v13_graph() {
    let mut input = scalar_module();
    input.required_capabilities = BTreeSet::from([TargetCapability::Execution(
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
        },
    )]);

    let optimized = optimize_production_kernel_ir_module_v4(&input).unwrap();
    assert_eq!(
        optimized.module().required_capabilities,
        input.required_capabilities
    );
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
    assert!(optimized.report().capability_replay().changed());
}

#[test]
fn exact_v13_replay_rejects_deleted_duplicated_and_reordered_issuance() {
    let input = context_module();
    let live = optimize_production_kernel_ir_module_v4(&input).unwrap();

    let mut deleted = input.clone();
    deleted.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v4(&input, &deleted, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch)
    ));

    let mut duplicated = input.clone();
    duplicated.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            1,
            Operation::kernel_context_issue(ValueId(9), context("entry"), source(8)),
        );
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v4(&input, &duplicated, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch)
    ));

    let mut reordered = input.clone();
    reordered.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(0, 1);
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v4(&input, &reordered, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch)
    ));

    let admitted =
        admit_production_kernel_ir_structural_replay_v4(&input, live.module(), live.report())
            .unwrap();
    assert!(admitted.establishes_exact_closed_replay());
    assert!(!admitted.establishes_semantic_preservation());
}

#[test]
fn exact_v13_replay_rejects_root_source_and_requirement_substitution() {
    let input = context_module();
    let live = optimize_production_kernel_ir_module_v4(&input).unwrap();

    let mut root = input.clone();
    root.functions[0].id = FunctionId::new("other-entry");
    root.kernels[0].entry = FunctionId::new("other-entry");
    root.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        Operation::kernel_context_issue(ValueId(0), context("other-entry"), source(7));
    root.functions[1].signature.parameters[0] = Type::KernelContext(context("other-entry"));
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v4(&input, &root, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch)
    ));

    let mut source = input.clone();
    source.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        Operation::kernel_context_issue(ValueId(0), context("entry"), self::source(9));
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v4(&input, &source, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch)
    ));

    let mut requirements = input.clone();
    requirements.required_capabilities.clear();
    assert!(matches!(
        admit_production_kernel_ir_structural_replay_v4(&input, &requirements, live.report()),
        Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch)
    ));
}
