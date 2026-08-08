use fe2o3_artifact_transaction::TargetIdentityV1;
use fe2o3_artifacts::{
    DigestAlgorithm, DigestBytes, DirectLinkContainerIdentityV1,
    DirectLinkFinalizedPayloadIdentityV1, DirectLinkLinkedOutputIdentityV1,
    DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1, IdentityText,
    MAX_IDENTITY_TEXT_BYTES, MeasuredToolIdentity, PayloadDigest,
};
use fe2o3_worker_v2_bundle::{
    BackendInvocationIdentityV1, COMPILER_TRANSACTION_EVIDENCE_MAGIC_V1,
    COMPILER_TRANSACTION_EVIDENCE_VERSION_V1, CompilerSourceClosureV1, CompilerSourceDependencyV1,
    CompilerTransactionDecodeErrorV1, CompilerTransactionEvidenceCapsuleV1,
    CompilerTransactionEvidencePartsV1, CompilerTransactionValidationErrorV1, KernelIrIdentityV1,
    MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1, MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1,
    MAX_COMPILER_TRANSACTION_FEATURES_V1, RustcInvocationIdentityV1, SemanticWitnessIdentityV1,
    SourceRootIdentityV1, WorkerV2EnvelopeIdentityV1,
};

const HEADER_BYTES: usize = 16;

fn digest(seed: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes([seed; 32]))
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).unwrap()
}

fn dependency(name: &str, seed: u8) -> CompilerSourceDependencyV1 {
    CompilerSourceDependencyV1::new(text(name), digest(seed)).unwrap()
}

fn tool(name: &str, version: &str, seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(
        text(name),
        text(version),
        digest(seed),
        digest(seed.wrapping_add(1)),
    )
}

fn parts(
    dependency_seed: u8,
    response_seed: u8,
    envelope_seed: u8,
) -> CompilerTransactionEvidencePartsV1 {
    CompilerTransactionEvidencePartsV1 {
        source_closure: CompilerSourceClosureV1::new(
            SourceRootIdentityV1::from_sha256([0x10; 32]),
            vec![
                dependency("dep-z", 0x12),
                dependency("dep-a", dependency_seed),
            ],
            vec![text("feature-z"), text("feature-a")],
        )
        .unwrap(),
        rustc_tool: tool("rustc", "1.94.0-nightly", 0x20),
        rustc_invocation: RustcInvocationIdentityV1::from_sha256([0x22; 32]),
        backend_tool: tool("rustc_codegen_fe2o3", "0.1.0", 0x30),
        backend_invocation: BackendInvocationIdentityV1::from_sha256([0x32; 32]),
        semantic_witness: SemanticWitnessIdentityV1::from_sha256([0x40; 32]),
        kernel_ir: KernelIrIdentityV1::from_sha256([0x41; 32]),
        worker_request: DirectLinkRequestIdentityV1::new(digest(0x50)),
        worker_response: DirectLinkResponseIdentityV1::new(digest(response_seed)),
        target: TargetIdentityV1::from_bytes([0x52; 32]),
        raw_hsaco: DirectLinkLinkedOutputIdentityV1::new(digest(0x60)),
        finalized_hsaco: DirectLinkFinalizedPayloadIdentityV1::new(digest(0x61)),
        artifact: DirectLinkContainerIdentityV1::new(digest(0x62)),
        envelope: WorkerV2EnvelopeIdentityV1::from_sha256([envelope_seed; 32]),
    }
}

fn capsule() -> CompilerTransactionEvidenceCapsuleV1 {
    CompilerTransactionEvidenceCapsuleV1::new(parts(0x11, 0x51, 0x63)).unwrap()
}

fn field_ranges(bytes: &[u8]) -> Vec<(u8, usize, usize)> {
    let mut result = Vec::new();
    let mut offset = HEADER_BYTES;
    while offset < bytes.len() {
        let start = offset;
        let tag = bytes[offset];
        let length = u32::from_le_bytes(bytes[offset + 1..offset + 5].try_into().unwrap()) as usize;
        offset += 5 + length;
        result.push((tag, start, offset));
    }
    result
}

fn field_range(bytes: &[u8], tag: u8) -> (usize, usize) {
    field_ranges(bytes)
        .into_iter()
        .find_map(|(actual, start, end)| (actual == tag).then_some((start, end)))
        .unwrap()
}

fn update_total_len(bytes: &mut [u8]) {
    let length = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
}

#[test]
fn deterministic_round_trip_canonicalizes_source_sets() {
    let capsule = capsule();
    let bytes = capsule.to_bytes();
    let decoded = CompilerTransactionEvidenceCapsuleV1::from_bytes(&bytes).unwrap();

    assert_eq!(decoded, capsule);
    assert_eq!(decoded.to_bytes(), bytes);
    assert_eq!(
        decoded
            .source_closure()
            .dependencies()
            .iter()
            .map(|entry| entry.name().as_str())
            .collect::<Vec<_>>(),
        ["dep-a", "dep-z"]
    );
    assert_eq!(
        decoded
            .source_closure()
            .features()
            .iter()
            .map(IdentityText::as_str)
            .collect::<Vec<_>>(),
        ["feature-a", "feature-z"]
    );
    assert_eq!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes_for_identity(&bytes, capsule.identity())
            .unwrap(),
        capsule
    );
}

#[test]
fn equivalent_source_set_permutations_have_identical_wire_bytes() {
    let first = capsule();
    let mut second_parts = parts(0x11, 0x51, 0x63);
    second_parts.source_closure = CompilerSourceClosureV1::new(
        second_parts.source_closure.root(),
        second_parts
            .source_closure
            .dependencies()
            .iter()
            .rev()
            .cloned()
            .collect(),
        second_parts
            .source_closure
            .features()
            .iter()
            .rev()
            .cloned()
            .collect(),
    )
    .unwrap();
    let second = CompilerTransactionEvidenceCapsuleV1::new(second_parts).unwrap();

    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.to_bytes(), second.to_bytes());
}

#[test]
fn wire_header_is_versioned_and_exact() {
    let bytes = capsule().to_bytes();
    assert_eq!(&bytes[..8], &COMPILER_TRANSACTION_EVIDENCE_MAGIC_V1);
    assert_eq!(
        u16::from_le_bytes(bytes[8..10].try_into().unwrap()),
        COMPILER_TRANSACTION_EVIDENCE_VERSION_V1
    );
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize,
        bytes.len()
    );
    assert!(bytes.len() <= MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1);
    assert_eq!(
        field_ranges(&bytes)
            .iter()
            .map(|(tag, _, _)| *tag)
            .collect::<Vec<_>>(),
        (1_u8..=17).collect::<Vec<_>>()
    );
}

#[test]
fn every_truncation_and_trailing_byte_is_rejected() {
    let bytes = capsule().to_bytes();
    for length in 0..bytes.len() {
        assert!(
            CompilerTransactionEvidenceCapsuleV1::from_bytes(&bytes[..length]).is_err(),
            "accepted truncated capsule of length {length}"
        );
    }

    let mut trailing = bytes;
    trailing.push(0);
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&trailing),
        Err(CompilerTransactionDecodeErrorV1::TrailingBytes)
    ));
}

#[test]
fn field_permutation_duplicate_unknown_and_omission_are_rejected() {
    let original = capsule().to_bytes();
    let ranges = field_ranges(&original);

    let (_, first_start, first_end) = ranges[0];
    let (_, second_start, second_end) = ranges[1];
    let mut permuted = original[..HEADER_BYTES].to_vec();
    permuted.extend_from_slice(&original[second_start..second_end]);
    permuted.extend_from_slice(&original[first_start..first_end]);
    permuted.extend_from_slice(&original[second_end..]);
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&permuted),
        Err(CompilerTransactionDecodeErrorV1::UnexpectedField {
            expected: 1,
            actual: 2
        })
    ));

    let mut duplicate = original.clone();
    duplicate[second_start] = 1;
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&duplicate),
        Err(CompilerTransactionDecodeErrorV1::DuplicateField(1))
    ));

    let mut unknown = original.clone();
    unknown[first_start] = 0x80;
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&unknown),
        Err(CompilerTransactionDecodeErrorV1::UnknownField(0x80))
    ));

    let (_, omitted_start, omitted_end) = ranges[7];
    let mut omitted = original;
    omitted.drain(omitted_start..omitted_end);
    update_total_len(&mut omitted);
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&omitted),
        Err(CompilerTransactionDecodeErrorV1::UnexpectedField {
            expected: 8,
            actual: 9
        })
    ));
}

#[test]
fn field_mutation_and_noncanonical_digest_encoding_fail_closed() {
    let mut mutation = capsule().to_bytes();
    let (source_start, _) = field_range(&mutation, 1);
    mutation[source_start + 6] ^= 1;
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&mutation),
        Err(CompilerTransactionDecodeErrorV1::CapsuleIdentityMismatch)
    ));

    let mut algorithm = capsule().to_bytes();
    let (source_start, _) = field_range(&algorithm, 1);
    algorithm[source_start + 5] = 0xff;
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&algorithm),
        Err(CompilerTransactionDecodeErrorV1::UnknownDigestAlgorithm(
            0xff
        ))
    ));
}

#[test]
fn noncanonical_dependency_and_feature_order_are_rejected() {
    let mut dependencies = capsule().to_bytes();
    let (start, _) = field_range(&dependencies, 2);
    let payload = start + 5;
    let first = payload + 2;
    let entry_len = 2 + "dep-a".len() + 33;
    let second = first + entry_len;
    for index in 0..entry_len {
        dependencies.swap(first + index, second + index);
    }
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&dependencies),
        Err(CompilerTransactionDecodeErrorV1::NonCanonicalDependencyOrder)
    ));

    let mut features = capsule().to_bytes();
    let (start, _) = field_range(&features, 3);
    let payload = start + 5;
    let first = payload + 2;
    let entry_len = 2 + "feature-a".len();
    let second = first + entry_len;
    for index in 0..entry_len {
        features.swap(first + index, second + index);
    }
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&features),
        Err(CompilerTransactionDecodeErrorV1::NonCanonicalFeatureOrder)
    ));
}

#[test]
fn nested_counts_strings_and_field_lengths_are_bounded_before_allocation() {
    let mut dependency_count = capsule().to_bytes();
    let (start, _) = field_range(&dependency_count, 2);
    dependency_count[start + 5..start + 7]
        .copy_from_slice(&((MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1 as u16) + 1).to_le_bytes());
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&dependency_count),
        Err(CompilerTransactionDecodeErrorV1::CountOutOfRange {
            field: "dependency",
            ..
        })
    ));

    let mut feature_count = capsule().to_bytes();
    let (start, _) = field_range(&feature_count, 3);
    feature_count[start + 5..start + 7]
        .copy_from_slice(&((MAX_COMPILER_TRANSACTION_FEATURES_V1 as u16) + 1).to_le_bytes());
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&feature_count),
        Err(CompilerTransactionDecodeErrorV1::CountOutOfRange {
            field: "feature",
            ..
        })
    ));

    let mut string_length = capsule().to_bytes();
    let (start, _) = field_range(&string_length, 2);
    string_length[start + 7..start + 9]
        .copy_from_slice(&((MAX_IDENTITY_TEXT_BYTES as u16) + 1).to_le_bytes());
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&string_length),
        Err(CompilerTransactionDecodeErrorV1::StringTooLong {
            field: "dependency name",
            ..
        })
    ));

    let mut field_length = capsule().to_bytes();
    let (start, _) = field_range(&field_length, 1);
    field_length[start + 1..start + 5].copy_from_slice(&34_u32.to_le_bytes());
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&field_length),
        Err(CompilerTransactionDecodeErrorV1::LengthOutOfRange { field: 1, .. })
    ));

    let oversized = vec![0; MAX_COMPILER_TRANSACTION_EVIDENCE_BYTES_V1 + 1];
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&oversized),
        Err(CompilerTransactionDecodeErrorV1::TooLarge { .. })
    ));
}

#[test]
fn malformed_nested_text_and_payload_trailing_bytes_are_rejected() {
    let mut invalid_utf8 = capsule().to_bytes();
    let (tool_start, _) = field_range(&invalid_utf8, 4);
    invalid_utf8[tool_start + 7] = 0xff;
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&invalid_utf8),
        Err(CompilerTransactionDecodeErrorV1::InvalidUtf8 { field: "tool name" })
    ));

    let mut invalid_text = capsule().to_bytes();
    let (tool_start, _) = field_range(&invalid_text, 4);
    invalid_text[tool_start + 7] = b' ';
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&invalid_text),
        Err(CompilerTransactionDecodeErrorV1::InvalidText { field: "tool name" })
    ));

    let mut trailing_payload = capsule().to_bytes();
    let (tool_start, tool_end) = field_range(&trailing_payload, 4);
    let tool_length = u32::from_le_bytes(
        trailing_payload[tool_start + 1..tool_start + 5]
            .try_into()
            .unwrap(),
    );
    trailing_payload.insert(tool_end, 0);
    trailing_payload[tool_start + 1..tool_start + 5]
        .copy_from_slice(&(tool_length + 1).to_le_bytes());
    update_total_len(&mut trailing_payload);
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&trailing_payload),
        Err(CompilerTransactionDecodeErrorV1::FieldTrailingBytes { field: 4 })
    ));
}

#[test]
fn constructor_rejects_duplicate_and_oversized_source_sets() {
    assert!(matches!(
        CompilerSourceClosureV1::new(
            SourceRootIdentityV1::from_sha256([1; 32]),
            vec![dependency("same", 1), dependency("same", 2)],
            vec![]
        ),
        Err(CompilerTransactionValidationErrorV1::DuplicateDependency)
    ));
    assert!(matches!(
        CompilerSourceClosureV1::new(
            SourceRootIdentityV1::from_sha256([1; 32]),
            vec![],
            vec![text("same"), text("same")]
        ),
        Err(CompilerTransactionValidationErrorV1::DuplicateFeature)
    ));

    let dependencies = (0..=MAX_COMPILER_TRANSACTION_DEPENDENCIES_V1)
        .map(|index| dependency(&format!("dep-{index:04}"), index as u8))
        .collect();
    assert!(matches!(
        CompilerSourceClosureV1::new(
            SourceRootIdentityV1::from_sha256([1; 32]),
            dependencies,
            vec![]
        ),
        Err(CompilerTransactionValidationErrorV1::TooManyDependencies { .. })
    ));

    let features = (0..=MAX_COMPILER_TRANSACTION_FEATURES_V1)
        .map(|index| text(&format!("feature-{index:04}")))
        .collect();
    assert!(matches!(
        CompilerSourceClosureV1::new(SourceRootIdentityV1::from_sha256([1; 32]), vec![], features),
        Err(CompilerTransactionValidationErrorV1::TooManyFeatures { .. })
    ));
}

#[test]
fn stale_and_substituted_valid_capsules_are_rejected_by_expected_identity() {
    let current = capsule();
    let stale_source = CompilerTransactionEvidenceCapsuleV1::new(parts(0x90, 0x51, 0x63)).unwrap();
    let substituted_response =
        CompilerTransactionEvidenceCapsuleV1::new(parts(0x11, 0x91, 0x63)).unwrap();
    let substituted_envelope =
        CompilerTransactionEvidenceCapsuleV1::new(parts(0x11, 0x51, 0x92)).unwrap();

    for substituted in [stale_source, substituted_response, substituted_envelope] {
        assert_ne!(substituted.identity(), current.identity());
        assert!(matches!(
            CompilerTransactionEvidenceCapsuleV1::from_bytes_for_identity(
                &substituted.to_bytes(),
                current.identity()
            ),
            Err(CompilerTransactionDecodeErrorV1::UnexpectedCapsuleIdentity)
        ));
    }
}

#[test]
fn source_closure_identity_binds_dependencies_and_features() {
    let original = parts(0x11, 0x51, 0x63).source_closure;
    let different_dependency = parts(0x12, 0x51, 0x63).source_closure;
    let different_features = CompilerSourceClosureV1::new(
        original.root(),
        original.dependencies().to_vec(),
        vec![text("feature-a")],
    )
    .unwrap();

    assert_ne!(original.identity(), different_dependency.identity());
    assert_ne!(original.identity(), different_features.identity());
}

#[test]
fn decoded_capsule_is_explicitly_inert() {
    let capsule = CompilerTransactionEvidenceCapsuleV1::from_bytes(&capsule().to_bytes()).unwrap();
    assert!(!capsule.authenticates_producer());
    assert!(!capsule.grants_publication_authority());
    assert!(!capsule.grants_load_authority());
    assert!(!capsule.grants_launch_authority());
}

#[test]
fn malformed_header_values_are_rejected() {
    let mut magic = capsule().to_bytes();
    magic[0] ^= 1;
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&magic),
        Err(CompilerTransactionDecodeErrorV1::InvalidMagic)
    ));

    let mut version = capsule().to_bytes();
    version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&version),
        Err(CompilerTransactionDecodeErrorV1::UnknownVersion(2))
    ));

    let mut flags = capsule().to_bytes();
    flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        CompilerTransactionEvidenceCapsuleV1::from_bytes(&flags),
        Err(CompilerTransactionDecodeErrorV1::UnsupportedFlags(1))
    ));
}
