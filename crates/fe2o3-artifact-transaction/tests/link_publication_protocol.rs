use fe2o3_artifact_transaction::{
    AtomicPublicationIdentityV1, BuildAttempt, CanonicalLinkRequestIdentityV1,
    FinalizationIdentityV1, FinalizedOutputIdentityV1, IdentityKindV1, InvalidationReasonV1,
    KernelSetIdentityV1, LinkPublicationCatalogV1, LinkPublicationCodecError,
    LinkPublicationPhaseV1, LinkPublicationRecordV1, LinkPublicationScopeV1,
    LinkPublicationStateV1, LinkedOutputIdentityV1, PackageIdentityV1, PinnedWorkerIdentityV1,
    PublicationOutcomeV1, RecoveryOutcomeV1, TargetIdentityV1, ValidatedResponseIdentityV1,
};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
struct Identities {
    request: CanonicalLinkRequestIdentityV1,
    worker: PinnedWorkerIdentityV1,
    response: ValidatedResponseIdentityV1,
    linked_output: LinkedOutputIdentityV1,
    finalization: FinalizationIdentityV1,
    finalized_output: FinalizedOutputIdentityV1,
    publication: AtomicPublicationIdentityV1,
}

fn identities(seed: u8) -> Identities {
    Identities {
        request: CanonicalLinkRequestIdentityV1::from_bytes([seed; 32]),
        worker: PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
        response: ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
        linked_output: LinkedOutputIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
        finalization: FinalizationIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
        finalized_output: FinalizedOutputIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
        publication: AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
    }
}

fn attempt(generation: u64, discriminator: u8) -> BuildAttempt {
    let session = format!("{discriminator:02x}").repeat(16);
    let invocation = format!("{:02x}", discriminator.wrapping_add(64)).repeat(32);
    BuildAttempt::from_env_value(&format!("{generation}:{session}:{invocation}")).unwrap()
}

fn scope(discriminator: u8) -> LinkPublicationScopeV1 {
    LinkPublicationScopeV1::new(
        PackageIdentityV1::from_bytes([discriminator; 32]),
        KernelSetIdentityV1::from_bytes([discriminator.wrapping_add(1); 32]),
        TargetIdentityV1::from_bytes([discriminator.wrapping_add(2); 32]),
    )
}

fn advance_to_finalized(
    record: &mut LinkPublicationRecordV1,
    catalog: &LinkPublicationCatalogV1,
    attempt: BuildAttempt,
    ids: Identities,
) {
    record
        .record_pinned_worker(catalog, attempt, ids.request, ids.worker)
        .unwrap();
    record
        .record_validated_response(
            catalog,
            attempt,
            ids.request,
            ids.worker,
            ids.response,
            ids.linked_output,
        )
        .unwrap();
    record
        .record_finalization(
            catalog,
            attempt,
            ids.response,
            ids.linked_output,
            ids.finalization,
            ids.finalized_output,
        )
        .unwrap();
}

fn publish(
    catalog: &mut LinkPublicationCatalogV1,
    attempt: BuildAttempt,
    scope: LinkPublicationScopeV1,
    ids: Identities,
) -> LinkPublicationRecordV1 {
    let mut record = catalog.begin(attempt, scope, ids.request).unwrap();
    advance_to_finalized(&mut record, catalog, attempt, ids);
    assert_eq!(
        record.publish(
            catalog,
            attempt,
            ids.finalization,
            ids.finalized_output,
            ids.publication,
        ),
        Ok(PublicationOutcomeV1::Published)
    );
    record
}

#[test]
fn every_ordered_state_has_a_canonical_restart_record() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let attempt = attempt(1, 1);
    let ids = identities(10);
    let mut record = catalog.begin(attempt, scope(1), ids.request).unwrap();
    let mut records = vec![record.clone()];

    record
        .record_pinned_worker(&catalog, attempt, ids.request, ids.worker)
        .unwrap();
    records.push(record.clone());
    record
        .record_validated_response(
            &catalog,
            attempt,
            ids.request,
            ids.worker,
            ids.response,
            ids.linked_output,
        )
        .unwrap();
    records.push(record.clone());
    record
        .record_finalization(
            &catalog,
            attempt,
            ids.response,
            ids.linked_output,
            ids.finalization,
            ids.finalized_output,
        )
        .unwrap();
    records.push(record.clone());
    record
        .publish(
            &mut catalog,
            attempt,
            ids.finalization,
            ids.finalized_output,
            ids.publication,
        )
        .unwrap();
    records.push(record);

    for expected in records {
        let encoded = expected.encode_canonical().unwrap();
        assert_eq!(
            LinkPublicationRecordV1::decode_canonical(&encoded),
            Ok(expected)
        );
    }
}

#[test]
fn active_and_invalidated_phase_records_have_a_stable_golden_transcript() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let attempt = attempt(17, 17);
    let scope = scope(17);
    let ids = identities(180);
    let mut record = catalog.begin(attempt, scope, ids.request).unwrap();
    let mut active = vec![record.clone()];
    record
        .record_pinned_worker(&catalog, attempt, ids.request, ids.worker)
        .unwrap();
    active.push(record.clone());
    record
        .record_validated_response(
            &catalog,
            attempt,
            ids.request,
            ids.worker,
            ids.response,
            ids.linked_output,
        )
        .unwrap();
    active.push(record.clone());
    record
        .record_finalization(
            &catalog,
            attempt,
            ids.response,
            ids.linked_output,
            ids.finalization,
            ids.finalized_output,
        )
        .unwrap();
    active.push(record.clone());
    let active_catalog = catalog.clone();
    record
        .publish(
            &mut catalog,
            attempt,
            ids.finalization,
            ids.finalized_output,
            ids.publication,
        )
        .unwrap();
    active.push(record.clone());

    let mut invalidated = Vec::new();
    for mut record in active.iter().take(4).cloned() {
        let mut phase_catalog = active_catalog.clone();
        record
            .invalidate(
                &mut phase_catalog,
                attempt,
                InvalidationReasonV1::ExplicitFailure,
            )
            .unwrap();
        invalidated.push(record);
    }
    let mut published_without_catalog = record;
    assert_eq!(
        published_without_catalog.recover(&mut LinkPublicationCatalogV1::default()),
        Ok(RecoveryOutcomeV1::InvalidatedStaleAttempt)
    );
    invalidated.push(published_without_catalog);

    let records = active.iter().chain(&invalidated).collect::<Vec<_>>();
    let encoded = records
        .iter()
        .map(|record| record.encode_canonical().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        encoded.iter().map(Vec::len).collect::<Vec<_>>(),
        vec![214, 247, 313, 379, 412, 216, 249, 315, 381, 414]
    );
    let mut transcript = b"fe2o3.link-publication.golden-phases.v1\0".to_vec();
    for bytes in encoded {
        transcript.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        transcript.extend_from_slice(&bytes);
    }
    let digest: [u8; 32] = Sha256::digest(transcript).into();
    assert_eq!(
        digest,
        [
            0x16, 0xce, 0x37, 0x36, 0xd8, 0x2e, 0x91, 0x30, 0xbb, 0x32, 0xe4, 0x93, 0x3e, 0x68,
            0x6f, 0x16, 0xd6, 0x16, 0x33, 0xd0, 0x70, 0x73, 0x74, 0x59, 0x8a, 0x3d, 0xdd, 0xd1,
            0x03, 0x0a, 0xa4, 0x32,
        ]
    );
}

#[test]
fn reordered_transitions_and_parent_mutations_fail_without_state_changes() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let attempt = attempt(1, 2);
    let ids = identities(20);
    let mut record = catalog.begin(attempt, scope(2), ids.request).unwrap();
    let original = record.clone();

    assert!(matches!(
        record.record_validated_response(
            &catalog,
            attempt,
            ids.request,
            ids.worker,
            ids.response,
            ids.linked_output,
        ),
        Err(LinkPublicationCodecError::InvalidTransition { .. })
    ));
    assert_eq!(record, original);

    assert_eq!(
        record.record_pinned_worker(
            &catalog,
            attempt,
            CanonicalLinkRequestIdentityV1::from_bytes([0xee; 32]),
            ids.worker,
        ),
        Err(LinkPublicationCodecError::IdentityMismatch {
            kind: IdentityKindV1::Request,
        })
    );
    assert_eq!(record, original);

    record
        .record_pinned_worker(&catalog, attempt, ids.request, ids.worker)
        .unwrap();
    let pinned = record.clone();
    assert_eq!(
        record.record_validated_response(
            &catalog,
            attempt,
            ids.request,
            PinnedWorkerIdentityV1::from_bytes([0xef; 32]),
            ids.response,
            ids.linked_output,
        ),
        Err(LinkPublicationCodecError::IdentityMismatch {
            kind: IdentityKindV1::Worker,
        })
    );
    assert_eq!(record, pinned);
}

#[test]
fn stale_linked_output_cannot_be_finalized_or_published() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let attempt = attempt(1, 3);
    let ids = identities(30);
    let mut record = catalog.begin(attempt, scope(3), ids.request).unwrap();
    record
        .record_pinned_worker(&catalog, attempt, ids.request, ids.worker)
        .unwrap();
    record
        .record_validated_response(
            &catalog,
            attempt,
            ids.request,
            ids.worker,
            ids.response,
            ids.linked_output,
        )
        .unwrap();
    let validated = record.clone();

    assert_eq!(
        record.record_finalization(
            &catalog,
            attempt,
            ids.response,
            LinkedOutputIdentityV1::from_bytes([0xaa; 32]),
            ids.finalization,
            ids.finalized_output,
        ),
        Err(LinkPublicationCodecError::IdentityMismatch {
            kind: IdentityKindV1::LinkedOutput,
        })
    );
    assert_eq!(record, validated);
    assert!(catalog.published(&scope(3)).is_none());
}

#[test]
fn crash_recovery_invalidates_incomplete_attempt_and_preserves_prior_and_unrelated_artifacts() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let target_scope = scope(4);
    let unrelated_scope = scope(5);
    let old_ids = identities(40);
    let unrelated_ids = identities(50);
    let old = publish(&mut catalog, attempt(1, 4), target_scope, old_ids);
    let unrelated = publish(&mut catalog, attempt(2, 5), unrelated_scope, unrelated_ids);
    let old_artifact = *catalog.published(&target_scope).unwrap();
    let unrelated_artifact = *catalog.published(&unrelated_scope).unwrap();
    assert_eq!(old.publication(), Some(old_ids.publication));
    assert_eq!(unrelated.publication(), Some(unrelated_ids.publication));

    for (index, expected_phase) in [
        LinkPublicationPhaseV1::RequestBound,
        LinkPublicationPhaseV1::WorkerPinned,
        LinkPublicationPhaseV1::ResponseValidated,
        LinkPublicationPhaseV1::Finalized,
    ]
    .into_iter()
    .enumerate()
    {
        let mut restarted_catalog = catalog.clone();
        let new_attempt = attempt(3 + index as u64, 6 + index as u8);
        let new_ids = identities(60 + index as u8 * 8);
        let mut incomplete = restarted_catalog
            .begin(new_attempt, target_scope, new_ids.request)
            .unwrap();
        if expected_phase != LinkPublicationPhaseV1::RequestBound {
            incomplete
                .record_pinned_worker(
                    &restarted_catalog,
                    new_attempt,
                    new_ids.request,
                    new_ids.worker,
                )
                .unwrap();
        }
        if matches!(
            expected_phase,
            LinkPublicationPhaseV1::ResponseValidated | LinkPublicationPhaseV1::Finalized
        ) {
            incomplete
                .record_validated_response(
                    &restarted_catalog,
                    new_attempt,
                    new_ids.request,
                    new_ids.worker,
                    new_ids.response,
                    new_ids.linked_output,
                )
                .unwrap();
        }
        if expected_phase == LinkPublicationPhaseV1::Finalized {
            incomplete
                .record_finalization(
                    &restarted_catalog,
                    new_attempt,
                    new_ids.response,
                    new_ids.linked_output,
                    new_ids.finalization,
                    new_ids.finalized_output,
                )
                .unwrap();
        }

        let bytes = incomplete.encode_canonical().unwrap();
        let mut restarted = LinkPublicationRecordV1::decode_canonical(&bytes).unwrap();
        assert_eq!(
            restarted.recover(&mut restarted_catalog),
            Ok(RecoveryOutcomeV1::InvalidatedIncomplete)
        );
        assert_eq!(
            restarted.state(),
            LinkPublicationStateV1::Invalidated {
                prior_phase: expected_phase,
                reason: InvalidationReasonV1::CrashRecovery,
            }
        );
        assert_eq!(
            restarted.recover(&mut restarted_catalog),
            Ok(RecoveryOutcomeV1::AlreadyInvalidated)
        );
        assert_eq!(
            restarted_catalog.published(&target_scope),
            Some(&old_artifact)
        );
        assert_eq!(
            restarted_catalog.published(&unrelated_scope),
            Some(&unrelated_artifact)
        );
    }
}

#[test]
fn newer_attempt_isolates_stale_record_and_preserves_current_authority() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let scope = scope(6);
    let stale_attempt = attempt(1, 7);
    let current_attempt = attempt(2, 8);
    let stale_ids = identities(70);
    let current_ids = identities(80);
    let mut stale = catalog
        .begin(stale_attempt, scope, stale_ids.request)
        .unwrap();
    let current = catalog
        .begin(current_attempt, scope, current_ids.request)
        .unwrap();

    assert_eq!(
        stale.record_pinned_worker(&catalog, stale_attempt, stale_ids.request, stale_ids.worker,),
        Err(LinkPublicationCodecError::StaleAttempt)
    );
    assert_eq!(
        stale.recover(&mut catalog),
        Ok(RecoveryOutcomeV1::InvalidatedStaleAttempt)
    );
    assert_eq!(catalog.active_attempt(&scope), Some(current_attempt));
    assert_eq!(
        stale.state(),
        LinkPublicationStateV1::Invalidated {
            prior_phase: LinkPublicationPhaseV1::RequestBound,
            reason: InvalidationReasonV1::StaleAttempt,
        }
    );
    assert_eq!(
        current.state(),
        LinkPublicationStateV1::Active(LinkPublicationPhaseV1::RequestBound)
    );
}

#[test]
fn publication_is_exactly_once_and_scoped() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let first_scope = scope(7);
    let second_scope = scope(8);
    let first_attempt = attempt(1, 9);
    let second_attempt = attempt(2, 10);
    let first_ids = identities(90);
    let second_ids = identities(100);
    let mut first = publish(&mut catalog, first_attempt, first_scope, first_ids);
    let second = publish(&mut catalog, second_attempt, second_scope, second_ids);
    let second_before = *catalog.published(&second_scope).unwrap();

    assert_eq!(
        first.publish(
            &mut catalog,
            first_attempt,
            first_ids.finalization,
            first_ids.finalized_output,
            first_ids.publication,
        ),
        Ok(PublicationOutcomeV1::AlreadyPublished)
    );
    assert_eq!(catalog.published(&second_scope), Some(&second_before));

    let catalog_before = catalog.clone();
    assert_eq!(
        first.publish(
            &mut catalog,
            first_attempt,
            first_ids.finalization,
            first_ids.finalized_output,
            AtomicPublicationIdentityV1::from_bytes([0xfe; 32]),
        ),
        Err(LinkPublicationCodecError::IdentityMismatch {
            kind: IdentityKindV1::Publication,
        })
    );
    assert_eq!(catalog, catalog_before);
    assert_eq!(second.publication(), Some(second_ids.publication));
}

#[test]
fn published_restart_requires_exact_catalog_identity() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let attempt = attempt(1, 11);
    let scope = scope(9);
    let ids = identities(110);
    let published = publish(&mut catalog, attempt, scope, ids);
    let bytes = published.encode_canonical().unwrap();
    let mut restarted = LinkPublicationRecordV1::decode_canonical(&bytes).unwrap();

    assert_eq!(
        restarted.recover(&mut catalog),
        Ok(RecoveryOutcomeV1::PublicationConfirmed)
    );
    assert_eq!(catalog.active_attempt(&scope), None);
    assert_eq!(
        restarted.publish(
            &mut catalog,
            attempt,
            ids.finalization,
            ids.finalized_output,
            ids.publication,
        ),
        Ok(PublicationOutcomeV1::AlreadyPublished)
    );
    assert_eq!(
        restarted.recover(&mut catalog),
        Ok(RecoveryOutcomeV1::PublicationConfirmed)
    );
    assert_eq!(
        catalog.published(&scope).unwrap().publication(),
        ids.publication
    );
}

#[test]
fn published_identity_mutation_cannot_remove_the_valid_publication() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let mutated_attempt = attempt(1, 13);
    let unrelated_attempt = attempt(2, 14);
    let mutated_scope = scope(11);
    let unrelated_scope = scope(12);
    let mutated = publish(
        &mut catalog,
        mutated_attempt,
        mutated_scope,
        identities(130),
    );
    publish(
        &mut catalog,
        unrelated_attempt,
        unrelated_scope,
        identities(140),
    );
    let mutated_before = *catalog.published(&mutated_scope).unwrap();
    let unrelated_before = *catalog.published(&unrelated_scope).unwrap();

    let mut encoded = mutated.encode_canonical().unwrap();
    *encoded.last_mut().unwrap() ^= 1;
    let mut restarted = LinkPublicationRecordV1::decode_canonical(&encoded).unwrap();
    assert_eq!(
        restarted.recover(&mut catalog),
        Ok(RecoveryOutcomeV1::InvalidatedCorruptPublication)
    );
    assert_eq!(
        restarted.state(),
        LinkPublicationStateV1::Invalidated {
            prior_phase: LinkPublicationPhaseV1::Published,
            reason: InvalidationReasonV1::CorruptPublication,
        }
    );
    assert_eq!(catalog.published(&mutated_scope), Some(&mutated_before));
    assert_eq!(catalog.published(&unrelated_scope), Some(&unrelated_before));
}

#[test]
fn same_attempt_different_request_stale_recovery_preserves_valid_publication() {
    let attempt = attempt(7, 15);
    let scope = scope(13);
    let valid_ids = identities(150);
    let stale_ids = identities(160);

    let mut valid_catalog = LinkPublicationCatalogV1::default();
    let valid = publish(&mut valid_catalog, attempt, scope, valid_ids);
    let valid_artifact = *valid_catalog.published(&scope).unwrap();

    let mut stale_catalog = LinkPublicationCatalogV1::default();
    let mut stale = publish(&mut stale_catalog, attempt, scope, stale_ids);
    assert_ne!(stale.request(), valid.request());

    assert_eq!(
        stale.recover(&mut valid_catalog),
        Ok(RecoveryOutcomeV1::InvalidatedStaleAttempt)
    );
    assert_eq!(valid_catalog.published(&scope), Some(&valid_artifact));
    assert_eq!(valid_catalog.active_attempt(&scope), Some(attempt));
}

#[test]
fn same_attempt_request_but_different_publication_cannot_claim_catalog_ownership() {
    let attempt = attempt(8, 16);
    let scope = scope(14);
    let valid_ids = identities(170);
    let mut conflicting_ids = valid_ids;
    conflicting_ids.publication = AtomicPublicationIdentityV1::from_bytes([0xfd; 32]);

    let mut valid_catalog = LinkPublicationCatalogV1::default();
    publish(&mut valid_catalog, attempt, scope, valid_ids);
    let valid_artifact = *valid_catalog.published(&scope).unwrap();

    let mut conflicting_catalog = LinkPublicationCatalogV1::default();
    let mut conflicting = publish(&mut conflicting_catalog, attempt, scope, conflicting_ids);
    assert_eq!(conflicting.request(), valid_ids.request);

    let before_replay = valid_catalog.clone();
    assert_eq!(
        conflicting.publish(
            &mut valid_catalog,
            attempt,
            conflicting_ids.finalization,
            conflicting_ids.finalized_output,
            conflicting_ids.publication,
        ),
        Err(LinkPublicationCodecError::CatalogMismatch)
    );
    assert_eq!(valid_catalog, before_replay);

    assert_eq!(
        conflicting.recover(&mut valid_catalog),
        Ok(RecoveryOutcomeV1::InvalidatedCorruptPublication)
    );
    assert_eq!(valid_catalog.published(&scope), Some(&valid_artifact));
    assert_eq!(valid_catalog.active_attempt(&scope), Some(attempt));
}

#[test]
fn codec_rejects_truncation_trailing_data_and_reordered_state_tags() {
    let mut catalog = LinkPublicationCatalogV1::default();
    let attempt = attempt(1, 12);
    let ids = identities(120);
    let record = catalog.begin(attempt, scope(10), ids.request).unwrap();
    let canonical = record.encode_canonical().unwrap();

    assert_eq!(
        LinkPublicationRecordV1::decode_canonical(&canonical[..canonical.len() - 1]),
        Err(LinkPublicationCodecError::Truncated)
    );
    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(
        LinkPublicationRecordV1::decode_canonical(&trailing),
        Err(LinkPublicationCodecError::TrailingBytes)
    );

    let mut reordered = canonical;
    let request_tag = reordered.iter().rposition(|byte| *byte == 0x20).unwrap();
    reordered[request_tag] = 0x21;
    assert_eq!(
        LinkPublicationRecordV1::decode_canonical(&reordered),
        Err(LinkPublicationCodecError::InvalidIdentityTag {
            expected: 0x20,
            actual: 0x21,
        })
    );
}
