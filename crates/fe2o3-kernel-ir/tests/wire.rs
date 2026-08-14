use std::collections::BTreeSet;
use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_kernel_ir::*;

const HEADER_BYTES: usize = 20;
const GOLDEN_HEX: &str = include_str!("fixtures/full_v1.hex");

fn all_scalar_types() -> Vec<Type> {
    vec![
        ScalarType::Bool,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::Index,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ]
    .into_iter()
    .map(Type::Scalar)
    .collect()
}

#[test]
fn wide_scalar_types_are_additive_v4_only() {
    let mut module = Module::new("wide-scalars");
    module.functions.push(Function::declaration(
        "wide",
        Signature::new(
            vec![Type::Scalar(ScalarType::I128)],
            vec![Type::Scalar(ScalarType::U128)],
        ),
    ));

    for (version, encoded) in [
        (KERNEL_IR_VERSION_V1, encode_module_v1(&module)),
        (KERNEL_IR_VERSION_V2, encode_module_v2(&module)),
        (KERNEL_IR_VERSION_V3, encode_module_v3(&module)),
    ] {
        assert_eq!(
            encoded,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "128-bit scalar types",
            })
        );
    }

    let encoded = encode_module_v4(&module).unwrap();
    assert_eq!(encoded[8..10], KERNEL_IR_VERSION_V4.to_le_bytes());
    assert_eq!(decode_module_v4(&encoded).unwrap(), module);
    for rejected in [
        decode_module_v1(&encoded),
        decode_module_v2(&encoded),
        decode_module_v3(&encoded),
    ] {
        assert_eq!(rejected, Err(KernelIrDecodeError::UnknownVersion(4)));
    }

    let mut forged_v3 = encoded;
    forged_v3[8..10].copy_from_slice(&KERNEL_IR_VERSION_V3.to_le_bytes());
    assert!(matches!(
        decode_module_v3(&forged_v3),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "scalar type",
            tag: 15 | 16,
        })
    ));
}

fn all_capabilities() -> BTreeSet<TargetCapability> {
    [
        TargetCapability::Float16,
        TargetCapability::BFloat16,
        TargetCapability::Float64,
        TargetCapability::Int64,
        TargetCapability::Subgroups,
        TargetCapability::SubgroupSize(32),
        TargetCapability::WorkgroupMemory,
        TargetCapability::WorkgroupBarrier,
        TargetCapability::Atomic {
            width_bits: 64,
            address_space: AddressSpace::Global,
            max_scope: SynchronizationScope::System,
        },
        TargetCapability::DynamicWorkgroupMemory,
        TargetCapability::Extension {
            namespace: "org.fe2o3.tests".into(),
            name: "future-capability".into(),
        },
    ]
    .into_iter()
    .collect()
}

#[test]
fn exact_gfx942_xnack_minus_binding_round_trips_in_every_kernel_ir_wire_version() {
    let mut module = Module::new("exact-gfx942-target-binding");
    module
        .required_capabilities
        .insert(gfx942_xnack_minus_target_capability());

    for (encode, decode) in [
        (
            encode_module_v1 as fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>,
            decode_module_v1 as fn(&[u8]) -> Result<Module, KernelIrDecodeError>,
        ),
        (encode_module_v2, decode_module_v2),
        (encode_module_v3, decode_module_v3),
        (encode_module_v4, decode_module_v4),
    ] {
        let encoded = encode(&module).expect("exact target binding encodes");
        let decoded = decode(&encoded).expect("exact target binding decodes");
        assert_eq!(decoded, module);
        assert_eq!(
            decoded.required_capabilities,
            BTreeSet::from([gfx942_xnack_minus_target_capability()])
        );
    }
}

#[test]
fn matrix_source_observation_and_projection_digests_round_trip_in_every_wire_version() {
    fn bytes(record: &mut Vec<u8>, value: &[u8]) {
        record.extend_from_slice(&(value.len() as u32).to_le_bytes());
        record.extend_from_slice(value);
    }

    let mut module = Module::new("matrix-source-and-projection-digests");
    let provider = MatrixProviderIdentityV2 {
        crate_name: "fe2o3_device".to_owned(),
        stable_crate_id: 1,
        crate_hash: [2; 16],
        cargo_metadata_build_observation: [3; 32],
        source_identity: [4; 32],
        definition_identities: vec![[5; 16]; 6],
    };
    let mut record = MATRIX_SOURCE_ABI_RECORD_DOMAIN_V2.to_vec();
    bytes(&mut record, provider.crate_name.as_bytes());
    record.extend_from_slice(&provider.stable_crate_id.to_le_bytes());
    bytes(&mut record, &provider.crate_hash);
    bytes(&mut record, &provider.cargo_metadata_build_observation);
    bytes(&mut record, &provider.source_identity);
    record.extend_from_slice(&(provider.definition_identities.len() as u32).to_le_bytes());
    for identity in &provider.definition_identities {
        bytes(&mut record, identity);
    }
    record.push(0);
    let observation = MatrixSourceAbiObservationV2::new_untrusted_claim(provider, record).unwrap();
    module.required_capabilities.extend([
        observation.capability(),
        MatrixProjectedKernargPolicyV1::canonical().capability(),
    ]);

    for (encode, decode) in [
        (
            encode_module_v1 as fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>,
            decode_module_v1 as fn(&[u8]) -> Result<Module, KernelIrDecodeError>,
        ),
        (encode_module_v2, decode_module_v2),
        (encode_module_v3, decode_module_v3),
        (encode_module_v4, decode_module_v4),
    ] {
        let encoded = encode(&module).expect("matrix ABI digest encodes");
        let decoded = decode(&encoded).expect("matrix ABI digest decodes");
        assert_eq!(decoded, module);
    }
}

fn all_operations() -> Vec<Operation> {
    let constants = vec![
        Constant::Bool(false),
        Constant::Bool(true),
        Constant::I8(-8),
        Constant::I16(-16),
        Constant::I32(-32),
        Constant::I64(-64),
        Constant::U8(8),
        Constant::U16(16),
        Constant::U32(32),
        Constant::U64(64),
        Constant::Index(u64::MAX),
        Constant::F16Bits(0x3c00),
        Constant::Bf16Bits(0x3f80),
        Constant::F32Bits(0x7fc0_1234),
        Constant::F64Bits(0xfff0_0000_0000_0000),
    ];
    let mut kinds = constants
        .into_iter()
        .map(OperationKind::Constant)
        .collect::<Vec<_>>();

    for (kind, axis) in [
        (IndexKind::Global, Axis::X),
        (IndexKind::Workgroup, Axis::Y),
        (IndexKind::Local, Axis::Z),
        (IndexKind::WorkgroupSize, Axis::X),
        (IndexKind::WorkgroupCount, Axis::Y),
    ] {
        kinds.push(OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::InvocationIndex { kind, axis },
            Type::INDEX,
        )));
    }
    kinds.extend([Axis::X, Axis::Y, Axis::Z].into_iter().map(|axis| {
        OperationKind::Intrinsic(IntrinsicOperation::new(
            IntrinsicKind::LaunchExtent { axis },
            Type::INDEX,
        ))
    }));

    kinds.extend(
        [UnaryOp::Negate, UnaryOp::Not]
            .into_iter()
            .map(|op| OperationKind::Unary {
                op,
                operand: ValueId(1),
            }),
    );
    kinds.extend(
        [
            BinaryOp::Add,
            BinaryOp::Subtract,
            BinaryOp::Multiply,
            BinaryOp::Divide,
            BinaryOp::Remainder,
            BinaryOp::BitAnd,
            BinaryOp::BitOr,
            BinaryOp::BitXor,
            BinaryOp::ShiftLeft,
            BinaryOp::ShiftRight,
        ]
        .into_iter()
        .map(|op| OperationKind::Binary {
            op,
            lhs: ValueId(2),
            rhs: ValueId(3),
        }),
    );
    kinds.extend(
        [
            ComparePredicate::Equal,
            ComparePredicate::NotEqual,
            ComparePredicate::LessThan,
            ComparePredicate::LessThanOrEqual,
            ComparePredicate::GreaterThan,
            ComparePredicate::GreaterThanOrEqual,
        ]
        .into_iter()
        .map(|predicate| OperationKind::Compare {
            predicate,
            lhs: ValueId(4),
            rhs: ValueId(5),
        }),
    );
    kinds.extend(
        [
            CastKind::Truncate,
            CastKind::ZeroExtend,
            CastKind::SignExtend,
            CastKind::FloatExtend,
            CastKind::FloatTruncate,
            CastKind::IntegerToFloat,
            CastKind::FloatToInteger,
            CastKind::Bitcast,
        ]
        .into_iter()
        .map(|kind| OperationKind::Cast {
            kind,
            value: ValueId(6),
            to: Type::F64,
        }),
    );
    kinds.extend([
        OperationKind::Select {
            condition: ValueId(7),
            true_value: ValueId(8),
            false_value: ValueId(9),
        },
        OperationKind::Call {
            callee: FunctionId::new("helper"),
            arguments: vec![ValueId(10), ValueId(u32::MAX)],
        },
        OperationKind::Alloca {
            element: Type::F32,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
        OperationKind::Alloca {
            element: Type::F64,
            count: Some(ValueId(11)),
            address_space: AddressSpace::Workgroup,
            alignment: 16,
        },
        OperationKind::SliceLength { slice: ValueId(12) },
        OperationKind::SliceData { slice: ValueId(13) },
        OperationKind::GetElementPointer {
            base: ValueId(14),
            offset: ValueId(15),
        },
        OperationKind::Load {
            pointer: ValueId(16),
            access: MemoryAccess {
                address_space: AddressSpace::Constant,
                alignment: 8,
                volatile: false,
            },
        },
        OperationKind::Store {
            pointer: ValueId(17),
            value: ValueId(18),
            access: MemoryAccess {
                address_space: AddressSpace::Generic,
                alignment: 1,
                volatile: true,
            },
        },
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Subgroup,
            memory_scope: SynchronizationScope::Device,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [
                    AddressSpace::Private,
                    AddressSpace::Workgroup,
                    AddressSpace::Global,
                    AddressSpace::Constant,
                    AddressSpace::Generic,
                ],
            ),
        }),
    ]);

    let atomic_kinds = [
        AtomicKind::Load,
        AtomicKind::Store,
        AtomicKind::Exchange,
        AtomicKind::CompareExchange,
        AtomicKind::Add,
        AtomicKind::Subtract,
        AtomicKind::Min,
        AtomicKind::Max,
        AtomicKind::BitAnd,
        AtomicKind::BitOr,
        AtomicKind::BitXor,
    ];
    let scopes = [
        SynchronizationScope::Invocation,
        SynchronizationScope::Subgroup,
        SynchronizationScope::Workgroup,
        SynchronizationScope::Device,
        SynchronizationScope::System,
    ];
    let orderings = [
        MemoryOrdering::Relaxed,
        MemoryOrdering::Acquire,
        MemoryOrdering::Release,
        MemoryOrdering::AcquireRelease,
        MemoryOrdering::SequentiallyConsistent,
    ];
    for (index, kind) in atomic_kinds.into_iter().enumerate() {
        kinds.push(OperationKind::Atomic(Atomic {
            kind,
            pointer: ValueId(100 + index as u32),
            value: (kind != AtomicKind::Load).then_some(ValueId(200 + index as u32)),
            compare: (kind == AtomicKind::CompareExchange).then_some(ValueId(300)),
            access: MemoryAccess {
                address_space: AddressSpace::Global,
                alignment: 8,
                volatile: index % 2 == 0,
            },
            scope: scopes[index % scopes.len()],
            ordering: orderings[index % orderings.len()],
            failure_ordering: (kind == AtomicKind::CompareExchange)
                .then_some(MemoryOrdering::Acquire),
        }));
    }

    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| {
            Operation::new(
                vec![ValueDef::new(ValueId(1000 + index as u32), Type::F32)],
                kind,
            )
        })
        .collect()
}

fn full_module() -> Module {
    let mut parameter_types = vec![Type::Unit];
    parameter_types.extend(all_scalar_types());
    parameter_types.extend([
        Type::pointer(
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
            AddressSpace::Generic,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::pointer(Type::F64, AddressSpace::Constant, AccessMode::ReadOnly),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        ),
    ]);

    let mut entry = BasicBlock::new(BlockId(0));
    entry.parameters = vec![
        ValueDef::new(ValueId(0), Type::BOOL),
        ValueDef::new(ValueId(1), Type::INDEX),
    ];
    entry.operations = all_operations();
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0), ValueId(1)],
    });

    let mut conditional = BasicBlock::new(BlockId(1));
    conditional.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(2),
        then_arguments: vec![ValueId(1)],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut switch = BasicBlock::new(BlockId(2));
    switch.terminator = Some(Terminator::Switch {
        selector: ValueId(1),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(3),
                arguments: vec![],
            },
            SwitchCase {
                value: u64::MAX,
                target: BlockId(4),
                arguments: vec![ValueId(0)],
            },
        ],
        default_target: BlockId(5),
        default_arguments: vec![ValueId(1)],
    });
    let mut returning = BasicBlock::new(BlockId(3));
    returning.terminator = Some(Terminator::Return {
        values: vec![ValueId(1)],
    });
    let mut unreachable = BasicBlock::new(BlockId(4));
    unreachable.terminator = Some(Terminator::Unreachable);
    let missing_terminator = BasicBlock::new(BlockId(5));

    let mut defined = Function::kernel_entry(
        "entry",
        Signature::new(parameter_types, vec![Type::INDEX]),
        vec![ValueId(0), ValueId(1)],
        vec![
            entry,
            conditional,
            switch,
            returning,
            unreachable,
            missing_terminator,
        ],
    );
    defined.required_capabilities = all_capabilities();

    let declaration = Function::declaration(
        "helper",
        Signature::new(vec![Type::F32, Type::F64], vec![Type::F64]),
    );

    let mut kernel_1d = Kernel::new(
        "kernel_1d",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel_1d.required_capabilities = all_capabilities();
    let mut kernel_2d = Kernel::new(
        "kernel_2d",
        "entry",
        LaunchDomain::D2 {
            x: LaunchExtent::Static(32),
            y: LaunchExtent::Dynamic,
        },
    );
    kernel_2d.workgroup_size = Some(WorkgroupSize::new(8, 4, 1));
    let kernel_3d = Kernel::new(
        "kernel_3d",
        "entry",
        LaunchDomain::D3 {
            x: LaunchExtent::Static(7),
            y: LaunchExtent::Static(11),
            z: LaunchExtent::Static(13),
        },
    );

    Module {
        id: ModuleId::new("tests::wire::full"),
        functions: vec![defined, declaration],
        kernels: vec![kernel_1d, kernel_2d, kernel_3d],
        required_capabilities: all_capabilities(),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn from_hex(text: &str) -> Vec<u8> {
    let compact = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    assert_eq!(compact.len() % 2, 0);
    compact
        .chunks_exact(2)
        .map(|pair| {
            let digit = |value: u8| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("invalid golden hex"),
            };
            (digit(pair[0]) << 4) | digit(pair[1])
        })
        .collect()
}

fn set_declared_length(bytes: &mut [u8]) {
    let length = u32::try_from(bytes.len()).unwrap();
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
}

#[test]
fn exhaustive_module_matches_golden_and_round_trips() {
    let module = full_module();
    let encoded = encode_module_v1(&module).expect("encode full fixture");
    let golden = from_hex(GOLDEN_HEX);
    assert_eq!(to_hex(&encoded), to_hex(&golden));
    assert_eq!(decode_module_v1(&golden).expect("decode golden"), module);
    assert_eq!(
        encode_module_v1(&decode_module_v1(&encoded).unwrap()).unwrap(),
        encoded
    );
}

#[test]
fn frozen_wire_versions_reject_semantic_memory_intrinsics() {
    let mut module = full_module();
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
            pointer: ValueId(0),
            element: MemoryElementType::Scalar(ScalarType::U32),
            address_space: AddressSpace::Global,
            layout: MemoryLayout::new(4, 4),
            contract: VolatileAccessContract::rust_allocation_load(),
        });

    for (version, result) in [
        (KERNEL_IR_VERSION_V1, encode_module_v1(&module)),
        (KERNEL_IR_VERSION_V2, encode_module_v2(&module)),
        (KERNEL_IR_VERSION_V3, encode_module_v3(&module)),
    ] {
        assert_eq!(
            result,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "semantic memory intrinsic",
            })
        );
    }
}

#[test]
fn set_insertion_order_does_not_change_bytes() {
    let capabilities = [
        TargetCapability::Float64,
        TargetCapability::Float16,
        TargetCapability::SubgroupSize(64),
    ];
    let mut forward = Module::new("ordered");
    forward.required_capabilities.extend(capabilities.clone());
    let mut reverse = Module::new("ordered");
    reverse
        .required_capabilities
        .extend(capabilities.into_iter().rev());

    assert_eq!(encode_module_v1(&forward), encode_module_v1(&reverse));
}

#[test]
fn vector_order_is_preserved() {
    let mut first = Module::new("sequences");
    first.functions = vec![
        Function::declaration("a", Signature::new(vec![], vec![])),
        Function::declaration("b", Signature::new(vec![], vec![])),
    ];
    let mut second = first.clone();
    second.functions.reverse();
    assert_ne!(encode_module_v1(&first), encode_module_v1(&second));
    assert_eq!(
        decode_module_v1(&encode_module_v1(&second).unwrap()).unwrap(),
        second
    );
}

#[test]
fn rejects_every_truncation_and_trailing_bytes() {
    let encoded = encode_module_v1(&full_module()).unwrap();
    for length in 0..encoded.len() {
        assert!(
            decode_module_v1(&encoded[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(
        decode_module_v1(&trailing),
        Err(KernelIrDecodeError::TrailingBytes)
    );
}

#[test]
fn rejects_header_errors_and_invalid_utf8() {
    let encoded = encode_module_v1(&Module::new("m")).unwrap();

    let mut invalid = encoded.clone();
    invalid[0] ^= 0xff;
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::InvalidMagic)
    );

    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::UnknownVersion(2))
    );

    let mut invalid = encoded.clone();
    invalid[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::UnsupportedFlags(1))
    );

    let mut invalid = encoded.clone();
    invalid[12..16].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::InvalidLength { declared: 1 })
    );

    let mut invalid = encoded.clone();
    invalid[16] = 1;
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::ReservedNonZero {
            field: "module header"
        })
    );

    let mut invalid = encoded;
    invalid[HEADER_BYTES + 4] = 0xff;
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::InvalidUtf8 { field: "module ID" })
    );
}

#[test]
fn rejects_unknown_capability_type_operation_and_terminator_tags() {
    let mut capability_module = Module::new("m");
    capability_module
        .required_capabilities
        .insert(TargetCapability::Float16);
    let mut encoded = encode_module_v1(&capability_module).unwrap();
    encoded[HEADER_BYTES + 5 + 12] = 0xff;
    assert_eq!(
        decode_module_v1(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "target capability",
            tag: 0xff
        })
    );

    let mut type_module = Module::new("m");
    type_module.functions.push(Function::declaration(
        "f",
        Signature::new(vec![Type::Unit], vec![]),
    ));
    let mut encoded = encode_module_v1(&type_module).unwrap();
    encoded[HEADER_BYTES + 5 + 12 + 5 + 4] = 0xff;
    assert_eq!(
        decode_module_v1(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "type",
            tag: 0xff
        })
    );

    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Constant(Constant::Bool(false)),
    ));
    let mut operation_module = Module::new("m");
    operation_module.functions.push(Function::definition(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut encoded = encode_module_v1(&operation_module).unwrap();
    // Header, module, function/signature, body, block, and empty result list.
    encoded[HEADER_BYTES + 5 + 12 + 5 + 8 + 1 + 4 + 4 + 4 + 4 + 4 + 4] = 0xff;
    assert_eq!(
        decode_module_v1(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "operation",
            tag: 0xff
        })
    );

    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Unreachable);
    let mut terminator_module = Module::new("m");
    terminator_module.functions.push(Function::definition(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut encoded = encode_module_v1(&terminator_module).unwrap();
    // The same prefix through an empty operation list, then the presence tag.
    encoded[HEADER_BYTES + 5 + 12 + 5 + 8 + 1 + 4 + 4 + 4 + 4 + 4 + 1] = 0xff;
    assert_eq!(
        decode_module_v1(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "terminator",
            tag: 0xff
        })
    );
}

#[test]
fn rejects_non_boolean_and_non_option_tags() {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Constant(Constant::Bool(false)),
    ));
    let mut module = Module::new("m");
    module.functions.push(Function::definition(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let encoded = encode_module_v1(&module).unwrap();

    let mut invalid = encoded.clone();
    invalid[50] = 2;
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "function body",
            tag: 2
        })
    );

    let mut invalid = encoded;
    invalid[77] = 2;
    assert_eq!(
        decode_module_v1(&invalid),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "boolean constant",
            tag: 2
        })
    );
}

#[test]
fn rejects_noncanonical_set_order_and_duplicates() {
    let mut module = Module::new("m");
    module.required_capabilities = [TargetCapability::Float16, TargetCapability::BFloat16]
        .into_iter()
        .collect();
    let encoded = encode_module_v1(&module).unwrap();
    let first_capability = HEADER_BYTES + 5 + 12;

    let mut reversed = encoded.clone();
    reversed.swap(first_capability, first_capability + 1);
    assert_eq!(
        decode_module_v1(&reversed),
        Err(KernelIrDecodeError::NonCanonical)
    );

    let mut duplicate = encoded;
    duplicate[first_capability + 1] = duplicate[first_capability];
    assert_eq!(
        decode_module_v1(&duplicate),
        Err(KernelIrDecodeError::NonCanonical)
    );
}

#[test]
fn enforces_encode_and_decode_resource_bounds_before_allocation() {
    let oversized_id = Module::new("x".repeat(MAX_TEXT_BYTES_V1 + 1));
    assert_eq!(
        encode_module_v1(&oversized_id),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "module ID",
            actual: MAX_TEXT_BYTES_V1 + 1,
            max: MAX_TEXT_BYTES_V1,
        })
    );

    let declaration = Function::declaration("f", Signature::new(vec![], vec![]));
    let mut too_many_functions = Module::new("functions");
    too_many_functions.functions = vec![declaration; MAX_FUNCTIONS_V1 + 1];
    assert_eq!(
        encode_module_v1(&too_many_functions),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "module functions",
            actual: MAX_FUNCTIONS_V1 + 1,
            max: MAX_FUNCTIONS_V1,
        })
    );

    let mut nested = Type::Unit;
    for _ in 0..=MAX_TYPE_DEPTH_V1 {
        nested = Type::pointer(nested, AddressSpace::Private, AccessMode::ReadOnly);
    }
    let mut module = Module::new("nested");
    module.functions.push(Function::declaration(
        "f",
        Signature::new(vec![nested], vec![]),
    ));
    assert_eq!(
        encode_module_v1(&module),
        Err(KernelIrEncodeError::TypeNestingTooDeep {
            max: MAX_TYPE_DEPTH_V1
        })
    );

    let mut encoded = encode_module_v1(&Module::new("m")).unwrap();
    let function_count = HEADER_BYTES + 5;
    encoded[function_count..function_count + 4]
        .copy_from_slice(&((MAX_FUNCTIONS_V1 + 1) as u32).to_le_bytes());
    assert_eq!(
        decode_module_v1(&encoded),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "module functions",
            actual: MAX_FUNCTIONS_V1 + 1,
            max: MAX_FUNCTIONS_V1,
        })
    );

    let mut type_module = Module::new("m");
    type_module.functions.push(Function::declaration(
        "f",
        Signature::new(vec![Type::Unit], vec![]),
    ));
    let encoded = encode_module_v1(&type_module).unwrap();
    let type_offset = HEADER_BYTES + 5 + 12 + 5 + 4;
    let mut too_deep = encoded[..type_offset].to_vec();
    for _ in 0..=MAX_TYPE_DEPTH_V1 {
        too_deep.extend_from_slice(&[3, 1, 1]);
    }
    too_deep.push(1);
    too_deep.extend_from_slice(&encoded[type_offset + 1..]);
    set_declared_length(&mut too_deep);
    assert_eq!(
        decode_module_v1(&too_deep),
        Err(KernelIrDecodeError::TypeNestingTooDeep {
            max: MAX_TYPE_DEPTH_V1
        })
    );

    assert_eq!(
        decode_module_v1(&vec![0; MAX_MODULE_BYTES_V1 + 1]),
        Err(KernelIrDecodeError::TooLarge {
            max: MAX_MODULE_BYTES_V1
        })
    );
}

#[test]
fn decoding_does_not_claim_semantic_verification() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = None;
    let mut module = Module::new("malformed-but-serializable");
    module.functions.push(Function::definition(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));

    let decoded = decode_module_v1(&encode_module_v1(&module).unwrap()).unwrap();
    assert_eq!(decoded, module);
    assert!(verify_module(&decoded).is_err());
}

#[test]
fn frozen_wire_versions_reject_unrepresentable_export_roles() {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("explicit-export");
    module.functions.push(Function::device_ffi_export(
        "exported",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));

    for (version, result) in [
        (KERNEL_IR_VERSION_V1, encode_module_v1(&module)),
        (KERNEL_IR_VERSION_V2, encode_module_v2(&module)),
    ] {
        assert_eq!(
            result,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "device-FFI export function roles",
            })
        );
    }
}

#[test]
fn decoder_never_panics_on_mutations_or_bounded_noise() {
    let encoded = encode_module_v1(&full_module()).unwrap();
    for index in 0..encoded.len() {
        for mask in [0x01, 0x80, 0xff] {
            let mut mutated = encoded.clone();
            mutated[index] ^= mask;
            assert!(
                catch_unwind(AssertUnwindSafe(|| decode_module_v1(&mutated))).is_ok(),
                "decoder panicked at byte {index}, mask {mask:#x}"
            );
        }
    }

    let mut state = 0x7a12_9e37_c4d5_b601_u64;
    for _ in 0..512 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let length = (state as usize) % 2048;
        let mut bytes = vec![0; length];
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            *byte = state as u8;
        }
        assert!(catch_unwind(|| decode_module_v1(&bytes)).is_ok());
    }
}
