use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct DeviceKeyV1 {
    pub physical: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct PoolKeyV1 {
    pub identity: nat,
    pub device: DeviceKeyV1,
}

#[derive(PartialEq, Eq)]
pub struct LeaseKeyV1 {
    pub pool: PoolKeyV1,
    pub block: nat,
    pub generation: nat,
}

#[derive(PartialEq, Eq)]
pub enum BlockPhaseV1 {
    Free,
    Leased,
    InFlight,
    CompletionObserved,
    Quarantined,
}

#[derive(PartialEq, Eq)]
pub struct BlockV1 {
    pub key: LeaseKeyV1,
    pub bytes: nat,
    pub phase: BlockPhaseV1,
}

pub open spec fn reusable_v1(phase: BlockPhaseV1) -> bool {
    phase == BlockPhaseV1::Free
}

pub open spec fn retained_v1(phase: BlockPhaseV1) -> bool {
    phase != BlockPhaseV1::Free
}

pub open spec fn same_storage_v1(left: LeaseKeyV1, right: LeaseKeyV1) -> bool {
    &&& left.pool == right.pool
    &&& left.block == right.block
}

pub open spec fn valid_block_v1(block: BlockV1) -> bool {
    &&& block.key.pool.identity > 0
    &&& block.key.pool.device.physical > 0
    &&& block.key.pool.device.generation > 0
    &&& block.key.block > 0
    &&& block.key.generation > 0
    &&& block.bytes > 0
}

pub open spec fn blocks_invariant_v1(blocks: Seq<BlockV1>) -> bool {
    &&& forall|index: int| 0 <= index < blocks.len() ==>
        valid_block_v1(#[trigger] blocks[index])
    &&& forall|left: int, right: int|
        0 <= left < right < blocks.len() ==>
            !same_storage_v1(#[trigger] blocks[left].key, #[trigger] blocks[right].key)
}

pub open spec fn lease_matches_block_v1(lease: LeaseKeyV1, block: BlockV1) -> bool {
    lease == block.key
}

pub open spec fn can_lease_v1(block: BlockV1, bytes: nat) -> bool {
    valid_block_v1(block) && reusable_v1(block.phase) && bytes > 0 && bytes <= block.bytes
}

pub open spec fn lease_block_v1(block: BlockV1) -> BlockV1 {
    BlockV1 { key: block.key, bytes: block.bytes, phase: BlockPhaseV1::Leased }
}

pub open spec fn submit_block_v1(block: BlockV1, lease: LeaseKeyV1) -> BlockV1 {
    if lease_matches_block_v1(lease, block) && block.phase == BlockPhaseV1::Leased {
        BlockV1 { key: block.key, bytes: block.bytes, phase: BlockPhaseV1::InFlight }
    } else {
        block
    }
}

pub open spec fn complete_block_v1(block: BlockV1, lease: LeaseKeyV1) -> BlockV1 {
    if lease_matches_block_v1(lease, block) && block.phase == BlockPhaseV1::InFlight {
        BlockV1 {
            key: block.key,
            bytes: block.bytes,
            phase: BlockPhaseV1::CompletionObserved,
        }
    } else {
        block
    }
}

pub open spec fn release_block_v1(block: BlockV1, lease: LeaseKeyV1) -> BlockV1 {
    if lease_matches_block_v1(lease, block)
        && (block.phase == BlockPhaseV1::Leased
            || block.phase == BlockPhaseV1::CompletionObserved)
    {
        BlockV1 {
            key: LeaseKeyV1 {
                pool: block.key.pool,
                block: block.key.block,
                generation: block.key.generation + 1,
            },
            bytes: block.bytes,
            phase: BlockPhaseV1::Free,
        }
    } else {
        block
    }
}

pub proof fn lease_preserves_exact_storage_generation_v1(block: BlockV1, bytes: nat)
    requires can_lease_v1(block, bytes),
    ensures
        lease_block_v1(block).key == block.key,
        lease_block_v1(block).bytes == block.bytes,
        lease_block_v1(block).phase == BlockPhaseV1::Leased,
        retained_v1(lease_block_v1(block).phase),
{
}

pub proof fn submitted_storage_is_not_reusable_v1(block: BlockV1, lease: LeaseKeyV1)
    requires
        valid_block_v1(block),
        block.phase == BlockPhaseV1::Leased,
        lease_matches_block_v1(lease, block),
    ensures
        submit_block_v1(block, lease).phase == BlockPhaseV1::InFlight,
        !reusable_v1(submit_block_v1(block, lease).phase),
        submit_block_v1(block, lease).key == lease,
{
}

pub proof fn completion_then_release_advances_generation_v1(
    block: BlockV1,
    lease: LeaseKeyV1,
)
    requires
        valid_block_v1(block),
        block.phase == BlockPhaseV1::InFlight,
        lease_matches_block_v1(lease, block),
    ensures {
        let completed = complete_block_v1(block, lease);
        let released = release_block_v1(completed, lease);
        &&& completed.phase == BlockPhaseV1::CompletionObserved
        &&& released.phase == BlockPhaseV1::Free
        &&& released.key.pool == lease.pool
        &&& released.key.block == lease.block
        &&& released.key.generation == lease.generation + 1
        &&& !lease_matches_block_v1(lease, released)
    },
{
}

pub proof fn stale_generation_cannot_submit_reused_block_v1(
    free: BlockV1,
    stale: LeaseKeyV1,
)
    requires
        valid_block_v1(free),
        free.phase == BlockPhaseV1::Free,
        same_storage_v1(stale, free.key),
        stale.generation < free.key.generation,
    ensures
        !lease_matches_block_v1(stale, free),
        submit_block_v1(free, stale) == free,
{
}

pub proof fn distinct_retained_blocks_have_distinct_storage_v1(
    blocks: Seq<BlockV1>,
    left: int,
    right: int,
)
    requires
        blocks_invariant_v1(blocks),
        0 <= left < right < blocks.len(),
        retained_v1(blocks[left].phase),
        retained_v1(blocks[right].phase),
    ensures !same_storage_v1(blocks[left].key, blocks[right].key),
{
}

#[derive(PartialEq, Eq)]
pub enum CopyPhaseV1 {
    Reserved,
    Submitted,
    VisibilityObserved,
    Quarantined,
    Released,
}

#[derive(PartialEq, Eq)]
pub struct CopyV1 {
    pub source: LeaseKeyV1,
    pub destination: LeaseKeyV1,
    pub execution_device: DeviceKeyV1,
    pub dependency_frontier: nat,
    pub phase: CopyPhaseV1,
}

pub open spec fn valid_peer_copy_v1(copy: CopyV1) -> bool {
    &&& copy.source.pool.device != copy.destination.pool.device
    &&& copy.execution_device == copy.destination.pool.device
    &&& !same_storage_v1(copy.source, copy.destination)
    &&& copy.source.generation > 0
    &&& copy.destination.generation > 0
}

pub open spec fn can_publish_copy_v1(copy: CopyV1, completed_frontier: nat) -> bool {
    &&& valid_peer_copy_v1(copy)
    &&& copy.phase == CopyPhaseV1::Reserved
    &&& completed_frontier >= copy.dependency_frontier
}

pub open spec fn publish_copy_v1(copy: CopyV1, completed_frontier: nat) -> CopyV1 {
    if can_publish_copy_v1(copy, completed_frontier) {
        CopyV1 { phase: CopyPhaseV1::Submitted, ..copy }
    } else {
        copy
    }
}

pub proof fn peer_copy_retains_exact_device_coordinates_v1(
    copy: CopyV1,
    completed_frontier: nat,
)
    requires can_publish_copy_v1(copy, completed_frontier),
    ensures
        publish_copy_v1(copy, completed_frontier).source == copy.source,
        publish_copy_v1(copy, completed_frontier).destination == copy.destination,
        publish_copy_v1(copy, completed_frontier).execution_device
            == copy.destination.pool.device,
        publish_copy_v1(copy, completed_frontier).phase == CopyPhaseV1::Submitted,
{
}

pub proof fn incomplete_dependency_frontier_blocks_copy_publication_v1(
    copy: CopyV1,
    completed_frontier: nat,
)
    requires
        valid_peer_copy_v1(copy),
        copy.phase == CopyPhaseV1::Reserved,
        completed_frontier < copy.dependency_frontier,
    ensures publish_copy_v1(copy, completed_frontier) == copy,
{
}

pub proof fn quarantined_storage_is_never_reusable_v1(block: BlockV1)
    requires valid_block_v1(block), block.phase == BlockPhaseV1::Quarantined,
    ensures !reusable_v1(block.phase), retained_v1(block.phase),
{
}

} // verus!
