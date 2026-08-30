use std::collections::BTreeSet;

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

fn valid_vecadd_module() -> Module {
    let read_slice = global_slice(AccessMode::ReadOnly);
    let write_slice = global_slice(AccessMode::ReadWrite);
    let read_pointer = global_pointer(AccessMode::ReadOnly);
    let write_pointer = global_pointer(AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Global, 4);

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            3,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            4,
            read_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            5,
            read_pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(3),
            },
        ),
        op(
            6,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(5),
                access,
            },
        ),
        op(
            7,
            read_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        op(
            8,
            read_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(7),
                offset: ValueId(3),
            },
        ),
        op(
            9,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(8),
                access,
            },
        ),
        op(
            10,
            Type::F32,
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(6),
                rhs: ValueId(9),
            },
        ),
        op(
            11,
            write_pointer.clone(),
            OperationKind::SliceData { slice: ValueId(2) },
        ),
        op(
            12,
            write_pointer,
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(10),
                access,
            },
        ),
    ];
    entry.terminator = Some(Terminator::Return { values: vec![] });

    let function = Function::kernel_entry(
        "vecadd_impl",
        Signature::new(vec![read_slice.clone(), read_slice, write_slice], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry],
    );
    let kernel = Kernel::new(
        "vecadd",
        "vecadd_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );

    let mut module = Module::new("tests::vecadd");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn one_block_module(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
    let parameter_values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "test",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    );
    let mut module = Module::new("tests::invalid");
    module.functions.push(function);
    module
}

#[test]
fn verifies_a_typed_ssa_kernel() {
    let module = valid_vecadd_module();
    verify_module(&module).expect("valid module should verify");

    let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    let effects: BTreeSet<_> = operations
        .iter()
        .flat_map(Operation::memory_effects)
        .collect();
    assert!(effects.contains(&MemoryEffect::Read(AddressSpace::Global)));
    assert!(effects.contains(&MemoryEffect::Write(AddressSpace::Global)));
    assert!(operations[3].effect_summary().reads(AddressSpace::Global));
    assert!(operations[10].effect_summary().writes(AddressSpace::Global));
}

#[test]
fn verifies_typed_1d_intrinsics_and_derives_baseline_metadata() {
    let module = one_block_module(
        vec![],
        vec![
            op(
                0,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
            op(
                1,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d()),
            ),
        ],
    );

    assert!(module.derived_capabilities().is_empty());
    verify_module_with_capabilities(&module, &BTreeSet::new())
        .expect("core launch queries require no optional target capability");

    let operations = &module.functions[0].body.as_ref().unwrap().blocks[0].operations;
    for operation in operations {
        assert!(operation.effect_summary().is_pure());
        assert!(operation.memory_effects().is_empty());
    }
}

#[test]
fn rejects_malformed_intrinsic_types_dimensions_and_arity() {
    let operations = vec![
        op(
            0,
            Type::F32,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::Y,
                },
                Type::F32,
            )),
        ),
        Operation::new(
            vec![],
            OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d()),
        ),
    ];

    let mut module = one_block_module(vec![], operations);
    module.kernels.push(Kernel::new(
        "test_kernel",
        "test",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));

    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidLaunchDomain));
    assert!(errors.contains(DiagnosticCode::TypeMismatch));
    assert!(errors.contains(DiagnosticCode::ResultArity));
}

#[test]
fn rejects_out_of_domain_intrinsics_in_recursive_kernel_helpers() {
    let mut entry_block = BasicBlock::new(BlockId(0));
    entry_block.operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![],
        },
    ));
    entry_block.terminator = Some(Terminator::Return { values: vec![] });

    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations = vec![
        op(
            0,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::Y,
                },
                Type::INDEX,
            )),
        ),
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("helper"),
                arguments: vec![],
            },
        ),
    ];
    helper_block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("tests::helper_intrinsic");
    module.functions = vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        ),
        Function::definition(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper_block],
        ),
    ];
    module.kernels.push(Kernel::new(
        "test_kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));

    let errors = verify_module(&module).unwrap_err();
    let axis_errors = errors
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidLaunchDomain)
        .collect::<Vec<_>>();
    assert_eq!(axis_errors.len(), 1);
    assert_eq!(
        axis_errors[0].location.function.as_ref().unwrap().as_str(),
        "helper"
    );
    assert_eq!(
        axis_errors[0].location.kernel.as_ref().unwrap().as_str(),
        "test_kernel"
    );
}

#[test]
fn rejects_declared_capabilities_unsupported_by_the_target() {
    let mut module = one_block_module(
        vec![],
        vec![op(
            0,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d()),
        )],
    );
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::Float64);

    verify_module(&module).expect("target-independent verification should succeed");
    assert!(module.derived_capabilities().is_empty());
    let errors = verify_module_with_capabilities(&module, &BTreeSet::new()).unwrap_err();
    assert!(errors.contains(DiagnosticCode::UnsupportedCapability));
}

#[test]
fn rejects_undefined_and_non_dominating_ssa_uses() {
    let undefined = op(
        0,
        Type::F32,
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(99),
            rhs: ValueId(99),
        },
    );
    let errors = verify_module(&one_block_module(vec![], vec![undefined])).unwrap_err();
    assert!(errors.contains(DiagnosticCode::UndefinedValue));

    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut then_block = BasicBlock::new(BlockId(1));
    then_block.operations.push(op(
        1,
        Type::F32,
        OperationKind::Constant(Constant::F32Bits(0)),
    ));
    then_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut else_block = BasicBlock::new(BlockId(2));
    else_block.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![],
    });
    let mut merge = BasicBlock::new(BlockId(3));
    merge.operations.push(op(
        2,
        Type::F32,
        OperationKind::Unary {
            op: UnaryOp::Negate,
            operand: ValueId(1),
        },
    ));
    merge.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::definition(
        "dominance",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![entry, then_block, else_block, merge],
    );
    let mut module = Module::new("tests::dominance");
    module.functions.push(function);
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::NonDominatingUse));
}

#[test]
fn rejects_invalid_branch_targets_and_malformed_terminators() {
    let mut bad_target = BasicBlock::new(BlockId(0));
    bad_target.terminator = Some(Terminator::Branch {
        target: BlockId(7),
        arguments: vec![],
    });
    let missing = BasicBlock::new(BlockId(1));
    let function = Function::definition(
        "cfg",
        Signature::new(vec![], vec![]),
        vec![],
        vec![bad_target, missing],
    );
    let mut module = Module::new("tests::cfg");
    module.functions.push(function);

    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidBranchTarget));
    assert!(errors.contains(DiagnosticCode::MissingTerminator));
}

#[test]
fn rejects_type_invalid_memory_operations() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let operations = vec![
        op(
            2,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(0),
                access,
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
    ];
    let module = one_block_module(
        vec![
            Type::Scalar(ScalarType::I32),
            global_pointer(AccessMode::ReadOnly),
        ],
        operations,
    );
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidOperandType));
    assert!(errors.contains(DiagnosticCode::InvalidMemoryAccess));
    assert!(errors.contains(DiagnosticCode::TypeMismatch));
}

#[test]
fn accepts_mixed_width_integer_shift_operands() {
    for shift in [BinaryOp::ShiftLeft, BinaryOp::ShiftRight] {
        let module = one_block_module(
            vec![Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U32)],
            vec![op(
                2,
                Type::Scalar(ScalarType::U64),
                OperationKind::Binary {
                    op: shift,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            )],
        );
        verify_module(&module).expect("shift counts may use a different integer type");
    }
}

#[test]
fn rejects_non_integer_shift_rhs() {
    for shift in [BinaryOp::ShiftLeft, BinaryOp::ShiftRight] {
        let module = one_block_module(
            vec![Type::Scalar(ScalarType::U64), Type::F32],
            vec![op(
                2,
                Type::Scalar(ScalarType::U64),
                OperationKind::Binary {
                    op: shift,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            )],
        );
        let errors = verify_module(&module).unwrap_err();
        assert!(errors.contains(DiagnosticCode::InvalidOperandType));
    }
}

#[test]
fn rejects_obviously_invalid_barrier_and_atomic_metadata() {
    let barrier = Operation::new(
        vec![],
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Device,
            memory_scope: SynchronizationScope::Workgroup,
            semantics: BarrierSemantics::new(MemoryOrdering::Relaxed, []),
        }),
    );
    let atomic = op(
        2,
        Type::F32,
        OperationKind::Atomic(Atomic {
            kind: AtomicKind::BitAnd,
            pointer: ValueId(0),
            value: Some(ValueId(1)),
            compare: Some(ValueId(1)),
            access: MemoryAccess::new(AddressSpace::Global, 1),
            scope: SynchronizationScope::Invocation,
            ordering: MemoryOrdering::Acquire,
            failure_ordering: Some(MemoryOrdering::Release),
        }),
    );
    let module = one_block_module(
        vec![global_pointer(AccessMode::ReadWrite), Type::F32],
        vec![barrier, atomic],
    );
    let errors = verify_module(&module).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidBarrier));
    assert!(errors.contains(DiagnosticCode::InvalidAtomic));
}

#[test]
fn diagnostics_are_deterministic_and_sorted() {
    let operation = op(
        0,
        Type::F32,
        OperationKind::Load {
            pointer: ValueId(42),
            access: MemoryAccess::new(AddressSpace::Global, 3),
        },
    );
    let module = one_block_module(vec![], vec![operation]);
    let first = verify_module(&module).unwrap_err().into_diagnostics();
    let second = verify_module(&module).unwrap_err().into_diagnostics();
    assert_eq!(first, second);
    assert!(first.windows(2).all(|window| window[0] <= window[1]));
}

#[test]
fn function_roles_are_explicit_and_conflicts_fail_closed() {
    let mut valid = valid_vecadd_module();
    let mut returning = BasicBlock::new(BlockId(0));
    returning.terminator = Some(Terminator::Return { values: vec![] });
    valid.functions.extend([
        Function::internal_helper(
            "private_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returning.clone()],
        ),
        Function::device_ffi_export(
            "public_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![returning],
        ),
        Function::external_import("imported", Signature::new(vec![], vec![])),
    ]);
    verify_module(&valid).expect("consistent explicit roles");

    let mut wrong_entry = valid.clone();
    wrong_entry.functions[0].role = FunctionRole::InternalHelper;
    let errors = verify_module(&wrong_entry).unwrap_err();
    assert!(errors.contains(DiagnosticCode::ConflictingFunctionRole));

    let mut bodyless_export = valid.clone();
    bodyless_export
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "public_helper")
        .unwrap()
        .body = None;
    let errors = verify_module(&bodyless_export).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidFunctionRole));

    let mut duplicate_role = valid;
    duplicate_role.functions.push(Function::device_ffi_export(
        "private_helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations: vec![],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    ));
    let errors = verify_module(&duplicate_role).unwrap_err();
    assert!(errors.contains(DiagnosticCode::DuplicateFunction));
    assert!(errors.contains(DiagnosticCode::ConflictingFunctionRole));
}
