//! Private runtime hooks. Observation failure never changes kernel execution.
use super::*;
use crate::debug_identity_state::{FrameIdentityState, InvocationIdentityState};
use crate::{
    SimulationDebugOperationOriginV1, SimulationDebugOriginContextV1,
    SimulationDebugOriginUnavailableV1,
};

impl<S: SimulationEventSinkV1> Engine<'_, S> {
    /// Clear even for the same invocation: aggregate paths can reuse its exact
    /// representative site without representing one operation attempt.
    pub(super) fn select_debug_invocation(&mut self, invocation: Option<SimulationInvocationV1>) {
        self.invocation = invocation;
        self.debug_origin = None;
    }

    fn identity_active(&self) -> bool {
        self.debug_origin_requested && !self.debug_identity_failed
    }

    fn identity_failed(&mut self) {
        self.debug_identity_failed = true;
        self.debug_origin = None;
    }

    pub(super) fn begin_debug_operation(
        &mut self,
        frame: &mut RuntimeFrame<'_>,
        site: CompactSite,
    ) {
        self.debug_origin = None;
        if !self.identity_active() {
            return;
        }
        let Some(operation) = site.operation else {
            self.identity_failed();
            return;
        };
        let location = SimulationDebugSiteV1 {
            function_ordinal: self.function_module_indices[site.function],
            block: site.block,
            operation,
        };
        let result = frame
            .debug_identity
            .as_mut()
            .map(|state| state.begin(location));
        match result {
            Some(Ok(origin)) if Some(origin.frame().invocation()) == self.invocation => {
                self.debug_origin = Some(origin);
            }
            _ => self.identity_failed(),
        }
    }

    pub(super) fn suspend_debug_operation(&mut self, frame: &mut RuntimeFrame<'_>) {
        if !self.identity_active() {
            return;
        }
        let result = frame
            .debug_identity
            .as_mut()
            .and_then(|state| state.pending().map(|origin| state.suspend(origin)));
        if !matches!(result, Some(Ok(()))) {
            self.identity_failed();
        }
        self.debug_origin = None;
    }

    pub(super) fn restore_debug_operation(&mut self, frame: &RuntimeFrame<'_>, site: CompactSite) {
        self.debug_origin = None;
        if !self.identity_active() {
            return;
        }
        let origin = frame
            .debug_identity
            .as_ref()
            .and_then(FrameIdentityState::pending);
        if let Some(origin) = origin
            && Some(origin.frame().invocation()) == self.invocation
            && origin.site().function_ordinal == self.function_module_indices[site.function]
            && origin.site().block == site.block
            && Some(origin.site().operation) == site.operation
        {
            self.debug_origin = Some(origin);
        } else {
            self.identity_failed();
        }
    }

    pub(super) fn resume_debug_operation(
        &mut self,
        frame: &mut RuntimeFrame<'_>,
        site: CompactSite,
    ) {
        self.restore_debug_operation(frame, site);
        if !self.identity_active() {
            return;
        }
        let result = frame
            .debug_identity
            .as_mut()
            .and_then(|state| state.pending().map(|origin| state.resume(origin)));
        if !matches!(result, Some(Ok(_))) {
            self.identity_failed();
        }
    }

    pub(super) fn complete_debug_operation(&mut self, frame: &mut RuntimeFrame<'_>) {
        if !self.identity_active() {
            return;
        }
        let result = frame
            .debug_identity
            .as_mut()
            .and_then(|state| state.pending().map(|origin| state.complete(origin)));
        if !matches!(result, Some(Ok(()))) {
            self.identity_failed();
        }
        self.debug_origin = None;
    }

    pub(super) fn retire_debug_frame(&mut self, frame: &mut RuntimeFrame<'_>) {
        if !self.identity_active() {
            return;
        }
        if !matches!(
            frame
                .debug_identity
                .as_mut()
                .map(FrameIdentityState::retire),
            Some(Ok(()))
        ) {
            self.identity_failed();
        }
        self.debug_origin = None;
    }

    pub(super) fn operation_origin_context(
        &self,
        record: &SimulationDebugRecordV1,
    ) -> SimulationDebugOriginContextV1 {
        use SimulationDebugOriginContextV1::{Available, Unavailable};
        use SimulationDebugOriginUnavailableV1::*;
        if !self.debug_origin_requested {
            return Unavailable(NotRequested);
        }
        if matches!(
            record.kind,
            SimulationDebugRecordKindV1::WorkgroupBarrier {
                action: SimulationDebugBarrierActionV1::Release,
                ..
            }
        ) {
            return Unavailable(AggregateRecord);
        }
        if self.debug_identity_failed {
            return Unavailable(IdentityInvariant);
        }
        match self.debug_origin {
            Some(origin)
                if origin.frame().invocation() == record.invocation
                    && origin.site() == record.site =>
            {
                Available(SimulationDebugOperationOriginV1::from_runtime(origin))
            }
            _ => Unavailable(NoMatchingOperation),
        }
    }
}

impl InvocationMachine<'_> {
    /// Called only after successful semantic helper entry/reset. Frame storage
    /// reuse retires and replaces its previous activation.
    pub(super) fn enter_debug_frame(
        &mut self,
        engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    ) {
        if !engine.identity_active() {
            return;
        }
        let slot = &mut self.frames[self.active_depth];
        let ordinal = engine.function_module_indices[slot.function_index];
        let Some(owner) = self.debug_identity.as_mut() else {
            engine.identity_failed();
            return;
        };
        let result = match slot.debug_identity.as_mut() {
            Some(state) => owner.reset_frame(state, ordinal),
            None => owner
                .enter_frame(ordinal)
                .map(|state| slot.debug_identity = Some(state)),
        };
        if result.is_err() {
            engine.identity_failed();
        }
    }
}

pub(super) fn root_identity(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    invocation: SimulationInvocationV1,
    function_index: usize,
) -> (Option<InvocationIdentityState>, Option<FrameIdentityState>) {
    if engine.identity_active()
        && let Ok((owner, root)) = InvocationIdentityState::new(
            invocation,
            engine.function_module_indices[function_index],
            engine.limits,
        )
    {
        (Some(owner), Some(root))
    } else {
        // Invalid limits are refused before execution. If that invariant ever
        // changes, the first attempted observation fails closed, not execution.
        (None, None)
    }
}
