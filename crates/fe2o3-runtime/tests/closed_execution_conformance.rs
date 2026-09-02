//! Bounded differential checks between the public runtime facade and the
//! executable closed-execution model.
//!
//! Runtime submission acceptance corresponds to model preparation. For work
//! queued behind an event, model publication corresponds to the point where
//! the backend makes that work eligible, not to creation of the facade token.
//! These tests exercise a deterministic mock backend; they are not a
//! Rust-to-Verus proof or evidence about a native driver or device.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::time::{Duration, Instant};

use fe2o3_runtime::{
    BackendCancellationV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeArgumentsV1,
    RuntimeAsyncCopyBackendV1, RuntimeBackendFailureV1, RuntimeBackendV1, RuntimeBindingV1,
    RuntimeCancellationBackendV1, RuntimeCancellationV1, RuntimeCapabilitiesV1, RuntimeContextV1,
    RuntimeErrorV1, RuntimeExecutionCapabilitiesV1, RuntimeLaunchGeometryV1, RuntimeMemoryKindV1,
    RuntimeMemoryRegionV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeValidationErrorV1,
    TypedRuntimeKernelV1,
};
use fe2o3_runtime_model::{
    ClosedExecutionErrorV1, ClosedExecutionModelV1, ClosedOperationKeyV1, ClosedOperationKindV1,
    ClosedOperationPhaseV1, ClosedPoolBlockPhaseV1, ClosedPoolKeyV1, ClosedPoolLeaseKeyV1,
    ClosedStreamKeyV1, DeviceGenerationV1, DeviceKeyV1, IdentityDigestV1, PhysicalDeviceIdV1,
};

const ALLOCATION_BYTES: u64 = 64;
const ALLOCATION_ALIGNMENT: u64 = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancelDisposition {
    Withdraw,
    TooLate,
}

#[derive(Debug)]
struct MockError(&'static str);

impl fmt::Display for MockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for MockError {}

#[derive(Debug)]
struct MockAllocation {
    device: u64,
    bytes: Vec<u8>,
    alignment: u64,
}

#[derive(Clone, Debug)]
struct PendingCopy {
    destination: u64,
    destination_offset: usize,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct MockSubmission {
    stream: u64,
    dependencies: Vec<u64>,
    allocations: Vec<u64>,
    eligible_polls: u8,
    quiescent: bool,
    terminal_on_poll: bool,
    pending_copy: Option<PendingCopy>,
}

#[derive(Debug)]
struct ConformanceBackend {
    next_handle: u64,
    streams: HashMap<u64, u64>,
    allocations: HashMap<u64, MockAllocation>,
    free_allocations: Vec<(u64, u64, u64, u64)>,
    allocation_generations: HashMap<u64, u64>,
    last_allocation: Option<(u64, u64)>,
    submissions: HashMap<u64, MockSubmission>,
    events: HashMap<u64, u64>,
    cancel_disposition: CancelDisposition,
    terminal_next_poll: bool,
    terminal: bool,
}

impl ConformanceBackend {
    fn new(cancel_disposition: CancelDisposition, terminal_next_poll: bool) -> Self {
        Self {
            next_handle: 100,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            free_allocations: Vec::new(),
            allocation_generations: HashMap::new(),
            last_allocation: None,
            submissions: HashMap::new(),
            events: HashMap::new(),
            cancel_disposition,
            terminal_next_poll,
            terminal: false,
        }
    }

    fn next_handle(&mut self) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        handle
    }

    fn fail_if_terminal(&self) -> Result<(), RuntimeBackendFailureV1<MockError>> {
        if self.terminal {
            Err(RuntimeBackendFailureV1::Terminal(MockError(
                "mock backend is terminal",
            )))
        } else {
            Ok(())
        }
    }

    fn submit(
        &mut self,
        stream: u64,
        dependencies: &[u64],
        allocations: Vec<u64>,
        pending_copy: Option<PendingCopy>,
    ) -> Result<u64, RuntimeBackendFailureV1<MockError>> {
        self.fail_if_terminal()?;
        let handle = self.next_handle();
        let terminal_on_poll = std::mem::take(&mut self.terminal_next_poll);
        self.submissions.insert(
            handle,
            MockSubmission {
                stream,
                dependencies: dependencies.to_vec(),
                allocations,
                eligible_polls: 0,
                quiescent: false,
                terminal_on_poll,
                pending_copy,
            },
        );
        Ok(handle)
    }

    fn dependencies_complete(&self, submission: u64) -> bool {
        self.submissions[&submission]
            .dependencies
            .iter()
            .all(|event| {
                self.events
                    .get(event)
                    .and_then(|source| self.submissions.get(source))
                    .is_some_and(|source| source.quiescent)
            })
    }

    fn complete(&mut self, submission: u64) {
        let copy = {
            let submission = self.submissions.get_mut(&submission).unwrap();
            submission.quiescent = true;
            submission.pending_copy.take()
        };
        if let Some(copy) = copy {
            let destination = self.allocations.get_mut(&copy.destination).unwrap();
            let end = copy.destination_offset + copy.bytes.len();
            destination.bytes[copy.destination_offset..end].copy_from_slice(&copy.bytes);
        }
    }

    fn last_native_allocation(&self) -> (u64, u64) {
        self.last_allocation.unwrap()
    }

    fn retained_submission_count(&self) -> usize {
        self.submissions.len()
    }
}

impl RuntimeBackendV1 for ConformanceBackend {
    type Error = MockError;

    fn execution_capabilities_v1(&self, _device: u64) -> RuntimeExecutionCapabilitiesV1 {
        RuntimeExecutionCapabilitiesV1 {
            native_async_copy: true,
            concurrent_compute: true,
            memory_pool: true,
            cancellation: true,
            ..RuntimeExecutionCapabilitiesV1::default()
        }
    }

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        let capabilities = RuntimeCapabilitiesV1 {
            typed_async_launch: true,
            streams: true,
            events: true,
            device_memory: true,
            host_visible_memory: true,
            peer_copy: true,
            multi_device: true,
            atomics: true,
            collectives: true,
        };
        Ok(vec![
            BackendDeviceDescriptionV1 {
                backend_device: 10,
                name: "conformance-device-0".into(),
                target: "gfx942".into(),
                global_memory_bytes: 1 << 30,
                capabilities,
            },
            BackendDeviceDescriptionV1 {
                backend_device: 20,
                name: "conformance-device-1".into(),
                target: "gfx942".into(),
                global_memory_bytes: 1 << 30,
                capabilities,
            },
        ])
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        let stream = self.next_handle();
        self.streams.insert(stream, device);
        Ok(stream)
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        for submission in self.submissions.values_mut() {
            if submission.stream == stream {
                submission.quiescent = true;
            }
        }
        self.streams.remove(&stream);
        Ok(())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        _kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        let reusable =
            self.free_allocations
                .iter()
                .position(|(_, owner, capacity, prior_alignment)| {
                    *owner == device && *capacity >= byte_len && *prior_alignment >= alignment
                });
        let handle = if let Some(index) = reusable {
            self.free_allocations.swap_remove(index).0
        } else {
            self.next_handle()
        };
        let generation = self
            .allocation_generations
            .get(&handle)
            .copied()
            .unwrap_or(0)
            + 1;
        self.allocation_generations.insert(handle, generation);
        self.allocations.insert(
            handle,
            MockAllocation {
                device,
                bytes: vec![0; byte_len as usize],
                alignment,
            },
        );
        self.last_allocation = Some((handle, generation));
        Ok(handle)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        if self
            .submissions
            .values()
            .any(|submission| submission.allocations.contains(&allocation))
        {
            return Err(RuntimeBackendFailureV1::Rejected(MockError(
                "submission retains allocation",
            )));
        }
        let handle = allocation;
        let allocation = self.allocations.remove(&handle).unwrap();
        self.free_allocations.push((
            handle,
            allocation.device,
            allocation.bytes.len() as u64,
            allocation.alignment,
        ));
        Ok(())
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        let allocation = self.allocations.get_mut(&allocation).unwrap();
        let start = byte_offset as usize;
        allocation.bytes[start..start + bytes.len()].copy_from_slice(bytes);
        Ok(())
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        let allocation = self.allocations.get(&allocation).unwrap();
        let start = byte_offset as usize;
        destination.copy_from_slice(&allocation.bytes[start..start + destination.len()]);
        Ok(())
    }

    fn load_module_v1(
        &mut self,
        _device: u64,
        _image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        Ok(self.next_handle())
    }

    fn unload_module_v1(
        &mut self,
        _module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()
    }

    fn resolve_kernel_v1(
        &mut self,
        _module: u64,
        _name: &str,
        _signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        Ok(self.next_handle())
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let allocations = launch
            .bindings
            .iter()
            .map(|binding| binding.region.allocation)
            .collect();
        self.submit(launch.stream, launch.dependencies, allocations, None)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        if self.submissions[&submission].terminal_on_poll {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(MockError(
                "indeterminate completion",
            )));
        }
        if !self.dependencies_complete(submission) {
            return Ok(BackendPollV1::Pending);
        }
        let polls = &mut self
            .submissions
            .get_mut(&submission)
            .unwrap()
            .eligible_polls;
        *polls += 1;
        if *polls == 1 {
            Ok(BackendPollV1::Pending)
        } else {
            self.complete(submission);
            Ok(BackendPollV1::Succeeded)
        }
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        _deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        if !self.dependencies_complete(submission) {
            return Ok(BackendPollV1::Pending);
        }
        self.complete(submission);
        Ok(BackendPollV1::Succeeded)
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        if !self.submissions[&submission].quiescent {
            return Err(RuntimeBackendFailureV1::Rejected(MockError(
                "submission is pending",
            )));
        }
        self.submissions.remove(&submission);
        Ok(())
    }

    fn record_event_v1(
        &mut self,
        _stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        let event = self.next_handle();
        self.events.insert(event, submission);
        Ok(event)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        self.events.remove(&event);
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.copy_submission(stream, source, destination, dependencies)
    }
}

impl ConformanceBackend {
    fn copy_submission(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<MockError>> {
        self.fail_if_terminal()?;
        let start = source.byte_offset as usize;
        let end = start + source.byte_len as usize;
        let bytes = self.allocations[&source.allocation].bytes[start..end].to_vec();
        self.submit(
            stream,
            dependencies,
            vec![source.allocation, destination.allocation],
            Some(PendingCopy {
                destination: destination.allocation,
                destination_offset: destination.byte_offset as usize,
                bytes,
            }),
        )
    }
}

impl RuntimeAsyncCopyBackendV1 for ConformanceBackend {
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.copy_submission(stream, source, destination, dependencies)
    }
}

impl RuntimeCancellationBackendV1 for ConformanceBackend {
    fn cancel_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendCancellationV1, RuntimeBackendFailureV1<Self::Error>> {
        self.fail_if_terminal()?;
        match self.cancel_disposition {
            CancelDisposition::Withdraw => {
                self.submissions.get_mut(&submission).unwrap().quiescent = true;
                Ok(BackendCancellationV1::Cancelled)
            }
            CancelDisposition::TooLate => Ok(BackendCancellationV1::TooLate),
        }
    }

    fn drain_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.wait_v1(submission, deadline)
    }
}

#[derive(Clone, Copy)]
struct BufferArguments {
    allocation: RuntimeAllocationIdV1,
}

impl RuntimeArgumentsV1 for BufferArguments {
    const SIGNATURE_V1: [u8; 32] = [0x51; 32];

    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; 8]
    }

    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        vec![RuntimeBindingV1 {
            region: RuntimeMemoryRegionV1 {
                allocation: self.allocation,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: ALLOCATION_BYTES,
            },
            kernarg_byte_offset: 0,
        }]
    }
}

struct Fixture {
    context: RuntimeContextV1<ConformanceBackend>,
    stream: RuntimeStreamIdV1,
    allocation: RuntimeAllocationIdV1,
    module: fe2o3_runtime::RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<BufferArguments>,
    model: ClosedExecutionModelV1,
    model_stream: ClosedStreamKeyV1,
    pool: ClosedPoolKeyV1,
    lease: ClosedPoolLeaseKeyV1,
}

impl Fixture {
    fn new(cancel_disposition: CancelDisposition, terminal_next_poll: bool) -> Self {
        let mut context = RuntimeContextV1::open(ConformanceBackend::new(
            cancel_disposition,
            terminal_next_poll,
        ))
        .unwrap();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(
                device,
                RuntimeMemoryKindV1::DeviceLocal,
                ALLOCATION_BYTES,
                ALLOCATION_ALIGNMENT,
            )
            .unwrap();
        let module = context.load_module(device, b"conformance-object").unwrap();
        let kernel = context
            .resolve_kernel::<BufferArguments>(module, "conformance_kernel")
            .unwrap();

        let model_device = model_device(1);
        let model_stream = ClosedStreamKeyV1 {
            device: model_device,
            stream_id: stream.get(),
            generation: 1,
        };
        let pool = ClosedPoolKeyV1 {
            device: model_device,
            pool_id: 1,
        };
        let mut model = ClosedExecutionModelV1::new_model_only(
            IdentityDigestV1::from_untrusted_bytes([0xC6; 32]),
        )
        .unwrap();
        model.register_stream_model_only(model_stream).unwrap();
        model
            .register_pool_model_only(pool, 4 * ALLOCATION_BYTES, 4)
            .unwrap();
        let lease = model
            .lease_model_only(pool, ALLOCATION_BYTES, ALLOCATION_ALIGNMENT)
            .unwrap();

        Self {
            context,
            stream,
            allocation,
            module,
            kernel,
            model,
            model_stream,
            pool,
            lease,
        }
    }

    fn operation(&self, sequence: u64) -> ClosedOperationKeyV1 {
        ClosedOperationKeyV1 {
            stream: self.model_stream,
            sequence,
        }
    }

    fn prepare(&mut self, operation: ClosedOperationKeyV1) {
        self.model
            .prepare_operation_model_only(
                operation,
                ClosedOperationKindV1::Compute {
                    execution_device: self.model_stream.device,
                },
                vec![],
                vec![self.lease],
            )
            .unwrap();
    }

    fn launch(&mut self) -> fe2o3_runtime::RuntimeSubmissionV1<BufferArguments> {
        self.context
            .launch(
                self.stream,
                &self.kernel,
                &BufferArguments {
                    allocation: self.allocation,
                },
                geometry(),
                &[],
            )
            .unwrap()
    }

    fn finish(mut self) {
        self.context.release_allocation(self.allocation).unwrap();
        self.context.unload_module(self.module).unwrap();
        self.context.destroy_stream(self.stream).unwrap();
        let backend = self.context.shutdown().unwrap();
        assert_eq!(backend.retained_submission_count(), 0);
    }
}

fn model_device(id: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(id),
        generation: DeviceGenerationV1(1),
    }
}

fn geometry() -> RuntimeLaunchGeometryV1 {
    RuntimeLaunchGeometryV1 {
        grid: [64, 1, 1],
        workgroup: [64, 1, 1],
        dynamic_shared_bytes: 0,
    }
}

fn publish_one(
    model: &mut ClosedExecutionModelV1,
    stream: ClosedStreamKeyV1,
    operation: ClosedOperationKeyV1,
) {
    let batch = model
        .form_prepared_batch_model_only(stream, vec![operation])
        .unwrap();
    model.publish_prepared_batch_model_only(&batch).unwrap();
}

fn validation_is<E>(error: &RuntimeErrorV1<E>, expected: RuntimeValidationErrorV1) -> bool {
    matches!(error, RuntimeErrorV1::Validation(actual) if *actual == expected)
}

#[test]
fn accepted_pending_completion_release_and_pool_reuse_conform() {
    let mut fixture = Fixture::new(CancelDisposition::TooLate, false);
    let first_native = fixture.context.backend().last_native_allocation();
    assert_eq!(first_native.1, fixture.lease.generation);

    let operation = fixture.operation(1);
    fixture.prepare(operation);
    let mut submission = fixture.launch();
    publish_one(&mut fixture.model, fixture.model_stream, operation);

    assert_eq!(
        fixture.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert!(matches!(
        fixture.model.operation(operation).unwrap().phase(),
        ClosedOperationPhaseV1::Published { .. }
    ));

    let failure = fixture.context.release_submission(submission).unwrap_err();
    let (mut submission, error) = failure.into_parts();
    assert!(validation_is(
        &error,
        RuntimeValidationErrorV1::SubmissionPending
    ));
    assert_eq!(
        fixture.model.release_completed_model_only(operation),
        Err(ClosedExecutionErrorV1::IllegalTransition)
    );
    assert!(matches!(
        fixture.context.release_allocation(fixture.allocation),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));

    assert_eq!(
        fixture.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    fixture
        .model
        .observe_completion_model_only(operation)
        .unwrap();
    fixture.context.release_submission(submission).unwrap();
    fixture
        .model
        .release_completed_model_only(operation)
        .unwrap();
    fixture
        .context
        .release_allocation(fixture.allocation)
        .unwrap();

    let replacement = fixture
        .context
        .allocate(
            fixture.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            ALLOCATION_BYTES,
            ALLOCATION_ALIGNMENT,
        )
        .unwrap();
    let second_native = fixture.context.backend().last_native_allocation();
    let replacement_lease = fixture
        .model
        .lease_model_only(fixture.pool, ALLOCATION_BYTES, ALLOCATION_ALIGNMENT)
        .unwrap();
    assert_ne!(replacement, fixture.allocation);
    assert_eq!(second_native.0, first_native.0);
    assert_eq!(second_native.1, first_native.1 + 1);
    assert_eq!(replacement_lease.block_id, fixture.lease.block_id);
    assert_eq!(replacement_lease.generation, fixture.lease.generation + 1);

    fixture
        .model
        .release_unprepared_lease_model_only(replacement_lease)
        .unwrap();
    fixture.context.release_allocation(replacement).unwrap();
    fixture.context.unload_module(fixture.module).unwrap();
    fixture.context.destroy_stream(fixture.stream).unwrap();
    fixture.context.shutdown().unwrap();
}

#[test]
fn cross_stream_dependency_readiness_and_rejection_conform() {
    let mut fixture = Fixture::new(CancelDisposition::TooLate, false);
    let device = fixture.context.devices()[0].id();
    let consumer_stream = fixture.context.create_stream(device).unwrap();
    let consumer_allocation = fixture
        .context
        .allocate(
            device,
            RuntimeMemoryKindV1::DeviceLocal,
            ALLOCATION_BYTES,
            ALLOCATION_ALIGNMENT,
        )
        .unwrap();
    let consumer_model_stream = ClosedStreamKeyV1 {
        device: fixture.model_stream.device,
        stream_id: consumer_stream.get(),
        generation: 1,
    };
    fixture
        .model
        .register_stream_model_only(consumer_model_stream)
        .unwrap();
    let consumer_lease = fixture
        .model
        .lease_model_only(fixture.pool, ALLOCATION_BYTES, ALLOCATION_ALIGNMENT)
        .unwrap();

    let producer = fixture.operation(1);
    fixture.prepare(producer);
    let mut producer_submission = fixture.launch();
    publish_one(&mut fixture.model, fixture.model_stream, producer);
    let event = fixture.context.record_event(&producer_submission).unwrap();

    let consumer = ClosedOperationKeyV1 {
        stream: consumer_model_stream,
        sequence: 1,
    };
    assert!(matches!(
        fixture.context.launch(
            consumer_stream,
            &fixture.kernel,
            &BufferArguments {
                allocation: consumer_allocation,
            },
            geometry(),
            &[event, event],
        ),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::DuplicateDependency
        ))
    ));
    assert_eq!(
        fixture.model.prepare_operation_model_only(
            consumer,
            ClosedOperationKindV1::Compute {
                execution_device: consumer_model_stream.device,
            },
            vec![producer, producer],
            vec![consumer_lease],
        ),
        Err(ClosedExecutionErrorV1::InvalidRoster)
    );

    let mut consumer_submission = fixture
        .context
        .launch(
            consumer_stream,
            &fixture.kernel,
            &BufferArguments {
                allocation: consumer_allocation,
            },
            geometry(),
            &[event],
        )
        .unwrap();
    fixture
        .model
        .prepare_operation_model_only(
            consumer,
            ClosedOperationKindV1::Compute {
                execution_device: consumer_model_stream.device,
            },
            vec![producer],
            vec![consumer_lease],
        )
        .unwrap();
    let consumer_batch = fixture
        .model
        .form_prepared_batch_model_only(consumer_model_stream, vec![consumer])
        .unwrap();
    assert_eq!(
        fixture
            .model
            .publish_prepared_batch_model_only(&consumer_batch),
        Err(ClosedExecutionErrorV1::DependencyNotCompleted)
    );
    assert_eq!(
        fixture.context.poll(&mut consumer_submission).unwrap(),
        RuntimePollV1::Pending
    );

    assert_eq!(
        fixture.context.poll(&mut producer_submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        fixture.context.poll(&mut producer_submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    fixture
        .model
        .observe_completion_model_only(producer)
        .unwrap();
    fixture
        .model
        .publish_prepared_batch_model_only(&consumer_batch)
        .unwrap();

    assert_eq!(
        fixture.context.poll(&mut consumer_submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        fixture.context.poll(&mut consumer_submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    fixture
        .model
        .observe_completion_model_only(consumer)
        .unwrap();
    fixture.context.release_event(event).unwrap();
    fixture
        .context
        .release_submission(producer_submission)
        .unwrap();
    fixture
        .context
        .release_submission(consumer_submission)
        .unwrap();
    fixture
        .model
        .release_completed_model_only(producer)
        .unwrap();
    fixture
        .model
        .release_completed_model_only(consumer)
        .unwrap();
    fixture.model.validate_global_invariants().unwrap();

    fixture
        .context
        .release_allocation(consumer_allocation)
        .unwrap();
    fixture.context.destroy_stream(consumer_stream).unwrap();
    fixture.finish();
}

#[test]
fn peer_copy_completion_retains_both_device_owners() {
    let mut context =
        RuntimeContextV1::open(ConformanceBackend::new(CancelDisposition::TooLate, false)).unwrap();
    let devices = context.devices().to_vec();
    let source = context
        .allocate(
            devices[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            ALLOCATION_BYTES,
            ALLOCATION_ALIGNMENT,
        )
        .unwrap();
    let destination = context
        .allocate(
            devices[1].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            ALLOCATION_BYTES,
            ALLOCATION_ALIGNMENT,
        )
        .unwrap();
    let stream = context.create_stream(devices[1].id()).unwrap();
    let expected = [0xA7; 16];
    context.write_allocation(source, 8, &expected).unwrap();

    let mut submission = context
        .peer_copy(
            stream,
            RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 8,
                byte_len: expected.len() as u64,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 24,
                byte_len: expected.len() as u64,
            },
            &[],
        )
        .unwrap();

    let source_device = model_device(1);
    let destination_device = model_device(2);
    let model_stream = ClosedStreamKeyV1 {
        device: destination_device,
        stream_id: stream.get(),
        generation: 1,
    };
    let source_pool = ClosedPoolKeyV1 {
        device: source_device,
        pool_id: 1,
    };
    let destination_pool = ClosedPoolKeyV1 {
        device: destination_device,
        pool_id: 1,
    };
    let mut model =
        ClosedExecutionModelV1::new_model_only(IdentityDigestV1::from_untrusted_bytes([0xD6; 32]))
            .unwrap();
    model.register_stream_model_only(model_stream).unwrap();
    model
        .register_pool_model_only(source_pool, ALLOCATION_BYTES, 1)
        .unwrap();
    model
        .register_pool_model_only(destination_pool, ALLOCATION_BYTES, 1)
        .unwrap();
    let source_lease = model
        .lease_model_only(source_pool, ALLOCATION_BYTES, ALLOCATION_ALIGNMENT)
        .unwrap();
    let destination_lease = model
        .lease_model_only(destination_pool, ALLOCATION_BYTES, ALLOCATION_ALIGNMENT)
        .unwrap();
    let operation = ClosedOperationKeyV1 {
        stream: model_stream,
        sequence: 1,
    };
    model
        .prepare_operation_model_only(
            operation,
            ClosedOperationKindV1::PeerCopy {
                source_device,
                destination_device,
                execution_device: destination_device,
            },
            vec![],
            vec![source_lease, destination_lease],
        )
        .unwrap();
    publish_one(&mut model, model_stream, operation);

    assert_eq!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert!(matches!(
        context.release_allocation(source),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert!(matches!(
        context.release_allocation(destination),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    model.observe_completion_model_only(operation).unwrap();
    let mut actual = [0; 16];
    context
        .read_allocation(destination, 24, &mut actual)
        .unwrap();
    assert_eq!(actual, expected);

    context.release_submission(submission).unwrap();
    model.release_completed_model_only(operation).unwrap();
    context.release_allocation(source).unwrap();
    context.release_allocation(destination).unwrap();
    context.destroy_stream(stream).unwrap();
    context.shutdown().unwrap();
    model.validate_global_invariants().unwrap();
}

#[test]
fn cancellation_before_publication_releases_both_generations() {
    let mut fixture = Fixture::new(CancelDisposition::Withdraw, false);
    let first_native = fixture.context.backend().last_native_allocation();
    let operation = fixture.operation(1);
    fixture.prepare(operation);
    let mut submission = fixture.launch();

    assert_eq!(
        fixture.context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    fixture
        .model
        .cancel_before_publication_model_only(operation)
        .unwrap();
    assert_eq!(
        fixture.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Failed { code: -2 }
    );
    assert_eq!(
        fixture.model.operation(operation).unwrap().phase(),
        ClosedOperationPhaseV1::CancelledBeforePublication
    );
    fixture.context.release_submission(submission).unwrap();
    fixture
        .context
        .release_allocation(fixture.allocation)
        .unwrap();

    let replacement = fixture
        .context
        .allocate(
            fixture.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            ALLOCATION_BYTES,
            ALLOCATION_ALIGNMENT,
        )
        .unwrap();
    let replacement_lease = fixture
        .model
        .lease_model_only(fixture.pool, ALLOCATION_BYTES, ALLOCATION_ALIGNMENT)
        .unwrap();
    assert_eq!(
        fixture.context.backend().last_native_allocation().0,
        first_native.0
    );
    assert_eq!(replacement_lease.block_id, fixture.lease.block_id);
    assert_eq!(replacement_lease.generation, fixture.lease.generation + 1);
    fixture
        .model
        .release_unprepared_lease_model_only(replacement_lease)
        .unwrap();
    fixture.context.release_allocation(replacement).unwrap();
    fixture.context.unload_module(fixture.module).unwrap();
    fixture.context.destroy_stream(fixture.stream).unwrap();
    fixture.context.shutdown().unwrap();
}

#[test]
fn too_late_cancellation_retains_until_drain_quiescence() {
    let mut fixture = Fixture::new(CancelDisposition::TooLate, false);
    let operation = fixture.operation(1);
    fixture.prepare(operation);
    let mut submission = fixture.launch();
    publish_one(&mut fixture.model, fixture.model_stream, operation);

    assert_eq!(
        fixture.context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::TooLate
    );
    assert!(
        fixture
            .model
            .request_cancellation_model_only(operation)
            .unwrap()
    );
    let failure = fixture.context.release_submission(submission).unwrap_err();
    let (mut submission, error) = failure.into_parts();
    assert!(validation_is(
        &error,
        RuntimeValidationErrorV1::SubmissionPending
    ));
    assert_eq!(fixture.model.retained_operation_count(), 1);

    assert_eq!(
        fixture
            .context
            .drain(&mut submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    fixture
        .model
        .observe_completion_model_only(operation)
        .unwrap();
    fixture.context.release_submission(submission).unwrap();
    fixture
        .model
        .release_completed_model_only(operation)
        .unwrap();
    fixture.finish();
}

#[test]
fn terminal_completion_ambiguity_quarantines_and_retains() {
    let mut fixture = Fixture::new(CancelDisposition::TooLate, true);
    let operation = fixture.operation(1);
    fixture.prepare(operation);
    let mut submission = fixture.launch();
    publish_one(&mut fixture.model, fixture.model_stream, operation);

    assert!(matches!(
        fixture.context.poll(&mut submission),
        Err(RuntimeErrorV1::BackendTerminal(_))
    ));
    fixture
        .model
        .quarantine_published_model_only(operation)
        .unwrap();
    assert!(fixture.context.is_terminal());
    assert_eq!(
        fixture.model.operation(operation).unwrap().phase(),
        ClosedOperationPhaseV1::Indeterminate
    );
    assert_eq!(
        fixture.model.blocks()[0].phase(),
        ClosedPoolBlockPhaseV1::Quarantined(operation)
    );
    assert!(matches!(
        fixture.context.release_allocation(fixture.allocation),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextTerminal
        ))
    ));
    assert_eq!(
        fixture.model.release_completed_model_only(operation),
        Err(ClosedExecutionErrorV1::IllegalTransition)
    );

    let report = fixture.context.cleanup();
    assert!(report.is_terminal());
    assert_eq!(report.retained().streams, 1);
    assert_eq!(report.retained().submissions, 1);
    assert_eq!(report.retained().modules, 1);
    assert_eq!(report.retained().allocations, 1);
    assert_eq!(fixture.model.retained_operation_count(), 1);
}
