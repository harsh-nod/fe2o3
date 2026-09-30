//! Owned native custody. RuntimeContext integration is a separate layer.

#![forbid(unsafe_code)]

use super::*;

/// The exact directional queue, source session, and destination session.
/// Returning these owners does not destroy their queues or release allocations.
pub type Gfx942XgmiOwnedRetainedPairPartsV1 = (
    Gfx942NativeXgmiSdmaQueueV1,
    SharedGttMemorySessionV1,
    SharedGttMemorySessionV1,
);

macro_rules! retained_pair_owned_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}

include!("owned/declarations.rs");
include!("owned/bodies.rs");

type NativeParts =
    Parts<Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1, SharedGttMemorySessionV1>;

fn empty_owned_context<T>() -> T {
    std::process::abort()
}

impl NativeParts {
    fn borrow(&mut self) -> Pair<'_> {
        Pair {
            queue: &mut self.queue,
            source: &mut self.source,
            destination: &mut self.destination,
        }
    }
}

impl<Q, S, D> Parts<Q, S, D> {
    fn into_parts(self) -> (Q, S, D) {
        retained_pair_owned_parts_body!(self)
    }
}

impl Custody for NativeParts {
    fn terminal(&self) -> bool {
        retained_pair_terminal_body!(self)
    }

    fn quarantine(&mut self) {
        self.queue
            .quarantine_batch_v1(&mut self.source, &mut self.destination);
    }
}

// The private generic holder lets CPU tests exercise the actual move/drop path.
// Only NativeParts is used by the public API; no provider or raw-owner accessor exists.
impl<C: Custody> Owned<C> {
    fn new(context: C) -> Self {
        retained_pair_owned_new_body!(context)
    }

    fn context(&self) -> &C {
        retained_pair_owned_context_body!(self)
    }

    fn context_mut(&mut self) -> &mut C {
        retained_pair_owned_context_mut_body!(self)
    }

    fn take(&mut self) -> C {
        retained_pair_owned_take_body!(self)
    }

    fn admit(
        mut self,
        admit: impl FnOnce(&mut C) -> Result<(), Gfx942SdmaErrorV1>,
    ) -> Result<Self, Failure<C, Gfx942SdmaErrorV1>> {
        let result = run_operation(self.context_mut(), admit);
        retained_pair_owned_admit_post_body!(self, result)
    }

    fn finish(
        mut self,
        close: impl FnOnce(&mut C) -> Result<(), Gfx942SdmaErrorV1>,
    ) -> Result<C, Failure<C, Gfx942SdmaErrorV1>> {
        let result = run_operation(self.context_mut(), close);
        retained_pair_owned_finish_post_body!(self, result)
    }
}

impl<C: Custody, E> Failure<C, E> {
    fn recover_unadmitted(mut self) -> Result<(E, C), Self> {
        retained_pair_owned_recover_body!(self)
    }
}

impl<C: Custody> Drop for Owned<C> {
    fn drop(&mut self) {
        if let Some(context) = self.context.as_mut() {
            // Quarantine can itself panic. Never unwind into native field Drop.
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| context.quarantine())) {
                core::mem::forget(payload);
            }
            std::process::abort();
        }
    }
}

/// Move-only native queue/session ownership for the explicit retained profile.
///
/// Entry runs the same drained binding and full-pair admission as the borrowed
/// profile. Every operation retains its own opening and closing operational
/// fences. This type never contains references into itself and grants no
/// RuntimeContext, Worker, or full-fresh completion authority.
///
/// Successful consuming `finish` returns the exact queue and both sessions.
/// Failed finish retains them in its error. Dropping an occupied owner or error
/// quarantines both sessions and the process gate, then aborts BEFORE native
/// field destruction. Forgetting an owner leaks custody; it is not cleanup.
///
/// ```no_run
/// use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1,
///     Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
///     Gfx942XgmiRetainedPairEnvironmentAssumptionV1 as Environment};
/// fn roundtrip(queue: Gfx942NativeXgmiSdmaQueueV1,
///     source: SharedGttMemorySessionV1, peer: SharedGttMemorySessionV1) {
///     let pair = Gfx942NativeXgmiSdmaOwnedRetainedPairV1::begin(queue, source, peer,
///         Environment::ReviewedMi300xAmdgpu61613OrdinaryLifetime).unwrap();
///     let (mut queue, mut source, mut peer) = pair.finish().unwrap();
///     queue.destroy_and_release(&mut source, &mut peer).unwrap();
/// }
/// ```
///
/// ```compile_fail,E0382
/// use fe2o3_kfd::{Gfx942NativeXgmiSdmaQueueV1, SharedGttMemorySessionV1,
///     Gfx942NativeXgmiSdmaOwnedRetainedPairV1,
///     Gfx942XgmiRetainedPairEnvironmentAssumptionV1 as Environment};
/// fn consumed(queue: Gfx942NativeXgmiSdmaQueueV1,
///     source: SharedGttMemorySessionV1, peer: SharedGttMemorySessionV1) {
///     let _owner = Gfx942NativeXgmiSdmaOwnedRetainedPairV1::begin(queue, source, peer,
///         Environment::ReviewedMi300xAmdgpu61613OrdinaryLifetime);
///     let _ = source.phase();
/// }
/// ```
///
/// ```compile_fail,E0599
/// use fe2o3_kfd::Gfx942NativeXgmiSdmaOwnedRetainedPairV1;
/// fn cannot_clone(pair: Gfx942NativeXgmiSdmaOwnedRetainedPairV1) { let _ = pair.clone(); }
/// ```
#[must_use = "finish and recover the exact native owners; occupied Drop aborts"]
pub struct Gfx942NativeXgmiSdmaOwnedRetainedPairV1 {
    owner: Owned<NativeParts>,
}

/// Retains the original error and every queue/session owner.
///
/// Only a nonterminal ENTRY refusal can return unadmitted parts. Failed finish
/// and terminal entry retain all parts until process teardown. Dropping this
/// error while it owns those parts aborts, including during unwinding.
#[must_use = "retain terminal custody or explicitly recover nonterminal entry owners"]
pub struct Gfx942XgmiOwnedRetainedPairFailureV1 {
    failure: Failure<NativeParts, Gfx942SdmaErrorV1>,
}

impl Gfx942XgmiOwnedRetainedPairFailureV1 {
    pub const fn error(&self) -> &Gfx942SdmaErrorV1 {
        &self.failure.error
    }

    pub fn is_terminal(&self) -> bool {
        !self.failure.entry_refusal || self.failure.owner.context().terminal()
    }

    /// Returns only the original nonterminal entry inputs, not admitted custody.
    #[allow(clippy::result_large_err)]
    pub fn recover_unadmitted(
        self,
    ) -> Result<(Gfx942SdmaErrorV1, Gfx942XgmiOwnedRetainedPairPartsV1), Self> {
        self.failure
            .recover_unadmitted()
            .map(|(error, parts)| (error, parts.into_parts()))
            .map_err(|failure| Self { failure })
    }
}

impl fmt::Debug for Gfx942XgmiOwnedRetainedPairFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942XgmiOwnedRetainedPairFailureV1")
            .field("error", self.error())
            .field("terminal", &self.is_terminal())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Gfx942XgmiOwnedRetainedPairFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(formatter)
    }
}

impl std::error::Error for Gfx942XgmiOwnedRetainedPairFailureV1 {}

#[allow(clippy::result_large_err)]
impl Gfx942NativeXgmiSdmaOwnedRetainedPairV1 {
    pub fn begin(
        queue: Gfx942NativeXgmiSdmaQueueV1,
        source: SharedGttMemorySessionV1,
        destination: SharedGttMemorySessionV1,
        _environment_assumption: Gfx942XgmiRetainedPairEnvironmentAssumptionV1,
    ) -> Result<Self, Gfx942XgmiOwnedRetainedPairFailureV1> {
        Owned::new(Parts {
            queue,
            source,
            destination,
        })
        .admit(|parts| {
            let pair = parts.borrow();
            pair.admit_binding()?;
            pair.source
                .validate_gfx942_xgmi_route_with_peer(pair.destination, pair.queue.route)?;
            pair.queue.require_live_queue_state_v1()
        })
        .map(|owner| Self { owner })
        .map_err(|failure| Gfx942XgmiOwnedRetainedPairFailureV1 { failure })
    }

    pub const fn profile(&self) -> &'static str {
        GFX942_XGMI_RETAINED_PAIR_PROFILE_V1
    }

    pub fn is_terminal(&self) -> bool {
        self.owner.context().terminal()
    }

    /// Fail-closes without dropping, releasing, or returning any native owner.
    pub fn quarantine(&mut self) {
        self.owner.context_mut().quarantine();
    }

    pub fn submit_batch(
        &mut self,
        requests: Vec<Gfx942XgmiSdmaCopyRequestV1>,
    ) -> Result<Vec<Gfx942SdmaCopyTicketV1>, Gfx942XgmiBatchSubmissionFailureV1> {
        run_operation(self.owner.context_mut(), |parts| {
            let pair = parts.borrow();
            pair.queue.submit_batch_with_currentness(
                pair.source,
                pair.destination,
                requests,
                XgmiRouteCurrentnessV1::OrdinaryRetainedPair,
            )
        })
    }

    pub fn wait_batch_until(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        deadline: Instant,
    ) -> Result<Gfx942XgmiRetainedPairCompletedBatchV1, Gfx942XgmiRetainedPairWaitFailureV1> {
        self.wait(tickets, XgmiBatchDeadlineV1::Absolute(deadline))
    }

    pub fn wait_batch_for(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        timeout: Duration,
    ) -> Result<Gfx942XgmiRetainedPairCompletedBatchV1, Gfx942XgmiRetainedPairWaitFailureV1> {
        self.wait(tickets, XgmiBatchDeadlineV1::Relative(timeout))
    }

    fn wait(
        &mut self,
        tickets: Vec<Gfx942SdmaCopyTicketV1>,
        deadline: XgmiBatchDeadlineV1,
    ) -> Result<Gfx942XgmiRetainedPairCompletedBatchV1, Gfx942XgmiRetainedPairWaitFailureV1> {
        run_operation(self.owner.context_mut(), |parts| {
            let pair = parts.borrow();
            pair.queue.wait_batch_for_with_currentness(
                pair.source,
                pair.destination,
                tickets,
                deadline,
                XgmiRouteCurrentnessV1::OrdinaryRetainedPair,
            )
        })
        .map(|inner| Gfx942XgmiRetainedPairCompletedBatchV1 { inner })
        .map_err(|inner| Gfx942XgmiRetainedPairWaitFailureV1 { inner })
    }

    /// Returns the original native owners only after drained operational close.
    /// Pending work makes consuming close terminal and remains in error custody.
    pub fn finish(
        self,
    ) -> Result<Gfx942XgmiOwnedRetainedPairPartsV1, Gfx942XgmiOwnedRetainedPairFailureV1> {
        self.owner
            .finish(|parts| {
                let mut pair = parts.borrow();
                pair.require_drained()?;
                pair.operational()
            })
            .map(Parts::into_parts)
            .map_err(|failure| Gfx942XgmiOwnedRetainedPairFailureV1 { failure })
    }
}

#[cfg(test)]
mod tests;
