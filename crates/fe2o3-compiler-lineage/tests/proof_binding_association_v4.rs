use fe2o3_compiler_lineage::{
    INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V4, INERT_PROOF_BINDING_ASSOCIATION_VERSION_V4,
    InertLineageContentIdentityV3, InertProofBindingAssociationInputsV4,
    InertProofBindingAssociationV4, MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4,
};

const DOMAIN: &[u8] = b"FE2O3/PRODUCTION-PROOF-BINDING-ASSOCIATION/V4\0";
const CLAIM: &[u8] = b"exact-signed-mir-pliron-verus-receipt/no-llvm-or-later-refinement-proof";

fn identity(seed: u8) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new([seed; 32], u64::from(seed) + 1).unwrap()
}

fn inputs() -> InertProofBindingAssociationInputsV4 {
    InertProofBindingAssociationInputsV4::new(
        identity(1),
        identity(2),
        identity(3),
        identity(4),
        identity(5),
    )
}

fn evidence() -> Vec<u8> {
    (0_u8..=127).collect()
}

#[test]
fn current_association_roundtrips_exact_stage_and_verus_bytes() {
    let evidence = evidence();
    let association = InertProofBindingAssociationV4::new(inputs(), &evidence).unwrap();
    let decoded = InertProofBindingAssociationV4::decode(association.canonical_bytes()).unwrap();

    assert_eq!(decoded.inputs(), inputs());
    assert_eq!(decoded.verus_execution_evidence(), evidence);
    assert_eq!(decoded.canonical_bytes(), association.canonical_bytes());
    assert!(decoded.retains_exact_signed_verus_execution_evidence());
    assert!(!decoded.authenticates_compiler_origin());
    assert!(!decoded.establishes_llvm_or_machine_refinement());
    assert!(!decoded.grants_authority());
    assert_eq!(
        &decoded.canonical_bytes()[..8],
        &INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V4
    );
    assert_eq!(
        u16::from_le_bytes(decoded.canonical_bytes()[8..10].try_into().unwrap()),
        INERT_PROOF_BINDING_ASSOCIATION_VERSION_V4
    );
}

#[test]
fn every_prefix_trailing_byte_and_structural_mutation_fail_closed() {
    let association = InertProofBindingAssociationV4::new(inputs(), &evidence()).unwrap();
    let bytes = association.canonical_bytes();
    for prefix in 0..bytes.len() {
        assert!(
            InertProofBindingAssociationV4::decode(&bytes[..prefix]).is_err(),
            "accepted prefix {prefix}"
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(InertProofBindingAssociationV4::decode(&trailing).is_err());

    for offset in [0, 8, 10, 12, 14, 16, 20, 24, 26, 28] {
        let mut mutated = bytes.to_vec();
        mutated[offset] ^= 0x80;
        assert!(
            InertProofBindingAssociationV4::decode(&mutated).is_err(),
            "accepted structural mutation at {offset}"
        );
    }
}

#[test]
fn invalid_identities_and_verus_evidence_lengths_fail_closed() {
    assert!(InertProofBindingAssociationV4::new(inputs(), &[]).is_err());
    assert!(
        InertProofBindingAssociationV4::new(
            inputs(),
            &vec![0; MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4 + 1]
        )
        .is_err()
    );

    let association = InertProofBindingAssociationV4::new(inputs(), &evidence()).unwrap();
    let first_identity_offset = 24 + 8 + DOMAIN.len() + 8 + CLAIM.len() + 8;
    let mut zero_digest = association.canonical_bytes().to_vec();
    zero_digest[first_identity_offset..first_identity_offset + 32].fill(0);
    assert!(InertProofBindingAssociationV4::decode(&zero_digest).is_err());

    let mut zero_length = association.canonical_bytes().to_vec();
    zero_length[first_identity_offset + 32..first_identity_offset + 40].fill(0);
    assert!(InertProofBindingAssociationV4::decode(&zero_length).is_err());
}

#[test]
fn nested_evidence_substitution_changes_the_exact_association() {
    let first = InertProofBindingAssociationV4::new(inputs(), &evidence()).unwrap();
    let mut substituted = evidence();
    substituted[17] ^= 0x80;
    let second = InertProofBindingAssociationV4::new(inputs(), &substituted).unwrap();
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(second.verus_execution_evidence(), substituted);
}
