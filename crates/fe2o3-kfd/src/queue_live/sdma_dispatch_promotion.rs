//! Keep the original coherent SDMA owner rooted through read and model retake.

#![forbid(unsafe_code)]

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[allow(
    clippy::large_enum_variant,
    reason = "retain exact input/output inline without allocating during native ownership transfer"
)]
pub(super) enum Custody {
    Input(Gfx942SdmaBufferV1),
    Output(Gfx942FixedDispatchDataV1, Gfx942SdmaDispatchDataBridgeV1),
}

pub(super) trait Context {
    type Loan;
    fn owner(&self) -> QueueKeyV1;
    fn preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn root(&mut self) -> &mut Option<Custody>;
    fn outstanding(&mut self) -> &mut usize;
    fn loan(&mut self) -> Result<Self::Loan, ComputeAqlQueueSessionErrorV1>;
    fn read(&mut self) -> Result<Box<[u8]>, ComputeAqlQueueSessionErrorV1>;
    fn retake(&mut self, loan: Self::Loan) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn poison(&mut self);
}

fn contract(message: &'static str) -> ComputeAqlQueueSessionErrorV1 {
    ComputeAqlQueueSessionErrorV1::Contract(message)
}

fn poison<C: Context>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

#[allow(clippy::result_large_err)]
pub(super) fn promote<C: Context>(
    context: &mut C,
    buffer: Gfx942SdmaBufferV1,
    content: Gfx942DeviceContentDescriptorV1,
) -> Result<
    (Gfx942FixedDispatchDataV1, Gfx942SdmaDispatchDataBridgeV1),
    Gfx942SdmaBufferTransitionFailureV1,
> {
    let admission = catch_unwind(AssertUnwindSafe(|| {
        // Preserve the existing borrowed refusal order before any native read.
        if !buffer.belongs_to(context.owner()) {
            return Err(contract("foreign SDMA buffer owner"));
        }
        context.preflight()?;
        if buffer.kind() != Gfx942SdmaBufferKindV1::HostVisibleCoherent
            || buffer.requested_bytes() != buffer.physical_bytes()
            || content.byte_len() != buffer.physical_bytes()
        {
            return Err(contract(
                "SDMA host promotion requires one exact full physical extent",
            ));
        }
        if context.root().is_some() {
            return Err(contract("unfinished SDMA dispatch promotion"));
        }
        Ok(())
    }));
    match admission {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return Err(Gfx942SdmaBufferTransitionFailureV1 {
                error,
                recovered: Some(buffer),
            });
        }
        Err(payload) => {
            if context.root().is_some() {
                std::process::abort();
            }
            *context.root() = Some(Custody::Input(buffer));
            poison(context);
            resume_unwind(payload)
        }
    }
    *context.root() = Some(Custody::Input(buffer));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (observed, closing) =
            execute_live_model_custody_v1(context, C::loan, C::read, C::retake, poison)?;
        // Closing failure keeps its original precedence over a read refusal.
        closing?;
        let observed = observed?;
        if !content_descriptor_matches_bytes(content, &observed) {
            return Ok(false);
        }
        let outstanding = *context.outstanding();
        if outstanding == 0 {
            return Err(contract("SDMA buffer ledger underflow"));
        }
        let Some(Custody::Input(buffer)) = context.root().take() else {
            std::process::abort();
        };
        let physical_bytes = buffer.physical_bytes();
        let storage_identity = buffer.storage_identity();
        let (storage, owner, pool_generation, logical_bytes) = buffer.into_bridge_parts();
        let Gfx942SdmaBufferStorageV1::Host(token) = storage else {
            // Exact immutable kind was admitted above. Never unwind/drop a
            // native owner if this internal representation invariant fails.
            std::process::abort();
        };
        let data = Gfx942FixedDispatchDataV1::host_visible_initialized(
            Gfx942InitializedHostVisibleMemoryV1::from_completed_dispatch(token),
        );
        let bridge = Gfx942SdmaDispatchDataBridgeV1 {
            owner,
            pool_generation,
            logical_bytes,
            physical_bytes,
            storage_identity,
        };
        // Conversion has no callbacks or native operations. Root its exact
        // result before debiting the original SDMA buffer ledger.
        *context.root() = Some(Custody::Output(data, bridge));
        *context.outstanding() = outstanding - 1;
        Ok(true)
    }));
    match result {
        Ok(Ok(true)) => {
            let Some(Custody::Output(data, bridge)) = context.root().take() else {
                std::process::abort();
            };
            Ok((data, bridge))
        }
        Ok(Ok(false)) => {
            let Some(Custody::Input(buffer)) = context.root().take() else {
                std::process::abort();
            };
            Err(Gfx942SdmaBufferTransitionFailureV1 {
                error: contract("SDMA host promotion content descriptor mismatch"),
                recovered: Some(buffer),
            })
        }
        Ok(Err(error)) => {
            poison(context);
            Err(Gfx942SdmaBufferTransitionFailureV1 {
                error,
                recovered: None,
            })
        }
        Err(payload) => {
            poison(context);
            resume_unwind(payload)
        }
    }
}

impl Context for ComputeAqlQueueSessionV1 {
    type Loan = LiveQueueModelFoundationLoanV1;

    fn owner(&self) -> QueueKeyV1 {
        self.key
    }
    fn preflight(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()
    }
    fn root(&mut self) -> &mut Option<Custody> {
        &mut self.sdma_dispatch_promotion
    }
    fn outstanding(&mut self) -> &mut usize {
        &mut self.sdma_outstanding_buffers
    }
    fn loan(&mut self) -> Result<Self::Loan, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }
    fn read(&mut self) -> Result<Box<[u8]>, ComputeAqlQueueSessionErrorV1> {
        let Some(Custody::Input(buffer)) = self.sdma_dispatch_promotion.as_ref() else {
            std::process::abort();
        };
        let Some(engine) = self.engine.as_mut() else {
            std::process::abort();
        };
        read_host_buffer(
            &mut engine.backend.session,
            buffer,
            0,
            buffer.physical_bytes(),
        )
        .map_err(Into::into)
    }
    fn retake(&mut self, loan: Self::Loan) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.retake_model_ownership_after_live_mutation(loan)
    }
    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

#[cfg(test)]
#[path = "sdma_dispatch_promotion/tests.rs"]
mod tests;
