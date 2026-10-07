//! Executable R28 model for a prepare-once persistent currentness scope.
//!
//! This finite model performs no I/O and grants no queue, memory, dispatch,
//! completion, currentness, or hardware authority. Full and operational audit
//! outcomes are contracted inputs, not proof of Linux environmental
//! currentness. The scope models the existing non-clone dispatch-resource
//! owner's `Ordinary`/`Attached`/`DataDetached` control state; it is not a new
//! production token. The model makes no production authority-conservation
//! claim; its private non-clone fields only prevent duplication inside this
//! finite executable state machine.

use crate::{IdentityDigestV1, QueueKeyV1};

pub const R28_PERSISTENT_HOT_CURRENTNESS_SCOPE_SCHEMA_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R28PersistentStableBindingV1 {
    pub queue: QueueKeyV1,
    pub memory_session_id: u64,
    pub control_identity: IdentityDigestV1,
    pub storage_identity: IdentityDigestV1,
    pub first_attachment_generation: u64,
    pub initial_predecessor_dispatch_generation: u64,
}

impl R28PersistentStableBindingV1 {
    pub fn is_valid(self) -> bool {
        self.queue.vm.device.physical.0 != 0
            && self.queue.vm.device.generation.0 != 0
            && self.queue.vm.id.0 != 0
            && self.queue.id.0 != 0
            && self.queue.generation.0 != 0
            && self.memory_session_id != 0
            && self.control_identity.as_bytes().iter().any(|byte| *byte != 0)
            && self.storage_identity.as_bytes().iter().any(|byte| *byte != 0)
            && self.first_attachment_generation != 0
            && self.first_attachment_generation.checked_add(1).is_some()
            // Production dispatch binding requires a nonzero successor, and a
            // subsequent retained replay must still have a successor.
            && self
                .initial_predecessor_dispatch_generation
                .checked_add(2)
                .is_some()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R28PersistentAttemptBindingV1 {
    pub stable: R28PersistentStableBindingV1,
    pub attachment_generation: u64,
    pub predecessor_dispatch_generation: u64,
    pub dispatch_generation: u64,
}

impl R28PersistentAttemptBindingV1 {
    pub const fn is_structurally_valid(self) -> bool {
        self.attachment_generation != 0
            && self.attachment_generation.checked_add(1).is_some()
            && self.dispatch_generation != 0
            && self.dispatch_generation.checked_add(1).is_some()
            && matches!(
                self.predecessor_dispatch_generation.checked_add(1),
                Some(next) if next == self.dispatch_generation
            )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28ContractedCurrentnessOutcomeV1 {
    Current,
    Lost,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R28SubmitCurrentnessV1 {
    pub before_counter: R28ContractedCurrentnessOutcomeV1,
    pub before_side_effect: R28ContractedCurrentnessOutcomeV1,
    pub after_publication: R28ContractedCurrentnessOutcomeV1,
}

impl R28SubmitCurrentnessV1 {
    pub const fn all_current() -> Self {
        Self {
            before_counter: R28ContractedCurrentnessOutcomeV1::Current,
            before_side_effect: R28ContractedCurrentnessOutcomeV1::Current,
            after_publication: R28ContractedCurrentnessOutcomeV1::Current,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R28CompletionCurrentnessV1 {
    pub before_observation: R28ContractedCurrentnessOutcomeV1,
    pub after_observation: R28ContractedCurrentnessOutcomeV1,
}

impl R28CompletionCurrentnessV1 {
    pub const fn all_current() -> Self {
        Self {
            before_observation: R28ContractedCurrentnessOutcomeV1::Current,
            after_observation: R28ContractedCurrentnessOutcomeV1::Current,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R28RecycleCurrentnessV1 {
    pub before_reset: R28ContractedCurrentnessOutcomeV1,
    pub after_reset: R28ContractedCurrentnessOutcomeV1,
}

impl R28RecycleCurrentnessV1 {
    pub const fn all_current() -> Self {
        Self {
            before_reset: R28ContractedCurrentnessOutcomeV1::Current,
            after_reset: R28ContractedCurrentnessOutcomeV1::Current,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28RingOccupancyV1 {
    Full,
    InsufficientSpace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28SubmitDispositionV1 {
    Published,
    Occupancy(R28RingOccupancyV1),
    StructuralFailureBeforeFirstCheckpoint,
    TerminalBeforeSideEffectAfterFirstCheckpoint,
    TerminalBeforeSideEffectAfterSecondCheckpoint,
    FailureAfterPossibleSideEffectBeforeFinalCheckpoint,
    FailureAfterFinalCheckpoint,
    PublicationLedgerFailureAfterFinalCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28CompletionDispositionV1 {
    Pending,
    Completed,
    TerminalFailureAfterFirstCheckpoint,
    CompletionLedgerFailureAfterSecondCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28RecycleDispositionV1 {
    Recycled,
    TerminalFailureAfterFirstCheckpoint,
    RecycleLedgerFailureAfterSecondCheckpoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28DetachDispositionV1 {
    Detached,
    PreflightFailure,
    ReleaseFailureAttached,
    StorageSubstitution,
    NativeRestoreFailure,
    SettlementFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28CancelDispositionV1 {
    Cancelled,
    ReleaseFailureAttached,
    ReleaseFailureDataDetached,
    StorageSubstitution,
    NativeRestoreFailure,
    LedgerFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28CloseDispositionV1 {
    ReleasedAndRetaken,
    ControlReleaseFailure,
    ModelRetakeFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28PersistentScopePhaseV1 {
    Inactive,
    Active,
    Cancelled,
    Closed,
    TerminalAbsorbed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28StableAuthorityCustodyV1 {
    Caller,
    Scope,
    Released,
    Opaque,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28PersistentControlStateV1 {
    Ordinary,
    Attached,
    DataDetached,
    Opaque,
}

/// Address-free stage matching production's retained terminal native custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28TerminalNativeCustodyStageV1 {
    Attached,
    Published,
    Completed,
    Recycled,
    DataDetached,
    StorageDetached,
    Restored,
    RetainedControl,
    ControlReleased,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28AttemptAuthorityCustodyV1 {
    Available,
    Prepared,
    Published,
    Completed,
    Recycled,
    Opaque,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28TerminalReasonV1 {
    FullOpenCurrentnessLost,
    ReplayBindCurrentnessLost,
    StableBindingSubstitution,
    PostAuthenticatedAttemptSubstitution,
    IllegalAttemptPhase,
    SubmitCurrentnessLostBeforeEffect,
    SubmitCurrentnessLostAfterPublication,
    StructuralSubmissionFailure,
    PossibleSubmissionSideEffect,
    PublicationLedgerFailure,
    CompletionCurrentnessLostBeforeObservation,
    CompletionCurrentnessLostAfterObservation,
    CompletionFailure,
    RecycleCurrentnessLostBeforeReset,
    RecycleCurrentnessLostAfterReset,
    RecycleFailure,
    CompletionLedgerFailure,
    RecycleLedgerFailure,
    DetachPreflightFailure,
    DetachReleaseFailure,
    DetachStorageSubstitution,
    DetachNativeRestoreFailure,
    DetachSettlementFailure,
    CancelReleaseFailure,
    CancelStorageSubstitution,
    CancelNativeRestoreFailure,
    CancelLedgerFailure,
    FullCloseCurrentnessLost,
    CloseControlReleaseFailure,
    CloseModelRetakeFailure,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R28PersistentHotCurrentnessErrorV1 {
    InvalidStableBinding,
    InvalidAttemptBinding,
    ReceiptMismatch,
    IllegalPhase,
    AttemptStillAttached,
    RetryableOccupancy(R28RingOccupancyV1),
    GenerationExhausted,
    TerminalAbsorbed,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R28PersistentHotCurrentnessSnapshotV1 {
    pub stable: R28PersistentStableBindingV1,
    pub scope_phase: R28PersistentScopePhaseV1,
    pub stable_custody: R28StableAuthorityCustodyV1,
    pub control_state: R28PersistentControlStateV1,
    pub attempt_custody: R28AttemptAuthorityCustodyV1,
    pub active_attempt: Option<R28PersistentAttemptBindingV1>,
    pub predecessor_dispatch_generation: u64,
    pub next_attachment_generation: u64,
    pub full_open_audit_count: u8,
    pub operational_checkpoint_count: u64,
    pub full_close_audit_count: u8,
    pub completed_attempt_count: u64,
    pub terminal_reason: Option<R28TerminalReasonV1>,
    pub terminal_custody: Option<R28TerminalNativeCustodyStageV1>,
}

struct R28StableAuthorityV1 {
    binding: R28PersistentStableBindingV1,
    custody: R28StableAuthorityCustodyV1,
}

enum R28AttemptAuthorityV1 {
    Available,
    Prepared(R28PersistentAttemptBindingV1),
    Published(R28PersistentAttemptBindingV1),
    Completed(R28PersistentAttemptBindingV1),
    Recycled(R28PersistentAttemptBindingV1),
    Opaque(Option<R28PersistentAttemptBindingV1>),
}

impl R28AttemptAuthorityV1 {
    const fn custody(&self) -> R28AttemptAuthorityCustodyV1 {
        match self {
            Self::Available => R28AttemptAuthorityCustodyV1::Available,
            Self::Prepared(_) => R28AttemptAuthorityCustodyV1::Prepared,
            Self::Published(_) => R28AttemptAuthorityCustodyV1::Published,
            Self::Completed(_) => R28AttemptAuthorityCustodyV1::Completed,
            Self::Recycled(_) => R28AttemptAuthorityCustodyV1::Recycled,
            Self::Opaque(_) => R28AttemptAuthorityCustodyV1::Opaque,
        }
    }

    const fn binding(&self) -> Option<R28PersistentAttemptBindingV1> {
        match self {
            Self::Available => None,
            Self::Prepared(binding)
            | Self::Published(binding)
            | Self::Completed(binding)
            | Self::Recycled(binding) => Some(*binding),
            Self::Opaque(binding) => *binding,
        }
    }
}

/// Sole executable owner of one modeled full-open/replay/full-close scope.
///
/// The owner and internal authorities deliberately do not implement `Clone` or
/// `Copy`. Snapshots are inert observations, not transition tokens.
pub struct R28PersistentHotCurrentnessScopeModelV1 {
    stable: R28StableAuthorityV1,
    attempt: R28AttemptAuthorityV1,
    control_state: R28PersistentControlStateV1,
    scope_phase: R28PersistentScopePhaseV1,
    predecessor_dispatch_generation: u64,
    next_attachment_generation: u64,
    full_open_audit_count: u8,
    operational_checkpoint_count: u64,
    full_close_audit_count: u8,
    completed_attempt_count: u64,
    terminal_reason: Option<R28TerminalReasonV1>,
    terminal_custody: Option<R28TerminalNativeCustodyStageV1>,
}

impl R28PersistentHotCurrentnessScopeModelV1 {
    pub fn new_model_only(
        stable: R28PersistentStableBindingV1,
    ) -> Result<Self, R28PersistentHotCurrentnessErrorV1> {
        if !stable.is_valid() {
            return Err(R28PersistentHotCurrentnessErrorV1::InvalidStableBinding);
        }
        Ok(Self {
            stable: R28StableAuthorityV1 {
                binding: stable,
                custody: R28StableAuthorityCustodyV1::Caller,
            },
            attempt: R28AttemptAuthorityV1::Available,
            control_state: R28PersistentControlStateV1::Ordinary,
            scope_phase: R28PersistentScopePhaseV1::Inactive,
            predecessor_dispatch_generation: stable.initial_predecessor_dispatch_generation,
            next_attachment_generation: stable.first_attachment_generation,
            full_open_audit_count: 0,
            operational_checkpoint_count: 0,
            full_close_audit_count: 0,
            completed_attempt_count: 0,
            terminal_reason: None,
            terminal_custody: None,
        })
    }

    pub fn snapshot(&self) -> R28PersistentHotCurrentnessSnapshotV1 {
        R28PersistentHotCurrentnessSnapshotV1 {
            stable: self.stable.binding,
            scope_phase: self.scope_phase,
            stable_custody: self.stable.custody,
            control_state: self.control_state,
            attempt_custody: self.attempt.custody(),
            active_attempt: self.attempt.binding(),
            predecessor_dispatch_generation: self.predecessor_dispatch_generation,
            next_attachment_generation: self.next_attachment_generation,
            full_open_audit_count: self.full_open_audit_count,
            operational_checkpoint_count: self.operational_checkpoint_count,
            full_close_audit_count: self.full_close_audit_count,
            completed_attempt_count: self.completed_attempt_count,
            terminal_reason: self.terminal_reason,
            terminal_custody: self.terminal_custody,
        }
    }

    #[cfg(test)]
    pub(crate) fn set_counters_for_test_only(
        &mut self,
        operational_checkpoint_count: u64,
        completed_attempt_count: u64,
    ) {
        self.operational_checkpoint_count = operational_checkpoint_count;
        self.completed_attempt_count = completed_attempt_count;
    }

    pub fn validate_global_invariants(&self) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        let active_binding_ok = match self.attempt.binding() {
            Some(binding) => {
                binding.is_structurally_valid()
                    && binding.stable == self.stable.binding
                    && binding.attachment_generation.checked_add(1)
                        == Some(self.next_attachment_generation)
                    && binding.predecessor_dispatch_generation
                        == self.predecessor_dispatch_generation
            }
            None => true,
        };
        let terminal_binding_ok = match self.terminal_custody {
            Some(
                R28TerminalNativeCustodyStageV1::RetainedControl
                | R28TerminalNativeCustodyStageV1::ControlReleased,
            ) => self.attempt.binding().is_none(),
            Some(_) => self.attempt.binding().is_some(),
            None => true,
        };
        let shape_ok = self.stable.binding.is_valid()
            && active_binding_ok
            && terminal_binding_ok
            && match self.scope_phase {
                R28PersistentScopePhaseV1::Inactive => {
                    self.stable.custody == R28StableAuthorityCustodyV1::Caller
                        && self.control_state == R28PersistentControlStateV1::Ordinary
                        && matches!(self.attempt, R28AttemptAuthorityV1::Available)
                        && self.full_open_audit_count == 0
                        && self.full_close_audit_count == 0
                        && self.terminal_reason.is_none()
                        && self.terminal_custody.is_none()
                }
                R28PersistentScopePhaseV1::Active => {
                    self.stable.custody == R28StableAuthorityCustodyV1::Scope
                        && self.full_open_audit_count == 1
                        && self.full_close_audit_count == 0
                        && self.terminal_reason.is_none()
                        && self.terminal_custody.is_none()
                        && match self.attempt {
                            R28AttemptAuthorityV1::Available => {
                                self.control_state == R28PersistentControlStateV1::DataDetached
                            }
                            R28AttemptAuthorityV1::Prepared(_)
                            | R28AttemptAuthorityV1::Published(_)
                            | R28AttemptAuthorityV1::Completed(_)
                            | R28AttemptAuthorityV1::Recycled(_) => {
                                self.control_state == R28PersistentControlStateV1::Attached
                            }
                            R28AttemptAuthorityV1::Opaque(_) => false,
                        }
                }
                R28PersistentScopePhaseV1::Cancelled => {
                    self.stable.custody == R28StableAuthorityCustodyV1::Released
                        && self.control_state == R28PersistentControlStateV1::Ordinary
                        && matches!(self.attempt, R28AttemptAuthorityV1::Available)
                        && self.full_open_audit_count == 1
                        && self.full_close_audit_count == 0
                        && self.terminal_reason.is_none()
                        && self.terminal_custody.is_none()
                }
                R28PersistentScopePhaseV1::Closed => {
                    self.stable.custody == R28StableAuthorityCustodyV1::Released
                        && self.control_state == R28PersistentControlStateV1::Ordinary
                        && matches!(self.attempt, R28AttemptAuthorityV1::Available)
                        && self.full_open_audit_count == 1
                        && self.full_close_audit_count == 1
                        && self.terminal_reason.is_none()
                        && self.terminal_custody.is_none()
                }
                R28PersistentScopePhaseV1::TerminalAbsorbed => {
                    self.stable.custody == R28StableAuthorityCustodyV1::Opaque
                        && self.control_state == R28PersistentControlStateV1::Opaque
                        && matches!(self.attempt, R28AttemptAuthorityV1::Opaque(_))
                        && self.terminal_reason.is_some()
                        && self.terminal_custody.is_some()
                }
            };
        if shape_ok {
            Ok(())
        } else {
            Err(R28PersistentHotCurrentnessErrorV1::InvariantViolation)
        }
    }

    pub fn preflight_receipt_model_only(
        &self,
        receipt: R28PersistentAttemptBindingV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        if self.scope_phase == R28PersistentScopePhaseV1::TerminalAbsorbed {
            return Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed);
        }
        if self.scope_phase != R28PersistentScopePhaseV1::Active {
            return Err(R28PersistentHotCurrentnessErrorV1::IllegalPhase);
        }
        if self.attempt.binding() == Some(receipt) {
            Ok(())
        } else {
            Err(R28PersistentHotCurrentnessErrorV1::ReceiptMismatch)
        }
    }

    pub fn open_model_only(
        &mut self,
        attempt: R28PersistentAttemptBindingV1,
        full_audit: R28ContractedCurrentnessOutcomeV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        if self.scope_phase != R28PersistentScopePhaseV1::Inactive {
            return Err(R28PersistentHotCurrentnessErrorV1::IllegalPhase);
        }
        self.validate_unconsumed_attempt(attempt)?;
        self.next_attachment_generation = attempt
            .attachment_generation
            .checked_add(1)
            .expect("validated attachment successor");
        self.stable.custody = R28StableAuthorityCustodyV1::Scope;
        self.control_state = R28PersistentControlStateV1::Attached;
        self.scope_phase = R28PersistentScopePhaseV1::Active;
        self.attempt = R28AttemptAuthorityV1::Prepared(attempt);
        self.full_open_audit_count = 1;
        if full_audit == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::FullOpenCurrentnessLost,
                Some(attempt),
                R28TerminalNativeCustodyStageV1::Attached,
            );
        }
        self.finish_transition()
    }

    pub fn prepare_replay_model_only(
        &mut self,
        attempt: R28PersistentAttemptBindingV1,
        replay_bind_audit: R28ContractedCurrentnessOutcomeV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        if self.scope_phase == R28PersistentScopePhaseV1::TerminalAbsorbed {
            return Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed);
        }
        if self.scope_phase != R28PersistentScopePhaseV1::Active
            || self.control_state != R28PersistentControlStateV1::DataDetached
            || !matches!(self.attempt, R28AttemptAuthorityV1::Available)
        {
            return Err(R28PersistentHotCurrentnessErrorV1::IllegalPhase);
        }
        self.validate_unconsumed_attempt(attempt)?;
        self.next_attachment_generation = attempt
            .attachment_generation
            .checked_add(1)
            .expect("validated attachment successor");
        self.control_state = R28PersistentControlStateV1::Attached;
        self.attempt = R28AttemptAuthorityV1::Prepared(attempt);
        self.record_operational_checkpoint();
        if replay_bind_audit == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::ReplayBindCurrentnessLost,
                Some(attempt),
                R28TerminalNativeCustodyStageV1::Attached,
            );
        }
        self.finish_transition()
    }

    /// Models a substitution found only after the expected receipt was already
    /// authenticated and consumed internally. Public receipt mismatch uses
    /// `preflight_receipt_model_only` and never mutates state.
    pub fn absorb_post_authenticated_substitution_model_only(
        &mut self,
        expected: R28PersistentAttemptBindingV1,
        observed: R28PersistentAttemptBindingV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.preflight_receipt_model_only(expected)?;
        if observed == expected {
            return Ok(());
        }
        let custody = self.current_terminal_custody();
        self.absorb(
            R28TerminalReasonV1::PostAuthenticatedAttemptSubstitution,
            Some(expected),
            custody,
        )
    }

    pub fn submit_model_only(
        &mut self,
        receipt: R28PersistentAttemptBindingV1,
        currentness: R28SubmitCurrentnessV1,
        disposition: R28SubmitDispositionV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.require_attempt(receipt, R28AttemptAuthorityCustodyV1::Prepared)?;
        if disposition == R28SubmitDispositionV1::StructuralFailureBeforeFirstCheckpoint {
            return self.absorb(
                R28TerminalReasonV1::StructuralSubmissionFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            );
        }
        self.record_operational_checkpoint();
        if currentness.before_counter == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::SubmitCurrentnessLostBeforeEffect,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            );
        }
        if disposition == R28SubmitDispositionV1::TerminalBeforeSideEffectAfterFirstCheckpoint {
            return self.absorb(
                R28TerminalReasonV1::StructuralSubmissionFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            );
        }
        self.record_operational_checkpoint();
        if currentness.before_side_effect == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::SubmitCurrentnessLostBeforeEffect,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            );
        }
        match disposition {
            R28SubmitDispositionV1::Occupancy(occupancy) => {
                self.finish_transition()?;
                Err(R28PersistentHotCurrentnessErrorV1::RetryableOccupancy(
                    occupancy,
                ))
            }
            R28SubmitDispositionV1::TerminalBeforeSideEffectAfterSecondCheckpoint => self.absorb(
                R28TerminalReasonV1::StructuralSubmissionFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            ),
            R28SubmitDispositionV1::FailureAfterPossibleSideEffectBeforeFinalCheckpoint => self
                .absorb(
                    R28TerminalReasonV1::PossibleSubmissionSideEffect,
                    Some(receipt),
                    R28TerminalNativeCustodyStageV1::Attached,
                ),
            R28SubmitDispositionV1::Published
            | R28SubmitDispositionV1::FailureAfterFinalCheckpoint
            | R28SubmitDispositionV1::PublicationLedgerFailureAfterFinalCheckpoint => {
                self.record_operational_checkpoint();
                if currentness.after_publication == R28ContractedCurrentnessOutcomeV1::Lost {
                    return self.absorb(
                        R28TerminalReasonV1::SubmitCurrentnessLostAfterPublication,
                        Some(receipt),
                        R28TerminalNativeCustodyStageV1::Attached,
                    );
                }
                match disposition {
                    R28SubmitDispositionV1::Published => {
                        self.attempt = R28AttemptAuthorityV1::Published(receipt);
                        self.finish_transition()
                    }
                    R28SubmitDispositionV1::FailureAfterFinalCheckpoint => self.absorb(
                        R28TerminalReasonV1::PossibleSubmissionSideEffect,
                        Some(receipt),
                        R28TerminalNativeCustodyStageV1::Attached,
                    ),
                    R28SubmitDispositionV1::PublicationLedgerFailureAfterFinalCheckpoint => self
                        .absorb(
                            R28TerminalReasonV1::PublicationLedgerFailure,
                            Some(receipt),
                            R28TerminalNativeCustodyStageV1::Published,
                        ),
                    _ => unreachable!(),
                }
            }
            R28SubmitDispositionV1::StructuralFailureBeforeFirstCheckpoint
            | R28SubmitDispositionV1::TerminalBeforeSideEffectAfterFirstCheckpoint => {
                unreachable!()
            }
        }
    }

    pub fn observe_completion_model_only(
        &mut self,
        receipt: R28PersistentAttemptBindingV1,
        currentness: R28CompletionCurrentnessV1,
        disposition: R28CompletionDispositionV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.require_attempt(receipt, R28AttemptAuthorityCustodyV1::Published)?;
        self.record_operational_checkpoint();
        if currentness.before_observation == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::CompletionCurrentnessLostBeforeObservation,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Published,
            );
        }
        match disposition {
            R28CompletionDispositionV1::TerminalFailureAfterFirstCheckpoint => {
                return self.absorb(
                    R28TerminalReasonV1::CompletionFailure,
                    Some(receipt),
                    R28TerminalNativeCustodyStageV1::Published,
                );
            }
            R28CompletionDispositionV1::Pending
            | R28CompletionDispositionV1::Completed
            | R28CompletionDispositionV1::CompletionLedgerFailureAfterSecondCheckpoint => {}
        }
        self.record_operational_checkpoint();
        if currentness.after_observation == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::CompletionCurrentnessLostAfterObservation,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Published,
            );
        }
        match disposition {
            R28CompletionDispositionV1::Pending => self.finish_transition(),
            R28CompletionDispositionV1::Completed => {
                self.attempt = R28AttemptAuthorityV1::Completed(receipt);
                self.finish_transition()
            }
            R28CompletionDispositionV1::CompletionLedgerFailureAfterSecondCheckpoint => self
                .absorb(
                    R28TerminalReasonV1::CompletionLedgerFailure,
                    Some(receipt),
                    R28TerminalNativeCustodyStageV1::Completed,
                ),
            R28CompletionDispositionV1::TerminalFailureAfterFirstCheckpoint => unreachable!(),
        }
    }

    pub fn recycle_model_only(
        &mut self,
        receipt: R28PersistentAttemptBindingV1,
        currentness: R28RecycleCurrentnessV1,
        disposition: R28RecycleDispositionV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.require_attempt(receipt, R28AttemptAuthorityCustodyV1::Completed)?;
        self.record_operational_checkpoint();
        if currentness.before_reset == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::RecycleCurrentnessLostBeforeReset,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Completed,
            );
        }
        match disposition {
            R28RecycleDispositionV1::TerminalFailureAfterFirstCheckpoint => {
                return self.absorb(
                    R28TerminalReasonV1::RecycleFailure,
                    Some(receipt),
                    R28TerminalNativeCustodyStageV1::Completed,
                );
            }
            R28RecycleDispositionV1::Recycled
            | R28RecycleDispositionV1::RecycleLedgerFailureAfterSecondCheckpoint => {}
        }
        self.record_operational_checkpoint();
        if currentness.after_reset == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::RecycleCurrentnessLostAfterReset,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Completed,
            );
        }
        match disposition {
            R28RecycleDispositionV1::Recycled => {
                self.attempt = R28AttemptAuthorityV1::Recycled(receipt);
                self.finish_transition()
            }
            R28RecycleDispositionV1::RecycleLedgerFailureAfterSecondCheckpoint => self.absorb(
                R28TerminalReasonV1::RecycleLedgerFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Recycled,
            ),
            R28RecycleDispositionV1::TerminalFailureAfterFirstCheckpoint => unreachable!(),
        }
    }

    pub fn detach_recycled_model_only(
        &mut self,
        receipt: R28PersistentAttemptBindingV1,
        disposition: R28DetachDispositionV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.require_attempt(receipt, R28AttemptAuthorityCustodyV1::Recycled)?;
        match disposition {
            R28DetachDispositionV1::Detached => {
                self.predecessor_dispatch_generation = receipt.dispatch_generation;
                self.completed_attempt_count = self.completed_attempt_count.saturating_add(1);
                self.control_state = R28PersistentControlStateV1::DataDetached;
                self.attempt = R28AttemptAuthorityV1::Available;
                self.finish_transition()
            }
            R28DetachDispositionV1::PreflightFailure => self.absorb(
                R28TerminalReasonV1::DetachPreflightFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            ),
            R28DetachDispositionV1::ReleaseFailureAttached => self.absorb(
                R28TerminalReasonV1::DetachReleaseFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            ),
            R28DetachDispositionV1::StorageSubstitution => self.absorb(
                R28TerminalReasonV1::DetachStorageSubstitution,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::DataDetached,
            ),
            R28DetachDispositionV1::NativeRestoreFailure => self.absorb(
                R28TerminalReasonV1::DetachNativeRestoreFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::StorageDetached,
            ),
            R28DetachDispositionV1::SettlementFailure => self.absorb(
                R28TerminalReasonV1::DetachSettlementFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Restored,
            ),
        }
    }

    pub fn cancel_prepared_model_only(
        &mut self,
        receipt: R28PersistentAttemptBindingV1,
        disposition: R28CancelDispositionV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.require_attempt(receipt, R28AttemptAuthorityCustodyV1::Prepared)?;
        match disposition {
            R28CancelDispositionV1::Cancelled => {
                self.scope_phase = R28PersistentScopePhaseV1::Cancelled;
                self.stable.custody = R28StableAuthorityCustodyV1::Released;
                self.control_state = R28PersistentControlStateV1::Ordinary;
                self.attempt = R28AttemptAuthorityV1::Available;
                self.finish_transition()
            }
            R28CancelDispositionV1::ReleaseFailureAttached => self.absorb(
                R28TerminalReasonV1::CancelReleaseFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Attached,
            ),
            R28CancelDispositionV1::ReleaseFailureDataDetached => self.absorb(
                R28TerminalReasonV1::CancelReleaseFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::DataDetached,
            ),
            R28CancelDispositionV1::StorageSubstitution => self.absorb(
                R28TerminalReasonV1::CancelStorageSubstitution,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::DataDetached,
            ),
            R28CancelDispositionV1::NativeRestoreFailure => self.absorb(
                R28TerminalReasonV1::CancelNativeRestoreFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::StorageDetached,
            ),
            R28CancelDispositionV1::LedgerFailure => self.absorb(
                R28TerminalReasonV1::CancelLedgerFailure,
                Some(receipt),
                R28TerminalNativeCustodyStageV1::Restored,
            ),
        }
    }

    pub fn close_model_only(
        &mut self,
        stable: R28PersistentStableBindingV1,
        full_audit: R28ContractedCurrentnessOutcomeV1,
        disposition: R28CloseDispositionV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        if self.scope_phase == R28PersistentScopePhaseV1::TerminalAbsorbed {
            return Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed);
        }
        if self.scope_phase != R28PersistentScopePhaseV1::Active {
            return Err(R28PersistentHotCurrentnessErrorV1::IllegalPhase);
        }
        if self.control_state != R28PersistentControlStateV1::DataDetached
            || !matches!(self.attempt, R28AttemptAuthorityV1::Available)
        {
            return Err(R28PersistentHotCurrentnessErrorV1::AttemptStillAttached);
        }
        if stable != self.stable.binding {
            return self.absorb(
                R28TerminalReasonV1::StableBindingSubstitution,
                None,
                R28TerminalNativeCustodyStageV1::RetainedControl,
            );
        }
        self.full_close_audit_count = 1;
        if full_audit == R28ContractedCurrentnessOutcomeV1::Lost {
            return self.absorb(
                R28TerminalReasonV1::FullCloseCurrentnessLost,
                None,
                R28TerminalNativeCustodyStageV1::RetainedControl,
            );
        }
        match disposition {
            R28CloseDispositionV1::ReleasedAndRetaken => {
                self.scope_phase = R28PersistentScopePhaseV1::Closed;
                self.stable.custody = R28StableAuthorityCustodyV1::Released;
                self.control_state = R28PersistentControlStateV1::Ordinary;
                self.finish_transition()
            }
            R28CloseDispositionV1::ControlReleaseFailure => self.absorb(
                R28TerminalReasonV1::CloseControlReleaseFailure,
                None,
                R28TerminalNativeCustodyStageV1::RetainedControl,
            ),
            R28CloseDispositionV1::ModelRetakeFailure => self.absorb(
                R28TerminalReasonV1::CloseModelRetakeFailure,
                None,
                R28TerminalNativeCustodyStageV1::ControlReleased,
            ),
        }
    }

    fn validate_unconsumed_attempt(
        &self,
        attempt: R28PersistentAttemptBindingV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        if attempt.stable != self.stable.binding {
            return Err(R28PersistentHotCurrentnessErrorV1::ReceiptMismatch);
        }
        if attempt.attachment_generation.checked_add(1).is_none()
            || attempt.dispatch_generation.checked_add(1).is_none()
        {
            return Err(R28PersistentHotCurrentnessErrorV1::GenerationExhausted);
        }
        if !attempt.is_structurally_valid()
            || attempt.attachment_generation != self.next_attachment_generation
            || attempt.predecessor_dispatch_generation != self.predecessor_dispatch_generation
        {
            return Err(R28PersistentHotCurrentnessErrorV1::InvalidAttemptBinding);
        }
        Ok(())
    }

    fn require_attempt(
        &mut self,
        receipt: R28PersistentAttemptBindingV1,
        required: R28AttemptAuthorityCustodyV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.preflight_receipt_model_only(receipt)?;
        if self.attempt.custody() != required {
            let custody = self.current_terminal_custody();
            return self.absorb(
                R28TerminalReasonV1::IllegalAttemptPhase,
                Some(receipt),
                custody,
            );
        }
        Ok(())
    }

    fn absorb(
        &mut self,
        reason: R28TerminalReasonV1,
        binding: Option<R28PersistentAttemptBindingV1>,
        custody: R28TerminalNativeCustodyStageV1,
    ) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        self.scope_phase = R28PersistentScopePhaseV1::TerminalAbsorbed;
        self.stable.custody = R28StableAuthorityCustodyV1::Opaque;
        self.control_state = R28PersistentControlStateV1::Opaque;
        self.attempt = R28AttemptAuthorityV1::Opaque(binding.or(self.attempt.binding()));
        self.terminal_reason = Some(reason);
        self.terminal_custody = Some(custody);
        Err(R28PersistentHotCurrentnessErrorV1::TerminalAbsorbed)
    }

    fn record_operational_checkpoint(&mut self) {
        self.operational_checkpoint_count = self.operational_checkpoint_count.saturating_add(1);
    }

    fn current_terminal_custody(&self) -> R28TerminalNativeCustodyStageV1 {
        match self.attempt.custody() {
            R28AttemptAuthorityCustodyV1::Prepared => R28TerminalNativeCustodyStageV1::Attached,
            R28AttemptAuthorityCustodyV1::Published => R28TerminalNativeCustodyStageV1::Published,
            R28AttemptAuthorityCustodyV1::Completed => R28TerminalNativeCustodyStageV1::Completed,
            R28AttemptAuthorityCustodyV1::Recycled => R28TerminalNativeCustodyStageV1::Recycled,
            R28AttemptAuthorityCustodyV1::Available => {
                if self.control_state == R28PersistentControlStateV1::DataDetached {
                    R28TerminalNativeCustodyStageV1::RetainedControl
                } else {
                    R28TerminalNativeCustodyStageV1::ControlReleased
                }
            }
            R28AttemptAuthorityCustodyV1::Opaque => self
                .terminal_custody
                .unwrap_or(R28TerminalNativeCustodyStageV1::Attached),
        }
    }

    fn finish_transition(&mut self) -> Result<(), R28PersistentHotCurrentnessErrorV1> {
        if self.validate_global_invariants().is_ok() {
            Ok(())
        } else {
            let custody = self.current_terminal_custody();
            self.absorb(
                R28TerminalReasonV1::InvariantViolation,
                self.attempt.binding(),
                custody,
            )
        }
    }
}
