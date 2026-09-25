//! Independent executable R35 model for fused retained-control replay binding.
//!
//! The model compares the former two-foundation-loan replay composition with
//! the R35 one-loan composition. All admission, native-operation, currentness,
//! cancellation, quarantine, and loan results are caller-supplied contracted
//! observations. This finite model performs no I/O and does not refine
//! production Rust, KFD, HSA, HIP, drivers, firmware, hardware, coherence,
//! progress, liveness, or performance.
//!
//! The comparison is only a custody-and-commit projection. It excludes
//! production/public error identity, the model's terminal failure stage,
//! internal prepared-authority label, event indices, foundation-loan counts,
//! and currentness counts. The last two counts have separate successful-path
//! checks; they are not part of the projected equivalence relation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35EffectV1 {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R35ReplayBindingV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub attachment_generation: u64,
    pub next_attachment_generation: u64,
    pub storage_identity: u64,
    pub predecessor_generation: u64,
    pub effect: R35EffectV1,
}

impl R35ReplayBindingV1 {
    pub const fn is_valid(self) -> bool {
        self.queue_id != 0
            && self.queue_generation != 0
            && self.attachment_generation != 0
            && matches!(self.attachment_generation.checked_add(1), Some(next)
                if next == self.next_attachment_generation)
            && self.storage_identity != 0
            && self.predecessor_generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35AdmissionObservationV1 {
    RetryableFailure,
    TerminalFailure,
    Admitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35PreparationObservationV1 {
    UseRequestRejected,
    ReserveRejected,
    PrepareRejected,
    Prepared,
}

impl R35PreparationObservationV1 {
    const fn prepared(self) -> bool {
        matches!(self, Self::Prepared)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R35LoanOutcomeV1 {
    pub open_succeeded: bool,
    pub retake_succeeded: bool,
}

impl R35LoanOutcomeV1 {
    pub const fn succeeded(self) -> bool {
        self.open_succeeded && self.retake_succeeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R35ReplayObservationsV1 {
    pub admission: R35AdmissionObservationV1,
    pub preparation: R35PreparationObservationV1,
    pub former_mapped_facts_loan: R35LoanOutcomeV1,
    pub former_retain_loan: R35LoanOutcomeV1,
    pub fused_loan: R35LoanOutcomeV1,
    pub mapped_facts_succeeded: bool,
    pub detach_succeeded: bool,
    pub authenticated_construction_succeeded: bool,
    pub retain_succeeded: bool,
    pub final_audit_succeeded: bool,
    pub cancellation_succeeded: bool,
    pub session_healthy: bool,
    pub quarantine_succeeded: bool,
}

impl R35ReplayObservationsV1 {
    /// Input-only relation connecting observations whose timing changed.
    /// It calls neither runner and compares no output state.
    pub const fn loan_equivalence_premise(self) -> bool {
        if !matches!(self.admission, R35AdmissionObservationV1::Admitted)
            || !self.preparation.prepared()
        {
            return true;
        }
        if self.former_mapped_facts_loan.open_succeeded != self.fused_loan.open_succeeded {
            return false;
        }
        if !self.former_mapped_facts_loan.open_succeeded {
            return true;
        }
        if !self.mapped_facts_succeeded {
            return self.former_mapped_facts_loan.succeeded() == self.fused_loan.succeeded();
        }
        if !self.former_mapped_facts_loan.retake_succeeded {
            return !self.detach_succeeded && !self.fused_loan.retake_succeeded;
        }
        if !self.detach_succeeded {
            return self.fused_loan.retake_succeeded;
        }
        if !self.authenticated_construction_succeeded {
            return true;
        }
        if !self.former_retain_loan.open_succeeded {
            return !self.retain_succeeded;
        }
        if !self.retain_succeeded || !self.final_audit_succeeded {
            return true;
        }
        if !self.former_retain_loan.retake_succeeded {
            return !self.fused_loan.retake_succeeded;
        }
        self.fused_loan.retake_succeeded
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35OutcomeV1 {
    Retryable,
    Prepared,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35CustodyV1 {
    RetryableInput,
    PreparedAttachment,
    TerminalInput,
    TerminalStorage,
    TerminalData,
    TerminalAttached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35PreparedAuthorityStateV1 {
    Prepared,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R35TerminalStageV1 {
    Admission,
    Preparation,
    FormerMappedFactsLoanOpen,
    FormerMappedFactsLoanRetake,
    FormerRetainLoanOpen,
    FormerRetainLoanRetake,
    FusedLoanOpen,
    FusedLoanRetake,
    MappedFacts,
    Detach,
    AuthenticatedConstruction,
    Retain,
    FinalAudit,
    Cancellation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R35AttachmentV1 {
    pub queue_id: u64,
    pub queue_generation: u64,
    pub attachment_generation: u64,
    pub storage_identity: u64,
    pub predecessor_generation: u64,
    pub effect: R35EffectV1,
    pub authority_state: R35PreparedAuthorityStateV1,
    pub terminal_custody: Option<R35CustodyV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R35ReplaySnapshotV1 {
    pub binding: R35ReplayBindingV1,
    pub outcome: R35OutcomeV1,
    pub custody: R35CustodyV1,
    pub terminal_stage: Option<R35TerminalStageV1>,
    pub terminal_poisoned: bool,
    pub dispatch_retained: bool,
    pub attachment: Option<R35AttachmentV1>,
    pub next_attachment_generation: u64,
    pub detached_data_count: u8,
    pub detached_generation: Option<u64>,
    pub detached_identity_count: u8,
    pub detached_next_insertion_index: Option<u8>,
    pub admission_event_index: u8,
    pub preparation_event_index: Option<u8>,
    pub detach_event_index: Option<u8>,
    pub commit_event_index: Option<u8>,
    pub foundation_loan_attempts: u8,
    pub currentness_observations: u8,
}

impl R35ReplaySnapshotV1 {
    /// Equality of the modeled custody and commit-coordinate projection.
    ///
    /// This excludes production/public error identity, terminal failure stage,
    /// internal authority label, event indices, foundation-loan counts, and
    /// currentness counts.
    pub fn same_projected_custody_and_commit_semantics(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.outcome == other.outcome
            && self.custody == other.custody
            && self.terminal_poisoned == other.terminal_poisoned
            && self.dispatch_retained == other.dispatch_retained
            && self.next_attachment_generation == other.next_attachment_generation
            && self.detached_data_count == other.detached_data_count
            && self.detached_generation == other.detached_generation
            && self.detached_identity_count == other.detached_identity_count
            && self.detached_next_insertion_index == other.detached_next_insertion_index
            && match (self.attachment, other.attachment) {
                (None, None) => true,
                (Some(left), Some(right)) => {
                    left.queue_id == right.queue_id
                        && left.queue_generation == right.queue_generation
                        && left.attachment_generation == right.attachment_generation
                        && left.storage_identity == right.storage_identity
                        && left.predecessor_generation == right.predecessor_generation
                        && left.effect == right.effect
                        && left.terminal_custody == right.terminal_custody
                }
                _ => false,
            }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R35ModelErrorV1 {
    InvalidBinding,
}

/// Move-only owner for the finite retained-control replay comparison.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R35EffectV1, R35ReplayBindingV1,
///     R35RetainedControlReplayModelV1};
/// let binding = R35ReplayBindingV1 {
///     queue_id: 1, queue_generation: 2, attachment_generation: 3,
///     next_attachment_generation: 4, storage_identity: 5,
///     predecessor_generation: 6, effect: R35EffectV1::ReadWrite,
/// };
/// let owner = R35RetainedControlReplayModelV1::new_model_only(binding).unwrap();
/// let duplicated = owner.clone();
/// # let _ = duplicated;
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R35RetainedControlReplayModelV1 {
    binding: R35ReplayBindingV1,
}

struct R35DetachedReplayAuthorityV1 {
    binding: R35ReplayBindingV1,
}

struct R35ReplayDataAuthorityV1 {
    detached: R35DetachedReplayAuthorityV1,
}

struct R35AttachedReplayAuthorityV1 {
    detached: R35DetachedReplayAuthorityV1,
}

impl R35RetainedControlReplayModelV1 {
    pub fn new_model_only(binding: R35ReplayBindingV1) -> Result<Self, R35ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R35ModelErrorV1::InvalidBinding);
        }
        Ok(Self { binding })
    }

    fn initial_snapshot(&self) -> R35ReplaySnapshotV1 {
        R35ReplaySnapshotV1 {
            binding: self.binding,
            outcome: R35OutcomeV1::Terminal,
            custody: R35CustodyV1::TerminalInput,
            terminal_stage: None,
            terminal_poisoned: false,
            dispatch_retained: true,
            attachment: None,
            next_attachment_generation: self.binding.attachment_generation,
            detached_data_count: 0,
            detached_generation: Some(self.binding.predecessor_generation),
            detached_identity_count: 0,
            detached_next_insertion_index: Some(0),
            admission_event_index: 1,
            preparation_event_index: None,
            detach_event_index: None,
            commit_event_index: None,
            foundation_loan_attempts: 0,
            currentness_observations: 0,
        }
    }

    fn enter_replay(
        &self,
        snapshot: &mut R35ReplaySnapshotV1,
        observations: R35ReplayObservationsV1,
    ) -> bool {
        match observations.admission {
            R35AdmissionObservationV1::RetryableFailure => {
                snapshot.outcome = R35OutcomeV1::Retryable;
                snapshot.custody = R35CustodyV1::RetryableInput;
                false
            }
            R35AdmissionObservationV1::TerminalFailure => {
                snapshot.terminal_poisoned = true;
                snapshot.terminal_stage = Some(R35TerminalStageV1::Admission);
                false
            }
            R35AdmissionObservationV1::Admitted => {
                snapshot.preparation_event_index = Some(2);
                snapshot.currentness_observations = 1;
                if observations.preparation.prepared() {
                    true
                } else {
                    snapshot.outcome = R35OutcomeV1::Retryable;
                    snapshot.custody = R35CustodyV1::RetryableInput;
                    snapshot.terminal_stage = Some(R35TerminalStageV1::Preparation);
                    false
                }
            }
        }
    }

    const fn authority_state(observations: R35ReplayObservationsV1) -> R35PreparedAuthorityStateV1 {
        if observations.quarantine_succeeded {
            R35PreparedAuthorityStateV1::Quarantined
        } else {
            R35PreparedAuthorityStateV1::Prepared
        }
    }

    fn attach_terminal(
        &self,
        snapshot: &mut R35ReplaySnapshotV1,
        custody: R35CustodyV1,
        stage: R35TerminalStageV1,
        observations: R35ReplayObservationsV1,
        fused: bool,
    ) {
        snapshot.custody = custody;
        snapshot.terminal_stage = Some(stage);
        snapshot.terminal_poisoned = true;
        snapshot.next_attachment_generation = self.binding.next_attachment_generation;
        snapshot.attachment = Some(R35AttachmentV1 {
            queue_id: self.binding.queue_id,
            queue_generation: self.binding.queue_generation,
            attachment_generation: self.binding.attachment_generation,
            storage_identity: self.binding.storage_identity,
            predecessor_generation: self.binding.predecessor_generation,
            effect: self.binding.effect,
            authority_state: if fused {
                Self::authority_state(observations)
            } else {
                // The former Rust path labeled this state quarantined even when
                // the quarantine operation failed. R35 retains Prepared.
                R35PreparedAuthorityStateV1::Quarantined
            },
            terminal_custody: Some(custody),
        });
    }

    fn finish_before_detach(
        &self,
        snapshot: &mut R35ReplaySnapshotV1,
        observations: R35ReplayObservationsV1,
        loan_succeeded: bool,
        stage: R35TerminalStageV1,
        fused: bool,
    ) {
        if loan_succeeded && observations.cancellation_succeeded && observations.session_healthy {
            snapshot.outcome = R35OutcomeV1::Retryable;
            snapshot.custody = R35CustodyV1::RetryableInput;
            snapshot.terminal_stage = Some(stage);
        } else if observations.cancellation_succeeded {
            snapshot.custody = R35CustodyV1::TerminalInput;
            snapshot.terminal_poisoned = true;
            snapshot.terminal_stage = Some(stage);
        } else {
            self.attach_terminal(
                snapshot,
                R35CustodyV1::TerminalAttached,
                R35TerminalStageV1::Cancellation,
                observations,
                fused,
            );
        }
    }

    fn commit_success(
        &self,
        snapshot: &mut R35ReplaySnapshotV1,
        attached: R35AttachedReplayAuthorityV1,
    ) {
        debug_assert_eq!(attached.detached.binding, self.binding);
        snapshot.outcome = R35OutcomeV1::Prepared;
        snapshot.custody = R35CustodyV1::PreparedAttachment;
        snapshot.next_attachment_generation = self.binding.next_attachment_generation;
        snapshot.detached_generation = None;
        snapshot.detached_next_insertion_index = None;
        snapshot.commit_event_index = Some(9);
        snapshot.attachment = Some(R35AttachmentV1 {
            queue_id: self.binding.queue_id,
            queue_generation: self.binding.queue_generation,
            attachment_generation: self.binding.attachment_generation,
            storage_identity: self.binding.storage_identity,
            predecessor_generation: self.binding.predecessor_generation,
            effect: self.binding.effect,
            authority_state: R35PreparedAuthorityStateV1::Prepared,
            terminal_custody: None,
        });
    }

    pub fn run_former_model_only(
        self,
        observations: R35ReplayObservationsV1,
    ) -> R35ReplaySnapshotV1 {
        let mut snapshot = self.initial_snapshot();
        if !self.enter_replay(&mut snapshot, observations) {
            return snapshot;
        }
        snapshot.foundation_loan_attempts = 1;
        let mapped_loan = observations.former_mapped_facts_loan;
        if !mapped_loan.open_succeeded {
            self.finish_before_detach(
                &mut snapshot,
                observations,
                false,
                R35TerminalStageV1::FormerMappedFactsLoanOpen,
                false,
            );
            return snapshot;
        }
        if !observations.mapped_facts_succeeded || !mapped_loan.retake_succeeded {
            self.finish_before_detach(
                &mut snapshot,
                observations,
                mapped_loan.succeeded(),
                if observations.mapped_facts_succeeded {
                    R35TerminalStageV1::FormerMappedFactsLoanRetake
                } else {
                    R35TerminalStageV1::MappedFacts
                },
                false,
            );
            return snapshot;
        }
        if !observations.detach_succeeded {
            self.finish_before_detach(
                &mut snapshot,
                observations,
                true,
                R35TerminalStageV1::Detach,
                false,
            );
            return snapshot;
        }
        snapshot.detach_event_index = Some(4);
        let detached = R35DetachedReplayAuthorityV1 {
            binding: self.binding,
        };
        if !observations.authenticated_construction_succeeded {
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalStorage,
                R35TerminalStageV1::AuthenticatedConstruction,
                observations,
                false,
            );
            return snapshot;
        }
        let data = R35ReplayDataAuthorityV1 { detached };
        snapshot.foundation_loan_attempts = 2;
        if !observations.former_retain_loan.open_succeeded {
            let R35ReplayDataAuthorityV1 { detached: _ } = data;
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalData,
                R35TerminalStageV1::FormerRetainLoanOpen,
                observations,
                false,
            );
            return snapshot;
        }
        if !observations.retain_succeeded {
            let R35ReplayDataAuthorityV1 { detached: _ } = data;
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalData,
                R35TerminalStageV1::Retain,
                observations,
                false,
            );
            return snapshot;
        }
        let R35ReplayDataAuthorityV1 { detached } = data;
        let attached = R35AttachedReplayAuthorityV1 { detached };
        if !observations.former_retain_loan.retake_succeeded {
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalAttached,
                R35TerminalStageV1::FormerRetainLoanRetake,
                observations,
                false,
            );
            return snapshot;
        }
        snapshot.currentness_observations = 2;
        if !observations.final_audit_succeeded {
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalAttached,
                R35TerminalStageV1::FinalAudit,
                observations,
                false,
            );
            return snapshot;
        }
        self.commit_success(&mut snapshot, attached);
        snapshot
    }

    pub fn run_fused_model_only(
        self,
        observations: R35ReplayObservationsV1,
    ) -> R35ReplaySnapshotV1 {
        let mut snapshot = self.initial_snapshot();
        if !self.enter_replay(&mut snapshot, observations) {
            return snapshot;
        }
        snapshot.foundation_loan_attempts = 1;
        if !observations.fused_loan.open_succeeded {
            self.finish_before_detach(
                &mut snapshot,
                observations,
                false,
                R35TerminalStageV1::FusedLoanOpen,
                true,
            );
            return snapshot;
        }
        if !observations.mapped_facts_succeeded {
            self.finish_before_detach(
                &mut snapshot,
                observations,
                observations.fused_loan.succeeded(),
                R35TerminalStageV1::MappedFacts,
                true,
            );
            return snapshot;
        }
        if !observations.detach_succeeded {
            self.finish_before_detach(
                &mut snapshot,
                observations,
                observations.fused_loan.succeeded(),
                R35TerminalStageV1::Detach,
                true,
            );
            return snapshot;
        }
        snapshot.detach_event_index = Some(4);
        let detached = R35DetachedReplayAuthorityV1 {
            binding: self.binding,
        };
        if !observations.authenticated_construction_succeeded {
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalStorage,
                R35TerminalStageV1::AuthenticatedConstruction,
                observations,
                true,
            );
            return snapshot;
        }
        let data = R35ReplayDataAuthorityV1 { detached };
        if !observations.retain_succeeded {
            let R35ReplayDataAuthorityV1 { detached: _ } = data;
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalData,
                R35TerminalStageV1::Retain,
                observations,
                true,
            );
            return snapshot;
        }
        let R35ReplayDataAuthorityV1 { detached } = data;
        let attached = R35AttachedReplayAuthorityV1 { detached };
        snapshot.currentness_observations = 2;
        if !observations.final_audit_succeeded {
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalAttached,
                R35TerminalStageV1::FinalAudit,
                observations,
                true,
            );
            return snapshot;
        }
        if !observations.fused_loan.retake_succeeded {
            self.attach_terminal(
                &mut snapshot,
                R35CustodyV1::TerminalAttached,
                R35TerminalStageV1::FusedLoanRetake,
                observations,
                true,
            );
            return snapshot;
        }
        self.commit_success(&mut snapshot, attached);
        snapshot
    }
}
