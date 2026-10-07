//! Scoped observers share the existing one-shot cell, never the borrowed carrier.

use super::*;
use std::{future::Future, pin::Pin, task::Poll};

/// Executor-neutral observation of an original scoped submission after settlement.
///
/// Polling does not submit or progress work. Run `drive_with_wake_v1` or explicitly
/// progress the owning scope. Dropping this future abandons only observation;
/// the scope still retains the original carrier and its cleanup obligation.
/// The scope brand prevents escape, and this same-thread observer is not `Send`.
/// A failed driver notifies unfinished observers with an error, not a settlement
/// receipt: retained unknown/live custody still requires process fail-stop.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeGfx942ScopedCompletionFutureV1;
/// fn requires_send<T: Send>() {}
/// requires_send::<RuntimeGfx942ScopedCompletionFutureV1<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_runtime::*;
/// fn escape<P: RuntimeGfx942GeneratedCompletionCarrierV1>(
///     context: &mut RuntimeContextV1<KfdRuntimeBackendV1>, device: RuntimeDeviceIdV1,
///     stream: RuntimeStreamIdV1, carrier: P,
/// ) {
///     let _observer = context.with_generated_gfx942_scope_v1(1,
///         std::time::Instant::now() + std::time::Duration::from_secs(1), |scope| {
///             let ticket = scope.try_submit_v1(device, stream, |_| Ok::<P, ()>(carrier)).unwrap();
///             scope.completion_future_v1(&ticket).unwrap()
///         });
/// }
/// ```
#[must_use = "dropping this future does not cancel or settle native work"]
pub struct RuntimeGfx942ScopedCompletionFutureV1<'scope> {
    future: crate::RuntimeAsyncCommandFutureV1<Outcome>,
    _scope: Rc<()>,
    invariant: PhantomData<fn(&'scope ()) -> &'scope ()>,
}

impl Future for RuntimeGfx942ScopedCompletionFutureV1<'_> {
    type Output = Result<(), RuntimeGfx942ScopeErrorV1>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(outcome)) => {
                Poll::Ready(outcome.map_err(RuntimeGfx942ScopeErrorV1::Readback))
            }
            Poll::Ready(Err(crate::RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)) => {
                Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::CancelledBeforeSubmission))
            }
            Poll::Ready(Err(_)) => Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown)),
        }
    }
}

impl<'scope, B, P> RuntimeGfx942GeneratedScopeV1<'scope, '_, B, P>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>,
    P: RuntimeGfx942GeneratedCompletionCarrierV1,
{
    /// Takes the sole future for this ticket, before or after exact settlement.
    /// The ticket remains available for the original typed result-owner gate.
    pub fn completion_future_v1(
        &mut self,
        ticket: &RuntimeGfx942ScopedTicketV1<'scope>,
    ) -> Result<RuntimeGfx942ScopedCompletionFutureV1<'scope>, RuntimeGfx942ScopeErrorV1> {
        if !Rc::ptr_eq(&self.identity, &ticket.scope) {
            return Err(RuntimeGfx942ScopeErrorV1::InvalidTicket);
        }
        let slot = self
            .slots
            .get_mut(ticket.index)
            .ok_or(RuntimeGfx942ScopeErrorV1::InvalidTicket)?;
        let future = slot
            .future
            .take()
            .ok_or(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)?;
        Ok(RuntimeGfx942ScopedCompletionFutureV1 {
            future,
            _scope: Rc::clone(&self.identity),
            invariant: PhantomData,
        })
    }

    /// Drives original borrowed carriers on the caller's thread and executor.
    ///
    /// Each nonadvancing roster scan awaits the caller's wake future if work remains.
    /// Every nonterminal scan yields before another scan, including when the
    /// supplied wake future is immediately ready. The driver requests at most
    /// one cooperative self-wake per scan; it creates no thread or timer.
    /// Supply a real timer/notification that schedules its waker no later than
    /// the supplied absolute deadline to avoid idle busy-polling across executor
    /// turns. Synchronous hook duration and native interrupt integration are not
    /// bounded by this scheduling boundary.
    ///
    /// Submit and obtain observers first, then drive this future concurrently
    /// with those observers inside the lexical callback. Dropping the driver
    /// does not cancel work: the owning scope must still settle or fail-stop.
    pub async fn drive_with_wake_v1<W, F>(
        &mut self,
        mut wait: W,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1>
    where
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        while self.pending_v1() != 0 {
            let transitions = self.progress_v1()?;
            if self.pending_v1() == 0 {
                break;
            }
            if transitions == 0 {
                wait(self.deadline).await;
            }
            yield_to_executor().await;
        }
        self.settled_result_v1()
    }
}

async fn yield_to_executor() {
    let mut yielded = false;
    std::future::poll_fn(move |cx| {
        if yielded {
            Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await
}
