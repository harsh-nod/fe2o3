//! Caller-driven ownership with the same scheduler and native retirement rules.

use super::*;
use crate::RuntimeOwnedShutdownBackendV1;
use std::any::Any;
use std::task::Wake;
use std::time::Instant;

#[derive(Debug)]
pub enum RuntimeAsyncCurrentThreadInitErrorV1<E> {
    InvalidEngineConfig(RuntimeAsyncEngineConfigErrorV1),
    InvalidProgressConfig(RuntimeAsyncProgressConfigErrorV1),
    CaptureBudget(fe2o3_resource_accounting::ResourceCreditErrorV1),
    Initialize(E),
    InitializerPanicked,
}

impl<E: fmt::Display> fmt::Display for RuntimeAsyncCurrentThreadInitErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEngineConfig(error) => error.fmt(f),
            Self::InvalidProgressConfig(error) => error.fmt(f),
            Self::CaptureBudget(error) => error.fmt(f),
            Self::Initialize(error) => write!(f, "runtime local owner initialization: {error}"),
            Self::InitializerPanicked => f.write_str("runtime local owner initializer panicked"),
        }
    }
}
impl<E: Error + 'static> Error for RuntimeAsyncCurrentThreadInitErrorV1<E> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncTickV1 {
    /// Admission/progress remains live; this does not assert GPU progress.
    Running,
    /// Observation has stopped. Explicit shutdown still reports native custody.
    Stopped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeAsyncDriveErrorV1 {
    DeadlineExceeded,
    EngineStopped,
    OwnerPanicked,
}
impl fmt::Display for RuntimeAsyncDriveErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "runtime local progress: {self:?}")
    }
}
impl Error for RuntimeAsyncDriveErrorV1 {}

struct ActiveGuard(Arc<AtomicBool>);
impl ActiveGuard {
    fn enter(active: &Arc<AtomicBool>) -> Self {
        assert!(
            !active.swap(true, Ordering::AcqRel),
            "recursive local scheduler entry"
        );
        Self(Arc::clone(active))
    }
}
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct DriveWake(thread::Thread);
impl Wake for DriveWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// An owned, thread-affine runtime driven explicitly by its creator.
///
/// Construction and progress spawn no threads. Nonblocking handle commands are
/// accepted between ticks; callbacks during a tick cannot recursively enqueue.
/// Blocking handle Context calls and generated `try_join` always reject on this
/// owner thread. Use `enqueue_with_context` and poll completion futures instead.
/// Synchronous event/progress observer registration also rejects on this owner.
/// Use [`RuntimeAsyncEngineHandleV1::enqueue_event_registration`],
/// [`RuntimeAsyncProgressHandleV1::enqueue_stream_registration`] or
/// [`RuntimeAsyncProgressHandleV1::enqueue_event_registration_with_progress`].
/// Generated and tracked operations maintain their own progress without those
/// registrations. Owner-side scheduling and observation require `tick` or
/// `drive_until_ready`; already-issued device work can continue independently.
/// This API grants no device, compiler, verifier, or completion authority.
///
/// Shutdown is Stop: it discards queued commands without executing them, then
/// uses the same cleanup and native disposition rules as the background owner.
/// Use `begin_drain` and drive its original future first to finish accepted work.
/// Failed cleanup or an owner panic retains custody until process exit.
///
/// ```no_run
/// use fe2o3_runtime::{RuntimeAsyncCurrentThreadOwnedEngineV1,
///     RuntimeAsyncEngineConfigV1, RuntimeAsyncProgressConfigV1,
///     RuntimeBackendV1, RuntimeContextV1, RuntimeFlushBackendV1,
///     RuntimeOwnedShutdownBackendV1};
/// use std::time::{Duration, Instant};
/// fn run<B>(factory: impl FnOnce() -> Result<RuntimeContextV1<B>, ()>)
/// where B: RuntimeBackendV1 + RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1 + 'static {
///     let (mut engine, handle) = RuntimeAsyncCurrentThreadOwnedEngineV1::new_with_progress(
///         factory, RuntimeAsyncEngineConfigV1::default(), RuntimeAsyncProgressConfigV1::default()
///     ).unwrap();
///     let mut work = std::pin::pin!(async {
///         handle.observer().enqueue_with_context(|context| context.devices().len()).unwrap().await
///     });
///     let result = engine.drive_until_ready(work.as_mut(), Instant::now() + Duration::from_secs(1));
///     // On deadline expiry, keep `work` and drive it again; it was not cancelled.
///     let shutdown = engine.shutdown();
///     let _ = (result, shutdown);
/// }
/// ```
///
/// ```compile_fail,E0277
/// use fe2o3_runtime::{KfdRuntimeBackendV1, RuntimeAsyncCurrentThreadOwnedEngineV1};
/// fn require_send<T: Send>() {}
/// fn reject() { require_send::<RuntimeAsyncCurrentThreadOwnedEngineV1<KfdRuntimeBackendV1>>(); }
/// ```
#[must_use = "call shutdown to inspect cleanup and retained native custody"]
pub struct RuntimeAsyncCurrentThreadOwnedEngineV1<
    B: RuntimeBackendV1 + RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1 + 'static,
> {
    context: Option<RuntimeContextV1<B>>,
    operations: Option<operation::OperationRegistryV1<B>>,
    scheduler: Option<scheduler::SchedulerV1<B>>,
    receiver: Option<Receiver<RuntimeAsyncEngineCommandV1<B>>>,
    sender: Option<SyncSender<RuntimeAsyncEngineCommandV1<B>>>,
    admission: Arc<drain::AdmissionV1>,
    active: Arc<AtomicBool>,
    config: RuntimeAsyncEngineConfigV1,
    progress: RuntimeAsyncProgressModeV1<B>,
    report: Option<RuntimeAsyncOwnedShutdownV1<B::Error>>,
    thread_affinity: PhantomData<Rc<()>>,
}

impl<B: RuntimeBackendV1 + RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1 + 'static>
    RuntimeAsyncCurrentThreadOwnedEngineV1<B>
{
    /// The factory may borrow local, non-Send values. Context constructors retain
    /// their accepted backend on initialization unwind. The factory remains
    /// responsible for resources outside that boundary, returned errors, and a
    /// complete Context until it returns it to this owner.
    pub fn new_with_progress<F, E>(
        factory: F,
        config: RuntimeAsyncEngineConfigV1,
        progress_config: RuntimeAsyncProgressConfigV1,
    ) -> Result<(Self, RuntimeAsyncProgressHandleV1<B>), RuntimeAsyncCurrentThreadInitErrorV1<E>>
    where
        F: FnOnce() -> Result<RuntimeContextV1<B>, E>,
    {
        RuntimeAsyncEngineConfigV1::new(
            config.command_capacity,
            config.waiter_capacity,
            config.commands_per_tick,
            config.polls_per_tick,
            config.poll_interval,
        )
        .and_then(|v| v.with_snapshot_byte_capacity(config.snapshot_byte_capacity))
        .and_then(|v| v.with_reply_capacity(config.reply_capacity))
        .and_then(|v| v.with_drain_capture_byte_capacity(config.drain_capture_byte_capacity))
        .map_err(RuntimeAsyncCurrentThreadInitErrorV1::InvalidEngineConfig)?;
        RuntimeAsyncProgressConfigV1::new(
            progress_config.stream_capacity,
            progress_config.flushes_per_tick,
        )
        .map_err(RuntimeAsyncCurrentThreadInitErrorV1::InvalidProgressConfig)?;
        let capture_budget = config
            .capture_budget_v1()
            .map_err(RuntimeAsyncCurrentThreadInitErrorV1::CaptureBudget)?;
        let (sender, receiver) = sync_channel(config.command_capacity);
        let admission = drain::AdmissionV1::new();
        let active = Arc::new(AtomicBool::new(false));
        let worker_thread = Arc::new(OnceLock::new());
        worker_thread
            .set(thread::current().id())
            .expect("one local owner");
        let reply_budget = reply_budget::ReplyBudgetV1::new(config.reply_capacity);
        let snapshot_budget = snapshot::SnapshotBudgetV1::new(config.snapshot_byte_capacity);
        let graph_slot = Arc::new(AtomicBool::new(false));
        let operations = operation::OperationRegistryV1::new(config.waiter_capacity, true);
        let context = match catch_unwind(AssertUnwindSafe(factory)) {
            Ok(Ok(context)) => context,
            Ok(Err(error)) => return Err(RuntimeAsyncCurrentThreadInitErrorV1::Initialize(error)),
            Err(payload) => {
                core::mem::forget(payload);
                return Err(RuntimeAsyncCurrentThreadInitErrorV1::InitializerPanicked);
            }
        };
        let scheduler =
            scheduler::SchedulerV1::new(Arc::clone(&admission), true, context.is_terminal());
        let observer = RuntimeAsyncEngineHandleV1 {
            context_generation: context.capture_context_generation_v1(),
            capture_budget,
            reply_budget,
            admission: Arc::clone(&admission),
            sender: sender.clone(),
            worker_thread,
            local_active: Some(Arc::clone(&active)),
            quarantine_command_panics: true,
            graph_slot,
            snapshot_budget,
        };
        Ok((
            Self {
                context: Some(context),
                operations: Some(operations),
                scheduler: Some(scheduler),
                receiver: Some(receiver),
                sender: Some(sender),
                admission,
                active,
                config,
                progress: RuntimeAsyncProgressModeV1 {
                    config: progress_config,
                    flush_stream: flush_stream_v1::<B>,
                },
                report: None,
                thread_affinity: PhantomData,
            },
            RuntimeAsyncProgressHandleV1 { observer },
        ))
    }

    /// Executes one configured work budget without waiting for a queued command.
    /// Backend calls and callbacks must return; this is not a preemptive bound.
    pub fn tick(&mut self) -> Result<RuntimeAsyncTickV1, RuntimeAsyncDriveErrorV1> {
        if self.context.is_none() {
            return Ok(RuntimeAsyncTickV1::Stopped);
        }
        let _active = ActiveGuard::enter(&self.active);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let context = self.context.as_mut().expect("owned context");
            let operations = self.operations.as_mut().expect("owned operations");
            let scheduler = self.scheduler.as_mut().expect("owned scheduler");
            if !scheduler.stopped {
                scheduler.tick(
                    context,
                    operations,
                    self.receiver.as_ref().expect("live receiver"),
                    self.config,
                    Some(&self.progress),
                    Duration::ZERO,
                );
            }
            if scheduler.stopped {
                scheduler.finish(context, operations);
                drop(self.receiver.take());
                RuntimeAsyncTickV1::Stopped
            } else {
                RuntimeAsyncTickV1::Running
            }
        }));
        match outcome {
            Ok(state) => Ok(state),
            Err(payload) => {
                self.report = Some(self.stop_inner(Some(payload)));
                Err(RuntimeAsyncDriveErrorV1::OwnerPanicked)
            }
        }
    }

    /// Polls a borrowed future between scheduler ticks. Deadline expiry neither
    /// cancels accepted work nor consumes the future; the same future may resume.
    /// The deadline is cooperative, checked only after callbacks return. A result
    /// already ready at entry wins over an expired deadline. Future poll panics
    /// propagate to the caller while this engine retains its Context.
    pub fn drive_until_ready<F: Future + ?Sized>(
        &mut self,
        mut future: Pin<&mut F>,
        deadline: Instant,
    ) -> Result<F::Output, RuntimeAsyncDriveErrorV1> {
        let waker = Waker::from(Arc::new(DriveWake(thread::current())));
        let mut cx = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return Ok(value);
            }
            if Instant::now() >= deadline {
                return Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded);
            }
            let state = self.tick()?;
            if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
                return Ok(value);
            }
            if state == RuntimeAsyncTickV1::Stopped {
                return Err(RuntimeAsyncDriveErrorV1::EngineStopped);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded);
            }
            thread::park_timeout(self.config.poll_interval.min(remaining));
        }
    }

    /// Stops locally, never by blocking a send into the owner's own channel.
    pub fn shutdown(mut self) -> RuntimeAsyncOwnedShutdownV1<B::Error> {
        let _active = ActiveGuard::enter(&self.active);
        self.report.take().unwrap_or_else(|| self.stop_inner(None))
    }

    fn stop_inner(
        &mut self,
        mut prior_panic: Option<Box<dyn Any + Send>>,
    ) -> RuntimeAsyncOwnedShutdownV1<B::Error> {
        self.admission.close();
        let mut context = self.context.take().expect("owned context settles once");
        let mut operations = self
            .operations
            .take()
            .expect("owned operations settle once");
        let mut scheduler = self.scheduler.take().expect("owned scheduler settles once");
        let receiver = self.receiver.take();
        let sender = self.sender.take();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if prior_panic.is_some() {
                context.quarantine_after_async_command_panic_v1();
            }
            scheduler.finish(&mut context, &mut operations);
            drop(receiver);
            drop(sender);
            drop(scheduler);
            if prior_panic.is_some() {
                None
            } else {
                Some(owned::cleanup_owned_context_v1(
                    &mut context,
                    &mut operations,
                ))
            }
        }));
        let outcome = match outcome {
            Ok(Some(result)) => Ok(result),
            Ok(None) => Err(prior_panic.take().expect("retained original panic")),
            Err(payload) => {
                if let Some(prior) = prior_panic.take() {
                    core::mem::forget(prior);
                }
                Err(payload)
            }
        };
        owned::settle_owned_context_v1(context, operations, outcome)
    }
}

impl<B: RuntimeBackendV1 + RuntimeFlushBackendV1 + RuntimeOwnedShutdownBackendV1 + 'static> Drop
    for RuntimeAsyncCurrentThreadOwnedEngineV1<B>
{
    fn drop(&mut self) {
        if self.context.is_some() {
            let _active = ActiveGuard::enter(&self.active);
            let report = self.stop_inner(None);
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(report))) {
                core::mem::forget(payload);
            }
        }
    }
}
