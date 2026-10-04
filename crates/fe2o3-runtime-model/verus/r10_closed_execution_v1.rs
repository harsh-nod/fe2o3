use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DeviceV1 {
    pub physical: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct StreamV1 {
    pub device: DeviceV1,
    pub identity: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct OperationKeyV1 {
    pub stream: StreamV1,
    pub sequence: nat,
}

#[derive(PartialEq, Eq)]
pub struct LeaseV1 {
    pub device: DeviceV1,
    pub pool: nat,
    pub block: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub enum OperationKindV1 {
    Compute,
    PeerCopy { source: DeviceV1, destination: DeviceV1 },
}

#[derive(PartialEq, Eq)]
pub enum OperationPhaseV1 {
    Prepared,
    Published,
    CompletionObserved,
    CancelledBeforePublication,
    Released,
    Quarantined,
}

pub struct OperationV1 {
    pub key: OperationKeyV1,
    pub execution_device: DeviceV1,
    pub kind: OperationKindV1,
    pub dependencies: Set<OperationKeyV1>,
    pub leases: Set<LeaseV1>,
    pub phase: OperationPhaseV1,
    pub batch_id: nat,
    pub publication_epoch: nat,
    pub cancellation_requested: bool,
    pub timeout_observations: nat,
}

pub open spec fn valid_device_v1(device: DeviceV1) -> bool {
    device.physical > 0 && device.generation > 0
}

pub open spec fn valid_lease_v1(lease: LeaseV1) -> bool {
    &&& valid_device_v1(lease.device)
    &&& lease.pool > 0
    &&& lease.block > 0
    &&& lease.generation > 0
}

pub open spec fn valid_operation_v1(operation: OperationV1) -> bool {
    &&& operation.key.stream.device == operation.execution_device
    &&& valid_device_v1(operation.execution_device)
    &&& operation.key.stream.identity > 0
    &&& operation.key.stream.generation > 0
    &&& operation.key.sequence > 0
    &&& 0 < operation.leases.len() <= 64
    &&& forall|lease: LeaseV1| operation.leases.contains(lease) ==> valid_lease_v1(lease)
    &&& match operation.kind {
        OperationKindV1::Compute =>
            forall|lease: LeaseV1| operation.leases.contains(lease) ==>
                lease.device == operation.execution_device,
        OperationKindV1::PeerCopy { source, destination } => {
            &&& source != destination
            &&& operation.execution_device == destination
            &&& operation.leases.len() == 2
            &&& exists|lease: LeaseV1| operation.leases.contains(lease) && lease.device == source
            &&& exists|lease: LeaseV1| operation.leases.contains(lease) && lease.device == destination
        }
    }
}

pub open spec fn dependencies_complete_v1(
    operation: OperationV1,
    completed: Set<OperationKeyV1>,
) -> bool {
    forall|dependency: OperationKeyV1|
        operation.dependencies.contains(dependency) ==> completed.contains(dependency)
}

pub open spec fn publish_operation_v1(
    operation: OperationV1,
    completed: Set<OperationKeyV1>,
    batch_id: nat,
    publication_epoch: nat,
) -> OperationV1 {
    if valid_operation_v1(operation)
        && operation.phase == OperationPhaseV1::Prepared
        && dependencies_complete_v1(operation, completed)
        && batch_id > 0
        && publication_epoch > 0
    {
        OperationV1 {
            phase: OperationPhaseV1::Published,
            batch_id,
            publication_epoch,
            ..operation
        }
    } else {
        operation
    }
}

pub proof fn incomplete_cross_stream_dependency_blocks_publication_v1(
    operation: OperationV1,
    completed: Set<OperationKeyV1>,
    dependency: OperationKeyV1,
    batch_id: nat,
    publication_epoch: nat,
)
    requires
        operation.phase == OperationPhaseV1::Prepared,
        operation.dependencies.contains(dependency),
        !completed.contains(dependency),
    ensures publish_operation_v1(operation, completed, batch_id, publication_epoch) == operation,
{
    assert(!dependencies_complete_v1(operation, completed));
}

pub proof fn published_operation_retains_exact_stream_device_dependencies_and_leases_v1(
    operation: OperationV1,
    completed: Set<OperationKeyV1>,
    batch_id: nat,
    publication_epoch: nat,
)
    requires
        valid_operation_v1(operation),
        operation.phase == OperationPhaseV1::Prepared,
        dependencies_complete_v1(operation, completed),
        batch_id > 0,
        publication_epoch > 0,
    ensures {
        let published = publish_operation_v1(operation, completed, batch_id, publication_epoch);
        &&& published.phase == OperationPhaseV1::Published
        &&& published.key == operation.key
        &&& published.execution_device == operation.execution_device
        &&& published.kind == operation.kind
        &&& published.dependencies == operation.dependencies
        &&& published.leases == operation.leases
        &&& published.batch_id == batch_id
        &&& published.publication_epoch == publication_epoch
    },
{
}

pub proof fn distinct_disjoint_operations_can_remain_in_flight_v1(
    left: OperationV1,
    right: OperationV1,
)
    requires
        valid_operation_v1(left),
        valid_operation_v1(right),
        left.phase == OperationPhaseV1::Published,
        right.phase == OperationPhaseV1::Published,
        left.key != right.key,
        left.leases.disjoint(right.leases),
    ensures
        left.phase == OperationPhaseV1::Published,
        right.phase == OperationPhaseV1::Published,
        left.leases.disjoint(right.leases),
        left.leases.len() + right.leases.len() > 1,
{
}

pub struct PreparedBatchV1 {
    pub stream: StreamV1,
    pub first_sequence: nat,
    pub operations: Seq<OperationV1>,
    pub batch_id: nat,
    pub publication_epoch: nat,
}

pub open spec fn prepared_batch_ready_v1(
    batch: PreparedBatchV1,
    completed: Set<OperationKeyV1>,
) -> bool {
    &&& batch.batch_id > 0
    &&& batch.publication_epoch > 0
    &&& batch.first_sequence > 0
    &&& 0 < batch.operations.len() <= 256
    &&& forall|index: int| 0 <= index < batch.operations.len() ==> {
        let operation = #[trigger] batch.operations[index];
        &&& valid_operation_v1(operation)
        &&& operation.phase == OperationPhaseV1::Prepared
        &&& operation.key.stream == batch.stream
        &&& operation.key.sequence == batch.first_sequence + index
        &&& dependencies_complete_v1(operation, completed)
    }
}

pub open spec fn publish_prepared_batch_v1(
    batch: PreparedBatchV1,
    completed: Set<OperationKeyV1>,
) -> PreparedBatchV1 {
    if prepared_batch_ready_v1(batch, completed) {
        PreparedBatchV1 {
            operations: Seq::new(batch.operations.len(), |index: int| {
                publish_operation_v1(
                    batch.operations[index],
                    completed,
                    batch.batch_id,
                    batch.publication_epoch,
                )
            }),
            ..batch
        }
    } else {
        batch
    }
}

pub proof fn unready_prepared_batch_has_no_partial_publication_v1(
    batch: PreparedBatchV1,
    completed: Set<OperationKeyV1>,
)
    requires !prepared_batch_ready_v1(batch, completed),
    ensures publish_prepared_batch_v1(batch, completed) == batch,
{
}

pub proof fn ready_batch_publishes_exact_roster_at_one_epoch_v1(
    batch: PreparedBatchV1,
    completed: Set<OperationKeyV1>,
)
    requires prepared_batch_ready_v1(batch, completed),
    ensures {
        let published = publish_prepared_batch_v1(batch, completed);
        &&& published.operations.len() == batch.operations.len()
        &&& published.stream == batch.stream
        &&& published.batch_id == batch.batch_id
        &&& published.publication_epoch == batch.publication_epoch
        &&& forall|index: int| 0 <= index < published.operations.len() ==> {
            let after = #[trigger] published.operations[index];
            let before = batch.operations[index];
            &&& after.phase == OperationPhaseV1::Published
            &&& after.key == before.key
            &&& after.dependencies == before.dependencies
            &&& after.leases == before.leases
            &&& after.batch_id == batch.batch_id
            &&& after.publication_epoch == batch.publication_epoch
        }
    },
{
}

#[derive(PartialEq, Eq)]
pub enum BlockPhaseV1 {
    Free,
    Leased,
    Prepared,
    Published,
    CompletionObserved,
    Quarantined,
}

#[derive(PartialEq, Eq)]
pub struct PoolBlockV1 {
    pub lease: LeaseV1,
    pub byte_len: nat,
    pub alignment: nat,
    pub phase: BlockPhaseV1,
    pub owner: Option<OperationKeyV1>,
}

pub open spec fn release_completed_block_v1(
    block: PoolBlockV1,
    operation: OperationKeyV1,
) -> PoolBlockV1 {
    if block.phase == BlockPhaseV1::CompletionObserved
        && block.owner == Some(operation)
        && valid_lease_v1(block.lease)
    {
        PoolBlockV1 {
            lease: LeaseV1 { generation: block.lease.generation + 1, ..block.lease },
            phase: BlockPhaseV1::Free,
            owner: None,
            ..block
        }
    } else {
        block
    }
}

pub proof fn completed_pool_release_advances_generation_before_reuse_v1(
    block: PoolBlockV1,
    operation: OperationKeyV1,
)
    requires
        valid_lease_v1(block.lease),
        block.phase == BlockPhaseV1::CompletionObserved,
        block.owner == Some(operation),
    ensures {
        let released = release_completed_block_v1(block, operation);
        &&& released.phase == BlockPhaseV1::Free
        &&& released.owner.is_none()
        &&& released.lease.device == block.lease.device
        &&& released.lease.pool == block.lease.pool
        &&& released.lease.block == block.lease.block
        &&& released.lease.generation == block.lease.generation + 1
        &&& released.lease != block.lease
    },
{
}

pub proof fn stale_pool_lease_cannot_equal_reused_generation_v1(
    old: LeaseV1,
    current: LeaseV1,
)
    requires
        old.device == current.device,
        old.pool == current.pool,
        old.block == current.block,
        old.generation < current.generation,
    ensures old != current,
{
}

pub proof fn peer_copy_binds_two_distinct_device_owners_v1(
    operation: OperationV1,
    source: DeviceV1,
    destination: DeviceV1,
)
    requires
        valid_operation_v1(operation),
        operation.kind == (OperationKindV1::PeerCopy { source, destination }),
    ensures
        source != destination,
        operation.execution_device == destination,
        operation.leases.len() == 2,
        exists|lease: LeaseV1| operation.leases.contains(lease) && lease.device == source,
        exists|lease: LeaseV1| operation.leases.contains(lease) && lease.device == destination,
{
}

pub open spec fn request_cancellation_v1(operation: OperationV1) -> OperationV1 {
    if operation.phase == OperationPhaseV1::Published {
        OperationV1 { cancellation_requested: true, ..operation }
    } else {
        operation
    }
}

pub open spec fn observe_timeout_v1(operation: OperationV1) -> OperationV1 {
    if operation.phase == OperationPhaseV1::Published {
        OperationV1 { timeout_observations: operation.timeout_observations + 1, ..operation }
    } else {
        operation
    }
}

pub proof fn published_cancel_and_timeout_retain_exact_leases_v1(operation: OperationV1)
    requires operation.phase == OperationPhaseV1::Published,
    ensures {
        let cancelled = request_cancellation_v1(operation);
        let timed_out = observe_timeout_v1(cancelled);
        &&& cancelled.phase == OperationPhaseV1::Published
        &&& cancelled.leases == operation.leases
        &&& cancelled.cancellation_requested
        &&& timed_out.phase == OperationPhaseV1::Published
        &&& timed_out.leases == operation.leases
        &&& timed_out.timeout_observations == operation.timeout_observations + 1
    },
{
}

pub open spec fn quarantine_published_v1(operation: OperationV1) -> OperationV1 {
    if operation.phase == OperationPhaseV1::Published {
        OperationV1 { phase: OperationPhaseV1::Quarantined, ..operation }
    } else {
        operation
    }
}

pub open spec fn operation_releasable_v1(operation: OperationV1) -> bool {
    operation.phase == OperationPhaseV1::CompletionObserved
}

pub proof fn indeterminate_failure_retains_leases_and_blocks_release_v1(operation: OperationV1)
    requires operation.phase == OperationPhaseV1::Published,
    ensures {
        let quarantined = quarantine_published_v1(operation);
        &&& quarantined.phase == OperationPhaseV1::Quarantined
        &&& quarantined.leases == operation.leases
        &&& !operation_releasable_v1(quarantined)
    },
{
}

#[derive(PartialEq, Eq)]
pub enum AtomicOperationV1 {
    Load,
    Store,
    Exchange,
    FetchAdd,
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
    Device,
    System,
}

#[derive(PartialEq, Eq)]
pub struct FencePlanV1 {
    pub pre_release: bool,
    pub post_acquire: bool,
    pub sequentially_consistent: bool,
}

#[derive(PartialEq, Eq)]
pub struct AtomicStepV1 {
    pub operation: AtomicOperationV1,
    pub declared_order: AtomicOrderV1,
    pub declared_scope: AtomicScopeV1,
    pub observed_operation: AtomicOperationV1,
    pub observed_order: AtomicOrderV1,
    pub observed_scope: AtomicScopeV1,
    pub fences: FencePlanV1,
    pub old_value: int,
    pub operand: int,
    pub new_value: int,
    pub returned_value: Option<int>,
}

pub open spec fn fences_for_v1(order: AtomicOrderV1) -> FencePlanV1 {
    FencePlanV1 {
        pre_release: order == AtomicOrderV1::Release
            || order == AtomicOrderV1::AcquireRelease
            || order == AtomicOrderV1::SequentiallyConsistent,
        post_acquire: order == AtomicOrderV1::Acquire
            || order == AtomicOrderV1::AcquireRelease
            || order == AtomicOrderV1::SequentiallyConsistent,
        sequentially_consistent: order == AtomicOrderV1::SequentiallyConsistent,
    }
}

pub open spec fn valid_atomic_order_v1(operation: AtomicOperationV1, order: AtomicOrderV1) -> bool {
    match operation {
        AtomicOperationV1::Load => order == AtomicOrderV1::Relaxed
            || order == AtomicOrderV1::Acquire
            || order == AtomicOrderV1::SequentiallyConsistent,
        AtomicOperationV1::Store => order == AtomicOrderV1::Relaxed
            || order == AtomicOrderV1::Release
            || order == AtomicOrderV1::SequentiallyConsistent,
        AtomicOperationV1::Exchange | AtomicOperationV1::FetchAdd => true,
    }
}

pub open spec fn atomic_step_corresponds_v1(step: AtomicStepV1) -> bool {
    &&& valid_atomic_order_v1(step.operation, step.declared_order)
    &&& step.observed_operation == step.operation
    &&& step.observed_order == step.declared_order
    &&& step.observed_scope == step.declared_scope
    &&& step.fences == fences_for_v1(step.declared_order)
    &&& match step.operation {
        AtomicOperationV1::Load =>
            step.new_value == step.old_value && step.returned_value == Some(step.old_value),
        AtomicOperationV1::Store =>
            step.new_value == step.operand && step.returned_value.is_none(),
        AtomicOperationV1::Exchange =>
            step.new_value == step.operand && step.returned_value == Some(step.old_value),
        AtomicOperationV1::FetchAdd =>
            step.new_value == step.old_value + step.operand
                && step.returned_value == Some(step.old_value),
    }
}

pub proof fn corresponding_atomic_load_binds_order_scope_fence_and_value_v1(step: AtomicStepV1)
    requires
        atomic_step_corresponds_v1(step),
        step.operation == AtomicOperationV1::Load,
    ensures
        step.observed_order == step.declared_order,
        step.observed_scope == step.declared_scope,
        step.fences == fences_for_v1(step.declared_order),
        step.new_value == step.old_value,
        step.returned_value == Some(step.old_value),
{
}

pub proof fn corresponding_atomic_store_binds_order_scope_fence_and_value_v1(step: AtomicStepV1)
    requires
        atomic_step_corresponds_v1(step),
        step.operation == AtomicOperationV1::Store,
    ensures
        step.observed_order == step.declared_order,
        step.observed_scope == step.declared_scope,
        step.fences == fences_for_v1(step.declared_order),
        step.new_value == step.operand,
        step.returned_value.is_none(),
{
}

pub proof fn corresponding_atomic_rmw_binds_order_scope_fence_and_old_value_v1(
    step: AtomicStepV1,
)
    requires
        atomic_step_corresponds_v1(step),
        step.operation == AtomicOperationV1::Exchange
            || step.operation == AtomicOperationV1::FetchAdd,
    ensures
        step.observed_operation == step.operation,
        step.observed_order == step.declared_order,
        step.observed_scope == step.declared_scope,
        step.fences == fences_for_v1(step.declared_order),
        step.returned_value == Some(step.old_value),
{
}

pub proof fn substituted_atomic_scope_never_corresponds_v1(step: AtomicStepV1)
    requires step.observed_scope != step.declared_scope,
    ensures !atomic_step_corresponds_v1(step),
{
}

#[derive(PartialEq, Eq)]
pub enum WaveOperationV1 {
    Barrier,
    ReduceSum,
    InclusiveScanSum,
    ExclusiveScanSum,
}

#[derive(PartialEq, Eq)]
pub enum WavePhaseV1 {
    Gathering,
    Ready,
    Published,
}

pub struct WaveV1 {
    pub operation: WaveOperationV1,
    pub convergent: bool,
    pub physical_lanes: Set<nat>,
    pub arrivals: Set<nat>,
    pub inputs: Seq<int>,
    pub outputs: Seq<int>,
    pub phase: WavePhaseV1,
}

pub open spec fn exact_wave64_lanes_v1(lanes: Set<nat>) -> bool {
    forall|lane: nat| lanes.contains(lane) <==> lane < 64
}

pub open spec fn sum_prefix_v1(inputs: Seq<int>, count: nat) -> int
    decreases count,
{
    if count == 0 {
        0
    } else {
        sum_prefix_v1(inputs, (count - 1) as nat) + inputs[(count - 1) as int]
    }
}

pub open spec fn wave_outputs_v1(operation: WaveOperationV1, inputs: Seq<int>) -> Seq<int> {
    match operation {
        WaveOperationV1::Barrier => inputs,
        WaveOperationV1::ReduceSum =>
            Seq::new(64, |index: int| sum_prefix_v1(inputs, 64)),
        WaveOperationV1::InclusiveScanSum =>
            Seq::new(64, |index: int| sum_prefix_v1(inputs, (index + 1) as nat)),
        WaveOperationV1::ExclusiveScanSum =>
            Seq::new(64, |index: int| sum_prefix_v1(inputs, index as nat)),
    }
}

pub open spec fn wave_ready_v1(wave: WaveV1) -> bool {
    &&& wave.convergent
    &&& exact_wave64_lanes_v1(wave.physical_lanes)
    &&& wave.inputs.len() == 64
    &&& wave.arrivals == wave.physical_lanes
    &&& wave.phase == WavePhaseV1::Ready
}

pub open spec fn publish_wave_v1(wave: WaveV1) -> WaveV1 {
    if wave_ready_v1(wave) {
        WaveV1 {
            outputs: wave_outputs_v1(wave.operation, wave.inputs),
            phase: WavePhaseV1::Published,
            ..wave
        }
    } else {
        wave
    }
}

pub proof fn incomplete_or_divergent_wave64_cannot_publish_v1(wave: WaveV1)
    requires
        !wave.convergent || wave.arrivals != wave.physical_lanes,
        wave.phase == WavePhaseV1::Gathering,
    ensures publish_wave_v1(wave) == wave,
{
}

pub proof fn converged_wave64_barrier_publishes_exact_inputs_v1(wave: WaveV1)
    requires
        wave_ready_v1(wave),
        wave.operation == WaveOperationV1::Barrier,
    ensures {
        let published = publish_wave_v1(wave);
        &&& published.phase == WavePhaseV1::Published
        &&& published.outputs == wave.inputs
        &&& published.outputs.len() == 64
    },
{
}

pub proof fn converged_wave64_reduction_publishes_exact_sum_v1(wave: WaveV1)
    requires
        wave_ready_v1(wave),
        wave.operation == WaveOperationV1::ReduceSum,
    ensures {
        let published = publish_wave_v1(wave);
        &&& published.phase == WavePhaseV1::Published
        &&& published.outputs.len() == 64
        &&& forall|lane: int| 0 <= lane < 64 ==>
            #[trigger] published.outputs[lane] == sum_prefix_v1(wave.inputs, 64)
    },
{
}

pub proof fn converged_wave64_inclusive_scan_publishes_exact_prefixes_v1(wave: WaveV1)
    requires
        wave_ready_v1(wave),
        wave.operation == WaveOperationV1::InclusiveScanSum,
    ensures {
        let published = publish_wave_v1(wave);
        &&& published.phase == WavePhaseV1::Published
        &&& published.outputs.len() == 64
        &&& forall|lane: int| 0 <= lane < 64 ==>
            #[trigger] published.outputs[lane]
                == sum_prefix_v1(wave.inputs, (lane + 1) as nat)
    },
{
}

pub proof fn converged_wave64_exclusive_scan_publishes_exact_prefixes_v1(wave: WaveV1)
    requires
        wave_ready_v1(wave),
        wave.operation == WaveOperationV1::ExclusiveScanSum,
    ensures {
        let published = publish_wave_v1(wave);
        &&& published.phase == WavePhaseV1::Published
        &&& published.outputs.len() == 64
        &&& forall|lane: int| 0 <= lane < 64 ==>
            #[trigger] published.outputs[lane] == sum_prefix_v1(wave.inputs, lane as nat)
    },
{
}

} // verus!
