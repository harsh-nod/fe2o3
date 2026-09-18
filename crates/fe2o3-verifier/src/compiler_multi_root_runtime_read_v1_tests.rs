use super::*;
use fe2o3_kernel_ir::{
    AccessMode, BasicBlock, BlockId, ComparePredicate, ExplicitLaunchExtent1d, FormalIndexWidth,
    FormalMemoryReceiptEncodingV4, Function, Kernel, KernelId, LaunchDomain, LaunchExtent,
    Operation, ScalarType, Signature, Type, ValueDef, ValueId,
    derive_kernel_memory_obligations_from_verified, verify_module_ref,
};

fn payload(name: &str) -> Vec<u8> {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            3,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        op(
            4,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(20));
    yes.operations = vec![
        op(
            5,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(1),
            },
        ),
        op(
            6,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(5),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(30));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("runtime-multiroot-payload-component");
    module.functions.push(Function::kernel_entry(
        name,
        Signature::new(
            vec![
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        name,
        name,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    let report = derive_kernel_memory_obligations_from_verified(
        verify_module_ref(&module).unwrap(),
        &KernelId::new(name),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(report.is_complete(), "{report:?}");
    InertFormalMemoryReceiptFormatV4::from_current_obligations(report.obligations())
        .unwrap()
        .into_canonical_bytes()
}

#[test]
fn runtime_formal_root_payload_preserves_typed_inert_content() {
    let bytes = payload("runtime");
    let decoded = decode_formal_root_payload_v1(7, "runtime", &bytes).unwrap();
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert_eq!(
        decoded.metadata().encoding(),
        FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
    );
    assert_eq!(decoded.metadata().encoding().extraction_policy(), 3);
    assert_eq!(decoded.metadata().index_width(), FormalIndexWidth::Bits64);
    assert!(!decoded.grants_authority());
    decoded.revalidate().unwrap();
    // This tests the actual nested consumer, not a complete signed capsule.
    assert!(matches!(
        decode_formal_root_payload_v1(7, "foreign", &bytes),
        Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
            root: 7,
            detail: "formal-memory payload names a different kernel or entry"
        })
    ));
}

#[test]
fn runtime_root_binding_checks_entry_as_well_as_kernel() {
    let mut bytes = payload("runtime");
    let kernel_length = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    let entry_start = 20 + 4 + kernel_length + 4;
    bytes[entry_start..entry_start + kernel_length].copy_from_slice(b"foreign");
    assert!(matches!(
        decode_formal_root_payload_v1(12, "runtime", &bytes),
        Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
            root: 12,
            detail: "formal-memory payload names a different kernel or entry"
        })
    ));
}

#[test]
fn runtime_payload_metadata_mismatch_reports_exact_root_without_fallback() {
    let original = payload("runtime");
    let mut width = 20;
    for _ in 0..2 {
        let length = u32::from_le_bytes(original[width..width + 4].try_into().unwrap()) as usize;
        width += 4 + length;
    }
    for (offset, value) in [
        (8, 3),
        (10, 2),
        (10, 4),
        (width, 1),
        (width, 3),
        (width + 1, 2),
    ] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        assert!(matches!(
            decode_formal_root_payload_v1(18, "runtime", &bytes),
            Err(CompilerMultiRootProofValidationErrorV1::FormalMemoryPayload { root: 18, .. })
        ));
    }
    assert!(matches!(
        decode_formal_root_payload_v1(18, "runtime", &original[..original.len() - 1]),
        Err(CompilerMultiRootProofValidationErrorV1::FormalMemoryPayload { root: 18, .. })
    ));
}
