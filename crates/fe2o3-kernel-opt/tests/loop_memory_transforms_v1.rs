use std::collections::BTreeMap;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function,
    FunctionId, GlobalCapabilityTypeV1, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType,
    Signature, Terminator, Type, ValueDef, ValueId, WorkgroupSize, verify_module,
};
use fe2o3_kernel_opt::{
    CanonicalLoopBlockRoleV1, CheckedOptimizerQueryKindV1, CheckedOptimizerQuerySessionV1,
    LoopMemoryEditV1, LoopMemoryTransformErrorV1, LoopMemoryTransformLimitsV1,
    LoopMemoryTransformV1, MemoryForwardKindV1, MemoryVersionEventKindV1,
    OperationCoordinateLineageV1, OptimizerQueryDecisionV1, OptimizerQueryFailureV1,
    OptimizerQueryLimitsV1, ProductionTransformationV1, check_loop_memory_transform_relation_v1,
    execute_checked_canonical_transformation_v13_v1, execute_loop_memory_transform_v1,
};

fn output_pointer() -> Type {
    Type::pointer(Type::INDEX, AddressSpace::Global, AccessMode::WriteOnly)
}

fn private_pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    )
}

fn u32_output_pointer() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    )
}

fn finish_module(functions: Vec<Function>) -> Module {
    let mut module = Module::new("loop-memory-transforms-v1");
    module.functions = functions;
    module.kernels.push(Kernel::new(
        "kernel",
        "kernel_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    verify_module(&module).unwrap();
    module
}

fn loop_module(
    static_bound: Option<u64>,
    reversed_update: bool,
    invariant: bool,
    critical_edges: bool,
    trapping_invariant: bool,
) -> Module {
    let dynamic = static_bound.is_none();
    let mut parameters = vec![
        Type::Scalar(ScalarType::U32),
        Type::Scalar(ScalarType::U32),
        output_pointer(),
        Type::BOOL,
    ];
    let mut parameter_values = vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)];
    if dynamic {
        parameters.push(Type::INDEX);
        parameter_values.push(ValueId(4));
    }

    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
    ]);
    let bound = if let Some(bound) = static_bound {
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(7), Type::INDEX),
            OperationKind::Constant(Constant::Index(bound)),
        ));
        ValueId(7)
    } else {
        ValueId(4)
    };
    entry.terminator = Some(if critical_edges {
        Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(1),
            then_arguments: vec![ValueId(5)],
            else_target: BlockId(3),
            else_arguments: vec![ValueId(5)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(5)],
        }
    });

    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = vec![ValueDef::new(ValueId(10), Type::INDEX)];
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(11), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(10),
            rhs: bound,
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(10)],
    });

    let mut body = BasicBlock::new(BlockId(2));
    if invariant {
        body.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(12),
                if trapping_invariant {
                    Type::Scalar(ScalarType::U32)
                } else {
                    Type::BOOL
                },
            ),
            if trapping_invariant {
                OperationKind::Binary {
                    op: BinaryOp::Divide,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                }
            } else {
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                }
            },
        ));
    }
    body.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(13), Type::INDEX),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: if reversed_update {
                ValueId(6)
            } else {
                ValueId(10)
            },
            rhs: if reversed_update {
                ValueId(10)
            } else {
                ValueId(6)
            },
        },
    ));
    body.terminator = Some(if critical_edges {
        Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(1),
            then_arguments: vec![ValueId(13)],
            else_target: BlockId(3),
            else_arguments: vec![ValueId(13)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(13)],
        }
    });

    let mut exit = BasicBlock::new(BlockId(3));
    exit.parameters = vec![ValueDef::new(ValueId(20), Type::INDEX)];
    exit.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(2),
            value: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Global, 8),
        },
    ));
    exit.terminator = Some(Terminator::Return { values: vec![] });

    finish_module(vec![Function::kernel_entry(
        "kernel_impl",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![entry, header, body, exit],
    )])
}

fn memory_module(boundary: bool, volatile: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(3), private_pointer()),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ]);
    if boundary {
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: "opaque".into(),
                arguments: vec![],
            },
        ));
    }
    let mut load_access = MemoryAccess::new(AddressSpace::Private, 4);
    load_access.volatile = volatile;
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(3),
                access: load_access,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let kernel = Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::WriteOnly,
                ),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    );
    let mut functions = vec![kernel];
    if boundary {
        functions.push(Function::external_import(
            "opaque",
            Signature::new(vec![], vec![]),
        ));
    }
    finish_module(functions)
}

fn load_to_load_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    let mut volatile = MemoryAccess::new(AddressSpace::Private, 4);
    volatile.volatile = true;
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), private_pointer()),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(0),
                access: volatile,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    finish_module(vec![Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::WriteOnly,
                ),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )])
}

#[derive(Clone, Copy)]
enum InterblockShape {
    Linear,
    LinearOverwrite,
    DiamondSame,
    DiamondDifferent,
    BranchOverwrite,
    UnknownCall,
}

fn interblock_private_module(shape: InterblockShape) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(4), private_pointer()),
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    ));
    if !matches!(shape, InterblockShape::DiamondDifferent) {
        entry.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(4),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ));
    }

    let mut blocks = vec![entry];
    let load_block = match shape {
        InterblockShape::Linear => {
            blocks[0].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
            BlockId(1)
        }
        InterblockShape::LinearOverwrite => {
            blocks[0].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
            let mut overwrite = BasicBlock::new(BlockId(1));
            overwrite.operations.push(Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(4),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ));
            overwrite.terminator = Some(Terminator::Branch {
                target: BlockId(2),
                arguments: vec![],
            });
            blocks.push(overwrite);
            BlockId(2)
        }
        InterblockShape::DiamondSame
        | InterblockShape::DiamondDifferent
        | InterblockShape::BranchOverwrite => {
            blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(1),
                then_arguments: vec![],
                else_target: BlockId(2),
                else_arguments: vec![],
            });
            for (id, value) in [(BlockId(1), ValueId(0)), (BlockId(2), ValueId(1))] {
                let mut branch = BasicBlock::new(id);
                if matches!(shape, InterblockShape::DiamondDifferent)
                    || (matches!(shape, InterblockShape::BranchOverwrite) && id == BlockId(1))
                {
                    let stored_value = if matches!(shape, InterblockShape::BranchOverwrite) {
                        ValueId(1)
                    } else {
                        value
                    };
                    branch.operations.push(Operation::new(
                        vec![],
                        OperationKind::Store {
                            pointer: ValueId(4),
                            value: stored_value,
                            access: MemoryAccess::new(AddressSpace::Private, 4),
                        },
                    ));
                }
                branch.terminator = Some(Terminator::Branch {
                    target: BlockId(3),
                    arguments: vec![],
                });
                blocks.push(branch);
            }
            BlockId(3)
        }
        InterblockShape::UnknownCall => {
            blocks[0].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
            let mut boundary = BasicBlock::new(BlockId(1));
            boundary.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "opaque".into(),
                    arguments: vec![],
                },
            ));
            boundary.terminator = Some(Terminator::Branch {
                target: BlockId(2),
                arguments: vec![],
            });
            blocks.push(boundary);
            BlockId(2)
        }
    };

    let mut load = BasicBlock::new(load_block);
    load.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    load.terminator = Some(Terminator::Return { values: vec![] });
    blocks.push(load);

    let kernel = Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
                u32_output_pointer(),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        blocks,
    );
    let mut functions = vec![kernel];
    if matches!(shape, InterblockShape::UnknownCall) {
        functions.push(Function::external_import(
            "opaque",
            Signature::new(vec![], vec![]),
        ));
    }
    finish_module(functions)
}

fn interblock_external_load_module(access: AccessMode) -> Module {
    let pointer = Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Global, access);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::Load {
            pointer: ValueId(0),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut successor = BasicBlock::new(BlockId(1));
    successor.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    successor.terminator = Some(Terminator::Return { values: vec![] });
    finish_module(vec![Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![pointer, u32_output_pointer()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, successor],
    )])
}

fn guarded_dynamic_loop_module() -> Module {
    let mut module = loop_module(None, false, true, false, false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(7), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(5),
            rhs: ValueId(4),
        },
    ));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(5)],
    });
    let mut preheader = BasicBlock::new(BlockId(4));
    preheader.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(5)],
    });
    body.blocks.insert(1, preheader);
    verify_module(&module).unwrap();
    module
}

fn affine_guarded_dynamic_loop_module(exact: bool, exact_bound: u64) -> Module {
    let mut module = loop_module(None, false, false, false, false);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::INDEX),
            OperationKind::Constant(Constant::Index(exact_bound)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Constant(Constant::Index(2)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(4),
                rhs: ValueId(8),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), Type::BOOL),
            OperationKind::Compare {
                predicate: if exact {
                    ComparePredicate::Equal
                } else {
                    ComparePredicate::LessThanOrEqual
                },
                lhs: ValueId(9),
                rhs: ValueId(7),
            },
        ),
    ]);
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(14),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(5)],
    });
    let OperationKind::Compare { rhs, .. } = &mut body.blocks[1].operations[0].kind else {
        unreachable!()
    };
    *rhs = ValueId(9);
    let mut preheader = BasicBlock::new(BlockId(4));
    preheader.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(5)],
    });
    body.blocks.insert(1, preheader);
    verify_module(&module).unwrap();
    module
}

fn loop_carried_memory_module(boundary: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(3), private_pointer()),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::Constant(Constant::Index(4)),
        ),
    ]);
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(4)],
    });

    let mut header = BasicBlock::new(BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(10), Type::INDEX));
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(11), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(10),
            rhs: ValueId(6),
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });

    let mut loop_body = BasicBlock::new(BlockId(2));
    if boundary {
        loop_body.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("opaque"),
                arguments: vec![],
            },
        ));
    }
    loop_body.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(12), Type::INDEX),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(10),
            rhs: ValueId(5),
        },
    ));
    loop_body.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(12)],
    });

    let mut exit = BasicBlock::new(BlockId(3));
    exit.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(13), Type::Scalar(ScalarType::U32)),
            OperationKind::Load {
                pointer: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(13),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let kernel = Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                Type::Scalar(ScalarType::U32),
                Type::INDEX,
                u32_output_pointer(),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, header, loop_body, exit],
    );
    let mut functions = vec![kernel];
    if boundary {
        functions.push(Function::external_import(
            "opaque",
            Signature::new(vec![], vec![]),
        ));
    }
    finish_module(functions)
}

fn workgroup_memory_module() -> Module {
    let mut module = memory_module(false, false);
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations[0].results[0].ty = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let OperationKind::Alloca { address_space, .. } = &mut operations[0].kind else {
        unreachable!()
    };
    *address_space = AddressSpace::Workgroup;
    for operation in &mut operations[1..6] {
        match &mut operation.kind {
            OperationKind::Load { access, .. } | OperationKind::Store { access, .. } => {
                access.address_space = AddressSpace::Workgroup;
            }
            _ => {}
        }
    }
    verify_module(&module).unwrap();
    module
}

fn exclusive_global_memory_module() -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let context = KernelContextTypeV1::new("kernel_impl", [1; 32], [2; 32], [3; 32]);
    let source = KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]);
    let capability = GlobalCapabilityTypeV1::exclusive_read_write(element.clone(), context.clone());
    let pointer = capability.physical_pointer_type();
    let physical = capability.physical_slice_type();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend([
        Operation::kernel_context_issue(ValueId(3), context, source),
        Operation::global_capability_bind(ValueId(4), capability, ValueId(3), ValueId(0)),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(4) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(6),
                offset: ValueId(5),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), element),
            OperationKind::Load {
                pointer: ValueId(7),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = finish_module(vec![Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                physical,
                Type::Scalar(ScalarType::U32),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    )]);
    module.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(1),
    };
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    verify_module(&module).unwrap();
    module
}

#[test]
fn critical_loop_edges_gain_preheader_latch_and_dedicated_exit() {
    let input = loop_module(Some(4), false, false, true, false);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::LoopCanonicalization,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(report.inserted_blocks(), 3);
    let roles = report
        .edits()
        .iter()
        .filter_map(|edit| match edit {
            LoopMemoryEditV1::CanonicalBlockInserted { role, .. } => Some(*role),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        vec![
            CanonicalLoopBlockRoleV1::Preheader,
            CanonicalLoopBlockRoleV1::Latch,
            CanonicalLoopBlockRoleV1::DedicatedExit,
        ]
    );
    check_loop_memory_transform_relation_v1(
        LoopMemoryTransformV1::LoopCanonicalization,
        &input,
        &output,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
}

#[test]
fn induction_recognition_normalizes_only_the_exact_reversed_update() {
    let input = loop_module(Some(5), true, false, false, false);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::InductionVariableSimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.changed());
    assert_eq!(report.induction_facts()[0].trip_count(), Some(5));
    let OperationKind::Binary { lhs, rhs, .. } =
        output.functions[0].body.as_ref().unwrap().blocks[2].operations[0].kind
    else {
        panic!("expected normalized update")
    };
    assert_eq!((lhs, rhs), (ValueId(10), ValueId(6)));

    let dynamic = loop_module(None, false, false, false, false);
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::FullLoopUnrolling,
        &dynamic,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, dynamic);
    assert_eq!(report.induction_facts()[0].trip_count(), None);
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::PartialLoopUnrolling,
        &dynamic,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, dynamic);
    assert_eq!(report.induction_facts()[0].trip_count(), None);
}

#[test]
fn licm_hoists_total_invariants_and_rejects_potential_traps() {
    let input = loop_module(Some(4), false, true, false, false);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::LoopInvariantCodeMotion,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.changed());
    assert!(report.coordinate_lineage().iter().any(|lineage| matches!(
        lineage,
        OperationCoordinateLineageV1::Moved { before, after }
            if before.block() == BlockId(2) && after.block() == BlockId(0)
    )));
    assert!(matches!(
        output.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .last()
            .unwrap()
            .kind,
        OperationKind::Compare { .. }
    ));

    let trapping = loop_module(Some(4), false, true, false, true);
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::LoopInvariantCodeMotion,
        &trapping,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, trapping);
    assert!(!report.changed());

    for input in [
        loop_module(Some(0), false, true, false, false),
        loop_module(None, false, true, false, false),
    ] {
        let (unchanged, report) = execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::LoopInvariantCodeMotion,
            &input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(unchanged, input);
        assert!(!report.changed());
    }
}

#[test]
fn full_and_partial_unrolling_are_differentially_equivalent_for_bounded_trips() {
    for trip in 0..=8 {
        let input = loop_module(Some(trip), false, false, false, false);
        let (output, report) = execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::FullLoopUnrolling,
            &input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        assert!(report.changed(), "trip {trip}");
        assert_eq!(report.cost_decisions().len(), 1);
        assert!(report.cost_decisions()[0].is_admitted());
        assert!(!report.cost_decisions()[0].grants_semantic_authority());
        assert!(!report.cost_decisions()[0].grants_target_authority());
        assert!(
            trip == 0
                || report.coordinate_lineage().iter().any(|lineage| matches!(
                    lineage,
                    OperationCoordinateLineageV1::Duplicated { .. }
                ))
        );
        for seed in 0..8 {
            assert_eq!(
                interpret(&input, [seed, seed + 1, 1, trip]),
                interpret(&output, [seed, seed + 1, 1, trip])
            );
        }
    }

    for trip in [10, 12, 14, 16, 18, 20] {
        let input = loop_module(Some(trip), false, false, false, false);
        let (output, report) = execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::PartialLoopUnrolling,
            &input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        assert!(report.changed(), "trip {trip}");
        assert_eq!(
            interpret(&input, [3, 7, 1, trip]),
            interpret(&output, [3, 7, 1, trip])
        );
    }
}

#[test]
fn memory_versions_drive_private_forwarding_redundant_loads_and_dse() {
    let input = memory_module(false, false);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.changed());
    assert_eq!(report.eliminated_operations(), 4);
    assert!(
        report
            .coordinate_lineage()
            .iter()
            .any(|lineage| matches!(lineage, OperationCoordinateLineageV1::Eliminated { .. }))
    );
    assert!(report.coordinate_lineage().iter().any(|lineage| matches!(
        lineage,
        OperationCoordinateLineageV1::Retained { before, after }
            if before.operation() != after.operation()
    )));
    assert!(report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::MemoryValueForwarded {
            kind: MemoryForwardKindV1::StoreToLoad,
            ..
        }
    )));
    assert!(
        report
            .edits()
            .iter()
            .any(|edit| matches!(edit, LoopMemoryEditV1::DeadStoreEliminated { .. }))
    );
    let graph = report.memory_graph().unwrap();
    assert!(
        graph
            .events()
            .iter()
            .any(|event| event.kind() == MemoryVersionEventKindV1::Load)
    );
    assert!(
        graph
            .events()
            .iter()
            .any(|event| event.kind() == MemoryVersionEventKindV1::Store)
    );
    for first in 0..16 {
        for second in 0..16 {
            assert_eq!(
                interpret(&input, [first, second, 1, 0]),
                interpret(&output, [first, second, 1, 0])
            );
        }
    }

    let input = load_to_load_module();
    let (_, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::MemoryValueForwarded {
            kind: MemoryForwardKindV1::LoadToLoad,
            ..
        }
    )));
}

#[test]
fn bounded_fixed_point_forwards_loop_carried_facts_and_rejects_epoch_conflicts() {
    let input = loop_carried_memory_module(false);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.loop_carried_memory_phis().iter().any(|phi| {
        phi.block() == BlockId(1)
            && phi.predecessors() == [BlockId(0), BlockId(2)]
            && !phi.available_producers().is_empty()
    }));
    assert!(report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::MemoryValueForwarded { load, .. } if load.block() == BlockId(3)
    )));
    for value in 0..16 {
        assert_eq!(
            interpret(&input, [value, 0, 1]),
            interpret(&output, [value, 0, 1])
        );
    }

    let boundary = loop_carried_memory_module(true);
    let (_, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &boundary,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.loop_carried_memory_phis().iter().any(|phi| {
        phi.block() == BlockId(1)
            && matches!(
                phi.synchronization_epoch(),
                fe2o3_kernel_opt::MemorySynchronizationEpochV1::Ambiguous
            )
    }));
    assert!(!report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::MemoryValueForwarded { load, .. } if load.block() == BlockId(3)
    )));

    let limits = LoopMemoryTransformLimitsV1::default()
        .with_memory_dataflow_iteration_limit(1)
        .unwrap();
    assert_eq!(
        execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            limits,
        )
        .unwrap_err(),
        LoopMemoryTransformErrorV1::MemoryDataflowIterationLimitExceeded { limit: 1 }
    );
}

#[test]
fn writable_workgroup_and_exclusive_global_require_exact_provenance() {
    let workgroup = workgroup_memory_module();
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &workgroup,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(
        report
            .edits()
            .iter()
            .any(|edit| matches!(edit, LoopMemoryEditV1::MemoryValueForwarded { .. }))
    );
    assert!(
        report
            .edits()
            .iter()
            .any(|edit| matches!(edit, LoopMemoryEditV1::DeadStoreEliminated { .. }))
    );
    for first in 0..8 {
        for second in 0..8 {
            assert_eq!(
                interpret(&workgroup, [first, second, 1]),
                interpret(&output, [first, second, 1])
            );
        }
    }

    let mut concurrent_workgroup = workgroup.clone();
    concurrent_workgroup.kernels[0].workgroup_size = Some(WorkgroupSize::new(2, 1, 1));
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &concurrent_workgroup,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, concurrent_workgroup);
    assert!(!report.changed());

    let global = exclusive_global_memory_module();
    let (_, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &global,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(
        report
            .edits()
            .iter()
            .any(|edit| matches!(edit, LoopMemoryEditV1::MemoryValueForwarded { .. }))
    );
    assert!(
        report
            .edits()
            .iter()
            .any(|edit| matches!(edit, LoopMemoryEditV1::DeadStoreEliminated { .. }))
    );
    assert!(report.memory_graph().unwrap().events().iter().any(|event| {
        event
            .location()
            .is_some_and(fe2o3_kernel_opt::MemoryLocationV1::is_exclusive_global)
    }));
    let mut concurrent_global = global.clone();
    concurrent_global.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Dynamic,
    };
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &concurrent_global,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, concurrent_global);
    assert!(!report.changed());
}

#[test]
fn shared_writable_admission_matches_the_small_launch_shape_product() {
    for x in 1..=2 {
        for y in 1..=2 {
            for z in 1..=2 {
                let mut input = workgroup_memory_module();
                input.kernels[0].domain = LaunchDomain::D3 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Dynamic,
                    z: LaunchExtent::Dynamic,
                };
                input.kernels[0].workgroup_size = Some(WorkgroupSize::new(x, y, z));
                verify_module(&input).unwrap();
                let expected = x == 1 && y == 1 && z == 1;
                let first = execute_loop_memory_transform_v1(
                    LoopMemoryTransformV1::MemorySimplification,
                    &input,
                    LoopMemoryTransformLimitsV1::default(),
                )
                .unwrap();
                let second = execute_loop_memory_transform_v1(
                    LoopMemoryTransformV1::MemorySimplification,
                    &input,
                    LoopMemoryTransformLimitsV1::default(),
                )
                .unwrap();
                assert_eq!(first, second);
                assert_eq!(first.1.changed(), expected);
                assert_eq!(first.0 == input, !expected);
            }
        }
    }

    for extent in [
        LaunchExtent::Static(1),
        LaunchExtent::Static(2),
        LaunchExtent::Dynamic,
    ] {
        for workgroup_x in 1..=2 {
            let mut input = exclusive_global_memory_module();
            input.kernels[0].domain = LaunchDomain::D1 { x: extent };
            input.kernels[0].workgroup_size = Some(WorkgroupSize::new(workgroup_x, 1, 1));
            verify_module(&input).unwrap();
            let expected = extent == LaunchExtent::Static(1) && workgroup_x == 1;
            let (output, report) = execute_loop_memory_transform_v1(
                LoopMemoryTransformV1::MemorySimplification,
                &input,
                LoopMemoryTransformLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(report.changed(), expected);
            assert_eq!(output == input, !expected);
        }
    }
}

#[test]
fn memory_facts_cross_linear_edges_and_agreeing_diamond_joins() {
    for shape in [InterblockShape::Linear, InterblockShape::DiamondSame] {
        let input = interblock_private_module(shape);
        let (output, report) = execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        assert!(report.edits().iter().any(|edit| matches!(
            edit,
            LoopMemoryEditV1::MemoryValueForwarded {
                kind: MemoryForwardKindV1::StoreToLoad,
                ..
            }
        )));
        for first in 0..8 {
            for condition in 0..=1 {
                assert_eq!(
                    interpret(&input, [first, first + 1, 1, condition]),
                    interpret(&output, [first, first + 1, 1, condition])
                );
            }
        }
    }
}

#[test]
fn dead_private_store_is_removed_across_only_a_linear_cfg_path() {
    let input = interblock_private_module(InterblockShape::LinearOverwrite);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::DeadStoreEliminated {
            store,
            overwriting_store,
            ..
        } if store.block() == BlockId(0) && overwriting_store.block() == BlockId(1)
    )));
    for first in 0..8 {
        for second in 0..8 {
            assert_eq!(
                interpret(&input, [first, second, 1, 0]),
                interpret(&output, [first, second, 1, 0])
            );
        }
    }
}

#[test]
fn branch_local_overwrite_cannot_eliminate_a_dominating_store() {
    let input = interblock_private_module(InterblockShape::BranchOverwrite);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(!report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::DeadStoreEliminated { store, .. }
            if store.block() == BlockId(0)
    )));
    for condition in 0..=1 {
        assert_eq!(
            interpret(&input, [7, 11, 1, condition]),
            interpret(&output, [7, 11, 1, condition])
        );
    }
}

#[test]
fn disagreeing_joins_and_unknown_calls_discard_memory_facts() {
    for shape in [
        InterblockShape::DiamondDifferent,
        InterblockShape::UnknownCall,
    ] {
        let input = interblock_private_module(shape);
        let (output, report) = execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        assert!(!report.edits().iter().any(|edit| matches!(
            edit,
            LoopMemoryEditV1::MemoryValueForwarded { load, .. }
                if load.block() == match shape {
                    InterblockShape::DiamondDifferent => BlockId(3),
                    InterblockShape::UnknownCall => BlockId(2),
                    _ => unreachable!(),
                }
        )));
        if matches!(shape, InterblockShape::DiamondDifferent) {
            for condition in 0..=1 {
                assert_eq!(
                    interpret(&input, [7, 11, 1, condition]),
                    interpret(&output, [7, 11, 1, condition])
                );
            }
        }
    }
}

#[test]
fn external_forwarding_requires_exact_read_only_provenance() {
    let read_only = interblock_external_load_module(AccessMode::ReadOnly);
    let (_, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &read_only,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.edits().iter().any(|edit| matches!(
        edit,
        LoopMemoryEditV1::MemoryValueForwarded {
            kind: MemoryForwardKindV1::LoadToLoad,
            ..
        }
    )));

    let read_write = interblock_external_load_module(AccessMode::ReadWrite);
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &read_write,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, read_write);
    assert!(!report.changed());
}

#[test]
fn guarded_dynamic_trip_query_enables_only_proved_licm() {
    let input = guarded_dynamic_loop_module();
    let session =
        CheckedOptimizerQuerySessionV1::new(&input, 17, OptimizerQueryLimitsV1::default()).unwrap();
    let decision = session.prove_dominating_unsigned_less_than(
        &FunctionId::new("kernel_impl"),
        BlockId(4),
        ValueId(5),
        ValueId(4),
    );
    let OptimizerQueryDecisionV1::Proved(receipt) = decision else {
        panic!("expected an exact dominating range proof")
    };
    assert_eq!(receipt.graph_epoch(), 17);
    assert!(!receipt.grants_semantic_authority());
    assert!(matches!(
        receipt.kind(),
        CheckedOptimizerQueryKindV1::DominatingUnsignedLessThan {
            destination: BlockId(4),
            ..
        }
    ));

    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::LoopInvariantCodeMotion,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.changed());
    let preheader = output.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| block.id == BlockId(4))
        .unwrap();
    assert!(matches!(
        preheader.operations.last().unwrap().kind,
        OperationKind::Compare {
            predicate: ComparePredicate::Equal,
            ..
        }
    ));
    for bound in 0..8 {
        assert_eq!(
            interpret(&input, [3, 7, 1, 1, bound]),
            interpret(&output, [3, 7, 1, 1, bound])
        );
    }
}

#[test]
fn exact_dominating_affine_bound_enables_bounded_dynamic_unrolling_and_replay() {
    let input = affine_guarded_dynamic_loop_module(true, 4);
    let session =
        CheckedOptimizerQuerySessionV1::new(&input, 23, OptimizerQueryLimitsV1::default()).unwrap();
    let OptimizerQueryDecisionV1::Proved(receipt) = session.prove_dominating_index_equals_constant(
        &FunctionId::new("kernel_impl"),
        BlockId(4),
        ValueId(9),
    ) else {
        panic!("expected exact dominating bound proof")
    };
    assert!(matches!(
        receipt.kind(),
        CheckedOptimizerQueryKindV1::DominatingIndexEqualsConstant { constant: 4, .. }
    ));
    assert_eq!(receipt.graph_epoch(), 23);
    let limited =
        CheckedOptimizerQuerySessionV1::new(&input, 23, OptimizerQueryLimitsV1::new(1, 1).unwrap())
            .unwrap();
    let exhausted = limited.prove_dominating_index_equals_constant(
        &FunctionId::new("kernel_impl"),
        BlockId(4),
        ValueId(9),
    );
    assert!(matches!(
        exhausted,
        OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::GraphWorkLimit {
            required: 2,
            limit: 1,
        })
    ));
    assert_eq!(
        limited.prove_dominating_index_equals_constant(
            &FunctionId::new("kernel_impl"),
            BlockId(4),
            ValueId(9),
        ),
        exhausted
    );

    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::FullLoopUnrolling,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.changed());
    let [proof] = report.dynamic_trip_proofs() else {
        panic!("expected one affine dynamic-trip proof")
    };
    assert_eq!(proof.exact_bound(), 4);
    assert_eq!(proof.expression().constant(), 2);
    assert_eq!(proof.expression().terms().len(), 1);
    assert_eq!(proof.expression().terms()[0].value(), ValueId(4));
    assert_eq!(proof.expression().terms()[0].coefficient(), 1);
    assert!(report.unsupported_dynamic_trip_expansions().is_empty());
    for dynamic_bound in 0..8 {
        assert_eq!(
            interpret(&input, [3, 7, 1, 0, dynamic_bound]),
            interpret(&output, [3, 7, 1, 0, dynamic_bound]),
        );
    }

    let checked = execute_checked_canonical_transformation_v13_v1(
        ProductionTransformationV1::FullLoopUnrolling,
        &input,
        23,
    )
    .unwrap();
    assert!(
        checked
            .preservation()
            .optimizer_query_replays()
            .iter()
            .any(|query| matches!(
                query.kind(),
                CheckedOptimizerQueryKindV1::DominatingIndexEqualsConstant { constant: 4, .. }
            ))
    );

    let ranged_only = affine_guarded_dynamic_loop_module(false, 4);
    let (unchanged, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::FullLoopUnrolling,
        &ranged_only,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(unchanged, ranged_only);
    assert!(report.dynamic_trip_proofs().is_empty());
    assert_eq!(report.unsupported_dynamic_trip_expansions().len(), 1);
}

#[test]
fn memory_dataflow_budget_fails_closed_deterministically() {
    let input = interblock_private_module(InterblockShape::Linear);
    let limits = LoopMemoryTransformLimitsV1::default()
        .with_memory_dataflow_work_limit(1)
        .unwrap();
    let first = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        limits,
    )
    .unwrap_err();
    assert!(matches!(
        first,
        LoopMemoryTransformErrorV1::MemoryDataflowWorkLimitExceeded { .. }
    ));
    assert_eq!(
        execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            limits,
        )
        .unwrap_err(),
        first
    );
}

#[test]
fn versioned_effect_graph_records_cfg_joins_and_loop_backedges() {
    let input = loop_module(Some(4), false, false, false, false);
    let (_, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemoryEffectVersioning,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    let graph = report.memory_graph().unwrap();
    let header = graph
        .block_entries()
        .iter()
        .find(|entry| entry.block() == BlockId(1))
        .unwrap();
    assert_eq!(
        header
            .incoming()
            .iter()
            .map(|incoming| incoming.predecessor())
            .collect::<Vec<_>>(),
        vec![BlockId(0), BlockId(2)]
    );
    for incoming in header.incoming() {
        let predecessor = graph
            .block_versions()
            .iter()
            .find(|block| block.block() == incoming.predecessor())
            .unwrap();
        assert_eq!(incoming.version(), predecessor.exit());
    }
}

#[test]
fn volatile_and_unknown_call_boundaries_fail_closed() {
    for input in [memory_module(true, false), memory_module(false, true)] {
        let (output, report) = execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        let load_count = |module: &Module| {
            module.functions[0].body.as_ref().unwrap().blocks[0]
                .operations
                .iter()
                .filter(|operation| matches!(operation.kind, OperationKind::Load { .. }))
                .count()
        };
        assert!(load_count(&output) >= 1);
        assert!(
            report
                .memory_graph()
                .unwrap()
                .events()
                .iter()
                .any(|event| matches!(
                    event.kind(),
                    MemoryVersionEventKindV1::VolatileLoadBoundary
                        | MemoryVersionEventKindV1::UnknownCallBoundary
                ))
        );
    }
}

#[test]
fn resource_limits_and_unaccounted_relations_are_rejected() {
    assert_eq!(
        LoopMemoryTransformLimitsV1::new(0, 1, 1, 1, 1, 1, 1, 2, 1, 1).unwrap_err(),
        LoopMemoryTransformErrorV1::InvalidLimits
    );
    let input = loop_module(Some(4), false, false, false, false);
    let limits =
        LoopMemoryTransformLimitsV1::new(64, 1_024, 64, 64, 64, 8, 32, 2, 1, 1_024).unwrap();
    assert!(matches!(
        execute_loop_memory_transform_v1(LoopMemoryTransformV1::FullLoopUnrolling, &input, limits),
        Err(LoopMemoryTransformErrorV1::UnrollGrowthLimitExceeded { .. })
    ));

    let memory = memory_module(false, false);
    let limits = LoopMemoryTransformLimitsV1::new(64, 1_024, 64, 64, 64, 8, 32, 2, 64, 1).unwrap();
    assert_eq!(
        execute_loop_memory_transform_v1(
            LoopMemoryTransformV1::MemoryEffectVersioning,
            &memory,
            limits
        )
        .unwrap_err(),
        LoopMemoryTransformErrorV1::MemoryEventLimitExceeded { limit: 1 }
    );

    let (mut altered, _) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::FullLoopUnrolling,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    altered.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(0, 1);
    assert!(
        check_loop_memory_transform_relation_v1(
            LoopMemoryTransformV1::FullLoopUnrolling,
            &input,
            &altered,
            LoopMemoryTransformLimitsV1::default(),
        )
        .is_err()
    );
}

#[test]
fn every_loop_memory_transform_is_deterministic() {
    let loops = loop_module(Some(12), true, true, false, false);
    let memory = memory_module(false, false);
    for transform in [
        LoopMemoryTransformV1::LoopCanonicalization,
        LoopMemoryTransformV1::InductionVariableSimplification,
        LoopMemoryTransformV1::LoopInvariantCodeMotion,
        LoopMemoryTransformV1::FullLoopUnrolling,
        LoopMemoryTransformV1::PartialLoopUnrolling,
    ] {
        let first = execute_loop_memory_transform_v1(
            transform,
            &loops,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        for _ in 0..4 {
            assert_eq!(
                execute_loop_memory_transform_v1(
                    transform,
                    &loops,
                    LoopMemoryTransformLimitsV1::default(),
                )
                .unwrap(),
                first
            );
        }
    }
    for transform in [
        LoopMemoryTransformV1::MemoryEffectVersioning,
        LoopMemoryTransformV1::MemorySimplification,
    ] {
        let first = execute_loop_memory_transform_v1(
            transform,
            &memory,
            LoopMemoryTransformLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            execute_loop_memory_transform_v1(
                transform,
                &memory,
                LoopMemoryTransformLimitsV1::default(),
            )
            .unwrap(),
            first
        );
    }
}

fn interpret<const N: usize>(module: &Module, arguments: [u64; N]) -> u64 {
    let function = &module.functions[0];
    let body = function.body.as_ref().unwrap();
    let mut values = function
        .body
        .as_ref()
        .unwrap()
        .parameters
        .iter()
        .copied()
        .zip(arguments)
        .collect::<BTreeMap<_, _>>();
    let mut memory = BTreeMap::<ValueId, u64>::new();
    let mut output = 0;
    let mut block = body.blocks[0].id;
    for _ in 0..10_000 {
        let current = body
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .unwrap();
        for operation in &current.operations {
            let result = match &operation.kind {
                OperationKind::Constant(Constant::Index(value)) => Some(*value),
                OperationKind::Constant(Constant::U32(value)) => Some(u64::from(*value)),
                OperationKind::Binary { op, lhs, rhs } => {
                    let (lhs, rhs) = (values[lhs], values[rhs]);
                    Some(match op {
                        BinaryOp::Add => lhs.checked_add(rhs).unwrap(),
                        BinaryOp::Subtract => lhs.checked_sub(rhs).unwrap(),
                        BinaryOp::Multiply => lhs.checked_mul(rhs).unwrap(),
                        BinaryOp::BitAnd => lhs & rhs,
                        BinaryOp::BitOr => lhs | rhs,
                        BinaryOp::BitXor => lhs ^ rhs,
                        other => panic!("unsupported interpreter binary {other:?}"),
                    })
                }
                OperationKind::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => Some(u64::from(match predicate {
                    ComparePredicate::Equal => values[lhs] == values[rhs],
                    ComparePredicate::NotEqual => values[lhs] != values[rhs],
                    ComparePredicate::LessThan => values[lhs] < values[rhs],
                    ComparePredicate::LessThanOrEqual => values[lhs] <= values[rhs],
                    ComparePredicate::GreaterThan => values[lhs] > values[rhs],
                    ComparePredicate::GreaterThanOrEqual => values[lhs] >= values[rhs],
                })),
                OperationKind::Alloca { .. } => {
                    let pointer = operation.results[0].id;
                    memory.insert(pointer, 0);
                    Some(u64::from(pointer.0))
                }
                OperationKind::Load { pointer, .. } => Some(memory[pointer]),
                OperationKind::Store { pointer, value, .. } => {
                    if *pointer == ValueId(2) {
                        output = values[value];
                    } else {
                        memory.insert(*pointer, values[value]);
                    }
                    None
                }
                kind => panic!("unsupported interpreter operation {kind:?}"),
            };
            if let Some(result) = result {
                values.insert(operation.results[0].id, result);
            }
        }
        match current.terminator.as_ref().unwrap() {
            Terminator::Branch { target, arguments } => {
                assign_block_arguments(body, *target, arguments, &mut values);
                block = *target;
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                let (target, arguments) = if values[condition] != 0 {
                    (*then_target, then_arguments)
                } else {
                    (*else_target, else_arguments)
                };
                assign_block_arguments(body, target, arguments, &mut values);
                block = target;
            }
            Terminator::Return { .. } => return output,
            terminator => panic!("unsupported interpreter terminator {terminator:?}"),
        }
    }
    panic!("interpreter step limit exceeded")
}

fn assign_block_arguments(
    body: &fe2o3_kernel_ir::FunctionBody,
    target: BlockId,
    arguments: &[ValueId],
    values: &mut BTreeMap<ValueId, u64>,
) {
    let block = body.blocks.iter().find(|block| block.id == target).unwrap();
    let incoming = arguments
        .iter()
        .map(|argument| values[argument])
        .collect::<Vec<_>>();
    for (parameter, value) in block.parameters.iter().zip(incoming) {
        values.insert(parameter.id, value);
    }
}
