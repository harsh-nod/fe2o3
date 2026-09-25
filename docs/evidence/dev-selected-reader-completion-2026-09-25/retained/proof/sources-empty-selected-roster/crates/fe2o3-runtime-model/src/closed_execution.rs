//! Closed, bounded execution-composition model.
//!
//! This module composes model-only streams, reusable pool blocks, compute and
//! peer-copy operations, cross-stream dependencies, atomic batch publication,
//! cancellation, completion, and quarantine in one state machine. It also
//! provides executable correspondence checks for declared atomic steps and
//! Wave64 collectives.
//!
//! Every input remains caller-constructible and no value is native authority.
//! The atomic correspondence checks compare declared abstract observations;
//! they do not decode instructions or establish compiler, ISA, coherence, or
//! hardware behavior.

use alloc::vec::Vec;

use crate::{DeviceKeyV1, IDENTITY_DIGEST_BYTES_V1, IdentityDigestV1};

pub const CLOSED_EXECUTION_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_CLOSED_STREAMS_V1: usize = 64;
pub const MAX_CLOSED_POOLS_V1: usize = 64;
pub const MAX_CLOSED_BLOCKS_V1: usize = 4_096;
pub const MAX_CLOSED_OPERATIONS_V1: usize = 4_096;
pub const MAX_CLOSED_OPERATION_LEASES_V1: usize = 64;
pub const MAX_CLOSED_DEPENDENCIES_V1: usize = 256;
pub const WAVE64_LANE_COUNT_V1: usize = 64;
pub const WAVE64_FULL_MASK_V1: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClosedStreamKeyV1 {
    pub device: DeviceKeyV1,
    pub stream_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClosedOperationKeyV1 {
    pub stream: ClosedStreamKeyV1,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClosedPoolKeyV1 {
    pub device: DeviceKeyV1,
    pub pool_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClosedPoolLeaseKeyV1 {
    pub pool: ClosedPoolKeyV1,
    pub block_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedPoolBlockPhaseV1 {
    Free,
    Leased,
    Prepared(ClosedOperationKeyV1),
    Published(ClosedOperationKeyV1),
    CompletionObserved(ClosedOperationKeyV1),
    Quarantined(ClosedOperationKeyV1),
}

impl ClosedPoolBlockPhaseV1 {
    pub const fn reusable(self) -> bool {
        matches!(self, Self::Free)
    }

    pub const fn operation(self) -> Option<ClosedOperationKeyV1> {
        match self {
            Self::Prepared(operation)
            | Self::Published(operation)
            | Self::CompletionObserved(operation)
            | Self::Quarantined(operation) => Some(operation),
            Self::Free | Self::Leased => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedPoolBlockRecordV1 {
    key: ClosedPoolLeaseKeyV1,
    byte_len: u64,
    alignment: u64,
    phase: ClosedPoolBlockPhaseV1,
}

impl ClosedPoolBlockRecordV1 {
    pub const fn key(self) -> ClosedPoolLeaseKeyV1 {
        self.key
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    pub const fn phase(self) -> ClosedPoolBlockPhaseV1 {
        self.phase
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClosedPoolRecordV1 {
    key: ClosedPoolKeyV1,
    byte_capacity: u64,
    block_capacity: usize,
    committed_bytes: u64,
    next_block_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClosedStreamRecordV1 {
    key: ClosedStreamKeyV1,
    next_sequence: u64,
    next_publication_sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedOperationKindV1 {
    Compute {
        execution_device: DeviceKeyV1,
    },
    PeerCopy {
        source_device: DeviceKeyV1,
        destination_device: DeviceKeyV1,
        execution_device: DeviceKeyV1,
    },
}

impl ClosedOperationKindV1 {
    pub const fn execution_device(self) -> DeviceKeyV1 {
        match self {
            Self::Compute { execution_device }
            | Self::PeerCopy {
                execution_device, ..
            } => execution_device,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedOperationPhaseV1 {
    Prepared,
    Published {
        batch_id: u64,
        publication_epoch: u64,
    },
    CompletionObserved,
    CancelledBeforePublication,
    Released,
    Indeterminate,
}

impl ClosedOperationPhaseV1 {
    pub const fn retains_leases(self) -> bool {
        matches!(
            self,
            Self::Prepared
                | Self::Published { .. }
                | Self::CompletionObserved
                | Self::Indeterminate
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedOperationRecordV1 {
    key: ClosedOperationKeyV1,
    kind: ClosedOperationKindV1,
    dependencies: Vec<ClosedOperationKeyV1>,
    leases: Vec<ClosedPoolLeaseKeyV1>,
    phase: ClosedOperationPhaseV1,
    cancellation_requested: bool,
    timeout_observations: u64,
}

impl ClosedOperationRecordV1 {
    pub const fn key(&self) -> ClosedOperationKeyV1 {
        self.key
    }

    pub const fn kind(&self) -> ClosedOperationKindV1 {
        self.kind
    }

    pub fn dependencies(&self) -> &[ClosedOperationKeyV1] {
        &self.dependencies
    }

    pub fn leases(&self) -> &[ClosedPoolLeaseKeyV1] {
        &self.leases
    }

    pub const fn phase(&self) -> ClosedOperationPhaseV1 {
        self.phase
    }

    pub const fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }

    pub const fn timeout_observations(&self) -> u64 {
        self.timeout_observations
    }
}

/// Exact prepared roster for one all-or-nothing publication transition.
#[derive(Debug)]
#[must_use = "a prepared batch must be published or deliberately retained"]
pub struct ClosedPreparedBatchV1 {
    model_incarnation: IdentityDigestV1,
    batch_id: u64,
    stream: ClosedStreamKeyV1,
    operations: Vec<ClosedOperationKeyV1>,
}

impl ClosedPreparedBatchV1 {
    pub const fn batch_id(&self) -> u64 {
        self.batch_id
    }

    pub const fn stream(&self) -> ClosedStreamKeyV1 {
        self.stream
    }

    pub fn operations(&self) -> &[ClosedOperationKeyV1] {
        &self.operations
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedExecutionErrorV1 {
    InvalidIdentity,
    CapacityExceeded,
    DuplicateIdentity,
    UnknownStream,
    UnknownPool,
    UnknownLease,
    UnknownOperation,
    StaleLease,
    InvalidRequest,
    InvalidRoster,
    InvalidDeviceOwnership,
    InvalidSequence,
    DependencyNotCompleted,
    IllegalTransition,
    IdentityExhausted,
    InvariantViolation,
}

/// One bounded model state closing streams, pools, operations, and batches.
pub struct ClosedExecutionModelV1 {
    incarnation: IdentityDigestV1,
    streams: Vec<ClosedStreamRecordV1>,
    pools: Vec<ClosedPoolRecordV1>,
    blocks: Vec<ClosedPoolBlockRecordV1>,
    operations: Vec<ClosedOperationRecordV1>,
    next_batch_id: u64,
    next_publication_epoch: u64,
}

impl core::fmt::Debug for ClosedExecutionModelV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ClosedExecutionModelV1")
            .field("incarnation", &self.incarnation)
            .field("streams", &self.streams)
            .field("pools", &self.pools)
            .field("blocks", &self.blocks)
            .field("operations", &self.operations)
            .field("next_batch_id", &self.next_batch_id)
            .field("next_publication_epoch", &self.next_publication_epoch)
            .finish()
    }
}

impl ClosedExecutionModelV1 {
    pub fn new_model_only(incarnation: IdentityDigestV1) -> Result<Self, ClosedExecutionErrorV1> {
        if digest_is_zero(incarnation) {
            return Err(ClosedExecutionErrorV1::InvalidIdentity);
        }
        Ok(Self {
            incarnation,
            streams: Vec::new(),
            pools: Vec::new(),
            blocks: Vec::new(),
            operations: Vec::new(),
            next_batch_id: 1,
            next_publication_epoch: 1,
        })
    }

    pub fn register_stream_model_only(
        &mut self,
        key: ClosedStreamKeyV1,
    ) -> Result<(), ClosedExecutionErrorV1> {
        if !valid_device(key.device) || key.stream_id == 0 || key.generation == 0 {
            return Err(ClosedExecutionErrorV1::InvalidIdentity);
        }
        if self.streams.len() >= MAX_CLOSED_STREAMS_V1 {
            return Err(ClosedExecutionErrorV1::CapacityExceeded);
        }
        if self.streams.iter().any(|stream| stream.key == key) {
            return Err(ClosedExecutionErrorV1::DuplicateIdentity);
        }
        self.streams.push(ClosedStreamRecordV1 {
            key,
            next_sequence: 1,
            next_publication_sequence: 1,
        });
        self.streams.sort_unstable_by_key(|stream| stream.key);
        Ok(())
    }

    pub fn register_pool_model_only(
        &mut self,
        key: ClosedPoolKeyV1,
        byte_capacity: u64,
        block_capacity: usize,
    ) -> Result<(), ClosedExecutionErrorV1> {
        if !valid_device(key.device) || key.pool_id == 0 {
            return Err(ClosedExecutionErrorV1::InvalidIdentity);
        }
        if byte_capacity == 0
            || block_capacity == 0
            || self.pools.len() >= MAX_CLOSED_POOLS_V1
            || block_capacity > MAX_CLOSED_BLOCKS_V1
        {
            return Err(ClosedExecutionErrorV1::CapacityExceeded);
        }
        if self.pools.iter().any(|pool| pool.key == key) {
            return Err(ClosedExecutionErrorV1::DuplicateIdentity);
        }
        self.pools.push(ClosedPoolRecordV1 {
            key,
            byte_capacity,
            block_capacity,
            committed_bytes: 0,
            next_block_id: 1,
        });
        self.pools.sort_unstable_by_key(|pool| pool.key);
        Ok(())
    }

    pub fn lease_model_only(
        &mut self,
        pool: ClosedPoolKeyV1,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<ClosedPoolLeaseKeyV1, ClosedExecutionErrorV1> {
        if requested_bytes == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(ClosedExecutionErrorV1::InvalidRequest);
        }
        let pool_index = self
            .pools
            .iter()
            .position(|record| record.key == pool)
            .ok_or(ClosedExecutionErrorV1::UnknownPool)?;
        let reusable = self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| {
                block.key.pool == pool
                    && block.phase == ClosedPoolBlockPhaseV1::Free
                    && block.byte_len >= requested_bytes
                    && block.alignment >= alignment
            })
            .min_by_key(|(_, block)| (block.byte_len, block.alignment, block.key.block_id))
            .map(|(index, _)| index);
        let block_index = match reusable {
            Some(index) => index,
            None => {
                let pool_block_count = self
                    .blocks
                    .iter()
                    .filter(|block| block.key.pool == pool)
                    .count();
                if self.blocks.len() >= MAX_CLOSED_BLOCKS_V1
                    || pool_block_count >= self.pools[pool_index].block_capacity
                {
                    return Err(ClosedExecutionErrorV1::CapacityExceeded);
                }
                let rounded = round_up(requested_bytes, alignment)
                    .ok_or(ClosedExecutionErrorV1::InvalidRequest)?;
                let committed = self.pools[pool_index]
                    .committed_bytes
                    .checked_add(rounded)
                    .filter(|bytes| *bytes <= self.pools[pool_index].byte_capacity)
                    .ok_or(ClosedExecutionErrorV1::CapacityExceeded)?;
                let block_id = self.pools[pool_index].next_block_id;
                let next_block_id = block_id
                    .checked_add(1)
                    .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
                self.pools[pool_index].next_block_id = next_block_id;
                self.pools[pool_index].committed_bytes = committed;
                self.blocks.push(ClosedPoolBlockRecordV1 {
                    key: ClosedPoolLeaseKeyV1 {
                        pool,
                        block_id,
                        generation: 1,
                    },
                    byte_len: rounded,
                    alignment,
                    phase: ClosedPoolBlockPhaseV1::Free,
                });
                self.blocks.len() - 1
            }
        };
        let block = &mut self.blocks[block_index];
        block.phase = ClosedPoolBlockPhaseV1::Leased;
        Ok(block.key)
    }

    pub fn release_unprepared_lease_model_only(
        &mut self,
        lease: ClosedPoolLeaseKeyV1,
    ) -> Result<(), ClosedExecutionErrorV1> {
        let block = self.exact_block_mut(lease)?;
        if block.phase != ClosedPoolBlockPhaseV1::Leased {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        release_block(block)
    }

    pub fn prepare_operation_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
        kind: ClosedOperationKindV1,
        dependencies: Vec<ClosedOperationKeyV1>,
        leases: Vec<ClosedPoolLeaseKeyV1>,
    ) -> Result<(), ClosedExecutionErrorV1> {
        if self.operations.len() >= MAX_CLOSED_OPERATIONS_V1 {
            return Err(ClosedExecutionErrorV1::CapacityExceeded);
        }
        if key.sequence == 0 || self.operations.iter().any(|operation| operation.key == key) {
            return Err(ClosedExecutionErrorV1::DuplicateIdentity);
        }
        if dependencies.len() > MAX_CLOSED_DEPENDENCIES_V1
            || dependencies.windows(2).any(|pair| pair[0] >= pair[1])
            || dependencies.contains(&key)
            || leases.is_empty()
            || leases.len() > MAX_CLOSED_OPERATION_LEASES_V1
            || leases.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ClosedExecutionErrorV1::InvalidRoster);
        }
        let stream_index = self
            .streams
            .iter()
            .position(|stream| stream.key == key.stream)
            .ok_or(ClosedExecutionErrorV1::UnknownStream)?;
        if self.streams[stream_index].next_sequence != key.sequence {
            return Err(ClosedExecutionErrorV1::InvalidSequence);
        }
        if key.stream.device != kind.execution_device() {
            return Err(ClosedExecutionErrorV1::InvalidDeviceOwnership);
        }
        if dependencies.iter().any(|dependency| {
            !self
                .operations
                .iter()
                .any(|operation| operation.key == *dependency)
        }) {
            return Err(ClosedExecutionErrorV1::UnknownOperation);
        }
        let block_indices = leases
            .iter()
            .map(|lease| self.exact_block_index(*lease))
            .collect::<Result<Vec<_>, _>>()?;
        if block_indices
            .iter()
            .any(|index| self.blocks[*index].phase != ClosedPoolBlockPhaseV1::Leased)
        {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        validate_operation_ownership(kind, &leases)?;
        let next_sequence = key
            .sequence
            .checked_add(1)
            .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;

        for index in block_indices {
            self.blocks[index].phase = ClosedPoolBlockPhaseV1::Prepared(key);
        }
        self.streams[stream_index].next_sequence = next_sequence;
        self.operations.push(ClosedOperationRecordV1 {
            key,
            kind,
            dependencies,
            leases,
            phase: ClosedOperationPhaseV1::Prepared,
            cancellation_requested: false,
            timeout_observations: 0,
        });
        self.operations
            .sort_unstable_by_key(|operation| operation.key);
        Ok(())
    }

    pub fn form_prepared_batch_model_only(
        &mut self,
        stream: ClosedStreamKeyV1,
        operations: Vec<ClosedOperationKeyV1>,
    ) -> Result<ClosedPreparedBatchV1, ClosedExecutionErrorV1> {
        if operations.is_empty()
            || operations.windows(2).any(|pair| pair[0] >= pair[1])
            || operations
                .iter()
                .any(|operation| operation.stream != stream)
        {
            return Err(ClosedExecutionErrorV1::InvalidRoster);
        }
        let stream_record = self
            .streams
            .iter()
            .find(|record| record.key == stream)
            .ok_or(ClosedExecutionErrorV1::UnknownStream)?;
        for (offset, key) in operations.iter().enumerate() {
            let expected = stream_record
                .next_publication_sequence
                .checked_add(offset as u64)
                .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
            let operation = self
                .operations
                .iter()
                .find(|operation| operation.key == *key)
                .ok_or(ClosedExecutionErrorV1::UnknownOperation)?;
            if key.sequence != expected || operation.phase != ClosedOperationPhaseV1::Prepared {
                return Err(ClosedExecutionErrorV1::InvalidSequence);
            }
        }
        let batch_id = self.next_batch_id;
        self.next_batch_id = batch_id
            .checked_add(1)
            .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
        Ok(ClosedPreparedBatchV1 {
            model_incarnation: self.incarnation,
            batch_id,
            stream,
            operations,
        })
    }

    /// Publishes every operation in the batch after all checks, or none.
    pub fn publish_prepared_batch_model_only(
        &mut self,
        batch: &ClosedPreparedBatchV1,
    ) -> Result<u64, ClosedExecutionErrorV1> {
        if batch.model_incarnation != self.incarnation || batch.operations.is_empty() {
            return Err(ClosedExecutionErrorV1::InvalidIdentity);
        }
        let stream_index = self
            .streams
            .iter()
            .position(|stream| stream.key == batch.stream)
            .ok_or(ClosedExecutionErrorV1::UnknownStream)?;
        for (offset, key) in batch.operations.iter().enumerate() {
            let expected = self.streams[stream_index]
                .next_publication_sequence
                .checked_add(offset as u64)
                .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
            if key.stream != batch.stream || key.sequence != expected {
                return Err(ClosedExecutionErrorV1::InvalidSequence);
            }
            let operation = self.operation(*key)?;
            if operation.phase != ClosedOperationPhaseV1::Prepared {
                return Err(ClosedExecutionErrorV1::IllegalTransition);
            }
            if operation
                .dependencies
                .iter()
                .any(|dependency| match self.operation(*dependency) {
                    Ok(dependency) => !matches!(
                        dependency.phase,
                        ClosedOperationPhaseV1::CompletionObserved
                            | ClosedOperationPhaseV1::Released
                    ),
                    Err(_) => true,
                })
            {
                return Err(ClosedExecutionErrorV1::DependencyNotCompleted);
            }
            for lease in &operation.leases {
                let block = self.exact_block(*lease)?;
                if block.phase != ClosedPoolBlockPhaseV1::Prepared(*key) {
                    return Err(ClosedExecutionErrorV1::InvariantViolation);
                }
            }
        }
        let publication_epoch = self.next_publication_epoch;
        let next_epoch = publication_epoch
            .checked_add(1)
            .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
        let next_publication_sequence = self.streams[stream_index]
            .next_publication_sequence
            .checked_add(batch.operations.len() as u64)
            .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;

        for key in &batch.operations {
            let operation_index = self.operation_index(*key)?;
            let leases = self.operations[operation_index].leases.clone();
            for lease in leases {
                self.exact_block_mut(lease)?.phase = ClosedPoolBlockPhaseV1::Published(*key);
            }
            self.operations[operation_index].phase = ClosedOperationPhaseV1::Published {
                batch_id: batch.batch_id,
                publication_epoch,
            };
        }
        self.streams[stream_index].next_publication_sequence = next_publication_sequence;
        self.next_publication_epoch = next_epoch;
        Ok(publication_epoch)
    }

    pub fn cancel_before_publication_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<(), ClosedExecutionErrorV1> {
        let operation_index = self.operation_index(key)?;
        if self.operations[operation_index].phase != ClosedOperationPhaseV1::Prepared {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        let stream_index = self
            .streams
            .iter()
            .position(|stream| stream.key == key.stream)
            .ok_or(ClosedExecutionErrorV1::UnknownStream)?;
        if self.streams[stream_index].next_publication_sequence != key.sequence {
            return Err(ClosedExecutionErrorV1::InvalidSequence);
        }
        let leases = self.operations[operation_index].leases.clone();
        for lease in leases {
            let block = self.exact_block_mut(lease)?;
            if block.phase != ClosedPoolBlockPhaseV1::Prepared(key) {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
            release_block(block)?;
        }
        self.operations[operation_index].phase = ClosedOperationPhaseV1::CancelledBeforePublication;
        self.streams[stream_index].next_publication_sequence = key
            .sequence
            .checked_add(1)
            .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
        Ok(())
    }

    pub fn request_cancellation_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<bool, ClosedExecutionErrorV1> {
        let operation = self.operation_mut(key)?;
        if !matches!(operation.phase, ClosedOperationPhaseV1::Published { .. }) {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        let first = !operation.cancellation_requested;
        operation.cancellation_requested = true;
        Ok(first)
    }

    pub fn observe_timeout_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<u64, ClosedExecutionErrorV1> {
        let operation = self.operation_mut(key)?;
        if !matches!(operation.phase, ClosedOperationPhaseV1::Published { .. }) {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        operation.timeout_observations = operation
            .timeout_observations
            .checked_add(1)
            .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
        Ok(operation.timeout_observations)
    }

    pub fn observe_completion_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<(), ClosedExecutionErrorV1> {
        let operation_index = self.operation_index(key)?;
        if !matches!(
            self.operations[operation_index].phase,
            ClosedOperationPhaseV1::Published { .. }
        ) {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        let leases = self.operations[operation_index].leases.clone();
        for lease in &leases {
            if self.exact_block(*lease)?.phase != ClosedPoolBlockPhaseV1::Published(key) {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
        }
        for lease in leases {
            self.exact_block_mut(lease)?.phase = ClosedPoolBlockPhaseV1::CompletionObserved(key);
        }
        self.operations[operation_index].phase = ClosedOperationPhaseV1::CompletionObserved;
        Ok(())
    }

    pub fn release_completed_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<(), ClosedExecutionErrorV1> {
        let operation_index = self.operation_index(key)?;
        if self.operations[operation_index].phase != ClosedOperationPhaseV1::CompletionObserved {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        let leases = self.operations[operation_index].leases.clone();
        for lease in &leases {
            if self.exact_block(*lease)?.phase != ClosedPoolBlockPhaseV1::CompletionObserved(key) {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
        }
        for lease in leases {
            release_block(self.exact_block_mut(lease)?)?;
        }
        self.operations[operation_index].phase = ClosedOperationPhaseV1::Released;
        Ok(())
    }

    pub fn quarantine_published_model_only(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<(), ClosedExecutionErrorV1> {
        let operation_index = self.operation_index(key)?;
        if !matches!(
            self.operations[operation_index].phase,
            ClosedOperationPhaseV1::Published { .. }
        ) {
            return Err(ClosedExecutionErrorV1::IllegalTransition);
        }
        let leases = self.operations[operation_index].leases.clone();
        for lease in &leases {
            if self.exact_block(*lease)?.phase != ClosedPoolBlockPhaseV1::Published(key) {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
        }
        for lease in leases {
            self.exact_block_mut(lease)?.phase = ClosedPoolBlockPhaseV1::Quarantined(key);
        }
        self.operations[operation_index].phase = ClosedOperationPhaseV1::Indeterminate;
        Ok(())
    }

    pub fn operations(&self) -> &[ClosedOperationRecordV1] {
        &self.operations
    }

    pub fn blocks(&self) -> &[ClosedPoolBlockRecordV1] {
        &self.blocks
    }

    pub fn operation(
        &self,
        key: ClosedOperationKeyV1,
    ) -> Result<&ClosedOperationRecordV1, ClosedExecutionErrorV1> {
        self.operations
            .iter()
            .find(|operation| operation.key == key)
            .ok_or(ClosedExecutionErrorV1::UnknownOperation)
    }

    pub fn retained_operation_count(&self) -> usize {
        self.operations
            .iter()
            .filter(|operation| operation.phase.retains_leases())
            .count()
    }

    pub fn validate_global_invariants(&self) -> Result<(), ClosedExecutionErrorV1> {
        if digest_is_zero(self.incarnation)
            || self.next_batch_id == 0
            || self.next_publication_epoch == 0
            || self.streams.len() > MAX_CLOSED_STREAMS_V1
            || self.pools.len() > MAX_CLOSED_POOLS_V1
            || self.blocks.len() > MAX_CLOSED_BLOCKS_V1
            || self.operations.len() > MAX_CLOSED_OPERATIONS_V1
            || self
                .streams
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
            || self.pools.windows(2).any(|pair| pair[0].key >= pair[1].key)
            || self
                .operations
                .windows(2)
                .any(|pair| pair[0].key >= pair[1].key)
        {
            return Err(ClosedExecutionErrorV1::InvariantViolation);
        }
        for pool in &self.pools {
            let mut blocks = self
                .blocks
                .iter()
                .filter(|block| block.key.pool == pool.key);
            let count = blocks.clone().count();
            let committed = blocks
                .try_fold(0_u64, |sum, block| sum.checked_add(block.byte_len))
                .ok_or(ClosedExecutionErrorV1::InvariantViolation)?;
            if count > pool.block_capacity
                || committed != pool.committed_bytes
                || committed > pool.byte_capacity
                || pool.next_block_id == 0
            {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
        }
        for (index, block) in self.blocks.iter().enumerate() {
            if block.key.block_id == 0
                || block.key.generation == 0
                || block.byte_len == 0
                || block.alignment == 0
                || !block.alignment.is_power_of_two()
                || self.blocks[..index].iter().any(|prior| {
                    prior.key.pool == block.key.pool && prior.key.block_id == block.key.block_id
                })
            {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
            if let Some(owner) = block.phase.operation() {
                let operation = self.operation(owner)?;
                let expected = match operation.phase {
                    ClosedOperationPhaseV1::Prepared => ClosedPoolBlockPhaseV1::Prepared(owner),
                    ClosedOperationPhaseV1::Published { .. } => {
                        ClosedPoolBlockPhaseV1::Published(owner)
                    }
                    ClosedOperationPhaseV1::CompletionObserved => {
                        ClosedPoolBlockPhaseV1::CompletionObserved(owner)
                    }
                    ClosedOperationPhaseV1::Indeterminate => {
                        ClosedPoolBlockPhaseV1::Quarantined(owner)
                    }
                    ClosedOperationPhaseV1::CancelledBeforePublication
                    | ClosedOperationPhaseV1::Released => {
                        return Err(ClosedExecutionErrorV1::InvariantViolation);
                    }
                };
                if block.phase != expected || !operation.leases.contains(&block.key) {
                    return Err(ClosedExecutionErrorV1::InvariantViolation);
                }
            }
        }
        for operation in &self.operations {
            if operation.dependencies.len() > MAX_CLOSED_DEPENDENCIES_V1
                || operation.leases.is_empty()
                || operation.leases.len() > MAX_CLOSED_OPERATION_LEASES_V1
                || operation
                    .dependencies
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || operation.leases.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(ClosedExecutionErrorV1::InvariantViolation);
            }
            for lease in &operation.leases {
                let current = self.blocks.iter().find(|block| {
                    block.key.pool == lease.pool && block.key.block_id == lease.block_id
                });
                let Some(current) = current else {
                    return Err(ClosedExecutionErrorV1::InvariantViolation);
                };
                if operation.phase.retains_leases() {
                    if current.key != *lease || current.phase.operation() != Some(operation.key) {
                        return Err(ClosedExecutionErrorV1::InvariantViolation);
                    }
                } else if current.key.generation <= lease.generation {
                    return Err(ClosedExecutionErrorV1::InvariantViolation);
                }
            }
        }
        Ok(())
    }

    fn operation_index(&self, key: ClosedOperationKeyV1) -> Result<usize, ClosedExecutionErrorV1> {
        self.operations
            .iter()
            .position(|operation| operation.key == key)
            .ok_or(ClosedExecutionErrorV1::UnknownOperation)
    }

    fn operation_mut(
        &mut self,
        key: ClosedOperationKeyV1,
    ) -> Result<&mut ClosedOperationRecordV1, ClosedExecutionErrorV1> {
        let index = self.operation_index(key)?;
        Ok(&mut self.operations[index])
    }

    fn exact_block_index(
        &self,
        lease: ClosedPoolLeaseKeyV1,
    ) -> Result<usize, ClosedExecutionErrorV1> {
        let index = self
            .blocks
            .iter()
            .position(|block| block.key.pool == lease.pool && block.key.block_id == lease.block_id)
            .ok_or(ClosedExecutionErrorV1::UnknownLease)?;
        if self.blocks[index].key.generation != lease.generation {
            return Err(ClosedExecutionErrorV1::StaleLease);
        }
        Ok(index)
    }

    fn exact_block(
        &self,
        lease: ClosedPoolLeaseKeyV1,
    ) -> Result<&ClosedPoolBlockRecordV1, ClosedExecutionErrorV1> {
        let index = self.exact_block_index(lease)?;
        Ok(&self.blocks[index])
    }

    fn exact_block_mut(
        &mut self,
        lease: ClosedPoolLeaseKeyV1,
    ) -> Result<&mut ClosedPoolBlockRecordV1, ClosedExecutionErrorV1> {
        let index = self.exact_block_index(lease)?;
        Ok(&mut self.blocks[index])
    }
}

fn validate_operation_ownership(
    kind: ClosedOperationKindV1,
    leases: &[ClosedPoolLeaseKeyV1],
) -> Result<(), ClosedExecutionErrorV1> {
    match kind {
        ClosedOperationKindV1::Compute { execution_device } => {
            if leases
                .iter()
                .any(|lease| lease.pool.device != execution_device)
            {
                return Err(ClosedExecutionErrorV1::InvalidDeviceOwnership);
            }
        }
        ClosedOperationKindV1::PeerCopy {
            source_device,
            destination_device,
            execution_device,
        } => {
            if source_device == destination_device
                || execution_device != destination_device
                || leases.len() != 2
                || !leases
                    .iter()
                    .any(|lease| lease.pool.device == source_device)
                || !leases
                    .iter()
                    .any(|lease| lease.pool.device == destination_device)
            {
                return Err(ClosedExecutionErrorV1::InvalidDeviceOwnership);
            }
        }
    }
    Ok(())
}

fn release_block(block: &mut ClosedPoolBlockRecordV1) -> Result<(), ClosedExecutionErrorV1> {
    block.key.generation = block
        .key
        .generation
        .checked_add(1)
        .ok_or(ClosedExecutionErrorV1::IdentityExhausted)?;
    block.phase = ClosedPoolBlockPhaseV1::Free;
    Ok(())
}

const fn valid_device(device: DeviceKeyV1) -> bool {
    device.physical.0 != 0 && device.generation.0 != 0
}

fn digest_is_zero(digest: IdentityDigestV1) -> bool {
    digest.as_bytes() == &[0; IDENTITY_DIGEST_BYTES_V1]
}

const fn round_up(value: u64, alignment: u64) -> Option<u64> {
    match value.checked_add(alignment - 1) {
        Some(sum) => Some(sum & !(alignment - 1)),
        None => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedAtomicOperationV1 {
    Load,
    Store,
    Exchange,
    FetchAdd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedAtomicOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

impl ClosedAtomicOrderV1 {
    const fn needs_pre_release_fence(self) -> bool {
        matches!(
            self,
            Self::Release | Self::AcquireRelease | Self::SequentiallyConsistent
        )
    }

    const fn needs_post_acquire_fence(self) -> bool {
        matches!(
            self,
            Self::Acquire | Self::AcquireRelease | Self::SequentiallyConsistent
        )
    }

    const fn needs_sequentially_consistent_fence(self) -> bool {
        matches!(self, Self::SequentiallyConsistent)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedAtomicScopeV1 {
    Workgroup,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosedAtomicFencePlanV1 {
    pub pre_release: bool,
    pub post_acquire: bool,
    pub sequentially_consistent: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedClosedAtomicStepV1 {
    pub operation: ClosedAtomicOperationV1,
    pub declared_order: ClosedAtomicOrderV1,
    pub declared_scope: ClosedAtomicScopeV1,
    pub observed_operation: ClosedAtomicOperationV1,
    pub observed_order: ClosedAtomicOrderV1,
    pub observed_scope: ClosedAtomicScopeV1,
    pub fences: ClosedAtomicFencePlanV1,
    pub old_value: u64,
    pub operand: u64,
    pub new_value: u64,
    pub returned_value: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelClosedAtomicStepV1(UntrustedClosedAtomicStepV1);

impl ModelClosedAtomicStepV1 {
    pub const fn step(self) -> UntrustedClosedAtomicStepV1 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosedAtomicCorrespondenceErrorV1 {
    InvalidOrdering,
    OperationMismatch,
    OrderingMismatch,
    ScopeMismatch,
    FenceMismatch,
    ValueMismatch,
}

pub fn admit_closed_atomic_step_model_only_v1(
    step: UntrustedClosedAtomicStepV1,
) -> Result<ModelClosedAtomicStepV1, ClosedAtomicCorrespondenceErrorV1> {
    let ordering_valid = match step.operation {
        ClosedAtomicOperationV1::Load => matches!(
            step.declared_order,
            ClosedAtomicOrderV1::Relaxed
                | ClosedAtomicOrderV1::Acquire
                | ClosedAtomicOrderV1::SequentiallyConsistent
        ),
        ClosedAtomicOperationV1::Store => matches!(
            step.declared_order,
            ClosedAtomicOrderV1::Relaxed
                | ClosedAtomicOrderV1::Release
                | ClosedAtomicOrderV1::SequentiallyConsistent
        ),
        ClosedAtomicOperationV1::Exchange | ClosedAtomicOperationV1::FetchAdd => true,
    };
    if !ordering_valid {
        return Err(ClosedAtomicCorrespondenceErrorV1::InvalidOrdering);
    }
    if step.observed_operation != step.operation {
        return Err(ClosedAtomicCorrespondenceErrorV1::OperationMismatch);
    }
    if step.observed_order != step.declared_order {
        return Err(ClosedAtomicCorrespondenceErrorV1::OrderingMismatch);
    }
    if step.observed_scope != step.declared_scope {
        return Err(ClosedAtomicCorrespondenceErrorV1::ScopeMismatch);
    }
    let expected_fences = ClosedAtomicFencePlanV1 {
        pre_release: step.declared_order.needs_pre_release_fence(),
        post_acquire: step.declared_order.needs_post_acquire_fence(),
        sequentially_consistent: step.declared_order.needs_sequentially_consistent_fence(),
    };
    if step.fences != expected_fences {
        return Err(ClosedAtomicCorrespondenceErrorV1::FenceMismatch);
    }
    let (new_value, returned_value) = match step.operation {
        ClosedAtomicOperationV1::Load => (step.old_value, Some(step.old_value)),
        ClosedAtomicOperationV1::Store => (step.operand, None),
        ClosedAtomicOperationV1::Exchange => (step.operand, Some(step.old_value)),
        ClosedAtomicOperationV1::FetchAdd => (
            step.old_value.wrapping_add(step.operand),
            Some(step.old_value),
        ),
    };
    if step.new_value != new_value || step.returned_value != returned_value {
        return Err(ClosedAtomicCorrespondenceErrorV1::ValueMismatch);
    }
    Ok(ModelClosedAtomicStepV1(step))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64OperationV1 {
    Barrier,
    ReduceSumWrappingU64,
    InclusiveScanSumWrappingU64,
    ExclusiveScanSumWrappingU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64PhaseV1 {
    Gathering,
    Ready,
    Published,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Wave64ConvergenceModelV1 {
    operation: Wave64OperationV1,
    inputs: [u64; WAVE64_LANE_COUNT_V1],
    arrivals: u64,
    phase: Wave64PhaseV1,
    outputs: Option<[u64; WAVE64_LANE_COUNT_V1]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Wave64ConvergenceErrorV1 {
    DivergentControlFlow,
    IncompletePhysicalMask,
    InvalidLane,
    DuplicateArrival,
    IncompleteArrival,
    IllegalTransition,
}

impl Wave64ConvergenceModelV1 {
    pub fn new_model_only(
        operation: Wave64OperationV1,
        inputs: [u64; WAVE64_LANE_COUNT_V1],
        physical_lane_mask: u64,
        convergent: bool,
    ) -> Result<Self, Wave64ConvergenceErrorV1> {
        if !convergent {
            return Err(Wave64ConvergenceErrorV1::DivergentControlFlow);
        }
        if physical_lane_mask != WAVE64_FULL_MASK_V1 {
            return Err(Wave64ConvergenceErrorV1::IncompletePhysicalMask);
        }
        Ok(Self {
            operation,
            inputs,
            arrivals: 0,
            phase: Wave64PhaseV1::Gathering,
            outputs: None,
        })
    }

    pub fn arrive_model_only(&mut self, lane: usize) -> Result<(), Wave64ConvergenceErrorV1> {
        if self.phase != Wave64PhaseV1::Gathering {
            return Err(Wave64ConvergenceErrorV1::IllegalTransition);
        }
        if lane >= WAVE64_LANE_COUNT_V1 {
            return Err(Wave64ConvergenceErrorV1::InvalidLane);
        }
        let bit = 1_u64 << lane;
        if self.arrivals & bit != 0 {
            return Err(Wave64ConvergenceErrorV1::DuplicateArrival);
        }
        self.arrivals |= bit;
        if self.arrivals == WAVE64_FULL_MASK_V1 {
            self.phase = Wave64PhaseV1::Ready;
        }
        Ok(())
    }

    pub fn publish_model_only(
        &mut self,
    ) -> Result<&[u64; WAVE64_LANE_COUNT_V1], Wave64ConvergenceErrorV1> {
        if self.phase == Wave64PhaseV1::Gathering {
            return Err(Wave64ConvergenceErrorV1::IncompleteArrival);
        }
        if self.phase != Wave64PhaseV1::Ready {
            return Err(Wave64ConvergenceErrorV1::IllegalTransition);
        }
        let mut outputs = [0_u64; WAVE64_LANE_COUNT_V1];
        match self.operation {
            Wave64OperationV1::Barrier => outputs = self.inputs,
            Wave64OperationV1::ReduceSumWrappingU64 => {
                let total = self.inputs.iter().copied().fold(0_u64, u64::wrapping_add);
                outputs.fill(total);
            }
            Wave64OperationV1::InclusiveScanSumWrappingU64 => {
                let mut prefix = 0_u64;
                for (index, value) in self.inputs.iter().copied().enumerate() {
                    prefix = prefix.wrapping_add(value);
                    outputs[index] = prefix;
                }
            }
            Wave64OperationV1::ExclusiveScanSumWrappingU64 => {
                let mut prefix = 0_u64;
                for (index, value) in self.inputs.iter().copied().enumerate() {
                    outputs[index] = prefix;
                    prefix = prefix.wrapping_add(value);
                }
            }
        }
        self.outputs = Some(outputs);
        self.phase = Wave64PhaseV1::Published;
        Ok(self.outputs.as_ref().expect("published Wave64 output"))
    }

    pub const fn arrivals(&self) -> u64 {
        self.arrivals
    }

    pub const fn phase(&self) -> Wave64PhaseV1 {
        self.phase
    }

    pub fn outputs(&self) -> Option<&[u64; WAVE64_LANE_COUNT_V1]> {
        self.outputs.as_ref()
    }
}
