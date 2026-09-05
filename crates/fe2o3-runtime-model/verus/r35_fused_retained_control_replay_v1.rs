// Independent finite R35 proof model for fused retained-control replay.
// All observations are contracted mathematical inputs. This file proves no
// Rust/native/hardware refinement, ownership implementation, or performance.
// Its custody-and-commit projection excludes production/public error identity,
// model terminal failure stage, internal authority label, event indices,
// foundation-loan counts, and currentness counts. The last two have separate
// successful-path theorems but are not fields of the projected equivalence.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum EffectV1 { Read, Write, ReadWrite }

#[derive(PartialEq, Eq)]
pub enum AdmissionV1 { RetryableFailure, TerminalFailure, Admitted }

#[derive(PartialEq, Eq)]
pub enum PreparationV1 { UseRejected, ReserveRejected, PrepareRejected, Prepared }

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { Retryable, Prepared, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 {
    RetryableInput,
    PreparedAttachment,
    TerminalInput,
    TerminalStorage,
    TerminalData,
    TerminalAttached,
}

#[derive(PartialEq, Eq)]
pub enum AuthorityStateV1 { Prepared, Quarantined }

#[derive(PartialEq, Eq)]
pub enum TerminalStageV1 {
    Admission,
    Preparation,
    FormerMappedOpen,
    FormerMappedRetake,
    FormerRetainOpen,
    FormerRetainRetake,
    FusedOpen,
    FusedRetake,
    MappedFacts,
    Detach,
    Construction,
    Retain,
    FinalAudit,
    Cancellation,
}

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub attachment_generation: nat,
    pub next_attachment_generation: nat,
    pub storage_identity: nat,
    pub predecessor_generation: nat,
    pub effect: EffectV1,
}

#[derive(PartialEq, Eq)]
pub struct LoanV1 { pub open_succeeded: bool, pub retake_succeeded: bool }

#[derive(PartialEq, Eq)]
pub struct ObservationsV1 {
    pub admission: AdmissionV1,
    pub preparation: PreparationV1,
    pub former_mapped_loan: LoanV1,
    pub former_retain_loan: LoanV1,
    pub fused_loan: LoanV1,
    pub mapped_facts_succeeded: bool,
    pub detach_succeeded: bool,
    pub construction_succeeded: bool,
    pub retain_succeeded: bool,
    pub final_audit_succeeded: bool,
    pub cancellation_succeeded: bool,
    pub session_healthy: bool,
    pub quarantine_succeeded: bool,
}

#[derive(PartialEq, Eq)]
pub struct AttachmentV1 {
    pub queue_id: nat,
    pub queue_generation: nat,
    pub attachment_generation: nat,
    pub storage_identity: nat,
    pub predecessor_generation: nat,
    pub effect: EffectV1,
    pub authority_state: AuthorityStateV1,
    pub terminal_custody: Option<CustodyV1>,
}

#[derive(PartialEq, Eq)]
pub struct StateV1 {
    pub binding: BindingV1,
    pub outcome: OutcomeV1,
    pub custody: CustodyV1,
    pub terminal_stage: Option<TerminalStageV1>,
    pub terminal_poisoned: bool,
    pub dispatch_retained: bool,
    pub attachment: Option<AttachmentV1>,
    pub next_attachment_generation: nat,
    pub detached_data_count: nat,
    pub detached_generation: Option<nat>,
    pub detached_identity_count: nat,
    pub detached_next_insertion_index: Option<nat>,
    pub admission_event: nat,
    pub preparation_event: Option<nat>,
    pub detach_event: Option<nat>,
    pub commit_event: Option<nat>,
    pub foundation_loan_attempts: nat,
    pub currentness_observations: nat,
}

// Mathematical values are copyable. This proves coordinate relations only,
// not Rust move-only ownership or borrow exclusivity.
pub struct DetachedAuthorityV1 { pub binding: BindingV1 }
pub struct DataAuthorityV1 { pub detached: DetachedAuthorityV1 }
pub struct AttachedAuthorityV1 { pub detached: DetachedAuthorityV1 }

pub open spec fn valid_binding_v1(binding: BindingV1) -> bool {
    &&& binding.queue_id > 0
    &&& binding.queue_generation > 0
    &&& binding.attachment_generation > 0
    &&& binding.next_attachment_generation == binding.attachment_generation + 1
    &&& binding.storage_identity > 0
    &&& binding.predecessor_generation > 0
}

pub open spec fn loan_succeeded_v1(loan: LoanV1) -> bool {
    loan.open_succeeded && loan.retake_succeeded
}

pub open spec fn prepared_v1(preparation: PreparationV1) -> bool {
    preparation == PreparationV1::Prepared
}

/// Input-only, path-sensitive premise. It invokes neither runner and compares
/// no output state.
pub open spec fn loan_equivalence_premise_v1(observations: ObservationsV1) -> bool {
    if observations.admission != AdmissionV1::Admitted
        || !prepared_v1(observations.preparation) {
        true
    } else if observations.former_mapped_loan.open_succeeded
        != observations.fused_loan.open_succeeded {
        false
    } else if !observations.former_mapped_loan.open_succeeded {
        true
    } else if !observations.mapped_facts_succeeded {
        loan_succeeded_v1(observations.former_mapped_loan)
            == loan_succeeded_v1(observations.fused_loan)
    } else if !observations.former_mapped_loan.retake_succeeded {
        !observations.detach_succeeded && !observations.fused_loan.retake_succeeded
    } else if !observations.detach_succeeded {
        observations.fused_loan.retake_succeeded
    } else if !observations.construction_succeeded {
        true
    } else if !observations.former_retain_loan.open_succeeded {
        !observations.retain_succeeded
    } else if !observations.retain_succeeded || !observations.final_audit_succeeded {
        true
    } else if !observations.former_retain_loan.retake_succeeded {
        !observations.fused_loan.retake_succeeded
    } else {
        observations.fused_loan.retake_succeeded
    }
}

pub open spec fn initial_state_v1(binding: BindingV1) -> StateV1 {
    StateV1 {
        binding,
        outcome: OutcomeV1::Terminal,
        custody: CustodyV1::TerminalInput,
        terminal_stage: None,
        terminal_poisoned: false,
        dispatch_retained: true,
        attachment: None,
        next_attachment_generation: binding.attachment_generation,
        detached_data_count: 0,
        detached_generation: Some(binding.predecessor_generation),
        detached_identity_count: 0,
        detached_next_insertion_index: Some(0),
        admission_event: 1,
        preparation_event: None,
        detach_event: None,
        commit_event: None,
        foundation_loan_attempts: 0,
        currentness_observations: 0,
    }
}

pub open spec fn terminal_attachment_v1(
    binding: BindingV1,
    custody: CustodyV1,
    quarantine_succeeded: bool,
    fused: bool,
) -> AttachmentV1 {
    AttachmentV1 {
        queue_id: binding.queue_id,
        queue_generation: binding.queue_generation,
        attachment_generation: binding.attachment_generation,
        storage_identity: binding.storage_identity,
        predecessor_generation: binding.predecessor_generation,
        effect: binding.effect,
        authority_state: if fused && !quarantine_succeeded {
            AuthorityStateV1::Prepared
        } else {
            AuthorityStateV1::Quarantined
        },
        terminal_custody: Some(custody),
    }
}

pub open spec fn attach_terminal_v1(
    state: StateV1,
    custody: CustodyV1,
    stage: TerminalStageV1,
    observations: ObservationsV1,
    fused: bool,
) -> StateV1 {
    StateV1 {
        custody,
        terminal_stage: Some(stage),
        terminal_poisoned: true,
        attachment: Some(terminal_attachment_v1(
            state.binding, custody, observations.quarantine_succeeded, fused)),
        next_attachment_generation: state.binding.next_attachment_generation,
        ..state
    }
}

pub open spec fn finish_before_detach_v1(
    state: StateV1,
    observations: ObservationsV1,
    loan_succeeded: bool,
    stage: TerminalStageV1,
    fused: bool,
) -> StateV1 {
    if loan_succeeded && observations.cancellation_succeeded && observations.session_healthy {
        StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableInput,
            terminal_stage: Some(stage),
            ..state
        }
    } else if observations.cancellation_succeeded {
        StateV1 {
            custody: CustodyV1::TerminalInput,
            terminal_stage: Some(stage),
            terminal_poisoned: true,
            ..state
        }
    } else {
        attach_terminal_v1(
            state, CustodyV1::TerminalAttached, TerminalStageV1::Cancellation,
            observations, fused)
    }
}

pub open spec fn commit_success_v1(state: StateV1) -> StateV1 {
    StateV1 {
        outcome: OutcomeV1::Prepared,
        custody: CustodyV1::PreparedAttachment,
        attachment: Some(AttachmentV1 {
            queue_id: state.binding.queue_id,
            queue_generation: state.binding.queue_generation,
            attachment_generation: state.binding.attachment_generation,
            storage_identity: state.binding.storage_identity,
            predecessor_generation: state.binding.predecessor_generation,
            effect: state.binding.effect,
            authority_state: AuthorityStateV1::Prepared,
            terminal_custody: None,
        }),
        next_attachment_generation: state.binding.next_attachment_generation,
        detached_generation: None,
        detached_next_insertion_index: None,
        commit_event: Some(9),
        ..state
    }
}

pub open spec fn enter_replay_v1(state: StateV1, observations: ObservationsV1) -> StateV1 {
    match observations.admission {
        AdmissionV1::RetryableFailure => StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableInput,
            ..state
        },
        AdmissionV1::TerminalFailure => StateV1 {
            terminal_stage: Some(TerminalStageV1::Admission),
            terminal_poisoned: true,
            ..state
        },
        AdmissionV1::Admitted if !prepared_v1(observations.preparation) => StateV1 {
            outcome: OutcomeV1::Retryable,
            custody: CustodyV1::RetryableInput,
            terminal_stage: Some(TerminalStageV1::Preparation),
            preparation_event: Some(2),
            currentness_observations: 1,
            ..state
        },
        AdmissionV1::Admitted => StateV1 {
            preparation_event: Some(2),
            currentness_observations: 1,
            ..state
        },
    }
}

pub open spec fn former_execute_v1(binding: BindingV1, observations: ObservationsV1) -> StateV1 {
    let initial = initial_state_v1(binding);
    let entered = enter_replay_v1(initial, observations);
    if observations.admission != AdmissionV1::Admitted
        || !prepared_v1(observations.preparation) {
        entered
    } else {
        let mapped_attempt = StateV1 { foundation_loan_attempts: 1, ..entered };
        if !observations.former_mapped_loan.open_succeeded {
            finish_before_detach_v1(mapped_attempt, observations, false,
                TerminalStageV1::FormerMappedOpen, false)
        } else if !observations.mapped_facts_succeeded {
            finish_before_detach_v1(mapped_attempt, observations,
                loan_succeeded_v1(observations.former_mapped_loan),
                TerminalStageV1::MappedFacts, false)
        } else if !observations.former_mapped_loan.retake_succeeded {
            finish_before_detach_v1(mapped_attempt, observations, false,
                TerminalStageV1::FormerMappedRetake, false)
        } else if !observations.detach_succeeded {
            finish_before_detach_v1(mapped_attempt, observations, true,
                TerminalStageV1::Detach, false)
        } else {
            let detached = StateV1 { detach_event: Some(4), ..mapped_attempt };
            if !observations.construction_succeeded {
                attach_terminal_v1(detached, CustodyV1::TerminalStorage,
                    TerminalStageV1::Construction, observations, false)
            } else {
                let retain_attempt = StateV1 { foundation_loan_attempts: 2, ..detached };
                if !observations.former_retain_loan.open_succeeded {
                    attach_terminal_v1(retain_attempt, CustodyV1::TerminalData,
                        TerminalStageV1::FormerRetainOpen, observations, false)
                } else if !observations.retain_succeeded {
                    attach_terminal_v1(retain_attempt, CustodyV1::TerminalData,
                        TerminalStageV1::Retain, observations, false)
                } else if !observations.former_retain_loan.retake_succeeded {
                    attach_terminal_v1(retain_attempt, CustodyV1::TerminalAttached,
                        TerminalStageV1::FormerRetainRetake, observations, false)
                } else {
                    let audited = StateV1 { currentness_observations: 2, ..retain_attempt };
                    if !observations.final_audit_succeeded {
                        attach_terminal_v1(audited, CustodyV1::TerminalAttached,
                            TerminalStageV1::FinalAudit, observations, false)
                    } else {
                        commit_success_v1(audited)
                    }
                }
            }
        }
    }
}

pub open spec fn fused_execute_v1(binding: BindingV1, observations: ObservationsV1) -> StateV1 {
    let initial = initial_state_v1(binding);
    let entered = enter_replay_v1(initial, observations);
    if observations.admission != AdmissionV1::Admitted
        || !prepared_v1(observations.preparation) {
        entered
    } else {
        let attempted = StateV1 { foundation_loan_attempts: 1, ..entered };
        if !observations.fused_loan.open_succeeded {
            finish_before_detach_v1(attempted, observations, false,
                TerminalStageV1::FusedOpen, true)
        } else if !observations.mapped_facts_succeeded {
            finish_before_detach_v1(attempted, observations,
                loan_succeeded_v1(observations.fused_loan),
                TerminalStageV1::MappedFacts, true)
        } else if !observations.detach_succeeded {
            finish_before_detach_v1(attempted, observations,
                loan_succeeded_v1(observations.fused_loan),
                TerminalStageV1::Detach, true)
        } else {
            let detached = StateV1 { detach_event: Some(4), ..attempted };
            if !observations.construction_succeeded {
                attach_terminal_v1(detached, CustodyV1::TerminalStorage,
                    TerminalStageV1::Construction, observations, true)
            } else if !observations.retain_succeeded {
                attach_terminal_v1(detached, CustodyV1::TerminalData,
                    TerminalStageV1::Retain, observations, true)
            } else {
                let audited = StateV1 { currentness_observations: 2, ..detached };
                if !observations.final_audit_succeeded {
                    attach_terminal_v1(audited, CustodyV1::TerminalAttached,
                        TerminalStageV1::FinalAudit, observations, true)
                } else if !observations.fused_loan.retake_succeeded {
                    attach_terminal_v1(audited, CustodyV1::TerminalAttached,
                        TerminalStageV1::FusedRetake, observations, true)
                } else {
                    commit_success_v1(audited)
                }
            }
        }
    }
}

pub open spec fn attachment_projected_commit_equal_v1(
    left: Option<AttachmentV1>, right: Option<AttachmentV1>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(l), Some(r)) => {
            &&& l.queue_id == r.queue_id
            &&& l.queue_generation == r.queue_generation
            &&& l.attachment_generation == r.attachment_generation
            &&& l.storage_identity == r.storage_identity
            &&& l.predecessor_generation == r.predecessor_generation
            &&& l.effect == r.effect
            &&& l.terminal_custody == r.terminal_custody
        },
        _ => false,
    }
}

pub open spec fn projected_custody_and_commit_equal_v1(left: StateV1, right: StateV1) -> bool {
    &&& left.binding == right.binding
    &&& left.outcome == right.outcome
    &&& left.custody == right.custody
    &&& left.terminal_poisoned == right.terminal_poisoned
    &&& left.dispatch_retained == right.dispatch_retained
    &&& attachment_projected_commit_equal_v1(left.attachment, right.attachment)
    &&& left.next_attachment_generation == right.next_attachment_generation
    &&& left.detached_data_count == right.detached_data_count
    &&& left.detached_generation == right.detached_generation
    &&& left.detached_identity_count == right.detached_identity_count
    &&& left.detached_next_insertion_index == right.detached_next_insertion_index
}

pub proof fn premised_projected_custody_and_commit_equivalence_v1(
    binding: BindingV1, observations: ObservationsV1)
    requires valid_binding_v1(binding), loan_equivalence_premise_v1(observations),
    ensures projected_custody_and_commit_equal_v1(former_execute_v1(binding, observations),
        fused_execute_v1(binding, observations)),
{
    reveal(former_execute_v1);
    reveal(fused_execute_v1);
    reveal(loan_equivalence_premise_v1);
    reveal(projected_custody_and_commit_equal_v1);
    reveal(attachment_projected_commit_equal_v1);
    reveal(enter_replay_v1);
    reveal(finish_before_detach_v1);
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
    reveal(commit_success_v1);
    reveal(initial_state_v1);
    reveal(loan_succeeded_v1);
    reveal(prepared_v1);
}

pub proof fn successful_exact_commit_v1(binding: BindingV1, observations: ObservationsV1)
    requires valid_binding_v1(binding),
        observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        observations.fused_loan.open_succeeded,
        observations.fused_loan.retake_succeeded,
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        observations.construction_succeeded,
        observations.retain_succeeded,
        observations.final_audit_succeeded,
    ensures
        fused_execute_v1(binding, observations).outcome == OutcomeV1::Prepared,
        fused_execute_v1(binding, observations).custody == CustodyV1::PreparedAttachment,
        fused_execute_v1(binding, observations).attachment == Some(AttachmentV1 {
            queue_id: binding.queue_id,
            queue_generation: binding.queue_generation,
            attachment_generation: binding.attachment_generation,
            storage_identity: binding.storage_identity,
            predecessor_generation: binding.predecessor_generation,
            effect: binding.effect,
            authority_state: AuthorityStateV1::Prepared,
            terminal_custody: None,
        }),
        fused_execute_v1(binding, observations).next_attachment_generation
            == binding.next_attachment_generation,
        fused_execute_v1(binding, observations).detached_generation == None,
        fused_execute_v1(binding, observations).detached_next_insertion_index == None,
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(commit_success_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

pub proof fn successful_two_to_one_loan_reduction_v1(
    binding: BindingV1, observations: ObservationsV1)
    requires valid_binding_v1(binding),
        observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        loan_succeeded_v1(observations.former_mapped_loan),
        loan_succeeded_v1(observations.former_retain_loan),
        loan_succeeded_v1(observations.fused_loan),
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        observations.construction_succeeded,
        observations.retain_succeeded,
        observations.final_audit_succeeded,
    ensures
        former_execute_v1(binding, observations).foundation_loan_attempts == 2,
        fused_execute_v1(binding, observations).foundation_loan_attempts == 1,
        former_execute_v1(binding, observations).currentness_observations == 2,
        fused_execute_v1(binding, observations).currentness_observations == 2,
{
    reveal(former_execute_v1);
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(commit_success_v1);
    reveal(initial_state_v1);
    reveal(loan_succeeded_v1);
    reveal(prepared_v1);
}

pub proof fn admission_preparation_order_v1(binding: BindingV1, observations: ObservationsV1)
    requires valid_binding_v1(binding), observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
    ensures
        fused_execute_v1(binding, observations).admission_event == 1,
        fused_execute_v1(binding, observations).preparation_event == Some(2),
        fused_execute_v1(binding, observations).foundation_loan_attempts == 1,
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

pub proof fn retry_requires_clean_round_trip_v1(
    state: StateV1, observations: ObservationsV1, loan_succeeded: bool,
    stage: TerminalStageV1)
    requires state.outcome == OutcomeV1::Terminal,
    ensures finish_before_detach_v1(state, observations, loan_succeeded, stage, true).outcome
            == OutcomeV1::Retryable
        <==> loan_succeeded && observations.cancellation_succeeded
            && observations.session_healthy,
{
    reveal(finish_before_detach_v1);
    reveal(attach_terminal_v1);
}

pub proof fn cancellation_failure_is_terminal_attached_v1(
    state: StateV1, observations: ObservationsV1, loan_succeeded: bool,
    stage: TerminalStageV1)
    requires !observations.cancellation_succeeded,
    ensures
        finish_before_detach_v1(state, observations, loan_succeeded, stage, true).custody
            == CustodyV1::TerminalAttached,
        finish_before_detach_v1(state, observations, loan_succeeded, stage, true).terminal_poisoned,
{
    reveal(finish_before_detach_v1);
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
}

pub proof fn storage_failure_is_terminal_v1(binding: BindingV1, observations: ObservationsV1)
    requires observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        observations.fused_loan.open_succeeded,
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        !observations.construction_succeeded,
    ensures fused_execute_v1(binding, observations).custody == CustodyV1::TerminalStorage,
        fused_execute_v1(binding, observations).outcome == OutcomeV1::Terminal,
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

pub proof fn data_failure_is_terminal_v1(binding: BindingV1, observations: ObservationsV1)
    requires observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        observations.fused_loan.open_succeeded,
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        observations.construction_succeeded,
        !observations.retain_succeeded,
    ensures fused_execute_v1(binding, observations).custody == CustodyV1::TerminalData,
        fused_execute_v1(binding, observations).outcome == OutcomeV1::Terminal,
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

pub proof fn final_audit_failure_is_terminal_attached_v1(
    binding: BindingV1, observations: ObservationsV1)
    requires observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        observations.fused_loan.open_succeeded,
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        observations.construction_succeeded,
        observations.retain_succeeded,
        !observations.final_audit_succeeded,
    ensures fused_execute_v1(binding, observations).custody == CustodyV1::TerminalAttached,
        fused_execute_v1(binding, observations).outcome == OutcomeV1::Terminal,
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

pub proof fn ready_retake_failure_is_terminal_attached_v1(
    binding: BindingV1, observations: ObservationsV1)
    requires observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        observations.fused_loan.open_succeeded,
        !observations.fused_loan.retake_succeeded,
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        observations.construction_succeeded,
        observations.retain_succeeded,
        observations.final_audit_succeeded,
    ensures fused_execute_v1(binding, observations).custody == CustodyV1::TerminalAttached,
        fused_execute_v1(binding, observations).terminal_stage == Some(TerminalStageV1::FusedRetake),
        fused_execute_v1(binding, observations).outcome == OutcomeV1::Terminal,
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

pub proof fn failed_quarantine_preserves_prepared_v1(
    state: StateV1, custody: CustodyV1, stage: TerminalStageV1,
    observations: ObservationsV1)
    requires !observations.quarantine_succeeded,
    ensures attach_terminal_v1(state, custody, stage, observations, true).attachment
        == Some(AttachmentV1 {
            queue_id: state.binding.queue_id,
            queue_generation: state.binding.queue_generation,
            attachment_generation: state.binding.attachment_generation,
            storage_identity: state.binding.storage_identity,
            predecessor_generation: state.binding.predecessor_generation,
            effect: state.binding.effect,
            authority_state: AuthorityStateV1::Prepared,
            terminal_custody: Some(custody),
        }),
{
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
}

pub proof fn terminal_attachment_binds_exact_commit_v1(
    state: StateV1, custody: CustodyV1, stage: TerminalStageV1,
    observations: ObservationsV1, fused: bool)
    ensures
        attach_terminal_v1(state, custody, stage, observations, fused).dispatch_retained
            == state.dispatch_retained,
        attach_terminal_v1(state, custody, stage, observations, fused).next_attachment_generation
            == state.binding.next_attachment_generation,
        attach_terminal_v1(state, custody, stage, observations, fused).attachment.unwrap().queue_id
            == state.binding.queue_id,
        attach_terminal_v1(state, custody, stage, observations, fused).attachment.unwrap().queue_generation
            == state.binding.queue_generation,
        attach_terminal_v1(state, custody, stage, observations, fused).attachment.unwrap().attachment_generation
            == state.binding.attachment_generation,
        attach_terminal_v1(state, custody, stage, observations, fused).attachment.unwrap().storage_identity
            == state.binding.storage_identity,
        attach_terminal_v1(state, custody, stage, observations, fused).attachment.unwrap().predecessor_generation
            == state.binding.predecessor_generation,
        attach_terminal_v1(state, custody, stage, observations, fused).attachment.unwrap().effect
            == state.binding.effect,
{
    reveal(attach_terminal_v1);
    reveal(terminal_attachment_v1);
}

pub proof fn successful_roster_commit_is_exact_v1(binding: BindingV1, observations: ObservationsV1)
    requires observations.admission == AdmissionV1::Admitted,
        observations.preparation == PreparationV1::Prepared,
        observations.fused_loan.open_succeeded,
        observations.fused_loan.retake_succeeded,
        observations.mapped_facts_succeeded,
        observations.detach_succeeded,
        observations.construction_succeeded,
        observations.retain_succeeded,
        observations.final_audit_succeeded,
    ensures
        fused_execute_v1(binding, observations).detached_data_count == 0,
        fused_execute_v1(binding, observations).detached_identity_count == 0,
        fused_execute_v1(binding, observations).detached_generation == None,
        fused_execute_v1(binding, observations).detached_next_insertion_index == None,
        fused_execute_v1(binding, observations).commit_event == Some(9),
{
    reveal(fused_execute_v1);
    reveal(enter_replay_v1);
    reveal(commit_success_v1);
    reveal(initial_state_v1);
    reveal(prepared_v1);
}

}
