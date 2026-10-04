use fe2o3_kernel_ir::*;

fn memory_intrinsic_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let source = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let destination = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let element = MemoryElementType::Scalar(ScalarType::U32);
    let layout = MemoryLayout::new(4, 4);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::I64)),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::PointerDistance {
                pointer: ValueId(0),
                origin: ValueId(0),
                kind: PointerDistanceKind::Signed,
                unit: PointerDistanceUnit::Elements,
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: PointerDistanceContract::supported_rust(PointerDistanceKind::Signed),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), scalar.clone()),
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileLoad {
                pointer: ValueId(0),
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_load(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(1),
                value: ValueId(3),
                element,
                address_space: AddressSpace::Global,
                layout,
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
        ),
        Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::CopyNonOverlapping {
                source: ValueId(0),
                destination: ValueId(1),
                count: ValueId(2),
                element,
                source_address_space: AddressSpace::Global,
                destination_address_space: AddressSpace::Global,
                layout,
                contract: CopyNonOverlappingContract::supported_rust(),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "memory_impl",
        Signature::new(vec![source, destination, Type::INDEX, scalar], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    let mut module = Module::new("wire-tests::memory-intrinsics-v10");
    module.functions.push(entry);
    module.kernels.push(Kernel::new(
        "memory",
        "memory_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn memory_intrinsics_are_additive_v10_and_round_trip_exactly() {
    let module = memory_intrinsic_module();
    verify_module(&module).unwrap();
    for (version, result) in [
        (KERNEL_IR_VERSION_V1, encode_module_v1(&module)),
        (KERNEL_IR_VERSION_V2, encode_module_v2(&module)),
        (KERNEL_IR_VERSION_V3, encode_module_v3(&module)),
        (KERNEL_IR_VERSION_V4, encode_module_v4(&module)),
        (KERNEL_IR_VERSION_V5, encode_module_v5(&module)),
        (KERNEL_IR_VERSION_V6, encode_module_v6(&module)),
        (KERNEL_IR_VERSION_V7, encode_module_v7(&module)),
        (KERNEL_IR_VERSION_V8, encode_module_v8(&module)),
        (KERNEL_IR_VERSION_V9, encode_module_v9(&module)),
    ] {
        assert_eq!(
            result,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "semantic memory intrinsic",
            })
        );
    }

    let first = encode_module_v10(&module).unwrap();
    let second = encode_module_v10(&module).unwrap();
    assert_eq!(first, second);
    assert_eq!(&first[8..10], &KERNEL_IR_VERSION_V10.to_le_bytes());
    assert_eq!(decode_module_v10(&first).unwrap(), module);
    let owner = VerifiedCanonicalKernelIrV10::from_module(module).unwrap();
    assert_eq!(owner.canonical_bytes(), first);
    owner.revalidate().unwrap();
}

#[test]
fn v10_reader_preserves_older_canonical_modules() {
    let module = Module::new("wire-tests::v10-backcompat");
    for encode in [
        encode_module_v1,
        encode_module_v2,
        encode_module_v3,
        encode_module_v4,
        encode_module_v5,
        encode_module_v6,
        encode_module_v7,
        encode_module_v8,
        encode_module_v9,
    ] {
        let canonical = encode(&module).unwrap();
        assert_eq!(decode_module_v10(&canonical).unwrap(), module);
        assert_eq!(encode(&module).unwrap(), canonical);
    }

    let v10 = encode_module_v10(&module).unwrap();
    assert_eq!(
        decode_module_v9(&v10),
        Err(KernelIrDecodeError::UnknownVersion(KERNEL_IR_VERSION_V10))
    );

    let mut forged_v9 = encode_module_v10(&memory_intrinsic_module()).unwrap();
    forged_v9[8..10].copy_from_slice(&KERNEL_IR_VERSION_V9.to_le_bytes());
    assert_eq!(
        decode_module_v9(&forged_v9),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "operation",
            tag: 26,
        })
    );
}

#[test]
fn v10_reader_rejects_mutated_or_oversized_semantic_instances() {
    let bytes = encode_module_v10(&memory_intrinsic_module()).unwrap();
    let magic = SEMANTIC_OPERATION_INSTANCE_MAGIC_V1;
    let instance = bytes
        .windows(magic.len())
        .position(|window| window == magic)
        .expect("embedded semantic instance");

    let mut contract_mutation = bytes.clone();
    contract_mutation[instance + SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + 4] = 0xff;
    assert_eq!(
        decode_module_v10(&contract_mutation),
        Err(KernelIrDecodeError::InvalidSemanticOperationInstance)
    );

    let mut oversized = bytes;
    let declared = u32::try_from(
        SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1
            + MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1
            + 1,
    )
    .unwrap();
    oversized[instance - 4..instance].copy_from_slice(&declared.to_le_bytes());
    assert_eq!(
        decode_module_v10(&oversized),
        Err(KernelIrDecodeError::LimitExceeded {
            field: "semantic operation instance",
            actual: declared as usize,
            max: SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1
                + MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1,
        })
    );
}
