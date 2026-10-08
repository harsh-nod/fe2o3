//! One-attempt cooperative cancellation, independent of compiler resource errors.
//! Polls are settled phase boundaries, not interruption or stop-latency bounds.
use super::{
    SourceLocalOrderRecipeAttemptV1 as Attempt, SourceLocalOrderRecipeFailurePhaseV1 as Phase,
    SourceLocalOrderRecipeFailureV1 as Failure,
};
use std::sync::atomic::{AtomicU8, Ordering};

const FRESH: u8 = 0;
const PRE_CANCELLED: u8 = 1;
const RUNNING: u8 = 2;
const REQUESTED: u8 = 3;
const SUCCEEDED: u8 = 4;
const FAILED: u8 = 5;
const CANCELLED: u8 = 6;

/// Closed observation boundaries. None is an interrupt inside an opaque phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderRecipeCancellationCheckpointV1 {
    BeforeInput,
    BeforeCompiler,
    AfterAnalysis,
    AfterTransaction,
    BeforeCapture,
    AfterCapture,
    AfterImport,
    AfterMiddleEnd,
    AfterSsa,
    AfterMaterialization,
    AfterRankedVerification,
    BeforePrefix,
    AfterPrefix,
    AfterSourceJoin,
    BeforeContinuation,
    AfterContinuation,
    BeforeReplay,
    AfterReplay,
    AfterFinalCurrentness,
    DriverCommit,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLocalOrderRecipeCancellationObservationV1 {
    checkpoint: SourceLocalOrderRecipeCancellationCheckpointV1,
}
impl SourceLocalOrderRecipeCancellationObservationV1 {
    pub const fn checkpoint(self) -> SourceLocalOrderRecipeCancellationCheckpointV1 {
        self.checkpoint
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceLocalOrderRecipeCancellationRequestV1 {
    Requested,
    AlreadyRequested,
    TooLate,
}

/// Caller-owned, reset-free, single-attempt token. No retained graph or heap.
/// A scoped caller thread may request cancellation; this API spawns no thread.
/// A request is not proof of cancellation until the returned Failure says so.
pub struct SourceLocalOrderRecipeCancellationV1 {
    state: AtomicU8,
}
impl Default for SourceLocalOrderRecipeCancellationV1 {
    fn default() -> Self {
        Self::new()
    }
}
impl SourceLocalOrderRecipeCancellationV1 {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(FRESH),
        }
    }

    /// At most two strong CAS operations; terminal commit and cancellation have
    /// one atomic linearization order. There is no weak-CAS retry or reset.
    pub fn request_cancellation(&self) -> SourceLocalOrderRecipeCancellationRequestV1 {
        use SourceLocalOrderRecipeCancellationRequestV1::*;
        match self
            .state
            .compare_exchange(FRESH, PRE_CANCELLED, Ordering::SeqCst, Ordering::SeqCst)
        {
            Ok(_) => Requested,
            Err(PRE_CANCELLED | REQUESTED | CANCELLED) => AlreadyRequested,
            Err(RUNNING) => match self.state.compare_exchange(
                RUNNING,
                REQUESTED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => Requested,
                Err(REQUESTED | CANCELLED) => AlreadyRequested,
                Err(_) => TooLate,
            },
            Err(_) => TooLate,
        }
    }
    pub(crate) fn claim(&self) -> Result<Gate<'_>, Failure> {
        let claimed =
            match self
                .state
                .compare_exchange(FRESH, RUNNING, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => true,
                Err(PRE_CANCELLED) => self
                    .state
                    .compare_exchange(PRE_CANCELLED, REQUESTED, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok(),
                Err(_) => false,
            };
        if !claimed {
            return Err(Failure::new(
                Phase::Request,
                "local-order cancellation token already claimed".into(),
            ));
        }
        Ok(Gate {
            token: self,
            finished: false,
            #[cfg(test)]
            hook: None,
        })
    }
}
pub(crate) type Checkpoint = SourceLocalOrderRecipeCancellationCheckpointV1;
#[cfg(test)]
pub(crate) type Hook<'a> = dyn Fn(Checkpoint, &SourceLocalOrderRecipeCancellationV1) + Sync + 'a;

pub(crate) struct Gate<'a> {
    token: &'a SourceLocalOrderRecipeCancellationV1,
    finished: bool,
    #[cfg(test)]
    hook: Option<&'a Hook<'a>>,
}
impl<'a> Gate<'a> {
    #[cfg(test)]
    pub(crate) fn with_hook(mut self, hook: &'a Hook<'a>) -> Self {
        self.hook = Some(hook);
        self
    }
    pub(crate) fn poll(&self, checkpoint: Checkpoint) -> Result<(), Failure> {
        #[cfg(test)]
        if let Some(hook) = self.hook {
            hook(checkpoint, self.token);
        }
        if self.token.state.load(Ordering::SeqCst) == REQUESTED {
            Err(cancelled(checkpoint))
        } else {
            Ok(())
        }
    }
    // Only this claimed Gate commits. A racing requester can change RUNNING
    // to REQUESTED once, hence at most two strong CAS attempts suffice.
    fn commit_error(&self, cancelled: bool) {
        let terminal = if cancelled { CANCELLED } else { FAILED };
        if self
            .token
            .state
            .compare_exchange(RUNNING, terminal, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            let _ = self.token.state.compare_exchange(
                REQUESTED,
                terminal,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
    }
    pub(crate) fn finish_attempt(mut self, mut attempt: Attempt) -> Attempt {
        // Preserve an earlier semantic/resource/currentness/fatal error even
        // when cancellation is requested later. Never manufacture Resource.
        if let Err(error) = &attempt.result {
            self.commit_error(error.cancellation().is_some());
        } else {
            #[cfg(test)]
            if let Some(hook) = self.hook {
                hook(Checkpoint::DriverCommit, self.token);
            }
            if self
                .token
                .state
                .compare_exchange(RUNNING, SUCCEEDED, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                // No output escapes when the request won before commit. The
                // provisional original Output is dropped, not serialized.
                attempt.result = Err(cancelled(Checkpoint::DriverCommit));
                let _ = self.token.state.compare_exchange(
                    REQUESTED,
                    CANCELLED,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
            }
        }
        self.finished = true;
        attempt
    }
}
impl Drop for Gate<'_> {
    fn drop(&mut self) {
        if !self.finished {
            // Unwind/abandon cannot leave a reusable running token. Ordinary
            // owner destructors still run; no panic interception or heap claim.
            self.commit_error(false);
        }
    }
}
pub(crate) fn poll(gate: Option<&Gate<'_>>, checkpoint: Checkpoint) -> Result<(), Failure> {
    match gate {
        Some(gate) => gate.poll(checkpoint),
        None => Ok(()),
    }
}
fn cancelled(checkpoint: Checkpoint) -> Failure {
    let mut error = Failure::new(
        Phase::Observation,
        "local-order recipe cooperatively cancelled at a phase boundary".into(),
    );
    error.cancellation = Some(SourceLocalOrderRecipeCancellationObservationV1 { checkpoint });
    error
}

#[cfg(test)]
#[path = "source_local_order_recipe_cancellation_v1_tests.rs"]
mod tests;
