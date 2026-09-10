use super::*;
use crate::{RuntimeAsyncLaunchRequestV1, RuntimeBindingV1};
use std::sync::atomic::AtomicUsize;

#[derive(Clone)]
struct Args {
    encodes: Arc<AtomicUsize>,
    bindings: Arc<AtomicUsize>,
    length: usize,
    effects: Vec<RuntimeBindingV1>,
}
impl RuntimeArgumentsV1 for Args {
    const SIGNATURE_V1: [u8; 32] = [9; 32];
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        let n = self.encodes.fetch_add(1, Ordering::SeqCst) as u8;
        let mut bytes = Vec::with_capacity(self.length + 4096);
        bytes.resize(self.length, n);
        bytes
    }
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.bindings.fetch_add(1, Ordering::SeqCst);
        self.effects.clone()
    }
}

struct Harness {
    context: RuntimeContextV1<MockBackend>,
    state: Arc<Mutex<MockState>>,
    handle: RuntimeAsyncProgressHandleV1<MockBackend>,
    receiver: Receiver<RuntimeAsyncEngineCommandV1<MockBackend>>,
    stream: RuntimeStreamIdV1,
    kernel: Arc<crate::TypedRuntimeKernelV1<Args>>,
    args: Args,
}
impl Harness {
    fn new(budget: usize, channel: usize) -> Self {
        let state = Arc::new(Mutex::new(MockState::default()));
        let mut context = RuntimeContextV1::open(MockBackend {
            state: state.clone(),
        })
        .unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let module = context.load_module(device, &[1]).unwrap();
        let kernel = Arc::new(context.resolve_kernel::<Args>(module, "args").unwrap());
        let (sender, receiver) = sync_channel(channel);
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
                snapshot_budget: snapshot::SnapshotBudgetV1::new(budget),
            },
        };
        Self {
            context,
            state,
            handle,
            receiver,
            stream,
            kernel,
            args: Args {
                encodes: Arc::new(AtomicUsize::new(0)),
                bindings: Arc::new(AtomicUsize::new(0)),
                length: 8,
                effects: Vec::new(),
            },
        }
    }
    fn request(&self) -> RuntimeAsyncLaunchRequestV1<Args> {
        RuntimeAsyncLaunchRequestV1::new(
            self.stream,
            self.kernel.clone(),
            &self.args,
            geometry(),
            Vec::new(),
        )
        .unwrap()
    }
    fn pop(&self) -> Box<dyn operation::EngineOperationV1<MockBackend>> {
        let RuntimeAsyncEngineCommandV1::Operation(operation) = self.receiver.try_recv().unwrap()
        else {
            panic!("operation")
        };
        operation
    }
    fn used(&self) -> usize {
        self.handle.observer().snapshot_bytes_in_use()
    }
}

#[test]
fn r64_snapshot_budget_survives_observer_drop_and_cancel_until_disposal() {
    let mut h = Harness::new(8, 2);
    let future = h.handle.enqueue_launch(h.request()).unwrap();
    drop(future);
    assert_eq!(h.used(), 8);
    assert!(matches!(
        h.handle.enqueue_launch(h.request()),
        Err(RuntimeAsyncEngineCallErrorV1::SnapshotCapacity)
    ));
    drop(h.pop());
    assert_eq!(h.used(), 0);
    let tracked = h.handle.enqueue_launch_tracked(h.request()).unwrap();
    let control = tracked.control();
    control.cancel_before_submission();
    assert_eq!(h.used(), 8);
    assert!(h.pop().advance(&mut h.context));
    assert_eq!(h.used(), 0);
    assert!(h.state.lock().unwrap().issues.is_empty());
    assert!(matches!(
        join_command(tracked.future),
        Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
    ));
}

#[test]
fn r64_snapshot_channel_rejection_and_shutdown_disposal_refund_exactly() {
    let h = Harness::new(16, 1);
    h.handle
        .observer
        .sender
        .try_send(RuntimeAsyncEngineCommandV1::Stop)
        .unwrap_or_else(|_| panic!("empty channel"));
    assert!(matches!(
        h.handle.enqueue_launch(h.request()),
        Err(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
    ));
    assert_eq!(h.used(), 0);
    drop(h.receiver.try_recv().unwrap());
    let future = h.handle.enqueue_launch(h.request()).unwrap();
    assert_eq!(h.used(), 8);
    drop(h.receiver);
    assert_eq!(h.handle.observer().snapshot_bytes_in_use(), 0);
    assert!(matches!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    let request =
        RuntimeAsyncLaunchRequestV1::new(h.stream, h.kernel, &h.args, geometry(), Vec::new())
            .unwrap();
    assert!(matches!(
        h.handle.enqueue_launch(request),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ));
    assert_eq!(h.handle.observer().snapshot_bytes_in_use(), 0);
}

#[test]
fn r64_snapshot_freezes_once_and_releases_payload_not_native_custody() {
    let mut h = Harness::new(8, 1);
    let request = h.request();
    assert_eq!(request.snapshot_bytes(), 8);
    assert_eq!(h.args.encodes.load(Ordering::SeqCst), 1);
    assert_eq!(h.args.bindings.load(Ordering::SeqCst), 1);
    let future = h.handle.enqueue_launch(request).unwrap();
    let mut operation = h.pop();
    assert!(!operation.advance(&mut h.context));
    assert_eq!(h.used(), 0);
    assert_eq!(h.args.encodes.load(Ordering::SeqCst), 1);
    let mut state = h.state.lock().unwrap();
    assert_eq!(state.issues[0].2, vec![0; 8]);
    assert_eq!(state.release_calls, 0);
    *state.statuses.values_mut().next().unwrap() = BackendPollV1::Succeeded;
    drop(state);
    assert!(operation.advance(&mut h.context));
    let result = join_command(future).unwrap();
    assert_eq!(
        result.observation.unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(h.state.lock().unwrap().release_calls, 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r64_snapshot_rechecks_live_context_and_refunds_failed_admission() {
    let mut h = Harness::new(8, 1);
    let future = h.handle.enqueue_launch(h.request()).unwrap();
    h.context.destroy_stream(h.stream).unwrap();
    assert!(h.pop().advance(&mut h.context));
    assert_eq!(h.used(), 0);
    assert!(matches!(
        join_command(future).unwrap().observation,
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownStream
        ))
    ));
    assert!(h.state.lock().unwrap().issues.is_empty());
}

#[test]
fn r64_snapshot_shape_bounds_reject_before_queueing() {
    let mut h = Harness::new(8, 1);
    h.args.length = crate::MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 + 1;
    assert!(matches!(
        RuntimeAsyncLaunchRequestV1::new(
            h.stream,
            h.kernel.clone(),
            &h.args,
            geometry(),
            Vec::new()
        ),
        Err(RuntimeAsyncSnapshotErrorV1::KernargTooLarge)
    ));
    assert_eq!(h.args.bindings.load(Ordering::SeqCst), 0);
    assert_eq!(h.used(), 0);
    assert!(h.receiver.try_recv().is_err());
    h.args.length = 8;
    let allocation = h
        .context
        .allocate(
            h.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            8,
            8,
        )
        .unwrap();
    h.args.effects = vec![
        RuntimeBindingV1 {
            region: crate::RuntimeMemoryRegionV1 {
                allocation,
                byte_offset: 0,
                byte_len: 8,
                access: crate::RuntimeAccessV1::Read,
            },
            kernarg_byte_offset: 0,
        };
        fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 + 1
    ];
    assert!(matches!(
        RuntimeAsyncLaunchRequestV1::new(
            h.stream,
            h.kernel.clone(),
            &h.args,
            geometry(),
            Vec::new()
        ),
        Err(RuntimeAsyncSnapshotErrorV1::TooManyBindings)
    ));
    assert_eq!(h.used(), 0);
    assert!(h.receiver.try_recv().is_err());
    for capacity in [0, MAX_RUNTIME_ASYNC_SNAPSHOT_BYTES_V1 + 1, usize::MAX] {
        assert_eq!(
            RuntimeAsyncEngineConfigV1::default().with_snapshot_byte_capacity(capacity),
            Err(RuntimeAsyncEngineConfigErrorV1::SnapshotByteCapacity)
        );
    }
}

#[test]
fn r64_all_standalone_operations_preflight_dependencies_before_enqueue() {
    let mut h = Harness::new(4096, 4);
    let submission = h
        .context
        .launch(h.stream, &h.kernel, &h.args, geometry(), &[])
        .unwrap();
    let event = h.context.record_event(&submission).unwrap();
    let allocation = h
        .context
        .allocate(
            h.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            16,
            8,
        )
        .unwrap();
    let region = crate::RuntimeMemoryRegionV1 {
        allocation,
        byte_offset: 0,
        byte_len: 8,
        access: crate::RuntimeAccessV1::ReadWrite,
    };
    let initial_encodes = h.args.encodes.load(Ordering::SeqCst);
    for (dependencies, error) in [
        (
            vec![event; 2],
            RuntimeAsyncSnapshotErrorV1::DuplicateDependency,
        ),
        (
            vec![event; crate::MAX_RUNTIME_DEPENDENCIES_V1 + 1],
            RuntimeAsyncSnapshotErrorV1::TooManyDependencies,
        ),
    ] {
        let expected = Some(RuntimeAsyncEngineCallErrorV1::InvalidSnapshot(error));
        assert_eq!(
            h.handle
                .launch(
                    h.stream,
                    h.kernel.clone(),
                    h.args.clone(),
                    geometry(),
                    dependencies.clone()
                )
                .err(),
            expected
        );
        assert_eq!(
            h.handle
                .launch_tracked(
                    h.stream,
                    h.kernel.clone(),
                    h.args.clone(),
                    geometry(),
                    dependencies.clone()
                )
                .err(),
            expected
        );
        assert_eq!(
            h.handle
                .copy_async(h.stream, region, region, dependencies.clone())
                .err(),
            expected
        );
        assert_eq!(
            h.handle
                .copy_async_tracked(h.stream, region, region, dependencies.clone())
                .err(),
            expected
        );
        assert_eq!(
            h.handle
                .peer_copy(h.stream, region, region, dependencies.clone())
                .err(),
            expected
        );
        assert_eq!(
            h.handle
                .peer_copy_tracked(h.stream, region, region, dependencies)
                .err(),
            expected
        );
    }
    assert_eq!(h.args.encodes.load(Ordering::SeqCst), initial_encodes);
    assert!(h.receiver.try_recv().is_err());
    assert_eq!(h.used(), 0);
    let mut dependencies = Vec::with_capacity(262_144);
    dependencies.push(event);
    let future = h
        .handle
        .launch(
            h.stream,
            h.kernel.clone(),
            h.args.clone(),
            geometry(),
            dependencies,
        )
        .unwrap();
    assert_eq!(h.used(), std::mem::size_of::<RuntimeEventIdV1>());
    drop(future);
    assert_eq!(h.used(), std::mem::size_of::<RuntimeEventIdV1>());
    drop(h.pop());
    assert_eq!(h.used(), 0);
}

#[test]
fn r64_snapshot_submit_rejection_and_panic_return_payload_credit() {
    for panics in [false, true] {
        let mut h = Harness::new(8, 1);
        if panics {
            h.state.lock().unwrap().panic_on_submit = true;
        } else {
            h.state
                .lock()
                .unwrap()
                .submit_failures
                .push_back(RuntimeBackendFailureV1::Rejected(MockError("capacity")));
        }
        let future = h.handle.enqueue_launch(h.request()).unwrap();
        let mut operations = operation::OperationRegistryV1::new();
        operations.insert(h.pop());
        operation::advance_operations_v1(
            &mut h.context,
            &mut operations,
            1,
            0,
            flush_stream_v1::<MockBackend>,
        );
        assert_eq!(h.used(), 0);
        assert_eq!(operations.len(), 0);
        assert!(h.state.lock().unwrap().issues.is_empty());
        if panics {
            assert!(h.context.is_terminal());
            assert!(matches!(
                join_command(future),
                Err(RuntimeAsyncEngineCallErrorV1::CommandPanicked)
            ));
        } else {
            assert!(matches!(
                join_command(future).unwrap().observation,
                Err(RuntimeErrorV1::BackendRejected(_))
            ));
        }
    }
}

#[test]
fn r64_snapshot_registry_rejection_keeps_charge_until_actual_disposal() {
    let h = Harness::new(8, 1);
    let future = h.handle.enqueue_launch(h.request()).unwrap();
    let mut operation = h.pop();
    operation.reject(RuntimeAsyncEngineCallErrorV1::OperationCapacity);
    assert!(matches!(
        join_command(future),
        Err(RuntimeAsyncEngineCallErrorV1::OperationCapacity)
    ));
    assert_eq!(h.used(), 8);
    drop(operation);
    assert_eq!(h.used(), 0);
}
