//! Same-thread lexical custody for generated carriers with borrowed owners.

use super::*;
use crate::generated_source::GeneratedHostRosterV1;
use crate::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeBackendErrorV1, KfdRuntimeBackendV1,
    RuntimeGfx942GeneratedCompletionCarrierV1, RuntimeGfx942GeneratedReservationErrorV1,
    RuntimeGfx942ReadbackErrorV1,
};
use std::rc::Rc;

mod lifecycle;
use lifecycle::{Lifecycle, Phase};
mod futures;
pub use futures::RuntimeGfx942ScopedCompletionFutureV1;
mod copies;
pub use copies::{RuntimeGfx942ScopedCopyFutureV1, RuntimeGfx942ScopedCopyTicketV1};
mod graph;
pub use graph::{
    RuntimeGfx942ScopedGraphAdmissionErrorV1, RuntimeGfx942ScopedGraphFutureV1,
    RuntimeGfx942ScopedGraphStagingErrorV1, RuntimeGfx942ScopedGraphTicketV1,
};
mod arena1024;
mod cancellation;
mod cohort3;
mod registry4;
pub use arena1024::{
    RuntimeGfx942Arena1024ResultFutureV1, RuntimeGfx942Arena1024ScopeV1,
    RuntimeGfx942Arena1024TicketV1,
};
pub use cancellation::RuntimeGfx942ScopedCancelResultV1;
pub use cohort3::RuntimeGfx942ScopedCohort3TicketV1;
pub use registry4::{
    RuntimeGfx942Registry4ResultFutureV1, RuntimeGfx942Registry4ScopeV1,
    RuntimeGfx942Registry4TicketV1, RuntimeGfx942Registry16ResultFutureV1,
    RuntimeGfx942Registry16ScopeV1, RuntimeGfx942Registry16TicketV1,
};

/// Finite owner-local metadata bound; native admission still uses Context credits.
pub const MAX_RUNTIME_GFX942_SCOPED_SUBMISSIONS_V1: usize = 4096;

/// Ceiling for the two owner-Vec backing allocations only. This excludes
/// pointees, replies, Context/backend storage and allocator bookkeeping, and is
/// neither total memory accounting nor native thousands-in-flight qualification.
pub const MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1: usize = 64 * 1024 * 1024;

type Outcome = Result<(), RuntimeGfx942ReadbackErrorV1>;
type NativeError = RuntimeErrorV1<KfdRuntimeBackendErrorV1>;

#[derive(Debug)]
pub enum RuntimeGfx942ScopeErrorV1 {
    Capacity,
    InvalidTicket,
    CompletionObserverTaken,
    CancelledBeforeSubmission,
    CancelledBeforePublication,
    /// Actual classified rejection followed by complete original-owner disposal.
    /// Native DATA may have existed; this does not certify a no-effect writer.
    RejectedBeforePublication,
    Deadline,
    Unknown,
    CopyFailed {
        code: i64,
    },
    Reservation(RuntimeGfx942GeneratedReservationErrorV1),
    Context(NativeError),
    Readback(RuntimeGfx942ReadbackErrorV1),
    Graph(crate::RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>),
}

#[derive(Debug)]
pub enum RuntimeGfx942ScopedSubmissionErrorV1<E> {
    Preparation(RuntimeGfx942PreparationErrorV1<E>),
    Scope(RuntimeGfx942ScopeErrorV1),
}

impl fmt::Display for RuntimeGfx942ScopeErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "generated scope: {self:?}")
    }
}
impl Error for RuntimeGfx942ScopeErrorV1 {}

impl<E: fmt::Debug> fmt::Display for RuntimeGfx942ScopedSubmissionErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "scoped submission: {self:?}")
    }
}
impl<E: fmt::Debug + 'static> Error for RuntimeGfx942ScopedSubmissionErrorV1<E> {}

/// Address-free, non-cloneable reference into one lexical scope.
/// Forgetting this ticket cannot detach the carrier retained by the scope.
pub struct RuntimeGfx942ScopedTicketV1<'scope> {
    scope: Rc<()>,
    index: usize,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

struct Slot<P> {
    lifecycle: Lifecycle<RuntimeGfx942PreparedV1<P>, Outcome>,
    roster: GeneratedHostRosterV1,
    hold: ContextUnpublishedHoldV1,
    domain: CompletionDomainsV1,
    reply: crate::async_engine::RuntimeAsyncReplyV1<Outcome>,
    future: Option<crate::RuntimeAsyncCommandFutureV1<Outcome>>,
}

enum CompletionDomainsV1 {
    Singleton(crate::RuntimeGeneratedResultDomainV1),
    Cohort3([crate::RuntimeGeneratedResultDomainV1; 3]),
}

impl CompletionDomainsV1 {
    fn matches_single<T: Send + Sync + 'static>(&self, owner: &std::sync::Arc<T>) -> bool {
        matches!(self, Self::Singleton(domain) if domain.matches_owner(owner))
    }
}

fn roster_bytes<P>(slots: usize, copies: usize) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
    slots
        .checked_mul(size_of::<Slot<P>>())
        .and_then(|n| {
            copies
                .checked_mul(size_of::<copies::CopySlot>())
                .and_then(|m| n.checked_add(m))
        })
        .filter(|&n| n <= MAX_RUNTIME_GFX942_SCOPED_ROSTER_BYTES_V1)
        .ok_or(RuntimeGfx942ScopeErrorV1::Capacity)
}

fn allocate_rosters<P>(
    capacity: usize,
) -> Result<(Vec<Slot<P>>, Vec<copies::CopySlot>), RuntimeGfx942ScopeErrorV1> {
    if capacity == 0 || capacity > MAX_RUNTIME_GFX942_SCOPED_SUBMISSIONS_V1 {
        return Err(RuntimeGfx942ScopeErrorV1::Capacity);
    }
    // Keep stable indices through settlement. Each Vec can hold the entire
    // combined roster; neither reallocates after an original owner is rooted.
    roster_bytes::<P>(capacity, capacity)?;
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(capacity)
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
    let mut copies = Vec::new();
    copies
        .try_reserve_exact(capacity)
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
    roster_bytes::<P>(slots.capacity(), copies.capacity())?;
    Ok((slots, copies))
}

type Reserved<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut RuntimeGfx942PreparedV1<P>,
)
    -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1>;
type Preflight<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &RuntimeGfx942PreparedV1<P>,
    &GeneratedHostRosterV1,
    RuntimeStreamIdV1,
    Option<ContextGraphReservationV1>,
) -> Result<(), NativeError>;
type Step<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut RuntimeGfx942PreparedV1<P>,
    &GeneratedHostRosterV1,
    &ContextUnpublishedHoldV1,
) -> Result<(), NativeError>;
type Progress<B, P> = fn(
    &mut RuntimeContextV1<B>,
    &mut RuntimeGfx942PreparedV1<P>,
    &GeneratedHostRosterV1,
    &ContextUnpublishedHoldV1,
) -> Result<bool, NativeError>;

type GraphSubmit<B> = fn(
    &mut RuntimeContextV1<B>,
    ContextGraphReservationV1,
    PreparedContextGraphActionV1,
) -> Result<ContextGraphSubmissionV1, NativeError>;
type GraphProgress<B> = fn(
    &mut RuntimeContextV1<B>,
    RuntimeStreamIdV1,
    Option<ContextGraphReservationV1>,
) -> Result<(), NativeError>;

struct Hooks<B: RuntimeBackendV1, P> {
    domains: fn(&P) -> Result<CompletionDomainsV1, RuntimeGfx942ReadbackErrorV1>,
    decode: fn(RuntimeGfx942PreparedV1<P>) -> Outcome,
    progress_graph: for<'s, 'e> fn(
        &mut RuntimeGfx942GeneratedScopeV1<'s, 'e, B, P>,
    ) -> Result<usize, RuntimeGfx942ScopeErrorV1>,
    progress_copies: for<'s, 'e> fn(
        &mut RuntimeGfx942GeneratedScopeV1<'s, 'e, B, P>,
    ) -> Result<usize, RuntimeGfx942ScopeErrorV1>,
    reserve: Reserved<B, P>,
    preflight: Preflight<B, P>,
    ready: fn(&RuntimeContextV1<B>, &ContextUnpublishedHoldV1) -> Result<bool, NativeError>,
    adopt: Step<B, P>,
    progress: Progress<B, P>,
    rejected: fn(&mut RuntimeContextV1<B>, &ContextUnpublishedHoldV1) -> Result<bool, NativeError>,
    retire_rejected: Step<B, P>,
    complete: Step<B, P>,
    unpublished:
        fn(&mut RuntimeContextV1<B>, &ContextUnpublishedHoldV1) -> Result<bool, NativeError>,
    retire_unpublished:
        fn(&mut RuntimeContextV1<B>, &ContextUnpublishedHoldV1) -> Result<(), NativeError>,
    copy_progress: fn(&mut RuntimeContextV1<B>, RuntimeStreamIdV1) -> Result<(), NativeError>,
    graph_submit: GraphSubmit<B>,
    graph_progress: GraphProgress<B>,
}

/// Owns every carrier until exact native settlement and its original decoder.
///
/// This scope neither spawns a thread nor erases a lifetime. Its preparation
/// closure may borrow same-thread proof/currentness/publication owners. On normal
/// return outstanding work drains to the fixed deadline. Unknown effects, a
/// timeout, or unwinding while custody remains cause process fail-stop *before*
/// any retained carrier can be freed. Cancellation is not a release operation.
/// This bounded metadata allocation is not a claim about aggregate process RSS.
pub struct RuntimeGfx942GeneratedScopeV1<'scope, 'env, B: RuntimeBackendV1, P> {
    epoch: scope_epoch::Owner,
    context: &'env mut RuntimeContextV1<B>,
    slots: Vec<Slot<P>>,
    copies: Vec<copies::CopySlot>,
    graph: Option<graph::Graph<P>>,
    capacity: usize,
    identity: Rc<()>,
    deadline: Instant,
    hooks: Hooks<B, P>,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl<B: RuntimeBackendV1, P> Drop for RuntimeGfx942GeneratedScopeV1<'_, '_, B, P> {
    fn drop(&mut self) {
        if self.slots.iter().any(|slot| slot.lifecycle.unsettled())
            || self.copies.iter().any(copies::CopySlot::unsettled)
            || self.graph.as_ref().is_some_and(graph::Graph::unsettled)
        {
            std::process::abort();
        }
        self.epoch.close();
    }
}

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
{
    /// Exact element-capacity bytes of the two preallocated owner arrays.
    /// Does not include indirect allocations, Context/backend resources or RSS.
    pub fn roster_capacity_bytes_v1(&self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        roster_bytes::<P>(self.slots.capacity(), self.copies.capacity())
    }

    fn check_submission(&self) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if self.graph.is_some() {
            return Err(RuntimeGfx942ScopeErrorV1::Graph(
                crate::RuntimeGraphErrorV1::Busy,
            ));
        }
        if self.slots.len() + self.copies.len() >= self.capacity {
            return Err(RuntimeGfx942ScopeErrorV1::Capacity);
        }
        if Instant::now() >= self.deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        if self.context.is_terminal()
            || self
                .slots
                .iter()
                .any(|s| s.lifecycle.phase == Phase::Unknown)
            || self.copies.iter().any(|s| s.unknown)
        {
            return Err(RuntimeGfx942ScopeErrorV1::Unknown);
        }
        Ok(())
    }

    fn admit(
        &mut self,
        mut prepared: RuntimeGfx942PreparedV1<P>,
        stream: RuntimeStreamIdV1,
    ) -> Result<RuntimeGfx942ScopedTicketV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.check_submission()?;
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        let domain =
            (self.hooks.domains)(prepared.value()).map_err(RuntimeGfx942ScopeErrorV1::Readback)?;
        let roster = (self.hooks.reserve)(self.context, &mut prepared)
            .map_err(RuntimeGfx942ScopeErrorV1::Reservation)?;
        (self.hooks.preflight)(self.context, &prepared, &roster, stream, None)
            .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
        let (reply, future) = crate::async_engine::RuntimeAsyncReplyV1::pair();
        let hold = self
            .context
            // This first lexical profile owns independent streams, not a graph
            // reservation. The same None was checked by preflight above; an
            // active graph reservation therefore rejects at both boundaries.
            .hold_unpublished_stream_with_access_v1(stream, None)
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        // All allocations precede this hold. The scope owns the entire prefix
        // before any generated-shell/native adoption hook can be called.
        let index = self.slots.len();
        self.slots.push(Slot {
            lifecycle: Lifecycle::new(prepared),
            roster,
            hold,
            domain,
            reply,
            future: Some(future),
        });
        Ok(RuntimeGfx942ScopedTicketV1 {
            scope: Rc::clone(&self.identity),
            index,
            invariant: PhantomData,
        })
    }

    /// Performs at most one transition per retained submission, in roster order.
    pub fn progress_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let result = self.progress_once_v1();
        if result.is_err() {
            // Notification is not settlement. Original carriers and holds stay
            // retained, so joining failed observers cannot release live custody.
            for slot in &mut self.slots {
                if slot.lifecycle.unsettled() {
                    slot.reply
                        .complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
                }
            }
            for slot in &mut self.copies {
                if slot.unsettled() {
                    slot.notify_unknown();
                }
            }
            if let Some(graph) = &mut self.graph {
                graph.notify_unknown();
            }
        }
        result
    }

    fn progress_once_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        if self.context.is_terminal()
            || self
                .slots
                .iter()
                .any(|s| s.lifecycle.phase == Phase::Unknown)
            || self.copies.iter().any(|s| s.unknown)
        {
            return Err(RuntimeGfx942ScopeErrorV1::Unknown);
        }
        if Instant::now() >= self.deadline && self.pending_v1() != 0 {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        let mut transitions = 0;
        transitions += (self.hooks.progress_graph)(self)?;
        for slot in &mut self.slots {
            let context = &mut *self.context;
            let hooks = &self.hooks;
            let before = slot.lifecycle.phase;
            let mut changed = slot
                .lifecycle
                .advance(
                    |phase, prepared| match phase {
                        Phase::Adopting => {
                            if !(hooks.ready)(context, &slot.hold)? {
                                return Ok(false);
                            }
                            (hooks.adopt)(context, prepared, &slot.roster, &slot.hold)?;
                            Ok(true)
                        }
                        Phase::Issuing => {
                            (hooks.progress)(context, prepared, &slot.roster, &slot.hold)
                        }
                        Phase::Completing => {
                            (hooks.complete)(context, prepared, &slot.roster, &slot.hold)?;
                            Ok(true)
                        }
                        _ => unreachable!("lifecycle rejects terminal transitions"),
                    },
                    hooks.decode,
                )
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            if before == Phase::Issuing
                && !changed
                && (hooks.rejected)(context, &slot.hold)
                    .map_err(RuntimeGfx942ScopeErrorV1::Context)?
            {
                changed = slot
                    .lifecycle
                    .settle_rejected(
                        context,
                        |context, prepared| {
                            (hooks.retire_rejected)(context, prepared, &slot.roster, &slot.hold)
                        },
                        |context| {
                            context
                                .release_unpublished_hold_v1(&slot.hold)
                                .map_err(Into::into)
                        },
                    )
                    .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
                slot.reply.complete(Err(
                    crate::RuntimeAsyncEngineCallErrorV1::RejectedBeforePublication,
                ));
            }
            if let Some(outcome) = &slot.lifecycle.outcome {
                slot.reply.complete(Ok(outcome.clone()));
            }
            transitions += usize::from(changed);
            if self.context.is_terminal() {
                return Err(RuntimeGfx942ScopeErrorV1::Unknown);
            }
        }
        transitions += (self.hooks.progress_copies)(self)?;
        Ok(transitions)
    }

    pub fn pending_v1(&self) -> usize {
        if let Some(graph) = &self.graph {
            return graph.pending();
        }
        self.slots
            .iter()
            .filter(|s| s.lifecycle.unsettled())
            .count()
            + self.copies.iter().filter(|s| s.unsettled()).count()
    }

    /// Observes only the result of the original decoder after native settlement.
    pub fn completion_v1(
        &self,
        ticket: &RuntimeGfx942ScopedTicketV1<'scope>,
    ) -> Result<Option<&Outcome>, RuntimeGfx942ScopeErrorV1> {
        if !Rc::ptr_eq(&self.identity, &ticket.scope) {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        let slot = self
            .slots
            .get(ticket.index)
            .ok_or(RuntimeGfx942ScopeErrorV1::InvalidTicket)?;
        if slot.lifecycle.phase == Phase::Cancelled {
            return Err(RuntimeGfx942ScopeErrorV1::CancelledBeforeSubmission);
        }
        if slot.lifecycle.phase == Phase::CancelledUnpublished {
            return Err(RuntimeGfx942ScopeErrorV1::CancelledBeforePublication);
        }
        if slot.lifecycle.phase == Phase::FailedUnpublished {
            return Err(RuntimeGfx942ScopeErrorV1::RejectedBeforePublication);
        }
        Ok(slot.lifecycle.outcome.as_ref())
    }

    /// Joins an exact successful decoder result to its original host gate.
    pub fn completion_matches_owner_v1<T: Send + Sync + 'static>(
        &self,
        ticket: &RuntimeGfx942ScopedTicketV1<'scope>,
        owner: &std::sync::Arc<T>,
    ) -> Result<bool, RuntimeGfx942ScopeErrorV1> {
        let complete = matches!(self.completion_v1(ticket)?, Some(Ok(())));
        Ok(complete && self.slots[ticket.index].domain.matches_single(owner))
    }

    pub fn drain_v1(&mut self) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        while self.pending_v1() != 0 {
            if self.progress_v1()? == 0 {
                std::thread::yield_now();
            }
        }
        self.settled_result_v1()
    }

    fn settled_result_v1(&self) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        for slot in &self.slots {
            if slot.lifecycle.phase == Phase::FailedUnpublished {
                return Err(RuntimeGfx942ScopeErrorV1::RejectedBeforePublication);
            }
            if let Some(Err(error)) = &slot.lifecycle.outcome {
                return Err(RuntimeGfx942ScopeErrorV1::Readback(error.clone()));
            }
        }
        for slot in &self.copies {
            if let Some(RuntimePollV1::Failed { code }) = slot.settled {
                return Err(RuntimeGfx942ScopeErrorV1::CopyFailed { code });
            }
        }
        Ok(())
    }
}

macro_rules! impl_scoped_generated {
    ($backend:ty) => {
        impl RuntimeContextV1<$backend> {
            /// Executes a lexical owner-local scope with no `'static` or Send requirement.
            /// Capacity is the total compute-and-copy roster for this scope, not an unbounded queue.
            /// Settled ticket slots are not reused; start another fully settled
            /// scope to reuse Context capacity. Both owner arrays are bounded
            /// independently of any storage retained by their pointees.
            /// A singleton's definite publication rejection fails locally only
            /// after original native abort, currentness, Context and source-owner
            /// settlement. It never produces decoded output or a successful DATA
            /// version. Unknown state and device/currentness failures still stop
            /// the whole Context; this is not device-reset fault isolation.
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
            ///     s: RuntimeStreamIdV1, p: P,
            /// ) {
            ///     let _ticket = c.with_generated_gfx942_scope_v1(1, std::time::Instant::now(),
            ///         |scope| scope.try_submit_v1(d, s, |_| Ok::<P, ()>(p)).unwrap());
            /// }
            /// ```
            pub fn with_generated_gfx942_scope_v1<'env, P, R>(
                &'env mut self,
                capacity: usize,
                deadline: Instant,
                use_scope: impl for<'scope> FnOnce(
                    &mut RuntimeGfx942GeneratedScopeV1<'scope, 'env, $backend, P>,
                ) -> R,
            ) -> Result<R, RuntimeGfx942ScopeErrorV1>
            where P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
            {
                let mut scope = self.new_generated_scope_v1(capacity, deadline)?;
                let result = use_scope(&mut scope);
                scope.drain_v1()?;
                Ok(result)
            }

            /// Lending same-thread scope over the original Context and carriers.
            /// The callback may await its submitted observers and driver. On
            /// normal callback return, the same scope asynchronously drains any
            /// remaining owners using only the caller's wake factory. A ready
            /// wake source can busy-poll across executor turns; each nonterminal
            /// scan cooperatively yields, and no timer or owner thread is created.
            ///
            /// Dropping a polled scope with unsettled owners fails stop. Forgetting
            /// it leaves a persistent Context reservation: ordinary mutation and
            /// cleanup refuse, backend access panics, and Context teardown fails
            /// stop before the backend. No cancellation or timeout releases work.
            /// The first poll establishes the epoch; an unpolled future is inert.
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// async fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
            ///     s: RuntimeStreamIdV1, p: P,
            /// ) {
            ///     let _ticket = c.with_generated_gfx942_scope_async_v1(1,
            ///         std::time::Instant::now(), |_| std::future::ready(()),
            ///         async |scope| scope.try_submit_v1(d, s, |_| Ok::<P, ()>(p)).unwrap()).await;
            /// }
            /// ```
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// async fn escape_observer<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
            ///     s: RuntimeStreamIdV1, p: P,
            /// ) {
            ///     let _observer = c.with_generated_gfx942_scope_async_v1(1,
            ///         std::time::Instant::now(), |_| std::future::ready(()),
            ///         async |scope| {
            ///             let ticket = scope.try_submit_v1(d, s, |_| Ok::<P, ()>(p)).unwrap();
            ///             scope.completion_future_v1(&ticket).unwrap()
            ///         }).await;
            /// }
            /// ```
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// fn same_thread<P: RuntimeGfx942GeneratedCompletionCarrierV1 + Send>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
            /// ) {
            ///     fn require_send<T: Send>(_: T) {}
            ///     require_send(c.with_generated_gfx942_scope_async_v1::<P, (), _, _>(1,
            ///         std::time::Instant::now(), |_| std::future::ready(()), async |_| {}));
            /// }
            /// ```
            ///
            /// ```compile_fail
            /// use fe2o3_runtime::*;
            /// fn unique_context<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
            ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
            /// ) {
            ///     let future = c.with_generated_gfx942_scope_async_v1::<P, (), _, _>(1,
            ///         std::time::Instant::now(), |_| std::future::ready(()), async |_| {});
            ///     let _ = c.cleanup();
            ///     drop(future);
            /// }
            /// ```
            pub async fn with_generated_gfx942_scope_async_v1<'env, P, R, W, F>(
                &'env mut self,
                capacity: usize,
                deadline: Instant,
                wait: W,
                use_scope: impl for<'scope> AsyncFnOnce(
                    &mut RuntimeGfx942GeneratedScopeV1<'scope, 'env, $backend, P>,
                ) -> R,
            ) -> Result<R, RuntimeGfx942ScopeErrorV1>
            where
                P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
                W: FnMut(Instant) -> F,
                F: std::future::Future<Output = ()>,
            {
                let mut scope = self.new_generated_scope_v1(capacity, deadline)?;
                let result = use_scope(&mut scope).await;
                scope.drive_with_wake_v1(wait).await?;
                Ok(result)
            }

            fn new_generated_scope_v1<'scope, 'env, P>(
                &'env mut self,
                capacity: usize,
                deadline: Instant,
            ) -> Result<RuntimeGfx942GeneratedScopeV1<'scope, 'env, $backend, P>, RuntimeGfx942ScopeErrorV1>
            where P: RuntimeGfx942GeneratedCompletionCarrierV1 + 'env,
            {
                if capacity == 0 || capacity > MAX_RUNTIME_GFX942_SCOPED_SUBMISSIONS_V1 {
                    return Err(RuntimeGfx942ScopeErrorV1::Capacity);
                }
                if Instant::now() >= deadline { return Err(RuntimeGfx942ScopeErrorV1::Deadline); }
                self.require_live().map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
                let (slots, copies) = allocate_rosters(capacity)?;
                let identity = Rc::new(());
                let epoch = self.scope_epoch.begin().map_err(|error|
                    RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
                Ok(RuntimeGfx942GeneratedScopeV1 {
                    epoch, context: self, slots, copies, graph: None, capacity, deadline, identity,
                    invariant: PhantomData,
                    hooks: Hooks {
                        domains: |value| value.completion_domain_v1().map(CompletionDomainsV1::Singleton),
                        decode: RuntimeGfx942PreparedV1::complete_readback_v1,
                        progress_graph: Self::progress_generated_scope_graph_v1::<P>,
                        progress_copies: Self::progress_generated_scope_copies_v1::<P>,
                        reserve: Self::reserve_gfx942_prepared_v1::<P>,
                        preflight: Self::preflight_gfx942_adoption_v1::<P>,
                        ready: Self::gfx942_adoption_ready_v1,
                        adopt: Self::adopt_gfx942_prepared_v1::<P>,
                        progress: Self::progress_gfx942_issue_preserving_rejection_v1::<P>,
                        rejected: Self::gfx942_issue_rejected_v1,
                        retire_rejected: Self::settle_gfx942_rejected_v1::<P>,
                        complete: Self::complete_gfx942_issue_v1::<P>,
                        unpublished: Self::gfx942_adoption_unpublished_v1,
                        retire_unpublished: Self::retire_gfx942_unpublished_v1,
                        copy_progress: Self::progress_stream_v1,
                        graph_submit: Self::submit_graph_action_v1,
                        graph_progress: |context, stream, access| context.drive_stream_with_graph_access_v1(
                            stream, access, <$backend as RuntimeFlushBackendV1>::progress_stream_v1),
                    },
                })
            }

            fn progress_generated_scope_graph_v1<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
                scope: &mut RuntimeGfx942GeneratedScopeV1<'_, '_, $backend, P>,
            ) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
                scope.progress_graph_v1()
            }

            fn progress_generated_scope_copies_v1<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
                scope: &mut RuntimeGfx942GeneratedScopeV1<'_, '_, $backend, P>,
            ) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
                scope.progress_copies_v1()
            }
        }

        impl<'scope, P: RuntimeGfx942GeneratedCompletionCarrierV1>
            RuntimeGfx942GeneratedScopeV1<'scope, '_, $backend, P>
        {
            /// Prepares and retains one original carrier without publishing GPU work.
            pub fn try_submit_v1<E>(
                &mut self,
                device: RuntimeDeviceIdV1,
                stream: RuntimeStreamIdV1,
                prepare: impl FnOnce(&fe2o3_kfd::CheckedGfx942XnackMinusDevice) -> Result<P, E>,
            ) -> Result<RuntimeGfx942ScopedTicketV1<'scope>, RuntimeGfx942ScopedSubmissionErrorV1<E>> {
                self.check_submission().map_err(RuntimeGfx942ScopedSubmissionErrorV1::Scope)?;
                let prepared = {
                    let _permit = self.epoch.enter().map_err(|error|
                        RuntimeGfx942ScopedSubmissionErrorV1::Scope(
                            RuntimeGfx942ScopeErrorV1::Context(error.into())))?;
                    self.context.with_gfx942_preparation_device_v1(device, prepare)
                        .map_err(RuntimeGfx942ScopedSubmissionErrorV1::Preparation)?
                };
                self.admit(prepared, stream).map_err(RuntimeGfx942ScopedSubmissionErrorV1::Scope)
            }
        }
    };
}
impl_scoped_generated!(KfdRuntimeBackendV1);
impl_scoped_generated!(KfdMultiDeviceRuntimeBackendV1);

#[cfg(test)]
mod tests;
