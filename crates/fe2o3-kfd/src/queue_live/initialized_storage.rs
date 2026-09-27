//! Borrow and authenticate the original storage before minting compute custody.

#![forbid(unsafe_code)]

use super::*;
use crate::persistent_compute::{
    Gfx942PersistentComputeInitializedStorageV1 as InitializedStorage,
    Gfx942PersistentComputeStoragePromotionCustodyV1 as Custody,
    Gfx942PersistentComputeStoragePromotionFailureV1 as Failure,
    Gfx942PersistentComputeStoragePromotionTerminalCustodyV1 as Root,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) trait InitializedStorageContextV1 {
    fn owner(&self) -> QueueKeyV1;
    fn preflight(
        &self,
        allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn root(&mut self) -> &mut Option<Root>;
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1>;
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn validate_mapping(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn retake(
        &mut self,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn is_terminal(&self) -> bool;
    fn poison(&mut self);
}

fn poison_without_replacing_failure<C: InitializedStorageContextV1>(context: &mut C) {
    core::mem::forget(catch_unwind(AssertUnwindSafe(|| context.poison())));
}

fn failure(
    error: ComputeAqlQueueSessionErrorV1,
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
    terminal: bool,
) -> Failure {
    Failure {
        error,
        custody: if terminal {
            Custody::ProcessTeardown(Root { allocation })
        } else {
            Custody::Retryable(allocation)
        },
    }
}

#[allow(clippy::result_large_err)]
pub(super) fn promote_in_place<C: InitializedStorageContextV1>(
    context: &mut C,
    allocation: Gfx942DirectionalQueuePersistentAllocationV1,
) -> Result<InitializedStorage, Failure> {
    if allocation.attachment.queue != context.owner() {
        return Err(Failure {
            error: ComputeAqlQueueSessionErrorV1::Contract("foreign initialized storage owner"),
            custody: Custody::ForeignQueue(allocation),
        });
    }
    if let Err(error) = context.preflight(&allocation) {
        return Err(failure(error, allocation, context.is_terminal()));
    }
    if context.root().is_some() {
        return Err(failure(
            ComputeAqlQueueSessionErrorV1::Contract("unfinished initialized storage promotion"),
            allocation,
            context.is_terminal(),
        ));
    }
    *context.root() = Some(Root { allocation });
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (validation, retake) = execute_live_model_custody_v1(
            context,
            C::loan,
            |context| {
                if let Err(error) = context.currentness() {
                    poison_without_replacing_failure(context);
                    return Err(error);
                }
                let mapping = context.validate_mapping();
                if let Err(error) = context.currentness() {
                    poison_without_replacing_failure(context);
                    return Err(error);
                }
                mapping
            },
            C::retake,
            poison_without_replacing_failure,
        )?;
        retake?;
        validation?;
        // No demotion, allocation, generation advance, or buffer-accounting change.
        Ok(InitializedStorage {
            allocation: context
                .root()
                .take()
                .expect("settled initialized storage")
                .allocation,
        })
    }));
    match result {
        Ok(Ok(ready)) => Ok(ready),
        Ok(Err(error)) => {
            let terminal = context.is_terminal();
            let allocation = context
                .root()
                .take()
                .expect("failed initialized storage")
                .allocation;
            Err(failure(error, allocation, terminal))
        }
        Err(payload) => {
            poison_without_replacing_failure(context);
            resume_unwind(payload)
        }
    }
}

pub(super) fn admit_allocation(
    allocation: &Gfx942DirectionalQueuePersistentAllocationV1,
    compute_queue: QueueKeyV1,
    attachment_current: bool,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    if allocation.attachment.queue != compute_queue || !attachment_current {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "initialized storage requires the exact primary compute and SDMA queue pair",
        ));
    }
    if allocation.owner.local_native_for_sdma().map(|lease| {
        crate::sdma::Gfx942SdmaBufferStorageIdentityV1::Device(lease.storage_identity())
    }) != Some(allocation.attachment.storage_identity)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "initialized storage identity changed",
        ));
    }
    allocation
        .owner
        .preflight_initialized_storage_for_compute(
            allocation.attachment.queue,
            allocation.attachment.pool_generation,
            allocation.attachment.logical_bytes,
            allocation.attachment.physical_bytes,
        )
        .map_err(map_directional_persistent_sdma_use_error_v1)
}

impl InitializedStorageContextV1 for ComputeAqlQueueSessionV1 {
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
            self.compute_lane_session,
            self.directional_persistent_sdma_attachment_is_current(&allocation.attachment),
        )
    }
    fn root(&mut self) -> &mut Option<Root> {
        &mut self.initialized_storage_promotion
    }
    fn loan(&mut self) -> Result<LiveQueueModelFoundationLoanV1, ComputeAqlQueueSessionErrorV1> {
        self.restore_model_ownership_for_live_mutation()
    }
    fn currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session
            .check_queue_operational_currentness()
            .map_err(Into::into)
    }
    fn validate_mapping(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        let allocation = &self
            .initialized_storage_promotion
            .as_ref()
            .expect("installed initialized storage custody")
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
