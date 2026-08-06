use fe2o3_kernel_analysis::{Diagnostic, UnsupportedReason, Variation, analyze_function};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Axis, Barrier, BarrierSemantics, BasicBlock, BlockId,
    ComparePredicate, Constant, Convergence, Function, FunctionId, IndexKind, IntrinsicKind,
    IntrinsicOperation, MemoryAccess, MemoryOrdering, Operation, OperationKind, Signature,
    SynchronizationScope, Terminator, Type, ValueDef, ValueId, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent,
};

fn function(parameters: Vec<ValueId>, blocks: Vec<BasicBlock>) -> Function {
    Function::definition(
        "test",
        Signature::new(parameters.iter().map(|_| Type::INDEX).collect(), vec![]),
        parameters,
        blocks,
    )
}

fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}

fn intrinsic(id: u32, kind: IntrinsicKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::new(kind, Type::INDEX)),
    )
}

fn compare(id: u32, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::NotEqual,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn workgroup_barrier() -> Operation {
    Operation::new(
        vec![],
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
        }),
    )
}

fn explicit_workgroup_barrier() -> Operation {
    Operation::new(
        vec![],
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
            convergence: Convergence::uniform(SynchronizationScope::Workgroup),
        }),
    )
}

fn returning(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

#[test]
fn intrinsic_rules_cover_launch_hierarchy() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(0, IntrinsicKind::LaunchExtent { axis: Axis::X }),
        intrinsic(
            1,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Workgroup,
                axis: Axis::X,
            },
        ),
        intrinsic(
            2,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
        ),
        intrinsic(
            3,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        intrinsic(
            4,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::WorkgroupSize,
                axis: Axis::X,
            },
        ),
        intrinsic(
            5,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::WorkgroupCount,
                axis: Axis::X,
            },
        ),
    ];

    let report = analyze_function(&function(vec![], vec![entry]));

    assert_eq!(report.value(ValueId(0)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(1)), Variation::WorkgroupUniform);
    assert_eq!(report.value(ValueId(2)), Variation::Varying);
    assert_eq!(report.value(ValueId(3)), Variation::Varying);
    assert_eq!(report.value(ValueId(4)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(5)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn arithmetic_comparison_and_memory_rules_propagate_conservatively() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Workgroup,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Binary {
                op: fe2o3_kernel_ir::BinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        compare(3, 2, 1),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(5),
                Type::pointer(Type::INDEX, AddressSpace::Workgroup, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: Type::INDEX,
                count: None,
                address_space: AddressSpace::Workgroup,
                alignment: 8,
            },
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(6),
                Type::pointer(Type::INDEX, AddressSpace::Workgroup, AccessMode::ReadWrite),
            ),
            OperationKind::Alloca {
                element: Type::INDEX,
                count: Some(ValueId(4)),
                address_space: AddressSpace::Workgroup,
                alignment: 8,
            },
        ),
    ];

    let report = analyze_function(&function(vec![], vec![entry]));

    assert_eq!(report.value(ValueId(2)), Variation::WorkgroupUniform);
    assert_eq!(report.value(ValueId(3)), Variation::WorkgroupUniform);
    assert_eq!(report.value(ValueId(4)), Variation::Varying);
    assert_eq!(report.value(ValueId(5)), Variation::WorkgroupUniform);
    assert_eq!(report.value(ValueId(6)), Variation::Varying);
}

#[test]
fn uniform_branch_allows_workgroup_barrier() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(constant(0, Constant::Bool(true)));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(workgroup_barrier());
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut else_block = BasicBlock::new(BlockId(2));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });

    let report = analyze_function(&function(
        vec![],
        vec![entry, then_block, else_block, returning(3)],
    ));

    assert_eq!(report.block_control(BlockId(1)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn thread_varying_branch_rejects_workgroup_barrier() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(0)),
        compare(2, 0, 1),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block
        .operations
        .extend([workgroup_barrier(), explicit_workgroup_barrier()]);
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });

    let report = analyze_function(&function(vec![], vec![entry, then_block, returning(2)]));

    assert_eq!(report.block_control(BlockId(1)), Variation::Varying);
    assert_eq!(
        report.diagnostics(),
        &[
            Diagnostic::DivergentBarrier {
                block: BlockId(1),
                operation_index: 0,
                execution_scope: SynchronizationScope::Workgroup,
                control: Variation::Varying,
            },
            Diagnostic::DivergentBarrier {
                block: BlockId(1),
                operation_index: 1,
                execution_scope: SynchronizationScope::Workgroup,
                control: Variation::Varying,
            },
        ]
    );
}

#[test]
fn explicit_lds_pointer_is_workgroup_uniform() {
    let mut entry = returning(0);
    entry.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(0),
            Type::pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::F32,
            extent: WorkgroupMemoryExtent::Static(64),
            alignment: 4,
        }),
    ));

    let report = analyze_function(&function(vec![], vec![entry]));
    assert_eq!(report.value(ValueId(0)), Variation::WorkgroupUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn nested_control_retains_outer_divergence() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(0)),
        compare(2, 0, 1),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![],
    });
    let mut outer = BasicBlock::new(BlockId(1));
    outer.operations.push(constant(3, Constant::Bool(true)));
    outer.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut inner = BasicBlock::new(BlockId(2));
    inner.operations.push(workgroup_barrier());
    inner.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut inner_merge = BasicBlock::new(BlockId(3));
    inner_merge.terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![],
    });

    let report = analyze_function(&function(
        vec![],
        vec![entry, outer, inner, inner_merge, returning(4)],
    ));

    assert_eq!(report.block_control(BlockId(2)), Variation::Varying);
    assert!(matches!(
        report.diagnostics(),
        [Diagnostic::DivergentBarrier {
            block: BlockId(2),
            ..
        }]
    ));
}

#[test]
fn varying_exitless_loop_fails_closed() {
    let mut header = BasicBlock::new(BlockId(0));
    header.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(0)),
        compare(2, 0, 1),
    ]);
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(0),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(1));
    body.operations.push(workgroup_barrier());
    body.terminator = Some(Terminator::Branch {
        target: BlockId(0),
        arguments: vec![],
    });

    let report = analyze_function(&function(vec![], vec![header, body]));

    assert_eq!(report.block_control(BlockId(1)), Variation::Varying);
    assert!(report.diagnostics().contains(&Diagnostic::Unsupported {
        block: None,
        operation_index: None,
        reason: UnsupportedReason::PostdominanceUnavailable {
            blocks: vec![BlockId(0), BlockId(1)],
        },
    }));
    assert!(
        report
            .diagnostics()
            .contains(&Diagnostic::DivergentBarrier {
                block: BlockId(1),
                operation_index: 0,
                execution_scope: SynchronizationScope::Workgroup,
                control: Variation::Varying,
            })
    );
}

#[test]
fn reconverged_merge_clears_control_but_not_phi_variation() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(0)),
        compare(2, 0, 1),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(constant(3, Constant::Index(10)));
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(3)],
    });
    let mut else_block = BasicBlock::new(BlockId(2));
    else_block.operations.push(constant(4, Constant::Index(20)));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(4)],
    });
    let mut merge = returning(3);
    merge
        .parameters
        .push(ValueDef::new(ValueId(5), Type::INDEX));
    merge.operations.push(workgroup_barrier());

    let report = analyze_function(&function(
        vec![],
        vec![entry, then_block, else_block, merge],
    ));

    assert_eq!(report.block_control(BlockId(3)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(5)), Variation::Varying);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn parameters_calls_and_unknown_values_fail_closed() {
    let mut entry = returning(0);
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Call {
                callee: FunctionId::new("helper"),
                arguments: vec![ValueId(0)],
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Bitcast,
                value: ValueId(99),
                to: Type::INDEX,
            },
        ),
    ];

    let report = analyze_function(&function(vec![ValueId(0)], vec![entry]));

    assert_eq!(report.value(ValueId(0)), Variation::Varying);
    assert_eq!(report.value(ValueId(1)), Variation::Varying);
    assert_eq!(report.value(ValueId(2)), Variation::Varying);
    assert!(report.diagnostics().contains(&Diagnostic::Unsupported {
        block: Some(BlockId(0)),
        operation_index: Some(0),
        reason: UnsupportedReason::CallWithoutSummary {
            callee: FunctionId::new("helper"),
        },
    }));
    assert!(report.diagnostics().contains(&Diagnostic::Unsupported {
        block: None,
        operation_index: None,
        reason: UnsupportedReason::UnknownValue { value: ValueId(99) },
    }));
}

#[test]
fn declarations_are_explicitly_unsupported() {
    let declaration = Function::declaration("external", Signature::new(vec![], vec![]));

    let report = analyze_function(&declaration);

    assert_eq!(
        report.diagnostics(),
        &[Diagnostic::Unsupported {
            block: None,
            operation_index: None,
            reason: UnsupportedReason::FunctionDeclaration,
        }]
    );
}
