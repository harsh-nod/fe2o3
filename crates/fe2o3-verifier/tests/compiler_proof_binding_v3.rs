use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV3, InertProofBindingAssociationV3,
    InertProofBindingReceiptV3,
};
#[path = "../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;

use compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v3;
use fe2o3_verifier::{CompilerProofInputValidationErrorV3, validate_compiler_proof_inputs_v3};

struct Receipts {
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
}

fn receipts(seed: u8) -> Receipts {
    let inputs = canonical_compiler_proof_inputs_v3(seed);
    Receipts {
        semantic_mir: InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
            inputs.semantic_mir().to_vec(),
        )
        .unwrap(),
        middle_end: InertMiddleEndReceiptV3::from_canonical_preimage(inputs.middle_end().to_vec())
            .unwrap(),
        kernel_ir: InertKernelIrReceiptV3::from_canonical_preimage(inputs.kernel_ir().to_vec())
            .unwrap(),
        correspondence: InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
            inputs.correspondence().to_vec(),
        )
        .unwrap(),
        formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(
            inputs.formal_memory().to_vec(),
        )
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

fn proof_binding(receipts: &Receipts, substitute: Option<usize>) -> InertProofBindingReceiptV3 {
    InertProofBindingReceiptV3::from_canonical_preimage(
        association(receipts, substitute).canonical_bytes(),
    )
    .unwrap()
}

fn validate(
    proof_binding: &InertProofBindingReceiptV3,
    receipts: &Receipts,
) -> Result<fe2o3_verifier::ValidatedCompilerProofInputsV3, CompilerProofInputValidationErrorV3> {
    validate_compiler_proof_inputs_v3(
        proof_binding,
        &receipts.semantic_mir,
        &receipts.middle_end,
        &receipts.kernel_ir,
        &receipts.correspondence,
        &receipts.formal_memory,
    )
}

#[test]
fn exact_compiler_inputs_are_independently_decoded_and_cross_checked() {
    let receipts = receipts(0);
    let proof_binding = proof_binding(&receipts, None);
    let validated = validate(&proof_binding, &receipts).unwrap();

    assert_eq!(validated.receipt_identity(), proof_binding.identity());
    assert_eq!(
        validated.semantic_mir().canonical_encoding(),
        receipts.semantic_mir.canonical_preimage()
    );
    assert_eq!(
        validated.middle_end().canonical_bytes(),
        receipts.middle_end.canonical_preimage()
    );
    assert_eq!(
        validated.kernel_ir().canonical_bytes(),
        receipts.kernel_ir.canonical_preimage()
    );
    assert_eq!(
        validated.correspondence().canonical_bytes(),
        receipts.correspondence.canonical_preimage()
    );
    assert_eq!(
        validated.formal_memory().canonical_bytes(),
        receipts.formal_memory.canonical_preimage()
    );
    assert!(validated.has_exact_decoded_input_association());
    assert!(validated.has_structural_mir_to_kir_correspondence());
    assert!(!validated.authenticates_verus_execution());
    assert!(!validated.establishes_compiler_refinement());
    assert!(!validated.grants_runtime_authority());
    assert!(!validated.middle_end().claims_verus_verification());
    assert!(!validated.correspondence().grants_authority());
    assert!(!validated.formal_memory().grants_authority());
}

#[test]
fn every_outer_compiler_stage_identity_substitution_fails_closed() {
    let fields = [
        "semantic MIR",
        "middle end",
        "Kernel IR",
        "MIR-to-KIR correspondence",
        "formal memory",
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let receipts = receipts(0);
        let proof_binding = proof_binding(&receipts, Some(index));
        assert!(matches!(
            validate(&proof_binding, &receipts),
            Err(CompilerProofInputValidationErrorV3::ProofBindingIdentityMismatch {
                field: actual
            }) if actual == field
        ));
    }
}

#[test]
fn malformed_outer_receipt_preimage_is_not_repaired() {
    let receipts = receipts(0);
    let association = association(&receipts, None);
    let mut malformed = association.canonical_bytes().to_vec();
    malformed[0] ^= 0xff;
    let proof_binding = InertProofBindingReceiptV3::from_canonical_preimage(malformed).unwrap();
    assert!(matches!(
        validate(&proof_binding, &receipts),
        Err(CompilerProofInputValidationErrorV3::ProofBindingDecode(_))
    ));
}

#[test]
fn every_malformed_exact_stage_preimage_fails_at_its_decoder() {
    let mut semantic = receipts(0);
    semantic.semantic_mir =
        InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(b"bad MIR".to_vec()).unwrap();
    let proof = proof_binding(&semantic, None);
    assert!(matches!(
        validate(&proof, &semantic),
        Err(CompilerProofInputValidationErrorV3::SemanticMirDecode(_))
    ));

    let mut middle = receipts(0);
    middle.middle_end =
        InertMiddleEndReceiptV3::from_canonical_preimage(b"bad middle".to_vec()).unwrap();
    let proof = proof_binding(&middle, None);
    assert!(matches!(
        validate(&proof, &middle),
        Err(CompilerProofInputValidationErrorV3::MiddleEndDecode(_))
    ));

    let mut kernel = receipts(0);
    kernel.kernel_ir =
        InertKernelIrReceiptV3::from_canonical_preimage(b"bad KIR".to_vec()).unwrap();
    let proof = proof_binding(&kernel, None);
    assert!(matches!(
        validate(&proof, &kernel),
        Err(CompilerProofInputValidationErrorV3::KernelIr(_))
    ));

    let mut correspondence = receipts(0);
    correspondence.correspondence = InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
        b"bad correspondence".to_vec(),
    )
    .unwrap();
    let proof = proof_binding(&correspondence, None);
    assert!(matches!(
        validate(&proof, &correspondence),
        Err(CompilerProofInputValidationErrorV3::CorrespondenceDecode(_))
    ));

    let mut formal = receipts(0);
    formal.formal_memory =
        InertFormalMemoryReceiptV3::from_canonical_preimage(b"bad formal".to_vec()).unwrap();
    let proof = proof_binding(&formal, None);
    assert!(matches!(
        validate(&proof, &formal),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryDecode(_))
    ));
}

#[test]
fn every_nested_semantic_and_kir_identity_substitution_fails_closed() {
    let mut middle = receipts(0);
    middle.middle_end = receipts(1).middle_end;
    let proof = proof_binding(&middle, None);
    assert!(matches!(
        validate(&proof, &middle),
        Err(
            CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                field: "middle-end source semantic MIR"
            }
        )
    ));

    let mut correspondence = receipts(0);
    correspondence.correspondence = receipts(1).correspondence;
    let proof = proof_binding(&correspondence, None);
    assert!(matches!(
        validate(&proof, &correspondence),
        Err(
            CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                field: "MIR-to-KIR correspondence semantic MIR"
            }
        )
    ));

    let mut kernel = receipts(0);
    kernel.kernel_ir = receipts(1).kernel_ir;
    let proof = proof_binding(&kernel, None);
    assert!(matches!(
        validate(&proof, &kernel),
        Err(
            CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                field: "MIR-to-KIR correspondence Kernel IR"
            }
        )
    ));

    let mut formal = receipts(0);
    formal.formal_memory = receipts(1).formal_memory;
    let proof = proof_binding(&formal, None);
    assert!(matches!(
        validate(&proof, &formal),
        Err(
            CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                field: "formal-memory admission Kernel IR"
            }
        )
    ));
}

#[test]
fn structurally_false_statement_correspondence_fails_after_strict_decode() {
    const FIRST_SOURCE_STATEMENT_COUNT_OFFSET: usize = 20 + 32 + 32 + 4 + 4 + 12;

    let mut receipts = receipts(0);
    let mut correspondence = receipts.correspondence.canonical_preimage().to_vec();
    correspondence[FIRST_SOURCE_STATEMENT_COUNT_OFFSET..FIRST_SOURCE_STATEMENT_COUNT_OFFSET + 4]
        .copy_from_slice(&9_u32.to_le_bytes());
    receipts.correspondence =
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(correspondence).unwrap();
    let proof = proof_binding(&receipts, None);
    assert!(matches!(
        validate(&proof, &receipts),
        Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "correspondence source statement count differs from semantic MIR"
            }
        )
    ));
}
