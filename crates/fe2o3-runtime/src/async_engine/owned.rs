//! Owner-thread construction and nonblocking commands. No backend crosses threads.

use super::*;
use crate::{RuntimeBackendFailureV1, RuntimeCleanupReportV1, RuntimeOwnedShutdownBackendV1};

struct ReplyState<R> {
    result: Option<Result<R, RuntimeAsyncEngineCallErrorV1>>,
    waker: Option<Waker>,
    // The retained result and waker drop before returning the cell's count credit.
    _permit: Option<reply_budget::ReplyPermitV1>,
}

pub(super) struct Reply<R> {
    state: Arc<Mutex<ReplyState<R>>>,
    completed: bool,
}

impl<R> Reply<R> {
    pub(super) fn pair() -> (Self, RuntimeAsyncCommandFutureV1<R>) {
        Self::with_permit(None)
    }

    pub(super) fn budgeted_pair(
        budget: &Arc<reply_budget::ReplyBudgetV1>,
    ) -> Result<(Self, RuntimeAsyncCommandFutureV1<R>), RuntimeAsyncEngineCallErrorV1> {
        Ok(Self::with_permit(Some(budget.reserve()?)))
    }

    fn with_permit(
        permit: Option<reply_budget::ReplyPermitV1>,
    ) -> (Self, RuntimeAsyncCommandFutureV1<R>) {
        let state = Arc::new(Mutex::new(ReplyState {
            result: None,
            waker: None,
            _permit: permit,
        }));
        (
            Self {
                state: Arc::clone(&state),
                completed: false,
            },
            RuntimeAsyncCommandFutureV1 {
                state,
                completed: false,
            },
        )
    }

    pub(super) fn complete(&mut self, result: Result<R, RuntimeAsyncEngineCallErrorV1>) {
        if !fe2o3_runtime_model::r61_reply_may_resolve_v1(self.completed) {
            return;
        }
        self.completed = true;
        let waker = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            state.result = Some(result);
            state.waker.take()
        };
        if let Some(waker) = waker
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| waker.wake()))
        {
            core::mem::forget(payload);
        }
    }
}

impl<R> Drop for Reply<R> {
    fn drop(&mut self) {
        self.complete(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped));
    }
}

/// Result of one boundedly enqueued context command, not GPU completion.
///
/// Dropping this future does not withdraw the command or release runtime custody.
/// A command discarded during shutdown resolves as `EngineStopped`.
#[must_use = "dropping the response does not cancel the accepted command"]
pub struct RuntimeAsyncCommandFutureV1<R> {
    state: Arc<Mutex<ReplyState<R>>>,
    completed: bool,
}

fn discard_waker(waker: Option<Waker>) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(waker))) {
        core::mem::forget(payload);
    }
}

impl<R> RuntimeAsyncCommandFutureV1<R> {
    pub(super) fn clear_waker(&self) {
        let waker = self
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .waker
            .take();
        discard_waker(waker);
    }
}

impl<R> Future for RuntimeAsyncCommandFutureV1<R> {
    type Output = Result<R, RuntimeAsyncEngineCallErrorV1>;

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        assert!(
            !self.completed,
            "runtime command future polled after completion"
        );
        // Executor-supplied clone/drop callbacks must not run under our lock.
        let mut new_waker = Some(context.waker().clone());
        let (result, old_waker) = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            let result = state.result.take();
            let old_waker = if result.is_none() {
                core::mem::replace(&mut state.waker, new_waker.take())
            } else {
                state.waker.take()
            };
            (result, old_waker)
        };
        discard_waker(old_waker);
        discard_waker(new_waker);
        match result {
            Some(result) => {
                self.completed = true;
                Poll::Ready(result)
            }
            None => Poll::Pending,
        }
    }
}

impl<R> Drop for RuntimeAsyncCommandFutureV1<R> {
    fn drop(&mut self) {
        self.clear_waker();
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncEngineHandleV1<B> {
    /// Enqueues a command without waiting for the owner thread to execute it.
    ///
    /// The bounded channel rejects immediately when full. The closure must use
    /// nonblocking runtime operations to preserve progress fairness. As with
    /// `try_with_context`, returning a submission means accepted custody, not
    /// completion. Local command captures are not a serialized transport API.
    pub fn enqueue_with_context<R, F>(
        &self,
        operation: F,
    ) -> Result<RuntimeAsyncCommandFutureV1<R>, RuntimeAsyncEngineCallErrorV1>
    where
        R: Send + 'static,
        F: FnOnce(&mut RuntimeContextV1<B>) -> R + Send + 'static,
    {
        if self.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (mut reply, future) = Reply::budgeted_pair(&self.reply_budget)?;
        let quarantine_command_panics = self.quarantine_command_panics;
        let command = RuntimeAsyncEngineCommandV1::Context(Box::new(move |context| {
            let result = catch_unwind(AssertUnwindSafe(|| operation(context))).map_err(|payload| {
                core::mem::forget(payload);
                if quarantine_command_panics {
                    context.quarantine_after_async_command_panic_v1();
                }
                RuntimeAsyncEngineCallErrorV1::CommandPanicked
            });
            reply.complete(result);
        }));
        match self.try_send_command(command) {
            Ok(()) => Ok(future),
            Err(TrySendError::Full(_)) => Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull),
            Err(TrySendError::Disconnected(_)) => Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
        }
    }
}

/// Failure before an owner-thread context becomes available.
#[derive(Debug)]
pub enum RuntimeAsyncOwnedSpawnErrorV1<E> {
    InvalidEngineConfig(RuntimeAsyncEngineConfigErrorV1),
    InvalidProgressConfig(RuntimeAsyncProgressConfigErrorV1),
    CaptureBudget(fe2o3_resource_accounting::ResourceCreditErrorV1),
    Thread(io::Error),
    Initialize(E),
    InitializerPanicked,
}

impl<E: fmt::Display> fmt::Display for RuntimeAsyncOwnedSpawnErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEngineConfig(error) => error.fmt(formatter),
            Self::InvalidProgressConfig(error) => error.fmt(formatter),
            Self::CaptureBudget(error) => error.fmt(formatter),
            Self::Thread(error) => write!(formatter, "runtime owner thread: {error}"),
            Self::Initialize(error) => write!(formatter, "runtime owner initialization: {error}"),
            Self::InitializerPanicked => formatter.write_str("runtime owner initializer panicked"),
        }
    }
}

impl<E: Error + 'static> Error for RuntimeAsyncOwnedSpawnErrorV1<E> {}

/// Owner-thread shutdown disposition. Quarantine never asserts quiescence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncOwnedDispositionV1 {
    Released,
    /// The complete owning context was deliberately retained until process exit.
    RetainedUntilProcessExit,
}

/// Exact cleanup report, or no report if an adapter panicked before returning one.
#[derive(Debug)]
pub struct RuntimeAsyncOwnedShutdownV1<E> {
    pub disposition: RuntimeAsyncOwnedDispositionV1,
    pub cleanup: Option<RuntimeCleanupReportV1<E>>,
    pub worker_panicked: bool,
    pub native_failure: Option<RuntimeBackendFailureV1<E>>,
}

/// One progress thread which creates, uses, and retires even a `!Send` backend.
///
/// Only factories, commands, observations, and cleanup reports cross threads.
/// Shutdown stops observation, then runs the existing context cleanup once on
/// its owner thread. Failed cleanup or an adapter panic retains the entire
/// context until process exit; it cannot be recovered on the caller thread.
/// Command panics also seal the context because a callback may have reached an
/// adapter before unwinding. The factory remains responsible for cleanup and
/// panic custody until it successfully returns its complete context.
/// Use an isolated process when that terminal retention policy is required to
/// be reclaimable. This API does not grant device or executable authority.
///
/// ```compile_fail
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeAsyncOwnedEngineV1};
/// fn require_send<T: Send>() {}
/// require_send::<RuntimeAsyncOwnedEngineV1<KfdRuntimeBackendV1>>();
/// ```
#[must_use = "the owner must be shut down to inspect cleanup and quarantine"]
pub struct RuntimeAsyncOwnedEngineV1<B: RuntimeBackendV1 + 'static> {
    admission: Arc<drain::AdmissionV1>,
    sender: Option<SyncSender<RuntimeAsyncEngineCommandV1<B>>>,
    worker: Option<JoinHandle<RuntimeAsyncOwnedShutdownV1<B::Error>>>,
    thread_affinity: PhantomData<Rc<()>>,
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncOwnedEngineV1<B> {
    pub fn spawn_with_progress<F, E>(
        factory: F,
        config: RuntimeAsyncEngineConfigV1,
        progress_config: RuntimeAsyncProgressConfigV1,
    ) -> Result<(Self, RuntimeAsyncProgressHandleV1<B>), RuntimeAsyncOwnedSpawnErrorV1<E>>
    where
        B: RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1,
        F: FnOnce() -> Result<RuntimeContextV1<B>, E> + Send + 'static,
        E: Send + 'static,
    {
        RuntimeAsyncEngineConfigV1::new(
            config.command_capacity,
            config.waiter_capacity,
            config.commands_per_tick,
            config.polls_per_tick,
            config.poll_interval,
        )
        .and_then(|validated| validated.with_snapshot_byte_capacity(config.snapshot_byte_capacity))
        .and_then(|validated| validated.with_reply_capacity(config.reply_capacity))
        .and_then(|validated| {
            validated.with_drain_capture_byte_capacity(config.drain_capture_byte_capacity)
        })
        .map_err(RuntimeAsyncOwnedSpawnErrorV1::InvalidEngineConfig)?;
        RuntimeAsyncProgressConfigV1::new(
            progress_config.stream_capacity,
            progress_config.flushes_per_tick,
        )
        .map_err(RuntimeAsyncOwnedSpawnErrorV1::InvalidProgressConfig)?;
        let capture_budget = config
            .capture_budget_v1()
            .map_err(RuntimeAsyncOwnedSpawnErrorV1::CaptureBudget)?;
        let (sender, receiver) = sync_channel(config.command_capacity);
        let admission = drain::AdmissionV1::new();
        let worker_admission = Arc::clone(&admission);
        let (startup_sender, startup_receiver) = sync_channel(1);
        let worker_thread = Arc::new(OnceLock::new());
        let worker_id = Arc::clone(&worker_thread);
        let worker = thread::Builder::new()
            .name("fe2o3-runtime-owner-v1".into())
            .spawn(move || {
                worker_id
                    .set(thread::current().id())
                    .expect("one owner thread");
                // Reserve the bounded roster before constructing native custody.
                let operations = operation::OperationRegistryV1::new(config.waiter_capacity, true);
                let mut context = match catch_unwind(AssertUnwindSafe(factory)) {
                    Ok(Ok(context)) => context,
                    Ok(Err(error)) => {
                        let _ = startup_sender
                            .send(Err(RuntimeAsyncOwnedSpawnErrorV1::Initialize(error)));
                        return RuntimeAsyncOwnedShutdownV1 {
                            disposition: RuntimeAsyncOwnedDispositionV1::Released,
                            cleanup: None,
                            worker_panicked: false,
                            native_failure: None,
                        };
                    }
                    Err(payload) => {
                        core::mem::forget(payload);
                        let _ = startup_sender
                            .send(Err(RuntimeAsyncOwnedSpawnErrorV1::InitializerPanicked));
                        return RuntimeAsyncOwnedShutdownV1 {
                            // No context was returned to engine ownership.
                            disposition: RuntimeAsyncOwnedDispositionV1::Released,
                            cleanup: None,
                            worker_panicked: true,
                            native_failure: None,
                        };
                    }
                };
                let _ = startup_sender.send(Ok(context.capture_context_generation_v1()));
                // Rebind after Context for successful drop order, outside the
                // unwind boundary so both owners survive failed native shutdown.
                let mut operations = operations;
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    run_engine_context_v1(
                        &mut context,
                        &mut operations,
                        receiver,
                        config,
                        Some(RuntimeAsyncProgressModeV1 {
                            config: progress_config,
                            flush_stream: flush_stream_v1::<B>,
                        }),
                        worker_admission,
                    );
                    let cleanup = context.cleanup();
                    let native_failure = if cleanup.is_complete() {
                        context.shutdown_owned_backend_v1().err()
                    } else {
                        None
                    };
                    (cleanup, native_failure)
                }));
                match outcome {
                    Ok((cleanup, None))
                        if fe2o3_runtime_model::r61_owner_may_release_v1(
                            fe2o3_runtime_model::R61ShutdownFactsV1 {
                                worker_returned_normally: true,
                                context_cleanup_complete: cleanup.is_complete(),
                                native_shutdown_attempted: cleanup.is_complete(),
                                native_shutdown_succeeded: cleanup.is_complete(),
                            },
                        ) =>
                    {
                        RuntimeAsyncOwnedShutdownV1 {
                            disposition: RuntimeAsyncOwnedDispositionV1::Released,
                            cleanup: Some(cleanup),
                            worker_panicked: false,
                            native_failure: None,
                        }
                    }
                    Ok((cleanup, native_failure)) => {
                        core::mem::forget(operations);
                        core::mem::forget(context);
                        RuntimeAsyncOwnedShutdownV1 {
                            disposition: RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit,
                            cleanup: Some(cleanup),
                            worker_panicked: false,
                            native_failure,
                        }
                    }
                    Err(payload) => {
                        core::mem::forget(payload);
                        operations.stop_observations();
                        core::mem::forget(operations);
                        core::mem::forget(context);
                        RuntimeAsyncOwnedShutdownV1 {
                            disposition: RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit,
                            cleanup: None,
                            worker_panicked: true,
                            native_failure: None,
                        }
                    }
                }
            })
            .map_err(RuntimeAsyncOwnedSpawnErrorV1::Thread)?;
        let context_generation = match startup_receiver
            .recv()
            .unwrap_or(Err(RuntimeAsyncOwnedSpawnErrorV1::InitializerPanicked))
        {
            Ok(generation) => generation,
            Err(error) => {
                let _ = worker.join();
                return Err(error);
            }
        };
        let observer = RuntimeAsyncEngineHandleV1 {
            context_generation,
            capture_budget,
            reply_budget: reply_budget::ReplyBudgetV1::new(config.reply_capacity),
            admission: Arc::clone(&admission),
            snapshot_budget: snapshot::SnapshotBudgetV1::new(config.snapshot_byte_capacity),
            graph_slot: Arc::new(AtomicBool::new(false)),
            sender: sender.clone(),
            worker_thread,
            quarantine_command_panics: true,
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

    pub fn shutdown(
        mut self,
    ) -> Result<RuntimeAsyncOwnedShutdownV1<B::Error>, RuntimeAsyncEngineJoinErrorV1> {
        self.stop_and_join()
    }

    fn stop_and_join(
        &mut self,
    ) -> Result<RuntimeAsyncOwnedShutdownV1<B::Error>, RuntimeAsyncEngineJoinErrorV1> {
        self.admission.close();
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(RuntimeAsyncEngineCommandV1::Stop);
        }
        self.worker
            .take()
            .ok_or(RuntimeAsyncEngineJoinErrorV1::AlreadyStopped)?
            .join()
            .map_err(|payload| {
                core::mem::forget(payload);
                RuntimeAsyncEngineJoinErrorV1::WorkerPanicked
            })
    }
}

impl<B: RuntimeBackendV1 + 'static> Drop for RuntimeAsyncOwnedEngineV1<B> {
    fn drop(&mut self) {
        let _ = self.stop_and_join();
    }
}
