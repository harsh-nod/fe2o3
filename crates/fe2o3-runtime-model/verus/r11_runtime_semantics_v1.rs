use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum CompletionStatusV1 {
    Pending,
    Succeeded,
    FailedBackend { code: int },
    Cancelled,
    QuiescentWithoutResult,
}

pub open spec fn terminal_v1(status: CompletionStatusV1) -> bool {
    status != CompletionStatusV1::Pending
}

#[derive(PartialEq, Eq)]
pub struct CompletionStateV1 {
    pub submission_id: nat,
    pub status: CompletionStatusV1,
    pub callbacks_registered: nat,
    pub callbacks_discharged: nat,
    pub callback_status: Option<CompletionStatusV1>,
    pub released: bool,
}

pub open spec fn valid_completion_state_v1(state: CompletionStateV1) -> bool {
    &&& state.submission_id > 0
    &&& state.callbacks_discharged <= state.callbacks_registered
    &&& if terminal_v1(state.status) {
        state.callbacks_discharged == state.callbacks_registered
            && if state.callbacks_registered == 0 {
                state.callback_status.is_none()
            } else {
                state.callback_status == Some(state.status)
            }
    } else {
        state.callbacks_discharged == 0 && state.callback_status.is_none() && !state.released
    }
    &&& state.released ==> terminal_v1(state.status)
}

pub open spec fn complete_v1(
    state: CompletionStateV1,
    observed: CompletionStatusV1,
) -> CompletionStateV1 {
    if state.status == CompletionStatusV1::Pending && terminal_v1(observed) {
        CompletionStateV1 {
            status: observed,
            callbacks_discharged: state.callbacks_registered,
            callback_status: if state.callbacks_registered == 0 { None } else { Some(observed) },
            ..state
        }
    } else {
        state
    }
}

pub struct EventV1 {
    pub event_id: nat,
    pub source_submission_id: nat,
}

pub open spec fn event_query_v1(
    event: EventV1,
    state: CompletionStateV1,
) -> CompletionStatusV1 {
    if event.event_id > 0 && event.source_submission_id == state.submission_id {
        state.status
    } else {
        CompletionStatusV1::Pending
    }
}

pub open spec fn submission_releasable_v1(
    state: CompletionStateV1,
    event_live: bool,
) -> bool {
    &&& terminal_v1(state.status)
    &&& state.callbacks_discharged == state.callbacks_registered
    &&& !event_live
}

pub proof fn event_query_aliases_pending_submission_v1(event: EventV1, state: CompletionStateV1)
    requires
        event.event_id > 0,
        event.source_submission_id == state.submission_id,
        state.status == CompletionStatusV1::Pending,
    ensures event_query_v1(event, state) == CompletionStatusV1::Pending,
{
}

pub proof fn event_and_submission_share_conclusive_status_v1(
    event: EventV1,
    state: CompletionStateV1,
    observed: CompletionStatusV1,
)
    requires
        event.event_id > 0,
        event.source_submission_id == state.submission_id,
        state.status == CompletionStatusV1::Pending,
        terminal_v1(observed),
    ensures {
        let completed = complete_v1(state, observed);
        &&& completed.status == observed
        &&& event_query_v1(event, completed) == observed
    },
{
}

pub proof fn completion_discharges_every_registered_callback_once_v1(
    state: CompletionStateV1,
    observed: CompletionStatusV1,
)
    requires
        valid_completion_state_v1(state),
        state.status == CompletionStatusV1::Pending,
        state.callbacks_registered > 0,
        terminal_v1(observed),
    ensures {
        let completed = complete_v1(state, observed);
        &&& completed.callbacks_discharged == state.callbacks_registered
        &&& completed.callback_status == Some(observed)
        &&& valid_completion_state_v1(completed)
    },
{
}

pub proof fn repeated_completion_cannot_redischarge_callbacks_v1(
    state: CompletionStateV1,
    first: CompletionStatusV1,
    second: CompletionStatusV1,
)
    requires
        state.status == CompletionStatusV1::Pending,
        terminal_v1(first),
        terminal_v1(second),
    ensures {
        let completed = complete_v1(state, first);
        &&& complete_v1(completed, second) == completed
        &&& completed.callbacks_discharged == state.callbacks_registered
    },
{
}

pub proof fn live_event_blocks_submission_release_v1(state: CompletionStateV1)
    requires terminal_v1(state.status),
    ensures !submission_releasable_v1(state, true),
{
}

#[derive(PartialEq, Eq)]
pub struct GeometryV1 {
    pub grid_x: nat,
    pub grid_y: nat,
    pub grid_z: nat,
    pub group_x: nat,
    pub group_y: nat,
    pub group_z: nat,
}

pub open spec fn valid_geometry_v1(geometry: GeometryV1) -> bool {
    &&& geometry.grid_x > 0 && geometry.grid_y > 0 && geometry.grid_z > 0
    &&& geometry.group_x > 0 && geometry.group_y > 0 && geometry.group_z > 0
}

pub open spec fn complete_workgroup_geometry_v1(geometry: GeometryV1) -> bool {
    &&& valid_geometry_v1(geometry)
    &&& geometry.grid_x >= geometry.group_x
    &&& geometry.grid_y >= geometry.group_y
    &&& geometry.grid_z >= geometry.group_z
    &&& geometry.grid_x % geometry.group_x == 0
    &&& geometry.grid_y % geometry.group_y == 0
    &&& geometry.grid_z % geometry.group_z == 0
}

#[derive(PartialEq, Eq)]
pub enum AtomicOperationV1 {
    Add,
    Minimum,
    Maximum,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Exchange,
    CompareExchange,
}

#[derive(PartialEq, Eq)]
pub enum MemoryOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(PartialEq, Eq)]
pub struct AtomicContractV1 {
    pub operation: AtomicOperationV1,
    pub scope: nat,
    pub order: MemoryOrderV1,
    pub failure_order: Option<MemoryOrderV1>,
    pub weak: bool,
    pub geometry: GeometryV1,
}

pub open spec fn valid_compare_exchange_order_pair_v1(
    success: MemoryOrderV1,
    failure: MemoryOrderV1,
) -> bool {
    match success {
        MemoryOrderV1::Relaxed => failure == MemoryOrderV1::Relaxed,
        MemoryOrderV1::Acquire =>
            failure == MemoryOrderV1::Relaxed || failure == MemoryOrderV1::Acquire,
        MemoryOrderV1::Release => failure == MemoryOrderV1::Relaxed,
        MemoryOrderV1::AcquireRelease =>
            failure == MemoryOrderV1::Relaxed || failure == MemoryOrderV1::Acquire,
        MemoryOrderV1::SequentiallyConsistent =>
            failure == MemoryOrderV1::Relaxed
                || failure == MemoryOrderV1::Acquire
                || failure == MemoryOrderV1::SequentiallyConsistent,
    }
}

pub open spec fn atomic_contract_is_legal_v1(contract: AtomicContractV1) -> bool {
    if contract.operation == AtomicOperationV1::CompareExchange {
        match contract.failure_order {
            Some(failure) => valid_compare_exchange_order_pair_v1(contract.order, failure),
            None => false,
        }
    } else {
        contract.failure_order.is_none() && !contract.weak
    }
}

pub open spec fn atomic_launch_admitted_v1(
    declared: AtomicContractV1,
    requested: AtomicContractV1,
    stable_capability: bool,
    execution_capability: bool,
) -> bool {
    stable_capability && execution_capability
        && declared == requested
        && valid_geometry_v1(requested.geometry)
        && atomic_contract_is_legal_v1(requested)
}

pub proof fn matching_atomic_contract_with_capabilities_is_admitted_v1(
    contract: AtomicContractV1,
)
    requires
        valid_geometry_v1(contract.geometry),
        atomic_contract_is_legal_v1(contract),
    ensures atomic_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn legal_compare_exchange_contract_is_admitted_v1(
    contract: AtomicContractV1,
    failure: MemoryOrderV1,
)
    requires
        valid_geometry_v1(contract.geometry),
        contract.operation == AtomicOperationV1::CompareExchange,
        contract.failure_order == Some(failure),
        valid_compare_exchange_order_pair_v1(contract.order, failure),
    ensures atomic_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn illegal_compare_exchange_failure_order_is_rejected_v1(
    contract: AtomicContractV1,
    failure: MemoryOrderV1,
)
    requires
        contract.operation == AtomicOperationV1::CompareExchange,
        contract.failure_order == Some(failure),
        !valid_compare_exchange_order_pair_v1(contract.order, failure),
    ensures !atomic_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn non_compare_exchange_controls_are_rejected_v1(contract: AtomicContractV1)
    requires
        contract.operation != AtomicOperationV1::CompareExchange,
        contract.failure_order.is_some() || contract.weak,
    ensures !atomic_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn substituted_atomic_label_is_rejected_v1(
    declared: AtomicContractV1,
    requested: AtomicContractV1,
)
    requires declared != requested,
    ensures !atomic_launch_admitted_v1(declared, requested, true, true),
{
}

pub proof fn absent_atomic_execution_capability_fails_closed_v1(
    declared: AtomicContractV1,
    requested: AtomicContractV1,
)
    ensures !atomic_launch_admitted_v1(declared, requested, true, false),
{
}

#[derive(PartialEq, Eq)]
pub enum CollectiveScopeV1 {
    Workgroup,
    Device,
    System,
}

#[derive(PartialEq, Eq)]
pub struct CollectiveContractV1 {
    pub operation: nat,
    pub scope: CollectiveScopeV1,
    pub order: nat,
    pub participants: nat,
    pub geometry: GeometryV1,
}

pub open spec fn expected_participants_v1(contract: CollectiveContractV1) -> nat {
    match contract.scope {
        CollectiveScopeV1::Workgroup =>
            contract.geometry.group_x * contract.geometry.group_y * contract.geometry.group_z,
        CollectiveScopeV1::Device =>
            contract.geometry.grid_x * contract.geometry.grid_y * contract.geometry.grid_z,
        CollectiveScopeV1::System => 0,
    }
}

pub open spec fn collective_launch_admitted_v1(
    declared: CollectiveContractV1,
    requested: CollectiveContractV1,
    stable_capability: bool,
    execution_capability: bool,
) -> bool {
    &&& stable_capability
    &&& execution_capability
    &&& declared == requested
    &&& complete_workgroup_geometry_v1(requested.geometry)
    &&& requested.scope != CollectiveScopeV1::System
    &&& requested.participants > 0
    &&& requested.participants == expected_participants_v1(requested)
}

pub proof fn matching_workgroup_collective_geometry_is_admitted_v1(
    contract: CollectiveContractV1,
)
    requires
        complete_workgroup_geometry_v1(contract.geometry),
        contract.scope == CollectiveScopeV1::Workgroup,
        contract.participants > 0,
        contract.participants
            == contract.geometry.group_x * contract.geometry.group_y * contract.geometry.group_z,
    ensures collective_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn collective_membership_mismatch_is_rejected_v1(contract: CollectiveContractV1)
    requires contract.participants != expected_participants_v1(contract),
    ensures !collective_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn partial_tail_collective_geometry_is_rejected_v1(contract: CollectiveContractV1)
    requires
        contract.geometry.group_x > 0,
        contract.geometry.grid_x >= contract.geometry.group_x,
        contract.geometry.grid_x % contract.geometry.group_x != 0,
    ensures !collective_launch_admitted_v1(contract, contract, true, true),
{
}

pub proof fn system_collective_is_rejected_by_single_launch_v1(contract: CollectiveContractV1)
    requires contract.scope == CollectiveScopeV1::System,
    ensures !collective_launch_admitted_v1(contract, contract, true, true),
{
}

#[derive(PartialEq, Eq)]
pub enum MappingPhaseV1 {
    Active,
    RetainedByBatch { batch_id: nat },
    Quarantined,
    Released,
}

#[derive(PartialEq, Eq)]
pub struct PersistentMappingV1 {
    pub mapping_id: nat,
    pub generation: nat,
    pub phase: MappingPhaseV1,
}

pub open spec fn retain_for_batch_v1(
    mapping: PersistentMappingV1,
    batch_id: nat,
) -> PersistentMappingV1 {
    if mapping.mapping_id > 0 && mapping.generation > 0
        && mapping.phase == MappingPhaseV1::Active && batch_id > 0
    {
        PersistentMappingV1 { phase: MappingPhaseV1::RetainedByBatch { batch_id }, ..mapping }
    } else {
        mapping
    }
}

pub open spec fn finish_batch_v1(
    mapping: PersistentMappingV1,
    batch_id: nat,
    conclusive: bool,
) -> PersistentMappingV1 {
    if mapping.phase == (MappingPhaseV1::RetainedByBatch { batch_id }) {
        PersistentMappingV1 {
            phase: if conclusive { MappingPhaseV1::Active } else { MappingPhaseV1::Quarantined },
            ..mapping
        }
    } else {
        mapping
    }
}

pub open spec fn mapping_releasable_v1(mapping: PersistentMappingV1) -> bool {
    mapping.phase == MappingPhaseV1::Active
}

pub proof fn active_mapping_is_retained_for_exact_batch_v1(
    mapping: PersistentMappingV1,
    batch_id: nat,
)
    requires
        mapping.mapping_id > 0,
        mapping.generation > 0,
        mapping.phase == MappingPhaseV1::Active,
        batch_id > 0,
    ensures {
        let retained = retain_for_batch_v1(mapping, batch_id);
        &&& retained.mapping_id == mapping.mapping_id
        &&& retained.generation == mapping.generation
        &&& retained.phase == (MappingPhaseV1::RetainedByBatch { batch_id })
        &&& !mapping_releasable_v1(retained)
    },
{
}

pub proof fn conclusive_batch_restores_same_persistent_mapping_v1(
    mapping: PersistentMappingV1,
    batch_id: nat,
)
    requires mapping.phase == (MappingPhaseV1::RetainedByBatch { batch_id }),
    ensures {
        let completed = finish_batch_v1(mapping, batch_id, true);
        &&& completed.mapping_id == mapping.mapping_id
        &&& completed.generation == mapping.generation
        &&& completed.phase == MappingPhaseV1::Active
        &&& mapping_releasable_v1(completed)
    },
{
}

pub proof fn indeterminate_batch_quarantines_mapping_and_blocks_release_v1(
    mapping: PersistentMappingV1,
    batch_id: nat,
)
    requires mapping.phase == (MappingPhaseV1::RetainedByBatch { batch_id }),
    ensures {
        let quarantined = finish_batch_v1(mapping, batch_id, false);
        &&& quarantined.mapping_id == mapping.mapping_id
        &&& quarantined.generation == mapping.generation
        &&& quarantined.phase == MappingPhaseV1::Quarantined
        &&& !mapping_releasable_v1(quarantined)
    },
{
}

} // verus!
