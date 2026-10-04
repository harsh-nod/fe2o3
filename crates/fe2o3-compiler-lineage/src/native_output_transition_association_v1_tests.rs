use super::*;
use crate::{
    InertProofBindingAssociationInputsV4, InertProofBindingAssociationV3,
    InertProofBindingAssociationV4,
};
use sha2::{Digest, Sha256};

const ROOTS: [NativeOutputTransitionRootV1; 2] = [
    NativeOutputTransitionRootV1::new(2, 1, 0, 1),
    NativeOutputTransitionRootV1::new(300, 0, 1, 0),
];
fn inputs(roots: &[NativeOutputTransitionRootV1]) -> NativeOutputTransitionAssociationInputsV1<'_> {
    NativeOutputTransitionAssociationInputsV1 {
        input_subject: InertNativeNeutralSubjectV1::new([1; 32], 7, [2; 32], 9).unwrap(),
        output_subject: InertNativeNeutralSubjectV1::new([3; 32], 11, [4; 32], 13).unwrap(),
        input_proof_binding: InertLineageContentIdentityV3::new([5; 32], 17).unwrap(),
        output_kernel_ir: InertLineageContentIdentityV3::new([6; 32], 19).unwrap(),
        output_formal_memory: InertLineageContentIdentityV3::new([7; 32], 23).unwrap(),
        roots,
        input_association: b"a",
        input_kernel_ir: b"k",
        input_formal_memory: b"f",
        transition: b"t",
    }
}
fn golden() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"F2NOUT1\0");
    bytes.extend_from_slice(&[1, 0, 1, 0]);
    bytes.extend_from_slice(&384_u32.to_le_bytes());
    for (graph_tag, graph_length, catalog_tag, catalog_length) in
        [(1, 7_u64, 2, 9_u64), (3, 11, 4, 13)]
    {
        bytes.extend_from_slice(b"F2NNAT1\0");
        bytes.extend_from_slice(&[1, 0, 1, 0, 12, 0, 0, 0]);
        bytes.extend_from_slice(&graph_length.to_le_bytes());
        bytes.extend_from_slice(&[graph_tag; 32]);
        bytes.extend_from_slice(&catalog_length.to_le_bytes());
        bytes.extend_from_slice(&[catalog_tag; 32]);
    }
    for (tag, length) in [(5, 17_u64), (6, 19), (7, 23)] {
        bytes.extend_from_slice(&[tag; 32]);
        bytes.extend_from_slice(&length.to_le_bytes());
    }
    for word in [2_u32, 1, 1, 1, 1, 2, 1, 0, 1, 300, 0, 1, 0] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(b"akft");
    bytes
}

#[test]
fn literal_outer_golden_preserves_sparse_semantic_ids_and_three_permutations() {
    let encoded = encode_native_output_transition_association_v1(inputs(&ROOTS)).unwrap();
    let expected = golden();
    assert_eq!(expected.len(), 348 + 2 * 16 + 4);
    assert_eq!(encoded, expected);
    assert_eq!(encoded.capacity(), 384);
    let view = NativeOutputTransitionAssociationRefV1::decode(&encoded).unwrap();
    assert_eq!(view.root_count(), 2);
    assert_eq!(view.root(0), Some(ROOTS[0]));
    assert_eq!(view.root(1), Some(ROOTS[1]));
    assert_eq!(view.root(2), None);
    assert_eq!(view.root(usize::MAX), None);
    assert_eq!(view.input_subject(), &inputs(&ROOTS).input_subject);
    assert_eq!(view.output_subject(), &inputs(&ROOTS).output_subject);
    assert_eq!(view.input_proof_binding().sha256(), [5; 32]);
    assert_eq!(view.output_kernel_ir().byte_len(), 19);
    assert_eq!(view.output_formal_memory().byte_len(), 23);
    assert_eq!(
        [
            view.input_association(),
            view.input_kernel_ir(),
            view.input_formal_memory(),
            view.transition()
        ],
        [b"a", b"k", b"f", b"t"]
    );
    assert!(std::ptr::eq(
        view.transition().as_ptr(),
        encoded[383..].as_ptr()
    ));
    assert!(!view.grants_authority());
    // These deliberately opaque one-byte fields are not valid nested evidence.
    // Outer framing accepts them, while actual typed admission remains mandatory.
    assert!(InertProofBindingAssociationV4::decode(view.input_association()).is_err());
    let hash: [u8; 32] = Sha256::digest(&expected).into();
    assert_eq!(hash, GOLDEN_SHA256);
}

#[test]
fn singleton_and_all_128_sparse_roots_are_admitted_without_semantic_id_bitsets() {
    let singleton = [NativeOutputTransitionRootV1::new(u32::MAX, 0, 0, 0)];
    let bytes = encode_native_output_transition_association_v1(inputs(&singleton)).unwrap();
    assert_eq!(
        NativeOutputTransitionAssociationRefV1::decode(&bytes)
            .unwrap()
            .root(0),
        Some(singleton[0])
    );
    let roots: Vec<_> = (0..128_u32)
        .map(|i| NativeOutputTransitionRootV1::new(1000 + 2 * i, 127 - i, i, (i + 1) % 128))
        .collect();
    let bytes = encode_native_output_transition_association_v1(inputs(&roots)).unwrap();
    assert_eq!(
        NativeOutputTransitionAssociationRefV1::decode(&bytes)
            .unwrap()
            .root_count(),
        128
    );
    let too_many: Vec<_> = (0..129_u32)
        .map(|i| NativeOutputTransitionRootV1::new(i, i, i, i))
        .collect();
    assert!(encode_native_output_transition_association_v1(inputs(&too_many)).is_err());
    assert!(encode_native_output_transition_association_v1(inputs(&[])).is_err());
}

#[test]
fn every_truncation_unknown_header_empty_extent_and_duplicate_axis_rejects() {
    let bytes = golden();
    for end in 0..bytes.len() {
        assert!(NativeOutputTransitionAssociationRefV1::decode(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(NativeOutputTransitionAssociationRefV1::decode(&trailing).is_err());
    for index in 0..16 {
        let mut changed = bytes.clone();
        changed[index] ^= 0x80;
        assert!(
            NativeOutputTransitionAssociationRefV1::decode(&changed).is_err(),
            "header {index}"
        );
    }
    for offset in [328, 332, 336, 340, 344] {
        for count in [0_u32, u32::MAX] {
            let mut changed = bytes.clone();
            changed[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
            assert!(NativeOutputTransitionAssociationRefV1::decode(&changed).is_err());
        }
    }
    for axis in 0..4 {
        let mut changed = bytes.clone();
        let first: [u8; 4] = changed[348 + axis * 4..352 + axis * 4].try_into().unwrap();
        changed[364 + axis * 4..368 + axis * 4].copy_from_slice(&first);
        assert!(matches!(
            NativeOutputTransitionAssociationRefV1::decode(&changed),
            Err(ErrorV1::RootAxes)
        ));
    }
    for offset in [208, 248, 288] {
        let mut zero_length = bytes.clone();
        zero_length[offset + 32..offset + 40].fill(0);
        assert!(matches!(
            NativeOutputTransitionAssociationRefV1::decode(&zero_length),
            Err(ErrorV1::Identity(_))
        ));
        let mut changed = bytes.clone();
        changed[offset..offset + 32].fill(0);
        assert!(matches!(
            NativeOutputTransitionAssociationRefV1::decode(&changed),
            Err(ErrorV1::Identity(_))
        ));
    }
    let mut reserved = bytes.clone();
    for axis in 1..4 {
        for ordinal in [2_u32, u32::MAX] {
            let mut changed = bytes.clone();
            changed[364 + axis * 4..368 + axis * 4].copy_from_slice(&ordinal.to_le_bytes());
            assert!(matches!(
                NativeOutputTransitionAssociationRefV1::decode(&changed),
                Err(ErrorV1::RootAxes)
            ));
        }
    }
    reserved[16 + 14] = 1;
    assert!(matches!(
        NativeOutputTransitionAssociationRefV1::decode(&reserved),
        Err(ErrorV1::Subject(_))
    ));
}

#[test]
fn aggregate_bound_is_exact_and_legacy_proof_decoders_remain_disjoint() {
    let root = [NativeOutputTransitionRootV1::new(0, 0, 0, 0)];
    let payload = vec![42; MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 - 348 - 16 - 3];
    let mut input = inputs(&root);
    input.transition = &payload;
    assert_eq!(
        native_output_transition_association_length_v1(input).unwrap(),
        MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3
    );
    let bytes = encode_native_output_transition_association_v1(input).unwrap();
    assert!(NativeOutputTransitionAssociationRefV1::decode(&bytes).is_ok());
    let mut oversized = bytes;
    oversized.push(0);
    oversized[0] ^= 0x80;
    assert!(matches!(
        NativeOutputTransitionAssociationRefV1::decode(&oversized),
        Err(ErrorV1::Length)
    ));
    input.input_association = b"aa";
    assert!(matches!(
        native_output_transition_association_length_v1(input),
        Err(ErrorV1::Length)
    ));
    assert!(InertProofBindingAssociationV4::decode(&golden()).is_err());
    assert!(InertProofBindingAssociationV3::decode(&golden()).is_err());
    let id = InertLineageContentIdentityV3::new([1; 32], 7).unwrap();
    let old = InertProofBindingAssociationV4::new(
        InertProofBindingAssociationInputsV4::new(id, id, id, id, id),
        b"signed-opaque",
    )
    .unwrap();
    let before = old.canonical_bytes().to_vec();
    let decoded = InertProofBindingAssociationV4::decode(&before).unwrap();
    assert_eq!(decoded.canonical_bytes(), before);
    assert!(matches!(
        NativeOutputTransitionAssociationRefV1::decode(&before),
        Err(ErrorV1::Header)
    ));
}

const GOLDEN_SHA256: [u8; 32] = [
    0x31, 0xd4, 0xc1, 0xa2, 0x1c, 0x11, 0x6d, 0x65, 0x79, 0xfc, 0xcb, 0x6a, 0x56, 0x07, 0xc2, 0xdb,
    0x9b, 0xe1, 0xc6, 0xf6, 0xc8, 0xac, 0x77, 0xc1, 0x7d, 0x3c, 0x42, 0x9a, 0xe5, 0x88, 0x8a, 0x7e,
];
