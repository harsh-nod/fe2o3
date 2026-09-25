use super::*;

fn binding(direction: R33DirectionV1) -> R33RequestBindingV1 {
    R33RequestBindingV1 {
        queue_id: 11,
        queue_generation: 12,
        native_queue_id: 13,
        direction,
        host_offset: 17,
        device_offset: 23,
        copy_bytes: 4096,
        sequence: 29,
        ticket_generation: 31,
    }
}

fn certificate(binding: R33RequestBindingV1) -> R33HostCertificateV1 {
    R33HostCertificateV1 {
        certificate_id: 37,
        queue_id: binding.queue_id,
        queue_generation: binding.queue_generation,
    }
}

fn model(direction: R33DirectionV1) -> R33SynchronousExecutionModelV1 {
    let binding = binding(direction);
    R33SynchronousExecutionModelV1::new_model_only(binding, Some(certificate(binding))).unwrap()
}

fn successful_loan() -> R33LoanOutcomeV1 {
    R33LoanOutcomeV1 {
        open_succeeded: true,
        retake_succeeded: true,
    }
}

fn observations() -> R33ExecutionObservationsV1 {
    R33ExecutionObservationsV1 {
        opening_loan: successful_loan(),
        opening_current: true,
        former_submit_loan: successful_loan(),
        former_submit_close_loan: successful_loan(),
        former_wait_loan: successful_loan(),
        fused_execution_loan: successful_loan(),
        preparation: R33PreparationObservationV1::Prepared,
        prepare_failure_close_current: true,
        prepublication_current: true,
        publication: R33PublicationObservationV1::Published,
        returned_ticket: R33TicketV1::for_binding(binding(R33DirectionV1::HostToDevice)),
        former_submit_close_current: true,
        former_wait_open_current: true,
        wait: R33WaitObservationV1::Completed,
        final_current: true,
        completion_restoration_succeeded: true,
    }
}

fn equivalent(
    observations: R33ExecutionObservationsV1,
) -> (R33ExecutionSnapshotV1, R33ExecutionSnapshotV1) {
    assert!(observations.middle_currentness_aligned());
    assert!(observations.retained_loans_aligned_when_needed(binding(R33DirectionV1::HostToDevice)));
    assert!(observations.removed_loans_succeed_when_needed(binding(R33DirectionV1::HostToDevice)));
    let model = model(R33DirectionV1::HostToDevice);
    let former = model.run_former_model_only(observations);
    let fused = model.run_fused_model_only(observations);
    assert!(former.same_external_semantics(&fused));
    (former, fused)
}

#[test]
fn constructor_rejects_invalid_binding_and_certificate() {
    let mut invalid = binding(R33DirectionV1::HostToDevice);
    invalid.copy_bytes = 0;
    assert_eq!(
        R33SynchronousExecutionModelV1::new_model_only(invalid, None),
        Err(R33ModelErrorV1::InvalidBinding)
    );
    let valid = binding(R33DirectionV1::HostToDevice);
    let mut wrong = certificate(valid);
    wrong.queue_generation += 1;
    assert_eq!(
        R33SynchronousExecutionModelV1::new_model_only(valid, Some(wrong)),
        Err(R33ModelErrorV1::InvalidCertificate)
    );
}

#[test]
fn completion_is_equivalent_and_removes_two_checks_and_two_loans() {
    let (former, fused) = equivalent(observations());
    assert_eq!(former.operational_checks, 5);
    assert_eq!(fused.operational_checks, 3);
    assert_eq!(former.model_loans, 4);
    assert_eq!(fused.model_loans, 2);
    assert_eq!(fused.outcome, R33OutcomeV1::Completed);
    assert_eq!(fused.custody, R33CustodyV1::Completed);
}

#[test]
fn fused_handoff_publishes_immediately_and_waits_in_same_loan() {
    let fused = model(R33DirectionV1::HostToDevice).run_fused_model_only(observations());
    assert_eq!(fused.handoff_event_index, Some(3));
    assert_eq!(fused.publication_event_index, Some(4));
    assert_eq!(fused.wait_event_index, Some(5));
    assert_eq!(fused.fallible_actions_between_handoff_and_publication, 0);
    assert_eq!(fused.native_actions_between_handoff_and_publication, 0);
    assert!(fused.wait_inside_publication_loan);
}

#[test]
fn final_currentness_precedes_completed_record_retirement() {
    let fused = model(R33DirectionV1::HostToDevice).run_fused_model_only(observations());
    assert!(fused.final_currentness_event_index < fused.retirement_event_index);
    assert!(fused.lower_record_retired);

    let mut lost = observations();
    lost.final_current = false;
    let fused = model(R33DirectionV1::HostToDevice).run_fused_model_only(lost);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPublished);
    assert!(!fused.lower_record_retired);
    assert_eq!(fused.retirement_event_index, None);
}

#[test]
fn opening_and_execution_loan_open_failures_retain_their_exact_custody() {
    let mut input = observations();
    input.opening_current = false;
    let (former, fused) = equivalent(input);
    assert_eq!(former.custody, R33CustodyV1::TerminalRequest);
    assert_eq!(fused.terminal_stage, Some(R33TerminalStageV1::Opening));

    input = observations();
    input.opening_loan.open_succeeded = false;
    let (former, fused) = equivalent(input);
    assert_eq!(former.custody, R33CustodyV1::TerminalRequest);
    assert_eq!(
        fused.terminal_stage,
        Some(R33TerminalStageV1::OpeningLoanOpen)
    );

    input = observations();
    input.former_submit_loan.open_succeeded = false;
    input.fused_execution_loan.open_succeeded = false;
    let (former, fused) = equivalent(input);
    assert_eq!(former.custody, R33CustodyV1::TerminalPrepared);
    assert_eq!(
        fused.terminal_stage,
        Some(R33TerminalStageV1::FusedExecutionLoanOpen)
    );
}

#[test]
fn retryable_prepare_failure_restores_exact_request() {
    let mut input = observations();
    input.preparation = R33PreparationObservationV1::RetryableFailure;
    let (_, fused) = equivalent(input);
    assert_eq!(fused.outcome, R33OutcomeV1::Retryable);
    assert_eq!(fused.custody, R33CustodyV1::RetryableRequest);
    assert_eq!(fused.binding, binding(R33DirectionV1::HostToDevice));
}

#[test]
fn failed_prepare_close_or_poison_is_terminal_prepared() {
    for preparation in [
        R33PreparationObservationV1::RetryableFailure,
        R33PreparationObservationV1::PoisonedFailure,
    ] {
        let mut input = observations();
        input.preparation = preparation;
        input.prepare_failure_close_current = false;
        let (_, fused) = equivalent(input);
        assert_eq!(fused.outcome, R33OutcomeV1::Terminal);
        assert_eq!(fused.custody, R33CustodyV1::TerminalPrepared);
    }
}

#[test]
fn prepublication_loss_retains_terminal_prepared_custody() {
    let mut input = observations();
    input.prepublication_current = false;
    input.former_submit_close_current = false;
    input.former_wait_open_current = false;
    let (_, fused) = equivalent(input);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPrepared);
    assert!(!fused.publication_attempted);
}

#[test]
fn recoverable_publication_restores_request_only_after_current_close() {
    let mut input = observations();
    input.publication = R33PublicationObservationV1::Recoverable;
    let (_, fused) = equivalent(input);
    assert_eq!(fused.outcome, R33OutcomeV1::Retryable);
    assert_eq!(fused.custody, R33CustodyV1::RetryableRequest);
    assert_eq!(fused.ticket, None);
}

#[test]
fn retained_publication_is_terminal_prepared_queue_retained() {
    let mut input = observations();
    input.publication = R33PublicationObservationV1::Retained;
    input.returned_ticket.generation += 1;
    let returned = input.returned_ticket;
    let (_, fused) = equivalent(input);
    assert_eq!(fused.outcome, R33OutcomeV1::Terminal);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPreparedQueueRetained);
    assert_eq!(fused.ticket, Some(returned));
    assert!(!returned.is_exact_for(fused.binding));
}

#[test]
fn timeout_retains_exact_published_submission() {
    let mut input = observations();
    input.wait = R33WaitObservationV1::Timeout;
    let (_, fused) = equivalent(input);
    assert_eq!(fused.outcome, R33OutcomeV1::Timeout);
    assert_eq!(fused.custody, R33CustodyV1::PendingPublished);
    assert!(fused.ticket.unwrap().is_exact_for(fused.binding));
    assert!(!fused.lower_record_retired);
}

#[test]
fn lower_wait_failure_is_terminal_published() {
    let mut input = observations();
    input.wait = R33WaitObservationV1::LowerFailure;
    let (_, fused) = equivalent(input);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPublished);
    assert_eq!(fused.terminal_stage, Some(R33TerminalStageV1::LowerWait));
    assert!(!fused.lower_record_retired);
}

#[test]
fn final_currentness_loss_retains_published_record_for_every_wait_observation() {
    for wait in [
        R33WaitObservationV1::Timeout,
        R33WaitObservationV1::LowerFailure,
        R33WaitObservationV1::Completed,
    ] {
        let mut input = observations();
        input.wait = wait;
        input.final_current = false;
        let (_, fused) = equivalent(input);
        assert_eq!(fused.custody, R33CustodyV1::TerminalPublished);
        assert!(!fused.lower_record_retired);
    }
}

#[test]
fn fused_loan_retake_failure_conservatively_terminalizes_exact_stage() {
    let mut prepared = observations();
    prepared.preparation = R33PreparationObservationV1::RetryableFailure;
    prepared.fused_execution_loan.retake_succeeded = false;
    let fused = model(R33DirectionV1::HostToDevice).run_fused_model_only(prepared);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPrepared);

    let mut recoverable = observations();
    recoverable.publication = R33PublicationObservationV1::Recoverable;
    recoverable.fused_execution_loan.retake_succeeded = false;
    let fused = model(R33DirectionV1::HostToDevice).run_fused_model_only(recoverable);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPrepared);

    for (wait, custody, retired) in [
        (
            R33WaitObservationV1::Timeout,
            R33CustodyV1::TerminalPublished,
            false,
        ),
        (
            R33WaitObservationV1::Completed,
            R33CustodyV1::TerminalCompletedUnrestored,
            true,
        ),
    ] {
        let mut input = observations();
        input.wait = wait;
        input.fused_execution_loan.retake_succeeded = false;
        let fused = model(R33DirectionV1::HostToDevice).run_fused_model_only(input);
        assert_eq!(fused.custody, custody);
        assert_eq!(fused.lower_record_retired, retired);
        assert_eq!(
            fused.terminal_stage,
            Some(R33TerminalStageV1::FusedExecutionLoanRetake)
        );
    }
}

#[test]
fn completed_restoration_failure_retains_exact_completed_unrestored_custody() {
    let mut input = observations();
    input.completion_restoration_succeeded = false;
    let (former, fused) = equivalent(input);
    for snapshot in [former, fused] {
        assert_eq!(snapshot.outcome, R33OutcomeV1::Terminal);
        assert_eq!(snapshot.custody, R33CustodyV1::TerminalCompletedUnrestored);
        assert!(snapshot.lower_record_retired);
        assert!(snapshot.completion_restoration_attempted);
        assert_eq!(
            snapshot.terminal_stage,
            Some(R33TerminalStageV1::CompletionRestoration)
        );
    }
}

#[test]
fn former_removed_loan_failures_keep_stage_appropriate_custody() {
    let mut close_open = observations();
    close_open.former_submit_close_loan.open_succeeded = false;
    let former = model(R33DirectionV1::HostToDevice).run_former_model_only(close_open);
    assert_eq!(former.custody, R33CustodyV1::TerminalPublished);
    assert_eq!(
        former.terminal_stage,
        Some(R33TerminalStageV1::FormerSubmitCloseLoanOpen)
    );

    let mut wait_open = observations();
    wait_open.former_wait_loan.open_succeeded = false;
    let former = model(R33DirectionV1::HostToDevice).run_former_model_only(wait_open);
    assert_eq!(former.custody, R33CustodyV1::TerminalPublished);
    assert!(!former.wait_attempted);

    let mut wait_retake = observations();
    wait_retake.former_wait_loan.retake_succeeded = false;
    let former = model(R33DirectionV1::HostToDevice).run_former_model_only(wait_retake);
    assert_eq!(former.custody, R33CustodyV1::TerminalCompletedUnrestored);
    assert!(former.lower_record_retired);
    assert!(!former.completion_restoration_attempted);
}

#[test]
fn equivalence_premise_requires_removed_loans_only_on_distinguishing_paths() {
    let binding = binding(R33DirectionV1::HostToDevice);
    let mut completed = observations();
    completed.former_wait_loan.retake_succeeded = false;
    assert!(!completed.removed_loans_succeed_when_needed(binding));

    let mut unrestorable = completed;
    unrestorable.completion_restoration_succeeded = false;
    assert!(unrestorable.removed_loans_succeed_when_needed(binding));
    let model = model(R33DirectionV1::HostToDevice);
    assert!(
        model
            .run_former_model_only(unrestorable)
            .same_external_semantics(&model.run_fused_model_only(unrestorable))
    );

    let mut retained = observations();
    retained.publication = R33PublicationObservationV1::Retained;
    retained.former_submit_close_loan.open_succeeded = false;
    retained.former_submit_close_loan.retake_succeeded = false;
    retained.former_wait_loan.open_succeeded = false;
    retained.former_wait_loan.retake_succeeded = false;
    assert!(retained.removed_loans_succeed_when_needed(binding));
}

#[test]
fn h2d_certificate_and_all_request_coordinates_are_preserved() {
    let (_, fused) = equivalent(observations());
    let expected = binding(R33DirectionV1::HostToDevice);
    assert_eq!(fused.binding, expected);
    assert_eq!(fused.host_certificate, Some(certificate(expected)));
    assert!(!fused.host_certificate_invalidated);
    assert!(fused.ticket.unwrap().is_exact_for(expected));
}

#[test]
fn d2h_invalidates_certificate_after_admission_but_not_before_it() {
    let model = model(R33DirectionV1::DeviceToHost);
    let mut input = observations();
    input.returned_ticket = R33TicketV1::for_binding(binding(R33DirectionV1::DeviceToHost));
    let completed = model.run_fused_model_only(input);
    assert_eq!(completed.host_certificate, None);
    assert!(completed.host_certificate_invalidated);

    let mut loan_rejected = observations();
    loan_rejected.returned_ticket = R33TicketV1::for_binding(binding(R33DirectionV1::DeviceToHost));
    loan_rejected.fused_execution_loan.open_succeeded = false;
    let loan_rejected = model.run_fused_model_only(loan_rejected);
    assert_eq!(loan_rejected.custody, R33CustodyV1::TerminalPrepared);
    assert_eq!(loan_rejected.host_certificate, None);
    assert!(loan_rejected.host_certificate_invalidated);

    let mut rejected = observations();
    rejected.opening_current = false;
    let rejected = model.run_fused_model_only(rejected);
    assert_eq!(
        rejected.host_certificate,
        Some(certificate(binding(R33DirectionV1::DeviceToHost)))
    );
    assert!(!rejected.host_certificate_invalidated);
}

#[test]
fn returned_ticket_mismatch_is_terminal_without_entering_wait() {
    let mut input = observations();
    input.returned_ticket.sequence += 1;
    let returned = input.returned_ticket;
    let (former, fused) = equivalent(input);
    assert_eq!(former.custody, R33CustodyV1::TerminalPublished);
    assert_eq!(fused.custody, R33CustodyV1::TerminalPublished);
    assert_eq!(
        fused.planned_ticket,
        Some(R33TicketV1::for_binding(fused.binding))
    );
    assert_eq!(fused.ticket, Some(returned));
    assert_eq!(
        fused.terminal_stage,
        Some(R33TerminalStageV1::TicketMismatch)
    );
    assert!(!fused.wait_attempted);
    assert!(!fused.lower_record_retired);
    assert_eq!(fused.final_currentness_event_index, Some(5));
}

#[test]
fn same_device_identity_is_an_unchanged_projection() {
    let identity = R33SameDeviceIdentityV1 {
        queue_id: 41,
        queue_generation: 43,
        source_storage_id: 47,
        destination_storage_id: 53,
    };
    assert_eq!(r33_same_device_identity_projection_v1(identity), identity);
}

#[test]
fn executable_handoff_carrier_is_private_and_has_no_clone_derive() {
    let source = include_str!("r33_fused_synchronous_directional_sdma.rs");
    let declaration = source
        .split("struct R33PreparedHandoffV1")
        .next()
        .unwrap()
        .rsplit_once('\n')
        .unwrap()
        .0
        .rsplit_once('\n')
        .unwrap()
        .1;
    assert!(!declaration.contains("derive"));
    assert!(!source.contains("impl Clone for R33PreparedHandoffV1"));
    assert!(!source.contains("pub struct R33PreparedHandoffV1"));
}

#[test]
fn alignment_is_an_explicit_refinement_premise() {
    let mut input = observations();
    assert!(input.middle_currentness_aligned());
    input.former_wait_open_current = false;
    assert!(!input.middle_currentness_aligned());
    let model = model(R33DirectionV1::HostToDevice);
    assert!(
        !model
            .run_former_model_only(input)
            .same_external_semantics(&model.run_fused_model_only(input))
    );
}
