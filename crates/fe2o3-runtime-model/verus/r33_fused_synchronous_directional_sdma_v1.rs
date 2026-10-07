// Independent R33 finite model comparing former R32 submit-then-wait with a
// fused synchronous directional SDMA transition. Observations and identities
// are contracted mathematical inputs. This does not refine executable Rust,
// native queues, KFD, HSA, HIP, drivers, firmware, hardware, or performance.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum DirectionV1 { HostToDevice, DeviceToHost }

#[derive(PartialEq, Eq)]
pub enum PreparationV1 { RetryableFailure, PoisonedFailure, Prepared }

#[derive(PartialEq, Eq)]
pub enum PublicationV1 { Recoverable, Retained, Published }

#[derive(PartialEq, Eq)]
pub enum WaitV1 { Timeout, LowerFailure, Completed }

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Retryable, Timeout, Completed, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    RetryableRequest,
    PendingPublished,
    Completed,
    TerminalRequest,
    TerminalPrepared,
    TerminalPreparedQueueRetained,
    TerminalPublished,
    TerminalCompletedUnrestored,
}

#[derive(PartialEq, Eq)]
pub enum TerminalStageV1 {
    Opening,
    OpeningLoanOpen,
    OpeningLoanRetake,
    FormerSubmitLoanOpen,
    FormerSubmitLoanRetake,
    FormerSubmitCloseLoanOpen,
    FormerSubmitCloseLoanRetake,
    FormerWaitLoanOpen,
    FormerWaitLoanRetake,
    FusedExecutionLoanOpen,
    FusedExecutionLoanRetake,
    PrepareFailureClose,
    Prepublication,
    PublicationRetained,
    TicketMismatch,
    SubmitClose,
    WaitOpen,
    LowerWait,
    FinalCurrentness,
    CompletionRestoration,
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
pub struct SameDeviceIdentityV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub source_storage_id: nat,
    pub destination_storage_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct LoanOutcomeV1 {
    pub open_succeeded: bool,
    pub retake_succeeded: bool,
}

#[derive(PartialEq, Eq)]
pub struct ObservationsV1 {
    pub opening_loan: LoanOutcomeV1,
    pub opening_current: bool,
    pub former_submit_loan: LoanOutcomeV1,
    pub former_submit_close_loan: LoanOutcomeV1,
    pub former_wait_loan: LoanOutcomeV1,
    pub fused_execution_loan: LoanOutcomeV1,
    pub preparation: PreparationV1,
    pub prepare_failure_close_current: bool,
    pub prepublication_current: bool,
    pub publication: PublicationV1,
    pub returned_ticket: TicketV1,
    pub former_submit_close_current: bool,
    pub former_wait_open_current: bool,
    pub wait: WaitV1,
    pub final_current: bool,
    pub completion_restoration_succeeded: bool,
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
    pub publication_attempted: bool,
    pub wait_attempted: bool,
    pub lower_record_retired: bool,
    pub completion_restoration_attempted: bool,
    pub operational_checks: nat,
    pub model_loans: nat,
    pub handoff_event_index: Option<nat>,
    pub publication_event_index: Option<nat>,
    pub wait_event_index: Option<nat>,
    pub final_currentness_event_index: Option<nat>,
    pub retirement_event_index: Option<nat>,
    pub fallible_actions_between_handoff_and_publication: nat,
    pub native_actions_between_handoff_and_publication: nat,
    pub wait_inside_publication_loan: bool,
}

// Mathematical values are copyable; this carrier proves coordinate equality,
// not Rust move-only ownership or borrow exclusivity.
pub struct PreparedHandoffV1 {
    pub binding: BindingV1,
    pub ticket: TicketV1,
    pub host_certificate: Option<CertificateV1>,
    pub host_certificate_invalidated: bool,
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

pub open spec fn middle_currentness_aligned_v1(observations: ObservationsV1) -> bool {
    &&& observations.former_submit_close_current == observations.prepublication_current
    &&& observations.former_wait_open_current == observations.prepublication_current
}

pub open spec fn loan_succeeded_v1(loan: LoanOutcomeV1) -> bool {
    loan.open_succeeded && loan.retake_succeeded
}

pub open spec fn retained_loans_aligned_when_needed_v1(
    binding: BindingV1,
    observations: ObservationsV1,
) -> bool {
    if !loan_succeeded_v1(observations.opening_loan) || !observations.opening_current {
        true
    } else if observations.former_submit_loan.open_succeeded
        != observations.fused_execution_loan.open_succeeded
    {
        false
    } else if !observations.former_submit_loan.open_succeeded {
        true
    } else {
        match observations.preparation {
            PreparationV1::RetryableFailure if observations.prepare_failure_close_current => {
                observations.former_submit_loan.retake_succeeded
                    == observations.fused_execution_loan.retake_succeeded
            }
            PreparationV1::RetryableFailure | PreparationV1::PoisonedFailure => true,
            PreparationV1::Prepared if !observations.prepublication_current => true,
            PreparationV1::Prepared => match observations.publication {
                PublicationV1::Recoverable if observations.former_submit_close_current => {
                    observations.former_submit_loan.retake_succeeded
                        == observations.fused_execution_loan.retake_succeeded
                }
                PublicationV1::Recoverable | PublicationV1::Retained => true,
                PublicationV1::Published
                    if observations.returned_ticket == ticket_for_v1(binding) =>
                {
                    observations.former_submit_loan.retake_succeeded
                        && observations.fused_execution_loan.retake_succeeded
                }
                PublicationV1::Published => true,
            },
        }
    }
}

pub open spec fn removed_loans_succeed_when_needed_v1(
    binding: BindingV1,
    observations: ObservationsV1,
) -> bool {
    if !loan_succeeded_v1(observations.opening_loan)
        || !observations.opening_current
        || !observations.former_submit_loan.open_succeeded
        || !observations.fused_execution_loan.open_succeeded
    {
        true
    } else {
        match observations.preparation {
            PreparationV1::RetryableFailure
                if observations.prepare_failure_close_current
                    && observations.former_submit_loan.retake_succeeded
                    && observations.fused_execution_loan.retake_succeeded =>
            {
                loan_succeeded_v1(observations.former_submit_close_loan)
            }
            PreparationV1::RetryableFailure | PreparationV1::PoisonedFailure => true,
            PreparationV1::Prepared if !observations.prepublication_current => true,
            PreparationV1::Prepared => match observations.publication {
                PublicationV1::Recoverable
                    if observations.former_submit_close_current
                        && observations.former_submit_loan.retake_succeeded
                        && observations.fused_execution_loan.retake_succeeded =>
                {
                    loan_succeeded_v1(observations.former_submit_close_loan)
                }
                PublicationV1::Recoverable | PublicationV1::Retained => true,
                PublicationV1::Published
                    if observations.returned_ticket == ticket_for_v1(binding)
                        && observations.former_submit_loan.retake_succeeded
                        && observations.fused_execution_loan.retake_succeeded =>
                {
                    &&& loan_succeeded_v1(observations.former_submit_close_loan)
                    &&& observations.former_wait_loan.open_succeeded
                    &&& match observations.wait {
                        WaitV1::Timeout if observations.final_current => {
                            observations.former_wait_loan.retake_succeeded
                        }
                        WaitV1::Completed
                            if observations.final_current
                                && observations.completion_restoration_succeeded =>
                        {
                            observations.former_wait_loan.retake_succeeded
                        }
                        WaitV1::Timeout | WaitV1::LowerFailure | WaitV1::Completed => true,
                    }
                }
                PublicationV1::Published => true,
            },
        }
    }
}

pub open spec fn initial_state_v1(
    binding: BindingV1,
    certificate: Option<CertificateV1>,
    former: bool,
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
        publication_attempted: false,
        wait_attempted: false,
        lower_record_retired: false,
        completion_restoration_attempted: false,
        operational_checks: 1,
        model_loans: if former { 4 } else { 2 },
        handoff_event_index: None,
        publication_event_index: None,
        wait_event_index: None,
        final_currentness_event_index: None,
        retirement_event_index: None,
        fallible_actions_between_handoff_and_publication: 0,
        native_actions_between_handoff_and_publication: 0,
        wait_inside_publication_loan: !former,
    }
}

pub open spec fn construct_request_v1(state: StateV1) -> StateV1 {
    if state.binding.direction == DirectionV1::DeviceToHost {
        StateV1 {
            host_certificate: None,
            host_certificate_invalidated: true,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn finish_prepare_failure_v1(
    state: StateV1,
    observations: ObservationsV1,
    former: bool,
) -> StateV1 {
    let operation_retake_succeeded = if former {
        observations.former_submit_loan.retake_succeeded
    } else {
        observations.fused_execution_loan.retake_succeeded
    };
    if !operation_retake_succeeded {
        StateV1 {
            custody: CustodyV1::TerminalPrepared,
            terminal_stage: Some(if former {
                TerminalStageV1::FormerSubmitLoanRetake
            } else {
                TerminalStageV1::FusedExecutionLoanRetake
            }),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else if former && !observations.former_submit_close_loan.open_succeeded {
        StateV1 {
            custody: CustodyV1::TerminalPrepared,
            terminal_stage: Some(TerminalStageV1::FormerSubmitCloseLoanOpen),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else if former && !observations.former_submit_close_loan.retake_succeeded {
        StateV1 {
            custody: CustodyV1::TerminalPrepared,
            terminal_stage: Some(TerminalStageV1::FormerSubmitCloseLoanRetake),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else if observations.preparation == PreparationV1::RetryableFailure
        && observations.prepare_failure_close_current
    {
        StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableRequest,
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else {
        StateV1 {
            custody: CustodyV1::TerminalPrepared,
            terminal_stage: Some(TerminalStageV1::PrepareFailureClose),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    }
}

pub open spec fn make_handoff_v1(state: StateV1) -> PreparedHandoffV1 {
    PreparedHandoffV1 {
        binding: state.binding,
        ticket: ticket_for_v1(state.binding),
        host_certificate: state.host_certificate,
        host_certificate_invalidated: state.host_certificate_invalidated,
    }
}

pub open spec fn publish_handoff_v1(
    state: StateV1,
    handoff: PreparedHandoffV1,
    returned_ticket: TicketV1,
) -> StateV1 {
    StateV1 {
        binding: handoff.binding,
        planned_ticket: Some(handoff.ticket),
        ticket: Some(returned_ticket),
        host_certificate: handoff.host_certificate,
        host_certificate_invalidated: handoff.host_certificate_invalidated,
        publication_attempted: true,
        handoff_event_index: Some(3),
        publication_event_index: Some(4),
        ..state
    }
}

pub open spec fn finish_publication_failure_v1(
    state: StateV1,
    observations: ObservationsV1,
    former: bool,
) -> StateV1 {
    if observations.publication == PublicationV1::Retained {
        StateV1 {
            custody: CustodyV1::TerminalPreparedQueueRetained,
            terminal_stage: Some(TerminalStageV1::PublicationRetained),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else if observations.publication == PublicationV1::Recoverable {
        let operation_retake_succeeded = if former {
            observations.former_submit_loan.retake_succeeded
        } else {
            observations.fused_execution_loan.retake_succeeded
        };
        let close_loan_succeeded = !former
            || loan_succeeded_v1(observations.former_submit_close_loan);
        if operation_retake_succeeded
            && close_loan_succeeded
            && observations.former_submit_close_current
        {
            StateV1 {
                ticket: None,
                outcome: OutcomeV1::Retryable,
                custody: CustodyV1::RetryableRequest,
                operational_checks: state.operational_checks + 1,
                ..state
            }
        } else {
            StateV1 {
                ticket: None,
                custody: CustodyV1::TerminalPrepared,
                terminal_stage: Some(if !operation_retake_succeeded {
                    if former {
                        TerminalStageV1::FormerSubmitLoanRetake
                    } else {
                        TerminalStageV1::FusedExecutionLoanRetake
                    }
                } else if former && !observations.former_submit_close_loan.open_succeeded {
                    TerminalStageV1::FormerSubmitCloseLoanOpen
                } else if former && !observations.former_submit_close_loan.retake_succeeded {
                    TerminalStageV1::FormerSubmitCloseLoanRetake
                } else {
                    TerminalStageV1::SubmitClose
                }),
                operational_checks: state.operational_checks + 1,
                ..state
            }
        }
    } else {
        state
    }
}

pub open spec fn finish_wait_v1(
    state: StateV1,
    observations: ObservationsV1,
    fused: bool,
    loan: LoanOutcomeV1,
) -> StateV1 {
    if observations.wait == WaitV1::Timeout
        && observations.final_current
        && loan.retake_succeeded
    {
        StateV1 {
            outcome: OutcomeV1::Timeout,
            custody: CustodyV1::PendingPublished,
            wait_attempted: true,
            wait_event_index: Some(if fused { 5 } else { 8 }),
            final_currentness_event_index: Some(if fused { 6 } else { 9 }),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else if observations.wait == WaitV1::LowerFailure {
        StateV1 {
            custody: CustodyV1::TerminalPublished,
            terminal_stage: Some(if !loan.retake_succeeded {
                if fused {
                    TerminalStageV1::FusedExecutionLoanRetake
                } else {
                    TerminalStageV1::FormerWaitLoanRetake
                }
            } else if observations.final_current {
                TerminalStageV1::LowerWait
            } else {
                TerminalStageV1::FinalCurrentness
            }),
            wait_attempted: true,
            wait_event_index: Some(if fused { 5 } else { 8 }),
            final_currentness_event_index: Some(if fused { 6 } else { 9 }),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    } else if observations.wait == WaitV1::Completed && observations.final_current {
        if !loan.retake_succeeded || !observations.completion_restoration_succeeded {
            StateV1 {
                outcome: OutcomeV1::Terminal,
                custody: CustodyV1::TerminalCompletedUnrestored,
                terminal_stage: Some(if !loan.retake_succeeded {
                    if fused {
                        TerminalStageV1::FusedExecutionLoanRetake
                    } else {
                        TerminalStageV1::FormerWaitLoanRetake
                    }
                } else {
                    TerminalStageV1::CompletionRestoration
                }),
                wait_attempted: true,
                lower_record_retired: true,
                completion_restoration_attempted: loan.retake_succeeded,
                wait_event_index: Some(if fused { 5 } else { 8 }),
                final_currentness_event_index: Some(if fused { 6 } else { 9 }),
                retirement_event_index: Some(if fused { 7 } else { 10 }),
                operational_checks: state.operational_checks + 1,
                ..state
            }
        } else {
            StateV1 {
                outcome: OutcomeV1::Completed,
                custody: CustodyV1::Completed,
                wait_attempted: true,
                lower_record_retired: true,
                completion_restoration_attempted: true,
                wait_event_index: Some(if fused { 5 } else { 8 }),
                final_currentness_event_index: Some(if fused { 6 } else { 9 }),
                retirement_event_index: Some(if fused { 7 } else { 10 }),
                operational_checks: state.operational_checks + 1,
                ..state
            }
        }
    } else {
        StateV1 {
            custody: CustodyV1::TerminalPublished,
            terminal_stage: Some(if loan.retake_succeeded {
                TerminalStageV1::FinalCurrentness
            } else if fused {
                TerminalStageV1::FusedExecutionLoanRetake
            } else {
                TerminalStageV1::FormerWaitLoanRetake
            }),
            wait_attempted: true,
            wait_event_index: Some(if fused { 5 } else { 8 }),
            final_currentness_event_index: Some(if fused { 6 } else { 9 }),
            operational_checks: state.operational_checks + 1,
            ..state
        }
    }
}

pub open spec fn former_execute_v1(
    binding: BindingV1,
    certificate: Option<CertificateV1>,
    observations: ObservationsV1,
) -> StateV1 {
    let initial = initial_state_v1(binding, certificate, true);
    if !observations.opening_loan.open_succeeded {
        StateV1 { terminal_stage: Some(TerminalStageV1::OpeningLoanOpen), ..initial }
    } else if !observations.opening_current {
        StateV1 { terminal_stage: Some(TerminalStageV1::Opening), ..initial }
    } else if !observations.opening_loan.retake_succeeded {
        StateV1 { terminal_stage: Some(TerminalStageV1::OpeningLoanRetake), ..initial }
    } else {
        let requested = construct_request_v1(initial);
        if !observations.former_submit_loan.open_succeeded {
            StateV1 {
                custody: CustodyV1::TerminalPrepared,
                terminal_stage: Some(TerminalStageV1::FormerSubmitLoanOpen),
                ..requested
            }
        } else if observations.preparation != PreparationV1::Prepared {
            finish_prepare_failure_v1(requested, observations, true)
        } else {
            let prepublication = StateV1 {
                operational_checks: requested.operational_checks + 1,
                ..requested
            };
            if !observations.prepublication_current {
                StateV1 {
                    custody: CustodyV1::TerminalPrepared,
                    terminal_stage: Some(TerminalStageV1::Prepublication),
                    ..prepublication
                }
            } else {
                let planned_ticket = ticket_for_v1(binding);
                let published = StateV1 {
                    planned_ticket: Some(planned_ticket),
                    ticket: Some(observations.returned_ticket),
                    publication_attempted: true,
                    ..prepublication
                };
                if observations.publication != PublicationV1::Published {
                    finish_publication_failure_v1(published, observations, true)
                } else {
                    let submit_closed = StateV1 {
                        operational_checks: published.operational_checks + 1,
                        ..published
                    };
                    if !observations.former_submit_loan.retake_succeeded {
                        StateV1 {
                            custody: CustodyV1::TerminalPublished,
                            terminal_stage: Some(TerminalStageV1::FormerSubmitLoanRetake),
                            ..submit_closed
                        }
                    } else if !observations.former_submit_close_loan.open_succeeded {
                        StateV1 {
                            custody: CustodyV1::TerminalPublished,
                            terminal_stage: Some(TerminalStageV1::FormerSubmitCloseLoanOpen),
                            ..submit_closed
                        }
                    } else if !observations.former_submit_close_loan.retake_succeeded {
                        StateV1 {
                            custody: CustodyV1::TerminalPublished,
                            terminal_stage: Some(TerminalStageV1::FormerSubmitCloseLoanRetake),
                            ..submit_closed
                        }
                    } else if observations.returned_ticket != planned_ticket {
                        StateV1 {
                            custody: CustodyV1::TerminalPublished,
                            terminal_stage: Some(TerminalStageV1::TicketMismatch),
                            ..submit_closed
                        }
                    } else if !observations.former_submit_close_current {
                        StateV1 {
                            custody: CustodyV1::TerminalPublished,
                            terminal_stage: Some(TerminalStageV1::SubmitClose),
                            ..submit_closed
                        }
                    } else if !observations.former_wait_loan.open_succeeded {
                        StateV1 {
                            custody: CustodyV1::TerminalPublished,
                            terminal_stage: Some(TerminalStageV1::FormerWaitLoanOpen),
                            ..submit_closed
                        }
                    } else {
                        let wait_opened = StateV1 {
                            operational_checks: submit_closed.operational_checks + 1,
                            ..submit_closed
                        };
                        if !observations.former_wait_open_current {
                            StateV1 {
                                custody: CustodyV1::TerminalPublished,
                                terminal_stage: Some(TerminalStageV1::WaitOpen),
                                ..wait_opened
                            }
                        } else {
                            finish_wait_v1(
                                wait_opened, observations, false, observations.former_wait_loan)
                        }
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
    let initial = initial_state_v1(binding, certificate, false);
    if !observations.opening_loan.open_succeeded {
        StateV1 { terminal_stage: Some(TerminalStageV1::OpeningLoanOpen), ..initial }
    } else if !observations.opening_current {
        StateV1 { terminal_stage: Some(TerminalStageV1::Opening), ..initial }
    } else if !observations.opening_loan.retake_succeeded {
        StateV1 { terminal_stage: Some(TerminalStageV1::OpeningLoanRetake), ..initial }
    } else {
        let requested = construct_request_v1(initial);
        if !observations.fused_execution_loan.open_succeeded {
            StateV1 {
                custody: CustodyV1::TerminalPrepared,
                terminal_stage: Some(TerminalStageV1::FusedExecutionLoanOpen),
                ..requested
            }
        } else if observations.preparation != PreparationV1::Prepared {
            finish_prepare_failure_v1(requested, observations, false)
        } else {
            let prepublication = StateV1 {
                operational_checks: requested.operational_checks + 1,
                ..requested
            };
            if !observations.prepublication_current {
                StateV1 {
                    custody: CustodyV1::TerminalPrepared,
                    terminal_stage: Some(TerminalStageV1::Prepublication),
                    ..prepublication
                }
            } else {
                let handoff = make_handoff_v1(prepublication);
                let published = publish_handoff_v1(
                    prepublication, handoff, observations.returned_ticket);
                if observations.publication != PublicationV1::Published {
                    finish_publication_failure_v1(published, observations, false)
                } else if observations.returned_ticket != ticket_for_v1(binding) {
                    StateV1 {
                        custody: CustodyV1::TerminalPublished,
                        terminal_stage: Some(TerminalStageV1::TicketMismatch),
                        operational_checks: published.operational_checks + 1,
                        final_currentness_event_index: Some(5),
                        ..published
                    }
                } else {
                    finish_wait_v1(
                        published, observations, true, observations.fused_execution_loan)
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
    &&& former.publication_attempted == fused.publication_attempted
    &&& former.wait_attempted == fused.wait_attempted
    &&& former.lower_record_retired == fused.lower_record_retired
}

pub open spec fn equivalence_premise_v1(
    binding: BindingV1,
    observations: ObservationsV1,
) -> bool {
    &&& middle_currentness_aligned_v1(observations)
    &&& retained_loans_aligned_when_needed_v1(binding, observations)
    &&& removed_loans_succeed_when_needed_v1(binding, observations)
}

pub open spec fn same_device_identity_projection_v1(
    identity: SameDeviceIdentityV1,
) -> SameDeviceIdentityV1 {
    identity
}

pub proof fn generated_ticket_is_exact_v1(binding: BindingV1)
    ensures ticket_exact_v1(ticket_for_v1(binding), binding),
{}

pub proof fn opening_loss_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires !observations.opening_current,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn opening_loan_failure_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires !loan_succeeded_v1(observations.opening_loan),
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn fused_execution_loan_open_failure_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        !observations.fused_execution_loan.open_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPrepared,
        !fused_execute_v1(binding, certificate, observations).publication_attempted,
        !fused_execute_v1(binding, certificate, observations).wait_attempted,
{}

pub proof fn retryable_prepare_failure_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::RetryableFailure,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn poisoned_prepare_failure_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::PoisonedFailure,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn prepublication_loss_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::Prepared,
        !observations.prepublication_current,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn recoverable_publication_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::Prepared,
        observations.prepublication_current,
        observations.publication == PublicationV1::Recoverable,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn retained_publication_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::Prepared,
        observations.prepublication_current,
        observations.publication == PublicationV1::Retained,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn timeout_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Published,
        observations.wait == WaitV1::Timeout,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn lower_wait_failure_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Published,
        observations.wait == WaitV1::LowerFailure,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn completion_is_equivalent_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
        observations.preparation == PreparationV1::Prepared,
        observations.prepublication_current, observations.publication == PublicationV1::Published,
        observations.wait == WaitV1::Completed,
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn aligned_compositions_have_equal_external_semantics_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires equivalence_premise_v1(binding, observations),
    ensures external_semantics_equal_v1(
        former_execute_v1(binding, certificate, observations),
        fused_execute_v1(binding, certificate, observations)),
{}

pub proof fn former_completed_success_has_five_checks_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.former_submit_loan),
        loan_succeeded_v1(observations.former_submit_close_loan),
        observations.former_wait_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.former_submit_close_current, observations.former_wait_open_current,
        observations.wait == WaitV1::Completed, observations.final_current,
    ensures former_execute_v1(binding, certificate, observations).operational_checks == 5,
{}

pub proof fn fused_completed_success_has_three_checks_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Completed, observations.final_current,
    ensures fused_execute_v1(binding, certificate, observations).operational_checks == 3,
{}

pub proof fn fused_removes_exactly_two_successful_checks_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.former_submit_loan),
        loan_succeeded_v1(observations.former_submit_close_loan),
        observations.former_wait_loan.open_succeeded,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.former_submit_close_current, observations.former_wait_open_current,
        observations.wait == WaitV1::Completed, observations.final_current,
    ensures former_execute_v1(binding, certificate, observations).operational_checks
        == fused_execute_v1(binding, certificate, observations).operational_checks + 2,
{}

pub proof fn former_composition_has_four_model_loans_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures former_execute_v1(binding, certificate, observations).model_loans == 4,
{}

pub proof fn fused_composition_has_two_model_loans_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures fused_execute_v1(binding, certificate, observations).model_loans == 2,
{}

pub proof fn fused_removes_exactly_two_model_loans_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures former_execute_v1(binding, certificate, observations).model_loans
        == fused_execute_v1(binding, certificate, observations).model_loans + 2,
{}

pub proof fn successful_handoff_publishes_immediately_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations).handoff_event_index == Some(3),
        fused_execute_v1(binding, certificate, observations).publication_event_index == Some(4),
{}

pub proof fn handoff_has_no_intervening_fallible_action_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations)
            .fallible_actions_between_handoff_and_publication == 0,
{}

pub proof fn handoff_has_no_intervening_native_action_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
    ensures fused_execute_v1(binding, certificate, observations)
            .native_actions_between_handoff_and_publication == 0,
{}

pub proof fn fused_wait_stays_inside_publication_loan_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures fused_execute_v1(binding, certificate, observations).wait_inside_publication_loan,
{}

pub proof fn final_currentness_precedes_retirement_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Completed, observations.final_current,
    ensures fused_execute_v1(binding, certificate, observations).final_currentness_event_index
            == Some(6),
        fused_execute_v1(binding, certificate, observations).retirement_event_index == Some(7),
        fused_execute_v1(binding, certificate, observations).lower_record_retired,
{}

pub proof fn final_currentness_loss_does_not_retire_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Completed, !observations.final_current,
    ensures !fused_execute_v1(binding, certificate, observations).lower_record_retired,
        fused_execute_v1(binding, certificate, observations).retirement_event_index.is_none(),
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPublished,
{}

pub proof fn timeout_retains_exact_published_custody_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.fused_execution_loan),
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Timeout, observations.final_current,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Timeout,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::PendingPublished,
        fused_execute_v1(binding, certificate, observations).ticket == Some(ticket_for_v1(binding)),
{}

pub proof fn lower_wait_failure_retains_published_custody_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::LowerFailure,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPublished,
{}

pub proof fn completion_returns_completed_custody_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.fused_execution_loan),
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Completed, observations.final_current,
        observations.completion_restoration_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Completed,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::Completed,
{}

pub proof fn timeout_retake_failure_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Timeout, observations.final_current,
        !observations.fused_execution_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPublished,
{}

pub proof fn completion_retake_failure_is_completed_unrestored_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Completed, observations.final_current,
        !observations.fused_execution_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalCompletedUnrestored,
        fused_execute_v1(binding, certificate, observations).lower_record_retired,
        !fused_execute_v1(binding, certificate, observations).completion_restoration_attempted,
{}

pub proof fn completion_restoration_failure_is_completed_unrestored_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.fused_execution_loan),
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.wait == WaitV1::Completed, observations.final_current,
        !observations.completion_restoration_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalCompletedUnrestored,
        fused_execute_v1(binding, certificate, observations).lower_record_retired,
        fused_execute_v1(binding, certificate, observations).completion_restoration_attempted,
{}

pub proof fn completed_result_requires_successful_restoration_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Completed,
    ensures observations.completion_restoration_succeeded,
        fused_execute_v1(binding, certificate, observations).custody == CustodyV1::Completed,
        fused_execute_v1(binding, certificate, observations).lower_record_retired,
        fused_execute_v1(binding, certificate, observations).completion_restoration_attempted,
{}

pub proof fn removed_wait_retake_is_not_required_for_unrestorable_completion_v1(
    binding: BindingV1, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.former_submit_loan),
        loan_succeeded_v1(observations.former_submit_close_loan),
        loan_succeeded_v1(observations.fused_execution_loan),
        observations.former_wait_loan.open_succeeded,
        !observations.former_wait_loan.retake_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
        observations.former_submit_close_current, observations.former_wait_open_current,
        observations.wait == WaitV1::Completed, observations.final_current,
        !observations.completion_restoration_succeeded,
    ensures removed_loans_succeed_when_needed_v1(binding, observations),
        external_semantics_equal_v1(
            former_execute_v1(binding, None, observations),
            fused_execute_v1(binding, None, observations)),
{}

pub proof fn prepare_retake_failure_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::RetryableFailure,
        observations.prepare_failure_close_current,
        !observations.fused_execution_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPrepared,
{}

pub proof fn recoverable_publication_retake_failure_is_terminal_prepared_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Recoverable,
        observations.former_submit_close_current,
        !observations.fused_execution_loan.retake_succeeded,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPrepared,
{}

pub proof fn request_binding_is_preserved_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    ensures fused_execute_v1(binding, certificate, observations).binding == binding,
{}

pub proof fn confirmed_exact_returned_ticket_preserves_all_identity_coordinates_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket == ticket_for_v1(binding),
    ensures fused_execute_v1(binding, certificate, observations).planned_ticket
            == Some(ticket_for_v1(binding)),
        fused_execute_v1(binding, certificate, observations).ticket
            == Some(observations.returned_ticket),
        ticket_exact_v1(fused_execute_v1(binding, certificate, observations).ticket.unwrap(), binding),
{}

pub proof fn confirmed_ticket_mismatch_is_terminal_published_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Published,
        observations.returned_ticket != ticket_for_v1(binding),
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Terminal,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPublished,
        fused_execute_v1(binding, certificate, observations).planned_ticket
            == Some(ticket_for_v1(binding)),
        fused_execute_v1(binding, certificate, observations).ticket
            == Some(observations.returned_ticket),
        !fused_execute_v1(binding, certificate, observations).wait_attempted,
        !fused_execute_v1(binding, certificate, observations).lower_record_retired,
        fused_execute_v1(binding, certificate, observations).final_currentness_event_index
            == Some(5),
{}

pub proof fn h2d_certificate_is_preserved_v1(
    binding: BindingV1, certificate: CertificateV1, observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::HostToDevice,
    ensures fused_execute_v1(binding, Some(certificate), observations).host_certificate
            == Some(certificate),
        !fused_execute_v1(binding, Some(certificate), observations).host_certificate_invalidated,
{}

pub proof fn d2h_certificate_is_invalidated_after_admission_v1(
    binding: BindingV1, certificate: CertificateV1, observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::DeviceToHost,
        loan_succeeded_v1(observations.opening_loan), observations.opening_current,
    ensures fused_execute_v1(binding, Some(certificate), observations).host_certificate.is_none(),
        fused_execute_v1(binding, Some(certificate), observations).host_certificate_invalidated,
{}

pub proof fn d2h_opening_loss_preserves_prior_certificate_v1(
    binding: BindingV1, certificate: CertificateV1, observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::DeviceToHost,
        !loan_succeeded_v1(observations.opening_loan) || !observations.opening_current,
    ensures fused_execute_v1(binding, Some(certificate), observations).host_certificate
            == Some(certificate),
        !fused_execute_v1(binding, Some(certificate), observations).host_certificate_invalidated,
{}

pub proof fn retryable_publication_clears_ticket_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        loan_succeeded_v1(observations.fused_execution_loan),
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Recoverable,
        observations.former_submit_close_current,
    ensures fused_execute_v1(binding, certificate, observations).outcome == OutcomeV1::Retryable,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::RetryableRequest,
        fused_execute_v1(binding, certificate, observations).ticket.is_none(),
{}

pub proof fn retained_publication_keeps_returned_ticket_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, observations.prepublication_current,
        observations.publication == PublicationV1::Retained,
    ensures fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPreparedQueueRetained,
        fused_execute_v1(binding, certificate, observations).planned_ticket
            == Some(ticket_for_v1(binding)),
        fused_execute_v1(binding, certificate, observations).ticket
            == Some(observations.returned_ticket),
{}

pub proof fn prepublication_loss_attempts_no_publication_v1(
    binding: BindingV1, certificate: Option<CertificateV1>, observations: ObservationsV1,
)
    requires loan_succeeded_v1(observations.opening_loan), observations.opening_current,
        observations.fused_execution_loan.open_succeeded,
        observations.preparation == PreparationV1::Prepared, !observations.prepublication_current,
    ensures !fused_execute_v1(binding, certificate, observations).publication_attempted,
        fused_execute_v1(binding, certificate, observations).custody
            == CustodyV1::TerminalPrepared,
{}

pub proof fn same_device_identity_is_unchanged_v1(identity: SameDeviceIdentityV1)
    ensures same_device_identity_projection_v1(identity) == identity,
{}

}
