//! Native sessions and directional queues share one concrete ownership root.

#![forbid(unsafe_code)]

use super::*;
use fe2o3_kfd::Gfx942XgmiRetainedEndpointBackingUsageV1;

pub(super) type NativeXgmiFullCustodyV1 =
    NativeCustody<SharedGttMemorySessionV1, Gfx942NativeXgmiSdmaQueueV1>;

type NativeXgmiPartsMutV1<'a> = (
    &'a mut [SharedGttMemorySessionV1; 2],
    &'a mut [Option<Gfx942NativeXgmiSdmaQueueV1>; 2],
);

pub(super) enum NativeXgmiStateV1 {
    Full(NativeXgmiFullCustodyV1),
    Retained(super::xgmi_retained::Retained),
    Failed(super::xgmi_retained::Failed),
}

pub(super) struct NativeXgmiCustodyV1 {
    pub(super) state: Option<NativeXgmiStateV1>,
}

impl NativeXgmiCustodyV1 {
    pub(super) fn new(sessions: [SharedGttMemorySessionV1; 2]) -> Self {
        Self {
            state: Some(NativeXgmiStateV1::Full(NativeCustody::new(sessions))),
        }
    }

    pub(super) fn is_full(&self) -> bool {
        matches!(self.state, Some(NativeXgmiStateV1::Full(_)))
    }

    pub(super) fn queue_count(&self) -> usize {
        match &self.state {
            Some(NativeXgmiStateV1::Full(full)) => full.queues().iter().flatten().count(),
            Some(NativeXgmiStateV1::Retained(retained)) => retained.queue_count(),
            Some(NativeXgmiStateV1::Failed(failed)) => failed.queue_count(),
            None => std::process::abort(),
        }
    }

    pub(super) fn is_terminal(&self) -> bool {
        match &self.state {
            Some(NativeXgmiStateV1::Full(full)) => full
                .queues()
                .iter()
                .flatten()
                .any(Gfx942NativeXgmiSdmaQueueV1::has_terminal_retirement_v1),
            Some(NativeXgmiStateV1::Retained(retained)) => retained.owner.is_terminal(),
            Some(NativeXgmiStateV1::Failed(_)) | None => true,
        }
    }

    fn unavailable(&self) -> RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1> {
        if self.is_terminal() {
            RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "native XGMI custody is terminal",
            ))
        } else {
            KfdNativeXgmiRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native XGMI custody is retained",
            )
        }
    }

    pub(super) fn full(
        &self,
    ) -> Result<&NativeXgmiFullCustodyV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        match &self.state {
            Some(NativeXgmiStateV1::Full(full)) => Ok(full),
            _ => Err(self.unavailable()),
        }
    }

    pub(super) fn full_mut(
        &mut self,
    ) -> Result<&mut NativeXgmiFullCustodyV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        if !self.is_full() {
            return Err(self.unavailable());
        }
        match &mut self.state {
            Some(NativeXgmiStateV1::Full(full)) => Ok(full),
            _ => std::process::abort(),
        }
    }

    pub(super) fn queues(
        &self,
    ) -> Result<
        &[Option<Gfx942NativeXgmiSdmaQueueV1>; 2],
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        Ok(self.full()?.queues())
    }

    pub(super) fn sessions_mut(
        &mut self,
    ) -> Result<&mut [SharedGttMemorySessionV1; 2], RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        Ok(self.full_mut()?.sessions_mut())
    }

    pub(super) fn parts_mut(
        &mut self,
    ) -> Result<NativeXgmiPartsMutV1<'_>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        Ok(self.full_mut()?.parts_mut())
    }

    pub(super) fn backing_usage(&self) -> [Gfx942XgmiRetainedEndpointBackingUsageV1; 2] {
        match &self.state {
            Some(NativeXgmiStateV1::Full(full)) => {
                full.sessions()
                    .each_ref()
                    .map(|s| Gfx942XgmiRetainedEndpointBackingUsageV1 {
                        device: s.device_backing_usage_v1(),
                        host_visible: s.host_visible_backing_usage_v1(),
                        native: s.native_backing_usage_v1(),
                    })
            }
            Some(NativeXgmiStateV1::Retained(retained)) => {
                endpoint_order(retained.owner.backing_usage_v1(), retained.direction)
            }
            Some(NativeXgmiStateV1::Failed(failed)) => {
                endpoint_order(failed.owner.backing_usage_v1(), failed.direction)
            }
            None => std::process::abort(),
        }
    }
}

pub(super) fn endpoint_order<T>(mut endpoints: [T; 2], direction: usize) -> [T; 2] {
    match direction {
        0 => {}
        1 => endpoints.swap(0, 1),
        _ => std::process::abort(),
    }
    endpoints
}

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

    pub(super) fn into_direction(mut self, direction: usize) -> (Q, S, S, Option<Q>) {
        let queue = self
            .queues
            .get_mut(direction)
            .and_then(Option::take)
            .unwrap_or_else(|| std::process::abort());
        let opposite = self.queues[1 - direction].take();
        let [source, destination] = endpoint_order(self.sessions, direction);
        (queue, source, destination, opposite)
    }

    pub(super) fn from_direction(
        queue: Q,
        source: S,
        destination: S,
        opposite: Option<Q>,
        direction: usize,
    ) -> Self {
        Self {
            sessions: endpoint_order([source, destination], direction),
            queues: endpoint_order([Some(queue), opposite], direction),
        }
    }
}

#[cfg(test)]
mod tests;
