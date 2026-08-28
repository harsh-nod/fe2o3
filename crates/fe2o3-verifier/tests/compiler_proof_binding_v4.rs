use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV4, InertProofBindingAssociationV4,
    InertProofBindingReceiptV3,
};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2, SafeReferenceKindV2,
    UnsignedFunctionalRefinementReceiptV2, VerusToolchainIdentityV2,
};
use fe2o3_pliron::InertProductionMiddleEndEvidenceV5;
use fe2o3_proof_contracts::DigestV1;
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1, CompilerProofInputValidationErrorV4,
    ProductionMirPlironVerusExecutionClaimsV1, ValidatedCompilerProofInputsV4,
    validate_compiler_proof_inputs_v4,
};

#[path = "../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;
use compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v3;

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

fn digest(seed: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([seed; 32])
}

fn identity(sha256: &[u8; 32], byte_len: u64) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new(*sha256, byte_len).unwrap()
}

fn stage_identities(receipts: &Receipts) -> [InertLineageContentIdentityV3; 5] {
    [
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
    ]
}

fn signed_verus_evidence(
    pliron_identity: DigestV1,
) -> CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
    let binding = FunctionalRefinementBindingV2::new(
        SafeReferenceKindV2::SourceAndMir,
        digest(10),
        digest(11),
        digest(12),
        digest(13),
        digest(14),
        digest(15),
    )
    .unwrap();
    let toolchain =
        VerusToolchainIdentityV2::new(digest(20), digest(21), digest(22), digest(23), digest(24))
            .unwrap();
    let signing = SigningKey::from_bytes(&[42; 32]);
    let verifying_key = signing.verifying_key().to_bytes();
    let policy = FunctionalRefinementImportPolicyV2::new(
        verifying_key,
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(30),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let claims = ProductionMirPlironVerusExecutionClaimsV1::new(
        digest(1),
        digest(2),
        pliron_identity,
        digest(4),
        digest(5),
        binding,
        imported.signer_identity(),
        toolchain,
        imported.execution_identity(),
        imported.receipt_identity().digest(),
        3,
    )
    .unwrap();
    CanonicalProductionMirPlironVerusExecutionEvidenceV1::new(claims, verifying_key, wire).unwrap()
}

fn exact_pliron_identity(receipts: &Receipts) -> DigestV1 {
    let middle_end =
        InertProductionMiddleEndEvidenceV5::decode(receipts.middle_end.canonical_preimage())
            .unwrap();
    DigestV1::from_untrusted_bytes(*middle_end.identity().sha256())
}

fn proof_binding(
    receipts: &Receipts,
    substitute: Option<usize>,
    evidence: &[u8],
) -> InertProofBindingReceiptV3 {
    let mut identities = stage_identities(receipts);
    if let Some(index) = substitute {
        identities[index] =
            InertLineageContentIdentityV3::new([0xa0 + index as u8; 32], 99).unwrap();
    }
    let association = InertProofBindingAssociationV4::new(
        InertProofBindingAssociationInputsV4::new(
            identities[0],
            identities[1],
            identities[2],
            identities[3],
            identities[4],
        ),
        evidence,
    )
    .unwrap();
    InertProofBindingReceiptV3::from_canonical_preimage(association.canonical_bytes()).unwrap()
}

fn validate(
    proof_binding: &InertProofBindingReceiptV3,
    receipts: &Receipts,
) -> Result<ValidatedCompilerProofInputsV4, CompilerProofInputValidationErrorV4> {
    validate_compiler_proof_inputs_v4(
        proof_binding,
        &receipts.semantic_mir,
        &receipts.middle_end,
        &receipts.kernel_ir,
        &receipts.correspondence,
        &receipts.formal_memory,
    )
}

#[test]
fn exact_current_inputs_reimport_the_signed_verus_receipt() {
    let receipts = receipts(0);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let proof_binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&proof_binding, &receipts).unwrap();

    assert_eq!(validated.receipt_identity(), proof_binding.identity());
    assert_eq!(
        validated.association().verus_execution_evidence(),
        evidence.canonical_bytes()
    );
    assert_eq!(
        validated.verus_execution().canonical_bytes(),
        evidence.canonical_bytes()
    );
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
    assert!(validated.has_exact_decoded_input_association());
    assert!(validated.authenticates_signed_verus_receipt_under_embedded_key());
    assert!(!validated.authenticates_compiler_origin());
    assert!(!validated.establishes_llvm_or_machine_refinement());
    assert!(!validated.grants_runtime_authority());
}

#[test]
fn every_outer_stage_identity_substitution_fails_closed() {
    let fields = [
        "semantic MIR",
        "middle end",
        "Kernel IR",
        "MIR-to-KIR correspondence",
        "formal memory",
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let receipts = receipts(0);
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let proof_binding = proof_binding(&receipts, Some(index), evidence.canonical_bytes());
        assert!(matches!(
            validate(&proof_binding, &receipts),
            Err(CompilerProofInputValidationErrorV4::ProofBindingIdentityMismatch {
                field: actual
            }) if actual == field
        ));
    }
}

#[test]
fn malformed_nested_verus_evidence_fails_at_the_signed_codec() {
    let receipts = receipts(0);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let mut malformed = evidence.canonical_bytes().to_vec();
    malformed[0] ^= 0xff;
    let proof_binding = proof_binding(&receipts, None, &malformed);
    assert!(matches!(
        validate(&proof_binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::VerusEvidence(_))
    ));
}

#[test]
fn signed_receipt_for_a_different_middle_end_fails_closed() {
    let current = receipts(0);
    let other = receipts(1);
    let evidence = signed_verus_evidence(exact_pliron_identity(&other));
    let proof_binding = proof_binding(&current, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&proof_binding, &current),
        Err(CompilerProofInputValidationErrorV4::VerusMiddleEndMismatch)
    ));
}

#[test]
fn malformed_shared_stage_still_fails_through_the_single_stage_decoder() {
    let mut receipts = receipts(0);
    receipts.middle_end =
        InertMiddleEndReceiptV3::from_canonical_preimage(b"bad middle".to_vec()).unwrap();
    let evidence = signed_verus_evidence(digest(99));
    let proof_binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&proof_binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(_))
    ));
}
