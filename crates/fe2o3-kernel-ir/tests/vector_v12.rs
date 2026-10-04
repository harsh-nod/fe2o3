use fe2o3_kernel_ir::*;

const VECTOR_V12_GOLDEN_HEX: &str = "4645324f334b49000c000000b9000000000000000a000000766563746f722d76313201000000000000000000000009000000726f756e647472697001000000030302020d00000000010100000000000000010000000000000000000000030000000100000001000000050d0400011b01000000000d0400010310000000000100000002000000050d04000202001d01000000020200000000001c0100000000020000000d040002020003100000000001040000000000000000";

type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;

fn vector_type(layout: VectorLayoutV12) -> FixedVectorTypeV12 {
    FixedVectorTypeV12::new(ScalarType::F32, 4, layout)
}

fn vector_module() -> Module {
    let pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let contiguous = vector_type(VectorLayoutV12::Contiguous);
    let interleaved = vector_type(VectorLayoutV12::Interleaved { factor: 2 });
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(1), Type::vector(contiguous))],
            OperationKind::VectorLoad(VectorLoadOperationV12::new(
                ValueId(0),
                VectorMemoryAccessV12::new(contiguous, MemoryAccess::new(AddressSpace::Global, 16)),
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(2), Type::vector(interleaved))],
            OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                ValueId(1),
                interleaved.layout,
            )),
        ),
        Operation::new(
            vec![],
            OperationKind::VectorStore(VectorStoreOperationV12::new(
                ValueId(0),
                ValueId(2),
                VectorMemoryAccessV12::new(
                    interleaved,
                    MemoryAccess::new(AddressSpace::Global, 16),
                ),
            )),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("vector-v12");
    module.functions.push(Function::internal_helper(
        "roundtrip",
        Signature::new(vec![pointer], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn untyped_operation_module(operation: Operation) -> Module {
    let pointer = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(operation);
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("vector-v12-operation-only");
    module.functions.push(Function::internal_helper(
        "operation_only",
        Signature::new(vec![pointer, Type::F32], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module
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
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
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

fn first_offset(bytes: &[u8], marker: &[u8]) -> usize {
    bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("marker must occur")
}

#[test]
fn vector_v12_round_trips_with_an_exact_canonical_owner() {
    let module = vector_module();
    verify_module(&module).unwrap();
    let encoded = encode_module_v12(&module).unwrap();
    assert_eq!(&encoded[8..10], &KERNEL_IR_VERSION_V12.to_le_bytes());
    assert_eq!(decode_module_v12(&encoded).unwrap(), module);
    assert_eq!(encode_module_v12(&module).unwrap(), encoded);

    let (owner, decoded) =
        VerifiedCanonicalKernelIrV12::from_canonical_bytes_with_module(encoded.clone()).unwrap();
    owner.revalidate().unwrap();
    assert_eq!(decoded, module);
    assert_eq!(owner.canonical_bytes(), encoded);
    assert_eq!(
        owner.identity(),
        VerifiedCanonicalKernelIrV12::from_module(module)
            .unwrap()
            .identity()
    );
}

#[test]
fn v12_vector_wire_is_frozen_and_deterministic() {
    let module = vector_module();
    let encoded = encode_module_v12(&module).unwrap();
    assert_eq!(to_hex(&encoded), VECTOR_V12_GOLDEN_HEX);
    assert_eq!(
        decode_module_v12(&from_hex(VECTOR_V12_GOLDEN_HEX)).unwrap(),
        module
    );
    for _ in 0..4 {
        assert_eq!(encode_module_v12(&module).unwrap(), encoded);
    }
}

#[test]
fn every_older_encoder_rejects_the_v12_type_without_reinterpretation() {
    let module = vector_module();
    let encoders: &[(u16, Encoder)] = &[
        (KERNEL_IR_VERSION_V1, encode_module_v1),
        (KERNEL_IR_VERSION_V2, encode_module_v2),
        (KERNEL_IR_VERSION_V3, encode_module_v3),
        (KERNEL_IR_VERSION_V4, encode_module_v4),
        (KERNEL_IR_VERSION_V5, encode_module_v5),
        (KERNEL_IR_VERSION_V6, encode_module_v6),
        (KERNEL_IR_VERSION_V7, encode_module_v7),
        (KERNEL_IR_VERSION_V8, encode_module_v8),
        (KERNEL_IR_VERSION_V9, encode_module_v9),
        (KERNEL_IR_VERSION_V10, encode_module_v10),
        (KERNEL_IR_VERSION_V11, encode_module_v11),
    ];
    for (version, encode) in encoders {
        assert_eq!(
            encode(&module),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version: *version,
                feature: "fixed-lane vector type",
            })
        );
    }

    let access = VectorMemoryAccessV12::new(
        vector_type(VectorLayoutV12::Contiguous),
        MemoryAccess::new(AddressSpace::Global, 16),
    );
    for (feature, operation) in [
        (
            "fixed-vector load",
            Operation::new(
                vec![ValueDef::new(ValueId(2), Type::F32)],
                OperationKind::VectorLoad(VectorLoadOperationV12::new(ValueId(0), access)),
            ),
        ),
        (
            "fixed-vector store",
            Operation::new(
                vec![],
                OperationKind::VectorStore(VectorStoreOperationV12::new(
                    ValueId(0),
                    ValueId(1),
                    access,
                )),
            ),
        ),
        (
            "fixed-vector layout conversion",
            Operation::new(
                vec![ValueDef::new(ValueId(2), Type::F32)],
                OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                    ValueId(1),
                    VectorLayoutV12::Interleaved { factor: 2 },
                )),
            ),
        ),
    ] {
        assert_eq!(
            encode_module_v11(&untyped_operation_module(operation)),
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version: KERNEL_IR_VERSION_V11,
                feature,
            })
        );
    }

    let mut forged = encode_module_v12(&module).unwrap();
    forged[8..10].copy_from_slice(&KERNEL_IR_VERSION_V11.to_le_bytes());
    assert_eq!(
        decode_module_v11(&forged),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "type",
            tag: 5,
        })
    );

    let operation = Operation::new(
        vec![ValueDef::new(ValueId(2), Type::F32)],
        OperationKind::VectorLoad(VectorLoadOperationV12::new(
            ValueId(0),
            VectorMemoryAccessV12::new(
                vector_type(VectorLayoutV12::Contiguous),
                MemoryAccess::new(AddressSpace::Global, 16),
            ),
        )),
    );
    let mut forged_operation = encode_module_v12(&untyped_operation_module(operation)).unwrap();
    forged_operation[8..10].copy_from_slice(&KERNEL_IR_VERSION_V11.to_le_bytes());
    assert_eq!(
        decode_module_v11(&forged_operation),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "operation",
            tag: 27,
        })
    );
}

#[test]
fn hostile_vector_types_layouts_and_accesses_fail_verification() {
    let mut lane_count = vector_module();
    lane_count.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
        Type::vector(FixedVectorTypeV12::new(
            ScalarType::F32,
            1,
            VectorLayoutV12::Contiguous,
        ));
    assert!(
        verify_module(&lane_count)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidVectorOperation)
    );

    let mut bad_layout = vector_module();
    let OperationKind::VectorLayoutConvert(conversion) =
        &mut bad_layout.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    conversion.to = VectorLayoutV12::Interleaved { factor: 3 };
    assert!(
        verify_module(&bad_layout)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidVectorOperation)
    );

    let mut bad_access = vector_module();
    let OperationKind::VectorLoad(load) =
        &mut bad_access.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    load.access.memory.address_space = AddressSpace::Workgroup;
    load.access.memory.alignment = 3;
    assert!(
        verify_module(&bad_access)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidMemoryAccess)
    );
    assert!(
        verify_module(&bad_access)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAlignment)
    );
}

#[test]
fn hostile_provenance_and_value_types_fail_closed() {
    let mut provenance = vector_module();
    let OperationKind::VectorLoad(load) =
        &mut provenance.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    load.provenance = VectorAccessProvenanceV12::Pointer(ValueId(99));
    assert!(
        verify_module(&provenance)
            .unwrap_err()
            .contains(DiagnosticCode::UndefinedValue)
    );

    let mut value = vector_module();
    let OperationKind::VectorStore(store) =
        &mut value.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind
    else {
        unreachable!()
    };
    store.value = ValueId(1);
    assert!(
        verify_module(&value)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );
}

#[test]
fn vector_lane_and_wire_tag_budgets_are_terminal() {
    let too_many = FixedVectorTypeV12::new(
        ScalarType::F32,
        MAX_FIXED_VECTOR_LANES_V12 + 1,
        VectorLayoutV12::Contiguous,
    );
    let mut module = Module::new("vector-budget-v12");
    module.functions.push(Function::external_import(
        "too_many",
        Signature::new(vec![Type::vector(too_many)], vec![]),
    ));
    assert_eq!(
        encode_module_v12(&module),
        Err(KernelIrEncodeError::LimitExceeded {
            field: "fixed vector lanes",
            actual: usize::from(MAX_FIXED_VECTOR_LANES_V12 + 1),
            max: usize::from(MAX_FIXED_VECTOR_LANES_V12),
        })
    );

    let mut hostile_lanes = encode_module_v12(&vector_module()).unwrap();
    let type_marker = [5, 13, 4, 0, 1];
    let offset = first_offset(&hostile_lanes, &type_marker);
    hostile_lanes[offset + 2..offset + 4]
        .copy_from_slice(&(MAX_FIXED_VECTOR_LANES_V12 + 1).to_le_bytes());
    assert!(matches!(
        decode_module_v12(&hostile_lanes),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "fixed vector lanes",
            ..
        })
    ));

    let mut hostile_layout = encode_module_v12(&vector_module()).unwrap();
    let offset = first_offset(&hostile_layout, &type_marker);
    hostile_layout[offset + 4] = 0xff;
    assert_eq!(
        decode_module_v12(&hostile_layout),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "vector layout",
            tag: 0xff,
        })
    );
}

#[test]
fn exact_v12_owner_rejects_older_and_semantically_hostile_bytes() {
    let v11 = encode_module_v11(&Module::new("old-v11")).unwrap();
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(v11),
        Err(VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version: 11 })
    ));

    let mut malformed = vector_module();
    let OperationKind::VectorLayoutConvert(conversion) =
        &mut malformed.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    conversion.to = VectorLayoutV12::Contiguous;
    let bytes = encode_module_v12(&malformed).unwrap();
    assert!(matches!(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(bytes),
        Err(VerifiedCanonicalKernelIrErrorV12::Verification(_))
    ));
}
