mod common;

use common::{kernel, manifest, object, target, text};
use fe2o3_artifacts::{
    AliasClass, ArgumentOwnership, Capability, CompilerIdentity, DecodeError, MAX_ABI_BYTES,
    MAX_CODE_OBJECTS, MAX_MANIFEST_BYTES, ManifestV1, PointerWidth, ToolIdentity, ValidationError,
};

fn find(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap()
}

fn two_object_manifest() -> ManifestV1 {
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        vec![object(0x44), object(0x43)],
        vec![kernel(
            0x11,
            "vector_add",
            "vector_add.kd",
            0x44,
            vec![Capability::AmdWave],
        )],
    )
    .unwrap()
}

#[test]
fn round_trip_preserves_validated_manifest() {
    let original = manifest();
    let decoded = ManifestV1::from_bytes(&original.to_bytes()).unwrap();

    assert_eq!(decoded, original);
    assert_eq!(decoded.kernels()[0].abi().fields().len(), 3);
    assert_eq!(
        decoded.kernels()[0].abi().fields()[1].ownership(),
        ArgumentOwnership::SharedBorrow
    );
    assert_eq!(
        decoded.kernels()[0].abi().fields()[2].alias_class(),
        AliasClass::Exclusive
    );
}

#[test]
fn malformed_headers_counts_tags_and_boundaries_are_rejected() {
    let valid = manifest().to_bytes();

    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 0xff;
    assert_eq!(
        ManifestV1::from_bytes(&bad_magic),
        Err(DecodeError::InvalidMagic)
    );

    let mut bad_version = valid.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        ManifestV1::from_bytes(&bad_version),
        Err(DecodeError::UnknownVersion(2))
    );

    let mut bad_flags = valid.clone();
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        ManifestV1::from_bytes(&bad_flags),
        Err(DecodeError::UnsupportedFlags(1))
    );

    let capability_encoding = [2, 0, 6, 0, 7, 0];
    let capability_position = find(&valid, &capability_encoding);
    let mut unknown_capability = valid.clone();
    unknown_capability[capability_position + 2..capability_position + 4]
        .copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        ManifestV1::from_bytes(&unknown_capability),
        Err(DecodeError::UnknownCapability(u16::MAX))
    );

    let mut duplicate_capability = valid.clone();
    duplicate_capability[capability_position + 4..capability_position + 6]
        .copy_from_slice(&6_u16.to_le_bytes());
    assert!(matches!(
        ManifestV1::from_bytes(&duplicate_capability),
        Err(DecodeError::Validation(ValidationError::Duplicate {
            field: "target capabilities"
        }))
    ));

    let mut noncanonical_capability = valid.clone();
    noncanonical_capability[capability_position + 2..capability_position + 4]
        .copy_from_slice(&7_u16.to_le_bytes());
    noncanonical_capability[capability_position + 4..capability_position + 6]
        .copy_from_slice(&6_u16.to_le_bytes());
    assert_eq!(
        ManifestV1::from_bytes(&noncanonical_capability),
        Err(DecodeError::NonCanonicalOrder {
            field: "target capabilities"
        })
    );

    let object_position = find(&valid, &[0x44; 32]);
    let mut unknown_format = valid.clone();
    unknown_format[object_position + 32] = 0xff;
    assert!(matches!(
        ManifestV1::from_bytes(&unknown_format),
        Err(DecodeError::UnknownTag {
            kind: "code object format",
            tag: 0xff
        })
    ));

    let mut zero_objects = valid.clone();
    zero_objects[object_position - 4..object_position].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        ManifestV1::from_bytes(&zero_objects),
        Err(DecodeError::CountOutOfRange {
            field: "code objects",
            count: 0,
            ..
        })
    ));

    let mut too_many_objects = valid.clone();
    too_many_objects[object_position - 4..object_position]
        .copy_from_slice(&((MAX_CODE_OBJECTS + 1) as u32).to_le_bytes());
    assert!(matches!(
        ManifestV1::from_bytes(&too_many_objects),
        Err(DecodeError::CountOutOfRange {
            field: "code objects",
            ..
        })
    ));

    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        ManifestV1::from_bytes(&trailing),
        Err(DecodeError::TrailingBytes)
    );

    let oversized = vec![0; MAX_MANIFEST_BYTES + 1];
    assert_eq!(
        ManifestV1::from_bytes(&oversized),
        Err(DecodeError::TooLarge {
            max: MAX_MANIFEST_BYTES
        })
    );
}

#[test]
fn duplicate_and_noncanonical_wire_records_are_rejected() {
    let valid = two_object_manifest().to_bytes();
    let first = find(&valid, &[0x43; 32]);
    let second = find(&valid, &[0x44; 32]);

    let mut duplicate = valid.clone();
    duplicate[second..second + 32].copy_from_slice(&[0x43; 32]);
    assert!(matches!(
        ManifestV1::from_bytes(&duplicate),
        Err(DecodeError::Validation(ValidationError::Duplicate {
            field: "code object digest"
        }))
    ));

    let mut noncanonical = valid;
    noncanonical[first..first + 32].copy_from_slice(&[0x44; 32]);
    noncanonical[second..second + 32].copy_from_slice(&[0x43; 32]);
    assert_eq!(
        ManifestV1::from_bytes(&noncanonical),
        Err(DecodeError::NonCanonicalOrder {
            field: "code objects"
        })
    );
}

#[test]
fn malformed_names_alignment_and_layout_are_rejected() {
    let valid = manifest().to_bytes();

    let abi_size = find(&valid, &32_u64.to_le_bytes());
    let mut oversized_abi = valid.clone();
    oversized_abi[abi_size..abi_size + 8].copy_from_slice(&(MAX_ABI_BYTES + 8).to_le_bytes());
    assert!(matches!(
        ManifestV1::from_bytes(&oversized_abi),
        Err(DecodeError::Validation(ValidationError::InvalidLayout(_)))
    ));

    let kernel_name = find(&valid, b"vector_add");
    let mut invalid_name = valid.clone();
    invalid_name[kernel_name] = b'!';
    assert!(matches!(
        ManifestV1::from_bytes(&invalid_name),
        Err(DecodeError::Validation(ValidationError::InvalidText { .. }))
    ));

    let input_name = find(&valid, b"input");
    let input_alignment = input_name + b"input".len() + 8 + 8;
    let mut invalid_alignment = valid.clone();
    invalid_alignment[input_alignment..input_alignment + 4].copy_from_slice(&3_u32.to_le_bytes());
    assert!(matches!(
        ManifestV1::from_bytes(&invalid_alignment),
        Err(DecodeError::Validation(
            ValidationError::InvalidAlignment { .. }
        ))
    ));

    let mut unknown_abi_kind = valid.clone();
    unknown_abi_kind[input_alignment + 4] = 0xff;
    assert!(matches!(
        ManifestV1::from_bytes(&unknown_abi_kind),
        Err(DecodeError::UnknownTag {
            kind: "ABI kind",
            tag: 0xff
        })
    ));

    let input_type_identity = find(&valid, &[0xb0; 32]);
    let ownership_position = input_type_identity + 64;
    let mut unknown_ownership = valid.clone();
    unknown_ownership[ownership_position] = 0xff;
    assert!(matches!(
        ManifestV1::from_bytes(&unknown_ownership),
        Err(DecodeError::UnknownTag {
            kind: "argument ownership",
            tag: 0xff
        })
    ));

    let mut unknown_alias_class = valid.clone();
    unknown_alias_class[ownership_position + 1] = 0xff;
    assert!(matches!(
        ManifestV1::from_bytes(&unknown_alias_class),
        Err(DecodeError::UnknownTag {
            kind: "alias class",
            tag: 0xff
        })
    ));

    let input_offset = input_name + b"input".len();
    let mut overlapping = valid;
    overlapping[input_offset..input_offset + 8].copy_from_slice(&0_u64.to_le_bytes());
    assert!(matches!(
        ManifestV1::from_bytes(&overlapping),
        Err(DecodeError::Validation(ValidationError::InvalidLayout(_)))
    ));
}
