use super::*;
use fe2o3_kernel_ir::{BasicBlock, BlockId, Function, Kernel, Signature, ValueDef};

pub(super) fn genuine_inputs() -> (
    fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    ValidatedConditionalCompilerProofInputsV1,
) {
    let handoff = fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3::decode(include_bytes!(
        "fill.handoff"
    ))
    .unwrap();
    let receipts = handoff.capsule().receipts();
    let inputs = crate::validate_conditional_compiler_proof_inputs_v1(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .unwrap();
    (handoff, inputs)
}

#[test]
fn genuine_fill_checks_both_complete_programs_without_granting_authority() {
    let (handoff, inputs) = genuine_inputs();
    let lineage =
        crate::validate_conditional_compiler_target_lineage_v1(handoff.capsule(), &inputs).unwrap();
    let checked = check_conditional_fill_program_v1(&inputs, &lineage).unwrap();
    assert_eq!(checked.function_symbol(), "fill_write_only");
    assert!(std::ptr::eq(checked.inputs(), &inputs));
    assert!(std::ptr::eq(checked.lineage(), &lineage));
    assert!(!checked.grants_runtime_authority());
}

fn module() -> Module {
    let mut module = Module::new("fill-profile-test");
    let mut kernel = Kernel::new(
        "fill",
        "fill",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let output = ValueId(u32::MAX);
    let id = ValueId;
    let mut block = BasicBlock::new(BlockId(17));
    for (index, (value, kind)) in [
        (
            Value::Index,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        (
            Value::IndexU64,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: id(40),
                to: Value::IndexU64.ty(),
            },
        ),
        (
            Value::TruncatedIndex,
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: id(41),
                to: Value::TruncatedIndex.ty(),
            },
        ),
        (Value::Length, OperationKind::SliceLength { slice: output }),
        (
            Value::InBounds,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: id(40),
                rhs: id(43),
            },
        ),
        (Value::Zero, OperationKind::Constant(Constant::Index(0))),
        (
            Value::SafeIndex,
            OperationKind::Select {
                condition: id(44),
                true_value: id(40),
                false_value: id(45),
            },
        ),
        (Value::Base, OperationKind::SliceData { slice: output }),
        (
            Value::Pointer,
            OperationKind::GetElementPointer {
                base: id(47),
                offset: id(46),
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        block.operations.push(Operation::new(
            vec![ValueDef::new(id(40 + index as u32), value.ty())],
            kind,
        ));
    }
    block.operations.push(Operation::new(
        vec![],
        OperationKind::GuardedStore {
            pointer: id(48),
            predicate: id(44),
            value: id(42),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::definition(
        "fill",
        Signature::new(vec![Value::Output.ty()], vec![]),
        vec![output],
        vec![block],
    );
    function.role = FunctionRole::KernelEntry;
    module.functions.push(function);
    module
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

#[test]
fn intrinsic_substitutions_cast_kind_and_stronger_store_alignment_reject() {
    use fe2o3_kernel_ir::{Axis, IndexKind, IntrinsicKind};
    for intrinsic in [
        IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            Type::INDEX,
        ),
        IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Y,
            },
            Type::INDEX,
        ),
        IntrinsicOperation::launch_extent_1d(),
        IntrinsicOperation::new(
            IntrinsicOperation::global_id_1d().kind,
            Type::Scalar(ScalarType::U64),
        ),
    ] {
        let mut module = module();
        operations(&mut module)[0].kind = OperationKind::Intrinsic(intrinsic);
        assert!(check_module(&module).is_err());
    }
    let mut changed = module();
    if let OperationKind::Cast { kind, .. } = &mut operations(&mut changed)[2].kind {
        *kind = CastKind::Bitcast;
    }
    assert!(check_module(&changed).is_err());
    let mut changed = module();
    if let OperationKind::GuardedStore { access, .. } = &mut operations(&mut changed)[9].kind {
        *access = MemoryAccess::new(AddressSpace::Global, 8);
    }
    assert!(check_module(&changed).is_err());
}

#[test]
fn profile_bounds_and_unsupported_block_argument_plumbing_are_checked() {
    let mut bounded = module();
    for index in 0..54 {
        operations(&mut bounded).push(Operation::new(
            vec![ValueDef::new(ValueId(100 + index), Type::INDEX)],
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ));
    }
    assert!(check_module(&bounded).is_ok());
    operations(&mut bounded).push(Operation::new(
        vec![ValueDef::new(ValueId(200), Type::INDEX)],
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    ));
    assert!(matches!(
        check_module(&bounded),
        Err(ConditionalFillProgramErrorV1::Profile)
    ));
    let mut bounded = module();
    let body = bounded.functions[0].body.as_mut().unwrap();
    for index in 1..64 {
        let id = BlockId(17 + index);
        body.blocks.last_mut().unwrap().terminator = Some(Terminator::Branch {
            target: id,
            arguments: vec![],
        });
        let mut block = BasicBlock::new(id);
        block.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.push(block);
    }
    assert!(check_module(&bounded).is_ok());
    bounded.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(BasicBlock::new(BlockId(1000)));
    assert!(check_module(&bounded).is_err());
    let mut changed = module();
    changed.functions[0].body.as_mut().unwrap().blocks[0]
        .parameters
        .push(ValueDef::new(ValueId(100), Type::INDEX));
    assert!(check_module(&changed).is_err());
    let mut changed = module();
    let body = changed.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(18),
        arguments: vec![ValueId(40)],
    });
    let mut block = BasicBlock::new(BlockId(18));
    block.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(block);
    assert!(check_module(&changed).is_err());
}

#[test]
fn whole_program_accepts_sparse_ids_and_unconditional_empty_plumbing() {
    let mut module = module();
    let shape = check_module(&module).unwrap();
    assert_eq!(shape.output, ValueId(u32::MAX));
    assert_eq!(shape.store, (17, 9));
    let body = module.functions[0].body.as_mut().unwrap();
    let mut entry = BasicBlock::new(BlockId(u32::MAX));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    body.blocks.insert(0, entry);
    assert_eq!(check_module(&module).unwrap().store, (17, 9));
}

#[test]
fn safety_only_predicates_and_changed_address_or_value_reject() {
    for case in 0..12 {
        let mut module = module();
        let ops = operations(&mut module);
        match case {
            0 => {
                if let OperationKind::Compare { predicate, .. } = &mut ops[4].kind {
                    *predicate = ComparePredicate::LessThanOrEqual;
                }
            }
            1 => ops[4].kind = OperationKind::Constant(Constant::Bool(false)),
            2 => ops[4].kind = OperationKind::Constant(Constant::Bool(true)),
            3 => {
                if let OperationKind::GuardedStore { predicate, .. } = &mut ops[9].kind {
                    *predicate = ValueId(45);
                }
            }
            4 => {
                if let OperationKind::GuardedStore { value, .. } = &mut ops[9].kind {
                    *value = ValueId(45);
                }
            }
            5 => {
                if let OperationKind::GuardedStore { pointer, .. } = &mut ops[9].kind {
                    *pointer = ValueId(47);
                }
            }
            6 => {
                if let OperationKind::GetElementPointer { offset, .. } = &mut ops[8].kind {
                    *offset = ValueId(40);
                }
            }
            7 => {
                if let OperationKind::Select {
                    true_value,
                    false_value,
                    ..
                } = &mut ops[6].kind
                {
                    std::mem::swap(true_value, false_value);
                }
            }
            8 => {
                if let OperationKind::SliceLength { slice } = &mut ops[3].kind {
                    *slice = ValueId(40);
                }
            }
            9 => {
                if let OperationKind::GuardedStore { access, .. } = &mut ops[9].kind {
                    access.volatile = true;
                }
            }
            10 => {
                if let OperationKind::GuardedStore { access, .. } = &mut ops[9].kind {
                    access.address_space = AddressSpace::Private;
                }
            }
            11 => ops[0].kind = OperationKind::Constant(Constant::Index(0)),
            _ => unreachable!(),
        }
        assert!(check_module(&module).is_err(), "mutant {case}");
    }
}

#[test]
fn signed_types_redefinitions_forward_references_and_additional_effects_reject() {
    for case in 0..8 {
        let mut module = module();
        let ops = operations(&mut module);
        match case {
            0 => ops[2].results[0].ty = Type::Scalar(ScalarType::I32),
            1 => ops[1].results[0].id = ValueId(40),
            2 => ops[1].results[0].id = ValueId(u32::MAX),
            3 => ops.swap(1, 2),
            4 => {
                let store = ops[9].clone();
                ops.push(store);
            }
            5 => {
                ops.pop();
            }
            6 => {
                if let OperationKind::Cast { to, .. } = &mut ops[2].kind {
                    *to = Type::Scalar(ScalarType::I32);
                }
            }
            7 => ops[9]
                .results
                .push(ValueDef::new(ValueId(60), Type::Scalar(ScalarType::Bool))),
            _ => unreachable!(),
        }
        assert!(check_module(&module).is_err(), "mutant {case}");
    }
}

#[test]
fn complete_control_flow_and_exact_profile_are_required() {
    for case in 0..10 {
        let mut module = module();
        match case {
            0 => module.kernels[0].id = "other".into(),
            1 => module.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1)),
            2 => module.functions[0].role = FunctionRole::InternalHelper,
            3 => {
                let copy = module.functions[0].clone();
                module.functions.push(copy);
            }
            case => {
                let body = module.functions[0].body.as_mut().unwrap();
                match case {
                    4 => body.blocks[0].terminator = None,
                    5 => {
                        body.blocks[0].terminator = Some(Terminator::Branch {
                            target: BlockId(17),
                            arguments: vec![],
                        })
                    }
                    6 => {
                        body.blocks[0].terminator = Some(Terminator::Branch {
                            target: BlockId(18),
                            arguments: vec![],
                        })
                    }
                    7 => {
                        let mut dead = BasicBlock::new(BlockId(18));
                        dead.terminator = Some(Terminator::Return { values: vec![] });
                        body.blocks.push(dead);
                    }
                    8 => {
                        body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                            condition: ValueId(44),
                            then_target: BlockId(17),
                            then_arguments: vec![],
                            else_target: BlockId(17),
                            else_arguments: vec![],
                        })
                    }
                    9 => {
                        let copy = body.blocks[0].clone();
                        body.blocks.push(copy);
                    }
                    _ => unreachable!(),
                }
            }
        }
        assert!(check_module(&module).is_err(), "mutant {case}");
    }
}
