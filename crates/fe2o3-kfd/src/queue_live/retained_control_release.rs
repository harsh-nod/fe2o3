//! Keep detached persistent controls outside the model loan until settlement.

use super::*;
use crate::queue::dispatch_binding::control_release::{
    ReturningControlCleanupCustodyV1, ReturningControlModeV1,
};
use crate::queue::dispatch_binding::pristine_abort::PristineControlReleaseV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type ControlReleaseEnvelopeV1 =
    Result<((), Result<(), ComputeAqlQueueSessionErrorV1>), ComputeAqlQueueSessionErrorV1>;

pub(in crate::queue) trait RetainedControlReleaseContextV1 {
    type Memory: PristineControlReleaseV1;

    fn require_releasable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1>;
    fn detached_generation(&self) -> Option<u64>;
    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> ControlReleaseEnvelopeV1;
    fn retain(&mut self, root: ReturningControlCleanupCustodyV1);
    fn poison(&mut self, panicked: bool);
}

pub(in crate::queue) struct SettledRetainedControlReleaseV1 {
    pub(in crate::queue) result: std::thread::Result<Result<bool, ComputeAqlQueueSessionErrorV1>>,
    pub(in crate::queue) transport: bool,
}

impl SettledRetainedControlReleaseV1 {
    pub(super) fn into_result(self) -> Result<bool, ComputeAqlQueueSessionErrorV1> {
        match self.result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

pub(in crate::queue) fn settle_retained_control_release_v1(
    context: &mut impl RetainedControlReleaseContextV1,
) -> SettledRetainedControlReleaseV1 {
    let mut original = None;
    let mut cleanup = None;
    let mut lower = None;
    let mut entered = false;
    let mut terminal_on_error = false;
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.require_releasable()?;
        if !context
            .dispatch()
            .as_ref()
            .is_some_and(DispatchResourceOwnerV1::persistent_data_is_detached_v1)
        {
            return Ok(false);
        }
        terminal_on_error = true;
        let generation =
            context
                .detached_generation()
                .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                    "retained persistent control lost its detached generation",
                ))?;
        context
            .dispatch()
            .as_ref()
            .expect("checked detached persistent control")
            .validate_detached_persistent_control_release_v1(generation)?;
        context.check_currentness()?;
        original = context.dispatch().take();
        let envelope = context.with_memory_custody(|memory| {
            entered = true;
            cleanup = Some(ReturningControlCleanupCustodyV1::new(
                original.take().expect("opened loan retains its dispatch"),
                ReturningControlModeV1::DetachedPersistent {
                    expected_generation: generation,
                },
            ));
            // Preserve the lower panic even if model retake or its poison hook panics.
            lower = Some(catch_unwind(AssertUnwindSafe(|| {
                cleanup
                    .as_mut()
                    .expect("rooted persistent control")
                    .release_in_place(memory)
            })));
        });
        let ((), retake) = match envelope {
            Ok(envelope) => envelope,
            Err(error) => {
                // Opening rejection precedes all effects and leaves the original owner intact.
                terminal_on_error = entered;
                return Err(error);
            }
        };
        retake?;
        match lower.take() {
            Some(Ok(result)) => result?,
            Some(Err(payload)) => resume_unwind(payload),
            None => {
                return Err(ComputeAqlQueueSessionErrorV1::Contract(
                    "retained persistent control callback did not execute",
                ));
            }
        }
        if !cleanup
            .as_ref()
            .is_some_and(ReturningControlCleanupCustodyV1::is_complete)
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "retained persistent control cleanup was incomplete",
            ));
        }
        Ok(true)
    }));
    let mut result = match lower {
        Some(Err(payload)) => {
            // A secondary panic payload may itself panic when dropped.
            core::mem::forget(result);
            Err(payload)
        }
        _ => result,
    };
    if let Some(dispatch) = original {
        *context.dispatch() = Some(dispatch);
    }
    let transport = result.is_err() || terminal_on_error && !matches!(result, Ok(Ok(_)));
    if transport {
        if let Some(root) = cleanup {
            context.retain(root);
        }
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| context.poison(result.is_err()))) {
            if result.is_err() {
                core::mem::forget(payload);
            } else {
                result = Err(payload);
            }
        }
    }
    SettledRetainedControlReleaseV1 { result, transport }
}

impl RetainedControlReleaseContextV1 for ComputeAqlQueueSessionV1 {
    type Memory = SharedGttMemorySessionV1;

    fn require_releasable(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if self.terminal_poisoned {
            return Err(Gfx942DispatchBindingErrorV1::Poisoned.into());
        }
        if !self.unpublished_dispatch.is_clear() || self.has_any_persistent_compute_attachment_v1()
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        Ok(())
    }

    fn dispatch(&mut self) -> &mut Option<DispatchResourceOwnerV1> {
        &mut self.dispatch
    }

    fn detached_generation(&self) -> Option<u64> {
        self.detached_dispatch_generation
    }

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?
            .backend
            .session
            .check_queue_currentness()
            .map_err(Into::into)
    }

    fn with_memory_custody(
        &mut self,
        operation: impl FnOnce(&mut Self::Memory),
    ) -> ControlReleaseEnvelopeV1 {
        self.with_live_queue_memory_model_custody(operation)
    }

    fn retain(&mut self, root: ReturningControlCleanupCustodyV1) {
        core::mem::forget(root);
    }

    fn poison(&mut self, panicked: bool) {
        self.poison_terminal();
        if panicked {
            poison_process_global_after_dispatch_terminal_v1();
        }
    }
}

impl ComputeAqlQueueSessionV1 {
    pub(super) fn release_retained_control_settled_v1(
        &mut self,
    ) -> SettledRetainedControlReleaseV1 {
        settle_retained_control_release_v1(self)
    }
}
