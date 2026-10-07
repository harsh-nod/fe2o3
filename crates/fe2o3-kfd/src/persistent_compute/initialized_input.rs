//! Initialized persistent compute observations and settled frontiers.

use super::*;

impl Gfx942PersistentComputeReadyV1 {
    pub const fn byte_len(&self) -> u64 {
        self.allocation.byte_len()
    }

    pub const fn physical_byte_len(&self) -> u64 {
        self.allocation.physical_byte_len()
    }

    pub const fn authenticated_sha256(&self) -> [u8; 32] {
        self.authenticated_sha256
    }

    pub fn into_allocation(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        self.allocation
    }
}

impl Gfx942PersistentComputeInitializedAfterDispatchV1 {
    pub const fn byte_len(&self) -> u64 {
        self.allocation.byte_len()
    }

    pub const fn physical_byte_len(&self) -> u64 {
        self.allocation.physical_byte_len()
    }

    pub fn into_allocation(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        self.allocation
    }
}

impl Gfx942PersistentComputeInitializedStorageV1 {
    pub const fn byte_len(&self) -> u64 {
        self.allocation.byte_len()
    }

    pub const fn physical_byte_len(&self) -> u64 {
        self.allocation.physical_byte_len()
    }

    pub fn into_allocation(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        self.allocation
    }
}

impl Gfx942ThreeBindingPersistentComputeCompletedV1 {
    pub fn into_completed(self) -> [Gfx942PersistentComputeCompletedV1; 3] {
        self.completed
    }

    #[allow(clippy::result_large_err)]
    pub fn retire_settled_frontiers_for_replay_v1(
        self,
    ) -> Result<
        [(
            Gfx942PersistentComputeInputV1,
            Gfx942PersistentComputeEffectV1,
        ); 3],
        Self,
    > {
        if self.completed.iter().any(|completed| {
            completed
                .allocation
                .owner
                .preflight_retire_settled_frontier(&completed.frontier)
                .is_err()
        }) {
            return Err(self);
        }
        Ok(self.completed.map(|completed| {
            completed
                .retire_settled_frontier_for_replay_v1()
                .unwrap_or_else(|_| unreachable!("preflighted three-binding frontier retirement"))
        }))
    }
}
