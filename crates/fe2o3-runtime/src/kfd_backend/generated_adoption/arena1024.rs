//! One original native arena, with independently retained single-use receipts.

use super::*;
use crate::generated_source::GeneratedProfileV1;
use fe2o3_kfd::{
    Gfx942NativeFillArenaBatchV1, Gfx942NativeFillArenaCompletedV1, Gfx942NativeFillArenaSessionV1,
    Gfx942NativeFillArenaStorageV1,
};
use fe2o3_resource_accounting::{HostMetadataTableV1, ResourceCreditAccountV1};

mod construction;
mod progress;
pub(crate) const SLOTS: usize = fe2o3_kfd::GFX942_NATIVE_FILL_ARENA_SLOTS_V1;

struct Cell {
    receipt: ReceiptV1<Gfx942NativeFillArenaBatchV1, Gfx942NativeFillArenaCompletedV1>,
    copied: bool,
}

/// Entire finite metadata/scratch allocation precedes any native adoption.
pub(crate) struct ArenaPreallocationV1 {
    lower: Option<Gfx942NativeFillArenaStorageV1>,
    cells: HostMetadataTableV1<Cell>,
    initialization: Option<HostMetadataTableV1<u8>>,
}

impl ArenaPreallocationV1 {
    pub(crate) fn new(account: &ResourceCreditAccountV1, bytes: u64) -> Result<Self, ()> {
        let bytes = usize::try_from(bytes).map_err(|_| ())?;
        if bytes == 0 {
            return Err(());
        }
        let cells = HostMetadataTableV1::try_new(SLOTS, Some(account), || Cell {
            receipt: ReceiptV1::Ready,
            copied: false,
        })
        .map_err(|_| ())?;
        let initialization =
            HostMetadataTableV1::try_new(bytes, Some(account), || 0).map_err(|_| ())?;
        let lower = Gfx942NativeFillArenaStorageV1::preallocate(account.clone()).map_err(|_| ())?;
        Ok(Self {
            lower: Some(lower),
            cells,
            initialization: Some(initialization),
        })
    }
}

pub(in crate::kfd_backend) struct ArenaV1 {
    phase: PhaseV1,
    data: Option<Gfx942FixedDispatchDataV1>,
    storage: ArenaPreallocationV1,
    session: Option<Gfx942NativeFillArenaSessionV1>,
}

impl ArenaV1 {
    pub(in crate::kfd_backend) fn is_retired(&self) -> bool {
        self.phase == PhaseV1::Retired
            && self.data.is_none()
            && self.session.is_none()
            && self.storage.lower.is_none()
            && self.storage.initialization.is_none()
            && self.storage.cells.len() == SLOTS
            && self
                .storage
                .cells
                .iter()
                .all(|cell| cell.copied && matches!(cell.receipt, ReceiptV1::Recycled))
    }

    pub(in crate::kfd_backend) fn with_device<R>(
        &mut self,
        observe: impl FnOnce(&CheckedGfx942XnackMinusDevice) -> R,
    ) -> Result<R, fe2o3_kfd::ComputeAqlQueueSessionErrorV1> {
        self.session
            .as_mut()
            .ok_or(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                "arena original session unavailable",
            ))?
            .with_retained_device_v1(observe)
    }
}

impl KfdRuntimeBackendV1 {
    fn check_arena_device_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let actual = self
            .with_retained_preparation_device_v1(plan.binding.backend_device, |device| {
                device.model_admission()
            })?;
        if actual != plan.binding.native_device
            || self.streams.get(&plan.binding.backend_stream) != Some(&plan.binding.backend_device)
        {
            return Err(self.terminal_error("arena original device or stream mismatch"));
        }
        Ok(())
    }

    fn validate_arena_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if plan.profile != GeneratedProfileV1::NativeFillArena1024
            || plan.count != 1
            || !self.validate_generated_shell_records_v1(plan)
            || !readback::roster_matches_plan_v1(plan, roster)
            || !self.generated_shells.get(&plan.key).is_some_and(|record| {
                record.native.is_none()
                    && record.registry.is_none()
                    && record.control.is_none()
                    && record.source_identity.matches(&roster.source_identity)
                    && record.arena.as_ref().is_some_and(|arena| {
                        arena.phase == PhaseV1::Adopted
                            && arena.session.is_some()
                            && arena.storage.cells.len() == SLOTS
                    })
            })
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "arena original common owner mismatch",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_generated_arena_shells_v1<P: crate::RuntimeGfx942GeneratedCarrierV1>(
        &mut self,
        authenticated: super::super::generated_shells::GeneratedShellCommitPlanV1,
        source: &mut crate::RuntimeGfx942GeneratedArena1024V1<P>,
        roster: &GeneratedHostRosterV1,
    ) {
        if !source.original_roster().matches(roster) || source.packets.is_none() {
            std::process::abort();
        }
        self.commit_generated_controls_v1(authenticated, roster, |control| {
            let generated_shells::GeneratedControlV1::Arena1024(packets) = control else {
                std::process::abort();
            };
            *packets = source.packets.take();
            packets.is_some()
        });
    }

    pub(crate) fn destroy_generated_arena_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        roster: &GeneratedHostRosterV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.validate_arena_v1(plan, roster)?;
        let arena = self
            .generated_shells
            .get_mut(&plan.key)
            .and_then(|record| record.arena.as_mut())
            .unwrap_or_else(|| std::process::abort());
        if !arena
            .storage
            .cells
            .iter()
            .all(|cell| cell.copied && matches!(cell.receipt, ReceiptV1::Recycled))
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "arena original recipes unfinished",
            ));
        }
        arena.phase = PhaseV1::Retiring;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_arena_device_v1(plan)?;
            let arena = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort());
            arena
                .session
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .destroy()
                .map_err(|error| {
                    self.generated_native_error_v1("arena common destruction", error)
                })?;
            let arena = self
                .generated_shells
                .get_mut(&plan.key)
                .and_then(|record| record.arena.as_mut())
                .unwrap_or_else(|| std::process::abort());
            drop(arena.session.take());
            arena.phase = PhaseV1::Retired;
            self.queue_retired = true;
            Ok(())
        }));
        self.finish_generated_native_call_v1(result)
    }
}
