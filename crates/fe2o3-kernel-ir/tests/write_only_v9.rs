use fe2o3_kernel_ir::*;

const POINTER: ValueId = ValueId(0x1020_3040);
const PREDICATE: ValueId = ValueId(0x5060_7080);
const VALUE: ValueId = ValueId(0x90a0_b0c0);

fn guarded_store_module(
    pointer_type: Type,
    predicate_type: Type,
    value_type: Type,
    results: Vec<ValueDef>,
) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::new(
        results,
        OperationKind::GuardedStore {
            pointer: POINTER,
            predicate: PREDICATE,
            value: VALUE,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("tests::guarded_store_v9");
    module.functions.push(Function::definition(
        "guarded_store",
        Signature::new(vec![pointer_type, predicate_type, value_type], vec![]),
        vec![POINTER, PREDICATE, VALUE],
        vec![block],
    ));
    module
}

fn valid_guarded_store_module(access: AccessMode) -> Module {
    guarded_store_module(
        Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Global, access),
        Type::BOOL,
        Type::Scalar(ScalarType::U32),
        vec![],
    )
}

fn store_module() -> Module {
    let mut module = valid_guarded_store_module(AccessMode::ReadWrite);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Store {
            pointer: POINTER,
            value: VALUE,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        };
    module
}

fn verifier_codes(module: &Module) -> Vec<DiagnosticCode> {
    verify_module(module)
        .unwrap_err()
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn write_only_type_module() -> Module {
    let mut module = Module::new("tests::write_only_type_v9");
    module.functions.push(Function::declaration(
        "write_only_type",
        Signature::new(
            vec![Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::WriteOnly,
            )],
            vec![],
        ),
    ));
    module
}

#[test]
fn write_only_nested_type_is_additive_v9_even_without_a_store() {
    let module = write_only_type_module();
    assert_eq!(
        encode_module_v7(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V7,
            feature: "write-only pointer and slice types",
        })
    );
    assert_eq!(
        encode_module_v8(&module),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V8,
            feature: "write-only pointer and slice types",
        })
    );

    let encoded = encode_module_v9(&module).unwrap();
    assert_eq!(decode_module_v9(&encoded).unwrap(), module);
    let owner = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    assert_eq!(owner.canonical_bytes(), encoded);
}

#[test]
fn frozen_v8_decoder_rejects_write_only_access_tag_three() {
    let mut encoded = encode_module_v9(&write_only_type_module()).unwrap();
    encoded[8..10].copy_from_slice(&KERNEL_IR_VERSION_V8.to_le_bytes());
    assert_eq!(
        decode_module_v8(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "access mode",
            tag: 3,
        })
    );
}

#[test]
fn guarded_store_is_additive_v9_and_roundtrips_canonically() {
    let module = valid_guarded_store_module(AccessMode::WriteOnly);
    verify_module(&module).unwrap();
    assert_eq!(
        encode_module_v8(&valid_guarded_store_module(AccessMode::ReadWrite)),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: KERNEL_IR_VERSION_V8,
            feature: "guarded store",
        })
    );

    let encoded = encode_module_v9(&module).unwrap();
    assert_eq!(decode_module_v9(&encoded).unwrap(), module);
    let owner = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    assert_eq!(owner.canonical_bytes(), encoded);
}

#[test]
fn frozen_v8_decoder_rejects_guarded_store_operation_tag_twenty_five() {
    let mut encoded = encode_module_v8(&store_module()).unwrap();
    let mut needle = vec![14];
    needle.extend_from_slice(&POINTER.0.to_le_bytes());
    needle.extend_from_slice(&VALUE.0.to_le_bytes());
    needle.push(3); // Global address space.
    needle.extend_from_slice(&4_u32.to_le_bytes());
    needle.push(0); // Non-volatile.
    let matches = encoded
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, window)| (window == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "store encoding must be uniquely located");
    encoded[matches[0]] = 25;

    assert_eq!(
        decode_module_v8(&encoded),
        Err(KernelIrDecodeError::UnknownTag {
            kind: "operation",
            tag: 25,
        })
    );
}

#[test]
fn guarded_store_verifier_rejects_each_malformed_operand_and_result_shape() {
    let write_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );

    assert!(
        verifier_codes(&guarded_store_module(
            write_pointer.clone(),
            Type::Scalar(ScalarType::U32),
            Type::Scalar(ScalarType::U32),
            vec![],
        ))
        .contains(&DiagnosticCode::TypeMismatch)
    );

    assert!(
        verifier_codes(&guarded_store_module(
            write_pointer.clone(),
            Type::BOOL,
            Type::F32,
            vec![],
        ))
        .contains(&DiagnosticCode::TypeMismatch)
    );

    assert!(
        verifier_codes(&guarded_store_module(
            Type::Scalar(ScalarType::U32),
            Type::BOOL,
            Type::Scalar(ScalarType::U32),
            vec![],
        ))
        .contains(&DiagnosticCode::InvalidOperandType)
    );

    assert!(
        verifier_codes(&guarded_store_module(
            write_pointer,
            Type::BOOL,
            Type::Scalar(ScalarType::U32),
            vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32),)],
        ))
        .contains(&DiagnosticCode::ResultArity)
    );
}
