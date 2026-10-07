//! Early dependency events share the ordinary operation's admission and custody.

use super::*;

#[derive(Debug)]
pub enum RuntimeAsyncOperationEventErrorV1<E> {
    /// Submission returned no usable handle. Inspect the completion result for
    /// its original error; this is NOT evidence of definite non-publication.
    SubmissionUnavailable,
    /// Recording failed after submission. The operation may still execute and
    /// continues to progress while its Context remains live. Never resubmit on
    /// this error alone; observe the independent completion result.
    RecordingFailed(RuntimeErrorV1<E>),
}

impl<E: fmt::Display> fmt::Display for RuntimeAsyncOperationEventErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SubmissionUnavailable => f.write_str("operation has no usable submission handle"),
            Self::RecordingFailed(error) => write!(f, "operation event recording failed: {error}"),
        }
    }
}
impl<E: Error + 'static> Error for RuntimeAsyncOperationEventErrorV1<E> {}

pub type RuntimeAsyncOperationEventFutureV1<E> =
    RuntimeAsyncCommandFutureV1<Result<RuntimeEventIdV1, RuntimeAsyncOperationEventErrorV1<E>>>;
pub(super) type EventReplyV1<E> =
    owned::Reply<Result<RuntimeEventIdV1, RuntimeAsyncOperationEventErrorV1<E>>>;

/// Independent early event and final observation for one tracked operation.
///
/// Event success means only that an exact dependency event was recorded, not
/// GPU completion. The Context retains the event until explicit `release_event`
/// or owned cleanup, even if either future is dropped. Keep it live until every
/// intended consumer has been admitted, not merely enqueued. Each future holds
/// its own reply credit; neither Drop cancels work or retires native custody.
#[must_use = "dropping either observer does not cancel execution or release its event"]
pub struct RuntimeAsyncEventOperationV1<A, E> {
    pub event: RuntimeAsyncOperationEventFutureV1<E>,
    pub operation: RuntimeAsyncTrackedOperationV1<A, E>,
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    fn enqueue_event_operation<A: 'static, P: OperationProgressV1<B, A>>(
        &self,
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
    ) -> Result<RuntimeAsyncEventOperationV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.rejects_async_enqueue() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        // Both credits precede command admission and any Context/native effect.
        let (reply, future) = owned::Reply::budgeted_pair(&self.observer.reply_budget)?;
        let (event_reply, event) = owned::Reply::budgeted_pair(&self.observer.reply_budget)?;
        let control = RuntimeAsyncOperationControlV1::new();
        let factory = factory::OperationFactoryV1::<B, A, P>::new(
            stream,
            submit,
            reply,
            Some(control.clone()),
        )
        .with_event(event_reply);
        self.send_operation_factory(Box::new(factory))?;
        Ok(RuntimeAsyncEventOperationV1 {
            event,
            operation: RuntimeAsyncTrackedOperationV1 { future, control },
        })
    }

    pub(in crate::async_engine) fn enqueue_observed_event_operation<A: 'static>(
        &self,
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
    ) -> Result<RuntimeAsyncEventOperationV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        self.enqueue_event_operation::<A, ObservedProgressV1>(stream, submit)
    }

    /// Like `copy_async_tracked`, with an early exact dependency event. Submission,
    /// event recording and completion polling occur on separate owner advances.
    /// Ordinary stream flushing retains its independent scheduling budget.
    pub fn copy_async_with_event(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<RuntimeAsyncEventOperationV1<RuntimeCopyV1, B::Error>, RuntimeAsyncEngineCallErrorV1>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_observed_event_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.copy_async(stream, source, destination, dependencies)
                })
            }),
        )
    }

    /// Like `directed_peer_copy_tracked`, with an early exact dependency event
    /// that can feed another directed copy before this copy completes. Recording
    /// uses a separate owner advance and adds no automatic stream flush or event
    /// observer. Other progress sources retain their independent budgets.
    pub fn directed_peer_copy_with_event(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<
        RuntimeAsyncEventOperationV1<RuntimeDirectedScalarPeerCopyV1, B::Error>,
        RuntimeAsyncEngineCallErrorV1,
    >
    where
        B: RuntimeDirectedScalarPeerCopyBackendV1,
    {
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_event_operation::<_, DirectedPeerProgressV1>(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.directed_peer_copy_v1(stream, source, destination, dependencies)
                })
            }),
        )
    }
}
