//! Executor-neutral, bounded background observation for runtime events.

use crate::{
    RuntimeBackendV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeErrorV1,
    RuntimeEventIdV1, RuntimeValidationErrorV1,
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

type RuntimeContextCommandV1<B> = Box<dyn FnOnce(&mut RuntimeContextV1<B>) + Send + 'static>;

enum RuntimeAsyncEngineCommandV1<B: RuntimeBackendV1> {
    Context(RuntimeContextCommandV1<B>),
    Register {
        event: RuntimeEventIdV1,
        cell: Arc<RuntimeAsyncFutureCellV1<B::Error>>,
        response: SyncSender<Result<(), RuntimeAsyncEventRegistrationErrorV1>>,
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
                run_engine_v1(context, receiver, config)
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

    /// Stops observation, wakes pending futures as stopped, and returns the context.
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
) -> RuntimeContextV1<B> {
    let mut waiters = RuntimeAsyncWaiterRegistryV1::new();
    let mut next_event = None;
    let mut stopped = false;
    while !stopped {
        match receiver.recv_timeout(config.poll_interval) {
            Ok(command) => {
                stopped = handle_command_v1(&mut context, &mut waiters.entries, command, config);
                for _ in 1..config.commands_per_tick {
                    if stopped {
                        break;
                    }
                    match receiver.try_recv() {
                        Ok(command) => {
                            stopped = handle_command_v1(
                                &mut context,
                                &mut waiters.entries,
                                command,
                                config,
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
            poll_waiters_v1(
                &mut context,
                &mut waiters.entries,
                &mut next_event,
                config.polls_per_tick,
            );
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
    command: RuntimeAsyncEngineCommandV1<B>,
    config: RuntimeAsyncEngineConfigV1,
) -> bool {
    match command {
        RuntimeAsyncEngineCommandV1::Context(command) => {
            command(context);
            false
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
        RuntimeAsyncEngineCommandV1::Stop => true,
    }
}

fn poll_waiters_v1<B: RuntimeBackendV1 + Send + 'static>(
    context: &mut RuntimeContextV1<B>,
    waiters: &mut BTreeMap<RuntimeEventIdV1, Arc<RuntimeAsyncFutureCellV1<B::Error>>>,
    next_event: &mut Option<RuntimeEventIdV1>,
    budget: usize,
) {
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
            waiters.remove(&event);
            continue;
        }
        match context.poll_event(event) {
            Ok(RuntimeCompletionStatusV1::Pending) => {}
            Ok(status) => {
                cell.complete(Ok(status));
                waiters.remove(&event);
            }
            Err(error) => {
                cell.complete(Err(RuntimeAsyncEventErrorV1::Runtime(error)));
                waiters.remove(&event);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1, BackendPollV1,
        RuntimeArgumentsV1, RuntimeBackendFailureV1, RuntimeBindingV1, RuntimeCapabilitiesV1,
        RuntimeLaunchGeometryV1, RuntimeMemoryKindV1,
    };
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use std::task::{Wake, Waker};
    use std::thread::ThreadId;
    use std::time::Instant;

    #[derive(Debug)]
    struct MockError;

    impl fmt::Display for MockError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("mock error")
        }
    }

    impl Error for MockError {}

    #[derive(Default)]
    struct MockState {
        next: u64,
        statuses: HashMap<u64, BackendPollV1>,
        poll_threads: HashSet<ThreadId>,
        poll_calls: usize,
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
            Ok(self.next())
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
            Err(RuntimeBackendFailureV1::Rejected(MockError))
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
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
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

    fn poll_once<E>(
        future: &mut RuntimeEventFutureV1<E>,
        waker: &Waker,
    ) -> Poll<Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>>> {
        let mut context = Context::from_waker(waker);
        Pin::new(future).poll(&mut context)
    }

    fn poll_until_ready<E>(
        future: &mut RuntimeEventFutureV1<E>,
        waker: &Waker,
    ) -> Result<RuntimeCompletionStatusV1, RuntimeAsyncEventErrorV1<E>> {
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
        poll_waiters_v1(&mut context, &mut waiters, &mut next_event, 1);
        assert_eq!(state.lock().unwrap().poll_calls, 1);
        assert_eq!(next_event, Some(second_event));
        poll_waiters_v1(&mut context, &mut waiters, &mut next_event, 1);
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
