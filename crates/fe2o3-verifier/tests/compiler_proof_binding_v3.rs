use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV3, InertProofBindingAssociationV3,
    InertProofBindingReceiptV3,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV5;
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV3,
    ProductionFormalMemoryOwnerV1, ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5, PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5,
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
};
use fe2o3_verifier::{CompilerProofInputValidationErrorV3, validate_compiler_proof_inputs_v3};
use sha2::{Digest, Sha256};

struct Receipts {
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
}

fn bytes(tag: u8, seed: u8) -> [u8; 32] {
    [tag.wrapping_add(seed); 32]
}

fn unit_type(seed: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4, seed)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4, seed)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    )
}

fn block(
    tag: u8,
    seed: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(tag, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn semantic_owner(seed: u8) -> ProductionSemanticMirOwnerV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let layout = SemanticLayoutIdentityV1::from_sha256(bytes(250, seed));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2, seed)),
        layout,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Nop,
    );
    let block0 = block(10, seed, vec![statement], SemanticTerminatorKindV1::Return);
    let block1 = block(
        11,
        seed,
        vec![],
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(0),
        )),
    );
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2, seed)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2, seed)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2, seed)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2, seed)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2, seed)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3, seed)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(1),
        vec![block0, block1],
    )
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let launch =
        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None).unwrap();
    let contract = SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
    let function = function.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(format!("proof_input_test_{seed}").into_bytes()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5, seed)),
        contract,
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout),
        vec![unit_type(seed)],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn middle_end_v5_bytes(source_semantic_identity: [u8; 32], seed: u8) -> Vec<u8> {
    const IDENTITY_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V5\0";
    const RANKED_IR: &[u8] = b"func @proof_input_test {\n  kernel.return\n}\n";
    const PASS_TAGS: [u8; 8] = [1, 2, 3, 4, 8, 5, 6, 7];
    const DECLARED_LENGTH_OFFSET: usize = 12;

    let mut encoded = Vec::new();
    encoded.extend_from_slice(b"F2MEV5\0\0");
    encoded.extend_from_slice(&5_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&0_u64.to_le_bytes());
    encoded.extend_from_slice(&0_u32.to_le_bytes());
    encoded
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5.len() as u16).to_le_bytes());
    encoded.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V5);
    encoded
        .extend_from_slice(&(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5.len() as u16).to_le_bytes());
    encoded.extend_from_slice(PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V5);
    encoded.push(1);
    encoded.push(1);
    encoded.extend_from_slice(&0_u16.to_le_bytes());
    encoded.extend_from_slice(&source_semantic_identity);
    encoded.extend_from_slice(&bytes(90, seed));
    encoded.extend_from_slice(&(RANKED_IR.len() as u32).to_le_bytes());
    encoded.extend_from_slice(RANKED_IR);
    encoded.push(PASS_TAGS.len() as u8);
    for tag in PASS_TAGS {
        encoded.push(tag);
        encoded.push(1);
        encoded.extend_from_slice(&0_u32.to_le_bytes());
        encoded.push(0);
        encoded.push(0);
        encoded.extend_from_slice(&0_u16.to_le_bytes());
    }
    for _ in 0..4 + 6 + 10 + 2 {
        encoded.extend_from_slice(&0_u64.to_le_bytes());
    }
    encoded.extend_from_slice(&bytes(91, seed));

    let total_length = encoded.len() + 32;
    encoded[DECLARED_LENGTH_OFFSET..DECLARED_LENGTH_OFFSET + 8]
        .copy_from_slice(&(total_length as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update((encoded.len() as u64).to_le_bytes());
    digest.update(&encoded);
    encoded.extend_from_slice(&<[u8; 32]>::from(digest.finalize()));
    assert_eq!(encoded.len(), total_length);
    encoded
}

fn receipts(seed: u8) -> Receipts {
    let semantic_kir = ProductionSemanticKirOwnerV1::try_lower(
        semantic_owner(seed),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    let semantic = semantic_kir.semantic().semantic();
    let semantic_bytes = semantic.canonical_encoding().to_vec();
    let semantic_identity = *semantic.semantic_sha256().as_bytes();
    let kernel_ir =
        VerifiedCanonicalKernelIrV5::from_module(semantic_kir.module().clone()).unwrap();
    let correspondence =
        InertCanonicalMirToKirCorrespondenceEvidenceV3::from_live_owner(&semantic_kir).unwrap();
    let formal_owner = ProductionFormalMemoryOwnerV1::try_admit(semantic_kir).unwrap();
    let formal =
        InertCanonicalFormalMemoryAdmissionEvidenceV3::from_live_owner(&formal_owner).unwrap();

    Receipts {
        semantic_mir: InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(semantic_bytes)
            .unwrap(),
        middle_end: InertMiddleEndReceiptV3::from_canonical_preimage(middle_end_v5_bytes(
            semantic_identity,
            seed,
        ))
        .unwrap(),
        kernel_ir: InertKernelIrReceiptV3::from_canonical_preimage(
            kernel_ir.into_canonical_bytes(),
        )
        .unwrap(),
        correspondence: InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
            correspondence.into_canonical_bytes(),
        )
        .unwrap(),
        formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(
            formal.into_canonical_bytes(),
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
