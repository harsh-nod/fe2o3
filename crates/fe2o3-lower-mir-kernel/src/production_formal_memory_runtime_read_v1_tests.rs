use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, ExplicitLaunchExtent1d,
    Function, Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV8, derive_kernel_memory_obligations_from_verified, verify_module_ref,
};

fn runtime_receipt() -> (
    ProductionCanonicalKernelIrIdentityV1,
    InertFormalMemoryReceiptFormatV4,
) {
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
    let mut module = Module::new("runtime-outer-policy-component");
    module.functions.push(Function::kernel_entry(
        "kernel",
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
        "kernel",
        "kernel",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    let report = derive_kernel_memory_obligations_from_verified(
        verify_module_ref(&module).unwrap(),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(report.is_complete(), "{report:?}");
    let receipt =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(report.obligations()).unwrap();
    assert_eq!(
        receipt.metadata().encoding(),
        FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
    );
    let owner = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
    let identity = ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
        ProductionCanonicalKernelIrVersionV1::V8,
        *owner.identity().digest(),
        owner.canonical_bytes().len() as u64,
    );
    (identity, receipt)
}

fn outer(
    policy: FormalMemoryAdmissionValidationPolicyV4,
    identity: ProductionCanonicalKernelIrIdentityV1,
    receipt: &InertFormalMemoryReceiptFormatV4,
    count: u64,
) -> Result<Vec<u8>, ProductionFormalMemoryEvidenceErrorV4> {
    encode(
        policy,
        identity,
        *receipt.identity_digest(),
        count,
        FormalMemoryCompletenessPolicyV4::RequireCompleteConflictFree,
        FormalMemoryCompletenessStatusV4::Complete,
        0,
        0,
        receipt,
    )
}

#[test]
fn genuine_runtime_receipt_outer_policy_roundtrips_but_stays_inert() {
    let (identity, receipt) = runtime_receipt();
    let bytes = outer(
        FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3,
        identity,
        &receipt,
        64,
    )
    .unwrap();
    assert_eq!(&bytes[10..12], &3_u16.to_le_bytes());
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes).unwrap();
    assert_eq!(
        evidence.validation_policy(),
        FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3
    );
    assert_eq!(evidence.canonical_kernel_ir_identity(), identity);
    assert_eq!(
        evidence.formal_obligation_receipt_bytes(),
        receipt.canonical_bytes()
    );
    assert_eq!(
        evidence.formal_obligation_receipt_identity(),
        receipt.identity_digest()
    );
    assert_eq!(evidence.witness_invocation_count(), 64);
    assert!(!evidence.grants_authority());
    evidence.revalidate().unwrap();
    assert_eq!(evidence.into_canonical_bytes(), bytes);
}

#[test]
fn outer_policy_and_witness_cannot_be_transplanted() {
    let (identity, receipt) = runtime_receipt();
    for policy in [
        FormalMemoryAdmissionValidationPolicyV4::LegacyV1,
        FormalMemoryAdmissionValidationPolicyV4::GuardedV2,
    ] {
        assert!(matches!(
            outer(policy, identity, &receipt, 64),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
        let mut bytes = outer(
            FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3,
            identity,
            &receipt,
            64,
        )
        .unwrap();
        bytes[10..12].copy_from_slice(&(policy as u16).to_le_bytes());
        assert!(matches!(
            InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
    }
    for count in [0, 1, 63, 65, u64::MAX] {
        assert!(matches!(
            outer(
                FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3,
                identity,
                &receipt,
                count
            ),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
    }
    let mut bytes = outer(
        FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3,
        identity,
        &receipt,
        64,
    )
    .unwrap();
    bytes[96..104].copy_from_slice(&63_u64.to_le_bytes());
    assert!(matches!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
        Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
    ));
}

fn nested_width_offset(bytes: &[u8]) -> usize {
    let mut cursor = HEADER_BYTES_V4 + 20;
    for _ in 0..2 {
        let length = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4 + length;
    }
    cursor
}

#[test]
fn runtime_nested_policy_version_width_and_basis_are_closed() {
    let (identity, receipt) = runtime_receipt();
    let original = outer(
        FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3,
        identity,
        &receipt,
        64,
    )
    .unwrap();
    for (offset, value) in [
        (HEADER_BYTES_V4 + 8, 3_u16),
        (HEADER_BYTES_V4 + 10, 2),
        (HEADER_BYTES_V4 + 10, 4),
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert!(matches!(
            InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
            Err(ProductionFormalMemoryEvidenceErrorV4::FormalReceipt(_))
        ));
    }
    let width = nested_width_offset(&original);
    for (offset, value) in [(width, 1), (width, 3), (width + 1, 0), (width + 1, 2)] {
        let mut bytes = original.clone();
        bytes[offset] = value;
        // Impossible runtime metadata fails inside the nested codec, before any
        // outer identity comparison; there is no unchecked metadata constructor.
        assert!(matches!(
            InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
            Err(ProductionFormalMemoryEvidenceErrorV4::FormalReceipt(_))
        ));
    }
    let mut bytes = original;
    bytes[64] ^= 1;
    assert!(matches!(
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes),
        Err(ProductionFormalMemoryEvidenceErrorV4::NestedIdentityMismatch)
    ));
}

fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn legacy_policy_one_and_two_keep_exact_outer_framing_and_reject_policy_three() {
    // These literal legacy codec components do not claim a live semantic owner.
    let rows = [
        (
            FormalMemoryAdmissionValidationPolicyV4::LegacyV1,
            "4645324f33464d00010001000000000053000000010000006b01000000650201000001000000000000000001000000000000000100000000000000000000000203010000000000000000000000000000000000",
        ),
        (
            FormalMemoryAdmissionValidationPolicyV4::GuardedV2,
            "4645324f33464d00030002000000000016010000010000006b010000006502010000010000000000000000010000000000000001000000000000000000000002030100010000001400000000000000000000000000000001030100000000000000000000000400000000000000040000000000000004000000000000000000000000000000010000000000000001000000000000000001000000020000000300000004000000050000000400000000000000010a00000000000000000000001400000001000000140000000000000000000000000000000101000000000000000001000000020000000300000004000000050000000400000000000000010a0000000000000000000000140000000000000000000000",
        ),
    ];
    for (policy, golden) in rows {
        let receipt = InertFormalMemoryReceiptFormatV4::decode_current(hex(golden)).unwrap();
        let identity = ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
            ProductionCanonicalKernelIrVersionV1::V8,
            [17; 32],
            123,
        );
        let bytes = outer(policy, identity, &receipt, 1).unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(b"F2FMA4\0\0");
        expected.extend_from_slice(&4_u16.to_le_bytes());
        expected.extend_from_slice(&(policy as u16).to_le_bytes());
        expected.extend_from_slice(&0_u32.to_le_bytes());
        expected.extend_from_slice(&((120 + receipt.canonical_bytes().len()) as u32).to_le_bytes());
        expected.extend_from_slice(&8_u16.to_le_bytes());
        expected.extend_from_slice(&0_u16.to_le_bytes());
        expected.extend_from_slice(&123_u64.to_le_bytes());
        expected.extend_from_slice(&[17; 32]);
        expected.extend_from_slice(receipt.identity_digest());
        expected.extend_from_slice(&1_u64.to_le_bytes());
        expected.extend_from_slice(&[1, 0, 1, 0]);
        expected.extend_from_slice(&[0; 8]);
        expected.extend_from_slice(&(receipt.canonical_bytes().len() as u32).to_le_bytes());
        assert_eq!(expected.len(), 120);
        expected.extend_from_slice(receipt.canonical_bytes());
        assert_eq!(bytes, expected);
        let decoded = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&expected).unwrap();
        assert_eq!(decoded.identity(), &evidence_identity(&expected).unwrap());
        assert!(matches!(
            outer(
                FormalMemoryAdmissionValidationPolicyV4::RuntimeBoundedV3,
                identity,
                &receipt,
                1
            ),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
        let mut wrong_policy = expected;
        wrong_policy[10..12].copy_from_slice(&3_u16.to_le_bytes());
        assert!(matches!(
            InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&wrong_policy),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
    }
}
