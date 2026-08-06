use fe2o3_kernel_ir::*;

fn global_slice(access: AccessMode) -> Type {
    Type::slice(Type::F32, AddressSpace::Global, access)
}

fn global_pointer(access: AccessMode) -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, access)
}

fn op(result: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(result), ty), kind)
}

fn module_with_kernel(
    parameters: Vec<Type>,
    operations: Vec<Operation>,
    domain: LaunchDomain,
) -> Module {
    let parameter_values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::kernel_entry(
        "kernel_impl",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    );
    let mut module = Module::new("formal-memory-test");
    module.functions.push(function);
    module
        .kernels
        .push(Kernel::new("kernel", "kernel_impl", domain));
    module
}

fn dynamic_1d() -> LaunchDomain {
    LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    }
}

fn analyze(module: &Module, extent: u64) -> FormalMemoryObligationAnalysis {
    derive_kernel_memory_obligations(
        module,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(extent),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

fn fill_module(index: OperationKind) -> Module {
    let pointer = global_pointer(AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    module_with_kernel(
        vec![global_slice(AccessMode::ReadWrite), Type::F32],
        vec![
            op(2, Type::INDEX, index),
            op(
                3,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            op(
                4,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(2),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(4),
                    value: ValueId(1),
                    access,
                },
            ),
        ],
        dynamic_1d(),
    )
}

fn exact_fill_module() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });

    let mut body = BasicBlock::new(BlockId(1));
    body.operations = vec![
        op(
            5,
            global_pointer(AccessMode::ReadWrite),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            6,
            global_pointer(AccessMode::ReadWrite),
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(6),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    body.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });

    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![global_slice(AccessMode::ReadWrite), Type::F32], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, body, exit],
    );
    let mut module = Module::new("exact-fill");
    module.functions.push(function);
    module
        .kernels
        .push(Kernel::new("kernel", "kernel_impl", dynamic_1d()));
    module
}

fn vecadd_module() -> Module {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(4, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            5,
            Type::INDEX,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ),
        op(
            6,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(2) },
        ),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(3),
                rhs: ValueId(6),
            },
        ),
        op(
            8,
            global_pointer(AccessMode::ReadWrite),
            OperationKind::SliceData { slice: ValueId(2) },
        ),
        op(
            9,
            global_pointer(AccessMode::ReadWrite),
            OperationKind::GetElementPointer {
                base: ValueId(8),
                offset: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(4),
        else_arguments: vec![],
    });

    let mut first_bounds = BasicBlock::new(BlockId(1));
    first_bounds.operations = vec![
        op(
            10,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            11,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: ValueId(10),
            },
        ),
    ];
    first_bounds.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(5),
        else_arguments: vec![],
    });

    let mut second_bounds = BasicBlock::new(BlockId(2));
    second_bounds.operations = vec![
        op(
            12,
            global_pointer(AccessMode::ReadOnly),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            13,
            global_pointer(AccessMode::ReadOnly),
            OperationKind::GetElementPointer {
                base: ValueId(12),
                offset: ValueId(5),
            },
        ),
        op(
            14,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(13),
                access,
            },
        ),
        op(
            15,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(1) },
        ),
        op(
            16,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(5),
                rhs: ValueId(15),
            },
        ),
    ];
    second_bounds.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(16),
        then_target: BlockId(3),
        then_arguments: vec![],
        else_target: BlockId(5),
        else_arguments: vec![],
    });

    let mut compute = BasicBlock::new(BlockId(3));
    compute.operations = vec![
        op(
            17,
            global_pointer(AccessMode::ReadOnly),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        op(
            18,
            global_pointer(AccessMode::ReadOnly),
            OperationKind::GetElementPointer {
                base: ValueId(17),
                offset: ValueId(5),
            },
        ),
        op(
            19,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(18),
                access,
            },
        ),
        op(
            20,
            Type::F32,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(14),
                rhs: ValueId(19),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(9),
                value: ValueId(20),
                access,
            },
        ),
    ];
    compute.terminator = Some(Terminator::Branch {
        target: BlockId(4),
        arguments: vec![],
    });

    let mut exit = BasicBlock::new(BlockId(4));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut trap = BasicBlock::new(BlockId(5));
    trap.terminator = Some(Terminator::Unreachable);
    let function = Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                global_slice(AccessMode::ReadOnly),
                global_slice(AccessMode::ReadOnly),
                global_slice(AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, first_bounds, second_bounds, compute, exit, trap],
    );
    let mut module = Module::new("exact-vecadd");
    module.functions.push(function);
    module
        .kernels
        .push(Kernel::new("kernel", "kernel_impl", dynamic_1d()));
    module
}

#[test]
fn derives_complete_formal_fill_obligations() {
    let module = exact_fill_module();
    let analysis = analyze(&module, 64);
    assert!(analysis.is_complete());
    let obligations = analysis.obligations();

    assert_eq!(
        obligations.analysis_basis(),
        FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
    );
    assert_eq!(obligations.invocations().unwrap().start(), 0);
    assert_eq!(obligations.invocations().unwrap().end_exclusive(), 64);
    assert_eq!(obligations.allocations().len(), 1);
    assert_eq!(obligations.allocations()[0].identity().parameter_index(), 0);
    assert_eq!(
        obligations.allocations()[0].kind(),
        FormalParameterKind::Slice
    );
    assert_eq!(obligations.accesses().len(), 1);
    assert_eq!(
        obligations.accesses()[0].byte_offset(),
        ByteExpression::invocation_affine(0, 4)
    );
    assert_eq!(obligations.accesses()[0].invocations().end_exclusive(), 64);
    assert_eq!(obligations.bounds_requirements()[0].minimum_byte_len(), 256);
    assert!(obligations.runtime_alias_requirements().is_empty());
    assert!(obligations.inter_invocation_conflicts().is_empty());
}

#[test]
fn analysis_basis_discloses_caller_supplied_launch_inputs() {
    let module = exact_fill_module();
    let first = analyze(&module, 8);
    let second = analyze(&module, 17);

    for analysis in [&first, &second] {
        assert_eq!(
            analysis.obligations().analysis_basis(),
            FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
        );
        assert_eq!(
            analysis.obligations().index_width(),
            FormalIndexWidth::Bits64
        );
    }
    assert_eq!(
        first.obligations().invocations().unwrap().end_exclusive(),
        8
    );
    assert_eq!(
        second.obligations().invocations().unwrap().end_exclusive(),
        17
    );
}

#[test]
fn vecadd_derives_output_alias_requirements_but_allows_input_aliasing() {
    let analysis = analyze(&vecadd_module(), 256);
    assert!(analysis.is_complete());
    let obligations = analysis.obligations();

    assert_eq!(obligations.allocations().len(), 3);
    assert_eq!(obligations.accesses().len(), 3);
    assert_eq!(obligations.bounds_requirements().len(), 3);
    assert!(
        obligations
            .bounds_requirements()
            .iter()
            .all(|requirement| requirement.minimum_byte_len() == 1024)
    );
    let alias_pairs: Vec<_> = obligations
        .runtime_alias_requirements()
        .iter()
        .map(|requirement| {
            (
                requirement.left().parameter_index(),
                requirement.right().parameter_index(),
            )
        })
        .collect();
    assert_eq!(alias_pairs, vec![(0, 2), (1, 2)]);
    assert!(obligations.inter_invocation_conflicts().is_empty());
}

#[test]
fn two_writable_formal_parameters_require_runtime_alias_discharge() {
    let pointer = global_pointer(AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let module = module_with_kernel(
        vec![
            global_slice(AccessMode::ReadWrite),
            global_slice(AccessMode::ReadWrite),
            Type::F32,
        ],
        vec![
            op(
                3,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
            op(
                4,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            op(
                5,
                pointer.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(4),
                    offset: ValueId(3),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(5),
                    value: ValueId(2),
                    access,
                },
            ),
            op(
                6,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(1) },
            ),
            op(
                7,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(6),
                    offset: ValueId(3),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(7),
                    value: ValueId(2),
                    access,
                },
            ),
        ],
        dynamic_1d(),
    );

    let analysis = analyze(&module, 32);
    assert!(analysis.is_complete());
    assert_eq!(analysis.obligations().runtime_alias_requirements().len(), 1);
}

#[test]
fn shifted_formal_ranges_still_require_runtime_alias_discharge() {
    let pointer = global_pointer(AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let module = module_with_kernel(
        vec![pointer.clone(), pointer.clone(), Type::F32],
        vec![
            op(3, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
            op(
                4,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(100)),
            ),
            op(
                5,
                pointer.clone(),
                OperationKind::GetElementPointer {
                    base: ValueId(0),
                    offset: ValueId(3),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(5),
                    value: ValueId(2),
                    access,
                },
            ),
            op(
                6,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(1),
                    offset: ValueId(4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(6),
                    value: ValueId(2),
                    access,
                },
            ),
        ],
        dynamic_1d(),
    );

    let analysis = analyze(&module, 1);
    let requirement = analysis.obligations().runtime_alias_requirements()[0];
    assert!(analysis.is_complete());
    assert_eq!(requirement.left_accessed_bytes().start(), 0);
    assert_eq!(requirement.left_accessed_bytes().end_exclusive(), 4);
    assert_eq!(requirement.right_accessed_bytes().start(), 400);
    assert_eq!(requirement.right_accessed_bytes().end_exclusive(), 404);
}

fn address_space_pair_module(left: AddressSpace, right: AddressSpace) -> Module {
    let parameter = |space| {
        Type::pointer(
            Type::F32,
            space,
            if space == AddressSpace::Constant {
                AccessMode::ReadOnly
            } else {
                AccessMode::ReadWrite
            },
        )
    };
    let access = |space| MemoryAccess::new(space, 4);
    let memory_operation = |pointer, space, write, result| {
        if write {
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(pointer),
                    value: ValueId(2),
                    access: access(space),
                },
            )
        } else {
            op(
                result,
                Type::F32,
                OperationKind::Load {
                    pointer: ValueId(pointer),
                    access: access(space),
                },
            )
        }
    };
    let left_writes = left != AddressSpace::Constant;
    let right_writes = !left_writes && right != AddressSpace::Constant;
    module_with_kernel(
        vec![parameter(left), parameter(right), Type::F32],
        vec![
            memory_operation(0, left, left_writes, 3),
            memory_operation(1, right, right_writes, 4),
        ],
        dynamic_1d(),
    )
}

#[test]
fn address_space_alias_requirements_follow_the_explicit_matrix() {
    let spaces = [
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Global,
        AddressSpace::Constant,
        AddressSpace::Generic,
    ];
    let compatible = [
        [true, false, false, false, true],
        [false, true, false, false, true],
        [false, false, true, true, true],
        [false, false, true, true, true],
        [true, true, true, true, true],
    ];

    for (left_index, left) in spaces.into_iter().enumerate() {
        for (right_index, right) in spaces.into_iter().enumerate() {
            let analysis = analyze(&address_space_pair_module(left, right), 1);
            let has_write = left != AddressSpace::Constant || right != AddressSpace::Constant;
            let expected = usize::from(compatible[left_index][right_index] && has_write);
            assert!(analysis.is_complete(), "{left:?} with {right:?}");
            assert_eq!(
                analysis.obligations().runtime_alias_requirements().len(),
                expected,
                "{left:?} with {right:?}",
            );
        }
    }
}

#[test]
fn bounds_requirement_exposes_an_out_of_bounds_runtime_extent() {
    let module = exact_fill_module();
    let analysis = analyze(&module, 65);
    let requirement = analysis.obligations().bounds_requirements()[0];

    assert!(analysis.is_complete());
    assert_eq!(requirement.minimum_byte_len(), 260);
    assert!(!requirement.is_met_by_untrusted_byte_len(256));
    assert!(requirement.is_met_by_untrusted_byte_len(260));
}

#[test]
fn constant_index_store_is_reported_as_an_inter_invocation_conflict() {
    let module = fill_module(OperationKind::Constant(Constant::Index(0)));
    let analysis = analyze(&module, 8);

    assert!(analysis.is_complete());
    assert_eq!(analysis.obligations().inter_invocation_conflicts().len(), 1);
}

#[test]
fn address_overflow_fails_closed() {
    let module = fill_module(OperationKind::Constant(Constant::Index(u64::MAX)));
    let analysis = analyze(&module, 2);

    assert!(!analysis.is_complete());
    assert!(matches!(
        analysis.incomplete_reasons(),
        [FormalMemoryIncompleteReason::AddressArithmeticOverflow { .. }]
    ));
    assert!(analysis.obligations().accesses().is_empty());
}

fn fixed_width_index_module(lhs: Constant, rhs: Constant, binary_op: BinaryOp) -> Module {
    let result_type = lhs.ty();
    assert_eq!(rhs.ty(), result_type);
    let pointer = global_pointer(AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    module_with_kernel(
        vec![global_slice(AccessMode::ReadWrite), Type::F32],
        vec![
            op(2, result_type.clone(), OperationKind::Constant(lhs)),
            op(3, result_type.clone(), OperationKind::Constant(rhs)),
            op(
                4,
                result_type,
                OperationKind::Binary {
                    op: binary_op,
                    lhs: ValueId(2),
                    rhs: ValueId(3),
                },
            ),
            op(
                5,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            op(
                6,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(5),
                    offset: ValueId(4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(6),
                    value: ValueId(1),
                    access,
                },
            ),
        ],
        dynamic_1d(),
    )
}

#[test]
fn fixed_width_wrapping_arithmetic_fails_closed() {
    let cases = [
        fixed_width_index_module(Constant::U8(255), Constant::U8(1), BinaryOp::Add),
        fixed_width_index_module(Constant::I8(127), Constant::I8(1), BinaryOp::Add),
        fixed_width_index_module(Constant::U8(255), Constant::U8(2), BinaryOp::Multiply),
    ];

    for module in cases {
        let analysis = analyze(&module, 1);
        assert!(matches!(
            analysis.incomplete_reasons(),
            [FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                index: ValueId(4),
                ..
            }]
        ));
        assert!(analysis.obligations().accesses().is_empty());
    }
}

#[test]
fn nonlinear_gep_index_fails_closed() {
    let pointer = global_pointer(AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let module = module_with_kernel(
        vec![global_slice(AccessMode::ReadWrite), Type::F32],
        vec![
            op(
                2,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
            op(
                3,
                Type::INDEX,
                OperationKind::Binary {
                    op: BinaryOp::Multiply,
                    lhs: ValueId(2),
                    rhs: ValueId(2),
                },
            ),
            op(
                4,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            op(
                5,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(4),
                    offset: ValueId(3),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(5),
                    value: ValueId(1),
                    access,
                },
            ),
        ],
        dynamic_1d(),
    );
    let analysis = analyze(&module, 8);

    assert!(matches!(
        analysis.incomplete_reasons(),
        [FormalMemoryIncompleteReason::UnsupportedIndexExpression {
            index: ValueId(3),
            ..
        }]
    ));
}

#[test]
fn pointer_not_rooted_in_a_kernel_parameter_fails_closed() {
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let private_pointer = Type::pointer(Type::F32, AddressSpace::Private, AccessMode::ReadWrite);
    let module = module_with_kernel(
        vec![Type::F32],
        vec![
            op(
                1,
                private_pointer,
                OperationKind::Alloca {
                    element: Type::F32,
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(0),
                    access,
                },
            ),
        ],
        dynamic_1d(),
    );
    let analysis = analyze(&module, 8);

    assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
        reason,
        FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
    )));
    assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
        reason,
        FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            pointer: ValueId(1),
            ..
        }
    )));
}

#[test]
fn unmodeled_barrier_effect_fails_closed() {
    let barrier = Barrier {
        execution_scope: SynchronizationScope::Workgroup,
        memory_scope: SynchronizationScope::Workgroup,
        semantics: BarrierSemantics::new(MemoryOrdering::AcquireRelease, [AddressSpace::Workgroup]),
    };
    let module = module_with_kernel(
        vec![],
        vec![Operation::new(vec![], OperationKind::Barrier(barrier))],
        dynamic_1d(),
    );
    let analysis = analyze(&module, 8);

    assert!(matches!(
        analysis.incomplete_reasons(),
        [FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }]
    ));
}

#[test]
fn unsupported_atomic_effect_fails_closed() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let atomic = Atomic {
        kind: AtomicKind::Add,
        pointer: ValueId(0),
        value: Some(ValueId(1)),
        compare: None,
        access,
        scope: SynchronizationScope::Device,
        ordering: MemoryOrdering::Relaxed,
        failure_ordering: None,
    };
    let module = module_with_kernel(
        vec![global_pointer(AccessMode::ReadWrite), Type::F32],
        vec![Operation::new(
            vec![ValueDef::new(ValueId(2), Type::F32)],
            OperationKind::Atomic(atomic),
        )],
        dynamic_1d(),
    );
    let result = derive_kernel_memory_obligations(
        &module,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(8),
        FormalIndexWidth::Bits64,
    );

    let analysis = result.unwrap();
    assert!(matches!(
        analysis.incomplete_reasons(),
        [FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }]
    ));
}

#[test]
fn calls_and_unknown_launches_fail_closed() {
    let call = Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![],
        },
    );
    let mut module = module_with_kernel(vec![], vec![call], dynamic_1d());
    module.functions.push(Function::declaration(
        "helper",
        Signature::new(vec![], vec![]),
    ));

    let call_analysis = analyze(&module, 1);
    assert!(matches!(
        call_analysis.incomplete_reasons(),
        [FormalMemoryIncompleteReason::CallEffectsUnavailable { .. }]
    ));

    let unknown_analysis = derive_kernel_memory_obligations(
        &module,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Unknown,
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(
        unknown_analysis
            .incomplete_reasons()
            .contains(&FormalMemoryIncompleteReason::LaunchExtentUnknown)
    );
    assert_eq!(unknown_analysis.obligations().invocations(), None);
}

#[test]
fn unknown_and_32_bit_index_widths_never_complete() {
    let module = exact_fill_module();
    for width in [FormalIndexWidth::Unknown, FormalIndexWidth::Bits32] {
        let analysis = derive_kernel_memory_obligations(
            &module,
            &KernelId::new("kernel"),
            ExplicitLaunchExtent1d::Exact(64),
            width,
        )
        .unwrap();

        assert!(!analysis.is_complete());
        assert_eq!(analysis.obligations().index_width(), width);
        assert_eq!(
            analysis.incomplete_reasons(),
            &[FormalMemoryIncompleteReason::UnsupportedIndexWidth { width }]
        );
        assert!(analysis.obligations().accesses().is_empty());
        assert!(analysis.obligations().bounds_requirements().is_empty());
        assert!(
            analysis
                .obligations()
                .runtime_alias_requirements()
                .is_empty()
        );
    }
}

#[test]
fn unsupported_rank_and_static_extent_mismatch_fail_closed() {
    let rank_two = module_with_kernel(
        vec![],
        vec![],
        LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
        },
    );
    assert_eq!(
        analyze(&rank_two, 4).incomplete_reasons(),
        &[FormalMemoryIncompleteReason::LaunchRankUnsupported { rank: 2 }]
    );

    let static_domain = module_with_kernel(
        vec![],
        vec![],
        LaunchDomain::D1 {
            x: LaunchExtent::Static(8),
        },
    );
    assert_eq!(
        analyze(&static_domain, 4).incomplete_reasons(),
        &[FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
            expected: 8,
            actual: 4,
        }]
    );
}

#[test]
fn malformed_modules_and_missing_kernels_are_typed_errors() {
    let mut malformed = vecadd_module();
    malformed.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
    assert!(matches!(
        derive_kernel_memory_obligations(
            &malformed,
            &KernelId::new("kernel"),
            ExplicitLaunchExtent1d::Exact(1),
            FormalIndexWidth::Bits64,
        ),
        Err(FormalMemoryObligationError::InvalidModule(_))
    ));

    let valid = vecadd_module();
    assert_eq!(
        derive_kernel_memory_obligations(
            &valid,
            &KernelId::new("missing"),
            ExplicitLaunchExtent1d::Exact(1),
            FormalIndexWidth::Bits64,
        ),
        Err(FormalMemoryObligationError::MissingKernel {
            kernel: KernelId::new("missing"),
        })
    );
}

#[test]
fn formal_extraction_is_deterministic() {
    let module = vecadd_module();
    let first = analyze(&module, 257);
    for _ in 0..32 {
        assert_eq!(analyze(&module, 257), first);
    }
}
