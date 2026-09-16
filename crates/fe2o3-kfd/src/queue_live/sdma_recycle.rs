//! Recycling retains the original buffer or its disposal receipt until settlement.

#![forbid(unsafe_code)]

use super::*;
use crate::shared_memory::{DataCleanupCustodyV1, DispatchDataReleaseV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[allow(
    clippy::large_enum_variant,
    reason = "retain buffer and disposal custody inline without allocating during transfer"
)]
pub(super) enum SdmaRecycleCustodyV1 {
    Buffer(Gfx942SdmaBufferV1),
    Disposal(DataCleanupCustodyV1),
}

impl SdmaRecycleCustodyV1 {
    fn buffer(&self) -> &Gfx942SdmaBufferV1 {
        match self {
            Self::Buffer(buffer) => buffer,
            Self::Disposal(_) => unreachable!("recycle policy precedes disposal"),
        }
    }
}

pub(super) struct SdmaRecycleRootsV1<'a> {
    pub(super) custody: &'a mut Option<SdmaRecycleCustodyV1>,
    pub(super) free: &'a mut Vec<Gfx942SdmaBufferV1>,
    pub(super) outstanding: &'a mut usize,
}

pub(super) trait SdmaRecycleContextV1 {
    fn owner(&self) -> QueueKeyV1;
    // Infallible: foreign custody must be returned without admission or cleanup.
    fn note_foreign_attempt(&mut self);
    // Infallible root access must also work after model restoration or engine loss.
    fn roots(&mut self) -> SdmaRecycleRootsV1<'_>;
    fn admit(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn should_dispose(&self) -> Result<bool, ComputeAqlQueueSessionErrorV1>;
    fn reserve_cache(&mut self) -> bool;
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn release(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn poison(&mut self);
}

fn poison_without_replacing_failure<C: SdmaRecycleContextV1>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

#[allow(clippy::result_large_err)]
pub(super) fn recycle_in_place<C: SdmaRecycleContextV1>(
    context: &mut C,
    buffer: Gfx942SdmaBufferV1,
    force_disposal: bool,
) -> Result<(), Gfx942SdmaBufferTransitionFailureV1> {
    if !buffer.belongs_to(context.owner()) {
        context.note_foreign_attempt();
        return Err(Gfx942SdmaBufferTransitionFailureV1 {
            error: ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
            recovered: Some(buffer),
        });
    }
    let roots = context.roots();
    if roots.custody.is_some() {
        // The legacy result can return only healthy retryable custody. Neither
        // overwriting an unfinished owner nor returning a terminal one is valid.
        std::process::abort();
    }
    *roots.custody = Some(SdmaRecycleCustodyV1::Buffer(buffer));
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.admit()?;
        if *context.roots().outstanding == 0 {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA buffer ledger underflow",
            ));
        }
        if force_disposal || context.should_dispose()? {
            let root = context.roots().custody;
            let Some(SdmaRecycleCustodyV1::Buffer(buffer)) = root.take() else {
                unreachable!("installed recycle input");
            };
            *root = Some(SdmaRecycleCustodyV1::Disposal(
                DataCleanupCustodyV1::from_sdma(buffer),
            ));
            let (lower, retake) = execute_live_model_custody_v1(
                context,
                C::loan,
                C::release,
                C::retake,
                poison_without_replacing_failure,
            )?;
            retake?;
            lower?;
            let roots = context.roots();
            if !matches!(roots.custody, Some(SdmaRecycleCustodyV1::Disposal(root)) if root.is_complete())
            {
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "incomplete SDMA recycle disposal",
                ));
            }
            let remaining =
                roots
                    .outstanding
                    .checked_sub(1)
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA buffer ledger underflow",
                    ))?;
            *roots.outstanding = remaining;
            *roots.custody = None;
        } else {
            if !context.reserve_cache() {
                return Ok(false);
            }
            let roots = context.roots();
            let remaining =
                roots
                    .outstanding
                    .checked_sub(1)
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "SDMA buffer ledger underflow",
                    ))?;
            let Some(SdmaRecycleCustodyV1::Buffer(buffer)) = roots.custody.as_mut() else {
                unreachable!("cache retains original buffer");
            };
            buffer.advance_pool_generation()?;
            let Some(SdmaRecycleCustodyV1::Buffer(buffer)) = roots.custody.take() else {
                unreachable!("cache retains advanced buffer");
            };
            roots.free.push(buffer);
            *roots.outstanding = remaining;
        }
        Ok(true)
    }));
    match result {
        Ok(Ok(true)) => Ok(()),
        Ok(Ok(false)) => {
            let Some(SdmaRecycleCustodyV1::Buffer(buffer)) = context.roots().custody.take() else {
                unreachable!("reservation rejection retains original buffer");
            };
            Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: ComputeAqlQueueSessionErrorV1::Contract("SDMA pool allocation failed"),
                recovered: Some(buffer),
            })
        }
        Ok(Err(error)) => {
            poison_without_replacing_failure(context);
            Err(Gfx942SdmaBufferTransitionFailureV1 {
                error,
                recovered: None,
            })
        }
        Err(payload) => {
            poison_without_replacing_failure(context);
            resume_unwind(payload)
        }
    }
}

impl SdmaRecycleContextV1 for ComputeAqlQueueSessionV1 {
    fn owner(&self) -> QueueKeyV1 {
        self.key
    }

    fn note_foreign_attempt(&mut self) {
        if !self.terminal_poisoned
            && self.sdma_pool_trim.is_none()
            && self.sdma_allocation.is_none()
            && self.sdma_promotion.is_none()
            && self.sdma_recycle.is_none()
        {
            self.sdma_device_pool.begin_activity();
        }
    }

    fn roots(&mut self) -> SdmaRecycleRootsV1<'_> {
        SdmaRecycleRootsV1 {
            custody: &mut self.sdma_recycle,
            free: &mut self.sdma_pool_free,
            outstanding: &mut self.sdma_outstanding_buffers,
        }
    }

    fn admit(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.sdma_device_pool.begin_activity();
        self.require_sdma_enabled_state_v1()
    }

    fn should_dispose(&self) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        let buffer = self
            .sdma_recycle
            .as_ref()
            .expect("installed recycle input")
            .buffer();
        if let Some(limits) = self.sdma_device_pool.limits
            && buffer.kind() == Gfx942SdmaBufferKindV1::DeviceLocal
        {
            let engine = self
                .engine
                .as_ref()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?;
            return device_pool_recycle_decision_v1(
                &engine.backend.session,
                self.key,
                limits,
                &self.sdma_pool_free,
                buffer,
            )
            .map(|disposition| disposition == DevicePoolDispositionV1::Dispose)
            .map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("invalid SDMA device pool recycle")
            });
        }
        if let Some(limits) = self.sdma_host_pool_limits
            && buffer.kind() == Gfx942SdmaBufferKindV1::HostVisibleCoherent
        {
            let engine = self
                .engine
                .as_ref()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "missing queue engine",
                ))?;
            return host_pool_recycle_decision_v1(
                &engine.backend.session,
                self.key,
                limits,
                &self.sdma_pool_free,
                buffer,
            )
            .map(|disposition| disposition == HostPoolDispositionV1::Dispose)
            .map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("invalid SDMA host pool recycle")
            });
        }
        Ok(false)
    }

    fn reserve_cache(&mut self) -> bool {
        self.sdma_pool_free.try_reserve(1).is_ok()
    }

    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }

    fn release(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        let Some(SdmaRecycleCustodyV1::Disposal(root)) = self.sdma_recycle.as_mut() else {
            unreachable!("installed recycle disposal");
        };
        engine
            .backend
            .session
            .release_data(root)
            .map_err(crate::sdma::Gfx942SdmaErrorV1::from)?;
        Ok(())
    }

    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.retake_model_ownership_after_live_mutation(loan)
    }

    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}
