//! Executor-neutral, bounded background observation for runtime events.

use crate::{
    RuntimeBackendV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeErrorV1,
    RuntimeEventIdV1, RuntimeFlushBackendV1, RuntimeStreamIdV1, RuntimeValidationErrorV1,
};
use core::fmt;
use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::io;
use std::marker::PhantomData;
use std::ops::Bound::{Excluded, Unbounded};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Hard upper bound for commands waiting to enter one async engine.
pub const MAX_RUNTIME_ASYNC_COMMANDS_V1: usize = 65_536;
/// Hard upper bound for event futures observed by one async engine.
pub const MAX_RUNTIME_ASYNC_WAITERS_V1: usize = 65_536;
/// Hard upper bound for commands processed before completion polling resumes.
pub const MAX_RUNTIME_ASYNC_COMMANDS_PER_TICK_V1: usize = 1024;
/// Hard upper bound for event completions polled in one scheduling tick.
pub const MAX_RUNTIME_ASYNC_POLLS_PER_TICK_V1: usize = 1024;
/// Longest accepted interval between background completion scans.
pub const MAX_RUNTIME_ASYNC_POLL_INTERVAL_V1: Duration = Duration::from_secs(1);
/// Hard upper bound for streams registered with one async progress engine.
pub const MAX_RUNTIME_ASYNC_PROGRESS_STREAMS_V1: usize = 65_536;
/// Hard upper bound for stream flushes attempted in one scheduling tick.
pub const MAX_RUNTIME_ASYNC_FLUSHES_PER_TICK_V1: usize = 1024;

/// Bounded scheduling configuration for one async observation engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAsyncEngineConfigV1 {
    command_capacity: usize,
    waiter_capacity: usize,
    commands_per_tick: usize,
    polls_per_tick: usize,
    poll_interval: Duration,
}

impl RuntimeAsyncEngineConfigV1 {
    pub fn new(
        command_capacity: usize,
        waiter_capacity: usize,
        commands_per_tick: usize,
        polls_per_tick: usize,
        poll_interval: Duration,
    ) -> Result<Self, RuntimeAsyncEngineConfigErrorV1> {
        if command_capacity == 0 || command_capacity > MAX_RUNTIME_ASYNC_COMMANDS_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::CommandCapacity);
        }
        if waiter_capacity == 0 || waiter_capacity > MAX_RUNTIME_ASYNC_WAITERS_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::WaiterCapacity);
        }
        if commands_per_tick == 0 || commands_per_tick > MAX_RUNTIME_ASYNC_COMMANDS_PER_TICK_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::CommandsPerTick);
        }
        if polls_per_tick == 0 || polls_per_tick > MAX_RUNTIME_ASYNC_POLLS_PER_TICK_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::PollsPerTick);
        }
        if poll_interval.is_zero() || poll_interval > MAX_RUNTIME_ASYNC_POLL_INTERVAL_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::PollInterval);
        }
        Ok(Self {
            command_capacity,
            waiter_capacity,
            commands_per_tick,
            polls_per_tick,
            poll_interval,
        })
    }

    pub const fn command_capacity(self) -> usize {
        self.command_capacity
    }

    pub const fn waiter_capacity(self) -> usize {
        self.waiter_capacity
    }

    pub const fn commands_per_tick(self) -> usize {
        self.commands_per_tick
    }

    pub const fn polls_per_tick(self) -> usize {
        self.polls_per_tick
    }

    pub const fn poll_interval(self) -> Duration {
        self.poll_interval
    }
}

impl Default for RuntimeAsyncEngineConfigV1 {
    fn default() -> Self {
        Self {
            command_capacity: 1024,
            waiter_capacity: 4096,
            commands_per_tick: 64,
            polls_per_tick: 64,
            poll_interval: Duration::from_millis(1),
        }
    }
}

/// Invalid async engine scheduling configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncEngineConfigErrorV1 {
    CommandCapacity,
    WaiterCapacity,
    CommandsPerTick,
    PollsPerTick,
    PollInterval,
}

impl fmt::Display for RuntimeAsyncEngineConfigErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid runtime async engine configuration: {self:?}"
        )
    }
}

impl Error for RuntimeAsyncEngineConfigErrorV1 {}

/// Bounded scheduling configuration for opt-in background stream progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAsyncProgressConfigV1 {
    stream_capacity: usize,
    flushes_per_tick: usize,
}

impl RuntimeAsyncProgressConfigV1 {
    pub fn new(
        stream_capacity: usize,
        flushes_per_tick: usize,
    ) -> Result<Self, RuntimeAsyncProgressConfigErrorV1> {
        if stream_capacity == 0 || stream_capacity > MAX_RUNTIME_ASYNC_PROGRESS_STREAMS_V1 {
            return Err(RuntimeAsyncProgressConfigErrorV1::StreamCapacity);
        }
        if flushes_per_tick == 0 || flushes_per_tick > MAX_RUNTIME_ASYNC_FLUSHES_PER_TICK_V1 {
            return Err(RuntimeAsyncProgressConfigErrorV1::FlushesPerTick);
        }
        Ok(Self {
            stream_capacity,
            flushes_per_tick,
        })
    }

    pub const fn stream_capacity(self) -> usize {
        self.stream_capacity
    }

    pub const fn flushes_per_tick(self) -> usize {
        self.flushes_per_tick
    }
}

impl Default for RuntimeAsyncProgressConfigV1 {
    fn default() -> Self {
        Self {
            stream_capacity: 1024,
            flushes_per_tick: 64,
        }
    }
}

/// Invalid async progress scheduling configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncProgressConfigErrorV1 {
    StreamCapacity,
    FlushesPerTick,
}

impl fmt::Display for RuntimeAsyncProgressConfigErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid runtime async progress configuration: {self:?}"
        )
    }
}

impl Error for RuntimeAsyncProgressConfigErrorV1 {}

/// Failure to start an async engine, retaining the still-owning context.
pub struct RuntimeAsyncEngineSpawnFailureV1<B: RuntimeBackendV1> {
    context: Box<RuntimeContextV1<B>>,
    error: RuntimeAsyncEngineSpawnErrorV1,
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeAsyncEngineSpawnFailureV1<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAsyncEngineSpawnFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<B: RuntimeBackendV1> RuntimeAsyncEngineSpawnFailureV1<B> {
    pub fn context(&self) -> &RuntimeContextV1<B> {
        self.context.as_ref()
    }

    pub const fn error(&self) -> &RuntimeAsyncEngineSpawnErrorV1 {
        &self.error
    }

    pub fn into_parts(self) -> (RuntimeContextV1<B>, RuntimeAsyncEngineSpawnErrorV1) {
        (*self.context, self.error)
    }
}

/// Reason an async engine could not be started.
#[derive(Debug)]
pub enum RuntimeAsyncEngineSpawnErrorV1 {
    InvalidConfig(RuntimeAsyncEngineConfigErrorV1),
    Thread(io::Error),
}

impl fmt::Display for RuntimeAsyncEngineSpawnErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(error) => error.fmt(formatter),
            Self::Thread(error) => write!(formatter, "runtime async engine thread: {error}"),
        }
    }
}

impl Error for RuntimeAsyncEngineSpawnErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfig(error) => Some(error),
            Self::Thread(error) => Some(error),
        }
    }
}

/// Failure to start an async progress engine, retaining the still-owning context.
pub struct RuntimeAsyncProgressEngineSpawnFailureV1<B: RuntimeBackendV1> {
    context: Box<RuntimeContextV1<B>>,
    error: RuntimeAsyncProgressEngineSpawnErrorV1,
}

impl<B: RuntimeBackendV1> fmt::Debug for RuntimeAsyncProgressEngineSpawnFailureV1<B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeAsyncProgressEngineSpawnFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<B: RuntimeBackendV1> RuntimeAsyncProgressEngineSpawnFailureV1<B> {
    pub fn context(&self) -> &RuntimeContextV1<B> {
        self.context.as_ref()
    }

    pub const fn error(&self) -> &RuntimeAsyncProgressEngineSpawnErrorV1 {
        &self.error
    }

    pub fn into_parts(self) -> (RuntimeContextV1<B>, RuntimeAsyncProgressEngineSpawnErrorV1) {
        (*self.context, self.error)
    }
}

/// Reason an async progress engine could not be started.
#[derive(Debug)]
pub enum RuntimeAsyncProgressEngineSpawnErrorV1 {
    InvalidEngineConfig(RuntimeAsyncEngineConfigErrorV1),
    InvalidProgressConfig(RuntimeAsyncProgressConfigErrorV1),
    Thread(io::Error),
}

impl fmt::Display for RuntimeAsyncProgressEngineSpawnErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEngineConfig(error) => error.fmt(formatter),
            Self::InvalidProgressConfig(error) => error.fmt(formatter),
            Self::Thread(error) => {
                write!(formatter, "runtime async progress engine thread: {error}")
            }
        }
    }
}

impl Error for RuntimeAsyncProgressEngineSpawnErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEngineConfig(error) => Some(error),
            Self::InvalidProgressConfig(error) => Some(error),
            Self::Thread(error) => Some(error),
        }
    }
}

/// Failure to enqueue or complete a context command on the engine thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncEngineCallErrorV1 {
    CommandQueueFull,
    EngineStopped,
    ReentrantCall,
    CommandPanicked,
}

impl fmt::Display for RuntimeAsyncEngineCallErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime async engine call failed: {self:?}")
    }
}

impl Error for RuntimeAsyncEngineCallErrorV1 {}

/// Failure to register a unique event future.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncEventRegistrationErrorV1 {
    CommandQueueFull,
    EngineStopped,
    ReentrantCall,
    Capacity,
    DuplicateEvent,
    InvalidEvent(RuntimeValidationErrorV1),
}

impl fmt::Display for RuntimeAsyncEventRegistrationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime event future registration failed: {self:?}"
        )
    }
}

impl Error for RuntimeAsyncEventRegistrationErrorV1 {}

/// Failure to register a unique stream for opt-in background progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncProgressRegistrationErrorV1 {
    CommandQueueFull,
    EngineStopped,
    ReentrantCall,
    Capacity,
    DuplicateStream,
    InvalidStream(RuntimeValidationErrorV1),
}

impl fmt::Display for RuntimeAsyncProgressRegistrationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime async progress registration failed: {self:?}"
        )
    }
}

impl Error for RuntimeAsyncProgressRegistrationErrorV1 {}

/// Failure to atomically register one event and its source stream for progress.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncProgressEventRegistrationErrorV1 {
    CommandQueueFull,
    EngineStopped,
    ReentrantCall,
    EventCapacity,
    ProgressCapacity,
    DuplicateEvent,
    DuplicateStream,
    InvalidEvent(RuntimeValidationErrorV1),
    InvalidStream(RuntimeValidationErrorV1),
    EventStreamMismatch,
}

impl fmt::Display for RuntimeAsyncProgressEventRegistrationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "runtime progress event registration failed: {self:?}"
        )
    }
}

impl Error for RuntimeAsyncProgressEventRegistrationErrorV1 {}

/// Error produced while awaiting one registered runtime event.
#[derive(Debug)]
pub enum RuntimeAsyncEventErrorV1<E> {
    Runtime(RuntimeErrorV1<E>),
    EngineStopped,
}

impl<E: fmt::Display> fmt::Display for RuntimeAsyncEventErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => error.fmt(formatter),
            Self::EngineStopped => formatter.write_str("runtime async engine stopped"),
        }
    }
}

impl<E: Error + 'static> Error for RuntimeAsyncEventErrorV1<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::EngineStopped => None,
        }
    }
}

struct RuntimeAsyncFutureStateV1<E> {
    outcome: Option<Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>>,
    waker: Option<Waker>,
}

struct RuntimeAsyncFutureCellV1<E> {
    abandoned: AtomicBool,
    state: Mutex<RuntimeAsyncFutureStateV1<E>>,
    paired_progress: Option<(RuntimeStreamIdV1, Arc<RuntimeAsyncProgressCellV1<E>>)>,
}

struct RuntimeAsyncWaiterRegistryV1<E> {
    entries: BTreeMap<RuntimeEventIdV1, Arc<RuntimeAsyncFutureCellV1<E>>>,
}

impl<E> RuntimeAsyncWaiterRegistryV1<E> {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<E> Drop for RuntimeAsyncWaiterRegistryV1<E> {
    fn drop(&mut self) {
        for cell in self.entries.values() {
            cell.complete(Err(RuntimeAsyncEventErrorV1::EngineStopped));
        }
    }
}

impl<E> RuntimeAsyncFutureCellV1<E> {
    fn new() -> Self {
        Self {
            abandoned: AtomicBool::new(false),
            state: Mutex::new(RuntimeAsyncFutureStateV1 {
                outcome: None,
                waker: None,
            }),
            paired_progress: None,
        }
    }

    fn with_progress(
        stream: RuntimeStreamIdV1,
        progress: Arc<RuntimeAsyncProgressCellV1<E>>,
    ) -> Self {
        Self {
            abandoned: AtomicBool::new(false),
            state: Mutex::new(RuntimeAsyncFutureStateV1 {
                outcome: None,
                waker: None,
            }),
            paired_progress: Some((stream, progress)),
        }
    }

    fn complete(&self, outcome: Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>) {
        let waker = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if state.outcome.is_some() || self.abandoned.load(Ordering::Acquire) {
                return;
            }
            state.outcome = Some(outcome);
            state.waker.take()
        };
        if let Some(waker) = waker
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| waker.wake()))
        {
            core::mem::forget(payload);
        }
    }
}

/// Executor-neutral future for one exact runtime event.
///
/// Dropping this value abandons only host observation. It never cancels the
/// submission, releases an event, or changes runtime resource custody.
#[must_use = "dropping an event future does not cancel or release its submission"]
pub struct RuntimeEventFutureV1<E> {
    event: RuntimeEventIdV1,
    cell: Arc<RuntimeAsyncFutureCellV1<E>>,
    completed: bool,
}

impl<E> RuntimeEventFutureV1<E> {
    pub const fn event(&self) -> RuntimeEventIdV1 {
        self.event
    }
}

impl<E> Future for RuntimeEventFutureV1<E> {
    type Output = Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        assert!(
            !self.completed,
            "runtime event future polled after completion"
        );
        let outcome = {
            let mut state = self
                .cell
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            if let Some(outcome) = state.outcome.take() {
                Some(outcome)
            } else {
                let replace = state
                    .waker
                    .as_ref()
                    .is_none_or(|waker| !waker.will_wake(context.waker()));
                if replace {
                    state.waker = Some(context.waker().clone());
                }
                None
            }
        };
        if let Some(outcome) = outcome {
            self.completed = true;
            Poll::Ready(outcome)
        } else {
            Poll::Pending
        }
    }
}

impl<E> Drop for RuntimeEventFutureV1<E> {
    fn drop(&mut self) {
        if !self.completed {
            self.cell.abandoned.store(true, Ordering::Release);
            let waker = self
                .cell
                .state
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .waker
                .take();
            drop(waker);
        }
    }
}

/// One event future paired with background progress for its exact source stream.
///
/// The engine admits both registrations in one transaction. Dropping this value
/// abandons event observation and future flush attempts; it never cancels work,
/// releases a resource, or performs a final flush.
#[must_use = "dropping a progress event future does not cancel or release its submission"]
pub struct RuntimeAsyncProgressEventFutureV1<E> {
    future: RuntimeEventFutureV1<E>,
    progress: RuntimeAsyncProgressRegistrationV1<E>,
}

impl<E> RuntimeAsyncProgressEventFutureV1<E> {
    pub const fn event(&self) -> RuntimeEventIdV1 {
        self.future.event()
    }

    pub const fn stream(&self) -> RuntimeStreamIdV1 {
        self.progress.stream()
    }

    pub fn progress_failure_count(&self) -> u64 {
        self.progress.failure_count()
    }

    pub fn take_progress_failure(&self) -> Option<RuntimeErrorV1<E>> {
        self.progress.take_failure()
    }

    pub fn is_progress_stopped(&self) -> bool {
        self.progress.is_stopped()
    }
}

impl<E> Future for RuntimeAsyncProgressEventFutureV1<E> {
    type Output = Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        Pin::new(&mut this.future).poll(context)
    }
}

struct RuntimeAsyncProgressStateV1<E> {
    failure: Option<RuntimeErrorV1<E>>,
    failure_count: u64,
}

struct RuntimeAsyncProgressCellV1<E> {
    abandoned: AtomicBool,
    stopped: AtomicBool,
    state: Mutex<RuntimeAsyncProgressStateV1<E>>,
}

impl<E> RuntimeAsyncProgressCellV1<E> {
    fn new() -> Self {
        Self {
            abandoned: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            state: Mutex::new(RuntimeAsyncProgressStateV1 {
                failure: None,
                failure_count: 0,
            }),
        }
    }

    fn retain_failure(&self, failure: RuntimeErrorV1<E>, terminal: bool) {
        if self.abandoned.load(Ordering::Acquire) {
            return;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        state.failure_count = state.failure_count.saturating_add(1);
        if terminal || state.failure.is_none() {
            state.failure = Some(failure);
        }
    }

    fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }
}

/// Unique lifetime guard for one stream's opt-in background progress.
///
/// Retryable flush failures remain available in one bounded slot until taken;
/// [`failure_count`](Self::failure_count) is a saturating count of observed
/// failures. A terminal failure replaces any retained retryable failure so the
/// exact sealing error remains observable. Dropping the guard only unregisters
/// the stream after any in-flight flush returns. It never cancels work,
/// destroys a stream, releases a resource, or performs a final flush.
#[must_use = "dropping a progress registration stops background flush attempts"]
pub struct RuntimeAsyncProgressRegistrationV1<E> {
    stream: RuntimeStreamIdV1,
    cell: Arc<RuntimeAsyncProgressCellV1<E>>,
}

impl<E> RuntimeAsyncProgressRegistrationV1<E> {
    pub const fn stream(&self) -> RuntimeStreamIdV1 {
        self.stream
    }

    /// Returns the number of observed failures, saturated at [`u64::MAX`].
    pub fn failure_count(&self) -> u64 {
        self.cell
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .failure_count
    }

    pub fn take_failure(&self) -> Option<RuntimeErrorV1<E>> {
        self.cell
            .state
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .failure
            .take()
    }

    /// Reports that the engine stopped or permanently removed this stream.
    pub fn is_stopped(&self) -> bool {
        self.cell.stopped.load(Ordering::Acquire)
    }
}

impl<E> Drop for RuntimeAsyncProgressRegistrationV1<E> {
    fn drop(&mut self) {
        self.cell.abandoned.store(true, Ordering::Release);
    }
}

struct RuntimeAsyncProgressRegistryV1<E> {
    entries: BTreeMap<RuntimeStreamIdV1, Arc<RuntimeAsyncProgressCellV1<E>>>,
}

impl<E> RuntimeAsyncProgressRegistryV1<E> {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<E> Drop for RuntimeAsyncProgressRegistryV1<E> {
    fn drop(&mut self) {
        for cell in self.entries.values() {
            cell.stop();
        }
    }
}

type RuntimeContextCommandV1<B> = Box<dyn FnOnce(&mut RuntimeContextV1<B>) + Send + 'static>;

enum RuntimeAsyncEngineCommandV1<B: RuntimeBackendV1> {
    Context(RuntimeContextCommandV1<B>),
    Register {
        event: RuntimeEventIdV1,
        cell: Arc<RuntimeAsyncFutureCellV1<B::Error>>,
        response: SyncSender<Result<(), RuntimeAsyncEventRegistrationErrorV1>>,
    },
    RegisterProgress {
        stream: RuntimeStreamIdV1,
        cell: Arc<RuntimeAsyncProgressCellV1<B::Error>>,
        response: SyncSender<Result<(), RuntimeAsyncProgressRegistrationErrorV1>>,
    },
    RegisterEventWithProgress {
        event: RuntimeEventIdV1,
        stream: RuntimeStreamIdV1,
        event_cell: Arc<RuntimeAsyncFutureCellV1<B::Error>>,
        progress_cell: Arc<RuntimeAsyncProgressCellV1<B::Error>>,
        response: SyncSender<Result<(), RuntimeAsyncProgressEventRegistrationErrorV1>>,
    },
    Stop,
}

/// Cloneable command and event-registration handle for one async engine.
pub struct RuntimeAsyncEngineHandleV1<B: RuntimeBackendV1 + Send + 'static> {
    sender: SyncSender<RuntimeAsyncEngineCommandV1<B>>,
    worker_thread: Arc<OnceLock<thread::ThreadId>>,
}

impl<B: RuntimeBackendV1 + Send + 'static> Clone for RuntimeAsyncEngineHandleV1<B> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            worker_thread: Arc::clone(&self.worker_thread),
        }
    }
}

impl<B: RuntimeBackendV1 + Send + 'static> RuntimeAsyncEngineHandleV1<B> {
    /// Runs one boundedly enqueued safe context operation on the engine thread.
    ///
    /// The operation is synchronous from the caller's perspective. A panic is
    /// contained and reported, while the context remains owned by the engine.
    /// Long-running operations delay completion scans and should not be used as
    /// a substitute for nonblocking runtime submission.
    pub fn try_with_context<R, F>(&self, operation: F) -> Result<R, RuntimeAsyncEngineCallErrorV1>
    where
        R: Send + 'static,
        F: FnOnce(&mut RuntimeContextV1<B>) -> R + Send + 'static,
    {
        if self.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (response_sender, response_receiver) = sync_channel(1);
        let command = RuntimeAsyncEngineCommandV1::Context(Box::new(move |context| {
            let result = catch_unwind(AssertUnwindSafe(|| operation(context))).map_err(|payload| {
                core::mem::forget(payload);
                RuntimeAsyncEngineCallErrorV1::CommandPanicked
            });
            let _ = response_sender.send(result);
        }));
        match self.sender.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull);
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(RuntimeAsyncEngineCallErrorV1::EngineStopped);
            }
        }
        response_receiver
            .recv()
            .unwrap_or(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    }

    /// Registers one unique event for background nonblocking observation.
    pub fn event_future(
        &self,
        event: RuntimeEventIdV1,
    ) -> Result<RuntimeEventFutureV1<B::Error>, RuntimeAsyncEventRegistrationErrorV1> {
        if self.is_worker_thread() {
            return Err(RuntimeAsyncEventRegistrationErrorV1::ReentrantCall);
        }
        let cell = Arc::new(RuntimeAsyncFutureCellV1::new());
        let (response_sender, response_receiver) = sync_channel(1);
        let command = RuntimeAsyncEngineCommandV1::Register {
            event,
            cell: Arc::clone(&cell),
            response: response_sender,
        };
        match self.sender.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(RuntimeAsyncEventRegistrationErrorV1::CommandQueueFull);
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(RuntimeAsyncEventRegistrationErrorV1::EngineStopped);
            }
        }
        response_receiver
            .recv()
            .unwrap_or(Err(RuntimeAsyncEventRegistrationErrorV1::EngineStopped))?;
        Ok(RuntimeEventFutureV1 {
            event,
            cell,
            completed: false,
        })
    }

    fn is_worker_thread(&self) -> bool {
        self.worker_thread
            .get()
            .is_some_and(|worker| *worker == thread::current().id())
    }
}

/// Cloneable observation and stream-registration handle for an opt-in progress engine.
///
/// Only this handle can register streams for background flushes. Its observer
/// view retains the ordinary engine's observation-only context and event APIs.
pub struct RuntimeAsyncProgressHandleV1<B: RuntimeBackendV1 + Send + 'static> {
    observer: RuntimeAsyncEngineHandleV1<B>,
}

impl<B: RuntimeBackendV1 + Send + 'static> Clone for RuntimeAsyncProgressHandleV1<B> {
    fn clone(&self) -> Self {
        Self {
            observer: self.observer.clone(),
        }
    }
}

impl<B: RuntimeBackendV1 + Send + 'static> RuntimeAsyncProgressHandleV1<B> {
    pub const fn observer(&self) -> &RuntimeAsyncEngineHandleV1<B> {
        &self.observer
    }

    /// Registers one unique live stream for cyclic background flush attempts.
    ///
    /// Registration authorizes the backend scheduling domain selected by this
    /// stream. A backend may publish other dependency-ready work in that same
    /// domain. Retryable failures do not unregister the stream.
    pub fn register_stream(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<RuntimeAsyncProgressRegistrationV1<B::Error>, RuntimeAsyncProgressRegistrationErrorV1>
    {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncProgressRegistrationErrorV1::ReentrantCall);
        }
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let (response_sender, response_receiver) = sync_channel(1);
        let command = RuntimeAsyncEngineCommandV1::RegisterProgress {
            stream,
            cell: Arc::clone(&cell),
            response: response_sender,
        };
        match self.observer.sender.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(RuntimeAsyncProgressRegistrationErrorV1::CommandQueueFull);
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(RuntimeAsyncProgressRegistrationErrorV1::EngineStopped);
            }
        }
        response_receiver
            .recv()
            .unwrap_or(Err(RuntimeAsyncProgressRegistrationErrorV1::EngineStopped))?;
        Ok(RuntimeAsyncProgressRegistrationV1 { stream, cell })
    }

    /// Atomically registers an event waiter and progress for its source stream.
    ///
    /// Event polling runs before stream flushing in every engine tick. This
    /// lets an observed completed native window make its continuation ready for
    /// the same tick's flush without requiring a caller-driven progress call.
    /// A nonterminal polling error resolves the future and retires its paired
    /// progress registration; explicitly register the same event and stream
    /// again to retry observation. Retryable flush errors retain registration.
    pub fn event_future_with_progress(
        &self,
        stream: RuntimeStreamIdV1,
        event: RuntimeEventIdV1,
    ) -> Result<
        RuntimeAsyncProgressEventFutureV1<B::Error>,
        RuntimeAsyncProgressEventRegistrationErrorV1,
    > {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncProgressEventRegistrationErrorV1::ReentrantCall);
        }
        let progress_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let event_cell = Arc::new(RuntimeAsyncFutureCellV1::with_progress(
            stream,
            Arc::clone(&progress_cell),
        ));
        let (response_sender, response_receiver) = sync_channel(1);
        let command = RuntimeAsyncEngineCommandV1::RegisterEventWithProgress {
            event,
            stream,
            event_cell: Arc::clone(&event_cell),
            progress_cell: Arc::clone(&progress_cell),
            response: response_sender,
        };
        match self.observer.sender.try_send(command) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                return Err(RuntimeAsyncProgressEventRegistrationErrorV1::CommandQueueFull);
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err(RuntimeAsyncProgressEventRegistrationErrorV1::EngineStopped);
            }
        }
        response_receiver.recv().unwrap_or(Err(
            RuntimeAsyncProgressEventRegistrationErrorV1::EngineStopped,
        ))?;
        Ok(RuntimeAsyncProgressEventFutureV1 {
            future: RuntimeEventFutureV1 {
                event,
                cell: event_cell,
                completed: false,
            },
            progress: RuntimeAsyncProgressRegistrationV1 {
                stream,
                cell: progress_cell,
            },
        })
    }
}

type RuntimeAsyncFlushDriverV1<B> =
    fn(
        &mut RuntimeContextV1<B>,
        RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<<B as RuntimeBackendV1>::Error>>;

struct RuntimeAsyncProgressModeV1<B: RuntimeBackendV1> {
    config: RuntimeAsyncProgressConfigV1,
    flush_stream: RuntimeAsyncFlushDriverV1<B>,
}

fn flush_stream_v1<B: RuntimeFlushBackendV1>(
    context: &mut RuntimeContextV1<B>,
    stream: RuntimeStreamIdV1,
) -> Result<(), RuntimeErrorV1<B::Error>> {
    context.flush_stream(stream)
}

/// One owned background observer for a runtime context.
///
/// The engine uses exactly one host thread for every registered event. It
/// polls events in stable identity order and processes a bounded number of
/// commands between scans. It observes completion only: callers must still use
/// the backend's declared portable progress operation to publish deferred work.
/// The owner is deliberately `!Send` and `!Sync` so consuming shutdown cannot
/// be moved onto its own worker; cloneable handles are the cross-thread surface.
#[must_use = "async engines own a runtime context until consuming shutdown"]
pub struct RuntimeAsyncEngineV1<B: RuntimeBackendV1 + Send + 'static> {
    sender: Option<SyncSender<RuntimeAsyncEngineCommandV1<B>>>,
    worker: Option<JoinHandle<RuntimeContextV1<B>>>,
    thread_affinity: PhantomData<Rc<()>>,
}

impl<B: RuntimeBackendV1 + Send + 'static> RuntimeAsyncEngineV1<B> {
    pub fn spawn(
        context: RuntimeContextV1<B>,
        config: RuntimeAsyncEngineConfigV1,
    ) -> Result<(Self, RuntimeAsyncEngineHandleV1<B>), RuntimeAsyncEngineSpawnFailureV1<B>> {
        if let Err(error) = RuntimeAsyncEngineConfigV1::new(
            config.command_capacity,
            config.waiter_capacity,
            config.commands_per_tick,
            config.polls_per_tick,
            config.poll_interval,
        ) {
            return Err(RuntimeAsyncEngineSpawnFailureV1 {
                context: Box::new(context),
                error: RuntimeAsyncEngineSpawnErrorV1::InvalidConfig(error),
            });
        }
        let (sender, receiver) = sync_channel(config.command_capacity);
        let context_slot = Arc::new(Mutex::new(Some(context)));
        let worker_slot = Arc::clone(&context_slot);
        let worker_thread = Arc::new(OnceLock::new());
        let worker_thread_slot = Arc::clone(&worker_thread);
        let worker = thread::Builder::new()
            .name("fe2o3-runtime-observer-v1".to_owned())
            .spawn(move || {
                worker_thread_slot
                    .set(thread::current().id())
                    .expect("async engine worker identity is set exactly once");
                let context = worker_slot
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take()
                    .expect("async engine context is taken exactly once");
                run_engine_v1(context, receiver, config, None)
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                let context = context_slot
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take()
                    .expect("failed thread spawn retains the runtime context");
                return Err(RuntimeAsyncEngineSpawnFailureV1 {
                    context: Box::new(context),
                    error: RuntimeAsyncEngineSpawnErrorV1::Thread(error),
                });
            }
        };
        drop(context_slot);
        let handle = RuntimeAsyncEngineHandleV1 {
            sender: sender.clone(),
            worker_thread,
        };
        Ok((
            Self {
                sender: Some(sender),
                worker: Some(worker),
                thread_affinity: PhantomData,
            },
            handle,
        ))
    }

    /// Starts an opt-in engine that observes events and flushes registered streams.
    ///
    /// The backend and its error type must be transferable without unsafe
    /// overrides. Runtime Worker V4 and V5 backends provide that path for KFD;
    /// thread-affine direct KFD owners remain caller-driven.
    pub fn spawn_with_progress(
        context: RuntimeContextV1<B>,
        config: RuntimeAsyncEngineConfigV1,
        progress_config: RuntimeAsyncProgressConfigV1,
    ) -> Result<(Self, RuntimeAsyncProgressHandleV1<B>), RuntimeAsyncProgressEngineSpawnFailureV1<B>>
    where
        B: RuntimeFlushBackendV1,
    {
        if let Err(error) = RuntimeAsyncEngineConfigV1::new(
            config.command_capacity,
            config.waiter_capacity,
            config.commands_per_tick,
            config.polls_per_tick,
            config.poll_interval,
        ) {
            return Err(RuntimeAsyncProgressEngineSpawnFailureV1 {
                context: Box::new(context),
                error: RuntimeAsyncProgressEngineSpawnErrorV1::InvalidEngineConfig(error),
            });
        }
        if let Err(error) = RuntimeAsyncProgressConfigV1::new(
            progress_config.stream_capacity,
            progress_config.flushes_per_tick,
        ) {
            return Err(RuntimeAsyncProgressEngineSpawnFailureV1 {
                context: Box::new(context),
                error: RuntimeAsyncProgressEngineSpawnErrorV1::InvalidProgressConfig(error),
            });
        }
        let (sender, receiver) = sync_channel(config.command_capacity);
        let context_slot = Arc::new(Mutex::new(Some(context)));
        let worker_slot = Arc::clone(&context_slot);
        let worker_thread = Arc::new(OnceLock::new());
        let worker_thread_slot = Arc::clone(&worker_thread);
        let progress = RuntimeAsyncProgressModeV1 {
            config: progress_config,
            flush_stream: flush_stream_v1::<B>,
        };
        let worker = thread::Builder::new()
            .name("fe2o3-runtime-progress-v1".to_owned())
            .spawn(move || {
                worker_thread_slot
                    .set(thread::current().id())
                    .expect("async engine worker identity is set exactly once");
                let context = worker_slot
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take()
                    .expect("async engine context is taken exactly once");
                run_engine_v1(context, receiver, config, Some(progress))
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                let context = context_slot
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .take()
                    .expect("failed thread spawn retains the runtime context");
                return Err(RuntimeAsyncProgressEngineSpawnFailureV1 {
                    context: Box::new(context),
                    error: RuntimeAsyncProgressEngineSpawnErrorV1::Thread(error),
                });
            }
        };
        drop(context_slot);
        let observer = RuntimeAsyncEngineHandleV1 {
            sender: sender.clone(),
            worker_thread,
        };
        Ok((
            Self {
                sender: Some(sender),
                worker: Some(worker),
                thread_affinity: PhantomData,
            },
            RuntimeAsyncProgressHandleV1 { observer },
        ))
    }

    /// Stops observation, wakes pending futures as stopped, and returns the context.
    ///
    /// Stop is an ordered command rather than enqueue-time preemption. If it is
    /// beyond the current command batch, that tick completes its event-poll and
    /// progress-flush phases before Stop is dequeued on the next tick. No final
    /// flush is added after the command is dequeued.
    pub fn into_context(mut self) -> Result<RuntimeContextV1<B>, RuntimeAsyncEngineJoinErrorV1> {
        self.stop_and_join()
    }

    fn stop_and_join(&mut self) -> Result<RuntimeContextV1<B>, RuntimeAsyncEngineJoinErrorV1> {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(RuntimeAsyncEngineCommandV1::Stop);
        }
        self.worker
            .take()
            .ok_or(RuntimeAsyncEngineJoinErrorV1::AlreadyStopped)?
            .join()
            .map_err(|_| RuntimeAsyncEngineJoinErrorV1::WorkerPanicked)
    }
}

impl<B: RuntimeBackendV1 + Send + 'static> Drop for RuntimeAsyncEngineV1<B> {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}

/// Failure to recover the context from its background engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncEngineJoinErrorV1 {
    AlreadyStopped,
    WorkerPanicked,
}

impl fmt::Display for RuntimeAsyncEngineJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime async engine join failed: {self:?}")
    }
}

impl Error for RuntimeAsyncEngineJoinErrorV1 {}

fn run_engine_v1<B: RuntimeBackendV1 + Send + 'static>(
    mut context: RuntimeContextV1<B>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<B>>,
    config: RuntimeAsyncEngineConfigV1,
    progress: Option<RuntimeAsyncProgressModeV1<B>>,
) -> RuntimeContextV1<B> {
    let mut waiters = RuntimeAsyncWaiterRegistryV1::new();
    let mut progress_registry = progress
        .as_ref()
        .map(|_| RuntimeAsyncProgressRegistryV1::new());
    let mut next_event = None;
    let mut next_stream = None;
    let mut stopped = context.is_terminal();
    while !stopped {
        match receiver.recv_timeout(config.poll_interval) {
            Ok(command) => {
                stopped = handle_command_v1(
                    &mut context,
                    &mut waiters.entries,
                    progress_registry.as_mut(),
                    command,
                    config,
                    progress.as_ref().map(|mode| mode.config),
                );
                for _ in 1..config.commands_per_tick {
                    if stopped {
                        break;
                    }
                    match receiver.try_recv() {
                        Ok(command) => {
                            stopped = handle_command_v1(
                                &mut context,
                                &mut waiters.entries,
                                progress_registry.as_mut(),
                                command,
                                config,
                                progress.as_ref().map(|mode| mode.config),
                            );
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            stopped = true;
                            break;
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => stopped = true,
        }
        if !stopped {
            stopped = poll_waiters_v1(
                &mut context,
                &mut waiters.entries,
                progress_registry
                    .as_mut()
                    .map(|registry| &mut registry.entries),
                &mut next_event,
                config.polls_per_tick,
            );
        }
        if !stopped
            && let (Some(mode), Some(registry)) = (progress.as_ref(), progress_registry.as_mut())
        {
            stopped = flush_progress_v1(
                &mut context,
                &mut registry.entries,
                &mut next_stream,
                mode.config.flushes_per_tick,
                mode.flush_stream,
            );
        }
    }
    if let Some(registry) = progress_registry.as_mut() {
        for (_, cell) in core::mem::take(&mut registry.entries) {
            cell.stop();
        }
    }
    for (_, cell) in core::mem::take(&mut waiters.entries) {
        cell.complete(Err(RuntimeAsyncEventErrorV1::EngineStopped));
    }
    context
}

fn handle_command_v1<B: RuntimeBackendV1 + Send + 'static>(
    context: &mut RuntimeContextV1<B>,
    waiters: &mut BTreeMap<RuntimeEventIdV1, Arc<RuntimeAsyncFutureCellV1<B::Error>>>,
    progress: Option<&mut RuntimeAsyncProgressRegistryV1<B::Error>>,
    command: RuntimeAsyncEngineCommandV1<B>,
    config: RuntimeAsyncEngineConfigV1,
    progress_config: Option<RuntimeAsyncProgressConfigV1>,
) -> bool {
    match command {
        RuntimeAsyncEngineCommandV1::Context(command) => {
            command(context);
            context.is_terminal()
        }
        RuntimeAsyncEngineCommandV1::Register {
            event,
            cell,
            response,
        } => {
            if waiters
                .get(&event)
                .is_some_and(|prior| prior.abandoned.load(Ordering::Acquire))
            {
                waiters.remove(&event);
            }
            let result = if waiters.contains_key(&event) {
                Err(RuntimeAsyncEventRegistrationErrorV1::DuplicateEvent)
            } else {
                match context.query_event(event) {
                    Ok(status) if status.is_terminal() => {
                        cell.complete(Ok(status));
                        Ok(())
                    }
                    Ok(RuntimeCompletionStatusV1::Pending) => {
                        if waiters.len() >= config.waiter_capacity {
                            waiters.retain(|_, prior| !prior.abandoned.load(Ordering::Acquire));
                        }
                        if waiters.len() >= config.waiter_capacity {
                            let _ =
                                response.send(Err(RuntimeAsyncEventRegistrationErrorV1::Capacity));
                            return false;
                        }
                        if response.send(Ok(())).is_ok() {
                            waiters.insert(event, cell);
                            return false;
                        }
                        return false;
                    }
                    Ok(_) => unreachable!("all non-pending runtime statuses are terminal"),
                    Err(error) => Err(RuntimeAsyncEventRegistrationErrorV1::InvalidEvent(error)),
                }
            };
            let _ = response.send(result);
            false
        }
        RuntimeAsyncEngineCommandV1::RegisterProgress {
            stream,
            cell,
            response,
        } => {
            let Some(progress) = progress else {
                let _ = response.send(Err(RuntimeAsyncProgressRegistrationErrorV1::EngineStopped));
                return false;
            };
            let capacity = progress_config
                .expect("a progress registry always has progress configuration")
                .stream_capacity;
            if progress
                .entries
                .get(&stream)
                .is_some_and(|prior| prior.abandoned.load(Ordering::Acquire))
                && let Some(prior) = progress.entries.remove(&stream)
            {
                prior.stop();
            }
            let result = if progress.entries.contains_key(&stream) {
                Err(RuntimeAsyncProgressRegistrationErrorV1::DuplicateStream)
            } else if let Err(error) = context.query_stream(stream) {
                Err(RuntimeAsyncProgressRegistrationErrorV1::InvalidStream(
                    error,
                ))
            } else {
                if progress.entries.len() >= capacity {
                    progress.entries.retain(|_, prior| {
                        let retained = !prior.abandoned.load(Ordering::Acquire);
                        if !retained {
                            prior.stop();
                        }
                        retained
                    });
                }
                if progress.entries.len() >= capacity {
                    let _ = response.send(Err(RuntimeAsyncProgressRegistrationErrorV1::Capacity));
                    return false;
                }
                if response.send(Ok(())).is_ok() {
                    progress.entries.insert(stream, cell);
                    return false;
                }
                return false;
            };
            let _ = response.send(result);
            false
        }
        RuntimeAsyncEngineCommandV1::RegisterEventWithProgress {
            event,
            stream,
            event_cell,
            progress_cell,
            response,
        } => {
            let Some(progress) = progress else {
                let _ = response.send(Err(
                    RuntimeAsyncProgressEventRegistrationErrorV1::EngineStopped,
                ));
                return false;
            };
            let progress_capacity = progress_config
                .expect("a progress registry always has progress configuration")
                .stream_capacity;

            if waiters
                .get(&event)
                .is_some_and(|prior| prior.abandoned.load(Ordering::Acquire))
            {
                waiters.remove(&event);
            }
            if progress
                .entries
                .get(&stream)
                .is_some_and(|prior| prior.abandoned.load(Ordering::Acquire))
                && let Some(prior) = progress.entries.remove(&stream)
            {
                prior.stop();
            }
            let result = if waiters.contains_key(&event) {
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateEvent)
            } else if progress.entries.contains_key(&stream) {
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateStream)
            } else if let Err(error) = context.query_stream(stream) {
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::InvalidStream(
                    error,
                ))
            } else {
                match context.event_stream_for_async_progress_v1(event) {
                    Err(error) => Err(RuntimeAsyncProgressEventRegistrationErrorV1::InvalidEvent(
                        error,
                    )),
                    Ok(event_stream) if event_stream != stream => {
                        Err(RuntimeAsyncProgressEventRegistrationErrorV1::EventStreamMismatch)
                    }
                    Ok(_) => match context.query_event(event) {
                        Err(error) => Err(
                            RuntimeAsyncProgressEventRegistrationErrorV1::InvalidEvent(error),
                        ),
                        Ok(status) if status.is_terminal() => {
                            event_cell.complete(Ok(status));
                            progress_cell.stop();
                            Ok(())
                        }
                        Ok(RuntimeCompletionStatusV1::Pending) => {
                            if waiters.len() >= config.waiter_capacity {
                                waiters.retain(|_, prior| !prior.abandoned.load(Ordering::Acquire));
                            }
                            if progress.entries.len() >= progress_capacity {
                                progress.entries.retain(|_, prior| {
                                    let retained = !prior.abandoned.load(Ordering::Acquire);
                                    if !retained {
                                        prior.stop();
                                    }
                                    retained
                                });
                            }
                            if waiters.len() >= config.waiter_capacity {
                                let _ = response.send(Err(
                                    RuntimeAsyncProgressEventRegistrationErrorV1::EventCapacity,
                                ));
                                return false;
                            }
                            if progress.entries.len() >= progress_capacity {
                                let _ = response.send(Err(
                                    RuntimeAsyncProgressEventRegistrationErrorV1::ProgressCapacity,
                                ));
                                return false;
                            }
                            waiters.insert(event, Arc::clone(&event_cell));
                            progress.entries.insert(stream, Arc::clone(&progress_cell));
                            if response.send(Ok(())).is_err() {
                                waiters.remove(&event);
                                progress.entries.remove(&stream);
                                progress_cell.stop();
                            }
                            return false;
                        }
                        Ok(_) => unreachable!("all non-pending runtime statuses are terminal"),
                    },
                }
            };
            let _ = response.send(result);
            false
        }
        RuntimeAsyncEngineCommandV1::Stop => true,
    }
}

fn poll_waiters_v1<B: RuntimeBackendV1 + Send + 'static>(
    context: &mut RuntimeContextV1<B>,
    waiters: &mut BTreeMap<RuntimeEventIdV1, Arc<RuntimeAsyncFutureCellV1<B::Error>>>,
    mut progress: Option<
        &mut BTreeMap<RuntimeStreamIdV1, Arc<RuntimeAsyncProgressCellV1<B::Error>>>,
    >,
    next_event: &mut Option<RuntimeEventIdV1>,
    budget: usize,
) -> bool {
    let mut events = Vec::with_capacity(budget.min(waiters.len()));
    if let Some(start) = *next_event {
        events.extend(waiters.range(start..).map(|(event, _)| *event).take(budget));
        if events.len() < budget {
            events.extend(
                waiters
                    .range(..start)
                    .map(|(event, _)| *event)
                    .take(budget - events.len()),
            );
        }
    } else {
        events.extend(waiters.keys().copied().take(budget));
    }

    for event in events.iter().copied() {
        let Some(cell) = waiters.get(&event) else {
            continue;
        };
        if cell.abandoned.load(Ordering::Acquire) {
            stop_paired_progress_v1(cell, progress.as_deref_mut());
            waiters.remove(&event);
            continue;
        }
        match context.poll_event(event) {
            Ok(RuntimeCompletionStatusV1::Pending) => {}
            Ok(status) => {
                stop_paired_progress_v1(cell, progress.as_deref_mut());
                cell.complete(Ok(status));
                waiters.remove(&event);
            }
            Err(error) => {
                let terminal = runtime_error_is_terminal_v1(&error);
                stop_paired_progress_v1(cell, progress.as_deref_mut());
                cell.complete(Err(RuntimeAsyncEventErrorV1::Runtime(error)));
                waiters.remove(&event);
                if terminal {
                    return true;
                }
            }
        }
    }

    *next_event = events.last().and_then(|last| {
        waiters
            .range((Excluded(*last), Unbounded))
            .next()
            .or_else(|| waiters.first_key_value())
            .map(|(event, _)| *event)
    });
    false
}

fn stop_paired_progress_v1<E>(
    event_cell: &RuntimeAsyncFutureCellV1<E>,
    progress: Option<&mut BTreeMap<RuntimeStreamIdV1, Arc<RuntimeAsyncProgressCellV1<E>>>>,
) {
    let Some((stream, paired_cell)) = event_cell.paired_progress.as_ref() else {
        return;
    };
    paired_cell.stop();
    let Some(progress) = progress else {
        return;
    };
    if progress
        .get(stream)
        .is_some_and(|registered| Arc::ptr_eq(registered, paired_cell))
    {
        progress.remove(stream);
    }
}

fn flush_progress_v1<B: RuntimeBackendV1 + Send + 'static>(
    context: &mut RuntimeContextV1<B>,
    registrations: &mut BTreeMap<RuntimeStreamIdV1, Arc<RuntimeAsyncProgressCellV1<B::Error>>>,
    next_stream: &mut Option<RuntimeStreamIdV1>,
    budget: usize,
    flush_stream: RuntimeAsyncFlushDriverV1<B>,
) -> bool {
    let mut streams = Vec::with_capacity(budget.min(registrations.len()));
    if let Some(start) = *next_stream {
        streams.extend(
            registrations
                .range(start..)
                .map(|(stream, _)| *stream)
                .take(budget),
        );
        if streams.len() < budget {
            streams.extend(
                registrations
                    .range(..start)
                    .map(|(stream, _)| *stream)
                    .take(budget - streams.len()),
            );
        }
    } else {
        streams.extend(registrations.keys().copied().take(budget));
    }

    for stream in streams.iter().copied() {
        let Some(cell) = registrations.get(&stream) else {
            continue;
        };
        if cell.abandoned.load(Ordering::Acquire) {
            cell.stop();
            registrations.remove(&stream);
            continue;
        }
        match context.query_stream(stream) {
            Ok(observation) if observation.is_quiescent() => continue,
            Ok(_) => {}
            Err(error) => {
                cell.retain_failure(RuntimeErrorV1::Validation(error), false);
                cell.stop();
                registrations.remove(&stream);
                continue;
            }
        }
        if let Err(error) = flush_stream(context, stream) {
            let terminal = runtime_error_is_terminal_v1(&error);
            cell.retain_failure(error, terminal);
            if terminal {
                cell.stop();
                return true;
            }
        }
    }

    *next_stream = streams.last().and_then(|last| {
        registrations
            .range((Excluded(*last), Unbounded))
            .next()
            .or_else(|| registrations.first_key_value())
            .map(|(stream, _)| *stream)
    });
    false
}

fn runtime_error_is_terminal_v1<E>(error: &RuntimeErrorV1<E>) -> bool {
    matches!(
        error,
        RuntimeErrorV1::BackendProtocol(_)
            | RuntimeErrorV1::BackendTerminal(_)
            | RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextTerminal)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1, BackendPollV1,
        RuntimeArgumentsV1, RuntimeBackendFailureV1, RuntimeBindingV1, RuntimeCapabilitiesV1,
        RuntimeLaunchGeometryV1, RuntimeMemoryKindV1,
    };
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::task::{Wake, Waker};
    use std::thread::ThreadId;
    use std::time::Instant;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MockError(&'static str);

    impl fmt::Display for MockError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(self.0)
        }
    }

    impl Error for MockError {}

    #[derive(Clone, Copy, Debug)]
    enum MockFlushOutcome {
        Success,
        Rejected(&'static str),
        Quiescent(&'static str),
        Terminal(&'static str),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum MockProgressStepV1 {
        Poll(usize),
        Flush(usize),
    }

    struct MockWindowProgressV1 {
        submission: u64,
        window_packet_counts: VecDeque<usize>,
        published: bool,
        continuation_ready: bool,
    }

    #[derive(Default)]
    struct MockState {
        next: u64,
        statuses: HashMap<u64, BackendPollV1>,
        poll_threads: HashSet<ThreadId>,
        poll_calls: usize,
        poll_failures: VecDeque<RuntimeBackendFailureV1<MockError>>,
        created_streams: Vec<u64>,
        flush_calls: Vec<(u64, ThreadId)>,
        flush_outcomes: VecDeque<MockFlushOutcome>,
        flush_barriers: Option<(Arc<Barrier>, Arc<Barrier>)>,
        window_progress: Option<MockWindowProgressV1>,
        progress_steps: Vec<MockProgressStepV1>,
        release_calls: usize,
        panic_on_poll: bool,
    }

    struct MockBackend {
        state: Arc<Mutex<MockState>>,
    }

    impl MockBackend {
        fn next(&self) -> u64 {
            let mut state = self.state.lock().unwrap();
            state.next += 1;
            state.next
        }
    }

    impl RuntimeBackendV1 for MockBackend {
        type Error = MockError;

        fn enumerate_devices_v1(
            &mut self,
        ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
            Ok(vec![BackendDeviceDescriptionV1 {
                backend_device: 1,
                name: "mock".to_owned(),
                target: "mock".to_owned(),
                global_memory_bytes: 4096,
                capabilities: RuntimeCapabilitiesV1 {
                    typed_async_launch: true,
                    streams: true,
                    events: true,
                    device_memory: true,
                    ..RuntimeCapabilitiesV1::default()
                },
            }])
        }

        fn create_stream_v1(
            &mut self,
            _device: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let stream = self.next();
            self.state.lock().unwrap().created_streams.push(stream);
            Ok(stream)
        }

        fn destroy_stream_v1(
            &mut self,
            _stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            Ok(())
        }

        fn allocate_v1(
            &mut self,
            _device: u64,
            _kind: RuntimeMemoryKindV1,
            _byte_len: u64,
            _alignment: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.next())
        }

        fn release_allocation_v1(
            &mut self,
            _allocation: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            Ok(())
        }

        fn write_allocation_v1(
            &mut self,
            _allocation: u64,
            _byte_offset: u64,
            _bytes: &[u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            Ok(())
        }

        fn read_allocation_v1(
            &mut self,
            _allocation: u64,
            _byte_offset: u64,
            _destination: &mut [u8],
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            Ok(())
        }

        fn load_module_v1(
            &mut self,
            _device: u64,
            _image: &[u8],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.next())
        }

        fn unload_module_v1(
            &mut self,
            _module: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            Ok(())
        }

        fn resolve_kernel_v1(
            &mut self,
            _module: u64,
            _name: &str,
            _signature: [u8; 32],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.next())
        }

        fn submit_v1(
            &mut self,
            _launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let handle = self.next();
            self.state
                .lock()
                .unwrap()
                .statuses
                .insert(handle, BackendPollV1::Pending);
            Ok(handle)
        }

        fn poll_v1(
            &mut self,
            submission: u64,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            let mut state = self.state.lock().unwrap();
            assert!(!state.panic_on_poll, "requested backend poll panic");
            state.poll_threads.insert(thread::current().id());
            state.poll_calls += 1;
            if let Some(failure) = state.poll_failures.pop_front() {
                return Err(failure);
            }
            let mut completed = false;
            if let Some(progress) = state.window_progress.as_mut()
                && progress.submission == submission
                && progress.published
            {
                let packet_count = progress
                    .window_packet_counts
                    .pop_front()
                    .expect("a published mock window remains incomplete");
                progress.published = false;
                completed = progress.window_packet_counts.is_empty();
                progress.continuation_ready = !completed;
                state
                    .progress_steps
                    .push(MockProgressStepV1::Poll(packet_count));
            }
            if completed {
                state.statuses.insert(submission, BackendPollV1::Succeeded);
            }
            Ok(*state.statuses.get(&submission).unwrap())
        }

        fn wait_v1(
            &mut self,
            submission: u64,
            _deadline: Instant,
        ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
            self.poll_v1(submission)
        }

        fn release_submission_v1(
            &mut self,
            submission: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            let mut state = self.state.lock().unwrap();
            state.release_calls += 1;
            state.statuses.remove(&submission);
            Ok(())
        }

        fn record_event_v1(
            &mut self,
            _stream: u64,
            _submission: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Ok(self.next())
        }

        fn release_event_v1(
            &mut self,
            _event: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            Ok(())
        }

        fn peer_copy_v1(
            &mut self,
            _stream: u64,
            _source: BackendMemoryRegionV1,
            _destination: BackendMemoryRegionV1,
            _dependencies: &[u64],
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            Err(RuntimeBackendFailureV1::Rejected(MockError("peer copy")))
        }
    }

    impl RuntimeFlushBackendV1 for MockBackend {
        fn flush_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            let barriers = {
                let mut state = self.state.lock().unwrap();
                state.flush_calls.push((stream, thread::current().id()));
                state.flush_barriers.take()
            };
            if let Some((entered, release)) = barriers {
                entered.wait();
                release.wait();
            }
            let outcome = {
                let mut state = self.state.lock().unwrap();
                let publish_continuation = state
                    .window_progress
                    .as_ref()
                    .is_some_and(|progress| progress.continuation_ready);
                if publish_continuation {
                    let progress = state
                        .window_progress
                        .as_mut()
                        .expect("checked mock window progress");
                    progress.continuation_ready = false;
                    progress.published = true;
                    let packet_count = *progress
                        .window_packet_counts
                        .front()
                        .expect("a continuation has one remaining mock window");
                    state
                        .progress_steps
                        .push(MockProgressStepV1::Flush(packet_count));
                }
                state
                    .flush_outcomes
                    .pop_front()
                    .unwrap_or(MockFlushOutcome::Success)
            };
            match outcome {
                MockFlushOutcome::Success => Ok(()),
                MockFlushOutcome::Rejected(message) => {
                    Err(RuntimeBackendFailureV1::Rejected(MockError(message)))
                }
                MockFlushOutcome::Quiescent(message) => {
                    Err(RuntimeBackendFailureV1::Quiescent(MockError(message)))
                }
                MockFlushOutcome::Terminal(message) => {
                    Err(RuntimeBackendFailureV1::Terminal(MockError(message)))
                }
            }
        }
    }

    struct EmptyArgs;

    impl RuntimeArgumentsV1 for EmptyArgs {
        const SIGNATURE_V1: [u8; 32] = [7; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            Vec::new()
        }

        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            Vec::new()
        }
    }

    struct WakeCounter(AtomicUsize);

    impl Wake for WakeCounter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, AtomicOrdering::SeqCst);
        }
    }

    struct ProgressStopOrderingWake {
        progress: Arc<RuntimeAsyncProgressCellV1<MockError>>,
        woke: AtomicBool,
        observed_stopped: AtomicBool,
    }

    impl Wake for ProgressStopOrderingWake {
        fn wake(self: Arc<Self>) {
            self.observed_stopped.store(
                self.progress.stopped.load(Ordering::Acquire),
                Ordering::Release,
            );
            self.woke.store(true, Ordering::Release);
        }
    }

    struct PanickingWake;

    struct PanickingDropPayload;

    impl Drop for PanickingDropPayload {
        fn drop(&mut self) {
            panic!("requested panic-payload drop panic");
        }
    }

    impl Wake for PanickingWake {
        fn wake(self: Arc<Self>) {
            std::panic::panic_any(PanickingDropPayload);
        }
    }

    fn fixture() -> (
        RuntimeContextV1<MockBackend>,
        Arc<Mutex<MockState>>,
        RuntimeEventIdV1,
        u64,
    ) {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::clone(&state),
        })
        .unwrap();
        let (event, backend_submission) = append_submission(&mut context, &state, 1, "empty");
        (context, state, event, backend_submission)
    }

    fn append_submission(
        context: &mut RuntimeContextV1<MockBackend>,
        state: &Arc<Mutex<MockState>>,
        image: u8,
        name: &str,
    ) -> (RuntimeEventIdV1, u64) {
        let (_, event, backend_submission) =
            append_submission_with_stream(context, state, image, name);
        (event, backend_submission)
    }

    fn append_submission_with_stream(
        context: &mut RuntimeContextV1<MockBackend>,
        state: &Arc<Mutex<MockState>>,
        image: u8,
        name: &str,
    ) -> (RuntimeStreamIdV1, RuntimeEventIdV1, u64) {
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let (event, backend_submission) =
            append_submission_on_stream(context, state, stream, image, name);
        (stream, event, backend_submission)
    }

    fn append_submission_on_stream(
        context: &mut RuntimeContextV1<MockBackend>,
        state: &Arc<Mutex<MockState>>,
        stream: RuntimeStreamIdV1,
        image: u8,
        name: &str,
    ) -> (RuntimeEventIdV1, u64) {
        let device = context.devices()[0].id();
        let module = context.load_module(device, &[image]).unwrap();
        let kernel = context.resolve_kernel::<EmptyArgs>(module, name).unwrap();
        let arguments = EmptyArgs;
        let submission = context
            .launch(
                stream,
                &kernel,
                &arguments,
                RuntimeLaunchGeometryV1 {
                    grid: [1, 1, 1],
                    workgroup: [1, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                &[],
            )
            .unwrap();
        let backend_submission = *state
            .lock()
            .unwrap()
            .statuses
            .keys()
            .max()
            .expect("fixture launch creates one backend submission");
        let event = context.record_event(&submission).unwrap();
        (event, backend_submission)
    }

    fn progress_fixture() -> (
        RuntimeContextV1<MockBackend>,
        Arc<Mutex<MockState>>,
        RuntimeStreamIdV1,
        RuntimeEventIdV1,
        u64,
    ) {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::clone(&state),
        })
        .unwrap();
        let (stream, event, submission) =
            append_submission_with_stream(&mut context, &state, 1, "empty");
        (context, state, stream, event, submission)
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(1);
        while !condition() {
            assert!(Instant::now() < deadline, "condition did not become true");
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn poll_once<E, F>(
        future: &mut F,
        waker: &Waker,
    ) -> Poll<Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>>
    where
        F: Future<Output = Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>> + Unpin,
    {
        let mut context = Context::from_waker(waker);
        Pin::new(future).poll(&mut context)
    }

    fn poll_until_ready<E, F>(
        future: &mut F,
        waker: &Waker,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>
    where
        F: Future<Output = Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>> + Unpin,
    {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match poll_once(future, waker) {
                Poll::Ready(outcome) => return outcome,
                Poll::Pending if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(1));
                }
                Poll::Pending => panic!("runtime event future did not become ready"),
            }
        }
    }

    #[test]
    fn one_background_thread_wakes_a_registered_event_future() {
        let (context, state, event, backend_submission) = fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        let mut future = handle.event_future(event).unwrap();
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&counter));
        assert!(poll_once(&mut future, &waker).is_pending());
        state
            .lock()
            .unwrap()
            .statuses
            .insert(backend_submission, BackendPollV1::Succeeded);
        for _ in 0..100 {
            if counter.0.load(AtomicOrdering::SeqCst) != 0 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_ne!(counter.0.load(AtomicOrdering::SeqCst), 0);
        assert!(matches!(
            poll_once(&mut future, &waker),
            Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
        ));
        assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn dropping_a_future_never_releases_or_cancels_runtime_work() {
        let (context, state, event, backend_submission) = fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        drop(handle.event_future(event).unwrap());
        thread::sleep(Duration::from_millis(5));
        assert_eq!(state.lock().unwrap().release_calls, 0);
        assert_eq!(
            state.lock().unwrap().statuses.get(&backend_submission),
            Some(&BackendPollV1::Pending)
        );
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn duplicate_and_over_capacity_waiters_fail_before_registration() {
        let (mut context, _state, event, _backend_submission) = fixture();
        let state = Arc::clone(&context.backend().state);
        let (second_event, _) = append_submission(&mut context, &state, 2, "second");
        let config = RuntimeAsyncEngineConfigV1::new(8, 1, 2, 2, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
        let future = handle.event_future(event).unwrap();
        assert!(matches!(
            handle.event_future(event),
            Err(RuntimeAsyncEventRegistrationErrorV1::DuplicateEvent)
        ));
        assert!(matches!(
            handle.event_future(second_event),
            Err(RuntimeAsyncEventRegistrationErrorV1::Capacity)
        ));
        drop(future);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn abandoned_waiter_immediately_frees_capacity_for_another_event() {
        let (mut context, state, event, _backend_submission) = fixture();
        let (second_event, _) = append_submission(&mut context, &state, 2, "second");
        let config = RuntimeAsyncEngineConfigV1::new(8, 1, 2, 2, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
        drop(handle.event_future(event).unwrap());
        let _second = handle.event_future(second_event).unwrap();
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn waiter_scan_obeys_its_budget_and_rotates_in_event_order() {
        let (mut context, state, first_event, _first_submission) = fixture();
        let (second_event, _) = append_submission(&mut context, &state, 2, "second");
        let mut waiters = BTreeMap::from([
            (first_event, Arc::new(RuntimeAsyncFutureCellV1::new())),
            (second_event, Arc::new(RuntimeAsyncFutureCellV1::new())),
        ]);
        let mut next_event = None;
        poll_waiters_v1(&mut context, &mut waiters, None, &mut next_event, 1);
        assert_eq!(state.lock().unwrap().poll_calls, 1);
        assert_eq!(next_event, Some(second_event));
        poll_waiters_v1(&mut context, &mut waiters, None, &mut next_event, 1);
        assert_eq!(state.lock().unwrap().poll_calls, 2);
        assert_eq!(next_event, Some(first_event));
    }

    #[test]
    fn terminal_event_does_not_consume_pending_waiter_capacity() {
        let (mut context, state, event, _backend_submission) = fixture();
        let (second_event, second_submission) =
            append_submission(&mut context, &state, 2, "second");
        let config = RuntimeAsyncEngineConfigV1::new(8, 1, 2, 2, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn(context, config).unwrap();
        let _pending = handle.event_future(event).unwrap();
        state
            .lock()
            .unwrap()
            .statuses
            .insert(second_submission, BackendPollV1::Succeeded);
        assert!(matches!(
            handle
                .try_with_context(move |context| context.poll_event(second_event))
                .unwrap(),
            Ok(RuntimeCompletionStatusV1::Succeeded)
        ));
        let mut terminal = handle.event_future(second_event).unwrap();
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_once(&mut terminal, &waker),
            Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
        ));
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn one_engine_observes_out_of_order_completions_independently() {
        let (mut context, state, first_event, first_submission) = fixture();
        let (second_event, second_submission) =
            append_submission(&mut context, &state, 2, "second");
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        let mut first = handle.event_future(first_event).unwrap();
        let mut second = handle.event_future(second_event).unwrap();
        let first_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let second_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let first_waker = Waker::from(Arc::clone(&first_counter));
        let second_waker = Waker::from(Arc::clone(&second_counter));
        assert!(poll_once(&mut first, &first_waker).is_pending());
        assert!(poll_once(&mut second, &second_waker).is_pending());

        state
            .lock()
            .unwrap()
            .statuses
            .insert(second_submission, BackendPollV1::Succeeded);
        for _ in 0..100 {
            if second_counter.0.load(AtomicOrdering::SeqCst) != 0 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(
            poll_once(&mut second, &second_waker),
            Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
        ));
        assert!(poll_once(&mut first, &first_waker).is_pending());

        state
            .lock()
            .unwrap()
            .statuses
            .insert(first_submission, BackendPollV1::Failed { code: 17 });
        for _ in 0..100 {
            if first_counter.0.load(AtomicOrdering::SeqCst) != 0 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert!(matches!(
            poll_once(&mut first, &first_waker),
            Poll::Ready(Ok(RuntimeCompletionStatusV1::Failed(
                crate::RuntimeCompletionFailureV1::BackendCode(17)
            )))
        ));
        assert_eq!(state.lock().unwrap().poll_threads.len(), 1);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn context_command_panic_is_contained_without_stopping_the_engine() {
        let (context, _state, event, _backend_submission) = fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        assert_eq!(
            handle.try_with_context::<(), _>(|_| { std::panic::panic_any(PanickingDropPayload) }),
            Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
        );
        assert_eq!(
            handle
                .try_with_context(move |context| context.query_event(event))
                .unwrap(),
            Ok(RuntimeCompletionStatusV1::Pending)
        );
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn context_command_reentry_is_rejected_without_deadlocking_the_engine() {
        let (context, _state, event, _backend_submission) = fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        let nested_context = handle.clone();
        assert_eq!(
            handle
                .try_with_context(move |_| nested_context.try_with_context(|_| ()))
                .unwrap(),
            Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
        );
        let nested_event = handle.clone();
        assert!(matches!(
            handle
                .try_with_context(move |_| nested_event.event_future(event))
                .unwrap(),
            Err(RuntimeAsyncEventRegistrationErrorV1::ReentrantCall)
        ));
        assert_eq!(handle.try_with_context(|_| 17).unwrap(), 17);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn panicking_executor_waker_is_contained_and_other_waiters_complete() {
        let (mut context, state, first_event, first_submission) = fixture();
        let (second_event, second_submission) =
            append_submission(&mut context, &state, 2, "second");
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        let mut first = handle.event_future(first_event).unwrap();
        let mut second = handle.event_future(second_event).unwrap();
        let panicking_waker = Waker::from(Arc::new(PanickingWake));
        let second_counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let second_waker = Waker::from(Arc::clone(&second_counter));
        assert!(poll_once(&mut first, &panicking_waker).is_pending());
        assert!(poll_once(&mut second, &second_waker).is_pending());
        {
            let mut state = state.lock().unwrap();
            state
                .statuses
                .insert(first_submission, BackendPollV1::Succeeded);
            state
                .statuses
                .insert(second_submission, BackendPollV1::Succeeded);
        }
        for _ in 0..1000 {
            if second_counter.0.load(AtomicOrdering::SeqCst) != 0 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_ne!(second_counter.0.load(AtomicOrdering::SeqCst), 0);
        assert!(matches!(
            poll_until_ready(&mut first, &panicking_waker),
            Ok(RuntimeCompletionStatusV1::Succeeded)
        ));
        assert!(matches!(
            poll_until_ready(&mut second, &second_waker),
            Ok(RuntimeCompletionStatusV1::Succeeded)
        ));
        assert_eq!(handle.try_with_context(|_| 17).unwrap(), 17);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn worker_panic_wakes_registered_futures_as_stopped() {
        let (context, state, event, _backend_submission) = fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        let mut future = handle.event_future(event).unwrap();
        let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
        let waker = Waker::from(Arc::clone(&counter));
        assert!(poll_once(&mut future, &waker).is_pending());
        state.lock().unwrap().panic_on_poll = true;
        for _ in 0..100 {
            if counter.0.load(AtomicOrdering::SeqCst) != 0 {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
        assert_ne!(counter.0.load(AtomicOrdering::SeqCst), 0);
        assert!(matches!(
            poll_once(&mut future, &waker),
            Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
        ));
        drop(handle);
        assert!(matches!(
            engine.into_context(),
            Err(RuntimeAsyncEngineJoinErrorV1::WorkerPanicked)
        ));
    }

    #[test]
    fn consuming_stop_wakes_outstanding_future_without_changing_custody() {
        let (context, state, event, backend_submission) = fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        let mut future = handle.event_future(event).unwrap();
        drop(handle);
        let _context = engine.into_context().unwrap();
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_once(&mut future, &waker),
            Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
        ));
        assert_eq!(state.lock().unwrap().release_calls, 0);
        assert_eq!(
            state.lock().unwrap().statuses.get(&backend_submission),
            Some(&BackendPollV1::Pending)
        );
    }

    #[test]
    fn ordinary_spawn_remains_observation_only() {
        let (context, state, _stream, _event, _submission) = progress_fixture();
        let (engine, handle) =
            RuntimeAsyncEngineV1::spawn(context, RuntimeAsyncEngineConfigV1::default()).unwrap();
        thread::sleep(Duration::from_millis(10));
        assert!(state.lock().unwrap().flush_calls.is_empty());
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn progress_engine_flushes_only_registered_pending_streams() {
        let (mut context, state, first_stream, _event, _submission) = progress_fixture();
        let (second_stream, _, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let backend_streams = state.lock().unwrap().created_streams.clone();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let registration = handle.register_stream(first_stream).unwrap();
        wait_until(|| !state.lock().unwrap().flush_calls.is_empty());
        assert!(
            state
                .lock()
                .unwrap()
                .flush_calls
                .iter()
                .all(|(stream, _)| *stream == backend_streams[0])
        );
        assert_ne!(first_stream, second_stream);
        drop(registration);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn idle_registration_starts_flushing_after_a_later_submission() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::clone(&state),
        })
        .unwrap();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let registration = handle.register_stream(stream).unwrap();
        thread::sleep(Duration::from_millis(10));
        assert!(state.lock().unwrap().flush_calls.is_empty());

        let submission_state = Arc::clone(&state);
        handle
            .observer()
            .try_with_context(move |context| {
                append_submission_on_stream(context, &submission_state, stream, 1, "later")
            })
            .unwrap();
        wait_until(|| !state.lock().unwrap().flush_calls.is_empty());

        drop(registration);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn paired_registration_rolls_back_both_sides_on_capacity_and_duplicates() {
        {
            let (context, _state, stream, event, _submission) = progress_fixture();
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                RuntimeAsyncEngineConfigV1::default(),
                RuntimeAsyncProgressConfigV1::default(),
            )
            .unwrap();
            let event_future = handle.observer().event_future(event).unwrap();
            assert!(matches!(
                handle.event_future_with_progress(stream, event),
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateEvent)
            ));
            let progress = handle.register_stream(stream).unwrap();
            drop(progress);
            drop(event_future);
            drop(handle);
            let _context = engine.into_context().unwrap();
        }

        {
            let (context, _state, stream, event, _submission) = progress_fixture();
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                RuntimeAsyncEngineConfigV1::default(),
                RuntimeAsyncProgressConfigV1::default(),
            )
            .unwrap();
            let progress = handle.register_stream(stream).unwrap();
            assert!(matches!(
                handle.event_future_with_progress(stream, event),
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::DuplicateStream)
            ));
            let event_future = handle.observer().event_future(event).unwrap();
            drop(event_future);
            drop(progress);
            drop(handle);
            let _context = engine.into_context().unwrap();
        }

        {
            let (mut context, state, first_stream, first_event, _submission) = progress_fixture();
            let (second_stream, second_event, _) =
                append_submission_with_stream(&mut context, &state, 2, "second");
            let engine_config =
                RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                engine_config,
                RuntimeAsyncProgressConfigV1::default(),
            )
            .unwrap();
            let first = handle.observer().event_future(first_event).unwrap();
            assert!(matches!(
                handle.event_future_with_progress(second_stream, second_event),
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::EventCapacity)
            ));
            let second_progress = handle.register_stream(second_stream).unwrap();
            assert_ne!(first_stream, second_stream);
            drop(second_progress);
            drop(first);
            drop(handle);
            let _context = engine.into_context().unwrap();
        }

        {
            let (mut context, state, first_stream, _first_event, _submission) = progress_fixture();
            let (second_stream, second_event, _) =
                append_submission_with_stream(&mut context, &state, 2, "second");
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                RuntimeAsyncEngineConfigV1::default(),
                RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
            )
            .unwrap();
            let first = handle.register_stream(first_stream).unwrap();
            assert!(matches!(
                handle.event_future_with_progress(second_stream, second_event),
                Err(RuntimeAsyncProgressEventRegistrationErrorV1::ProgressCapacity)
            ));
            let second_event_future = handle.observer().event_future(second_event).unwrap();
            drop(second_event_future);
            drop(first);
            drop(handle);
            let _context = engine.into_context().unwrap();
        }
    }

    #[test]
    fn paired_registration_rejects_valid_wrong_stream_without_consuming_capacity() {
        let (mut context, state, first_stream, first_event, _submission) = progress_fixture();
        let (second_stream, _second_event, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let engine_config =
            RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            engine_config,
            RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            handle.event_future_with_progress(second_stream, first_event),
            Err(RuntimeAsyncProgressEventRegistrationErrorV1::EventStreamMismatch)
        ));
        let correct = handle
            .event_future_with_progress(first_stream, first_event)
            .unwrap();

        drop(correct);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn paired_progress_polls_63_packet_window_before_flushing_2_packet_continuation() {
        let (context, state, stream, event, submission) = progress_fixture();
        state.lock().unwrap().window_progress = Some(MockWindowProgressV1 {
            submission,
            window_packet_counts: VecDeque::from([63, 2]),
            published: true,
            continuation_ready: false,
        });
        let config =
            RuntimeAsyncEngineConfigV1::new(16, 16, 16, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            config,
            RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
        )
        .unwrap();
        let mut future = handle.event_future_with_progress(stream, event).unwrap();
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_until_ready(&mut future, &waker),
            Ok(RuntimeCompletionStatusV1::Succeeded)
        ));
        assert_eq!(future.progress_failure_count(), 0);
        let state = state.lock().unwrap();
        assert_eq!(
            state.progress_steps,
            [
                MockProgressStepV1::Poll(63),
                MockProgressStepV1::Flush(2),
                MockProgressStepV1::Poll(2)
            ]
        );
        assert_eq!(state.flush_calls.len(), 1);
        drop(state);
        drop(future);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn completed_paired_future_stops_flushing_and_reuses_both_capacities() {
        let (mut context, state, first_stream, first_event, first_submission) = progress_fixture();
        let (second_stream, second_event, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let engine_config =
            RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            engine_config,
            RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
        )
        .unwrap();
        let mut completed = handle
            .event_future_with_progress(first_stream, first_event)
            .unwrap();
        state
            .lock()
            .unwrap()
            .statuses
            .insert(first_submission, BackendPollV1::Succeeded);
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_until_ready(&mut completed, &waker),
            Ok(RuntimeCompletionStatusV1::Succeeded)
        ));
        assert!(completed.is_progress_stopped());
        let flush_count = state.lock().unwrap().flush_calls.len();
        thread::sleep(Duration::from_millis(10));
        assert_eq!(state.lock().unwrap().flush_calls.len(), flush_count);

        let replacement = handle
            .event_future_with_progress(second_stream, second_event)
            .unwrap();
        assert_eq!(replacement.event(), second_event);
        assert_eq!(replacement.stream(), second_stream);
        drop(replacement);
        drop(completed);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn paired_event_poll_error_stops_flushing_and_reuses_both_capacities() {
        let (context, state, stream, event, _submission) = progress_fixture();
        state
            .lock()
            .unwrap()
            .poll_failures
            .push_back(RuntimeBackendFailureV1::Rejected(MockError("poll")));
        let engine_config =
            RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            engine_config,
            RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
        )
        .unwrap();
        let mut completed = handle.event_future_with_progress(stream, event).unwrap();
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_until_ready(&mut completed, &waker),
            Err(RuntimeAsyncEventErrorV1::Runtime(
                RuntimeErrorV1::BackendRejected(MockError("poll"))
            ))
        ));
        assert!(completed.is_progress_stopped());
        assert!(state.lock().unwrap().flush_calls.is_empty());

        let replacement = handle.event_future_with_progress(stream, event).unwrap();
        drop(replacement);
        drop(completed);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn terminal_paired_event_bypasses_unused_registry_capacity_checks() {
        {
            let (mut context, state, _first_stream, first_event, _first_submission) =
                progress_fixture();
            let (second_stream, second_event, second_submission) =
                append_submission_with_stream(&mut context, &state, 2, "second");
            state
                .lock()
                .unwrap()
                .statuses
                .insert(second_submission, BackendPollV1::Succeeded);
            assert_eq!(
                context.poll_event(second_event).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            let engine_config =
                RuntimeAsyncEngineConfigV1::new(16, 1, 16, 1, Duration::from_millis(1)).unwrap();
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                engine_config,
                RuntimeAsyncProgressConfigV1::default(),
            )
            .unwrap();
            let pending = handle.observer().event_future(first_event).unwrap();
            let mut terminal = handle
                .event_future_with_progress(second_stream, second_event)
                .unwrap();
            let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
            assert!(matches!(
                poll_once(&mut terminal, &waker),
                Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
            ));
            assert!(terminal.is_progress_stopped());
            drop(terminal);
            drop(pending);
            drop(handle);
            let _context = engine.into_context().unwrap();
        }

        {
            let (mut context, state, first_stream, _first_event, _first_submission) =
                progress_fixture();
            let (second_stream, second_event, second_submission) =
                append_submission_with_stream(&mut context, &state, 2, "second");
            state
                .lock()
                .unwrap()
                .statuses
                .insert(second_submission, BackendPollV1::Succeeded);
            assert_eq!(
                context.poll_event(second_event).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
                context,
                RuntimeAsyncEngineConfigV1::default(),
                RuntimeAsyncProgressConfigV1::new(1, 1).unwrap(),
            )
            .unwrap();
            let progress = handle.register_stream(first_stream).unwrap();
            let mut terminal = handle
                .event_future_with_progress(second_stream, second_event)
                .unwrap();
            let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
            assert!(matches!(
                poll_once(&mut terminal, &waker),
                Poll::Ready(Ok(RuntimeCompletionStatusV1::Succeeded))
            ));
            assert!(terminal.is_progress_stopped());
            drop(terminal);
            drop(progress);
            drop(handle);
            let _context = engine.into_context().unwrap();
        }
    }

    #[test]
    fn paired_progress_retains_retryable_flush_failure() {
        let (context, state, stream, event, _submission) = progress_fixture();
        state
            .lock()
            .unwrap()
            .flush_outcomes
            .push_back(MockFlushOutcome::Rejected("retry"));
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let future = handle.event_future_with_progress(stream, event).unwrap();
        wait_until(|| future.progress_failure_count() != 0);
        assert!(matches!(
            future.take_progress_failure(),
            Some(RuntimeErrorV1::BackendRejected(MockError("retry")))
        ));
        assert!(!future.is_progress_stopped());
        drop(future);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn dropping_paired_future_frees_both_registries_without_release() {
        let (context, state, stream, event, submission) = progress_fixture();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let future = handle.event_future_with_progress(stream, event).unwrap();
        drop(future);
        let replacement = handle.event_future_with_progress(stream, event).unwrap();
        assert_eq!(replacement.event(), event);
        assert_eq!(replacement.stream(), stream);
        drop(replacement);
        drop(handle);
        let _context = engine.into_context().unwrap();
        let state = state.lock().unwrap();
        assert_eq!(state.release_calls, 0);
        assert_eq!(
            state.statuses.get(&submission),
            Some(&BackendPollV1::Pending)
        );
    }

    #[test]
    fn paired_progress_terminal_failure_seals_future_and_registration() {
        let (context, state, stream, event, _submission) = progress_fixture();
        state
            .lock()
            .unwrap()
            .flush_outcomes
            .push_back(MockFlushOutcome::Terminal("sealed"));
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let mut future = handle.event_future_with_progress(stream, event).unwrap();
        wait_until(|| future.is_progress_stopped());
        assert!(matches!(
            future.take_progress_failure(),
            Some(RuntimeErrorV1::BackendTerminal(MockError("sealed")))
        ));
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_until_ready(&mut future, &waker),
            Err(RuntimeAsyncEventErrorV1::EngineStopped)
        ));
        drop(handle);
        let context = engine.into_context().unwrap();
        assert!(context.is_terminal());
    }

    #[test]
    fn queued_stop_after_paired_registration_performs_no_poll_or_final_flush() {
        let (context, state, stream, event, submission) = progress_fixture();
        let config = RuntimeAsyncEngineConfigV1::default();
        let progress_config = RuntimeAsyncProgressConfigV1::default();
        let (sender, receiver) = sync_channel(config.command_capacity);
        let progress_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let event_cell = Arc::new(RuntimeAsyncFutureCellV1::with_progress(
            stream,
            Arc::clone(&progress_cell),
        ));
        let (response_sender, response_receiver) = sync_channel(1);
        sender
            .try_send(RuntimeAsyncEngineCommandV1::RegisterEventWithProgress {
                event,
                stream,
                event_cell: Arc::clone(&event_cell),
                progress_cell: Arc::clone(&progress_cell),
                response: response_sender,
            })
            .unwrap();
        sender.try_send(RuntimeAsyncEngineCommandV1::Stop).unwrap();
        drop(sender);

        let context = run_engine_v1(
            context,
            receiver,
            config,
            Some(RuntimeAsyncProgressModeV1 {
                config: progress_config,
                flush_stream: flush_stream_v1::<MockBackend>,
            }),
        );
        assert_eq!(response_receiver.recv().unwrap(), Ok(()));
        assert!(progress_cell.stopped.load(Ordering::Acquire));
        let state = state.lock().unwrap();
        assert_eq!(state.poll_calls, 0);
        assert!(state.flush_calls.is_empty());
        assert_eq!(state.release_calls, 0);
        assert_eq!(
            state.statuses.get(&submission),
            Some(&BackendPollV1::Pending)
        );
        drop(state);

        let mut future = RuntimeEventFutureV1 {
            event,
            cell: event_cell,
            completed: false,
        };
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_once(&mut future, &waker),
            Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
        ));
        assert!(!context.is_terminal());
    }

    #[test]
    fn shutdown_wakes_paired_future_only_after_progress_is_stopped() {
        let (context, _state, stream, event, _submission) = progress_fixture();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let mut future = handle.event_future_with_progress(stream, event).unwrap();
        let ordering = Arc::new(ProgressStopOrderingWake {
            progress: Arc::clone(&future.progress.cell),
            woke: AtomicBool::new(false),
            observed_stopped: AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&ordering));
        assert!(poll_once(&mut future, &waker).is_pending());

        drop(handle);
        let _context = engine.into_context().unwrap();

        assert!(ordering.woke.load(Ordering::Acquire));
        assert!(ordering.observed_stopped.load(Ordering::Acquire));
        assert!(matches!(
            poll_once(&mut future, &waker),
            Poll::Ready(Err(RuntimeAsyncEventErrorV1::EngineStopped))
        ));
        assert!(future.is_progress_stopped());
    }

    #[test]
    fn progress_registration_is_unique_bounded_and_context_checked() {
        let (mut context, state, first_stream, _event, _submission) = progress_fixture();
        let (second_stream, _, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let (mut foreign_context, foreign_state, _, _, _) = progress_fixture();
        let foreign_stream = foreign_context
            .create_stream(foreign_context.devices()[0].id())
            .unwrap();
        drop(foreign_state);

        let progress_config = RuntimeAsyncProgressConfigV1::new(1, 1).unwrap();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            progress_config,
        )
        .unwrap();
        let first = handle.register_stream(first_stream).unwrap();
        assert!(matches!(
            handle.register_stream(first_stream),
            Err(RuntimeAsyncProgressRegistrationErrorV1::DuplicateStream)
        ));
        assert!(matches!(
            handle.register_stream(second_stream),
            Err(RuntimeAsyncProgressRegistrationErrorV1::Capacity)
        ));
        assert!(matches!(
            handle.register_stream(foreign_stream),
            Err(RuntimeAsyncProgressRegistrationErrorV1::InvalidStream(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        let nested = handle.clone();
        assert!(matches!(
            handle
                .observer()
                .try_with_context(move |_| nested.register_stream(second_stream))
                .unwrap(),
            Err(RuntimeAsyncProgressRegistrationErrorV1::ReentrantCall)
        ));
        drop(first);
        let second = handle.register_stream(second_stream).unwrap();
        drop(second);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn progress_scan_has_an_independent_budget_and_cyclic_cursor() {
        let (mut context, state, first_stream, _event, _submission) = progress_fixture();
        let (second_stream, _, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let (third_stream, _, _) = append_submission_with_stream(&mut context, &state, 3, "third");
        state.lock().unwrap().flush_outcomes.extend([
            MockFlushOutcome::Rejected("first"),
            MockFlushOutcome::Rejected("second"),
            MockFlushOutcome::Rejected("third"),
        ]);
        let cells = [
            (first_stream, Arc::new(RuntimeAsyncProgressCellV1::new())),
            (second_stream, Arc::new(RuntimeAsyncProgressCellV1::new())),
            (third_stream, Arc::new(RuntimeAsyncProgressCellV1::new())),
        ];
        let mut registrations = BTreeMap::from(cells.clone());
        let mut next_stream = None;
        for _ in 0..3 {
            assert!(!flush_progress_v1(
                &mut context,
                &mut registrations,
                &mut next_stream,
                1,
                flush_stream_v1::<MockBackend>,
            ));
        }
        let state = state.lock().unwrap();
        assert_eq!(
            state
                .flush_calls
                .iter()
                .map(|(stream, _)| *stream)
                .collect::<Vec<_>>(),
            state.created_streams
        );
        assert!(
            cells
                .iter()
                .all(|(_, cell)| { cell.state.lock().unwrap().failure_count == 1 })
        );
    }

    #[test]
    fn progress_cursor_rotates_across_quiescent_abandoned_and_removed_streams() {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::clone(&state),
        })
        .unwrap();
        let device = context.devices()[0].id();

        let first = context.create_stream(device).unwrap();
        append_submission_on_stream(&mut context, &state, first, 1, "first");
        let quiescent = context.create_stream(device).unwrap();
        let abandoned = context.create_stream(device).unwrap();
        append_submission_on_stream(&mut context, &state, abandoned, 2, "abandoned");
        let removed = context.create_stream(device).unwrap();
        let last = context.create_stream(device).unwrap();
        append_submission_on_stream(&mut context, &state, last, 3, "last");
        let backend_streams = state.lock().unwrap().created_streams.clone();
        context.destroy_stream(removed).unwrap();

        let first_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let quiescent_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let abandoned_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let removed_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let last_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let abandoned_registration = RuntimeAsyncProgressRegistrationV1 {
            stream: abandoned,
            cell: Arc::clone(&abandoned_cell),
        };
        let removed_registration = RuntimeAsyncProgressRegistrationV1 {
            stream: removed,
            cell: Arc::clone(&removed_cell),
        };
        let mut registrations = BTreeMap::from([
            (first, first_cell),
            (quiescent, Arc::clone(&quiescent_cell)),
            (abandoned, Arc::clone(&abandoned_cell)),
            (removed, Arc::clone(&removed_cell)),
            (last, last_cell),
        ]);
        drop(abandoned_registration);

        let mut next_stream = None;
        for _ in 0..6 {
            assert!(!flush_progress_v1(
                &mut context,
                &mut registrations,
                &mut next_stream,
                1,
                flush_stream_v1::<MockBackend>,
            ));
        }

        assert_eq!(
            state
                .lock()
                .unwrap()
                .flush_calls
                .iter()
                .map(|(stream, _)| *stream)
                .collect::<Vec<_>>(),
            vec![backend_streams[0], backend_streams[4], backend_streams[0]]
        );
        assert_eq!(next_stream, Some(quiescent));
        assert!(abandoned_cell.stopped.load(Ordering::Acquire));
        assert!(removed_registration.is_stopped());
        assert!(matches!(
            removed_registration.take_failure(),
            Some(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        assert_eq!(removed_registration.failure_count(), 1);
        assert!(!quiescent_cell.stopped.load(Ordering::Acquire));
        assert_eq!(registrations.len(), 3);
    }

    #[test]
    fn retryable_progress_failures_are_retained_without_unregistering() {
        let (mut context, state, stream, _event, _submission) = progress_fixture();
        state.lock().unwrap().flush_outcomes.extend([
            MockFlushOutcome::Rejected("busy"),
            MockFlushOutcome::Quiescent("retry quiescent"),
            MockFlushOutcome::Success,
        ]);
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let registration = RuntimeAsyncProgressRegistrationV1 {
            stream,
            cell: Arc::clone(&cell),
        };
        let mut registrations = BTreeMap::from([(stream, cell)]);
        let mut next_stream = None;
        for _ in 0..3 {
            assert!(!flush_progress_v1(
                &mut context,
                &mut registrations,
                &mut next_stream,
                1,
                flush_stream_v1::<MockBackend>,
            ));
        }
        assert_eq!(registration.failure_count(), 2);
        assert!(matches!(
            registration.take_failure(),
            Some(RuntimeErrorV1::BackendRejected(MockError("busy")))
        ));
        assert!(!registration.is_stopped());
        assert!(registrations.contains_key(&stream));
    }

    #[test]
    fn progress_failure_count_saturates_without_losing_the_retained_failure() {
        let cell = RuntimeAsyncProgressCellV1::new();
        cell.state.lock().unwrap().failure_count = u64::MAX - 1;

        cell.retain_failure(RuntimeErrorV1::BackendRejected(MockError("first")), false);
        assert_eq!(cell.state.lock().unwrap().failure_count, u64::MAX);
        cell.retain_failure(
            RuntimeErrorV1::BackendQuiescent(MockError("discarded")),
            false,
        );
        let mut state = cell.state.lock().unwrap();
        assert_eq!(state.failure_count, u64::MAX);
        assert!(matches!(
            state.failure.take(),
            Some(RuntimeErrorV1::BackendRejected(MockError("first")))
        ));
        drop(state);

        cell.retain_failure(RuntimeErrorV1::BackendTerminal(MockError("terminal")), true);
        let state = cell.state.lock().unwrap();
        assert_eq!(state.failure_count, u64::MAX);
        assert!(matches!(
            state.failure,
            Some(RuntimeErrorV1::BackendTerminal(MockError("terminal")))
        ));
    }

    #[test]
    fn terminal_progress_failure_replaces_a_retained_retryable_failure() {
        let (mut context, state, stream, _event, _submission) = progress_fixture();
        state.lock().unwrap().flush_outcomes.extend([
            MockFlushOutcome::Rejected("busy"),
            MockFlushOutcome::Terminal("terminal"),
        ]);
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let registration = RuntimeAsyncProgressRegistrationV1 {
            stream,
            cell: Arc::clone(&cell),
        };
        let mut registrations = BTreeMap::from([(stream, cell)]);
        let mut next_stream = None;
        assert!(!flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut next_stream,
            1,
            flush_stream_v1::<MockBackend>,
        ));
        assert!(flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut next_stream,
            1,
            flush_stream_v1::<MockBackend>,
        ));
        assert_eq!(registration.failure_count(), 2);
        assert!(matches!(
            registration.take_failure(),
            Some(RuntimeErrorV1::BackendTerminal(MockError("terminal")))
        ));
        assert!(registration.is_stopped());
        assert!(context.is_terminal());
    }

    #[test]
    fn dropping_progress_observers_never_cancels_or_releases_work() {
        let (context, state, stream, event, submission) = progress_fixture();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let registration = handle.register_stream(stream).unwrap();
        drop(handle.observer().event_future(event).unwrap());
        wait_until(|| !state.lock().unwrap().flush_calls.is_empty());
        assert_eq!(state.lock().unwrap().release_calls, 0);
        assert_eq!(
            state.lock().unwrap().statuses.get(&submission),
            Some(&BackendPollV1::Pending)
        );
        drop(registration);
        drop(handle);
        let _context = engine.into_context().unwrap();
        assert_eq!(state.lock().unwrap().release_calls, 0);
    }

    #[test]
    fn dropping_registration_removes_it_without_a_final_flush() {
        let (mut context, state, stream, _event, _submission) = progress_fixture();
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let registration = RuntimeAsyncProgressRegistrationV1 {
            stream,
            cell: Arc::clone(&cell),
        };
        let mut registrations = BTreeMap::from([(stream, cell)]);
        drop(registration);
        assert!(!flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut None,
            1,
            flush_stream_v1::<MockBackend>,
        ));
        assert!(registrations.is_empty());
        assert!(state.lock().unwrap().flush_calls.is_empty());
        assert_eq!(state.lock().unwrap().release_calls, 0);
    }

    #[test]
    fn dropping_registration_during_a_claimed_flush_allows_only_that_flush() {
        let (context, state, stream, _event, _submission) = progress_fixture();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        state.lock().unwrap().flush_barriers = Some((Arc::clone(&entered), Arc::clone(&release)));
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let registration = handle.register_stream(stream).unwrap();

        entered.wait();
        drop(registration);
        release.wait();
        handle.observer().try_with_context(|_| ()).unwrap();
        drop(handle);
        let _context = engine.into_context().unwrap();

        let state = state.lock().unwrap();
        assert_eq!(state.flush_calls.len(), 1);
        assert_eq!(state.release_calls, 0);
    }

    #[test]
    fn queued_stop_with_active_registration_performs_no_final_backend_call() {
        let (context, state, stream, _event, submission) = progress_fixture();
        let config = RuntimeAsyncEngineConfigV1::default();
        let progress_config = RuntimeAsyncProgressConfigV1::default();
        let (sender, receiver) = sync_channel(config.command_capacity);
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let (response_sender, response_receiver) = sync_channel(1);
        sender
            .try_send(RuntimeAsyncEngineCommandV1::RegisterProgress {
                stream,
                cell: Arc::clone(&cell),
                response: response_sender,
            })
            .unwrap();
        sender.try_send(RuntimeAsyncEngineCommandV1::Stop).unwrap();
        drop(sender);

        let context = run_engine_v1(
            context,
            receiver,
            config,
            Some(RuntimeAsyncProgressModeV1 {
                config: progress_config,
                flush_stream: flush_stream_v1::<MockBackend>,
            }),
        );
        assert_eq!(response_receiver.recv().unwrap(), Ok(()));
        assert!(cell.stopped.load(Ordering::Acquire));
        let state = state.lock().unwrap();
        assert!(state.flush_calls.is_empty());
        assert_eq!(state.release_calls, 0);
        assert_eq!(
            state.statuses.get(&submission),
            Some(&BackendPollV1::Pending)
        );
        drop(state);
        assert!(!context.is_terminal());
    }

    #[test]
    fn stop_beyond_the_command_budget_takes_effect_at_the_next_tick_boundary() {
        let (context, state, stream, _event, submission) = progress_fixture();
        let config = RuntimeAsyncEngineConfigV1::new(8, 8, 1, 1, Duration::from_millis(1)).unwrap();
        let progress_config = RuntimeAsyncProgressConfigV1::default();
        let (sender, receiver) = sync_channel(config.command_capacity);
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let (response_sender, response_receiver) = sync_channel(1);
        sender
            .try_send(RuntimeAsyncEngineCommandV1::RegisterProgress {
                stream,
                cell: Arc::clone(&cell),
                response: response_sender,
            })
            .unwrap();
        sender.try_send(RuntimeAsyncEngineCommandV1::Stop).unwrap();
        drop(sender);

        let context = run_engine_v1(
            context,
            receiver,
            config,
            Some(RuntimeAsyncProgressModeV1 {
                config: progress_config,
                flush_stream: flush_stream_v1::<MockBackend>,
            }),
        );

        assert_eq!(response_receiver.recv().unwrap(), Ok(()));
        assert!(cell.stopped.load(Ordering::Acquire));
        let state = state.lock().unwrap();
        assert_eq!(state.flush_calls.len(), 1);
        assert_eq!(state.release_calls, 0);
        assert_eq!(
            state.statuses.get(&submission),
            Some(&BackendPollV1::Pending)
        );
        drop(state);
        assert!(!context.is_terminal());
    }

    #[test]
    fn destroyed_registered_stream_retains_validation_failure_and_stops() {
        let (mut context, _state, stream, _event, _submission) = progress_fixture();
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let registration = RuntimeAsyncProgressRegistrationV1 {
            stream,
            cell: Arc::clone(&cell),
        };
        let mut registrations = BTreeMap::from([(stream, cell)]);
        context.destroy_stream(stream).unwrap();
        assert!(!flush_progress_v1(
            &mut context,
            &mut registrations,
            &mut None,
            1,
            flush_stream_v1::<MockBackend>,
        ));
        assert!(matches!(
            registration.take_failure(),
            Some(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        assert!(registration.is_stopped());
        assert!(registrations.is_empty());
    }

    #[test]
    fn terminal_progress_failure_is_exact_and_seals_all_engine_activity() {
        let (mut context, state, first_stream, event, _submission) = progress_fixture();
        let (second_stream, _, _) =
            append_submission_with_stream(&mut context, &state, 2, "second");
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let mut future = handle.observer().event_future(event).unwrap();
        let first = handle.register_stream(first_stream).unwrap();
        let second = handle.register_stream(second_stream).unwrap();
        state
            .lock()
            .unwrap()
            .flush_outcomes
            .push_back(MockFlushOutcome::Terminal("sealed"));
        wait_until(|| first.is_stopped() && second.is_stopped());
        let failures = [first.take_failure(), second.take_failure()];
        assert_eq!(
            failures
                .iter()
                .filter(|failure| matches!(
                    failure,
                    Some(RuntimeErrorV1::BackendTerminal(MockError("sealed")))
                ))
                .count(),
            1
        );
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_until_ready(&mut future, &waker),
            Err(RuntimeAsyncEventErrorV1::EngineStopped)
        ));
        let calls_after_seal = state.lock().unwrap().flush_calls.len();
        thread::sleep(Duration::from_millis(5));
        assert_eq!(state.lock().unwrap().flush_calls.len(), calls_after_seal);
        drop(handle);
        let context = engine.into_context().unwrap();
        assert!(context.is_terminal());
        assert_eq!(state.lock().unwrap().release_calls, 0);
    }

    #[test]
    fn terminal_event_poll_stops_progress_before_the_flush_phase() {
        let (context, state, stream, event, _submission) = progress_fixture();
        state
            .lock()
            .unwrap()
            .poll_failures
            .push_back(RuntimeBackendFailureV1::Terminal(MockError("poll sealed")));
        let config = RuntimeAsyncEngineConfigV1::default();
        let progress_config = RuntimeAsyncProgressConfigV1::default();
        let (sender, receiver) = sync_channel(config.command_capacity);
        let progress_cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        let (progress_response_sender, progress_response_receiver) = sync_channel(1);
        sender
            .try_send(RuntimeAsyncEngineCommandV1::RegisterProgress {
                stream,
                cell: Arc::clone(&progress_cell),
                response: progress_response_sender,
            })
            .unwrap();
        let future_cell = Arc::new(RuntimeAsyncFutureCellV1::new());
        let (future_response_sender, future_response_receiver) = sync_channel(1);
        sender
            .try_send(RuntimeAsyncEngineCommandV1::Register {
                event,
                cell: Arc::clone(&future_cell),
                response: future_response_sender,
            })
            .unwrap();

        let context = run_engine_v1(
            context,
            receiver,
            config,
            Some(RuntimeAsyncProgressModeV1 {
                config: progress_config,
                flush_stream: flush_stream_v1::<MockBackend>,
            }),
        );
        drop(sender);
        assert_eq!(progress_response_receiver.recv().unwrap(), Ok(()));
        assert_eq!(future_response_receiver.recv().unwrap(), Ok(()));
        assert!(progress_cell.stopped.load(Ordering::Acquire));
        assert!(state.lock().unwrap().flush_calls.is_empty());
        assert!(context.is_terminal());

        let mut future = RuntimeEventFutureV1 {
            event,
            cell: future_cell,
            completed: false,
        };
        let waker = Waker::from(Arc::new(WakeCounter(AtomicUsize::new(0))));
        assert!(matches!(
            poll_once(&mut future, &waker),
            Poll::Ready(Err(RuntimeAsyncEventErrorV1::Runtime(
                RuntimeErrorV1::BackendTerminal(MockError("poll sealed"))
            )))
        ));
    }

    #[test]
    fn event_poll_and_progress_flush_share_one_worker_thread() {
        let (context, state, stream, event, _submission) = progress_fixture();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let _future = handle.observer().event_future(event).unwrap();
        let registration = handle.register_stream(stream).unwrap();
        wait_until(|| {
            let state = state.lock().unwrap();
            !state.poll_threads.is_empty() && !state.flush_calls.is_empty()
        });
        let state_guard = state.lock().unwrap();
        assert_eq!(state_guard.poll_threads.len(), 1);
        assert!(
            state_guard
                .flush_calls
                .iter()
                .all(|(_, worker)| state_guard.poll_threads.contains(worker))
        );
        drop(state_guard);
        drop(registration);
        drop(handle);
        let _context = engine.into_context().unwrap();
    }

    #[test]
    fn progress_handle_and_worker_v4_v5_paths_are_send_compatible() {
        fn require_send<T: Send>() {}
        fn require_send_sync<T: Send + Sync>() {}
        require_send_sync::<RuntimeAsyncProgressHandleV1<MockBackend>>();
        require_send::<crate::RuntimeWorkerBackendV4<crate::RuntimeBinaryCodecV4>>();
        require_send::<crate::RuntimeWorkerBackendV5<crate::RuntimeBinaryCodecV5>>();
    }

    #[test]
    fn invalid_progress_config_retains_the_unstarted_context() {
        let (context, _state, _stream, _event, _submission) = progress_fixture();
        let invalid = RuntimeAsyncProgressConfigV1 {
            stream_capacity: 0,
            ..RuntimeAsyncProgressConfigV1::default()
        };
        let failure = match RuntimeAsyncEngineV1::spawn_with_progress(
            context,
            RuntimeAsyncEngineConfigV1::default(),
            invalid,
        ) {
            Err(failure) => failure,
            Ok(_) => panic!("invalid progress configuration must retain the context"),
        };
        assert!(matches!(
            failure.error(),
            RuntimeAsyncProgressEngineSpawnErrorV1::InvalidProgressConfig(
                RuntimeAsyncProgressConfigErrorV1::StreamCapacity
            )
        ));
        let (_context, _) = failure.into_parts();
    }

    #[test]
    fn invalid_config_retains_the_unstarted_context() {
        let (context, _state, _event, _backend_submission) = fixture();
        let invalid = RuntimeAsyncEngineConfigV1 {
            command_capacity: 0,
            ..RuntimeAsyncEngineConfigV1::default()
        };
        let failure = match RuntimeAsyncEngineV1::spawn(context, invalid) {
            Err(failure) => failure,
            Ok(_) => panic!("invalid configuration must retain the context"),
        };
        assert!(matches!(
            failure.error(),
            RuntimeAsyncEngineSpawnErrorV1::InvalidConfig(
                RuntimeAsyncEngineConfigErrorV1::CommandCapacity
            )
        ));
        let (_context, _) = failure.into_parts();
    }
}
