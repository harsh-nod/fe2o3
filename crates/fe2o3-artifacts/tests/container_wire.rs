#[allow(dead_code)]
mod common;

use common::{kernel_with_object_digest, object_identity, target, text};
use fe2o3_artifacts::{
    ArtifactContainerV1, CONTAINER_HEADER_BYTES, CONTAINER_MAGIC, CONTAINER_VERSION,
    CodeObjectPayload, CompilerIdentity, ContainerDecodeError, ContainerValidationError,
    DecodeError, DigestAlgorithm, DigestBytes, MAX_CODE_OBJECT_BYTES, MAX_CODE_OBJECTS,
    MAX_EMBEDDED_PAYLOAD_BYTES, MAX_MANIFEST_BYTES, ManifestV1, PAYLOAD_DESCRIPTOR_BYTES,
    PointerWidth, ToolIdentity,
};

fn manifest_for(values: &[&[u8]]) -> ManifestV1 {
    let identities = values
        .iter()
        .map(|bytes| {
            let digest = DigestAlgorithm::Sha256.calculate(bytes).bytes();
            object_identity(digest, bytes.len() as u64)
        })
        .collect::<Vec<_>>();
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        identities,
        vec![kernel_with_object_digest(
            0x11,
            "kernel",
            "kernel.kd",
            DigestAlgorithm::Sha256.calculate(values[0]).bytes(),
            vec![],
        )],
    )
    .unwrap()
}

fn container_for(values: &[&[u8]]) -> ArtifactContainerV1 {
    let manifest = manifest_for(values);
    let payloads = values
        .iter()
        .map(|bytes| {
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, bytes.to_vec()).unwrap()
        })
        .collect();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, payloads).unwrap()
}

fn raw_container(manifest: &ManifestV1, entries: &[(DigestBytes, &[u8])]) -> Vec<u8> {
    let manifest = manifest.to_bytes();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&CONTAINER_MAGIC);
    bytes.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&manifest);
    for (digest, payload) in entries {
        bytes.extend_from_slice(digest.as_bytes());
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    }
    for (_, payload) in entries {
        bytes.extend_from_slice(payload);
    }
    bytes
}

fn manifest_len(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize
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

#[test]
fn v1_round_trip_is_canonical_and_content_bound() {
    let encoded = container_for(&[b"second", b"first"]).to_bytes();
    let decoded = ArtifactContainerV1::from_bytes(&encoded).unwrap();

    assert_eq!(decoded.to_bytes(), encoded);
    assert_eq!(decoded.digest_algorithm(), DigestAlgorithm::Sha256);
    assert_eq!(decoded.payloads().len(), 2);
    for (identity, payload) in decoded
        .manifest()
        .code_objects()
        .iter()
        .zip(decoded.payloads())
    {
        assert_eq!(identity.digest(), payload.digest().bytes());
        assert_eq!(identity.byte_len(), payload.bytes().len() as u64);
        assert_eq!(payload.digest().verify(payload.bytes()), Ok(()));
    }
}

#[test]
fn v1_container_golden_bytes_are_stable() {
    const GOLDEN_HEX: &str = "4645324f33414300010000000100000078020000010000004645324f33414d0001000000050072757374630600312e39342e3005006665326f330500302e312e301100616d6467636e2d616d642d616d646873610700676678313130300100000001000000ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad00030000000000000001000000111111111111111111111111111111111111111111111111111111111111111106006b65726e656c09006b65726e656c2e6b6422222222222222222222222222222222222222222222222222222222222222223333333333333333333333333333333333333333333333333333333333333333ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad00000101000100000100000001000000ffff000001000000010000000000000000100000200000000000000008000000030001006e00000000000000000400000000000000040000000005000000a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a100000500696e707574080000000000000008000000000000000800000001040000000000000004000000000101b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1010106006f7574707574100000000000000010000000000000000800000002040000000000000004000000010301c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c1c10202ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad0300000000000000616263";
    assert_eq!(hex(&container_for(&[b"abc"]).to_bytes()), GOLDEN_HEX);
}

#[test]
fn malformed_headers_lengths_counts_and_manifest_are_rejected() {
    let valid = container_for(&[b"abc"]).to_bytes();

    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 0xff;
    assert_eq!(
        ArtifactContainerV1::from_bytes(&bad_magic),
        Err(ContainerDecodeError::InvalidMagic)
    );

    let mut bad_version = valid.clone();
    bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        ArtifactContainerV1::from_bytes(&bad_version),
        Err(ContainerDecodeError::UnknownVersion(2))
    );

    let mut bad_flags = valid.clone();
    bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        ArtifactContainerV1::from_bytes(&bad_flags),
        Err(ContainerDecodeError::UnsupportedFlags(1))
    );

    let mut bad_algorithm = valid.clone();
    bad_algorithm[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        ArtifactContainerV1::from_bytes(&bad_algorithm),
        Err(ContainerDecodeError::UnknownDigestAlgorithm(u16::MAX))
    );

    let mut bad_reserved = valid.clone();
    bad_reserved[14..16].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        ArtifactContainerV1::from_bytes(&bad_reserved),
        Err(ContainerDecodeError::NonZeroReserved(1))
    );

    let mut empty_manifest = valid.clone();
    empty_manifest[16..20].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&empty_manifest),
        Err(ContainerDecodeError::LengthOutOfRange {
            field: "manifest",
            value: 0,
            ..
        })
    ));

    let mut huge_manifest = valid.clone();
    huge_manifest[16..20].copy_from_slice(&((MAX_MANIFEST_BYTES + 1) as u32).to_le_bytes());
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&huge_manifest),
        Err(ContainerDecodeError::LengthOutOfRange {
            field: "manifest",
            ..
        })
    ));

    let mut zero_payloads = valid.clone();
    zero_payloads[20..24].copy_from_slice(&0_u32.to_le_bytes());
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&zero_payloads),
        Err(ContainerDecodeError::LengthOutOfRange {
            field: "payload count",
            value: 0,
            ..
        })
    ));

    let mut too_many_payloads = valid.clone();
    too_many_payloads[20..24].copy_from_slice(&((MAX_CODE_OBJECTS + 1) as u32).to_le_bytes());
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&too_many_payloads),
        Err(ContainerDecodeError::LengthOutOfRange {
            field: "payload count",
            ..
        })
    ));

    let mut bad_manifest = valid.clone();
    bad_manifest[CONTAINER_HEADER_BYTES] ^= 0xff;
    assert_eq!(
        ArtifactContainerV1::from_bytes(&bad_manifest),
        Err(ContainerDecodeError::Manifest(DecodeError::InvalidMagic))
    );

    let descriptor = CONTAINER_HEADER_BYTES + manifest_len(&valid);
    let mut huge_payload = valid;
    huge_payload[descriptor + 32..descriptor + PAYLOAD_DESCRIPTOR_BYTES]
        .copy_from_slice(&((MAX_CODE_OBJECT_BYTES + 1) as u64).to_le_bytes());
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&huge_payload),
        Err(ContainerDecodeError::LengthOutOfRange {
            field: "code-object payload",
            ..
        })
    ));
}

#[test]
fn payload_descriptor_order_duplicates_and_exact_closure_are_rejected() {
    let first = b"first".as_slice();
    let second = b"second".as_slice();
    let extra = b"extra".as_slice();
    let manifest = manifest_for(&[first, second]);
    let mut entries =
        [first, second].map(|bytes| (DigestAlgorithm::Sha256.calculate(bytes).bytes(), bytes));
    entries.sort_unstable_by_key(|(digest, _)| *digest);

    let mut reversed = entries;
    reversed.reverse();
    assert_eq!(
        ArtifactContainerV1::from_bytes(&raw_container(&manifest, &reversed)),
        Err(ContainerDecodeError::NonCanonicalPayloadOrder)
    );

    let duplicate = [entries[0], entries[0]];
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&raw_container(&manifest, &duplicate)),
        Err(ContainerDecodeError::Validation(
            ContainerValidationError::DuplicatePayload(_)
        ))
    ));

    assert!(matches!(
        ArtifactContainerV1::from_bytes(&raw_container(&manifest, &entries[..1])),
        Err(ContainerDecodeError::Validation(
            ContainerValidationError::MissingPayload(_)
        ))
    ));

    let one_manifest = manifest_for(&[first]);
    let mut with_extra =
        [first, extra].map(|bytes| (DigestAlgorithm::Sha256.calculate(bytes).bytes(), bytes));
    with_extra.sort_unstable_by_key(|(digest, _)| *digest);
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&raw_container(&one_manifest, &with_extra)),
        Err(ContainerDecodeError::Validation(
            ContainerValidationError::ExtraPayload(_)
        ))
    ));
}

#[test]
fn modified_truncated_trailing_and_aggregate_oversized_payloads_are_rejected() {
    let valid = container_for(&[b"abc"]).to_bytes();
    let descriptor = CONTAINER_HEADER_BYTES + manifest_len(&valid);
    let payload = descriptor + PAYLOAD_DESCRIPTOR_BYTES;

    let mut modified = valid.clone();
    modified[payload] ^= 1;
    assert!(matches!(
        ArtifactContainerV1::from_bytes(&modified),
        Err(ContainerDecodeError::Validation(
            ContainerValidationError::DigestMismatch(_)
        ))
    ));

    assert_eq!(
        ArtifactContainerV1::from_bytes(&valid[..valid.len() - 1]),
        Err(ContainerDecodeError::Truncated)
    );

    let mut trailing = valid;
    trailing.push(0);
    assert_eq!(
        ArtifactContainerV1::from_bytes(&trailing),
        Err(ContainerDecodeError::TrailingBytes)
    );

    let manifest = manifest_for(&[b"x"]);
    let manifest_bytes = manifest.to_bytes();
    let count = MAX_EMBEDDED_PAYLOAD_BYTES / MAX_CODE_OBJECT_BYTES + 1;
    let mut declared_oversized = Vec::new();
    declared_oversized.extend_from_slice(&CONTAINER_MAGIC);
    declared_oversized.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
    declared_oversized.extend_from_slice(&0_u16.to_le_bytes());
    declared_oversized.extend_from_slice(&1_u16.to_le_bytes());
    declared_oversized.extend_from_slice(&0_u16.to_le_bytes());
    declared_oversized.extend_from_slice(&(manifest_bytes.len() as u32).to_le_bytes());
    declared_oversized.extend_from_slice(&(count as u32).to_le_bytes());
    declared_oversized.extend_from_slice(&manifest_bytes);
    for id in 0..count {
        declared_oversized.extend_from_slice(&[id as u8; 32]);
        declared_oversized.extend_from_slice(&(MAX_CODE_OBJECT_BYTES as u64).to_le_bytes());
    }
    assert_eq!(
        ArtifactContainerV1::from_bytes(&declared_oversized),
        Err(ContainerDecodeError::PayloadBytesTooLarge {
            max: MAX_EMBEDDED_PAYLOAD_BYTES
        })
    );
}
