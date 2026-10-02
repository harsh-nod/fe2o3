//! Public Context integration with scripted CPU effects, not native kernel authority.

use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;

use gpu_host::{GeneratedContextReadSlice as Read, GeneratedContextWriteSlice as Write};
use gpu_runtime::*;

use crate::{context_map_gpu, scalar_packet_gpu, transform_gpu};

type Failure = RuntimeBackendFailureV1<std::io::Error>;
type Context = RuntimeContextV1<Backend>;
type Arguments = context_map_gpu::ContextArguments;

fn denied() -> Failure {
    RuntimeBackendFailureV1::Rejected(std::io::Error::other("fixture denies execution"))
}

#[derive(Clone)]
enum Effect {
    Map(Vec<BackendBindingV1>, f32),
    Copy(BackendMemoryRegionV1, BackendMemoryRegionV1),
}

struct Operation {
    stream: u64,
    parents: Vec<u64>,
    effect: Effect,
    status: BackendPollV1,
}

#[derive(Default)]
struct Backend {
    next: u64,
    memory: BTreeMap<u64, Vec<u8>>,
    streams: BTreeMap<u64, u64>,
    modules: BTreeMap<u64, u64>,
    events: BTreeMap<u64, u64>,
    operations: BTreeMap<u64, Operation>,
    submitted: usize,
    attempts: usize,
    effects: usize,
    deny_launch: bool,
}

impl Backend {
    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn parents(&self, events: &[u64]) -> Vec<u64> {
        events.iter().map(|event| self.events[event]).collect()
    }

    fn enqueue(&mut self, stream: u64, parents: Vec<u64>, effect: Effect) -> u64 {
        let id = self.id();
        assert!(parents.iter().all(|parent| *parent < id));
        self.operations.insert(
            id,
            Operation {
                stream,
                parents,
                effect,
                status: BackendPollV1::Pending,
            },
        );
        self.submitted += 1;
        id
    }

    fn finish(&mut self, id: u64) {
        let op = &self.operations[&id];
        if op.status != BackendPollV1::Pending {
            return;
        }
        let parents = op.parents.clone();
        let effect = op.effect.clone();
        for parent in parents {
            self.finish(parent);
            assert_eq!(self.operations[&parent].status, BackendPollV1::Succeeded);
        }
        match effect {
            Effect::Copy(source, destination) => {
                let bytes = self.region(source).to_vec();
                self.region_mut(destination).copy_from_slice(&bytes);
            }
            Effect::Map(bindings, factor) => {
                // Deterministic backend fixture behavior, not execution of generated machine code.
                let bytes = self.region(bindings[0].region).to_vec();
                let output = self.region_mut(bindings[1].region);
                assert_eq!(bytes.len(), output.len());
                for (input, output) in bytes.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
                    let value = f32::from_le_bytes(input.try_into().unwrap()) * factor;
                    output.copy_from_slice(&value.to_le_bytes());
                }
            }
        }
        self.effects += 1;
        self.operations.get_mut(&id).unwrap().status = BackendPollV1::Succeeded;
    }

    fn region(&self, region: BackendMemoryRegionV1) -> &[u8] {
        &self.memory[&region.allocation]
            [region.byte_offset as usize..(region.byte_offset + region.byte_len) as usize]
    }

    fn region_mut(&mut self, region: BackendMemoryRegionV1) -> &mut [u8] {
        &mut self.memory.get_mut(&region.allocation).unwrap()
            [region.byte_offset as usize..(region.byte_offset + region.byte_len) as usize]
    }

    fn launch(
        &mut self,
        stream: u64,
        bytes: &[u8],
        bindings: &[BackendBindingV1],
        parents: Vec<u64>,
    ) -> Result<u64, Failure> {
        self.attempts += 1;
        if self.deny_launch {
            return Err(denied());
        }
        // Context validates handles and spans; this fixture backend owns its
        // kernel-specific disjoint-input/output policy, not the descriptors.
        if bindings.len() == 2 && bindings[0].region.allocation == bindings[1].region.allocation {
            return Err(denied());
        }
        assert_eq!(bytes.len(), 40);
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].kernarg_byte_offset, 8);
        assert_eq!(bindings[1].kernarg_byte_offset, 24);
        assert_eq!(bindings[0].region.access, RuntimeAccessV1::Read);
        assert_eq!(bindings[1].region.access, RuntimeAccessV1::Write);
        assert_eq!(&bytes[8..16], &[0; 8]);
        assert_eq!(&bytes[24..32], &[0; 8]);
        Ok(self.enqueue(
            stream,
            parents,
            Effect::Map(
                bindings.to_vec(),
                f32::from_le_bytes(bytes[..4].try_into().unwrap()),
            ),
        ))
    }
}

impl RuntimeBackendV1 for Backend {
    type Error = std::io::Error;

    fn enumerate_devices_v1(&mut self) -> Result<Vec<BackendDeviceDescriptionV1>, Failure> {
        let capabilities = RuntimeCapabilitiesV1 {
            typed_async_launch: true,
            streams: true,
            events: true,
            device_memory: true,
            host_visible_memory: true,
            peer_copy: true,
            multi_device: true,
            atomics: false,
            collectives: false,
        };
        Ok([10, 20]
            .into_iter()
            .map(|backend_device| BackendDeviceDescriptionV1 {
                backend_device,
                name: format!("scripted-{backend_device}"),
                target: "gfx942".into(),
                global_memory_bytes: 1 << 20,
                capabilities,
            })
            .collect())
    }

    fn create_stream_v1(&mut self, device: u64) -> Result<u64, Failure> {
        let id = self.id();
        self.streams.insert(id, device);
        Ok(id)
    }
    fn destroy_stream_v1(&mut self, stream: u64) -> Result<(), Failure> {
        assert!(
            self.operations
                .values()
                .all(|op| op.stream != stream || op.status != BackendPollV1::Pending)
        );
        self.streams.remove(&stream);
        Ok(())
    }
    fn allocate_v1(
        &mut self,
        _: u64,
        _: RuntimeMemoryKindV1,
        byte_len: u64,
        _: u64,
    ) -> Result<u64, Failure> {
        let id = self.id();
        self.memory.insert(id, vec![0; byte_len as usize]);
        Ok(id)
    }
    fn release_allocation_v1(&mut self, allocation: u64) -> Result<(), Failure> {
        self.memory.remove(&allocation);
        Ok(())
    }
    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), Failure> {
        self.memory.get_mut(&allocation).unwrap()
            [byte_offset as usize..byte_offset as usize + bytes.len()]
            .copy_from_slice(bytes);
        Ok(())
    }
    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        output: &mut [u8],
    ) -> Result<(), Failure> {
        output.copy_from_slice(
            &self.memory[&allocation][byte_offset as usize..byte_offset as usize + output.len()],
        );
        Ok(())
    }
    fn load_module_v1(&mut self, device: u64, _: &[u8]) -> Result<u64, Failure> {
        let id = self.id();
        self.modules.insert(id, device);
        Ok(id)
    }
    fn unload_module_v1(&mut self, module: u64) -> Result<(), Failure> {
        self.modules.remove(&module);
        Ok(())
    }
    fn resolve_kernel_v1(
        &mut self,
        _: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, Failure> {
        if name != "context_map" || signature != Arguments::SIGNATURE_V1 {
            return Err(denied());
        }
        Ok(self.id())
    }
    fn submit_v1(&mut self, request: BackendLaunchV1<'_>) -> Result<u64, Failure> {
        assert!(matches!(
            request.semantic_launch,
            BackendSemanticLaunchV1::Ordinary
        ));
        self.launch(
            request.stream,
            request.explicit_kernarg,
            request.bindings,
            self.parents(request.dependencies),
        )
    }
    fn poll_v1(&mut self, id: u64) -> Result<BackendPollV1, Failure> {
        Ok(self.operations[&id].status)
    }
    fn wait_v1(&mut self, id: u64, _: Instant) -> Result<BackendPollV1, Failure> {
        self.poll_v1(id)
    }
    fn release_submission_v1(&mut self, id: u64) -> Result<(), Failure> {
        assert_ne!(self.operations[&id].status, BackendPollV1::Pending);
        self.operations.remove(&id);
        Ok(())
    }
    fn record_event_v1(&mut self, _: u64, submission: u64) -> Result<u64, Failure> {
        let id = self.id();
        self.events.insert(id, submission);
        Ok(id)
    }
    fn release_event_v1(&mut self, event: u64) -> Result<(), Failure> {
        self.events.remove(&event);
        Ok(())
    }
    fn supports_pending_compute_peer_copy_v1(&self) -> bool {
        true
    }
    fn supports_ordered_compute_peer_copy_v1(&self) -> bool {
        true
    }
    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, Failure> {
        Ok(self.enqueue(
            stream,
            self.parents(dependencies),
            Effect::Copy(source, destination),
        ))
    }
}

impl RuntimeProducerAwareLaunchBackendV1 for Backend {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, Failure> {
        let parents = request
            .dependencies
            .iter()
            .map(|dependency| {
                assert_eq!(
                    self.events[&dependency.event],
                    dependency.producer_submission
                );
                dependency.producer_submission
            })
            .collect();
        self.launch(
            request.stream,
            request.explicit_kernarg,
            request.bindings,
            parents,
        )
    }
}

impl RuntimeAsyncCopyBackendV1 for Backend {
    fn supports_pending_peer_readback_v1(&self) -> bool {
        true
    }
    fn copy_async_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Result<u64, Failure> {
        self.peer_copy_v1(stream, source, destination, dependencies)
    }
}

impl RuntimeFlushBackendV1 for Backend {
    fn flush_stream_v1(&mut self, stream: u64) -> Result<(), Failure> {
        let ids: Vec<_> = self
            .operations
            .iter()
            .filter_map(|(id, op)| (op.stream == stream).then_some(*id))
            .collect();
        for id in ids {
            self.finish(id);
        }
        Ok(())
    }
}

fn geometry() -> RuntimeLaunchGeometryV1 {
    RuntimeLaunchGeometryV1 {
        grid: [4, 1, 1],
        workgroup: [1, 1, 1],
        dynamic_shared_bytes: 0,
    }
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    byte_offset: u64,
    byte_len: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len,
    }
}

fn allocate(context: &mut Context, device: RuntimeDeviceIdV1, bytes: u64) -> RuntimeAllocationIdV1 {
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, bytes, 4)
        .unwrap();
    context
        .write_allocation(allocation, 0, &vec![0; bytes as usize])
        .unwrap();
    allocation
}

fn arguments(
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    factor: f32,
    elements: u64,
) -> Arguments {
    Arguments::new(
        factor,
        Read::new(source, 0, elements).unwrap(),
        Write::new(destination, 0, elements).unwrap(),
    )
}

fn close(context: Context) {
    let backend = context.shutdown().unwrap();
    assert!(backend.memory.is_empty() && backend.streams.is_empty() && backend.modules.is_empty());
    assert!(backend.events.is_empty() && backend.operations.is_empty());
}

#[test]
fn generated_context_abi_is_canonical_and_address_free() {
    use gpu_host::__generated::CompilerGeneratedKernelExpectationV1;
    let mut context = Context::open(Backend::default()).unwrap();
    let device = context.devices()[0].id();
    let input = allocate(&mut context, device, 64);
    let output = allocate(&mut context, device, 64);
    let args = Arguments::new(
        -2.5,
        Read::new(input, 4, 3).unwrap(),
        Write::new(output, 8, 3).unwrap(),
    );
    let mut expected = vec![0; 40];
    expected[..4].copy_from_slice(&(-2.5_f32).to_le_bytes());
    expected[16..24].copy_from_slice(&3_u64.to_le_bytes());
    expected[32..40].copy_from_slice(&3_u64.to_le_bytes());
    assert_eq!(args.encode_explicit_kernarg_v1(), expected);
    assert_eq!(
        args.bindings_v1(),
        vec![
            RuntimeBindingV1 {
                region: region(input, RuntimeAccessV1::Read, 4, 12),
                kernarg_byte_offset: 8
            },
            RuntimeBindingV1 {
                region: region(output, RuntimeAccessV1::Write, 8, 12),
                kernarg_byte_offset: 24
            },
        ]
    );
    assert_eq!(
        Arguments::SIGNATURE_V1,
        context_map_gpu::Marker::PROFILE.generated_host_contract_identity()
    );
    assert_ne!(
        Arguments::SIGNATURE_V1,
        transform_gpu::ContextArguments::SIGNATURE_V1
    );
    let rw = transform_gpu::ContextArguments::new(
        1.0,
        Read::new(input, 0, 4).unwrap(),
        gpu_host::GeneratedContextReadWriteSlice::new(output, 0, 4).unwrap(),
    );
    assert_eq!(
        rw.bindings_v1()[1].region.access,
        RuntimeAccessV1::ReadWrite
    );
    close(context);
}

#[test]
fn generated_context_all_scalar_widths_preserve_little_endian_bits_and_padding() {
    let args = scalar_packet_gpu::ContextArguments::new(
        -2,
        253,
        -1234,
        54321,
        -123456,
        0xfedc_ba98,
        i64::MIN + 1,
        u64::MAX - 1,
        -0.0,
        f64::from_bits(0x7ff8_0000_0000_0042),
    );
    let mut expected = vec![0; 48];
    for (offset, bytes) in [
        (0, (-2_i8).to_le_bytes().to_vec()),
        (1, 253_u8.to_le_bytes().to_vec()),
        (2, (-1234_i16).to_le_bytes().to_vec()),
        (4, 54321_u16.to_le_bytes().to_vec()),
        (8, (-123456_i32).to_le_bytes().to_vec()),
        (12, 0xfedc_ba98_u32.to_le_bytes().to_vec()),
        (16, (i64::MIN + 1).to_le_bytes().to_vec()),
        (24, (u64::MAX - 1).to_le_bytes().to_vec()),
        (32, (-0.0_f32).to_le_bytes().to_vec()),
        (40, 0x7ff8_0000_0000_0042_u64.to_le_bytes().to_vec()),
    ] {
        expected[offset..offset + bytes.len()].copy_from_slice(&bytes);
    }
    assert_eq!(args.encode_explicit_kernarg_v1(), expected);
    assert!(args.bindings_v1().is_empty());
}

#[test]
fn generated_context_validation_and_backend_alias_policy_reject_without_effects() {
    let mut context = Context::open(Backend::default()).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let stream = context.create_stream(devices[0]).unwrap();
    let module = context
        .load_module(devices[0], b"scripted-not-hsaco")
        .unwrap();
    let kernel = context
        .resolve_kernel::<Arguments>(module, "context_map")
        .unwrap();
    let input = allocate(&mut context, devices[0], 16);
    let output = allocate(&mut context, devices[0], 16);
    let wrong_device = allocate(&mut context, devices[1], 16);
    let stale = allocate(&mut context, devices[0], 16);
    context.release_allocation(stale).unwrap();
    let mut foreign = Context::open(Backend::default()).unwrap();
    let foreign_device = foreign.devices()[0].id();
    let foreign_input = allocate(&mut foreign, foreign_device, 16);
    for (args, expected) in [
        (
            arguments(foreign_input, output, 1.0, 4),
            RuntimeValidationErrorV1::UnknownAllocation,
        ),
        (
            arguments(wrong_device, output, 1.0, 4),
            RuntimeValidationErrorV1::WrongDevice,
        ),
        (
            arguments(stale, output, 1.0, 4),
            RuntimeValidationErrorV1::UnknownAllocation,
        ),
        (
            arguments(input, output, 1.0, 5),
            RuntimeValidationErrorV1::InvalidRange,
        ),
    ] {
        assert!(
            matches!(context.launch(stream, &kernel, &args, geometry(), &[]),
            Err(RuntimeErrorV1::Validation(error)) if error == expected)
        );
        assert_eq!(context.backend().attempts, 0);
        assert_eq!(context.backend().effects, 0);
    }
    assert!(matches!(
        context.launch(
            stream,
            &kernel,
            &arguments(input, input, 1.0, 4),
            geometry(),
            &[]
        ),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(context.backend().attempts, 1);
    assert_eq!(context.backend().effects, 0);
    assert!(
        context
            .resolve_kernel::<transform_gpu::ContextArguments>(module, "context_map")
            .is_err()
    );
    assert_eq!(context.backend().submitted, 0);
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &arguments(input, output, 1.0, 4),
            geometry(),
            &[],
        )
        .unwrap();
    assert!(matches!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    ));
    context.flush_stream(stream).unwrap();
    assert!(matches!(
        context.poll(&mut submission).unwrap(),
        RuntimePollV1::Succeeded
    ));
    context.release_submission(submission).unwrap();
    close(foreign);
    close(context);
}

#[test]
fn generated_context_metadata_never_overrides_backend_execution_denial() {
    let mut context = Context::open(Backend {
        deny_launch: true,
        ..Backend::default()
    })
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let module = context.load_module(device, b"scripted-not-hsaco").unwrap();
    let kernel = context
        .resolve_kernel::<Arguments>(module, "context_map")
        .unwrap();
    let input = allocate(&mut context, device, 16);
    let output = allocate(&mut context, device, 16);
    assert!(matches!(
        context.launch(
            stream,
            &kernel,
            &arguments(input, output, 1.0, 4),
            geometry(),
            &[]
        ),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(context.backend().attempts, 1);
    assert_eq!(context.backend().submitted, 0);
    assert_eq!(context.backend().effects, 0);
    context.write_allocation(output, 0, &[9; 16]).unwrap();
    close(context);
}

#[test]
fn generated_context_queued_compute_gather_compute_readback_uses_only_public_handles() {
    let mut context =
        Context::open_with_version_journal_members_v1(Backend::default(), 16, 16, 32).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let source_stream = context.create_stream(devices[0]).unwrap();
    let gather_stream = context.create_stream(devices[1]).unwrap();
    let consumer_stream = context.create_stream(devices[1]).unwrap();
    let return_stream = context.create_stream(devices[0]).unwrap();
    let readback_stream = context.create_stream(devices[0]).unwrap();
    let source_module = context
        .load_module(devices[0], b"scripted-not-hsaco")
        .unwrap();
    let target_module = context
        .load_module(devices[1], b"scripted-not-hsaco")
        .unwrap();
    let producer_kernel = context
        .resolve_kernel::<Arguments>(source_module, "context_map")
        .unwrap();
    let consumer_kernel = context
        .resolve_kernel::<Arguments>(target_module, "context_map")
        .unwrap();
    let inputs = [
        allocate(&mut context, devices[0], 8),
        allocate(&mut context, devices[0], 8),
    ];
    let outputs = [
        allocate(&mut context, devices[0], 8),
        allocate(&mut context, devices[0], 8),
    ];
    let gathered = allocate(&mut context, devices[1], 24);
    let result = allocate(&mut context, devices[1], 24);
    let returned = allocate(&mut context, devices[0], 24);
    let host = context
        .allocate(devices[0], RuntimeMemoryKindV1::HostVisible, 24, 4)
        .unwrap();
    context.write_allocation(host, 0, &[0xa5; 24]).unwrap();
    let initial = [5.0_f32; 6]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    context.write_allocation(gathered, 0, &initial).unwrap();
    for (input, values) in inputs.into_iter().zip([[1.0_f32, 2.0], [3.0, 4.0]]) {
        context
            .write_allocation(
                input,
                0,
                &values
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect::<Vec<_>>(),
            )
            .unwrap();
    }
    let mut producers = Vec::new();
    let mut peers = Vec::new();
    let mut events = Vec::new();
    let mut latest = None;
    for index in 0..2 {
        let producer = context
            .launch_producer_aware_v1(
                source_stream,
                &producer_kernel,
                &arguments(inputs[index], outputs[index], 2.0, 2),
                geometry(),
                &[],
            )
            .unwrap();
        let event = context.record_event(&producer).unwrap();
        let mut dependencies = vec![event];
        dependencies.extend(latest);
        let peer = context
            .peer_copy(
                gather_stream,
                region(outputs[index], RuntimeAccessV1::Read, 0, 8),
                region(gathered, RuntimeAccessV1::Write, (index * 12) as u64, 8),
                &dependencies,
            )
            .unwrap();
        let peer_event = context.record_event(&peer).unwrap();
        events.extend([event, peer_event]);
        latest = Some(peer_event);
        producers.push(producer);
        peers.push(peer);
    }
    let consumer = context
        .launch_producer_aware_v1(
            consumer_stream,
            &consumer_kernel,
            &arguments(gathered, result, 3.0, 6),
            geometry(),
            &[latest.unwrap()],
        )
        .unwrap();
    let consumer_event = context.record_event(&consumer).unwrap();
    events.push(consumer_event);
    let returning = context
        .peer_copy(
            return_stream,
            region(result, RuntimeAccessV1::Read, 0, 24),
            region(returned, RuntimeAccessV1::Write, 0, 24),
            &[consumer_event],
        )
        .unwrap();
    let return_event = context.record_event(&returning).unwrap();
    events.push(return_event);
    let mut readback = context
        .copy_async(
            readback_stream,
            region(returned, RuntimeAccessV1::Read, 0, 24),
            region(host, RuntimeAccessV1::Write, 0, 24),
            &[return_event],
        )
        .unwrap();
    let callbacks = Arc::new(AtomicUsize::new(0));
    let completed = Arc::clone(&callbacks);
    context
        .on_completion(&readback, move |status| {
            assert_eq!(status, RuntimeCompletionStatusV1::Succeeded);
            completed.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    for event in events {
        context.release_event(event).unwrap();
    }
    assert_eq!(context.backend().submitted, 7);
    assert_eq!(context.backend().effects, 0);
    assert!(matches!(
        context.poll(&mut readback).unwrap(),
        RuntimePollV1::Pending
    ));
    assert_eq!(callbacks.load(Ordering::SeqCst), 0);
    assert!(context.release_allocation(gathered).is_err());
    context.flush_stream(readback_stream).unwrap();
    for _ in 0..16 {
        if matches!(
            context.poll(&mut readback).unwrap(),
            RuntimePollV1::Succeeded
        ) {
            break;
        }
    }
    assert!(matches!(
        context.poll(&mut readback).unwrap(),
        RuntimePollV1::Succeeded
    ));
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
    let mut bytes = [0; 24];
    context.read_allocation(host, 0, &mut bytes).unwrap();
    let expected = [6.0_f32, 12.0, 15.0, 18.0, 24.0, 15.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(bytes.as_slice(), expected);
    assert_eq!(context.backend().effects, 7);
    context.release_submission(readback).unwrap();
    context.release_submission(returning).unwrap();
    context.release_submission(consumer).unwrap();
    for peer in peers.into_iter().rev() {
        context.release_submission(peer).unwrap();
    }
    for producer in producers.into_iter().rev() {
        context.release_submission(producer).unwrap();
    }
    close(context);
}
