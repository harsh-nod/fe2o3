//! Scope-owned asynchronous copies on the original Context queues and journal.

use super::*;
use std::{future::Future, pin::Pin, task::Poll};

enum Submission {
    SameDevice(RuntimeSubmissionV1<RuntimeCopyV1>),
    Peer(RuntimeSubmissionV1<RuntimePeerCopyV1>),
}

pub(super) struct CopySlot {
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
    submission: Option<Submission>,
    pub(super) unknown: bool,
    pub(super) settled: Option<RuntimePollV1>,
    reply: crate::async_engine::RuntimeAsyncReplyV1<RuntimePollV1>,
    future: Option<crate::RuntimeAsyncCommandFutureV1<RuntimePollV1>>,
}

impl CopySlot {
    pub(super) fn conflicts_with_generated(
        &self,
        stream: RuntimeStreamIdV1,
        allocation: RuntimeAllocationIdV1,
    ) -> bool {
        self.unsettled()
            && (self.stream == stream
                || self.source.allocation == allocation
                || self.destination.allocation == allocation)
    }

    pub(super) fn unsettled(&self) -> bool {
        self.settled.is_none()
    }

    pub(super) fn notify_unknown(&mut self) {
        self.reply
            .complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
    }
}

/// A non-cloneable copy observer branded to its original lexical scope.
///
/// It does not own a native handle. Forgetting it cannot detach the original
/// Context submission or release either allocation. Neither copy tickets nor
/// their futures can escape the higher-ranked scope callback.
///
/// ```compile_fail
/// use fe2o3_runtime::*;
/// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, s: RuntimeStreamIdV1,
///     source: RuntimeMemoryRegionV1, destination: RuntimeMemoryRegionV1,
/// ) {
///     let _ticket = c.with_generated_gfx942_scope_v1::<P, _>(1,
///         std::time::Instant::now() + std::time::Duration::from_secs(1),
///         |scope| scope.copy_async_v1(s, source, destination).unwrap());
/// }
/// ```
pub struct RuntimeGfx942ScopedCopyTicketV1<'scope> {
    pub(super) scope: Rc<()>,
    pub(super) index: usize,
    pub(super) invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

/// Observes copy settlement and exact Context submission release, not merely
/// hardware notification. Dropping it does not cancel or settle the copy.
///
/// Polling never progresses work. Use the scope's existing caller-driven
/// `drive_with_wake_v1` or `progress_v1`; no new thread, queue or allocator is
/// introduced. Native SDMA and cooperative staging retain their existing
/// concrete backend behavior, including backend-defined host reconciliation
/// bounds during progress. No hard wall-clock bound is claimed. Simultaneous
/// in-flight work is not evidence of physical copy/compute overlap.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeGfx942ScopedCopyFutureV1;
/// fn requires_send<T: Send>() {}
/// requires_send::<RuntimeGfx942ScopedCopyFutureV1<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_runtime::*;
/// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, s: RuntimeStreamIdV1,
///     source: RuntimeMemoryRegionV1, destination: RuntimeMemoryRegionV1,
/// ) {
///     let _future = c.with_generated_gfx942_scope_v1::<P, _>(1,
///         std::time::Instant::now() + std::time::Duration::from_secs(1), |scope| {
///             let ticket = scope.copy_async_v1(s, source, destination).unwrap();
///             scope.copy_completion_future_v1(&ticket).unwrap()
///         });
/// }
/// ```
#[must_use = "dropping this future does not cancel or settle native work"]
pub struct RuntimeGfx942ScopedCopyFutureV1<'scope> {
    future: crate::RuntimeAsyncCommandFutureV1<RuntimePollV1>,
    _scope: Rc<()>,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl Future for RuntimeGfx942ScopedCopyFutureV1<'_> {
    type Output = Result<(), RuntimeGfx942ScopeErrorV1>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(RuntimePollV1::Succeeded)) => Poll::Ready(Ok(())),
            Poll::Ready(Ok(RuntimePollV1::Failed { code })) => {
                Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::CopyFailed { code }))
            }
            Poll::Ready(Ok(RuntimePollV1::Pending) | Err(_)) => {
                Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown))
            }
        }
    }
}

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    fn submit_copy_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        submit: impl FnOnce(&mut RuntimeContextV1<B>) -> Result<Submission, NativeError>,
    ) -> Result<RuntimeGfx942ScopedCopyTicketV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.check_submission()?;
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        let validate = || -> Result<(), RuntimeValidationErrorV1> {
            self.context.require_graph_access(None)?;
            if self.context.versions.is_none() {
                return Err(RuntimeValidationErrorV1::Unsupported);
            }
            self.context.require_stream_unheld_v1(stream)?;
            if self
                .slots
                .iter()
                .filter(|slot| slot.lifecycle.unsettled())
                .filter_map(|slot| slot.data_copy.as_ref())
                .any(|request| {
                    request.stream == stream
                        || [source.allocation, destination.allocation]
                            .contains(&request.destination.allocation)
                })
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            if self
                .context
                .submissions
                .values()
                .any(|record| record.stream == stream && !record.quiescent)
                || self
                    .copies
                    .iter()
                    .filter(|slot| slot.unsettled())
                    .any(|slot| {
                        slot.stream == stream
                            || [slot.source.allocation, slot.destination.allocation]
                                .iter()
                                .any(|old| {
                                    *old == source.allocation || *old == destination.allocation
                                })
                    })
            {
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            Ok(())
        };
        validate().map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        let (reply, future) = crate::async_engine::RuntimeAsyncReplyV1::pair();
        let index = self.copies.len();
        // Root the exact endpoints before Context can enter the backend. Even
        // a panic or an invalid post-effect handle cannot return this Context.
        self.copies.push(CopySlot {
            stream,
            source,
            destination,
            submission: None,
            unknown: true,
            settled: None,
            reply,
            future: Some(future),
        });
        match submit(self.context) {
            Ok(submission) => {
                self.copies[index].submission = Some(submission);
                self.copies[index].unknown = false;
            }
            Err(error) => {
                if !self.context.is_terminal()
                    && matches!(
                        &error,
                        RuntimeErrorV1::Validation(_) | RuntimeErrorV1::BackendRejected(_)
                    )
                {
                    // These existing Context outcomes guarantee no issued
                    // owner. Quiescent/ambiguous failures are not promoted.
                    self.copies.pop();
                }
                return Err(RuntimeGfx942ScopeErrorV1::Context(error));
            }
        }
        Ok(RuntimeGfx942ScopedCopyTicketV1 {
            scope: Rc::clone(&self.identity),
            index,
            invariant: PhantomData,
        })
    }

    /// Submits an independent same-device asynchronous copy without waiting.
    ///
    /// Both allocations must already belong to this journal-enabled Context.
    /// The exclusive Context borrow keeps them alive and inaccessible until the
    /// whole scope settles. Pending copies require distinct streams and
    /// disjoint allocation endpoints. This first profile accepts no dependency
    /// events or graph reservation; generated buffers remain separately guarded
    /// by the backend and are not cast into ordinary copy allocations.
    pub fn copy_async_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<RuntimeGfx942ScopedCopyTicketV1<'scope>, RuntimeGfx942ScopeErrorV1>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.submit_copy_v1(stream, source, destination, |context| {
            context
                .copy_async(stream, source, destination, &[])
                .map(Submission::SameDevice)
        })
    }

    /// Submits an independent cross-device copy through the already selected
    /// native-peer or staged backend policy. This does not enable a native route
    /// or label cooperative staging as XGMI. Other rules match `copy_async_v1`.
    pub fn peer_copy_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<RuntimeGfx942ScopedCopyTicketV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.submit_copy_v1(stream, source, destination, |context| {
            context
                .peer_copy(stream, source, destination, &[])
                .map(Submission::Peer)
        })
    }

    /// Returns a conclusive status only after exact original submission release.
    pub fn copy_completion_v1(
        &self,
        ticket: &RuntimeGfx942ScopedCopyTicketV1<'scope>,
    ) -> Result<Option<RuntimePollV1>, RuntimeGfx942ScopeErrorV1> {
        self.copy_slot_v1(ticket).map(|slot| slot.settled)
    }

    fn copy_slot_v1(
        &self,
        ticket: &RuntimeGfx942ScopedCopyTicketV1<'scope>,
    ) -> Result<&CopySlot, RuntimeGfx942ScopeErrorV1> {
        if !Rc::ptr_eq(&self.identity, &ticket.scope) {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        self.copies
            .get(ticket.index)
            .ok_or(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    }

    /// Takes the sole non-owning future for this exact copy ticket.
    pub fn copy_completion_future_v1(
        &mut self,
        ticket: &RuntimeGfx942ScopedCopyTicketV1<'scope>,
    ) -> Result<RuntimeGfx942ScopedCopyFutureV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        self.copy_slot_v1(ticket)?;
        let future = self.copies[ticket.index]
            .future
            .take()
            .ok_or(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)?;
        Ok(RuntimeGfx942ScopedCopyFutureV1 {
            future,
            _scope: Rc::clone(&self.identity),
            invariant: PhantomData,
        })
    }

    pub(super) fn progress_copies_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let mut transitions = 0;
        for slot in &mut self.copies {
            if !slot.unsettled() {
                continue;
            }
            if slot.unknown {
                return Err(RuntimeGfx942ScopeErrorV1::Unknown);
            }
            slot.unknown = true;
            (self.hooks.copy_progress)(self.context, slot.stream)
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            let status = match slot.submission.as_mut() {
                Some(Submission::SameDevice(submission)) => self.context.poll(submission),
                Some(Submission::Peer(submission)) => self.context.poll(submission),
                None => return Err(RuntimeGfx942ScopeErrorV1::Unknown),
            }
            .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            if status != RuntimePollV1::Pending {
                match slot.submission.as_ref() {
                    Some(Submission::SameDevice(submission)) => {
                        self.context.release_submission_ref(submission, None)
                    }
                    Some(Submission::Peer(submission)) => {
                        self.context.release_submission_ref(submission, None)
                    }
                    None => return Err(RuntimeGfx942ScopeErrorV1::Unknown),
                }
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
                slot.submission = None;
                slot.settled = Some(status);
                slot.reply.complete(Ok(status));
                transitions += 1;
            }
            slot.unknown = false;
        }
        Ok(transitions)
    }
}
