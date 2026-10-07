//! Retain the original buffer until borrowed validation and model retake settle.

#![forbid(unsafe_code)]

use super::*;
use crate::persistent_directional_sdma::{
    Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1,
    Gfx942PersistentDirectionalSdmaPairV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) trait SdmaPromotionContextV1 {
    fn owner(&self) -> QueueKeyV1;
    fn preflight(
        &self,
        buffer: &Gfx942SdmaBufferV1,
    ) -> Result<Gfx942PersistentDirectionalSdmaPairV1, ComputeAqlQueueSessionErrorV1>;
    // Root access must survive missing engines and failed model restoration.
    fn roots(
        &mut self,
    ) -> (
        &mut Option<Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1>,
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

fn poison_without_replacing_failure<C: SdmaPromotionContextV1>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

#[allow(clippy::result_large_err)]
pub(super) fn promote_in_place<C: SdmaPromotionContextV1>(
    context: &mut C,
    buffer: Gfx942SdmaBufferV1,
) -> Result<
    Gfx942DirectionalQueuePersistentAllocationV1,
    Gfx942DirectionalPersistentSdmaPromotionFailureV1,
> {
    if !buffer.belongs_to(context.owner()) {
        return Err(classify_directional_persistent_sdma_promotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("foreign SDMA buffer owner"),
            buffer,
            false,
        ));
    }
    // Preflight is borrowed, non-mutating admission. Preserve its original error order.
    let pair = match context.preflight(&buffer) {
        Ok(pair) => pair,
        Err(error) => {
            return Err(classify_directional_persistent_sdma_promotion_failure_v1(
                error,
                buffer,
                context.is_terminal(),
            ));
        }
    };
    let (root, _) = context.roots();
    if root.is_some() {
        return Err(classify_directional_persistent_sdma_promotion_failure_v1(
            ComputeAqlQueueSessionErrorV1::Contract("unfinished SDMA promotion"),
            buffer,
            context.is_terminal(),
        ));
    }
    *root = Some(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1 { buffer });
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
        let buffer = root.take().expect("settled promotion custody").buffer;
        match promote_directional_persistent_sdma_custody_v1(buffer, pair, *outstanding) {
            Ok((allocation, debit)) => {
                *outstanding = debit;
                Ok(allocation)
            }
            Err(buffer) => {
                *root = Some(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1 { buffer });
                poison_without_replacing_failure(context);
                Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "directional persistent SDMA promotion custody mismatch",
                ))
            }
        }
    }));
    match result {
        Ok(Ok(allocation)) => Ok(allocation),
        Ok(Err(error)) => {
            let terminal = context.is_terminal();
            let buffer = context
                .roots()
                .0
                .take()
                .expect("failed promotion custody")
                .buffer;
            Err(classify_directional_persistent_sdma_promotion_failure_v1(
                error, buffer, terminal,
            ))
        }
        Err(payload) => {
            poison_without_replacing_failure(context);
            resume_unwind(payload)
        }
    }
}

impl SdmaPromotionContextV1 for ComputeAqlQueueSessionV1 {
    fn owner(&self) -> QueueKeyV1 {
        self.key
    }
    fn preflight(
        &self,
        buffer: &Gfx942SdmaBufferV1,
    ) -> Result<Gfx942PersistentDirectionalSdmaPairV1, ComputeAqlQueueSessionErrorV1> {
        self.require_sdma_enabled()?;
        admit_buffer(
            buffer,
            self.sdma
                .as_ref()
                .and_then(Gfx942SdmaQueueSetV1::directional_observation),
        )
    }
    fn roots(
        &mut self,
    ) -> (
        &mut Option<Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1>,
        &mut usize,
    ) {
        (&mut self.sdma_promotion, &mut self.sdma_outstanding_buffers)
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
        self.sdma_promotion
            .as_ref()
            .expect("installed promotion custody")
            .buffer
            .validate_physical_device_mapping(&engine.backend.session)
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

pub(super) fn admit_buffer(
    buffer: &Gfx942SdmaBufferV1,
    observation: Option<crate::sdma::Gfx942DirectionalSdmaQueueObservationV1>,
) -> Result<Gfx942PersistentDirectionalSdmaPairV1, ComputeAqlQueueSessionErrorV1> {
    if buffer.kind() != Gfx942SdmaBufferKindV1::DeviceLocal
        || !directional_persistent_sdma_extents_are_admitted_v1(
            buffer.requested_bytes(),
            buffer.physical_bytes(),
            buffer.pool_generation(),
        )
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "directional persistent SDMA promotion requires 0 < logical <= page-rounded physical <= 256 MiB",
        ));
    }
    let observation = observation.ok_or(ComputeAqlQueueSessionErrorV1::Contract(
        "directional persistent SDMA promotion requires one directional queue pair",
    ))?;
    admit_persistent_directional_sdma_pair_v1(observation)
        .map_err(ComputeAqlQueueSessionErrorV1::Contract)
}
