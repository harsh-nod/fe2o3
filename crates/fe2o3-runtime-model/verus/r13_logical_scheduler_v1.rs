use vstd::prelude::*;

verus! {

pub open spec fn physical_lane_count_v1() -> nat { 2 }
pub open spec fn max_logical_streams_v1() -> nat { 64 }
pub open spec fn max_dependencies_v1() -> nat { 32 }
pub open spec fn max_dependency_depth_v1() -> nat { 32 }

#[derive(PartialEq, Eq)]
pub enum OperationClassV1 {
    Compute,
    Copy,
}

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    Queued,
    Published,
    TerminalSucceeded,
    TerminalFailed,
    CancelledBeforePublication,
    Indeterminate,
    Released,
}

pub struct SubmissionV1 {
    pub submission_id: nat,
    pub stream_id: nat,
    pub sequence: nat,
    pub class: OperationClassV1,
    pub predecessor: Option<nat>,
    pub successor: Option<nat>,
    pub dependencies: Set<nat>,
    pub dependency_count: nat,
    pub dependency_depth: nat,
    pub resources: Set<nat>,
    pub phase: PhaseV1,
    pub lane: Option<nat>,
    pub owns_resources: bool,
    pub quarantined: bool,
    pub current: bool,
}

#[derive(PartialEq, Eq)]
pub struct LogicalStreamV1 {
    pub stream_id: nat,
    pub head: Option<nat>,
    pub tail: Option<nat>,
}

pub struct SchedulerV1 {
    pub current: bool,
    pub streams: Seq<LogicalStreamV1>,
    pub lane0_owner: Option<nat>,
    pub lane1_owner: Option<nat>,
}

pub open spec fn valid_logical_stream_v1(stream: LogicalStreamV1) -> bool {
    &&& stream.stream_id > 0
    &&& stream.head.is_none() == stream.tail.is_none()
    &&& (stream.head.is_some() ==> stream.head.unwrap() > 0)
    &&& (stream.tail.is_some() ==> stream.tail.unwrap() > 0)
}

pub open spec fn stream_registered_v1(
    scheduler: SchedulerV1,
    stream: LogicalStreamV1,
) -> bool {
    exists|index: int| 0 <= index < scheduler.streams.len()
        && scheduler.streams[index] == stream
}

pub open spec fn bounded_scheduler_v1(scheduler: SchedulerV1) -> bool {
    &&& 1 <= scheduler.streams.len() <= max_logical_streams_v1()
    &&& forall|index: int| 0 <= index < scheduler.streams.len()
        ==> valid_logical_stream_v1(scheduler.streams[index])
    &&& forall|first: int, second: int|
        0 <= first < scheduler.streams.len()
            && 0 <= second < scheduler.streams.len()
            && scheduler.streams[first].stream_id == scheduler.streams[second].stream_id
            ==> first == second
    &&& match (scheduler.lane0_owner, scheduler.lane1_owner) {
        (Some(first), Some(second)) => first != second,
        _ => true,
    }
    &&& (scheduler.lane0_owner.is_some() ==> scheduler.lane0_owner.unwrap() > 0)
    &&& (scheduler.lane1_owner.is_some() ==> scheduler.lane1_owner.unwrap() > 0)
}

pub open spec fn valid_submission_v1(state: SubmissionV1) -> bool {
    &&& state.submission_id > 0
    &&& state.stream_id > 0
    &&& state.sequence > 0
    &&& state.dependency_count == state.dependencies.len()
    &&& state.dependency_count <= max_dependencies_v1()
    &&& state.dependency_depth >= 1
    &&& state.dependency_depth <= max_dependency_depth_v1()
    &&& !state.dependencies.contains(state.submission_id)
    &&& forall|dependency: nat| state.dependencies.contains(dependency) ==> dependency > 0
    &&& (state.predecessor.is_some()
        ==> state.dependencies.contains(state.predecessor.unwrap()))
    &&& match state.phase {
        PhaseV1::Queued => {
            &&& state.current
            &&& state.lane.is_none()
            &&& !state.owns_resources
            &&& !state.quarantined
        },
        PhaseV1::Published => {
            &&& state.current
            &&& state.lane.is_some()
            &&& state.lane.unwrap() < physical_lane_count_v1()
            &&& state.owns_resources
            &&& !state.quarantined
        },
        PhaseV1::TerminalSucceeded | PhaseV1::TerminalFailed => {
            &&& state.current
            &&& state.lane.is_none()
            &&& state.owns_resources
            &&& !state.quarantined
        },
        PhaseV1::CancelledBeforePublication | PhaseV1::Released => {
            &&& state.lane.is_none()
            &&& !state.owns_resources
            &&& !state.quarantined
        },
        PhaseV1::Indeterminate => {
            &&& !state.current
            &&& state.owns_resources
            &&& state.quarantined
            &&& (state.lane.is_none() || state.lane.unwrap() < physical_lane_count_v1())
        },
    }
}

pub open spec fn dependency_succeeded_v1(state: SubmissionV1) -> bool {
    state.phase == PhaseV1::TerminalSucceeded
}

pub open spec fn dependencies_succeeded_v1(
    consumer: SubmissionV1,
    submissions: Seq<SubmissionV1>,
) -> bool {
    forall|dependency: nat| consumer.dependencies.contains(dependency) ==> exists|index: int|
        0 <= index < submissions.len()
            && submissions[index].submission_id == dependency
            && dependency_succeeded_v1(submissions[index])
}

pub open spec fn resources_available_v1(
    candidate: SubmissionV1,
    retained_active_resources: Set<nat>,
) -> bool {
    candidate.resources.intersect(retained_active_resources).is_empty()
}

pub open spec fn select_free_lane_v1(scheduler: SchedulerV1) -> Option<nat> {
    if scheduler.lane0_owner.is_none() {
        Some(0)
    } else if scheduler.lane1_owner.is_none() {
        Some(1)
    } else {
        None
    }
}

pub open spec fn publication_allowed_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    retained_active_resources: Set<nat>,
) -> bool {
    &&& valid_submission_v1(candidate)
    &&& bounded_scheduler_v1(scheduler)
    &&& candidate.phase == PhaseV1::Queued
    &&& candidate.current
    &&& scheduler.current
    &&& valid_logical_stream_v1(stream)
    &&& stream_registered_v1(scheduler, stream)
    &&& candidate.stream_id == stream.stream_id
    &&& stream.head == Some(candidate.submission_id)
    &&& dependencies_succeeded_v1(candidate, submissions)
    &&& resources_available_v1(candidate, retained_active_resources)
    &&& select_free_lane_v1(scheduler).is_some()
    &&& scheduler.lane0_owner != Some(candidate.submission_id)
    &&& scheduler.lane1_owner != Some(candidate.submission_id)
}

pub open spec fn publish_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    retained_active_resources: Set<nat>,
) -> SubmissionV1 {
    if publication_allowed_v1(
        candidate,
        stream,
        scheduler,
        submissions,
        retained_active_resources,
    ) {
        SubmissionV1 {
            phase: PhaseV1::Published,
            lane: select_free_lane_v1(scheduler),
            owns_resources: true,
            ..candidate
        }
    } else {
        candidate
    }
}

pub open spec fn publish_scheduler_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    retained_active_resources: Set<nat>,
) -> SchedulerV1 {
    if publication_allowed_v1(
        candidate,
        stream,
        scheduler,
        submissions,
        retained_active_resources,
    ) {
        if select_free_lane_v1(scheduler) == Some(0) {
            SchedulerV1 { lane0_owner: Some(candidate.submission_id), ..scheduler }
        } else {
            SchedulerV1 { lane1_owner: Some(candidate.submission_id), ..scheduler }
        }
    } else {
        scheduler
    }
}

pub open spec fn terminal_observation_allowed_v1(
    state: SubmissionV1,
    scheduler: SchedulerV1,
    observed_lane: nat,
) -> bool {
    &&& state.phase == PhaseV1::Published
    &&& state.lane == Some(observed_lane)
    &&& ((observed_lane == 0 && scheduler.lane0_owner == Some(state.submission_id))
        || (observed_lane == 1 && scheduler.lane1_owner == Some(state.submission_id)))
}

pub open spec fn observe_terminal_v1(
    state: SubmissionV1,
    scheduler: SchedulerV1,
    observed_lane: nat,
    succeeded: bool,
) -> SubmissionV1 {
    if terminal_observation_allowed_v1(state, scheduler, observed_lane) {
        SubmissionV1 {
            phase: if succeeded { PhaseV1::TerminalSucceeded } else { PhaseV1::TerminalFailed },
            lane: None,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn observe_terminal_scheduler_v1(
    state: SubmissionV1,
    scheduler: SchedulerV1,
    observed_lane: nat,
) -> SchedulerV1 {
    if terminal_observation_allowed_v1(state, scheduler, observed_lane) {
        if observed_lane == 0 {
            SchedulerV1 { lane0_owner: None, ..scheduler }
        } else {
            SchedulerV1 { lane1_owner: None, ..scheduler }
        }
    } else {
        scheduler
    }
}

pub open spec fn cancel_tail_v1(
    state: SubmissionV1,
    stream: LogicalStreamV1,
) -> SubmissionV1 {
    if state.phase == PhaseV1::Queued
        && state.stream_id == stream.stream_id
        && stream.tail == Some(state.submission_id) {
        SubmissionV1 { phase: PhaseV1::CancelledBeforePublication, ..state }
    } else {
        state
    }
}

pub open spec fn cancel_tail_stream_v1(
    state: SubmissionV1,
    stream: LogicalStreamV1,
) -> LogicalStreamV1 {
    if state.phase == PhaseV1::Queued
        && state.stream_id == stream.stream_id
        && stream.tail == Some(state.submission_id) {
        LogicalStreamV1 {
            head: if stream.head == Some(state.submission_id) {
                None
            } else {
                stream.head
            },
            tail: state.predecessor,
            ..stream
        }
    } else {
        stream
    }
}

pub open spec fn is_queued_dependent_v1(
    producer: SubmissionV1,
    candidate: SubmissionV1,
) -> bool {
    candidate.phase == PhaseV1::Queued
        && candidate.dependencies.contains(producer.submission_id)
}

pub open spec fn has_queued_dependent_v1(
    producer: SubmissionV1,
    submissions: Seq<SubmissionV1>,
) -> bool {
    exists|index: int| 0 <= index < submissions.len()
        && is_queued_dependent_v1(producer, submissions[index])
}

pub open spec fn release_terminal_v1(
    state: SubmissionV1,
    submissions: Seq<SubmissionV1>,
) -> SubmissionV1 {
    if (state.phase == PhaseV1::TerminalSucceeded || state.phase == PhaseV1::TerminalFailed)
        && !has_queued_dependent_v1(state, submissions) {
        SubmissionV1 {
            phase: PhaseV1::Released,
            owns_resources: false,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn lose_currentness_v1(state: SubmissionV1) -> SubmissionV1 {
    match state.phase {
        PhaseV1::Queued => SubmissionV1 {
            phase: PhaseV1::CancelledBeforePublication,
            current: false,
            ..state
        },
        PhaseV1::Published | PhaseV1::TerminalSucceeded | PhaseV1::TerminalFailed => SubmissionV1 {
            phase: PhaseV1::Indeterminate,
            current: false,
            quarantined: true,
            ..state
        },
        _ => SubmissionV1 { current: false, ..state },
    }
}

pub proof fn bounded_scheduler_supports_more_streams_than_lanes_v1(scheduler: SchedulerV1)
    requires
        bounded_scheduler_v1(scheduler),
        scheduler.streams.len() >= 3,
    ensures scheduler.streams.len() > physical_lane_count_v1(),
{
}

pub proof fn dependency_bounds_are_retained_v1(state: SubmissionV1)
    requires valid_submission_v1(state),
    ensures
        state.dependency_count <= max_dependencies_v1(),
        state.dependency_depth <= max_dependency_depth_v1(),
{
}

pub proof fn stream_predecessor_is_an_effective_dependency_v1(state: SubmissionV1)
    requires
        valid_submission_v1(state),
        state.predecessor.is_some(),
    ensures state.dependencies.contains(state.predecessor.unwrap()),
{
}

pub proof fn non_head_cannot_publish_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    resources: Set<nat>,
)
    requires
        candidate.phase == PhaseV1::Queued,
        stream.head != Some(candidate.submission_id),
    ensures publish_v1(candidate, stream, scheduler, submissions, resources) == candidate,
{
}

pub proof fn mixed_class_fifo_tail_cannot_bypass_head_v1(
    head: SubmissionV1,
    tail: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
)
    requires
        head.stream_id == tail.stream_id,
        head.submission_id != tail.submission_id,
        head.class != tail.class,
        tail.predecessor == Some(head.submission_id),
        tail.dependencies.contains(head.submission_id),
        stream.stream_id == head.stream_id,
        stream.head == Some(head.submission_id),
        tail.phase == PhaseV1::Queued,
    ensures publish_v1(tail, stream, scheduler, Seq::empty(), Set::empty()) == tail,
{
}

pub proof fn unready_dependency_blocks_publication_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    resources: Set<nat>,
)
    requires
        candidate.phase == PhaseV1::Queued,
        !dependencies_succeeded_v1(candidate, submissions),
    ensures publish_v1(candidate, stream, scheduler, submissions, resources) == candidate,
{
}

pub proof fn resource_overlap_blocks_publication_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    retained: Set<nat>,
    resource: nat,
)
    requires
        candidate.resources.contains(resource),
        retained.contains(resource),
    ensures publish_v1(candidate, stream, scheduler, submissions, retained) == candidate,
{
    assert(candidate.resources.intersect(retained).contains(resource));
    assert(!resources_available_v1(candidate, retained));
}

pub proof fn ready_head_leases_one_bounded_lane_and_resources_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    resources: Set<nat>,
)
    requires
        valid_submission_v1(candidate),
        bounded_scheduler_v1(scheduler),
        publication_allowed_v1(candidate, stream, scheduler, submissions, resources),
    ensures {
        let published = publish_v1(candidate, stream, scheduler, submissions, resources);
        &&& published.phase == PhaseV1::Published
        &&& published.lane.is_some()
        &&& published.lane.unwrap() < physical_lane_count_v1()
        &&& published.owns_resources
        &&& valid_submission_v1(published)
    },
{
}

pub proof fn publication_preserves_unique_lane_owners_v1(
    candidate: SubmissionV1,
    stream: LogicalStreamV1,
    scheduler: SchedulerV1,
    submissions: Seq<SubmissionV1>,
    resources: Set<nat>,
)
    requires
        bounded_scheduler_v1(scheduler),
        publication_allowed_v1(candidate, stream, scheduler, submissions, resources),
        scheduler.lane0_owner != Some(candidate.submission_id),
        scheduler.lane1_owner != Some(candidate.submission_id),
    ensures bounded_scheduler_v1(
        publish_scheduler_v1(candidate, stream, scheduler, submissions, resources),
    ),
{
}

pub proof fn foreign_lane_cannot_complete_v1(
    state: SubmissionV1,
    scheduler: SchedulerV1,
    observed_lane: nat,
    succeeded: bool,
)
    requires
        state.phase == PhaseV1::Published,
        state.lane != Some(observed_lane),
    ensures
        observe_terminal_v1(state, scheduler, observed_lane, succeeded) == state,
        observe_terminal_scheduler_v1(state, scheduler, observed_lane) == scheduler,
{
}

pub proof fn foreign_lane_owner_cannot_complete_v1(
    state: SubmissionV1,
    scheduler: SchedulerV1,
    observed_lane: nat,
    succeeded: bool,
)
    requires
        state.phase == PhaseV1::Published,
        state.lane == Some(observed_lane),
        !((observed_lane == 0 && scheduler.lane0_owner == Some(state.submission_id))
            || (observed_lane == 1 && scheduler.lane1_owner == Some(state.submission_id))),
    ensures
        observe_terminal_v1(state, scheduler, observed_lane, succeeded) == state,
        observe_terminal_scheduler_v1(state, scheduler, observed_lane) == scheduler,
{
}

pub proof fn exact_terminal_returns_lane_and_retains_resources_v1(
    state: SubmissionV1,
    scheduler: SchedulerV1,
    lane: nat,
)
    requires
        valid_submission_v1(state),
        state.phase == PhaseV1::Published,
        state.lane == Some(lane),
        (lane == 0 && scheduler.lane0_owner == Some(state.submission_id))
            || (lane == 1 && scheduler.lane1_owner == Some(state.submission_id)),
    ensures {
        let terminal = observe_terminal_v1(state, scheduler, lane, true);
        let after = observe_terminal_scheduler_v1(state, scheduler, lane);
        &&& terminal.phase == PhaseV1::TerminalSucceeded
        &&& terminal.lane.is_none()
        &&& terminal.owns_resources
        &&& valid_submission_v1(terminal)
        &&& (lane == 0 ==> after.lane0_owner.is_none())
        &&& (lane == 1 ==> after.lane1_owner.is_none())
    },
{
}

pub proof fn tail_cancel_restores_predecessor_v1(
    state: SubmissionV1,
    stream: LogicalStreamV1,
)
    requires
        valid_logical_stream_v1(stream),
        valid_submission_v1(state),
        state.phase == PhaseV1::Queued,
        state.stream_id == stream.stream_id,
        stream.tail == Some(state.submission_id),
        stream.head == Some(state.submission_id) ==> state.predecessor.is_none(),
        stream.head != Some(state.submission_id) ==> state.predecessor.is_some(),
    ensures {
        let cancelled = cancel_tail_v1(state, stream);
        let after = cancel_tail_stream_v1(state, stream);
        &&& cancelled.phase == PhaseV1::CancelledBeforePublication
        &&& after.tail == state.predecessor
        &&& (stream.head == Some(state.submission_id) ==> after.head.is_none())
        &&& valid_logical_stream_v1(after)
    },
{
}

pub proof fn non_tail_cancel_is_rejected_v1(state: SubmissionV1, stream: LogicalStreamV1)
    requires stream.tail != Some(state.submission_id),
    ensures
        cancel_tail_v1(state, stream) == state,
        cancel_tail_stream_v1(state, stream) == stream,
{
}

pub proof fn published_cancel_is_rejected_v1(state: SubmissionV1, stream: LogicalStreamV1)
    requires state.phase == PhaseV1::Published,
    ensures
        cancel_tail_v1(state, stream) == state,
        cancel_tail_stream_v1(state, stream) == stream,
{
}

pub proof fn queued_dependent_retains_terminal_resources_v1(
    producer: SubmissionV1,
    dependent: SubmissionV1,
)
    requires
        producer.phase == PhaseV1::TerminalSucceeded,
        dependent.phase == PhaseV1::Queued,
        dependent.dependencies.contains(producer.submission_id),
    ensures release_terminal_v1(producer, Seq::empty().push(dependent)) == producer,
{
    let submissions = Seq::empty().push(dependent);
    assert(submissions[0] == dependent);
    assert(has_queued_dependent_v1(producer, submissions)) by {
        assert(exists|index: int| 0 <= index < submissions.len()
            && is_queued_dependent_v1(producer, submissions[index]));
    }
}

pub proof fn unreferenced_terminal_release_relinquishes_resources_v1(state: SubmissionV1)
    requires
        valid_submission_v1(state),
        state.phase == PhaseV1::TerminalSucceeded || state.phase == PhaseV1::TerminalFailed,
    ensures {
        let released = release_terminal_v1(state, Seq::empty());
        &&& released.phase == PhaseV1::Released
        &&& !released.owns_resources
        &&& valid_submission_v1(released)
    },
{
}

pub proof fn currentness_loss_cancels_queued_v1(state: SubmissionV1)
    requires valid_submission_v1(state), state.phase == PhaseV1::Queued,
    ensures {
        let lost = lose_currentness_v1(state);
        &&& lost.phase == PhaseV1::CancelledBeforePublication
        &&& !lost.current
        &&& !lost.owns_resources
        &&& valid_submission_v1(lost)
    },
{
}

pub proof fn currentness_loss_quarantines_published_v1(state: SubmissionV1)
    requires valid_submission_v1(state), state.phase == PhaseV1::Published,
    ensures {
        let lost = lose_currentness_v1(state);
        &&& lost.phase == PhaseV1::Indeterminate
        &&& !lost.current
        &&& lost.owns_resources
        &&& lost.quarantined
        &&& lost.lane == state.lane
        &&& valid_submission_v1(lost)
    },
{
}

pub proof fn currentness_loss_quarantines_terminal_v1(state: SubmissionV1)
    requires
        valid_submission_v1(state),
        state.phase == PhaseV1::TerminalSucceeded || state.phase == PhaseV1::TerminalFailed,
    ensures {
        let lost = lose_currentness_v1(state);
        &&& lost.phase == PhaseV1::Indeterminate
        &&& !lost.current
        &&& lost.owns_resources
        &&& lost.quarantined
        &&& lost.lane.is_none()
        &&& valid_submission_v1(lost)
    },
{
}

}
