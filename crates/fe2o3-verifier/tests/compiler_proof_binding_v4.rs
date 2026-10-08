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
    CanonicalProductionMirPlironVerusExecutionEvidenceV1, CompilerProofInputValidationErrorV3,
    CompilerProofInputValidationErrorV4, ProductionMirPlironVerusExecutionClaimsV1,
    ValidatedCompilerProofInputsV4, validate_compiler_proof_inputs_v4,
};

#[path = "../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;
#[path = "support/conditional_output_evidence.rs"]
mod conditional_output_evidence;
#[path = "support/guarded_v9_proof_inputs.rs"]
mod guarded_v9_proof_inputs;
use compiler_proof_inputs_v3::{
    CanonicalCompilerProofInputsV3, canonical_compiler_proof_inputs_v4,
    canonical_compiler_proof_inputs_v4_with_induction,
    canonical_compiler_proof_inputs_with_v5_correspondence,
};

struct Receipts {
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
}

fn conditional_receipts() -> Receipts {
    use sha2::{Digest as _, Sha256};
    let mut receipts = receipts_from(guarded_v9_proof_inputs::inputs(0));
    // Synthetic transport fixture only. Genuine live counters and proof execution
    // are exercised by production_extraction_driver_v1, not this byte mutation.
    let mut bytes = receipts.middle_end.canonical_preimage().to_vec();
    let coverage_start = bytes.len() - (22 * 8 + 32 + 32);
    for counter in [6, 7] {
        put_u64(&mut bytes, coverage_start + counter * 8, 1);
    }
    let terminal = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/PRODUCTION-MIDDLE-END-EVIDENCE-IDENTITY/V5\0");
    hash.update((terminal as u64).to_le_bytes());
    hash.update(&bytes[..terminal]);
    bytes[terminal..].copy_from_slice(&hash.finalize());
    receipts.middle_end = InertMiddleEndReceiptV3::from_canonical_preimage(bytes).unwrap();
    receipts
}

fn conditional_evidence(receipts: &Receipts, substitute: Option<usize>) -> Vec<u8> {
    let middle =
        InertProductionMiddleEndEvidenceV5::decode(receipts.middle_end.canonical_preimage())
            .unwrap();
    let mut identities = [
        DigestV1::from_untrusted_bytes(*middle.identity().sha256()),
        DigestV1::from_untrusted_bytes(*middle.source_semantic_identity()),
        DigestV1::from_untrusted_bytes(*middle.ranked_kernel_identity()),
        digest(4),
    ];
    if let Some(index) = substitute {
        identities[index] = digest(200);
    }
    conditional_output_evidence::signed(&conditional_output_evidence::preimage(identities))
}

fn validate_conditional(
    binding: &InertProofBindingReceiptV3,
    receipts: &Receipts,
) -> Result<
    fe2o3_verifier::ValidatedConditionalCompilerProofInputsV1,
    fe2o3_verifier::ConditionalCompilerProofInputValidationErrorV1,
> {
    fe2o3_verifier::validate_conditional_compiler_proof_inputs_v1(
        binding,
        &receipts.semantic_mir,
        &receipts.middle_end,
        &receipts.kernel_ir,
        &receipts.correspondence,
        &receipts.formal_memory,
    )
}

#[test]
fn conditional_transport_retains_v9_without_unconditional_or_runtime_authority() {
    let receipts = conditional_receipts();
    let evidence = conditional_evidence(&receipts, None);
    let binding = proof_binding(&receipts, None, &evidence);
    let validated = validate_conditional(&binding, &receipts).unwrap();
    assert_eq!(validated.receipt_identity(), binding.identity());
    assert_eq!(validated.kernel_ir().wire_version(), 9);
    assert_eq!(
        validated.kernel_ir().canonical_bytes(),
        receipts.kernel_ir.canonical_preimage()
    );
    assert_eq!(validated.verus_execution().canonical_bytes(), evidence);
    assert_eq!(
        validated
            .verus_execution()
            .obligation()
            .ranked_extent_argument(),
        2
    );
    assert_eq!(
        validated
            .verus_execution()
            .obligation()
            .reference_output_argument(),
        0
    );
    assert!(validated.has_lossless_mir_to_kir_correspondence());
    assert!(validated.requires_packed_extent_and_launch_discharge());
    assert!(!validated.authenticates_compiler_origin());
    assert!(!validated.establishes_llvm_or_machine_refinement());
    assert!(!validated.grants_runtime_authority());
    assert!(validate(&binding, &receipts).is_err());
    let unconditional = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, unconditional.canonical_bytes());
    assert!(validate_conditional(&binding, &receipts).is_err());
}

#[test]
fn conditional_import_rejects_all_outer_and_signed_identity_substitutions() {
    use fe2o3_verifier::ConditionalCompilerProofInputValidationErrorV1 as E;
    let receipts = conditional_receipts();
    let evidence = conditional_evidence(&receipts, None);
    for index in 0..5 {
        let binding = proof_binding(&receipts, Some(index), &evidence);
        assert!(matches!(
            validate_conditional(&binding, &receipts),
            Err(E::Stage(
                CompilerProofInputValidationErrorV4::ProofBindingIdentityMismatch { .. }
            ))
        ));
    }
    for index in 0..3 {
        let evidence = conditional_evidence(&receipts, Some(index));
        let binding = proof_binding(&receipts, None, &evidence);
        assert!(matches!(
            validate_conditional(&binding, &receipts),
            Err(E::IdentityMismatch(_))
        ));
    }
}

#[test]
fn conditional_import_requires_nonvacuous_effect_and_exact_correspondence() {
    use fe2o3_verifier::ConditionalCompilerProofInputValidationErrorV1 as E;
    let empty = receipts_from(guarded_v9_proof_inputs::inputs(0));
    let binding = proof_binding(&empty, None, &conditional_evidence(&empty, None));
    assert!(matches!(
        validate_conditional(&binding, &empty),
        Err(E::UnsupportedProfile)
    ));
    let mut receipts = conditional_receipts();
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    bytes[64] ^= 1;
    replace_correspondence(&mut receipts, bytes);
    let binding = proof_binding(&receipts, None, &conditional_evidence(&receipts, None));
    assert!(matches!(
        validate_conditional(&binding, &receipts),
        Err(E::Stage(CompilerProofInputValidationErrorV4::Stage(error))) if matches!(*error, CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                field: "current production Kernel IR custody"
            })
    ));
}

#[test]
fn exact_v5_function_roster_survives_unconditional_import() {
    let receipts = receipts_from(canonical_compiler_proof_inputs_with_v5_correspondence(0));
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&binding, &receipts).unwrap();
    let original = receipts.correspondence.canonical_preimage();
    assert_eq!(&original[..8], b"F2M2K5\0\0");
    assert_eq!(validated.exact_correspondence_bytes(), original);
    assert_ne!(validated.correspondence().canonical_bytes(), original);
    let decoded =
        fe2o3_lower_mir_kernel::InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(original)
            .unwrap();
    assert_eq!(
        validated.correspondence().canonical_bytes(),
        decoded.nested_v4().canonical_bytes()
    );
    assert!(!validated.grants_runtime_authority());
}

#[test]
fn validly_encoded_v5_function_substitutions_reject_against_bound_kir() {
    use fe2o3_lower_mir_kernel::{
        InertCanonicalMirToKirCorrespondenceEvidenceV5, ProductionCorrespondenceEvidenceErrorV5,
    };
    for change_ordinal in [false, true] {
        let mut receipts = receipts_from(canonical_compiler_proof_inputs_with_v5_correspondence(0));
        let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
        let nested_len = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
        let record = 28 + nested_len;
        if change_ordinal {
            bytes[record + 8..record + 12].copy_from_slice(&u32::MAX.to_le_bytes());
        } else {
            // A same-length name remains canonical but no longer names its KIR function.
            bytes[record + 20] ^= 1;
        }
        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(&bytes).unwrap();
        replace_correspondence(&mut receipts, bytes);
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
        assert!(matches!(
            validate(&binding, &receipts),
            Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::CorrespondenceV5Decode(
                    ProductionCorrespondenceEvidenceErrorV5::InvalidFunctionRoster
                ))
        ));
    }
}

#[test]
fn truncated_v5_correspondence_rejects_before_import() {
    let mut receipts = receipts_from(canonical_compiler_proof_inputs_with_v5_correspondence(0));
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    bytes.pop().unwrap();
    replace_correspondence(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::CorrespondenceV5Decode(_))
    ));
}

#[test]
fn actual_guarded_store_lowering_reaches_exact_v9_singleton_admission() {
    let receipts = receipts_from(guarded_v9_proof_inputs::inputs(0));
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&binding, &receipts).unwrap();
    assert_eq!(validated.kernel_ir().wire_version(), 9);
    assert!(validated.kernel_ir().as_v9().is_some());
    assert!(validated.kernel_ir().as_v8().is_none());
    assert_eq!(
        validated.kernel_ir().canonical_bytes(),
        receipts.kernel_ir.canonical_preimage()
    );
    assert_eq!(
        validated.formal_memory().canonical_bytes(),
        receipts.formal_memory.canonical_preimage()
    );
    assert_eq!(
        u16::from_le_bytes(
            receipts.formal_memory.canonical_preimage()[128..130]
                .try_into()
                .unwrap()
        ),
        fe2o3_kernel_ir::FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V3
    );
    assert_eq!(
        u16::from_le_bytes(
            receipts.formal_memory.canonical_preimage()[130..132]
                .try_into()
                .unwrap()
        ),
        2
    );
    assert_eq!(
        validated.formal_memory().validation_policy(),
        fe2o3_lower_mir_kernel::FormalMemoryAdmissionValidationPolicyV4::GuardedV2
    );
    assert!(validated.has_lossless_mir_to_kir_correspondence());
    assert!(validated.authenticates_signed_verus_receipt_under_embedded_key());
    assert!(!validated.authenticates_compiler_origin());
    assert!(!validated.establishes_llvm_or_machine_refinement());
    assert!(!validated.grants_runtime_authority());
}

#[test]
fn v9_bytes_cannot_be_decoded_under_a_v8_claim() {
    let mut receipts = receipts_from(guarded_v9_proof_inputs::inputs(0));
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    bytes[52..54].copy_from_slice(&8_u16.to_le_bytes());
    replace_correspondence(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::KernelIrV8(_))
    ));
}

#[test]
fn v9_nested_custody_requires_the_decoded_version_digest_and_length() {
    for axis in 0..5 {
        let mut receipts = receipts_from(guarded_v9_proof_inputs::inputs(0));
        let mut correspondence = receipts.correspondence.canonical_preimage().to_vec();
        let mut formal = receipts.formal_memory.canonical_preimage().to_vec();
        match axis {
            0 => formal[20..22].copy_from_slice(&8_u16.to_le_bytes()),
            1 => correspondence[64] ^= 1,
            2 => formal[32] ^= 1,
            3 => {
                correspondence[64] ^= 1;
                formal[32] ^= 1;
            }
            4 => {
                let length = u64_at(&correspondence, 56) + 1;
                put_u64(&mut correspondence, 56, length);
                put_u64(&mut formal, 24, length);
            }
            _ => unreachable!(),
        }
        replace_correspondence(&mut receipts, correspondence);
        replace_formal_memory(&mut receipts, formal);
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
        assert!(
            matches!(
                validate(&binding, &receipts),
                Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                        field: "current production Kernel IR custody",
                    })
            ),
            "axis {axis}"
        );
    }
}

#[test]
fn malformed_truncated_and_trailing_v9_bytes_are_not_repaired() {
    for axis in 0..3 {
        let mut receipts = receipts_from(guarded_v9_proof_inputs::inputs(0));
        let mut bytes = receipts.kernel_ir.canonical_preimage().to_vec();
        match axis {
            0 => bytes[0] ^= 1,
            1 => {
                bytes.pop().unwrap();
            }
            2 => bytes.push(0),
            _ => unreachable!(),
        }
        receipts.kernel_ir = InertKernelIrReceiptV3::from_canonical_preimage(bytes).unwrap();
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
        assert!(
            matches!(
                validate(&binding, &receipts),
                Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::KernelIrV9(_))
            ),
            "axis {axis}"
        );
    }
}

#[test]
fn write_only_formal_obligation_receipt_cannot_be_downgraded_or_retagged() {
    for version in [1_u16, 2, 4] {
        let mut receipts = receipts_from(guarded_v9_proof_inputs::inputs(0));
        let original = receipts.formal_memory.canonical_preimage();
        assert_eq!(
            u16::from_le_bytes(original[128..130].try_into().unwrap()),
            fe2o3_kernel_ir::FORMAL_MEMORY_OBLIGATION_RECEIPT_VERSION_V3
        );
        assert_eq!(
            u16::from_le_bytes(original[130..132].try_into().unwrap()),
            2
        );
        let mut bytes = original.to_vec();
        // V4's 120-byte header precedes the guarded V3 receipt with policy 2.
        bytes[128..130].copy_from_slice(&version.to_le_bytes());
        assert_ne!(
            bytes.as_slice(),
            original,
            "version mutation must change bytes"
        );
        replace_formal_memory(&mut receipts, bytes);
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
        assert!(
            matches!(
                validate(&binding, &receipts),
                Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::FormalMemoryV4Decode(
                        fe2o3_lower_mir_kernel::ProductionFormalMemoryEvidenceErrorV4::FormalReceipt(_)
                    ))
            ),
            "retagged guarded receipt as version {version}"
        );
    }
}

#[test]
fn v1_formal_obligations_without_write_only_allocations_cannot_claim_v2() {
    let mut receipts = receipts(0);
    let mut bytes = receipts.formal_memory.canonical_preimage().to_vec();
    bytes[128..130].copy_from_slice(&2_u16.to_le_bytes());
    replace_formal_memory(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::FormalMemoryV4Decode(
                fe2o3_lower_mir_kernel::ProductionFormalMemoryEvidenceErrorV4::FormalReceipt(_)
            ))
    ));
}

#[test]
fn formal_witness_count_must_match_the_nested_receipt_for_both_versions() {
    for mut receipts in [
        receipts(0),
        receipts_from(guarded_v9_proof_inputs::inputs(0)),
    ] {
        let mut bytes = receipts.formal_memory.canonical_preimage().to_vec();
        let count = u64_at(&bytes, 96) + 1;
        put_u64(&mut bytes, 96, count);
        replace_formal_memory(&mut receipts, bytes);
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
        assert!(matches!(
            validate(&binding, &receipts),
            Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::FormalMemoryV4Decode(
                    fe2o3_lower_mir_kernel::ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission
                ))
        ));
    }
}

#[test]
fn v9_encoding_is_retained_even_when_its_module_is_representable_in_v8() {
    // Codec-only fixture, distinct from the actual guarded-store lowering positive above.
    let mut receipts = receipts(0);
    let module =
        fe2o3_kernel_ir::decode_module_v8(receipts.kernel_ir.canonical_preimage()).unwrap();
    let v9 = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
    let mut correspondence = receipts.correspondence.canonical_preimage().to_vec();
    correspondence[52..54].copy_from_slice(&9_u16.to_le_bytes());
    put_u64(&mut correspondence, 56, v9.identity().canonical_length());
    correspondence[64..96].copy_from_slice(v9.identity().digest());
    let mut formal = receipts.formal_memory.canonical_preimage().to_vec();
    formal[20..22].copy_from_slice(&9_u16.to_le_bytes());
    put_u64(&mut formal, 24, v9.identity().canonical_length());
    formal[32..64].copy_from_slice(v9.identity().digest());
    replace_correspondence(&mut receipts, correspondence);
    replace_formal_memory(&mut receipts, formal);
    receipts.kernel_ir =
        InertKernelIrReceiptV3::from_canonical_preimage(v9.canonical_bytes().to_vec()).unwrap();
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&binding, &receipts).unwrap();
    assert_eq!(validated.kernel_ir().wire_version(), 9);
    assert_eq!(
        validated.kernel_ir().canonical_bytes(),
        v9.canonical_bytes()
    );
    assert_eq!(
        validated.kernel_ir().identity_digest(),
        v9.identity().digest()
    );
    assert!(validated.kernel_ir().as_v8().is_none());
    assert!(!validated.grants_runtime_authority());
}

const CORRESPONDENCE_HEADER_BYTES_V4: usize = 124;
const BLOCK_RECORD_BYTES_V4: usize = 16;
const STATEMENT_RECORD_BYTES_V4: usize = 24;
const TERMINATOR_RECORD_BYTES_V4: usize = 20;
const SYNTHETIC_RECORD_BYTES_V4: usize = 16;
const PARAMETER_RECORD_BYTES_V4: usize = 12;

#[derive(Clone, Copy)]
struct CorrespondenceSectionsV4 {
    statement_start: usize,
    statement_count: usize,
    synthetic_start: usize,
    synthetic_count: usize,
    parameter_start: usize,
    parameter_count: usize,
    induction_start: usize,
}

fn receipts(seed: u8) -> Receipts {
    receipts_from(canonical_compiler_proof_inputs_v4(seed))
}

fn induction_receipts(seed: u8) -> Receipts {
    receipts_from(canonical_compiler_proof_inputs_v4_with_induction(seed))
}

fn receipts_from(inputs: CanonicalCompilerProofInputsV3) -> Receipts {
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

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn correspondence_sections(bytes: &[u8]) -> CorrespondenceSectionsV4 {
    assert!(bytes.len() >= CORRESPONDENCE_HEADER_BYTES_V4);
    let block_count = u32_at(bytes, 100) as usize;
    let statement_count = u32_at(bytes, 104) as usize;
    let terminator_count = u32_at(bytes, 108) as usize;
    let synthetic_count = u32_at(bytes, 112) as usize;
    let parameter_count = u32_at(bytes, 116) as usize;
    let statement_start = CORRESPONDENCE_HEADER_BYTES_V4 + block_count * BLOCK_RECORD_BYTES_V4;
    let terminator_start = statement_start + statement_count * STATEMENT_RECORD_BYTES_V4;
    let synthetic_start = terminator_start + terminator_count * TERMINATOR_RECORD_BYTES_V4;
    let parameter_start = synthetic_start + synthetic_count * SYNTHETIC_RECORD_BYTES_V4;
    let induction_start = parameter_start + parameter_count * PARAMETER_RECORD_BYTES_V4;
    assert!(induction_start < bytes.len());
    CorrespondenceSectionsV4 {
        statement_start,
        statement_count,
        synthetic_start,
        synthetic_count,
        parameter_start,
        parameter_count,
        induction_start,
    }
}

fn statement_record_offset(
    bytes: &[u8],
    sections: CorrespondenceSectionsV4,
    function: u32,
    block: u32,
    statement: u32,
) -> usize {
    (0..sections.statement_count)
        .map(|index| sections.statement_start + index * STATEMENT_RECORD_BYTES_V4)
        .find(|offset| {
            u32_at(bytes, *offset) == function
                && u32_at(bytes, *offset + 4) == block
                && u32_at(bytes, *offset + 8) == statement
        })
        .expect("fixture contains the requested statement span")
}

fn replace_correspondence(receipts: &mut Receipts, canonical_bytes: Vec<u8>) {
    receipts.correspondence =
        InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(canonical_bytes).unwrap();
}

fn replace_formal_memory(receipts: &mut Receipts, canonical_bytes: Vec<u8>) {
    receipts.formal_memory =
        InertFormalMemoryReceiptV3::from_canonical_preimage(canonical_bytes).unwrap();
}

fn assert_structural_failure(receipts: &Receipts, expected: &'static str) {
    let evidence = signed_verus_evidence(exact_pliron_identity(receipts));
    let proof_binding = proof_binding(receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&proof_binding, receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error))
            if matches!(*error,
                CompilerProofInputValidationErrorV3::StructuralCorrespondence { detail }
                    if detail == expected)
    ));
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
fn v4_validation_error_stays_below_result_large_err_threshold() {
    let size = std::mem::size_of::<CompilerProofInputValidationErrorV4>();
    assert!(size < 128, "V4 validation error occupies {size} bytes");
}

#[test]
fn v4_stage_error_preserves_display_and_direct_source_downcast() {
    let error = CompilerProofInputValidationErrorV4::Stage(Box::new(
        CompilerProofInputValidationErrorV3::StructuralCorrespondence {
            detail: "stage regression",
        },
    ));
    assert_eq!(
        error.to_string(),
        "current compiler proof stage failed: compiler proof inputs have invalid structural correspondence: stage regression"
    );
    let source = std::error::Error::source(&error).unwrap();
    let direct = source
        .downcast_ref::<CompilerProofInputValidationErrorV3>()
        .unwrap();
    let CompilerProofInputValidationErrorV4::Stage(stage) = &error else {
        panic!("stage error changed variant");
    };
    assert!(std::ptr::eq(direct, stage.as_ref()));
    assert!(matches!(
        direct,
        CompilerProofInputValidationErrorV3::StructuralCorrespondence {
            detail: "stage regression"
        }
    ));
    assert!(source.source().is_none());
}

#[test]
fn exact_current_inputs_reimport_the_signed_verus_receipt() {
    use fe2o3_verifier::ValidatedCompilerMultiRootKernelIrV1::{V8, V9, V11};

    let receipts = receipts(0);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let proof_binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&proof_binding, &receipts).unwrap();

    assert_eq!(validated.receipt_identity(), proof_binding.identity());
    assert_eq!(
        validated.exact_correspondence_bytes(),
        receipts.correspondence.canonical_preimage()
    );
    assert_eq!(
        validated.exact_correspondence_bytes(),
        validated.correspondence().canonical_bytes()
    );
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
    assert_eq!(validated.kernel_ir().wire_version(), 8);
    assert!(validated.kernel_ir().as_v8().is_some());
    assert!(validated.kernel_ir().as_v9().is_none());
    assert!(matches!(validated.kernel_ir(), V8(_)));
    assert!(!matches!(validated.kernel_ir(), V9(_) | V11(_)));
    assert!(validated.has_exact_decoded_input_association());
    assert!(validated.has_lossless_mir_to_kir_correspondence());
    assert!(validated.semantic_u32_induction_kir_anchors().is_empty());
    assert!(validated.authenticates_signed_verus_receipt_under_embedded_key());
    assert!(!validated.authenticates_compiler_origin());
    assert!(!validated.establishes_llvm_or_machine_refinement());
    assert!(!validated.grants_runtime_authority());
}

#[test]
fn guarded_policy_two_inputs_reimport_exact_source_and_signed_test_evidence() {
    use fe2o3_kernel_ir::{
        FormalMemoryReceiptEncodingV3, InertFormalMemoryReceiptFormatV3, OperationKind,
    };
    use fe2o3_lower_mir_kernel::FormalMemoryAdmissionValidationPolicyV4;
    let receipts = receipts_from(
        compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4_with_guarded_read(7),
    );
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&binding, &receipts).unwrap();
    assert_eq!(
        validated.formal_memory().validation_policy(),
        FormalMemoryAdmissionValidationPolicyV4::GuardedV2
    );
    assert_eq!(
        validated.formal_memory().canonical_bytes(),
        receipts.formal_memory.canonical_preimage()
    );
    let nested = InertFormalMemoryReceiptFormatV3::decode_current(
        receipts.formal_memory.canonical_preimage()[120..].to_vec(),
    )
    .unwrap();
    assert_eq!(
        nested.metadata().encoding(),
        FormalMemoryReceiptEncodingV3::GuardedV3
    );
    assert!(!nested.grants_authority());
    assert!(
        fe2o3_kernel_ir::decode_module_v8(validated.kernel_ir().canonical_bytes())
            .unwrap()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::GuardedLoad { .. }))
    );
    assert!(validated.has_exact_decoded_input_association());
    assert!(validated.has_lossless_mir_to_kir_correspondence());
    assert!(validated.authenticates_signed_verus_receipt_under_embedded_key());
    assert!(!validated.authenticates_compiler_origin());
    assert!(!validated.establishes_llvm_or_machine_refinement());
    assert!(!validated.grants_runtime_authority());
}

#[test]
fn guarded_outer_policy_substitution_is_rejected_even_with_rebound_test_association() {
    use fe2o3_lower_mir_kernel::ProductionFormalMemoryEvidenceErrorV4;
    let mut receipts = receipts_from(
        compiler_proof_inputs_v3::canonical_compiler_proof_inputs_v4_with_guarded_read(8),
    );
    let mut bytes = receipts.formal_memory.canonical_preimage().to_vec();
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    replace_formal_memory(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(stage))
            if matches!(stage.as_ref(),
                CompilerProofInputValidationErrorV3::FormalMemoryV4Decode(
                    ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission
                ))
    ));
}

#[test]
fn exact_induction_certificate_is_anchored_to_one_checked_kir_addition() {
    let receipts = induction_receipts(0);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let proof_binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    let validated = validate(&proof_binding, &receipts).unwrap();

    let [anchor] = validated.semantic_u32_induction_kir_anchors() else {
        panic!("the exact induction fixture must retain one checked-add anchor");
    };
    assert_eq!(anchor.semantic_function(), 0);
    assert_eq!(anchor.semantic_block(), 2);
    assert_eq!(anchor.semantic_statement(), 1);
    assert_eq!(anchor.kernel_ir_block(), 2);
    assert_ne!(anchor.value_result(), anchor.overflow_result());
    assert!(validated.has_lossless_mir_to_kir_correspondence());
    assert!(!validated.establishes_llvm_or_machine_refinement());
}

#[test]
fn checked_addition_cannot_be_reassigned_to_an_adjacent_nop_span() {
    let mut receipts = induction_receipts(0);
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    let sections = correspondence_sections(&bytes);
    let nop = statement_record_offset(&bytes, sections, 0, 2, 0);
    let checked = statement_record_offset(&bytes, sections, 0, 2, 1);
    let checked_first = u32_at(&bytes, checked + 16);
    let checked_count = u32_at(&bytes, checked + 20);
    assert_ne!(checked_count, 0);
    put_u32(&mut bytes, nop + 16, checked_first);
    put_u32(&mut bytes, nop + 20, checked_count);
    put_u32(
        &mut bytes,
        checked + 16,
        checked_first.checked_add(checked_count).unwrap(),
    );
    put_u32(&mut bytes, checked + 20, 0);
    replace_correspondence(&mut receipts, bytes);

    assert_structural_failure(
        &receipts,
        "induction statement span contains no checked KIR addition",
    );
}

#[test]
fn retained_induction_report_must_equal_deterministic_replay() {
    let mut receipts = induction_receipts(0);
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    let sections = correspondence_sections(&bytes);
    let work_units = sections.induction_start + 92;
    let mutated_work_units = u64_at(&bytes, work_units).checked_add(1).unwrap();
    put_u64(&mut bytes, work_units, mutated_work_units);
    replace_correspondence(&mut receipts, bytes);

    assert_structural_failure(
        &receipts,
        "retained semantic induction report differs from deterministic replay",
    );
}

#[test]
fn semantic_argument_cannot_be_rebound_to_another_kir_value() {
    let mut receipts = induction_receipts(0);
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    let sections = correspondence_sections(&bytes);
    assert_ne!(sections.parameter_count, 0);
    let value = sections.parameter_start + 8;
    let mutated_value = u32_at(&bytes, value).checked_add(1).unwrap();
    put_u32(&mut bytes, value, mutated_value);
    replace_correspondence(&mut receipts, bytes);

    assert_structural_failure(
        &receipts,
        "semantic argument binding names a different KIR parameter",
    );
}

#[test]
fn runtime_assert_trap_requires_exact_synthetic_custody() {
    let mut receipts = induction_receipts(0);
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    let sections = correspondence_sections(&bytes);
    assert_ne!(sections.synthetic_count, 0);
    let trap = (0..sections.synthetic_count)
        .map(|index| sections.synthetic_start + index * SYNTHETIC_RECORD_BYTES_V4)
        .find(|offset| u32_at(&bytes, *offset) == 2)
        .expect("fixture contains the runtime-assert trap span");
    put_u32(&mut bytes, trap + 12, 0);
    replace_correspondence(&mut receipts, bytes);

    assert_structural_failure(
        &receipts,
        "unmapped KIR block is not the canonical runtime-assert trap",
    );
}

#[test]
fn independently_well_formed_kir_custody_substitutions_fail_closed() {
    for mutate_correspondence in [true, false] {
        let mut receipts = induction_receipts(0);
        if mutate_correspondence {
            let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
            bytes[64] ^= 1;
            replace_correspondence(&mut receipts, bytes);
        } else {
            let mut bytes = receipts.formal_memory.canonical_preimage().to_vec();
            bytes[32] ^= 1;
            replace_formal_memory(&mut receipts, bytes);
        }
        let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
        let proof_binding = proof_binding(&receipts, None, evidence.canonical_bytes());
        assert!(matches!(
            validate(&proof_binding, &receipts),
            Err(CompilerProofInputValidationErrorV4::Stage(error))
                if matches!(*error,
                    CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                        field: "current production Kernel IR custody"
                    })
        ));
    }
}

#[test]
fn claimed_v9_cannot_reinterpret_v8_bytes() {
    let mut receipts = receipts(0);
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    bytes[52..54].copy_from_slice(&9_u16.to_le_bytes());
    replace_correspondence(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::KernelIrV9(_))
    ));
}

#[test]
fn singleton_contract_rejects_claimed_v11_before_version_projection() {
    let mut receipts = receipts(0);
    let mut bytes = receipts.correspondence.canonical_preimage().to_vec();
    bytes[52..54].copy_from_slice(&11_u16.to_le_bytes());
    replace_correspondence(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::UnsupportedKernelIrVersion {
                version: fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V11,
            })
    ));
}

#[test]
fn formal_memory_version_must_match_the_exact_decoded_owner() {
    let mut receipts = receipts(0);
    let mut bytes = receipts.formal_memory.canonical_preimage().to_vec();
    bytes[20..22].copy_from_slice(&9_u16.to_le_bytes());
    replace_formal_memory(&mut receipts, bytes);
    let evidence = signed_verus_evidence(exact_pliron_identity(&receipts));
    let binding = proof_binding(&receipts, None, evidence.canonical_bytes());
    assert!(matches!(
        validate(&binding, &receipts),
        Err(CompilerProofInputValidationErrorV4::Stage(error)) if matches!(*error, CompilerProofInputValidationErrorV3::NestedIdentityMismatch {
                field: "current production Kernel IR custody",
            })
    ));
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
