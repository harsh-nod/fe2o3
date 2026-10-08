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

mod owned;
pub(crate) use owned::Reply as RuntimeAsyncReplyV1;
pub use owned::*;
mod current_thread;
mod drain;
mod scheduler;
pub use current_thread::*;
pub(crate) use drain::DrainQuiescenceV1;
pub use drain::*;
mod drain_capture;
pub use drain_capture::*;
mod drain_capture_storage;
pub use drain_capture_storage::RuntimeAsyncCapturedBytesV1;
mod generated_operation;
mod operation;
pub use generated_operation::*;
mod registration;
mod reply_budget;
pub use operation::*;
pub use registration::*;
mod snapshot;
pub use snapshot::{RuntimeAsyncLaunchRequestV1, RuntimeAsyncSnapshotErrorV1};
mod operation_control;
pub use operation_control::*;
#[allow(unsafe_code)] // Authenticates exact runtime observations for CompletionAuthorityV1.
mod graph;
pub use graph::*;

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
/// Hard upper bound for stream-progress attempts in one scheduling lane per tick.
pub const MAX_RUNTIME_ASYNC_FLUSHES_PER_TICK_V1: usize = 1024;
/// Maximum retained standalone request payload budget, excluding native resources.
pub const MAX_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1: usize = 1024 * 1024 * 1024;
pub const DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1: usize = 16 * 1024 * 1024;
pub const MAX_RUNTIME_ASYNC_REPLIES_V1: usize = 65_536;
pub const DEFAULT_RUNTIME_ASYNC_REPLIES_V1: usize = 16_384;
/// Maximum single-range owned coherent capture extent; disabled by default.
pub const MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_BYTES_V1: usize =
    drain_capture_storage::MAX_CAPTURE_BYTES_V1;
/// Maximum concatenated extent of one owned coherent capture group.
pub const MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1: usize =
    drain_capture_storage::MAX_CAPTURE_GROUP_BYTES_V1;
/// Maximum ordered source ranges in one coherent capture group.
pub const MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_RANGES_V1: usize = 16;

/// Bounded scheduling configuration for one async observation engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAsyncEngineConfigV1 {
    command_capacity: usize,
    waiter_capacity: usize,
    commands_per_tick: usize,
    polls_per_tick: usize,
    poll_interval: Duration,
    snapshot_byte_capacity: usize,
    reply_capacity: usize,
    drain_capture_byte_capacity: usize,
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
            snapshot_byte_capacity: DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1,
            reply_capacity: DEFAULT_RUNTIME_ASYNC_REPLIES_V1,
            drain_capture_byte_capacity: 0,
        })
    }

    /// Bounds frozen launch payloads and compact standalone dependency lists.
    /// Does not bound legacy argument objects, generic callbacks, replies or GPU memory.
    pub fn with_snapshot_byte_capacity(
        mut self,
        capacity: usize,
    ) -> Result<Self, RuntimeAsyncEngineConfigErrorV1> {
        if capacity == 0 || capacity > MAX_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::SnapshotByteCapacity);
        }
        self.snapshot_byte_capacity = capacity;
        Ok(self)
    }

    pub const fn snapshot_byte_capacity(self) -> usize {
        self.snapshot_byte_capacity
    }

    /// Bounds command/operation/graph reply cells through final disposal, even
    /// after completion. This is a record count, not arbitrary result bytes.
    /// Event/progress registrations have separate bounds; drain has one slot.
    pub fn with_reply_capacity(
        mut self,
        capacity: usize,
    ) -> Result<Self, RuntimeAsyncEngineConfigErrorV1> {
        if capacity == 0 || capacity > MAX_RUNTIME_ASYNC_REPLIES_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::ReplyCapacity);
        }
        self.reply_capacity = capacity;
        Ok(self)
    }

    pub const fn reply_capacity(self) -> usize {
        self.reply_capacity
    }

    /// Opts into one owned coherent drain capture. Zero disables capture.
    /// This bounds its slice payload, not native backing, allocator overhead,
    /// arbitrary application allocations, wakers or other generic results.
    pub fn with_drain_capture_byte_capacity(
        mut self,
        capacity: usize,
    ) -> Result<Self, RuntimeAsyncEngineConfigErrorV1> {
        if capacity > MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_BYTES_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::DrainCaptureByteCapacity);
        }
        self.drain_capture_byte_capacity = capacity;
        Ok(self)
    }

    pub const fn drain_capture_byte_capacity(self) -> usize {
        self.drain_capture_byte_capacity
    }

    /// Opts into one capture group with a larger aggregate slice-byte budget.
    /// Single and group capture share one account and one owner record. This
    /// replaces, rather than adds to, the single-capture budget; zero disables
    /// both. Single-range admission still has its independent 64 MiB limit.
    /// Native backing, allocator overhead and caller-owned metadata are excluded.
    pub fn with_drain_capture_group_byte_capacity(
        mut self,
        capacity: usize,
    ) -> Result<Self, RuntimeAsyncEngineConfigErrorV1> {
        if capacity > MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1 {
            return Err(RuntimeAsyncEngineConfigErrorV1::DrainCaptureByteCapacity);
        }
        self.drain_capture_byte_capacity = capacity;
        Ok(self)
    }

    fn capture_budget_v1(
        self,
    ) -> Result<
        Option<drain_capture_storage::CaptureBudgetV1>,
        fe2o3_resource_accounting::ResourceCreditErrorV1,
    > {
        if self.drain_capture_byte_capacity == 0 {
            Ok(None)
        } else {
            drain_capture_storage::CaptureBudgetV1::new(self.drain_capture_byte_capacity).map(Some)
        }
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
            snapshot_byte_capacity: DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1,
            reply_capacity: DEFAULT_RUNTIME_ASYNC_REPLIES_V1,
            drain_capture_byte_capacity: 0,
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
    SnapshotByteCapacity,
    ReplyCapacity,
    DrainCaptureByteCapacity,
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

    /// Maximum stream-progress attempts per scheduling lane and tick. The
    /// legacy name is retained; normal drivers use `progress_stream_v1`, whose
    /// default delegates to full flush. Graph execution still uses strict flush.
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
    CaptureBudget(fe2o3_resource_accounting::ResourceCreditErrorV1),
    Thread(io::Error),
}

impl fmt::Display for RuntimeAsyncEngineSpawnErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(error) => error.fmt(formatter),
            Self::CaptureBudget(error) => error.fmt(formatter),
            Self::Thread(error) => write!(formatter, "runtime async engine thread: {error}"),
        }
    }
}

impl Error for RuntimeAsyncEngineSpawnErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidConfig(error) => Some(error),
            Self::CaptureBudget(error) => Some(error),
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
    CaptureBudget(fe2o3_resource_accounting::ResourceCreditErrorV1),
    Thread(io::Error),
}

impl fmt::Display for RuntimeAsyncProgressEngineSpawnErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEngineConfig(error) => error.fmt(formatter),
            Self::InvalidProgressConfig(error) => error.fmt(formatter),
            Self::CaptureBudget(error) => error.fmt(formatter),
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
            Self::CaptureBudget(error) => Some(error),
            Self::Thread(error) => Some(error),
        }
    }
}

/// Failure to enqueue or complete a context command on the engine thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncEngineCallErrorV1 {
    CommandQueueFull,
    OperationCapacity,
    InvalidPreparedTicket,
    /// This operation never entered context submission. Not GPU completion.
    CancelledBeforeSubmission,
    /// Adopted DATA was retired before packet publication. Not successful output.
    CancelledBeforePublication,
    /// No successful result: classified rejection and original-owner settlement.
    RejectedBeforePublication,
    /// Original no-VM reset owner retained; this operation never activated DATA.
    DeviceUnavailableBeforeActivation {
        device_uid: u64,
    },
    EngineStopped,
    ReentrantCall,
    CommandPanicked,
    GraphCapacity,
    SnapshotCapacity,
    ReplyCapacity,
    InvalidSnapshot(RuntimeAsyncSnapshotErrorV1),
    CaptureFailed(crate::RuntimeHostCaptureErrorV1),
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
/// abandons event observation and future progress attempts; it never cancels
/// work, releases a resource, or performs a final progress attempt.
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
/// Retryable progress failures remain available in one bounded slot until taken;
/// [`failure_count`](Self::failure_count) is a saturating count of observed
/// failures. A terminal failure replaces any retained retryable failure so the
/// exact sealing error remains observable. Dropping the guard only unregisters
/// the stream after any in-flight progress attempt returns. It never cancels
/// work, destroys a stream, releases a resource, or performs a final attempt.
#[must_use = "dropping a progress registration stops background progress attempts"]
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
    Operation(Box<dyn operation::EngineOperationFactoryV1<B>>),
    DiscardPrepared {
        ticket: RuntimeAsyncPreparedTicketV1,
        reply: owned::Reply<()>,
    },
    ReservePrepared(generated_operation::ReserveCommandV1),
    ActivateReserved(generated_operation::adoption::ActivateCommandV1<B>),
    DiscardReserved {
        ticket: RuntimeAsyncReservedTicketV1,
        reply: owned::Reply<()>,
    },
    Graph(Box<dyn graph::EngineGraphV1<B>>),
    Register {
        event: RuntimeEventIdV1,
        cell: Arc<RuntimeAsyncFutureCellV1<B::Error>>,
        response: registration::RegistrationResponseV1<RuntimeAsyncEventRegistrationErrorV1>,
    },
    RegisterProgress {
        stream: RuntimeStreamIdV1,
        cell: Arc<RuntimeAsyncProgressCellV1<B::Error>>,
        response: registration::RegistrationResponseV1<RuntimeAsyncProgressRegistrationErrorV1>,
    },
    RegisterEventWithProgress {
        event: RuntimeEventIdV1,
        stream: RuntimeStreamIdV1,
        event_cell: Arc<RuntimeAsyncFutureCellV1<B::Error>>,
        progress_cell: Arc<RuntimeAsyncProgressCellV1<B::Error>>,
        response:
            registration::RegistrationResponseV1<RuntimeAsyncProgressEventRegistrationErrorV1>,
    },
    Stop,
}

/// Cloneable command and event-registration handle for one async engine.
pub struct RuntimeAsyncEngineHandleV1<B: RuntimeBackendV1 + 'static> {
    context_generation: u64,
    capture_budget: Option<drain_capture_storage::CaptureBudgetV1>,
    reply_budget: Arc<reply_budget::ReplyBudgetV1>,
    admission: Arc<drain::AdmissionV1>,
    sender: SyncSender<RuntimeAsyncEngineCommandV1<B>>,
    worker_thread: Arc<OnceLock<thread::ThreadId>>,
    local_active: Option<Arc<AtomicBool>>,
    quarantine_command_panics: bool,
    graph_slot: Arc<AtomicBool>,
    snapshot_budget: Arc<snapshot::SnapshotBudgetV1>,
}

impl<B: RuntimeBackendV1 + 'static> Clone for RuntimeAsyncEngineHandleV1<B> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            context_generation: self.context_generation,
            capture_budget: self.capture_budget.clone(),
            admission: Arc::clone(&self.admission),
            reply_budget: Arc::clone(&self.reply_budget),
            worker_thread: Arc::clone(&self.worker_thread),
            local_active: self.local_active.clone(),
            quarantine_command_panics: self.quarantine_command_panics,
            graph_slot: Arc::clone(&self.graph_slot),
            snapshot_budget: Arc::clone(&self.snapshot_budget),
        }
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncEngineHandleV1<B> {
    /// Counts retained async command/operation/graph reply cells, not GPU resources.
    pub fn reply_cells_in_use(&self) -> usize {
        self.reply_budget.used()
    }
    /// Descriptive retained standalone payload bytes, not native resource usage.
    pub fn snapshot_bytes_in_use(&self) -> usize {
        self.snapshot_budget.used()
    }

    /// Owned capture bytes, including results retained after reply extraction.
    /// This observation grants no source, completion or disposal authority.
    pub fn drain_capture_bytes_in_use(&self) -> usize {
        self.capture_budget
            .as_ref()
            .map_or(0, |budget| budget.used_bytes())
    }

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
        let quarantine_command_panics = self.quarantine_command_panics;
        let command = RuntimeAsyncEngineCommandV1::Context(Box::new(move |context| {
            let result = catch_unwind(AssertUnwindSafe(|| operation(context))).map_err(|payload| {
                core::mem::forget(payload);
                if quarantine_command_panics {
                    context.quarantine_after_async_command_panic_v1();
                }
                RuntimeAsyncEngineCallErrorV1::CommandPanicked
            });
            let _ = response_sender.send(result);
        }));
        match self.try_send_command(command) {
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
    /// This method waits for admission; [`Self::enqueue_event_registration`]
    /// provides a bounded nonblocking acknowledgment instead.
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
            response: response_sender.into(),
        };
        match self.try_send_command(command) {
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

    fn rejects_async_enqueue(&self) -> bool {
        self.is_worker_thread()
            && self
                .local_active
                .as_ref()
                .is_none_or(|active| active.load(Ordering::Acquire))
    }

    fn is_worker_thread(&self) -> bool {
        self.worker_thread
            .get()
            .is_some_and(|worker| *worker == thread::current().id())
    }
}

/// Cloneable observation and stream-registration handle for an opt-in progress engine.
///
/// Only this handle can register streams for background progress. Its observer
/// view retains the ordinary engine's observation-only context and event APIs.
pub struct RuntimeAsyncProgressHandleV1<B: RuntimeBackendV1 + 'static> {
    observer: RuntimeAsyncEngineHandleV1<B>,
}

impl<B: RuntimeBackendV1 + 'static> Clone for RuntimeAsyncProgressHandleV1<B> {
    fn clone(&self) -> Self {
        Self {
            observer: self.observer.clone(),
        }
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    pub const fn observer(&self) -> &RuntimeAsyncEngineHandleV1<B> {
        &self.observer
    }

    /// Registers one unique live stream for cyclic background progress attempts.
    ///
    /// Registration authorizes the backend scheduling domain selected by this
    /// stream. A backend may publish other dependency-ready work in that same
    /// domain. Each call uses `progress_stream_v1`; success can leave ready work
    /// unpublished. Its default delegates to full flush; the backend documents
    /// its work bound. Retryable failures do not unregister the stream.
    /// Use [`Self::enqueue_stream_registration`] for nonblocking admission.
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
            response: response_sender.into(),
        };
        match self.observer.try_send_command(command) {
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
    /// Event polling runs before stream progress in every engine tick. This
    /// lets an observed completed native window make its continuation ready for
    /// the same tick's progress attempt, without promising full publication.
    /// A nonterminal polling error resolves the future and retires its paired
    /// progress registration; explicitly register the same event and stream
    /// again to retry observation. Retryable progress errors retain registration.
    /// Use [`Self::enqueue_event_registration_with_progress`] for nonblocking admission.
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
            response: response_sender.into(),
        };
        match self.observer.try_send_command(command) {
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
    context.progress_stream_v1(stream)
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
    admission: Arc<drain::AdmissionV1>,
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
        )
        .and_then(|validated| validated.with_snapshot_byte_capacity(config.snapshot_byte_capacity))
        .and_then(|validated| validated.with_reply_capacity(config.reply_capacity))
        .and_then(|validated| {
            validated.with_drain_capture_group_byte_capacity(config.drain_capture_byte_capacity)
        }) {
            return Err(RuntimeAsyncEngineSpawnFailureV1 {
                context: Box::new(context),
                error: RuntimeAsyncEngineSpawnErrorV1::InvalidConfig(error),
            });
        }
        let capture_budget = match config.capture_budget_v1() {
            Ok(budget) => budget,
            Err(error) => {
                return Err(RuntimeAsyncEngineSpawnFailureV1 {
                    context: Box::new(context),
                    error: RuntimeAsyncEngineSpawnErrorV1::CaptureBudget(error),
                });
            }
        };
        let context_generation = context.capture_context_generation_v1();
        let (sender, receiver) = sync_channel(config.command_capacity);
        let admission = drain::AdmissionV1::new();
        let worker_admission = Arc::clone(&admission);
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
                run_engine_v1(context, receiver, config, None, worker_admission)
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
            context_generation,
            capture_budget,
            reply_budget: reply_budget::ReplyBudgetV1::new(config.reply_capacity),
            admission: Arc::clone(&admission),
            snapshot_budget: snapshot::SnapshotBudgetV1::new(config.snapshot_byte_capacity),
            graph_slot: Arc::new(AtomicBool::new(false)),
            sender: sender.clone(),
            worker_thread,
            local_active: None,
            quarantine_command_panics: false,
        };
        Ok((
            Self {
                admission,
                sender: Some(sender),
                worker: Some(worker),
                thread_affinity: PhantomData,
            },
            handle,
        ))
    }

    /// Starts an opt-in engine that observes events and progresses registered streams.
    ///
    /// Each selected stream receives one backend-defined progress attempt, not a
    /// promise of full publication. Legacy backends default to explicit flush;
    /// neither a tick nor an individual backend call has a generic hard time bound.
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
        )
        .and_then(|validated| validated.with_snapshot_byte_capacity(config.snapshot_byte_capacity))
        .and_then(|validated| validated.with_reply_capacity(config.reply_capacity))
        .and_then(|validated| {
            validated.with_drain_capture_group_byte_capacity(config.drain_capture_byte_capacity)
        }) {
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
        let capture_budget = match config.capture_budget_v1() {
            Ok(budget) => budget,
            Err(error) => {
                return Err(RuntimeAsyncProgressEngineSpawnFailureV1 {
                    context: Box::new(context),
                    error: RuntimeAsyncProgressEngineSpawnErrorV1::CaptureBudget(error),
                });
            }
        };
        let context_generation = context.capture_context_generation_v1();
        let (sender, receiver) = sync_channel(config.command_capacity);
        let admission = drain::AdmissionV1::new();
        let worker_admission = Arc::clone(&admission);
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
                run_engine_v1(context, receiver, config, Some(progress), worker_admission)
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
            context_generation,
            capture_budget,
            reply_budget: reply_budget::ReplyBudgetV1::new(config.reply_capacity),
            admission: Arc::clone(&admission),
            snapshot_budget: snapshot::SnapshotBudgetV1::new(config.snapshot_byte_capacity),
            graph_slot: Arc::new(AtomicBool::new(false)),
            sender: sender.clone(),
            worker_thread,
            local_active: None,
            quarantine_command_panics: false,
        };
        Ok((
            Self {
                admission,
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
    /// stream-progress phases before Stop is dequeued on the next tick. No final
    /// progress attempt is added after the command is dequeued.
    pub fn into_context(mut self) -> Result<RuntimeContextV1<B>, RuntimeAsyncEngineJoinErrorV1> {
        self.stop_and_join()
    }

    fn stop_and_join(&mut self) -> Result<RuntimeContextV1<B>, RuntimeAsyncEngineJoinErrorV1> {
        self.admission.close();
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

fn run_engine_v1<B: RuntimeBackendV1 + 'static>(
    mut context: RuntimeContextV1<B>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<B>>,
    config: RuntimeAsyncEngineConfigV1,
    progress: Option<RuntimeAsyncProgressModeV1<B>>,
    admission: Arc<drain::AdmissionV1>,
) -> RuntimeContextV1<B> {
    let mut operations = operation::OperationRegistryV1::new(config.waiter_capacity, false);
    run_engine_context_v1(
        &mut context,
        &mut operations,
        receiver,
        config,
        progress,
        admission,
    );
    context
}

fn run_engine_context_v1<B: RuntimeBackendV1 + 'static>(
    context: &mut RuntimeContextV1<B>,
    operations: &mut operation::OperationRegistryV1<B>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<B>>,
    config: RuntimeAsyncEngineConfigV1,
    progress: Option<RuntimeAsyncProgressModeV1<B>>,
    admission: Arc<drain::AdmissionV1>,
) {
    let mut scheduler =
        scheduler::SchedulerV1::new(admission, progress.is_some(), context.is_terminal());
    while !scheduler.stopped {
        scheduler.tick(
            context,
            operations,
            &receiver,
            config,
            progress.as_ref(),
            config.poll_interval,
        );
    }
    scheduler.finish(context, operations);
}

#[allow(clippy::too_many_arguments)] // Independently bounded observer and operation registries.
fn handle_command_v1<B: RuntimeBackendV1 + 'static>(
    context: &mut RuntimeContextV1<B>,
    waiters: &mut BTreeMap<RuntimeEventIdV1, Arc<RuntimeAsyncFutureCellV1<B::Error>>>,
    operations: &mut operation::OperationRegistryV1<B>,
    graph: &mut Option<Box<dyn graph::EngineGraphV1<B>>>,
    progress: Option<&mut RuntimeAsyncProgressRegistryV1<B::Error>>,
    command: RuntimeAsyncEngineCommandV1<B>,
    config: RuntimeAsyncEngineConfigV1,
    progress_config: Option<RuntimeAsyncProgressConfigV1>,
) -> bool {
    match command {
        RuntimeAsyncEngineCommandV1::Graph(mut incoming) => {
            if progress_config.is_none()
                || graph.is_some()
                || operations.active_len() != 0
                || !waiters.is_empty()
                || progress
                    .as_ref()
                    .is_some_and(|registry| !registry.entries.is_empty())
            {
                incoming.reject(RuntimeGraphErrorV1::Busy);
            } else {
                match catch_unwind(AssertUnwindSafe(|| incoming.admit(context, operations))) {
                    Ok(true) => *graph = Some(incoming),
                    Ok(false) => {}
                    Err(payload) => {
                        core::mem::forget(payload);
                        context.quarantine_after_async_command_panic_v1();
                    }
                }
            }
            context.is_terminal()
        }
        RuntimeAsyncEngineCommandV1::Operation(mut factory) => {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                if progress_config.is_none() || !operations.accepts_factory(factory.as_ref()) {
                    factory.reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
                } else if !fe2o3_runtime_model::r61_operation_registry_accepts_v1(
                    operations.len(),
                    config.waiter_capacity,
                ) {
                    factory.reject(RuntimeAsyncEngineCallErrorV1::OperationCapacity);
                } else {
                    operations.insert(factory.materialize());
                }
            })) {
                core::mem::forget(payload);
                context.quarantine_after_async_command_panic_v1();
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                    factory.reject(RuntimeAsyncEngineCallErrorV1::CommandPanicked);
                })) {
                    core::mem::forget(payload);
                }
            }
            context.is_terminal()
        }
        RuntimeAsyncEngineCommandV1::ReservePrepared(mut command) => {
            command.run(context, operations);
            context.is_terminal()
        }
        RuntimeAsyncEngineCommandV1::ActivateReserved(mut command) => {
            if graph.is_some() {
                command.reject_reserved_context();
            } else {
                command.run(context, operations);
            }
            context.is_terminal()
        }
        RuntimeAsyncEngineCommandV1::DiscardReserved { ticket, mut reply } => {
            let result = if context.is_terminal() {
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            } else if ticket.key.context_generation != context.capture_context_generation_v1() {
                Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
            } else {
                match catch_unwind(AssertUnwindSafe(|| {
                    operations.discard_reserved(&ticket.key)
                })) {
                    Ok(true) => Ok(()),
                    Ok(false) => Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket),
                    Err(payload) => {
                        core::mem::forget(payload);
                        context.quarantine_after_async_command_panic_v1();
                        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
                    }
                }
            };
            reply.complete(result);
            context.is_terminal()
        }
        RuntimeAsyncEngineCommandV1::DiscardPrepared { ticket, mut reply } => {
            let result = if context.is_terminal() {
                Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
            } else if ticket.key.context_generation != context.capture_context_generation_v1() {
                Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
            } else {
                match catch_unwind(AssertUnwindSafe(|| {
                    operations.discard_prepared(&ticket.key)
                })) {
                    Ok(true) => Ok(()),
                    Ok(false) => Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket),
                    Err(payload) => {
                        core::mem::forget(payload);
                        context.quarantine_after_async_command_panic_v1();
                        Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
                    }
                }
            };
            reply.complete(result);
            context.is_terminal()
        }
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
                        if response.is_nonblocking() && cell.abandoned.load(Ordering::Acquire) {
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
                if response.is_nonblocking() && cell.abandoned.load(Ordering::Acquire) {
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
                            if response.is_nonblocking()
                                && (event_cell.abandoned.load(Ordering::Acquire)
                                    || progress_cell.abandoned.load(Ordering::Acquire))
                            {
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

fn poll_waiters_v1<B: RuntimeBackendV1 + 'static>(
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

fn flush_progress_v1<B: RuntimeBackendV1 + 'static>(
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
    mod configuration;
    mod paired_progress;
    mod stream_progress;
    mod waiter_lifecycle;

    use super::*;

    mod owned_tests;
    mod progress_spi_tests;
    pub(super) fn scheduler_fixture() -> (RuntimeContextV1<impl RuntimeBackendV1>, RuntimeStreamIdV1)
    {
        let mut context = RuntimeContextV1::open(MockBackend {
            state: Arc::new(Mutex::new(MockState::default())),
        })
        .unwrap();
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        (context, stream)
    }
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
        peer_devices: bool,
        directed_routes: HashMap<
            u64,
            (
                crate::BackendDirectedPeerRouteV1,
                Vec<crate::BackendDirectedPeerDependencyV1>,
            ),
        >,
        directed_calls: Vec<(&'static str, u64)>,
        directed_panic: bool,
        peer_segment_issues: Vec<(u64, Vec<crate::RuntimePeerCopySegmentV1>, Vec<u64>)>,
        adoption_ready_calls: usize,
        adoption_ready_mode: u8,
        adoption_retire_calls: usize,
        adoption_retire_mode: u8,
        adoption_order: Vec<&'static str>,
        adoption_retire_modes: HashMap<RuntimeStreamIdV1, u8>,
        adoption_retire_attempts: Vec<(RuntimeStreamIdV1, ThreadId)>,
        adoption_completed: Vec<(RuntimeStreamIdV1, ThreadId)>,
        adoption_payload_drops: Vec<(Option<RuntimeStreamIdV1>, ThreadId)>,
        next: u64,
        statuses: HashMap<u64, BackendPollV1>,
        poll_threads: HashSet<ThreadId>,
        poll_calls: usize,
        poll_failures: VecDeque<RuntimeBackendFailureV1<MockError>>,
        created_streams: Vec<u64>,
        flush_calls: Vec<(u64, ThreadId)>,
        override_progress: bool,
        complete_on_progress: bool,
        progress_calls: Vec<(u64, ThreadId)>,
        progress_outcomes: VecDeque<MockFlushOutcome>,
        flush_outcomes: VecDeque<MockFlushOutcome>,
        flush_barriers: Option<(Arc<Barrier>, Arc<Barrier>)>,
        window_progress: Option<MockWindowProgressV1>,
        progress_steps: Vec<MockProgressStepV1>,
        release_calls: usize,
        panic_on_poll: bool,
        panic_on_submit: bool,
        issues: Vec<(u64, u64, Vec<u8>, Vec<crate::BackendBindingV1>)>,
        submission_dependencies: HashMap<u64, Vec<u64>>,
        event_sources: HashMap<u64, u64>,
        event_record_calls: usize,
        event_release_calls: usize,
        event_record_failures: VecDeque<RuntimeBackendFailureV1<MockError>>,
        panic_on_event_record: bool,
        event_record_override: Option<u64>,
        complete_on_flush: bool,
        copy_issues: Vec<(u64, BackendMemoryRegionV1, BackendMemoryRegionV1, Vec<u64>)>,
        submit_failures: VecDeque<RuntimeBackendFailureV1<MockError>>,
        release_failures: VecDeque<RuntimeBackendFailureV1<MockError>>,
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
            let peer_devices = self.state.lock().unwrap().peer_devices;
            Ok((1..=if peer_devices { 2 } else { 1 })
                .map(|backend_device| BackendDeviceDescriptionV1 {
                    backend_device,
                    name: "mock".to_owned(),
                    target: "mock".to_owned(),
                    global_memory_bytes: 4096,
                    capabilities: RuntimeCapabilitiesV1 {
                        typed_async_launch: true,
                        streams: true,
                        events: true,
                        device_memory: true,
                        host_visible_memory: true,
                        peer_copy: peer_devices,
                        multi_device: peer_devices,
                        ..RuntimeCapabilitiesV1::default()
                    },
                })
                .collect())
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
            launch: BackendLaunchV1<'_>,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let panics = self.state.lock().unwrap().panic_on_submit;
            assert!(!panics, "requested backend submit panic");
            if let Some(error) = self.state.lock().unwrap().submit_failures.pop_front() {
                return Err(error);
            }
            let handle = self.next();
            // Observing an issue must also expose its initial polling status.
            let mut state = self.state.lock().unwrap();
            state
                .submission_dependencies
                .insert(handle, launch.dependencies.to_vec());
            state.issues.push((
                launch.stream,
                handle,
                launch.explicit_kernarg.to_vec(),
                launch.bindings.to_vec(),
            ));
            state.statuses.insert(handle, BackendPollV1::Pending);
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
            if state.directed_routes.contains_key(&submission) {
                state.directed_calls.push(("poll", submission));
            }
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
            if let Some(error) = state.release_failures.pop_front() {
                return Err(error);
            }
            state.statuses.remove(&submission);
            Ok(())
        }

        fn record_event_v1(
            &mut self,
            _stream: u64,
            submission: u64,
        ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
            let panic = {
                let mut state = self.state.lock().unwrap();
                state.event_record_calls += 1;
                if let Some(error) = state.event_record_failures.pop_front() {
                    return Err(error);
                }
                state.panic_on_event_record
            };
            assert!(!panic, "event record panic");
            let next = self.next();
            let event = self
                .state
                .lock()
                .unwrap()
                .event_record_override
                .unwrap_or(next);
            self.state
                .lock()
                .unwrap()
                .event_sources
                .insert(event, submission);
            Ok(event)
        }

        fn release_event_v1(
            &mut self,
            _event: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            self.state.lock().unwrap().event_release_calls += 1;
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
        fn progress_stream_v1(
            &mut self,
            stream: u64,
        ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
            if !self.state.lock().unwrap().override_progress {
                return self.flush_stream_v1(stream);
            }
            let mut state = self.state.lock().unwrap();
            state.progress_calls.push((stream, thread::current().id()));
            if state.complete_on_progress {
                let ready: Vec<_> = state
                    .issues
                    .iter()
                    .filter_map(|(owner, id, _, _)| {
                        (*owner == stream
                            && state.submission_dependencies[id].iter().all(|event| {
                                state.statuses.get(&state.event_sources[event])
                                    == Some(&BackendPollV1::Succeeded)
                            }))
                        .then_some(*id)
                    })
                    .collect();
                for id in ready {
                    state.statuses.insert(id, BackendPollV1::Succeeded);
                }
            }
            match state
                .progress_outcomes
                .pop_front()
                .unwrap_or(MockFlushOutcome::Success)
            {
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
                if state.complete_on_flush {
                    let ready: Vec<_> = state
                        .issues
                        .iter()
                        .filter_map(|(owner, id, _, _)| {
                            (*owner == stream
                                && state.submission_dependencies[id].iter().all(|event| {
                                    state.statuses.get(&state.event_sources[event])
                                        == Some(&BackendPollV1::Succeeded)
                                }))
                            .then_some(*id)
                        })
                        .collect();
                    for id in ready {
                        if state.statuses.get(&id) == Some(&BackendPollV1::Pending) {
                            state.statuses.insert(id, BackendPollV1::Succeeded);
                        }
                    }
                }
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
}
