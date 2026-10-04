//! Project real active frames without copying or retaining interpreter storage.
use super::*;
use crate::debug_runtime_frames::FrameOriginsSourceV1;
use crate::{
    SimulationDebugCheckpointFramesV1 as Frames, SimulationDebugFrameOperationV1,
    SimulationDebugFrameOriginUnavailableV1 as Missing, SimulationDebugFrameOriginV1,
    SimulationDebugFrameOriginsV1, SimulationDebugFrameParentV1,
};

pub(super) struct RuntimeFrameOrigins<'a, 'module> {
    pub(super) frames: &'a [RuntimeFrame<'module>],
    pub(super) function_module_indices: &'a [usize],
    pub(super) invocation: SimulationInvocationV1,
}

impl FrameOriginsSourceV1 for RuntimeFrameOrigins<'_, '_> {
    fn len(&self) -> usize {
        self.frames.len()
    }
    fn get(&self, index: usize) -> Option<SimulationDebugFrameOriginV1> {
        let frame = self.frames.get(index)?;
        let state = frame.debug_identity.as_ref()?;
        if state.identity().invocation() != self.invocation {
            return None;
        }
        let ordinal = *self.function_module_indices.get(frame.function_index)?;
        let next = frame
            .function
            .body
            .as_ref()
            .and_then(|body| body.blocks.get(frame.current_index))
            .and_then(|block| block.operations.get(frame.operation))
            .and_then(|_| u32::try_from(frame.operation).ok());
        let row = SimulationDebugFrameOriginV1::from_runtime(
            u32::try_from(index).ok()?,
            ordinal,
            frame.current,
            next,
            state,
        )?;
        match row.operation_state() {
            SimulationDebugFrameOperationV1::ActiveOperation { site, .. }
            | SimulationDebugFrameOperationV1::Suspended { site, .. }
                if site.function_ordinal != ordinal =>
            {
                return None;
            }
            _ => {}
        }
        Some(row)
    }
}

pub(super) fn context<'a>(
    record: &SimulationDebugRecordV1,
    source: &'a RuntimeFrameOrigins<'_, '_>,
    requested: bool,
    identity_failed: bool,
) -> Frames<'a> {
    if !requested {
        return Frames::Unavailable(Missing::NotRequested);
    }
    let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
        return Frames::Unavailable(Missing::NotCheckpoint);
    };
    let stack = match stack {
        SimulationDebugCollectionV1::Unavailable { reason, required } => {
            return Frames::Unavailable(Missing::LegacyStackUnavailable {
                reason: *reason,
                required: *required,
            });
        }
        SimulationDebugCollectionV1::Captured(stack) => stack,
    };
    if identity_failed || stack.is_empty() || stack.len() != source.len() {
        return Frames::Unavailable(Missing::IdentityInvariant);
    }
    let mut previous: Option<SimulationDebugFrameOriginV1> = None;
    for (index, legacy) in stack.iter().enumerate() {
        let Some(row) = source.get(index) else {
            return Frames::Unavailable(Missing::IdentityInvariant);
        };
        if row.legacy_depth() != legacy.depth
            || row.function_ordinal() != legacy.function_ordinal
            || row.block() != legacy.block
            || row.next_operation() != legacy.next_operation
            || row.activation() == 0
        {
            return Frames::Unavailable(Missing::IdentityInvariant);
        }
        match (previous, row.parent()) {
            (None, SimulationDebugFrameParentV1::Root) => {}
            (
                Some(caller),
                SimulationDebugFrameParentV1::Caller {
                    activation,
                    attempt,
                    call_site,
                },
            ) if row.activation() > caller.activation()
                && activation == caller.activation()
                && matches!(caller.operation_state(),
                        SimulationDebugFrameOperationV1::Suspended { attempt: pending, site }
                        if pending == attempt && site == call_site) => {}
            _ => return Frames::Unavailable(Missing::IdentityInvariant),
        }
        previous = Some(row);
    }
    Frames::Captured(SimulationDebugFrameOriginsV1::new(source))
}
