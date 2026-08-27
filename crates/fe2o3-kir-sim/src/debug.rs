use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BlockId, MemoryOrdering, ScalarType, SynchronizationScope, ValueId,
};
use std::error::Error;
use std::fmt;

use crate::{ScalarBitsV1, SimulationInvocationV1, SimulationScheduleIdentityV1};

pub const MAX_DEBUG_FRAMES_PER_CHECKPOINT_V1: usize = 4_096;
pub const MAX_DEBUG_VALUES_PER_CHECKPOINT_V1: usize = 1_000_000;
pub const MAX_DEBUG_ALLOCATIONS_PER_CHECKPOINT_V1: usize = 65_536;
pub const MAX_DEBUG_MEMORY_BYTES_PER_CHECKPOINT_V1: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationDebugCaptureLimitsV1 {
    max_frames_per_checkpoint: usize,
    max_values_per_checkpoint: usize,
    max_allocations_per_checkpoint: usize,
    max_memory_bytes_per_checkpoint: usize,
}

impl SimulationDebugCaptureLimitsV1 {
    pub fn new(
        max_frames_per_checkpoint: usize,
        max_values_per_checkpoint: usize,
        max_allocations_per_checkpoint: usize,
        max_memory_bytes_per_checkpoint: usize,
    ) -> Result<Self, SimulationDebugCaptureLimitsErrorV1> {
        let limits = Self {
            max_frames_per_checkpoint,
            max_values_per_checkpoint,
            max_allocations_per_checkpoint,
            max_memory_bytes_per_checkpoint,
        };
        limits.validate()?;
        Ok(limits)
    }

    pub const fn disabled() -> Self {
        Self {
            max_frames_per_checkpoint: 0,
            max_values_per_checkpoint: 0,
            max_allocations_per_checkpoint: 0,
            max_memory_bytes_per_checkpoint: 0,
        }
    }

    pub const fn is_enabled(self) -> bool {
        self.max_frames_per_checkpoint != 0
    }

    pub const fn max_frames_per_checkpoint(self) -> usize {
        self.max_frames_per_checkpoint
    }

    pub const fn max_values_per_checkpoint(self) -> usize {
        self.max_values_per_checkpoint
    }

    pub const fn max_allocations_per_checkpoint(self) -> usize {
        self.max_allocations_per_checkpoint
    }

    pub const fn max_memory_bytes_per_checkpoint(self) -> usize {
        self.max_memory_bytes_per_checkpoint
    }

    fn validate(self) -> Result<(), SimulationDebugCaptureLimitsErrorV1> {
        let fields = [
            (
                SimulationDebugCaptureLimitFieldV1::Frames,
                self.max_frames_per_checkpoint,
                MAX_DEBUG_FRAMES_PER_CHECKPOINT_V1,
            ),
            (
                SimulationDebugCaptureLimitFieldV1::Values,
                self.max_values_per_checkpoint,
                MAX_DEBUG_VALUES_PER_CHECKPOINT_V1,
            ),
            (
                SimulationDebugCaptureLimitFieldV1::Allocations,
                self.max_allocations_per_checkpoint,
                MAX_DEBUG_ALLOCATIONS_PER_CHECKPOINT_V1,
            ),
            (
                SimulationDebugCaptureLimitFieldV1::MemoryBytes,
                self.max_memory_bytes_per_checkpoint,
                MAX_DEBUG_MEMORY_BYTES_PER_CHECKPOINT_V1,
            ),
        ];
        for (field, actual, maximum) in fields {
            if actual == 0 || actual > maximum {
                return Err(SimulationDebugCaptureLimitsErrorV1 {
                    field,
                    actual,
                    maximum,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugCaptureLimitFieldV1 {
    Frames,
    Values,
    Allocations,
    MemoryBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationDebugCaptureLimitsErrorV1 {
    pub field: SimulationDebugCaptureLimitFieldV1,
    pub actual: usize,
    pub maximum: usize,
}

impl fmt::Display for SimulationDebugCaptureLimitsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "debug capture limit {:?} is {}, expected 1..={}",
            self.field, self.actual, self.maximum
        )
    }
}

impl Error for SimulationDebugCaptureLimitsErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugUnavailableReasonV1 {
    FrameLimit,
    ValueLimit,
    AllocationLimit,
    MemoryByteLimit,
    AllocationFailure,
    NotCaptured,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationDebugCollectionV1<T> {
    Captured(Vec<T>),
    Unavailable {
        reason: SimulationDebugUnavailableReasonV1,
        required: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationDebugValueV1 {
    Scalar(ScalarBitsV1),
    Pointer {
        allocation: u64,
        byte_offset: usize,
        element: ScalarType,
        address_space: AddressSpace,
        access: AccessMode,
        lower_bound: usize,
        upper_bound: usize,
    },
    Slice {
        allocation: u64,
        elements: usize,
        element: ScalarType,
        address_space: AddressSpace,
        access: AccessMode,
        byte_offset: usize,
        byte_len: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationDebugBindingV1 {
    pub value: ValueId,
    pub observed: SimulationDebugValueV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationDebugFrameV1 {
    pub depth: u32,
    pub function_ordinal: usize,
    pub block: BlockId,
    pub next_operation: Option<u32>,
    pub values: SimulationDebugCollectionV1<SimulationDebugBindingV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationDebugAllocationV1 {
    pub allocation: u64,
    pub address_space: AddressSpace,
    pub access: AccessMode,
    pub alignment: u32,
    pub bytes: Vec<u8>,
    pub initialized: Vec<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugCheckpointPhaseV1 {
    BeforeOperation,
    AfterOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugMemoryAccessV1 {
    Read,
    WriteCommitted,
    /// An atomic load, or a compare-exchange whose comparison failed.
    AtomicRead,
    /// An atomic store with no old-value result.
    AtomicWriteCommitted,
    /// One indivisible atomic read-modify-write whose committed value is recorded.
    AtomicReadWriteCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugBarrierActionV1 {
    Arrive,
    Release,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationDebugRecordKindV1 {
    Checkpoint {
        phase: SimulationDebugCheckpointPhaseV1,
        stack: SimulationDebugCollectionV1<SimulationDebugFrameV1>,
        memory: SimulationDebugCollectionV1<SimulationDebugAllocationV1>,
    },
    Memory {
        access: SimulationDebugMemoryAccessV1,
        allocation: u64,
        byte_offset: usize,
        byte_len: usize,
        address_space: AddressSpace,
        value: SimulationDebugValueV1,
    },
    WorkgroupBarrier {
        action: SimulationDebugBarrierActionV1,
        phase: u64,
        participants: u32,
    },
    /// A scoped memory-order point. It is not an execution barrier.
    Fence {
        memory_scope: SynchronizationScope,
        ordering: MemoryOrdering,
        /// Bits 0 through 4 represent private, workgroup, global, constant,
        /// and generic address spaces, respectively.
        address_space_mask: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationDebugRecordV1 {
    pub ordinal: u64,
    /// Semantic CPU schedule and realized runnable-decision prefix for this record.
    pub schedule: SimulationDebugScheduleV1,
    pub invocation: SimulationInvocationV1,
    pub site: SimulationDebugSiteV1,
    pub kind: SimulationDebugRecordKindV1,
}

/// Schedule provenance attached to one live debugger observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationDebugScheduleV1 {
    pub identity: SimulationScheduleIdentityV1,
    /// Zero-based runnable-invocation decision that produced this observation.
    pub decision_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationDebugSiteV1 {
    pub function_ordinal: usize,
    pub block: BlockId,
    pub operation: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugSinkControlV1 {
    Continue,
    Stop,
    DropAndStop,
}

pub trait SimulationDebugSinkV1 {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1;
}

#[derive(Default)]
pub struct NoopSimulationDebugSinkV1;

impl SimulationDebugSinkV1 for NoopSimulationDebugSinkV1 {
    fn record(&mut self, _record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        SimulationDebugSinkControlV1::DropAndStop
    }
}
