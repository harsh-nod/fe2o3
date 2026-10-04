//! Fixed target transaction; neither this progress nor a report is an authority input.
use super::super::{Gfx950DebugAllocationRetirementV1 as Retirement, Gfx950DebugLocalErrorV1 as E};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugOneStopStepV1 {
    ValidateFixedArtifact,
    PrepareEmpty,
    AllocateOutput,
    PreparePacket,
    OwnedCheckpoint,
    ReservePacket,
    WriteCounter,
    WriteBody,
    ReleaseHeader,
    Doorbell,
    ObserveCompletion,
    ValidateOutput,
    DestroyQueue,
    DestroyEvent,
    WithdrawMetadata,
    DisableRuntime,
    ClearTrap,
    UnmapDoorbell,
    RetireAllocation { ordinal: u8, operation: Retirement },
    Reconcile,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugOneStopPhaseV1 {
    Ready(Gfx950DebugOneStopStepV1),
    Attempting(Gfx950DebugOneStopStepV1),
    LocalBackingRetired,
}
pub(super) struct Cursor {
    phase: Gfx950DebugOneStopPhaseV1,
    pid: u32,
    publication_possible: bool,
}
impl Cursor {
    pub(super) fn new(pid: u32) -> Self {
        Self {
            phase: Gfx950DebugOneStopPhaseV1::Ready(
                Gfx950DebugOneStopStepV1::ValidateFixedArtifact,
            ),
            pid,
            publication_possible: false,
        }
    }
    pub(super) fn phase(&self) -> Gfx950DebugOneStopPhaseV1 {
        self.phase
    }
    pub(super) fn publication_possible(&self) -> bool {
        self.publication_possible
    }
    pub(super) fn run(
        &mut self,
        pid: u32,
        operation: impl FnOnce(Gfx950DebugOneStopStepV1) -> Result<(), E>,
    ) -> Result<(), E> {
        if self.pid == 0 || pid != self.pid {
            return Err(E::ProcessChanged);
        }
        let Gfx950DebugOneStopPhaseV1::Ready(step) = self.phase else {
            return Err(E::Phase);
        };
        self.phase = Gfx950DebugOneStopPhaseV1::Attempting(step);
        // Conservative: precedes even the retained ring-model reservation. There is
        // no retry after a panic or failure before/within any visible mutation.
        if step == Gfx950DebugOneStopStepV1::ReservePacket {
            self.publication_possible = true;
        }
        operation(step)?;
        self.phase = next(step);
        Ok(())
    }
}
fn next(step: Gfx950DebugOneStopStepV1) -> Gfx950DebugOneStopPhaseV1 {
    use Gfx950DebugOneStopPhaseV1 as P;
    use Gfx950DebugOneStopStepV1 as S;
    P::Ready(match step {
        S::ValidateFixedArtifact => S::PrepareEmpty,
        S::PrepareEmpty => S::AllocateOutput,
        S::AllocateOutput => S::PreparePacket,
        S::PreparePacket => S::OwnedCheckpoint,
        S::OwnedCheckpoint => S::ReservePacket,
        S::ReservePacket => S::WriteCounter,
        S::WriteCounter => S::WriteBody,
        S::WriteBody => S::ReleaseHeader,
        S::ReleaseHeader => S::Doorbell,
        S::Doorbell => S::ObserveCompletion,
        S::ObserveCompletion => S::ValidateOutput,
        S::ValidateOutput => S::DestroyQueue,
        S::DestroyQueue => S::DestroyEvent,
        S::DestroyEvent => S::WithdrawMetadata,
        S::WithdrawMetadata => S::DisableRuntime,
        S::DisableRuntime => S::ClearTrap,
        S::ClearTrap => S::UnmapDoorbell,
        S::UnmapDoorbell => S::RetireAllocation {
            ordinal: 0,
            operation: Retirement::UnmapGpu,
        },
        S::RetireAllocation { ordinal, operation } => match operation {
            Retirement::UnmapGpu => S::RetireAllocation {
                ordinal,
                operation: Retirement::UnmapCpu,
            },
            Retirement::UnmapCpu => S::RetireAllocation {
                ordinal,
                operation: Retirement::FreeGpuHandle,
            },
            Retirement::FreeGpuHandle => S::RetireAllocation {
                ordinal,
                operation: Retirement::ReleaseVa,
            },
            Retirement::ReleaseVa => S::RetireAllocation {
                ordinal,
                operation: Retirement::ReconcileAccounting,
            },
            Retirement::ReconcileAccounting if ordinal < 8 => S::RetireAllocation {
                ordinal: ordinal + 1,
                operation: Retirement::UnmapGpu,
            },
            Retirement::ReconcileAccounting => S::Reconcile,
        },
        S::Reconcile => return P::LocalBackingRetired,
    })
}
#[cfg(test)]
#[path = "engineering_gfx950_debug_one_stop_cursor_v1_tests.rs"]
mod tests;
