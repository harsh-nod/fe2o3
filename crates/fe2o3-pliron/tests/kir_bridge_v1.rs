use std::collections::BTreeSet;

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, CastKind, CheckedBinaryOperator, ComparePredicate,
    Constant, Function, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, UnaryOp, ValueDef, ValueId, VerifiedCanonicalKernelIrV9,
};
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
fn bridge_rejects_foreign_sessions_and_unsupported_semantics() {
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
    let mut unsupported = Module::new("tests::unsupported_terminator");
    unsupported.functions.push(Function::internal_helper(
        "unsupported",
        Signature::new(vec![], vec![]),
        vec![],
        vec![unreachable],
    ));
    let unsupported = VerifiedCanonicalKernelIrV9::from_module(unsupported).unwrap();
    let unsupported_error = match owner.import_canonical_kir_v9_o0(&unsupported) {
        Err(error) => error,
        Ok(_) => panic!("unsupported terminator was imported"),
    };
    assert_eq!(
        unsupported_error,
        KirBridgeErrorV1::UnsupportedTerminator {
            coordinate: KirBridgeCoordinateV1::Terminator {
                function: 0,
                block: 0,
            },
        }
    );

    let mut generic = Module::new("tests::unsupported_generic");
    generic.functions.push(Function::external_import(
        "generic",
        Signature::new(
            vec![Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Generic,
                AccessMode::ReadOnly,
            )],
            vec![],
        ),
    ));
    let generic = VerifiedCanonicalKernelIrV9::from_module(generic).unwrap();
    let generic_error = match owner.import_canonical_kir_v9_o0(&generic) {
        Err(error) => error,
        Ok(_) => panic!("generic address space was imported"),
    };
    assert_eq!(
        generic_error,
        KirBridgeErrorV1::UnsupportedGenericAddressSpace
    );
}

#[test]
fn unsupported_operation_fails_before_allocating_a_graph() {
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(0), Type::INDEX),
        OperationKind::Intrinsic(fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("tests::unsupported_intrinsic");
    module.functions.push(Function::internal_helper(
        "unsupported",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.required_capabilities = BTreeSet::new();
    let input = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    let mut owner = session();

    let error = match owner.import_canonical_kir_v9_o0(&input) {
        Err(error) => error,
        Ok(_) => panic!("unsupported intrinsic was imported"),
    };
    assert_eq!(
        error,
        KirBridgeErrorV1::UnsupportedOperation {
            coordinate: KirBridgeCoordinateV1::Operation {
                function: 0,
                block: 0,
                operation: 0,
            },
        }
    );
    assert!(!owner.is_poisoned());
}
