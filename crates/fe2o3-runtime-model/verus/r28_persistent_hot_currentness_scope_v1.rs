// Independent finite R28 persistent-currentness policy model. Every audit
// result is a contracted input. This proves policy/typestate conservation; it
// does not prove Linux environmental currentness, Rust ownership, KFD/HSA/HIP
// behavior, hardware behavior, or performance.
use vstd::prelude::*;

verus! {

pub open spec fn max_generation_v1() -> nat { 18_446_744_073_709_551_615 }

#[derive(PartialEq, Eq)]
pub struct StableBindingV1 {
    pub queue: nat,
    pub memory_session: nat,
    pub control_identity: nat,
    pub storage_identity: nat,
    pub first_attachment: nat,
    pub initial_predecessor: nat,
}

#[derive(PartialEq, Eq)]
pub struct AttemptBindingV1 {
    pub stable: StableBindingV1,
    pub attachment: nat,
    pub predecessor: nat,
    pub dispatch: nat,
}

#[derive(PartialEq, Eq)]
pub enum ScopePhaseV1 { Inactive, Active, Cancelled, Closed, TerminalAbsorbed }

#[derive(PartialEq, Eq)]
pub enum StableCustodyV1 { Caller, Scope, Released, Opaque }

#[derive(PartialEq, Eq)]
pub enum ControlStateV1 { Ordinary, Attached, DataDetached, Opaque }

#[derive(PartialEq, Eq)]
pub enum AttemptPhaseV1 { Available, Prepared, Published, Completed, Recycled, Opaque }

#[derive(PartialEq, Eq)]
pub enum TerminalNativeCustodyStageV1 {
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

// These are contracted observations supplied by a future concrete refinement.
// `Current` is not a proof about Linux or hardware.
#[derive(PartialEq, Eq)]
pub enum ContractedAuditOutcomeV1 { Current, Lost }

#[derive(PartialEq, Eq)]
pub struct SubmitCurrentnessV1 {
    pub before_counter: ContractedAuditOutcomeV1,
    pub before_side_effect: ContractedAuditOutcomeV1,
    pub after_publication: ContractedAuditOutcomeV1,
}

#[derive(PartialEq, Eq)]
pub struct CompletionCurrentnessV1 {
    pub before_observation: ContractedAuditOutcomeV1,
    pub after_observation: ContractedAuditOutcomeV1,
}

#[derive(PartialEq, Eq)]
pub struct RecycleCurrentnessV1 {
    pub before_reset: ContractedAuditOutcomeV1,
    pub after_reset: ContractedAuditOutcomeV1,
}

#[derive(PartialEq, Eq)]
pub enum RingOccupancyV1 { Full, InsufficientSpace }

#[derive(PartialEq, Eq)]
pub enum SubmitDispositionV1 {
    Published,
    Occupancy(RingOccupancyV1),
    StructuralFailureBeforeFirstCheckpoint,
    TerminalBeforeSideEffectAfterFirstCheckpoint,
    TerminalBeforeSideEffectAfterSecondCheckpoint,
    FailureAfterPossibleSideEffectBeforeFinalCheckpoint,
    FailureAfterFinalCheckpoint,
    PublicationLedgerFailureAfterFinalCheckpoint,
}

#[derive(PartialEq, Eq)]
pub enum CompletionDispositionV1 {
    Pending,
    Completed,
    TerminalFailureAfterFirstCheckpoint,
    CompletionLedgerFailureAfterSecondCheckpoint,
}

#[derive(PartialEq, Eq)]
pub enum RecycleDispositionV1 {
    Recycled,
    TerminalFailureAfterFirstCheckpoint,
    RecycleLedgerFailureAfterSecondCheckpoint,
}

#[derive(PartialEq, Eq)]
pub enum DetachDispositionV1 {
    Detached,
    PreflightFailure,
    ReleaseFailureAttached,
    StorageSubstitution,
    NativeRestoreFailure,
    SettlementFailure,
}

#[derive(PartialEq, Eq)]
pub enum CancelDispositionV1 {
    Cancelled,
    ReleaseFailureAttached,
    ReleaseFailureDataDetached,
    StorageSubstitution,
    NativeRestoreFailure,
    LedgerFailure,
}

#[derive(PartialEq, Eq)]
pub enum CloseDispositionV1 { ReleasedAndRetaken, ControlReleaseFailure, ModelRetakeFailure }

#[derive(PartialEq, Eq)]
pub enum TerminalReasonV1 {
    FullOpenLost,
    ReplayBindLost,
    PostAuthenticatedSubstitution,
    IllegalAttemptPhase,
    SubmitLostBeforeEffect,
    SubmitLostAfterPublication,
    StructuralSubmission,
    PossibleSubmissionSideEffect,
    PublicationLedgerFailure,
    CompletionLostBeforeObservation,
    CompletionLostAfterObservation,
    CompletionFailure,
    CompletionLedgerFailure,
    RecycleLostBeforeReset,
    RecycleLostAfterReset,
    RecycleFailure,
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
    FullCloseLost,
    CloseFailure,
    StableSubstitution,
}

#[derive(PartialEq, Eq)]
pub struct ScopeStateV1 {
    pub stable: StableBindingV1,
    pub attempt: Option<AttemptBindingV1>,
    pub scope: ScopePhaseV1,
    pub stable_custody: StableCustodyV1,
    pub control: ControlStateV1,
    pub attempt_phase: AttemptPhaseV1,
    pub predecessor: nat,
    pub next_attachment: nat,
    pub full_open_audits: nat,
    // Mathematical observations do not drive the production state machine.
    pub operational_checkpoints: nat,
    pub full_close_audits: nat,
    pub completed_attempts: nat,
    pub terminal_reason: Option<TerminalReasonV1>,
    pub terminal_custody: Option<TerminalNativeCustodyStageV1>,
}

pub open spec fn valid_stable_v1(stable: StableBindingV1) -> bool {
    &&& stable.queue > 0 && stable.memory_session > 0
    &&& stable.control_identity > 0 && stable.storage_identity > 0
    &&& 0 < stable.first_attachment < max_generation_v1()
    &&& stable.initial_predecessor + 2 <= max_generation_v1()
}

pub open spec fn valid_attempt_shape_v1(attempt: AttemptBindingV1) -> bool {
    &&& 0 < attempt.attachment < max_generation_v1()
    &&& 0 < attempt.dispatch < max_generation_v1()
    &&& attempt.dispatch == attempt.predecessor + 1
}

pub open spec fn exact_next_attempt_v1(state: ScopeStateV1, attempt: AttemptBindingV1) -> bool {
    &&& valid_attempt_shape_v1(attempt)
    &&& attempt.stable == state.stable
    &&& attempt.attachment == state.next_attachment
    &&& attempt.predecessor == state.predecessor
}

pub open spec fn exact_active_attempt_v1(state: ScopeStateV1, attempt: AttemptBindingV1) -> bool {
    &&& state.attempt == Some(attempt)
    &&& attempt.stable == state.stable
    &&& attempt.attachment + 1 == state.next_attachment
    &&& attempt.predecessor == state.predecessor
}

pub open spec fn terminal_stage_has_matching_attempt_v1(state: ScopeStateV1) -> bool {
    match state.terminal_custody {
        Some(TerminalNativeCustodyStageV1::RetainedControl)
        | Some(TerminalNativeCustodyStageV1::ControlReleased) => state.attempt.is_none(),
        Some(_) => state.attempt.is_some(),
        None => true,
    }
}

pub open spec fn terminal_custody_for_attempt_phase_v1(state: ScopeStateV1)
    -> TerminalNativeCustodyStageV1
{
    match state.attempt_phase {
        AttemptPhaseV1::Published => TerminalNativeCustodyStageV1::Published,
        AttemptPhaseV1::Completed => TerminalNativeCustodyStageV1::Completed,
        AttemptPhaseV1::Recycled => TerminalNativeCustodyStageV1::Recycled,
        AttemptPhaseV1::Available => if state.control == ControlStateV1::DataDetached {
            TerminalNativeCustodyStageV1::RetainedControl
        } else {
            TerminalNativeCustodyStageV1::ControlReleased
        },
        _ => TerminalNativeCustodyStageV1::Attached,
    }
}

pub open spec fn valid_state_v1(state: ScopeStateV1) -> bool {
    &&& valid_stable_v1(state.stable)
    &&& state.full_open_audits <= 1 && state.full_close_audits <= 1
    &&& terminal_stage_has_matching_attempt_v1(state)
    &&& match state.attempt {
        Some(attempt) => valid_attempt_shape_v1(attempt)
            && attempt.stable == state.stable
            && attempt.attachment + 1 == state.next_attachment
            && attempt.predecessor == state.predecessor,
        None => true,
    }
    &&& match state.scope {
        ScopePhaseV1::Inactive =>
            state.stable_custody == StableCustodyV1::Caller
                && state.control == ControlStateV1::Ordinary
                && state.attempt_phase == AttemptPhaseV1::Available
                && state.attempt.is_none() && state.full_open_audits == 0
                && state.full_close_audits == 0 && state.terminal_reason.is_none()
                && state.terminal_custody.is_none(),
        ScopePhaseV1::Active =>
            state.stable_custody == StableCustodyV1::Scope
                && state.full_open_audits == 1 && state.full_close_audits == 0
                && state.terminal_reason.is_none() && state.terminal_custody.is_none()
                && match state.attempt_phase {
                    AttemptPhaseV1::Available => state.attempt.is_none()
                        && state.control == ControlStateV1::DataDetached,
                    AttemptPhaseV1::Prepared | AttemptPhaseV1::Published
                        | AttemptPhaseV1::Completed | AttemptPhaseV1::Recycled =>
                            state.attempt.is_some() && state.control == ControlStateV1::Attached,
                    AttemptPhaseV1::Opaque => false,
                },
        ScopePhaseV1::Cancelled =>
            state.stable_custody == StableCustodyV1::Released
                && state.control == ControlStateV1::Ordinary
                && state.attempt_phase == AttemptPhaseV1::Available
                && state.attempt.is_none() && state.full_open_audits == 1
                && state.full_close_audits == 0 && state.terminal_reason.is_none()
                && state.terminal_custody.is_none(),
        ScopePhaseV1::Closed =>
            state.stable_custody == StableCustodyV1::Released
                && state.control == ControlStateV1::Ordinary
                && state.attempt_phase == AttemptPhaseV1::Available
                && state.attempt.is_none() && state.full_open_audits == 1
                && state.full_close_audits == 1 && state.terminal_reason.is_none()
                && state.terminal_custody.is_none(),
        ScopePhaseV1::TerminalAbsorbed =>
            state.stable_custody == StableCustodyV1::Opaque
                && state.control == ControlStateV1::Opaque
                && state.attempt_phase == AttemptPhaseV1::Opaque
                && state.terminal_reason.is_some() && state.terminal_custody.is_some(),
    }
}

pub open spec fn initial_state_v1(stable: StableBindingV1) -> ScopeStateV1 {
    ScopeStateV1 {
        stable, attempt: None, scope: ScopePhaseV1::Inactive,
        stable_custody: StableCustodyV1::Caller, control: ControlStateV1::Ordinary,
        attempt_phase: AttemptPhaseV1::Available,
        predecessor: stable.initial_predecessor, next_attachment: stable.first_attachment,
        full_open_audits: 0, operational_checkpoints: 0, full_close_audits: 0,
        completed_attempts: 0, terminal_reason: None, terminal_custody: None,
    }
}

pub open spec fn terminalize_v1(state: ScopeStateV1, reason: TerminalReasonV1,
    custody: TerminalNativeCustodyStageV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else { ScopeStateV1 {
        scope: ScopePhaseV1::TerminalAbsorbed, stable_custody: StableCustodyV1::Opaque,
        control: ControlStateV1::Opaque, attempt_phase: AttemptPhaseV1::Opaque,
        terminal_reason: Some(reason), terminal_custody: Some(custody), ..state
    }}
}

pub open spec fn open_scope_v1(state: ScopeStateV1, attempt: AttemptBindingV1,
    audit: ContractedAuditOutcomeV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if state.scope != ScopePhaseV1::Inactive || !exact_next_attempt_v1(state, attempt) { state }
    else {
        let consumed = ScopeStateV1 {
            attempt: Some(attempt), scope: ScopePhaseV1::Active,
            stable_custody: StableCustodyV1::Scope, control: ControlStateV1::Attached,
            attempt_phase: AttemptPhaseV1::Prepared,
            next_attachment: attempt.attachment + 1, full_open_audits: 1, ..state
        };
        match audit {
            ContractedAuditOutcomeV1::Current => consumed,
            ContractedAuditOutcomeV1::Lost => terminalize_v1(consumed,
                TerminalReasonV1::FullOpenLost, TerminalNativeCustodyStageV1::Attached),
        }
    }
}

pub open spec fn prepare_replay_v1(state: ScopeStateV1, attempt: AttemptBindingV1,
    audit: ContractedAuditOutcomeV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if state.scope == ScopePhaseV1::Active
        && state.control == ControlStateV1::DataDetached
        && state.attempt_phase == AttemptPhaseV1::Available
        && state.attempt.is_none() && exact_next_attempt_v1(state, attempt)
    {
        let consumed = ScopeStateV1 {
            attempt: Some(attempt), control: ControlStateV1::Attached,
            attempt_phase: AttemptPhaseV1::Prepared,
            next_attachment: attempt.attachment + 1,
            operational_checkpoints: state.operational_checkpoints + 1, ..state
        };
        match audit {
            ContractedAuditOutcomeV1::Current => consumed,
            ContractedAuditOutcomeV1::Lost => terminalize_v1(consumed,
                TerminalReasonV1::ReplayBindLost, TerminalNativeCustodyStageV1::Attached),
        }
    } else { state }
}

pub open spec fn post_authenticated_substitution_v1(state: ScopeStateV1,
    expected: AttemptBindingV1, observed: AttemptBindingV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if exact_active_attempt_v1(state, expected) && observed != expected {
        let custody = terminal_custody_for_attempt_phase_v1(state);
        terminalize_v1(state, TerminalReasonV1::PostAuthenticatedSubstitution, custody)
    } else { state }
}

pub open spec fn submit_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    currentness: SubmitCurrentnessV1, disposition: SubmitDispositionV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if !exact_active_attempt_v1(state, receipt) { state }
    else if state.scope != ScopePhaseV1::Active || state.attempt_phase != AttemptPhaseV1::Prepared {
        terminalize_v1(state, TerminalReasonV1::IllegalAttemptPhase,
            terminal_custody_for_attempt_phase_v1(state))
    } else if disposition == SubmitDispositionV1::StructuralFailureBeforeFirstCheckpoint {
        terminalize_v1(state, TerminalReasonV1::StructuralSubmission,
            TerminalNativeCustodyStageV1::Attached)
    } else {
        let first = ScopeStateV1 {
            operational_checkpoints: state.operational_checkpoints + 1, ..state
        };
        if currentness.before_counter == ContractedAuditOutcomeV1::Lost {
            terminalize_v1(first, TerminalReasonV1::SubmitLostBeforeEffect,
                TerminalNativeCustodyStageV1::Attached)
        } else if disposition == SubmitDispositionV1::TerminalBeforeSideEffectAfterFirstCheckpoint {
            terminalize_v1(first, TerminalReasonV1::StructuralSubmission,
                TerminalNativeCustodyStageV1::Attached)
        } else {
            let second = ScopeStateV1 {
                operational_checkpoints: first.operational_checkpoints + 1, ..first
            };
            if currentness.before_side_effect == ContractedAuditOutcomeV1::Lost {
                terminalize_v1(second, TerminalReasonV1::SubmitLostBeforeEffect,
                    TerminalNativeCustodyStageV1::Attached)
            } else { match disposition {
                SubmitDispositionV1::Occupancy(_) => second,
                SubmitDispositionV1::TerminalBeforeSideEffectAfterSecondCheckpoint =>
                    terminalize_v1(second, TerminalReasonV1::StructuralSubmission,
                        TerminalNativeCustodyStageV1::Attached),
                SubmitDispositionV1::FailureAfterPossibleSideEffectBeforeFinalCheckpoint =>
                    terminalize_v1(second, TerminalReasonV1::PossibleSubmissionSideEffect,
                        TerminalNativeCustodyStageV1::Attached),
                SubmitDispositionV1::Published
                | SubmitDispositionV1::FailureAfterFinalCheckpoint
                | SubmitDispositionV1::PublicationLedgerFailureAfterFinalCheckpoint => {
                    let third = ScopeStateV1 {
                        operational_checkpoints: second.operational_checkpoints + 1, ..second
                    };
                    if currentness.after_publication == ContractedAuditOutcomeV1::Lost {
                        terminalize_v1(third, TerminalReasonV1::SubmitLostAfterPublication,
                            TerminalNativeCustodyStageV1::Attached)
                    } else { match disposition {
                        SubmitDispositionV1::Published => ScopeStateV1 {
                            attempt_phase: AttemptPhaseV1::Published, ..third
                        },
                        SubmitDispositionV1::FailureAfterFinalCheckpoint => terminalize_v1(third,
                            TerminalReasonV1::PossibleSubmissionSideEffect,
                            TerminalNativeCustodyStageV1::Attached),
                        SubmitDispositionV1::PublicationLedgerFailureAfterFinalCheckpoint =>
                            terminalize_v1(third, TerminalReasonV1::PublicationLedgerFailure,
                                TerminalNativeCustodyStageV1::Published),
                        _ => third,
                    }}
                },
                _ => second,
            }}
        }
    }
}

pub open spec fn observe_completion_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    currentness: CompletionCurrentnessV1, disposition: CompletionDispositionV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if !exact_active_attempt_v1(state, receipt) { state }
    else if state.scope != ScopePhaseV1::Active || state.attempt_phase != AttemptPhaseV1::Published {
        terminalize_v1(state, TerminalReasonV1::IllegalAttemptPhase,
            terminal_custody_for_attempt_phase_v1(state))
    } else {
        let first = ScopeStateV1 {
            operational_checkpoints: state.operational_checkpoints + 1, ..state
        };
        if currentness.before_observation == ContractedAuditOutcomeV1::Lost {
            terminalize_v1(first, TerminalReasonV1::CompletionLostBeforeObservation,
                TerminalNativeCustodyStageV1::Published)
        } else if disposition == CompletionDispositionV1::TerminalFailureAfterFirstCheckpoint {
            terminalize_v1(first, TerminalReasonV1::CompletionFailure,
                TerminalNativeCustodyStageV1::Published)
        } else {
            let second = ScopeStateV1 {
                operational_checkpoints: first.operational_checkpoints + 1, ..first
            };
            if currentness.after_observation == ContractedAuditOutcomeV1::Lost {
                terminalize_v1(second, TerminalReasonV1::CompletionLostAfterObservation,
                    TerminalNativeCustodyStageV1::Published)
            } else { match disposition {
                CompletionDispositionV1::Pending => second,
                CompletionDispositionV1::Completed => ScopeStateV1 {
                    attempt_phase: AttemptPhaseV1::Completed, ..second
                },
                CompletionDispositionV1::CompletionLedgerFailureAfterSecondCheckpoint =>
                    terminalize_v1(second, TerminalReasonV1::CompletionLedgerFailure,
                        TerminalNativeCustodyStageV1::Completed),
                _ => second,
            }}
        }
    }
}

pub open spec fn recycle_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    currentness: RecycleCurrentnessV1, disposition: RecycleDispositionV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if !exact_active_attempt_v1(state, receipt) { state }
    else if state.scope != ScopePhaseV1::Active || state.attempt_phase != AttemptPhaseV1::Completed {
        terminalize_v1(state, TerminalReasonV1::IllegalAttemptPhase,
            terminal_custody_for_attempt_phase_v1(state))
    } else {
        let first = ScopeStateV1 {
            operational_checkpoints: state.operational_checkpoints + 1, ..state
        };
        if currentness.before_reset == ContractedAuditOutcomeV1::Lost {
            terminalize_v1(first, TerminalReasonV1::RecycleLostBeforeReset,
                TerminalNativeCustodyStageV1::Completed)
        } else if disposition == RecycleDispositionV1::TerminalFailureAfterFirstCheckpoint {
            terminalize_v1(first, TerminalReasonV1::RecycleFailure,
                TerminalNativeCustodyStageV1::Completed)
        } else {
            let second = ScopeStateV1 {
                operational_checkpoints: first.operational_checkpoints + 1, ..first
            };
            if currentness.after_reset == ContractedAuditOutcomeV1::Lost {
                terminalize_v1(second, TerminalReasonV1::RecycleLostAfterReset,
                    TerminalNativeCustodyStageV1::Completed)
            } else { match disposition {
                RecycleDispositionV1::Recycled => ScopeStateV1 {
                    attempt_phase: AttemptPhaseV1::Recycled, ..second
                },
                RecycleDispositionV1::RecycleLedgerFailureAfterSecondCheckpoint =>
                    terminalize_v1(second, TerminalReasonV1::RecycleLedgerFailure,
                        TerminalNativeCustodyStageV1::Recycled),
                _ => second,
            }}
        }
    }
}

pub open spec fn detach_recycled_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    disposition: DetachDispositionV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if !exact_active_attempt_v1(state, receipt) { state }
    else if state.scope != ScopePhaseV1::Active || state.attempt_phase != AttemptPhaseV1::Recycled {
        terminalize_v1(state, TerminalReasonV1::IllegalAttemptPhase,
            terminal_custody_for_attempt_phase_v1(state))
    } else { match disposition {
        DetachDispositionV1::Detached => ScopeStateV1 {
            attempt: None, control: ControlStateV1::DataDetached,
            attempt_phase: AttemptPhaseV1::Available, predecessor: receipt.dispatch,
            completed_attempts: state.completed_attempts + 1, ..state
        },
        DetachDispositionV1::PreflightFailure => terminalize_v1(state,
            TerminalReasonV1::DetachPreflightFailure, TerminalNativeCustodyStageV1::Attached),
        DetachDispositionV1::ReleaseFailureAttached => terminalize_v1(state,
            TerminalReasonV1::DetachReleaseFailure, TerminalNativeCustodyStageV1::Attached),
        DetachDispositionV1::StorageSubstitution => terminalize_v1(state,
            TerminalReasonV1::DetachStorageSubstitution, TerminalNativeCustodyStageV1::DataDetached),
        DetachDispositionV1::NativeRestoreFailure => terminalize_v1(state,
            TerminalReasonV1::DetachNativeRestoreFailure,
            TerminalNativeCustodyStageV1::StorageDetached),
        DetachDispositionV1::SettlementFailure => terminalize_v1(state,
            TerminalReasonV1::DetachSettlementFailure, TerminalNativeCustodyStageV1::Restored),
    }}
}

pub open spec fn cancel_prepared_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    disposition: CancelDispositionV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if !exact_active_attempt_v1(state, receipt) { state }
    else if state.scope != ScopePhaseV1::Active || state.attempt_phase != AttemptPhaseV1::Prepared {
        terminalize_v1(state, TerminalReasonV1::IllegalAttemptPhase,
            terminal_custody_for_attempt_phase_v1(state))
    } else { match disposition {
        CancelDispositionV1::Cancelled => ScopeStateV1 {
            attempt: None, scope: ScopePhaseV1::Cancelled,
            stable_custody: StableCustodyV1::Released, control: ControlStateV1::Ordinary,
            attempt_phase: AttemptPhaseV1::Available, ..state
        },
        CancelDispositionV1::ReleaseFailureAttached => terminalize_v1(state,
            TerminalReasonV1::CancelReleaseFailure, TerminalNativeCustodyStageV1::Attached),
        CancelDispositionV1::ReleaseFailureDataDetached => terminalize_v1(state,
            TerminalReasonV1::CancelReleaseFailure, TerminalNativeCustodyStageV1::DataDetached),
        CancelDispositionV1::StorageSubstitution => terminalize_v1(state,
            TerminalReasonV1::CancelStorageSubstitution, TerminalNativeCustodyStageV1::DataDetached),
        CancelDispositionV1::NativeRestoreFailure => terminalize_v1(state,
            TerminalReasonV1::CancelNativeRestoreFailure,
            TerminalNativeCustodyStageV1::StorageDetached),
        CancelDispositionV1::LedgerFailure => terminalize_v1(state,
            TerminalReasonV1::CancelLedgerFailure, TerminalNativeCustodyStageV1::Restored),
    }}
}

pub open spec fn close_scope_v1(state: ScopeStateV1, stable: StableBindingV1,
    audit: ContractedAuditOutcomeV1, disposition: CloseDispositionV1) -> ScopeStateV1
{
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if state.scope != ScopePhaseV1::Active || state.control != ControlStateV1::DataDetached
        || state.attempt_phase != AttemptPhaseV1::Available || state.attempt.is_some() { state }
    else if stable != state.stable { terminalize_v1(state, TerminalReasonV1::StableSubstitution,
        TerminalNativeCustodyStageV1::RetainedControl) }
    else {
        let audited = ScopeStateV1 { full_close_audits: 1, ..state };
        if audit == ContractedAuditOutcomeV1::Lost {
            terminalize_v1(audited, TerminalReasonV1::FullCloseLost,
                TerminalNativeCustodyStageV1::RetainedControl)
        } else { match disposition {
            CloseDispositionV1::ReleasedAndRetaken => ScopeStateV1 {
                scope: ScopePhaseV1::Closed, stable_custody: StableCustodyV1::Released,
                control: ControlStateV1::Ordinary, ..audited
            },
            CloseDispositionV1::ControlReleaseFailure => terminalize_v1(audited,
                TerminalReasonV1::CloseFailure, TerminalNativeCustodyStageV1::RetainedControl),
            CloseDispositionV1::ModelRetakeFailure => terminalize_v1(audited,
                TerminalReasonV1::CloseFailure, TerminalNativeCustodyStageV1::ControlReleased),
        }}
    }
}

pub open spec fn current_submit_v1() -> SubmitCurrentnessV1 {
    SubmitCurrentnessV1 { before_counter: ContractedAuditOutcomeV1::Current,
        before_side_effect: ContractedAuditOutcomeV1::Current,
        after_publication: ContractedAuditOutcomeV1::Current }
}

pub open spec fn current_completion_v1() -> CompletionCurrentnessV1 {
    CompletionCurrentnessV1 { before_observation: ContractedAuditOutcomeV1::Current,
        after_observation: ContractedAuditOutcomeV1::Current }
}

pub open spec fn current_recycle_v1() -> RecycleCurrentnessV1 {
    RecycleCurrentnessV1 { before_reset: ContractedAuditOutcomeV1::Current,
        after_reset: ContractedAuditOutcomeV1::Current }
}

pub open spec fn sample_stable_v1() -> StableBindingV1 {
    StableBindingV1 { queue: 1, memory_session: 2, control_identity: 3, storage_identity: 4,
        first_attachment: 1, initial_predecessor: 40 }
}

pub open spec fn sample_attempt_v1() -> AttemptBindingV1 {
    AttemptBindingV1 { stable: sample_stable_v1(), attachment: 1, predecessor: 40, dispatch: 41 }
}

pub open spec fn sample_open_v1() -> ScopeStateV1 {
    open_scope_v1(initial_state_v1(sample_stable_v1()), sample_attempt_v1(),
        ContractedAuditOutcomeV1::Current)
}

pub proof fn sample_initial_and_open_are_valid_v1()
    ensures valid_stable_v1(sample_stable_v1()),
        valid_attempt_shape_v1(sample_attempt_v1()),
        valid_state_v1(initial_state_v1(sample_stable_v1())),
        valid_state_v1(sample_open_v1()), {}

pub proof fn generation_exhaustion_is_preflight_atomic_v1(state: ScopeStateV1,
    attempt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Inactive,
        attempt.stable == state.stable,
        (attempt.attachment >= max_generation_v1() || attempt.dispatch >= max_generation_v1()),
    ensures !exact_next_attempt_v1(state, attempt),
        open_scope_v1(state, attempt, ContractedAuditOutcomeV1::Current) == state, {}

pub proof fn open_consumes_generation_and_performs_one_full_audit_v1(state: ScopeStateV1,
    attempt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Inactive,
        exact_next_attempt_v1(state, attempt),
    ensures {
        let opened = open_scope_v1(state, attempt, ContractedAuditOutcomeV1::Current);
        &&& opened.scope == ScopePhaseV1::Active
        &&& opened.stable_custody == StableCustodyV1::Scope
        &&& opened.attempt_phase == AttemptPhaseV1::Prepared
        &&& opened.next_attachment == attempt.attachment + 1
        &&& opened.full_open_audits == 1
    }, {}

pub proof fn failed_open_has_exact_attached_custody_v1(state: ScopeStateV1,
    attempt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Inactive,
        exact_next_attempt_v1(state, attempt),
    ensures {
        let failed = open_scope_v1(state, attempt, ContractedAuditOutcomeV1::Lost);
        &&& failed.scope == ScopePhaseV1::TerminalAbsorbed
        &&& failed.next_attachment == attempt.attachment + 1
        &&& failed.full_open_audits == 1
        &&& failed.terminal_custody == Some(TerminalNativeCustodyStageV1::Attached)
    }, {}

pub proof fn replay_bind_is_one_fresh_checkpoint_and_consumes_generation_v1(
    state: ScopeStateV1, attempt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available, state.attempt.is_none(),
        exact_next_attempt_v1(state, attempt),
    ensures {
        let replay = prepare_replay_v1(state, attempt, ContractedAuditOutcomeV1::Current);
        &&& replay.scope == ScopePhaseV1::Active
        &&& replay.attempt_phase == AttemptPhaseV1::Prepared
        &&& replay.operational_checkpoints == state.operational_checkpoints + 1
        &&& replay.next_attachment == attempt.attachment + 1
    }, {}

pub proof fn replay_bind_loss_absorbs_after_checkpoint_v1(state: ScopeStateV1,
    attempt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available, state.attempt.is_none(),
        exact_next_attempt_v1(state, attempt),
    ensures {
        let failed = prepare_replay_v1(state, attempt, ContractedAuditOutcomeV1::Lost);
        &&& failed.scope == ScopePhaseV1::TerminalAbsorbed
        &&& failed.operational_checkpoints == state.operational_checkpoints + 1
        &&& failed.terminal_custody == Some(TerminalNativeCustodyStageV1::Attached)
    }, {}

pub proof fn public_receipt_mismatch_is_atomic_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope != ScopePhaseV1::TerminalAbsorbed,
        !exact_active_attempt_v1(state, receipt),
    ensures submit_v1(state, receipt, current_submit_v1(), SubmitDispositionV1::Published) == state,
        observe_completion_v1(state, receipt, current_completion_v1(),
            CompletionDispositionV1::Completed) == state,
        recycle_v1(state, receipt, current_recycle_v1(), RecycleDispositionV1::Recycled) == state, {}

pub proof fn post_authenticated_substitution_absorbs_v1(state: ScopeStateV1,
    expected: AttemptBindingV1, observed: AttemptBindingV1)
    requires valid_state_v1(state), state.scope != ScopePhaseV1::TerminalAbsorbed,
        exact_active_attempt_v1(state, expected), observed != expected,
    ensures post_authenticated_substitution_v1(state, expected, observed).scope
        == ScopePhaseV1::TerminalAbsorbed, {}

pub proof fn occupancy_is_exact_pre_effect_retry_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1, occupancy: RingOccupancyV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let retry = submit_v1(state, receipt, current_submit_v1(),
            SubmitDispositionV1::Occupancy(occupancy));
        &&& retry.scope == ScopePhaseV1::Active && retry.attempt == state.attempt
        &&& retry.attempt_phase == AttemptPhaseV1::Prepared
        &&& retry.operational_checkpoints == state.operational_checkpoints + 2
    }, {}

pub proof fn published_submit_has_three_exact_checkpoints_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let published = submit_v1(state, receipt, current_submit_v1(),
            SubmitDispositionV1::Published);
        &&& published.scope == ScopePhaseV1::Active
        &&& published.attempt_phase == AttemptPhaseV1::Published
        &&& published.operational_checkpoints == state.operational_checkpoints + 3
    }, {}

pub proof fn submit_failures_have_exact_stage_and_checkpoint_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let first = submit_v1(state, receipt, current_submit_v1(),
            SubmitDispositionV1::TerminalBeforeSideEffectAfterFirstCheckpoint);
        let second = submit_v1(state, receipt, current_submit_v1(),
            SubmitDispositionV1::FailureAfterPossibleSideEffectBeforeFinalCheckpoint);
        let third = submit_v1(state, receipt, current_submit_v1(),
            SubmitDispositionV1::FailureAfterFinalCheckpoint);
        let ledger = submit_v1(state, receipt, current_submit_v1(),
            SubmitDispositionV1::PublicationLedgerFailureAfterFinalCheckpoint);
        &&& first.operational_checkpoints == state.operational_checkpoints + 1
        &&& second.operational_checkpoints == state.operational_checkpoints + 2
        &&& third.operational_checkpoints == state.operational_checkpoints + 3
        &&& ledger.operational_checkpoints == state.operational_checkpoints + 3
        &&& first.terminal_custody == Some(TerminalNativeCustodyStageV1::Attached)
        &&& second.terminal_custody == Some(TerminalNativeCustodyStageV1::Attached)
        &&& third.terminal_custody == Some(TerminalNativeCustodyStageV1::Attached)
        &&& ledger.terminal_custody == Some(TerminalNativeCustodyStageV1::Published)
    }, {}

pub proof fn completion_has_two_exact_checkpoints_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Published,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let completed = observe_completion_v1(state, receipt, current_completion_v1(),
            CompletionDispositionV1::Completed);
        &&& completed.attempt_phase == AttemptPhaseV1::Completed
        &&& completed.operational_checkpoints == state.operational_checkpoints + 2
    }, {}

pub proof fn completion_failures_have_exact_custody_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Published,
        exact_active_attempt_v1(state, receipt),
    ensures observe_completion_v1(state, receipt, current_completion_v1(),
            CompletionDispositionV1::TerminalFailureAfterFirstCheckpoint).terminal_custody
            == Some(TerminalNativeCustodyStageV1::Published),
        observe_completion_v1(state, receipt, current_completion_v1(),
            CompletionDispositionV1::CompletionLedgerFailureAfterSecondCheckpoint).terminal_custody
            == Some(TerminalNativeCustodyStageV1::Completed), {}

pub proof fn recycle_has_two_exact_checkpoints_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Completed,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let recycled = recycle_v1(state, receipt, current_recycle_v1(),
            RecycleDispositionV1::Recycled);
        &&& recycled.attempt_phase == AttemptPhaseV1::Recycled
        &&& recycled.operational_checkpoints == state.operational_checkpoints + 2
    }, {}

pub proof fn recycle_failures_have_exact_custody_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Completed,
        exact_active_attempt_v1(state, receipt),
    ensures recycle_v1(state, receipt, current_recycle_v1(),
            RecycleDispositionV1::TerminalFailureAfterFirstCheckpoint).terminal_custody
            == Some(TerminalNativeCustodyStageV1::Completed),
        recycle_v1(state, receipt, current_recycle_v1(),
            RecycleDispositionV1::RecycleLedgerFailureAfterSecondCheckpoint).terminal_custody
            == Some(TerminalNativeCustodyStageV1::Recycled), {}

pub proof fn initial_checkpoint_budget_is_exact_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let published = submit_v1(state, receipt, current_submit_v1(), SubmitDispositionV1::Published);
        let completed = observe_completion_v1(published, receipt, current_completion_v1(),
            CompletionDispositionV1::Completed);
        let recycled = recycle_v1(completed, receipt, current_recycle_v1(),
            RecycleDispositionV1::Recycled);
        recycled.operational_checkpoints == state.operational_checkpoints + 7
    }, {}

pub proof fn detach_success_retains_control_and_advances_predecessor_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Recycled,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let detached = detach_recycled_v1(state, receipt, DetachDispositionV1::Detached);
        &&& detached.scope == ScopePhaseV1::Active
        &&& detached.control == ControlStateV1::DataDetached
        &&& detached.attempt_phase == AttemptPhaseV1::Available
        &&& detached.predecessor == receipt.dispatch
    }, {}

pub proof fn detach_failures_preserve_exact_terminal_stage_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Recycled,
        exact_active_attempt_v1(state, receipt),
    ensures detach_recycled_v1(state, receipt,
            DetachDispositionV1::StorageSubstitution).terminal_custody
            == Some(TerminalNativeCustodyStageV1::DataDetached),
        detach_recycled_v1(state, receipt,
            DetachDispositionV1::NativeRestoreFailure).terminal_custody
            == Some(TerminalNativeCustodyStageV1::StorageDetached),
        detach_recycled_v1(state, receipt,
            DetachDispositionV1::SettlementFailure).terminal_custody
            == Some(TerminalNativeCustodyStageV1::Restored), {}

pub proof fn cancel_consumes_control_and_failures_retain_exact_stage_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures {
        let cancelled = cancel_prepared_v1(state, receipt, CancelDispositionV1::Cancelled);
        let restore_failure = cancel_prepared_v1(state, receipt,
            CancelDispositionV1::NativeRestoreFailure);
        let ledger_failure = cancel_prepared_v1(state, receipt, CancelDispositionV1::LedgerFailure);
        &&& cancelled.scope == ScopePhaseV1::Cancelled
        &&& cancelled.stable_custody == StableCustodyV1::Released
        &&& cancelled.control == ControlStateV1::Ordinary
        &&& restore_failure.terminal_custody
            == Some(TerminalNativeCustodyStageV1::StorageDetached)
        &&& ledger_failure.terminal_custody == Some(TerminalNativeCustodyStageV1::Restored)
    }, {}

pub proof fn close_checks_closeability_before_stable_authentication_v1(state: ScopeStateV1,
    substituted: StableBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        (state.control != ControlStateV1::DataDetached
            || state.attempt_phase != AttemptPhaseV1::Available || state.attempt.is_some()),
    ensures close_scope_v1(state, substituted, ContractedAuditOutcomeV1::Current,
        CloseDispositionV1::ReleasedAndRetaken) == state, {}

pub proof fn close_has_one_full_audit_and_exact_failure_custody_v1(state: ScopeStateV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available, state.attempt.is_none(),
    ensures {
        let closed = close_scope_v1(state, state.stable, ContractedAuditOutcomeV1::Current,
            CloseDispositionV1::ReleasedAndRetaken);
        let retained = close_scope_v1(state, state.stable, ContractedAuditOutcomeV1::Current,
            CloseDispositionV1::ControlReleaseFailure);
        let released = close_scope_v1(state, state.stable, ContractedAuditOutcomeV1::Current,
            CloseDispositionV1::ModelRetakeFailure);
        &&& closed.scope == ScopePhaseV1::Closed && closed.full_close_audits == 1
        &&& retained.terminal_custody == Some(TerminalNativeCustodyStageV1::RetainedControl)
        &&& retained.full_close_audits == 1
        &&& released.terminal_custody == Some(TerminalNativeCustodyStageV1::ControlReleased)
        &&& released.full_close_audits == 1
    }, {}

pub proof fn open_preserves_valid_state_v1(state: ScopeStateV1, attempt: AttemptBindingV1,
    audit: ContractedAuditOutcomeV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Inactive,
        exact_next_attempt_v1(state, attempt),
    ensures valid_state_v1(open_scope_v1(state, attempt, audit)), {}

pub proof fn replay_preserves_valid_state_v1(state: ScopeStateV1, attempt: AttemptBindingV1,
    audit: ContractedAuditOutcomeV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available, state.attempt.is_none(),
        exact_next_attempt_v1(state, attempt),
    ensures valid_state_v1(prepare_replay_v1(state, attempt, audit)), {}

pub proof fn submit_preserves_valid_state_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    currentness: SubmitCurrentnessV1, disposition: SubmitDispositionV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures valid_state_v1(submit_v1(state, receipt, currentness, disposition)), {}

pub proof fn completion_preserves_valid_state_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1, currentness: CompletionCurrentnessV1,
    disposition: CompletionDispositionV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Published,
        exact_active_attempt_v1(state, receipt),
    ensures valid_state_v1(observe_completion_v1(state, receipt, currentness, disposition)), {}

pub proof fn recycle_preserves_valid_state_v1(state: ScopeStateV1,
    receipt: AttemptBindingV1, currentness: RecycleCurrentnessV1,
    disposition: RecycleDispositionV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Completed,
        exact_active_attempt_v1(state, receipt),
    ensures valid_state_v1(recycle_v1(state, receipt, currentness, disposition)), {}

pub proof fn detach_preserves_valid_state_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    disposition: DetachDispositionV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Recycled,
        exact_active_attempt_v1(state, receipt),
    ensures valid_state_v1(detach_recycled_v1(state, receipt, disposition)), {}

pub proof fn cancel_preserves_valid_state_v1(state: ScopeStateV1, receipt: AttemptBindingV1,
    disposition: CancelDispositionV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.attempt_phase == AttemptPhaseV1::Prepared,
        exact_active_attempt_v1(state, receipt),
    ensures valid_state_v1(cancel_prepared_v1(state, receipt, disposition)), {}

pub proof fn close_preserves_valid_state_v1(state: ScopeStateV1, stable: StableBindingV1,
    audit: ContractedAuditOutcomeV1, disposition: CloseDispositionV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available, state.attempt.is_none(),
    ensures valid_state_v1(close_scope_v1(state, stable, audit, disposition)), {}

pub proof fn terminal_absorbs_every_transition_v1(state: ScopeStateV1,
    stable: StableBindingV1, receipt: AttemptBindingV1)
    requires valid_state_v1(state), state.scope == ScopePhaseV1::TerminalAbsorbed,
    ensures open_scope_v1(state, receipt, ContractedAuditOutcomeV1::Current) == state,
        prepare_replay_v1(state, receipt, ContractedAuditOutcomeV1::Current) == state,
        submit_v1(state, receipt, current_submit_v1(), SubmitDispositionV1::Published) == state,
        observe_completion_v1(state, receipt, current_completion_v1(),
            CompletionDispositionV1::Completed) == state,
        recycle_v1(state, receipt, current_recycle_v1(), RecycleDispositionV1::Recycled) == state,
        detach_recycled_v1(state, receipt, DetachDispositionV1::Detached) == state,
        cancel_prepared_v1(state, receipt, CancelDispositionV1::Cancelled) == state,
        close_scope_v1(state, stable, ContractedAuditOutcomeV1::Current,
            CloseDispositionV1::ReleasedAndRetaken) == state, {}

fn main() {}

}
