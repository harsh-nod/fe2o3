//! Bounded registration acknowledgments over the existing observer commands.
use super::*;

pub(super) enum RegistrationResponseV1<E> {
    Blocking(SyncSender<Result<(), E>>),
    Nonblocking(owned::Reply<Result<(), E>>),
}

impl<E> From<SyncSender<Result<(), E>>> for RegistrationResponseV1<E> {
    fn from(value: SyncSender<Result<(), E>>) -> Self {
        Self::Blocking(value)
    }
}

impl<E> RegistrationResponseV1<E> {
    pub(super) fn is_nonblocking(&self) -> bool {
        matches!(self, Self::Nonblocking(_))
    }

    pub(super) fn send(self, result: Result<(), E>) -> Result<(), ()> {
        match self {
            Self::Blocking(sender) => sender.send(result).map_err(|_| ()),
            Self::Nonblocking(mut reply) => {
                reply.complete(Ok(result));
                Ok(())
            }
        }
    }
}

/// Admission acknowledgment carrying the original event/progress observer.
///
/// The outer error reports command failure (including Stop before processing);
/// the inner error reports registration validation/capacity failure. Success
/// returns an observer, not GPU completion. One reply credit remains charged
/// until this acknowledgment future is dropped, including after readiness.
/// Dropping it abandons provisional observation and never cancels or releases
/// native work. A registration already committed may have an in-flight poll or
/// flush; ordinary observer abandonment removes it on subsequent owner progress.
#[must_use = "poll the admission acknowledgment to obtain the original observer"]
pub struct RuntimeAsyncRegistrationFutureV1<T, E> {
    acknowledgment: RuntimeAsyncCommandFutureV1<Result<(), E>>,
    registration: Option<T>,
}

impl<T: Unpin, E> Future for RuntimeAsyncRegistrationFutureV1<T, E> {
    type Output = Result<Result<T, E>, RuntimeAsyncEngineCallErrorV1>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.acknowledgment).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(Ok(()))) => {
                Poll::Ready(Ok(Ok(this.registration.take().expect("original observer"))))
            }
            Poll::Ready(Ok(Err(error))) => {
                drop(this.registration.take());
                Poll::Ready(Ok(Err(error)))
            }
            Poll::Ready(Err(error)) => {
                drop(this.registration.take());
                Poll::Ready(Err(error))
            }
        }
    }
}

impl<T, E> Drop for RuntimeAsyncRegistrationFutureV1<T, E> {
    fn drop(&mut self) {
        // Publish abandonment before disposing the acknowledgment and its waker.
        drop(self.registration.take());
    }
}

pub type RuntimeAsyncEventRegistrationFutureV1<E> =
    RuntimeAsyncRegistrationFutureV1<RuntimeEventFutureV1<E>, RuntimeAsyncEventRegistrationErrorV1>;
pub type RuntimeAsyncProgressRegistrationFutureV1<E> = RuntimeAsyncRegistrationFutureV1<
    RuntimeAsyncProgressRegistrationV1<E>,
    RuntimeAsyncProgressRegistrationErrorV1,
>;
pub type RuntimeAsyncProgressEventRegistrationFutureV1<E> = RuntimeAsyncRegistrationFutureV1<
    RuntimeAsyncProgressEventFutureV1<E>,
    RuntimeAsyncProgressEventRegistrationErrorV1,
>;

type RegistrationPairV1<T, E> = (
    RegistrationResponseV1<E>,
    RuntimeAsyncRegistrationFutureV1<T, E>,
);

fn pair<T, E>(
    budget: &Arc<reply_budget::ReplyBudgetV1>,
    observer: impl FnOnce() -> T,
) -> Result<RegistrationPairV1<T, E>, RuntimeAsyncEngineCallErrorV1> {
    let (reply, acknowledgment) = owned::Reply::budgeted_pair(budget)?;
    Ok((
        RegistrationResponseV1::Nonblocking(reply),
        RuntimeAsyncRegistrationFutureV1 {
            acknowledgment,
            registration: Some(observer()),
        },
    ))
}

fn enqueue<B: RuntimeBackendV1 + 'static>(
    handle: &RuntimeAsyncEngineHandleV1<B>,
    command: RuntimeAsyncEngineCommandV1<B>,
) -> Result<(), RuntimeAsyncEngineCallErrorV1> {
    match handle.try_send_command(command) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(_)) => Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull),
        Err(TrySendError::Disconnected(_)) => Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncEngineHandleV1<B> {
    /// Enqueues unique event observation without waiting for owner admission.
    /// Await the acknowledgment, then await the returned original event future.
    /// Reserves a reply cell before bounded command admission and the existing
    /// owner-side waiter-capacity check.
    pub fn enqueue_event_registration(
        &self,
        event: RuntimeEventIdV1,
    ) -> Result<RuntimeAsyncEventRegistrationFutureV1<B::Error>, RuntimeAsyncEngineCallErrorV1>
    {
        if self.rejects_async_enqueue() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (response, pending) = pair(&self.reply_budget, || RuntimeEventFutureV1 {
            event,
            cell: Arc::new(RuntimeAsyncFutureCellV1::new()),
            completed: false,
        })?;
        enqueue(
            self,
            RuntimeAsyncEngineCommandV1::Register {
                event,
                cell: Arc::clone(
                    &pending
                        .registration
                        .as_ref()
                        .expect("provisional observer")
                        .cell,
                ),
                response,
            },
        )?;
        Ok(pending)
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    /// Enqueues a stream-progress registration without a blocking acknowledgment.
    /// The returned guard has the same scheduling-domain and Drop semantics as
    /// [`Self::register_stream`]. Admission success is not device progress.
    pub fn enqueue_stream_registration(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<RuntimeAsyncProgressRegistrationFutureV1<B::Error>, RuntimeAsyncEngineCallErrorV1>
    {
        if self.observer.rejects_async_enqueue() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (response, pending) = pair(&self.observer.reply_budget, || {
            RuntimeAsyncProgressRegistrationV1 {
                stream,
                cell: Arc::new(RuntimeAsyncProgressCellV1::new()),
            }
        })?;
        enqueue(
            &self.observer,
            RuntimeAsyncEngineCommandV1::RegisterProgress {
                stream,
                cell: Arc::clone(
                    &pending
                        .registration
                        .as_ref()
                        .expect("provisional observer")
                        .cell,
                ),
                response,
            },
        )?;
        Ok(pending)
    }

    /// Enqueues the existing atomic event/source-stream registration transaction.
    /// Neither registration commits on admission failure. Polling, flushing,
    /// retryable failures and Drop retain the semantics of
    /// [`Self::event_future_with_progress`]. No host thread is started.
    ///
    /// ```no_run
    /// use fe2o3_runtime::{RuntimeAsyncCurrentThreadOwnedEngineV1,
    ///     RuntimeAsyncProgressHandleV1, RuntimeBackendV1, RuntimeFlushBackendV1,
    ///     RuntimeOwnedShutdownBackendV1, RuntimeStreamIdV1, RuntimeEventIdV1};
    /// use std::time::{Duration, Instant};
    /// fn observe<B>(engine: &mut RuntimeAsyncCurrentThreadOwnedEngineV1<B>,
    ///     handle: &RuntimeAsyncProgressHandleV1<B>, stream: RuntimeStreamIdV1,
    ///     event: RuntimeEventIdV1)
    /// where B: RuntimeBackendV1 + RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1 + 'static {
    ///     let mut work = std::pin::pin!(async {
    ///         let observer = handle.enqueue_event_registration_with_progress(stream, event)
    ///             .unwrap().await.unwrap().unwrap();
    ///         observer.await
    ///     });
    ///     let result = engine.drive_until_ready(work.as_mut(), Instant::now() + Duration::from_secs(1));
    ///     // On deadline expiry, the same pinned work can be driven again.
    ///     let _ = result;
    /// }
    /// ```
    pub fn enqueue_event_registration_with_progress(
        &self,
        stream: RuntimeStreamIdV1,
        event: RuntimeEventIdV1,
    ) -> Result<
        RuntimeAsyncProgressEventRegistrationFutureV1<B::Error>,
        RuntimeAsyncEngineCallErrorV1,
    > {
        if self.observer.rejects_async_enqueue() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (response, pending) = pair(&self.observer.reply_budget, || {
            let progress_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
            RuntimeAsyncProgressEventFutureV1 {
                future: RuntimeEventFutureV1 {
                    event,
                    cell: Arc::new(RuntimeAsyncFutureCellV1::with_progress(
                        stream,
                        Arc::clone(&progress_cell),
                    )),
                    completed: false,
                },
                progress: RuntimeAsyncProgressRegistrationV1 {
                    stream,
                    cell: progress_cell,
                },
            }
        })?;
        let observer = pending.registration.as_ref().expect("provisional observer");
        enqueue(
            &self.observer,
            RuntimeAsyncEngineCommandV1::RegisterEventWithProgress {
                event,
                stream,
                event_cell: Arc::clone(&observer.future.cell),
                progress_cell: Arc::clone(&observer.progress.cell),
                response,
            },
        )?;
        Ok(pending)
    }
}
