//! Independent executable R32 model for the directional SDMA currentness handoff.
//!
//! The model compares the former prepare-close/publication-open pair with one
//! shared observation. All currentness values, lower outcomes, certificates,
//! and identities are caller-supplied contracted inputs. This finite model
//! performs no I/O and does not refine executable Rust, KFD, HSA, HIP, driver,
//! firmware, hardware, coherent memory, DMA visibility, progress, or timing.

pub const R32_MAX_DIRECTIONAL_PACKETS_V1: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32DirectionV1 {
    HostToDevice,
    DeviceToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32HandoffBindingV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub native_queue_id: u32,
    pub direction: R32DirectionV1,
    pub packet_count: u8,
}

impl R32HandoffBindingV1 {
    pub const fn is_valid(self) -> bool {
        self.queue_id != 0
            && self.queue_generation != 0
            && self.native_queue_id != 0
            && self.packet_count != 0
            && (self.packet_count as usize) <= R32_MAX_DIRECTIONAL_PACKETS_V1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32TicketV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub native_queue_id: u32,
    pub direction: R32DirectionV1,
    pub occurrence: u8,
    pub generation: u64,
}

impl R32TicketV1 {
    pub const fn is_exact_for(self, binding: R32HandoffBindingV1, occurrence: usize) -> bool {
        self.queue_id == binding.queue_id
            && self.queue_generation == binding.queue_generation
            && self.native_queue_id == binding.native_queue_id
            && self.direction as u8 == binding.direction as u8
            && self.occurrence as usize == occurrence
            && self.generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32TicketRosterV1 {
    pub tickets: [Option<R32TicketV1>; R32_MAX_DIRECTIONAL_PACKETS_V1],
}

impl R32TicketRosterV1 {
    pub fn exact_for(&self, binding: R32HandoffBindingV1) -> bool {
        self.tickets.iter().enumerate().all(|(index, ticket)| {
            if index < binding.packet_count as usize {
                ticket.is_some_and(|ticket| ticket.is_exact_for(binding, index))
            } else {
                ticket.is_none()
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32HostCertificateV1 {
    pub certificate_id: u64,
    pub queue_id: u64,
    pub queue_generation: u64,
}

impl R32HostCertificateV1 {
    pub const fn is_exact_for(self, binding: R32HandoffBindingV1) -> bool {
        self.certificate_id != 0
            && self.queue_id == binding.queue_id
            && self.queue_generation == binding.queue_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32PreparationObservationV1 {
    RetryableFailure,
    PoisonedFailure,
    RosterMismatch,
    Prepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32PublicationObservationV1 {
    Recoverable,
    Retained,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32SubmitObservationsV1 {
    pub opening_current: bool,
    pub preparation: R32PreparationObservationV1,
    /// Used only after a failed preparation, where the old explicit close is retained.
    pub prepare_failure_close_current: bool,
    /// Used twice by the old reference and once by the shared implementation.
    pub shared_current: bool,
    pub publication: R32PublicationObservationV1,
    pub final_close_current: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32OutcomeV1 {
    Retryable,
    Published,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32CustodyV1 {
    RetryableRequest,
    PublishedRoster,
    TerminalRequest,
    TerminalPrepared,
    TerminalPublished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32TerminalStageV1 {
    Opening,
    PrepareFailureClose,
    PrepareSuccessClose,
    SharedCloseOpen,
    PublicationRetained,
    FinalClose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R32TraceEventV1 {
    OpeningCheck,
    Prepare,
    PrepareClose,
    SharedCloseOpen,
    PublicationOpen,
    Publication,
    FinalClose,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32SubmitSnapshotV1 {
    pub binding: R32HandoffBindingV1,
    pub roster: R32TicketRosterV1,
    pub host_certificate: Option<R32HostCertificateV1>,
    pub host_certificate_invalidated: bool,
    pub outcome: R32OutcomeV1,
    pub custody: R32CustodyV1,
    pub terminal_stage: Option<R32TerminalStageV1>,
    pub directional_checks: u8,
    pub queue_checks: u8,
    pub prepare_failure_close_observed: bool,
    pub shared_close_open_observed: bool,
    pub publication_attempted: bool,
    pub final_close_observed: bool,
    pub fallible_actions_between_shared_and_publication: u8,
    pub native_actions_between_shared_and_publication: u8,
    pub trace: [Option<R32TraceEventV1>; 7],
    pub trace_len: u8,
}

impl R32SubmitSnapshotV1 {
    pub const fn operational_checks(self) -> u8 {
        self.directional_checks + self.queue_checks
    }

    pub fn trace(&self) -> &[Option<R32TraceEventV1>] {
        &self.trace[..self.trace_len as usize]
    }

    pub fn same_external_semantics(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.roster == other.roster
            && self.host_certificate == other.host_certificate
            && self.host_certificate_invalidated == other.host_certificate_invalidated
            && self.outcome == other.outcome
            && self.custody == other.custody
            && self.publication_attempted == other.publication_attempted
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R32ModelErrorV1 {
    InvalidBinding,
    InvalidRoster,
    InvalidCertificate,
}

/// Move-only model carrier. It deliberately does not implement `Clone`.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R32DirectionalCurrentnessHandoffModelV1;
/// fn cannot_clone(model: R32DirectionalCurrentnessHandoffModelV1) {
///     let _duplicate = model.clone();
/// }
/// ```
pub struct R32DirectionalCurrentnessHandoffModelV1 {
    binding: R32HandoffBindingV1,
    roster: R32TicketRosterV1,
    host_certificate: Option<R32HostCertificateV1>,
    host_certificate_invalidated: bool,
}

/// Private move-only witness created only after the shared observation succeeds.
struct DirectionalPreparedHandoffV1 {
    binding: R32HandoffBindingV1,
    roster: R32TicketRosterV1,
    host_certificate: Option<R32HostCertificateV1>,
    host_certificate_invalidated: bool,
}

impl DirectionalPreparedHandoffV1 {
    fn publish(
        self,
        observation: R32PublicationObservationV1,
    ) -> (
        R32HandoffBindingV1,
        R32TicketRosterV1,
        Option<R32HostCertificateV1>,
        bool,
        R32PublicationObservationV1,
    ) {
        (
            self.binding,
            self.roster,
            self.host_certificate,
            self.host_certificate_invalidated,
            observation,
        )
    }
}

impl R32DirectionalCurrentnessHandoffModelV1 {
    pub fn new_model_only(
        binding: R32HandoffBindingV1,
        roster: R32TicketRosterV1,
        certificate: Option<R32HostCertificateV1>,
    ) -> Result<Self, R32ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R32ModelErrorV1::InvalidBinding);
        }
        if !roster.exact_for(binding) {
            return Err(R32ModelErrorV1::InvalidRoster);
        }
        if certificate.is_some_and(|certificate| !certificate.is_exact_for(binding)) {
            return Err(R32ModelErrorV1::InvalidCertificate);
        }
        let (host_certificate, host_certificate_invalidated) = match binding.direction {
            R32DirectionV1::HostToDevice => (certificate, false),
            R32DirectionV1::DeviceToHost => (None, true),
        };
        Ok(Self {
            binding,
            roster,
            host_certificate,
            host_certificate_invalidated,
        })
    }

    fn initial_snapshot(&self) -> R32SubmitSnapshotV1 {
        R32SubmitSnapshotV1 {
            binding: self.binding,
            roster: self.roster,
            host_certificate: self.host_certificate,
            host_certificate_invalidated: self.host_certificate_invalidated,
            outcome: R32OutcomeV1::Terminal,
            custody: R32CustodyV1::TerminalRequest,
            terminal_stage: None,
            directional_checks: 0,
            queue_checks: 0,
            prepare_failure_close_observed: false,
            shared_close_open_observed: false,
            publication_attempted: false,
            final_close_observed: false,
            fallible_actions_between_shared_and_publication: 0,
            native_actions_between_shared_and_publication: 0,
            trace: [None; 7],
            trace_len: 0,
        }
    }

    /// Reference lifecycle with separate prepare-close and publication-open checks.
    pub fn run_old_model_only(self, observations: R32SubmitObservationsV1) -> R32SubmitSnapshotV1 {
        let mut state = self.initial_snapshot();
        push_event(&mut state, R32TraceEventV1::OpeningCheck);
        state.directional_checks = 1;
        if !observations.opening_current {
            state.terminal_stage = Some(R32TerminalStageV1::Opening);
            return state;
        }

        push_event(&mut state, R32TraceEventV1::Prepare);
        if observations.preparation != R32PreparationObservationV1::Prepared {
            return finish_prepare_failure(state, observations);
        }

        push_event(&mut state, R32TraceEventV1::PrepareClose);
        state.directional_checks += 1;
        if !observations.shared_current {
            state.custody = R32CustodyV1::TerminalPrepared;
            state.terminal_stage = Some(R32TerminalStageV1::PrepareSuccessClose);
            return state;
        }

        push_event(&mut state, R32TraceEventV1::PublicationOpen);
        state.queue_checks += 1;
        // Refinement supplies the same observation to the former close and open.
        debug_assert!(observations.shared_current);
        state.publication_attempted = true;
        push_event(&mut state, R32TraceEventV1::Publication);
        finish_publication(state, observations)
    }

    /// Optimized lifecycle with one shared prepare-close/publication-open observation.
    pub fn run_shared_model_only(
        self,
        observations: R32SubmitObservationsV1,
    ) -> R32SubmitSnapshotV1 {
        let mut state = self.initial_snapshot();
        push_event(&mut state, R32TraceEventV1::OpeningCheck);
        state.directional_checks = 1;
        if !observations.opening_current {
            state.terminal_stage = Some(R32TerminalStageV1::Opening);
            return state;
        }

        push_event(&mut state, R32TraceEventV1::Prepare);
        if observations.preparation != R32PreparationObservationV1::Prepared {
            return finish_prepare_failure(state, observations);
        }

        push_event(&mut state, R32TraceEventV1::SharedCloseOpen);
        state.queue_checks += 1;
        state.shared_close_open_observed = true;
        if !observations.shared_current {
            state.custody = R32CustodyV1::TerminalPrepared;
            state.terminal_stage = Some(R32TerminalStageV1::SharedCloseOpen);
            return state;
        }

        let handoff = DirectionalPreparedHandoffV1 {
            binding: state.binding,
            roster: state.roster,
            host_certificate: state.host_certificate,
            host_certificate_invalidated: state.host_certificate_invalidated,
        };
        let (binding, roster, certificate, invalidated, publication) =
            handoff.publish(observations.publication);
        state.binding = binding;
        state.roster = roster;
        state.host_certificate = certificate;
        state.host_certificate_invalidated = invalidated;
        state.publication_attempted = true;
        push_event(&mut state, R32TraceEventV1::Publication);
        finish_publication_with_observation(state, publication, observations.final_close_current)
    }
}

fn finish_prepare_failure(
    mut state: R32SubmitSnapshotV1,
    observations: R32SubmitObservationsV1,
) -> R32SubmitSnapshotV1 {
    push_event(&mut state, R32TraceEventV1::PrepareClose);
    state.directional_checks += 1;
    state.prepare_failure_close_observed = true;
    if observations.preparation == R32PreparationObservationV1::RetryableFailure
        && observations.prepare_failure_close_current
    {
        state.outcome = R32OutcomeV1::Retryable;
        state.custody = R32CustodyV1::RetryableRequest;
    } else {
        state.terminal_stage = Some(R32TerminalStageV1::PrepareFailureClose);
    }
    state
}

fn finish_publication(
    state: R32SubmitSnapshotV1,
    observations: R32SubmitObservationsV1,
) -> R32SubmitSnapshotV1 {
    finish_publication_with_observation(
        state,
        observations.publication,
        observations.final_close_current,
    )
}

fn finish_publication_with_observation(
    mut state: R32SubmitSnapshotV1,
    publication: R32PublicationObservationV1,
    final_close_current: bool,
) -> R32SubmitSnapshotV1 {
    push_event(&mut state, R32TraceEventV1::FinalClose);
    state.directional_checks += 1;
    state.final_close_observed = true;
    match (publication, final_close_current) {
        (R32PublicationObservationV1::Recoverable, true) => {
            state.outcome = R32OutcomeV1::Retryable;
            state.custody = R32CustodyV1::RetryableRequest;
        }
        (R32PublicationObservationV1::Retained, _) => {
            state.custody = R32CustodyV1::TerminalPublished;
            state.terminal_stage = Some(R32TerminalStageV1::PublicationRetained);
        }
        (R32PublicationObservationV1::Published, true) => {
            state.outcome = R32OutcomeV1::Published;
            state.custody = R32CustodyV1::PublishedRoster;
        }
        (R32PublicationObservationV1::Recoverable, false) => {
            state.custody = R32CustodyV1::TerminalPrepared;
            state.terminal_stage = Some(R32TerminalStageV1::FinalClose);
        }
        (R32PublicationObservationV1::Published, false) => {
            state.custody = R32CustodyV1::TerminalPublished;
            state.terminal_stage = Some(R32TerminalStageV1::FinalClose);
        }
    }
    state
}

fn push_event(state: &mut R32SubmitSnapshotV1, event: R32TraceEventV1) {
    let index = state.trace_len as usize;
    state.trace[index] = Some(event);
    state.trace_len += 1;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R32SameDeviceSnapshotV1 {
    pub source_queue_id: u64,
    pub destination_queue_id: u64,
    pub packet_count: u8,
    pub operational_checks: u8,
    pub publication_count: u8,
}

/// R32 does not transform the separate same-device D2D lifecycle.
pub const fn r32_same_device_unchanged_model_only(
    state: R32SameDeviceSnapshotV1,
) -> R32SameDeviceSnapshotV1 {
    state
}
