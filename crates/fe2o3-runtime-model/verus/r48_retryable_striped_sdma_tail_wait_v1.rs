// Independent finite R48 summary model for the production-shaped retryable
// striped-SDMA tail wait. Per-physical-queue tail/audit masks, the 64-slot ring,
// packet/fence/signal publication evidence and order, currentness, observation
// results, time, panic interception, and model-retake results are explicit
// mathematical inputs. This proves no production Rust
// refinement, allocation behavior, native coherence, KFD/HSA/HIP or firmware
// semantics, hardware property, progress, parity, or performance claim.

use vstd::arithmetic::div_mod::{
    lemma_add_mod_noop_right, lemma_mod_bound, lemma_mod_sub_multiples_vanish,
    lemma_small_mod,
};
use vstd::prelude::*;

verus! {

pub open spec fn maximum_queues_v1() -> nat { 16 }
pub open spec fn maximum_combined_queues_v1() -> nat { 14 }
pub open spec fn requests_per_shard_v1() -> nat { 63 }
pub open spec fn ring_slot_count_v1() -> nat { 64 }
pub open spec fn maximum_requests_v1() -> nat { 1008 }
pub open spec fn exact_fence_header_v1() -> nat { 0x0053_0005 }

#[derive(PartialEq, Eq)]
pub enum ProfileV1 {
    Combined,
    Standalone,
}

#[derive(PartialEq, Eq)]
pub enum EngineV1 {
    Engine0,
    Engine1,
}

pub open spec fn profile_admits_v1(profile: ProfileV1, queues: nat) -> bool {
    2 <= queues
        && queues % 2 == 0
        && match profile {
            ProfileV1::Combined => queues <= maximum_combined_queues_v1(),
            ProfileV1::Standalone => queues <= maximum_queues_v1(),
        }
}

pub open spec fn engine_for_queue_v1(queue: nat) -> EngineV1 {
    if queue % 2 == 0 { EngineV1::Engine0 } else { EngineV1::Engine1 }
}

#[derive(PartialEq, Eq)]
pub struct SignalV1 {
    pub mapping: nat,
    pub slot: nat,
    pub generation: nat,
}

pub open spec fn signal_is_exact_v1(signal: SignalV1) -> bool {
    signal.mapping > 0 && signal.generation > 0
}

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub owner_occurrence: nat,
    pub session_occurrence: nat,
    pub submission_epoch: nat,
    pub request_index: nat,
    pub queue_ordinal: nat,
    pub queue_id: nat,
    pub engine: EngineV1,
    pub queue_generation: nat,
    pub ring_slot: nat,
    pub signal: SignalV1,
}

#[derive(PartialEq, Eq)]
pub struct PublicationV1 {
    pub ticket: TicketV1,
    pub packet_occurrence: nat,
    pub fence_header: nat,
    pub packet_signal: SignalV1,
    pub complete_packet_written: bool,
    pub complete_packet_before_write_pointer_release: bool,
    pub record_retained_before_write_pointer_release: bool,
    pub write_pointer_published_release: bool,
    pub doorbell_published_release: bool,
    pub write_pointer_release_before_doorbell_release: bool,
    pub admitted_gfx942_engine: bool,
    pub system_scope: bool,
    pub snoop: bool,
    pub signal_read_names_fence: bool,
    pub completion_implies_preceding_visible: bool,
}

#[derive(PartialEq, Eq)]
pub struct PublishedRequestV1 {
    pub ticket: TicketV1,
    pub publication: PublicationV1,
    pub payload_identity: nat,
}

#[derive(PartialEq, Eq)]
pub struct TailV1 {
    pub normalized_queue_slot: nat,
    pub ticket: TicketV1,
    pub publication: PublicationV1,
}

pub struct OwnerV1 {
    pub exactly_one_epoch_issuer: bool,
    pub owner_occurrence: nat,
    pub session_occurrence: nat,
    pub burned_epoch_high_water: nat,
    pub submission_epoch: nat,
    pub profile: ProfileV1,
    pub queue_generation: nat,
    pub first_queue: nat,
    pub queue_ids: Seq<nat>,
    pub requests: Seq<PublishedRequestV1>,
    pub tails: Seq<TailV1>,
    pub wait_epoch: nat,
    pub completed_tail_rounds: nat,
    pub tail_load_attempts: nat,
    pub audit_load_attempts: nat,
    pub cumulative_tail_load_attempts: nat,
    pub cumulative_audit_load_attempts: nat,
    pub retired_count: nat,
    pub post_bind_allocation_events: nat,
}

pub open spec fn queue_count_v1(owner: OwnerV1) -> nat { owner.queue_ids.len() }
pub open spec fn request_count_v1(owner: OwnerV1) -> nat { owner.requests.len() }

pub open spec fn queue_for_request_v1(first: nat, queues: nat, index: nat) -> nat {
    (first + index) % queues
}

pub open spec fn normalized_slot_v1(first: nat, queues: nat, queue: nat) -> nat {
    if queue >= first {
        (queue - first) as nat
    } else {
        (queue + queues - first) as nat
    }
}

pub open spec fn active_shards_v1(requests: nat, queues: nat) -> nat {
    if requests < queues { requests } else { queues }
}

pub open spec fn last_request_for_normalized_slot_v1(
    requests: nat,
    queues: nat,
    slot: nat,
) -> nat {
    slot + (((requests - 1 - slot) as nat) / queues) * queues
}

pub open spec fn publication_is_exact_v1(publication: PublicationV1, ticket: TicketV1) -> bool {
    publication.ticket == ticket
        && publication.packet_occurrence > 0
        && publication.fence_header == exact_fence_header_v1()
        && publication.packet_signal == ticket.signal
        && signal_is_exact_v1(ticket.signal)
        && publication.complete_packet_written
        && publication.complete_packet_before_write_pointer_release
        && publication.record_retained_before_write_pointer_release
        && publication.write_pointer_published_release
        && publication.doorbell_published_release
        && publication.write_pointer_release_before_doorbell_release
        && publication.admitted_gfx942_engine
        && publication.system_scope
        && publication.snoop
        && publication.signal_read_names_fence
        && publication.completion_implies_preceding_visible
}

pub open spec fn queue_ids_are_distinct_v1(queue_ids: Seq<nat>) -> bool {
    forall|left: int, right: int|
        0 <= left < queue_ids.len() && 0 <= right < queue_ids.len() && left != right
            ==> queue_ids[left] != queue_ids[right]
}

pub open spec fn requests_are_distinct_v1(requests: Seq<PublishedRequestV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < requests.len() && 0 <= right < requests.len() && left != right
            ==> requests[left].ticket.signal != requests[right].ticket.signal
                && (requests[left].ticket.queue_ordinal
                        != requests[right].ticket.queue_ordinal
                    || (requests[left].publication.packet_occurrence
                            != requests[right].publication.packet_occurrence
                        && requests[left].ticket.ring_slot
                            != requests[right].ticket.ring_slot))
}

pub open spec fn request_is_exact_v1(owner: OwnerV1, index: int) -> bool {
    0 <= index < owner.requests.len()
        && owner.requests[index].ticket.owner_occurrence == owner.owner_occurrence
        && owner.requests[index].ticket.session_occurrence == owner.session_occurrence
        && owner.requests[index].ticket.submission_epoch == owner.submission_epoch
        && owner.requests[index].ticket.request_index == index
        && owner.requests[index].ticket.queue_ordinal
            == queue_for_request_v1(owner.first_queue, queue_count_v1(owner), index as nat)
        && owner.requests[index].ticket.queue_id
            == owner.queue_ids[owner.requests[index].ticket.queue_ordinal as int]
        && owner.requests[index].ticket.engine
            == engine_for_queue_v1(owner.requests[index].ticket.queue_ordinal)
        && owner.requests[index].ticket.queue_generation == owner.queue_generation
        && owner.requests[index].ticket.ring_slot < ring_slot_count_v1()
        && owner.requests[index].payload_identity > 0
        && publication_is_exact_v1(
            owner.requests[index].publication,
            owner.requests[index].ticket,
        )
}

pub open spec fn tail_is_exact_v1(owner: OwnerV1, slot: int) -> bool {
    0 <= slot < owner.tails.len()
        && owner.tails[slot].normalized_queue_slot == slot
        && {
            let request = last_request_for_normalized_slot_v1(
                request_count_v1(owner),
                queue_count_v1(owner),
                slot as nat,
            );
            &&& request < request_count_v1(owner)
            &&& owner.tails[slot].ticket == owner.requests[request as int].ticket
            &&& owner.tails[slot].publication == owner.requests[request as int].publication
        }
}

pub open spec fn publication_roster_is_exact_v1(owner: OwnerV1) -> bool {
    &&& owner.exactly_one_epoch_issuer
    &&& owner.owner_occurrence > 0
    &&& owner.session_occurrence > 0
    &&& 0 < owner.submission_epoch <= owner.burned_epoch_high_water
    &&& profile_admits_v1(owner.profile, queue_count_v1(owner))
    &&& owner.queue_generation > 0
    &&& owner.first_queue < queue_count_v1(owner)
    &&& owner.queue_ids.len() == queue_count_v1(owner)
    &&& (forall|queue: int| 0 <= queue < owner.queue_ids.len() ==>
        owner.queue_ids[queue] > 0)
    &&& queue_ids_are_distinct_v1(owner.queue_ids)
    &&& 0 < request_count_v1(owner)
    &&& request_count_v1(owner)
        <= queue_count_v1(owner) * requests_per_shard_v1()
    &&& request_count_v1(owner) <= maximum_requests_v1()
    &&& (forall|index: int| 0 <= index < owner.requests.len() ==>
        request_is_exact_v1(owner, index))
    &&& requests_are_distinct_v1(owner.requests)
    &&& owner.tails.len()
        == active_shards_v1(request_count_v1(owner), queue_count_v1(owner))
    &&& (forall|slot: int| 0 <= slot < owner.tails.len() ==>
        tail_is_exact_v1(owner, slot))
}

pub open spec fn waiting_owner_is_exact_v1(owner: OwnerV1) -> bool {
    publication_roster_is_exact_v1(owner)
        && owner.wait_epoch > 0
        && owner.tail_load_attempts
            == owner.completed_tail_rounds * owner.tails.len()
        && owner.audit_load_attempts == 0
        && owner.cumulative_tail_load_attempts >= owner.tail_load_attempts
        && owner.cumulative_audit_load_attempts >= owner.audit_load_attempts
        && owner.retired_count == 0
        && owner.post_bind_allocation_events == 0
}

#[derive(PartialEq, Eq)]
pub struct CurrentnessV1 {
    pub owner_occurrence: nat,
    pub session_occurrence: nat,
    pub submission_epoch: nat,
    pub queue_generation: nat,
    pub wait_epoch: nat,
    pub opening_current: bool,
    pub closing_current: bool,
}

pub open spec fn currentness_identity_is_exact_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
) -> bool {
    current.owner_occurrence == owner.owner_occurrence
        && current.session_occurrence == owner.session_occurrence
        && current.submission_epoch == owner.submission_epoch
        && current.queue_generation == owner.queue_generation
        && current.wait_epoch == owner.wait_epoch
}

pub struct WaitInputV1 {
    pub exact_tail_roster: bool,
    pub tail_error_prefix: Option<nat>,
    pub initial_ready_tail_queues: Seq<bool>,
    pub deadline_reached: bool,
    pub exact_audit_roster: bool,
    pub audit_error_prefix: Option<nat>,
    pub audit_pending_queues: Seq<bool>,
    pub final_ready_tail_queues: Seq<bool>,
    pub retirement_preflight: bool,
    pub retake_succeeds: bool,
}

pub open spec fn tail_error_prefix_is_valid_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    input.tail_error_prefix.is_none()
        || 1 <= input.tail_error_prefix.unwrap() <= owner.tails.len()
}

pub open spec fn audit_error_prefix_is_valid_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    input.audit_error_prefix.is_none()
        || 1 <= input.audit_error_prefix.unwrap() <= owner.requests.len()
}

pub open spec fn queue_is_active_v1(owner: OwnerV1, queue: int) -> bool {
    0 <= queue < owner.queue_ids.len()
        && exists|slot: int| 0 <= slot < owner.tails.len()
            && owner.tails[slot].ticket.queue_ordinal == queue
}

pub open spec fn tail_queue_mask_is_exact_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    &&& input.initial_ready_tail_queues.len() == owner.queue_ids.len()
    &&& (forall|queue: int| 0 <= queue < owner.queue_ids.len() ==> {
        &&& !input.initial_ready_tail_queues[queue] || queue_is_active_v1(owner, queue)
    })
}

pub open spec fn audit_queue_masks_are_exact_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    &&& input.audit_pending_queues.len() == owner.queue_ids.len()
    &&& input.final_ready_tail_queues.len() == owner.queue_ids.len()
    &&& (forall|queue: int| 0 <= queue < owner.queue_ids.len() ==> {
        &&& !input.audit_pending_queues[queue] || queue_is_active_v1(owner, queue)
        &&& !input.final_ready_tail_queues[queue] || queue_is_active_v1(owner, queue)
        &&& queue_is_active_v1(owner, queue)
            && !input.final_ready_tail_queues[queue] ==>
                input.audit_pending_queues[queue]
    })
}

pub open spec fn all_initial_tails_ready_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    forall|queue: int| 0 <= queue < owner.queue_ids.len()
        && queue_is_active_v1(owner, queue) ==>
            input.initial_ready_tail_queues[queue]
}

pub open spec fn any_audit_pending_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    exists|queue: int| 0 <= queue < owner.queue_ids.len()
        && input.audit_pending_queues[queue]
}

pub open spec fn tail_ordering_violation_v1(owner: OwnerV1, input: WaitInputV1) -> bool {
    exists|queue: int| 0 <= queue < owner.queue_ids.len()
        && input.audit_pending_queues[queue]
        && (input.initial_ready_tail_queues[queue]
            || input.final_ready_tail_queues[queue])
}

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    OpeningCurrentnessTerminal,
    TailRosterTerminal,
    TailErrorTerminal,
    Pending,
    AuditRosterTerminal,
    AuditErrorTerminal,
    ClosingCurrentnessTerminal,
    TailOrderingTerminal,
    TimedOut,
    RetirementPreflightTerminal,
    PanicStageTerminal,
    Completed,
    CompletedOpaque,
    PanicRetained,
}

pub struct WaitOutcomeV1 {
    pub phase: PhaseV1,
    pub returned_owner: Option<OwnerV1>,
    pub completed_requests: Seq<PublishedRequestV1>,
    pub completed_tail_rounds: nat,
    pub tail_load_attempts: nat,
    pub audit_load_attempts: nat,
    pub cumulative_tail_load_attempts: nat,
    pub cumulative_audit_load_attempts: nat,
    pub retired_count: nat,
    pub post_bind_allocation_events: nat,
}

pub open spec fn owner_outcome_v1(phase: PhaseV1, owner: OwnerV1) -> WaitOutcomeV1 {
    WaitOutcomeV1 {
        phase,
        returned_owner: Some(owner),
        completed_requests: Seq::empty(),
        completed_tail_rounds: owner.completed_tail_rounds,
        tail_load_attempts: owner.tail_load_attempts,
        audit_load_attempts: owner.audit_load_attempts,
        cumulative_tail_load_attempts: owner.cumulative_tail_load_attempts,
        cumulative_audit_load_attempts: owner.cumulative_audit_load_attempts,
        retired_count: owner.retired_count,
        post_bind_allocation_events: owner.post_bind_allocation_events,
    }
}

pub open spec fn owner_after_tail_prefix_v1(owner: OwnerV1, prefix: nat) -> OwnerV1 {
    OwnerV1 {
        tail_load_attempts: owner.tail_load_attempts + prefix,
        cumulative_tail_load_attempts: owner.cumulative_tail_load_attempts + prefix,
        ..owner
    }
}

pub open spec fn owner_after_tail_round_v1(owner: OwnerV1) -> OwnerV1 {
    OwnerV1 {
        completed_tail_rounds: owner.completed_tail_rounds + 1,
        tail_load_attempts: owner.tail_load_attempts + owner.tails.len(),
        cumulative_tail_load_attempts:
            owner.cumulative_tail_load_attempts + owner.tails.len(),
        ..owner
    }
}

pub open spec fn owner_after_audit_prefix_v1(owner: OwnerV1, prefix: nat) -> OwnerV1 {
    OwnerV1 {
        audit_load_attempts: prefix,
        cumulative_audit_load_attempts: owner.cumulative_audit_load_attempts + prefix,
        ..owner
    }
}

pub open spec fn owner_after_full_audit_v1(owner: OwnerV1) -> OwnerV1 {
    owner_after_audit_prefix_v1(owner, owner.requests.len())
}

pub open spec fn completed_outcome_v1(
    phase: PhaseV1,
    owner: OwnerV1,
) -> WaitOutcomeV1 {
    WaitOutcomeV1 {
        phase,
        returned_owner: None,
        completed_requests: owner.requests,
        completed_tail_rounds: owner.completed_tail_rounds,
        tail_load_attempts: owner.tail_load_attempts,
        audit_load_attempts: owner.audit_load_attempts,
        cumulative_tail_load_attempts: owner.cumulative_tail_load_attempts,
        cumulative_audit_load_attempts: owner.cumulative_audit_load_attempts,
        retired_count: owner.requests.len(),
        post_bind_allocation_events: owner.post_bind_allocation_events,
    }
}

pub open spec fn wait_round_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
) -> WaitOutcomeV1 {
    if !waiting_owner_is_exact_v1(owner)
        || !currentness_identity_is_exact_v1(owner, current)
        || !current.opening_current
    {
        owner_outcome_v1(PhaseV1::OpeningCurrentnessTerminal, owner)
    } else if !input.exact_tail_roster
        || !tail_error_prefix_is_valid_v1(owner, input)
        || !tail_queue_mask_is_exact_v1(owner, input)
    {
        owner_outcome_v1(PhaseV1::TailRosterTerminal, owner)
    } else if input.tail_error_prefix.is_some() {
        owner_outcome_v1(
            PhaseV1::TailErrorTerminal,
            owner_after_tail_prefix_v1(owner, input.tail_error_prefix.unwrap()),
        )
    } else {
        let after_tail = owner_after_tail_round_v1(owner);
        if !all_initial_tails_ready_v1(owner, input) && !input.deadline_reached {
            owner_outcome_v1(PhaseV1::Pending, after_tail)
        } else if !input.exact_audit_roster
            || !audit_error_prefix_is_valid_v1(owner, input)
        {
            owner_outcome_v1(PhaseV1::AuditRosterTerminal, after_tail)
        } else if input.audit_error_prefix.is_some() {
            owner_outcome_v1(
                PhaseV1::AuditErrorTerminal,
                owner_after_audit_prefix_v1(after_tail, input.audit_error_prefix.unwrap()),
            )
        } else if !audit_queue_masks_are_exact_v1(owner, input) {
            owner_outcome_v1(PhaseV1::AuditRosterTerminal, after_tail)
        } else {
            let audited = owner_after_full_audit_v1(after_tail);
            if !current.closing_current {
                owner_outcome_v1(PhaseV1::ClosingCurrentnessTerminal, audited)
            } else if tail_ordering_violation_v1(owner, input) {
                owner_outcome_v1(PhaseV1::TailOrderingTerminal, audited)
            } else if any_audit_pending_v1(owner, input) {
                owner_outcome_v1(PhaseV1::TimedOut, audited)
            } else if !input.retirement_preflight {
                owner_outcome_v1(PhaseV1::RetirementPreflightTerminal, audited)
            } else if input.retake_succeeds {
                completed_outcome_v1(PhaseV1::Completed, audited)
            } else {
                completed_outcome_v1(PhaseV1::CompletedOpaque, audited)
            }
        }
    }
}

pub open spec fn retry_timeout_owner_v1(owner: OwnerV1) -> OwnerV1 {
    OwnerV1 {
        wait_epoch: owner.wait_epoch + 1,
        completed_tail_rounds: 0,
        tail_load_attempts: 0,
        audit_load_attempts: 0,
        ..owner
    }
}

pub open spec fn timeout_owner_is_exact_v1(owner: OwnerV1) -> bool {
    publication_roster_is_exact_v1(owner)
        && owner.wait_epoch > 0
        && owner.tail_load_attempts == owner.completed_tail_rounds * owner.tails.len()
        && owner.audit_load_attempts == owner.requests.len()
        && owner.cumulative_tail_load_attempts >= owner.tail_load_attempts
        && owner.cumulative_audit_load_attempts >= owner.audit_load_attempts
        && owner.retired_count == 0
        && owner.post_bind_allocation_events == 0
}

pub struct PanicStageV1 {
    pub tail_prefix: nat,
    pub completed_tail_round: bool,
    pub audit_prefix: nat,
}

pub open spec fn panic_stage_is_valid_v1(owner: OwnerV1, stage: PanicStageV1) -> bool {
    if stage.completed_tail_round {
        stage.tail_prefix == owner.tails.len()
            && stage.audit_prefix <= owner.requests.len()
    } else {
        stage.tail_prefix < owner.tails.len() && stage.audit_prefix == 0
    }
}

pub open spec fn panic_guard_v1(owner: OwnerV1, stage: PanicStageV1) -> WaitOutcomeV1 {
    if !waiting_owner_is_exact_v1(owner) || !panic_stage_is_valid_v1(owner, stage) {
        owner_outcome_v1(PhaseV1::PanicStageTerminal, owner)
    } else {
        let after_tail = if stage.completed_tail_round {
            owner_after_tail_round_v1(owner)
        } else {
            owner_after_tail_prefix_v1(owner, stage.tail_prefix)
        };
        owner_outcome_v1(
            PhaseV1::PanicRetained,
            owner_after_audit_prefix_v1(after_tail, stage.audit_prefix),
        )
    }
}

// Obligation 1: the two profile capacities and packet capacity are exact.
pub proof fn constants_are_exact_v1()
    ensures
        maximum_queues_v1() == 16,
        maximum_combined_queues_v1() == 14,
        requests_per_shard_v1() == 63,
        ring_slot_count_v1() == 64,
        maximum_requests_v1() == 1008,
{}

// Obligation 2: combined admission is exactly the even 2..14 domain.
pub proof fn combined_profile_is_exact_v1(queues: nat)
    ensures profile_admits_v1(ProfileV1::Combined, queues)
        <==> 2 <= queues <= 14 && queues % 2 == 0,
{}

// Obligation 3: standalone admission is exactly the even 2..16 domain.
pub proof fn standalone_profile_is_exact_v1(queues: nat)
    ensures profile_admits_v1(ProfileV1::Standalone, queues)
        <==> 2 <= queues <= 16 && queues % 2 == 0,
{}

// Obligation 4: the standalone maximum carries exactly 1008 requests.
pub proof fn standalone_capacity_is_exact_v1()
    ensures 16 * requests_per_shard_v1() == maximum_requests_v1(),
{}

// Obligation 5: every admitted queue cursor is normalized into range.
pub proof fn normalized_cursor_is_bounded_v1(first: nat, queues: nat, queue: nat)
    requires 0 < queues, first < queues, queue < queues,
    ensures normalized_slot_v1(first, queues, queue) < queues,
{
    assert(queue + queues - first < 2 * queues);
}

// Obligation 6: a striped request assigned from the cursor normalizes to i mod Q.
pub proof fn rotating_assignment_normalizes_v1(first: nat, queues: nat, index: nat)
    requires 0 < queues, first < queues,
    ensures normalized_slot_v1(
        first,
        queues,
        queue_for_request_v1(first, queues, index),
    ) == index % queues,
{
    lemma_mod_bound(index as int, queues as int);
    lemma_add_mod_noop_right(first as int, index as int, queues as int);
    let remainder = index % queues;
    assert(remainder < queues);
    if first + remainder < queues {
        lemma_small_mod(first + remainder, queues);
        assert((first + index) % queues == first + remainder);
    } else {
        let reduced = (first + remainder - queues) as nat;
        assert(reduced < queues);
        lemma_small_mod(reduced, queues);
        lemma_mod_sub_multiples_vanish((first + remainder) as int, queues as int);
        assert((first + remainder) % queues == reduced);
        assert((first + index) % queues == reduced);
    }
}

// Obligation 7: exact tickets retain physical engine labels, not normalized labels.
pub proof fn exact_ticket_has_physical_engine_v1(owner: OwnerV1, index: int)
    requires publication_roster_is_exact_v1(owner), 0 <= index < owner.requests.len(),
    ensures owner.requests[index].ticket.engine
        == engine_for_queue_v1(owner.requests[index].ticket.queue_ordinal),
{
    reveal(publication_roster_is_exact_v1);
    assert(forall|candidate: int| 0 <= candidate < owner.requests.len() ==>
        request_is_exact_v1(owner, candidate));
    assert(request_is_exact_v1(owner, index));
}

// Obligation 8: admission requires a nonzero occurrence and burned epoch.
pub proof fn submission_epoch_is_nonzero_and_burned_v1(owner: OwnerV1)
    requires publication_roster_is_exact_v1(owner),
    ensures
        owner.owner_occurrence > 0,
        owner.session_occurrence > 0,
        0 < owner.submission_epoch <= owner.burned_epoch_high_water,
        owner.exactly_one_epoch_issuer,
{}

// Obligation 9: one owner mints the next monotonic nonzero epoch.
pub proof fn epoch_mint_is_monotonic_v1(high_water: nat)
    requires high_water > 0,
    ensures high_water + 1 > high_water, high_water + 1 > 0,
{}

// Obligation 10: the packet publication contract binds the exact ticket and signal.
pub proof fn packet_and_signal_binding_is_exact_v1(owner: OwnerV1, index: int)
    requires publication_roster_is_exact_v1(owner), 0 <= index < owner.requests.len(),
    ensures {
        let request = owner.requests[index];
        &&& request.publication.ticket == request.ticket
        &&& request.publication.packet_signal == request.ticket.signal
        &&& signal_is_exact_v1(request.ticket.signal)
    },
{
    reveal(publication_roster_is_exact_v1);
    assert(request_is_exact_v1(owner, index));
}

// Obligation 11: the exact system+snoop fence header is an explicit premise.
pub proof fn fence_header_is_exact_v1(owner: OwnerV1, index: int)
    requires publication_roster_is_exact_v1(owner), 0 <= index < owner.requests.len(),
    ensures owner.requests[index].publication.fence_header == 0x0053_0005,
{
    reveal(publication_roster_is_exact_v1);
    assert(request_is_exact_v1(owner, index));
}

// Obligation 12: body, record, pointer, then doorbell facts are all explicit premises.
pub proof fn publication_boundary_is_explicit_v1(owner: OwnerV1, index: int)
    requires publication_roster_is_exact_v1(owner), 0 <= index < owner.requests.len(),
    ensures {
        let publication = owner.requests[index].publication;
        &&& publication.complete_packet_written
        &&& publication.complete_packet_before_write_pointer_release
        &&& publication.record_retained_before_write_pointer_release
        &&& publication.write_pointer_published_release
        &&& publication.doorbell_published_release
        &&& publication.write_pointer_release_before_doorbell_release
    },
{
    reveal(publication_roster_is_exact_v1);
    assert(request_is_exact_v1(owner, index));
}

// Obligation 13: engine admission, scope, snoop, and visibility are premises.
pub proof fn native_visibility_premises_are_explicit_v1(owner: OwnerV1, index: int)
    requires publication_roster_is_exact_v1(owner), 0 <= index < owner.requests.len(),
    ensures {
        let publication = owner.requests[index].publication;
        &&& publication.admitted_gfx942_engine
        &&& publication.system_scope
        &&& publication.snoop
        &&& publication.signal_read_names_fence
        &&& publication.completion_implies_preceding_visible
    },
{
    reveal(publication_roster_is_exact_v1);
    assert(request_is_exact_v1(owner, index));
}

// Obligation 14: request signals and same-queue packet occurrences are distinct.
pub proof fn published_requests_are_distinct_v1(owner: OwnerV1)
    requires publication_roster_is_exact_v1(owner),
    ensures requests_are_distinct_v1(owner.requests),
{}

// Obligation 15: the 64-slot physical ring admits slot 63 while each shard holds at most 63.
pub proof fn physical_ring_slot_domain_is_exact_v1()
    ensures requests_per_shard_v1() == 63,
        ring_slot_count_v1() == 64,
        requests_per_shard_v1() < ring_slot_count_v1(),
        63 < ring_slot_count_v1(),
{}

// Obligation 16: exactly one normalized tail is bound to every active shard.
pub proof fn tail_roster_has_exact_active_count_v1(owner: OwnerV1)
    requires publication_roster_is_exact_v1(owner),
    ensures owner.tails.len()
        == active_shards_v1(owner.requests.len(), owner.queue_ids.len()),
{}

// Obligation 17: each tail is the last request in its normalized striped shard.
pub proof fn each_tail_is_exact_last_request_v1(owner: OwnerV1, slot: int)
    requires publication_roster_is_exact_v1(owner), 0 <= slot < owner.tails.len(),
    ensures {
        let last = last_request_for_normalized_slot_v1(
            owner.requests.len(),
            owner.queue_ids.len(),
            slot as nat,
        );
        &&& last < owner.requests.len()
        &&& owner.tails[slot].normalized_queue_slot == slot
        &&& owner.tails[slot].ticket == owner.requests[last as int].ticket
    },
{
    reveal(publication_roster_is_exact_v1);
    assert(tail_is_exact_v1(owner, slot));
}

// Obligation 18: completed rounds account for exactly rounds times active tails.
pub proof fn waiting_round_formula_is_exact_v1(owner: OwnerV1)
    requires waiting_owner_is_exact_v1(owner),
    ensures owner.tail_load_attempts
        == owner.completed_tail_rounds * owner.tails.len(),
{}

// Obligation 19: one full tail round preserves the exact round formula.
pub proof fn completed_tail_round_adds_exact_s_v1(owner: OwnerV1)
    requires waiting_owner_is_exact_v1(owner),
    ensures {
        let after = owner_after_tail_round_v1(owner);
        &&& after.completed_tail_rounds == owner.completed_tail_rounds + 1
        &&& after.tail_load_attempts == owner.tail_load_attempts + owner.tails.len()
        &&& after.tail_load_attempts == after.completed_tail_rounds * after.tails.len()
    },
{
    reveal(waiting_owner_is_exact_v1);
    assert(owner.tail_load_attempts
        == owner.completed_tail_rounds * owner.tails.len());
    assert((owner.completed_tail_rounds + 1) * owner.tails.len()
        == owner.completed_tail_rounds * owner.tails.len() + owner.tails.len())
        by (nonlinear_arith);
}

// Obligation 20: an early tail error accounts for rounds*S plus its exact prefix.
pub proof fn tail_error_prefix_formula_is_exact_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        tail_queue_mask_is_exact_v1(owner, input),
        tail_error_prefix_is_valid_v1(owner, input),
        input.tail_error_prefix.is_some(),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::TailErrorTerminal
        &&& outcome.completed_tail_rounds == owner.completed_tail_rounds
        &&& outcome.tail_load_attempts
            == owner.completed_tail_rounds * owner.tails.len()
                + input.tail_error_prefix.unwrap()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 21: malformed tail-error coordinates fail closed before a load.
pub proof fn malformed_tail_prefix_is_fail_closed_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        !tail_error_prefix_is_valid_v1(owner, input),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::TailRosterTerminal
        &&& outcome.tail_load_attempts == owner.tail_load_attempts
        &&& outcome.retired_count == 0
    },
{}

// Obligation 22: Pending before the deadline performs one tail round and no audit.
pub proof fn pending_before_deadline_is_tail_only_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        !all_initial_tails_ready_v1(owner, input),
        !input.deadline_reached,
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::Pending
        &&& outcome.audit_load_attempts == 0
        &&& outcome.tail_load_attempts == owner.tail_load_attempts + owner.tails.len()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 23: a deadline or all-ready tails enters the one audit path.
pub proof fn final_path_enters_audit_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached || all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        audit_error_prefix_is_valid_v1(owner, input),
    ensures wait_round_v1(owner, current, input).phase != PhaseV1::Pending,
{}

// Obligation 24: an early audit error accounts for completed rounds*S and prefix k.
pub proof fn audit_error_prefix_formula_is_exact_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached || all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        audit_error_prefix_is_valid_v1(owner, input),
        input.audit_error_prefix.is_some(),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::AuditErrorTerminal
        &&& outcome.completed_tail_rounds == owner.completed_tail_rounds + 1
        &&& outcome.tail_load_attempts
            == (owner.completed_tail_rounds + 1) * owner.tails.len()
        &&& outcome.audit_load_attempts == input.audit_error_prefix.unwrap()
        &&& outcome.retired_count == 0
    },
{
    completed_tail_round_adds_exact_s_v1(owner);
}

// Obligation 25: malformed audit coordinates fail closed with zero audit loads.
pub proof fn malformed_audit_prefix_is_fail_closed_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached || all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        !audit_error_prefix_is_valid_v1(owner, input),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::AuditRosterTerminal
        &&& outcome.audit_load_attempts == 0
        &&& outcome.retired_count == 0
    },
{}

// Obligation 26: any non-error terminal audit is exactly one N scan.
pub proof fn full_audit_has_exact_n_work_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached || all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
    ensures wait_round_v1(owner, current, input).audit_load_attempts
        == owner.requests.len(),
{}

// Obligation 27: closing currentness is checked after the full audit and before retirement.
pub proof fn closing_currentness_failure_is_terminal_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        !current.closing_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached || all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::ClosingCurrentnessTerminal
        &&& outcome.audit_load_attempts == owner.requests.len()
        &&& outcome.retired_count == 0
    },
{}

// Obligation 28: a ready tail with a pending predecessor is terminal.
pub proof fn ready_tail_pending_prefix_is_terminal_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
    queue: int,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        current.closing_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached || all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
        0 <= queue < owner.queue_ids.len(),
        input.audit_pending_queues[queue],
        input.initial_ready_tail_queues[queue] || input.final_ready_tail_queues[queue],
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::TailOrderingTerminal
        &&& outcome.retired_count == 0
    },
{}

// Obligation 29: timeout is retryable exact custody with zero retirement.
pub proof fn timeout_preserves_exact_custody_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        current.closing_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        input.deadline_reached,
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
        any_audit_pending_v1(owner, input),
        !tail_ordering_violation_v1(owner, input),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::TimedOut
        &&& outcome.returned_owner.is_some()
        &&& outcome.returned_owner.unwrap().requests == owner.requests
        &&& outcome.returned_owner.unwrap().tails == owner.tails
        &&& outcome.audit_load_attempts == owner.requests.len()
        &&& outcome.retired_count == 0
        &&& timeout_owner_is_exact_v1(outcome.returned_owner.unwrap())
    },
{
    reveal(timeout_owner_is_exact_v1);
    reveal(waiting_owner_is_exact_v1);
    reveal(wait_round_v1);
    reveal(owner_after_tail_round_v1);
    reveal(owner_after_full_audit_v1);
    reveal(owner_after_audit_prefix_v1);
    reveal(publication_roster_is_exact_v1);
    reveal(request_is_exact_v1);
    reveal(tail_is_exact_v1);
    completed_tail_round_adds_exact_s_v1(owner);
    let after_tail = owner_after_tail_round_v1(owner);
    let audited = owner_after_full_audit_v1(after_tail);
    assert(audited.requests == owner.requests);
    assert(audited.tails == owner.tails);
    assert forall|index: int| 0 <= index < audited.requests.len() implies
        request_is_exact_v1(audited, index) by {
        assert(request_is_exact_v1(owner, index));
    }
    assert forall|slot: int| 0 <= slot < audited.tails.len() implies
        tail_is_exact_v1(audited, slot) by {
        assert(tail_is_exact_v1(owner, slot));
    }
    assert(publication_roster_is_exact_v1(audited));
    assert(audited.tail_load_attempts
        == audited.completed_tail_rounds * audited.tails.len());
    assert(audited.audit_load_attempts == audited.requests.len());
    assert(audited.cumulative_tail_load_attempts >= audited.tail_load_attempts);
    assert(audited.cumulative_audit_load_attempts >= audited.audit_load_attempts);
    assert(timeout_owner_is_exact_v1(audited));
}

// Obligation 30: retry burns the old wait epoch and resets only per-epoch work.
pub proof fn timeout_retry_advances_wait_epoch_v1(timeout_owner: OwnerV1)
    requires timeout_owner_is_exact_v1(timeout_owner),
    ensures {
        let retried = retry_timeout_owner_v1(timeout_owner);
        &&& retried.wait_epoch == timeout_owner.wait_epoch + 1
        &&& retried.completed_tail_rounds == 0
        &&& retried.tail_load_attempts == 0
        &&& retried.audit_load_attempts == 0
        &&& retried.requests == timeout_owner.requests
        &&& retried.tails == timeout_owner.tails
        &&& retried.retired_count == 0
    },
{}

// Obligation 31: retry preserves cumulative work across timeout epochs.
pub proof fn timeout_retry_preserves_cumulative_work_v1(timeout_owner: OwnerV1)
    requires timeout_owner_is_exact_v1(timeout_owner),
    ensures {
        let retried = retry_timeout_owner_v1(timeout_owner);
        &&& retried.cumulative_tail_load_attempts
            == timeout_owner.cumulative_tail_load_attempts
        &&& retried.cumulative_audit_load_attempts
            == timeout_owner.cumulative_audit_load_attempts
    },
{}

// Obligation 32: a retried exact timeout owner is a fresh exact waiting owner.
pub proof fn timeout_retry_restores_waiting_invariant_v1(timeout_owner: OwnerV1)
    requires timeout_owner_is_exact_v1(timeout_owner),
    ensures waiting_owner_is_exact_v1(retry_timeout_owner_v1(timeout_owner)),
{
    reveal(waiting_owner_is_exact_v1);
    reveal(retry_timeout_owner_v1);
    reveal(timeout_owner_is_exact_v1);
    reveal(publication_roster_is_exact_v1);
    reveal(request_is_exact_v1);
    reveal(tail_is_exact_v1);
    let retried = retry_timeout_owner_v1(timeout_owner);
    assert(retried.requests == timeout_owner.requests);
    assert(retried.tails == timeout_owner.tails);
    assert forall|index: int| 0 <= index < retried.requests.len() implies
        request_is_exact_v1(retried, index) by {
        assert(request_is_exact_v1(timeout_owner, index));
    }
    assert forall|slot: int| 0 <= slot < retried.tails.len() implies
        tail_is_exact_v1(retried, slot) by {
        assert(tail_is_exact_v1(timeout_owner, slot));
    }
    assert(publication_roster_is_exact_v1(retried));
    assert(retried.wait_epoch > 0);
    assert(retried.tail_load_attempts
        == retried.completed_tail_rounds * retried.tails.len());
}

// Obligation 33: failure of retirement preflight retires nothing.
pub proof fn retirement_preflight_is_all_or_zero_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        current.closing_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
        !any_audit_pending_v1(owner, input),
        !input.retirement_preflight,
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::RetirementPreflightTerminal
        &&& outcome.returned_owner.unwrap().requests == owner.requests
        &&& outcome.retired_count == 0
    },
{}

// Obligation 34: success retires exactly N in original request order.
pub proof fn success_is_all_or_n_and_ordered_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        current.closing_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
        !any_audit_pending_v1(owner, input),
        input.retirement_preflight,
        input.retake_succeeds,
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::Completed
        &&& outcome.returned_owner.is_none()
        &&& outcome.completed_requests == owner.requests
        &&& outcome.retired_count == owner.requests.len()
    },
{}

// Obligation 35: post-retirement retake failure retains opaque exact roster identity.
pub proof fn retake_failure_is_completed_opaque_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires
        waiting_owner_is_exact_v1(owner),
        currentness_identity_is_exact_v1(owner, current),
        current.opening_current,
        current.closing_current,
        input.exact_tail_roster,
        input.tail_error_prefix.is_none(),
        tail_queue_mask_is_exact_v1(owner, input),
        all_initial_tails_ready_v1(owner, input),
        input.exact_audit_roster,
        input.audit_error_prefix.is_none(),
        audit_queue_masks_are_exact_v1(owner, input),
        !any_audit_pending_v1(owner, input),
        input.retirement_preflight,
        !input.retake_succeeds,
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase == PhaseV1::CompletedOpaque
        &&& outcome.completed_requests == owner.requests
        &&& outcome.retired_count == owner.requests.len()
    },
{}

// Obligation 36: all non-retirement phases retain the exact whole roster.
pub proof fn nonretirement_outcomes_conserve_roster_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires waiting_owner_is_exact_v1(owner),
    ensures {
        let outcome = wait_round_v1(owner, current, input);
        &&& outcome.phase != PhaseV1::Completed
            && outcome.phase != PhaseV1::CompletedOpaque ==>
                outcome.returned_owner.is_some()
        &&& outcome.phase != PhaseV1::Completed
            && outcome.phase != PhaseV1::CompletedOpaque ==>
                outcome.returned_owner.unwrap().requests == owner.requests
        &&& outcome.phase != PhaseV1::Completed
            && outcome.phase != PhaseV1::CompletedOpaque ==>
                outcome.retired_count == 0
    },
{}

// Obligation 37: an invalid panic coordinate is terminal rather than reusable panic custody.
pub proof fn invalid_panic_stage_is_terminal_v1(owner: OwnerV1, stage: PanicStageV1)
    requires waiting_owner_is_exact_v1(owner), !panic_stage_is_valid_v1(owner, stage),
    ensures {
        let outcome = panic_guard_v1(owner, stage);
        &&& outcome.phase == PhaseV1::PanicStageTerminal
        &&& outcome.returned_owner.unwrap().requests == owner.requests
        &&& outcome.returned_owner.unwrap().tails == owner.tails
        &&& outcome.retired_count == 0
    },
{}

// Obligation 38: a valid panic stage conserves exact roster and retires nothing.
pub proof fn panic_guard_conserves_custody_v1(owner: OwnerV1, stage: PanicStageV1)
    requires waiting_owner_is_exact_v1(owner), panic_stage_is_valid_v1(owner, stage),
    ensures {
        let outcome = panic_guard_v1(owner, stage);
        &&& outcome.phase == PhaseV1::PanicRetained
        &&& outcome.returned_owner.unwrap().requests == owner.requests
        &&& outcome.returned_owner.unwrap().tails == owner.tails
        &&& outcome.retired_count == 0
    },
{}

// Obligation 39: a panic in a tail prefix has rounds*S+k exact work.
pub proof fn panic_tail_prefix_formula_is_exact_v1(owner: OwnerV1, stage: PanicStageV1)
    requires
        waiting_owner_is_exact_v1(owner),
        panic_stage_is_valid_v1(owner, stage),
        !stage.completed_tail_round,
    ensures {
        let outcome = panic_guard_v1(owner, stage);
        &&& outcome.completed_tail_rounds == owner.completed_tail_rounds
        &&& outcome.tail_load_attempts
            == owner.completed_tail_rounds * owner.tails.len() + stage.tail_prefix
        &&& outcome.audit_load_attempts == 0
    },
{}

// Obligation 40: a panic in the audit has one full tail round and exact audit prefix.
pub proof fn panic_audit_prefix_formula_is_exact_v1(owner: OwnerV1, stage: PanicStageV1)
    requires
        waiting_owner_is_exact_v1(owner),
        panic_stage_is_valid_v1(owner, stage),
        stage.completed_tail_round,
    ensures {
        let outcome = panic_guard_v1(owner, stage);
        &&& outcome.completed_tail_rounds == owner.completed_tail_rounds + 1
        &&& outcome.tail_load_attempts
            == (owner.completed_tail_rounds + 1) * owner.tails.len()
        &&& outcome.audit_load_attempts == stage.audit_prefix
        &&& outcome.retired_count == 0
    },
{
    completed_tail_round_adds_exact_s_v1(owner);
}

// Obligation 41: this abstract transition adds no post-bind allocation event.
pub proof fn modeled_wait_adds_no_allocation_event_v1(
    owner: OwnerV1,
    current: CurrentnessV1,
    input: WaitInputV1,
)
    requires owner.post_bind_allocation_events == 0,
    ensures wait_round_v1(owner, current, input).post_bind_allocation_events == 0,
{}

// Obligation 42: consecutive authentic timeout receipts advance epochs and preserve custody.
pub proof fn consecutive_timeout_retries_are_monotonic_v1(
    first_timeout: OwnerV1,
    second_timeout: OwnerV1,
)
    requires
        timeout_owner_is_exact_v1(first_timeout),
        timeout_owner_is_exact_v1(second_timeout),
        second_timeout.wait_epoch == first_timeout.wait_epoch + 1,
        second_timeout.requests == first_timeout.requests,
        second_timeout.tails == first_timeout.tails,
    ensures {
        let first_retry = retry_timeout_owner_v1(first_timeout);
        let second_retry = retry_timeout_owner_v1(second_timeout);
        &&& first_timeout.wait_epoch < first_retry.wait_epoch
        &&& first_retry.wait_epoch < second_retry.wait_epoch
        &&& second_retry.requests == first_retry.requests
        &&& second_retry.tails == first_retry.tails
        &&& first_retry.cumulative_tail_load_attempts
            == first_timeout.cumulative_tail_load_attempts
        &&& second_retry.cumulative_tail_load_attempts
            == second_timeout.cumulative_tail_load_attempts
        &&& first_retry.cumulative_audit_load_attempts
            == first_timeout.cumulative_audit_load_attempts
        &&& second_retry.cumulative_audit_load_attempts
            == second_timeout.cumulative_audit_load_attempts
    },
{}

}
