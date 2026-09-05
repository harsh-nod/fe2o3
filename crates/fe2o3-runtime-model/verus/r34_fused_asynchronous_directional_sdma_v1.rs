// Independent R34 finite model comparing the former three-loan public
// asynchronous single-copy submission with the fused one-loan composition.
// Observations are contracted mathematical inputs. This does not refine Rust,
// KFD, HSA, HIP, drivers, firmware, hardware, or performance.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum DirectionV1 { HostToDevice, DeviceToHost }

#[derive(PartialEq, Eq)]
pub enum AdmissionV1 { RetryableFailure, TerminalFailure, Admitted }

#[derive(PartialEq, Eq)]
pub enum RequestPreparationV1 {
    UseRequestRejected,
    ReserveRejected,
    PrepareRejected,
    DetachRejected,
    Prepared,
}

#[derive(PartialEq, Eq)]
pub enum LowerPreparationV1 { RetryableFailure, PoisonedFailure, Prepared }

#[derive(PartialEq, Eq)]
pub enum PublicationV1 { Recoverable, Retained, Confirmed }

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Retryable, Published, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    RetryableRequest,
    Published,
    TerminalRequest,
    TerminalPrepared,
    TerminalPreparedQueueRetained,
    TerminalPublished,
}

#[derive(PartialEq, Eq)]
pub enum TerminalStageV1 {
    Admission,
    FormerOpeningLoanOpen,
    FormerOpeningLoanRetake,
    FusedLoanOpen,
    FusedLoanRetake,
    OpeningCurrentness,
    FormerExecutionLoanOpen,
    FormerExecutionLoanRetake,
    FormerFinalLoanOpen,
    FormerFinalLoanRetake,
    LowerPreparation,
    LowerFailureClose,
    Prepublication,
    PublicationRetained,
    FinalCurrentness,
    PlannedTicketOccurrence,
    ReturnedTicketMismatch,
}

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub native_queue_id: nat,
    pub direction: DirectionV1,
    pub host_offset: nat,
    pub device_offset: nat,
    pub copy_bytes: nat,
    pub sequence: nat,
    pub ticket_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub native_queue_id: nat,
    pub direction: DirectionV1,
    pub sequence: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct CertificateV1 {
    pub certificate_id: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct LoanOutcomeV1 {
    pub open_succeeded: bool,
    pub retake_succeeded: bool,
}

#[derive(PartialEq, Eq)]
pub struct ObservationsV1 {
    pub admission: AdmissionV1,
    pub former_opening_loan: LoanOutcomeV1,
    pub former_execution_loan: LoanOutcomeV1,
    pub former_final_loan: LoanOutcomeV1,
    pub fused_loan: LoanOutcomeV1,
    pub opening_current: bool,
    pub request_preparation: RequestPreparationV1,
    pub lower_preparation: LowerPreparationV1,
    pub lower_failure_close_current: bool,
    pub prepublication_current: bool,
    pub prepublication_failure_close_current: bool,
    pub publication: PublicationV1,
    pub planned_ticket: TicketV1,
    pub returned_ticket: TicketV1,
    pub final_current: bool,
}

pub struct StateV1 {
    pub binding: BindingV1,
    pub planned_ticket: Option<TicketV1>,
    pub ticket: Option<TicketV1>,
    pub host_certificate: Option<CertificateV1>,
    pub host_certificate_invalidated: bool,
    pub outcome: OutcomeV1,
    pub custody: CustodyV1,
    pub terminal_stage: Option<TerminalStageV1>,
    pub request_constructed: bool,
    pub publication_attempted: bool,
    pub operational_checks: nat,
    pub loan_attempts: nat,
    pub admission_event_index: nat,
    pub request_event_index: Option<nat>,
    pub handoff_event_index: Option<nat>,
    pub publication_event_index: Option<nat>,
    pub final_currentness_event_index: Option<nat>,
    pub fallible_actions_between_handoff_and_publication: nat,
    pub native_actions_between_handoff_and_publication: nat,
    pub prepublication_failure_close_observed: bool,
}

// Mathematical values are copyable; this proves coordinate equality, not Rust
// move-only ownership or borrow exclusivity.
pub struct PreparedHandoffV1 {
    pub binding: BindingV1,
    pub planned_ticket: TicketV1,
    pub certificate: Option<CertificateV1>,
    pub certificate_invalidated: bool,
}

pub open spec fn valid_binding_v1(binding: BindingV1) -> bool {
    &&& binding.queue_id > 0
    &&& binding.queue_generation > 0
    &&& binding.native_queue_id > 0
    &&& binding.copy_bytes > 0
    &&& binding.sequence > 0
    &&& binding.ticket_generation > 0
}

pub open spec fn ticket_for_v1(binding: BindingV1) -> TicketV1 {
    TicketV1 {
        queue_id: binding.queue_id,
        queue_generation: binding.queue_generation,
        native_queue_id: binding.native_queue_id,
        direction: binding.direction,
        sequence: binding.sequence,
        generation: binding.ticket_generation,
    }
}

pub open spec fn ticket_exact_v1(ticket: TicketV1, binding: BindingV1) -> bool {
    &&& ticket.queue_id == binding.queue_id
    &&& ticket.queue_generation == binding.queue_generation
    &&& ticket.native_queue_id == binding.native_queue_id
    &&& ticket.direction == binding.direction
    &&& ticket.sequence == binding.sequence
    &&& ticket.generation == binding.ticket_generation
}

pub open spec fn certificate_exact_v1(certificate: CertificateV1, binding: BindingV1) -> bool {
    &&& certificate.certificate_id > 0
    &&& certificate.queue_id == binding.queue_id
    &&& certificate.queue_generation == binding.queue_generation
}

pub open spec fn loan_succeeded_v1(loan: LoanOutcomeV1) -> bool {
    loan.open_succeeded && loan.retake_succeeded
}

pub open spec fn admission_succeeded_v1(admission: AdmissionV1) -> bool {
    admission == AdmissionV1::Admitted
}

pub open spec fn request_prepared_v1(preparation: RequestPreparationV1) -> bool {
    preparation == RequestPreparationV1::Prepared
}

/// This premise is stated entirely over inputs. It does not invoke a runner or
/// compare output states, and therefore is not circular.
pub open spec fn loan_equivalence_premise_v1(
    binding: BindingV1,
    observations: ObservationsV1,
) -> bool {
    if !admission_succeeded_v1(observations.admission) || !observations.opening_current {
        true
    } else if observations.former_opening_loan.open_succeeded
        != observations.fused_loan.open_succeeded
    {
        false
    } else if !observations.former_opening_loan.open_succeeded {
        true
    } else if !observations.former_opening_loan.retake_succeeded {
        !request_prepared_v1(observations.request_preparation)
            && !observations.fused_loan.retake_succeeded
    } else if !request_prepared_v1(observations.request_preparation) {
        observations.fused_loan.retake_succeeded
    } else if !observations.former_execution_loan.open_succeeded {
        match observations.lower_preparation {
            LowerPreparationV1::RetryableFailure => {
                !observations.lower_failure_close_current
                    || !observations.fused_loan.retake_succeeded
            }
            LowerPreparationV1::PoisonedFailure => true,
            LowerPreparationV1::Prepared => false,
        }
    } else {
        match observations.lower_preparation {
            LowerPreparationV1::RetryableFailure => {
                !observations.lower_failure_close_current
                    || (observations.former_execution_loan.retake_succeeded
                        && loan_succeeded_v1(observations.former_final_loan))
                        == observations.fused_loan.retake_succeeded
            }
            LowerPreparationV1::PoisonedFailure => true,
            LowerPreparationV1::Prepared if !observations.prepublication_current => true,
            LowerPreparationV1::Prepared => match observations.publication {
                PublicationV1::Retained => true,
                PublicationV1::Recoverable => {
                    !observations.final_current
                        || (observations.former_execution_loan.retake_succeeded
                            && loan_succeeded_v1(observations.former_final_loan))
                            == observations.fused_loan.retake_succeeded
                }
                PublicationV1::Confirmed => {
                    !observations.final_current
                        || !ticket_exact_v1(observations.planned_ticket, binding)
                        || observations.returned_ticket != observations.planned_ticket
                        || (observations.former_execution_loan.retake_succeeded
                            && loan_succeeded_v1(observations.former_final_loan))
                            == observations.fused_loan.retake_succeeded
                }
            },
        }
    }
}

pub open spec fn initial_state_v1(
    binding: BindingV1,
    certificate: Option<CertificateV1>,
) -> StateV1 {
    StateV1 {
        binding,
        planned_ticket: None,
        ticket: None,
        host_certificate: certificate,
        host_certificate_invalidated: false,
        outcome: OutcomeV1::Terminal,
        custody: CustodyV1::TerminalRequest,
        terminal_stage: None,
        request_constructed: false,
        publication_attempted: false,
        operational_checks: 0,
        loan_attempts: 0,
        admission_event_index: 1,
        request_event_index: None,
        handoff_event_index: None,
        publication_event_index: None,
        final_currentness_event_index: None,
        fallible_actions_between_handoff_and_publication: 0,
        native_actions_between_handoff_and_publication: 0,
        prepublication_failure_close_observed: false,
    }
}

pub open spec fn construct_request_v1(state: StateV1) -> StateV1 {
    if state.binding.direction == DirectionV1::DeviceToHost {
        StateV1 {
            host_certificate: None,
            host_certificate_invalidated: true,
            request_constructed: true,
            request_event_index: Some(3),
            ..state
        }
    } else {
        StateV1 {
            request_constructed: true,
            request_event_index: Some(3),
            ..state
        }
    }
}

pub open spec fn install_planned_ticket_v1(
    state: StateV1,
    observations: ObservationsV1,
) -> StateV1 {
    StateV1 { planned_ticket: Some(observations.planned_ticket), ..state }
}

pub open spec fn make_handoff_v1(
    state: StateV1,
    observations: ObservationsV1,
) -> PreparedHandoffV1 {
    PreparedHandoffV1 {
        binding: state.binding,
        planned_ticket: observations.planned_ticket,
        certificate: state.host_certificate,
        certificate_invalidated: state.host_certificate_invalidated,
    }
}

pub open spec fn publish_handoff_v1(
    state: StateV1,
    handoff: PreparedHandoffV1,
    observations: ObservationsV1,
) -> StateV1 {
    StateV1 {
        binding: handoff.binding,
        planned_ticket: Some(handoff.planned_ticket),
        ticket: if observations.publication == PublicationV1::Recoverable {
            None
        } else {
            Some(observations.returned_ticket)
        },
        host_certificate: handoff.certificate,
        host_certificate_invalidated: handoff.certificate_invalidated,
        publication_attempted: true,
        handoff_event_index: Some(6),
        publication_event_index: Some(7),
        ..state
    }
}

pub open spec fn former_final_close_v1(
    state: StateV1,
    observations: ObservationsV1,
    lower_failure: bool,
) -> StateV1 {
    StateV1 {
        loan_attempts: state.loan_attempts + 1,
        operational_checks: state.operational_checks
            + if observations.former_final_loan.open_succeeded { 1nat } else { 0nat },
        final_currentness_event_index:
            if !lower_failure && observations.former_final_loan.open_succeeded {
                Some(8)
            } else {
                state.final_currentness_event_index
            },
        ..state
    }
}

pub open spec fn former_final_close_succeeded_v1(
    observations: ObservationsV1,
    lower_failure: bool,
) -> bool {
    &&& observations.former_final_loan.open_succeeded
    &&& if lower_failure {
        observations.lower_failure_close_current
    } else {
        observations.final_current
    }
    &&& observations.former_final_loan.retake_succeeded
}

pub open spec fn former_post_operation_stage_v1(
    observations: ObservationsV1,
    lower_failure: bool,
) -> TerminalStageV1 {
    if !observations.former_execution_loan.retake_succeeded {
        TerminalStageV1::FormerExecutionLoanRetake
    } else if !observations.former_final_loan.open_succeeded {
        TerminalStageV1::FormerFinalLoanOpen
    } else if !observations.former_final_loan.retake_succeeded {
        TerminalStageV1::FormerFinalLoanRetake
    } else if lower_failure && !observations.lower_failure_close_current {
        TerminalStageV1::LowerFailureClose
    } else if lower_failure {
        TerminalStageV1::LowerPreparation
    } else {
        TerminalStageV1::FinalCurrentness
    }
}

pub open spec fn publication_terminal_stage_v1(
    binding: BindingV1,
    observations: ObservationsV1,
    operation_succeeded: bool,
    closing_succeeded: bool,
    fused: bool,
) -> TerminalStageV1 {
    if !operation_succeeded {
        if fused { TerminalStageV1::FusedLoanRetake }
        else { TerminalStageV1::FormerExecutionLoanRetake }
    } else if !closing_succeeded {
        if fused && !observations.fused_loan.retake_succeeded {
            TerminalStageV1::FusedLoanRetake
        } else if !fused && !observations.former_final_loan.open_succeeded {
            TerminalStageV1::FormerFinalLoanOpen
        } else if !fused && !observations.former_final_loan.retake_succeeded {
            TerminalStageV1::FormerFinalLoanRetake
        } else {
            TerminalStageV1::FinalCurrentness
        }
    } else if !ticket_exact_v1(observations.planned_ticket, binding) {
        TerminalStageV1::PlannedTicketOccurrence
    } else {
        TerminalStageV1::ReturnedTicketMismatch
    }
}

pub open spec fn finish_publication_v1(
    state: StateV1,
    observations: ObservationsV1,
    operation_succeeded: bool,
    closing_succeeded: bool,
    fused: bool,
) -> StateV1 {
    match observations.publication {
        PublicationV1::Retained => StateV1 {
            custody: CustodyV1::TerminalPreparedQueueRetained,
            terminal_stage: Some(TerminalStageV1::PublicationRetained),
            ..state
        },
        PublicationV1::Recoverable if operation_succeeded && closing_succeeded => StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableRequest,
            ..state
        },
        PublicationV1::Recoverable => StateV1 {
            custody: CustodyV1::TerminalPrepared,
            terminal_stage: Some(publication_terminal_stage_v1(
                state.binding, observations, operation_succeeded, closing_succeeded, fused)),
            ..state
        },
        PublicationV1::Confirmed
            if operation_succeeded
                && closing_succeeded
                && ticket_exact_v1(observations.planned_ticket, state.binding)
                && observations.returned_ticket == observations.planned_ticket => StateV1 {
            outcome: OutcomeV1::Published,
            custody: CustodyV1::Published,
            ..state
        },
        PublicationV1::Confirmed => StateV1 {
            custody: CustodyV1::TerminalPublished,
            terminal_stage: Some(publication_terminal_stage_v1(
                state.binding, observations, operation_succeeded, closing_succeeded, fused)),
            ..state
        },
    }
}

pub open spec fn former_execute_v1(
    binding: BindingV1,
    certificate: Option<CertificateV1>,
    observations: ObservationsV1,
) -> StateV1 {
    let initial = initial_state_v1(binding, certificate);
    if observations.admission == AdmissionV1::RetryableFailure {
        StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableRequest,
            ..initial
        }
    } else if observations.admission == AdmissionV1::TerminalFailure {
        StateV1 { terminal_stage: Some(TerminalStageV1::Admission), ..initial }
    } else {
        let opening_attempted = StateV1 { loan_attempts: 1, ..initial };
        if !observations.former_opening_loan.open_succeeded {
            StateV1 {
                terminal_stage: Some(TerminalStageV1::FormerOpeningLoanOpen),
                ..opening_attempted
            }
        } else {
            let opening = StateV1 { operational_checks: 1, ..opening_attempted };
            if !observations.opening_current {
                StateV1 {
                    terminal_stage: Some(
                        if observations.former_opening_loan.retake_succeeded {
                            TerminalStageV1::OpeningCurrentness
                        } else {
                            TerminalStageV1::FormerOpeningLoanRetake
                        }),
                    ..opening
                }
            } else if !observations.former_opening_loan.retake_succeeded {
                StateV1 {
                    terminal_stage: Some(TerminalStageV1::FormerOpeningLoanRetake),
                    ..opening
                }
            } else if !request_prepared_v1(observations.request_preparation) {
                StateV1 {
                    outcome: OutcomeV1::Retryable,
                    custody: CustodyV1::RetryableRequest,
                    ..opening
                }
            } else {
                let requested = construct_request_v1(opening);
                let operation = StateV1 {
                    loan_attempts: requested.loan_attempts + 1,
                    ..requested
                };
                if !observations.former_execution_loan.open_succeeded {
                    let closed = former_final_close_v1(operation, observations, true);
                    StateV1 {
                        custody: CustodyV1::TerminalPrepared,
                        terminal_stage: Some(TerminalStageV1::FormerExecutionLoanOpen),
                        ..closed
                    }
                } else if observations.lower_preparation != LowerPreparationV1::Prepared {
                    let closed = former_final_close_v1(operation, observations, true);
                    if observations.lower_preparation == LowerPreparationV1::RetryableFailure
                        && observations.former_execution_loan.retake_succeeded
                        && former_final_close_succeeded_v1(observations, true)
                    {
                        StateV1 {
                            outcome: OutcomeV1::Retryable,
                            custody: CustodyV1::RetryableRequest,
                            ..closed
                        }
                    } else {
                        StateV1 {
                            custody: CustodyV1::TerminalPrepared,
                            terminal_stage: Some(
                                former_post_operation_stage_v1(observations, true)),
                            ..closed
                        }
                    }
                } else {
                    let planned = install_planned_ticket_v1(operation, observations);
                    let prepublication = StateV1 {
                        operational_checks: planned.operational_checks + 1,
                        ..planned
                    };
                    if !observations.prepublication_current {
                        StateV1 {
                            custody: CustodyV1::TerminalPrepared,
                            terminal_stage: Some(
                                if observations.former_execution_loan.retake_succeeded {
                                    TerminalStageV1::Prepublication
                                } else {
                                    TerminalStageV1::FormerExecutionLoanRetake
                                }),
                            ..prepublication
                        }
                    } else {
                        let handoff = make_handoff_v1(prepublication, observations);
                        let published = publish_handoff_v1(prepublication, handoff, observations);
                        let closed = former_final_close_v1(published, observations, false);
                        finish_publication_v1(
                            closed,
                            observations,
                            observations.former_execution_loan.retake_succeeded,
                            former_final_close_succeeded_v1(observations, false),
                            false,
                        )
                    }
                }
            }
        }
    }
}

pub open spec fn fused_execute_v1(
    binding: BindingV1,
    certificate: Option<CertificateV1>,
    observations: ObservationsV1,
) -> StateV1 {
    let initial = initial_state_v1(binding, certificate);
    if observations.admission == AdmissionV1::RetryableFailure {
        StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableRequest,
            ..initial
        }
    } else if observations.admission == AdmissionV1::TerminalFailure {
        StateV1 { terminal_stage: Some(TerminalStageV1::Admission), ..initial }
    } else {
        let attempted = StateV1 { loan_attempts: 1, ..initial };
        if !observations.fused_loan.open_succeeded {
            StateV1 {
                terminal_stage: Some(TerminalStageV1::FusedLoanOpen),
                ..attempted
            }
        } else {
            let opening = StateV1 { operational_checks: 1, ..attempted };
            if !observations.opening_current {
                StateV1 {
                    terminal_stage: Some(
                        if observations.fused_loan.retake_succeeded {
                            TerminalStageV1::OpeningCurrentness
                        } else {
                            TerminalStageV1::FusedLoanRetake
                        }),
                    ..opening
                }
            } else if !request_prepared_v1(observations.request_preparation) {
                if observations.fused_loan.retake_succeeded {
                    StateV1 {
                        outcome: OutcomeV1::Retryable,
                        custody: CustodyV1::RetryableRequest,
                        ..opening
                    }
                } else {
                    StateV1 {
                        terminal_stage: Some(TerminalStageV1::FusedLoanRetake),
                        ..opening
                    }
                }
            } else {
                let requested = construct_request_v1(opening);
                if observations.lower_preparation != LowerPreparationV1::Prepared {
                    let closed = StateV1 {
                        operational_checks: requested.operational_checks + 1,
                        ..requested
                    };
                    if observations.lower_preparation == LowerPreparationV1::RetryableFailure
                        && observations.lower_failure_close_current
                        && observations.fused_loan.retake_succeeded
                    {
                        StateV1 {
                            outcome: OutcomeV1::Retryable,
                            custody: CustodyV1::RetryableRequest,
                            ..closed
                        }
                    } else {
                        StateV1 {
                            custody: CustodyV1::TerminalPrepared,
                            terminal_stage: Some(
                                if !observations.fused_loan.retake_succeeded {
                                    TerminalStageV1::FusedLoanRetake
                                } else if !observations.lower_failure_close_current {
                                    TerminalStageV1::LowerFailureClose
                                } else {
                                    TerminalStageV1::LowerPreparation
                                }),
                            ..closed
                        }
                    }
                } else {
                    let planned = install_planned_ticket_v1(requested, observations);
                    let prepublication = StateV1 {
                        operational_checks: planned.operational_checks + 1,
                        ..planned
                    };
                    if !observations.prepublication_current {
                        StateV1 {
                            custody: CustodyV1::TerminalPrepared,
                            terminal_stage: Some(
                                if observations.fused_loan.retake_succeeded {
                                    TerminalStageV1::Prepublication
                                } else {
                                    TerminalStageV1::FusedLoanRetake
                                }),
                            operational_checks: prepublication.operational_checks + 1,
                            prepublication_failure_close_observed: true,
                            ..prepublication
                        }
                    } else {
                        let handoff = make_handoff_v1(prepublication, observations);
                        let published = publish_handoff_v1(prepublication, handoff, observations);
                        let closed = StateV1 {
                            operational_checks: published.operational_checks + 1,
                            final_currentness_event_index: Some(8),
                            ..published
                        };
                        finish_publication_v1(
                            closed,
                            observations,
                            observations.fused_loan.retake_succeeded,
                            observations.final_current
                                && observations.fused_loan.retake_succeeded,
                            true,
                        )
                    }
                }
            }
        }
    }
}

pub open spec fn external_semantics_equal_v1(former: StateV1, fused: StateV1) -> bool {
    &&& former.binding == fused.binding
    &&& former.planned_ticket == fused.planned_ticket
    &&& former.ticket == fused.ticket
    &&& former.host_certificate == fused.host_certificate
    &&& former.host_certificate_invalidated == fused.host_certificate_invalidated
    &&& former.outcome == fused.outcome
    &&& former.custody == fused.custody
    &&& former.request_constructed == fused.request_constructed
    &&& former.publication_attempted == fused.publication_attempted
}

pub proof fn generated_ticket_is_exact_v1(binding: BindingV1)
    ensures ticket_exact_v1(ticket_for_v1(binding), binding),
{}

pub proof fn admission_retryable_preserves_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::RetryableFailure,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Retryable,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::RetryableRequest,
        fused_execute_v1(binding, certificate, observations).loan_attempts == 0,
{}

pub proof fn admission_terminal_preserves_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::TerminalFailure,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalRequest,
        fused_execute_v1(binding, certificate, observations).loan_attempts == 0,
{}

pub proof fn admission_precedes_every_loan_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission != AdmissionV1::Admitted,
    ensures former_execute_v1(binding, certificate, observations).loan_attempts == 0,
        fused_execute_v1(binding, certificate, observations).loan_attempts == 0,
{}

pub proof fn paired_open_failure_is_externally_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        !observations.former_opening_loan.open_succeeded,
        !observations.fused_loan.open_succeeded,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn opening_currentness_loss_is_externally_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_equivalence_premise_v1(binding, observations),
        !observations.opening_current,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn former_opening_retake_failure_precedes_construction_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.former_opening_loan.open_succeeded,
        observations.opening_current,
        !observations.former_opening_loan.retake_succeeded,
    ensures former_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalRequest,
        !former_execute_v1(binding, certificate, observations).request_constructed,
        former_execute_v1(binding, certificate, observations).host_certificate == certificate,
{}

pub proof fn fused_opening_retake_failure_after_request_rejection_is_terminal_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        !request_prepared_v1(observations.request_preparation),
        !observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalRequest,
        !fused_execute_v1(binding, certificate, observations).request_constructed,
{}

pub proof fn request_preparation_rejection_is_retryable_after_retake_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        !request_prepared_v1(observations.request_preparation),
        observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Retryable,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::RetryableRequest,
{}

pub proof fn retry_after_detach_is_forbidden_on_fused_retake_failure_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        !observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome != OutcomeV1::Retryable,
        fused_execute_v1(binding, certificate, observations).custody != CustodyV1::RetryableRequest,
        fused_execute_v1(binding, certificate, observations).request_constructed,
{}

pub proof fn rejected_detach_has_not_constructed_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        observations.request_preparation == RequestPreparationV1::DetachRejected,
    ensures !fused_execute_v1(binding, certificate, observations).request_constructed,
{}

pub proof fn successful_detach_constructs_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        observations.request_preparation == RequestPreparationV1::Prepared,
    ensures fused_execute_v1(binding, certificate, observations).request_constructed,
        fused_execute_v1(binding, certificate, observations).request_event_index == Some(3),
{}

pub proof fn d2h_certificate_invalidates_only_after_constructed_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::DeviceToHost,
        observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        observations.request_preparation == RequestPreparationV1::Prepared,
    ensures fused_execute_v1(binding, certificate, observations).host_certificate.is_none(),
        fused_execute_v1(binding, certificate, observations).host_certificate_invalidated,
{}

pub proof fn d2h_rejection_preserves_prior_certificate_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::DeviceToHost,
        observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        observations.request_preparation != RequestPreparationV1::Prepared,
    ensures fused_execute_v1(binding, certificate, observations).host_certificate == certificate,
        !fused_execute_v1(binding, certificate, observations).host_certificate_invalidated,
{}

pub proof fn h2d_certificate_is_preserved_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::HostToDevice,
    ensures fused_execute_v1(binding, certificate, observations).host_certificate == certificate,
        !fused_execute_v1(binding, certificate, observations).host_certificate_invalidated,
{}

pub proof fn lower_retryable_failure_restores_request_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::RetryableFailure,
        observations.lower_failure_close_current, observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Retryable,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::RetryableRequest,
{}

pub proof fn lower_failure_close_loss_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation != LowerPreparationV1::Prepared,
        !observations.lower_failure_close_current,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
{}

pub proof fn lower_poisoned_failure_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::PoisonedFailure,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
{}

pub proof fn former_execution_open_failure_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        !observations.former_execution_loan.open_succeeded,
    ensures former_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
        !former_execute_v1(binding, certificate, observations).publication_attempted,
{}

pub proof fn lower_failure_fused_retake_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation != LowerPreparationV1::Prepared,
        !observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
        fused_execute_v1(binding, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FusedLoanRetake),
{}

pub proof fn lower_failure_former_execution_retake_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.lower_preparation != LowerPreparationV1::Prepared,
        !observations.former_execution_loan.retake_succeeded,
    ensures former_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
        former_execute_v1(binding, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FormerExecutionLoanRetake),
{}

pub proof fn lower_failure_former_final_retake_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.former_execution_loan.retake_succeeded,
        observations.lower_preparation != LowerPreparationV1::Prepared,
        observations.former_final_loan.open_succeeded,
        !observations.former_final_loan.retake_succeeded,
    ensures former_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
        former_execute_v1(binding, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FormerFinalLoanRetake),
{}

pub proof fn prepublication_loss_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        !observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
        !fused_execute_v1(binding, certificate, observations).publication_attempted,
{}

pub proof fn prepublication_loss_performs_second_close_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        !observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations).prepublication_failure_close_observed,
        fused_execute_v1(binding, certificate, observations).operational_checks == 3,
{}

pub proof fn prepublication_second_close_cannot_make_retryable_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        !observations.prepublication_current,
        observations.prepublication_failure_close_current,
        observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
{}

pub proof fn recoverable_publication_is_retryable_inside_final_scope_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Recoverable,
        observations.final_current, observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Retryable,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::RetryableRequest,
{}

pub proof fn recoverable_publication_retake_failure_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Recoverable,
        !observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPrepared,
{}

pub proof fn retained_publication_is_terminal_queue_retained_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Retained,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPreparedQueueRetained,
        fused_execute_v1(binding, certificate, observations).ticket
            == Some(observations.returned_ticket),
{}

pub proof fn retained_publication_ignores_retake_and_final_currentness_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Retained,
        !observations.fused_loan.retake_succeeded, !observations.final_current,
    ensures fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPreparedQueueRetained,
{}

pub proof fn confirmed_exact_publication_succeeds_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        observations.final_current, observations.fused_loan.retake_succeeded,
        ticket_exact_v1(observations.planned_ticket, binding),
        observations.returned_ticket == observations.planned_ticket,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Published,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::Published,
{}

pub proof fn invalid_planned_occurrence_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        !ticket_exact_v1(observations.planned_ticket, binding),
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPublished,
{}

pub proof fn returned_ticket_mismatch_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        observations.returned_ticket != observations.planned_ticket,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPublished,
        fused_execute_v1(binding, certificate, observations).planned_ticket
            == Some(observations.planned_ticket),
        fused_execute_v1(binding, certificate, observations).ticket
            == Some(observations.returned_ticket),
{}

pub proof fn confirmed_fused_retake_failure_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        !observations.fused_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPublished,
        fused_execute_v1(binding, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FusedLoanRetake),
{}

pub proof fn confirmed_former_execution_retake_failure_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        !observations.former_execution_loan.retake_succeeded,
    ensures former_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPublished,
        former_execute_v1(binding, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FormerExecutionLoanRetake),
{}

pub proof fn confirmed_former_final_retake_failure_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.former_execution_loan.retake_succeeded,
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        observations.former_final_loan.open_succeeded,
        !observations.former_final_loan.retake_succeeded,
    ensures former_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPublished,
        former_execute_v1(binding, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FormerFinalLoanRetake),
{}

pub proof fn final_currentness_loss_is_terminal_after_publication_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        !observations.final_current,
    ensures fused_execute_v1(binding, certificate, observations).custody == CustodyV1::TerminalPublished,
        fused_execute_v1(binding, certificate, observations).publication_attempted,
{}

pub proof fn premised_former_and_fused_are_externally_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_equivalence_premise_v1(binding, observations),
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn successful_former_uses_three_loans_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        loan_succeeded_v1(observations.former_execution_loan),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        loan_succeeded_v1(observations.former_final_loan), observations.final_current,
        ticket_exact_v1(observations.planned_ticket, binding),
        observations.returned_ticket == observations.planned_ticket,
    ensures former_execute_v1(binding, certificate, observations).loan_attempts == 3,
        former_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Published,
{}

pub proof fn successful_fused_uses_one_loan_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.fused_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        observations.final_current, ticket_exact_v1(observations.planned_ticket, binding),
        observations.returned_ticket == observations.planned_ticket,
    ensures fused_execute_v1(binding, certificate, observations).loan_attempts == 1,
        fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Published,
{}

pub proof fn successful_fusion_removes_exactly_two_loans_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan),
        loan_succeeded_v1(observations.former_execution_loan),
        loan_succeeded_v1(observations.former_final_loan),
        loan_succeeded_v1(observations.fused_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Confirmed,
        observations.final_current, ticket_exact_v1(observations.planned_ticket, binding),
        observations.returned_ticket == observations.planned_ticket,
    ensures former_execute_v1(binding, certificate, observations).loan_attempts
        == fused_execute_v1(binding, certificate, observations).loan_attempts + 2,
{}

pub proof fn successful_former_has_three_currentness_checks_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan), observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current,
        observations.former_final_loan.open_succeeded,
    ensures former_execute_v1(binding, certificate, observations).operational_checks == 3,
{}

pub proof fn successful_fused_has_three_currentness_checks_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations).operational_checks == 3,
{}

pub proof fn handoff_is_immediately_followed_by_publication_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.fused_loan.open_succeeded, observations.opening_current,
        request_prepared_v1(observations.request_preparation),
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations).handoff_event_index == Some(6),
        fused_execute_v1(binding, certificate, observations).publication_event_index == Some(7),
{}

pub proof fn no_fallible_work_between_handoff_and_publication_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures fused_execute_v1(binding, certificate, observations)
        .fallible_actions_between_handoff_and_publication == 0,
{}

pub proof fn no_native_work_between_handoff_and_publication_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures fused_execute_v1(binding, certificate, observations)
        .native_actions_between_handoff_and_publication == 0,
{}

pub proof fn request_construction_precedes_handoff_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires fused_execute_v1(binding, certificate, observations).publication_attempted,
    ensures fused_execute_v1(binding, certificate, observations).request_event_index == Some(3),
        fused_execute_v1(binding, certificate, observations).handoff_event_index == Some(6),
{}

pub proof fn final_currentness_follows_publication_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires fused_execute_v1(binding, certificate, observations).publication_attempted,
    ensures fused_execute_v1(binding, certificate, observations).publication_event_index == Some(7),
        fused_execute_v1(binding, certificate, observations).final_currentness_event_index == Some(8),
{}

pub proof fn binding_is_preserved_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures fused_execute_v1(binding, certificate, observations).binding == binding,
{}

pub proof fn successful_publication_preserves_both_ticket_values_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires fused_execute_v1(binding, certificate, observations).publication_attempted,
    ensures fused_execute_v1(binding, certificate, observations).planned_ticket
            == Some(observations.planned_ticket),
        if observations.publication == PublicationV1::Recoverable {
            fused_execute_v1(binding, certificate, observations).ticket.is_none()
        } else {
            fused_execute_v1(binding, certificate, observations).ticket
                == Some(observations.returned_ticket)
        },
{}

pub proof fn published_result_has_exact_ticket_occurrence_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Published,
    ensures ticket_exact_v1(observations.planned_ticket, binding),
        observations.returned_ticket == observations.planned_ticket,
        fused_execute_v1(binding, certificate, observations).ticket
            == Some(observations.returned_ticket),
{}

pub proof fn request_rejection_equivalence_requires_fused_retake_v1(
    binding: BindingV1, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.former_opening_loan.open_succeeded,
        observations.former_opening_loan.retake_succeeded,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        !request_prepared_v1(observations.request_preparation),
        loan_equivalence_premise_v1(binding, observations),
    ensures observations.fused_loan.retake_succeeded,
{}

pub proof fn retained_path_needs_no_retake_alignment_v1(
    binding: BindingV1, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan),
        observations.fused_loan.open_succeeded,
        observations.opening_current, request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.lower_preparation == LowerPreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Retained,
    ensures loan_equivalence_premise_v1(binding, observations),
{}

pub proof fn prepublication_loss_needs_no_retake_alignment_v1(
    binding: BindingV1, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        loan_succeeded_v1(observations.former_opening_loan),
        observations.fused_loan.open_succeeded,
        observations.opening_current, request_prepared_v1(observations.request_preparation),
        observations.former_execution_loan.open_succeeded,
        observations.lower_preparation == LowerPreparationV1::Prepared,
        !observations.prepublication_current,
    ensures loan_equivalence_premise_v1(binding, observations),
{}

pub proof fn former_opening_retake_asymmetry_is_excluded_after_detach_v1(
    binding: BindingV1, observations: ObservationsV1,
)
    requires observations.admission == AdmissionV1::Admitted,
        observations.former_opening_loan.open_succeeded,
        observations.fused_loan.open_succeeded,
        observations.opening_current,
        !observations.former_opening_loan.retake_succeeded,
        request_prepared_v1(observations.request_preparation),
    ensures !loan_equivalence_premise_v1(binding, observations),
{}

}
