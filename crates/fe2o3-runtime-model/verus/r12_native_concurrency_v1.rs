use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub device_id: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct MultiQueueCapabilityV1 {
    pub device: DeviceKeyV1,
    pub stable: bool,
    pub multi_queue_compute: bool,
    pub max_compute_queues: nat,
    pub max_slots_per_queue: nat,
}

pub open spec fn capability_admitted_v1(
    requested_device: DeviceKeyV1,
    capability: MultiQueueCapabilityV1,
    queue_count: nat,
    slots_per_queue: nat,
) -> bool {
    &&& requested_device.device_id > 0
    &&& requested_device.generation > 0
    &&& capability.device == requested_device
    &&& capability.stable
    &&& capability.multi_queue_compute
    &&& 2 <= queue_count <= capability.max_compute_queues
    &&& queue_count <= 16
    &&& 1 <= slots_per_queue <= capability.max_slots_per_queue
    &&& slots_per_queue <= 64
}

#[derive(PartialEq, Eq)]
pub struct QueueOccurrenceV1 {
    pub device: DeviceKeyV1,
    pub queue_id: nat,
    pub occurrence: nat,
}

#[derive(PartialEq, Eq)]
pub struct SlotKeyV1 {
    pub queue: QueueOccurrenceV1,
    pub slot_index: nat,
    pub generation: nat,
}

pub open spec fn queue_occurrence_matches_v1(
    expected: QueueOccurrenceV1,
    observed: QueueOccurrenceV1,
) -> bool {
    expected == observed
}

pub open spec fn slot_matches_v1(expected: SlotKeyV1, observed: SlotKeyV1) -> bool {
    expected == observed
}

#[derive(PartialEq, Eq)]
pub enum TerminalStatusV1 {
    Succeeded,
    Failed { code: int },
    QuiescentWithoutResult,
}

#[derive(PartialEq, Eq)]
pub enum SubmissionPhaseV1 {
    Reserved,
    Published,
    Terminal { status: TerminalStatusV1 },
    CancelledBeforePublication,
    Indeterminate,
    Released,
}

pub struct CustodyStateV1 {
    pub submission_id: nat,
    pub dependencies: Set<nat>,
    pub queue: QueueOccurrenceV1,
    pub slot: SlotKeyV1,
    pub phase: SubmissionPhaseV1,
    pub owns_slot: bool,
    pub owns_resource: bool,
    pub resource_quarantined: bool,
    pub live_slot_generation: nat,
    pub current: bool,
}

pub open spec fn valid_custody_v1(state: CustodyStateV1) -> bool {
    &&& state.submission_id > 0
    &&& state.queue.queue_id > 0
    &&& state.queue.occurrence > 0
    &&& state.queue.device.device_id > 0
    &&& state.queue.device.generation > 0
    &&& state.slot.queue == state.queue
    &&& state.slot.generation > 0
    &&& !state.dependencies.contains(state.submission_id)
    &&& match state.phase {
        SubmissionPhaseV1::Reserved =>
            state.current && state.owns_slot && state.owns_resource && !state.resource_quarantined
                && state.live_slot_generation == state.slot.generation,
        SubmissionPhaseV1::Published =>
            state.owns_slot && state.owns_resource && !state.resource_quarantined
                && state.live_slot_generation == state.slot.generation,
        SubmissionPhaseV1::Terminal { .. } =>
            state.owns_slot && state.owns_resource && !state.resource_quarantined
                && state.live_slot_generation == state.slot.generation,
        SubmissionPhaseV1::CancelledBeforePublication =>
            !state.owns_slot && !state.owns_resource && !state.resource_quarantined
                && state.live_slot_generation == state.slot.generation + 1,
        SubmissionPhaseV1::Indeterminate =>
            !state.current && state.owns_slot && state.owns_resource && state.resource_quarantined
                && state.live_slot_generation == state.slot.generation,
        SubmissionPhaseV1::Released =>
            !state.owns_slot && !state.owns_resource && !state.resource_quarantined
                && state.live_slot_generation == state.slot.generation + 1,
    }
}

pub open spec fn dependency_succeeded_v1(producer: CustodyStateV1) -> bool {
    producer.phase == SubmissionPhaseV1::Terminal { status: TerminalStatusV1::Succeeded }
}

pub open spec fn dependencies_succeeded_v1(
    consumer: CustodyStateV1,
    submissions: Seq<CustodyStateV1>,
) -> bool {
    forall|dependency: nat| consumer.dependencies.contains(dependency) ==> exists|index: int|
        0 <= index < submissions.len()
            && submissions[index].submission_id == dependency
            && dependency_succeeded_v1(submissions[index])
}

pub open spec fn publish_v1(
    state: CustodyStateV1,
    submissions: Seq<CustodyStateV1>,
) -> CustodyStateV1 {
    if state.phase == SubmissionPhaseV1::Reserved && state.current
        && dependencies_succeeded_v1(state, submissions) {
        CustodyStateV1 { phase: SubmissionPhaseV1::Published, ..state }
    } else {
        state
    }
}

pub open spec fn reserved_dependent_v1(
    producer: CustodyStateV1,
    candidate: CustodyStateV1,
) -> bool {
    candidate.phase == SubmissionPhaseV1::Reserved
        && candidate.dependencies.contains(producer.submission_id)
}

pub open spec fn has_reserved_dependent_v1(
    producer: CustodyStateV1,
    submissions: Seq<CustodyStateV1>,
) -> bool {
    exists|index: int| 0 <= index < submissions.len()
        && #[trigger] reserved_dependent_v1(producer, submissions[index])
}

pub open spec fn observe_terminal_v1(
    state: CustodyStateV1,
    observed_slot: SlotKeyV1,
    status: TerminalStatusV1,
) -> CustodyStateV1 {
    if state.phase == SubmissionPhaseV1::Published && slot_matches_v1(state.slot, observed_slot) {
        CustodyStateV1 { phase: SubmissionPhaseV1::Terminal { status }, ..state }
    } else {
        state
    }
}

pub open spec fn cancel_v1(state: CustodyStateV1) -> CustodyStateV1 {
    if state.phase == SubmissionPhaseV1::Reserved {
        CustodyStateV1 {
            phase: SubmissionPhaseV1::CancelledBeforePublication,
            owns_slot: false,
            owns_resource: false,
            live_slot_generation: state.slot.generation + 1,
            ..state
        }
    } else {
        state
    }
}

pub open spec fn release_terminal_v1(
    state: CustodyStateV1,
    submissions: Seq<CustodyStateV1>,
) -> CustodyStateV1 {
    if !has_reserved_dependent_v1(state, submissions) {
        match state.phase {
            SubmissionPhaseV1::Terminal { .. } => CustodyStateV1 {
                phase: SubmissionPhaseV1::Released,
                owns_slot: false,
                owns_resource: false,
                live_slot_generation: state.slot.generation + 1,
                ..state
            },
            _ => state,
        }
    } else {
        state
    }
}

pub open spec fn lose_currentness_v1(state: CustodyStateV1) -> CustodyStateV1 {
    match state.phase {
        SubmissionPhaseV1::Reserved => CustodyStateV1 {
            phase: SubmissionPhaseV1::CancelledBeforePublication,
            current: false,
            owns_slot: false,
            owns_resource: false,
            live_slot_generation: state.slot.generation + 1,
            ..state
        },
        SubmissionPhaseV1::Published => CustodyStateV1 {
            phase: SubmissionPhaseV1::Indeterminate,
            current: false,
            resource_quarantined: true,
            ..state
        },
        _ => CustodyStateV1 { current: false, ..state },
    }
}

pub open spec fn drain_allowed_v1(
    queue: QueueOccurrenceV1,
    observed_queue: QueueOccurrenceV1,
    submissions: Seq<CustodyStateV1>,
) -> bool {
    observed_queue == queue
        && forall|index: int| 0 <= index < submissions.len()
            && submissions[index].queue == queue ==> {
                let state = submissions[index];
                &&& !state.owns_slot
                &&& !state.owns_resource
                &&& state.phase != SubmissionPhaseV1::Reserved
                &&& state.phase != SubmissionPhaseV1::Published
                &&& state.phase != SubmissionPhaseV1::Indeterminate
            }
}

#[derive(PartialEq, Eq)]
pub struct QueueLifecycleStateV1 {
    pub queue: QueueOccurrenceV1,
    pub drained: bool,
    pub current: bool,
}

pub open spec fn drain_queue_v1(
    state: QueueLifecycleStateV1,
    observed_queue: QueueOccurrenceV1,
    submissions: Seq<CustodyStateV1>,
) -> QueueLifecycleStateV1 {
    if drain_allowed_v1(state.queue, observed_queue, submissions) {
        QueueLifecycleStateV1 { drained: true, ..state }
    } else {
        state
    }
}

pub open spec fn recreate_drained_queue_v1(
    state: QueueLifecycleStateV1,
) -> Option<QueueLifecycleStateV1> {
    if state.drained && state.current {
        Some(QueueLifecycleStateV1 {
            queue: QueueOccurrenceV1 { occurrence: state.queue.occurrence + 1, ..state.queue },
            drained: false,
            ..state
        })
    } else {
        None
    }
}

pub proof fn exact_stable_multi_queue_capability_is_admitted_v1(
    device: DeviceKeyV1,
    capability: MultiQueueCapabilityV1,
    queue_count: nat,
    slots_per_queue: nat,
)
    requires
        device.device_id > 0,
        device.generation > 0,
        capability.device == device,
        capability.stable,
        capability.multi_queue_compute,
        2 <= queue_count <= capability.max_compute_queues,
        queue_count <= 16,
        1 <= slots_per_queue <= capability.max_slots_per_queue,
        slots_per_queue <= 64,
    ensures capability_admitted_v1(device, capability, queue_count, slots_per_queue),
{
}

pub proof fn single_queue_request_is_rejected_v1(
    device: DeviceKeyV1,
    capability: MultiQueueCapabilityV1,
    slots_per_queue: nat,
)
    ensures !capability_admitted_v1(device, capability, 1, slots_per_queue),
{
}

pub proof fn unstable_capability_is_rejected_v1(
    device: DeviceKeyV1,
    capability: MultiQueueCapabilityV1,
    queue_count: nat,
    slots_per_queue: nat,
)
    requires !capability.stable,
    ensures !capability_admitted_v1(device, capability, queue_count, slots_per_queue),
{
}

pub proof fn queue_count_above_capability_is_rejected_v1(
    device: DeviceKeyV1,
    capability: MultiQueueCapabilityV1,
    queue_count: nat,
    slots_per_queue: nat,
)
    requires queue_count > capability.max_compute_queues,
    ensures !capability_admitted_v1(device, capability, queue_count, slots_per_queue),
{
}

pub proof fn stale_queue_occurrence_is_rejected_v1(
    expected: QueueOccurrenceV1,
    observed: QueueOccurrenceV1,
)
    requires expected != observed,
    ensures !queue_occurrence_matches_v1(expected, observed),
{
}

pub proof fn stale_slot_generation_is_rejected_v1(expected: SlotKeyV1, observed: SlotKeyV1)
    requires expected != observed,
    ensures !slot_matches_v1(expected, observed),
{
}

pub proof fn unready_dependency_blocks_publication_v1(
    state: CustodyStateV1,
    submissions: Seq<CustodyStateV1>,
)
    requires
        state.phase == SubmissionPhaseV1::Reserved,
        !dependencies_succeeded_v1(state, submissions),
    ensures publish_v1(state, submissions) == state,
{
}

pub proof fn ready_dependency_publishes_with_custody_v1(
    state: CustodyStateV1,
    submissions: Seq<CustodyStateV1>,
)
    requires
        valid_custody_v1(state),
        state.phase == SubmissionPhaseV1::Reserved,
        dependencies_succeeded_v1(state, submissions),
    ensures {
        let published = publish_v1(state, submissions);
        &&& published.phase == SubmissionPhaseV1::Published
        &&& published.owns_slot
        &&& published.owns_resource
        &&& valid_custody_v1(published)
    },
{
}

pub proof fn successful_terminal_dependency_publishes_consumer_v1(
    consumer: CustodyStateV1,
    producer: CustodyStateV1,
)
    requires
        valid_custody_v1(consumer),
        consumer.phase == SubmissionPhaseV1::Reserved,
        consumer.dependencies == Set::empty().insert(producer.submission_id),
        producer.phase == (SubmissionPhaseV1::Terminal {
            status: TerminalStatusV1::Succeeded,
        }),
    ensures publish_v1(consumer, Seq::empty().push(producer)).phase == SubmissionPhaseV1::Published,
{
    let submissions = Seq::empty().push(producer);
    assert forall|dependency: nat| consumer.dependencies.contains(dependency) implies
        exists|index: int| 0 <= index < submissions.len()
            && submissions[index].submission_id == dependency
            && dependency_succeeded_v1(submissions[index]) by {
        assert(dependency == producer.submission_id);
        assert(submissions[0] == producer);
        assert(exists|index: int| 0 <= index < submissions.len()
            && submissions[index].submission_id == dependency
            && dependency_succeeded_v1(submissions[index]));
    }
}

pub proof fn failed_terminal_dependency_blocks_consumer_v1(
    consumer: CustodyStateV1,
    producer: CustodyStateV1,
)
    requires
        consumer.phase == SubmissionPhaseV1::Reserved,
        consumer.dependencies == Set::empty().insert(producer.submission_id),
        !dependency_succeeded_v1(producer),
    ensures publish_v1(consumer, Seq::empty().push(producer)) == consumer,
{
    let submissions = Seq::empty().push(producer);
    assert(consumer.dependencies.contains(producer.submission_id));
    assert(!dependencies_succeeded_v1(consumer, submissions)) by {
        if dependencies_succeeded_v1(consumer, submissions) {
            let index = choose|index: int| 0 <= index < submissions.len()
                && submissions[index].submission_id == producer.submission_id
                && dependency_succeeded_v1(submissions[index]);
            assert(index == 0);
            assert(submissions[index] == producer);
        }
    }
}

pub proof fn exact_terminal_event_preserves_custody_v1(
    state: CustodyStateV1,
    status: TerminalStatusV1,
)
    requires valid_custody_v1(state), state.phase == SubmissionPhaseV1::Published,
    ensures {
        let terminal = observe_terminal_v1(state, state.slot, status);
        &&& terminal.phase == SubmissionPhaseV1::Terminal { status }
        &&& terminal.owns_slot
        &&& terminal.owns_resource
        &&& valid_custody_v1(terminal)
    },
{
}

pub proof fn foreign_terminal_event_cannot_complete_submission_v1(
    state: CustodyStateV1,
    foreign_slot: SlotKeyV1,
    status: TerminalStatusV1,
)
    requires state.phase == SubmissionPhaseV1::Published, foreign_slot != state.slot,
    ensures observe_terminal_v1(state, foreign_slot, status) == state,
{
}

pub proof fn out_of_order_terminal_event_changes_only_exact_owner_v1(
    first: CustodyStateV1,
    second: CustodyStateV1,
    status: TerminalStatusV1,
)
    requires
        first.phase == SubmissionPhaseV1::Published,
        second.phase == SubmissionPhaseV1::Published,
        first.slot != second.slot,
    ensures {
        &&& observe_terminal_v1(first, second.slot, status) == first
        &&& observe_terminal_v1(second, second.slot, status).phase
            == SubmissionPhaseV1::Terminal { status }
    },
{
}

pub proof fn prepublication_cancel_relinquishes_custody_v1(state: CustodyStateV1)
    requires valid_custody_v1(state), state.phase == SubmissionPhaseV1::Reserved,
    ensures {
        let cancelled = cancel_v1(state);
        &&& cancelled.phase == SubmissionPhaseV1::CancelledBeforePublication
        &&& !cancelled.owns_slot
        &&& !cancelled.owns_resource
        &&& cancelled.live_slot_generation == state.slot.generation + 1
        &&& valid_custody_v1(cancelled)
        &&& drain_allowed_v1(
            state.queue,
            state.queue,
            Seq::empty().push(cancelled),
        )
    },
{
}

pub proof fn published_cancel_retains_custody_v1(state: CustodyStateV1)
    requires state.phase == SubmissionPhaseV1::Published,
    ensures cancel_v1(state) == state,
{
}

pub proof fn terminal_release_relinquishes_custody_v1(state: CustodyStateV1)
    requires valid_custody_v1(state), matches!(state.phase, SubmissionPhaseV1::Terminal { .. }),
    ensures {
        let released = release_terminal_v1(state, Seq::empty());
        &&& released.phase == SubmissionPhaseV1::Released
        &&& !released.owns_slot
        &&& !released.owns_resource
        &&& released.live_slot_generation == state.slot.generation + 1
        &&& valid_custody_v1(released)
        &&& drain_allowed_v1(
            state.queue,
            state.queue,
            Seq::empty().push(released),
        )
    },
{
}

pub proof fn published_release_retains_custody_v1(state: CustodyStateV1)
    requires state.phase == SubmissionPhaseV1::Published,
    ensures release_terminal_v1(state, Seq::empty()) == state,
{
}

pub proof fn reserved_dependent_blocks_terminal_release_v1(
    state: CustodyStateV1,
    dependent: CustodyStateV1,
)
    requires
        matches!(state.phase, SubmissionPhaseV1::Terminal { .. }),
        dependent.phase == SubmissionPhaseV1::Reserved,
        dependent.dependencies.contains(state.submission_id),
    ensures release_terminal_v1(state, Seq::empty().push(dependent)) == state,
{
    let submissions = Seq::empty().push(dependent);
    assert(submissions[0] == dependent);
    assert(has_reserved_dependent_v1(state, submissions)) by {
        assert(exists|index: int| 0 <= index < submissions.len()
            && reserved_dependent_v1(state, submissions[index]));
    }
}

pub proof fn currentness_loss_cancels_reserved_v1(state: CustodyStateV1)
    requires valid_custody_v1(state), state.phase == SubmissionPhaseV1::Reserved,
    ensures {
        let lost = lose_currentness_v1(state);
        &&& lost.phase == SubmissionPhaseV1::CancelledBeforePublication
        &&& !lost.current
        &&& !lost.owns_slot
        &&& !lost.owns_resource
        &&& lost.live_slot_generation == state.slot.generation + 1
        &&& valid_custody_v1(lost)
    },
{
}

pub proof fn currentness_loss_quarantines_published_v1(state: CustodyStateV1)
    requires valid_custody_v1(state), state.phase == SubmissionPhaseV1::Published,
    ensures {
        let lost = lose_currentness_v1(state);
        &&& lost.phase == SubmissionPhaseV1::Indeterminate
        &&& !lost.current
        &&& lost.owns_slot
        &&& lost.owns_resource
        &&& lost.resource_quarantined
        &&& valid_custody_v1(lost)
    },
{
}

pub proof fn indeterminate_custody_blocks_drain_v1(state: CustodyStateV1)
    requires valid_custody_v1(state), state.phase == SubmissionPhaseV1::Indeterminate,
    ensures
        !drain_allowed_v1(state.queue, state.queue, Seq::empty().push(state)),
        release_terminal_v1(state, Seq::empty()) == state,
{
    let submissions = Seq::empty().push(state);
    assert(submissions[0] == state);
    assert(!drain_allowed_v1(state.queue, state.queue, submissions)) by {
        if drain_allowed_v1(state.queue, state.queue, submissions) {
            assert(!submissions[0].owns_slot);
        }
    }
}

pub proof fn stale_queue_occurrence_cannot_be_drained_v1(
    state: CustodyStateV1,
    stale_queue: QueueOccurrenceV1,
)
    requires stale_queue != state.queue,
    ensures !drain_allowed_v1(state.queue, stale_queue, Seq::empty().push(state)),
{
}

pub proof fn drained_current_queue_recreation_advances_occurrence_v1(
    queue: QueueOccurrenceV1,
    released: CustodyStateV1,
)
    requires
        valid_custody_v1(released),
        released.queue == queue,
        released.phase == SubmissionPhaseV1::Released,
    ensures {
        let before = QueueLifecycleStateV1 { queue, drained: false, current: true };
        let drained = drain_queue_v1(before, queue, Seq::empty().push(released));
        &&& drained.drained
        &&& recreate_drained_queue_v1(drained) == Some(QueueLifecycleStateV1 {
            queue: QueueOccurrenceV1 { occurrence: queue.occurrence + 1, ..queue },
            drained: false,
            current: true,
        })
    },
{
    let submissions = Seq::empty().push(released);
    assert(submissions[0] == released);
    assert(drain_allowed_v1(queue, queue, submissions));
}

}
