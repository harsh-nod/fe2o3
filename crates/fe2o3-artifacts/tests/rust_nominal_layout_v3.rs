use fe2o3_artifacts::{
    PointerWidth, RUST_NOMINAL_LAYOUT_DOMAIN_V3, RUST_NOMINAL_TYPE_DOMAIN_V3, RustLayoutEvidenceV1,
    RustNominalLayoutErrorV3, RustNominalScalarEvidenceV3, RustNominalScalarKindV3 as Kind,
    RustPhysicalComponentKindV1, RustPhysicalComponentV1, RustScalarElementTypeV1 as Scalar,
    RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
};
use sha2::{Digest, Sha256};

struct Literal {
    kind: Kind,
    tag: u8,
    physical: Scalar,
    type_hash: &'static str,
    layout: &'static str,
    layout_hash: &'static str,
}
const LITERALS: [Literal; 2] = [
    Literal {
        kind: Kind::Usize,
        tag: 5,
        physical: Scalar::U64,
        type_hash: "e0bf66b90f468eca606421648e8b1bd39ca0b1df0bb5a99486895ac814794c66",
        layout: "e0bf66b90f468eca606421648e8b1bd39ca0b1df0bb5a99486895ac814794c66400008010800000000000000080000000000000000000000",
        layout_hash: "a3c45e8da159d55f8668a43f211f8bd9eaca71ea60e41251388ce3b39767ccfc",
    },
    Literal {
        kind: Kind::Isize,
        tag: 6,
        physical: Scalar::I64,
        type_hash: "a72513453368014d767433b9851760419a0bee2de3dd451f02d5163c1927f2d1",
        layout: "a72513453368014d767433b9851760419a0bee2de3dd451f02d5163c1927f2d1400007010800000000000000080000000000000000000000",
        layout_hash: "730ef72a5e234a60a888cf0e73652b494b83a09024db57d4a430ebbbacb220f2",
    },
];

fn bytes(hex: &str) -> Vec<u8> {
    assert!(hex.is_ascii() && hex.len().is_multiple_of(2));
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn digest(preimage: &[u8]) -> [u8; 32] {
    Sha256::digest(preimage).into()
}

fn fixed(scalar: Scalar) -> RustLayoutEvidenceV1 {
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(scalar)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        8,
        8,
        vec![
            RustPhysicalComponentV1::new(0, 8, 8, RustPhysicalComponentKindV1::Scalar { scalar })
                .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn nominal_v3_exact_kinds_and_physical_layout() {
    for literal in LITERALS {
        let value = RustNominalScalarEvidenceV3::new(literal.kind, PointerWidth::Bits64).unwrap();
        assert_eq!(value.kind(), literal.kind);
        assert_eq!(value.pointer_width(), PointerWidth::Bits64);
        assert_eq!(value.physical_scalar(), literal.physical);
        assert_eq!(value.abi_class(), RustcAbiClassV1::Scalar);
        assert_eq!((value.size(), value.abi_alignment()), (8, 8));
    }
}

#[test]
fn nominal_v3_rejects_32_bit_width_with_exact_typed_error() {
    for kind in [Kind::Usize, Kind::Isize] {
        let error = RustNominalLayoutErrorV3::UnsupportedPointerWidth(PointerWidth::Bits32);
        assert_eq!(
            RustNominalScalarEvidenceV3::new(kind, PointerWidth::Bits32),
            Err(error)
        );
        assert_eq!(
            error.to_string(),
            "nominal scalar ABI V3 requires 64-bit pointers, got 32-bit"
        );
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn nominal_v3_payloads_match_literal_wire_recipes() {
    for literal in LITERALS {
        let value = RustNominalScalarEvidenceV3::new(literal.kind, PointerWidth::Bits64).unwrap();
        let type_payload: [u8; 4] = value.canonical_type_payload();
        let layout_payload: [u8; 56] = value.canonical_layout_payload();
        assert_eq!(type_payload, [3, 0, literal.tag, 0]);
        assert_eq!(layout_payload.as_slice(), bytes(literal.layout).as_slice());
        assert_eq!(&layout_payload[32..34], &[64, 0]);
        assert_eq!(layout_payload[35], 1);
        assert_eq!(&layout_payload[36..44], &8_u64.to_le_bytes());
        assert_eq!(&layout_payload[44..48], &8_u32.to_le_bytes());
        assert_eq!(&layout_payload[48..], &[0; 8]);
    }
}

#[test]
fn nominal_v3_hashes_match_independent_literal_preimages() {
    for literal in LITERALS {
        // Expected bytes come only from the frozen recipe, not the producer.
        let type_preimage = [
            b"FE2O3/RUST-NOMINAL-TYPE/V3\0".as_slice(),
            &4_u64.to_le_bytes(),
            &[3, 0, literal.tag, 0],
        ]
        .concat();
        let layout_preimage = [
            b"FE2O3/RUST-NOMINAL-LAYOUT/V3\0".as_slice(),
            bytes(literal.layout).as_slice(),
        ]
        .concat();
        let type_digest = digest(&type_preimage);
        let layout_digest = digest(&layout_preimage);
        assert_eq!(type_digest.as_slice(), bytes(literal.type_hash).as_slice());
        assert_eq!(
            layout_digest.as_slice(),
            bytes(literal.layout_hash).as_slice()
        );
        let actual = RustNominalScalarEvidenceV3::new(literal.kind, PointerWidth::Bits64)
            .unwrap()
            .type_identity();
        assert_eq!(actual.rust_type().bytes().as_bytes(), &type_digest);
        assert_eq!(actual.layout().bytes().as_bytes(), &layout_digest);
    }
}

#[test]
fn nominal_v3_domains_and_length_framing_are_exact() {
    assert_eq!(RUST_NOMINAL_TYPE_DOMAIN_V3, b"FE2O3/RUST-NOMINAL-TYPE/V3\0");
    assert_eq!(
        RUST_NOMINAL_LAYOUT_DOMAIN_V3,
        b"FE2O3/RUST-NOMINAL-LAYOUT/V3\0"
    );
    for literal in LITERALS {
        let unframed_type = [
            b"FE2O3/RUST-NOMINAL-TYPE/V3\0".as_slice(),
            &[3, 0, literal.tag, 0],
        ]
        .concat();
        assert_ne!(
            digest(&unframed_type).as_slice(),
            bytes(literal.type_hash).as_slice()
        );
        let wrongly_framed_layout = [
            b"FE2O3/RUST-NOMINAL-LAYOUT/V3\0".as_slice(),
            &56_u64.to_le_bytes(),
            bytes(literal.layout).as_slice(),
        ]
        .concat();
        assert_ne!(
            digest(&wrongly_framed_layout).as_slice(),
            bytes(literal.layout_hash).as_slice()
        );
    }
}

#[test]
fn nominal_v3_every_payload_byte_is_bound_to_its_identity() {
    for literal in LITERALS {
        for offset in 0..4 {
            let mut payload = [3, 0, literal.tag, 0];
            payload[offset] ^= 1;
            let preimage = [
                b"FE2O3/RUST-NOMINAL-TYPE/V3\0".as_slice(),
                &4_u64.to_le_bytes(),
                &payload,
            ]
            .concat();
            assert_ne!(
                digest(&preimage).as_slice(),
                bytes(literal.type_hash).as_slice()
            );
        }
        for offset in 0..56 {
            let mut payload = bytes(literal.layout);
            payload[offset] ^= 1;
            let preimage = [b"FE2O3/RUST-NOMINAL-LAYOUT/V3\0".as_slice(), &payload].concat();
            assert_ne!(
                digest(&preimage).as_slice(),
                bytes(literal.layout_hash).as_slice()
            );
        }
    }
}

#[test]
fn nominal_v3_is_distinct_from_fixed_v1_and_descriptor_source_ids() {
    for literal in LITERALS {
        let nominal = RustNominalScalarEvidenceV3::new(literal.kind, PointerWidth::Bits64).unwrap();
        let fixed = fixed(literal.physical);
        assert_eq!(
            (nominal.size(), nominal.abi_alignment()),
            (fixed.size(), fixed.abi_alignment())
        );
        assert_eq!(nominal.abi_class(), fixed.abi_class());
        assert_ne!(
            nominal.type_identity().rust_type(),
            fixed.type_identity().rust_type()
        );
        assert_ne!(
            nominal.type_identity().layout(),
            fixed.type_identity().layout()
        );
        let descriptor_preimage = [
            b"FE2O3/RUST-TYPE/V3\0".as_slice(),
            &4_u64.to_le_bytes(),
            &[literal.tag, 0, 0, 0],
        ]
        .concat();
        assert_ne!(
            nominal.type_identity().rust_type().bytes().as_bytes(),
            &digest(&descriptor_preimage)
        );
    }
}

#[test]
fn nominal_v3_fixed_size_copy_evidence_preserves_identity() {
    use std::mem::{align_of, size_of};

    fn copy_value<T: Copy + Eq + std::fmt::Debug>(value: T) {
        let duplicate = value;
        assert_eq!(value, duplicate);
    }
    // Target-layout regression only; repr(Rust) is not a wire or FFI contract.
    let alignment = align_of::<Kind>().max(align_of::<PointerWidth>());
    let pointer_offset = size_of::<Kind>()
        .div_ceil(align_of::<PointerWidth>())
        .checked_mul(align_of::<PointerWidth>())
        .unwrap();
    let expected_size = pointer_offset
        .checked_add(size_of::<PointerWidth>())
        .unwrap()
        .div_ceil(alignment)
        .checked_mul(alignment)
        .unwrap();
    assert_eq!(align_of::<RustNominalScalarEvidenceV3>(), alignment);
    assert_eq!(size_of::<RustNominalScalarEvidenceV3>(), expected_size);
    assert!(!std::mem::needs_drop::<RustNominalScalarEvidenceV3>());
    assert!(!std::mem::needs_drop::<Kind>());
    assert!(!std::mem::needs_drop::<RustNominalLayoutErrorV3>());
    let unsigned = RustNominalScalarEvidenceV3::new(Kind::Usize, PointerWidth::Bits64).unwrap();
    let signed = RustNominalScalarEvidenceV3::new(Kind::Isize, PointerWidth::Bits64).unwrap();
    copy_value(unsigned);
    copy_value(signed);
    copy_value(Kind::Usize);
    copy_value(RustNominalLayoutErrorV3::UnsupportedPointerWidth(
        PointerWidth::Bits32,
    ));
    assert_ne!(unsigned, signed);
    assert_ne!(
        unsigned.type_identity().rust_type(),
        signed.type_identity().rust_type()
    );
    assert_ne!(
        unsigned.type_identity().layout(),
        signed.type_identity().layout()
    );
    let retained = [unsigned.type_identity(), signed.type_identity()];
    assert_eq!(retained, [unsigned.type_identity(), signed.type_identity()]);
}
