// Independent R32 model of the directional SDMA prepare-close/publication-open
// currentness handoff. Caller-supplied observations and identities are
// contracted mathematical inputs. This does not refine executable Rust, KFD,
// HSA, HIP, driver, firmware, hardware, coherence, DMA visibility, or timing.
use vstd::prelude::*;

verus! {

pub open spec fn max_directional_packets_v1() -> nat { 64 }

#[derive(PartialEq, Eq)]
pub enum DirectionV1 { HostToDevice, DeviceToHost }

#[derive(PartialEq, Eq)]
pub enum PreparationObservationV1 {
    RetryableFailure,
    PoisonedFailure,
    RosterMismatch,
    Prepared,
}

#[derive(PartialEq, Eq)]
pub enum PublicationObservationV1 { Recoverable, Retained, Published }

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Retryable, Published, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    RetryableRequest,
    PublishedRoster,
    TerminalRequest,
    TerminalPrepared,
    TerminalPublished,
}

#[derive(PartialEq, Eq)]
pub enum TerminalStageV1 {
    Opening,
    PrepareFailureClose,
    PrepareSuccessClose,
    SharedCloseOpen,
    PublicationRetained,
    FinalClose,
}

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub native_queue_id: nat,
    pub direction: DirectionV1,
    pub packet_count: nat,
}

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub native_queue_id: nat,
    pub direction: DirectionV1,
    pub occurrence: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct CertificateV1 {
    pub certificate_id: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct ObservationsV1 {
    pub opening_current: bool,
    pub preparation: PreparationObservationV1,
    pub prepare_failure_close_current: bool,
    pub shared_current: bool,
    pub publication: PublicationObservationV1,
    pub final_close_current: bool,
}

pub struct StateV1 {
    pub binding: BindingV1,
    pub roster: Seq<TicketV1>,
    pub host_certificate: Option<CertificateV1>,
    pub host_certificate_invalidated: bool,
    pub outcome: OutcomeV1,
    pub custody: CustodyV1,
    pub terminal_stage: Option<TerminalStageV1>,
    pub directional_checks: nat,
    pub queue_checks: nat,
    pub prepare_failure_close_observed: bool,
    pub shared_close_open_observed: bool,
    pub publication_attempted: bool,
    pub final_close_observed: bool,
    pub fallible_actions_between_shared_and_publication: nat,
    pub native_actions_between_shared_and_publication: nat,
    pub shared_event_index: Option<nat>,
    pub publication_event_index: Option<nat>,
}

// Private, non-duplicated proof carrier in the abstract transition. It binds
// every coordinate moved directly from successful shared observation to publish.
pub struct PreparedHandoffV1 {
    pub binding: BindingV1,
    pub roster: Seq<TicketV1>,
    pub host_certificate: Option<CertificateV1>,
    pub host_certificate_invalidated: bool,
}

pub open spec fn valid_binding_v1(binding: BindingV1) -> bool {
    &&& binding.queue_id > 0
    &&& binding.queue_generation > 0
    &&& binding.native_queue_id > 0
    &&& 0 < binding.packet_count <= max_directional_packets_v1()
}

pub open spec fn ticket_exact_v1(
    ticket: TicketV1,
    binding: BindingV1,
    occurrence: nat,
) -> bool {
    &&& ticket.queue_id == binding.queue_id
    &&& ticket.queue_generation == binding.queue_generation
    &&& ticket.native_queue_id == binding.native_queue_id
    &&& ticket.direction == binding.direction
    &&& ticket.occurrence == occurrence
    &&& ticket.generation > 0
}

pub open spec fn roster_exact_v1(roster: Seq<TicketV1>, binding: BindingV1) -> bool {
    &&& roster.len() == binding.packet_count
    &&& forall|index: int| 0 <= index < roster.len() ==>
        ticket_exact_v1(roster[index], binding, index as nat)
}

pub open spec fn certificate_exact_v1(
    certificate: CertificateV1,
    binding: BindingV1,
) -> bool {
    &&& certificate.certificate_id > 0
    &&& certificate.queue_id == binding.queue_id
    &&& certificate.queue_generation == binding.queue_generation
}

pub open spec fn valid_input_v1(
    binding: BindingV1,
    roster: Seq<TicketV1>,
    certificate: Option<CertificateV1>,
) -> bool {
    &&& valid_binding_v1(binding)
    &&& roster_exact_v1(roster, binding)
    &&& certificate.is_some() ==> certificate_exact_v1(certificate.unwrap(), binding)
}

pub open spec fn initial_state_v1(
    binding: BindingV1,
    roster: Seq<TicketV1>,
    certificate: Option<CertificateV1>,
) -> StateV1 {
    StateV1 {
        binding,
        roster,
        host_certificate: if binding.direction == DirectionV1::HostToDevice {
            certificate
        } else {
            None
        },
        host_certificate_invalidated: binding.direction == DirectionV1::DeviceToHost,
        outcome: OutcomeV1::Terminal,
        custody: CustodyV1::TerminalRequest,
        terminal_stage: None,
        directional_checks: 0,
        queue_checks: 0,
        prepare_failure_close_observed: false,
        shared_close_open_observed: false,
        publication_attempted: false,
        final_close_observed: false,
        fallible_actions_between_shared_and_publication: 0,
        native_actions_between_shared_and_publication: 0,
        shared_event_index: None,
        publication_event_index: None,
    }
}

pub open spec fn with_opening_check_v1(state: StateV1) -> StateV1 {
    StateV1 { directional_checks: 1, ..state }
}

pub open spec fn opening_terminal_v1(state: StateV1) -> StateV1 {
    StateV1 { terminal_stage: Some(TerminalStageV1::Opening), ..state }
}

pub open spec fn finish_prepare_failure_v1(
    state: StateV1,
    observations: ObservationsV1,
) -> StateV1 {
    if observations.preparation == PreparationObservationV1::RetryableFailure
        && observations.prepare_failure_close_current
    {
        StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableRequest,
            directional_checks: state.directional_checks + 1,
            prepare_failure_close_observed: true,
            ..state
        }
    } else {
        StateV1 {
            terminal_stage: Some(TerminalStageV1::PrepareFailureClose),
            directional_checks: state.directional_checks + 1,
            prepare_failure_close_observed: true,
            ..state
        }
    }
}

pub open spec fn finish_publication_v1(
    state: StateV1,
    publication: PublicationObservationV1,
    final_close_current: bool,
) -> StateV1 {
    if publication == PublicationObservationV1::Recoverable && final_close_current {
        StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableRequest,
            directional_checks: state.directional_checks + 1,
            final_close_observed: true,
            ..state
        }
    } else if publication == PublicationObservationV1::Retained {
        StateV1 {
            custody: CustodyV1::TerminalPublished,
            terminal_stage: Some(TerminalStageV1::PublicationRetained),
            directional_checks: state.directional_checks + 1,
            final_close_observed: true,
            ..state
        }
    } else if publication == PublicationObservationV1::Published && final_close_current {
        StateV1 {
            outcome: OutcomeV1::Published,
            custody: CustodyV1::PublishedRoster,
            directional_checks: state.directional_checks + 1,
            final_close_observed: true,
            ..state
        }
    } else if publication == PublicationObservationV1::Recoverable {
        StateV1 {
            custody: CustodyV1::TerminalPrepared,
            terminal_stage: Some(TerminalStageV1::FinalClose),
            directional_checks: state.directional_checks + 1,
            final_close_observed: true,
            ..state
        }
    } else {
        StateV1 {
            custody: CustodyV1::TerminalPublished,
            terminal_stage: Some(TerminalStageV1::FinalClose),
            directional_checks: state.directional_checks + 1,
            final_close_observed: true,
            ..state
        }
    }
}

pub open spec fn make_handoff_v1(state: StateV1) -> PreparedHandoffV1 {
    PreparedHandoffV1 {
        binding: state.binding,
        roster: state.roster,
        host_certificate: state.host_certificate,
        host_certificate_invalidated: state.host_certificate_invalidated,
    }
}

pub open spec fn publish_handoff_v1(state: StateV1, handoff: PreparedHandoffV1) -> StateV1 {
    StateV1 {
        binding: handoff.binding,
        roster: handoff.roster,
        host_certificate: handoff.host_certificate,
        host_certificate_invalidated: handoff.host_certificate_invalidated,
        publication_attempted: true,
        fallible_actions_between_shared_and_publication: 0,
        native_actions_between_shared_and_publication: 0,
        publication_event_index: Some(3),
        ..state
    }
}

/// Reference lifecycle: opening, separate successful-prepare close, separate
/// publication open, and final close are four operational observations.
pub open spec fn old_submit_v1(
    binding: BindingV1,
    roster: Seq<TicketV1>,
    certificate: Option<CertificateV1>,
    observations: ObservationsV1,
) -> StateV1 {
    let opened = with_opening_check_v1(initial_state_v1(binding, roster, certificate));
    if !observations.opening_current {
        opening_terminal_v1(opened)
    } else if observations.preparation != PreparationObservationV1::Prepared {
        finish_prepare_failure_v1(opened, observations)
    } else {
        let prepare_closed = StateV1 {
            directional_checks: opened.directional_checks + 1,
            ..opened
        };
        if !observations.shared_current {
            StateV1 {
                custody: CustodyV1::TerminalPrepared,
                terminal_stage: Some(TerminalStageV1::PrepareSuccessClose),
                ..prepare_closed
            }
        } else {
            let publication_opened = StateV1 {
                queue_checks: prepare_closed.queue_checks + 1,
                publication_attempted: true,
                publication_event_index: Some(4),
                ..prepare_closed
            };
            finish_publication_v1(
                publication_opened,
                observations.publication,
                observations.final_close_current,
            )
        }
    }
}

/// Shared lifecycle: opening, one shared prepare-close/publication-open
/// observation, and final close are three operational observations.
pub open spec fn shared_submit_v1(
    binding: BindingV1,
    roster: Seq<TicketV1>,
    certificate: Option<CertificateV1>,
    observations: ObservationsV1,
) -> StateV1 {
    let opened = with_opening_check_v1(initial_state_v1(binding, roster, certificate));
    if !observations.opening_current {
        opening_terminal_v1(opened)
    } else if observations.preparation != PreparationObservationV1::Prepared {
        finish_prepare_failure_v1(opened, observations)
    } else {
        let shared = StateV1 {
            queue_checks: opened.queue_checks + 1,
            shared_close_open_observed: true,
            shared_event_index: Some(2),
            ..opened
        };
        if !observations.shared_current {
            StateV1 {
                custody: CustodyV1::TerminalPrepared,
                terminal_stage: Some(TerminalStageV1::SharedCloseOpen),
                ..shared
            }
        } else {
            let handoff = make_handoff_v1(shared);
            finish_publication_v1(
                publish_handoff_v1(shared, handoff),
                observations.publication,
                observations.final_close_current,
            )
        }
    }
}

pub open spec fn external_semantics_equal_v1(old: StateV1, shared: StateV1) -> bool {
    &&& old.binding == shared.binding
    &&& old.roster == shared.roster
    &&& old.host_certificate == shared.host_certificate
    &&& old.host_certificate_invalidated == shared.host_certificate_invalidated
    &&& old.outcome == shared.outcome
    &&& old.custody == shared.custody
    &&& old.publication_attempted == shared.publication_attempted
}

pub open spec fn handoff_binding_v1(handoff: PreparedHandoffV1) -> BindingV1 {
    handoff.binding
}

pub open spec fn handoff_roster_v1(handoff: PreparedHandoffV1) -> Seq<TicketV1> {
    handoff.roster
}

pub open spec fn handoff_certificate_v1(
    handoff: PreparedHandoffV1,
) -> Option<CertificateV1> {
    handoff.host_certificate
}

pub open spec fn handoff_certificate_invalidated_v1(handoff: PreparedHandoffV1) -> bool {
    handoff.host_certificate_invalidated
}

#[derive(PartialEq, Eq)]
pub struct SameDeviceStateV1 {
    pub source_queue_id: nat,
    pub destination_queue_id: nat,
    pub packet_count: nat,
    pub operational_checks: nat,
    pub publication_count: nat,
}

pub open spec fn same_device_unchanged_v1(state: SameDeviceStateV1) -> SameDeviceStateV1 {
    state
}

pub proof fn exact_roster_has_bound_length_v1(
    roster: Seq<TicketV1>, binding: BindingV1,
)
    requires valid_binding_v1(binding), roster_exact_v1(roster, binding),
    ensures roster.len() == binding.packet_count,
        0 < roster.len() <= max_directional_packets_v1(),
{}

pub proof fn exact_roster_ticket_binds_every_coordinate_v1(
    roster: Seq<TicketV1>, binding: BindingV1, index: int,
)
    requires roster_exact_v1(roster, binding), 0 <= index < roster.len(),
    ensures ticket_exact_v1(roster[index], binding, index as nat),
{}

pub proof fn handoff_binds_queue_and_native_queue_v1(state: StateV1)
    ensures handoff_binding_v1(make_handoff_v1(state)).queue_id == state.binding.queue_id,
        handoff_binding_v1(make_handoff_v1(state)).queue_generation
            == state.binding.queue_generation,
        handoff_binding_v1(make_handoff_v1(state)).native_queue_id
            == state.binding.native_queue_id,
{}

pub proof fn handoff_binds_direction_and_packet_roster_v1(state: StateV1)
    ensures handoff_binding_v1(make_handoff_v1(state)).direction == state.binding.direction,
        handoff_binding_v1(make_handoff_v1(state)).packet_count == state.binding.packet_count,
        handoff_roster_v1(make_handoff_v1(state)) == state.roster,
{}

pub proof fn handoff_binds_certificate_state_v1(state: StateV1)
    ensures handoff_certificate_v1(make_handoff_v1(state)) == state.host_certificate,
        handoff_certificate_invalidated_v1(make_handoff_v1(state))
            == state.host_certificate_invalidated,
{}

pub proof fn shared_refines_old_external_semantics_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    ensures external_semantics_equal_v1(
        old_submit_v1(binding, roster, certificate, observations),
        shared_submit_v1(binding, roster, certificate, observations),
    ),
{}

pub proof fn opening_failure_retains_request_without_prepare_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires !observations.opening_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalRequest,
        shared_submit_v1(binding, roster, certificate, observations).directional_checks == 1,
        !shared_submit_v1(binding, roster, certificate, observations).publication_attempted,
{}

pub proof fn prepare_failure_always_executes_old_close_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation != PreparationObservationV1::Prepared,
    ensures shared_submit_v1(binding, roster, certificate, observations)
            .prepare_failure_close_observed,
        shared_submit_v1(binding, roster, certificate, observations).directional_checks == 2,
        !shared_submit_v1(binding, roster, certificate, observations)
            .shared_close_open_observed,
        !shared_submit_v1(binding, roster, certificate, observations).publication_attempted,
{}

pub proof fn old_prepare_failure_also_executes_close_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation != PreparationObservationV1::Prepared,
    ensures old_submit_v1(binding, roster, certificate, observations)
            .prepare_failure_close_observed,
        old_submit_v1(binding, roster, certificate, observations).directional_checks == 2,
{}

pub proof fn retryable_prepare_failure_has_exact_custody_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::RetryableFailure,
        observations.prepare_failure_close_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Retryable,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::RetryableRequest,
{}

pub proof fn failed_prepare_close_is_terminal_request_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation != PreparationObservationV1::Prepared,
        !observations.prepare_failure_close_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalRequest,
{}

pub proof fn poisoned_prepare_failure_is_terminal_request_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::PoisonedFailure,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalRequest,
{}

pub proof fn roster_mismatch_is_terminal_request_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::RosterMismatch,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalRequest,
{}

pub proof fn every_prepare_failure_refines_external_custody_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation != PreparationObservationV1::Prepared,
    ensures external_semantics_equal_v1(
        old_submit_v1(binding, roster, certificate, observations),
        shared_submit_v1(binding, roster, certificate, observations),
    ),
{}

pub proof fn shared_failure_is_terminal_prepared_custody_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        !observations.shared_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalPrepared,
        shared_submit_v1(binding, roster, certificate, observations).terminal_stage
            == Some(TerminalStageV1::SharedCloseOpen),
{}

pub proof fn shared_failure_never_attempts_publication_or_final_close_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        !observations.shared_current,
    ensures shared_submit_v1(binding, roster, certificate, observations)
            .shared_close_open_observed,
        !shared_submit_v1(binding, roster, certificate, observations).publication_attempted,
        !shared_submit_v1(binding, roster, certificate, observations).final_close_observed,
{}

pub proof fn shared_failure_has_two_checks_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        !observations.shared_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).directional_checks == 1,
        shared_submit_v1(binding, roster, certificate, observations).queue_checks == 1,
{}

pub proof fn old_success_has_four_operational_checks_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
    ensures old_submit_v1(binding, roster, certificate, observations).directional_checks == 3,
        old_submit_v1(binding, roster, certificate, observations).queue_checks == 1,
{}

pub proof fn shared_success_has_three_operational_checks_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).directional_checks == 2,
        shared_submit_v1(binding, roster, certificate, observations).queue_checks == 1,
{}

pub proof fn successful_handoff_removes_exactly_one_check_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
    ensures old_submit_v1(binding, roster, certificate, observations).directional_checks
            + old_submit_v1(binding, roster, certificate, observations).queue_checks
            == shared_submit_v1(binding, roster, certificate, observations).directional_checks
                + shared_submit_v1(binding, roster, certificate, observations).queue_checks + 1,
{}

pub proof fn publication_immediately_consumes_successful_handoff_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).publication_attempted,
        shared_submit_v1(binding, roster, certificate, observations)
            .fallible_actions_between_shared_and_publication == 0,
        shared_submit_v1(binding, roster, certificate, observations)
            .native_actions_between_shared_and_publication == 0,
        shared_submit_v1(binding, roster, certificate, observations).shared_event_index == Some(2),
        shared_submit_v1(binding, roster, certificate, observations).publication_event_index
            == Some(3),
{}

pub proof fn recoverable_publication_closes_then_restores_request_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
        observations.publication == PublicationObservationV1::Recoverable,
        observations.final_close_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).final_close_observed,
        shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Retryable,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::RetryableRequest,
{}

pub proof fn retained_publication_closes_then_keeps_published_custody_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
        observations.publication == PublicationObservationV1::Retained,
    ensures shared_submit_v1(binding, roster, certificate, observations).final_close_observed,
        shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalPublished,
{}

pub proof fn published_result_retains_final_close_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
        observations.publication == PublicationObservationV1::Published,
    ensures shared_submit_v1(binding, roster, certificate, observations).final_close_observed,
{}

pub proof fn current_final_close_publishes_exact_roster_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
        observations.publication == PublicationObservationV1::Published,
        observations.final_close_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Published,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::PublishedRoster,
{}

pub proof fn recoverable_final_close_loss_is_terminal_prepared_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
        observations.publication == PublicationObservationV1::Recoverable,
        !observations.final_close_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalPrepared,
        shared_submit_v1(binding, roster, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FinalClose),
{}

pub proof fn published_final_close_loss_is_terminal_published_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
        observations.publication == PublicationObservationV1::Published,
        !observations.final_close_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).outcome
            == OutcomeV1::Terminal,
        shared_submit_v1(binding, roster, certificate, observations).custody
            == CustodyV1::TerminalPublished,
        shared_submit_v1(binding, roster, certificate, observations).terminal_stage
            == Some(TerminalStageV1::FinalClose),
{}

pub proof fn h2d_certificate_is_unchanged_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: CertificateV1,
    observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::HostToDevice,
    ensures shared_submit_v1(binding, roster, Some(certificate), observations).host_certificate
            == Some(certificate),
        !shared_submit_v1(binding, roster, Some(certificate), observations)
            .host_certificate_invalidated,
{}

pub proof fn d2h_certificate_is_invalidated_for_every_outcome_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: CertificateV1,
    observations: ObservationsV1,
)
    requires binding.direction == DirectionV1::DeviceToHost,
    ensures shared_submit_v1(binding, roster, Some(certificate), observations)
            .host_certificate.is_none(),
        shared_submit_v1(binding, roster, Some(certificate), observations)
            .host_certificate_invalidated,
{}

pub proof fn shared_transition_preserves_binding_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    ensures shared_submit_v1(binding, roster, certificate, observations).binding == binding,
{}

pub proof fn shared_transition_preserves_roster_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    ensures shared_submit_v1(binding, roster, certificate, observations).roster == roster,
{}

pub proof fn single_packet_handoff_is_admitted_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires valid_input_v1(binding, roster, certificate), binding.packet_count == 1,
        observations.opening_current,
        observations.preparation == PreparationObservationV1::Prepared,
        observations.shared_current,
    ensures shared_submit_v1(binding, roster, certificate, observations).binding.packet_count == 1,
        shared_submit_v1(binding, roster, certificate, observations).roster.len() == 1,
{}

pub proof fn maximum_window_handoff_is_admitted_v1(
    binding: BindingV1, roster: Seq<TicketV1>, certificate: Option<CertificateV1>,
    observations: ObservationsV1,
)
    requires valid_input_v1(binding, roster, certificate),
        binding.packet_count == max_directional_packets_v1(),
    ensures shared_submit_v1(binding, roster, certificate, observations).binding.packet_count
            == max_directional_packets_v1(),
        shared_submit_v1(binding, roster, certificate, observations).roster.len()
            == max_directional_packets_v1(),
{}

pub proof fn same_device_path_is_identity_v1(state: SameDeviceStateV1)
    ensures same_device_unchanged_v1(state) == state,
{}

}
