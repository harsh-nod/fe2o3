//! Move-only generated graph admission and per-node original completion receipts.

use super::*;
use generated_operation::adoption::ActivationErrorV1;

/// An ordinary graph plus fresh reserved generated invocations. Dependencies
/// order execution; they do not rebind a successor's already-frozen DATA inputs.
/// Dropping an unsubmitted request drops tickets, not their parked runtime owners;
/// recover tickets with `take_reserved` to explicitly discard them instead.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeGeneratedGraphRequestV1};
/// fn duplicate(request: RuntimeGeneratedGraphRequestV1<KfdRuntimeBackendV1>) { request.clone(); }
/// ```
pub struct RuntimeGeneratedGraphRequestV1<B: RuntimeBackendV1> {
    request: RuntimeGraphRequestV1<B>,
    generated: BTreeMap<CompletionNodeIdV1, RuntimeAsyncReservedTicketV1>,
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeGeneratedGraphRequestV1<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGeneratedGraphRequestV1")
            .field("generated", &self.generated)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct RuntimeGeneratedGraphBindingFailureV1 {
    pub ticket: RuntimeAsyncReservedTicketV1,
    pub error: RuntimeGraphValidationErrorV1,
}
impl fmt::Display for RuntimeGeneratedGraphBindingFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for RuntimeGeneratedGraphBindingFailureV1 {}

impl<B: RuntimeBackendV1> RuntimeGeneratedGraphRequestV1<B> {
    pub fn new(request: RuntimeGraphRequestV1<B>) -> Self {
        Self {
            request,
            generated: BTreeMap::new(),
        }
    }

    /// Rejects without consuming the original ticket. Context/registry identity
    /// and completion support are checked on the owner thread before admission.
    pub fn bind_reserved(
        &mut self,
        node: CompletionNodeIdV1,
        ticket: RuntimeAsyncReservedTicketV1,
    ) -> Result<(), RuntimeGeneratedGraphBindingFailureV1> {
        let valid = self.request.check_node(node).and_then(|()| {
            if self.generated.contains_key(&node) {
                Err(RuntimeGraphValidationErrorV1::DuplicateOperation)
            } else {
                Ok(())
            }
        });
        if let Err(error) = valid {
            return Err(RuntimeGeneratedGraphBindingFailureV1 { ticket, error });
        }
        self.generated.insert(node, ticket);
        Ok(())
    }

    pub fn take_reserved(
        &mut self,
        node: CompletionNodeIdV1,
    ) -> Option<RuntimeAsyncReservedTicketV1> {
        self.generated.remove(&node)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeGeneratedGraphAdmissionErrorV1<E> {
    Engine(RuntimeAsyncEngineCallErrorV1),
    Graph(RuntimeGraphErrorV1<E>),
    Activation(ActivationErrorV1<E>),
}
impl<E: fmt::Display> fmt::Display for RuntimeGeneratedGraphAdmissionErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine(error) => error.fmt(f),
            Self::Graph(error) => error.fmt(f),
            Self::Activation(error) => error.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for RuntimeGeneratedGraphAdmissionErrorV1<E> {}
impl<E> From<RuntimeGraphErrorV1<E>> for RuntimeGeneratedGraphAdmissionErrorV1<E> {
    fn from(error: RuntimeGraphErrorV1<E>) -> Self {
        Self::Graph(error)
    }
}

/// No graph reservation was committed. Every original ticket is returned.
pub struct RuntimeGeneratedGraphSubmissionFailureV1<B: RuntimeBackendV1> {
    pub request: Box<RuntimeGeneratedGraphRequestV1<B>>,
    pub error: RuntimeGeneratedGraphAdmissionErrorV1<B::Error>,
}

pub enum RuntimeGeneratedGraphFailureV1<B: RuntimeBackendV1> {
    Rejected(RuntimeGeneratedGraphSubmissionFailureV1<B>),
    /// Admission committed. This error is not proof of native quiescence.
    Execution(RuntimeGraphErrorV1<B::Error>),
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeGeneratedGraphSubmissionFailureV1<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGeneratedGraphSubmissionFailureV1")
            .field("request", &self.request)
            .field("error", &self.error)
            .finish()
    }
}
impl<B: RuntimeBackendV1> fmt::Debug for RuntimeGeneratedGraphFailureV1<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(failure) => f.debug_tuple("Rejected").field(failure).finish(),
            Self::Execution(error) => f.debug_tuple("Execution").field(error).finish(),
        }
    }
}
impl<B: RuntimeBackendV1> fmt::Display for RuntimeGeneratedGraphSubmissionFailureV1<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl<B: RuntimeBackendV1> Error for RuntimeGeneratedGraphSubmissionFailureV1<B> {}
impl<B: RuntimeBackendV1> fmt::Display for RuntimeGeneratedGraphFailureV1<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(failure) => failure.fmt(f),
            Self::Execution(error) => error.fmt(f),
        }
    }
}
impl<B: RuntimeBackendV1> Error for RuntimeGeneratedGraphFailureV1<B> {}

#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeGeneratedGraphNodeErrorV1<E> {
    Activation(ActivationErrorV1<E>),
    Engine(RuntimeAsyncEngineCallErrorV1),
    Readback(crate::RuntimeGfx942ReadbackErrorV1),
}
impl<E: fmt::Display> fmt::Display for RuntimeGeneratedGraphNodeErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Activation(error) => error.fmt(f),
            Self::Engine(error) => error.fmt(f),
            Self::Readback(error) => error.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for RuntimeGeneratedGraphNodeErrorV1<E> {}

/// Retired graph plus exact, move-only receipts for host typed output extraction.
/// Failed or cancelled generated nodes never produce a successful receipt.
#[derive(Debug)]
pub struct RuntimeGeneratedGraphReportV1<E> {
    pub graph: RuntimeGraphReportV1<E>,
    pub completions: Vec<(CompletionNodeIdV1, RuntimeGeneratedCompletionReceiptV1)>,
    pub errors: Vec<(CompletionNodeIdV1, RuntimeGeneratedGraphNodeErrorV1<E>)>,
}

pub type RuntimeGeneratedGraphResultV1<B> = Result<
    RuntimeGeneratedGraphReportV1<<B as RuntimeBackendV1>::Error>,
    RuntimeGeneratedGraphFailureV1<B>,
>;

#[must_use = "dropping graph observation does not cancel execution"]
/// ```compile_fail,E0599
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeAsyncGeneratedGraphFutureV1};
/// fn duplicate(future: RuntimeAsyncGeneratedGraphFutureV1<KfdRuntimeBackendV1>) { future.clone(); }
/// ```
pub struct RuntimeAsyncGeneratedGraphFutureV1<B: RuntimeBackendV1> {
    future: RuntimeAsyncCommandFutureV1<RuntimeGeneratedGraphResultV1<B>>,
    control: RuntimeGraphControlV1,
}
impl<B: RuntimeBackendV1> RuntimeAsyncGeneratedGraphFutureV1<B> {
    pub fn control(&self) -> RuntimeGraphControlV1 {
        self.control.clone()
    }
}
impl<B: RuntimeBackendV1> Future for RuntimeAsyncGeneratedGraphFutureV1<B> {
    type Output = Result<RuntimeGeneratedGraphResultV1<B>, RuntimeAsyncEngineCallErrorV1>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.future).poll(cx)
    }
}

pub(super) enum GraphReplyV1<B: RuntimeBackendV1> {
    Ordinary(owned::Reply<RuntimeGraphResultV1<B::Error>>),
    Generated(owned::Reply<RuntimeGeneratedGraphResultV1<B>>),
}

impl<B: RuntimeBackendV1> Graph<B> {
    pub(super) fn take_generated_request(&mut self) -> Option<RuntimeGeneratedGraphRequestV1<B>> {
        if !matches!(self.reply, GraphReplyV1::Generated(_)) {
            return None;
        }
        Some(RuntimeGeneratedGraphRequestV1 {
            request: self.request.take()?,
            generated: core::mem::take(&mut self.generated),
        })
    }

    pub(super) fn reject_admission(
        &mut self,
        error: RuntimeGeneratedGraphAdmissionErrorV1<B::Error>,
    ) {
        self.release_slot();
        let request = self.take_generated_request();
        match (&mut self.reply, error) {
            (GraphReplyV1::Generated(reply), error) => {
                reply.complete(Ok(Err(RuntimeGeneratedGraphFailureV1::Rejected(
                    RuntimeGeneratedGraphSubmissionFailureV1 {
                        request: Box::new(request.expect("uncommitted generated graph")),
                        error,
                    },
                ))));
            }
            (
                GraphReplyV1::Ordinary(reply),
                RuntimeGeneratedGraphAdmissionErrorV1::Graph(error),
            ) => {
                reply.complete(Ok(Err(error)));
            }
            _ => unreachable!("generated-only admission error"),
        }
    }
}

impl<B: RuntimeAsyncCopyBackendV1 + RuntimeFlushBackendV1 + 'static>
    RuntimeAsyncProgressHandleV1<B>
{
    /// Uses the existing exclusive graph executor and generated operation registry.
    /// Immediate and ordinary owner-thread rejection return the unchanged request.
    ///
    /// ```no_run
    /// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeAsyncProgressHandleV1,
    ///     RuntimeAsyncReservedTicketV1, RuntimeGeneratedGraphRequestV1,
    ///     RuntimeGeneratedGraphReportV1, RuntimeGraphRequestV1, KfdRuntimeBackendErrorV1};
    /// use fe2o3_runtime::completion::CompletionNodeIdV1;
    /// async fn execute(handle: &RuntimeAsyncProgressHandleV1<KfdRuntimeBackendV1>,
    ///     graph: RuntimeGraphRequestV1<KfdRuntimeBackendV1>, node: CompletionNodeIdV1,
    ///     ticket: RuntimeAsyncReservedTicketV1) -> RuntimeGeneratedGraphReportV1<KfdRuntimeBackendErrorV1> {
    ///     let mut request = RuntimeGeneratedGraphRequestV1::new(graph);
    ///     request.bind_reserved(node, ticket).expect("valid node");
    ///     let future = handle.try_submit_generated_graph_v1(request).expect("queued");
    ///     fn assert_send<T: Send>(_: &T) {}
    ///     assert_send(&future);
    ///     future.await.expect("live engine").expect("retired graph")
    /// }
    /// ```
    pub fn try_submit_generated_graph_v1(
        &self,
        request: RuntimeGeneratedGraphRequestV1<B>,
    ) -> Result<RuntimeAsyncGeneratedGraphFutureV1<B>, RuntimeGeneratedGraphSubmissionFailureV1<B>>
    {
        let failure = |request, error| RuntimeGeneratedGraphSubmissionFailureV1 {
            request: Box::new(request),
            error: RuntimeGeneratedGraphAdmissionErrorV1::Engine(error),
        };
        if self.observer.rejects_async_enqueue() {
            return Err(failure(
                request,
                RuntimeAsyncEngineCallErrorV1::ReentrantCall,
            ));
        }
        let (reply, future) = match owned::Reply::budgeted_pair(&self.observer.reply_budget) {
            Ok(pair) => pair,
            Err(error) => return Err(failure(request, error)),
        };
        if self
            .observer
            .graph_slot
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(failure(
                request,
                RuntimeAsyncEngineCallErrorV1::GraphCapacity,
            ));
        }
        let control = RuntimeGraphControlV1(Arc::new(AtomicBool::new(false)));
        let graph = Graph::new(
            request.request,
            request.generated,
            control.clone(),
            Arc::clone(&self.observer.graph_slot),
            GraphReplyV1::Generated(reply),
        );
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::Graph(Box::new(graph)))
        {
            Ok(()) => Ok(RuntimeAsyncGeneratedGraphFutureV1 { future, control }),
            Err(error) => {
                let (command, error) = match error {
                    TrySendError::Full(command) => {
                        (command, RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
                    }
                    TrySendError::Disconnected(command) => {
                        (command, RuntimeAsyncEngineCallErrorV1::EngineStopped)
                    }
                };
                let RuntimeAsyncEngineCommandV1::Graph(mut graph) = command else {
                    unreachable!()
                };
                let request = graph
                    .take_generated_request()
                    .expect("unsubmitted generated graph");
                Err(failure(request, error))
            }
        }
    }
}
