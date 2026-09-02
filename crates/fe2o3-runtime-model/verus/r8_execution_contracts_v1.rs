use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct ResourceKeyV1 {
    pub device: nat,
    pub allocation: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct AccessV1 {
    pub resource: ResourceKeyV1,
    pub write: bool,
}

#[derive(PartialEq, Eq)]
pub enum OperationPhaseV1 {
    Reserved,
    Published,
    Complete,
    Quarantined,
}

#[derive(PartialEq, Eq)]
pub struct AsyncOperationV1 {
    pub identity: nat,
    pub execution_device: nat,
    pub source: AccessV1,
    pub destination: AccessV1,
    pub dependency_frontier: nat,
    pub destination_epoch: nat,
    pub phase: OperationPhaseV1,
}

// This is deliberately a whole-resource abstraction. It has no byte offsets,
// lengths, physical-alias relation, or refinement to the executable ranged-copy
// SPI. In particular, same-resource copies are outside this model's domain.
pub open spec fn valid_resource_v1(resource: ResourceKeyV1) -> bool {
    resource.device > 0 && resource.allocation > 0 && resource.generation > 0
}

pub open spec fn valid_copy_v1(operation: AsyncOperationV1) -> bool {
    &&& operation.identity > 0
    &&& operation.execution_device == operation.destination.resource.device
    &&& valid_resource_v1(operation.source.resource)
    &&& valid_resource_v1(operation.destination.resource)
    &&& operation.source.resource != operation.destination.resource
    &&& !operation.source.write
    &&& operation.destination.write
}

pub open spec fn reserve_copy_v1(
    identity: nat,
    source: ResourceKeyV1,
    destination: ResourceKeyV1,
    dependency_frontier: nat,
    destination_epoch: nat,
) -> AsyncOperationV1 {
    AsyncOperationV1 {
        identity,
        execution_device: destination.device,
        source: AccessV1 { resource: source, write: false },
        destination: AccessV1 { resource: destination, write: true },
        dependency_frontier,
        destination_epoch,
        phase: OperationPhaseV1::Reserved,
    }
}

pub open spec fn publish_ready_v1(
    operation: AsyncOperationV1,
    completed_frontier: nat,
) -> AsyncOperationV1 {
    if valid_copy_v1(operation)
        && operation.phase == OperationPhaseV1::Reserved
        && completed_frontier >= operation.dependency_frontier
    {
        AsyncOperationV1 { phase: OperationPhaseV1::Published, ..operation }
    } else {
        operation
    }
}

pub proof fn reservation_is_deferred_and_preserves_destination_epoch_v1(
    identity: nat,
    source: ResourceKeyV1,
    destination: ResourceKeyV1,
    dependency_frontier: nat,
    destination_epoch: nat,
)
    ensures {
        let reserved = reserve_copy_v1(
            identity,
            source,
            destination,
            dependency_frontier,
            destination_epoch,
        );
        &&& reserved.phase == OperationPhaseV1::Reserved
        &&& reserved.destination_epoch == destination_epoch
        &&& reserved.source.resource == source
        &&& reserved.destination.resource == destination
    },
{
}

pub proof fn incomplete_dependency_blocks_publication_v1(
    operation: AsyncOperationV1,
    completed_frontier: nat,
)
    requires
        valid_copy_v1(operation),
        operation.phase == OperationPhaseV1::Reserved,
        completed_frontier < operation.dependency_frontier,
    ensures publish_ready_v1(operation, completed_frontier) == operation,
{
}

pub proof fn ready_publication_retains_exact_binding_v1(
    operation: AsyncOperationV1,
    completed_frontier: nat,
)
    requires
        valid_copy_v1(operation),
        operation.phase == OperationPhaseV1::Reserved,
        completed_frontier >= operation.dependency_frontier,
    ensures {
        let published = publish_ready_v1(operation, completed_frontier);
        &&& published.phase == OperationPhaseV1::Published
        &&& published.identity == operation.identity
        &&& published.execution_device == operation.destination.resource.device
        &&& published.source == operation.source
        &&& published.destination == operation.destination
        &&& published.destination_epoch == operation.destination_epoch
    },
{
}

pub open spec fn accesses_conflict_v1(left: AccessV1, right: AccessV1) -> bool {
    left.resource == right.resource && (left.write || right.write)
}

pub open spec fn operations_conflict_v1(
    left: AsyncOperationV1,
    right: AsyncOperationV1,
) -> bool {
    ||| accesses_conflict_v1(left.source, right.source)
    ||| accesses_conflict_v1(left.source, right.destination)
    ||| accesses_conflict_v1(left.destination, right.source)
    ||| accesses_conflict_v1(left.destination, right.destination)
}

pub open spec fn overlap_admitted_v1(
    left: AsyncOperationV1,
    right: AsyncOperationV1,
) -> bool {
    &&& valid_copy_v1(left)
    &&& valid_copy_v1(right)
    &&& left.phase == OperationPhaseV1::Published
    &&& right.phase == OperationPhaseV1::Published
    &&& left.identity != right.identity
    &&& !operations_conflict_v1(left, right)
}

pub proof fn admitted_overlap_has_no_read_write_or_write_write_conflict_v1(
    left: AsyncOperationV1,
    right: AsyncOperationV1,
)
    requires overlap_admitted_v1(left, right),
    ensures
        !accesses_conflict_v1(left.source, right.source),
        !accesses_conflict_v1(left.source, right.destination),
        !accesses_conflict_v1(left.destination, right.source),
        !accesses_conflict_v1(left.destination, right.destination),
{
}

#[derive(PartialEq, Eq)]
pub struct AtomicLocationV1 {
    pub resource: ResourceKeyV1,
    pub byte_offset: nat,
    pub width_bits: nat,
}

#[derive(PartialEq, Eq)]
pub enum AtomicOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(PartialEq, Eq)]
pub enum AtomicScopeV1 {
    Workgroup,
    Agent,
    System,
}

#[derive(PartialEq, Eq)]
pub struct AtomicLinearizationV1 {
    pub location: AtomicLocationV1,
    pub order: AtomicOrderV1,
    pub scope: AtomicScopeV1,
    pub coherent: bool,
    pub old_value: int,
    pub new_value: int,
    pub returned_value: int,
}

pub open spec fn valid_atomic_location_v1(location: AtomicLocationV1) -> bool {
    &&& valid_resource_v1(location.resource)
    &&& (location.width_bits == 32 || location.width_bits == 64)
    &&& location.byte_offset % (location.width_bits / 8) == 0
}

pub proof fn valid_atomic_location_has_supported_aligned_width_v1(
    location: AtomicLocationV1,
)
    requires valid_atomic_location_v1(location),
    ensures
        location.width_bits == 32 || location.width_bits == 64,
        location.byte_offset % (location.width_bits / 8) == 0,
{
}

pub open spec fn linearize_fetch_add_v1(
    location: AtomicLocationV1,
    order: AtomicOrderV1,
    scope: AtomicScopeV1,
    coherent: bool,
    old_value: int,
    operand: int,
) -> AtomicLinearizationV1 {
    AtomicLinearizationV1 {
        location,
        order,
        scope,
        coherent,
        old_value,
        new_value: old_value + operand,
        returned_value: old_value,
    }
}

pub proof fn fetch_add_linearization_retains_binding_and_returns_old_v1(
    location: AtomicLocationV1,
    order: AtomicOrderV1,
    scope: AtomicScopeV1,
    old_value: int,
    operand: int,
)
    requires valid_atomic_location_v1(location),
    ensures {
        let step = linearize_fetch_add_v1(location, order, scope, true, old_value, operand);
        &&& step.location == location
        &&& step.order == order
        &&& step.scope == scope
        &&& step.coherent
        &&& step.returned_value == old_value
        &&& step.new_value == old_value + operand
    },
{
}

#[derive(PartialEq, Eq)]
pub enum CollectivePhaseV1 {
    Gathering,
    Ready,
    Published,
}

pub struct CollectiveV1 {
    pub device: nat,
    pub epoch: nat,
    pub members: Set<nat>,
    pub arrived: Set<nat>,
    pub phase: CollectivePhaseV1,
}

pub open spec fn max_collective_members_v1() -> nat {
    256
}

pub open spec fn valid_collective_v1(collective: CollectiveV1) -> bool {
    &&& collective.device > 0
    &&& collective.epoch > 0
    &&& 0 < collective.members.len() <= max_collective_members_v1()
    &&& forall|participant: nat| collective.members.contains(participant) ==> participant > 0
    &&& collective.arrived.subset_of(collective.members)
    &&& (collective.phase == CollectivePhaseV1::Gathering ==>
        collective.arrived != collective.members)
    &&& (collective.phase != CollectivePhaseV1::Gathering ==>
        collective.arrived == collective.members)
}

pub open spec fn arrive_collective_v1(
    collective: CollectiveV1,
    participant: nat,
) -> CollectiveV1 {
    if valid_collective_v1(collective)
        && collective.phase == CollectivePhaseV1::Gathering
        && collective.members.contains(participant)
        && !collective.arrived.contains(participant)
    {
        let arrived = collective.arrived.insert(participant);
        if arrived == collective.members {
            CollectiveV1 {
                arrived,
                phase: CollectivePhaseV1::Ready,
                ..collective
            }
        } else {
            CollectiveV1 { arrived, ..collective }
        }
    } else {
        collective
    }
}

pub open spec fn publish_collective_v1(collective: CollectiveV1) -> CollectiveV1 {
    if valid_collective_v1(collective) && collective.phase == CollectivePhaseV1::Ready {
        CollectiveV1 { phase: CollectivePhaseV1::Published, ..collective }
    } else {
        collective
    }
}

pub proof fn partial_collective_arrival_cannot_publish_v1(
    collective: CollectiveV1,
    participant: nat,
)
    requires
        valid_collective_v1(collective),
        collective.phase == CollectivePhaseV1::Gathering,
        collective.members.contains(participant),
        !collective.arrived.contains(participant),
        collective.arrived.insert(participant) != collective.members,
    ensures {
        let next = arrive_collective_v1(collective, participant);
        &&& next.arrived == collective.arrived.insert(participant)
        &&& next.phase == CollectivePhaseV1::Gathering
        &&& publish_collective_v1(next) == next
    },
{
}

pub proof fn final_collective_arrival_is_ready_but_not_published_v1(
    collective: CollectiveV1,
    participant: nat,
)
    requires
        valid_collective_v1(collective),
        collective.phase == CollectivePhaseV1::Gathering,
        collective.members.contains(participant),
        !collective.arrived.contains(participant),
        collective.arrived.insert(participant) == collective.members,
    ensures {
        let next = arrive_collective_v1(collective, participant);
        &&& next.arrived == collective.members
        &&& next.phase == CollectivePhaseV1::Ready
        &&& next.device == collective.device
        &&& next.epoch == collective.epoch
    },
{
}

pub proof fn collective_publication_requires_complete_membership_v1(
    collective: CollectiveV1,
)
    requires
        valid_collective_v1(collective),
        collective.phase == CollectivePhaseV1::Ready,
    ensures {
        let published = publish_collective_v1(collective);
        &&& published.phase == CollectivePhaseV1::Published
        &&& published.members == collective.members
        &&& published.arrived == collective.members
        &&& published.device == collective.device
        &&& published.epoch == collective.epoch
    },
{
}

pub proof fn duplicate_collective_arrival_does_not_advance_v1(
    collective: CollectiveV1,
    participant: nat,
)
    requires
        valid_collective_v1(collective),
        collective.phase == CollectivePhaseV1::Gathering,
        collective.arrived.contains(participant),
    ensures arrive_collective_v1(collective, participant) == collective,
{
}

} // verus!
