// Independent finite R46 summary model for an optimized blocking wait over an
// R40 gfx942 striped-SDMA aggregate. Tail-fence completion ordering,
// currentness, time, and observations are contracted mathematical inputs. This
// proves no Rust refinement, allocation/panic behavior, KFD/HSA/HIP semantics,
// native fence behavior, clock truth, hardware property, progress, parity, or
// performance claim.

use vstd::prelude::*;

verus! {

pub open spec fn maximum_requests_v1() -> nat { 882 }
pub open spec fn maximum_active_shards_v1() -> nat { 14 }

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub session: nat,
    pub submission: nat,
    pub request_index: nat,
    pub queue_slot: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct TailContractV1 {
    pub admitted_gfx942_sdma_engine: bool,
    pub system_scope_completion_fence: bool,
    pub signal_read_is_bound_to_named_fence: bool,
    pub completion_implies_preceding_visible: bool,
}

#[derive(PartialEq, Eq)]
pub struct TailV1 {
    pub session: nat,
    pub submission: nat,
    pub engine: nat,
    pub queue_slot: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
    pub last_request_index: nat,
    pub fence_occurrence: nat,
    pub signal_slot: nat,
    pub signal_generation: nat,
    pub contract: TailContractV1,
}

pub struct ActiveShardV1 {
    pub engine: nat,
    pub queue_slot: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
    pub request_indices: Seq<nat>,
    pub tail: TailV1,
}

pub struct OwnerV1 {
    pub owner_token: nat,
    pub session: nat,
    pub submission: nat,
    pub started_ns: nat,
    pub deadline_ns: nat,
    pub striped_count: nat,
    pub striped_queue_ids: Seq<nat>,
    pub queue_generation: nat,
    pub tickets: Seq<TicketV1>,
    pub active_shards: Seq<ActiveShardV1>,
    pub tail_rounds: nat,
    pub tail_observations: nat,
    pub final_audit_observations: nat,
    pub post_bind_allocation_count: nat,
}

pub open spec fn active_shard_count_v1(request_count: nat, striped_count: nat) -> nat {
    if request_count < striped_count { request_count } else { striped_count }
}

pub open spec fn shard_load_v1(request_count: nat, striped_count: nat, slot: nat) -> nat {
    if striped_count == 0 || slot >= striped_count {
        0
    } else {
        request_count / striped_count
            + if slot < request_count % striped_count { 1nat } else { 0nat }
    }
}

pub open spec fn expected_request_indices_v1(
    request_count: nat,
    striped_count: nat,
    slot: nat,
) -> Seq<nat> {
    Seq::new(shard_load_v1(request_count, striped_count, slot), |order: int|
        slot + (order as nat) * striped_count)
}

pub open spec fn tail_contract_is_exact_v1(contract: TailContractV1) -> bool {
    contract.admitted_gfx942_sdma_engine
        && contract.system_scope_completion_fence
        && contract.signal_read_is_bound_to_named_fence
        && contract.completion_implies_preceding_visible
}

pub open spec fn ticket_is_exact_v1(owner: OwnerV1, index: int) -> bool {
    0 <= index < owner.tickets.len()
        && owner.tickets[index].session == owner.session
        && owner.tickets[index].submission == owner.submission
        && owner.tickets[index].request_index == index
        && owner.tickets[index].queue_slot == (index as nat) % owner.striped_count
        && owner.tickets[index].queue_id
            == owner.striped_queue_ids[((index as nat) % owner.striped_count) as int]
        && owner.tickets[index].queue_generation == owner.queue_generation
}

pub open spec fn shard_is_exact_v1(owner: OwnerV1, slot: int) -> bool {
    0 <= slot < owner.active_shards.len()
        && owner.active_shards[slot].engine == (slot as nat) % 2
        && owner.active_shards[slot].queue_slot == slot
        && owner.active_shards[slot].queue_id == owner.striped_queue_ids[slot]
        && owner.active_shards[slot].queue_generation == owner.queue_generation
        && owner.active_shards[slot].request_indices
            == expected_request_indices_v1(owner.tickets.len(), owner.striped_count, slot as nat)
        && owner.active_shards[slot].tail.session == owner.session
        && owner.active_shards[slot].tail.submission == owner.submission
        && owner.active_shards[slot].tail.engine == owner.active_shards[slot].engine
        && owner.active_shards[slot].tail.queue_slot == slot
        && owner.active_shards[slot].tail.queue_id == owner.active_shards[slot].queue_id
        && owner.active_shards[slot].tail.queue_generation == owner.queue_generation
        && owner.active_shards[slot].tail.last_request_index
            == slot as nat
                + (shard_load_v1(owner.tickets.len(), owner.striped_count, slot as nat) - 1)
                    * owner.striped_count
        && owner.active_shards[slot].tail.fence_occurrence > 0
        && owner.active_shards[slot].tail.signal_generation > 0
        && tail_contract_is_exact_v1(owner.active_shards[slot].tail.contract)
}

pub open spec fn tails_are_distinct_v1(shards: Seq<ActiveShardV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < shards.len() && 0 <= right < shards.len() && left != right
            ==> shards[left].tail.fence_occurrence != shards[right].tail.fence_occurrence
                && (shards[left].tail.signal_slot != shards[right].tail.signal_slot
                    || shards[left].tail.signal_generation != shards[right].tail.signal_generation)
}

pub open spec fn owner_identity_is_exact_v1(owner: OwnerV1) -> bool {
    owner.owner_token > 0
        && owner.session > 0
        && owner.submission > 0
        && owner.started_ns <= owner.deadline_ns
        && 2 <= owner.striped_count <= maximum_active_shards_v1()
        && owner.striped_count % 2 == 0
        && owner.striped_queue_ids.len() == owner.striped_count
        && owner.queue_generation > 0
        && 0 < owner.tickets.len() <= maximum_requests_v1()
        && owner.active_shards.len()
            == active_shard_count_v1(owner.tickets.len(), owner.striped_count)
        && (forall|index: int| 0 <= index < owner.tickets.len() ==>
            ticket_is_exact_v1(owner, index))
        && (forall|slot: int| 0 <= slot < owner.active_shards.len() ==>
            shard_is_exact_v1(owner, slot))
        && tails_are_distinct_v1(owner.active_shards)
}

pub open spec fn owner_binding_is_exact_v1(owner: OwnerV1) -> bool {
    owner_identity_is_exact_v1(owner)
        && owner.tail_observations == owner.tail_rounds * owner.active_shards.len()
        && owner.post_bind_allocation_count == 0
}

pub open spec fn owner_is_exact_v1(owner: OwnerV1) -> bool {
    owner_binding_is_exact_v1(owner) && owner.final_audit_observations == 0
}

pub open spec fn terminal_owner_is_exact_v1(owner: OwnerV1) -> bool {
    owner_binding_is_exact_v1(owner)
        && owner.final_audit_observations == owner.tickets.len()
}

pub open spec fn optimized_observation_work_v1(
    rounds: nat,
    active_shards: nat,
    request_count: nat,
) -> nat {
    rounds * active_shards + request_count
}

pub open spec fn general_polling_observation_work_v1(rounds: nat, request_count: nat) -> nat {
    rounds * request_count
}

#[derive(PartialEq, Eq)]
pub struct CurrentnessV1 {
    pub session: nat,
    pub submission: nat,
    pub queue_generation: nat,
    pub scope_closed: bool,
}

pub open spec fn currentness_is_exact_v1(owner: OwnerV1, current: CurrentnessV1) -> bool {
    current.session == owner.session
        && current.submission == owner.submission
        && current.queue_generation == owner.queue_generation
        && current.scope_closed
}

#[derive(PartialEq, Eq)]
pub enum TailStateV1 {
    Ready,
    Pending,
    Error,
}

#[derive(PartialEq, Eq)]
pub struct TailObservationV1 {
    pub tail: TailV1,
    pub state: TailStateV1,
}

pub open spec fn exact_tail_observation_roster_v1(
    owner: OwnerV1,
    observations: Seq<TailObservationV1>,
) -> bool {
    observations.len() == owner.active_shards.len()
        && forall|index: int| 0 <= index < observations.len() ==>
            observations[index].tail == owner.active_shards[index].tail
}

pub open spec fn tail_has_error_v1(observations: Seq<TailObservationV1>) -> bool {
    exists|index: int| 0 <= index < observations.len()
        && observations[index].state == TailStateV1::Error
}

pub open spec fn tail_has_pending_v1(observations: Seq<TailObservationV1>) -> bool {
    exists|index: int| 0 <= index < observations.len()
        && observations[index].state == TailStateV1::Pending
}

#[derive(PartialEq, Eq)]
pub enum AuditStateV1 {
    Ready,
    Pending,
    Error,
}

#[derive(PartialEq, Eq)]
pub struct AuditObservationV1 {
    pub ticket: TicketV1,
    pub state: AuditStateV1,
}

pub open spec fn exact_final_audit_roster_v1(
    owner: OwnerV1,
    audit: Seq<AuditObservationV1>,
) -> bool {
    audit.len() == owner.tickets.len()
        && forall|index: int| 0 <= index < audit.len() ==>
            audit[index].ticket == owner.tickets[index]
}

pub open spec fn audit_has_error_v1(audit: Seq<AuditObservationV1>) -> bool {
    exists|index: int| 0 <= index < audit.len()
        && audit[index].state == AuditStateV1::Error
}

pub open spec fn audit_has_pending_v1(audit: Seq<AuditObservationV1>) -> bool {
    exists|index: int| 0 <= index < audit.len()
        && audit[index].state == AuditStateV1::Pending
}

pub struct AllReadyAuditWitnessV1 {
    pub owner_token: nat,
    pub session: nat,
    pub submission: nat,
    pub queue_generation: nat,
    pub observation_count: nat,
}

pub open spec fn mint_all_ready_audit_witness_v1(
    owner: OwnerV1,
    audit: Seq<AuditObservationV1>,
    full_retirement_preflight: bool,
) -> Option<AllReadyAuditWitnessV1> {
    if exact_final_audit_roster_v1(owner, audit)
        && !audit_has_error_v1(audit)
        && !audit_has_pending_v1(audit)
        && full_retirement_preflight
    {
        Some(AllReadyAuditWitnessV1 {
            owner_token: owner.owner_token,
            session: owner.session,
            submission: owner.submission,
            queue_generation: owner.queue_generation,
            observation_count: audit.len(),
        })
    } else {
        None
    }
}

#[derive(PartialEq, Eq)]
pub enum WaitPhaseV1 {
    ValidationTerminal,
    TailTerminal,
    Pending,
    AuditTerminal,
    TailContractTerminal,
    TimedOut,
    PreflightTerminal,
    Completed,
}

pub struct WaitOutcomeV1 {
    pub phase: WaitPhaseV1,
    pub returned_owner: Option<OwnerV1>,
    pub completed_tickets: Seq<TicketV1>,
    pub tail_rounds: nat,
    pub tail_observations: nat,
    pub final_audit_observations: nat,
    pub retired_count: nat,
    pub post_bind_allocation_count: nat,
}

pub open spec fn outcome_with_owner_v1(
    phase: WaitPhaseV1,
    owner: OwnerV1,
) -> WaitOutcomeV1 {
    WaitOutcomeV1 {
        phase,
        returned_owner: Some(owner),
        completed_tickets: Seq::empty(),
        tail_rounds: owner.tail_rounds,
        tail_observations: owner.tail_observations,
        final_audit_observations: owner.final_audit_observations,
        retired_count: 0,
        post_bind_allocation_count: owner.post_bind_allocation_count,
    }
}

pub open spec fn owner_after_tail_round_v1(owner: OwnerV1) -> OwnerV1 {
    OwnerV1 {
        tail_rounds: owner.tail_rounds + 1,
        tail_observations: owner.tail_observations + owner.active_shards.len(),
        ..owner
    }
}

pub open spec fn owner_after_final_audit_v1(owner: OwnerV1) -> OwnerV1 {
    OwnerV1 { final_audit_observations: owner.tickets.len(), ..owner }
}

pub open spec fn completed_outcome_v1(owner: OwnerV1) -> WaitOutcomeV1 {
    WaitOutcomeV1 {
        phase: WaitPhaseV1::Completed,
        returned_owner: None,
        completed_tickets: owner.tickets,
        tail_rounds: owner.tail_rounds,
        tail_observations: owner.tail_observations,
        final_audit_observations: owner.final_audit_observations,
        retired_count: owner.tickets.len(),
        post_bind_allocation_count: owner.post_bind_allocation_count,
    }
}

pub open spec fn wait_round_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
    full_retirement_preflight: bool,
) -> WaitOutcomeV1 {
    if !owner_is_exact_v1(owner)
        || now_ns < owner.started_ns
        || !currentness_is_exact_v1(owner, current)
        || !exact_tail_observation_roster_v1(owner, tails)
    {
        outcome_with_owner_v1(WaitPhaseV1::ValidationTerminal, owner)
    } else {
        let after_tail = owner_after_tail_round_v1(owner);
        if tail_has_error_v1(tails) {
            outcome_with_owner_v1(WaitPhaseV1::TailTerminal, after_tail)
        } else if tail_has_pending_v1(tails) && now_ns < owner.deadline_ns {
            outcome_with_owner_v1(WaitPhaseV1::Pending, after_tail)
        } else if !exact_final_audit_roster_v1(owner, audit) {
            outcome_with_owner_v1(WaitPhaseV1::AuditTerminal, after_tail)
        } else {
            let audited = owner_after_final_audit_v1(after_tail);
            if audit_has_error_v1(audit) {
                outcome_with_owner_v1(WaitPhaseV1::AuditTerminal, audited)
            } else if !tail_has_pending_v1(tails) && audit_has_pending_v1(audit) {
                outcome_with_owner_v1(WaitPhaseV1::TailContractTerminal, audited)
            } else if audit_has_pending_v1(audit) {
                outcome_with_owner_v1(WaitPhaseV1::TimedOut, audited)
            } else if mint_all_ready_audit_witness_v1(
                owner,
                audit,
                full_retirement_preflight,
            ).is_none() {
                outcome_with_owner_v1(WaitPhaseV1::PreflightTerminal, audited)
            } else {
                completed_outcome_v1(audited)
            }
        }
    }
}

// Obligation 1: the finite gfx942 bounds are exact.
pub proof fn constants_are_exact_v1()
    ensures maximum_requests_v1() == 882, maximum_active_shards_v1() == 14,
{}

// Obligation 2: an exact owner binds a nonempty bounded submission and shard roster.
pub proof fn exact_owner_is_bounded_v1(owner: OwnerV1)
    requires owner_is_exact_v1(owner),
    ensures
        0 < owner.tickets.len() <= 882,
        0 < owner.active_shards.len() <= 14,
{}

// Obligation 3: active shards are exactly min(request count, striped count).
pub proof fn active_shard_roster_is_exact_v1(owner: OwnerV1)
    requires owner_is_exact_v1(owner),
    ensures owner.active_shards.len()
        == active_shard_count_v1(owner.tickets.len(), owner.striped_count),
{}

// Obligation 4: every request ticket binds exact submission, queue, slot, and generation.
pub proof fn every_ticket_is_exact_v1(owner: OwnerV1, index: int)
    requires owner_is_exact_v1(owner), 0 <= index < owner.tickets.len(),
    ensures ticket_is_exact_v1(owner, index),
{}

// Obligation 5: every shard retains the exact per-queue ordered partition.
pub proof fn every_shard_partition_is_exact_v1(owner: OwnerV1, slot: int)
    requires owner_is_exact_v1(owner), 0 <= slot < owner.active_shards.len(),
    ensures owner.active_shards[slot].request_indices
        == expected_request_indices_v1(owner.tickets.len(), owner.striped_count, slot as nat),
{
    reveal(owner_is_exact_v1);
    reveal(owner_binding_is_exact_v1);
    assert(owner_binding_is_exact_v1(owner));
    assert(forall|candidate: int| 0 <= candidate < owner.active_shards.len() ==>
        shard_is_exact_v1(owner, candidate));
    assert(shard_is_exact_v1(owner, slot));
}

// Obligation 6: one exact final tail is bound to every active shard.
pub proof fn every_shard_tail_is_exact_v1(owner: OwnerV1, slot: int)
    requires owner_is_exact_v1(owner), 0 <= slot < owner.active_shards.len(),
    ensures
        owner.active_shards[slot].tail.queue_slot == slot,
        owner.active_shards[slot].tail.queue_id == owner.active_shards[slot].queue_id,
        owner.active_shards[slot].tail.queue_generation == owner.queue_generation,
        owner.active_shards[slot].tail.last_request_index
            == slot as nat
                + (shard_load_v1(owner.tickets.len(), owner.striped_count, slot as nat) - 1)
                    * owner.striped_count,
{
    reveal(owner_is_exact_v1);
    reveal(owner_binding_is_exact_v1);
    assert(owner_binding_is_exact_v1(owner));
    assert(forall|candidate: int| 0 <= candidate < owner.active_shards.len() ==>
        shard_is_exact_v1(owner, candidate));
    assert(shard_is_exact_v1(owner, slot));
}

// Obligation 7: tail fence occurrences and native signal uses are pairwise distinct.
pub proof fn authenticated_tails_are_distinct_v1(owner: OwnerV1)
    requires owner_is_exact_v1(owner),
    ensures tails_are_distinct_v1(owner.active_shards),
{
    reveal(owner_is_exact_v1);
    reveal(owner_binding_is_exact_v1);
    assert(owner_binding_is_exact_v1(owner));
    assert(tails_are_distinct_v1(owner.active_shards));
}

// Obligation 8: the only ordering premise is explicit on each exact tail.
pub proof fn tail_ordering_contract_is_explicit_v1(owner: OwnerV1, slot: int)
    requires owner_is_exact_v1(owner), 0 <= slot < owner.active_shards.len(),
    ensures tail_contract_is_exact_v1(owner.active_shards[slot].tail.contract),
{
    reveal(owner_is_exact_v1);
    reveal(owner_binding_is_exact_v1);
    assert(owner_binding_is_exact_v1(owner));
    assert(forall|candidate: int| 0 <= candidate < owner.active_shards.len() ==>
        shard_is_exact_v1(owner, candidate));
    assert(shard_is_exact_v1(owner, slot));
}

// Obligation 9: currentness omission is terminal before any new observation.
pub proof fn currentness_failure_is_terminal_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires owner_is_exact_v1(owner), !currentness_is_exact_v1(owner, current),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::ValidationTerminal
        &&& outcome.returned_owner == Some(owner)
        &&& outcome.tail_observations == owner.tail_observations
    },
{}

// Obligation 10: tail-roster substitution is terminal before status observation.
pub proof fn tail_roster_substitution_is_terminal_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        now_ns >= owner.started_ns,
        !exact_tail_observation_roster_v1(owner, tails),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::ValidationTerminal
        &&& outcome.returned_owner == Some(owner)
        &&& outcome.tail_observations == owner.tail_observations
    },
{}

// Obligation 11: a tail error is terminal after exactly one full tail round.
pub proof fn tail_error_is_terminal_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        now_ns >= owner.started_ns,
        tail_has_error_v1(tails),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::TailTerminal
        &&& outcome.returned_owner == Some(owner_after_tail_round_v1(owner))
        &&& outcome.tail_observations
            == owner.tail_observations + owner.active_shards.len()
        &&& outcome.final_audit_observations == 0
    },
{
    assert(wait_round_v1(owner, current, tails, audit, now_ns, false)
        == outcome_with_owner_v1(
            WaitPhaseV1::TailTerminal,
            owner_after_tail_round_v1(owner),
        ));
}

// Obligation 12: Pending before the deadline observes no full-roster entry.
pub proof fn pending_before_deadline_uses_only_tails_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        owner.started_ns <= now_ns < owner.deadline_ns,
        !tail_has_error_v1(tails),
        tail_has_pending_v1(tails),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::Pending
        &&& outcome.returned_owner == Some(owner_after_tail_round_v1(owner))
        &&& outcome.final_audit_observations == 0
        &&& outcome.retired_count == 0
    },
{
    assert(wait_round_v1(owner, current, tails, audit, now_ns, false)
        == outcome_with_owner_v1(
            WaitPhaseV1::Pending,
            owner_after_tail_round_v1(owner),
        ));
}

// Obligation 13: all-ready tails trigger the final audit path.
pub proof fn all_ready_tails_trigger_final_audit_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.started_ns,
        !tail_has_error_v1(tails),
        !tail_has_pending_v1(tails),
    ensures wait_round_v1(owner, current, tails, audit, now_ns, false)
        .final_audit_observations == owner.tickets.len(),
{}

// Obligation 14: reaching the shared deadline triggers the final audit path.
pub proof fn deadline_triggers_final_audit_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
    ensures wait_round_v1(owner, current, tails, audit, now_ns, false)
        .final_audit_observations == owner.tickets.len(),
{}

// Obligation 15: a malformed final audit fails closed before auditing statuses.
pub proof fn final_audit_roster_substitution_is_terminal_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        !exact_final_audit_roster_v1(owner, audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::AuditTerminal
        &&& outcome.returned_owner == Some(owner_after_tail_round_v1(owner))
        &&& outcome.final_audit_observations == 0
    },
{
    assert(wait_round_v1(owner, current, tails, audit, now_ns, false)
        == outcome_with_owner_v1(
            WaitPhaseV1::AuditTerminal,
            owner_after_tail_round_v1(owner),
        ));
}

// Obligation 16: a final-audit error is terminal only after the exact N audit.
pub proof fn final_audit_error_is_terminal_after_full_scan_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        audit_has_error_v1(audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::AuditTerminal
        &&& outcome.final_audit_observations == owner.tickets.len()
        &&& outcome.returned_owner.is_some()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 17: ready tails with a Pending prefix violate the tail contract.
pub proof fn ready_tail_with_pending_prefix_is_fail_closed_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.started_ns,
        !tail_has_error_v1(tails),
        !tail_has_pending_v1(tails),
        !audit_has_error_v1(audit),
        audit_has_pending_v1(audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::TailContractTerminal
        &&& outcome.returned_owner.is_some()
        &&& outcome.final_audit_observations == owner.tickets.len()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 18: timeout requires an expired deadline and Pending final audit.
pub proof fn timeout_requires_pending_final_audit_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        tail_has_pending_v1(tails),
        !audit_has_error_v1(audit),
        audit_has_pending_v1(audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::TimedOut
        &&& outcome.returned_owner.is_some()
        &&& outcome.final_audit_observations == owner.tickets.len()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 19: an all-ready final audit succeeds even at the deadline.
pub proof fn all_ready_final_audit_wins_at_deadline_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        !audit_has_error_v1(audit),
        !audit_has_pending_v1(audit),
    ensures wait_round_v1(owner, current, tails, audit, now_ns, true).phase
        == WaitPhaseV1::Completed,
{}

// Obligation 20: failed retirement preflight moves nothing and retains custody.
pub proof fn failed_preflight_retires_nothing_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        !audit_has_error_v1(audit),
        !audit_has_pending_v1(audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::PreflightTerminal
        &&& outcome.returned_owner.is_some()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 21: success retires the complete exact roster in original order.
pub proof fn success_is_all_or_none_and_ordered_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        !audit_has_error_v1(audit),
        !audit_has_pending_v1(audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, true);
        &&& outcome.phase == WaitPhaseV1::Completed
        &&& outcome.returned_owner.is_none()
        &&& outcome.completed_tickets == owner.tickets
        &&& outcome.retired_count == owner.tickets.len()
    },
{}

// Obligation 22: no wait outcome adds a post-bind allocation event.
pub proof fn wait_adds_no_late_allocation_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
    preflight: bool,
)
    requires owner.post_bind_allocation_count == 0,
    ensures wait_round_v1(owner, current, tails, audit, now_ns, preflight)
        .post_bind_allocation_count == 0,
{}

// Obligation 23: one admitted tail round preserves rounds times active shards.
pub proof fn one_round_has_exact_tail_work_v1(owner: OwnerV1)
    requires owner_is_exact_v1(owner),
    ensures owner_after_tail_round_v1(owner).tail_observations
        == owner_after_tail_round_v1(owner).tail_rounds * owner.active_shards.len(),
{
    reveal(owner_is_exact_v1);
    reveal(owner_binding_is_exact_v1);
    assert(owner_binding_is_exact_v1(owner));
    assert(owner.tail_observations
        == owner.tail_rounds * owner.active_shards.len());
    assert((owner.tail_rounds + 1) * owner.active_shards.len()
        == owner.tail_rounds * owner.active_shards.len() + owner.active_shards.len())
        by (nonlinear_arith);
}

// Obligation 24: a final path has rounds*S tail work plus one exact N audit.
pub proof fn final_work_count_is_exact_v1(owner: OwnerV1)
    requires owner_is_exact_v1(owner),
    ensures {
        let final_owner = owner_after_final_audit_v1(owner_after_tail_round_v1(owner));
        &&& final_owner.tail_observations
            == final_owner.tail_rounds * owner.active_shards.len()
        &&& final_owner.final_audit_observations == owner.tickets.len()
        &&& final_owner.tail_observations + final_owner.final_audit_observations
            == final_owner.tail_rounds * owner.active_shards.len() + owner.tickets.len()
    },
{
    one_round_has_exact_tail_work_v1(owner);
}

// Obligation 25: Pending preserves the exact submission and whole custody.
pub proof fn pending_preserves_exact_submission_custody_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        owner.started_ns <= now_ns < owner.deadline_ns,
        !tail_has_error_v1(tails),
        tail_has_pending_v1(tails),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.returned_owner.unwrap().owner_token == owner.owner_token
        &&& outcome.returned_owner.unwrap().tickets == owner.tickets
        &&& outcome.returned_owner.unwrap().active_shards == owner.active_shards
        &&& outcome.retired_count == 0
    },
{}

// Obligation 26: every noncompleted exact final outcome retains whole custody.
pub proof fn noncompleted_final_outcome_retains_custody_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
    preflight: bool,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, preflight);
        &&& outcome.phase != WaitPhaseV1::Completed ==> outcome.returned_owner.is_some()
        &&& outcome.phase != WaitPhaseV1::Completed ==> outcome.retired_count == 0
        &&& outcome.phase != WaitPhaseV1::Completed ==>
            outcome.returned_owner.unwrap().tickets == owner.tickets
        &&& outcome.phase != WaitPhaseV1::Completed ==>
            terminal_owner_is_exact_v1(outcome.returned_owner.unwrap())
    },
{
    audited_terminal_owner_preserves_exact_binding_v1(owner);
}

// Obligation 27: the unchanged general full observer remains a distinct N-work model.
pub proof fn optimized_work_formula_is_not_full_scan_per_round_v1(
)
    ensures
        optimized_observation_work_v1(3, 2, 8) == 14,
        general_polling_observation_work_v1(3, 8) == 24,
        optimized_observation_work_v1(3, 2, 8)
            < general_polling_observation_work_v1(3, 8),
{
}

// Obligation 28: timeout custody is a structurally exact terminal owner.
pub proof fn timeout_custody_remains_structurally_exact_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    tails: Seq<TailObservationV1>,
    audit: Seq<AuditObservationV1>,
    now_ns: nat,
)
    requires
        owner_is_exact_v1(owner),
        currentness_is_exact_v1(owner, current),
        exact_tail_observation_roster_v1(owner, tails),
        exact_final_audit_roster_v1(owner, audit),
        now_ns >= owner.deadline_ns,
        !tail_has_error_v1(tails),
        tail_has_pending_v1(tails),
        !audit_has_error_v1(audit),
        audit_has_pending_v1(audit),
    ensures {
        let outcome = wait_round_v1(owner, current, tails, audit, now_ns, false);
        &&& outcome.phase == WaitPhaseV1::TimedOut
        &&& terminal_owner_is_exact_v1(outcome.returned_owner.unwrap())
    },
{
    audited_terminal_owner_preserves_exact_binding_v1(owner);
}

// Obligation 29: the retirement witness cannot be minted without one exact,
// all-ready full audit and complete retirement preflight.
pub proof fn all_ready_audit_witness_is_unforgeable_v1(
    owner: OwnerV1,
    audit: Seq<AuditObservationV1>,
    preflight: bool,
)
    requires mint_all_ready_audit_witness_v1(owner, audit, preflight).is_some(),
    ensures
        exact_final_audit_roster_v1(owner, audit),
        !audit_has_error_v1(audit),
        !audit_has_pending_v1(audit),
        preflight,
        mint_all_ready_audit_witness_v1(owner, audit, preflight).unwrap().owner_token
            == owner.owner_token,
        mint_all_ready_audit_witness_v1(owner, audit, preflight).unwrap().session
            == owner.session,
        mint_all_ready_audit_witness_v1(owner, audit, preflight).unwrap().submission
            == owner.submission,
        mint_all_ready_audit_witness_v1(owner, audit, preflight).unwrap().queue_generation
            == owner.queue_generation,
        mint_all_ready_audit_witness_v1(owner, audit, preflight)
            .unwrap().observation_count == owner.tickets.len(),
{}

// Obligation 30: completing one admitted tail round and one exact final audit
// preserves every binding while changing only the terminal work counters.
pub proof fn audited_terminal_owner_preserves_exact_binding_v1(owner: OwnerV1)
    requires owner_is_exact_v1(owner),
    ensures terminal_owner_is_exact_v1(
        owner_after_final_audit_v1(owner_after_tail_round_v1(owner)),
    ),
{
    reveal(owner_is_exact_v1);
    reveal(owner_binding_is_exact_v1);
    reveal(owner_identity_is_exact_v1);
    reveal(terminal_owner_is_exact_v1);
    reveal(owner_after_tail_round_v1);
    reveal(owner_after_final_audit_v1);
    reveal(ticket_is_exact_v1);
    reveal(shard_is_exact_v1);
    assert(owner_binding_is_exact_v1(owner));
    assert(owner_identity_is_exact_v1(owner));
    let terminal = owner_after_final_audit_v1(owner_after_tail_round_v1(owner));
    assert(terminal.owner_token == owner.owner_token);
    assert(terminal.session == owner.session);
    assert(terminal.submission == owner.submission);
    assert(terminal.started_ns == owner.started_ns);
    assert(terminal.deadline_ns == owner.deadline_ns);
    assert(terminal.striped_count == owner.striped_count);
    assert(terminal.striped_queue_ids == owner.striped_queue_ids);
    assert(terminal.queue_generation == owner.queue_generation);
    assert(terminal.tickets == owner.tickets);
    assert(terminal.active_shards == owner.active_shards);
    assert forall|index: int| 0 <= index < terminal.tickets.len() implies
        ticket_is_exact_v1(terminal, index) by {
        assert(ticket_is_exact_v1(owner, index));
    }
    assert forall|slot: int| 0 <= slot < terminal.active_shards.len() implies
        shard_is_exact_v1(terminal, slot) by {
        assert(shard_is_exact_v1(owner, slot));
    }
    assert(tails_are_distinct_v1(terminal.active_shards));
    assert(owner_identity_is_exact_v1(terminal));
    assert(owner.tail_observations
        == owner.tail_rounds * owner.active_shards.len());
    assert((owner.tail_rounds + 1) * owner.active_shards.len()
        == owner.tail_rounds * owner.active_shards.len() + owner.active_shards.len())
        by (nonlinear_arith);
}

}
