//! Demotion borrows the original allocation until validation and model retake settle.

#![forbid(unsafe_code)]

use super::*;
use crate::persistent_directional_sdma::Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) trait SdmaDemotionContextV1 {
    fn owner(&self) -> QueueKeyV1;
    fn preflight(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn roots(
        &mut self,
    ) -> (
        &mut Option<Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1>,
        &mut usize,
    );
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn validate(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn is_terminal(&self) -> bool;
    fn poison(&mut self);
}

fn poison_without_replacing_failure<C: SdmaDemotionContextV1>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

#[allow(clippy::result_large_err)]
pub(super) fn demote_in_place<C: SdmaDemotionContextV1>(
    context: &mut C,
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
) -> Result<Gfx942SdmaBufferV1, Gfx942DirectionalPersistentSdmaDemotionFailureV1> {
    if allocation.attachment.queue != context.owner() {
        return Err(classify_directional_persistent_sdma_demotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract(
                "foreign directional persistent SDMA allocation owner",
            ),
            allocation,
            false,
        ));
    }
    // Admission is borrowed and preserves the existing public error order.
    if let Err(error) = context.preflight(&allocation) {
        return Err(classify_directional_persistent_sdma_demotion_failure_v1(
            error,
            allocation,
            context.is_terminal(),
        ));
    }
    let (root, _) = context.roots();
    if root.is_some() {
        return Err(classify_directional_persistent_sdma_demotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("unfinished SDMA demotion"),
            allocation,
            context.is_terminal(),
        ));
    }
    *root = Some(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1 { allocation });
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (validation, retake) = execute_live_model_custody_v1(
            context,
            C::loan,
            |context| context.validate(),
            C::retake,
            poison_without_replacing_failure,
        )?;
        retake?;
        validation?;
        let (root, outstanding) = context.roots();
        let allocation = root.take().expect("settled demotion custody").allocation;
        match demote_directional_persistent_sdma_custody_v1(allocation, *outstanding) {
            Ok((buffer, debit)) => {
                *outstanding = debit;
                Ok(buffer)
            }
            Err((error, allocation)) => {
                *root =
                    Some(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1 { allocation });
                Err(map_directional_persistent_sdma_use_error_v1(error))
            }
        }
    }));
    match result {
        Ok(Ok(buffer)) => Ok(buffer),
        Ok(Err(error)) => {
            let terminal = context.is_terminal();
            let allocation = context
                .roots()
                .0
                .take()
                .expect("failed demotion custody")
                .allocation;
            Err(classify_directional_persistent_sdma_demotion_failure_v1(
                error, allocation, terminal,
            ))
        }
        Err(payload) => {
            poison_without_replacing_failure(context);
            resume_unwind(payload)
        }
    }
}

impl SdmaDemotionContextV1 for ComputeAqlQueueSessionV1 {
    fn owner(&self) -> QueueKeyV1 {
        self.key
    }
    fn preflight(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        admit_allocation(
            allocation,
            self.directional_persistent_sdma_attachment_is_current(&allocation.attachment),
        )
    }
    fn roots(
        &mut self,
    ) -> (
        &mut Option<Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1>,
        &mut usize,
    ) {
        (&mut self.sdma_demotion, &mut self.sdma_outstanding_buffers)
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }
    fn validate(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        let allocation = &self
            .sdma_demotion
            .as_ref()
            .expect("installed demotion custody")
            .allocation;
        engine
            .backend
            .session
            .mapped_gfx942_device_memory_facts(
                allocation
                    .owner
                    .local_native_for_sdma()
                    .expect("admitted local allocation"),
            )
            .map(|_| ())
            .map_err(Into::into)
    }
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.retake_model_ownership_after_live_mutation(loan)
    }
    fn is_terminal(&self) -> bool {
        self.terminal_poisoned
    }
    fn poison(&mut self) {
        self.poison_terminal();
        permanently_poison_process_global_kfd_runtime_gate_v1();
    }
}

pub(super) fn admit_allocation(
    allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    attachment_current: bool,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    if !attachment_current {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "directional persistent SDMA queue-pair attachment changed",
        ));
    }
    if allocation
        .attachment
        .pool_generation
        .checked_add(1)
        .is_none()
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "directional persistent SDMA pool generation exhausted",
        ));
    }
    if allocation.owner.local_native_for_sdma().is_none() {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "directional persistent SDMA allocation is active or not local",
        ));
    }
    Ok(())
}
