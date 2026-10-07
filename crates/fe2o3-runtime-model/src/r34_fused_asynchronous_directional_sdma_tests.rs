use crate::{
    R34AdmissionObservationV1, R34AsyncSingleCopyModelV1, R34CustodyV1, R34DirectionV1,
    R34ExecutionObservationsV1, R34HostCertificateV1, R34LoanOutcomeV1,
    R34LowerPreparationObservationV1, R34ModelErrorV1, R34OutcomeV1, R34PublicationObservationV1,
    R34RequestBindingV1, R34RequestPreparationObservationV1, R34TerminalStageV1, R34TicketV1,
};

const SUCCESSFUL_LOAN: R34LoanOutcomeV1 = R34LoanOutcomeV1 {
    open_succeeded: true,
    retake_succeeded: true,
};

fn binding(direction: R34DirectionV1) -> R34RequestBindingV1 {
    R34RequestBindingV1 {
        queue_id: 7,
        queue_generation: 11,
        native_queue_id: 13,
        direction,
        host_offset: 64,
        device_offset: 128,
        copy_bytes: 256,
        sequence: 17,
        ticket_generation: 19,
    }
}

fn certificate(binding: R34RequestBindingV1) -> R34HostCertificateV1 {
    R34HostCertificateV1 {
        certificate_id: 23,
        queue_id: binding.queue_id,
        queue_generation: binding.queue_generation,
    }
}

fn observations(binding: R34RequestBindingV1) -> R34ExecutionObservationsV1 {
    let ticket = R34TicketV1::for_binding(binding);
    R34ExecutionObservationsV1 {
        admission: R34AdmissionObservationV1::Admitted,
        former_opening_loan: SUCCESSFUL_LOAN,
        former_execution_loan: SUCCESSFUL_LOAN,
        former_final_loan: SUCCESSFUL_LOAN,
        fused_loan: SUCCESSFUL_LOAN,
        opening_current: true,
        request_preparation: R34RequestPreparationObservationV1::Prepared,
        lower_preparation: R34LowerPreparationObservationV1::Prepared,
        lower_failure_close_current: true,
        prepublication_current: true,
        prepublication_failure_close_current: true,
        publication: R34PublicationObservationV1::Confirmed,
        planned_ticket: ticket,
        returned_ticket: ticket,
        final_current: true,
    }
}

fn model(direction: R34DirectionV1) -> R34AsyncSingleCopyModelV1 {
    let binding = binding(direction);
    R34AsyncSingleCopyModelV1::new_model_only(binding, Some(certificate(binding))).unwrap()
}

fn assert_equivalent(input: R34ExecutionObservationsV1, direction: R34DirectionV1) {
    let binding = binding(direction);
    assert!(input.loan_equivalence_premise(binding));
    let model = model(direction);
    assert!(
        model
            .run_former_model_only(input)
            .same_external_semantics(&model.run_fused_model_only(input))
    );
}

#[test]
fn successful_publication_is_equivalent_with_exact_three_to_one_loan_reduction() {
    let binding = binding(R34DirectionV1::HostToDevice);
    let input = observations(binding);
    let model = model(binding.direction);
    let former = model.run_former_model_only(input);
    let fused = model.run_fused_model_only(input);

    assert!(former.same_external_semantics(&fused));
    assert_eq!((former.loan_attempts, fused.loan_attempts), (3, 1));
    assert_eq!(
        (former.operational_checks, fused.operational_checks),
        (3, 3)
    );
    assert_eq!(
        (fused.outcome, fused.custody),
        (R34OutcomeV1::Published, R34CustodyV1::Published)
    );
    assert_eq!(fused.planned_ticket, Some(input.planned_ticket));
    assert_eq!(fused.ticket, Some(input.returned_ticket));
}

#[test]
fn admission_finishes_before_any_loan_attempt() {
    for (admission, outcome, custody) in [
        (
            R34AdmissionObservationV1::RetryableFailure,
            R34OutcomeV1::Retryable,
            R34CustodyV1::RetryableRequest,
        ),
        (
            R34AdmissionObservationV1::TerminalFailure,
            R34OutcomeV1::Terminal,
            R34CustodyV1::TerminalRequest,
        ),
    ] {
        let mut input = observations(binding(R34DirectionV1::HostToDevice));
        input.admission = admission;
        let model = model(R34DirectionV1::HostToDevice);
        for snapshot in [
            model.run_former_model_only(input),
            model.run_fused_model_only(input),
        ] {
            assert_eq!((snapshot.outcome, snapshot.custody), (outcome, custody));
            assert_eq!(snapshot.loan_attempts, 0);
            assert_eq!(snapshot.operational_checks, 0);
            assert!(!snapshot.request_constructed);
        }
    }
}

#[test]
fn opening_loan_failure_retains_unconstructed_request_custody() {
    let mut input = observations(binding(R34DirectionV1::DeviceToHost));
    input.former_opening_loan.open_succeeded = false;
    input.fused_loan.open_succeeded = false;
    let model = model(R34DirectionV1::DeviceToHost);
    let former = model.run_former_model_only(input);
    let fused = model.run_fused_model_only(input);

    assert!(former.same_external_semantics(&fused));
    assert_eq!(
        former.terminal_stage,
        Some(R34TerminalStageV1::FormerOpeningLoanOpen)
    );
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::FusedLoanOpen)
    );
    assert_eq!(former.custody, R34CustodyV1::TerminalRequest);
    assert!(!former.host_certificate_invalidated);
    assert!(!fused.request_constructed);
}

#[test]
fn opening_currentness_loss_is_terminal_before_request_construction() {
    let mut input = observations(binding(R34DirectionV1::DeviceToHost));
    input.opening_current = false;
    assert_equivalent(input, R34DirectionV1::DeviceToHost);
    let fused = model(R34DirectionV1::DeviceToHost).run_fused_model_only(input);
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::OpeningCurrentness)
    );
    assert!(!fused.request_constructed);
    assert!(!fused.host_certificate_invalidated);
}

#[test]
fn former_opening_retake_failure_precedes_request_construction() {
    let mut input = observations(binding(R34DirectionV1::DeviceToHost));
    input.former_opening_loan.retake_succeeded = false;
    let snapshot = model(R34DirectionV1::DeviceToHost).run_former_model_only(input);
    assert_eq!(snapshot.custody, R34CustodyV1::TerminalRequest);
    assert_eq!(
        snapshot.terminal_stage,
        Some(R34TerminalStageV1::FormerOpeningLoanRetake)
    );
    assert!(!snapshot.request_constructed);
    assert!(!snapshot.host_certificate_invalidated);
}

#[test]
fn every_request_preparation_rejection_is_retryable_after_successful_fused_retake() {
    for request_preparation in [
        R34RequestPreparationObservationV1::UseRequestRejected,
        R34RequestPreparationObservationV1::ReserveRejected,
        R34RequestPreparationObservationV1::PrepareRejected,
        R34RequestPreparationObservationV1::DetachRejected,
    ] {
        let mut input = observations(binding(R34DirectionV1::DeviceToHost));
        input.request_preparation = request_preparation;
        assert_equivalent(input, R34DirectionV1::DeviceToHost);
        let fused = model(R34DirectionV1::DeviceToHost).run_fused_model_only(input);
        assert_eq!(
            (fused.outcome, fused.custody),
            (R34OutcomeV1::Retryable, R34CustodyV1::RetryableRequest)
        );
        assert!(!fused.request_constructed);
        assert!(!fused.host_certificate_invalidated);
    }
}

#[test]
fn fused_retake_failure_after_request_rejection_is_terminal_request_custody() {
    let mut input = observations(binding(R34DirectionV1::HostToDevice));
    input.request_preparation = R34RequestPreparationObservationV1::DetachRejected;
    input.fused_loan.retake_succeeded = false;
    let snapshot = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
    assert_eq!(snapshot.custody, R34CustodyV1::TerminalRequest);
    assert_eq!(
        snapshot.terminal_stage,
        Some(R34TerminalStageV1::FusedLoanRetake)
    );
    assert!(!snapshot.request_constructed);
}

#[test]
fn d2h_certificate_invalidation_begins_only_after_request_construction() {
    let mut rejected = observations(binding(R34DirectionV1::DeviceToHost));
    rejected.request_preparation = R34RequestPreparationObservationV1::DetachRejected;
    let rejected = model(R34DirectionV1::DeviceToHost).run_fused_model_only(rejected);
    assert!(!rejected.host_certificate_invalidated);
    assert!(rejected.host_certificate.is_some());

    let mut detached = observations(binding(R34DirectionV1::DeviceToHost));
    detached.fused_loan.retake_succeeded = false;
    let detached = model(R34DirectionV1::DeviceToHost).run_fused_model_only(detached);
    assert!(detached.request_constructed);
    assert!(detached.host_certificate_invalidated);
    assert_eq!(detached.host_certificate, None);
    assert_eq!(detached.custody, R34CustodyV1::TerminalPublished);
}

#[test]
fn lower_retryable_failure_restores_exact_request_only_inside_current_scope() {
    let mut input = observations(binding(R34DirectionV1::HostToDevice));
    input.lower_preparation = R34LowerPreparationObservationV1::RetryableFailure;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
    assert_eq!(
        (fused.outcome, fused.custody),
        (R34OutcomeV1::Retryable, R34CustodyV1::RetryableRequest)
    );
    assert!(!fused.publication_attempted);

    input.lower_failure_close_current = false;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPrepared);
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::LowerFailureClose)
    );
}

#[test]
fn poisoned_lower_preparation_is_terminal_prepared() {
    let mut input = observations(binding(R34DirectionV1::HostToDevice));
    input.lower_preparation = R34LowerPreparationObservationV1::PoisonedFailure;
    input.former_execution_loan.retake_succeeded = false;
    input.former_final_loan.open_succeeded = false;
    input.fused_loan.retake_succeeded = false;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPrepared);
}

#[test]
fn old_execution_open_failure_can_align_only_with_prepublication_terminal_fused_path() {
    let mut input = observations(binding(R34DirectionV1::HostToDevice));
    input.former_execution_loan.open_succeeded = false;
    input.lower_preparation = R34LowerPreparationObservationV1::RetryableFailure;
    input.fused_loan.retake_succeeded = false;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    let former = model(R34DirectionV1::HostToDevice).run_former_model_only(input);
    assert_eq!(
        former.terminal_stage,
        Some(R34TerminalStageV1::FormerExecutionLoanOpen)
    );

    input.fused_loan.retake_succeeded = true;
    assert!(!input.loan_equivalence_premise(binding(R34DirectionV1::HostToDevice)));
}

#[test]
fn lower_failure_retake_stages_preserve_terminal_prepared_custody() {
    let mut former_execution = observations(binding(R34DirectionV1::HostToDevice));
    former_execution.lower_preparation = R34LowerPreparationObservationV1::RetryableFailure;
    former_execution.former_execution_loan.retake_succeeded = false;
    let former = model(R34DirectionV1::HostToDevice).run_former_model_only(former_execution);
    assert_eq!(former.custody, R34CustodyV1::TerminalPrepared);
    assert_eq!(
        former.terminal_stage,
        Some(R34TerminalStageV1::FormerExecutionLoanRetake)
    );

    let mut former_final = observations(binding(R34DirectionV1::HostToDevice));
    former_final.lower_preparation = R34LowerPreparationObservationV1::RetryableFailure;
    former_final.former_final_loan.retake_succeeded = false;
    let former = model(R34DirectionV1::HostToDevice).run_former_model_only(former_final);
    assert_eq!(former.custody, R34CustodyV1::TerminalPrepared);
    assert_eq!(
        former.terminal_stage,
        Some(R34TerminalStageV1::FormerFinalLoanRetake)
    );

    let mut fused = observations(binding(R34DirectionV1::HostToDevice));
    fused.lower_preparation = R34LowerPreparationObservationV1::RetryableFailure;
    fused.fused_loan.retake_succeeded = false;
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(fused);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPrepared);
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::FusedLoanRetake)
    );
}

#[test]
fn prepublication_loss_performs_fused_second_close_but_never_retries() {
    for (second_close, retake) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut input = observations(binding(R34DirectionV1::HostToDevice));
        input.prepublication_current = false;
        input.prepublication_failure_close_current = second_close;
        input.fused_loan.retake_succeeded = retake;
        let snapshot = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
        assert_eq!(snapshot.custody, R34CustodyV1::TerminalPrepared);
        assert!(snapshot.prepublication_failure_close_observed);
        assert_eq!(snapshot.operational_checks, 3);
        assert!(!snapshot.publication_attempted);
    }
}

#[test]
fn handoff_publishes_immediately_without_intervening_work() {
    let snapshot = model(R34DirectionV1::HostToDevice)
        .run_fused_model_only(observations(binding(R34DirectionV1::HostToDevice)));
    assert_eq!(snapshot.handoff_event_index, Some(6));
    assert_eq!(snapshot.publication_event_index, Some(7));
    assert_eq!(snapshot.fallible_actions_between_handoff_and_publication, 0);
    assert_eq!(snapshot.native_actions_between_handoff_and_publication, 0);
    assert!(snapshot.handoff_event_index < snapshot.publication_event_index);
}

#[test]
fn recoverable_publication_is_retryable_only_after_final_currentness_and_retake() {
    let mut input = observations(binding(R34DirectionV1::HostToDevice));
    input.publication = R34PublicationObservationV1::Recoverable;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
    assert_eq!(
        (fused.outcome, fused.custody),
        (R34OutcomeV1::Retryable, R34CustodyV1::RetryableRequest)
    );

    input.final_current = false;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    assert_eq!(
        model(R34DirectionV1::HostToDevice)
            .run_fused_model_only(input)
            .custody,
        R34CustodyV1::TerminalPrepared
    );
}

#[test]
fn retained_publication_is_terminal_queue_retained_regardless_of_close_or_retake() {
    let mut input = observations(binding(R34DirectionV1::HostToDevice));
    input.publication = R34PublicationObservationV1::Retained;
    input.former_execution_loan.retake_succeeded = false;
    input.former_final_loan.open_succeeded = false;
    input.fused_loan.retake_succeeded = false;
    input.final_current = false;
    assert_equivalent(input, R34DirectionV1::HostToDevice);
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(input);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPreparedQueueRetained);
    assert_eq!(fused.ticket, Some(input.returned_ticket));
}

#[test]
fn confirmed_publication_retake_failures_retain_terminal_published_custody() {
    let mut former_execution = observations(binding(R34DirectionV1::HostToDevice));
    former_execution.former_execution_loan.retake_succeeded = false;
    let former = model(R34DirectionV1::HostToDevice).run_former_model_only(former_execution);
    assert_eq!(former.custody, R34CustodyV1::TerminalPublished);
    assert_eq!(
        former.terminal_stage,
        Some(R34TerminalStageV1::FormerExecutionLoanRetake)
    );

    let mut former_final = observations(binding(R34DirectionV1::HostToDevice));
    former_final.former_final_loan.retake_succeeded = false;
    let former = model(R34DirectionV1::HostToDevice).run_former_model_only(former_final);
    assert_eq!(former.custody, R34CustodyV1::TerminalPublished);
    assert_eq!(
        former.terminal_stage,
        Some(R34TerminalStageV1::FormerFinalLoanRetake)
    );

    let mut fused = observations(binding(R34DirectionV1::HostToDevice));
    fused.fused_loan.retake_succeeded = false;
    let fused = model(R34DirectionV1::HostToDevice).run_fused_model_only(fused);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPublished);
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::FusedLoanRetake)
    );
}

#[test]
fn returned_ticket_mismatch_is_terminal_and_preserves_both_tickets() {
    let binding = binding(R34DirectionV1::HostToDevice);
    let mut input = observations(binding);
    input.returned_ticket.sequence += 1;
    assert_equivalent(input, binding.direction);
    let fused = model(binding.direction).run_fused_model_only(input);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPublished);
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::ReturnedTicketMismatch)
    );
    assert_eq!(fused.planned_ticket, Some(input.planned_ticket));
    assert_eq!(fused.ticket, Some(input.returned_ticket));
}

#[test]
fn invalid_planned_ticket_occurrence_is_terminal_published() {
    let binding = binding(R34DirectionV1::HostToDevice);
    let mut input = observations(binding);
    input.planned_ticket.native_queue_id += 1;
    input.returned_ticket = input.planned_ticket;
    assert_equivalent(input, binding.direction);
    let fused = model(binding.direction).run_fused_model_only(input);
    assert_eq!(fused.custody, R34CustodyV1::TerminalPublished);
    assert_eq!(
        fused.terminal_stage,
        Some(R34TerminalStageV1::PlannedTicketOccurrence)
    );
}

#[test]
fn h2d_certificate_is_preserved_through_publication() {
    let binding = binding(R34DirectionV1::HostToDevice);
    let snapshot = model(binding.direction).run_fused_model_only(observations(binding));
    assert_eq!(snapshot.host_certificate, Some(certificate(binding)));
    assert!(!snapshot.host_certificate_invalidated);
}

#[test]
fn equivalence_premise_is_path_sensitive_and_not_output_defined() {
    let binding = binding(R34DirectionV1::HostToDevice);
    let mut input = observations(binding);
    input.publication = R34PublicationObservationV1::Retained;
    input.former_execution_loan.retake_succeeded = false;
    input.former_final_loan.open_succeeded = false;
    input.fused_loan.retake_succeeded = false;
    assert!(input.loan_equivalence_premise(binding));

    input.publication = R34PublicationObservationV1::Recoverable;
    input.fused_loan.retake_succeeded = true;
    assert!(!input.loan_equivalence_premise(binding));

    let source = include_str!("r34_fused_asynchronous_directional_sdma.rs");
    let premise = source
        .split("pub const fn loan_equivalence_premise")
        .nth(1)
        .unwrap()
        .split("#[derive(Clone, Copy, Debug, Eq, PartialEq)]")
        .next()
        .unwrap();
    assert!(!premise.contains("run_former_model_only"));
    assert!(!premise.contains("run_fused_model_only"));
    assert!(!premise.contains("same_external_semantics"));
}

#[test]
fn omitted_request_failure_retake_premise_has_a_counterexample() {
    let binding = binding(R34DirectionV1::HostToDevice);
    let mut input = observations(binding);
    input.request_preparation = R34RequestPreparationObservationV1::ReserveRejected;
    input.fused_loan.retake_succeeded = false;
    assert!(!input.loan_equivalence_premise(binding));
    let model = model(binding.direction);
    let former = model.run_former_model_only(input);
    let fused = model.run_fused_model_only(input);
    assert_eq!(former.custody, R34CustodyV1::RetryableRequest);
    assert_eq!(fused.custody, R34CustodyV1::TerminalRequest);
    assert!(!former.same_external_semantics(&fused));
}

#[test]
fn exhaustive_finite_premise_implies_external_equivalence() {
    let binding = binding(R34DirectionV1::DeviceToHost);
    let model = model(binding.direction);
    let loan_outcomes = [
        R34LoanOutcomeV1 {
            open_succeeded: false,
            retake_succeeded: false,
        },
        R34LoanOutcomeV1 {
            open_succeeded: false,
            retake_succeeded: true,
        },
        R34LoanOutcomeV1 {
            open_succeeded: true,
            retake_succeeded: false,
        },
        SUCCESSFUL_LOAN,
    ];
    let admissions = [
        R34AdmissionObservationV1::RetryableFailure,
        R34AdmissionObservationV1::TerminalFailure,
        R34AdmissionObservationV1::Admitted,
    ];
    let request_preparations = [
        R34RequestPreparationObservationV1::UseRequestRejected,
        R34RequestPreparationObservationV1::ReserveRejected,
        R34RequestPreparationObservationV1::PrepareRejected,
        R34RequestPreparationObservationV1::DetachRejected,
        R34RequestPreparationObservationV1::Prepared,
    ];
    let lower_preparations = [
        R34LowerPreparationObservationV1::RetryableFailure,
        R34LowerPreparationObservationV1::PoisonedFailure,
        R34LowerPreparationObservationV1::Prepared,
    ];
    let publications = [
        R34PublicationObservationV1::Recoverable,
        R34PublicationObservationV1::Retained,
        R34PublicationObservationV1::Confirmed,
    ];
    let exact_ticket = R34TicketV1::for_binding(binding);
    let mut substituted_ticket = exact_ticket;
    substituted_ticket.sequence += 1;

    for admission in admissions {
        for former_opening_loan in loan_outcomes {
            for former_execution_loan in loan_outcomes {
                for former_final_loan in loan_outcomes {
                    for fused_loan in loan_outcomes {
                        for opening_current in [false, true] {
                            for request_preparation in request_preparations {
                                for lower_preparation in lower_preparations {
                                    for lower_failure_close_current in [false, true] {
                                        for prepublication_current in [false, true] {
                                            for prepublication_failure_close_current in
                                                [false, true]
                                            {
                                                for publication in publications {
                                                    for planned_ticket in
                                                        [exact_ticket, substituted_ticket]
                                                    {
                                                        for returned_ticket in [
                                                            planned_ticket,
                                                            exact_ticket,
                                                            substituted_ticket,
                                                        ] {
                                                            for final_current in [false, true] {
                                                                let input =
                                                                    R34ExecutionObservationsV1 {
                                                                        admission,
                                                                        former_opening_loan,
                                                                        former_execution_loan,
                                                                        former_final_loan,
                                                                        fused_loan,
                                                                        opening_current,
                                                                        request_preparation,
                                                                        lower_preparation,
                                                                        lower_failure_close_current,
                                                                        prepublication_current,
                                                                        prepublication_failure_close_current,
                                                                        publication,
                                                                        planned_ticket,
                                                                        returned_ticket,
                                                                        final_current,
                                                                    };
                                                                if input.loan_equivalence_premise(
                                                                    binding,
                                                                ) {
                                                                    let former = model
                                                                        .run_former_model_only(
                                                                            input,
                                                                        );
                                                                    let fused = model
                                                                        .run_fused_model_only(
                                                                            input,
                                                                        );
                                                                    assert!(
                                                                    former.same_external_semantics(
                                                                        &fused
                                                                    ),
                                                                    "premise admitted divergent input: {input:?}"
                                                                );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn constructor_rejects_invalid_binding_and_certificate() {
    let mut invalid = binding(R34DirectionV1::HostToDevice);
    invalid.copy_bytes = 0;
    assert_eq!(
        R34AsyncSingleCopyModelV1::new_model_only(invalid, None),
        Err(R34ModelErrorV1::InvalidBinding)
    );

    let binding = binding(R34DirectionV1::HostToDevice);
    let mut wrong = certificate(binding);
    wrong.queue_generation += 1;
    assert_eq!(
        R34AsyncSingleCopyModelV1::new_model_only(binding, Some(wrong)),
        Err(R34ModelErrorV1::InvalidCertificate)
    );
}

#[test]
fn prepared_handoff_carrier_is_private_and_has_no_clone_derive() {
    let source = include_str!("r34_fused_asynchronous_directional_sdma.rs");
    let carrier = source
        .split("struct R34PreparedHandoffV1")
        .nth(1)
        .unwrap()
        .split("impl R34AsyncSingleCopyModelV1")
        .next()
        .unwrap();
    assert!(!carrier.contains("pub struct"));
    assert!(!carrier.contains("derive(Clone"));
    assert!(!source.contains("impl Clone for R34PreparedHandoffV1"));
}
