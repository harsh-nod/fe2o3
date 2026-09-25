//! Independent executable R34 model for fused asynchronous directional SDMA.
//!
//! The model compares the former three-loan public single-copy submission with
//! the R34 one-loan composition. All failures, currentness values, tickets,
//! identities, certificates, and loan results are caller-supplied contracted
//! observations. This finite model performs no I/O and does not refine
//! production Rust, KFD, HSA, HIP, drivers, firmware, hardware, coherence,
//! progress, liveness, or performance.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34DirectionV1 {
    HostToDevice,
    DeviceToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R34RequestBindingV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub native_queue_id: u32,
    pub direction: R34DirectionV1,
    pub host_offset: u64,
    pub device_offset: u64,
    pub copy_bytes: u32,
    pub sequence: u64,
    pub ticket_generation: u64,
}

impl R34RequestBindingV1 {
    pub const fn is_valid(self) -> bool {
        self.queue_id != 0
            && self.queue_generation != 0
            && self.native_queue_id != 0
            && self.copy_bytes != 0
            && self.sequence != 0
            && self.ticket_generation != 0
            && self
                .host_offset
                .checked_add(self.copy_bytes as u64)
                .is_some()
            && self
                .device_offset
                .checked_add(self.copy_bytes as u64)
                .is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R34TicketV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub native_queue_id: u32,
    pub direction: R34DirectionV1,
    pub sequence: u64,
    pub generation: u64,
}

impl R34TicketV1 {
    pub const fn for_binding(binding: R34RequestBindingV1) -> Self {
        Self {
            queue_id: binding.queue_id,
            queue_generation: binding.queue_generation,
            native_queue_id: binding.native_queue_id,
            direction: binding.direction,
            sequence: binding.sequence,
            generation: binding.ticket_generation,
        }
    }

    pub const fn is_exact_for(self, binding: R34RequestBindingV1) -> bool {
        self.queue_id == binding.queue_id
            && self.queue_generation == binding.queue_generation
            && self.native_queue_id == binding.native_queue_id
            && self.direction as u8 == binding.direction as u8
            && self.sequence == binding.sequence
            && self.generation == binding.ticket_generation
    }

    pub const fn same_as(self, other: Self) -> bool {
        self.queue_id == other.queue_id
            && self.queue_generation == other.queue_generation
            && self.native_queue_id == other.native_queue_id
            && self.direction as u8 == other.direction as u8
            && self.sequence == other.sequence
            && self.generation == other.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R34HostCertificateV1 {
    pub certificate_id: u64,
    pub queue_id: u64,
    pub queue_generation: u64,
}

impl R34HostCertificateV1 {
    pub const fn is_exact_for(self, binding: R34RequestBindingV1) -> bool {
        self.certificate_id != 0
            && self.queue_id == binding.queue_id
            && self.queue_generation == binding.queue_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34AdmissionObservationV1 {
    RetryableFailure,
    TerminalFailure,
    Admitted,
}

impl R34AdmissionObservationV1 {
    const fn admitted(self) -> bool {
        matches!(self, Self::Admitted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34RequestPreparationObservationV1 {
    UseRequestRejected,
    ReserveRejected,
    PrepareRejected,
    DetachRejected,
    Prepared,
}

impl R34RequestPreparationObservationV1 {
    const fn prepared(self) -> bool {
        matches!(self, Self::Prepared)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34LowerPreparationObservationV1 {
    RetryableFailure,
    PoisonedFailure,
    Prepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34PublicationObservationV1 {
    Recoverable,
    Retained,
    Confirmed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R34LoanOutcomeV1 {
    pub open_succeeded: bool,
    pub retake_succeeded: bool,
}

impl R34LoanOutcomeV1 {
    pub const fn succeeded(self) -> bool {
        self.open_succeeded && self.retake_succeeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R34ExecutionObservationsV1 {
    pub admission: R34AdmissionObservationV1,
    pub former_opening_loan: R34LoanOutcomeV1,
    pub former_execution_loan: R34LoanOutcomeV1,
    pub former_final_loan: R34LoanOutcomeV1,
    pub fused_loan: R34LoanOutcomeV1,
    pub opening_current: bool,
    pub request_preparation: R34RequestPreparationObservationV1,
    pub lower_preparation: R34LowerPreparationObservationV1,
    pub lower_failure_close_current: bool,
    pub prepublication_current: bool,
    /// The second in-loan close after fused prepublication loss. The result
    /// cannot make that already-terminal prepared custody retryable.
    pub prepublication_failure_close_current: bool,
    pub publication: R34PublicationObservationV1,
    pub planned_ticket: R34TicketV1,
    pub returned_ticket: R34TicketV1,
    pub final_current: bool,
}

impl R34ExecutionObservationsV1 {
    /// Explicit, path-sensitive relation on contracted loan observations.
    /// It never calls either runner or compares their outputs.
    pub const fn loan_equivalence_premise(self, binding: R34RequestBindingV1) -> bool {
        if !self.admission.admitted() || !self.opening_current {
            return true;
        }
        if self.former_opening_loan.open_succeeded != self.fused_loan.open_succeeded {
            return false;
        }
        if !self.former_opening_loan.open_succeeded {
            return true;
        }
        if !self.former_opening_loan.retake_succeeded {
            return !self.request_preparation.prepared() && !self.fused_loan.retake_succeeded;
        }
        if !self.request_preparation.prepared() {
            return self.fused_loan.retake_succeeded;
        }
        if !self.former_execution_loan.open_succeeded {
            return match self.lower_preparation {
                R34LowerPreparationObservationV1::RetryableFailure => {
                    !self.lower_failure_close_current || !self.fused_loan.retake_succeeded
                }
                R34LowerPreparationObservationV1::PoisonedFailure => true,
                R34LowerPreparationObservationV1::Prepared => false,
            };
        }
        match self.lower_preparation {
            R34LowerPreparationObservationV1::RetryableFailure => {
                !self.lower_failure_close_current
                    || (self.former_execution_loan.retake_succeeded
                        && self.former_final_loan.succeeded())
                        == self.fused_loan.retake_succeeded
            }
            R34LowerPreparationObservationV1::PoisonedFailure => true,
            R34LowerPreparationObservationV1::Prepared if !self.prepublication_current => true,
            R34LowerPreparationObservationV1::Prepared => match self.publication {
                R34PublicationObservationV1::Retained => true,
                R34PublicationObservationV1::Recoverable => {
                    !self.final_current
                        || (self.former_execution_loan.retake_succeeded
                            && self.former_final_loan.succeeded())
                            == self.fused_loan.retake_succeeded
                }
                R34PublicationObservationV1::Confirmed => {
                    !self.final_current
                        || !self.planned_ticket.is_exact_for(binding)
                        || !self.returned_ticket.same_as(self.planned_ticket)
                        || (self.former_execution_loan.retake_succeeded
                            && self.former_final_loan.succeeded())
                            == self.fused_loan.retake_succeeded
                }
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34OutcomeV1 {
    Retryable,
    Published,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34CustodyV1 {
    RetryableRequest,
    Published,
    TerminalRequest,
    TerminalPrepared,
    TerminalPreparedQueueRetained,
    TerminalPublished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R34TerminalStageV1 {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R34ExecutionSnapshotV1 {
    pub binding: R34RequestBindingV1,
    pub planned_ticket: Option<R34TicketV1>,
    pub ticket: Option<R34TicketV1>,
    pub host_certificate: Option<R34HostCertificateV1>,
    pub host_certificate_invalidated: bool,
    pub outcome: R34OutcomeV1,
    pub custody: R34CustodyV1,
    pub terminal_stage: Option<R34TerminalStageV1>,
    pub request_constructed: bool,
    pub publication_attempted: bool,
    pub operational_checks: u8,
    pub loan_attempts: u8,
    pub admission_event_index: u8,
    pub request_event_index: Option<u8>,
    pub handoff_event_index: Option<u8>,
    pub publication_event_index: Option<u8>,
    pub final_currentness_event_index: Option<u8>,
    pub fallible_actions_between_handoff_and_publication: u8,
    pub native_actions_between_handoff_and_publication: u8,
    pub prepublication_failure_close_observed: bool,
}

impl R34ExecutionSnapshotV1 {
    pub fn same_external_semantics(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.planned_ticket == other.planned_ticket
            && self.ticket == other.ticket
            && self.host_certificate == other.host_certificate
            && self.host_certificate_invalidated == other.host_certificate_invalidated
            && self.outcome == other.outcome
            && self.custody == other.custody
            && self.request_constructed == other.request_constructed
            && self.publication_attempted == other.publication_attempted
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R34ModelErrorV1 {
    InvalidBinding,
    InvalidCertificate,
}

/// Owning executable model for the abstract comparison.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R34AsyncSingleCopyModelV1, R34DirectionV1,
///     R34RequestBindingV1};
/// let binding = R34RequestBindingV1 {
///     queue_id: 1, queue_generation: 1, native_queue_id: 2,
///     direction: R34DirectionV1::HostToDevice,
///     host_offset: 0, device_offset: 0, copy_bytes: 4,
///     sequence: 1, ticket_generation: 1,
/// };
/// let model = R34AsyncSingleCopyModelV1::new_model_only(binding, None).unwrap();
/// let duplicated = model.clone();
/// # let _ = duplicated;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R34AsyncSingleCopyModelV1 {
    binding: R34RequestBindingV1,
    certificate: Option<R34HostCertificateV1>,
}

struct R34PreparedHandoffV1 {
    binding: R34RequestBindingV1,
    planned_ticket: R34TicketV1,
    certificate: Option<R34HostCertificateV1>,
    certificate_invalidated: bool,
}

impl R34AsyncSingleCopyModelV1 {
    pub fn new_model_only(
        binding: R34RequestBindingV1,
        certificate: Option<R34HostCertificateV1>,
    ) -> Result<Self, R34ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R34ModelErrorV1::InvalidBinding);
        }
        if certificate.is_some_and(|certificate| !certificate.is_exact_for(binding)) {
            return Err(R34ModelErrorV1::InvalidCertificate);
        }
        Ok(Self {
            binding,
            certificate,
        })
    }

    fn initial_snapshot(&self) -> R34ExecutionSnapshotV1 {
        R34ExecutionSnapshotV1 {
            binding: self.binding,
            planned_ticket: None,
            ticket: None,
            host_certificate: self.certificate,
            host_certificate_invalidated: false,
            outcome: R34OutcomeV1::Terminal,
            custody: R34CustodyV1::TerminalRequest,
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

    fn apply_admission(
        snapshot: &mut R34ExecutionSnapshotV1,
        admission: R34AdmissionObservationV1,
    ) -> bool {
        match admission {
            R34AdmissionObservationV1::RetryableFailure => {
                snapshot.outcome = R34OutcomeV1::Retryable;
                snapshot.custody = R34CustodyV1::RetryableRequest;
                false
            }
            R34AdmissionObservationV1::TerminalFailure => {
                snapshot.terminal_stage = Some(R34TerminalStageV1::Admission);
                false
            }
            R34AdmissionObservationV1::Admitted => true,
        }
    }

    fn construct_request(&self, snapshot: &mut R34ExecutionSnapshotV1) {
        snapshot.request_constructed = true;
        snapshot.request_event_index = Some(3);
        if self.binding.direction == R34DirectionV1::DeviceToHost {
            snapshot.host_certificate = None;
            snapshot.host_certificate_invalidated = true;
        }
    }

    fn request_preparation_failed(preparation: R34RequestPreparationObservationV1) -> bool {
        !preparation.prepared()
    }

    fn install_planned_ticket(
        snapshot: &mut R34ExecutionSnapshotV1,
        observations: R34ExecutionObservationsV1,
    ) {
        snapshot.planned_ticket = Some(observations.planned_ticket);
    }

    fn publish(
        &self,
        snapshot: &mut R34ExecutionSnapshotV1,
        observations: R34ExecutionObservationsV1,
    ) {
        let handoff = R34PreparedHandoffV1 {
            binding: snapshot.binding,
            planned_ticket: observations.planned_ticket,
            certificate: snapshot.host_certificate,
            certificate_invalidated: snapshot.host_certificate_invalidated,
        };
        snapshot.binding = handoff.binding;
        snapshot.planned_ticket = Some(handoff.planned_ticket);
        snapshot.host_certificate = handoff.certificate;
        snapshot.host_certificate_invalidated = handoff.certificate_invalidated;
        snapshot.handoff_event_index = Some(6);
        snapshot.publication_event_index = Some(7);
        snapshot.publication_attempted = true;
        if !matches!(
            observations.publication,
            R34PublicationObservationV1::Recoverable
        ) {
            snapshot.ticket = Some(observations.returned_ticket);
        }
    }

    fn former_final_close(
        snapshot: &mut R34ExecutionSnapshotV1,
        observations: R34ExecutionObservationsV1,
        lower_failure: bool,
    ) -> bool {
        snapshot.loan_attempts += 1;
        if !observations.former_final_loan.open_succeeded {
            return false;
        }
        snapshot.operational_checks += 1;
        if !lower_failure {
            snapshot.final_currentness_event_index = Some(8);
        }
        let current = if lower_failure {
            observations.lower_failure_close_current
        } else {
            observations.final_current
        };
        current && observations.former_final_loan.retake_succeeded
    }

    fn former_post_operation_stage(
        observations: R34ExecutionObservationsV1,
        lower_failure: bool,
    ) -> R34TerminalStageV1 {
        if !observations.former_execution_loan.retake_succeeded {
            R34TerminalStageV1::FormerExecutionLoanRetake
        } else if !observations.former_final_loan.open_succeeded {
            R34TerminalStageV1::FormerFinalLoanOpen
        } else if !observations.former_final_loan.retake_succeeded {
            R34TerminalStageV1::FormerFinalLoanRetake
        } else if lower_failure && !observations.lower_failure_close_current {
            R34TerminalStageV1::LowerFailureClose
        } else if lower_failure {
            R34TerminalStageV1::LowerPreparation
        } else {
            R34TerminalStageV1::FinalCurrentness
        }
    }

    fn publication_terminal_stage(
        &self,
        observations: R34ExecutionObservationsV1,
        operation_succeeded: bool,
        closing_succeeded: bool,
        fused: bool,
    ) -> R34TerminalStageV1 {
        if !operation_succeeded {
            return if fused {
                R34TerminalStageV1::FusedLoanRetake
            } else {
                R34TerminalStageV1::FormerExecutionLoanRetake
            };
        }
        if !closing_succeeded {
            if fused {
                if !observations.fused_loan.retake_succeeded {
                    return R34TerminalStageV1::FusedLoanRetake;
                }
            } else if !observations.former_final_loan.open_succeeded {
                return R34TerminalStageV1::FormerFinalLoanOpen;
            } else if !observations.former_final_loan.retake_succeeded {
                return R34TerminalStageV1::FormerFinalLoanRetake;
            }
            return R34TerminalStageV1::FinalCurrentness;
        }
        if !observations.planned_ticket.is_exact_for(self.binding) {
            R34TerminalStageV1::PlannedTicketOccurrence
        } else {
            R34TerminalStageV1::ReturnedTicketMismatch
        }
    }

    fn finish_publication(
        &self,
        snapshot: &mut R34ExecutionSnapshotV1,
        observations: R34ExecutionObservationsV1,
        operation_succeeded: bool,
        closing_succeeded: bool,
        fused: bool,
    ) {
        match observations.publication {
            R34PublicationObservationV1::Retained => {
                snapshot.custody = R34CustodyV1::TerminalPreparedQueueRetained;
                snapshot.terminal_stage = Some(R34TerminalStageV1::PublicationRetained);
            }
            R34PublicationObservationV1::Recoverable
                if operation_succeeded && closing_succeeded =>
            {
                snapshot.outcome = R34OutcomeV1::Retryable;
                snapshot.custody = R34CustodyV1::RetryableRequest;
            }
            R34PublicationObservationV1::Recoverable => {
                snapshot.custody = R34CustodyV1::TerminalPrepared;
                snapshot.terminal_stage = Some(self.publication_terminal_stage(
                    observations,
                    operation_succeeded,
                    closing_succeeded,
                    fused,
                ));
            }
            R34PublicationObservationV1::Confirmed
                if operation_succeeded
                    && closing_succeeded
                    && observations.planned_ticket.is_exact_for(self.binding)
                    && observations
                        .returned_ticket
                        .same_as(observations.planned_ticket) =>
            {
                snapshot.outcome = R34OutcomeV1::Published;
                snapshot.custody = R34CustodyV1::Published;
            }
            R34PublicationObservationV1::Confirmed => {
                snapshot.custody = R34CustodyV1::TerminalPublished;
                snapshot.terminal_stage = Some(self.publication_terminal_stage(
                    observations,
                    operation_succeeded,
                    closing_succeeded,
                    fused,
                ));
            }
        }
    }

    pub fn run_former_model_only(
        &self,
        observations: R34ExecutionObservationsV1,
    ) -> R34ExecutionSnapshotV1 {
        let mut snapshot = self.initial_snapshot();
        if !Self::apply_admission(&mut snapshot, observations.admission) {
            return snapshot;
        }

        snapshot.loan_attempts += 1;
        if !observations.former_opening_loan.open_succeeded {
            snapshot.terminal_stage = Some(R34TerminalStageV1::FormerOpeningLoanOpen);
            return snapshot;
        }
        snapshot.operational_checks += 1;
        if !observations.opening_current {
            snapshot.terminal_stage = Some(if observations.former_opening_loan.retake_succeeded {
                R34TerminalStageV1::OpeningCurrentness
            } else {
                R34TerminalStageV1::FormerOpeningLoanRetake
            });
            return snapshot;
        }
        if !observations.former_opening_loan.retake_succeeded {
            snapshot.terminal_stage = Some(R34TerminalStageV1::FormerOpeningLoanRetake);
            return snapshot;
        }
        if Self::request_preparation_failed(observations.request_preparation) {
            snapshot.outcome = R34OutcomeV1::Retryable;
            snapshot.custody = R34CustodyV1::RetryableRequest;
            return snapshot;
        }
        self.construct_request(&mut snapshot);

        snapshot.loan_attempts += 1;
        if !observations.former_execution_loan.open_succeeded {
            let _ = Self::former_final_close(&mut snapshot, observations, true);
            snapshot.custody = R34CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R34TerminalStageV1::FormerExecutionLoanOpen);
            return snapshot;
        }
        match observations.lower_preparation {
            R34LowerPreparationObservationV1::RetryableFailure
            | R34LowerPreparationObservationV1::PoisonedFailure => {
                let closing_succeeded = Self::former_final_close(&mut snapshot, observations, true);
                if matches!(
                    observations.lower_preparation,
                    R34LowerPreparationObservationV1::RetryableFailure
                ) && observations.former_execution_loan.retake_succeeded
                    && closing_succeeded
                {
                    snapshot.outcome = R34OutcomeV1::Retryable;
                    snapshot.custody = R34CustodyV1::RetryableRequest;
                } else {
                    snapshot.custody = R34CustodyV1::TerminalPrepared;
                    snapshot.terminal_stage =
                        Some(Self::former_post_operation_stage(observations, true));
                }
                return snapshot;
            }
            R34LowerPreparationObservationV1::Prepared => {
                Self::install_planned_ticket(&mut snapshot, observations);
            }
        }
        snapshot.operational_checks += 1;
        if !observations.prepublication_current {
            snapshot.custody = R34CustodyV1::TerminalPrepared;
            snapshot.terminal_stage =
                Some(if observations.former_execution_loan.retake_succeeded {
                    R34TerminalStageV1::Prepublication
                } else {
                    R34TerminalStageV1::FormerExecutionLoanRetake
                });
            return snapshot;
        }

        self.publish(&mut snapshot, observations);
        let closing_succeeded = Self::former_final_close(&mut snapshot, observations, false);
        self.finish_publication(
            &mut snapshot,
            observations,
            observations.former_execution_loan.retake_succeeded,
            closing_succeeded,
            false,
        );
        snapshot
    }

    pub fn run_fused_model_only(
        &self,
        observations: R34ExecutionObservationsV1,
    ) -> R34ExecutionSnapshotV1 {
        let mut snapshot = self.initial_snapshot();
        if !Self::apply_admission(&mut snapshot, observations.admission) {
            return snapshot;
        }

        snapshot.loan_attempts += 1;
        if !observations.fused_loan.open_succeeded {
            snapshot.terminal_stage = Some(R34TerminalStageV1::FusedLoanOpen);
            return snapshot;
        }
        snapshot.operational_checks += 1;
        if !observations.opening_current {
            snapshot.terminal_stage = Some(if observations.fused_loan.retake_succeeded {
                R34TerminalStageV1::OpeningCurrentness
            } else {
                R34TerminalStageV1::FusedLoanRetake
            });
            return snapshot;
        }
        if Self::request_preparation_failed(observations.request_preparation) {
            if observations.fused_loan.retake_succeeded {
                snapshot.outcome = R34OutcomeV1::Retryable;
                snapshot.custody = R34CustodyV1::RetryableRequest;
            } else {
                snapshot.terminal_stage = Some(R34TerminalStageV1::FusedLoanRetake);
            }
            return snapshot;
        }
        self.construct_request(&mut snapshot);

        match observations.lower_preparation {
            R34LowerPreparationObservationV1::RetryableFailure
            | R34LowerPreparationObservationV1::PoisonedFailure => {
                snapshot.operational_checks += 1;
                if matches!(
                    observations.lower_preparation,
                    R34LowerPreparationObservationV1::RetryableFailure
                ) && observations.lower_failure_close_current
                    && observations.fused_loan.retake_succeeded
                {
                    snapshot.outcome = R34OutcomeV1::Retryable;
                    snapshot.custody = R34CustodyV1::RetryableRequest;
                } else {
                    snapshot.custody = R34CustodyV1::TerminalPrepared;
                    snapshot.terminal_stage = Some(if !observations.fused_loan.retake_succeeded {
                        R34TerminalStageV1::FusedLoanRetake
                    } else if !observations.lower_failure_close_current {
                        R34TerminalStageV1::LowerFailureClose
                    } else {
                        R34TerminalStageV1::LowerPreparation
                    });
                }
                return snapshot;
            }
            R34LowerPreparationObservationV1::Prepared => {
                Self::install_planned_ticket(&mut snapshot, observations);
            }
        }
        snapshot.operational_checks += 1;
        if !observations.prepublication_current {
            snapshot.operational_checks += 1;
            snapshot.prepublication_failure_close_observed = true;
            snapshot.custody = R34CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(if observations.fused_loan.retake_succeeded {
                R34TerminalStageV1::Prepublication
            } else {
                R34TerminalStageV1::FusedLoanRetake
            });
            return snapshot;
        }

        self.publish(&mut snapshot, observations);
        snapshot.operational_checks += 1;
        snapshot.final_currentness_event_index = Some(8);
        let closing_succeeded =
            observations.final_current && observations.fused_loan.retake_succeeded;
        self.finish_publication(
            &mut snapshot,
            observations,
            observations.fused_loan.retake_succeeded,
            closing_succeeded,
            true,
        );
        snapshot
    }
}
