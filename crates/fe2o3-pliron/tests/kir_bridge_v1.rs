use fe2o3_kernel_ir::*;
use fe2o3_pliron::{
    KirBridgeCoordinateV1, KirBridgeErrorV1, PlironOptimizationPlanV1, PlironSession, ShellLimits,
};

#[cfg(feature = "internal-test-context-access")]
use dialect_gpu::{
    AllocaOp as PlironAllocaOp, AtomicOp as PlironAtomicOp, CanonicalBarrierOp as PlironBarrierOp,
    CanonicalFenceOp as PlironFenceOp, CanonicalKirOperationAttr, CanonicalKirSafetyOpInterface,
    ExecutionCapabilityOp, Gfx950LdsTransposeOp as PlironGfx950LdsTransposeOp,
    GuardedLoadOp as PlironGuardedLoadOp, GuardedStoreOp as PlironGuardedStoreOp,
    InlineAssemblyOp as PlironInlineAssemblyOp, IntrinsicOp as PlironIntrinsicOp,
    MatrixOp as PlironMatrixOp, MemoryIntrinsicOp as PlironMemoryIntrinsicOp,
    WaveOp as PlironWaveOp, WorkgroupBarrierOp as PlironWorkgroupBarrierOp,
    WorkgroupMemoryOp as PlironWorkgroupMemoryOp,
};
#[cfg(feature = "internal-test-context-access")]
use dialect_kernel::{CanonicalIdentityAttr, ExecutionCapabilityContractAttr};
#[cfg(feature = "internal-test-context-access")]
use pliron::{
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::{Op, op_cast},
    operation::Operation as PlironOperation,
};

fn session() -> PlironSession {
    PlironSession::new(
        ShellLimits::default(),
        [
            dialect_gpu::dialect_registration().expect("valid gpu registration"),
            dialect_kernel::dialect_registration().expect("valid kernel registration"),
        ],
    )
    .expect("fresh Pliron session")
}

fn rich_supported_module() -> Module {
    let slice = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let u32_ty = Type::Scalar(ScalarType::U32);
    let u64_ty = Type::Scalar(ScalarType::U64);
    let mut entry = BasicBlock::new(fe2o3_kernel_ir::BlockId(10));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(5), pointer.clone()),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::INDEX),
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(6),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(7)),
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(9), u32_ty.clone()),
            ValueDef::new(ValueId(11), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(8),
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(7),
                value: ValueId(9),
                access: MemoryAccess {
                    address_space: AddressSpace::Global,
                    alignment: 4,
                    volatile: true,
                },
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), u32_ty.clone()),
            OperationKind::Load {
                pointer: ValueId(7),
                access: MemoryAccess {
                    address_space: AddressSpace::Global,
                    alignment: 4,
                    volatile: true,
                },
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), u32_ty.clone()),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(12),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::NotEqual,
                lhs: ValueId(12),
                rhs: ValueId(9),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(15), u32_ty),
            OperationKind::Select {
                condition: ValueId(14),
                true_value: ValueId(13),
                false_value: ValueId(9),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(16), u64_ty.clone()),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(15),
                to: u64_ty.clone(),
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(17), u64_ty.clone())],
            OperationKind::Call {
                callee: "identity_u64".into(),
                arguments: vec![ValueId(16)],
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(14),
        then_target: fe2o3_kernel_ir::BlockId(20),
        then_arguments: vec![ValueId(17)],
        else_target: fe2o3_kernel_ir::BlockId(30),
        else_arguments: vec![ValueId(17)],
    });

    let mut then_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(20));
    then_block
        .parameters
        .push(ValueDef::new(ValueId(20), u64_ty.clone()));
    then_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });
    let mut else_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(30));
    else_block
        .parameters
        .push(ValueDef::new(ValueId(30), u64_ty.clone()));
    else_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(30)],
    });
    let mut unreachable_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(40));
    unreachable_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(40), u64_ty.clone()),
        OperationKind::Constant(Constant::U64(99)),
    ));
    unreachable_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(40)],
    });

    let mut module = Module::new("tests::kir_pliron_bridge_v1");
    module.functions.push(Function::external_import(
        "identity_u64",
        Signature::new(vec![u64_ty.clone()], vec![u64_ty.clone()]),
    ));
    module.functions.push(Function::internal_helper(
        "supported",
        Signature::new(vec![slice, Type::Scalar(ScalarType::U32)], vec![u64_ty]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, then_block, else_block, unreachable_block],
    ));
    module
}

fn all_scalar_constants_module() -> Module {
    let constants = [
        Constant::Bool(true),
        Constant::I8(-7),
        Constant::I16(-1_234),
        Constant::I32(-123_456),
        Constant::I64(-9_876_543_210),
        Constant::U8(0xfe),
        Constant::U16(0xfedc),
        Constant::U32(0xfedc_ba98),
        Constant::U64(0xfedc_ba98_7654_3210),
        Constant::Index(u64::MAX),
        Constant::F16Bits(0x7e01),
        Constant::Bf16Bits(0x7fc1),
        Constant::F32Bits(0x7fc0_1234),
        Constant::F64Bits(0x7ff8_0000_0000_1234),
    ];
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations = constants
        .into_iter()
        .enumerate()
        .map(|(index, constant)| {
            Operation::effect_free(
                ValueDef::new(ValueId(index as u32), constant.ty()),
                OperationKind::Constant(constant),
            )
        })
        .collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::kir_pliron_scalar_constants_v1");
    module.functions.push(Function::internal_helper(
        "constants",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

fn non_dominance_physical_order_module() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: fe2o3_kernel_ir::BlockId(2),
        arguments: vec![],
    });

    let mut use_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(1));
    use_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), u32_ty.clone()),
        OperationKind::Unary {
            op: UnaryOp::Not,
            operand: ValueId(0),
        },
    ));
    use_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });

    let mut defining_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(2));
    defining_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), u32_ty.clone()),
        OperationKind::Constant(Constant::U32(7)),
    ));
    defining_block.terminator = Some(Terminator::Branch {
        target: fe2o3_kernel_ir::BlockId(1),
        arguments: vec![],
    });

    let mut module = Module::new("tests::kir_pliron_non_dominance_order_v1");
    module.functions.push(Function::internal_helper(
        "non_dominance_order",
        Signature::new(vec![], vec![u32_ty]),
        vec![],
        vec![entry, use_block, defining_block],
    ));
    module
}

fn assert_exact_through_standard_optimization(module: Module) {
    let input = VerifiedCanonicalKernelIrV9::from_module(module).expect("valid canonical KIR");
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let (o0, report) = owner.extract_canonical_kir_v9_o0(&graph).unwrap();
    assert_eq!(o0.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
    owner
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    let (optimized, _) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    assert_eq!(optimized.canonical_bytes(), input.canonical_bytes());
}

fn preserved_memory_and_synchronization_module() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let global_pointer = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let private_pointer =
        Type::pointer(u32_ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let workgroup_pointer = Type::pointer(
        u32_ty.clone(),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), private_pointer),
            OperationKind::Alloca {
                element: u32_ty.clone(),
                count: Some(ValueId(3)),
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), u32_ty.clone()),
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(1),
                fallback: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(0),
                predicate: ValueId(1),
                value: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
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
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), workgroup_pointer),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: u32_ty.clone(),
                extent: WorkgroupMemoryExtent::Static(64),
                alignment: 16,
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::Fence(Fence {
                memory_scope: SynchronizationScope::Device,
                semantics: BarrierSemantics::new(MemoryOrdering::Release, [AddressSpace::Global]),
            }),
        ),
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
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), u32_ty.clone()),
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(5)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::AcquireRelease,
                failure_ordering: None,
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::internal_helper(
        "preserved_memory_and_sync",
        Signature::new(
            vec![global_pointer, Type::BOOL, u32_ty, Type::INDEX],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut module = Module::new("tests::preserved_memory_and_sync");
    module.functions.push(function);
    module
}

fn preserved_matrix_module() -> Module {
    let mut parameters = vec![Type::Scalar(ScalarType::Bf16); 8];
    parameters.extend(vec![Type::F32; 4]);
    let ids = (0..12).map(ValueId).collect::<Vec<_>>();
    let matrix = MatrixOperation::multiply_accumulate(
        [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
        [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
    )
    .with_declared_tensor_layout(TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64());
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        (12..16)
            .map(|id| ValueDef::new(ValueId(id), Type::F32))
            .collect(),
        OperationKind::Matrix(matrix),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::internal_helper(
        "preserved_matrix",
        Signature::new(parameters, vec![]),
        ids,
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut module = Module::new("tests::preserved_matrix");
    module.functions.push(function);
    module
}

fn preserved_wave_and_transpose_module() -> Module {
    let storage_type = Type::pointer(
        Type::Scalar(ScalarType::U8),
        AddressSpace::Workgroup,
        AccessMode::ReadWrite,
    );
    let parameters = vec![
        Type::slice(
            Type::Scalar(ScalarType::U8),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::INDEX,
        Type::F32,
    ];
    let format = Gfx950LdsTransposeFormatV1::Fp8E4M3;
    let transpose = |result, kind| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), storage_type.clone()),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(kind)),
        )
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        transpose(8, Gfx950LdsTransposeOperationKindV1::Current { format }),
        transpose(
            9,
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage: ValueId(8),
                source_slice: ValueId(0),
                offset: ValueId(1),
                rows: ValueId(2),
                columns: ValueId(3),
                stride: ValueId(4),
                token_base: ValueId(5),
                reduction_base: ValueId(6),
            },
        ),
        transpose(
            10,
            Gfx950LdsTransposeOperationKindV1::Publish {
                format,
                storage: ValueId(9),
            },
        ),
        Operation::new(
            (11..19)
                .map(|id| ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)))
                .collect(),
            OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                Gfx950LdsTransposeOperationKindV1::Read {
                    format,
                    storage: ValueId(10),
                },
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(19), Type::F32),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ReduceF32 {
                    value: ValueId(7),
                    tile_width: 16,
                    kind: WaveF32ReductionKindV1::Maximum,
                },
                WaveWidth::Wave64,
            )),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "preserved_wave_and_transpose",
        Signature::new(parameters.clone(), vec![]),
        (0..parameters.len() as u32).map(ValueId).collect(),
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut kernel = Kernel::new(
        "preserved_wave_and_transpose_kernel",
        "preserved_wave_and_transpose",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tests::preserved_wave_and_transpose");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn preserved_inline_assembly_module() -> Module {
    let assembly = InlineAssembly {
        target: InlineAssemblyTarget::AmdGpuGfx942,
        source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        mnemonic: "v_add_u32".to_owned(),
        operands: vec![
            AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
            AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
            AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
        ],
        options: [
            AssemblyOption::NoMemory,
            AssemblyOption::Pure,
            AssemblyOption::NoStack,
        ]
        .into_iter()
        .collect(),
        declared_effects: Default::default(),
    };
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::InlineAssembly(assembly),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::preserved_inline_assembly");
    module.functions.push(Function::internal_helper(
        "preserved_inline_assembly",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

fn preserved_switch_module() -> Module {
    let mut legacy_entry = BasicBlock::new(BlockId(0));
    legacy_entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![SwitchCase {
            value: 7,
            target: BlockId(1),
            arguments: vec![],
        }],
        default_target: BlockId(2),
        default_arguments: vec![],
    });
    let mut legacy_case = BasicBlock::new(BlockId(1));
    legacy_case.terminator = Some(Terminator::Return { values: vec![] });
    let mut legacy_default = BasicBlock::new(BlockId(2));
    legacy_default.terminator = Some(Terminator::Return { values: vec![] });

    let mut integer_entry = BasicBlock::new(BlockId(10));
    integer_entry.terminator = Some(Terminator::IntegerSwitch {
        selector: ValueId(1),
        cases: vec![IntegerSwitchCase {
            value: Constant::I32(-7),
            target: BlockId(11),
            arguments: vec![],
        }],
        default_target: BlockId(12),
        default_arguments: vec![],
    });
    let mut integer_case = BasicBlock::new(BlockId(11));
    integer_case.terminator = Some(Terminator::Return { values: vec![] });
    let mut integer_default = BasicBlock::new(BlockId(12));
    integer_default.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("tests::preserved_switches");
    module.functions.push(Function::internal_helper(
        "legacy_switch",
        Signature::new(vec![Type::INDEX], vec![]),
        vec![ValueId(0)],
        vec![legacy_entry, legacy_case, legacy_default],
    ));
    module.functions.push(Function::internal_helper(
        "integer_switch",
        Signature::new(vec![Type::Scalar(ScalarType::I32)], vec![]),
        vec![ValueId(1)],
        vec![integer_entry, integer_case, integer_default],
    ));
    module
}

fn preserved_switch_cfg_rewrite_module() -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(3), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(6)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(3)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), u32_ty.clone()),
            OperationKind::Binary {
                op: fe2o3_kernel_ir::BinaryOp::BitAnd,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ),
    ];
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: vec![
            SwitchCase {
                value: 7,
                target: BlockId(10),
                arguments: vec![ValueId(5), ValueId(1)],
            },
            SwitchCase {
                value: 8,
                target: BlockId(10),
                arguments: vec![ValueId(2), ValueId(1)],
            },
        ],
        default_target: BlockId(20),
        default_arguments: vec![ValueId(1), ValueId(2)],
    });

    let mut repeated_target = BasicBlock::new(BlockId(10));
    repeated_target.parameters = vec![
        ValueDef::new(ValueId(10), u32_ty.clone()),
        ValueDef::new(ValueId(11), u32_ty.clone()),
    ];
    repeated_target.terminator = Some(Terminator::Return {
        values: vec![ValueId(10)],
    });

    let mut merge_predecessor = BasicBlock::new(BlockId(20));
    merge_predecessor.parameters = vec![
        ValueDef::new(ValueId(20), u32_ty.clone()),
        ValueDef::new(ValueId(21), u32_ty.clone()),
    ];
    merge_predecessor.terminator = Some(Terminator::Branch {
        target: BlockId(30),
        arguments: vec![ValueId(20), ValueId(21)],
    });

    let mut merge_successor = BasicBlock::new(BlockId(30));
    merge_successor.parameters = vec![
        ValueDef::new(ValueId(30), u32_ty.clone()),
        ValueDef::new(ValueId(31), u32_ty.clone()),
    ];
    merge_successor.terminator = Some(Terminator::Return {
        values: vec![ValueId(30)],
    });

    let mut unreachable = BasicBlock::new(BlockId(40));
    unreachable.parameters = vec![
        ValueDef::new(ValueId(40), u32_ty.clone()),
        ValueDef::new(ValueId(41), u32_ty.clone()),
    ];
    unreachable.terminator = Some(Terminator::Return {
        values: vec![ValueId(40)],
    });

    let mut module = Module::new("tests::preserved_switch_cfg_rewrite");
    module.functions.push(Function::internal_helper(
        "preserved_switch_cfg_rewrite",
        Signature::new(
            vec![Type::INDEX, u32_ty.clone(), u32_ty.clone()],
            vec![u32_ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![
            entry,
            repeated_target,
            merge_predecessor,
            merge_successor,
            unreachable,
        ],
    ));
    module
}

fn preserved_intrinsic_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::preserved_intrinsic");
    module.functions.push(Function::internal_helper(
        "intrinsic",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

fn preserved_memory_intrinsic_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
            pointer: ValueId(0),
            value: ValueId(1),
            element: MemoryElementType::Scalar(ScalarType::U32),
            address_space: AddressSpace::Global,
            layout: MemoryLayout::new(4, 4),
            contract: VolatileAccessContract::rust_allocation_store(),
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::memory_intrinsic");
    module.functions.push(Function::internal_helper(
        "memory_intrinsic",
        Signature::new(vec![pointer, scalar], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
}

fn preserved_unreachable_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Unreachable);
    let mut module = Module::new("tests::preserved_unreachable");
    module.functions.push(Function::internal_helper(
        "unreachable",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module
}

const V13_DYNAMIC_UPPER_BOUND: u64 = 257;

fn v13_identity(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
}

fn v13_provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn v13_dynamic_extent() -> ExecutionDynamicExtentV1 {
    ExecutionDynamicExtentV1 {
        operand: 2,
        source_argument: 2,
        source_type: v13_identity(9),
        value_type: ScalarType::Index,
        upper_bound: V13_DYNAMIC_UPPER_BOUND,
        bound_check_operand: 3,
        nonnegative_check_operand: None,
    }
}

fn v13_raw_bind_operation(extent: ExecutionDynamicExtentV1) -> ExecutionCapabilityOperationV1 {
    ExecutionCapabilityOperationV1::RawMemoryBind {
        authority: v13_identity(7),
        pointer: v13_identity(8),
        length: v13_identity(9),
        extent,
        view: v13_identity(10),
        element: v13_identity(11),
        layout: ExecutionElementLayoutV1 {
            byte_size: 4,
            byte_alignment: 4,
        },
        space: ExecutionMemoryAddressSpaceV1::Private,
        access: ExecutionMemoryAccessV1::ReadOnly,
        index_space: None,
        atomic_scope: None,
        unsafe_obligation: v13_identity(12),
    }
}

fn v13_raw_bind_result(extent: ExecutionDynamicExtentV1) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: v13_identity(10),
        provenance: v13_provenance(),
        workgroup_brand: None,
        epoch: None,
        role: ExecutionCapabilityRoleV1::MemoryView {
            element: v13_identity(11),
            layout: ExecutionElementLayoutV1 {
                byte_size: 4,
                byte_alignment: 4,
            },
            space: ExecutionMemoryAddressSpaceV1::Private,
            access: ExecutionMemoryAccessV1::ReadOnly,
            extent: ExecutionMemoryExtentV1::Dynamic(extent),
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            index_space: None,
            atomic_scope: None,
        },
    })
}

fn v13_raw_bind_module() -> Module {
    let extent = v13_dynamic_extent();
    let operation = v13_raw_bind_operation(extent);
    let requirements = operation.required_capabilities();
    let contract = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(2), ValueId(0), ValueId(1), ValueId(4)],
        operation,
        signature: ExecutionCapabilitySignatureV1::new(
            &[
                v13_identity(7),
                v13_identity(8),
                v13_identity(9),
                v13_identity(12),
            ],
            v13_identity(10),
        )
        .unwrap(),
        provenance: v13_provenance(),
        workgroup_brand: None,
        epoch_before: None,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &v13_raw_bind_operation(extent),
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [13; 32],
            operation: [14; 32],
            block: 0,
        },
    };
    let provenance = v13_provenance();
    let context = KernelContextTypeV1::new(
        "entry",
        provenance.kernel_marker,
        provenance.target_brand,
        provenance.launch_brand,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context,
            KernelContextSourceIdentityV1::new([21; 32], [22; 32], [23; 32], [24; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(V13_DYNAMIC_UPPER_BOUND)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(1),
                rhs: ValueId(3),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), v13_raw_bind_result(extent)),
            OperationKind::ExecutionCapability(contract),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    function.required_capabilities = requirements.clone();
    let mut module = Module::new("tests::execution_capability_v13");
    module.functions.push(function);
    module.required_capabilities = requirements;
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[cfg(feature = "internal-test-context-access")]
fn expected_ids(ids: &[&str]) -> Vec<String> {
    let mut ids = ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(feature = "internal-test-context-access")]
fn represented_safety_families(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(feature = "internal-test-context-access")]
fn assert_fresh_text_round_trip_v9(module: Module, ids: &[&str]) -> String {
    let input = VerifiedCanonicalKernelIrV9::from_module(module).expect("verified V9 fixture");
    let mut source = session();
    let graph = source.import_canonical_kir_v9_o0(&input).unwrap();
    assert_eq!(
        represented_safety_families(
            source
                .canonical_kir_safety_operation_ids_for_test(&graph)
                .unwrap(),
        ),
        expected_ids(ids)
    );
    let source_text = source.canonical_kir_text_for_test(&graph).unwrap();
    let mut repeated = session();
    let repeated_graph = repeated.import_canonical_kir_v9_o0(&input).unwrap();
    assert_eq!(
        repeated
            .canonical_kir_text_for_test(&repeated_graph)
            .unwrap(),
        source_text
    );

    let mut fresh = session();
    let fresh_graph = fresh
        .reparse_canonical_kir_text_for_test(&graph, &source_text)
        .expect("fresh V9 parse and recursive verification");
    assert_eq!(
        represented_safety_families(
            fresh
                .canonical_kir_safety_operation_ids_for_test(&fresh_graph)
                .unwrap(),
        ),
        expected_ids(ids)
    );
    let (output, report) = fresh.extract_canonical_kir_v9_o0(&fresh_graph).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
    source_text
}

#[cfg(feature = "internal-test-context-access")]
fn assert_fresh_text_round_trip_v10(module: Module, ids: &[&str]) {
    let input = VerifiedCanonicalKernelIrV10::from_module(module).expect("verified V10 fixture");
    let mut source = session();
    let graph = source.import_canonical_kir_v10_o0(&input).unwrap();
    assert_eq!(
        represented_safety_families(
            source
                .canonical_kir_safety_operation_ids_for_test(&graph)
                .unwrap(),
        ),
        expected_ids(ids)
    );
    let text = source.canonical_kir_text_for_test(&graph).unwrap();
    let mut repeated = session();
    let repeated_graph = repeated.import_canonical_kir_v10_o0(&input).unwrap();
    assert_eq!(
        repeated
            .canonical_kir_text_for_test(&repeated_graph)
            .unwrap(),
        text
    );
    let mut fresh = session();
    let fresh_graph = fresh
        .reparse_canonical_kir_text_for_test(&graph, &text)
        .expect("fresh V10 parse and recursive verification");
    assert_eq!(
        represented_safety_families(
            fresh
                .canonical_kir_safety_operation_ids_for_test(&fresh_graph)
                .unwrap(),
        ),
        expected_ids(ids)
    );
    let (output, report) = fresh.extract_canonical_kir_v10_o0(&fresh_graph).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
}

#[cfg(feature = "internal-test-context-access")]
fn assert_fresh_text_round_trip_v13(module: Module) {
    let input = VerifiedCanonicalKernelIrV13::from_module(module).expect("verified V13 fixture");
    let mut source = session();
    let graph = source.import_canonical_kir_v13_o0(&input).unwrap();
    let text = source.canonical_kir_text_for_test(&graph).unwrap();

    let mut repeated = session();
    let repeated_graph = repeated.import_canonical_kir_v13_o0(&input).unwrap();
    assert_eq!(
        repeated
            .canonical_kir_text_for_test(&repeated_graph)
            .unwrap(),
        text
    );

    let mut fresh = session();
    let fresh_graph = fresh
        .reparse_canonical_kir_text_for_test(&graph, &text)
        .expect("fresh V13 parse and recursive verification");
    let (output, report) = fresh.extract_canonical_kir_v13_o0(&fresh_graph).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
}

#[cfg(feature = "internal-test-context-access")]
fn operation_tree(root: Ptr<PlironOperation>, context: &Context) -> Vec<Ptr<PlironOperation>> {
    let mut pending = vec![root];
    let mut operations = Vec::new();
    while let Some(operation) = pending.pop() {
        operations.push(operation);
        let mut children = Vec::new();
        for region in operation.deref(context).regions() {
            for block in region.deref(context).iter(context) {
                children.extend(block.deref(context).iter(context));
            }
        }
        pending.extend(children.into_iter().rev());
    }
    operations
}

#[cfg(feature = "internal-test-context-access")]
fn typed_op<O: Op>(root: Ptr<PlironOperation>, context: &Context) -> O {
    let pointer = operation_tree(root, context)
        .into_iter()
        .find(|operation| PlironOperation::is_op::<O>(*operation, context))
        .expect("typed operation is present");
    PlironOperation::get_op::<O>(pointer, context).unwrap()
}

#[cfg(feature = "internal-test-context-access")]
fn typed_safety_op<O: Op>(root: Ptr<PlironOperation>, context: &Context) -> O {
    let operation = typed_op::<O>(root, context);
    let pointer = operation.get_operation();
    let operation = PlironOperation::get_op_dyn(pointer, context);
    let interface = op_cast::<dyn CanonicalKirSafetyOpInterface>(&*operation)
        .expect("capability operation is registered with the typed safety interface");
    assert!(interface.is_self_contained_canonical_kir());
    PlironOperation::get_op::<O>(pointer, context).unwrap()
}

#[cfg(feature = "internal-test-context-access")]
fn require_mutated_v9_rejection(mutation: impl FnOnce(&mut Context, Ptr<PlironOperation>)) {
    require_mutated_v9_module_rejection(preserved_memory_and_synchronization_module(), mutation);
}

#[cfg(feature = "internal-test-context-access")]
fn require_mutated_v9_module_rejection(
    module: Module,
    mutation: impl FnOnce(&mut Context, Ptr<PlironOperation>),
) {
    let input = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, mutation)
        .unwrap();
    assert_eq!(
        owner.extract_canonical_kir_v9_o0(&graph),
        Err(KirBridgeErrorV1::MalformedGraph)
    );
}

#[cfg(feature = "internal-test-context-access")]
fn require_mutated_v10_module_rejection(
    module: Module,
    mutation: impl FnOnce(&mut Context, Ptr<PlironOperation>),
) {
    let input = VerifiedCanonicalKernelIrV10::from_module(module).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v10_o0(&input).unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, mutation)
        .unwrap();
    assert_eq!(
        owner.extract_canonical_kir_v10_o0(&graph),
        Err(KirBridgeErrorV1::MalformedGraph)
    );
}

#[cfg(feature = "internal-test-context-access")]
fn require_execution_contract_mutation_rejection(
    mutation: impl FnOnce(&mut ExecutionCapabilityOpV1),
) {
    let input = VerifiedCanonicalKernelIrV13::from_module(v13_raw_bind_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v13_o0(&input).unwrap();
    let mut rejected_by_attribute = false;
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, |context, root| {
            let operation = typed_op::<ExecutionCapabilityOp>(root, context);
            let mut contract = operation.contract(context).unwrap();
            mutation(&mut contract);
            let Some(attribute) = ExecutionCapabilityContractAttr::new(&contract) else {
                rejected_by_attribute = true;
                return;
            };
            operation.set_attr_gpu_execution_capability_contract(context, attribute);
        })
        .unwrap();
    if rejected_by_attribute {
        return;
    }
    assert_eq!(
        owner.extract_canonical_kir_v13_o0(&graph),
        Err(KirBridgeErrorV1::MalformedGraph)
    );
}

#[cfg(feature = "internal-test-context-access")]
fn require_metadata_substitution_rejection(mutation: impl FnOnce(&mut Module)) {
    let original = VerifiedCanonicalKernelIrV13::from_module(v13_raw_bind_module()).unwrap();
    let mut original_owner = session();
    let original_graph = original_owner
        .import_canonical_kir_v13_o0(&original)
        .unwrap();
    let original_text = original_owner
        .canonical_kir_text_for_test(&original_graph)
        .unwrap();

    let mut substituted_module = v13_raw_bind_module();
    mutation(&mut substituted_module);
    let substituted = VerifiedCanonicalKernelIrV13::from_module(substituted_module)
        .expect("one-axis metadata substitution remains canonical KIR");
    let mut template_owner = session();
    let substituted_template = template_owner
        .import_canonical_kir_v13_o0(&substituted)
        .unwrap();

    let mut fresh = session();
    let substituted_graph =
        match fresh.reparse_canonical_kir_text_for_test(&substituted_template, &original_text) {
            Ok(graph) => graph,
            Err(KirBridgeErrorV1::Session(
                fe2o3_pliron::OperationHandleError::OperationVerificationRejected,
            )) => return,
            Err(error) => panic!("unexpected metadata-substitution failure: {error:?}"),
        };
    assert_eq!(
        fresh.extract_canonical_kir_v13_o0(&substituted_graph),
        Err(KirBridgeErrorV1::MalformedGraph)
    );
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn every_canonical_safety_carrier_round_trips_through_fresh_context_text() {
    assert_fresh_text_round_trip_v9(preserved_intrinsic_module(), &["gpu.kir_intrinsic"]);
    assert_fresh_text_round_trip_v10(
        preserved_memory_intrinsic_module(),
        &["gpu.kir_memory_intrinsic"],
    );
    assert_fresh_text_round_trip_v9(
        preserved_memory_and_synchronization_module(),
        &[
            "gpu.kir_alloca",
            "gpu.kir_atomic",
            "gpu.kir_barrier",
            "gpu.kir_fence",
            "gpu.kir_guarded_load",
            "gpu.kir_guarded_store",
            "gpu.kir_workgroup_barrier",
            "gpu.kir_workgroup_memory",
        ],
    );
    assert_fresh_text_round_trip_v9(preserved_matrix_module(), &["gpu.kir_matrix"]);
    assert_fresh_text_round_trip_v9(
        preserved_wave_and_transpose_module(),
        &["gpu.kir_gfx950_lds_transpose", "gpu.kir_wave"],
    );
    assert_fresh_text_round_trip_v9(
        preserved_inline_assembly_module(),
        &["gpu.kir_inline_assembly"],
    );
    assert_fresh_text_round_trip_v9(
        preserved_switch_module(),
        &["gpu.kir_integer_switch", "gpu.kir_switch"],
    );
    assert_fresh_text_round_trip_v9(preserved_unreachable_module(), &["gpu.kir_unreachable"]);
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn execution_capability_graph_round_trips_through_fresh_context_text() {
    assert_fresh_text_round_trip_v13(v13_raw_bind_module());
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn typed_capability_graph_rejects_every_required_one_axis_mutation() {
    require_execution_contract_mutation_rejection(|contract| {
        let ExecutionCapabilityOperationV1::RawMemoryBind { space, .. } = &mut contract.operation
        else {
            unreachable!()
        };
        *space = ExecutionMemoryAddressSpaceV1::Global;
        contract.workgroup_brand = Some([41; 32]);
        contract.epoch_before = Some([42; 32]);
    });
    require_execution_contract_mutation_rejection(|contract| {
        let ExecutionCapabilityOperationV1::RawMemoryBind { access, .. } = &mut contract.operation
        else {
            unreachable!()
        };
        *access = ExecutionMemoryAccessV1::ExclusiveReadWrite;
    });
    require_execution_contract_mutation_rejection(|contract| {
        let ExecutionCapabilityOperationV1::RawMemoryBind { extent, .. } = &mut contract.operation
        else {
            unreachable!()
        };
        extent.upper_bound += 1;
    });
    require_execution_contract_mutation_rejection(|contract| {
        contract.provenance.target_brand = [43; 32];
    });

    require_mutated_v9_rejection(|context, root| {
        let barrier = typed_safety_op::<PlironBarrierOp>(root, context);
        let mut contract = barrier.contract(context).unwrap();
        let OperationKind::Barrier(payload) = &mut contract.kind else {
            unreachable!()
        };
        payload.execution_scope = SynchronizationScope::Subgroup;
        barrier.set_attr_gpu_kir_barrier_contract(
            context,
            CanonicalKirOperationAttr::new(&contract).unwrap(),
        );
    });
    require_mutated_v9_rejection(|context, root| {
        let atomic = typed_safety_op::<PlironAtomicOp>(root, context);
        let mut contract = atomic.contract(context).unwrap();
        let OperationKind::Atomic(payload) = &mut contract.kind else {
            unreachable!()
        };
        payload.ordering = MemoryOrdering::SequentiallyConsistent;
        atomic.set_attr_gpu_kir_atomic_contract(
            context,
            CanonicalKirOperationAttr::new(&contract).unwrap(),
        );
    });
    require_mutated_v9_rejection(|context, root| {
        let atomic = typed_safety_op::<PlironAtomicOp>(root, context);
        let mut contract = atomic.contract(context).unwrap();
        let OperationKind::Atomic(payload) = &mut contract.kind else {
            unreachable!()
        };
        payload.scope = SynchronizationScope::System;
        atomic.set_attr_gpu_kir_atomic_contract(
            context,
            CanonicalKirOperationAttr::new(&contract).unwrap(),
        );
    });
    require_mutated_v9_rejection(|context, root| {
        let barrier = typed_safety_op::<PlironBarrierOp>(root, context);
        barrier.set_attr_gpu_kir_barrier_identity(
            context,
            CanonicalIdentityAttr::from_bytes([44; 32]),
        );
    });
    require_mutated_v9_rejection(|context, root| {
        let barrier = typed_safety_op::<PlironBarrierOp>(root, context);
        let memory = typed_safety_op::<PlironWorkgroupMemoryOp>(root, context);
        let barrier = barrier.get_operation();
        barrier.unlink(context);
        barrier.insert_after(context, memory.get_operation());
    });
    require_mutated_v9_rejection(|context, root| {
        let barrier = typed_safety_op::<PlironBarrierOp>(root, context);
        barrier.set_attr_gpu_kir_barrier_graph_epoch(
            context,
            CanonicalIdentityAttr::from_bytes([45; 32]),
        );
    });

    require_metadata_substitution_rejection(|module| {
        module
            .required_capabilities
            .insert(TargetCapability::Float64);
    });
    require_metadata_substitution_rejection(|module| {
        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(V13_DYNAMIC_UPPER_BOUND as u32),
        };
    });

    let input = VerifiedCanonicalKernelIrV13::from_module(v13_raw_bind_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v13_o0(&input).unwrap();
    owner
        .with_canonical_kir_graph_mut_for_test(&graph, |context, root| {
            let operation = typed_op::<ExecutionCapabilityOp>(root, context);
            operation.set_attr_gpu_execution_capability_source_operation_identity(
                context,
                CanonicalIdentityAttr::from_bytes([46; 32]),
            );
        })
        .unwrap();
    assert_eq!(
        owner.extract_canonical_kir_v13_o0(&graph),
        Err(KirBridgeErrorV1::MalformedGraph)
    );
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn every_operation_family_rejects_an_identity_substitution() {
    macro_rules! reject_v9_identity_substitution {
        ($module:expr, $op:ty, $setter:ident) => {
            require_mutated_v9_module_rejection($module, |context, root| {
                typed_safety_op::<$op>(root, context)
                    .$setter(context, CanonicalIdentityAttr::from_bytes([0xfe; 32]));
            });
        };
    }

    reject_v9_identity_substitution!(
        preserved_intrinsic_module(),
        PlironIntrinsicOp,
        set_attr_gpu_kir_intrinsic_identity
    );
    require_mutated_v10_module_rejection(preserved_memory_intrinsic_module(), |context, root| {
        typed_safety_op::<PlironMemoryIntrinsicOp>(root, context)
            .set_attr_gpu_kir_memory_intrinsic_identity(
                context,
                CanonicalIdentityAttr::from_bytes([0xfe; 32]),
            );
    });
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironAllocaOp,
        set_attr_gpu_kir_alloca_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironGuardedLoadOp,
        set_attr_gpu_kir_guarded_load_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironGuardedStoreOp,
        set_attr_gpu_kir_guarded_store_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironBarrierOp,
        set_attr_gpu_kir_barrier_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironAtomicOp,
        set_attr_gpu_kir_atomic_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironFenceOp,
        set_attr_gpu_kir_fence_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironWorkgroupBarrierOp,
        set_attr_gpu_kir_workgroup_barrier_identity
    );
    reject_v9_identity_substitution!(
        preserved_memory_and_synchronization_module(),
        PlironWorkgroupMemoryOp,
        set_attr_gpu_kir_workgroup_memory_identity
    );
    reject_v9_identity_substitution!(
        preserved_matrix_module(),
        PlironMatrixOp,
        set_attr_gpu_kir_matrix_identity
    );
    reject_v9_identity_substitution!(
        preserved_wave_and_transpose_module(),
        PlironGfx950LdsTransposeOp,
        set_attr_gpu_kir_gfx950_lds_transpose_identity
    );
    reject_v9_identity_substitution!(
        preserved_wave_and_transpose_module(),
        PlironWaveOp,
        set_attr_gpu_kir_wave_identity
    );
    reject_v9_identity_substitution!(
        preserved_inline_assembly_module(),
        PlironInlineAssemblyOp,
        set_attr_gpu_kir_inline_assembly_identity
    );
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn canonical_kir_text_process_probe() {
    const ENV: &str = "FE2O3_CANONICAL_KIR_TEXT_PROCESS_PROBE_V1";
    if std::env::var_os(ENV).is_none() {
        return;
    }
    use sha2::{Digest, Sha256};

    let text = assert_fresh_text_round_trip_v9(
        preserved_memory_and_synchronization_module(),
        &[
            "gpu.kir_alloca",
            "gpu.kir_atomic",
            "gpu.kir_barrier",
            "gpu.kir_fence",
            "gpu.kir_guarded_load",
            "gpu.kir_guarded_store",
            "gpu.kir_workgroup_barrier",
            "gpu.kir_workgroup_memory",
        ],
    );
    let digest: [u8; 32] = Sha256::digest(text.as_bytes()).into();
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!("FE2O3_CANONICAL_KIR_TEXT_DIGEST_V1={digest}");
}

#[cfg(feature = "internal-test-context-access")]
#[test]
fn canonical_kir_text_is_stable_across_fresh_processes() {
    const ENV: &str = "FE2O3_CANONICAL_KIR_TEXT_PROCESS_PROBE_V1";
    const PREFIX: &str = "FE2O3_CANONICAL_KIR_TEXT_DIGEST_V1=";
    let run = || {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "canonical_kir_text_process_probe", "--nocapture"])
            .env(ENV, "1")
            .output()
            .expect("spawn current conformance test process");
        assert!(
            output.status.success(),
            "child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix(PREFIX).map(str::to_owned))
            .expect("child emitted canonical text digest")
    };

    assert_eq!(run(), run());
}

#[test]
fn typed_o0_round_trip_is_exact_and_has_stable_correspondence() {
    let input = VerifiedCanonicalKernelIrV9::from_module(rich_supported_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let (output, report) = owner.extract_canonical_kir_v9_o0(&graph).unwrap();

    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
    assert_eq!(report.input(), report.output());
    assert_eq!(graph.input(), report.input());
    assert_eq!(graph.correspondence(), report.correspondence());
    assert!(
        report.correspondence().windows(2).all(|pair| {
            pair[0].pliron_ordinal().checked_add(1) == Some(pair[1].pliron_ordinal())
        })
    );
    assert!(report.correspondence().iter().any(|record| {
        record.coordinate()
            == KirBridgeCoordinateV1::Operation {
                function: 1,
                block: 0,
                operation: 5,
            }
    }));
}

#[test]
fn accepted_graph_reconstructs_exactly_in_a_fresh_context() {
    let input = VerifiedCanonicalKernelIrV9::from_module(rich_supported_module()).unwrap();
    let canonical_bytes = {
        let mut first = session();
        let graph = first.import_canonical_kir_v9_o0(&input).unwrap();
        first
            .extract_canonical_kir_v9_o0(&graph)
            .unwrap()
            .0
            .canonical_bytes()
            .to_vec()
    };

    let decoded = decode_module_v9(&canonical_bytes).expect("fresh canonical decode");
    let reencoded =
        VerifiedCanonicalKernelIrV9::from_module(decoded).expect("fresh canonical encode");
    let mut fresh = session();
    let graph = fresh.import_canonical_kir_v9_o0(&reencoded).unwrap();
    let (output, report) = fresh.extract_canonical_kir_v9_o0(&graph).unwrap();

    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
}

#[test]
fn safety_significant_operation_families_survive_the_standard_pipeline() {
    for module in [
        preserved_memory_and_synchronization_module(),
        preserved_matrix_module(),
        preserved_wave_and_transpose_module(),
        preserved_inline_assembly_module(),
    ] {
        assert_exact_through_standard_optimization(module);
    }
}

#[test]
fn switch_terminators_round_trip_exactly() {
    assert_exact_through_standard_optimization(preserved_switch_module());
}

#[test]
fn switch_cfg_operands_and_repeated_successors_round_trip_exactly() {
    let input = VerifiedCanonicalKernelIrV9::from_module(preserved_switch_cfg_rewrite_module())
        .expect("valid switch rewrite fixture");
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let (output, report) = owner.extract_canonical_kir_v9_o0(&graph).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());

    owner
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    let (optimized, receipt) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    assert!(receipt.changed());
    let optimized = decode_module_v9(optimized.canonical_bytes()).unwrap();
    verify_module(&optimized).unwrap();
    let entry = optimized.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|block| block.id == BlockId(0))
        .unwrap();
    let Terminator::Switch {
        cases,
        default_arguments,
        ..
    } = entry.terminator.as_ref().unwrap()
    else {
        panic!("typed switch was omitted by optimization");
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].target, cases[1].target);
    assert_eq!(cases[0].arguments.len(), 2);
    assert_eq!(cases[1].arguments.len(), 2);
    assert_eq!(default_arguments.len(), 2);
}

#[test]
fn optimized_export_remaps_rewritten_atomic_operands() {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let pointer_ty = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(6)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(3)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), u32_ty.clone()),
            OperationKind::Binary {
                op: fe2o3_kernel_ir::BinaryOp::BitAnd,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), u32_ty.clone()),
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(3)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::internal_helper(
        "preserved_operand_remap",
        Signature::new(vec![pointer_ty], vec![]),
        vec![ValueId(0)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut module = Module::new("tests::preserved_operand_remap");
    module.functions.push(function);

    let input = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    owner
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    let (optimized, receipt) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    assert!(receipt.changed());
    let optimized = decode_module_v9(optimized.canonical_bytes()).unwrap();
    verify_module(&optimized).unwrap();
    let operations = &optimized.functions[0].body.as_ref().unwrap().blocks[0].operations;
    let atomic = operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::Atomic(atomic) => Some(atomic),
            _ => None,
        })
        .expect("typed atomic survives optimization");
    let value = atomic.value.expect("atomic add operand survives");
    assert!(operations.iter().any(|operation| {
        operation.results.iter().any(|result| result.id == value)
            && matches!(operation.kind, OperationKind::Constant(Constant::U32(2)))
    }));
}

#[test]
fn typed_o0_round_trip_preserves_every_scalar_constant_bit_pattern() {
    let input = VerifiedCanonicalKernelIrV9::from_module(all_scalar_constants_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let (output, report) = owner.extract_canonical_kir_v9_o0(&graph).unwrap();

    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
}

#[test]
fn import_and_optimized_export_accept_non_dominance_physical_block_order() {
    let input =
        VerifiedCanonicalKernelIrV9::from_module(non_dominance_physical_order_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();

    let (o0_output, report) = owner.extract_canonical_kir_v9_o0(&graph).unwrap();
    assert_eq!(o0_output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());

    let (optimized_output, receipt) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    assert_eq!(optimized_output.canonical_bytes(), input.canonical_bytes());
    assert!(!receipt.changed());
    assert_eq!(
        receipt.correspondence_digest(),
        report.correspondence_digest()
    );
    assert_eq!(
        receipt.correspondence_digest().count(),
        receipt.correspondence().len() as u64
    );
}

#[test]
fn optimized_noop_export_retains_exact_identity_and_memory_semantics() {
    let input = VerifiedCanonicalKernelIrV9::from_module(rich_supported_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let (output, receipt) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();

    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(!receipt.changed());
    assert_eq!(receipt.input(), receipt.output());
}

#[test]
fn optimized_export_accepts_an_unreachable_block_removed_by_simplify_cfg() {
    let input = VerifiedCanonicalKernelIrV9::from_module(rich_supported_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let before_blocks = rich_supported_module().functions[1]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .len();

    let optimization = owner
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    assert!(optimization.passes().iter().any(|pass| pass.changed()));
    let (output, receipt) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    let output_module = fe2o3_kernel_ir::decode_module_v9(output.canonical_bytes()).unwrap();

    assert!(receipt.changed());
    assert!(
        output_module.functions[1]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .len()
            < before_blocks
    );
}

#[test]
fn bridge_rejects_foreign_sessions_unreachable_and_preserves_generic_types() {
    let input = VerifiedCanonicalKernelIrV9::from_module(rich_supported_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let mut foreign = session();
    assert_eq!(
        foreign.extract_canonical_kir_v9_o0(&graph),
        Err(KirBridgeErrorV1::GraphIdentityMismatch)
    );

    let mut unreachable = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    unreachable.terminator = Some(Terminator::Unreachable);
    let mut unreachable_module = Module::new("tests::unreachable_terminator");
    unreachable_module.functions.push(Function::internal_helper(
        "unreachable",
        Signature::new(vec![], vec![]),
        vec![],
        vec![unreachable],
    ));
    let unreachable_input = VerifiedCanonicalKernelIrV9::from_module(unreachable_module).unwrap();
    let unreachable_graph = owner
        .import_canonical_kir_v9_o0(&unreachable_input)
        .unwrap();
    let (unreachable_output, report) = owner
        .extract_canonical_kir_v9_o0(&unreachable_graph)
        .unwrap();
    assert_eq!(
        unreachable_output.canonical_bytes(),
        unreachable_input.canonical_bytes()
    );
    assert!(report.is_exact());

    let mut generic = Module::new("tests::generic_address_space");
    let generic_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let generic_slice = Type::slice(
        Type::Scalar(ScalarType::U16),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    );
    let mut generic_body = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    generic_body.terminator = Some(Terminator::Return { values: vec![] });
    generic.functions.push(Function::internal_helper(
        "generic",
        Signature::new(vec![generic_pointer, generic_slice], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![generic_body],
    ));
    let generic_input = VerifiedCanonicalKernelIrV9::from_module(generic).unwrap();
    let generic_graph = owner.import_canonical_kir_v9_o0(&generic_input).unwrap();
    let (generic_output, report) = owner.extract_canonical_kir_v9_o0(&generic_graph).unwrap();
    assert_eq!(
        generic_output.canonical_bytes(),
        generic_input.canonical_bytes()
    );
    assert!(report.is_exact());
}

#[test]
fn intrinsic_is_exact_and_survives_optimization() {
    let input = VerifiedCanonicalKernelIrV9::from_module(preserved_intrinsic_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    let (output, report) = owner.extract_canonical_kir_v9_o0(&graph).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
    owner
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    let (optimized, _) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    assert_eq!(optimized.canonical_bytes(), input.canonical_bytes());
}

#[test]
fn memory_intrinsic_has_self_contained_semantics_and_round_trips_exactly() {
    let input =
        VerifiedCanonicalKernelIrV10::from_module(preserved_memory_intrinsic_module()).unwrap();
    let mut owner = session();
    let graph = owner.import_canonical_kir_v10_o0(&input).unwrap();
    let (output, report) = owner.extract_canonical_kir_v10_o0(&graph).unwrap();
    assert_eq!(output.canonical_bytes(), input.canonical_bytes());
    assert!(report.is_exact());
}
