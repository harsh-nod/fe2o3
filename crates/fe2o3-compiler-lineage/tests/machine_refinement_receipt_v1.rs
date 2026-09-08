use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptIdentityV1, InertCapabilityRefinementReceiptKindV1,
    InertCapabilityRefinementReceiptV1, MachineRefinementContentIdentityV1,
    MachineRefinementFamilyV1, TargetMachineRefinementReceiptErrorV1,
    TargetMachineRefinementReceiptPartsV1, TargetMachineRefinementReceiptV1,
    TargetMachineRefinementTargetV1, check_target_machine_refinement_receipt_v1,
    decode_compiler_instruction_selection_correspondence_identity_v1,
    decode_exact_compiler_stage_content_identity_v1, decode_post_llvm_stage_custody_identity_v1,
};
use sha2::{Digest as _, Sha256};

const LEGACY_MACHINE_RECEIPT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/CAPABILITY/MACHINE-REFINEMENT-RECEIPT/V1\0";

fn parts() -> TargetMachineRefinementReceiptPartsV1 {
    let identity = |tag: u8| {
        let mut value = [0; 32];
        value[0] = tag;
        value
    };
    let required = MachineRefinementFamilyV1::ControlFlow.bit()
        | MachineRefinementFamilyV1::GlobalMemory.bit();
    TargetMachineRefinementReceiptPartsV1 {
        target: TargetMachineRefinementTargetV1::Gfx942,
        post_llvm_custody: decode_post_llvm_stage_custody_identity_v1(identity(1), 101).unwrap(),
        instruction_selection: decode_compiler_instruction_selection_correspondence_identity_v1(
            identity(2),
            102,
        )
        .unwrap(),
        decoded_isa: MachineRefinementContentIdentityV1::new(identity(3), 103).unwrap(),
        final_code_object: decode_exact_compiler_stage_content_identity_v1(identity(4), 104)
            .unwrap(),
        machine_refinement_sha256: identity(5),
        required_families: required,
        established_families: required,
    }
}

#[test]
fn machine_capability_receipt_requires_the_typed_canonical_schema() {
    let arbitrary = b"arbitrary correctly hashable machine bytes";
    let mut digest = Sha256::new();
    digest.update(LEGACY_MACHINE_RECEIPT_IDENTITY_DOMAIN_V1);
    digest.update((arbitrary.len() as u64).to_le_bytes());
    digest.update(arbitrary);
    let correctly_hashed = InertCapabilityRefinementReceiptIdentityV1::from_exact_identity_v1(
        digest.finalize().into(),
        arbitrary.len() as u64,
    )
    .unwrap();
    assert_ne!(correctly_hashed.sha256(), [0; 32]);
    assert!(
        InertCapabilityRefinementReceiptV1::from_canonical_preimage(
            InertCapabilityRefinementReceiptKindV1::Machine,
            arbitrary.to_vec(),
        )
        .is_err()
    );

    let typed = TargetMachineRefinementReceiptV1::from_parts(parts()).unwrap();
    let inert = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::Machine,
        typed.canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(inert.typed_machine_refinement().unwrap().parts(), parts());
    let checked =
        check_target_machine_refinement_receipt_v1(inert.canonical_preimage(), parts()).unwrap();
    assert_eq!(checked.parts(), parts());
    assert!(!checked.authenticates_issuer());
}

#[test]
fn correctly_checksummed_coordinate_substitution_is_rejected() {
    let expected = parts();
    let mut substituted = expected;
    substituted.machine_refinement_sha256[0] ^= 0x80;
    let substituted = TargetMachineRefinementReceiptV1::from_parts(substituted).unwrap();
    assert_eq!(
        check_target_machine_refinement_receipt_v1(substituted.canonical_bytes(), expected)
            .unwrap_err(),
        TargetMachineRefinementReceiptErrorV1::CoordinateMismatch
    );
}

#[test]
fn missing_required_family_and_target_substitution_fail_closed() {
    let mut unsupported = parts();
    unsupported.required_families |= MachineRefinementFamilyV1::Matrix.bit();
    assert_eq!(
        TargetMachineRefinementReceiptV1::from_parts(unsupported).unwrap_err(),
        TargetMachineRefinementReceiptErrorV1::UnsupportedFamily
    );

    let expected = parts();
    let mut substituted = expected;
    substituted.target = TargetMachineRefinementTargetV1::Gfx950;
    let substituted = TargetMachineRefinementReceiptV1::from_parts(substituted).unwrap();
    assert_eq!(
        check_target_machine_refinement_receipt_v1(substituted.canonical_bytes(), expected)
            .unwrap_err(),
        TargetMachineRefinementReceiptErrorV1::CoordinateMismatch
    );
}

#[test]
fn machine_receipt_rejects_every_prefix_trailing_reserved_and_unknown_target_bytes() {
    let receipt = TargetMachineRefinementReceiptV1::from_parts(parts()).unwrap();
    let bytes = receipt.canonical_bytes();
    for prefix in 0..bytes.len() {
        assert_eq!(
            TargetMachineRefinementReceiptV1::decode(&bytes[..prefix]).unwrap_err(),
            TargetMachineRefinementReceiptErrorV1::Length
        );
    }

    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        TargetMachineRefinementReceiptV1::decode(&trailing).unwrap_err(),
        TargetMachineRefinementReceiptErrorV1::Length
    );

    let mut reserved = bytes.to_vec();
    reserved[13] = 1;
    assert_eq!(
        TargetMachineRefinementReceiptV1::decode(&reserved).unwrap_err(),
        TargetMachineRefinementReceiptErrorV1::NonCanonical
    );

    let mut target = bytes.to_vec();
    target[12] = u8::MAX;
    assert_eq!(
        TargetMachineRefinementReceiptV1::decode(&target).unwrap_err(),
        TargetMachineRefinementReceiptErrorV1::Target
    );
}
