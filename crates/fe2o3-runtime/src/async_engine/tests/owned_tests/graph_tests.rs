use super::*;
mod version_tests;
use crate::async_engine::graph::EngineGraphV1;
use crate::completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, EventIdentityV1, FutureIdentityV1,
};
use crate::{
    RuntimeAccessV1, RuntimeAsyncGraphFutureV1, RuntimeBindingV1, RuntimeGraphErrorV1,
    RuntimeGraphRequestV1, RuntimeGraphValidationErrorV1, RuntimeMemoryRegionV1,
};

impl crate::RuntimeAsyncCopyBackendV1 for ThreadBoundBackend {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.record("copy_async_v1");
        self.inner
            .copy_async_v1(stream, source, destination, dependencies)
    }
}

fn id(n: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(n).unwrap()
}

struct Harness {
    context: RuntimeContextV1<MockBackend>,
    state: Arc<Mutex<MockState>>,
    handle: RuntimeAsyncProgressHandleV1<MockBackend>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<MockBackend>>,
    streams: Vec<RuntimeStreamIdV1>,
    module: crate::RuntimeModuleIdV1,
    kernel: Arc<crate::TypedRuntimeKernelV1<EmptyArgs>>,
    graph: Option<Box<dyn EngineGraphV1<MockBackend>>>,
}
impl Harness {
    fn new() -> Self {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: state.clone(),
        })
        .unwrap();
        let device = context.devices()[0].id();
        let streams = (0..4)
            .map(|_| context.create_stream(device).unwrap())
            .collect();
        let module = context.load_module(device, &[1]).unwrap();
        let kernel = Arc::new(
            context
                .resolve_kernel::<EmptyArgs>(module, "empty")
                .unwrap(),
        );
        let (sender, receiver) = sync_channel(4);
        let handle = RuntimeAsyncProgressHandleV1 {
            observer: RuntimeAsyncEngineHandleV1 {
                context_generation: context.capture_context_generation_v1(),
                capture_budget: None,
                reply_budget: reply_budget::ReplyBudgetV1::new(DEFAULT_RUNTIME_ASYNC_REPLIES_V1),
                admission: drain::AdmissionV1::new(),
                sender,
                worker_thread: Arc::new(OnceLock::new()),
                quarantine_command_panics: true,
                graph_slot: Arc::new(AtomicBool::new(false)),
                snapshot_budget: snapshot::SnapshotBudgetV1::new(
                    DEFAULT_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1,
                ),
            },
        };
        Self {
            context,
            state,
            handle,
            receiver,
            streams,
            module,
            kernel,
            graph: None,
        }
    }
    fn request(&self, diamond: bool) -> RuntimeGraphRequestV1<MockBackend> {
        let streams: Vec<_> = self
            .streams
            .iter()
            .map(|&s| self.context.completion_stream_identity_v1(s).unwrap())
            .collect();
        let ctx = streams[0].context();
        let future = |n, stream, pred: Option<u32>| {
            CompletionNodeV1::future(
                id(n),
                FutureIdentityV1::new(stream, [n as u8; 32]),
                pred.map(id),
            )
        };
        let (used, nodes) = if diamond {
            let root = EventIdentityV1::new(ctx, [1; 32]);
            let left = EventIdentityV1::new(ctx, [2; 32]);
            let right = EventIdentityV1::new(ctx, [3; 32]);
            (
                4,
                vec![
                    future(1, streams[0], None),
                    CompletionNodeV1::record_event(id(2), streams[0], root, Some(id(1))),
                    CompletionNodeV1::wait_event(id(3), streams[1], root, id(2), None),
                    future(4, streams[1], Some(3)),
                    CompletionNodeV1::record_event(id(5), streams[1], left, Some(id(4))),
                    CompletionNodeV1::wait_event(id(6), streams[2], root, id(2), None),
                    future(7, streams[2], Some(6)),
                    CompletionNodeV1::record_event(id(8), streams[2], right, Some(id(7))),
                    CompletionNodeV1::wait_event(id(9), streams[3], left, id(5), None),
                    CompletionNodeV1::wait_event(id(10), streams[3], right, id(8), Some(id(9))),
                    future(11, streams[3], Some(10)),
                ],
            )
        } else {
            (1, vec![future(1, streams[0], None)])
        };
        RuntimeGraphRequestV1::new(
            CompletionGraphV1::new(ctx, streams[..used].to_vec(), nodes).unwrap(),
            streams[..used]
                .iter()
                .copied()
                .zip(self.streams.iter().copied())
                .collect(),
        )
        .unwrap()
    }
    fn bound(&self, diamond: bool) -> RuntimeGraphRequestV1<MockBackend> {
        let mut request = self.request(diamond);
        for n in if diamond { vec![1, 4, 7, 11] } else { vec![1] } {
            request
                .bind_launch(id(n), self.kernel.clone(), &EmptyArgs, geometry())
                .unwrap();
        }
        request
    }
    fn submit(
        &mut self,
        request: RuntimeGraphRequestV1<MockBackend>,
    ) -> RuntimeAsyncGraphFutureV1<MockError> {
        let future = self.handle.submit_graph(request).unwrap();
        let RuntimeAsyncEngineCommandV1::Graph(mut graph) = self.receiver.recv().unwrap() else {
            panic!("graph command");
        };
        if graph.admit(&mut self.context) {
            self.graph = Some(graph);
        }
        future
    }
    fn tick(&mut self, budget: usize) {
        if self
            .graph
            .as_mut()
            .is_some_and(|graph| graph.advance(&mut self.context, budget, 1))
        {
            self.graph = None;
        }
    }
    fn succeed(&mut self) {
        for _ in 0..200 {
            for status in self.state.lock().unwrap().statuses.values_mut() {
                *status = BackendPollV1::Succeeded;
            }
            self.tick(8);
            if self.graph.is_none() {
                return;
            }
        }
        panic!("graph stalled");
    }
    fn issue_count(&self) -> usize {
        self.state.lock().unwrap().issues.len()
    }
    fn allocate(&mut self) -> crate::RuntimeAllocationIdV1 {
        self.context
            .allocate(
                self.context.devices()[0].id(),
                RuntimeMemoryKindV1::DeviceLocal,
                64,
                8,
            )
            .unwrap()
    }
}

fn result(
    future: RuntimeAsyncGraphFutureV1<MockError>,
) -> Result<crate::RuntimeGraphReportV1<MockError>, RuntimeGraphErrorV1<MockError>> {
    let mut future = Box::pin(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(Ok(result)) => result,
        _ => panic!("graph response unavailable"),
    }
}

#[test]
fn r63_diamond_joins_both_branch_orders_and_releases_before_successors() {
    for reversed in [false, true] {
        let mut h = Harness::new();
        let future = h.submit(h.bound(true));
        h.tick(16);
        assert_eq!(h.issue_count(), 1);
        let root = h.state.lock().unwrap().issues[0].1;
        h.state
            .lock()
            .unwrap()
            .statuses
            .insert(root, BackendPollV1::Succeeded);
        h.tick(64);
        assert_eq!(h.issue_count(), 3);
        let issues = h.state.lock().unwrap().issues.clone();
        let order = if reversed { [2, 1] } else { [1, 2] };
        h.state
            .lock()
            .unwrap()
            .statuses
            .insert(issues[order[0]].1, BackendPollV1::Succeeded);
        h.tick(64);
        assert_eq!(h.issue_count(), 3);
        h.state
            .lock()
            .unwrap()
            .statuses
            .insert(issues[order[1]].1, BackendPollV1::Succeeded);
        h.tick(64);
        assert_eq!(h.issue_count(), 4);
        assert_eq!(h.state.lock().unwrap().release_calls, 3);
        h.succeed();
        let report = result(future).unwrap();
        assert_eq!(report.observations.len(), 4);
        assert!(report.errors.is_empty());
        assert_eq!(h.state.lock().unwrap().release_calls, 4);
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn r63_full_stream_flush_roster_is_round_robin() {
    let mut h = Harness::new();
    let future = h.submit(h.bound(true));
    for _ in 0..12 {
        h.tick(1);
    }
    let state = h.state.lock().unwrap();
    assert_eq!(state.flush_calls.len(), 12);
    let roster: Vec<_> = state.flush_calls[..4].iter().map(|entry| entry.0).collect();
    assert_eq!(
        roster
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        4
    );
    for (i, call) in state.flush_calls.iter().enumerate() {
        assert_eq!(call.0, roster[i % 4]);
    }
    drop(state);
    h.succeed();
    result(future).unwrap();
}

#[test]
fn r63_reservation_blocks_mutation_and_cleanup_until_retirement() {
    let mut h = Harness::new();
    let allocation = h.allocate();
    let future = h.submit(h.bound(false));
    let reserved = |result: Result<(), RuntimeErrorV1<MockError>>| {
        assert!(matches!(
            result,
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ))
    };
    reserved(h.context.write_allocation(allocation, 0, &[1]));
    reserved(h.context.read_allocation(allocation, 0, &mut [0]));
    reserved(h.context.release_allocation(allocation));
    reserved(h.context.destroy_stream(h.streams[0]));
    reserved(h.context.unload_module(h.module));
    reserved(h.context.flush_stream(h.streams[0]));
    assert!(h.context.cleanup().is_graph_reserved());
    assert!(!h.context.cleanup().is_complete());
    assert!(matches!(
        h.context
            .launch(h.streams[0], &h.kernel, &EmptyArgs, geometry(), &[]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    h.succeed();
    result(future).unwrap();
    h.context.write_allocation(allocation, 0, &[1]).unwrap();
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r63_cancel_and_drop_do_not_withdraw_issued_work() {
    let mut h = Harness::new();
    let future = h.submit(h.bound(true));
    h.tick(1);
    future.control().cancel_unissued();
    drop(future);
    h.tick(64);
    assert_eq!(h.issue_count(), 1);
    assert!(h.context.cleanup().is_graph_reserved());
    h.succeed();
    assert_eq!(h.issue_count(), 1);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r63_cancel_before_issue_produces_no_backend_work() {
    let mut h = Harness::new();
    let future = h.submit(h.bound(true));
    future.control().cancel_unissued();
    h.tick(1);
    let report = result(future).unwrap();
    assert!(report.observations.is_empty());
    assert_eq!(h.issue_count(), 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r63_rejected_poll_and_release_keep_same_token() {
    let mut h = Harness::new();
    let future = h.submit(h.bound(false));
    h.tick(1);
    h.state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError("retry poll")));
    h.tick(1);
    h.state
        .lock()
        .unwrap()
        .release_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError(
            "retry release",
        )));
    h.succeed();
    let report = result(future).unwrap();
    assert_eq!(report.rejected_observations, 1);
    assert_eq!(report.rejected_releases, 1);
    assert_eq!(h.issue_count(), 1);
    assert_eq!(h.state.lock().unwrap().release_calls, 2);
}

#[test]
fn r63_rejected_root_never_issues_descendants() {
    let mut h = Harness::new();
    h.state
        .lock()
        .unwrap()
        .submit_failures
        .push_back(RuntimeBackendFailureV1::Rejected(MockError("not admitted")));
    let future = h.submit(h.bound(true));
    h.succeed();
    let report = result(future).unwrap();
    assert_eq!(report.errors.len(), 1);
    assert_eq!(h.issue_count(), 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r63_terminal_poll_retains_reservation_and_no_completion_report() {
    let mut h = Harness::new();
    let future = h.submit(h.bound(true));
    h.tick(1);
    h.state
        .lock()
        .unwrap()
        .poll_failures
        .push_back(RuntimeBackendFailureV1::Terminal(MockError("unknown")));
    h.tick(32);
    assert!(matches!(
        result(future),
        Err(RuntimeGraphErrorV1::Context(
            RuntimeErrorV1::BackendTerminal(_)
        ))
    ));
    let cleanup = h.context.cleanup();
    assert!(cleanup.is_terminal());
    assert!(cleanup.is_graph_reserved());
    assert_eq!(cleanup.retained().submissions, 1);
    assert_eq!(h.issue_count(), 1);
    assert_eq!(h.state.lock().unwrap().release_calls, 0);
}

#[test]
fn r63_preexisting_work_rejects_without_reservation() {
    let mut h = Harness::new();
    let submission = h
        .context
        .launch(h.streams[0], &h.kernel, &EmptyArgs, geometry(), &[])
        .unwrap();
    let future = h.submit(h.bound(false));
    assert!(matches!(
        result(future),
        Err(RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        )))
    ));
    assert!(!h.context.cleanup().is_graph_reserved());
    assert!(h.context.query_submission(&submission).is_err());
}

#[test]
fn r63_one_queued_graph_per_engine_and_failed_admission_releases_slot() {
    let mut h = Harness::new();
    let request = h.bound(false);
    let first = h.handle.submit_graph(request).unwrap();
    assert!(matches!(
        h.handle.submit_graph(h.bound(false)),
        Err(RuntimeAsyncEngineCallErrorV1::GraphCapacity)
    ));
    drop(h.receiver.recv().unwrap());
    assert!(matches!(
        Box::pin(first)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop())),
        Poll::Ready(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped))
    ));
    let invalid = h.submit(h.request(false));
    assert!(matches!(
        result(invalid),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::MissingOperation
        ))
    ));
    let future = h.submit(h.bound(false));
    h.succeed();
    result(future).unwrap();
}

struct MutableArgs {
    value: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
    region: RuntimeMemoryRegionV1,
}
impl crate::RuntimeArgumentsV1 for MutableArgs {
    const SIGNATURE_V1: [u8; 32] = [91; 32];
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        self.calls.fetch_add(1, AtomicOrdering::SeqCst);
        let mut bytes = vec![0; 16];
        bytes[8..].copy_from_slice(&(self.value.load(AtomicOrdering::SeqCst) as u64).to_le_bytes());
        bytes
    }
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.calls.fetch_add(1, AtomicOrdering::SeqCst);
        vec![RuntimeBindingV1 {
            region: self.region,
            kernarg_byte_offset: 0,
        }]
    }
}
fn args(allocation: crate::RuntimeAllocationIdV1) -> MutableArgs {
    MutableArgs {
        value: Arc::new(AtomicUsize::new(37)),
        calls: Arc::new(AtomicUsize::new(0)),
        region: RuntimeMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Write,
            byte_offset: 0,
            byte_len: 8,
        },
    }
}

#[test]
fn r63_frozen_arguments_are_encoded_once_and_retain_exact_bindings() {
    let mut h = Harness::new();
    let args = args(h.allocate());
    let kernel = Arc::new(
        h.context
            .resolve_kernel::<MutableArgs>(h.module, "mutable")
            .unwrap(),
    );
    let mut request = h.request(false);
    request
        .bind_launch(id(1), kernel, &args, geometry())
        .unwrap();
    args.value.store(99, AtomicOrdering::SeqCst);
    let future = h.submit(request);
    h.succeed();
    result(future).unwrap();
    assert_eq!(args.calls.load(AtomicOrdering::SeqCst), 2);
    let state = h.state.lock().unwrap();
    assert_eq!(&state.issues[0].2[8..], &37u64.to_le_bytes());
    assert_eq!(state.issues[0].3.len(), 1);
    assert_eq!(state.issues[0].3[0].region.byte_len, 8);
}

#[test]
fn r63_unordered_writes_reject_before_any_publication() {
    let mut h = Harness::new();
    let args = args(h.allocate());
    let kernel = Arc::new(
        h.context
            .resolve_kernel::<MutableArgs>(h.module, "mutable")
            .unwrap(),
    );
    let mut request = h.request(true);
    for n in [1, 4, 7, 11] {
        request
            .bind_launch(id(n), kernel.clone(), &args, geometry())
            .unwrap();
    }
    let future = h.submit(request);
    assert!(matches!(
        result(future),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::UnorderedMemoryConflict { .. }
        ))
    ));
    assert_eq!(h.issue_count(), 0);
    assert!(!h.context.cleanup().is_graph_reserved());
}

#[test]
fn r63_repeated_graphs_retire_all_submission_records() {
    let mut h = Harness::new();
    for _ in 0..128 {
        let future = h.submit(h.bound(true));
        h.succeed();
        result(future).unwrap();
        for &stream in &h.streams {
            assert_eq!(h.context.query_stream(stream).unwrap().total_submissions, 0);
        }
    }
    assert_eq!(h.issue_count(), 512);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r64_repeated_graph_reports_identify_distinct_admitted_occurrences() {
    let mut h = Harness::new();
    let mut prior = None;
    let mut first = None;
    for _ in 0..16 {
        let future = h.submit(h.bound(false));
        h.succeed();
        let report = result(future).unwrap();
        assert_eq!(
            report.execution.graph_identity(),
            report.completion.graph_identity()
        );
        assert_eq!(
            report.execution.context(),
            h.context
                .completion_stream_identity_v1(h.streams[0])
                .unwrap()
                .context()
        );
        if let Some(previous) = prior {
            let previous: crate::RuntimeGraphExecutionIdentityV1 = previous;
            assert_eq!(previous.graph_identity(), report.execution.graph_identity());
            assert_eq!(previous.context(), report.execution.context());
            assert!(previous.generation() < report.execution.generation());
            assert_ne!(previous, report.execution);
        }
        first.get_or_insert(report.execution);
        prior = Some(report.execution);
    }
    assert_ne!(first, prior);
    let mut other = Harness::new();
    let future = other.submit(other.bound(false));
    other.succeed();
    let report = result(future).unwrap();
    assert_ne!(first.unwrap().context(), report.execution.context());
    assert_ne!(first.unwrap(), report.execution);
}

#[test]
fn r63_mixed_copy_diamond_preserves_regions_without_native_events() {
    let mut h = Harness::new();
    let source = args(h.allocate()).region;
    let destination = args(h.allocate()).region;
    let source = RuntimeMemoryRegionV1 {
        access: RuntimeAccessV1::Read,
        byte_offset: 8,
        ..source
    };
    let destination = RuntimeMemoryRegionV1 {
        byte_offset: 24,
        ..destination
    };
    let mut request = h.request(true);
    request.bind_copy(id(4), source, destination).unwrap();
    for n in [1, 7, 11] {
        request
            .bind_launch(id(n), h.kernel.clone(), &EmptyArgs, geometry())
            .unwrap();
    }
    let future = h.submit(request);
    h.succeed();
    let report = result(future).unwrap();
    assert!(
        report
            .completion
            .entries()
            .iter()
            .all(|e| e.state() == crate::completion::CompletionNodeStateV1::Succeeded)
    );
    let state = h.state.lock().unwrap();
    assert_eq!(state.copy_issues.len(), 1);
    assert_eq!(state.copy_issues[0].1.byte_offset, 8);
    assert_eq!(state.copy_issues[0].2.byte_offset, 24);
    assert_eq!(state.copy_issues[0].1.byte_len, 8);
    assert_eq!(state.copy_issues[0].2.byte_len, 8);
    assert_ne!(
        state.copy_issues[0].1.allocation,
        state.copy_issues[0].2.allocation
    );
    assert!(state.copy_issues[0].3.is_empty());
}

#[test]
fn r63_failed_or_quiescent_branch_drains_pending_sibling() {
    for quiescent in [false, true] {
        let mut h = Harness::new();
        let mut future = Box::pin(h.submit(h.bound(true)));
        h.tick(1);
        for status in h.state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
        h.tick(64);
        assert_eq!(h.issue_count(), 3);
        if quiescent {
            h.state
                .lock()
                .unwrap()
                .poll_failures
                .push_back(RuntimeBackendFailureV1::Quiescent(MockError(
                    "quiescent failure",
                )));
        } else {
            let branch = h.state.lock().unwrap().issues[1].1;
            h.state
                .lock()
                .unwrap()
                .statuses
                .insert(branch, BackendPollV1::Failed { code: 17 });
        }
        h.tick(64);
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        assert_eq!(h.issue_count(), 3);
        assert!(h.context.cleanup().is_graph_reserved());
        h.succeed();
        let Poll::Ready(Ok(Ok(report))) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        else {
            panic!("terminal graph");
        };
        assert_eq!(report.observations.len(), 3);
        assert!(report.completion.entries().iter().any(|e| matches!(
            e.state(),
            crate::completion::CompletionNodeStateV1::DependencyFailed { .. }
        )));
        assert_eq!(h.issue_count(), 3);
        assert!(h.context.cleanup().is_complete());
    }
}

struct EnqueueOnWake {
    handle: RuntimeAsyncProgressHandleV1<MockBackend>,
    request: Mutex<Option<RuntimeGraphRequestV1<MockBackend>>>,
    successor: Mutex<Option<RuntimeAsyncGraphFutureV1<MockError>>>,
    accepted: AtomicBool,
}
impl std::task::Wake for EnqueueOnWake {
    fn wake(self: Arc<Self>) {
        if let Some(request) = self.request.lock().unwrap().take()
            && let Ok(future) = self.handle.submit_graph(request)
        {
            *self.successor.lock().unwrap() = Some(future);
            self.accepted.store(true, Ordering::Release);
        }
    }
}

#[test]
fn r63_reply_releases_slot_before_wake_without_clearing_successor_slot() {
    let mut h = Harness::new();
    let mut future = Box::pin(h.submit(h.bound(false)));
    let wake = Arc::new(EnqueueOnWake {
        handle: h.handle.clone(),
        request: Mutex::new(Some(h.bound(false))),
        successor: Mutex::new(None),
        accepted: AtomicBool::new(false),
    });
    let waker = Waker::from(wake.clone());
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(&waker))
            .is_pending()
    );
    h.succeed();
    assert!(wake.accepted.load(Ordering::Acquire));
    assert!(matches!(
        h.handle.submit_graph(h.bound(false)),
        Err(RuntimeAsyncEngineCallErrorV1::GraphCapacity)
    ));
    let RuntimeAsyncEngineCommandV1::Graph(mut graph) = h.receiver.recv().unwrap() else {
        panic!("successor command");
    };
    assert!(graph.admit(&mut h.context));
    h.graph = Some(graph);
    h.succeed();
    result(wake.successor.lock().unwrap().take().unwrap()).unwrap();
    assert_eq!(h.issue_count(), 2);
}

fn owner_request(
    context: &mut RuntimeContextV1<ThreadBoundBackend>,
) -> RuntimeGraphRequestV1<ThreadBoundBackend> {
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let identity = context.completion_stream_identity_v1(stream).unwrap();
    let module = context.load_module(device, &[1]).unwrap();
    let kernel = Arc::new(
        context
            .resolve_kernel::<EmptyArgs>(module, "empty")
            .unwrap(),
    );
    let graph = CompletionGraphV1::new(
        identity.context(),
        vec![identity],
        vec![CompletionNodeV1::future(
            id(1),
            FutureIdentityV1::new(identity, [1; 32]),
            None,
        )],
    )
    .unwrap();
    let mut request = RuntimeGraphRequestV1::new(graph, vec![(identity, stream)]).unwrap();
    request
        .bind_launch(id(1), kernel, &EmptyArgs, geometry())
        .unwrap();
    request
}

#[test]
fn r63_real_owner_loop_completes_and_cleans_up() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let trace = Arc::new(Mutex::new(OwnerTrace::default()));
    let (engine, handle) = start(state.clone(), trace.clone());
    let request = join_command(
        handle
            .observer()
            .enqueue_with_context(owner_request)
            .unwrap(),
    )
    .unwrap();
    let mut future = Box::pin(handle.submit_graph(request).unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        for status in state.lock().unwrap().statuses.values_mut() {
            *status = BackendPollV1::Succeeded;
        }
        if let Poll::Ready(report) = future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            assert_eq!(report.unwrap().unwrap().observations.len(), 1);
            break;
        }
        assert!(Instant::now() < deadline);
        thread::yield_now();
    }
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
    assert!(
        trace
            .lock()
            .unwrap()
            .calls
            .iter()
            .any(|call| call.0 == "release_submission_v1")
    );
}

#[test]
fn r63_owner_stop_and_backend_panic_preserve_custody() {
    for panic_backend in [false, true] {
        let state = Arc::new(Mutex::new(MockState::default()));
        let trace = Arc::new(Mutex::new(OwnerTrace::default()));
        let (engine, handle) = start(state.clone(), trace.clone());
        let request = join_command(
            handle
                .observer()
                .enqueue_with_context(owner_request)
                .unwrap(),
        )
        .unwrap();
        let future = handle.submit_graph(request).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.lock().unwrap().issues.is_empty() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        if panic_backend {
            trace.lock().unwrap().flush_panics = true;
            let mut future = Box::pin(future);
            loop {
                if let Poll::Ready(result) = future
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                {
                    assert!(matches!(
                        result,
                        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
                    ));
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::yield_now();
            }
        } else {
            drop(future);
        }
        let report = engine.shutdown().unwrap();
        assert_eq!(
            report.disposition,
            RuntimeAsyncOwnedDispositionV1::RetainedUntilProcessExit
        );
        let cleanup = report.cleanup.unwrap();
        assert!(cleanup.is_graph_reserved());
        assert_eq!(cleanup.retained().submissions, 1);
        assert!(
            !trace
                .lock()
                .unwrap()
                .calls
                .iter()
                .any(|call| call.0 == "finalize")
        );
    }
}

#[test]
fn r63_hazards_allow_ordered_writes_disjoint_windows_and_shared_reads() {
    for mode in 0..3 {
        let mut h = Harness::new();
        let allocation = h.allocate();
        let kernel = Arc::new(
            h.context
                .resolve_kernel::<MutableArgs>(h.module, "mutable")
                .unwrap(),
        );
        let mut request = h.request(true);
        for n in [1, 4, 7, 11] {
            let uses_memory = if mode == 0 {
                n == 1 || n == 11
            } else {
                n == 4 || n == 7
            };
            if uses_memory {
                let mut arguments = args(allocation);
                if mode == 1 && n == 7 {
                    arguments.region.byte_offset = 16;
                }
                if mode == 2 {
                    arguments.region.access = RuntimeAccessV1::Read;
                }
                request
                    .bind_launch(id(n), kernel.clone(), &arguments, geometry())
                    .unwrap();
            } else {
                request
                    .bind_launch(id(n), h.kernel.clone(), &EmptyArgs, geometry())
                    .unwrap();
            }
        }
        let future = h.submit(request);
        h.succeed();
        assert_eq!(result(future).unwrap().observations.len(), 4);
        assert!(h.context.cleanup().is_complete());
    }
}

#[test]
fn r63_graph_node_bound_and_foreign_context_are_checked_before_issue() {
    let mut h = Harness::new();
    let stream = h
        .context
        .completion_stream_identity_v1(h.streams[0])
        .unwrap();
    for count in [256, 257] {
        let nodes = (1..=count)
            .map(|n: u32| {
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&n.to_le_bytes());
                CompletionNodeV1::future(
                    id(n),
                    FutureIdentityV1::new(stream, bytes),
                    (n > 1).then(|| id(n - 1)),
                )
            })
            .collect();
        let graph = CompletionGraphV1::new(stream.context(), vec![stream], nodes).unwrap();
        let request =
            RuntimeGraphRequestV1::<MockBackend>::new(graph, vec![(stream, h.streams[0])]);
        assert_eq!(request.is_ok(), count == 256);
    }
    let other = Harness::new();
    let future = h.submit(other.bound(false));
    assert!(matches!(
        result(future),
        Err(RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        )))
    ));
    assert_eq!(h.issue_count(), 0);
    assert!(!h.context.cleanup().is_graph_reserved());
}
