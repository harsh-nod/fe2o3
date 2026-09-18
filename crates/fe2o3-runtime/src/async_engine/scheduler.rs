//! One scheduler state for background and caller-driven owned progress.
use super::*;

pub(super) struct SchedulerV1<B: RuntimeBackendV1 + 'static> {
    admission: drain::AdmissionWorkerGuardV1,
    draining: Option<drain::DrainRequest>,
    queue_exhausted: bool,
    waiters: RuntimeAsyncWaiterRegistryV1<B::Error>,
    graph: Option<Box<dyn graph::EngineGraphV1<B>>>,
    progress_registry: Option<RuntimeAsyncProgressRegistryV1<B::Error>>,
    next_event: Option<RuntimeEventIdV1>,
    next_stream: Option<RuntimeStreamIdV1>,
    pub(super) stopped: bool,
    finished: bool,
}

impl<B: RuntimeBackendV1 + 'static> SchedulerV1<B> {
    pub(super) fn new(admission: Arc<drain::AdmissionV1>, progress: bool, stopped: bool) -> Self {
        Self {
            admission: drain::AdmissionWorkerGuardV1(admission),
            draining: None,
            queue_exhausted: false,
            waiters: RuntimeAsyncWaiterRegistryV1::new(),
            graph: None,
            progress_registry: progress.then(RuntimeAsyncProgressRegistryV1::new),
            next_event: None,
            next_stream: None,
            stopped,
            finished: false,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn tick(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
        receiver: &Receiver<RuntimeAsyncEngineCommandV1<B>>,
        config: RuntimeAsyncEngineConfigV1,
        progress: Option<&RuntimeAsyncProgressModeV1<B>>,
        wait: Duration,
    ) {
        if self.stopped {
            return;
        }
        if self.draining.is_none() {
            self.draining = drain::DrainRequest::take(&self.admission.0);
        }
        match receiver.recv_timeout(wait) {
            Ok(command) => {
                self.stopped = handle_command_v1(
                    context,
                    &mut self.waiters.entries,
                    operations,
                    &mut self.graph,
                    self.progress_registry.as_mut(),
                    command,
                    config,
                    progress.as_ref().map(|mode| mode.config),
                );
                for _ in 1..config.commands_per_tick {
                    if self.stopped {
                        break;
                    }
                    match receiver.try_recv() {
                        Ok(command) => {
                            self.stopped = handle_command_v1(
                                context,
                                &mut self.waiters.entries,
                                operations,
                                &mut self.graph,
                                self.progress_registry.as_mut(),
                                command,
                                config,
                                progress.as_ref().map(|mode| mode.config),
                            );
                        }
                        Err(TryRecvError::Empty) => {
                            self.queue_exhausted = self.draining.is_some();
                            break;
                        }
                        Err(TryRecvError::Disconnected) => {
                            self.stopped = true;
                            break;
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.queue_exhausted = self.draining.is_some();
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => self.stopped = true,
        }
        if !self.stopped
            && let Some(mode) = progress.as_ref()
        {
            if let Some(active) = self.graph.as_mut() {
                match catch_unwind(AssertUnwindSafe(|| {
                    active.advance(
                        context,
                        operations,
                        config.polls_per_tick,
                        mode.config.flushes_per_tick,
                    )
                })) {
                    Ok(true) => self.graph = None,
                    Ok(false) => {}
                    Err(payload) => {
                        core::mem::forget(payload);
                        context.quarantine_after_async_command_panic_v1();
                    }
                }
            }
            if context.is_terminal() {
                self.stopped = true;
                return;
            }
            operation::advance_operations_v1(
                context,
                operations,
                config.polls_per_tick,
                mode.config.flushes_per_tick,
                mode.flush_stream,
            );
            self.stopped = context.is_terminal();
        }
        if !self.stopped {
            self.stopped = poll_waiters_v1(
                context,
                &mut self.waiters.entries,
                self.progress_registry
                    .as_mut()
                    .map(|registry| &mut registry.entries),
                &mut self.next_event,
                config.polls_per_tick,
            );
        }
        if !self.stopped
            && let (Some(mode), Some(registry)) =
                (progress.as_ref(), self.progress_registry.as_mut())
        {
            self.stopped = flush_progress_v1(
                context,
                &mut registry.entries,
                &mut self.next_stream,
                mode.config.flushes_per_tick,
                mode.flush_stream,
            );
        }
        if !self.stopped
            && let (Some(request), Some(mode)) = (self.draining.as_mut(), progress.as_ref())
        {
            if self.queue_exhausted {
                operations.retire_unpublished_v1(context, config.polls_per_tick);
                if context.is_terminal() {
                    self.stopped = true;
                    return;
                }
            }
            self.stopped = match catch_unwind(AssertUnwindSafe(|| {
                request.tick(
                    context,
                    self.queue_exhausted,
                    operations.active_len(),
                    self.graph.is_some(),
                    self.waiters.entries.is_empty(),
                    config,
                    mode,
                )
            })) {
                Ok(stopped) => stopped,
                Err(payload) => {
                    core::mem::forget(payload);
                    context.quarantine_after_async_command_panic_v1();
                    true
                }
            };
        }
    }
    pub(super) fn finish(
        &mut self,
        context: &mut RuntimeContextV1<B>,
        operations: &mut operation::OperationRegistryV1<B>,
    ) {
        if self.finished {
            return;
        }
        self.stopped = true;
        let pending_drain = self.admission.0.close_and_take();
        if self.draining.is_none() {
            self.draining = pending_drain;
        }
        if self
            .draining
            .as_ref()
            .is_some_and(|request| !request.is_quiescent())
        {
            context.quarantine_after_async_command_panic_v1();
        }
        if let Some(mut graph) = self.graph.take() {
            if context.is_terminal() {
                // Dropping an observer/driver cannot discharge ambiguous custody.
            } else if let Err(payload) =
                catch_unwind(AssertUnwindSafe(|| graph.stop(context, operations)))
            {
                core::mem::forget(payload);
                context.quarantine_after_async_command_panic_v1();
            }
        }
        if let Some(registry) = self.progress_registry.as_mut() {
            for (_, cell) in core::mem::take(&mut registry.entries) {
                cell.stop();
            }
        }
        for (_, cell) in core::mem::take(&mut self.waiters.entries) {
            cell.complete(Err(RuntimeAsyncEventErrorV1::EngineStopped));
        }
        if operations.stop_observations() {
            context.quarantine_after_async_command_panic_v1();
        }
        self.finished = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PanicDropGraph;
    impl<B: RuntimeBackendV1> graph::EngineGraphV1<B> for PanicDropGraph {
        fn admit(
            &mut self,
            _: &mut RuntimeContextV1<B>,
            _: &mut operation::OperationRegistryV1<B>,
        ) -> bool {
            true
        }
        fn advance(
            &mut self,
            _: &mut RuntimeContextV1<B>,
            _: &mut operation::OperationRegistryV1<B>,
            _: usize,
            _: usize,
        ) -> bool {
            false
        }
        fn reject(&mut self, _: RuntimeGraphErrorV1<B::Error>) {}
        fn stop(&mut self, _: &mut RuntimeContextV1<B>, _: &mut operation::OperationRegistryV1<B>) {
        }
    }
    impl Drop for PanicDropGraph {
        fn drop(&mut self) {
            panic!("scripted graph destructor panic");
        }
    }

    #[test]
    fn current_thread_finish_retry_stops_suffix_after_graph_destructor_panics() {
        let (mut context, stream) = super::super::tests::scheduler_fixture();
        let mut operations = operation::OperationRegistryV1::new(4, true);
        let mut scheduler = SchedulerV1::new(drain::AdmissionV1::new(), true, false);
        let cell = Arc::new(RuntimeAsyncProgressCellV1::new());
        scheduler
            .progress_registry
            .as_mut()
            .unwrap()
            .entries
            .insert(stream, cell.clone());
        scheduler.graph = Some(Box::new(PanicDropGraph));
        assert!(
            catch_unwind(AssertUnwindSafe(
                || scheduler.finish(&mut context, &mut operations)
            ))
            .is_err()
        );
        assert!(!cell.stopped.load(Ordering::Acquire));
        assert!(!scheduler.finished);
        scheduler.finish(&mut context, &mut operations);
        assert!(cell.stopped.load(Ordering::Acquire));
        assert!(scheduler.finished);
        assert!(context.cleanup().is_complete());
    }
}
