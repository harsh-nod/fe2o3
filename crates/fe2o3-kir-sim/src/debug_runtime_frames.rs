//! Borrowed, producer-owned identities for one exact captured checkpoint.
//! No serialized layout, caller constructor, or cross-capture identity.
use crate::{SimulationDebugSiteV1, SimulationDebugUnavailableReasonV1};
use fe2o3_kernel_ir::BlockId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugFrameOperationV1 {
    Ready,
    ActiveOperation {
        attempt: u64,
        site: SimulationDebugSiteV1,
    },
    Suspended {
        attempt: u64,
        site: SimulationDebugSiteV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugFrameParentV1 {
    Root,
    Caller {
        activation: u64,
        attempt: u64,
        call_site: SimulationDebugSiteV1,
    },
}

/// An actual runtime frame. Depth is only a same-checkpoint join coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationDebugFrameOriginV1 {
    legacy_depth: u32,
    function_ordinal: usize,
    block: BlockId,
    next_operation: Option<u32>,
    activation: u64,
    operation_state: SimulationDebugFrameOperationV1,
    parent: SimulationDebugFrameParentV1,
}

impl SimulationDebugFrameOriginV1 {
    pub(crate) fn from_runtime(
        legacy_depth: u32,
        function_ordinal: usize,
        block: BlockId,
        next_operation: Option<u32>,
        state: &crate::debug_identity_state::FrameIdentityState,
    ) -> Option<Self> {
        Some(Self {
            legacy_depth,
            function_ordinal,
            block,
            next_operation,
            activation: state.identity().activation().get(),
            operation_state: state.observed_phase()?,
            parent: match state.parent() {
                Some(parent) => SimulationDebugFrameParentV1::Caller {
                    activation: parent.activation().get(),
                    attempt: parent.attempt().get(),
                    call_site: parent.site(),
                },
                None if state.identity().activation().get() == 1 => {
                    SimulationDebugFrameParentV1::Root
                }
                None => return None,
            },
        })
    }
    pub const fn legacy_depth(self) -> u32 {
        self.legacy_depth
    }
    pub const fn function_ordinal(self) -> usize {
        self.function_ordinal
    }
    pub const fn block(self) -> BlockId {
        self.block
    }
    pub const fn next_operation(self) -> Option<u32> {
        self.next_operation
    }
    pub const fn activation(self) -> u64 {
        self.activation
    }
    pub const fn operation_state(self) -> SimulationDebugFrameOperationV1 {
        self.operation_state
    }
    pub const fn parent(self) -> SimulationDebugFrameParentV1 {
        self.parent
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationDebugFrameOriginUnavailableV1 {
    NotRequested,
    NotCheckpoint,
    LegacyStackUnavailable {
        reason: SimulationDebugUnavailableReasonV1,
        required: u64,
    },
    IdentityInvariant,
}

pub(crate) trait FrameOriginsSourceV1 {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> Option<SimulationDebugFrameOriginV1>;
}

/// Immutable projection over the live frames during the synchronous callback.
/// There is no per-callback vector allocation and the borrow cannot escape.
#[derive(Clone, Copy)]
pub struct SimulationDebugFrameOriginsV1<'a> {
    source: &'a dyn FrameOriginsSourceV1,
}
impl<'a> SimulationDebugFrameOriginsV1<'a> {
    pub(crate) fn new(source: &'a dyn FrameOriginsSourceV1) -> Self {
        Self { source }
    }
    pub fn len(self) -> usize {
        self.source.len()
    }
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    pub fn get(self, index: usize) -> Option<SimulationDebugFrameOriginV1> {
        self.source.get(index)
    }
}
impl std::fmt::Debug for SimulationDebugFrameOriginsV1<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SimulationDebugFrameOriginsV1")
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SimulationDebugCheckpointFramesV1<'a> {
    Captured(SimulationDebugFrameOriginsV1<'a>),
    Unavailable(SimulationDebugFrameOriginUnavailableV1),
}
