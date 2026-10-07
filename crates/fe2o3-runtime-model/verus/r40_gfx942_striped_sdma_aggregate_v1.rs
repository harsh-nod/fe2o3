// Independent finite R40 model for gfx942 striped-SDMA capacity and atomic
// whole-submission aggregate completion. Queue IDs and all aggregate inputs are
// contracted mathematical values. The preparation counters name abstract
// phases; they do not model a Rust allocator. This proves no Rust-to-Verus or
// production-Rust refinement and no KFD, HSA, HIP, ioctl, packet, atomic,
// clock, driver, firmware, hardware, progress, parity, or performance claim.

use vstd::prelude::*;

verus! {

pub open spec fn engine_count_v1() -> nat { 2 }
pub open spec fn queues_per_engine_v1() -> nat { 8 }
pub open spec fn requests_per_striped_queue_v1() -> nat { 63 }

#[derive(PartialEq, Eq)]
pub enum PlanKindV1 {
    CombinedDirectionalStriped,
    StandaloneStriped,
}

pub struct QueuePlanV1 {
    pub session: nat,
    pub kind: PlanKindV1,
    pub striped_count: nat,
    pub queue_ids: Seq<nat>,
}

pub open spec fn valid_striped_count_v1(kind: PlanKindV1, count: nat) -> bool {
    match kind {
        PlanKindV1::CombinedDirectionalStriped => 2 <= count && count <= 14 && count % 2 == 0,
        PlanKindV1::StandaloneStriped => 2 <= count && count <= 16 && count % 2 == 0,
    }
}

pub open spec fn directional_count_v1(kind: PlanKindV1) -> nat {
    match kind {
        PlanKindV1::CombinedDirectionalStriped => 2,
        PlanKindV1::StandaloneStriped => 0,
    }
}

pub open spec fn queue_count_v1(plan: QueuePlanV1) -> nat {
    directional_count_v1(plan.kind) + plan.striped_count
}

pub open spec fn distinct_queue_ids_v1(ids: Seq<nat>) -> bool {
    forall|left: int, right: int|
        0 <= left < ids.len() && 0 <= right < ids.len() && left != right
            ==> ids[left] != ids[right]
}

pub open spec fn admitted_queue_plan_v1(plan: QueuePlanV1) -> bool {
    valid_striped_count_v1(plan.kind, plan.striped_count)
        && plan.queue_ids.len() == queue_count_v1(plan)
        && distinct_queue_ids_v1(plan.queue_ids)
}

pub open spec fn striped_engine_v1(striped_slot: nat) -> nat {
    striped_slot % engine_count_v1()
}

pub open spec fn striped_queues_per_engine_v1(plan: QueuePlanV1) -> nat {
    plan.striped_count / engine_count_v1()
}

pub open spec fn striped_queues_on_engine_v1(plan: QueuePlanV1, engine: nat) -> nat {
    if engine < engine_count_v1() { striped_queues_per_engine_v1(plan) } else { 0 }
}

pub open spec fn total_queues_per_engine_v1(plan: QueuePlanV1) -> nat {
    striped_queues_per_engine_v1(plan)
        + if plan.kind == PlanKindV1::CombinedDirectionalStriped { 1nat } else { 0nat }
}

pub open spec fn request_capacity_v1(plan: QueuePlanV1) -> nat {
    plan.striped_count * requests_per_striped_queue_v1()
}

// Obligation 1: the gfx942 capacity constants are exact.
pub proof fn gfx942_capacity_constants_are_exact_v1()
    ensures
        engine_count_v1() == 2,
        queues_per_engine_v1() == 8,
        requests_per_striped_queue_v1() == 63,
{}

// Obligation 2: the minimum combined configuration is admitted.
pub proof fn combined_minimum_is_admitted_v1()
    ensures valid_striped_count_v1(PlanKindV1::CombinedDirectionalStriped, 2),
{}

// Obligation 3: the maximum combined configuration is admitted.
pub proof fn combined_maximum_is_admitted_v1()
    ensures valid_striped_count_v1(PlanKindV1::CombinedDirectionalStriped, 14),
{}

// Obligation 4: a combined request for all 16 striped queues is rejected.
pub proof fn combined_sixteen_is_rejected_v1()
    ensures !valid_striped_count_v1(PlanKindV1::CombinedDirectionalStriped, 16),
{}

// Obligation 5: the minimum standalone configuration is admitted.
pub proof fn standalone_minimum_is_admitted_v1()
    ensures valid_striped_count_v1(PlanKindV1::StandaloneStriped, 2),
{}

// Obligation 6: the maximum standalone configuration is admitted.
pub proof fn standalone_maximum_is_admitted_v1()
    ensures valid_striped_count_v1(PlanKindV1::StandaloneStriped, 16),
{}

// Obligation 7: standalone cannot exceed the physical 16-queue inventory.
pub proof fn standalone_eighteen_is_rejected_v1()
    ensures !valid_striped_count_v1(PlanKindV1::StandaloneStriped, 18),
{}

// Obligation 8: combined plans reserve exactly one directional queue per engine.
pub proof fn combined_reserves_one_directional_queue_per_engine_v1(plan: QueuePlanV1)
    requires
        admitted_queue_plan_v1(plan),
        plan.kind == PlanKindV1::CombinedDirectionalStriped,
    ensures
        directional_count_v1(plan.kind) == engine_count_v1(),
        total_queues_per_engine_v1(plan) == striped_queues_per_engine_v1(plan) + 1,
{}

// Obligation 9: admitted combined placement is balanced and respects eight queues per engine.
pub proof fn combined_placement_is_balanced_and_bounded_v1(plan: QueuePlanV1)
    requires
        admitted_queue_plan_v1(plan),
        plan.kind == PlanKindV1::CombinedDirectionalStriped,
    ensures
        striped_engine_v1(0) == 0,
        striped_engine_v1(1) == 1,
        striped_queues_on_engine_v1(plan, 0) == striped_queues_on_engine_v1(plan, 1),
        total_queues_per_engine_v1(plan) <= queues_per_engine_v1(),
{
    assert(plan.striped_count / 2 <= 7);
}

// Obligation 10: admitted standalone placement is balanced and bounded.
pub proof fn standalone_placement_is_balanced_and_bounded_v1(plan: QueuePlanV1)
    requires
        admitted_queue_plan_v1(plan),
        plan.kind == PlanKindV1::StandaloneStriped,
    ensures
        striped_engine_v1(0) == 0,
        striped_engine_v1(1) == 1,
        striped_queues_on_engine_v1(plan, 0) == striped_queues_on_engine_v1(plan, 1),
        total_queues_per_engine_v1(plan) <= queues_per_engine_v1(),
{
    assert(plan.striped_count / 2 <= 8);
}

// Obligation 11: admission retains the exact session-local ID roster and its distinctness.
pub proof fn admitted_plan_retains_exact_distinct_session_roster_v1(plan: QueuePlanV1)
    requires admitted_queue_plan_v1(plan),
    ensures
        plan.queue_ids.len() == directional_count_v1(plan.kind) + plan.striped_count,
        distinct_queue_ids_v1(plan.queue_ids),
{}

// Obligation 12: any duplicate session-local queue ID prevents admission.
pub proof fn duplicate_session_queue_id_is_rejected_v1(plan: QueuePlanV1, left: int, right: int)
    requires
        0 <= left < plan.queue_ids.len(),
        0 <= right < plan.queue_ids.len(),
        left != right,
        plan.queue_ids[left] == plan.queue_ids[right],
    ensures !admitted_queue_plan_v1(plan),
{}

// Obligation 13: maximum combined and standalone request capacities are exact.
pub proof fn maximum_request_capacities_are_exact_v1()
    ensures
        14 * requests_per_striped_queue_v1() == 882,
        16 * requests_per_striped_queue_v1() == 1008,
{}

#[derive(PartialEq, Eq)]
pub struct TicketV1 {
    pub session: nat,
    pub submission: nat,
    pub request_index: nat,
    pub queue_slot: nat,
    pub queue_id: nat,
    pub queue_generation: nat,
}

pub struct AggregateOwnerV1 {
    pub plan_token: nat,
    pub session: nat,
    pub submission: nat,
    pub started_ns: nat,
    pub deadline_ns: nat,
    pub request_count: nat,
    pub tickets: Seq<TicketV1>,
    pub completion_validation_roster: Seq<TicketV1>,
    pub published: bool,
    pub completed_output_len: nat,
    pub completed_output_capacity: nat,
    pub prepublication_allocation_count: nat,
    pub postpublication_allocation_count: nat,
}

pub struct PresentationV1 {
    pub plan_token: nat,
    pub session: nat,
    pub submission: nat,
    pub request_count: nat,
    pub tickets: Seq<TicketV1>,
    pub completion_validation_roster: Seq<TicketV1>,
    pub currentness_closed: bool,
}

pub open spec fn prepare_owner_v1(
    plan_token: nat,
    session: nat,
    submission: nat,
    started_ns: nat,
    deadline_ns: nat,
    tickets: Seq<TicketV1>,
) -> AggregateOwnerV1 {
    AggregateOwnerV1 {
        plan_token,
        session,
        submission,
        started_ns,
        deadline_ns,
        request_count: tickets.len(),
        tickets,
        completion_validation_roster: tickets,
        published: true,
        completed_output_len: 0,
        completed_output_capacity: tickets.len(),
        prepublication_allocation_count: 2,
        postpublication_allocation_count: 0,
    }
}

pub open spec fn owner_is_prepared_v1(owner: AggregateOwnerV1) -> bool {
    owner.published
        && owner.request_count > 0
        && owner.tickets.len() == owner.request_count
        && owner.completion_validation_roster == owner.tickets
        && owner.completed_output_len == 0
        && owner.completed_output_capacity >= owner.request_count
        && owner.prepublication_allocation_count == 2
        && owner.postpublication_allocation_count == 0
}

pub open spec fn exact_presentation_v1(owner: AggregateOwnerV1, presented: PresentationV1) -> bool {
    presented.plan_token == owner.plan_token
        && presented.session == owner.session
        && presented.submission == owner.submission
        && presented.request_count == owner.request_count
        && presented.tickets == owner.tickets
        && presented.completion_validation_roster == owner.completion_validation_roster
        && presented.currentness_closed
}

#[derive(PartialEq, Eq)]
pub enum ScanClassV1 {
    AllReady,
    Pending,
    Error { first_error_index: nat, terminal_token: nat },
}

#[derive(PartialEq, Eq)]
pub enum PollPhaseV1 {
    ValidationTerminal,
    Pending,
    TimedOut,
    ObservationTerminal,
    PreflightTerminal,
    RetakeTerminal,
    Completed,
}

pub struct PollOutcomeV1 {
    pub phase: PollPhaseV1,
    pub observation_count: nat,
    pub retired_count: nat,
    pub returned_owner: Option<AggregateOwnerV1>,
    pub completed_tickets: Seq<TicketV1>,
    pub terminal_retained_tickets: Seq<TicketV1>,
    pub output_is_request_ordered: bool,
    pub terminal_token: Option<nat>,
    pub postpublication_allocation_count: nat,
}

pub open spec fn terminal_before_observation_v1(owner: AggregateOwnerV1) -> PollOutcomeV1 {
    PollOutcomeV1 {
        phase: PollPhaseV1::ValidationTerminal,
        observation_count: 0,
        retired_count: 0,
        returned_owner: Option::Some(owner),
        completed_tickets: Seq::empty(),
        terminal_retained_tickets: Seq::empty(),
        output_is_request_ordered: false,
        terminal_token: Option::None,
        postpublication_allocation_count: owner.postpublication_allocation_count,
    }
}

pub open spec fn poll_with_retake_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
    full_retirement_preflight: bool,
    model_retake_succeeds: bool,
) -> PollOutcomeV1 {
    if !owner_is_prepared_v1(owner) || !exact_presentation_v1(owner, presented) {
        terminal_before_observation_v1(owner)
    } else {
        match scan {
            ScanClassV1::Pending => PollOutcomeV1 {
                phase: if now_ns >= owner.deadline_ns {
                    PollPhaseV1::TimedOut
                } else {
                    PollPhaseV1::Pending
                },
                observation_count: owner.request_count,
                retired_count: 0,
                returned_owner: Option::Some(owner),
                completed_tickets: Seq::empty(),
                terminal_retained_tickets: Seq::empty(),
                output_is_request_ordered: false,
                terminal_token: Option::None,
                postpublication_allocation_count: owner.postpublication_allocation_count,
            },
            ScanClassV1::Error { first_error_index, terminal_token } => PollOutcomeV1 {
                phase: PollPhaseV1::ObservationTerminal,
                observation_count: first_error_index + 1,
                retired_count: 0,
                returned_owner: Option::Some(owner),
                completed_tickets: Seq::empty(),
                terminal_retained_tickets: Seq::empty(),
                output_is_request_ordered: false,
                terminal_token: Option::Some(terminal_token),
                postpublication_allocation_count: owner.postpublication_allocation_count,
            },
            ScanClassV1::AllReady => if !full_retirement_preflight {
                PollOutcomeV1 {
                    phase: PollPhaseV1::PreflightTerminal,
                    observation_count: owner.request_count,
                    retired_count: 0,
                    returned_owner: Option::Some(owner),
                    completed_tickets: Seq::empty(),
                    terminal_retained_tickets: Seq::empty(),
                    output_is_request_ordered: false,
                    terminal_token: Option::None,
                    postpublication_allocation_count: owner.postpublication_allocation_count,
                }
            } else if !model_retake_succeeds {
                PollOutcomeV1 {
                    phase: PollPhaseV1::RetakeTerminal,
                    observation_count: owner.request_count,
                    retired_count: owner.request_count,
                    returned_owner: Option::None,
                    completed_tickets: Seq::empty(),
                    terminal_retained_tickets: owner.tickets,
                    output_is_request_ordered: true,
                    terminal_token: Option::None,
                    postpublication_allocation_count: owner.postpublication_allocation_count,
                }
            } else {
                PollOutcomeV1 {
                    phase: PollPhaseV1::Completed,
                    observation_count: owner.request_count,
                    retired_count: owner.request_count,
                    returned_owner: Option::None,
                    completed_tickets: owner.tickets,
                    terminal_retained_tickets: Seq::empty(),
                    output_is_request_ordered: true,
                    terminal_token: Option::None,
                    postpublication_allocation_count: owner.postpublication_allocation_count,
                }
            },
        }
    }
}

pub open spec fn poll_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
    full_retirement_preflight: bool,
) -> PollOutcomeV1 {
    poll_with_retake_v1(owner, presented, scan, now_ns, full_retirement_preflight, true)
}

// Obligation 14: construction prepares the exact immutable validation roster and empty
// output capacity before publication, with no postpublication allocation.
pub proof fn construction_prepares_exact_roster_before_publication_v1(
    plan_token: nat,
    session: nat,
    submission: nat,
    started_ns: nat,
    deadline_ns: nat,
    tickets: Seq<TicketV1>,
)
    requires tickets.len() > 0,
    ensures
        prepare_owner_v1(plan_token, session, submission, started_ns, deadline_ns, tickets).published,
        prepare_owner_v1(plan_token, session, submission, started_ns, deadline_ns, tickets)
            .completion_validation_roster == tickets,
        prepare_owner_v1(plan_token, session, submission, started_ns, deadline_ns, tickets)
            .prepublication_allocation_count == 2,
        prepare_owner_v1(plan_token, session, submission, started_ns, deadline_ns, tickets)
            .completed_output_len == 0,
        prepare_owner_v1(plan_token, session, submission, started_ns, deadline_ns, tickets)
            .completed_output_capacity == tickets.len(),
        prepare_owner_v1(plan_token, session, submission, started_ns, deadline_ns, tickets)
            .postpublication_allocation_count == 0,
{}

// Obligation 15: an exact closed-currentness presentation passes validation.
pub proof fn exact_closed_presentation_is_accepted_v1(owner: AggregateOwnerV1)
    requires owner_is_prepared_v1(owner),
    ensures exact_presentation_v1(owner, PresentationV1 {
        plan_token: owner.plan_token,
        session: owner.session,
        submission: owner.submission,
        request_count: owner.request_count,
        tickets: owner.tickets,
        completion_validation_roster: owner.completion_validation_roster,
        currentness_closed: true,
    }),
{}

// Obligation 16: any non-exact plan/presentation terminates before observation.
pub proof fn presentation_substitution_is_terminal_before_observation_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
    preflight: bool,
)
    requires
        owner_is_prepared_v1(owner),
        !exact_presentation_v1(owner, presented),
    ensures
        poll_v1(owner, presented, scan, now_ns, preflight).phase
            == PollPhaseV1::ValidationTerminal,
        poll_v1(owner, presented, scan, now_ns, preflight).observation_count == 0,
        poll_v1(owner, presented, scan, now_ns, preflight).retired_count == 0,
{}

// Obligation 17: exact shard/ticket/index/slot/generation rosters are checked as sequence equality.
pub proof fn ticket_roster_substitution_is_terminal_before_observation_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    scan: ScanClassV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        presented.plan_token == owner.plan_token,
        presented.session == owner.session,
        presented.submission == owner.submission,
        presented.request_count == owner.request_count,
        presented.currentness_closed,
        presented.tickets != owner.tickets
            || presented.completion_validation_roster != owner.completion_validation_roster,
    ensures
        poll_v1(owner, presented, scan, now_ns, false).phase
            == PollPhaseV1::ValidationTerminal,
        poll_v1(owner, presented, scan, now_ns, false).observation_count == 0,
{}

// Obligation 18: Pending is classified only after the entire exact roster is observed.
pub proof fn pending_observes_entire_roster_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
        now_ns < owner.deadline_ns,
    ensures
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).phase
            == PollPhaseV1::Pending,
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).observation_count
            == owner.request_count,
{}

// Obligation 19: Pending returns exact whole custody and both preparations with zero retirement.
pub proof fn pending_retains_exact_whole_prepared_submission_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
        now_ns < owner.deadline_ns,
    ensures
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).returned_owner
            == Option::Some(owner),
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).retired_count == 0,
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false)
            .postpublication_allocation_count == 0,
{}

// Obligation 20: one shared absolute deadline is applied after the complete Pending scan.
pub proof fn shared_deadline_timeout_follows_full_scan_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
        now_ns >= owner.deadline_ns,
    ensures
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).phase
            == PollPhaseV1::TimedOut,
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).observation_count
            == owner.request_count,
{}

// Obligation 21: timeout returns exact whole custody and both preparations with zero retirement.
pub proof fn timeout_retains_exact_whole_prepared_submission_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
        now_ns >= owner.deadline_ns,
    ensures
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).returned_owner
            == Option::Some(owner),
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false).retired_count == 0,
        poll_v1(owner, presented, ScanClassV1::Pending, now_ns, false)
            .postpublication_allocation_count == 0,
{}

// Obligation 22: the first contracted error stops observation and yields terminal custody.
pub proof fn error_stops_and_yields_terminal_custody_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    first_error_index: nat,
    terminal_token: nat,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
        first_error_index < owner.request_count,
    ensures
        poll_v1(owner, presented, ScanClassV1::Error {
            first_error_index,
            terminal_token,
        }, now_ns, false).phase == PollPhaseV1::ObservationTerminal,
        poll_v1(owner, presented, ScanClassV1::Error {
            first_error_index,
            terminal_token,
        }, now_ns, false).observation_count == first_error_index + 1,
        poll_v1(owner, presented, ScanClassV1::Error {
            first_error_index,
            terminal_token,
        }, now_ns, false).returned_owner == Option::Some(owner),
        poll_v1(owner, presented, ScanClassV1::Error {
            first_error_index,
            terminal_token,
        }, now_ns, false).terminal_token == Option::Some(terminal_token),
{}

// Obligation 23: failed full retirement preflight is terminal and retires nothing.
pub proof fn failed_full_preflight_retires_nothing_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
    ensures
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, false).phase
            == PollPhaseV1::PreflightTerminal,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, false).observation_count
            == owner.request_count,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, false).retired_count == 0,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, false).returned_owner
            == Option::Some(owner),
{}

// Obligation 24: successful full preflight retires all shards in request order without late allocation.
pub proof fn successful_preflight_retires_all_in_order_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
    ensures
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, true).phase
            == PollPhaseV1::Completed,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, true).observation_count
            == owner.request_count,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, true).retired_count
            == owner.request_count,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, true).completed_tickets
            == owner.tickets,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, true)
            .output_is_request_ordered,
        poll_v1(owner, presented, ScanClassV1::AllReady, now_ns, true)
            .postpublication_allocation_count == 0,
{}

// Obligation 25: a post-retirement model-retake failure retains every moved ticket in terminal
// custody and exposes no normal completed output.
pub proof fn failed_model_retake_retains_terminal_output_custody_v1(
    owner: AggregateOwnerV1,
    presented: PresentationV1,
    now_ns: nat,
)
    requires
        owner_is_prepared_v1(owner),
        exact_presentation_v1(owner, presented),
    ensures
        poll_with_retake_v1(owner, presented, ScanClassV1::AllReady, now_ns, true, false).phase
            == PollPhaseV1::RetakeTerminal,
        poll_with_retake_v1(owner, presented, ScanClassV1::AllReady, now_ns, true, false)
            .observation_count == owner.request_count,
        poll_with_retake_v1(owner, presented, ScanClassV1::AllReady, now_ns, true, false)
            .retired_count == owner.request_count,
        poll_with_retake_v1(owner, presented, ScanClassV1::AllReady, now_ns, true, false)
            .completed_tickets == Seq::<TicketV1>::empty(),
        poll_with_retake_v1(owner, presented, ScanClassV1::AllReady, now_ns, true, false)
            .terminal_retained_tickets == owner.tickets,
        poll_with_retake_v1(owner, presented, ScanClassV1::AllReady, now_ns, true, false)
            .postpublication_allocation_count == 0,
{}

}
