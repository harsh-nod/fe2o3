//! Runtime-owned typed operations, independently of observer lifetime.

use super::*;
use crate::{
    RuntimeArgumentsV1, RuntimeAsyncCopyBackendV1, RuntimeCopyV1, RuntimeLaunchGeometryV1,
    RuntimeMemoryRegionV1, RuntimePeerCopyV1, RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use std::collections::{BTreeMap, VecDeque};

mod factory;
pub(super) use factory::{EngineOperationFactoryV1, stop_reply};

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

/// Owner-local driver installed before its first Context/native effect.
pub(super) trait EngineOperationV1<B: RuntimeBackendV1> {
    /// True grants driver disposal, not merely completion of its reply. Issued
    /// custody must remain here or in the Context until conclusively retired.
    fn advance(&mut self, context: &mut RuntimeContextV1<B>) -> bool;
    /// Inert flush identity, cached before first advance. Preparation has none.
    fn stream(&self) -> Option<RuntimeStreamIdV1>;
    /// Only a fully host-owned, never-adopted preparation may park.
    fn prepared_key(&self) -> Option<&Arc<generated_operation::PreparedKeyV1>> {
        None
    }
    fn reserved_key(&self) -> Option<&Arc<generated_operation::PreparedKeyV1>> {
        None
    }
    fn reserve_generated(
        &mut self,
        _context: &mut RuntimeContextV1<B>,
        _completion: &mut Option<owned::Reply<()>>,
    ) -> Result<(), crate::RuntimeGfx942GeneratedReservationErrorV1> {
        Err(crate::RuntimeGfx942GeneratedReservationErrorV1::UnsupportedPreparation)
    }
    fn preflight_adoption(
        &mut self,
        _context: &mut RuntimeContextV1<B>,
        _stream: RuntimeStreamIdV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        Err(crate::RuntimeValidationErrorV1::Unsupported.into())
    }
    fn activate_adoption(
        &mut self,
        _hold: crate::context::ContextUnpublishedHoldV1,
        _ticket: RuntimeAsyncReservedTicketV1,
    ) {
        unreachable!("only an admitted generated driver activates")
    }
    /// True permits disposal only after exact unpublished native retirement.
    fn retire_unpublished(
        &mut self,
        _context: &mut RuntimeContextV1<B>,
    ) -> Result<bool, RuntimeErrorV1<B::Error>> {
        Ok(false)
    }
    /// Called after the driver is rooted in the preallocated parked roster.
    fn complete_preparation(&mut self) {}
    /// Infallibly detach/resolve the reply before any fallible handling, without
    /// disposing possibly reachable native custody. Panic containment preserves
    /// custody, but cannot recover a reply hidden by a broken implementation.
    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1);
}

struct OperationEntryV1<B: RuntimeBackendV1> {
    stream: Option<RuntimeStreamIdV1>,
    driver: Box<dyn EngineOperationV1<B>>,
}

pub(super) struct OperationRegistryV1<B: RuntimeBackendV1> {
    entries: VecDeque<OperationEntryV1<B>>,
    parked: VecDeque<OperationEntryV1<B>>,
    streams: BTreeMap<RuntimeStreamIdV1, usize>,
    flush_cursor: Option<RuntimeStreamIdV1>,
    owner_cleanup: bool,
}

impl<B: RuntimeBackendV1> OperationRegistryV1<B> {
    pub(super) fn new(capacity: usize, owner_cleanup: bool) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            parked: VecDeque::with_capacity(capacity),
            streams: BTreeMap::new(),
            flush_cursor: None,
            owner_cleanup,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len() + self.parked.len()
    }

    pub(super) fn active_len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn insert(&mut self, operation: Box<dyn EngineOperationV1<B>>) {
        let stream = operation.stream();
        if let Some(stream) = stream {
            *self.streams.entry(stream).or_default() += 1;
        }
        self.entries.push_back(OperationEntryV1 {
            stream,
            driver: operation,
        });
    }

    pub(super) fn accepts_factory(&self, factory: &dyn EngineOperationFactoryV1<B>) -> bool {
        self.owner_cleanup || !factory.requires_owned_shutdown()
    }

    /// Resolving a reply never removes its driver from the custody roster.
    pub(super) fn stop_observations(&mut self) -> bool {
        let mut panicked = false;
        for entry in self.entries.iter_mut().chain(&mut self.parked) {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                entry
                    .driver
                    .reject(RuntimeAsyncEngineCallErrorV1::EngineStopped);
            })) {
                core::mem::forget(payload);
                panicked = true;
            }
        }
        panicked
    }

    fn retire_stream(&mut self, stream: Option<RuntimeStreamIdV1>) {
        let Some(stream) = stream else { return };
        let count = self
            .streams
            .get_mut(&stream)
            .expect("operation stream is retained");
        *count -= 1;
        if *count == 0 {
            self.streams.remove(&stream);
        }
    }

    pub(super) fn discard_prepared(
        &mut self,
        key: &Arc<generated_operation::PreparedKeyV1>,
    ) -> bool {
        self.discard_parked(key, false)
    }

    pub(super) fn discard_reserved(
        &mut self,
        key: &Arc<generated_operation::PreparedKeyV1>,
    ) -> bool {
        self.discard_parked(key, true)
    }

    fn discard_parked(
        &mut self,
        key: &Arc<generated_operation::PreparedKeyV1>,
        reserved: bool,
    ) -> bool {
        let Some(index) = self.parked.iter().position(|entry| {
            (if reserved {
                entry.driver.reserved_key()
            } else {
                entry.driver.prepared_key()
            })
            .is_some_and(|stored| Arc::ptr_eq(stored, key))
        }) else {
            return false;
        };
        // The caller contains destructor panic and owns the discard reply.
        drop(self.parked.remove(index).expect("matched parked owner"));
        true
    }

    pub(super) fn reserve_generated(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        key: &Arc<generated_operation::PreparedKeyV1>,
        completion: &mut Option<owned::Reply<()>>,
    ) -> Result<(), crate::RuntimeGfx942GeneratedReservationErrorV1> {
        let entry = self
            .parked
            .iter_mut()
            .find(|entry| {
                entry
                    .driver
                    .prepared_key()
                    .is_some_and(|stored| Arc::ptr_eq(stored, key))
            })
            .ok_or(crate::RuntimeGfx942GeneratedReservationErrorV1::Engine(
                RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket,
            ))?;
        entry.driver.reserve_generated(context, completion)
    }

    pub(super) fn activate_reserved(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        ticket: &mut Option<RuntimeAsyncReservedTicketV1>,
        stream: RuntimeStreamIdV1,
    ) -> Result<(), generated_operation::adoption::ActivationErrorV1<B::Error>> {
        use generated_operation::adoption::ActivationErrorV1 as Error;
        if context.is_terminal() || !self.owner_cleanup {
            return Err(Error::Engine(RuntimeAsyncEngineCallErrorV1::EngineStopped));
        }
        let key = &ticket.as_ref().expect("queued reserved ticket").key;
        if key.context_generation != context.capture_context_generation_v1() {
            return Err(Error::Engine(
                RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket,
            ));
        }
        let index = self
            .parked
            .iter()
            .position(|entry| {
                entry
                    .driver
                    .reserved_key()
                    .is_some_and(|stored| Arc::ptr_eq(stored, key))
            })
            .ok_or(Error::Engine(
                RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket,
            ))?;
        if self.entries.len() == self.entries.capacity() {
            return Err(Error::Engine(
                RuntimeAsyncEngineCallErrorV1::OperationCapacity,
            ));
        }
        self.parked[index]
            .driver
            .preflight_adoption(context, stream)
            .map_err(Error::Context)?;
        let hold = context
            .hold_unpublished_stream_v1(stream)
            .map_err(|error| Error::Context(error.into()))?;
        let entry = self.parked.remove(index).expect("exact parked entry");
        debug_assert!(
            entry.stream.is_none(),
            "adoption never enters the flush roster"
        );
        self.entries.push_back(entry);
        // Root the same driver before its phase changes or any adapter effect.
        self.entries
            .back_mut()
            .expect("rooted adoption")
            .driver
            .activate_adoption(hold, ticket.take().expect("consumed exact reserved ticket"));
        Ok(())
    }

    pub(super) fn retire_unpublished_v1(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        budget: usize,
    ) {
        if context.is_terminal() {
            return;
        }
        let mut index = 0;
        for _ in 0..budget.min(self.entries.len()) {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let retired = self
                    .entries
                    .get_mut(index)
                    .expect("bounded retirement roster")
                    .driver
                    .retire_unpublished(context)?;
                if context.is_terminal() {
                    return Err(crate::RuntimeValidationErrorV1::ContextTerminal.into());
                }
                if retired {
                    let entry = self
                        .entries
                        .remove(index)
                        .expect("retired unpublished owner");
                    self.retire_stream(entry.stream);
                    drop(entry);
                } else {
                    // Progress owns round-robin rotation. Rotating again here
                    // can starve both scans when their equal budgets alias.
                    index += 1;
                }
                Ok::<_, RuntimeErrorV1<B::Error>>(())
            }));
            match result {
                Ok(Ok(())) if !context.is_terminal() => {}
                failed => {
                    if let Err(payload) = failed {
                        core::mem::forget(payload);
                    }
                    context.quarantine_after_async_command_panic_v1();
                    return;
                }
            }
        }
    }

    pub(super) fn dispose_quiescent(&mut self) {
        // Pop one at a time so outer panic containment retains every later owner.
        while let Some(entry) = self.entries.pop_front() {
            drop(entry);
        }
        while let Some(entry) = self.parked.pop_front() {
            drop(entry);
        }
        self.streams.clear();
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
    reply: Option<owned::Reply<RuntimeAsyncOperationResultV1<A, B::Error>>>,
    rejected_observations: u64,
    last_rejected_observation: Option<B::Error>,
    control: Option<RuntimeAsyncOperationControlV1>,
}

impl<B: RuntimeBackendV1, A> Operation<B, A> {
    fn finish(&mut self, observation: Result<RuntimeCompletionStatusV1, RuntimeErrorV1<B::Error>>) {
        if let Some(control) = &self.control {
            control.finish_observation();
        }
        if let Some(mut reply) = self.reply.take() {
            reply.complete(Ok(RuntimeAsyncOperationResultV1 {
                submission: self.submission.take(),
                observation,
                rejected_observations: self.rejected_observations,
                last_rejected_observation: self.last_rejected_observation.take(),
            }));
        }
    }
}

impl<B: RuntimeBackendV1, A> Drop for Operation<B, A> {
    fn drop(&mut self) {
        factory::stop_reply(
            &mut self.reply,
            self.control.as_ref(),
            RuntimeAsyncEngineCallErrorV1::EngineStopped,
        );
    }
}

impl<B: RuntimeBackendV1, A> EngineOperationV1<B> for Operation<B, A> {
    fn advance(&mut self, context: &mut RuntimeContextV1<B>) -> bool {
        if let Some(submit) = self.submit.take() {
            if let Some(control) = &self.control
                && !control.start_submission()
            {
                let error =
                    if control.phase() == RuntimeAsyncOperationPhaseV1::CancelledBeforeSubmission {
                        RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission
                    } else {
                        RuntimeAsyncEngineCallErrorV1::EngineStopped
                    };
                factory::stop_reply(&mut self.reply, self.control.as_ref(), error);
                return true;
            }
            match submit(context) {
                Ok(submission) => {
                    self.submission = Some(submission);
                    if let Some(control) = &self.control {
                        control.observing();
                    }
                }
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

    fn stream(&self) -> Option<RuntimeStreamIdV1> {
        Some(self.stream)
    }

    fn reject(&mut self, error: RuntimeAsyncEngineCallErrorV1) {
        let discard_unissued = matches!(
            error,
            RuntimeAsyncEngineCallErrorV1::EngineStopped
                | RuntimeAsyncEngineCallErrorV1::CommandPanicked
        );
        factory::stop_reply(&mut self.reply, self.control.as_ref(), error);
        if discard_unissued {
            // This ordinary driver retains only an unissued host callback here;
            // possibly live native resources remain owned by the Context.
            drop(self.submit.take());
        }
    }
}

impl<B: RuntimeBackendV1 + 'static> RuntimeAsyncProgressHandleV1<B> {
    fn operation_dependencies(
        &self,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<snapshot::Charged<Box<[RuntimeEventIdV1]>>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        snapshot::charge_dependencies(&self.observer.snapshot_budget, dependencies)
    }

    pub(super) fn enqueue_operation<A: 'static>(
        &self,
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
    ) -> Result<RuntimeAsyncOperationFutureV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        self.enqueue_controlled_operation(stream, submit, None)
    }

    pub(super) fn enqueue_tracked_operation<A: 'static>(
        &self,
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
    ) -> Result<RuntimeAsyncTrackedOperationV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        let control = RuntimeAsyncOperationControlV1::new();
        let future = self.enqueue_controlled_operation(stream, submit, Some(control.clone()))?;
        Ok(RuntimeAsyncTrackedOperationV1 { future, control })
    }

    fn enqueue_controlled_operation<A: 'static>(
        &self,
        stream: RuntimeStreamIdV1,
        submit: Submit<B, A>,
        control: Option<RuntimeAsyncOperationControlV1>,
    ) -> Result<RuntimeAsyncOperationFutureV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        if self.observer.is_worker_thread() {
            return Err(RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        }
        let (reply, future) = owned::Reply::budgeted_pair(&self.observer.reply_budget)?;
        let factory = factory::OperationFactoryV1::new(stream, submit, reply, control);
        match self
            .observer
            .try_send_command(RuntimeAsyncEngineCommandV1::Operation(Box::new(factory)))
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
    /// Dependency lists are checked, compacted and charged before enqueue.
    /// Use `enqueue_launch` for a fully frozen, payload-budgeted launch request.
    pub fn launch<A: RuntimeArgumentsV1>(
        &self,
        stream: RuntimeStreamIdV1,
        kernel: Arc<TypedRuntimeKernelV1<A>>,
        arguments: A,
        geometry: RuntimeLaunchGeometryV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<RuntimeAsyncOperationFutureV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.launch(stream, &kernel, &arguments, geometry, dependencies)
                })
            }),
        )
    }

    /// Like `launch`, with local identity, pre-submission cancellation, and
    /// recoverable timeout observation. It uses the same admission and registry.
    pub fn launch_tracked<A: RuntimeArgumentsV1>(
        &self,
        stream: RuntimeStreamIdV1,
        kernel: Arc<TypedRuntimeKernelV1<A>>,
        arguments: A,
        geometry: RuntimeLaunchGeometryV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<RuntimeAsyncTrackedOperationV1<A, B::Error>, RuntimeAsyncEngineCallErrorV1> {
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_tracked_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.launch(stream, &kernel, &arguments, geometry, dependencies)
                })
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
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.copy_async(stream, source, destination, dependencies)
                })
            }),
        )
    }

    /// Same-device transfer with the same control and custody as `launch_tracked`.
    pub fn copy_async_tracked(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<
        RuntimeAsyncTrackedOperationV1<RuntimeCopyV1, B::Error>,
        RuntimeAsyncEngineCallErrorV1,
    >
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_tracked_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.copy_async(stream, source, destination, dependencies)
                })
            }),
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
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.peer_copy(stream, source, destination, dependencies)
                })
            }),
        )
    }

    /// Admitted peer transfer with local control; it does not imply XGMI routing.
    pub fn peer_copy_tracked(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<
        RuntimeAsyncTrackedOperationV1<RuntimePeerCopyV1, B::Error>,
        RuntimeAsyncEngineCallErrorV1,
    > {
        let dependencies = self.operation_dependencies(dependencies)?;
        self.enqueue_tracked_operation(
            stream,
            Box::new(move |context| {
                dependencies.with(|dependencies| {
                    context.peer_copy(stream, source, destination, dependencies)
                })
            }),
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
    for _ in 0..poll_budget.min(operations.active_len()) {
        let entry = operations
            .entries
            .front_mut()
            .expect("bounded operation roster");
        // An adapter may have performed a side effect before unwinding. Do not
        // let command panic containment turn that into permission to retry.
        match catch_unwind(AssertUnwindSafe(|| {
            let retired = entry.driver.advance(context);
            (retired, !retired && entry.driver.prepared_key().is_some())
        })) {
            Ok(_) if context.is_terminal() => return,
            Ok((true, _)) => {
                let retired = operations.entries.pop_front().expect("retired driver");
                operations.retire_stream(retired.stream);
            }
            Ok((false, false)) => operations.entries.rotate_left(1),
            Ok((false, true)) => {
                let parked = operations.entries.pop_front().expect("prepared driver");
                operations.retire_stream(parked.stream);
                operations.parked.push_back(parked);
                let entry = operations.parked.back_mut().expect("parked driver");
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                    entry.driver.complete_preparation();
                })) {
                    core::mem::forget(payload);
                    context.quarantine_after_async_command_panic_v1();
                    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                        entry
                            .driver
                            .reject(RuntimeAsyncEngineCallErrorV1::CommandPanicked);
                    })) {
                        core::mem::forget(payload);
                    }
                    return;
                }
            }
            Err(payload) => {
                core::mem::forget(payload);
                context.quarantine_after_async_command_panic_v1();
                if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                    entry
                        .driver
                        .reject(RuntimeAsyncEngineCallErrorV1::CommandPanicked);
                })) {
                    core::mem::forget(payload);
                }
                return;
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
