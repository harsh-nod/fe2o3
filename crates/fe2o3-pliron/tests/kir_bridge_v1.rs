use fe2o3_kernel_ir::*;
use fe2o3_pliron::{
    KirBridgeCoordinateV1, KirBridgeErrorV1, PlironOptimizationPlanV1, PlironSession, ShellLimits,
};

fn session() -> PlironSession {
    PlironSession::new(
        ShellLimits::default(),
        [dialect_gpu::dialect_registration().expect("valid gpu registration")],
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
fn every_preserved_operation_family_round_trips_through_the_standard_pipeline() {
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
fn both_switch_forms_round_trip_with_real_cfg_successors() {
    assert_exact_through_standard_optimization(preserved_switch_module());
}

#[test]
fn preserved_switch_export_tracks_dead_arguments_repeated_edges_and_block_merging() {
    let input = VerifiedCanonicalKernelIrV9::from_module(preserved_switch_cfg_rewrite_module())
        .expect("valid switch rewrite fixture");
    let mut owner = session();
    let graph = owner.import_canonical_kir_v9_o0(&input).unwrap();
    owner
        .execute_optimization_v1(graph.root(), &PlironOptimizationPlanV1::standard())
        .unwrap();
    let (optimized, receipt) = owner.extract_optimized_canonical_kir_v9_v1(&graph).unwrap();
    assert!(receipt.changed());

    let optimized = decode_module_v9(optimized.canonical_bytes()).unwrap();
    verify_module(&optimized).expect("rewritten switch CFG remains verified Kernel IR");
    let body = optimized.functions[0].body.as_ref().unwrap();
    assert_eq!(
        body.blocks.len(),
        3,
        "one block merged and one removed as unreachable"
    );
    assert!(body.blocks.iter().all(|block| block.id != BlockId(30)));
    assert!(body.blocks.iter().all(|block| block.id != BlockId(40)));

    let entry = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(0))
        .unwrap();
    let Terminator::Switch {
        cases,
        default_target,
        default_arguments,
        ..
    } = entry.terminator.as_ref().unwrap()
    else {
        panic!("entry switch must remain a typed preserved terminator");
    };
    assert_eq!(cases.len(), 2);
    assert_eq!(cases[0].target, BlockId(10));
    assert_eq!(cases[1].target, BlockId(10));
    assert_eq!(cases[0].arguments.len(), 2);
    assert_eq!(cases[1].arguments, vec![ValueId(2), ValueId(1)]);
    assert_eq!(*default_target, BlockId(20));
    assert_eq!(default_arguments.len(), 2);

    let folded_value = cases[0].arguments[0];
    assert_eq!(cases[0].arguments[1], ValueId(1));
    assert_eq!(default_arguments, &[ValueId(1), ValueId(2)]);
    assert!(entry.operations.iter().any(|operation| {
        operation
            .results
            .iter()
            .any(|result| result.id == folded_value)
            && matches!(operation.kind, OperationKind::Constant(Constant::U32(2)))
    }));

    let repeated_target = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(10))
        .unwrap();
    // Builtin FuncOp retains non-entry arguments, so both repeated edges must
    // keep the unused second slot aligned while other CFG nodes are rewritten.
    assert_eq!(repeated_target.parameters.len(), 2);
    let merge_predecessor = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(20))
        .unwrap();
    assert_eq!(merge_predecessor.parameters.len(), 2);
    let merged_live_parameter = merge_predecessor.parameters[0].id;
    assert!(
        matches!(
            merge_predecessor.terminator,
            Some(Terminator::Return { ref values }) if values == &[merged_live_parameter]
        ),
        "unexpected merged block: {:?}",
        merge_predecessor
    );
}

#[test]
fn optimized_export_remaps_rewritten_operands_inside_preserved_payloads() {
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
        .expect("preserved atomic survives");
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
fn bridge_rejects_foreign_sessions_and_preserves_unreachable_and_generic_types() {
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
fn intrinsic_carrier_is_exact_and_survives_optimization() {
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::INDEX),
        OperationKind::Intrinsic(fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::preserved_intrinsic");
    module.functions.push(Function::internal_helper(
        "intrinsic",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let input = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
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
