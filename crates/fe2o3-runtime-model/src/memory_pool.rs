//! Bounded reusable allocation-pool state machine.
//!
//! The pool owns no native allocation. It models generation-safe best-fit
//! reuse and the rule that storage cannot return to the free set until every
//! device reference is conclusively quiescent. A concrete backend must bind
//! each block to sealed native allocation and completion identities.

use alloc::vec::Vec;

use crate::{DeviceKeyV1, IDENTITY_DIGEST_BYTES_V1, IdentityDigestV1, MemoryKindV1};

pub const MEMORY_POOL_MODEL_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_MEMORY_POOL_BLOCKS_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPoolBlockPhaseV1 {
    Free,
    Leased,
    InFlight,
    CompletionObserved,
    Quarantined,
}

impl MemoryPoolBlockPhaseV1 {
    pub const fn reusable(self) -> bool {
        matches!(self, Self::Free)
    }

    pub const fn blocks_reuse(self) -> bool {
        !matches!(self, Self::Free)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryPoolLeaseIdentityV1 {
    pool: IdentityDigestV1,
    device: DeviceKeyV1,
    block_id: u64,
    generation: u64,
}

impl MemoryPoolLeaseIdentityV1 {
    pub const fn pool(self) -> IdentityDigestV1 {
        self.pool
    }

    pub const fn device(self) -> DeviceKeyV1 {
        self.device
    }

    pub const fn block_id(self) -> u64 {
        self.block_id
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Debug)]
#[must_use = "a pool lease must be released after quiescence or quarantined"]
pub struct MemoryPoolLeaseV1 {
    identity: MemoryPoolLeaseIdentityV1,
    requested_bytes: u64,
    alignment: u64,
}

impl MemoryPoolLeaseV1 {
    pub const fn identity(&self) -> MemoryPoolLeaseIdentityV1 {
        self.identity
    }

    pub const fn requested_bytes(&self) -> u64 {
        self.requested_bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryPoolBlockRecordV1 {
    block_id: u64,
    generation: u64,
    byte_len: u64,
    alignment: u64,
    phase: MemoryPoolBlockPhaseV1,
}

impl MemoryPoolBlockRecordV1 {
    pub const fn block_id(self) -> u64 {
        self.block_id
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    pub const fn phase(self) -> MemoryPoolBlockPhaseV1 {
        self.phase
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryPoolErrorV1 {
    InvalidIdentity,
    InvalidCapacity,
    InvalidRequest,
    CapacityExceeded,
    IdentityExhausted,
    UnknownLease,
    StaleLease,
    IllegalTransition,
    InvariantViolation,
}

#[derive(Debug)]
pub struct MemoryPoolTransitionFailureV1 {
    error: MemoryPoolErrorV1,
    lease: MemoryPoolLeaseV1,
}

impl MemoryPoolTransitionFailureV1 {
    pub const fn error(&self) -> MemoryPoolErrorV1 {
        self.error
    }

    pub fn into_lease(self) -> MemoryPoolLeaseV1 {
        self.lease
    }
}

/// Pure reusable-pool model with monotonic block and lease generations.
pub struct MemoryPoolModelV1 {
    identity: IdentityDigestV1,
    device: DeviceKeyV1,
    kind: MemoryKindV1,
    byte_capacity: u64,
    block_capacity: usize,
    committed_bytes: u64,
    next_block_id: u64,
    blocks: Vec<MemoryPoolBlockRecordV1>,
}

impl core::fmt::Debug for MemoryPoolModelV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MemoryPoolModelV1")
            .field("identity", &self.identity)
            .field("device", &self.device)
            .field("kind", &self.kind)
            .field("byte_capacity", &self.byte_capacity)
            .field("block_capacity", &self.block_capacity)
            .field("committed_bytes", &self.committed_bytes)
            .field("next_block_id", &self.next_block_id)
            .field("blocks", &self.blocks)
            .finish()
    }
}

impl MemoryPoolModelV1 {
    pub fn new_model_only(
        identity: IdentityDigestV1,
        device: DeviceKeyV1,
        kind: MemoryKindV1,
        byte_capacity: u64,
        block_capacity: usize,
    ) -> Result<Self, MemoryPoolErrorV1> {
        if identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || device.physical.0 == 0
            || device.generation.0 == 0
        {
            return Err(MemoryPoolErrorV1::InvalidIdentity);
        }
        if byte_capacity == 0 || block_capacity == 0 || block_capacity > MAX_MEMORY_POOL_BLOCKS_V1 {
            return Err(MemoryPoolErrorV1::InvalidCapacity);
        }
        Ok(Self {
            identity,
            device,
            kind,
            byte_capacity,
            block_capacity,
            committed_bytes: 0,
            next_block_id: 1,
            blocks: Vec::new(),
        })
    }

    pub const fn identity(&self) -> IdentityDigestV1 {
        self.identity
    }

    pub const fn device(&self) -> DeviceKeyV1 {
        self.device
    }

    pub const fn kind(&self) -> MemoryKindV1 {
        self.kind
    }

    pub const fn byte_capacity(&self) -> u64 {
        self.byte_capacity
    }

    pub const fn committed_bytes(&self) -> u64 {
        self.committed_bytes
    }

    pub fn blocks(&self) -> &[MemoryPoolBlockRecordV1] {
        &self.blocks
    }

    pub fn retained_block_count(&self) -> usize {
        self.blocks.len()
    }

    pub fn non_reusable_block_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|block| block.phase.blocks_reuse())
            .count()
    }

    pub fn lease_model_only(
        &mut self,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<MemoryPoolLeaseV1, MemoryPoolErrorV1> {
        if requested_bytes == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(MemoryPoolErrorV1::InvalidRequest);
        }
        let reusable = self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                block.phase == MemoryPoolBlockPhaseV1::Free
                    && block.byte_len >= requested_bytes
                    && block.alignment >= alignment
            })
            .min_by_key(|(_, block)| (block.byte_len, block.alignment, block.block_id))
            .map(|(index, _)| index);
        let index = match reusable {
            Some(index) => index,
            None => {
                if self.blocks.len() >= self.block_capacity {
                    return Err(MemoryPoolErrorV1::CapacityExceeded);
                }
                let rounded = requested_bytes
                    .checked_add(alignment - 1)
                    .map(|bytes| bytes & !(alignment - 1))
                    .ok_or(MemoryPoolErrorV1::InvalidRequest)?;
                let committed = self
                    .committed_bytes
                    .checked_add(rounded)
                    .filter(|bytes| *bytes <= self.byte_capacity)
                    .ok_or(MemoryPoolErrorV1::CapacityExceeded)?;
                let block_id = self.next_block_id;
                self.next_block_id = self
                    .next_block_id
                    .checked_add(1)
                    .ok_or(MemoryPoolErrorV1::IdentityExhausted)?;
                self.blocks.push(MemoryPoolBlockRecordV1 {
                    block_id,
                    generation: 1,
                    byte_len: rounded,
                    alignment,
                    phase: MemoryPoolBlockPhaseV1::Free,
                });
                self.committed_bytes = committed;
                self.blocks.len() - 1
            }
        };
        let block = &mut self.blocks[index];
        debug_assert_eq!(block.phase, MemoryPoolBlockPhaseV1::Free);
        block.phase = MemoryPoolBlockPhaseV1::Leased;
        Ok(MemoryPoolLeaseV1 {
            identity: MemoryPoolLeaseIdentityV1 {
                pool: self.identity,
                device: self.device,
                block_id: block.block_id,
                generation: block.generation,
            },
            requested_bytes,
            alignment,
        })
    }

    pub fn mark_in_flight_model_only(
        &mut self,
        lease: MemoryPoolLeaseV1,
    ) -> Result<MemoryPoolLeaseV1, MemoryPoolTransitionFailureV1> {
        self.transition(
            lease,
            MemoryPoolBlockPhaseV1::Leased,
            MemoryPoolBlockPhaseV1::InFlight,
        )
    }

    pub fn observe_completion_model_only(
        &mut self,
        lease: MemoryPoolLeaseV1,
    ) -> Result<MemoryPoolLeaseV1, MemoryPoolTransitionFailureV1> {
        self.transition(
            lease,
            MemoryPoolBlockPhaseV1::InFlight,
            MemoryPoolBlockPhaseV1::CompletionObserved,
        )
    }

    pub fn release_model_only(
        &mut self,
        lease: MemoryPoolLeaseV1,
    ) -> Result<(), MemoryPoolTransitionFailureV1> {
        let identity = lease.identity;
        let index = match self.lease_index(&lease) {
            Ok(index) => index,
            Err(error) => return Err(MemoryPoolTransitionFailureV1 { error, lease }),
        };
        let block = &mut self.blocks[index];
        if !matches!(
            block.phase,
            MemoryPoolBlockPhaseV1::Leased | MemoryPoolBlockPhaseV1::CompletionObserved
        ) {
            return Err(MemoryPoolTransitionFailureV1 {
                error: MemoryPoolErrorV1::IllegalTransition,
                lease,
            });
        }
        let Some(next_generation) = block.generation.checked_add(1) else {
            block.phase = MemoryPoolBlockPhaseV1::Quarantined;
            return Err(MemoryPoolTransitionFailureV1 {
                error: MemoryPoolErrorV1::IdentityExhausted,
                lease,
            });
        };
        debug_assert_eq!(identity.generation, block.generation);
        block.generation = next_generation;
        block.phase = MemoryPoolBlockPhaseV1::Free;
        Ok(())
    }

    pub fn quarantine_model_only(
        &mut self,
        lease: MemoryPoolLeaseV1,
    ) -> Result<(), MemoryPoolTransitionFailureV1> {
        let index = match self.lease_index(&lease) {
            Ok(index) => index,
            Err(error) => return Err(MemoryPoolTransitionFailureV1 { error, lease }),
        };
        if self.blocks[index].phase == MemoryPoolBlockPhaseV1::Free {
            return Err(MemoryPoolTransitionFailureV1 {
                error: MemoryPoolErrorV1::IllegalTransition,
                lease,
            });
        }
        self.blocks[index].phase = MemoryPoolBlockPhaseV1::Quarantined;
        Ok(())
    }

    pub fn trim_model_only(&mut self) -> Result<u64, MemoryPoolErrorV1> {
        let released = self
            .blocks
            .iter()
            .filter(|block| block.phase == MemoryPoolBlockPhaseV1::Free)
            .try_fold(0_u64, |sum, block| sum.checked_add(block.byte_len))
            .ok_or(MemoryPoolErrorV1::InvariantViolation)?;
        self.blocks
            .retain(|block| block.phase != MemoryPoolBlockPhaseV1::Free);
        self.committed_bytes = self
            .committed_bytes
            .checked_sub(released)
            .ok_or(MemoryPoolErrorV1::InvariantViolation)?;
        Ok(released)
    }

    pub fn validate_global_invariants(&self) -> Result<(), MemoryPoolErrorV1> {
        if self.identity.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
            || self.device.physical.0 == 0
            || self.device.generation.0 == 0
            || self.blocks.len() > self.block_capacity
            || self.block_capacity > MAX_MEMORY_POOL_BLOCKS_V1
            || self.next_block_id == 0
            || self.blocks.iter().enumerate().any(|(index, block)| {
                block.block_id == 0
                    || block.generation == 0
                    || block.byte_len == 0
                    || block.alignment == 0
                    || !block.alignment.is_power_of_two()
                    || self.blocks[..index]
                        .iter()
                        .any(|prior| prior.block_id == block.block_id)
            })
        {
            return Err(MemoryPoolErrorV1::InvariantViolation);
        }
        let committed = self
            .blocks
            .iter()
            .try_fold(0_u64, |sum, block| sum.checked_add(block.byte_len))
            .ok_or(MemoryPoolErrorV1::InvariantViolation)?;
        if committed != self.committed_bytes || committed > self.byte_capacity {
            return Err(MemoryPoolErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn transition(
        &mut self,
        lease: MemoryPoolLeaseV1,
        expected: MemoryPoolBlockPhaseV1,
        next: MemoryPoolBlockPhaseV1,
    ) -> Result<MemoryPoolLeaseV1, MemoryPoolTransitionFailureV1> {
        let index = match self.lease_index(&lease) {
            Ok(index) => index,
            Err(error) => return Err(MemoryPoolTransitionFailureV1 { error, lease }),
        };
        if self.blocks[index].phase != expected {
            return Err(MemoryPoolTransitionFailureV1 {
                error: MemoryPoolErrorV1::IllegalTransition,
                lease,
            });
        }
        self.blocks[index].phase = next;
        Ok(lease)
    }

    fn lease_index(&self, lease: &MemoryPoolLeaseV1) -> Result<usize, MemoryPoolErrorV1> {
        if lease.identity.pool != self.identity || lease.identity.device != self.device {
            return Err(MemoryPoolErrorV1::UnknownLease);
        }
        let index = self
            .blocks
            .iter()
            .position(|block| block.block_id == lease.identity.block_id)
            .ok_or(MemoryPoolErrorV1::UnknownLease)?;
        if self.blocks[index].generation != lease.identity.generation {
            return Err(MemoryPoolErrorV1::StaleLease);
        }
        Ok(index)
    }
}
