use super::*;
use crate::{
    BackendDirectedScalarPeerCopyV1, BackendDirectedScalarProgressV1, RuntimeAccessV1,
    RuntimeDirectedScalarPeerCopyBackendV1, RuntimeDirectedScalarPeerCopyV1, RuntimeMemoryRegionV1,
};

type Submission = crate::RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;
type CopyFuture = RuntimeAsyncOperationFutureV1<RuntimeDirectedScalarPeerCopyV1, MockError>;

mod bounded_settlement;
mod early_event;
mod lifecycle;
mod scheduling;

impl RuntimeDirectedScalarPeerCopyBackendV1 for MockBackend {
    fn submit_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let panics = self.state.lock().unwrap().panic_on_submit;
        assert!(!panics, "directed submit panic");
        if let Some(error) = self.state.lock().unwrap().submit_failures.pop_front() {
            return Err(error);
        }
        let id = self.next();
        let mut state = self.state.lock().unwrap();
        for dependency in request.dependencies {
            assert_eq!(
                state.event_sources[&dependency.event],
                dependency.producer_submission
            );
        }
        state
            .directed_routes
            .insert(id, (request.route, request.dependencies.to_vec()));
        state.directed_calls.push(("submit", id));
        state.statuses.insert(id, BackendPollV1::Pending);
        Ok(id)
    }

    fn progress_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarProgressV1<'_>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        let panics = {
            let mut state = self.state.lock().unwrap();
            let (route, dependencies) = &state.directed_routes[&request.submission];
            assert_eq!(*route, request.route);
            assert_eq!(
                dependencies
                    .iter()
                    .map(|dep| dep.producer_submission)
                    .collect::<Vec<_>>(),
                request.producer_submissions
            );
            state.directed_calls.push(("progress", request.submission));
            state.directed_panic
        };
        assert!(!panics, "directed progress panic");
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.poll_failures.pop_front() {
            return Err(error);
        }
        Ok(state.statuses[&request.submission])
    }
}

impl RuntimeDirectedScalarPeerCopyBackendV1 for ThreadBoundBackend {
    fn submit_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.record("directed_submit");
        self.inner.submit_directed_scalar_peer_copy_v1(request)
    }

    fn progress_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarProgressV1<'_>,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.record("directed_progress");
        self.inner.progress_directed_scalar_peer_copy_v1(request)
    }
}

struct Fixture {
    h: Harness,
    streams: [RuntimeStreamIdV1; 2],
    allocations: Vec<crate::RuntimeAllocationIdV1>,
}

impl Fixture {
    fn new(journal: bool) -> Self {
        let mut h = Harness::with_journal(4096, 8, true, journal);
        let streams = [
            h.stream,
            h.context
                .create_stream(h.context.devices()[1].id())
                .unwrap(),
        ];
        let allocations = (0..6)
            .map(|index| {
                h.context
                    .allocate(
                        h.context.devices()[index % 2].id(),
                        RuntimeMemoryKindV1::DeviceLocal,
                        16,
                        8,
                    )
                    .unwrap()
            })
            .collect();
        Self {
            h,
            streams,
            allocations,
        }
    }

    fn region(&self, index: usize, write: bool) -> RuntimeMemoryRegionV1 {
        RuntimeMemoryRegionV1 {
            allocation: self.allocations[index],
            byte_offset: 0,
            byte_len: 8,
            access: if write {
                RuntimeAccessV1::Write
            } else {
                RuntimeAccessV1::Read
            },
        }
    }

    fn producer(
        &mut self,
        source: usize,
        destination: usize,
        dependencies: &[RuntimeEventIdV1],
    ) -> (Submission, RuntimeEventIdV1, u64) {
        let submission = self
            .h
            .context
            .directed_peer_copy_v1(
                self.streams[destination % 2],
                self.region(source, false),
                self.region(destination, true),
                dependencies,
            )
            .unwrap();
        let event = self.h.context.record_event(&submission).unwrap();
        let id = self.last_id();
        (submission, event, id)
    }

    fn enqueue(
        &self,
        source: usize,
        destination: usize,
        dependencies: Vec<RuntimeEventIdV1>,
        tracked: bool,
    ) -> (CopyFuture, Option<RuntimeAsyncOperationControlV1>) {
        if tracked {
            let operation = self
                .h
                .handle
                .directed_peer_copy_tracked(
                    self.streams[destination % 2],
                    self.region(source, false),
                    self.region(destination, true),
                    dependencies,
                )
                .unwrap();
            (operation.future, Some(operation.control))
        } else {
            (
                self.h
                    .handle
                    .directed_peer_copy(
                        self.streams[destination % 2],
                        self.region(source, false),
                        self.region(destination, true),
                        dependencies,
                    )
                    .unwrap(),
                None,
            )
        }
    }

    fn last_id(&self) -> u64 {
        self.h
            .state
            .lock()
            .unwrap()
            .directed_calls
            .iter()
            .rev()
            .find(|(kind, _)| *kind == "submit")
            .unwrap()
            .1
    }

    fn complete(&self) {
        for status in self.h.state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
    }

    fn tick(&mut self, registry: &mut operation::OperationRegistryV1<MockBackend>) {
        operation::advance_operations_v1(&mut self.h.context, registry, 8, 8, |context, stream| {
            context.flush_stream(stream)
        });
    }
}

fn pending<F: Future + Unpin>(future: &mut F) -> bool {
    Pin::new(future)
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending()
}

#[test]
fn directed_async_consumer_reconciles_released_event_without_implicit_flush() {
    for journal in [false, true] {
        for tracked in [false, true] {
            let mut f = Fixture::new(journal);
            let (producer, event, producer_id) = f.producer(0, 1, &[]);
            let (mut future, control) = f.enqueue(1, 2, vec![event], tracked);
            assert!(pending(&mut future));
            let mut factory = f.h.pop_factory();
            assert!(!factory.requires_owned_shutdown());
            let driver = factory.materialize();
            assert_eq!(driver.stream(), None);
            let mut registry = operation::OperationRegistryV1::new(4, false);
            assert!(registry.accepts_factory(&*factory));
            registry.insert(driver);
            f.tick(&mut registry);
            let consumer_id = f.last_id();
            assert_eq!(
                f.h.state.lock().unwrap().directed_calls,
                [("submit", producer_id), ("submit", consumer_id)]
            );
            assert_eq!(f.h.used(), 0);
            f.h.context.release_event(event).unwrap();
            f.complete();
            f.tick(&mut registry);
            assert!(pending(&mut future));
            assert_eq!(registry.len(), 1);
            assert_eq!(
                f.h.context.query_submission(&producer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            if let Some(control) = &control {
                assert_eq!(
                    control.cancel_before_submission(),
                    RuntimeAsyncCancelResultV1::NotCancellable(
                        RuntimeAsyncOperationPhaseV1::Observing
                    )
                );
            }
            f.tick(&mut registry);
            assert_eq!(registry.len(), 0);
            let result = join_command(future).unwrap();
            assert_eq!(
                result.observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            let consumer = result.submission.unwrap();
            assert_eq!(consumer.stream(), f.streams[0]);
            assert_ne!(consumer.id(), producer.id());
            assert_eq!(
                f.h.context.query_submission(&consumer).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            f.h.context.record_event(&consumer).unwrap();
            {
                let state = f.h.state.lock().unwrap();
                assert_eq!(state.event_sources[&state.next], consumer_id);
            }
            assert_eq!(
                f.h.context.query_submission(&producer).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            let state = f.h.state.lock().unwrap();
            assert_eq!(
                state.directed_calls,
                [
                    ("submit", producer_id),
                    ("submit", consumer_id),
                    ("progress", consumer_id),
                    ("poll", producer_id)
                ]
            );
            assert!(state.flush_calls.is_empty());
            assert_eq!(state.release_calls, 0);
            drop(state);
            assert_eq!(
                f.h.context.version_journal_read_records_v1(),
                journal.then_some(0)
            );
            assert!(f.h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_async_diamond_has_one_action_per_advance_and_one_producer_callback() {
    let mut f = Fixture::new(true);
    let (root, root_event, root_id) = f.producer(0, 1, &[]);
    let (left, left_event, left_id) = f.producer(1, 2, &[root_event]);
    let (right, right_event, right_id) = f.producer(1, 4, &[root_event]);
    let completed = Arc::new(Mutex::new(Vec::new()));
    for (submission, id) in [(&root, root_id), (&left, left_id), (&right, right_id)] {
        let callbacks = completed.clone();
        f.h.context
            .on_completion(submission, move |status| {
                assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
                callbacks.lock().unwrap().push(id);
            })
            .unwrap();
    }
    let (future, _) = f.enqueue(2, 3, vec![left_event, right_event], true);
    let mut driver = f.h.pop();
    assert!(!driver.advance(&mut f.h.context));
    let consumer_id = f.last_id();
    for event in [root_event, left_event, right_event] {
        f.h.context.release_event(event).unwrap();
    }
    f.complete();
    for index in 0..4 {
        let before = f.h.state.lock().unwrap().directed_calls.len();
        assert_eq!(driver.advance(&mut f.h.context), index == 3);
        assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), before + 1);
    }
    assert_eq!(
        join_command(future).unwrap().observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(*completed.lock().unwrap(), [root_id, left_id, right_id]);
    let state = f.h.state.lock().unwrap();
    assert_eq!(
        &state.directed_calls[4..],
        [
            ("progress", consumer_id),
            ("poll", left_id),
            ("poll", root_id),
            ("poll", right_id)
        ]
    );
    assert!(state.flush_calls.is_empty());
    drop(state);
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn directed_async_live_rejections_retry_but_sealed_producer_rejection_preserves_diagnostic() {
    for contradiction in [false, true] {
        let mut f = Fixture::new(true);
        let (_, event, _) = f.producer(0, 1, &[]);
        let (future, _) = f.enqueue(1, 2, vec![event], true);
        let mut registry = operation::OperationRegistryV1::new(4, true);
        registry.insert(f.h.pop());
        f.tick(&mut registry);
        if contradiction {
            f.complete();
            f.tick(&mut registry);
        }
        f.h.state
            .lock()
            .unwrap()
            .poll_failures
            .push_back(RuntimeBackendFailureV1::Rejected(MockError(
                "exact rejection",
            )));
        f.tick(&mut registry);
        assert_eq!(f.h.context.is_terminal(), contradiction);
        assert_eq!(registry.len(), 1);
        if contradiction {
            assert!(!registry.stop_observations());
            let result = join_command(future).unwrap();
            assert!(matches!(
                result.observation,
                Err(RuntimeErrorV1::BackendRejected(MockError(
                    "exact rejection"
                )))
            ));
            assert!(result.submission.is_some());
            assert_eq!(result.rejected_observations, 0);
            assert_eq!(f.h.context.version_journal_read_records_v1(), Some(2));
            assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
        } else {
            f.complete();
            f.tick(&mut registry);
            f.tick(&mut registry);
            let result = join_command(future).unwrap();
            assert_eq!(
                result.observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(result.rejected_observations, 1);
            assert_eq!(
                result.last_rejected_observation,
                Some(MockError("exact rejection"))
            );
            assert_eq!(registry.len(), 0);
            assert!(f.h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_async_failure_and_quiescence_do_not_wait_for_pending_producer() {
    for quiescent in [false, true] {
        let mut f = Fixture::new(true);
        let (producer, event, _) = f.producer(0, 1, &[]);
        let (future, _) = f.enqueue(1, 2, vec![event], false);
        let mut driver = f.h.pop();
        assert!(!driver.advance(&mut f.h.context));
        if quiescent {
            f.h.state
                .lock()
                .unwrap()
                .poll_failures
                .push_back(RuntimeBackendFailureV1::Quiescent(MockError("quiescent")));
        } else {
            let consumer = f.last_id();
            f.h.state
                .lock()
                .unwrap()
                .statuses
                .insert(consumer, BackendPollV1::Failed { code: 17 });
        }
        assert!(driver.advance(&mut f.h.context));
        let result = join_command(future).unwrap();
        if quiescent {
            assert!(matches!(
                result.observation,
                Err(RuntimeErrorV1::BackendQuiescent(MockError("quiescent")))
            ));
        } else {
            assert_eq!(
                result.observation.unwrap(),
                RuntimeCompletionStatusV1::Failed(crate::RuntimeCompletionFailureV1::BackendCode(
                    17
                ))
            );
        }
        assert_eq!(
            f.h.context.query_submission(&producer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(f.h.context.version_journal_read_records_v1(), Some(1));
        f.complete();
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn directed_async_cancel_and_observer_drop_do_not_release_accepted_custody() {
    for phase in 0..3 {
        let mut f = Fixture::new(true);
        let (_, event, _) = f.producer(0, 1, &[]);
        let (future, control) = f.enqueue(1, 2, vec![event], true);
        let control = control.unwrap();
        if phase == 0 {
            assert_eq!(
                control.cancel_before_submission(),
                RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
            );
        }
        let mut driver = f.h.pop();
        drop(future);
        assert_ne!(f.h.used(), 0);
        if phase == 1 {
            assert_eq!(
                control.cancel_before_submission(),
                RuntimeAsyncCancelResultV1::CancelledBeforeSubmission
            );
        }
        assert_eq!(driver.advance(&mut f.h.context), phase != 2);
        assert_eq!(f.h.used(), 0);
        if phase == 2 {
            assert_eq!(
                control.cancel_before_submission(),
                RuntimeAsyncCancelResultV1::NotCancellable(RuntimeAsyncOperationPhaseV1::Observing)
            );
            f.complete();
            assert!(!driver.advance(&mut f.h.context));
            assert!(driver.advance(&mut f.h.context));
        }
        assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
        f.complete();
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn directed_async_terminal_and_panic_stop_keep_context_roots() {
    for panics in [false, true] {
        let mut f = Fixture::new(true);
        let (_, event, _) = f.producer(0, 1, &[]);
        let (future, _) = f.enqueue(1, 2, vec![event], false);
        let mut registry = operation::OperationRegistryV1::new(4, true);
        registry.insert(f.h.pop());
        f.tick(&mut registry);
        if panics {
            f.h.state.lock().unwrap().directed_panic = true;
        } else {
            f.h.state
                .lock()
                .unwrap()
                .poll_failures
                .push_back(RuntimeBackendFailureV1::Terminal(MockError("terminal")));
        }
        f.tick(&mut registry);
        assert!(f.h.context.is_terminal());
        assert_eq!(registry.len(), 1);
        if panics {
            assert!(matches!(
                join_command(future),
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            ));
        } else {
            assert!(matches!(
                join_command(future).unwrap().observation,
                Err(RuntimeErrorV1::BackendTerminal(MockError("terminal")))
            ));
        }
        assert!(!registry.stop_observations());
        assert_eq!(f.h.context.version_journal_read_records_v1(), Some(2));
        assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
        assert_eq!(f.h.state.lock().unwrap().directed_calls.len(), 3);
    }
}

#[test]
fn directed_async_mixed_registry_flushes_only_ordinary_stream_identities() {
    for same_stream in [false, true] {
        for ordinary_first in [false, true] {
            let mut f = Fixture::new(false);
            let (directed, _) = f.enqueue(0, 1, vec![], false);
            let directed_driver = f.h.pop();
            let stream = f.streams[usize::from(same_stream)];
            let device = f.h.context.devices()[usize::from(same_stream)].id();
            let module = f.h.context.load_module(device, &[1]).unwrap();
            let kernel = Arc::new(
                f.h.context
                    .resolve_kernel::<EmptyArgs>(module, "empty")
                    .unwrap(),
            );
            let ordinary =
                f.h.handle
                    .launch(stream, kernel, EmptyArgs, geometry(), vec![])
                    .unwrap();
            let ordinary_driver = f.h.pop();
            let mut registry = operation::OperationRegistryV1::new(4, false);
            for driver in if ordinary_first {
                [ordinary_driver, directed_driver]
            } else {
                [directed_driver, ordinary_driver]
            } {
                registry.insert(driver);
            }
            f.tick(&mut registry);
            let ordinary_id = f.h.state.lock().unwrap().issues[0].1;
            let backend_stream = f.h.state.lock().unwrap().issues[0].0;
            assert_eq!(
                f.h.state
                    .lock()
                    .unwrap()
                    .flush_calls
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>(),
                [backend_stream]
            );
            f.h.state
                .lock()
                .unwrap()
                .statuses
                .insert(ordinary_id, BackendPollV1::Succeeded);
            f.tick(&mut registry);
            assert_eq!(registry.len(), 1);
            f.tick(&mut registry);
            assert_eq!(f.h.state.lock().unwrap().flush_calls.len(), 1);
            assert_eq!(
                join_command(ordinary).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            f.complete();
            f.tick(&mut registry);
            assert_eq!(registry.len(), 0);
            assert_eq!(
                join_command(directed).unwrap().observation.unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert!(f.h.context.cleanup().is_complete());
        }
    }
}

#[test]
fn directed_async_snapshot_credit_and_factory_disposal_are_exact() {
    for tracked in [false, true] {
        let mut f = Fixture::new(true);
        let (_, event, _) = f.producer(0, 1, &[]);
        let mut dependencies = Vec::with_capacity(262_144);
        dependencies.push(event);
        let (future, _) = f.enqueue(1, 2, dependencies, tracked);
        assert_eq!(f.h.used(), core::mem::size_of::<RuntimeEventIdV1>());
        drop(future);
        let mut factory = f.h.pop_factory();
        factory.reject(RuntimeAsyncEngineCallErrorV1::OperationCapacity);
        assert_eq!(f.h.used(), core::mem::size_of::<RuntimeEventIdV1>());
        drop(factory);
        assert_eq!(f.h.used(), 0);
        assert_eq!(f.h.handle.observer().reply_cells_in_use(), 0);
        f.complete();
        assert!(f.h.context.cleanup().is_complete());
    }
}
