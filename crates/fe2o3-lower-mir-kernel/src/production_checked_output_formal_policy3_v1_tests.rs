use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BarrierSemantics, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Fence, Function, FunctionId, Kernel, MemoryAccess, MemoryOrdering, Module, Operation,
    OperationKind, ScalarType, Signature, SynchronizationScope, Terminator, Type, ValueDef,
    ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy3_v1;
use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1 as Checked;

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 17;

// These are verified KIR component fixtures, not semantic-source or launch
// admission fixtures. Every check runs the real fixed policy and semantic check.
fn component_module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
    let arguments = (0..parameters.len())
        .map(|index| ValueId(u32::try_from(index).unwrap()))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("policy3-formal-component");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        arguments,
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn with_component(module: Module, check: impl FnOnce(&Owner, &Checked)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (input, input_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    drop(module);
    let checked = optimize_checked_canonical_kernel_ir_policy3_v1(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + input_storage.retained_storage());
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    assert_eq!(
        checked.native_input_audit_bytes(),
        input.canonical().canonical_bytes()
    );
    let passes = checked.report().passes();
    assert_eq!(passes.len(), 8);
    assert_eq!(
        passes[5].pass().name(),
        "dominance-pure-common-subexpression-elimination"
    );
    assert!(!checked.grants_authority());
    check(&input, &checked);
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn read_module() -> Module {
    component_module(
        vec![Type::pointer(
            Type::F32,
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )],
        vec![Operation::effect_free(
            ValueDef::new(ValueId(1), Type::F32),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )],
    )
}

fn dominating_private_diamond() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let expression = |id| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), scalar.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        )
    };
    let store = |value| {
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(4),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        )
    };
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        Operation::new(
            vec![ValueDef::new(
                ValueId(4),
                Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            )],
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        expression(5),
        store(5),
        Operation::effect_free(
            ValueDef::new(ValueId(6), scalar.clone()),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(30),
        then_arguments: vec![],
        else_target: BlockId(70),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(30));
    left.operations = vec![expression(7), store(7)];
    left.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut right = BasicBlock::new(BlockId(70));
    right.operations = vec![expression(8), store(8)];
    right.terminator = Some(Terminator::Branch {
        target: BlockId(90),
        arguments: vec![],
    });
    let mut join = BasicBlock::new(BlockId(90));
    join.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("policy3-formal-diamond");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly),
                Type::BOOL,
                scalar.clone(),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![entry, left, right, join],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn operation_count(module: &Module, predicate: impl Fn(&OperationKind) -> bool) -> usize {
    module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| predicate(&operation.kind))
        .count()
}

#[test]
fn policy3_mutated_actual_output_has_fresh_complete_obligations() {
    with_component(dominating_private_diamond(), |input, checked| {
        let binary = |kind: &OperationKind| {
            matches!(
                kind,
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    ..
                }
            )
        };
        assert_eq!(operation_count(input.module(), binary), 3);
        assert_eq!(operation_count(checked.owner().module(), binary), 1);
        assert!(checked.report().passes()[5].changed());
        assert_ne!(
            input.canonical().identity(),
            checked.owner().canonical().identity()
        );
        assert_eq!(
            operation_count(checked.owner().module(), |kind| matches!(
                kind,
                OperationKind::Store { .. }
            )),
            3
        );
        let report = analyze_checked_output_formal_memory_policy3_v1(checked).unwrap();
        assert!(std::ptr::eq(report.output(), checked.owner()));
        assert!(!std::ptr::eq(report.output(), input));
        let [kernel] = report.output().module().kernels.as_slice() else {
            panic!("one kernel")
        };
        let [obligations] = report.kernels() else {
            panic!("one report")
        };
        assert_eq!(obligations.kernel(), &kernel.id);
        assert_eq!(obligations.entry(), &kernel.entry);
        assert_eq!(obligations.index_width(), FormalIndexWidth::Bits64);
        assert_eq!(obligations.accesses().len(), 1);
        assert!(obligations.inter_invocation_conflicts().is_empty());
        let invocations = obligations.invocations().unwrap();
        assert_eq!(invocations.end_exclusive() - invocations.start(), 2);
        let fresh = derive_kernel_memory_obligations_for_launch(
            checked.owner().module(),
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [2, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(matches!(fresh, FormalMemoryObligationAnalysis::Complete(_)));
        assert_eq!(obligations, fresh.obligations());
    });
}

#[test]
fn policy3_report_preserves_kernel_order_and_fixed_dynamic_or_static_witness() {
    let mut module = read_module();
    let mut second = BasicBlock::new(BlockId(0));
    second.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "second",
        Signature::new(vec![], vec![]),
        vec![],
        vec![second],
    ));
    module.kernels.push(Kernel::new(
        "static",
        "second",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(7),
        },
    ));
    with_component(module, |_, checked| {
        let report = analyze_checked_output_formal_memory_policy3_v1(checked).unwrap();
        assert_eq!(report.kernels().len(), 2);
        for ((obligations, kernel), extent) in report
            .kernels()
            .iter()
            .zip(&report.output().module().kernels)
            .zip([2, 7])
        {
            assert_eq!(obligations.kernel(), &kernel.id);
            assert_eq!(obligations.entry(), &kernel.entry);
            let invocations = obligations.invocations().unwrap();
            assert_eq!(invocations.end_exclusive() - invocations.start(), extent);
        }
        assert_eq!(report.kernels()[0].accesses().len(), 1);
        assert!(report.kernels()[1].accesses().is_empty());
    });
}

#[test]
fn policy3_actual_output_without_kernels_rejects() {
    with_component(Module::new("empty"), |_, checked| {
        assert!(matches!(
            analyze_checked_output_formal_memory_policy3_v1(checked),
            Err(ProductionFormalMemoryErrorV1::KernelCount { actual: 0 })
        ));
    });
}

#[test]
fn policy3_actual_external_call_cannot_import_a_discharge() {
    let mut module = component_module(
        vec![],
        vec![Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("external"),
                arguments: vec![],
            },
        )],
    );
    module.functions.push(Function::declaration(
        "external",
        Signature::new(vec![], vec![]),
    ));
    with_component(module, |_, checked| {
        let Err(ProductionFormalMemoryErrorV1::Incomplete { reasons }) =
            analyze_checked_output_formal_memory_policy3_v1(checked)
        else {
            panic!("unknown external effects must reject")
        };
        assert!(matches!(reasons.as_ref(),
            [FormalMemoryIncompleteReason::CallEffectsUnavailable { location, callee }]
                if location.block == BlockId(0) && location.operation_index == 0
                    && callee.as_str() == "external"));
    });
}

#[test]
fn policy3_actual_fence_cannot_be_reported_memory_complete() {
    let module = component_module(
        vec![],
        vec![Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(MemoryOrdering::Release, [AddressSpace::Global]),
            }),
        )],
    );
    with_component(module, |_, checked| {
        assert_eq!(
            operation_count(checked.owner().module(), |kind| matches!(
                kind,
                OperationKind::Fence(_)
            )),
            1
        );
        let Err(ProductionFormalMemoryErrorV1::Incomplete { reasons }) =
            analyze_checked_output_formal_memory_policy3_v1(checked)
        else {
            panic!("unsupported fence must reject")
        };
        assert!(matches!(reasons.as_ref(),
            [FormalMemoryIncompleteReason::UnsupportedMemoryEffect { location }]
                if location.block == BlockId(0) && location.operation_index == 0));
    });
}

#[test]
fn policy3_actual_constant_address_write_rejects_inter_invocation_collision() {
    let module = component_module(
        vec![
            Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
            Type::F32,
        ],
        vec![Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )],
    );
    with_component(module, |_, checked| {
        let Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts { conflicts }) =
            analyze_checked_output_formal_memory_policy3_v1(checked)
        else {
            panic!("two witness invocations collide")
        };
        assert_eq!(conflicts.len(), 1);
    });
}
