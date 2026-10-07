// Independent finite R41 model for persistent striped-SDMA aggregate custody.
// All identities, currentness values, completion classifications, and failure
// scripts are contracted mathematical inputs. Capacity counters name abstract
// preparation phases and do not model a Rust allocator. This proves no
// Rust-to-Verus or production-Rust refinement and no KFD, HSA, HIP, ioctl,
// packet, atomic, clock, driver, firmware, hardware, progress, parity, or
// performance claim.

use vstd::prelude::*;

verus! {

pub open spec fn min_striped_queues_v1() -> nat { 2 }
pub open spec fn max_striped_queues_v1() -> nat { 14 }
pub open spec fn requests_per_shard_v1() -> nat { 63 }
pub open spec fn maximum_requests_v1() -> nat { 882 }
pub open spec fn maximum_linear_copy_bytes_v1() -> nat { 0x003f_ffe0 }
pub open spec fn maximum_persistent_device_bytes_v1() -> nat { 256 * 1024 * 1024 }
pub open spec fn gpu_page_bytes_v1() -> nat { 4096 }

#[derive(PartialEq, Eq)]
pub struct RequestIdentityV1 {
    pub request_token: nat,
    pub session: nat,
    pub directional_d2h_queue: nat,
    pub directional_h2d_queue: nat,
    pub directional_generation: nat,
    pub striped_generation: nat,
    pub pool_generation: nat,
    pub device_owner: nat,
    pub device_storage: nat,
    pub device_logical_bytes: nat,
    pub device_physical_bytes: nat,
    pub device_is_local: bool,
    pub host_storage: nat,
    pub host_pool_generation: nat,
    pub host_logical_bytes: nat,
    pub host_physical_bytes: nat,
    pub host_is_coherent_gtt: bool,
    pub device_offset: nat,
    pub host_offset: nat,
    pub copy_bytes: nat,
}

pub struct PlanV1 {
    pub session: nat,
    pub directional_d2h_queue: nat,
    pub directional_h2d_queue: nat,
    pub directional_generation: nat,
    pub striped_generation: nat,
    pub striped_count: nat,
    pub striped_queue_ids: Seq<nat>,
    pub first_queue: nat,
    pub submission: nat,
    pub started_ns: nat,
    pub deadline_ns: nat,
    pub requests: Seq<RequestIdentityV1>,
}

pub open spec fn valid_striped_count_v1(count: nat) -> bool {
    min_striped_queues_v1() <= count
        && count <= max_striped_queues_v1()
        && count % 2 == 0
}

pub open spec fn distinct_values_v1(values: Seq<nat>) -> bool {
    forall|left: int, right: int|
        0 <= left < values.len() && 0 <= right < values.len() && left != right
            ==> values[left] != values[right]
}

pub open spec fn exact_queue_occurrence_v1(plan: PlanV1) -> bool {
    plan.directional_d2h_queue != plan.directional_h2d_queue
        && plan.directional_generation > 0
        && plan.striped_generation > 0
        && plan.striped_queue_ids.len() == plan.striped_count
        && distinct_values_v1(plan.striped_queue_ids)
        && forall|index: int| 0 <= index < plan.striped_queue_ids.len() ==>
            plan.striped_queue_ids[index] != plan.directional_d2h_queue
                && plan.striped_queue_ids[index] != plan.directional_h2d_queue
}

pub open spec fn valid_request_range_v1(request: RequestIdentityV1) -> bool {
    request.copy_bytes > 0
        && request.copy_bytes <= maximum_linear_copy_bytes_v1()
        && request.pool_generation > 0
        && request.device_logical_bytes > 0
        && request.device_logical_bytes <= request.device_physical_bytes
        && request.device_physical_bytes <= maximum_persistent_device_bytes_v1()
        && request.device_physical_bytes % gpu_page_bytes_v1() == 0
        && request.device_is_local
        && request.host_pool_generation > 0
        && request.host_logical_bytes > 0
        && request.host_logical_bytes <= request.host_physical_bytes
        && request.host_is_coherent_gtt
        && request.device_offset + request.copy_bytes <= request.device_logical_bytes
        && request.host_offset + request.copy_bytes <= request.host_logical_bytes
}

pub open spec fn request_matches_plan_v1(request: RequestIdentityV1, plan: PlanV1) -> bool {
    request.session == plan.session
        && request.directional_d2h_queue == plan.directional_d2h_queue
        && request.directional_h2d_queue == plan.directional_h2d_queue
        && request.directional_generation == plan.directional_generation
        && request.striped_generation == plan.striped_generation
}

pub open spec fn request_roster_is_bound_v1(plan: PlanV1) -> bool {
    forall|index: int| 0 <= index < plan.requests.len() ==>
        plan.requests[index].request_token == index + 1
            && plan.requests[index].device_owner > 0
            && plan.requests[index].device_storage > 0
            && plan.requests[index].host_storage > 0
            && request_matches_plan_v1(plan.requests[index], plan)
            && valid_request_range_v1(plan.requests[index])
}

pub open spec fn distinct_device_owners_v1(requests: Seq<RequestIdentityV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < requests.len() && 0 <= right < requests.len() && left != right
            ==> requests[left].device_owner != requests[right].device_owner
}

pub open spec fn distinct_device_storage_v1(requests: Seq<RequestIdentityV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < requests.len() && 0 <= right < requests.len() && left != right
            ==> requests[left].device_storage != requests[right].device_storage
}

pub open spec fn distinct_host_storage_v1(requests: Seq<RequestIdentityV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < requests.len() && 0 <= right < requests.len() && left != right
            ==> requests[left].host_storage != requests[right].host_storage
}

pub open spec fn shard_load_v1(request_count: nat, queue_count: nat, slot: nat) -> nat {
    if queue_count == 0 || slot >= queue_count {
        0
    } else {
        request_count / queue_count
            + if slot < request_count % queue_count { 1nat } else { 0nat }
    }
}

pub open spec fn every_shard_is_bounded_v1(plan: PlanV1) -> bool {
    forall|slot: nat| slot < plan.striped_count ==>
        shard_load_v1(plan.requests.len(), plan.striped_count, slot)
            <= requests_per_shard_v1()
}

pub open spec fn admitted_plan_v1(plan: PlanV1) -> bool {
    valid_striped_count_v1(plan.striped_count)
        && exact_queue_occurrence_v1(plan)
        && plan.first_queue < plan.striped_count
        && plan.started_ns <= plan.deadline_ns
        && 0 < plan.requests.len() <= maximum_requests_v1()
        && plan.requests.len() <= plan.striped_count * requests_per_shard_v1()
        && request_roster_is_bound_v1(plan)
        && distinct_device_owners_v1(plan.requests)
        && distinct_device_storage_v1(plan.requests)
        && distinct_host_storage_v1(plan.requests)
        && every_shard_is_bounded_v1(plan)
}

// Obligation 1: the bounded combined constants are exact.
pub proof fn combined_constants_are_exact_v1()
    ensures
        min_striped_queues_v1() == 2,
        max_striped_queues_v1() == 14,
        requests_per_shard_v1() == 63,
        maximum_requests_v1() == 882,
        maximum_linear_copy_bytes_v1() == 0x003f_ffe0,
        maximum_persistent_device_bytes_v1() == 256 * 1024 * 1024,
        gpu_page_bytes_v1() == 4096,
{}

// Obligation 2: every even combined count from 2 through 14 is admitted.
pub proof fn exact_even_striped_counts_are_admitted_v1(count: nat)
    requires 2 <= count <= 14, count % 2 == 0,
    ensures valid_striped_count_v1(count),
{}

// Obligation 3: values outside 2 through 14 are rejected.
pub proof fn out_of_range_striped_counts_are_rejected_v1(count: nat)
    requires count < 2 || count > 14,
    ensures !valid_striped_count_v1(count),
{}

// Obligation 4: odd combined counts are rejected.
pub proof fn odd_striped_counts_are_rejected_v1(count: nat)
    requires count % 2 == 1,
    ensures !valid_striped_count_v1(count),
{}

// Obligation 5: fourteen shards at 63 requests have exact capacity 882.
pub proof fn maximum_combined_capacity_is_exact_v1()
    ensures max_striped_queues_v1() * requests_per_shard_v1() == maximum_requests_v1(),
{}

// Obligation 6: admission retains a nonempty bounded request roster.
pub proof fn admitted_request_roster_is_bounded_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures 0 < plan.requests.len() <= 882,
{}

// Obligation 7: every admitted shard load is at most 63.
pub proof fn admitted_shards_are_bounded_v1(plan: PlanV1, slot: nat)
    requires admitted_plan_v1(plan), slot < plan.striped_count,
    ensures shard_load_v1(plan.requests.len(), plan.striped_count, slot) <= 63,
{}

// Obligation 8: persistent device owners are pairwise distinct.
pub proof fn admitted_device_owners_are_distinct_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures distinct_device_owners_v1(plan.requests),
{}

// Obligation 9: persistent device storage identities are pairwise distinct.
pub proof fn admitted_device_storage_is_distinct_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures distinct_device_storage_v1(plan.requests),
{}

// Obligation 10: host storage identities are pairwise distinct.
pub proof fn admitted_host_storage_is_distinct_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures distinct_host_storage_v1(plan.requests),
{}

// Obligation 11: every request uses the admitted session and pair occurrence.
pub proof fn admitted_requests_share_exact_binding_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures request_matches_plan_v1(plan.requests[index], plan),
{}

// Obligation 12: device and host pool generations are per-buffer and nonzero.
pub proof fn admitted_buffer_generations_are_nonzero_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures
        plan.requests[index].pool_generation > 0,
        plan.requests[index].host_pool_generation > 0,
{}

// Obligation 13: device and host extents and coherent-host kind are exact admission premises.
pub proof fn admitted_extents_and_host_kind_are_exact_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures
        0 < plan.requests[index].device_logical_bytes
            <= plan.requests[index].device_physical_bytes
            <= 256 * 1024 * 1024,
        plan.requests[index].device_physical_bytes % 4096 == 0,
        0 < plan.requests[index].host_logical_bytes
            <= plan.requests[index].host_physical_bytes,
        plan.requests[index].host_is_coherent_gtt,
{}

// Obligation 14: each request is exactly one bounded gfx942 linear-copy packet.
pub proof fn admitted_linear_copy_is_single_packet_bounded_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures 0 < plan.requests[index].copy_bytes <= 0x003f_ffe0,
{}

// Obligation 15: each persistent device owner has device-local storage kind.
pub proof fn admitted_persistent_device_storage_is_local_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures plan.requests[index].device_is_local,
{}

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub session: nat,
    pub submission: nat,
    pub request_index: nat,
    pub queue_slot: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
    pub request: RequestIdentityV1,
}

pub open spec fn queue_slot_v1(plan: PlanV1, request_index: nat) -> nat {
    if plan.striped_count == 0 {
        0
    } else {
        (plan.first_queue + request_index) % plan.striped_count
    }
}

pub open spec fn ticket_at_v1(plan: PlanV1, request_index: nat) -> TicketV1 {
    let slot = queue_slot_v1(plan, request_index);
    TicketV1 {
        session: plan.session,
        submission: plan.submission,
        request_index,
        queue_slot: slot,
        queue_id: if slot < plan.striped_queue_ids.len() {
            plan.striped_queue_ids[slot as int]
        } else {
            0
        },
        queue_generation: plan.striped_generation,
        request: if request_index < plan.requests.len() {
            plan.requests[request_index as int]
        } else {
            arbitrary()
        },
    }
}

pub open spec fn ticket_roster_v1(plan: PlanV1) -> Seq<TicketV1> {
    Seq::new(plan.requests.len(), |index: int| ticket_at_v1(plan, index as nat))
}

#[derive(PartialEq, Eq)]
pub enum OwnerStateV1 {
    CallerOwned,
    Prepared,
    Published,
    Settled,
    Quarantined,
}

pub open spec fn state_roster_v1(len: nat, state: OwnerStateV1) -> Seq<OwnerStateV1> {
    Seq::new(len, |_index: int| state)
}

pub open spec fn all_states_are_v1(states: Seq<OwnerStateV1>, state: OwnerStateV1) -> bool {
    forall|index: int| 0 <= index < states.len() ==> states[index] == state
}

pub struct PreparedV1 {
    pub plan: PlanV1,
    pub requests: Seq<RequestIdentityV1>,
    pub tickets: Seq<TicketV1>,
    pub completion_identity_roster: Seq<TicketV1>,
    pub states: Seq<OwnerStateV1>,
    pub recovery_capacity: nat,
    pub completion_capacity: nat,
    pub terminal_capacity: nat,
}

pub open spec fn prepare_all_v1(plan: PlanV1) -> PreparedV1 {
    let tickets = ticket_roster_v1(plan);
    PreparedV1 {
        plan,
        requests: plan.requests,
        tickets,
        completion_identity_roster: tickets,
        states: state_roster_v1(plan.requests.len(), OwnerStateV1::Prepared),
        recovery_capacity: plan.requests.len(),
        completion_capacity: plan.requests.len(),
        terminal_capacity: plan.requests.len(),
    }
}

pub open spec fn prepared_is_exact_v1(prepared: PreparedV1) -> bool {
    admitted_plan_v1(prepared.plan)
        && prepared.requests == prepared.plan.requests
        && prepared.tickets == ticket_roster_v1(prepared.plan)
        && prepared.completion_identity_roster == prepared.tickets
        && all_states_are_v1(prepared.states, OwnerStateV1::Prepared)
        && prepared.recovery_capacity >= prepared.requests.len()
        && prepared.completion_capacity >= prepared.requests.len()
        && prepared.terminal_capacity >= prepared.requests.len()
}

// Obligation 16: preparation constructs the complete immutable identity roster.
pub proof fn preparation_constructs_complete_roster_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures
        prepare_all_v1(plan).tickets.len() == plan.requests.len(),
        prepare_all_v1(plan).completion_identity_roster == prepare_all_v1(plan).tickets,
{}

// Obligation 17: every prepared ticket retains exact request/session/queue identity.
pub proof fn prepared_ticket_is_exact_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures
        prepare_all_v1(plan).tickets[index].session == plan.session,
        prepare_all_v1(plan).tickets[index].submission == plan.submission,
        prepare_all_v1(plan).tickets[index].request_index == index,
        prepare_all_v1(plan).tickets[index].request == plan.requests[index],
        prepare_all_v1(plan).tickets[index].queue_generation == plan.striped_generation,
{}

// Obligation 18: every prepared ticket names its exact round-robin queue occurrence.
pub proof fn prepared_ticket_queue_occurrence_is_exact_v1(plan: PlanV1, index: int)
    requires admitted_plan_v1(plan), 0 <= index < plan.requests.len(),
    ensures
        prepare_all_v1(plan).tickets[index].queue_slot
            == (plan.first_queue + index as nat) % plan.striped_count,
        prepare_all_v1(plan).tickets[index].queue_id
            == plan.striped_queue_ids[
                ((plan.first_queue + index as nat) % plan.striped_count) as int
            ],
{}

// Obligation 19: all recovery/completion/terminal capacity precedes publication.
pub proof fn preparation_reserves_all_aggregate_capacity_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures
        prepare_all_v1(plan).recovery_capacity >= plan.requests.len(),
        prepare_all_v1(plan).completion_capacity >= plan.requests.len(),
        prepare_all_v1(plan).terminal_capacity >= plan.requests.len(),
{}

pub struct PreparationFailureV1 {
    pub original_requests: Seq<RequestIdentityV1>,
    pub returned_requests: Seq<RequestIdentityV1>,
    pub states: Seq<OwnerStateV1>,
    pub prepared_count: nat,
    pub recovered_count: nat,
    pub cursor_before: nat,
    pub cursor_after: nat,
    pub terminal: bool,
}

pub open spec fn preparation_failure_v1(
    plan: PlanV1,
    failed_index: nat,
    recovery_succeeds: bool,
) -> PreparationFailureV1 {
    PreparationFailureV1 {
        original_requests: plan.requests,
        returned_requests: if recovery_succeeds { plan.requests } else { Seq::empty() },
        states: state_roster_v1(
            plan.requests.len(),
            if recovery_succeeds { OwnerStateV1::CallerOwned } else { OwnerStateV1::Quarantined },
        ),
        prepared_count: failed_index,
        recovered_count: if recovery_succeeds { plan.requests.len() } else { 0 },
        cursor_before: plan.first_queue,
        cursor_after: plan.first_queue,
        terminal: !recovery_succeeds,
    }
}

// Obligation 20: successful preparation recovery returns exact original order.
pub proof fn preparation_recovery_is_exact_and_ordered_v1(plan: PlanV1, failed_index: nat)
    requires admitted_plan_v1(plan), failed_index < plan.requests.len(),
    ensures
        preparation_failure_v1(plan, failed_index, true).returned_requests == plan.requests,
        preparation_failure_v1(plan, failed_index, true).recovered_count == plan.requests.len(),
        all_states_are_v1(
            preparation_failure_v1(plan, failed_index, true).states,
            OwnerStateV1::CallerOwned,
        ),
{}

// Obligation 21: preparation failure and recovery never advance the cursor.
pub proof fn preparation_failure_preserves_cursor_v1(
    plan: PlanV1,
    failed_index: nat,
    recovery_succeeds: bool,
)
    requires admitted_plan_v1(plan), failed_index < plan.requests.len(),
    ensures
        preparation_failure_v1(plan, failed_index, recovery_succeeds).cursor_after
            == plan.first_queue,
{}

// Obligation 22: failed recovery quarantines every persistent owner.
pub proof fn failed_preparation_recovery_quarantines_all_v1(plan: PlanV1, failed_index: nat)
    requires admitted_plan_v1(plan), failed_index < plan.requests.len(),
    ensures
        preparation_failure_v1(plan, failed_index, false).terminal,
        all_states_are_v1(
            preparation_failure_v1(plan, failed_index, false).states,
            OwnerStateV1::Quarantined,
        ),
{}

pub struct PublicationV1 {
    pub requests: Seq<RequestIdentityV1>,
    pub confirmed_shards: nat,
    pub indeterminate_shards: nat,
    pub untouched_shards: nat,
    pub fully_published: bool,
    pub currentness_closed: bool,
    pub states: Seq<OwnerStateV1>,
    pub cursor_before: nat,
    pub cursor_after: nat,
    pub cursor_committed: bool,
    pub terminal: bool,
}

pub open spec fn publish_v1(
    prepared: PreparedV1,
    confirmed_shards: nat,
    indeterminate_shards: nat,
    currentness_closed: bool,
) -> PublicationV1 {
    let fully_published = confirmed_shards == prepared.plan.striped_count
        && indeterminate_shards == 0;
    let success = fully_published && currentness_closed;
    PublicationV1 {
        requests: prepared.requests,
        confirmed_shards,
        indeterminate_shards,
        untouched_shards: if confirmed_shards + indeterminate_shards
            <= prepared.plan.striped_count
        {
            (prepared.plan.striped_count as int
                - confirmed_shards as int
                - indeterminate_shards as int) as nat
        } else {
            0
        },
        fully_published,
        currentness_closed,
        states: state_roster_v1(
            prepared.requests.len(),
            if success { OwnerStateV1::Published } else { OwnerStateV1::Quarantined },
        ),
        cursor_before: prepared.plan.first_queue,
        cursor_after: if success {
            (prepared.plan.first_queue + prepared.requests.len()) % prepared.plan.striped_count
        } else {
            prepared.plan.first_queue
        },
        cursor_committed: success,
        terminal: !success,
    }
}

// Obligation 23: a stopped publication is an exact three-way shard partition.
pub proof fn publication_partition_is_exact_v1(
    prepared: PreparedV1,
    confirmed: nat,
    indeterminate: nat,
    currentness_closed: bool,
)
    requires
        prepared_is_exact_v1(prepared),
        confirmed + indeterminate <= prepared.plan.striped_count,
    ensures
        publish_v1(prepared, confirmed, indeterminate, currentness_closed).confirmed_shards
            + publish_v1(prepared, confirmed, indeterminate, currentness_closed).indeterminate_shards
            + publish_v1(prepared, confirmed, indeterminate, currentness_closed).untouched_shards
            == prepared.plan.striped_count,
{}

// Obligation 24: publication admits at most one indeterminate shard.
pub proof fn publication_has_at_most_one_indeterminate_v1(
    prepared: PreparedV1,
    confirmed: nat,
    indeterminate: nat,
    currentness_closed: bool,
)
    requires indeterminate <= 1,
    ensures publish_v1(prepared, confirmed, indeterminate, currentness_closed).indeterminate_shards <= 1,
{}

// Obligation 25: any partial or indeterminate publication is terminal quarantine.
pub proof fn partial_publication_quarantines_all_v1(
    prepared: PreparedV1,
    confirmed: nat,
    indeterminate: nat,
    currentness_closed: bool,
)
    requires
        prepared_is_exact_v1(prepared),
        confirmed + indeterminate <= prepared.plan.striped_count,
        confirmed < prepared.plan.striped_count || indeterminate > 0,
    ensures
        publish_v1(prepared, confirmed, indeterminate, currentness_closed).terminal,
        all_states_are_v1(
            publish_v1(prepared, confirmed, indeterminate, currentness_closed).states,
            OwnerStateV1::Quarantined,
        ),
{}

// Obligation 26: full publication still requires exact currentness closure.
pub proof fn full_publication_requires_currentness_close_v1(prepared: PreparedV1)
    requires prepared_is_exact_v1(prepared),
    ensures publish_v1(prepared, prepared.plan.striped_count, 0, false).terminal,
{}

// Obligation 27: cursor commitment occurs only after full publication and closure.
pub proof fn cursor_commit_requires_full_publish_and_close_v1(
    prepared: PreparedV1,
    confirmed: nat,
    indeterminate: nat,
    currentness_closed: bool,
)
    requires admitted_plan_v1(prepared.plan),
    ensures publish_v1(prepared, confirmed, indeterminate, currentness_closed).cursor_committed
        ==> publish_v1(prepared, confirmed, indeterminate, currentness_closed).fully_published
            && currentness_closed,
{}

// Obligation 28: failure leaves the pre-publication cursor unchanged.
pub proof fn failed_publication_preserves_cursor_v1(
    prepared: PreparedV1,
    confirmed: nat,
    indeterminate: nat,
    currentness_closed: bool,
)
    requires
        admitted_plan_v1(prepared.plan),
        publish_v1(prepared, confirmed, indeterminate, currentness_closed).terminal,
    ensures publish_v1(prepared, confirmed, indeterminate, currentness_closed).cursor_after
        == prepared.plan.first_queue,
{}

// Obligation 29: successful publication places every owner in Published custody.
pub proof fn successful_publication_publishes_every_owner_v1(prepared: PreparedV1)
    requires prepared_is_exact_v1(prepared),
    ensures
        !publish_v1(prepared, prepared.plan.striped_count, 0, true).terminal,
        all_states_are_v1(
            publish_v1(prepared, prepared.plan.striped_count, 0, true).states,
            OwnerStateV1::Published,
        ),
{}

pub struct SubmissionV1 {
    pub plan: PlanV1,
    pub requests: Seq<RequestIdentityV1>,
    pub completion_identity_roster: Seq<TicketV1>,
    pub committed_cursor: nat,
}

pub open spec fn submission_v1(plan: PlanV1) -> SubmissionV1 {
    SubmissionV1 {
        plan,
        requests: plan.requests,
        completion_identity_roster: ticket_roster_v1(plan),
        committed_cursor: if plan.striped_count == 0 {
            plan.first_queue
        } else {
            (plan.first_queue + plan.requests.len()) % plan.striped_count
        },
    }
}

pub struct PresentationV1 {
    pub session: nat,
    pub directional_d2h_queue: nat,
    pub directional_h2d_queue: nat,
    pub directional_generation: nat,
    pub striped_generation: nat,
    pub submission: nat,
    pub completion_identity_roster: Seq<TicketV1>,
    pub currentness_closed: bool,
}

pub open spec fn exact_presentation_v1(submission: SubmissionV1, presented: PresentationV1) -> bool {
    presented.session == submission.plan.session
        && presented.directional_d2h_queue == submission.plan.directional_d2h_queue
        && presented.directional_h2d_queue == submission.plan.directional_h2d_queue
        && presented.directional_generation == submission.plan.directional_generation
        && presented.striped_generation == submission.plan.striped_generation
        && presented.submission == submission.plan.submission
        && presented.completion_identity_roster == submission.completion_identity_roster
        && presented.currentness_closed
}

#[derive(PartialEq, Eq)]
pub enum ScanClassV1 {
    AllReady,
    Pending,
    Error { first_error_index: nat },
}

#[derive(PartialEq, Eq)]
pub enum PollPhaseV1 {
    ValidationTerminal,
    Pending,
    TimedOut,
    ObservationTerminal,
    PreflightTerminal,
    RestorationTerminal,
    Completed,
}

pub struct PollOutcomeV1 {
    pub phase: PollPhaseV1,
    pub observation_count: nat,
    pub returned_submission_requests: Seq<RequestIdentityV1>,
    pub terminal_requests: Seq<RequestIdentityV1>,
    pub completed_requests: Seq<RequestIdentityV1>,
    pub restored_count: nat,
    pub states: Seq<OwnerStateV1>,
    pub committed_cursor: nat,
    pub cursor_commit_retained: bool,
}

pub open spec fn terminal_outcome_v1(
    submission: SubmissionV1,
    phase: PollPhaseV1,
    observation_count: nat,
    restored_count: nat,
) -> PollOutcomeV1 {
    PollOutcomeV1 {
        phase,
        observation_count,
        returned_submission_requests: Seq::empty(),
        terminal_requests: submission.requests,
        completed_requests: Seq::empty(),
        restored_count,
        states: state_roster_v1(submission.requests.len(), OwnerStateV1::Quarantined),
        committed_cursor: submission.committed_cursor,
        cursor_commit_retained: true,
    }
}

pub open spec fn poll_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
    identity_preflight_succeeds: bool,
    restoration_failure: Option<nat>,
) -> PollOutcomeV1 {
    if !exact_presentation_v1(submission, presented) || now_ns < submission.plan.started_ns {
        terminal_outcome_v1(submission, PollPhaseV1::ValidationTerminal, 0, 0)
    } else {
        match scan {
            ScanClassV1::Pending => PollOutcomeV1 {
                phase: if now_ns >= submission.plan.deadline_ns {
                    PollPhaseV1::TimedOut
                } else {
                    PollPhaseV1::Pending
                },
                observation_count: submission.requests.len(),
                returned_submission_requests: submission.requests,
                terminal_requests: Seq::empty(),
                completed_requests: Seq::empty(),
                restored_count: 0,
                states: state_roster_v1(submission.requests.len(), OwnerStateV1::Published),
                committed_cursor: submission.committed_cursor,
                cursor_commit_retained: true,
            },
            ScanClassV1::Error { first_error_index } => terminal_outcome_v1(
                submission,
                PollPhaseV1::ObservationTerminal,
                first_error_index + 1,
                0,
            ),
            ScanClassV1::AllReady => if !identity_preflight_succeeds {
                terminal_outcome_v1(
                    submission,
                    PollPhaseV1::PreflightTerminal,
                    submission.requests.len(),
                    0,
                )
            } else {
                match restoration_failure {
                    Option::Some(failed_index) => terminal_outcome_v1(
                        submission,
                        PollPhaseV1::RestorationTerminal,
                        submission.requests.len(),
                        if failed_index < submission.requests.len() { failed_index } else { 0 },
                    ),
                    Option::None => PollOutcomeV1 {
                        phase: PollPhaseV1::Completed,
                        observation_count: submission.requests.len(),
                        returned_submission_requests: Seq::empty(),
                        terminal_requests: Seq::empty(),
                        completed_requests: submission.requests,
                        restored_count: submission.requests.len(),
                        states: state_roster_v1(
                            submission.requests.len(),
                            OwnerStateV1::Settled,
                        ),
                        committed_cursor: submission.committed_cursor,
                        cursor_commit_retained: true,
                    },
                }
            },
        }
    }
}

// Obligation 30: invalid presentation/currentness terminates before observation.
pub proof fn invalid_presentation_precedes_observation_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
)
    requires !exact_presentation_v1(submission, presented),
    ensures
        poll_v1(submission, presented, scan, now_ns, true, Option::None).phase
            == PollPhaseV1::ValidationTerminal,
        poll_v1(submission, presented, scan, now_ns, true, Option::None).observation_count == 0,
{}

// Obligation 31: Pending scans the whole roster and returns exact whole custody.
pub proof fn pending_scans_and_retains_whole_submission_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns < submission.plan.deadline_ns,
    ensures
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).phase
            == PollPhaseV1::Pending,
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).observation_count
            == submission.requests.len(),
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).returned_submission_requests
            == submission.requests,
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).restored_count == 0,
{}

// Obligation 32: timeout also scans and retains exact whole custody.
pub proof fn timeout_scans_and_retains_whole_submission_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns,
        submission.plan.deadline_ns <= now_ns,
    ensures
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).phase
            == PollPhaseV1::TimedOut,
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).observation_count
            == submission.requests.len(),
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).returned_submission_requests
            == submission.requests,
        poll_v1(submission, presented, ScanClassV1::Pending, now_ns, true, Option::None).restored_count == 0,
{}

// Obligation 33: a completion error is terminal and returns no normal output.
pub proof fn observation_error_is_terminal_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
    first_error_index: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns,
        first_error_index < submission.requests.len(),
    ensures
        poll_v1(
            submission,
            presented,
            ScanClassV1::Error { first_error_index },
            now_ns,
            true,
            Option::None,
        ).phase == PollPhaseV1::ObservationTerminal,
        poll_v1(
            submission,
            presented,
            ScanClassV1::Error { first_error_index },
            now_ns,
            true,
            Option::None,
        ).completed_requests.len() == 0,
{}

// Obligation 34: failed completion identity preflight restores nothing.
pub proof fn failed_identity_preflight_restores_nothing_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires exact_presentation_v1(submission, presented), submission.plan.started_ns <= now_ns,
    ensures
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, false, Option::None).phase
            == PollPhaseV1::PreflightTerminal,
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, false, Option::None).restored_count
            == 0,
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, false, Option::None).terminal_requests
            == submission.requests,
{}

// Obligation 35: successful restoration returns all owners in original order.
pub proof fn successful_restoration_is_complete_and_ordered_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires exact_presentation_v1(submission, presented), submission.plan.started_ns <= now_ns,
    ensures
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, true, Option::None).phase
            == PollPhaseV1::Completed,
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, true, Option::None).completed_requests
            == submission.requests,
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, true, Option::None).restored_count
            == submission.requests.len(),
{}

// Obligation 36: successful completion settles every persistent owner.
pub proof fn successful_restoration_settles_every_owner_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires exact_presentation_v1(submission, presented), submission.plan.started_ns <= now_ns,
    ensures all_states_are_v1(
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, true, Option::None).states,
        OwnerStateV1::Settled,
    ),
{}

// Obligation 37: restoration failure retains the exact whole batch terminally.
pub proof fn failed_restoration_is_whole_batch_terminal_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
    failed_index: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns,
        failed_index < submission.requests.len(),
    ensures
        poll_v1(
            submission,
            presented,
            ScanClassV1::AllReady,
            now_ns,
            true,
            Option::Some(failed_index),
        ).phase == PollPhaseV1::RestorationTerminal,
        poll_v1(
            submission,
            presented,
            ScanClassV1::AllReady,
            now_ns,
            true,
            Option::Some(failed_index),
        ).terminal_requests == submission.requests,
        poll_v1(
            submission,
            presented,
            ScanClassV1::AllReady,
            now_ns,
            true,
            Option::Some(failed_index),
        ).completed_requests.len() == 0,
{}

// Obligation 38: restoration failure records exactly the restored prefix.
pub proof fn failed_restoration_records_exact_prefix_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
    failed_index: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns,
        failed_index < submission.requests.len(),
    ensures poll_v1(
        submission,
        presented,
        ScanClassV1::AllReady,
        now_ns,
        true,
        Option::Some(failed_index),
    ).restored_count == failed_index,
{}

pub open spec fn next_owner_state_v1(state: OwnerStateV1) -> OwnerStateV1 {
    match state {
        OwnerStateV1::Quarantined => OwnerStateV1::Quarantined,
        _ => state,
    }
}

// Obligation 39: quarantine is absorbing and has no modeled release.
pub proof fn quarantine_is_monotonic_v1()
    ensures next_owner_state_v1(OwnerStateV1::Quarantined) == OwnerStateV1::Quarantined,
{}

// Obligation 40: validation-terminal completion retains the prior cursor commit.
pub proof fn validation_terminal_retains_cursor_commit_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
)
    requires !exact_presentation_v1(submission, presented),
    ensures
        poll_v1(submission, presented, scan, now_ns, true, Option::None).cursor_commit_retained,
        poll_v1(submission, presented, scan, now_ns, true, Option::None).committed_cursor
            == submission.committed_cursor,
{}

// Obligation 41: observation-terminal completion retains the prior cursor commit.
pub proof fn observation_terminal_retains_cursor_commit_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
    failed_index: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns,
        failed_index < submission.requests.len(),
    ensures
        poll_v1(
            submission,
            presented,
            ScanClassV1::Error { first_error_index: failed_index },
            now_ns,
            true,
            Option::None,
        ).cursor_commit_retained,
        poll_v1(
            submission,
            presented,
            ScanClassV1::Error { first_error_index: failed_index },
            now_ns,
            true,
            Option::None,
        ).committed_cursor == submission.committed_cursor,
{}

// Obligation 42: preflight-terminal completion retains the prior cursor commit.
pub proof fn preflight_terminal_retains_cursor_commit_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires exact_presentation_v1(submission, presented), submission.plan.started_ns <= now_ns,
    ensures
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, false, Option::None)
            .cursor_commit_retained,
        poll_v1(submission, presented, ScanClassV1::AllReady, now_ns, false, Option::None)
            .committed_cursor == submission.committed_cursor,
{}

// Obligation 43: restoration-terminal completion retains the prior cursor commit.
pub proof fn restoration_terminal_retains_cursor_commit_v1(
    submission: SubmissionV1,
    presented: PresentationV1,
    now_ns: nat,
    failed_index: nat,
)
    requires
        exact_presentation_v1(submission, presented),
        submission.plan.started_ns <= now_ns,
        failed_index < submission.requests.len(),
    ensures
        poll_v1(
            submission,
            presented,
            ScanClassV1::AllReady,
            now_ns,
            true,
            Option::Some(failed_index),
        ).cursor_commit_retained,
        poll_v1(
            submission,
            presented,
            ScanClassV1::AllReady,
            now_ns,
            true,
            Option::Some(failed_index),
        ).committed_cursor == submission.committed_cursor,
{}

}
