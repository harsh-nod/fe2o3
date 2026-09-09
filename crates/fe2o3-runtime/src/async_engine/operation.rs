//! Runtime-owned typed operations, independently of observer lifetime.

use super::*;
use crate::{
    RuntimeArgumentsV1, RuntimeAsyncCopyBackendV1, RuntimeCopyV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryRegionV1, RuntimePeerCopyV1, RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use std::collections::{BTreeMap, VecDeque};

/// An exact submission and its final host observation. An error is not GPU
/// completion; the context continues to retain possibly reachable resources.
pub struct RuntimeAsyncOperationResultV1<A, E> {
    /// Absent only when launch returned no usable submission handle.
    pub submission: Option<RuntimeSubmissionV1<A>>,
    pub observation: Result<RuntimeCompletionStatusV1, RuntimeErrorV1<E>>,
    pub rejected_observations: u64,
    pub last_rejected_observation: Option<E>,
}

/// Standard future for runtime-owned submission, progress, and observation.
///
/// Dropping it does not cancel the operation or withdraw its progress. A stopped
/// engine is not evidence of non-publication or quiescence. Completion does not
/// release the submission, allocations, or module; use ordinary context release
/// APIs after inspecting the outcome, or inspect owner shutdown's cleanup report.
pub type RuntimeAsyncOperationFutureV1<A, E> =
    RuntimeAsyncCommandFutureV1<RuntimeAsyncOperationResultV1<A, E>>;

pub(super) trait EngineOperationV1<B: RuntimeBackendV1>: Send {
    fn advance(&mut self, context: &mut RuntimeContextV1<B>) -> bool;
    fn stream(&self) -> RuntimeStreamIdV1;
    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1);
}

pub(super) struct OperationRegistryV1<B: RuntimeBackendV1> {
    entries: VecDeque<Box<dyn EngineOperationV1<B>>>,
    streams: BTreeMap<RuntimeStreamIdV1, usize>,
    flush_cursor: Option<RuntimeStreamIdV1>,
}

impl<B: RuntimeBackendV1> OperationRegistryV1<B> {
    pub(super) fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            streams: BTreeMap::new(),
            flush_cursor: None,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn insert(&mut self, operation: Box<dyn EngineOperationV1<B>>) {
        *self.streams.entry(operation.stream()).or_default() += 1;
        self.entries.push_back(operation);
    }

    fn retire_stream(&mut self, stream: RuntimeStreamIdV1) {
        let count = self
            .streams
            .get_mut(&stream)
            .expect("operation stream is retained");
        *count -= 1;
        if *count == 0 {
            self.streams.remove(&stream);
        }
    }
}

type Submit<B, A> = Box<
    dyn FnOnce(
            &mut RuntimeContextV1<B>,
        )
            -> Result<RuntimeSubmissionV1<A>, RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>
        + Send,
>;

struct Operation<B: RuntimeBackendV1, A> {
    stream: RuntimeStreamIdV1,
    submit: Option<Submit<B, A>>,
    submission: Option<RuntimeSubmissionV1<A>>,
    reply: owned::Reply<RuntimeAsyncOperationResultV1<A, B::Error>>,
    rejected_observations: u64,
    last_rejected_observation: Option<B::Error>,
}

impl<B: RuntimeBackendV1, A> Operation<B, A> {
    fn finish(&mut self, observation: Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>>) {
        self.reply.complete(Ok(RuntimeAsyncOperationResultV1 {
            submission: self.submission.take(),
            observation,
            rejected_observations: self.rejected_observations,
            last_rejected_observation: self.last_rejected_observation.take(),
        }));
    }
}

impl<B: RuntimeBackendV1, A> EngineOperationV1<B> for Operation<B, A> {
    fn advance(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        if let Some(submit) = self.submit.take() {
            match submit(context) {
                Ok(submission) => self.submission = Some(submission),
                Err(error) => {
                    self.finish(Err(error));
                    return true;
                }
            }
            return false;
        }
        let submission = self
            .submission
            .as_mut()
            .expect("accepted operation retains submission");
        if let Err(error) = context.poll(submission) {
            match error {
                RuntimeErrorV1::BackendRejected(error) => {
                    self.rejected_observations = self.rejected_observations.saturating_add(1);
                    self.last_rejected_observation = Some(error);
                    return false;
                }
                error => {
                    self.finish(Err(error));
                    return true;
                }
            }
        }
        match context.query_submission(submission) {
            Ok(RuntimeCompletionStatusV1::Pending) => false,
            Ok(status) => {
                self.finish(Ok(status));
                true
            }
            Err(error) => {
                self.finish(Err(error.into()));
                true
            }
        }
    }

    fn stream(&self) -> RuntimeStreamIdV1 {
        self.stream
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        self.reply.complete(Err(error));
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    fn enqueue_operation<A: 'static>(
        &self,
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
    ) -> Result<RuntimeAsyncOperationFutureV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (reply, future) = owned::Reply::pair();
        let operation = Operation {
            stream,
            submit: Some(submit),
            submission: None,
            reply,
            rejected_observations: 0,
            last_rejected_observation: None,
        };
        match self
            .observer
            .sender
            .try_send(RuntimeAsyncEngineCommandV1::Operation(Box::new(operation)))
        {
            Ok(()) => Ok(future),
            Err(TrySendError::Full(_)) => Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull),
            Err(TrySendError::Disconnected(_)) => Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
        }
    }

    /// Enqueues a typed launch through the existing context admission path.
    ///
    /// Registry capacity is reserved before submission. The engine retains and
    /// advances accepted work even if this future is dropped. At most
    /// `waiter_capacity` operations occupy this independent registry; each tick
    /// advances at most `polls_per_tick` and flushes at most `flushes_per_tick`
    /// distinct operation streams, in addition to the observer registries.
    /// Arguments and kernel ownership remain subject to their existing contracts;
    /// these record bounds do not bound arbitrary user-owned argument bytes.
    pub fn launch<A: RuntimeArgumentsV1>(
        &self,
        stream: RuntimeStreamIdV1,
        kernel: Arc<TypedRuntimeKernelV1<A>>,
        arguments: A,
        geometry: RuntimeLaunchGeometryV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<RuntimeAsyncOperationFutureV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        self.enqueue_operation(
            stream,
            Box::new(move |context| {
                context.launch(stream, &kernel, &arguments, geometry, &dependencies)
            }),
        )
    }

    /// Enqueues a same-device transfer with runtime-owned progress and custody.
    pub fn copy_async(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<RuntimeAsyncOperationFutureV1<RuntimeCopyV1, B::Error>, RuntimeAsyncEngineCallErrorV1>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.enqueue_operation(
            stream,
            Box::new(move |context| context.copy_async(stream, source, destination, &dependencies)),
        )
    }

    /// Enqueues an admitted peer transfer; it does not imply native XGMI routing.
    pub fn peer_copy(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<
        RuntimeAsyncOperationFutureV1<RuntimePeerCopyV1, B::Error>,
        RuntimeAsyncEngineCallErrorV1,
    > {
        self.enqueue_operation(
            stream,
            Box::new(move |context| context.peer_copy(stream, source, destination, &dependencies)),
        )
    }
}

pub(super) fn advance_operations_v1<B: RuntimeBackendV1>(
    context: &mut RuntimeContextV1<B>,
    operations: &mut OperationRegistryV1<B>,
    poll_budget: usize,
    flush_budget: usize,
    flush: RuntimeAsyncFlushDriverV1<B>,
) {
    for _ in 0..poll_budget.min(operations.len()) {
        let mut operation = operations
            .entries
            .pop_front()
            .expect("bounded operation roster");
        // An adapter may have performed a side effect before unwinding. Do not
        // let command panic containment turn that into permission to retry.
        match catch_unwind(AssertUnwindSafe(|| operation.advance(context))) {
            Ok(true) => {
                operations.retire_stream(operation.stream());
            }
            Ok(false) => {
                operations.entries.push_back(operation);
            }
            Err(payload) => {
                core::mem::forget(payload);
                context.quarantine_after_async_command_panic_v1();
                operation.reject(RuntimeAsyncEngineCallErrorV1::CommandPanicked);
                operations.retire_stream(operation.stream());
            }
        }
        if context.is_terminal() {
            return;
        }
    }
    let selected: Vec<_> = match operations.flush_cursor {
        Some(cursor) => operations
            .streams
            .range((Excluded(cursor), Unbounded))
            .chain(operations.streams.range(..=cursor))
            .map(|(stream, _)| *stream)
            .take(flush_budget)
            .collect(),
        None => operations
            .streams
            .keys()
            .copied()
            .take(flush_budget)
            .collect(),
    };
    for stream in selected {
        operations.flush_cursor = Some(stream);
        match catch_unwind(AssertUnwindSafe(|| flush(context, stream))) {
            Ok(Err(error)) if runtime_error_is_terminal_v1(&error) => return,
            Err(payload) => {
                core::mem::forget(payload);
                context.quarantine_after_async_command_panic_v1();
                return;
            }
            _ => {}
        }
    }
}
