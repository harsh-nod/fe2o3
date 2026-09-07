// Independent finite R56 summary model for multiplexing logical SDMA lanes
// over two native queue occurrences. All identities, currentness, publication,
// partial/indeterminate shard custody, timeout, and completion facts are
// mathematical inputs. Post-effect panic is excluded from typed recovery and
// requires a concrete fail-stop or separately proved custody boundary. This proves no
// executable-Rust refinement and no allocator, packet encoder, atomic, KFD,
// HSA, HIP, firmware, hardware, concurrency, progress, parity, or performance
// claim.

use vstd::prelude::*;

verus! {

pub open spec fn native_queue_count_v1() -> nat { 2 }
pub open spec fn max_requests_per_native_v1() -> nat { 63 }
pub open spec fn max_requests_v1() -> nat { 126 }

pub open spec fn valid_lane_count_v1(lanes: nat) -> bool {
    lanes == 2 || lanes == 4 || lanes == 8 || lanes == 14 || lanes == 16
}

#[derive(PartialEq, Eq)]
pub enum PresentationLabelV1 {
    LogicalLanesOverTwoNativeQueues,
    FalsePhysicalStriped2,
}

#[derive(PartialEq, Eq)]
pub struct NativeQueueV1 {
    pub session: nat,
    pub native: nat,
    pub engine: nat,
    pub queue: nat,
    pub generation: nat,
}

pub struct PlanV1 {
    pub label: PresentationLabelV1,
    pub owner: nat,
    pub session: nat,
    pub submission: nat,
    pub lanes: nat,
    pub cursor: nat,
    pub queues: Seq<NativeQueueV1>,
}

pub open spec fn exact_queue_v1(plan: PlanV1, native: nat) -> bool {
    native < 2
        && plan.queues[native as int].session == plan.session
        && plan.queues[native as int].native == native
        && plan.queues[native as int].engine == native
        && plan.queues[native as int].generation > 0
}

pub open spec fn admitted_plan_v1(plan: PlanV1) -> bool {
    plan.label == PresentationLabelV1::LogicalLanesOverTwoNativeQueues
        && plan.owner > 0
        && plan.session > 0
        && plan.submission > 0
        && valid_lane_count_v1(plan.lanes)
        && plan.cursor < plan.lanes
        && plan.queues.len() == 2
        && exact_queue_v1(plan, 0)
        && exact_queue_v1(plan, 1)
        && plan.queues[0].queue != plan.queues[1].queue
}

pub open spec fn lane_for_v1(plan: PlanV1, request_index: nat) -> nat {
    (plan.cursor + request_index) % plan.lanes
}

pub open spec fn native_for_v1(plan: PlanV1, request_index: nat) -> nat {
    lane_for_v1(plan, request_index) % 2
}

pub open spec fn slot_for_v1(request_index: nat) -> nat {
    request_index / 2
}

pub open spec fn native_load_v1(plan: PlanV1, request_count: nat, native: nat) -> nat {
    request_count / 2
        + if request_count % 2 == 1 && plan.cursor % 2 == native { 1nat } else { 0nat }
}

pub open spec fn native_request_index_at_v1(
    plan: PlanV1,
    native: nat,
    slot: nat,
) -> nat {
    if native == plan.cursor % 2 { slot * 2 } else { slot * 2 + 1 }
}

pub open spec fn native_request_indices_v1(
    plan: PlanV1,
    request_count: nat,
    native: nat,
) -> Seq<nat> {
    Seq::new(native_load_v1(plan, request_count, native), |slot: int|
        native_request_index_at_v1(plan, native, slot as nat))
}

pub open spec fn native_ticket_roster_v1(
    plan: PlanV1,
    tickets: Seq<TicketV1>,
    native: nat,
) -> Seq<TicketV1> {
    Seq::new(native_load_v1(plan, tickets.len(), native), |slot: int|
        tickets[native_request_index_at_v1(plan, native, slot as nat) as int])
}

pub open spec fn active_native_count_v1(request_count: nat) -> nat {
    if request_count < 2 { request_count } else { 2 }
}

#[derive(PartialEq, Eq)]
pub struct RequestV1 {
    pub owner: nat,
    pub session: nat,
    pub submission: nat,
    pub request_token: nat,
    pub payload: nat,
}

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub owner: nat,
    pub session: nat,
    pub submission: nat,
    pub request_index: nat,
    pub request_token: nat,
    pub lane: nat,
    pub native: nat,
    pub engine: nat,
    pub queue: nat,
    pub slot: nat,
    pub generation: nat,
    pub packet: nat,
}

pub open spec fn request_matches_plan_v1(plan: PlanV1, request: RequestV1) -> bool {
    request.owner == plan.owner
        && request.session == plan.session
        && request.submission == plan.submission
        && request.request_token > 0
        && request.payload > 0
}

pub open spec fn ticket_matches_v1(
    plan: PlanV1,
    request: RequestV1,
    ticket: TicketV1,
    index: nat,
) -> bool {
    ticket.owner == request.owner
        && ticket.session == request.session
        && ticket.submission == request.submission
        && ticket.request_index == index
        && ticket.request_token == request.request_token
        && ticket.lane == lane_for_v1(plan, index)
        && ticket.native == native_for_v1(plan, index)
        && ticket.engine == ticket.native
        && ticket.queue == plan.queues[ticket.native as int].queue
        && ticket.slot == slot_for_v1(index)
        && ticket.generation == plan.queues[ticket.native as int].generation
        && ticket.packet == plan.submission * 1000 + index + 1
}

pub open spec fn distinct_request_identities_v1(requests: Seq<RequestV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < requests.len() && 0 <= right < requests.len() && left != right
            ==> requests[left].request_token != requests[right].request_token
                && requests[left].payload != requests[right].payload
}

pub open spec fn distinct_packet_identities_v1(tickets: Seq<TicketV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < tickets.len() && 0 <= right < tickets.len() && left != right
            ==> tickets[left].packet != tickets[right].packet
}

pub open spec fn stable_native_filter_v1(tickets: Seq<TicketV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < right < tickets.len() && tickets[left].native == tickets[right].native
            ==> tickets[left].request_index < tickets[right].request_index
                && tickets[left].slot < tickets[right].slot
}

pub open spec fn per_lane_fifo_v1(tickets: Seq<TicketV1>) -> bool {
    forall|left: int, right: int|
        0 <= left < right < tickets.len() && tickets[left].lane == tickets[right].lane
            ==> tickets[left].request_index < tickets[right].request_index
}

pub open spec fn exact_presentation_v1(
    plan: PlanV1,
    requests: Seq<RequestV1>,
    tickets: Seq<TicketV1>,
) -> bool {
    admitted_plan_v1(plan)
        && 2 <= requests.len() <= max_requests_v1()
        && tickets.len() == requests.len()
        && forall|index: int| 0 <= index < requests.len() ==>
            request_matches_plan_v1(plan, requests[index])
                && ticket_matches_v1(plan, requests[index], tickets[index], index as nat)
        && distinct_request_identities_v1(requests)
        && distinct_packet_identities_v1(tickets)
        && stable_native_filter_v1(tickets)
        && per_lane_fifo_v1(tickets)
}

// Obligation 1: the native and request-capacity constants are exact.
pub proof fn constants_are_exact_v1()
    ensures native_queue_count_v1() == 2,
        max_requests_per_native_v1() == 63,
        max_requests_v1() == 126,
{}

// Obligation 2: exactly the five designed logical-lane counts are admitted.
pub proof fn designed_lane_counts_are_admitted_v1()
    ensures valid_lane_count_v1(2), valid_lane_count_v1(4), valid_lane_count_v1(8),
        valid_lane_count_v1(14), valid_lane_count_v1(16),
{}

// Obligation 3: every other logical-lane count is rejected.
pub proof fn other_lane_counts_are_rejected_v1(lanes: nat)
    requires lanes != 2, lanes != 4, lanes != 8, lanes != 14, lanes != 16,
    ensures !valid_lane_count_v1(lanes),
{}

// Obligation 4: every admitted request count is between 2 and 126.
pub proof fn admitted_request_count_is_bounded_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures 2 <= requests.len() <= 126,
{}

// Obligation 5: 127 requests cannot form an exact presentation.
pub proof fn request_127_is_rejected_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires requests.len() == 127,
    ensures !exact_presentation_v1(plan, requests, tickets),
{}

// Obligation 6: each native queue has at most 63 requests.
pub proof fn native_load_is_bounded_v1(plan: PlanV1, request_count: nat, native: nat)
    requires admitted_plan_v1(plan), 0 < request_count <= 126, native < 2,
    ensures native_load_v1(plan, request_count, native) <= 63,
{
    assert(request_count / 2 <= 63);
}

// Obligation 7: every canonical logical lane is within the admitted lane set.
pub proof fn canonical_lane_is_bounded_v1(plan: PlanV1, index: nat)
    requires admitted_plan_v1(plan), index < 126,
    ensures lane_for_v1(plan, index) < plan.lanes,
{}

// Obligation 8: every canonical native ordinal is zero or one.
pub proof fn canonical_native_is_bounded_v1(plan: PlanV1, index: nat)
    requires admitted_plan_v1(plan), index < 126,
    ensures native_for_v1(plan, index) < 2,
{}

// Obligation 9: exact tickets preserve every request and queue coordinate.
pub proof fn exact_ticket_preserves_all_coordinates_v1(
    plan: PlanV1, request: RequestV1, ticket: TicketV1, index: nat,
)
    requires admitted_plan_v1(plan), request_matches_plan_v1(plan, request),
        ticket_matches_v1(plan, request, ticket, index),
    ensures ticket.owner == request.owner,
        ticket.session == request.session,
        ticket.submission == request.submission,
        ticket.request_index == index,
        ticket.request_token == request.request_token,
        ticket.lane == lane_for_v1(plan, index),
        ticket.native == native_for_v1(plan, index),
        ticket.engine == ticket.native,
        ticket.queue == plan.queues[ticket.native as int].queue,
        ticket.slot == index / 2,
        ticket.generation == plan.queues[ticket.native as int].generation,
        ticket.packet == plan.submission * 1000 + index + 1,
{}

// Obligation 10: the two native occurrences have exact engine bindings.
pub proof fn exact_native_engine_bindings_v1(plan: PlanV1)
    requires admitted_plan_v1(plan),
    ensures plan.queues[0].native == 0, plan.queues[0].engine == 0,
        plan.queues[1].native == 1, plan.queues[1].engine == 1,
        plan.queues[0].queue != plan.queues[1].queue,
{}

// Obligation 11: native issue order is a stable canonical filter.
pub proof fn admitted_native_filter_is_stable_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures stable_native_filter_v1(tickets),
{}

// Obligation 12: every logical lane remains FIFO ordered.
pub proof fn admitted_logical_lanes_are_fifo_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures per_lane_fifo_v1(tickets),
{}

// Obligation 13: exact slots stay within one 63-packet native batch.
pub proof fn exact_ticket_slot_is_bounded_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>, index: int,
)
    requires exact_presentation_v1(plan, requests, tickets), 0 <= index < tickets.len(),
    ensures tickets[index].slot < 63,
{
    assert((index as nat) < 126);
    assert((index as nat) / 2 < 63);
}

// Obligation 14: a physical-striped-two relabel is not an admitted mux plan.
pub proof fn false_striped_two_relabel_is_rejected_v1(plan: PlanV1)
    requires plan.label == PresentationLabelV1::FalsePhysicalStriped2,
    ensures !admitted_plan_v1(plan),
{}

// Obligation 15: exact presentation preserves the entire request roster.
pub proof fn exact_presentation_preserves_roster_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures tickets.len() == requests.len(),
        forall|index: int| 0 <= index < requests.len() ==>
            tickets[index].request_token == requests[index].request_token,
{}

// Obligation 16: a missing ticket rejects the presentation.
pub proof fn missing_ticket_is_rejected_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires tickets.len() < requests.len(),
    ensures !exact_presentation_v1(plan, requests, tickets),
{}

// Obligation 17: duplicate request identity rejects the presentation.
pub proof fn duplicate_request_is_rejected_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>, left: int, right: int,
)
    requires 0 <= left < requests.len(), 0 <= right < requests.len(), left != right,
        requests[left].request_token == requests[right].request_token,
    ensures !exact_presentation_v1(plan, requests, tickets),
{}

// Obligation 18: duplicate packet identity rejects the presentation.
pub proof fn duplicate_packet_is_rejected_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>, left: int, right: int,
)
    requires 0 <= left < tickets.len(), 0 <= right < tickets.len(), left != right,
        tickets[left].packet == tickets[right].packet,
    ensures !exact_presentation_v1(plan, requests, tickets),
{}

#[derive(PartialEq, Eq)]
pub enum PublicationScriptV1 {
    Complete,
    RecoverableFirstNativeNoEffect,
    IndeterminateFirstNative,
    RecoverableSecondNativeAfterConfirmedFirst,
    IndeterminateSecondNativeAfterConfirmedFirst,
    CompleteThenClosingCurrentnessFailure,
}

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Published, Pending, Retained, Quarantined, Released }

#[derive(PartialEq, Eq)]
pub enum RetentionReasonV1 { RecoverableNoNativeEffect, Timeout }

pub open spec fn first_publication_native_v1(plan: PlanV1) -> nat {
    plan.cursor % 2
}

pub open spec fn second_publication_native_v1(plan: PlanV1) -> nat {
    (first_publication_native_v1(plan) + 1) % 2
}

pub struct OutcomeV1 {
    pub phase: PhaseV1,
    pub retention_reason: Option<RetentionReasonV1>,
    pub custody: Seq<RequestV1>,
    pub tickets: Seq<TicketV1>,
    pub released: Seq<RequestV1>,
    pub native_publications: nat,
    pub write_pointer_publications: nat,
    pub doorbells: nat,
    pub tails: nat,
    pub confirmed_first_native: Option<nat>,
    pub indeterminate_native: Option<nat>,
    pub untouched_native: Option<nat>,
    pub indeterminate_request_indices: Seq<nat>,
    pub indeterminate_tickets: Seq<TicketV1>,
    pub untouched_request_indices: Seq<nat>,
    pub untouched_tickets: Seq<TicketV1>,
    pub cursor_before: nat,
    pub cursor_after: nat,
    pub cursor_committed: bool,
}

pub open spec fn failure_outcome_v1(
    phase: PhaseV1,
    retention_reason: Option<RetentionReasonV1>,
    plan: PlanV1,
    requests: Seq<RequestV1>,
    tickets: Seq<TicketV1>,
    published_prefix: nat,
    confirmed_first_native: Option<nat>,
    indeterminate_native: Option<nat>,
    untouched_native: Option<nat>,
    indeterminate_request_indices: Seq<nat>,
    indeterminate_tickets: Seq<TicketV1>,
    untouched_request_indices: Seq<nat>,
    untouched_tickets: Seq<TicketV1>,
) -> OutcomeV1 {
    OutcomeV1 { phase, retention_reason, custody: requests, tickets, released: Seq::empty(),
        native_publications: published_prefix,
        write_pointer_publications: published_prefix,
        doorbells: published_prefix,
        tails: published_prefix,
        confirmed_first_native,
        indeterminate_native,
        untouched_native,
        indeterminate_request_indices,
        indeterminate_tickets,
        untouched_request_indices,
        untouched_tickets,
        cursor_before: plan.cursor, cursor_after: plan.cursor, cursor_committed: false }
}

pub open spec fn publish_v1(
    plan: PlanV1,
    requests: Seq<RequestV1>,
    tickets: Seq<TicketV1>,
    presentation_exact: bool,
    opening_currentness_exact: bool,
    script: PublicationScriptV1,
) -> OutcomeV1 {
    if !presentation_exact || !exact_presentation_v1(plan, requests, tickets) {
        failure_outcome_v1(PhaseV1::Quarantined, Option::None, plan, requests, tickets, 0,
            Option::None, Option::None, Option::None,
            Seq::empty(), Seq::empty(), Seq::empty(), Seq::empty())
    } else if !opening_currentness_exact {
        failure_outcome_v1(PhaseV1::Quarantined, Option::None, plan, requests, tickets, 0,
            Option::None, Option::None, Option::None,
            Seq::empty(), Seq::empty(), Seq::empty(), Seq::empty())
    } else {
        let first = first_publication_native_v1(plan);
        let second = second_publication_native_v1(plan);
        let first_indices = native_request_indices_v1(plan, requests.len(), first);
        let first_tickets = native_ticket_roster_v1(plan, tickets, first);
        let second_indices = native_request_indices_v1(plan, requests.len(), second);
        let second_tickets = native_ticket_roster_v1(plan, tickets, second);
        match script {
            PublicationScriptV1::RecoverableFirstNativeNoEffect =>
                failure_outcome_v1(PhaseV1::Retained,
                    Option::Some(RetentionReasonV1::RecoverableNoNativeEffect), plan, requests,
                    Seq::empty(), 0, Option::None, Option::None, Option::None,
                    Seq::empty(), Seq::empty(), Seq::empty(), Seq::empty()),
            PublicationScriptV1::IndeterminateFirstNative =>
                failure_outcome_v1(PhaseV1::Quarantined, Option::None, plan, requests, tickets, 0,
                    Option::None, Option::Some(first), Option::Some(second),
                    first_indices, first_tickets, second_indices, second_tickets),
            PublicationScriptV1::RecoverableSecondNativeAfterConfirmedFirst =>
                failure_outcome_v1(PhaseV1::Quarantined, Option::None, plan, requests, tickets, 1,
                    Option::Some(first), Option::None, Option::Some(second),
                    Seq::empty(), Seq::empty(), second_indices, second_tickets),
            PublicationScriptV1::IndeterminateSecondNativeAfterConfirmedFirst =>
                failure_outcome_v1(PhaseV1::Quarantined, Option::None, plan, requests, tickets, 1,
                    Option::Some(first), Option::Some(second), Option::None,
                    second_indices, second_tickets, Seq::empty(), Seq::empty()),
            PublicationScriptV1::CompleteThenClosingCurrentnessFailure =>
                failure_outcome_v1(PhaseV1::Quarantined, Option::None, plan, requests, tickets, 2,
                    Option::Some(first), Option::None, Option::None,
                    Seq::empty(), Seq::empty(), Seq::empty(), Seq::empty()),
            PublicationScriptV1::Complete => {
                OutcomeV1 { phase: PhaseV1::Published, retention_reason: Option::None,
                    custody: requests, tickets, released: Seq::empty(), native_publications: 2,
                    write_pointer_publications: 2, doorbells: 2, tails: 2,
                    confirmed_first_native: Option::Some(first),
                    indeterminate_native: Option::None,
                    untouched_native: Option::None,
                    indeterminate_request_indices: Seq::empty(),
                    indeterminate_tickets: Seq::empty(),
                    untouched_request_indices: Seq::empty(),
                    untouched_tickets: Seq::empty(),
                    cursor_before: plan.cursor,
                    cursor_after: (plan.cursor + requests.len()) % plan.lanes,
                    cursor_committed: true }
            },
        }
    }
}

pub open spec fn exact_published_summary_v1(published: OutcomeV1) -> bool {
    published.phase == PhaseV1::Published
        && published.retention_reason.is_none()
        && published.released.len() == 0
        && published.custody.len() == published.tickets.len()
        && 2 <= published.tickets.len() <= 126
        && published.native_publications == 2
        && published.write_pointer_publications == 2
        && published.doorbells == 2
        && published.tails == 2
        && published.confirmed_first_native == Option::Some(published.cursor_before % 2)
        && published.indeterminate_native.is_none()
        && published.untouched_native.is_none()
        && published.indeterminate_request_indices.len() == 0
        && published.indeterminate_tickets.len() == 0
        && published.untouched_request_indices.len() == 0
        && published.untouched_tickets.len() == 0
        && published.cursor_committed
}

pub open spec fn exact_pending_summary_v1(pending: OutcomeV1) -> bool {
    pending.phase == PhaseV1::Pending
        && pending.retention_reason == Option::Some(RetentionReasonV1::Timeout)
        && pending.released.len() == 0
        && pending.custody.len() == pending.tickets.len()
        && 2 <= pending.tickets.len() <= 126
        && pending.native_publications == 2
        && pending.write_pointer_publications == 2
        && pending.doorbells == 2
        && pending.tails == 2
        && pending.confirmed_first_native == Option::Some(pending.cursor_before % 2)
        && pending.indeterminate_native.is_none()
        && pending.untouched_native.is_none()
        && pending.indeterminate_request_indices.len() == 0
        && pending.indeterminate_tickets.len() == 0
        && pending.untouched_request_indices.len() == 0
        && pending.untouched_tickets.len() == 0
        && pending.cursor_committed
}

pub open spec fn wait_timeout_v1(published: OutcomeV1) -> OutcomeV1 {
    if exact_published_summary_v1(published) {
        OutcomeV1 { phase: PhaseV1::Pending,
            retention_reason: Option::Some(RetentionReasonV1::Timeout),
            custody: published.custody, tickets: published.tickets,
            released: Seq::empty(), native_publications: published.native_publications,
            write_pointer_publications: published.write_pointer_publications,
            doorbells: published.doorbells, tails: published.tails,
            confirmed_first_native: published.confirmed_first_native,
            indeterminate_native: published.indeterminate_native,
            untouched_native: published.untouched_native,
            indeterminate_request_indices: published.indeterminate_request_indices,
            indeterminate_tickets: published.indeterminate_tickets,
            untouched_request_indices: published.untouched_request_indices,
            untouched_tickets: published.untouched_tickets,
            cursor_before: published.cursor_before, cursor_after: published.cursor_after,
            cursor_committed: published.cursor_committed }
    } else { published }
}

pub open spec fn release_v1(
    published: OutcomeV1, completion_roster: Seq<TicketV1>,
) -> OutcomeV1 {
    if (exact_published_summary_v1(published) || exact_pending_summary_v1(published))
        && completion_roster == published.tickets {
        OutcomeV1 { phase: PhaseV1::Released, retention_reason: Option::None,
            custody: Seq::empty(), tickets: published.tickets,
            released: published.custody,
            native_publications: published.native_publications,
            write_pointer_publications: published.write_pointer_publications,
            doorbells: published.doorbells, tails: published.tails,
            confirmed_first_native: published.confirmed_first_native,
            indeterminate_native: published.indeterminate_native,
            untouched_native: published.untouched_native,
            indeterminate_request_indices: published.indeterminate_request_indices,
            indeterminate_tickets: published.indeterminate_tickets,
            untouched_request_indices: published.untouched_request_indices,
            untouched_tickets: published.untouched_tickets,
            cursor_before: published.cursor_before, cursor_after: published.cursor_after,
            cursor_committed: published.cursor_committed }
    } else { published }
}

// Obligation 19: two or more requests activate both native queues.
pub proof fn full_batch_activates_two_native_queues_v1(request_count: nat)
    requires 2 <= request_count <= 126,
    ensures active_native_count_v1(request_count) == 2,
{}

// Obligation 20: the first publication follows cursor parity.
pub proof fn first_publication_is_cursor_rotated_v1(plan: PlanV1)
    ensures first_publication_native_v1(plan) == plan.cursor % 2,
        first_publication_native_v1(plan) < 2,
{}

// Obligation 21: the second publication is the other native queue.
pub proof fn second_publication_is_rotated_and_distinct_v1(plan: PlanV1)
    ensures second_publication_native_v1(plan) < 2,
        second_publication_native_v1(plan) != first_publication_native_v1(plan),
{}

// Obligation 22: a full successful batch has exactly two publications,
// write pointers, doorbells, and tails.
pub proof fn full_success_has_exactly_two_native_publications_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets), requests.len() >= 2,
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true, PublicationScriptV1::Complete);
        out.native_publications == 2 && out.write_pointer_publications == 2
            && out.doorbells == 2 && out.tails == 2
    },
{}

// Obligation 23: one request rejects before publication.
pub proof fn singleton_is_rejected_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires requests.len() == 1,
    ensures !exact_presentation_v1(plan, requests, tickets),
{}

// Obligation 24: only complete two-native success commits the rotated cursor.
pub proof fn full_success_commits_exact_cursor_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets), requests.len() >= 2,
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true, PublicationScriptV1::Complete);
        out.phase == PhaseV1::Published && out.cursor_committed
            && out.cursor_after == (plan.cursor + requests.len()) % plan.lanes
            && out.confirmed_first_native == Option::Some(plan.cursor % 2)
    },
{}

// Obligation 25: opening-currentness rejection has prefix zero and never commits.
pub proof fn opening_currentness_rejection_never_commits_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>, script: PublicationScriptV1,
)
    ensures {
        let out = publish_v1(plan, requests, tickets, true, false, script);
        out.phase == PhaseV1::Quarantined && out.custody == requests
            && out.native_publications == 0 && !out.cursor_committed
            && out.cursor_after == plan.cursor
    },
{}

// Obligation 26: recoverable first-native rejection returns only exact requests
// with no ticket or native-effect carrier.
pub proof fn recoverable_first_native_is_exact_and_effect_free_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::RecoverableFirstNativeNoEffect);
        out.phase == PhaseV1::Retained && out.custody == requests
            && out.retention_reason == Option::Some(RetentionReasonV1::RecoverableNoNativeEffect)
            && out.tickets.len() == 0 && out.native_publications == 0
            && out.released.len() == 0 && !out.cursor_committed
    },
{}

// Obligation 27: an indeterminate first native has no confirmed prefix and
// retains the exact first shard plus the exact untouched second shard.
pub proof fn first_indeterminate_has_exact_shard_custody_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let first = plan.cursor % 2;
        let second = (first + 1) % 2;
        let out = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::IndeterminateFirstNative);
        out.phase == PhaseV1::Quarantined && out.custody == requests
            && out.tickets == tickets && out.native_publications == 0
            && out.confirmed_first_native == Option::None
            && out.indeterminate_native == Option::Some(first)
            && out.untouched_native == Option::Some(second)
            && out.indeterminate_request_indices
                == native_request_indices_v1(plan, requests.len(), first)
            && out.indeterminate_tickets == native_ticket_roster_v1(plan, tickets, first)
            && out.untouched_request_indices
                == native_request_indices_v1(plan, requests.len(), second)
            && out.untouched_tickets == native_ticket_roster_v1(plan, tickets, second)
            && !out.cursor_committed && out.cursor_after == plan.cursor
    },
{}

// Obligation 28: a recoverable second native after confirmed first retains the
// first and exact untouched second coordinate without committing.
pub proof fn second_recoverable_has_exact_coordinates_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets), requests.len() >= 2,
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::RecoverableSecondNativeAfterConfirmedFirst);
        out.phase == PhaseV1::Quarantined && out.custody == requests
            && out.native_publications == 1 && !out.cursor_committed
            && out.confirmed_first_native == Option::Some(plan.cursor % 2)
            && out.indeterminate_native == Option::None
            && out.untouched_native == Option::Some((plan.cursor % 2 + 1) % 2)
            && out.indeterminate_request_indices.len() == 0
            && out.indeterminate_tickets.len() == 0
            && out.untouched_request_indices == native_request_indices_v1(
                plan, requests.len(), (plan.cursor % 2 + 1) % 2)
            && out.untouched_tickets == native_ticket_roster_v1(
                plan, tickets, (plan.cursor % 2 + 1) % 2)
    },
{}

// Obligation 29: an indeterminate second native is distinct from the confirmed
// first and has no falsely untouched coordinate.
pub proof fn second_indeterminate_has_exact_coordinates_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::IndeterminateSecondNativeAfterConfirmedFirst);
        out.phase == PhaseV1::Quarantined && out.custody == requests
            && out.native_publications == 1 && !out.cursor_committed
            && out.confirmed_first_native == Option::Some(plan.cursor % 2)
            && out.indeterminate_native == Option::Some((plan.cursor % 2 + 1) % 2)
            && out.untouched_native == Option::None
            && out.indeterminate_request_indices == native_request_indices_v1(
                plan, requests.len(), (plan.cursor % 2 + 1) % 2)
            && out.indeterminate_tickets == native_ticket_roster_v1(
                plan, tickets, (plan.cursor % 2 + 1) % 2)
            && out.untouched_request_indices.len() == 0
            && out.untouched_tickets.len() == 0
    },
{}

// Obligation 30: closing-currentness loss after both publications retains prefix
// two but never commits the cursor.
pub proof fn closing_currentness_failure_has_prefix_two_without_commit_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::CompleteThenClosingCurrentnessFailure);
        out.phase == PhaseV1::Quarantined && out.custody == requests
            && out.tickets == tickets && out.native_publications == 2
            && out.write_pointer_publications == 2 && out.doorbells == 2 && out.tails == 2
            && out.confirmed_first_native == Option::Some(plan.cursor % 2)
            && !out.cursor_committed && out.cursor_after == plan.cursor
    },
{}

// Obligation 31: malformed presentation quarantines without false commit.
pub proof fn malformed_presentation_never_commits_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
    currentness: bool, script: PublicationScriptV1,
)
    requires !exact_presentation_v1(plan, requests, tickets),
    ensures {
        let out = publish_v1(plan, requests, tickets, true, currentness, script);
        out.phase == PhaseV1::Quarantined && out.custody == requests
            && !out.cursor_committed && out.cursor_after == plan.cursor
    },
{}

// Obligation 32: timeout over complete publication retains exact request and
// ticket custody plus both publication effects.
pub proof fn wait_timeout_retains_published_custody_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let published = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::Complete);
        let out = wait_timeout_v1(published);
        out.phase == PhaseV1::Pending
            && out.retention_reason == Option::Some(RetentionReasonV1::Timeout)
            && out.custody == requests && out.tickets == tickets
            && out.native_publications == 2 && out.write_pointer_publications == 2
            && out.doorbells == 2 && out.tails == 2
    },
{}

// Obligation 33: wait timeout preserves the already committed cursor.
pub proof fn wait_timeout_preserves_committed_cursor_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let published = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::Complete);
        let out = wait_timeout_v1(published);
        out.cursor_committed && out.cursor_before == plan.cursor
            && out.cursor_after == (plan.cursor + requests.len()) % plan.lanes
    },
{}

// Obligation 34: exact ticket-roster completion releases published custody.
pub proof fn exact_completion_releases_exact_custody_v1(published: OutcomeV1)
    requires exact_published_summary_v1(published),
    ensures {
        let out = release_v1(published, published.tickets);
        out.phase == PhaseV1::Released && out.custody.len() == 0
            && out.released == published.custody
    },
{}

// Obligation 35: exact completion after timeout releases the same pending custody.
pub proof fn exact_completion_after_timeout_releases_v1(published: OutcomeV1)
    requires exact_published_summary_v1(published),
    ensures {
        let pending = wait_timeout_v1(published);
        let out = release_v1(pending, pending.tickets);
        out.phase == PhaseV1::Released && out.released == published.custody
    },
{}

// Obligation 36: an inexact completion roster retains whole custody.
pub proof fn inexact_completion_retains_whole_custody_v1(
    published: OutcomeV1, completion_roster: Seq<TicketV1>,
)
    requires completion_roster != published.tickets,
    ensures release_v1(published, completion_roster) == published,
{}

// Obligation 37: a nonpublished/nonpending batch cannot be released.
pub proof fn nonpublished_batch_cannot_release_v1(
    published: OutcomeV1, completion_roster: Seq<TicketV1>,
)
    requires published.phase != PhaseV1::Published, published.phase != PhaseV1::Pending,
    ensures release_v1(published, completion_roster) == published,
{}

// Obligation 38: complete publication retains exact request and ticket custody.
pub proof fn publication_retains_exact_custody_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let out = publish_v1(plan, requests, tickets, true, true, PublicationScriptV1::Complete);
        out.custody == requests && out.tickets == tickets
    },
{}

// Obligation 39: admitted packet identities are request-index unique.
pub proof fn admitted_packets_are_distinct_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures distinct_packet_identities_v1(tickets),
{}

// Obligation 40: post-effect panic is not a typed recoverable transition.
pub open spec fn post_effect_panic_has_typed_recovery_v1() -> bool { false }

pub proof fn post_effect_panic_is_excluded_v1()
    ensures !post_effect_panic_has_typed_recovery_v1(),
{}

// Obligation 41: both confirmed-prefix partial scripts record the cursor-rotated
// first native.
pub proof fn partial_prefix_is_cursor_rotated_v1(
    plan: PlanV1, requests: Seq<RequestV1>, tickets: Seq<TicketV1>,
)
    requires exact_presentation_v1(plan, requests, tickets),
    ensures {
        let recoverable = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::RecoverableSecondNativeAfterConfirmedFirst);
        let indeterminate = publish_v1(plan, requests, tickets, true, true,
            PublicationScriptV1::IndeterminateSecondNativeAfterConfirmedFirst);
        recoverable.confirmed_first_native == Option::Some(plan.cursor % 2)
            && indeterminate.confirmed_first_native == Option::Some(plan.cursor % 2)
    },
{}

} // verus!

fn main() {}
