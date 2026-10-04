#[allow(dead_code)]
mod common;

use fe2o3_artifacts::{
    ArtifactContainerV1, CodeObjectPayload, CompilerIdentity, ContainerValidationError,
    DigestAlgorithm, MAX_CODE_OBJECTS, ManifestV1, PointerWidth, ToolIdentity,
};

use common::{kernel_with_object_digest, object_identity, target, text};

fn payload(bytes: &[u8]) -> CodeObjectPayload {
    CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, bytes.to_vec()).unwrap()
}

fn manifest_for(payloads: &[&CodeObjectPayload]) -> ManifestV1 {
    let objects = payloads
        .iter()
        .map(|payload| object_identity(payload.digest().bytes(), payload.bytes().len() as u64))
        .collect();
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        objects,
        vec![kernel_with_object_digest(
            0x11,
            "kernel",
            "kernel.kd",
            payloads[0].digest().bytes(),
            vec![],
        )],
    )
    .unwrap()
}

#[test]
fn container_validates_and_canonicalizes_the_exact_manifest_closure() {
    let first = payload(b"first code object");
    let second = payload(b"second code object");
    let manifest = manifest_for(&[&first, &second]);

    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![second, first]).unwrap();

    assert_eq!(container.digest_algorithm(), DigestAlgorithm::Sha256);
    assert_eq!(container.payloads().len(), 2);
    assert!(container.payloads()[0].digest().bytes() < container.payloads()[1].digest().bytes());
    for (identity, embedded) in container
        .manifest()
        .code_objects()
        .iter()
        .zip(container.payloads())
    {
        assert_eq!(identity.digest(), embedded.digest().bytes());
        assert_eq!(identity.byte_len(), embedded.bytes().len() as u64);
        assert_eq!(embedded.digest().verify(embedded.bytes()), Ok(()));
    }
}

#[test]
fn payload_construction_rejects_empty_or_modified_bytes() {
    assert_eq!(
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, vec![]),
        Err(ContainerValidationError::EmptyPayload)
    );

    let expected = DigestAlgorithm::Sha256.calculate(b"expected");
    assert!(matches!(
        CodeObjectPayload::new(expected, b"modified".to_vec()),
        Err(ContainerValidationError::DigestMismatch(_))
    ));
}

#[test]
fn container_rejects_duplicate_missing_and_extra_payloads() {
    let first = payload(b"first");
    let duplicate = payload(b"first");
    let second = payload(b"second");
    let extra = payload(b"extra");
    let first_digest = first.digest().bytes();
    let second_digest = second.digest().bytes();
    let extra_digest = extra.digest().bytes();

    let duplicate_manifest = manifest_for(&[&first]);
    assert_eq!(
        ArtifactContainerV1::new(
            duplicate_manifest,
            DigestAlgorithm::Sha256,
            vec![first, duplicate]
        ),
        Err(ContainerValidationError::DuplicatePayload(first_digest))
    );

    let first = payload(b"first");
    let second = payload(b"second");
    let missing_manifest = manifest_for(&[&first, &second]);
    assert!(matches!(
        ArtifactContainerV1::new(
            missing_manifest,
            DigestAlgorithm::Sha256,
            vec![payload(b"first")]
        ),
        Err(ContainerValidationError::MissingPayload(digest)) if digest == second_digest
    ));

    let first = payload(b"first");
    let extra_manifest = manifest_for(&[&first]);
    assert!(matches!(
        ArtifactContainerV1::new(
            extra_manifest,
            DigestAlgorithm::Sha256,
            vec![payload(b"first"), extra]
        ),
        Err(ContainerValidationError::ExtraPayload(digest)) if digest == extra_digest
    ));
}

#[test]
fn container_rejects_declared_length_mismatches() {
    let value = payload(b"code object");
    let digest = value.digest().bytes();
    let wrong_length_manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![object_identity(digest, value.bytes().len() as u64 + 1)],
        vec![kernel_with_object_digest(
            0x11,
            "kernel",
            "kernel.kd",
            digest,
            vec![],
        )],
    )
    .unwrap();
    assert!(matches!(
        ArtifactContainerV1::new(
            wrong_length_manifest,
            DigestAlgorithm::Sha256,
            vec![value]
        ),
        Err(ContainerValidationError::PayloadLengthMismatch { digest: actual, .. })
            if actual == digest
    ));
}

#[test]
fn container_rejects_payload_counts_above_the_manifest_limit() {
    let value = payload(b"manifest payload");
    let manifest = manifest_for(&[&value]);
    let payloads = (0..=MAX_CODE_OBJECTS)
        .map(|index| payload(index.to_string().as_bytes()))
        .collect();

    assert_eq!(
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, payloads),
        Err(ContainerValidationError::TooManyPayloads {
            max: MAX_CODE_OBJECTS
        })
    );
}
