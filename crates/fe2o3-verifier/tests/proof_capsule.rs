use fe2o3_verifier::{
    AxiomPolicy, CorrelationId, Digest, MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1,
    MAX_PROOF_CAPSULE_BYTES_V1, MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1, MeasuredToolIdentity,
    ProcessLocalProofCapsuleDuplicateDetectorV1, ProofCapsuleBuildErrorV1,
    ProofCapsuleContextErrorV1, ProofCapsuleDecodeErrorV1, ProofCapsuleDependencyV1,
    ProofCapsuleExecutionV1, ProofCapsuleExpectationV1, ProofCapsuleFreshnessExpectationV1,
    ProofCapsuleFreshnessIdentityV1, ProofCapsuleIdentityFieldV1, ProofCapsulePayloadIdentityV1,
    ProofCapsulePolicyV1, ProofCapsuleResultV1, ProofCapsuleTargetV1, ProofCapsuleV1, ProofOutcome,
    ProofProperty, ProofTargetIdentity, Text, TrustedItem, VerificationModelIdentity,
};

const FIRST_FIELD_OFFSET: usize = 16;
const TOTAL_LEN_OFFSET: usize = 12;
const SECOND_FIELD_OFFSET: usize = 434;
const DEPENDENCY_COUNT_OFFSET: usize = SECOND_FIELD_OFFSET + 2;
const DEPENDENCY_RECORDS_OFFSET: usize = 438;
const DEPENDENCY_RECORD_BYTES: usize = 39;
const FEATURE_RECORDS_OFFSET: usize = 520;
const FEATURE_RECORD_BYTES: usize = 8;

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn result_payload(seed: u8) -> ProofCapsulePayloadIdentityV1 {
    ProofCapsulePayloadIdentityV1::sealed_result(u64::from(seed) + 100, digest(seed)).unwrap()
}

fn finalized_payload_identity(seed: u8) -> Digest {
    digest(seed)
}

fn proof_target() -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(1),
        instance_digest: digest(2),
        source_tree_digest: digest(3),
        crate_graph_digest: digest(4),
        executable_digest: digest(5),
        environment_digest: digest(6),
        artifact_selection_digest: digest(7),
        artifact_contract_digest: digest(8),
        memory_contract_digest: digest(9),
        effects_contract_digest: digest(10),
        type_layout_digest: digest(11),
        capability_semantics_digest: digest(12),
        functional_specification_digest: digest(13),
    }
}

fn target_with(
    target: ProofTargetIdentity,
    machine_effect_seed: u8,
    artifact_seed: u8,
) -> ProofCapsuleTargetV1 {
    target_with_finalized_identity(
        target,
        machine_effect_seed,
        finalized_payload_identity(60),
        artifact_seed,
    )
}

fn target_with_finalized_identity(
    target: ProofTargetIdentity,
    machine_effect_seed: u8,
    finalized_payload_identity: Digest,
    artifact_seed: u8,
) -> ProofCapsuleTargetV1 {
    ProofCapsuleTargetV1::new(
        target,
        vec![
            ProofCapsuleDependencyV1::new("z_dep", digest(52)).unwrap(),
            ProofCapsuleDependencyV1::new("a_dep", digest(51)).unwrap(),
        ],
        vec![
            Text::identifier("feature", "feat_z").unwrap(),
            Text::identifier("feature", "feat_a").unwrap(),
        ],
        digest(57),
        digest(58),
        digest(machine_effect_seed),
        finalized_payload_identity,
        digest(artifact_seed),
    )
    .unwrap()
}

fn target() -> ProofCapsuleTargetV1 {
    target_with(proof_target(), 59, 61)
}

fn tool(name: &str, seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, "1.2.3", digest(seed), digest(seed + 1)).unwrap()
}

fn policy() -> ProofCapsulePolicyV1 {
    let axiom = TrustedItem::new("gpu_integer_model", digest(36)).unwrap();
    ProofCapsulePolicyV1::new(
        VerificationModelIdentity::new("gpu-model-v1", digest(30)).unwrap(),
        tool("claimed-verus", 31),
        tool("z3", 33),
        AxiomPolicy::allow_list(vec![axiom.clone()]).unwrap(),
        vec![axiom],
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
    )
    .unwrap()
}

fn freshness_with(
    generation: u64,
    state_seed: u8,
    persistent_seed: u8,
) -> ProofCapsuleFreshnessIdentityV1 {
    ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(40),
        digest(45),
        digest(46),
        digest(47),
        digest(48),
        digest(49),
        generation,
        digest(state_seed),
        digest(persistent_seed),
    )
    .unwrap()
}

fn freshness() -> ProofCapsuleFreshnessIdentityV1 {
    freshness_with(7, 50, 51)
}

fn execution_with(
    correlation_seed: u8,
    freshness: Option<ProofCapsuleFreshnessIdentityV1>,
) -> ProofCapsuleExecutionV1 {
    ProofCapsuleExecutionV1::new_inert(
        CorrelationId::from_bytes([correlation_seed; 16]),
        digest(41),
        digest(42),
        digest(43),
        digest(45),
        digest(46),
        result_payload(47),
        freshness,
    )
    .unwrap()
}

fn reported_proved_result() -> ProofCapsuleResultV1 {
    ProofCapsuleResultV1::new(
        ProofOutcome::Proved,
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
    )
    .unwrap()
}

fn capsule_with(
    target: ProofCapsuleTargetV1,
    correlation_seed: u8,
    freshness: ProofCapsuleFreshnessIdentityV1,
) -> ProofCapsuleV1 {
    ProofCapsuleV1::new_inert(
        target,
        policy(),
        execution_with(correlation_seed, Some(freshness)),
        reported_proved_result(),
    )
    .unwrap()
}

fn capsule() -> ProofCapsuleV1 {
    capsule_with(target(), 44, freshness())
}

fn expectation(capsule: &ProofCapsuleV1) -> ProofCapsuleExpectationV1 {
    ProofCapsuleExpectationV1::new(
        capsule.identity(),
        capsule.target().artifact_identity(),
        capsule
            .execution()
            .freshness()
            .map(ProofCapsuleFreshnessExpectationV1::new),
    )
    .unwrap()
}

fn set_total_len(bytes: &mut [u8]) {
    let len = u32::try_from(bytes.len()).unwrap();
    bytes[TOTAL_LEN_OFFSET..TOTAL_LEN_OFFSET + 4].copy_from_slice(&len.to_le_bytes());
}

fn replace_unique_u64(bytes: &mut [u8], old: u64, new: u64) {
    let old = old.to_le_bytes();
    let positions = bytes
        .windows(old.len())
        .enumerate()
        .filter_map(|(index, window)| (window == old).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(positions.len(), 1);
    let offset = positions[0];
    bytes[offset..offset + old.len()].copy_from_slice(&new.to_le_bytes());
}

#[test]
fn canonical_round_trip_binds_every_axis_without_authority() {
    let capsule = capsule();
    let bytes = capsule.to_bytes();
    let decoded = ProofCapsuleV1::from_bytes(&bytes).unwrap();

    assert_eq!(decoded, capsule);
    assert_eq!(decoded.to_bytes(), bytes);
    assert_eq!(decoded.target().proof_target(), proof_target());
    assert_eq!(decoded.target().dependencies()[0].name().as_str(), "a_dep");
    assert_eq!(decoded.target().dependencies()[1].name().as_str(), "z_dep");
    assert_eq!(decoded.target().features()[0].as_str(), "feat_a");
    assert_eq!(decoded.target().features()[1].as_str(), "feat_z");
    assert_eq!(decoded.target().abi_identity(), digest(57));
    assert_eq!(decoded.target().effects_identity(), digest(10));
    assert_eq!(decoded.target().launch_identity(), digest(58));
    assert_eq!(
        decoded.target().machine_effect_evidence_identity(),
        digest(59)
    );
    assert_eq!(
        decoded.target().finalized_payload_identity(),
        finalized_payload_identity(60)
    );
    assert_eq!(decoded.target().artifact_identity(), digest(61));
    assert_eq!(decoded.policy().model().version().as_str(), "gpu-model-v1");
    assert_eq!(
        decoded.policy().claimed_verifier().name().as_str(),
        "claimed-verus"
    );
    assert_eq!(decoded.policy().claimed_solver().name().as_str(), "z3");
    assert_eq!(decoded.policy().approved_axioms().allowed().len(), 1);
    assert_eq!(decoded.policy().requested_axioms().len(), 1);
    assert_eq!(
        decoded.policy().requested_properties(),
        &[ProofProperty::Bounds, ProofProperty::RaceFreedom]
    );
    assert_eq!(
        decoded.result().reported_properties(),
        &[ProofProperty::Bounds, ProofProperty::RaceFreedom]
    );
    assert_eq!(decoded.execution().sealed_result(), result_payload(47));
    assert_eq!(decoded.execution().freshness(), Some(freshness()));
    assert_eq!(
        decoded
            .execution()
            .freshness()
            .unwrap()
            .previous_ledger_state_identity(),
        digest(49)
    );
    assert!(decoded.is_pre_envelope_evidence());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert!(!decoded.proves_compiler_refinement());
}

#[test]
fn failed_and_timed_out_capsules_are_honest_non_proofs() {
    for outcome in [ProofOutcome::Failed, ProofOutcome::TimedOut] {
        assert_eq!(
            ProofCapsuleResultV1::new(outcome, vec![ProofProperty::Bounds]),
            Err(ProofCapsuleBuildErrorV1::ClaimsOnIncompleteProof)
        );
        let capsule = ProofCapsuleV1::new_inert(
            target(),
            policy(),
            execution_with(44, None),
            ProofCapsuleResultV1::new(outcome, vec![]).unwrap(),
        )
        .unwrap();
        let decoded = ProofCapsuleV1::from_bytes(&capsule.to_bytes()).unwrap();
        assert_eq!(decoded.result().outcome(), outcome);
        assert!(decoded.result().reported_properties().is_empty());
        assert_eq!(decoded.execution().freshness(), None);
    }
}

#[test]
fn reported_proved_capsules_require_exact_claims_and_persistent_freshness() {
    let partial =
        ProofCapsuleResultV1::new(ProofOutcome::Proved, vec![ProofProperty::Bounds]).unwrap();
    assert_eq!(
        ProofCapsuleV1::new_inert(
            target(),
            policy(),
            execution_with(44, Some(freshness())),
            partial,
        ),
        Err(ProofCapsuleBuildErrorV1::IncompleteProof)
    );
    assert_eq!(
        ProofCapsuleV1::new_inert(
            target(),
            policy(),
            execution_with(44, None),
            reported_proved_result(),
        ),
        Err(ProofCapsuleBuildErrorV1::MissingPersistentFreshness)
    );
    assert_eq!(
        ProofCapsuleV1::new_inert(
            target(),
            policy(),
            execution_with(44, Some(freshness())),
            ProofCapsuleResultV1::new(ProofOutcome::Failed, vec![]).unwrap(),
        ),
        Err(ProofCapsuleBuildErrorV1::UnexpectedPersistentFreshness)
    );
}

#[test]
fn freshness_must_match_the_exact_sealed_execution() {
    let wrong = ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(40),
        digest(99),
        digest(46),
        digest(47),
        digest(48),
        digest(49),
        7,
        digest(50),
        digest(51),
    )
    .unwrap();
    assert_eq!(
        ProofCapsuleExecutionV1::new_inert(
            CorrelationId::from_bytes([44; 16]),
            digest(41),
            digest(42),
            digest(43),
            digest(45),
            digest(46),
            result_payload(47),
            Some(wrong),
        ),
        Err(ProofCapsuleBuildErrorV1::FreshnessMismatch { field: "challenge" })
    );
}

#[test]
fn parser_rejects_truncation_trailing_oversized_and_mutated_bytes() {
    let bytes = capsule().to_bytes();
    for length in 0..bytes.len() {
        assert!(ProofCapsuleV1::from_bytes(&bytes[..length]).is_err());
    }

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        ProofCapsuleV1::from_bytes(&trailing),
        Err(ProofCapsuleDecodeErrorV1::TrailingBytes)
    );

    assert_eq!(
        ProofCapsuleV1::from_bytes(&vec![0; MAX_PROOF_CAPSULE_BYTES_V1 + 1]),
        Err(ProofCapsuleDecodeErrorV1::TooLarge {
            max: MAX_PROOF_CAPSULE_BYTES_V1
        })
    );

    let mut mutated = bytes;
    mutated[FIRST_FIELD_OFFSET + 4] ^= 1;
    assert_eq!(
        ProofCapsuleV1::from_bytes(&mutated),
        Err(ProofCapsuleDecodeErrorV1::IdentityMismatch)
    );
}

#[test]
fn sealed_result_lengths_are_bounded_in_constructors_and_decoder() {
    assert_eq!(
        ProofCapsulePayloadIdentityV1::sealed_result(u64::MAX, digest(90)),
        Err(ProofCapsuleBuildErrorV1::PayloadLengthOutOfRange {
            field: "sealed proof result",
            value: u64::MAX,
            max: MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1,
        })
    );
    let capsule = capsule();
    let mut oversized_result = capsule.to_bytes();
    replace_unique_u64(
        &mut oversized_result,
        capsule.execution().sealed_result().byte_len(),
        u64::MAX,
    );
    assert_eq!(
        ProofCapsuleV1::from_bytes(&oversized_result),
        Err(ProofCapsuleDecodeErrorV1::Build(
            ProofCapsuleBuildErrorV1::PayloadLengthOutOfRange {
                field: "sealed proof result",
                value: u64::MAX,
                max: MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1,
            }
        ))
    );
}

#[test]
fn finalized_occurrence_length_is_deferred_to_the_byte_bearing_join() {
    let finalized_digest = finalized_payload_identity(60);
    let first_occurrence = (4096_u64, finalized_digest);
    let substituted_length_occurrence = (8192_u64, finalized_digest);

    let first = target_with_finalized_identity(proof_target(), 59, first_occurrence.1, 61);
    let substituted_length =
        target_with_finalized_identity(proof_target(), 59, substituted_length_occurrence.1, 61);
    assert_ne!(first_occurrence.0, substituted_length_occurrence.0);
    assert_eq!(first_occurrence.1, substituted_length_occurrence.1);
    assert_eq!(first, substituted_length);
    assert_eq!(first.finalized_payload_identity(), finalized_digest);
    let first_capsule = capsule_with(first, 44, freshness());
    let substituted_length_capsule = capsule_with(substituted_length, 44, freshness());
    assert_eq!(
        first_capsule.to_bytes(),
        substituted_length_capsule.to_bytes()
    );
}

#[test]
fn decoder_rejects_tiny_records_and_max_counts_before_allocation() {
    let bytes = capsule().to_bytes();

    let mut tiny = bytes[..FIRST_FIELD_OFFSET + 2].to_vec();
    set_total_len(&mut tiny);
    assert_eq!(
        ProofCapsuleV1::from_bytes(&tiny),
        Err(ProofCapsuleDecodeErrorV1::Truncated)
    );

    let mut max_count = bytes[..DEPENDENCY_RECORDS_OFFSET].to_vec();
    max_count[DEPENDENCY_COUNT_OFFSET..DEPENDENCY_COUNT_OFFSET + 2]
        .copy_from_slice(&128_u16.to_le_bytes());
    set_total_len(&mut max_count);
    assert_eq!(
        ProofCapsuleV1::from_bytes(&max_count),
        Err(ProofCapsuleDecodeErrorV1::TruncatedCollection {
            field: "dependencies",
            minimum: 128 * 35,
            remaining: 0,
        })
    );
}

#[test]
fn parser_rejects_reordered_duplicate_and_unknown_fields() {
    let bytes = capsule().to_bytes();

    let mut reordered = bytes.clone();
    reordered[FIRST_FIELD_OFFSET..FIRST_FIELD_OFFSET + 2].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        ProofCapsuleV1::from_bytes(&reordered),
        Err(ProofCapsuleDecodeErrorV1::NonCanonicalFieldOrder {
            expected: 1,
            actual: 2,
        })
    );

    let mut duplicate = bytes.clone();
    duplicate[SECOND_FIELD_OFFSET..SECOND_FIELD_OFFSET + 2].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        ProofCapsuleV1::from_bytes(&duplicate),
        Err(ProofCapsuleDecodeErrorV1::DuplicateField(1))
    );

    let mut unknown = bytes;
    unknown[FIRST_FIELD_OFFSET..FIRST_FIELD_OFFSET + 2].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        ProofCapsuleV1::from_bytes(&unknown),
        Err(ProofCapsuleDecodeErrorV1::UnknownField(99))
    );
}

#[test]
fn parser_rejects_reordered_and_duplicate_dependency_or_feature_entries() {
    let bytes = capsule().to_bytes();

    let mut dependencies = bytes.clone();
    let first = dependencies
        [DEPENDENCY_RECORDS_OFFSET..DEPENDENCY_RECORDS_OFFSET + DEPENDENCY_RECORD_BYTES]
        .to_vec();
    let second = dependencies[DEPENDENCY_RECORDS_OFFSET + DEPENDENCY_RECORD_BYTES
        ..DEPENDENCY_RECORDS_OFFSET + 2 * DEPENDENCY_RECORD_BYTES]
        .to_vec();
    dependencies[DEPENDENCY_RECORDS_OFFSET..DEPENDENCY_RECORDS_OFFSET + DEPENDENCY_RECORD_BYTES]
        .copy_from_slice(&second);
    dependencies[DEPENDENCY_RECORDS_OFFSET + DEPENDENCY_RECORD_BYTES
        ..DEPENDENCY_RECORDS_OFFSET + 2 * DEPENDENCY_RECORD_BYTES]
        .copy_from_slice(&first);
    assert_eq!(
        ProofCapsuleV1::from_bytes(&dependencies),
        Err(ProofCapsuleDecodeErrorV1::NonCanonicalCollection {
            field: "dependencies"
        })
    );

    let mut duplicate = bytes.clone();
    let first_name =
        duplicate[DEPENDENCY_RECORDS_OFFSET + 2..DEPENDENCY_RECORDS_OFFSET + 7].to_vec();
    duplicate[DEPENDENCY_RECORDS_OFFSET + DEPENDENCY_RECORD_BYTES + 2
        ..DEPENDENCY_RECORDS_OFFSET + DEPENDENCY_RECORD_BYTES + 7]
        .copy_from_slice(&first_name);
    assert_eq!(
        ProofCapsuleV1::from_bytes(&duplicate),
        Err(ProofCapsuleDecodeErrorV1::DuplicateItem {
            field: "dependency name"
        })
    );

    let mut features = bytes;
    let first =
        features[FEATURE_RECORDS_OFFSET..FEATURE_RECORDS_OFFSET + FEATURE_RECORD_BYTES].to_vec();
    let second = features[FEATURE_RECORDS_OFFSET + FEATURE_RECORD_BYTES
        ..FEATURE_RECORDS_OFFSET + 2 * FEATURE_RECORD_BYTES]
        .to_vec();
    features[FEATURE_RECORDS_OFFSET..FEATURE_RECORDS_OFFSET + FEATURE_RECORD_BYTES]
        .copy_from_slice(&second);
    features[FEATURE_RECORDS_OFFSET + FEATURE_RECORD_BYTES
        ..FEATURE_RECORDS_OFFSET + 2 * FEATURE_RECORD_BYTES]
        .copy_from_slice(&first);
    assert_eq!(
        ProofCapsuleV1::from_bytes(&features),
        Err(ProofCapsuleDecodeErrorV1::NonCanonicalCollection { field: "features" })
    );
}

#[test]
fn expected_context_rejects_stale_and_forked_freshness() {
    let stale_freshness = freshness_with(6, 70, 71);
    let stale = capsule_with(target(), 44, stale_freshness);
    let expected_freshness = freshness();
    let expected = ProofCapsuleExpectationV1::new(
        stale.identity(),
        stale.target().artifact_identity(),
        Some(ProofCapsuleFreshnessExpectationV1::new(expected_freshness)),
    )
    .unwrap();
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::new()
            .parse_validate_and_record(&stale.to_bytes(), expected),
        Err(ProofCapsuleContextErrorV1::StaleLedgerGeneration {
            expected: 7,
            actual: 6,
        })
    );

    let fork = freshness_with(7, 70, 71);
    let forked = capsule_with(target(), 44, fork);
    let expected = ProofCapsuleExpectationV1::new(
        forked.identity(),
        forked.target().artifact_identity(),
        Some(ProofCapsuleFreshnessExpectationV1::new(expected_freshness)),
    )
    .unwrap();
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::new()
            .parse_validate_and_record(&forked.to_bytes(), expected),
        Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::LedgerState,
        })
    );

    let wrong_previous = ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(40),
        digest(45),
        digest(46),
        digest(47),
        digest(48),
        digest(99),
        7,
        digest(50),
        digest(51),
    )
    .unwrap();
    let wrong_previous_capsule = capsule_with(target(), 44, wrong_previous);
    let expected = ProofCapsuleExpectationV1::new(
        wrong_previous_capsule.identity(),
        wrong_previous_capsule.target().artifact_identity(),
        Some(ProofCapsuleFreshnessExpectationV1::new(expected_freshness)),
    )
    .unwrap();
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::new()
            .parse_validate_and_record(&wrong_previous_capsule.to_bytes(), expected,),
        Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::PreviousLedgerState,
        })
    );
}

#[test]
fn expected_context_rejects_artifact_and_recomputed_capsule_substitution() {
    let expected_capsule = capsule();
    let expected = expectation(&expected_capsule);

    let artifact = capsule_with(target_with(proof_target(), 59, 80), 44, freshness());
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::new()
            .parse_validate_and_record(&artifact.to_bytes(), expected),
        Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::Artifact,
        })
    );

    let source = ProofTargetIdentity {
        source_tree_digest: digest(80),
        ..proof_target()
    };
    let substituted = capsule_with(target_with(source, 59, 61), 44, freshness());
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::new()
            .parse_validate_and_record(&substituted.to_bytes(), expected),
        Err(ProofCapsuleContextErrorV1::IdentitySubstitution {
            field: ProofCapsuleIdentityFieldV1::Capsule,
        })
    );
}

#[test]
fn process_local_detector_only_records_duplicates() {
    let original = capsule();
    let expected = expectation(&original);
    let mut detector = ProcessLocalProofCapsuleDuplicateDetectorV1::new();
    assert_eq!(
        detector
            .parse_validate_and_record(&original.to_bytes(), expected)
            .unwrap(),
        original
    );
    assert_eq!(detector.recorded_count(), 1);
    assert_eq!(
        detector.parse_validate_and_record(&original.to_bytes(), expected),
        Err(ProofCapsuleContextErrorV1::CapsuleDuplicate)
    );

    let same_execution = capsule_with(target_with(proof_target(), 80, 61), 44, freshness());
    assert_eq!(
        detector
            .parse_validate_and_record(&same_execution.to_bytes(), expectation(&same_execution),),
        Err(ProofCapsuleContextErrorV1::ExecutionDuplicate)
    );

    let rebound_freshness = ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(83),
        digest(84),
        digest(85),
        digest(86),
        digest(48),
        digest(49),
        8,
        digest(87),
        digest(51),
    )
    .unwrap();
    let rebound_execution = ProofCapsuleExecutionV1::new_inert(
        CorrelationId::from_bytes([82; 16]),
        digest(88),
        digest(89),
        digest(90),
        digest(84),
        digest(85),
        ProofCapsulePayloadIdentityV1::sealed_result(200, digest(86)).unwrap(),
        Some(rebound_freshness),
    )
    .unwrap();
    let same_persistent_binding = ProofCapsuleV1::new_inert(
        target_with(proof_target(), 81, 61),
        policy(),
        rebound_execution,
        reported_proved_result(),
    )
    .unwrap();
    assert_eq!(
        detector.parse_validate_and_record(
            &same_persistent_binding.to_bytes(),
            expectation(&same_persistent_binding),
        ),
        Err(ProofCapsuleContextErrorV1::PersistentProofDuplicate)
    );
}

#[test]
fn process_local_detector_is_hard_bounded_and_fails_closed_when_full() {
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::new().max_records(),
        MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1
    );
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::default().max_records(),
        MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1
    );
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::with_max_records(0).unwrap_err(),
        ProofCapsuleContextErrorV1::DetectorCapacityOutOfRange {
            value: 0,
            max: MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1,
        }
    );
    assert_eq!(
        ProcessLocalProofCapsuleDuplicateDetectorV1::with_max_records(
            MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1 + 1,
        )
        .unwrap_err(),
        ProofCapsuleContextErrorV1::DetectorCapacityOutOfRange {
            value: MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1 + 1,
            max: MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1,
        }
    );

    let second_freshness = ProofCapsuleFreshnessIdentityV1::new_inert(
        digest(80),
        digest(81),
        digest(82),
        digest(83),
        digest(84),
        digest(85),
        8,
        digest(86),
        digest(87),
    )
    .unwrap();
    let second_execution = ProofCapsuleExecutionV1::new_inert(
        CorrelationId::from_bytes([80; 16]),
        digest(88),
        digest(89),
        digest(90),
        digest(81),
        digest(82),
        ProofCapsulePayloadIdentityV1::sealed_result(200, digest(83)).unwrap(),
        Some(second_freshness),
    )
    .unwrap();
    let second = ProofCapsuleV1::new_inert(
        target_with(proof_target(), 81, 82),
        policy(),
        second_execution,
        reported_proved_result(),
    )
    .unwrap();

    let first = capsule();
    let mut detector = ProcessLocalProofCapsuleDuplicateDetectorV1::with_max_records(1).unwrap();
    detector
        .parse_validate_and_record(&first.to_bytes(), expectation(&first))
        .unwrap();
    assert_eq!(detector.recorded_count(), 1);
    assert_eq!(
        detector.parse_validate_and_record(&second.to_bytes(), expectation(&second)),
        Err(ProofCapsuleContextErrorV1::DetectorCapacityReached { capacity: 1 })
    );
    assert_eq!(detector.recorded_count(), 1);
}

#[test]
fn constructors_reject_duplicate_and_oversized_collections() {
    let zero_axiom = TrustedItem::new("zero_axiom", Digest::from_bytes([0; 32])).unwrap();
    assert_eq!(
        ProofCapsulePolicyV1::new(
            VerificationModelIdentity::new("gpu-model-v1", digest(30)).unwrap(),
            tool("claimed-verus", 31),
            tool("z3", 33),
            AxiomPolicy::allow_list(vec![zero_axiom]).unwrap(),
            vec![],
            vec![ProofProperty::Bounds],
        ),
        Err(ProofCapsuleBuildErrorV1::ZeroIdentity {
            field: "approved axiom contract identity"
        })
    );

    assert_eq!(
        ProofCapsuleTargetV1::new(
            proof_target(),
            vec![
                ProofCapsuleDependencyV1::new("dep", digest(51)).unwrap(),
                ProofCapsuleDependencyV1::new("dep", digest(52)).unwrap(),
            ],
            vec![],
            digest(57),
            digest(58),
            digest(59),
            finalized_payload_identity(60),
            digest(61),
        ),
        Err(ProofCapsuleBuildErrorV1::DuplicateItem {
            field: "dependency name"
        })
    );

    let dependencies = (0..=128)
        .map(|index| ProofCapsuleDependencyV1::new(format!("dep_{index:03}"), digest(51)).unwrap())
        .collect();
    assert_eq!(
        ProofCapsuleTargetV1::new(
            proof_target(),
            dependencies,
            vec![],
            digest(57),
            digest(58),
            digest(59),
            finalized_payload_identity(60),
            digest(61),
        ),
        Err(ProofCapsuleBuildErrorV1::TooManyItems {
            field: "dependencies",
            max: 128,
        })
    );
}
