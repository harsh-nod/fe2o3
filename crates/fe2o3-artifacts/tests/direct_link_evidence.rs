#[allow(dead_code)]
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::{digest, kernel_with_object_digest, object_identity, target, text};
use fe2o3_artifacts::{
    ArtifactContainerV1, BundleIndexV1, CodeObjectFormat, CodeObjectIdentity, CodeObjectPayload,
    CompilerIdentity, DIRECT_LINK_EVIDENCE_HEADER_BYTES, DIRECT_LINK_EVIDENCE_VERSION,
    DigestAlgorithm, DirectLinkBindingExpectationV1, DirectLinkBindingSourceV1,
    DirectLinkBundleEvidenceV1, DirectLinkDecodeError, DirectLinkEvidenceError,
    DirectLinkToolIdentityV1, DirectLinkTransformationIdentityV1, MAX_DIRECT_LINK_BINDINGS,
    MAX_DIRECT_LINK_EVIDENCE_BYTES, ManifestV1, PayloadDigest, PointerWidth, ToolIdentity,
};

struct Fixture {
    container: ArtifactContainerV1,
    expectation: DirectLinkBindingExpectationV1,
}

fn measured(seed: u8, name: &str) -> DirectLinkToolIdentityV1 {
    DirectLinkToolIdentityV1::new(
        text(name),
        text("22.0.0-build.17"),
        tagged(seed),
        tagged(seed.wrapping_add(1)),
    )
}

fn tagged(seed: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(seed))
}

fn expectation(seed: u8, payload: PayloadDigest) -> DirectLinkBindingExpectationV1 {
    DirectLinkBindingExpectationV1::new(
        tagged(seed),
        measured(0x80, "fe2o3-llvm-link-worker"),
        measured(0x90, "rocm-llvm-lld"),
        tagged(seed.wrapping_add(1)),
        DirectLinkTransformationIdentityV1::new(
            tagged(seed.wrapping_add(2)),
            tagged(seed.wrapping_add(3)),
            payload,
        ),
        tagged(seed.wrapping_add(4)),
    )
}

fn fixture(seed: u8, format: CodeObjectFormat) -> Fixture {
    let bytes = vec![seed; 32];
    let payload = CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, bytes).unwrap();
    let payload_identity = payload.digest();
    let object = CodeObjectIdentity::new(
        payload_identity.bytes(),
        format,
        payload.bytes().len() as u64,
    )
    .unwrap();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![object],
        vec![kernel_with_object_digest(
            seed,
            &format!("kernel_{seed:02x}"),
            &format!("kernel_{seed:02x}.kd"),
            payload_identity.bytes(),
            vec![],
        )],
    )
    .unwrap();
    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
    Fixture {
        container,
        expectation: expectation(seed.wrapping_add(0x10), payload_identity),
    }
}

fn evidence<'a>(
    bundle: &BundleIndexV1,
    fixtures: impl IntoIterator<Item = &'a Fixture>,
) -> DirectLinkBundleEvidenceV1 {
    let fixtures = fixtures.into_iter().collect::<Vec<_>>();
    let sources = fixtures
        .iter()
        .map(|fixture| {
            DirectLinkBindingSourceV1::new(&fixture.container, fixture.expectation.clone())
        })
        .collect::<Vec<_>>();
    DirectLinkBundleEvidenceV1::bind(bundle, &sources).unwrap()
}

fn bundle_for(fixtures: &[&Fixture]) -> BundleIndexV1 {
    let indexes = fixtures
        .iter()
        .map(|fixture| {
            BundleIndexV1::from_containers(std::slice::from_ref(&fixture.container)).unwrap()
        })
        .collect::<Vec<_>>();
    BundleIndexV1::new(
        indexes
            .iter()
            .flat_map(|index| index.target_associations().iter().cloned())
            .collect(),
        indexes
            .iter()
            .flat_map(|index| index.payloads().iter().cloned())
            .collect(),
        indexes
            .iter()
            .flat_map(|index| index.kernels().iter().cloned())
            .collect(),
    )
    .unwrap()
}

fn find(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("needle must occur")
}

#[test]
fn canonical_round_trip_binds_exact_container_bundle_and_provenance() {
    let first = fixture(0x11, CodeObjectFormat::NativeExecutable);
    let second = fixture(0x22, CodeObjectFormat::NativeExecutable);
    let bundle = bundle_for(&[&first, &second]);
    let container_v1_before = first.container.to_bytes();
    let bundle_v1_before = bundle.to_bytes();

    let record = evidence(&bundle, [&second, &first]);
    let bytes = record.to_bytes();
    let decoded = DirectLinkBundleEvidenceV1::from_bytes(&bytes).unwrap();

    assert_eq!(decoded, record);
    assert_eq!(decoded.to_bytes(), bytes);
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert_eq!(first.container.to_bytes(), container_v1_before);
    assert_eq!(bundle.to_bytes(), bundle_v1_before);
    assert_eq!(decoded.bindings().len(), 2);
    assert!(
        decoded.bindings()[0]
            .expectation()
            .finalized_payload_identity()
            < decoded.bindings()[1]
                .expectation()
                .finalized_payload_identity()
    );

    let expectations = vec![first.expectation.clone(), second.expectation.clone()];
    let validation: Result<(), DirectLinkEvidenceError> =
        decoded.validate_against(&bundle, &[first.container, second.container], &expectations);
    assert_eq!(validation, Ok(()));
}

#[test]
fn every_provenance_substitution_is_rejected_by_external_matching() {
    let original = fixture(0x31, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&original.container)).unwrap();
    let record = evidence(&bundle, [&original]);
    let payload = original.expectation.finalized_payload_identity();
    let request = original.expectation.request_identity();
    let worker = original.expectation.worker().clone();
    let toolchain = original.expectation.toolchain().clone();
    let response = original.expectation.response_identity();
    let linked = original.expectation.linked_output_identity();
    let finalization = original.expectation.finalization_identity();
    let ffi = original.expectation.ffi_contract_identity();

    let substitutions = [
        DirectLinkBindingExpectationV1::new(
            tagged(0x01),
            worker.clone(),
            toolchain.clone(),
            response,
            DirectLinkTransformationIdentityV1::new(linked, finalization, payload),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            measured(0x02, "substituted-worker"),
            toolchain.clone(),
            response,
            DirectLinkTransformationIdentityV1::new(linked, finalization, payload),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            worker.clone(),
            measured(0x03, "substituted-toolchain"),
            response,
            DirectLinkTransformationIdentityV1::new(linked, finalization, payload),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            worker.clone(),
            toolchain.clone(),
            tagged(0x04),
            DirectLinkTransformationIdentityV1::new(linked, finalization, payload),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            worker.clone(),
            toolchain.clone(),
            response,
            DirectLinkTransformationIdentityV1::new(tagged(0x05), finalization, payload),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            worker.clone(),
            toolchain.clone(),
            response,
            DirectLinkTransformationIdentityV1::new(linked, tagged(0x06), payload),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            worker.clone(),
            toolchain.clone(),
            response,
            DirectLinkTransformationIdentityV1::new(linked, finalization, tagged(0x07)),
            ffi,
        ),
        DirectLinkBindingExpectationV1::new(
            request,
            worker,
            toolchain,
            response,
            DirectLinkTransformationIdentityV1::new(linked, finalization, payload),
            tagged(0x08),
        ),
    ];

    for substitution in substitutions {
        assert_eq!(
            record.validate_against(
                &bundle,
                std::slice::from_ref(&original.container),
                &[substitution]
            ),
            Err(DirectLinkEvidenceError::ExpectationMismatch)
        );
    }

    assert_ne!(original.expectation.linked_output_identity(), payload);
}

#[test]
fn container_and_bundle_substitution_and_extra_inputs_are_rejected() {
    let original = fixture(0x41, CodeObjectFormat::NativeExecutable);
    let replacement = fixture(0x42, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&original.container)).unwrap();
    let replacement_bundle =
        BundleIndexV1::from_containers(std::slice::from_ref(&replacement.container)).unwrap();
    let record = evidence(&bundle, [&original]);

    assert_eq!(
        record.validate_against(
            &replacement_bundle,
            std::slice::from_ref(&original.container),
            std::slice::from_ref(&original.expectation),
        ),
        Err(DirectLinkEvidenceError::BundleIdentityMismatch)
    );
    assert_eq!(
        record.validate_against(
            &bundle,
            std::slice::from_ref(&replacement.container),
            std::slice::from_ref(&original.expectation),
        ),
        Err(DirectLinkEvidenceError::MissingContainer)
    );
    assert_eq!(
        record.validate_against(
            &bundle,
            &[original.container, replacement.container],
            std::slice::from_ref(&original.expectation),
        ),
        Err(DirectLinkEvidenceError::ExtraContainer)
    );
}

#[test]
fn binding_requires_native_payload_and_complete_bundle_membership() {
    let native = fixture(0x51, CodeObjectFormat::NativeExecutable);
    let other = fixture(0x52, CodeObjectFormat::NativeExecutable);
    assert_ne!(
        native.expectation.linked_output_identity(),
        native.expectation.finalized_payload_identity()
    );
    let unrelated_bundle =
        BundleIndexV1::from_containers(std::slice::from_ref(&other.container)).unwrap();
    let source = DirectLinkBindingSourceV1::new(&native.container, native.expectation.clone());
    assert!(matches!(
        DirectLinkBundleEvidenceV1::bind(&unrelated_bundle, &[source]),
        Err(DirectLinkEvidenceError::ContainerBundleMismatch { .. })
    ));

    let native_bundle =
        BundleIndexV1::from_containers(std::slice::from_ref(&native.container)).unwrap();
    let changed_linked_output = DirectLinkBindingExpectationV1::new(
        native.expectation.request_identity(),
        native.expectation.worker().clone(),
        native.expectation.toolchain().clone(),
        native.expectation.response_identity(),
        DirectLinkTransformationIdentityV1::new(
            tagged(0xf1),
            native.expectation.finalization_identity(),
            native.expectation.finalized_payload_identity(),
        ),
        native.expectation.ffi_contract_identity(),
    );
    let source = DirectLinkBindingSourceV1::new(&native.container, changed_linked_output);
    assert!(DirectLinkBundleEvidenceV1::bind(&native_bundle, &[source]).is_ok());

    let changed_finalized_output = DirectLinkBindingExpectationV1::new(
        native.expectation.request_identity(),
        native.expectation.worker().clone(),
        native.expectation.toolchain().clone(),
        native.expectation.response_identity(),
        DirectLinkTransformationIdentityV1::new(
            native.expectation.linked_output_identity(),
            native.expectation.finalization_identity(),
            tagged(0xf2),
        ),
        native.expectation.ffi_contract_identity(),
    );
    let source = DirectLinkBindingSourceV1::new(&native.container, changed_finalized_output);
    assert_eq!(
        DirectLinkBundleEvidenceV1::bind(&native_bundle, &[source]),
        Err(DirectLinkEvidenceError::MissingFinalizedPayload)
    );

    let relocatable = fixture(0x53, CodeObjectFormat::RelocatableObject);
    let relocatable_bundle =
        BundleIndexV1::from_containers(std::slice::from_ref(&relocatable.container)).unwrap();
    let source =
        DirectLinkBindingSourceV1::new(&relocatable.container, relocatable.expectation.clone());
    assert_eq!(
        DirectLinkBundleEvidenceV1::bind(&relocatable_bundle, &[source]),
        Err(DirectLinkEvidenceError::FinalizedPayloadNotNative)
    );

    let referenced =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"referenced native".to_vec())
            .unwrap();
    let unreferenced =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"unreferenced native".to_vec())
            .unwrap();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![
            object_identity(referenced.digest().bytes(), referenced.bytes().len() as u64),
            object_identity(
                unreferenced.digest().bytes(),
                unreferenced.bytes().len() as u64,
            ),
        ],
        vec![kernel_with_object_digest(
            0x54,
            "referenced",
            "referenced.kd",
            referenced.digest().bytes(),
            vec![],
        )],
    )
    .unwrap();
    let unreferenced_identity = unreferenced.digest();
    let container = ArtifactContainerV1::new(
        manifest,
        DigestAlgorithm::Sha256,
        vec![referenced, unreferenced],
    )
    .unwrap();
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
    let source =
        DirectLinkBindingSourceV1::new(&container, expectation(0x55, unreferenced_identity));
    assert_eq!(
        DirectLinkBundleEvidenceV1::bind(&bundle, &[source]),
        Err(DirectLinkEvidenceError::UnreferencedFinalizedPayload)
    );
}

#[test]
fn constructor_rejects_empty_duplicate_and_mismatched_closures() {
    let item = fixture(0x61, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&item.container)).unwrap();
    assert_eq!(
        DirectLinkBundleEvidenceV1::bind(&bundle, &[]),
        Err(DirectLinkEvidenceError::EmptyBindings)
    );

    let sources = [
        DirectLinkBindingSourceV1::new(&item.container, item.expectation.clone()),
        DirectLinkBindingSourceV1::new(&item.container, item.expectation.clone()),
    ];
    assert_eq!(
        DirectLinkBundleEvidenceV1::bind(&bundle, &sources),
        Err(DirectLinkEvidenceError::Duplicate {
            field: "request identity"
        })
    );
}

#[test]
fn every_truncated_prefix_and_trailing_data_is_rejected_without_panicking() {
    let item = fixture(0x71, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&item.container)).unwrap();
    let bytes = evidence(&bundle, [&item]).to_bytes();

    for length in 0..bytes.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            DirectLinkBundleEvidenceV1::from_bytes(&bytes[..length])
        }));
        assert!(result.is_ok(), "decoder panicked at prefix {length}");
        assert!(result.unwrap().is_err(), "decoder accepted prefix {length}");
    }

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&trailing),
        Err(DirectLinkDecodeError::TrailingBytes)
    );
}

#[test]
fn header_downgrades_unknowns_bounds_and_oversized_text_are_rejected() {
    let item = fixture(0x72, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&item.container)).unwrap();
    let valid = evidence(&bundle, [&item]).to_bytes();

    let mut bad_magic = valid.clone();
    bad_magic[0] ^= 1;
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&bad_magic),
        Err(DirectLinkDecodeError::InvalidMagic)
    );

    for version in [0, DIRECT_LINK_EVIDENCE_VERSION + 1] {
        let mut changed = valid.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert_eq!(
            DirectLinkBundleEvidenceV1::from_bytes(&changed),
            Err(DirectLinkDecodeError::UnknownVersion(version))
        );
    }

    let mut flags = valid.clone();
    flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&flags),
        Err(DirectLinkDecodeError::UnsupportedFlags(1))
    );

    let mut unknown_digest = valid.clone();
    unknown_digest[12] = 0xff;
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&unknown_digest),
        Err(DirectLinkDecodeError::UnknownDigestAlgorithm(0xff))
    );

    let mut zero_count = valid.clone();
    zero_count[45..47].copy_from_slice(&0_u16.to_le_bytes());
    assert!(matches!(
        DirectLinkBundleEvidenceV1::from_bytes(&zero_count),
        Err(DirectLinkDecodeError::CountOutOfRange { count: 0, .. })
    ));

    let mut excessive_count = valid.clone();
    excessive_count[45..47].copy_from_slice(&((MAX_DIRECT_LINK_BINDINGS + 1) as u16).to_le_bytes());
    assert!(matches!(
        DirectLinkBundleEvidenceV1::from_bytes(&excessive_count),
        Err(DirectLinkDecodeError::CountOutOfRange { .. })
    ));

    let mut reserved = valid.clone();
    reserved[47..49].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&reserved),
        Err(DirectLinkDecodeError::NonZeroReserved(1))
    );

    let worker_name_length = DIRECT_LINK_EVIDENCE_HEADER_BYTES + 33 + 33;
    let mut oversized_text = valid;
    oversized_text[worker_name_length..worker_name_length + 2]
        .copy_from_slice(&257_u16.to_le_bytes());
    assert!(matches!(
        DirectLinkBundleEvidenceV1::from_bytes(&oversized_text),
        Err(DirectLinkDecodeError::CountOutOfRange {
            field: "tool name",
            ..
        })
    ));

    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&vec![0; MAX_DIRECT_LINK_EVIDENCE_BYTES + 1]),
        Err(DirectLinkDecodeError::TooLarge {
            max: MAX_DIRECT_LINK_EVIDENCE_BYTES
        })
    );
}

#[test]
fn duplicate_and_noncanonical_wire_bindings_are_rejected() {
    let first = fixture(0x73, CodeObjectFormat::NativeExecutable);
    let second = fixture(0x74, CodeObjectFormat::NativeExecutable);
    let bundle = bundle_for(&[&first, &second]);
    let combined = evidence(&bundle, [&first, &second]);

    let mut duplicate_request = combined.to_bytes();
    let first_request = first.expectation.request_identity().bytes();
    let second_request = second.expectation.request_identity().bytes();
    let second_request_at = find(&duplicate_request, second_request.as_bytes());
    duplicate_request[second_request_at..second_request_at + 32]
        .copy_from_slice(first_request.as_bytes());
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&duplicate_request),
        Err(DirectLinkDecodeError::Evidence(
            DirectLinkEvidenceError::Duplicate {
                field: "request identity"
            }
        ))
    );

    let first_entry =
        evidence(&bundle, [&first]).to_bytes()[DIRECT_LINK_EVIDENCE_HEADER_BYTES..].to_vec();
    let second_entry =
        evidence(&bundle, [&second]).to_bytes()[DIRECT_LINK_EVIDENCE_HEADER_BYTES..].to_vec();
    assert_eq!(first_entry.len(), second_entry.len());
    let mut reversed = combined.to_bytes();
    let body = &reversed[DIRECT_LINK_EVIDENCE_HEADER_BYTES..];
    let reversed_body = if body.starts_with(&first_entry) {
        [second_entry, first_entry].concat()
    } else {
        [first_entry, second_entry].concat()
    };
    reversed[DIRECT_LINK_EVIDENCE_HEADER_BYTES..].copy_from_slice(&reversed_body);
    assert_eq!(
        DirectLinkBundleEvidenceV1::from_bytes(&reversed),
        Err(DirectLinkDecodeError::Evidence(
            DirectLinkEvidenceError::NonCanonicalBindingOrder
        ))
    );
}

#[test]
fn deterministic_mutations_are_panic_free_and_only_decode_canonically() {
    let item = fixture(0x75, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&item.container)).unwrap();
    let valid = evidence(&bundle, [&item]).to_bytes();

    for case in 0..128_usize {
        let mut mutated = valid.clone();
        let index = (case * 67 + 13) % mutated.len();
        mutated[index] ^= 1 << (case % 8);
        let result = catch_unwind(AssertUnwindSafe(|| {
            DirectLinkBundleEvidenceV1::from_bytes(&mutated)
        }));
        assert!(result.is_ok(), "decoder panicked for mutation {case}");
        if let Ok(decoded) = result.unwrap() {
            assert_eq!(
                decoded.to_bytes(),
                mutated,
                "accepted noncanonical mutation {case}"
            );
            assert_ne!(
                decoded.digest(DigestAlgorithm::Sha256),
                evidence(&bundle, [&item]).digest(DigestAlgorithm::Sha256)
            );
        }
    }
}

#[test]
fn valid_identity_mutation_changes_record_digest_and_cannot_match_original_policy() {
    let item = fixture(0x76, CodeObjectFormat::NativeExecutable);
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&item.container)).unwrap();
    let original = evidence(&bundle, [&item]);
    let payload = item.expectation.finalized_payload_identity();
    let changed_expectation = DirectLinkBindingExpectationV1::new(
        tagged(0xee),
        item.expectation.worker().clone(),
        item.expectation.toolchain().clone(),
        item.expectation.response_identity(),
        DirectLinkTransformationIdentityV1::new(
            item.expectation.linked_output_identity(),
            item.expectation.finalization_identity(),
            payload,
        ),
        item.expectation.ffi_contract_identity(),
    );
    let source = DirectLinkBindingSourceV1::new(&item.container, changed_expectation);
    let changed = DirectLinkBundleEvidenceV1::bind(&bundle, &[source]).unwrap();

    assert_ne!(
        original.digest(DigestAlgorithm::Sha256),
        changed.digest(DigestAlgorithm::Sha256)
    );
    assert_eq!(
        changed.validate_against(
            &bundle,
            std::slice::from_ref(&item.container),
            std::slice::from_ref(&item.expectation),
        ),
        Err(DirectLinkEvidenceError::ExpectationMismatch)
    );
}

#[test]
fn canonical_container_and_bundle_v1_encodings_remain_unchanged_by_companion_record() {
    let payload =
        CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, b"native".to_vec()).unwrap();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        vec![object_identity(
            payload.digest().bytes(),
            payload.bytes().len() as u64,
        )],
        vec![kernel_with_object_digest(
            0x7f,
            "wire_stability",
            "wire_stability.kd",
            payload.digest().bytes(),
            vec![],
        )],
    )
    .unwrap();
    let container =
        ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, vec![payload]).unwrap();
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container)).unwrap();
    let container_bytes = container.to_bytes();
    let bundle_bytes = bundle.to_bytes();
    let expectation = expectation(0xd0, container.payloads()[0].digest());
    let source = DirectLinkBindingSourceV1::new(&container, expectation);
    let _record = DirectLinkBundleEvidenceV1::bind(&bundle, &[source]).unwrap();

    assert_eq!(container.to_bytes(), container_bytes);
    assert_eq!(bundle.to_bytes(), bundle_bytes);
}
