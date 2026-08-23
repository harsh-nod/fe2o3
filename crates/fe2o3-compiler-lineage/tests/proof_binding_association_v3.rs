use fe2o3_compiler_lineage::{
    INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3, INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3,
    InertLineageContentIdentityV3, InertProofBindingAssociationInputsV3,
    InertProofBindingAssociationV3,
};

const DOMAIN: &[u8] = b"FE2O3/PRODUCTION-PROOF-BINDING-ASSOCIATION/V3\0";
const CLAIM: &[u8] = b"association-only/no-refinement-proof";

fn identity(seed: u8) -> InertLineageContentIdentityV3 {
    InertLineageContentIdentityV3::new([seed; 32], u64::from(seed) + 1).unwrap()
}

fn inputs() -> InertProofBindingAssociationInputsV3 {
    InertProofBindingAssociationInputsV3::new(
        identity(1),
        identity(2),
        identity(3),
        identity(4),
        identity(5),
    )
}

fn legacy_encoder(inputs: InertProofBindingAssociationInputsV3) -> Vec<u8> {
    fn encode_identity(value: InertLineageContentIdentityV3) -> [u8; 40] {
        let mut bytes = [0_u8; 40];
        bytes[..32].copy_from_slice(&value.sha256());
        bytes[32..].copy_from_slice(&value.byte_len().to_le_bytes());
        bytes
    }

    let identities = [
        encode_identity(inputs.semantic_mir()),
        encode_identity(inputs.middle_end()),
        encode_identity(inputs.kernel_ir()),
        encode_identity(inputs.mir_to_kir_correspondence()),
        encode_identity(inputs.formal_memory()),
    ];
    let fields: [&[u8]; 7] = [
        DOMAIN,
        CLAIM,
        &identities[0],
        &identities[1],
        &identities[2],
        &identities[3],
        &identities[4],
    ];
    let total = 24 + fields.len() * 8 + fields.iter().map(|field| field.len()).sum::<usize>();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3);
    bytes.extend_from_slice(&INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3.to_le_bytes());
    bytes.extend_from_slice(&6_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&7_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u32).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for (index, field) in fields.iter().enumerate() {
        bytes.extend_from_slice(&((index + 1) as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
        bytes.extend_from_slice(field);
    }
    bytes
}

#[test]
fn canonical_encoder_is_byte_compatible_with_the_frozen_compiler_format() {
    let association = InertProofBindingAssociationV3::new(inputs()).unwrap();
    assert_eq!(association.canonical_bytes(), legacy_encoder(inputs()));

    let decoded = InertProofBindingAssociationV3::decode(association.canonical_bytes()).unwrap();
    assert_eq!(decoded.inputs(), inputs());
    assert_eq!(decoded.canonical_bytes(), association.canonical_bytes());
    assert!(!decoded.claims_verus_verification());
    assert!(!decoded.establishes_refinement_proof());
    assert!(!decoded.grants_authority());
}

#[test]
fn every_prefix_and_any_trailing_byte_fail_closed() {
    let association = InertProofBindingAssociationV3::new(inputs()).unwrap();
    let bytes = association.canonical_bytes();
    for prefix in 0..bytes.len() {
        assert!(
            InertProofBindingAssociationV3::decode(&bytes[..prefix]).is_err(),
            "accepted prefix {prefix}"
        );
    }
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(InertProofBindingAssociationV3::decode(&trailing).is_err());
}

#[test]
fn structural_and_zero_identity_mutations_fail_closed() {
    let association = InertProofBindingAssociationV3::new(inputs()).unwrap();
    let original = association.canonical_bytes();
    for offset in [0, 8, 10, 12, 14, 16, 20, 24, 26, 28] {
        let mut mutated = original.to_vec();
        mutated[offset] ^= 0x80;
        assert!(
            InertProofBindingAssociationV3::decode(&mutated).is_err(),
            "accepted structural mutation at {offset}"
        );
    }

    let first_identity_offset = 24 + 8 + DOMAIN.len() + 8 + CLAIM.len() + 8;
    let mut zero_digest = original.to_vec();
    zero_digest[first_identity_offset..first_identity_offset + 32].fill(0);
    assert!(InertProofBindingAssociationV3::decode(&zero_digest).is_err());

    let mut zero_length = original.to_vec();
    zero_length[first_identity_offset + 32..first_identity_offset + 40].fill(0);
    assert!(InertProofBindingAssociationV3::decode(&zero_length).is_err());
}
