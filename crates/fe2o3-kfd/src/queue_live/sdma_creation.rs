//! Root returned SDMA creation owners before model-retake failure escapes.

use super::*;
use crate::sdma::creation::SdmaCreationEscrowV1;
use crate::sdma::{Gfx942SdmaQueueSetCreationDispositionV1, Gfx942SdmaQueueSetCreationFailureV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(in crate::queue) struct ReturnedSdmaCreationV1<O> {
    primary: Gfx942SdmaQueueSetV1,
    secondary: Option<Gfx942SdmaQueueSetV1>,
    output: O,
}

impl<O> ReturnedSdmaCreationV1<O> {
    pub(in crate::queue) fn single(primary: Gfx942SdmaQueueSetV1, output: O) -> Self {
        Self {
            primary,
            secondary: None,
            output,
        }
    }

    pub(in crate::queue) fn combined(
        primary: Gfx942SdmaQueueSetV1,
        secondary: Gfx942SdmaQueueSetV1,
        output: O,
    ) -> Self {
        Self {
            primary,
            secondary: Some(secondary),
            output,
        }
    }

    pub(in crate::queue) fn into_single(self) -> (Gfx942SdmaQueueSetV1, O) {
        if self.secondary.is_some() {
            std::process::abort();
        }
        (self.primary, self.output)
    }

    pub(in crate::queue) fn into_combined(self) -> (Gfx942SdmaQueueSetV1, Gfx942SdmaQueueSetV1, O) {
        let Some(secondary) = self.secondary else {
            std::process::abort()
        };
        (self.primary, secondary, self.output)
    }
}

type CreationResultV1<O> = Result<ReturnedSdmaCreationV1<O>, Gfx942SdmaQueueSetCreationFailureV1>;
type CreationEnvelopeV1 =
    Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>;

pub(in crate::queue) trait SdmaCreationContextV1 {
    type Memory;

    fn require_vacant(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> CreationEnvelopeV1;
    fn retained_slot(&mut self) -> &mut Option<Gfx942SdmaQueueSetV1>;
    fn poison(&mut self);
}

fn install(context: &mut impl SdmaCreationContextV1, owner: Gfx942SdmaQueueSetV1) {
    let slot = context.retained_slot();
    if slot.is_some() {
        // Vacancy is checked before the memory-only callback. Never unwind by
        // destroying either native owner if that invariant is violated.
        core::mem::forget(owner);
        std::process::abort();
    }
    *slot = Some(owner);
}

fn retain<O>(context: &mut impl SdmaCreationContextV1, created: Option<CreationResultV1<O>>) {
    match created {
        Some(Ok(created)) => {
            let ReturnedSdmaCreationV1 {
                primary,
                secondary,
                output,
            } = created;
            install(
                context,
                Gfx942SdmaQueueSetV1::retain_created_for_terminal(primary, secondary),
            );
            // Even a metadata destructor must observe rooted native custody.
            drop(output);
        }
        Some(Err(failure)) => {
            let (error, _, retained) = failure.into_parts();
            if let Some(owner) = retained {
                install(context, owner);
            }
            drop(error);
        }
        None => {}
    }
}

fn resume_poisoned(
    context: &mut impl SdmaCreationContextV1,
    payload: Box<dyn core::any::Any + Send>,
) -> ! {
    if let Err(secondary) = catch_unwind(AssertUnwindSafe(|| context.poison())) {
        core::mem::forget(secondary);
    }
    resume_unwind(payload)
}

fn retain_and_resume<O>(
    context: &mut impl SdmaCreationContextV1,
    created: Option<CreationResultV1<O>>,
    payload: Box<dyn core::any::Any + Send>,
) -> ! {
    if let Err(secondary) = catch_unwind(AssertUnwindSafe(|| retain(context, created))) {
        core::mem::forget(secondary);
    }
    resume_poisoned(context, payload)
}

// Failure custody stays inline so retaining native owners needs no allocation.
#[allow(clippy::result_large_err)]
pub(in crate::queue) fn create_with_custody_v1<C: SdmaCreationContextV1, O>(
    context: &mut C,
    stage: &'static str,
    operation: impl FnOnce(&mut C::Memory, &mut SdmaCreationEscrowV1) -> CreationResultV1<O>,
) -> Result<ReturnedSdmaCreationV1<O>, ComputeAqlQueueSessionErrorV1> {
    context.require_vacant()?;
    let mut created = None;
    let mut escrow = SdmaCreationEscrowV1::default();
    let envelope = catch_unwind(AssertUnwindSafe(|| {
        let ((), retake) = context.with_memory_custody(|memory| {
            // Keep a lower panic outside model retake and its poison callbacks.
            created = Some(catch_unwind(AssertUnwindSafe(|| {
                operation(memory, &mut escrow)
            })));
        })?;
        retake
    }));
    let created = match created {
        Some(Ok(created)) => Some(created),
        Some(Err(payload)) => {
            core::mem::forget(envelope);
            if let Some(owner) = escrow.take_terminal() {
                install(context, owner);
            }
            resume_poisoned(context, payload)
        }
        None => None,
    };
    // Returned custody and the borrowed escrow must never both own a queue.
    if !escrow.is_empty() {
        core::mem::forget(created);
        core::mem::forget(envelope);
        core::mem::forget(escrow);
        std::process::abort();
    }
    let envelope = match envelope {
        Ok(envelope) => envelope,
        Err(payload) => retain_and_resume(context, created, payload),
    };
    let error = match (envelope, created) {
        (Ok(()), Some(Ok(created))) => return Ok(created),
        (Ok(()), Some(Err(failure))) => {
            let (error, disposition, retained) = failure.into_parts();
            let has_retained = retained.is_some();
            if let Some(owner) = retained {
                install(context, owner);
            }
            if disposition == Gfx942SdmaQueueSetCreationDispositionV1::Retryable && !has_retained {
                return Err(error.into());
            }
            error.into()
        }
        (Err(error), created) => {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| retain(context, created))) {
                resume_poisoned(context, payload);
            }
            error
        }
        (Ok(()), None) => {
            ComputeAqlQueueSessionErrorV1::Contract("SDMA creation operation did not execute")
        }
    };
    context.poison();
    Err(terminal_creation(stage, error))
}

impl SdmaCreationContextV1 for ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;

    fn require_vacant(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned || self.sdma.is_some() || self.striped_sdma.is_some() {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "SDMA creation requires live vacant owner slots",
            ));
        }
        Ok(())
    }

    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> CreationEnvelopeV1 {
        // Creation settlement must root returned owners before poisoning. All
        // opening/closing errors and panics are terminalized by the outer driver.
        self.with_live_queue_memory_model_custody_with_poison(operation, |_| {})
    }

    fn retained_slot(&mut self) -> &mut Option<Gfx942SdmaQueueSetV1> {
        &mut self.sdma
    }

    fn poison(&mut self) {
        let local = catch_unwind(AssertUnwindSafe(|| self.poison_terminal()));
        permanently_poison_process_global_kfd_runtime_gate_v1();
        if let Err(payload) = local {
            resume_unwind(payload);
        }
    }
}
