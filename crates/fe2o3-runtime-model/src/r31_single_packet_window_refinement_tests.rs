use super::*;

const BYTES: u64 = 4096;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; 32])
}

fn request(direction: R31DirectionV1) -> R31SingleRequestV1 {
    R31SingleRequestV1 {
        transfer_id: 31,
        queue_id: 7,
        queue_generation: 9,
        host_storage_id: 11,
        host_storage_generation: 13,
        pool_generation: 15,
        host_extent_bytes: BYTES,
        device_extent_bytes: BYTES,
        host_offset: 0,
        device_offset: 0,
        copy_bytes: BYTES,
        direction,
    }
}

fn certificate(request: R31SingleRequestV1) -> R31CertificateV1 {
    R31CertificateV1 {
        queue_id: request.queue_id,
        queue_generation: request.queue_generation,
        host_storage_id: request.host_storage_id,
        host_storage_generation: request.host_storage_generation,
        pool_generation: request.pool_generation,
        extent_bytes: request.host_extent_bytes,
        digest: digest(0x31),
    }
}

fn model(direction: R31DirectionV1) -> R31SingleWindowModelV1 {
    let request = request(direction);
    R31SingleWindowModelV1::new_model_only(request, Some(certificate(request))).unwrap()
}

fn completed_h2d() -> R31SingleWindowModelV1 {
    let mut model = model(R31DirectionV1::HostToDevice);
    model
        .submit_model_only(R31SubmitDispositionV1::Published)
        .unwrap();
    model
        .poll_model_only(R31PollDispositionV1::Completed)
        .unwrap();
    model
}

#[test]
fn invalid_and_oversized_requests_are_rejected() {
    let mut invalid = request(R31DirectionV1::HostToDevice);
    invalid.copy_bytes = 0;
    assert!(matches!(
        R31SingleWindowModelV1::new_model_only(invalid, None),
        Err(R31ErrorV1::InvalidRequest)
    ));
    invalid.host_extent_bytes = R31_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1;
    invalid.device_extent_bytes = invalid.host_extent_bytes;
    invalid.copy_bytes = invalid.host_extent_bytes;
    assert!(matches!(
        R31SingleWindowModelV1::new_model_only(invalid, None),
        Err(R31ErrorV1::InvalidRequest)
    ));
}

#[test]
fn substituted_certificate_is_rejected() {
    let request = request(R31DirectionV1::HostToDevice);
    let mut certificate = certificate(request);
    certificate.pool_generation += 1;
    assert!(matches!(
        R31SingleWindowModelV1::new_model_only(request, Some(certificate)),
        Err(R31ErrorV1::InvalidCertificate)
    ));
}

#[test]
fn maximum_packet_is_single_and_maximum_plus_one_is_out_of_scope() {
    let mut maximum = request(R31DirectionV1::HostToDevice);
    maximum.host_extent_bytes = R31_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    maximum.device_extent_bytes = maximum.host_extent_bytes;
    maximum.copy_bytes = maximum.host_extent_bytes;
    let model = R31SingleWindowModelV1::new_model_only(maximum, None).unwrap();
    assert_eq!(model.window_snapshot().requests, [maximum]);
    maximum.host_extent_bytes += 1;
    maximum.device_extent_bytes += 1;
    maximum.copy_bytes += 1;
    assert!(matches!(
        R31SingleWindowModelV1::new_model_only(maximum, None),
        Err(R31ErrorV1::InvalidRequest)
    ));
}

#[test]
fn successful_h2d_submit_is_one_packet_and_preserves_certificate() {
    let mut model = model(R31DirectionV1::HostToDevice);
    let certificate = model.single_snapshot().host_certificate;
    assert_eq!(
        model.submit_model_only(R31SubmitDispositionV1::Published),
        Ok(R31SubmitOutcomeV1::Published)
    );
    let state = model.single_snapshot();
    assert_eq!(
        (state.packet_count, state.ticket_count, state.lease_count),
        (1, 1, 1)
    );
    assert_eq!((state.directional_checks, state.queue_checks), (3, 1));
    assert_eq!(state.host_certificate, certificate);
    assert!(!state.host_certificate_invalidated);
}

#[test]
fn d2h_request_construction_invalidates_before_possible_mutation() {
    let mut published = model(R31DirectionV1::DeviceToHost);
    published
        .submit_model_only(R31SubmitDispositionV1::Published)
        .unwrap();
    let state = published.single_snapshot();
    assert_eq!(state.host_certificate, None);
    assert!(state.host_certificate_invalidated);
    assert!(state.host_destination_may_have_mutated);

    let mut ambiguous = model(R31DirectionV1::DeviceToHost);
    assert_eq!(
        ambiguous.submit_model_only(R31SubmitDispositionV1::ClosingAmbiguous),
        Ok(R31SubmitOutcomeV1::TerminalAbsorbed)
    );
    let state = ambiguous.single_snapshot();
    assert_eq!(state.host_certificate, None);
    assert!(state.host_certificate_invalidated);
    assert!(state.host_destination_may_have_mutated);
    assert_eq!(state.retired_frontiers, 0);
}

#[test]
fn d2h_retry_after_request_construction_remains_invalidated() {
    for disposition in [
        R31SubmitDispositionV1::PrepareRetryable,
        R31SubmitDispositionV1::PublicationRetryable,
    ] {
        let mut model = model(R31DirectionV1::DeviceToHost);
        assert_eq!(
            model.submit_model_only(disposition),
            Ok(R31SubmitOutcomeV1::Retryable)
        );
        let state = model.single_snapshot();
        assert_eq!(state.phase, R31PhaseV1::Ready);
        assert_eq!(state.host_certificate, None);
        assert!(state.host_certificate_invalidated);
        assert!(!state.host_destination_may_have_mutated);
    }
}

#[test]
fn pre_request_failures_do_not_construct_d2h_destination() {
    for disposition in [
        R31SubmitDispositionV1::RetryableBeforeRequest,
        R31SubmitDispositionV1::OpeningAmbiguous,
    ] {
        let mut model = model(R31DirectionV1::DeviceToHost);
        let certificate = model.single_snapshot().host_certificate;
        model.submit_model_only(disposition).unwrap();
        assert_eq!(model.single_snapshot().host_certificate, certificate);
        assert!(!model.single_snapshot().host_certificate_invalidated);
    }
}

#[test]
fn submit_ambiguity_retains_exact_stage_and_is_absorbing() {
    for (disposition, stage, checks) in [
        (
            R31SubmitDispositionV1::OpeningAmbiguous,
            R31TerminalStageV1::SubmitOpening,
            (1, 0),
        ),
        (
            R31SubmitDispositionV1::PrepareAmbiguous,
            R31TerminalStageV1::SubmitPrepare,
            (2, 0),
        ),
        (
            R31SubmitDispositionV1::ClosingAmbiguous,
            R31TerminalStageV1::SubmitClosing,
            (3, 1),
        ),
    ] {
        let mut model = model(R31DirectionV1::HostToDevice);
        assert_eq!(
            model.submit_model_only(disposition),
            Ok(R31SubmitOutcomeV1::TerminalAbsorbed)
        );
        let state = model.single_snapshot();
        assert_eq!(state.terminal_stage, Some(stage));
        assert_eq!((state.directional_checks, state.queue_checks), checks);
        assert_eq!(
            model.submit_model_only(R31SubmitDispositionV1::Published),
            Err(R31ErrorV1::IllegalPhase)
        );
    }
}

#[test]
fn pending_poll_preserves_custody_and_counts_two_checks() {
    let mut model = model(R31DirectionV1::HostToDevice);
    model
        .submit_model_only(R31SubmitDispositionV1::Published)
        .unwrap();
    let before = model.single_snapshot();
    assert_eq!(
        model.poll_model_only(R31PollDispositionV1::Pending),
        Ok(R31PollOutcomeV1::Pending)
    );
    let after = model.single_snapshot();
    assert_eq!(after.phase, R31PhaseV1::Published);
    assert_eq!(after.queue_checks, before.queue_checks + 2);
    assert_eq!(
        (after.ticket_count, after.lease_count),
        (before.ticket_count, before.lease_count)
    );
}

#[test]
fn exact_completion_retains_offsets_and_normalizes_to_one_packet() {
    let mut request = request(R31DirectionV1::HostToDevice);
    request.host_extent_bytes = 8192;
    request.device_extent_bytes = 8192;
    request.host_offset = 1024;
    request.device_offset = 2048;
    request.copy_bytes = 3072;
    let mut model = R31SingleWindowModelV1::new_model_only(request, None).unwrap();
    model
        .submit_model_only(R31SubmitDispositionV1::Published)
        .unwrap();
    model
        .poll_model_only(R31PollDispositionV1::Completed)
        .unwrap();
    assert_eq!(
        model.single_snapshot().completion,
        Some(R31CompletionV1::exact_for(request))
    );
    assert_eq!(
        r31_project_single_to_window_v1(model.single_snapshot()),
        model.window_snapshot()
    );
}

#[test]
fn poll_ambiguity_is_stage_specific_and_never_completes() {
    for (disposition, stage, checks) in [
        (
            R31PollDispositionV1::OpeningAmbiguous,
            R31TerminalStageV1::PollOpening,
            1,
        ),
        (
            R31PollDispositionV1::ClosingAmbiguous,
            R31TerminalStageV1::PollClosing,
            2,
        ),
    ] {
        let mut model = model(R31DirectionV1::HostToDevice);
        model
            .submit_model_only(R31SubmitDispositionV1::Published)
            .unwrap();
        let before = model.single_snapshot();
        assert_eq!(
            model.poll_model_only(disposition),
            Ok(R31PollOutcomeV1::TerminalAbsorbed)
        );
        let state = model.single_snapshot();
        assert_eq!(state.terminal_stage, Some(stage));
        assert_eq!(state.queue_checks, before.queue_checks + checks);
        assert_eq!((state.completion, state.retired_frontiers), (None, 0));
    }
}

#[test]
fn promotion_mismatch_is_retryable_after_checks_without_retirement() {
    let mut model = completed_h2d();
    let before = model.single_snapshot();
    let mut substituted = before.host_certificate.unwrap();
    substituted.digest = digest(0x55);
    assert_eq!(
        model.promote_model_only(substituted, R31PromotionDispositionV1::Current),
        Ok(R31PromotionOutcomeV1::Retryable)
    );
    let after = model.single_snapshot();
    assert_eq!(after.phase, R31PhaseV1::Completed);
    assert_eq!(after.queue_checks, before.queue_checks + 2);
    assert_eq!(
        (after.retired_frontiers, after.completion),
        (0, before.completion)
    );
}

#[test]
fn promotion_ambiguity_precedes_certificate_classification() {
    for (disposition, stage, checks) in [
        (
            R31PromotionDispositionV1::OpeningAmbiguous,
            R31TerminalStageV1::PromotionOpening,
            1,
        ),
        (
            R31PromotionDispositionV1::ClosingAmbiguous,
            R31TerminalStageV1::PromotionClosing,
            2,
        ),
    ] {
        let mut model = completed_h2d();
        let before = model.single_snapshot();
        let mut substituted = before.host_certificate.unwrap();
        substituted.digest = digest(0x77);
        assert_eq!(
            model.promote_model_only(substituted, disposition),
            Ok(R31PromotionOutcomeV1::TerminalAbsorbed)
        );
        let after = model.single_snapshot();
        assert_eq!(after.terminal_stage, Some(stage));
        assert_eq!(after.queue_checks, before.queue_checks + checks);
        assert_eq!(
            (after.retired_frontiers, after.completion),
            (0, before.completion)
        );
    }
}

#[test]
fn exact_full_h2d_promotion_retires_once_and_mints_digest() {
    let mut model = completed_h2d();
    let certificate = model.single_snapshot().host_certificate.unwrap();
    assert_eq!(
        model.promote_model_only(certificate, R31PromotionDispositionV1::Current),
        Ok(R31PromotionOutcomeV1::Ready)
    );
    let state = model.single_snapshot();
    assert_eq!(state.phase, R31PhaseV1::ComputeReady);
    assert_eq!(
        (state.retired_frontiers, state.ready_digest),
        (1, Some(certificate.digest))
    );
    assert_eq!(
        (state.completion, state.ticket_count, state.lease_count),
        (None, 0, 0)
    );
}

#[test]
fn d2h_and_partial_h2d_cannot_promote() {
    let mut d2h = model(R31DirectionV1::DeviceToHost);
    d2h.submit_model_only(R31SubmitDispositionV1::Published)
        .unwrap();
    d2h.poll_model_only(R31PollDispositionV1::Completed)
        .unwrap();
    assert_eq!(
        d2h.promote_model_only(
            certificate(request(R31DirectionV1::DeviceToHost)),
            R31PromotionDispositionV1::Current
        ),
        Err(R31ErrorV1::DirectionMismatch)
    );

    let mut partial = request(R31DirectionV1::HostToDevice);
    partial.host_extent_bytes = 8192;
    partial.device_extent_bytes = 8192;
    let mut partial_model = R31SingleWindowModelV1::new_model_only(partial, None).unwrap();
    partial_model
        .submit_model_only(R31SubmitDispositionV1::Published)
        .unwrap();
    partial_model
        .poll_model_only(R31PollDispositionV1::Completed)
        .unwrap();
    assert_eq!(
        partial_model.promote_model_only(certificate(partial), R31PromotionDispositionV1::Current),
        Err(R31ErrorV1::DirectionMismatch)
    );
}

#[test]
fn all_submit_dispositions_keep_models_in_lockstep() {
    for direction in [R31DirectionV1::HostToDevice, R31DirectionV1::DeviceToHost] {
        for disposition in [
            R31SubmitDispositionV1::RetryableBeforeRequest,
            R31SubmitDispositionV1::OpeningAmbiguous,
            R31SubmitDispositionV1::PrepareRetryable,
            R31SubmitDispositionV1::PrepareAmbiguous,
            R31SubmitDispositionV1::PublicationRetryable,
            R31SubmitDispositionV1::ClosingAmbiguous,
            R31SubmitDispositionV1::Published,
        ] {
            let mut model = model(direction);
            model.submit_model_only(disposition).unwrap();
            assert_eq!(
                r31_project_single_to_window_v1(model.single_snapshot()),
                model.window_snapshot()
            );
        }
    }
}
