use super::*;

const LOGICAL_BYTES: u64 = 4096;
const PHYSICAL_BYTES: u64 = LOGICAL_BYTES;

fn storage() -> R30BoundHostStorageV1 {
    R30BoundHostStorageV1 {
        queue_id: 30,
        queue_generation: 4,
        storage_id: 71,
        storage_generation: 9,
        pool_generation: 12,
        logical_extent_bytes: LOGICAL_BYTES,
        physical_extent_bytes: PHYSICAL_BYTES,
    }
}

fn full_range() -> R30HostStorageRangeV1 {
    R30HostStorageRangeV1 {
        logical_offset: 0,
        logical_bytes: LOGICAL_BYTES,
        physical_offset: 0,
        physical_bytes: PHYSICAL_BYTES,
    }
}

fn digest(seed: u8) -> R30HostContentDigestV1 {
    R30HostContentDigestV1(IdentityDigestV1::from_untrusted_bytes([seed; 32]))
}

fn model() -> R30BoundHostContentCertificateModelV1 {
    R30BoundHostContentCertificateModelV1::new_model_only(storage()).unwrap()
}

fn certified(
    model: &mut R30BoundHostContentCertificateModelV1,
) -> R30BoundHostContentCertificateV1 {
    model
        .complete_exact_full_cpu_write_model_only(
            full_range(),
            digest(0x30),
            R30CurrentnessEnvelopeV1::all_current(),
        )
        .unwrap()
}

fn completed(
    model: &mut R30BoundHostContentCertificateModelV1,
) -> (R30BoundHostContentCertificateV1, R30FullH2dCompletionV1) {
    let certificate = certified(model);
    let completion = model
        .record_exact_full_h2d_completion_model_only(full_range(), 1)
        .unwrap();
    (certificate, completion)
}

#[test]
fn exact_full_write_establishes_bound_certificate_only_after_currentness() {
    let mut model = model();
    let certificate = certified(&mut model);
    assert_eq!(certificate.storage, storage());
    assert_eq!(certificate.full_range, full_range());
    assert_eq!(certificate.digest, digest(0x30));
    assert_eq!(model.snapshot().certificate, Some(certificate));
    let ordering = model.snapshot().last_mutation_ordering.unwrap();
    assert_eq!(ordering.kind, R30HostMutationKindV1::CpuDestinationWrite);
    assert!(ordering.invalidated_before_possible_mutation());
    model.validate_global_invariants().unwrap();
}

#[test]
fn partial_full_write_claim_is_atomic_and_does_not_establish() {
    let mut model = model();
    let before = model.snapshot();
    let mut partial = full_range();
    partial.logical_bytes -= 1;
    assert_eq!(
        model.complete_exact_full_cpu_write_model_only(
            partial,
            digest(1),
            R30CurrentnessEnvelopeV1::all_current(),
        ),
        Err(R30BoundHostContentCertificateErrorV1::InvalidRange)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn ambiguous_post_write_currentness_leaves_content_uncertified() {
    let mut model = model();
    assert_eq!(
        model.complete_exact_full_cpu_write_model_only(
            full_range(),
            digest(1),
            R30CurrentnessEnvelopeV1 {
                opening: R30ContractedCurrentnessV1::Current,
                closing: R30ContractedCurrentnessV1::Ambiguous,
            },
        ),
        Err(R30BoundHostContentCertificateErrorV1::CurrentnessAmbiguous)
    );
    assert_eq!(model.snapshot().certificate, None);
    assert!(
        model
            .snapshot()
            .last_mutation_ordering
            .unwrap()
            .invalidated_before_possible_mutation()
    );
    model.validate_global_invariants().unwrap();
}

#[test]
fn ambiguous_opening_write_currentness_clears_without_possible_mutation() {
    let mut model = model();
    certified(&mut model);
    let before = model.snapshot();
    assert_eq!(
        model.complete_exact_full_cpu_write_model_only(
            full_range(),
            digest(1),
            R30CurrentnessEnvelopeV1 {
                opening: R30ContractedCurrentnessV1::Ambiguous,
                closing: R30ContractedCurrentnessV1::Current,
            },
        ),
        Err(R30BoundHostContentCertificateErrorV1::CurrentnessAmbiguous)
    );
    let after = model.snapshot();
    assert_eq!(after.certificate, None);
    assert_eq!(after.transition_clock, before.transition_clock + 1);
    assert_eq!(
        after.last_certificate_invalidation_step,
        Some(before.transition_clock + 1)
    );
    assert_eq!(after.last_mutation_ordering, None);
}

#[test]
fn promotion_currentness_ambiguity_precedes_candidate_certificate_mismatch() {
    let mut model = model();
    let (certificate, completion) = completed(&mut model);
    let mut substituted = certificate;
    substituted.digest = digest(0x44);
    assert_eq!(
        model
            .promote_model_only(
                completion,
                substituted,
                R30CurrentnessEnvelopeV1 {
                    opening: R30ContractedCurrentnessV1::Current,
                    closing: R30ContractedCurrentnessV1::Ambiguous,
                },
            )
            .unwrap(),
        R30PromotionOutcomeV1::TerminalAbsorbed
    );
    assert_eq!(
        model.snapshot().terminal_custody,
        Some(R30TerminalPromotionCustodyV1 {
            completion,
            stored_certificate: Some(certificate),
            stage: R30TerminalPromotionStageV1::ClosingCurrentnessAmbiguous,
        })
    );
    assert_eq!(model.snapshot().retired_completion_generation, 0);
}

#[test]
fn missing_stored_certificate_retries_only_after_currentness() {
    let mut model = model();
    let completion = model
        .record_exact_full_h2d_completion_model_only(full_range(), 1)
        .unwrap();
    let candidate = R30BoundHostContentCertificateV1 {
        storage: storage(),
        full_range: full_range(),
        digest: digest(0x55),
    };
    let before = model.snapshot();
    assert_eq!(
        model
            .promote_model_only(
                completion,
                candidate,
                R30CurrentnessEnvelopeV1::all_current(),
            )
            .unwrap(),
        R30PromotionOutcomeV1::RetryableNoEffect
    );
    assert_eq!(model.snapshot(), before);
    assert_eq!(
        model
            .promote_model_only(
                completion,
                candidate,
                R30CurrentnessEnvelopeV1 {
                    opening: R30ContractedCurrentnessV1::Current,
                    closing: R30ContractedCurrentnessV1::Ambiguous,
                },
            )
            .unwrap(),
        R30PromotionOutcomeV1::TerminalAbsorbed
    );
    assert_eq!(
        model.snapshot().terminal_custody,
        Some(R30TerminalPromotionCustodyV1 {
            completion,
            stored_certificate: None,
            stage: R30TerminalPromotionStageV1::ClosingCurrentnessAmbiguous,
        })
    );
    assert_eq!(model.snapshot().retired_completion_generation, 0);
    model.validate_global_invariants().unwrap();
}

#[test]
fn cpu_and_sdma_destinations_invalidate_before_possible_mutation() {
    for (sdma, expected_kind) in [
        (false, R30HostMutationKindV1::CpuDestinationWrite),
        (true, R30HostMutationKindV1::SdmaDestinationWrite),
    ] {
        let mut model = model();
        certified(&mut model);
        let partial = R30HostStorageRangeV1 {
            logical_offset: 64,
            logical_bytes: 32,
            physical_offset: 128,
            physical_bytes: 32,
        };
        if sdma {
            model.sdma_destination_write_model_only(partial).unwrap();
        } else {
            model.cpu_destination_write_model_only(partial).unwrap();
        }
        assert_eq!(model.snapshot().certificate, None);
        let ordering = model.snapshot().last_mutation_ordering.unwrap();
        assert_eq!(ordering.kind, expected_kind);
        assert!(ordering.invalidated_before_possible_mutation());
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn resize_and_recycle_invalidate_before_changing_bound_coordinates() {
    let mut resized = model();
    certified(&mut resized);
    resized.resize_model_only(2048, 4096).unwrap();
    assert_eq!(resized.snapshot().certificate, None);
    assert_eq!(resized.snapshot().storage.logical_extent_bytes, 2048);
    assert_eq!(
        resized.snapshot().last_mutation_ordering.unwrap().kind,
        R30HostMutationKindV1::Resize
    );
    resized.validate_global_invariants().unwrap();

    let mut recycled = model();
    certified(&mut recycled);
    recycled.recycle_model_only(10, 13).unwrap();
    assert_eq!(recycled.snapshot().certificate, None);
    assert_eq!(recycled.snapshot().storage.storage_generation, 10);
    assert_eq!(recycled.snapshot().storage.pool_generation, 13);
    assert_eq!(
        recycled.snapshot().last_mutation_ordering.unwrap().kind,
        R30HostMutationKindV1::Recycle
    );
    recycled.validate_global_invariants().unwrap();
}

#[test]
fn h2d_source_use_and_exact_completion_preserve_certificate() {
    let mut model = model();
    let certificate = certified(&mut model);
    let before_source_use = model.snapshot();
    model.use_as_h2d_source_model_only(certificate).unwrap();
    assert_eq!(model.snapshot(), before_source_use);
    let completion = model
        .record_exact_full_h2d_completion_model_only(full_range(), 1)
        .unwrap();
    assert_eq!(completion.storage, certificate.storage);
    assert_eq!(completion.full_range, certificate.full_range);
    assert_eq!(model.snapshot().certificate, Some(certificate));
    model.validate_global_invariants().unwrap();
}

#[test]
fn substituted_h2d_source_is_rejected_without_effect() {
    let mut model = model();
    let certificate = certified(&mut model);
    let before = model.snapshot();
    let mut substituted = certificate;
    substituted.storage.pool_generation += 1;
    assert_eq!(
        model.use_as_h2d_source_model_only(substituted),
        Err(R30BoundHostContentCertificateErrorV1::InvalidRange)
    );
    assert_eq!(model.snapshot(), before);
}

#[test]
fn promotion_mismatch_is_retryable_and_retires_nothing() {
    let mut model = model();
    let (certificate, completion) = completed(&mut model);
    let before = model.snapshot();
    let mut substituted = certificate;
    substituted.digest = digest(0x31);
    assert_eq!(
        model
            .promote_model_only(
                completion,
                substituted,
                R30CurrentnessEnvelopeV1::all_current(),
            )
            .unwrap(),
        R30PromotionOutcomeV1::RetryableNoEffect
    );
    assert_eq!(model.snapshot(), before);
    assert_eq!(model.snapshot().retired_completion_generation, 0);
}

#[test]
fn opening_and_closing_promotion_ambiguity_have_distinct_exact_custody() {
    for (currentness, stage) in [
        (
            R30CurrentnessEnvelopeV1 {
                opening: R30ContractedCurrentnessV1::Ambiguous,
                closing: R30ContractedCurrentnessV1::Current,
            },
            R30TerminalPromotionStageV1::OpeningCurrentnessAmbiguous,
        ),
        (
            R30CurrentnessEnvelopeV1 {
                opening: R30ContractedCurrentnessV1::Current,
                closing: R30ContractedCurrentnessV1::Ambiguous,
            },
            R30TerminalPromotionStageV1::ClosingCurrentnessAmbiguous,
        ),
    ] {
        let mut model = model();
        let (certificate, completion) = completed(&mut model);
        assert_eq!(
            model
                .promote_model_only(completion, certificate, currentness)
                .unwrap(),
            R30PromotionOutcomeV1::TerminalAbsorbed
        );
        let terminal = model.snapshot();
        assert_eq!(terminal.phase, R30CertificatePhaseV1::TerminalAbsorbed);
        assert_eq!(
            terminal.terminal_custody,
            Some(R30TerminalPromotionCustodyV1 {
                completion,
                stored_certificate: Some(certificate),
                stage,
            })
        );
        assert_eq!(terminal.retired_completion_generation, 0);
        assert_eq!(
            model
                .promote_model_only(
                    completion,
                    certificate,
                    R30CurrentnessEnvelopeV1::all_current(),
                )
                .unwrap(),
            R30PromotionOutcomeV1::TerminalAbsorbed
        );
        assert_eq!(model.snapshot(), terminal);
        model.validate_global_invariants().unwrap();
    }
}

#[test]
fn successful_promotion_atomically_retires_completion_and_mints_ready() {
    let mut model = model();
    let (certificate, completion) = completed(&mut model);
    assert_eq!(
        model
            .promote_model_only(
                completion,
                certificate,
                R30CurrentnessEnvelopeV1::all_current(),
            )
            .unwrap(),
        R30PromotionOutcomeV1::Ready
    );
    let ready = model.snapshot();
    assert_eq!(ready.phase, R30CertificatePhaseV1::Ready);
    assert_eq!(ready.pending_completion, None);
    assert_eq!(ready.retired_completion_generation, 1);
    assert_eq!(
        ready.ready,
        Some(R30ReadyContentV1 {
            completion_generation: 1,
            digest: certificate.digest,
        })
    );
    assert_eq!(ready.certificate, Some(certificate));
    model.validate_global_invariants().unwrap();
}

#[test]
fn returned_host_can_recycle_without_changing_ready_digest() {
    let mut model = model();
    let (certificate, completion) = completed(&mut model);
    model
        .promote_model_only(
            completion,
            certificate,
            R30CurrentnessEnvelopeV1::all_current(),
        )
        .unwrap();
    let ready = model.snapshot().ready;
    model.recycle_model_only(10, 13).unwrap();
    assert_eq!(model.snapshot().phase, R30CertificatePhaseV1::Ready);
    assert_eq!(model.snapshot().certificate, None);
    assert_eq!(model.snapshot().ready, ready);
    assert_eq!(model.snapshot().storage.storage_generation, 10);
    assert_eq!(model.snapshot().storage.pool_generation, 13);
    model.validate_global_invariants().unwrap();
}
