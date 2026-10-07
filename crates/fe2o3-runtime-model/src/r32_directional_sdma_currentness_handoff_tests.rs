use super::*;

fn binding(direction: R32DirectionV1, packet_count: u8) -> R32HandoffBindingV1 {
    R32HandoffBindingV1 {
        queue_id: 32,
        queue_generation: 7,
        native_queue_id: 19,
        direction,
        packet_count,
    }
}

fn roster(binding: R32HandoffBindingV1) -> R32TicketRosterV1 {
    let mut tickets = [None; R32_MAX_DIRECTIONAL_PACKETS_V1];
    for (index, slot) in tickets
        .iter_mut()
        .enumerate()
        .take(binding.packet_count as usize)
    {
        *slot = Some(R32TicketV1 {
            queue_id: binding.queue_id,
            queue_generation: binding.queue_generation,
            native_queue_id: binding.native_queue_id,
            direction: binding.direction,
            occurrence: index as u8,
            generation: index as u64 + 1,
        });
    }
    R32TicketRosterV1 { tickets }
}

fn certificate(binding: R32HandoffBindingV1) -> R32HostCertificateV1 {
    R32HostCertificateV1 {
        certificate_id: 320,
        queue_id: binding.queue_id,
        queue_generation: binding.queue_generation,
    }
}

fn observations(
    preparation: R32PreparationObservationV1,
    publication: R32PublicationObservationV1,
) -> R32SubmitObservationsV1 {
    R32SubmitObservationsV1 {
        opening_current: true,
        preparation,
        prepare_failure_close_current: true,
        shared_current: true,
        publication,
        final_close_current: true,
    }
}

fn model(direction: R32DirectionV1, packet_count: u8) -> R32DirectionalCurrentnessHandoffModelV1 {
    let binding = binding(direction, packet_count);
    R32DirectionalCurrentnessHandoffModelV1::new_model_only(
        binding,
        roster(binding),
        Some(certificate(binding)),
    )
    .unwrap()
}

fn old_and_shared(
    observations: R32SubmitObservationsV1,
) -> (R32SubmitSnapshotV1, R32SubmitSnapshotV1) {
    let old = model(R32DirectionV1::HostToDevice, 4).run_old_model_only(observations);
    let shared = model(R32DirectionV1::HostToDevice, 4).run_shared_model_only(observations);
    (old, shared)
}

#[test]
fn invalid_binding_and_non_exact_rosters_are_rejected() {
    let mut invalid = binding(R32DirectionV1::HostToDevice, 1);
    invalid.queue_id = 0;
    assert!(matches!(
        R32DirectionalCurrentnessHandoffModelV1::new_model_only(
            invalid,
            roster(binding(R32DirectionV1::HostToDevice, 1)),
            None,
        ),
        Err(R32ModelErrorV1::InvalidBinding)
    ));

    let binding = binding(R32DirectionV1::HostToDevice, 2);
    let mut substituted = roster(binding);
    substituted.tickets[1].as_mut().unwrap().native_queue_id += 1;
    assert!(matches!(
        R32DirectionalCurrentnessHandoffModelV1::new_model_only(binding, substituted, None),
        Err(R32ModelErrorV1::InvalidRoster)
    ));
}

#[test]
fn certificate_must_bind_the_queue_occurrence() {
    let binding = binding(R32DirectionV1::HostToDevice, 1);
    let mut substituted = certificate(binding);
    substituted.queue_generation += 1;
    assert!(matches!(
        R32DirectionalCurrentnessHandoffModelV1::new_model_only(
            binding,
            roster(binding),
            Some(substituted),
        ),
        Err(R32ModelErrorV1::InvalidCertificate)
    ));
}

#[test]
fn successful_shared_path_refines_four_checks_to_three() {
    let (old, shared) = old_and_shared(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    ));
    assert!(old.same_external_semantics(&shared));
    assert_eq!(old.operational_checks(), 4);
    assert_eq!(shared.operational_checks(), 3);
    assert_eq!(old.directional_checks, 3);
    assert_eq!(shared.directional_checks, 2);
    assert_eq!(old.queue_checks, shared.queue_checks);
}

#[test]
fn shared_success_publishes_immediately_without_intervening_actions() {
    let shared = model(R32DirectionV1::HostToDevice, 3).run_shared_model_only(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    ));
    assert_eq!(
        shared.trace(),
        &[
            Some(R32TraceEventV1::OpeningCheck),
            Some(R32TraceEventV1::Prepare),
            Some(R32TraceEventV1::SharedCloseOpen),
            Some(R32TraceEventV1::Publication),
            Some(R32TraceEventV1::FinalClose),
        ]
    );
    assert_eq!(shared.fallible_actions_between_shared_and_publication, 0);
    assert_eq!(shared.native_actions_between_shared_and_publication, 0);
}

#[test]
fn every_prepare_failure_executes_old_close_and_keeps_exact_custody() {
    for preparation in [
        R32PreparationObservationV1::RetryableFailure,
        R32PreparationObservationV1::PoisonedFailure,
        R32PreparationObservationV1::RosterMismatch,
    ] {
        for close_current in [false, true] {
            let mut input = observations(preparation, R32PublicationObservationV1::Published);
            input.prepare_failure_close_current = close_current;
            let (old, shared) = old_and_shared(input);
            assert!(old.same_external_semantics(&shared));
            assert!(shared.prepare_failure_close_observed);
            assert_eq!(shared.operational_checks(), 2);
            assert!(!shared.shared_close_open_observed);
            assert!(!shared.publication_attempted);
            let retryable =
                preparation == R32PreparationObservationV1::RetryableFailure && close_current;
            assert_eq!(shared.outcome == R32OutcomeV1::Retryable, retryable);
            assert_eq!(
                shared.custody,
                if retryable {
                    R32CustodyV1::RetryableRequest
                } else {
                    R32CustodyV1::TerminalRequest
                }
            );
        }
    }
}

#[test]
fn shared_observation_failure_is_terminal_with_prepared_custody() {
    let mut input = observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    );
    input.shared_current = false;
    let (old, shared) = old_and_shared(input);
    assert!(old.same_external_semantics(&shared));
    assert_eq!(shared.outcome, R32OutcomeV1::Terminal);
    assert_eq!(shared.custody, R32CustodyV1::TerminalPrepared);
    assert_eq!(
        shared.terminal_stage,
        Some(R32TerminalStageV1::SharedCloseOpen)
    );
    assert!(shared.shared_close_open_observed);
    assert!(!shared.publication_attempted);
    assert!(!shared.final_close_observed);
}

#[test]
fn opening_failure_never_prepares_or_publishes() {
    let mut input = observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    );
    input.opening_current = false;
    let (old, shared) = old_and_shared(input);
    assert!(old.same_external_semantics(&shared));
    assert_eq!(shared.operational_checks(), 1);
    assert_eq!(shared.custody, R32CustodyV1::TerminalRequest);
    assert_eq!(shared.trace(), &[Some(R32TraceEventV1::OpeningCheck)]);
}

#[test]
fn recoverable_publication_restores_request_after_final_close() {
    let (old, shared) = old_and_shared(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Recoverable,
    ));
    assert!(old.same_external_semantics(&shared));
    assert_eq!(shared.outcome, R32OutcomeV1::Retryable);
    assert_eq!(shared.custody, R32CustodyV1::RetryableRequest);
    assert!(shared.final_close_observed);
}

#[test]
fn retained_publication_is_terminal_published_custody() {
    let (old, shared) = old_and_shared(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Retained,
    ));
    assert!(old.same_external_semantics(&shared));
    assert_eq!(shared.outcome, R32OutcomeV1::Terminal);
    assert_eq!(shared.custody, R32CustodyV1::TerminalPublished);
    assert!(shared.final_close_observed);
}

#[test]
fn every_publication_outcome_retains_final_close() {
    for publication in [
        R32PublicationObservationV1::Recoverable,
        R32PublicationObservationV1::Retained,
        R32PublicationObservationV1::Published,
    ] {
        for close_current in [false, true] {
            let mut input = observations(R32PreparationObservationV1::Prepared, publication);
            input.final_close_current = close_current;
            let (old, shared) = old_and_shared(input);
            assert!(old.same_external_semantics(&shared));
            assert!(shared.final_close_observed);
            assert_eq!(shared.operational_checks(), 3);
            if !close_current {
                assert_eq!(shared.outcome, R32OutcomeV1::Terminal);
            }
        }
    }
}

#[test]
fn handoff_preserves_exact_roster_binding() {
    let binding = binding(R32DirectionV1::HostToDevice, 64);
    let expected = roster(binding);
    let shared = R32DirectionalCurrentnessHandoffModelV1::new_model_only(
        binding,
        expected,
        Some(certificate(binding)),
    )
    .unwrap()
    .run_shared_model_only(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    ));
    assert_eq!(shared.binding, binding);
    assert_eq!(shared.roster, expected);
    assert!(shared.roster.exact_for(binding));
}

#[test]
fn h2d_certificate_is_unchanged_across_handoff() {
    let binding = binding(R32DirectionV1::HostToDevice, 2);
    let certificate = certificate(binding);
    let shared = R32DirectionalCurrentnessHandoffModelV1::new_model_only(
        binding,
        roster(binding),
        Some(certificate),
    )
    .unwrap()
    .run_shared_model_only(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    ));
    assert_eq!(shared.host_certificate, Some(certificate));
    assert!(!shared.host_certificate_invalidated);
}

#[test]
fn d2h_certificate_is_invalidated_before_every_modeled_outcome() {
    for preparation in [
        R32PreparationObservationV1::RetryableFailure,
        R32PreparationObservationV1::Prepared,
    ] {
        for publication in [
            R32PublicationObservationV1::Recoverable,
            R32PublicationObservationV1::Retained,
            R32PublicationObservationV1::Published,
        ] {
            let shared = model(R32DirectionV1::DeviceToHost, 2)
                .run_shared_model_only(observations(preparation, publication));
            assert_eq!(shared.host_certificate, None);
            assert!(shared.host_certificate_invalidated);
        }
    }
}

#[test]
fn same_device_state_is_not_transformed() {
    let state = R32SameDeviceSnapshotV1 {
        source_queue_id: 3,
        destination_queue_id: 5,
        packet_count: 17,
        operational_checks: 6,
        publication_count: 1,
    };
    assert_eq!(r32_same_device_unchanged_model_only(state), state);
}

#[test]
fn single_packet_uses_the_same_bound_handoff() {
    let binding = binding(R32DirectionV1::HostToDevice, 1);
    let expected = roster(binding);
    let shared = R32DirectionalCurrentnessHandoffModelV1::new_model_only(
        binding,
        expected,
        Some(certificate(binding)),
    )
    .unwrap()
    .run_shared_model_only(observations(
        R32PreparationObservationV1::Prepared,
        R32PublicationObservationV1::Published,
    ));
    assert_eq!(shared.roster, expected);
    assert_eq!(shared.outcome, R32OutcomeV1::Published);
    assert_eq!(shared.operational_checks(), 3);
}

#[test]
fn executable_handoff_carrier_is_private_and_has_no_clone_derive() {
    let source = include_str!("r32_directional_sdma_currentness_handoff.rs");
    let declaration = source
        .split("struct DirectionalPreparedHandoffV1")
        .nth(1)
        .unwrap()
        .split("impl DirectionalPreparedHandoffV1")
        .next()
        .unwrap();
    let prefix = source
        .split("struct DirectionalPreparedHandoffV1")
        .next()
        .unwrap();
    let preceding_line = prefix.lines().next_back().unwrap();
    assert!(!preceding_line.contains("pub"));
    assert!(!declaration.contains("derive(Clone"));
    assert!(!declaration.contains("impl Clone"));
}
