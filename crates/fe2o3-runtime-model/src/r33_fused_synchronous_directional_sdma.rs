//! Independent executable R33 model for fused synchronous directional SDMA.
//!
//! The model compares the former R32 submit-then-wait composition with the R33
//! fused synchronous composition. Currentness, lower-operation outcomes, loan
//! results, identities, and certificates are caller-supplied contracted inputs.
//! This finite model performs no I/O and does not refine executable Rust, KFD,
//! HSA, HIP, driver, firmware, hardware, coherence, DMA visibility, progress,
//! liveness, or performance.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33DirectionV1 {
    HostToDevice,
    DeviceToHost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R33RequestBindingV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub native_queue_id: u32,
    pub direction: R33DirectionV1,
    pub host_offset: u64,
    pub device_offset: u64,
    pub copy_bytes: u32,
    pub sequence: u64,
    pub ticket_generation: u64,
}

impl R33RequestBindingV1 {
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
pub struct R33TicketV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub native_queue_id: u32,
    pub direction: R33DirectionV1,
    pub sequence: u64,
    pub generation: u64,
}

impl R33TicketV1 {
    pub const fn for_binding(binding: R33RequestBindingV1) -> Self {
        Self {
            queue_id: binding.queue_id,
            queue_generation: binding.queue_generation,
            native_queue_id: binding.native_queue_id,
            direction: binding.direction,
            sequence: binding.sequence,
            generation: binding.ticket_generation,
        }
    }

    pub const fn is_exact_for(self, binding: R33RequestBindingV1) -> bool {
        self.queue_id == binding.queue_id
            && self.queue_generation == binding.queue_generation
            && self.native_queue_id == binding.native_queue_id
            && self.direction as u8 == binding.direction as u8
            && self.sequence == binding.sequence
            && self.generation == binding.ticket_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R33HostCertificateV1 {
    pub certificate_id: u64,
    pub queue_id: u64,
    pub queue_generation: u64,
}

impl R33HostCertificateV1 {
    pub const fn is_exact_for(self, binding: R33RequestBindingV1) -> bool {
        self.certificate_id != 0
            && self.queue_id == binding.queue_id
            && self.queue_generation == binding.queue_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33PreparationObservationV1 {
    RetryableFailure,
    PoisonedFailure,
    Prepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33PublicationObservationV1 {
    Recoverable,
    Retained,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33WaitObservationV1 {
    Timeout,
    LowerFailure,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R33LoanOutcomeV1 {
    pub open_succeeded: bool,
    pub retake_succeeded: bool,
}

impl R33LoanOutcomeV1 {
    pub const fn succeeded(self) -> bool {
        self.open_succeeded && self.retake_succeeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R33ExecutionObservationsV1 {
    /// The opening operational-currentness loan retained by both compositions.
    pub opening_loan: R33LoanOutcomeV1,
    pub opening_current: bool,
    /// The former submit operation's preparation/publication loan.
    pub former_submit_loan: R33LoanOutcomeV1,
    /// The former submit close uses its own model-foundation loan.
    pub former_submit_close_loan: R33LoanOutcomeV1,
    /// The former standalone wait uses its own model-foundation loan.
    pub former_wait_loan: R33LoanOutcomeV1,
    /// The fused preparation/publication/wait operation's single loan.
    pub fused_execution_loan: R33LoanOutcomeV1,
    pub preparation: R33PreparationObservationV1,
    pub prepare_failure_close_current: bool,
    pub prepublication_current: bool,
    pub publication: R33PublicationObservationV1,
    /// Ticket returned by retained or confirmed lower publication. It is a
    /// contracted input and may differ from the exact planned ticket.
    pub returned_ticket: R33TicketV1,
    /// Former submit close. The fused path uses it only to close a failed
    /// publication; successful publication carries its current scope to wait.
    pub former_submit_close_current: bool,
    /// Former wait open. The fused path has no corresponding observation.
    pub former_wait_open_current: bool,
    pub wait: R33WaitObservationV1,
    pub final_current: bool,
    /// Restoration after the completed lower record has been removed.
    pub completion_restoration_succeeded: bool,
}

impl R33ExecutionObservationsV1 {
    /// Premise for removing the successful submit close and wait open: both
    /// removed observations are aligned with the retained prepublication one.
    pub const fn middle_currentness_aligned(self) -> bool {
        self.former_submit_close_current == self.prepublication_current
            && self.former_wait_open_current == self.prepublication_current
    }

    /// Relates the former submit loan to the fused operation loan only on paths
    /// where a difference can change externally visible custody.
    pub const fn retained_loans_aligned_when_needed(self, binding: R33RequestBindingV1) -> bool {
        if !self.opening_loan.succeeded() || !self.opening_current {
            true
        } else if self.former_submit_loan.open_succeeded != self.fused_execution_loan.open_succeeded
        {
            false
        } else if !self.former_submit_loan.open_succeeded {
            true
        } else {
            match self.preparation {
                R33PreparationObservationV1::RetryableFailure
                    if self.prepare_failure_close_current =>
                {
                    self.former_submit_loan.retake_succeeded
                        == self.fused_execution_loan.retake_succeeded
                }
                R33PreparationObservationV1::RetryableFailure
                | R33PreparationObservationV1::PoisonedFailure => true,
                R33PreparationObservationV1::Prepared if !self.prepublication_current => true,
                R33PreparationObservationV1::Prepared => match self.publication {
                    R33PublicationObservationV1::Recoverable
                        if self.former_submit_close_current =>
                    {
                        self.former_submit_loan.retake_succeeded
                            == self.fused_execution_loan.retake_succeeded
                    }
                    R33PublicationObservationV1::Recoverable
                    | R33PublicationObservationV1::Retained => true,
                    R33PublicationObservationV1::Published
                        if self.returned_ticket.is_exact_for(binding) =>
                    {
                        self.former_submit_loan.retake_succeeded
                            && self.fused_execution_loan.retake_succeeded
                    }
                    R33PublicationObservationV1::Published => true,
                },
            }
        }
    }

    /// Requires the two former-only loan boundaries to succeed only on paths
    /// where their failure would distinguish the former and fused results.
    pub const fn removed_loans_succeed_when_needed(self, binding: R33RequestBindingV1) -> bool {
        if !self.opening_loan.succeeded()
            || !self.opening_current
            || !self.former_submit_loan.open_succeeded
            || !self.fused_execution_loan.open_succeeded
        {
            true
        } else {
            match self.preparation {
                R33PreparationObservationV1::RetryableFailure
                    if self.prepare_failure_close_current
                        && self.former_submit_loan.retake_succeeded
                        && self.fused_execution_loan.retake_succeeded =>
                {
                    self.former_submit_close_loan.succeeded()
                }
                R33PreparationObservationV1::RetryableFailure
                | R33PreparationObservationV1::PoisonedFailure => true,
                R33PreparationObservationV1::Prepared if !self.prepublication_current => true,
                R33PreparationObservationV1::Prepared => match self.publication {
                    R33PublicationObservationV1::Recoverable
                        if self.former_submit_close_current
                            && self.former_submit_loan.retake_succeeded
                            && self.fused_execution_loan.retake_succeeded =>
                    {
                        self.former_submit_close_loan.succeeded()
                    }
                    R33PublicationObservationV1::Recoverable
                    | R33PublicationObservationV1::Retained => true,
                    R33PublicationObservationV1::Published
                        if self.returned_ticket.is_exact_for(binding)
                            && self.former_submit_loan.retake_succeeded
                            && self.fused_execution_loan.retake_succeeded =>
                    {
                        self.former_submit_close_loan.succeeded()
                            && self.former_wait_loan.open_succeeded
                            && match self.wait {
                                R33WaitObservationV1::Timeout if self.final_current => {
                                    self.former_wait_loan.retake_succeeded
                                }
                                R33WaitObservationV1::Completed
                                    if self.final_current
                                        && self.completion_restoration_succeeded =>
                                {
                                    self.former_wait_loan.retake_succeeded
                                }
                                R33WaitObservationV1::Timeout
                                | R33WaitObservationV1::LowerFailure
                                | R33WaitObservationV1::Completed => true,
                            }
                    }
                    R33PublicationObservationV1::Published => true,
                },
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33OutcomeV1 {
    Retryable,
    Timeout,
    Completed,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33CustodyV1 {
    RetryableRequest,
    PendingPublished,
    Completed,
    TerminalRequest,
    TerminalPrepared,
    TerminalPreparedQueueRetained,
    TerminalPublished,
    TerminalCompletedUnrestored,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R33TerminalStageV1 {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R33ExecutionSnapshotV1 {
    pub binding: R33RequestBindingV1,
    pub planned_ticket: Option<R33TicketV1>,
    pub ticket: Option<R33TicketV1>,
    pub host_certificate: Option<R33HostCertificateV1>,
    pub host_certificate_invalidated: bool,
    pub outcome: R33OutcomeV1,
    pub custody: R33CustodyV1,
    pub terminal_stage: Option<R33TerminalStageV1>,
    pub publication_attempted: bool,
    pub wait_attempted: bool,
    pub lower_record_retired: bool,
    pub completion_restoration_attempted: bool,
    pub operational_checks: u8,
    pub model_loans: u8,
    pub handoff_event_index: Option<u8>,
    pub publication_event_index: Option<u8>,
    pub wait_event_index: Option<u8>,
    pub final_currentness_event_index: Option<u8>,
    pub retirement_event_index: Option<u8>,
    pub fallible_actions_between_handoff_and_publication: u8,
    pub native_actions_between_handoff_and_publication: u8,
    pub wait_inside_publication_loan: bool,
}

impl R33ExecutionSnapshotV1 {
    pub fn same_external_semantics(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.planned_ticket == other.planned_ticket
            && self.ticket == other.ticket
            && self.host_certificate == other.host_certificate
            && self.host_certificate_invalidated == other.host_certificate_invalidated
            && self.outcome == other.outcome
            && self.custody == other.custody
            && self.publication_attempted == other.publication_attempted
            && self.wait_attempted == other.wait_attempted
            && self.lower_record_retired == other.lower_record_retired
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R33SameDeviceIdentityV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub source_storage_id: u64,
    pub destination_storage_id: u64,
}

pub const fn r33_same_device_identity_projection_v1(
    identity: R33SameDeviceIdentityV1,
) -> R33SameDeviceIdentityV1 {
    identity
}

#[derive(Debug, Eq, PartialEq)]
pub enum R33ModelErrorV1 {
    InvalidBinding,
    InvalidCertificate,
}

/// Owning executable model for the abstract comparison.
///
/// The model intentionally has no `Clone` implementation. Mathematical Verus
/// values do not establish this executable ownership property.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R33DirectionV1, R33RequestBindingV1,
///     R33SynchronousExecutionModelV1};
/// let binding = R33RequestBindingV1 {
///     queue_id: 1, queue_generation: 1, native_queue_id: 2,
///     direction: R33DirectionV1::HostToDevice,
///     host_offset: 0, device_offset: 0, copy_bytes: 4,
///     sequence: 1, ticket_generation: 1,
/// };
/// let model = R33SynchronousExecutionModelV1::new_model_only(binding, None).unwrap();
/// let duplicated = model.clone();
/// # let _ = duplicated;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R33SynchronousExecutionModelV1 {
    binding: R33RequestBindingV1,
    certificate: Option<R33HostCertificateV1>,
}

// Private move-only carrier between the retained prepublication observation
// and the immediately following publication transition.
struct R33PreparedHandoffV1 {
    binding: R33RequestBindingV1,
    ticket: R33TicketV1,
    host_certificate: Option<R33HostCertificateV1>,
    host_certificate_invalidated: bool,
}

impl R33SynchronousExecutionModelV1 {
    pub fn new_model_only(
        binding: R33RequestBindingV1,
        certificate: Option<R33HostCertificateV1>,
    ) -> Result<Self, R33ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R33ModelErrorV1::InvalidBinding);
        }
        if certificate.is_some_and(|certificate| !certificate.is_exact_for(binding)) {
            return Err(R33ModelErrorV1::InvalidCertificate);
        }
        Ok(Self {
            binding,
            certificate,
        })
    }

    fn initial_snapshot(&self, former: bool) -> R33ExecutionSnapshotV1 {
        R33ExecutionSnapshotV1 {
            binding: self.binding,
            planned_ticket: None,
            ticket: None,
            host_certificate: self.certificate,
            host_certificate_invalidated: false,
            outcome: R33OutcomeV1::Terminal,
            custody: R33CustodyV1::TerminalRequest,
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

    fn construct_request(&self, snapshot: &mut R33ExecutionSnapshotV1) {
        if self.binding.direction == R33DirectionV1::DeviceToHost {
            snapshot.host_certificate = None;
            snapshot.host_certificate_invalidated = true;
        }
    }

    fn finish_prepare_failure(
        snapshot: &mut R33ExecutionSnapshotV1,
        observations: R33ExecutionObservationsV1,
        former: bool,
    ) {
        snapshot.operational_checks += 1;
        let operation_retake_succeeded = if former {
            observations.former_submit_loan.retake_succeeded
        } else {
            observations.fused_execution_loan.retake_succeeded
        };
        if !operation_retake_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(if former {
                R33TerminalStageV1::FormerSubmitLoanRetake
            } else {
                R33TerminalStageV1::FusedExecutionLoanRetake
            });
            return;
        }
        if former && !observations.former_submit_close_loan.open_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerSubmitCloseLoanOpen);
            return;
        }
        if former && !observations.former_submit_close_loan.retake_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerSubmitCloseLoanRetake);
            return;
        }
        if observations.preparation == R33PreparationObservationV1::RetryableFailure
            && observations.prepare_failure_close_current
        {
            snapshot.outcome = R33OutcomeV1::Retryable;
            snapshot.custody = R33CustodyV1::RetryableRequest;
        } else {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::PrepareFailureClose);
        }
    }

    fn publish_from_handoff(
        handoff: R33PreparedHandoffV1,
        returned_ticket: R33TicketV1,
        snapshot: &mut R33ExecutionSnapshotV1,
    ) {
        snapshot.binding = handoff.binding;
        snapshot.planned_ticket = Some(handoff.ticket);
        snapshot.ticket = Some(returned_ticket);
        snapshot.host_certificate = handoff.host_certificate;
        snapshot.host_certificate_invalidated = handoff.host_certificate_invalidated;
        snapshot.publication_attempted = true;
        snapshot.handoff_event_index = Some(3);
        snapshot.publication_event_index = Some(4);
    }

    fn finish_publication_failure(
        snapshot: &mut R33ExecutionSnapshotV1,
        observations: R33ExecutionObservationsV1,
        former: bool,
    ) {
        snapshot.operational_checks += 1;
        match observations.publication {
            R33PublicationObservationV1::Recoverable => {
                snapshot.ticket = None;
                let operation_retake_succeeded = if former {
                    observations.former_submit_loan.retake_succeeded
                } else {
                    observations.fused_execution_loan.retake_succeeded
                };
                let close_loan_succeeded =
                    !former || observations.former_submit_close_loan.succeeded();
                if operation_retake_succeeded
                    && close_loan_succeeded
                    && observations.former_submit_close_current
                {
                    snapshot.outcome = R33OutcomeV1::Retryable;
                    snapshot.custody = R33CustodyV1::RetryableRequest;
                } else {
                    snapshot.custody = R33CustodyV1::TerminalPrepared;
                    snapshot.terminal_stage = Some(if !operation_retake_succeeded {
                        if former {
                            R33TerminalStageV1::FormerSubmitLoanRetake
                        } else {
                            R33TerminalStageV1::FusedExecutionLoanRetake
                        }
                    } else if former && !observations.former_submit_close_loan.open_succeeded {
                        R33TerminalStageV1::FormerSubmitCloseLoanOpen
                    } else if former && !observations.former_submit_close_loan.retake_succeeded {
                        R33TerminalStageV1::FormerSubmitCloseLoanRetake
                    } else {
                        R33TerminalStageV1::SubmitClose
                    });
                }
            }
            R33PublicationObservationV1::Retained => {
                snapshot.custody = R33CustodyV1::TerminalPreparedQueueRetained;
                snapshot.terminal_stage = Some(R33TerminalStageV1::PublicationRetained);
            }
            R33PublicationObservationV1::Published => unreachable!(),
        }
    }

    fn finish_wait(
        snapshot: &mut R33ExecutionSnapshotV1,
        observations: R33ExecutionObservationsV1,
        fused: bool,
        loan: R33LoanOutcomeV1,
    ) {
        snapshot.wait_attempted = true;
        snapshot.wait_event_index = Some(if fused { 5 } else { 8 });
        snapshot.final_currentness_event_index = Some(if fused { 6 } else { 9 });
        snapshot.operational_checks += 1;
        match observations.wait {
            R33WaitObservationV1::Timeout
                if observations.final_current && loan.retake_succeeded =>
            {
                snapshot.outcome = R33OutcomeV1::Timeout;
                snapshot.custody = R33CustodyV1::PendingPublished;
            }
            R33WaitObservationV1::LowerFailure => {
                snapshot.custody = R33CustodyV1::TerminalPublished;
                snapshot.terminal_stage = Some(if !loan.retake_succeeded {
                    if fused {
                        R33TerminalStageV1::FusedExecutionLoanRetake
                    } else {
                        R33TerminalStageV1::FormerWaitLoanRetake
                    }
                } else if observations.final_current {
                    R33TerminalStageV1::LowerWait
                } else {
                    R33TerminalStageV1::FinalCurrentness
                });
            }
            R33WaitObservationV1::Completed if observations.final_current => {
                snapshot.lower_record_retired = true;
                snapshot.retirement_event_index = Some(if fused { 7 } else { 10 });
                if loan.retake_succeeded {
                    snapshot.completion_restoration_attempted = true;
                }
                if !loan.retake_succeeded || !observations.completion_restoration_succeeded {
                    snapshot.outcome = R33OutcomeV1::Terminal;
                    snapshot.custody = R33CustodyV1::TerminalCompletedUnrestored;
                    snapshot.terminal_stage = Some(if !loan.retake_succeeded {
                        if fused {
                            R33TerminalStageV1::FusedExecutionLoanRetake
                        } else {
                            R33TerminalStageV1::FormerWaitLoanRetake
                        }
                    } else {
                        R33TerminalStageV1::CompletionRestoration
                    });
                } else {
                    snapshot.outcome = R33OutcomeV1::Completed;
                    snapshot.custody = R33CustodyV1::Completed;
                }
            }
            R33WaitObservationV1::Completed => {
                snapshot.custody = R33CustodyV1::TerminalPublished;
                snapshot.terminal_stage = Some(R33TerminalStageV1::FinalCurrentness);
            }
            R33WaitObservationV1::Timeout => {
                snapshot.custody = R33CustodyV1::TerminalPublished;
                snapshot.terminal_stage = Some(if loan.retake_succeeded {
                    R33TerminalStageV1::FinalCurrentness
                } else if fused {
                    R33TerminalStageV1::FusedExecutionLoanRetake
                } else {
                    R33TerminalStageV1::FormerWaitLoanRetake
                });
            }
        }
    }

    pub fn run_former_model_only(
        &self,
        observations: R33ExecutionObservationsV1,
    ) -> R33ExecutionSnapshotV1 {
        let mut snapshot = self.initial_snapshot(true);
        if !observations.opening_loan.open_succeeded {
            snapshot.terminal_stage = Some(R33TerminalStageV1::OpeningLoanOpen);
            return snapshot;
        }
        if !observations.opening_current {
            snapshot.terminal_stage = Some(R33TerminalStageV1::Opening);
            return snapshot;
        }
        if !observations.opening_loan.retake_succeeded {
            snapshot.terminal_stage = Some(R33TerminalStageV1::OpeningLoanRetake);
            return snapshot;
        }
        self.construct_request(&mut snapshot);
        if !observations.former_submit_loan.open_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerSubmitLoanOpen);
            return snapshot;
        }
        if observations.preparation != R33PreparationObservationV1::Prepared {
            Self::finish_prepare_failure(&mut snapshot, observations, true);
            return snapshot;
        }
        snapshot.operational_checks += 1;
        if !observations.prepublication_current {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::Prepublication);
            return snapshot;
        }
        let planned_ticket = R33TicketV1::for_binding(self.binding);
        snapshot.planned_ticket = Some(planned_ticket);
        snapshot.publication_attempted = true;
        snapshot.ticket = Some(observations.returned_ticket);
        if observations.publication != R33PublicationObservationV1::Published {
            Self::finish_publication_failure(&mut snapshot, observations, true);
            return snapshot;
        }
        snapshot.operational_checks += 1;
        if !observations.former_submit_loan.retake_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerSubmitLoanRetake);
            return snapshot;
        }
        if !observations.former_submit_close_loan.open_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerSubmitCloseLoanOpen);
            return snapshot;
        }
        if !observations.former_submit_close_loan.retake_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerSubmitCloseLoanRetake);
            return snapshot;
        }
        if observations.returned_ticket != planned_ticket {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::TicketMismatch);
            return snapshot;
        }
        if !observations.former_submit_close_current {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::SubmitClose);
            return snapshot;
        }
        if !observations.former_wait_loan.open_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FormerWaitLoanOpen);
            return snapshot;
        }
        snapshot.operational_checks += 1;
        if !observations.former_wait_open_current {
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::WaitOpen);
            return snapshot;
        }
        Self::finish_wait(
            &mut snapshot,
            observations,
            false,
            observations.former_wait_loan,
        );
        snapshot
    }

    pub fn run_fused_model_only(
        &self,
        observations: R33ExecutionObservationsV1,
    ) -> R33ExecutionSnapshotV1 {
        let mut snapshot = self.initial_snapshot(false);
        if !observations.opening_loan.open_succeeded {
            snapshot.terminal_stage = Some(R33TerminalStageV1::OpeningLoanOpen);
            return snapshot;
        }
        if !observations.opening_current {
            snapshot.terminal_stage = Some(R33TerminalStageV1::Opening);
            return snapshot;
        }
        if !observations.opening_loan.retake_succeeded {
            snapshot.terminal_stage = Some(R33TerminalStageV1::OpeningLoanRetake);
            return snapshot;
        }
        self.construct_request(&mut snapshot);
        if !observations.fused_execution_loan.open_succeeded {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::FusedExecutionLoanOpen);
            return snapshot;
        }
        if observations.preparation != R33PreparationObservationV1::Prepared {
            Self::finish_prepare_failure(&mut snapshot, observations, false);
            return snapshot;
        }
        snapshot.operational_checks += 1;
        if !observations.prepublication_current {
            snapshot.custody = R33CustodyV1::TerminalPrepared;
            snapshot.terminal_stage = Some(R33TerminalStageV1::Prepublication);
            return snapshot;
        }
        let handoff = R33PreparedHandoffV1 {
            binding: self.binding,
            ticket: R33TicketV1::for_binding(self.binding),
            host_certificate: snapshot.host_certificate,
            host_certificate_invalidated: snapshot.host_certificate_invalidated,
        };
        Self::publish_from_handoff(handoff, observations.returned_ticket, &mut snapshot);
        if observations.publication != R33PublicationObservationV1::Published {
            Self::finish_publication_failure(&mut snapshot, observations, false);
            return snapshot;
        }
        if observations.returned_ticket != R33TicketV1::for_binding(self.binding) {
            snapshot.operational_checks += 1;
            snapshot.final_currentness_event_index = Some(5);
            snapshot.custody = R33CustodyV1::TerminalPublished;
            snapshot.terminal_stage = Some(R33TerminalStageV1::TicketMismatch);
            return snapshot;
        }
        Self::finish_wait(
            &mut snapshot,
            observations,
            true,
            observations.fused_execution_loan,
        );
        snapshot
    }
}
