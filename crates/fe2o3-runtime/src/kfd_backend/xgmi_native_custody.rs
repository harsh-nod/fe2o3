//! Native sessions and directional queues share one concrete ownership root.

#![forbid(unsafe_code)]

use super::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1};

pub(super) type NativeXgmiCustodyV1 =
    NativeCustody<SharedGttMemorySessionV1, Gfx942NativeXgmiSdmaQueueV1>;

// Keep the former backend field destruction order. Backend Drop must retire
// both queues, or abort, before this storage reaches ordinary field Drop.
pub(super) struct NativeCustody<S, Q> {
    sessions: [S; 2],
    queues: [Option<Q>; 2],
}

impl<S, Q> NativeCustody<S, Q> {
    pub(super) fn new(sessions: [S; 2]) -> Self {
        Self {
            sessions,
            queues: [None, None],
        }
    }

    pub(super) fn sessions(&self) -> &[S; 2] {
        &self.sessions
    }

    pub(super) fn sessions_mut(&mut self) -> &mut [S; 2] {
        &mut self.sessions
    }

    pub(super) fn queues(&self) -> &[Option<Q>; 2] {
        &self.queues
    }

    pub(super) fn parts_mut(&mut self) -> (&mut [S; 2], &mut [Option<Q>; 2]) {
        (&mut self.sessions, &mut self.queues)
    }
}

#[cfg(test)]
mod tests;
