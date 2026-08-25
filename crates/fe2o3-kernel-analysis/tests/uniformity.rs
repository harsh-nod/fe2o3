use fe2o3_kernel_analysis::{
    Diagnostic, UnsupportedReason, Variation, analyze_function, analyze_kernel_entry,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AmdGpuDiagnosticOperation, Axis, Barrier, BarrierSemantics,
    BasicBlock, BinaryOp, BlockId, CastKind, CheckedBinaryOperator, ComparePredicate, Constant,
    Convergence, F32MathFunction, FloatOperation, Function, FunctionId, IndexKind, IntrinsicKind,
    IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, MemoryOrdering, Module,
    Operation, OperationKind, ScalarType, Signature, SwitchCase, SynchronizationScope, Terminator,
    Type, ValueDef, ValueId, WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier,
    WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize,
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

fn add(id: u32, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn binary(id: u32, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn cast(id: u32, kind: CastKind, value: u32, to: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), to.clone()),
        OperationKind::Cast {
            kind,
            value: ValueId(value),
            to,
        },
    )
}

fn checked_index(
    value: u32,
    overflow: u32,
    operator: CheckedBinaryOperator,
    lhs: u32,
    rhs: u32,
) -> Operation {
    Operation::checked_binary(
        ValueDef::new(ValueId(value), Type::INDEX),
        ValueDef::new(ValueId(overflow), Type::BOOL),
        operator,
        ValueId(lhs),
        ValueId(rhs),
    )
}

fn compare_with(id: u32, predicate: ComparePredicate, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::BOOL),
        OperationKind::Compare {
            predicate,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn zero_extend_to_index(id: u32, value: u32) -> Operation {
    cast(id, CastKind::ZeroExtend, value, Type::INDEX)
}

fn private_slot(id: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            ValueId(id),
            Type::pointer(Type::INDEX, AddressSpace::Private, AccessMode::ReadWrite),
        ),
        OperationKind::Alloca {
            element: Type::INDEX,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    )
}

fn private_store(pointer: u32, value: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        },
    )
}

fn private_load(id: u32, pointer: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Load {
            pointer: ValueId(pointer),
            access: MemoryAccess::new(AddressSpace::Private, 8),
        },
    )
}

fn analyze_as_kernel(function: &Function) -> fe2o3_kernel_analysis::AnalysisReport {
    let mut module = Module::new("uniformity_private_storage");
    module.functions.push(function.clone());
    analyze_kernel_entry(&module, function)
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

fn wave(id: u32, kind: WaveOperationKind) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        OperationKind::Wave(WaveOperation::full(kind, WaveWidth::Wave64)),
    )
}

fn returning(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

#[test]
fn bounded_lane_arithmetic_has_a_uniform_overflow_flag() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(64)),
        binary(2, BinaryOp::Remainder, 0, 1),
        constant(3, Constant::Index(16)),
        binary(4, BinaryOp::Divide, 2, 3),
        constant(5, Constant::Index(4)),
        checked_index(6, 7, CheckedBinaryOperator::Multiply, 4, 5),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut success = returning(2);
    success.operations.push(workgroup_barrier());

    let report = analyze_function(&function(vec![], vec![entry, returning(1), success]));

    assert_eq!(report.value(ValueId(6)), Variation::Varying);
    assert_eq!(report.value(ValueId(7)), Variation::GridUniform);
    assert_eq!(report.block_control(BlockId(2)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn unconstrained_lane_arithmetic_keeps_overflow_varying() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(4)),
        checked_index(2, 3, CheckedBinaryOperator::Multiply, 0, 1),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut success = returning(2);
    success.operations.push(workgroup_barrier());

    let report = analyze_function(&function(vec![], vec![entry, returning(1), success]));

    assert_eq!(report.value(ValueId(2)), Variation::Varying);
    assert_eq!(report.value(ValueId(3)), Variation::Varying);
    assert!(matches!(
        report.diagnostics(),
        [Diagnostic::DivergentBarrier {
            block: BlockId(2),
            ..
        }]
    ));
}

#[test]
fn dominating_u32_guards_bound_index_multiply_and_add() {
    let parameters = vec![ValueId(0), ValueId(1), ValueId(2)];
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        zero_extend_to_index(3, 0),
        zero_extend_to_index(4, 1),
        zero_extend_to_index(5, 2),
        intrinsic(
            6,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        intrinsic(
            7,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
        ),
        compare_with(8, ComparePredicate::LessThan, 6, 3),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(8),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut depth_guard = BasicBlock::new(BlockId(1));
    depth_guard
        .operations
        .push(compare_with(9, ComparePredicate::LessThan, 7, 5));
    depth_guard.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(9),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut arithmetic = returning(2);
    arithmetic.operations.extend([
        checked_index(10, 11, CheckedBinaryOperator::Multiply, 6, 4),
        checked_index(12, 13, CheckedBinaryOperator::Add, 10, 7),
    ]);
    let kernel = Function::definition(
        "guarded_index",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        parameters,
        vec![entry, depth_guard, arithmetic, returning(3)],
    );

    let report = analyze_as_kernel(&kernel);

    assert_eq!(report.value(ValueId(10)), Variation::Varying);
    assert_eq!(report.value(ValueId(11)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(12)), Variation::Varying);
    assert_eq!(report.value(ValueId(13)), Variation::GridUniform);
}

#[test]
fn shared_branch_target_does_not_authenticate_a_range_guard() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(16)),
        compare_with(2, ComparePredicate::LessThan, 0, 1),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut shared_target = returning(1);
    shared_target.operations.extend([
        constant(3, Constant::Index(2)),
        checked_index(4, 5, CheckedBinaryOperator::Multiply, 0, 3),
    ]);
    let mut false_path = BasicBlock::new(BlockId(2));
    false_path.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });

    let report = analyze_function(&function(vec![], vec![entry, shared_target, false_path]));

    assert_eq!(report.value(ValueId(5)), Variation::Varying);
}

#[test]
fn near_maximum_checked_addition_fails_closed() {
    let mut entry = returning(0);
    entry.operations.extend([
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(1)),
        checked_index(2, 3, CheckedBinaryOperator::Add, 0, 1),
    ]);

    let report = analyze_function(&function(vec![], vec![entry]));

    assert_eq!(report.value(ValueId(3)), Variation::Varying);
}

#[test]
fn exhaustive_known_enum_switch_does_not_create_a_synthetic_divergent_exit() {
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
        compare_with(2, ComparePredicate::NotEqual, 0, 1),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut zero = BasicBlock::new(BlockId(1));
    zero.operations.push(constant(3, Constant::I64(0)));
    zero.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(3)],
    });
    let mut one = BasicBlock::new(BlockId(2));
    one.operations.push(constant(4, Constant::I64(1)));
    one.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(4)],
    });
    let mut switch = BasicBlock::new(BlockId(3));
    switch
        .parameters
        .push(ValueDef::new(ValueId(5), Type::Scalar(ScalarType::I64)));
    switch.terminator = Some(Terminator::Switch {
        selector: ValueId(5),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(4),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(5),
                arguments: vec![],
            },
        ],
        default_target: BlockId(6),
        default_arguments: vec![],
    });
    let mut case_zero = BasicBlock::new(BlockId(4));
    case_zero.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let mut case_one = BasicBlock::new(BlockId(5));
    case_one.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let mut merge = returning(7);
    merge.operations.push(workgroup_barrier());

    let report = analyze_function(&function(
        vec![],
        vec![
            entry,
            zero,
            one,
            switch,
            case_zero,
            case_one,
            returning(6),
            merge,
        ],
    ));

    assert_eq!(report.value(ValueId(5)), Variation::Varying);
    assert_eq!(report.block_control(BlockId(7)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn unknown_switch_selector_keeps_the_default_exit_and_fails_closed() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(intrinsic(
        0,
        IntrinsicKind::InvocationIndex {
            kind: IndexKind::Global,
            axis: Axis::X,
        },
    ));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(1),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(2),
                arguments: vec![],
            },
        ],
        default_target: BlockId(3),
        default_arguments: vec![],
    });
    let mut case_zero = BasicBlock::new(BlockId(1));
    case_zero.terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![],
    });
    let mut case_one = BasicBlock::new(BlockId(2));
    case_one.terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![],
    });
    let mut merge = returning(4);
    merge.operations.push(workgroup_barrier());

    let report = analyze_function(&function(
        vec![],
        vec![entry, case_zero, case_one, returning(3), merge],
    ));

    assert_eq!(report.block_control(BlockId(4)), Variation::Varying);
    assert!(matches!(
        report.diagnostics(),
        [Diagnostic::DivergentBarrier {
            block: BlockId(4),
            ..
        }]
    ));
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
fn exact_global_index_workgroup_quotient_is_uniform_but_lane_remainder_is_not() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::U32(64)),
        cast(2, CastKind::ZeroExtend, 1, Type::INDEX),
        binary(3, BinaryOp::Divide, 0, 2),
        binary(4, BinaryOp::Remainder, 0, 2),
    ];
    let function = function(vec![], vec![entry]);
    let mut kernel = Kernel::new(
        "test_kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("uniformity_workgroup_quotient");
    module.functions.push(function.clone());
    module.kernels.push(kernel);

    let report = analyze_kernel_entry(&module, &function);
    assert_eq!(report.value(ValueId(3)), Variation::WorkgroupUniform);
    assert_eq!(report.value(ValueId(4)), Variation::Varying);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn workgroup_quotient_uniformity_is_axis_exact_and_requires_launch_contract() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        intrinsic(
            1,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Y,
            },
        ),
        intrinsic(
            2,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Z,
            },
        ),
        constant(3, Constant::Index(8)),
        constant(4, Constant::Index(4)),
        constant(5, Constant::Index(2)),
        constant(6, Constant::Index(3)),
        binary(10, BinaryOp::Divide, 0, 3),
        binary(11, BinaryOp::Divide, 1, 4),
        binary(12, BinaryOp::Divide, 2, 5),
        binary(13, BinaryOp::Divide, 0, 6),
        binary(14, BinaryOp::Divide, 1, 3),
        binary(15, BinaryOp::Divide, 2, 4),
        binary(16, BinaryOp::Remainder, 0, 3),
    ];
    let function = function(vec![], vec![entry]);
    let mut kernel = Kernel::new(
        "test_kernel",
        function.id.clone(),
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(8, 4, 2));
    let mut module = Module::new("uniformity_workgroup_quotient_axes");
    module.functions.push(function.clone());
    module.kernels.push(kernel);

    let report = analyze_kernel_entry(&module, &function);
    for value in 10..=12 {
        assert_eq!(report.value(ValueId(value)), Variation::WorkgroupUniform);
    }
    for value in 13..=16 {
        assert_eq!(report.value(ValueId(value)), Variation::Varying);
    }

    module.kernels.clear();
    let without_contract = analyze_kernel_entry(&module, &function);
    for value in 10..=12 {
        assert_eq!(without_contract.value(ValueId(value)), Variation::Varying);
    }
}

#[test]
fn workgroup_quotient_rejects_non_value_preserving_constant_casts() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::U64(300)),
        cast(2, CastKind::Truncate, 1, Type::Scalar(ScalarType::U8)),
        cast(3, CastKind::ZeroExtend, 2, Type::INDEX),
        binary(4, BinaryOp::Divide, 0, 3),
        constant(5, Constant::U64(256)),
        cast(6, CastKind::Truncate, 5, Type::Scalar(ScalarType::U8)),
        cast(7, CastKind::ZeroExtend, 6, Type::INDEX),
        binary(8, BinaryOp::Divide, 0, 7),
        constant(9, Constant::I32(64)),
        cast(10, CastKind::SignExtend, 9, Type::INDEX),
        binary(11, BinaryOp::Divide, 0, 10),
        constant(12, Constant::U64(64)),
        cast(13, CastKind::Bitcast, 12, Type::INDEX),
        binary(14, BinaryOp::Divide, 0, 13),
    ];
    let function = function(vec![], vec![entry]);
    let mut kernel = Kernel::new(
        "test_kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("uniformity_hostile_constant_casts");
    module.functions.push(function.clone());
    module.kernels.push(kernel);

    let report = analyze_kernel_entry(&module, &function);
    for value in [4, 8, 11, 14] {
        assert_eq!(report.value(ValueId(value)), Variation::Varying);
    }
}

#[test]
fn workgroup_quotient_accepts_only_value_preserving_unsigned_cast_chains() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::U8(64)),
        cast(2, CastKind::ZeroExtend, 1, Type::Scalar(ScalarType::U16)),
        cast(3, CastKind::ZeroExtend, 2, Type::Scalar(ScalarType::U32)),
        cast(4, CastKind::ZeroExtend, 3, Type::INDEX),
        binary(5, BinaryOp::Divide, 0, 4),
    ];
    let function = function(vec![], vec![entry]);
    let mut kernel = Kernel::new(
        "test_kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("uniformity_value_preserving_constant_casts");
    module.functions.push(function.clone());
    module.kernels.push(kernel);

    assert_eq!(
        analyze_kernel_entry(&module, &function).value(ValueId(5)),
        Variation::WorkgroupUniform,
    );
}

#[test]
fn workgroup_contract_selection_rejects_conflicting_duplicate_entries() {
    let mut entry = returning(0);
    entry.operations = vec![
        intrinsic(
            0,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::X,
            },
        ),
        constant(1, Constant::Index(64)),
        binary(2, BinaryOp::Divide, 0, 1),
    ];
    let function = function(vec![], vec![entry]);
    let kernel = |name: &str, width| {
        let mut kernel = Kernel::new(
            name,
            function.id.clone(),
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(width, 1, 1));
        kernel
    };

    for widths in [[32, 64], [64, 32]] {
        let mut module = Module::new("uniformity_conflicting_duplicate_entries");
        module.functions.push(function.clone());
        module.kernels.push(kernel("first", widths[0]));
        module.kernels.push(kernel("second", widths[1]));
        assert_eq!(
            analyze_kernel_entry(&module, &function).value(ValueId(2)),
            Variation::Varying,
        );
    }

    let mut identical = Module::new("uniformity_identical_duplicate_entries");
    identical.functions.push(function.clone());
    identical.kernels.push(kernel("first", 64));
    identical.kernels.push(kernel("second", 64));
    assert_eq!(
        analyze_kernel_entry(&identical, &function).value(ValueId(2)),
        Variation::WorkgroupUniform,
    );
}

#[test]
fn wave_rules_preserve_uniform_inputs_and_collapse_full_wave_collectives() {
    let mut entry = returning(0);
    entry.operations = vec![
        constant(2, Constant::Bool(true)),
        intrinsic(
            3,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Workgroup,
                axis: Axis::X,
            },
        ),
        constant(4, Constant::Index(0)),
        compare(5, 3, 4),
        wave(6, WaveOperationKind::LaneId),
        wave(
            7,
            WaveOperationKind::Ballot {
                predicate: ValueId(0),
            },
        ),
        wave(
            8,
            WaveOperationKind::Any {
                predicate: ValueId(2),
            },
        ),
        wave(
            9,
            WaveOperationKind::All {
                predicate: ValueId(5),
            },
        ),
        wave(
            10,
            WaveOperationKind::ShuffleIndex {
                value: ValueId(0),
                source_lane: ValueId(4),
                tile_width: 64,
            },
        ),
        wave(
            11,
            WaveOperationKind::ShuffleIndex {
                value: ValueId(4),
                source_lane: ValueId(1),
                tile_width: 64,
            },
        ),
        wave(
            12,
            WaveOperationKind::ShuffleIndex {
                value: ValueId(0),
                source_lane: ValueId(1),
                tile_width: 64,
            },
        ),
    ];

    let report = analyze_function(&function(vec![ValueId(0), ValueId(1)], vec![entry]));

    assert_eq!(report.value(ValueId(6)), Variation::Varying);
    assert_eq!(report.value(ValueId(7)), Variation::SubgroupUniform);
    assert_eq!(report.value(ValueId(8)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(9)), Variation::WorkgroupUniform);
    assert_eq!(report.value(ValueId(10)), Variation::SubgroupUniform);
    assert_eq!(report.value(ValueId(11)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(12)), Variation::Varying);
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
fn dominated_private_slot_round_trip_preserves_kernel_parameter_uniformity() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        private_slot(1),
        private_store(1, 0),
        private_load(2, 1),
        constant(3, Constant::Index(0)),
        compare(4, 2, 3),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(workgroup_barrier());
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let kernel = function(vec![ValueId(0)], vec![entry, then_block, returning(2)]);

    let report = analyze_as_kernel(&kernel);

    assert_eq!(report.value(ValueId(2)), Variation::GridUniform);
    assert_eq!(report.block_control(BlockId(1)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn private_slot_round_trip_does_not_make_lane_values_uniform() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        private_slot(0),
        intrinsic(
            1,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
        ),
        private_store(0, 1),
        private_load(2, 0),
        constant(3, Constant::Index(0)),
        compare(4, 2, 3),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(workgroup_barrier());
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let kernel = function(vec![], vec![entry, then_block, returning(2)]);

    let report = analyze_as_kernel(&kernel);

    assert_eq!(report.value(ValueId(2)), Variation::Varying);
    assert!(matches!(
        report.diagnostics(),
        [Diagnostic::DivergentBarrier {
            block: BlockId(1),
            ..
        }]
    ));
}

#[test]
fn private_load_without_a_dominating_store_fails_closed() {
    let mut entry = returning(0);
    entry.operations.extend([
        private_slot(0),
        constant(1, Constant::Index(7)),
        private_load(2, 0),
        private_store(0, 1),
    ]);
    let kernel = function(vec![], vec![entry]);

    let report = analyze_as_kernel(&kernel);

    assert_eq!(report.value(ValueId(2)), Variation::Varying);
}

#[test]
fn escaped_private_slot_load_fails_closed() {
    let pointer_type = Type::pointer(Type::INDEX, AddressSpace::Private, AccessMode::ReadWrite);
    let mut entry = returning(0);
    entry.operations.extend([
        private_slot(0),
        constant(1, Constant::Index(7)),
        Operation::effect_free(
            ValueDef::new(ValueId(2), pointer_type.clone()),
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::Bitcast,
                value: ValueId(0),
                to: pointer_type,
            },
        ),
        private_store(0, 1),
        private_load(3, 0),
    ]);
    let kernel = function(vec![], vec![entry]);

    let report = analyze_as_kernel(&kernel);

    assert_eq!(report.value(ValueId(3)), Variation::Varying);
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
fn divergent_loop_exit_does_not_manufacture_reconvergence() {
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
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });

    let mut header = BasicBlock::new(BlockId(1));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(2));
    body.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut exit = returning(3);
    exit.operations.push(workgroup_barrier());

    let report = analyze_function(&function(vec![], vec![entry, header, body, exit]));

    assert_eq!(report.block_control(BlockId(3)), Variation::Varying);
    assert!(matches!(
        report.diagnostics(),
        [Diagnostic::DivergentBarrier {
            block: BlockId(3),
            ..
        }]
    ));
}

#[test]
fn divergent_branch_can_reconverge_within_the_same_natural_loop() {
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
        constant(3, Constant::Bool(true)),
    ]);
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });

    let mut header = BasicBlock::new(BlockId(1));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(6),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(2));
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(3));
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![],
    });
    let mut else_block = BasicBlock::new(BlockId(4));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(5));
    merge.operations.push(workgroup_barrier());
    merge.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });

    let report = analyze_function(&function(
        vec![],
        vec![
            entry,
            header,
            body,
            then_block,
            else_block,
            merge,
            returning(6),
        ],
    ));

    assert_eq!(report.block_control(BlockId(5)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
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
fn reconverged_loop_preserves_trivial_phi_uniformity() {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        constant(0, Constant::Index(0)),
        constant(1, Constant::Index(3)),
        constant(2, Constant::Index(1)),
        intrinsic(
            3,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
        ),
        compare(4, 3, 0),
    ]);
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0)],
    });

    let mut header = BasicBlock::new(BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(10), Type::INDEX));
    header.operations.push(compare(11, 10, 1));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(2),
        then_arguments: vec![ValueId(10)],
        else_target: BlockId(6),
        else_arguments: vec![],
    });

    let mut body = BasicBlock::new(BlockId(2));
    body.parameters
        .push(ValueDef::new(ValueId(12), Type::INDEX));
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(3),
        then_arguments: vec![ValueId(12)],
        else_target: BlockId(4),
        else_arguments: vec![ValueId(12)],
    });

    let mut then_block = BasicBlock::new(BlockId(3));
    then_block
        .parameters
        .push(ValueDef::new(ValueId(13), Type::INDEX));
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![ValueId(13)],
    });

    let mut else_block = BasicBlock::new(BlockId(4));
    else_block
        .parameters
        .push(ValueDef::new(ValueId(14), Type::INDEX));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(5),
        arguments: vec![ValueId(14)],
    });

    let mut merge = BasicBlock::new(BlockId(5));
    merge
        .parameters
        .push(ValueDef::new(ValueId(15), Type::INDEX));
    merge
        .operations
        .extend([workgroup_barrier(), add(16, 15, 2)]);
    merge.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(16)],
    });

    let report = analyze_function(&function(
        vec![],
        vec![
            entry,
            header,
            body,
            then_block,
            else_block,
            merge,
            returning(6),
        ],
    ));

    assert_eq!(report.value(ValueId(10)), Variation::GridUniform);
    assert_eq!(report.value(ValueId(15)), Variation::GridUniform);
    assert_eq!(report.block_control(BlockId(5)), Variation::GridUniform);
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
fn closed_float_intrinsics_and_pure_math_helpers_are_summarized() {
    let exp = FloatOperation::F32Math {
        function: F32MathFunction::Exp,
        implementation: F32MathFunction::Exp.required_implementation(),
        arguments: vec![ValueId(10)],
    };
    let mut helper_block = returning(1);
    helper_block.operations.push(exp.operation(ValueId(11)));
    helper_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(11)],
    });
    let helper = Function::definition(
        "math_helper",
        Signature::new(vec![Type::F32], vec![Type::F32]),
        vec![ValueId(10)],
        vec![helper_block],
    );

    let mut entry = returning(0);
    entry.operations = vec![
        constant(0, Constant::F32Bits(1.0_f32.to_bits())),
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::F32),
            OperationKind::Call {
                callee: helper.id.clone(),
                arguments: vec![ValueId(0)],
            },
        ),
    ];
    let kernel = function(vec![], vec![entry]);
    let mut module = Module::new("uniformity_math_helper");
    module.functions = vec![kernel.clone(), helper, exp.declaration()];

    let report = analyze_kernel_entry(&module, &kernel);
    assert_eq!(report.value(ValueId(1)), Variation::GridUniform);
    assert!(report.diagnostics().is_empty());
}

#[test]
fn known_diagnostic_call_is_accepted_but_remains_lane_local() {
    let mut entry = returning(0);
    entry
        .operations
        .push(AmdGpuDiagnosticOperation::Clock32.operation(Some(ValueId(0))));
    let kernel = function(vec![], vec![entry]);

    let report = analyze_function(&kernel);
    assert_eq!(report.value(ValueId(0)), Variation::Varying);
    assert!(report.diagnostics().is_empty());
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
