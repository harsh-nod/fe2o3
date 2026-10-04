//! The actual local transaction cursor. No state deserialization or reset.
use super::Gfx950DebugLocalErrorV1;

/// Descriptive progress only. Supplying this enum grants no native operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugLocalStepV1 {
    Revalidate,
    RegisterRuntime,
    CreateEvent,
    AllocateRing,
    AllocateControl,
    InitializeControl,
    AllocateSignal,
    InitializeSignal,
    AllocateKernarg,
    AllocateEop,
    AllocateCwsr,
    CreateQueue,
    MapDoorbell,
    InspectEmpty,
    DestroyQueue,
    DestroyEvent,
    WithdrawMetadata,
    DisableRuntime,
    ClearTrap,
    UnmapDoorbell,
    RetireAllocation {
        ordinal: u8,
        operation: Gfx950DebugAllocationRetirementV1,
    },
    Reconcile,
}
/// Fixed in-place retirement order; never accepted as an input operation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugAllocationRetirementV1 {
    UnmapGpu,
    UnmapCpu,
    FreeGpuHandle,
    ReleaseVa,
    ReconcileAccounting,
}
/// A failed or panicked attempt remains Attempting permanently. There is no retry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950DebugLocalPhaseV1 {
    Ready(Gfx950DebugLocalStepV1),
    Attempting(Gfx950DebugLocalStepV1),
    LocalBackingRetired,
}

pub(super) struct Cursor {
    phase: Gfx950DebugLocalPhaseV1,
    opener_pid: u32,
}
impl Cursor {
    pub(super) fn new(opener_pid: u32) -> Self {
        Self {
            phase: Gfx950DebugLocalPhaseV1::Ready(Gfx950DebugLocalStepV1::Revalidate),
            opener_pid,
        }
    }
    pub(super) const fn phase(&self) -> Gfx950DebugLocalPhaseV1 {
        self.phase
    }
    pub(super) fn run(
        &mut self,
        observed_pid: u32,
        operation: impl FnOnce(Gfx950DebugLocalStepV1) -> Result<(), Gfx950DebugLocalErrorV1>,
    ) -> Result<(), Gfx950DebugLocalErrorV1> {
        // Before any closure, native request or inherited global mutex.
        if self.opener_pid == 0 || self.opener_pid != observed_pid {
            return Err(Gfx950DebugLocalErrorV1::ProcessChanged);
        }
        let Gfx950DebugLocalPhaseV1::Ready(step) = self.phase else {
            return Err(Gfx950DebugLocalErrorV1::Phase);
        };
        self.phase = Gfx950DebugLocalPhaseV1::Attempting(step);
        operation(step)?;
        self.phase = next(step);
        Ok(())
    }
}
fn next(step: Gfx950DebugLocalStepV1) -> Gfx950DebugLocalPhaseV1 {
    use Gfx950DebugAllocationRetirementV1 as A;
    use Gfx950DebugLocalPhaseV1 as P;
    use Gfx950DebugLocalStepV1 as S;
    P::Ready(match step {
        S::Revalidate => S::RegisterRuntime,
        S::RegisterRuntime => S::CreateEvent,
        S::CreateEvent => S::AllocateRing,
        S::AllocateRing => S::AllocateControl,
        S::AllocateControl => S::InitializeControl,
        S::InitializeControl => S::AllocateSignal,
        S::AllocateSignal => S::InitializeSignal,
        S::InitializeSignal => S::AllocateKernarg,
        S::AllocateKernarg => S::AllocateEop,
        S::AllocateEop => S::AllocateCwsr,
        S::AllocateCwsr => S::CreateQueue,
        S::CreateQueue => S::MapDoorbell,
        S::MapDoorbell => S::InspectEmpty,
        S::InspectEmpty => S::DestroyQueue,
        S::DestroyQueue => S::DestroyEvent,
        S::DestroyEvent => S::WithdrawMetadata,
        S::WithdrawMetadata => S::DisableRuntime,
        S::DisableRuntime => S::ClearTrap,
        S::ClearTrap => S::UnmapDoorbell,
        S::UnmapDoorbell => S::RetireAllocation {
            ordinal: 0,
            operation: A::UnmapGpu,
        },
        S::RetireAllocation { ordinal, operation } => match operation {
            A::UnmapGpu => S::RetireAllocation {
                ordinal,
                operation: A::UnmapCpu,
            },
            A::UnmapCpu => S::RetireAllocation {
                ordinal,
                operation: A::FreeGpuHandle,
            },
            A::FreeGpuHandle => S::RetireAllocation {
                ordinal,
                operation: A::ReleaseVa,
            },
            A::ReleaseVa => S::RetireAllocation {
                ordinal,
                operation: A::ReconcileAccounting,
            },
            A::ReconcileAccounting if ordinal < 7 => S::RetireAllocation {
                ordinal: ordinal + 1,
                operation: A::UnmapGpu,
            },
            A::ReconcileAccounting => S::Reconcile,
        },
        S::Reconcile => return P::LocalBackingRetired,
    })
}

#[cfg(test)]
#[path = "engineering_gfx950_debug_empty_cursor_v1_tests.rs"]
mod tests;
