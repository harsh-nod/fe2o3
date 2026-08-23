use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV3, InertProofBindingAssociationV3,
    InertProofBindingReceiptV3,
};
use fe2o3_verifier::{
    CompilerProofBindingValidationErrorV3, validate_compiler_proof_binding_association_v3,
};

struct Receipts {
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
}

fn receipts() -> Receipts {
    Receipts {
        semantic_mir: InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(b"mir".to_vec())
            .unwrap(),
        middle_end: InertMiddleEndReceiptV3::from_canonical_preimage(b"middle".to_vec()).unwrap(),
        kernel_ir: InertKernelIrReceiptV3::from_canonical_preimage(b"kir".to_vec()).unwrap(),
        correspondence: InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
            b"correspondence".to_vec(),
        )
        .unwrap(),
        formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(b"formal".to_vec())
            .unwrap(),
    }
}

fn identity(sha256: &[u8; 32], byte_len: u64) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new(*sha256, byte_len).unwrap()
}

fn association(receipts: &Receipts, substitute: Option<usize>) -> InertProofBindingAssociationV3 {
    let mut identities = [
        identity(
            receipts.semantic_mir.identity().sha256(),
            receipts.semantic_mir.identity().byte_len(),
        ),
        identity(
            receipts.middle_end.identity().sha256(),
            receipts.middle_end.identity().byte_len(),
        ),
        identity(
            receipts.kernel_ir.identity().sha256(),
            receipts.kernel_ir.identity().byte_len(),
        ),
        identity(
            receipts.correspondence.identity().sha256(),
            receipts.correspondence.identity().byte_len(),
        ),
        identity(
            receipts.formal_memory.identity().sha256(),
            receipts.formal_memory.identity().byte_len(),
        ),
    ];
    if let Some(index) = substitute {
        identities[index] =
            InertLineageContentIdentityV3::new([0xa0 + index as u8; 32], 99).unwrap();
    }
    InertProofBindingAssociationV3::new(InertProofBindingAssociationInputsV3::new(
        identities[0],
        identities[1],
        identities[2],
        identities[3],
        identities[4],
    ))
    .unwrap()
}

fn validate(
    proof_binding: &InertProofBindingReceiptV3,
    receipts: &Receipts,
) -> Result<
    fe2o3_verifier::ValidatedCompilerProofBindingAssociationV3,
    CompilerProofBindingValidationErrorV3,
> {
    validate_compiler_proof_binding_association_v3(
        proof_binding,
        &receipts.semantic_mir,
        &receipts.middle_end,
        &receipts.kernel_ir,
        &receipts.correspondence,
        &receipts.formal_memory,
    )
}

#[test]
fn exact_compiler_association_is_independently_recovered() {
    let receipts = receipts();
    let association = association(&receipts, None);
    let proof_binding =
        InertProofBindingReceiptV3::from_canonical_preimage(association.canonical_bytes().to_vec())
            .unwrap();
    let validated = validate(&proof_binding, &receipts).unwrap();
    assert_eq!(validated.association().inputs(), association.inputs());
    assert_eq!(validated.receipt_identity(), proof_binding.identity());
    assert!(!validated.authenticates_verus_execution());
    assert!(!validated.establishes_compiler_refinement());
    assert!(!validated.grants_runtime_authority());
}

#[test]
fn every_compiler_stage_identity_substitution_fails_closed() {
    let fields = [
        "semantic MIR",
        "middle end",
        "Kernel IR",
        "MIR-to-KIR correspondence",
        "formal memory",
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let receipts = receipts();
        let association = association(&receipts, Some(index));
        let proof_binding = InertProofBindingReceiptV3::from_canonical_preimage(
            association.canonical_bytes().to_vec(),
        )
        .unwrap();
        assert_eq!(
            validate(&proof_binding, &receipts).unwrap_err(),
            CompilerProofBindingValidationErrorV3::IdentityMismatch { field }
        );
    }
}

#[test]
fn malformed_outer_receipt_preimage_is_not_repaired() {
    let receipts = receipts();
    let association = association(&receipts, None);
    let mut malformed = association.canonical_bytes().to_vec();
    malformed[0] ^= 0xff;
    let proof_binding = InertProofBindingReceiptV3::from_canonical_preimage(malformed).unwrap();
    assert!(matches!(
        validate(&proof_binding, &receipts),
        Err(CompilerProofBindingValidationErrorV3::Decode(_))
    ));
}
