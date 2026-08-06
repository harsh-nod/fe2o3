#[allow(dead_code)]
mod common;

use common::{digest, kernel_with_object_digest, name, object_identity, target, text};
use fe2o3_artifacts::{
    ArtifactContainerV1, BUNDLE_INDEX_MAGIC, BUNDLE_INDEX_VERSION, BundleDecodeError,
    BundleIndexV1, BundleKernelIndexEntryV1, BundlePayloadReferenceV1, BundleTargetAssociationV1,
    BundleValidationError, Capability, CodeObjectFormat, CodeObjectPayload, CompilerIdentity,
    DigestAlgorithm, MAX_BUNDLE_INDEX_BYTES, MAX_BUNDLE_KERNELS, MAX_BUNDLE_PAYLOAD_REFERENCES,
    MAX_BUNDLE_TARGET_ASSOCIATIONS, MAX_CODE_OBJECT_BYTES, MAX_KERNEL_PAYLOAD_REFERENCES,
    ManifestV1, PointerWidth, ToolIdentity,
};

fn sample_index() -> BundleIndexV1 {
    BundleIndexV1::new(
        vec![
            BundleTargetAssociationV1::new(
                digest(0x20),
                target(PointerWidth::Bits64, vec![Capability::Atomics]),
            ),
            BundleTargetAssociationV1::new(
                digest(0x10),
                target(PointerWidth::Bits64, vec![Capability::AmdWave]),
            ),
        ],
        vec![payload_reference(0x40), payload_reference(0x30)],
        vec![
            kernel_reference(0x60, "z_kernel.kd", 0x20, &[0x40]),
            kernel_reference(0x50, "a_kernel.kd", 0x10, &[0x30]),
        ],
    )
    .unwrap()
}

fn payload_reference(id: u8) -> BundlePayloadReferenceV1 {
    BundlePayloadReferenceV1::new(
        digest(id),
        CodeObjectFormat::NativeExecutable,
        u64::from(id),
    )
    .unwrap()
}

fn kernel_reference(
    id: u8,
    symbol: &str,
    manifest: u8,
    payloads: &[u8],
) -> BundleKernelIndexEntryV1 {
    BundleKernelIndexEntryV1::new(
        digest(id),
        name(symbol),
        digest(manifest),
        payloads.iter().copied().map(digest).collect(),
    )
    .unwrap()
}

fn container() -> ArtifactContainerV1 {
    let first = CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"first".to_vec()).unwrap();
    let second =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"second".to_vec()).unwrap();
    let first_digest = first.digest().bytes();
    let second_digest = second.digest().bytes();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![Capability::AmdWave]),
        vec![
            object_identity(first_digest, first.bytes().len() as u64),
            object_identity(second_digest, second.bytes().len() as u64),
        ],
        vec![
            kernel_with_object_digest(
                0x12,
                "second_kernel",
                "second_kernel.kd",
                second_digest,
                vec![],
            ),
            kernel_with_object_digest(
                0x11,
                "first_kernel",
                "first_kernel.kd",
                first_digest,
                vec![Capability::AmdWave],
            ),
        ],
    )
    .unwrap();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![second, first]).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0xf)]));
    }
    output
}

fn find(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap()
}

#[test]
fn validated_containers_derive_a_canonical_closed_index() {
    let container = container();
    let index = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
    let expected_manifest_digest = DigestAlgorithm::Sha256
        .calculate(&container.manifest().to_bytes())
        .bytes();

    assert_eq!(index.target_associations().len(), 1);
    assert_eq!(
        index.target_associations()[0].manifest_digest(),
        expected_manifest_digest
    );
    assert_eq!(
        index.target_associations()[0].target(),
        container.manifest().target()
    );
    assert_eq!(index.payloads().len(), 2);
    assert_eq!(index.kernels().len(), 2);
    assert!(index.payloads()[0].digest() < index.payloads()[1].digest());
    assert!(index.kernels()[0].kernel_id() < index.kernels()[1].kernel_id());
    for kernel in index.kernels() {
        assert_eq!(kernel.manifest_digest(), expected_manifest_digest);
        assert_eq!(kernel.payload_digests().len(), 1);
        assert!(
            index
                .payloads()
                .iter()
                .any(|payload| payload.digest() == kernel.payload_digests()[0])
        );
    }
}

#[test]
fn v1_round_trip_is_canonical_for_set_like_input() {
    let index = sample_index();
    let encoded = index.to_bytes();
    let decoded = BundleIndexV1::from_bytes(&encoded).unwrap();

    assert_eq!(decoded, index);
    assert_eq!(decoded.to_bytes(), encoded);
    assert_eq!(
        decoded.target_associations()[0].manifest_digest(),
        digest(0x10)
    );
    assert_eq!(decoded.payloads()[0].digest(), digest(0x30));
    assert_eq!(decoded.kernels()[0].kernel_id(), digest(0x50));
}

#[test]
fn v1_golden_bytes_are_stable() {
    const GOLDEN_HEX: &str = "4645324f33424900010000000200000010101010101010101010101010101010101010101010101010101010101010101100616d6467636e2d616d642d616d6468736107006766783131303001000100070020202020202020202020202020202020202020202020202020202020202020201100616d6467636e2d616d642d616d6468736107006766783131303001000100060002000000303030303030303030303030303030303030303030303030303030303030303000300000000000000040404040404040404040404040404040404040404040404040404040404040400040000000000000000200000050505050505050505050505050505050505050505050505050505050505050500b00615f6b65726e656c2e6b6410101010101010101010101010101010101010101010101010101010101010100100303030303030303030303030303030303030303030303030303030303030303060606060606060606060606060606060606060606060606060606060606060600b007a5f6b65726e656c2e6b64202020202020202020202020202020202020202020202020202020202020202001004040404040404040404040404040404040404040404040404040404040404040";

    assert_eq!(hex(&sample_index().to_bytes()), GOLDEN_HEX);
}

#[test]
fn duplicate_and_dangling_references_are_rejected() {
    let target_a =
        BundleTargetAssociationV1::new(digest(0x10), target(PointerWidth::Bits64, vec![]));
    let target_b =
        BundleTargetAssociationV1::new(digest(0x20), target(PointerWidth::Bits64, vec![]));
    let payload_a = payload_reference(0x30);
    let payload_b = payload_reference(0x40);
    let kernel_a = kernel_reference(0x50, "a.kd", 0x10, &[0x30]);

    assert_eq!(
        BundleIndexV1::new(
            vec![target_a.clone(), target_a.clone()],
            vec![payload_a.clone()],
            vec![kernel_a.clone()],
        ),
        Err(BundleValidationError::Duplicate {
            field: "bundle manifest digest"
        })
    );
    assert_eq!(
        BundleIndexV1::new(
            vec![target_a.clone()],
            vec![payload_a.clone(), payload_a.clone()],
            vec![kernel_a.clone()],
        ),
        Err(BundleValidationError::Duplicate {
            field: "bundle payload digest"
        })
    );
    assert_eq!(
        BundleIndexV1::new(
            vec![target_a.clone()],
            vec![payload_a.clone()],
            vec![
                kernel_a.clone(),
                kernel_reference(0x50, "different.kd", 0x10, &[0x30]),
            ],
        ),
        Err(BundleValidationError::Duplicate {
            field: "bundle kernel ID"
        })
    );
    assert_eq!(
        BundleIndexV1::new(
            vec![target_a.clone(), target_b],
            vec![payload_a.clone(), payload_b],
            vec![
                kernel_a.clone(),
                kernel_reference(0x60, "a.kd", 0x20, &[0x40]),
            ],
        ),
        Err(BundleValidationError::Duplicate {
            field: "bundle kernel symbol"
        })
    );
    assert_eq!(
        BundleIndexV1::new(
            vec![target_a.clone()],
            vec![payload_a.clone()],
            vec![kernel_reference(0x60, "missing_target.kd", 0x20, &[0x30])],
        ),
        Err(BundleValidationError::MissingTargetAssociation(digest(
            0x20
        )))
    );
    assert_eq!(
        BundleIndexV1::new(
            vec![target_a],
            vec![payload_a],
            vec![kernel_reference(0x60, "missing_payload.kd", 0x10, &[0x40])],
        ),
        Err(BundleValidationError::MissingPayload(digest(0x40)))
    );
}

#[test]
fn per_record_and_aggregate_bounds_are_enforced() {
    assert!(matches!(
        BundlePayloadReferenceV1::new(digest(0x30), CodeObjectFormat::NativeExecutable, 0),
        Err(BundleValidationError::InvalidPayloadLength { .. })
    ));
    assert!(matches!(
        BundlePayloadReferenceV1::new(
            digest(0x30),
            CodeObjectFormat::NativeExecutable,
            MAX_CODE_OBJECT_BYTES as u64 + 1,
        ),
        Err(BundleValidationError::InvalidPayloadLength { .. })
    ));
    assert_eq!(
        BundleKernelIndexEntryV1::new(digest(0x50), name("empty.kd"), digest(0x10), vec![]),
        Err(BundleValidationError::EmptyCollection {
            field: "kernel payload references"
        })
    );
    assert_eq!(
        BundleKernelIndexEntryV1::new(
            digest(0x50),
            name("duplicate.kd"),
            digest(0x10),
            vec![digest(0x30), digest(0x30)],
        ),
        Err(BundleValidationError::Duplicate {
            field: "kernel payload reference"
        })
    );
    let canonical = BundleKernelIndexEntryV1::new(
        digest(0x50),
        name("canonical.kd"),
        digest(0x10),
        vec![digest(0x40), digest(0x30)],
    )
    .unwrap();
    assert_eq!(canonical.payload_digests(), &[digest(0x30), digest(0x40)]);
    assert!(matches!(
        BundleKernelIndexEntryV1::new(
            digest(0x50),
            name("too_many.kd"),
            digest(0x10),
            vec![digest(0x30); MAX_KERNEL_PAYLOAD_REFERENCES + 1],
        ),
        Err(BundleValidationError::TooMany { .. })
    ));
}

#[test]
fn malformed_envelope_counts_tags_and_order_are_rejected() {
    let valid = sample_index().to_bytes();

    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 0xff;
    assert_eq!(
        BundleIndexV1::from_bytes(&bad_magic),
        Err(BundleDecodeError::InvalidMagic)
    );

    let mut bad_version = valid.clone();
    bad_version[8..10].copy_from_slice(&(BUNDLE_INDEX_VERSION + 1).to_le_bytes());
    assert_eq!(
        BundleIndexV1::from_bytes(&bad_version),
        Err(BundleDecodeError::UnknownVersion(BUNDLE_INDEX_VERSION + 1))
    );

    let mut bad_flags = valid.clone();
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        BundleIndexV1::from_bytes(&bad_flags),
        Err(BundleDecodeError::UnsupportedFlags(1))
    );

    let mut zero_targets = valid.clone();
    zero_targets[12..16].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        BundleIndexV1::from_bytes(&zero_targets),
        Err(BundleDecodeError::CountOutOfRange {
            field: "bundle target associations",
            count: 0,
            ..
        })
    ));

    let mut too_many_targets = valid.clone();
    too_many_targets[12..16]
        .copy_from_slice(&((MAX_BUNDLE_TARGET_ASSOCIATIONS + 1) as u32).to_le_bytes());
    assert!(matches!(
        BundleIndexV1::from_bytes(&too_many_targets),
        Err(BundleDecodeError::CountOutOfRange { .. })
    ));

    let architecture = find(&valid, b"gfx1100");
    let pointer_width = architecture + b"gfx1100".len();
    let mut unknown_pointer_width = valid.clone();
    unknown_pointer_width[pointer_width] = 0xff;
    assert!(matches!(
        BundleIndexV1::from_bytes(&unknown_pointer_width),
        Err(BundleDecodeError::UnknownTag {
            kind: "pointer width",
            tag: 0xff
        })
    ));

    let payload = find(&valid, &[0x30; 32]);
    let payload_count = payload - 4;
    let mut too_many_payloads = valid.clone();
    too_many_payloads[payload_count..payload_count + 4]
        .copy_from_slice(&((MAX_BUNDLE_PAYLOAD_REFERENCES + 1) as u32).to_le_bytes());
    assert!(matches!(
        BundleIndexV1::from_bytes(&too_many_payloads),
        Err(BundleDecodeError::CountOutOfRange {
            field: "bundle payload references",
            ..
        })
    ));

    let mut unknown_format = valid.clone();
    unknown_format[payload + 32] = 0xff;
    assert!(matches!(
        BundleIndexV1::from_bytes(&unknown_format),
        Err(BundleDecodeError::UnknownTag {
            kind: "code object format",
            tag: 0xff
        })
    ));

    let first_target = find(&valid, &[0x10; 32]);
    let second_target = find(&valid, &[0x20; 32]);
    let mut noncanonical_targets = valid.clone();
    noncanonical_targets[first_target..first_target + 32].copy_from_slice(&[0x20; 32]);
    noncanonical_targets[second_target..second_target + 32].copy_from_slice(&[0x10; 32]);
    assert_eq!(
        BundleIndexV1::from_bytes(&noncanonical_targets),
        Err(BundleDecodeError::NonCanonicalOrder {
            field: "bundle target associations"
        })
    );

    let mut duplicate_targets = valid.clone();
    duplicate_targets[second_target..second_target + 32].copy_from_slice(&[0x10; 32]);
    assert!(matches!(
        BundleIndexV1::from_bytes(&duplicate_targets),
        Err(BundleDecodeError::Validation(
            BundleValidationError::Duplicate {
                field: "bundle manifest digest"
            }
        ))
    ));

    let first_kernel = find(&valid, &[0x50; 32]);
    let kernel_count = first_kernel - 4;
    let mut too_many_kernels = valid.clone();
    too_many_kernels[kernel_count..kernel_count + 4]
        .copy_from_slice(&((MAX_BUNDLE_KERNELS + 1) as u32).to_le_bytes());
    assert!(matches!(
        BundleIndexV1::from_bytes(&too_many_kernels),
        Err(BundleDecodeError::CountOutOfRange {
            field: "bundle kernels",
            ..
        })
    ));

    let first_symbol = find(&valid, b"a_kernel.kd");
    let kernel_manifest = first_symbol + b"a_kernel.kd".len();
    let kernel_payload_count = kernel_manifest + 32;
    let kernel_payload = kernel_payload_count + 2;
    let mut too_many_kernel_payloads = valid.clone();
    too_many_kernel_payloads[kernel_payload_count..kernel_payload_count + 2]
        .copy_from_slice(&((MAX_KERNEL_PAYLOAD_REFERENCES + 1) as u16).to_le_bytes());
    assert!(matches!(
        BundleIndexV1::from_bytes(&too_many_kernel_payloads),
        Err(BundleDecodeError::CountOutOfRange {
            field: "kernel payload references",
            ..
        })
    ));

    let mut dangling_manifest = valid.clone();
    dangling_manifest[kernel_manifest..kernel_manifest + 32].copy_from_slice(&[0x99; 32]);
    assert_eq!(
        BundleIndexV1::from_bytes(&dangling_manifest),
        Err(BundleDecodeError::Validation(
            BundleValidationError::MissingTargetAssociation(digest(0x99))
        ))
    );

    let mut dangling_payload = valid.clone();
    dangling_payload[kernel_payload..kernel_payload + 32].copy_from_slice(&[0x99; 32]);
    assert_eq!(
        BundleIndexV1::from_bytes(&dangling_payload),
        Err(BundleDecodeError::Validation(
            BundleValidationError::MissingPayload(digest(0x99))
        ))
    );

    let second_kernel = find(&valid, &[0x60; 32]);
    let mut duplicate_kernel_id = valid.clone();
    duplicate_kernel_id[second_kernel..second_kernel + 32].copy_from_slice(&[0x50; 32]);
    assert!(matches!(
        BundleIndexV1::from_bytes(&duplicate_kernel_id),
        Err(BundleDecodeError::Validation(
            BundleValidationError::Duplicate {
                field: "bundle kernel ID"
            }
        ))
    ));

    let second_symbol = find(&valid, b"z_kernel.kd");
    let mut duplicate_kernel_symbol = valid.clone();
    duplicate_kernel_symbol[second_symbol..second_symbol + b"z_kernel.kd".len()]
        .copy_from_slice(b"a_kernel.kd");
    assert!(matches!(
        BundleIndexV1::from_bytes(&duplicate_kernel_symbol),
        Err(BundleDecodeError::Validation(
            BundleValidationError::Duplicate {
                field: "bundle kernel symbol"
            }
        ))
    ));

    let mut trailing = valid.clone();
    trailing.push(0);
    assert_eq!(
        BundleIndexV1::from_bytes(&trailing),
        Err(BundleDecodeError::TrailingBytes)
    );

    assert_eq!(
        BundleIndexV1::from_bytes(&vec![0; MAX_BUNDLE_INDEX_BYTES + 1]),
        Err(BundleDecodeError::TooLarge {
            max: MAX_BUNDLE_INDEX_BYTES
        })
    );

    assert_eq!(&valid[..8], &BUNDLE_INDEX_MAGIC);
    assert!(MAX_BUNDLE_PAYLOAD_REFERENCES >= sample_index().payloads().len());
    assert!(MAX_BUNDLE_KERNELS >= sample_index().kernels().len());
}
