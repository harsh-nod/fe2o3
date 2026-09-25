use super::async_journal_tests::{MixedArguments, region, state, writer_state};
use super::peer_directed_tests::Observation;
use super::*;
use fe2o3_runtime_model::{ContextProducerReadStatusV1, ContextWriterStateV1};
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<MockBackend>;
type Submission = RuntimeSubmissionV1<MixedArguments>;

#[derive(Clone, Debug)]
struct CapturedLaunch {
    stream: u64,
    kernel: u64,
    explicit_kernarg: Vec<u8>,
    bindings: Vec<BackendBindingV1>,
    dependencies: Vec<BackendLaunchProducerV1>,
    geometry: RuntimeLaunchGeometryV1,
}

#[derive(Debug, Default)]
pub(super) struct MockProducerLaunchState {
    requests: HashMap<u64, CapturedLaunch>,
    events: HashMap<u64, u64>,
    completed: HashMap<u64, BackendPollV1>,
    observations: HashMap<u64, Observation>,
    calls: Vec<(&'static str, u64)>,
}

fn output_byte(submission: u64) -> u8 {
    (submission as u8).wrapping_add(37)
}

impl MockBackend {
    pub(super) fn is_producer_launch_test_v1(&self, id: u64) -> bool {
        self.producer_launch.requests.contains_key(&id)
    }

    pub(super) fn record_producer_launch_event_test_v1(&mut self, event: u64, producer: u64) {
        self.producer_launch.events.insert(event, producer);
    }

    pub(super) fn release_producer_launch_event_test_v1(&mut self, event: u64) {
        self.producer_launch.events.remove(&event);
    }

    pub(super) fn release_producer_launch_test_v1(&mut self, id: u64) {
        self.producer_launch.requests.remove(&id);
        self.producer_launch.completed.remove(&id);
        self.producer_launch.observations.remove(&id);
    }

    pub(super) fn finish_producer_launch_test_v1(&mut self, id: u64, success: bool) -> bool {
        if !self.is_producer_launch_test_v1(id) {
            return false;
        }
        if self.producer_launch.completed.contains_key(&id) {
            return true;
        }
        if !success {
            self.pending_kernel_reads.remove(&id);
            self.producer_launch
                .completed
                .insert(id, BackendPollV1::Failed { code: 7 });
            return true;
        }
        // Execute the physical graph without manufacturing Context observations.
        let mut stack = vec![(id, false)];
        while let Some((current, visited)) = stack.pop() {
            if self.producer_launch.completed.contains_key(&current) {
                continue;
            }
            let request = self.producer_launch.requests[&current].clone();
            if !visited {
                stack.push((current, true));
                for dependency in request.dependencies.iter().rev() {
                    assert!(dependency.producer_submission < current);
                    if !self
                        .producer_launch
                        .completed
                        .contains_key(&dependency.producer_submission)
                    {
                        stack.push((dependency.producer_submission, false));
                    }
                }
                continue;
            }
            let ready = request.dependencies.iter().all(|dependency| {
                self.producer_launch
                    .completed
                    .get(&dependency.producer_submission)
                    == Some(&BackendPollV1::Succeeded)
            });
            let pending = self.pending_kernel_reads.remove(&current).unwrap();
            assert_eq!(pending.stream, request.stream);
            assert_eq!(pending.bindings, request.bindings);
            if ready {
                // Read every native input before applying this launch's own writes.
                for (ordinal, binding) in request.bindings.iter().enumerate() {
                    if binding.region.access == RuntimeAccessV1::Write {
                        continue;
                    }
                    let start = binding.region.byte_offset as usize;
                    let end = start + binding.region.byte_len as usize;
                    self.observed_kernel_reads.push(MockObservedKernelRead {
                        submission: current,
                        ordinal,
                        binding: *binding,
                        bytes: self.memory[&binding.region.allocation][start..end].to_vec(),
                    });
                }
                for binding in &request.bindings {
                    if binding.region.access != RuntimeAccessV1::Read {
                        let start = binding.region.byte_offset as usize;
                        let end = start + binding.region.byte_len as usize;
                        self.memory.get_mut(&binding.region.allocation).unwrap()[start..end]
                            .fill(output_byte(current));
                    }
                }
            }
            self.producer_launch.completed.insert(
                current,
                if ready {
                    BackendPollV1::Succeeded
                } else {
                    BackendPollV1::Failed { code: 7 }
                },
            );
        }
        true
    }

    pub(super) fn observe_producer_launch_test_v1(
        &mut self,
        kind: &'static str,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<MockError>> {
        self.producer_launch.calls.push((kind, id));
        match self.producer_launch.observations.remove(&id) {
            Some(Observation::Pending) => return Ok(BackendPollV1::Pending),
            Some(Observation::Failed) => {
                self.finish_producer_launch_test_v1(id, false);
                return Ok(BackendPollV1::Failed { code: 7 });
            }
            Some(Observation::Rejected) => {
                return Err(RuntimeBackendFailureV1::Rejected(MockError(
                    "producer launch rejected",
                )));
            }
            Some(Observation::Quiescent) => {
                self.finish_producer_launch_test_v1(id, false);
                return Err(RuntimeBackendFailureV1::Quiescent(MockError(
                    "producer launch quiescent",
                )));
            }
            Some(Observation::Terminal) => {
                return Err(RuntimeBackendFailureV1::Terminal(MockError(
                    "producer launch terminal",
                )));
            }
            Some(Observation::Panic) => panic!("producer launch observation panic"),
            None => {}
        }
        self.finish_producer_launch_test_v1(id, true);
        Ok(self.producer_launch.completed[&id])
    }
}

impl RuntimeProducerAwareLaunchBackendV1 for MockBackend {
    fn submit_producer_aware_launch_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        for (index, dependency) in request.dependencies.iter().enumerate() {
            if self.producer_launch.events.get(&dependency.event)
                != Some(&dependency.producer_submission)
                || !self
                    .producer_launch
                    .requests
                    .contains_key(&dependency.producer_submission)
                || request.dependencies[..index]
                    .iter()
                    .any(|earlier| earlier.producer_submission == dependency.producer_submission)
            {
                return Err(RuntimeBackendFailureV1::Rejected(MockError(
                    "producer binding mismatch",
                )));
            }
        }
        self.submit_count += 1;
        self.last_dependency_count = request.dependencies.len();
        self.last_launch_geometry = Some(request.geometry);
        let failure = core::mem::take(&mut self.launch_failure);
        if failure == MockMemoryFailure::Rejected {
            mock_memory_failure_v1(failure)?;
        }
        let id = self.handle(MockHandleKind::Submission);
        self.polls.insert(id, 0);
        self.pending_kernel_reads.insert(
            id,
            MockPendingKernelReads {
                stream: request.stream,
                bindings: request.bindings.to_vec(),
            },
        );
        self.producer_launch.requests.insert(
            id,
            CapturedLaunch {
                stream: request.stream,
                kernel: request.kernel,
                explicit_kernarg: request.explicit_kernarg.to_vec(),
                bindings: request.bindings.to_vec(),
                dependencies: request.dependencies.to_vec(),
                geometry: request.geometry,
            },
        );
        if failure == MockMemoryFailure::Quiescent {
            self.finish_producer_launch_test_v1(id, false);
        }
        mock_memory_failure_v1(failure)?;
        Ok(id)
    }
}

struct Fixture {
    context: Context,
    streams: [RuntimeStreamIdV1; 3],
    allocations: [RuntimeAllocationIdV1; 6],
    module: RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<MixedArguments>,
}

fn initial_bytes(index: usize) -> Vec<u8> {
    (0..64).map(|offset| (index * 31 + offset) as u8).collect()
}

fn span(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    start: u64,
    len: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        byte_len: len,
        ..region(allocation, access, start)
    }
}

fn validation<T>(result: Result<T, RuntimeErrorV1<MockError>>, expected: RuntimeValidationErrorV1) {
    match result {
        Err(RuntimeErrorV1::Validation(actual)) => assert_eq!(actual, expected),
        _ => panic!("expected exact validation error"),
    }
}

impl Fixture {
    fn new(capacity: usize) -> Self {
        Self::configured(capacity, true)
    }

    fn configured(capacity: usize, journal: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_kernel_reads: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, 16, capacity).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let device = context.devices()[0].id();
        context
            .configure_allocation_admission_v1(device, 1024, 16)
            .unwrap();
        let streams = core::array::from_fn(|_| context.create_stream(device).unwrap());
        let allocations = core::array::from_fn(|index| {
            let allocation = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            context
                .write_allocation(allocation, 0, &initial_bytes(index))
                .unwrap();
            allocation
        });
        let module = context
            .load_module(device, b"producer-launch-tests")
            .unwrap();
        let kernel = context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
        Self {
            context,
            streams,
            allocations,
            module,
            kernel,
        }
    }

    fn launch(
        &mut self,
        stream: usize,
        regions: Vec<RuntimeMemoryRegionV1>,
        events: &[RuntimeEventIdV1],
    ) -> Result<Submission, RuntimeErrorV1<MockError>> {
        self.context.launch_producer_aware_v1(
            self.streams[stream],
            &self.kernel,
            &MixedArguments(regions),
            geometry(),
            events,
        )
    }

    fn producers(&mut self) -> ([Submission; 2], [RuntimeEventIdV1; 2]) {
        let a = self
            .launch(
                0,
                vec![span(self.allocations[0], RuntimeAccessV1::Write, 8, 16)],
                &[],
            )
            .unwrap();
        let b = self
            .launch(
                1,
                vec![span(self.allocations[1], RuntimeAccessV1::Write, 16, 16)],
                &[],
            )
            .unwrap();
        let events = [
            self.context.record_event(&a).unwrap(),
            self.context.record_event(&b).unwrap(),
        ];
        ([a, b], events)
    }

    fn mixed(&self, writable: bool) -> Vec<RuntimeMemoryRegionV1> {
        let mut regions = vec![
            region(self.allocations[1], RuntimeAccessV1::Read, 20),
            region(self.allocations[2], RuntimeAccessV1::Read, 3),
            region(self.allocations[0], RuntimeAccessV1::Read, 12),
            region(self.allocations[1], RuntimeAccessV1::Read, 20),
        ];
        if writable {
            regions.push(region(self.allocations[3], RuntimeAccessV1::Write, 4));
        }
        regions
    }

    fn complete(&mut self, submission: &mut Submission) {
        for _ in 0..16 {
            if self.context.poll(submission).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("bounded producer reconciliation did not finish");
    }

    fn snapshot(&self) -> String {
        let mut retains: Vec<_> = self
            .context
            .submissions
            .iter()
            .map(|(id, record)| (*id, record.dependency_retains, record.status))
            .collect();
        retains.sort_unstable_by_key(|entry| entry.0);
        format!(
            "{:?}",
            (
                self.context.next_identity,
                self.context.backend.submit_count,
                self.context.version_journal_read_records_v1(),
                self.context.version_journal_writer_records_v1(),
                self.allocations.map(|id| state(&self.context, id)),
                retains,
                self.context
                    .allocation_admission_usage_v1(self.context.devices()[0].id())
                    .unwrap(),
                self.context.backend.memory.clone(),
            )
        )
    }

    fn assert_mixed_bytes(&self, consumer: u64, producers: &[Submission; 2]) {
        let request = &self.context.backend.producer_launch.requests[&consumer];
        let reads: Vec<_> = self
            .context
            .backend
            .observed_kernel_reads
            .iter()
            .filter(|read| read.submission == consumer)
            .collect();
        assert_eq!(reads.len(), 4);
        for (ordinal, read) in reads.iter().enumerate() {
            assert_eq!(read.ordinal, ordinal);
            assert_eq!(read.binding, request.bindings[ordinal]);
            let expected = match ordinal {
                0 | 3 => vec![output_byte(producers[1].backend_submission); 8],
                1 => initial_bytes(2)[3..11].to_vec(),
                2 => vec![output_byte(producers[0].backend_submission); 8],
                _ => unreachable!(),
            };
            assert_eq!(read.bytes, expected);
        }
    }
}

#[test]
fn producer_launch_mixed_inputs_preserve_native_ranges_and_whole_allocation_custody() {
    let mut f = Fixture::new(4);
    let (producers, events) = f.producers();
    let regions = f.mixed(true);
    let mut consumer = f
        .launch(2, regions.clone(), &[events[1], events[0]])
        .unwrap();
    let record = f.context.submissions[&consumer.id];
    assert_eq!(record.journal_read.unwrap().count, 1);
    assert_eq!(record.journal_producer_read.unwrap().count, 2);
    assert!(record.journal_writer.is_some());
    assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
    let owner = f
        .context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1();
    let stable = owner
        .lookup_read(record.journal_read.unwrap().first)
        .unwrap();
    let pending = owner
        .lookup_producer_read(record.journal_producer_read.unwrap().first)
        .unwrap();
    assert_eq!((stable.byte_offset, stable.byte_len), (0, 64));
    assert_eq!((pending.read.byte_offset, pending.read.byte_len), (0, 64));
    let request = &f.context.backend.producer_launch.requests[&consumer.backend_submission];
    assert_eq!(request.kernel, f.kernel.backend_kernel);
    assert_eq!(request.explicit_kernarg, vec![0; regions.len() * 8]);
    assert_eq!(request.geometry, geometry());
    assert_eq!(
        request
            .dependencies
            .iter()
            .map(|dep| dep.producer_submission)
            .collect::<Vec<_>>(),
        vec![
            producers[1].backend_submission,
            producers[0].backend_submission
        ]
    );
    for (ordinal, (binding, original)) in request.bindings.iter().zip(&regions).enumerate() {
        assert_eq!(
            binding.region.allocation,
            f.context.allocations[&original.allocation].backend_allocation
        );
        assert_eq!(binding.kernarg_byte_offset, ordinal as u32 * 8);
        assert_eq!(binding.region.byte_offset, original.byte_offset);
        assert_eq!(binding.region.byte_len, original.byte_len);
        assert_eq!(binding.region.access, original.access);
    }
    assert!(f.context.backend.observed_kernel_reads.is_empty());
    for id in &f.allocations[..3] {
        validation(
            f.context.write_allocation(*id, 56, &[9]),
            RuntimeValidationErrorV1::ContextReserved,
        );
    }
    f.complete(&mut consumer);
    f.assert_mixed_bytes(consumer.backend_submission, &producers);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(f.context.backend.flush_call_count, 0);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn producer_launch_writerless_consumer_does_not_need_a_free_writer_slot() {
    let mut f = Fixture::new(3);
    let (producers, events) = f.producers();
    let _unrelated = f
        .launch(
            0,
            vec![region(f.allocations[4], RuntimeAccessV1::Write, 0)],
            &[],
        )
        .unwrap();
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(3));
    let mut consumer = f.launch(2, f.mixed(false), &events).unwrap();
    assert!(f.context.submissions[&consumer.id].journal_writer.is_none());
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(3));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
    f.complete(&mut consumer);
    f.assert_mixed_bytes(consumer.backend_submission, &producers);
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn producer_launch_coverage_uses_every_original_alias_and_writable_interval_union() {
    for case in 0..5 {
        let mut f = Fixture::new(4);
        let a = f.allocations[0];
        let second = if case == 1 || case == 2 { 17 } else { 16 };
        let mut writes = vec![
            span(a, RuntimeAccessV1::Write, second, 24 - second),
            span(a, RuntimeAccessV1::Write, 8, 8),
        ];
        if case == 2 {
            writes.push(span(a, RuntimeAccessV1::Read, 16, 1));
        }
        let producer = f.launch(0, writes, &[]).unwrap();
        let event = f.context.record_event(&producer).unwrap();
        let mut reads = vec![region(a, RuntimeAccessV1::Read, 12)];
        if case == 3 {
            reads.push(region(a, RuntimeAccessV1::Read, 24));
        }
        if case == 4 {
            reads.push(region(a, RuntimeAccessV1::ReadWrite, 12));
        }
        let before = f.snapshot();
        let result = f.launch(1, reads, &[event]);
        if case == 0 {
            let mut consumer = result.unwrap();
            assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            f.complete(&mut consumer);
            let observed = f
                .context
                .backend
                .observed_kernel_reads
                .iter()
                .find(|read| read.submission == consumer.backend_submission)
                .unwrap();
            assert_eq!(
                observed.bytes,
                vec![output_byte(producer.backend_submission); 8]
            );
        } else {
            validation(result, RuntimeValidationErrorV1::ContextReserved);
            assert_eq!(f.snapshot(), before);
            assert!(!f.context.terminal);
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_combined_capacity_and_late_request_rejection_are_pre_issue() {
    for capacity in [2, 3] {
        let mut f = Fixture::new(capacity);
        let (_producers, events) = f.producers();
        let before = f.snapshot();
        let mut regions = f.mixed(false);
        if capacity == 3 {
            regions[0].byte_offset = 40;
        }
        validation(
            f.launch(2, regions, &events),
            if capacity == 2 {
                RuntimeValidationErrorV1::Capacity
            } else {
                RuntimeValidationErrorV1::ContextReserved
            },
        );
        assert_eq!(f.snapshot(), before);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(!f.context.terminal);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_mixed_acquisition_handles_each_empty_side_and_no_inputs() {
    for stable_input in [false, true] {
        for pending_input in [false, true] {
            let mut f = Fixture::new(4);
            let (mut producers, events) = f.producers();
            let mut regions = Vec::new();
            if stable_input {
                regions.push(region(f.allocations[2], RuntimeAccessV1::Read, 3));
            }
            if pending_input {
                regions.push(region(f.allocations[0], RuntimeAccessV1::Read, 12));
            }
            let mut consumer = f.launch(2, regions, &events).unwrap();
            let record = &f.context.submissions[&consumer.id];
            assert_eq!(record.journal_read.is_some(), stable_input);
            assert_eq!(record.journal_producer_read.is_some(), pending_input);
            assert!(record.journal_writer.is_none());
            assert_eq!(
                f.context.version_journal_read_records_v1(),
                Some(usize::from(stable_input) + usize::from(pending_input))
            );
            for event in events {
                f.context.release_event(event).unwrap();
            }
            f.complete(&mut consumer);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            for producer in &mut producers {
                f.complete(producer);
            }
            assert!(!f.context.terminal);
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_mixed_release_prevalidates_both_complete_rosters() {
    for corruption in 0..4 {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let consumer = f.launch(2, f.mixed(true), &events).unwrap();
        match corruption {
            0 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_producer_read_reference_for_test_v1(consumer.id, 1),
            1 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_submission_read_reference_for_test_v1(consumer.id, 0),
            2 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .remove_producer_read_root_for_test_v1(consumer.id),
            3 => {
                f.context
                    .submissions
                    .get_mut(&consumer.id)
                    .unwrap()
                    .journal_producer_read
                    .as_mut()
                    .unwrap()
                    .count += 1
            }
            _ => unreachable!(),
        }
        let calls = f.context.backend.producer_launch.calls.len();
        assert!(
            f.context
                .backend
                .finish_producer_launch_test_v1(consumer.backend_submission, false)
        );
        assert!(f.context.release_submission_inputs_v1(consumer.id).is_err());
        assert_eq!(f.context.backend.producer_launch.calls.len(), calls);
        assert!(f.context.terminal);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        assert!(f.context.submissions[&consumer.id].journal_read.is_some());
        assert!(
            f.context.submissions[&consumer.id]
                .journal_producer_read
                .is_some()
        );
        assert!(f.context.submissions[&consumer.id].journal_writer.is_some());
        for producer in &producers {
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
    }
}

#[test]
fn producer_launch_acquire_error_retains_both_unmarked_original_input_roots() {
    let mut f = Fixture::new(4);
    let (producers, events) = f.producers();
    let before = f.context.backend.submit_count;
    f.context
        .versions
        .as_mut()
        .unwrap()
        .reject_mixed_input_for_test_v1();
    validation(
        f.launch(2, f.mixed(true), &events),
        RuntimeValidationErrorV1::InvalidBackendDescription,
    );
    let id = *f.context.producer_launches.keys().max().unwrap();
    assert!(f.context.terminal);
    assert_eq!(f.context.backend.submit_count, before);
    assert_eq!(
        f.context
            .versions
            .as_ref()
            .unwrap()
            .mixed_input_roots_for_test_v1(id),
        [(1, 0, false), (2, 0, false)]
    );
    assert_eq!(
        f.context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .retained_read_count(),
        0
    );
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(3));
    assert!(!f.context.submissions.contains_key(&id));
    for producer in &producers {
        assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    }
}

#[test]
fn producer_launch_finalization_panic_retains_all_leases_without_publishing_markers() {
    let mut f = Fixture::new(4);
    let (producers, events) = f.producers();
    let before = f.context.backend.submit_count;
    let payload = Arc::new(91u64);
    f.context
        .versions
        .as_mut()
        .unwrap()
        .panic_mixed_finalization_for_test_v1(Box::new(Arc::clone(&payload)));
    let result = catch_unwind(AssertUnwindSafe(|| f.launch(2, f.mixed(true), &events)));
    let Err(panic) = result else {
        panic!("expected original finalization panic");
    };
    let observed = panic.downcast::<Arc<u64>>().unwrap();
    assert!(Arc::ptr_eq(&payload, &observed));
    let id = *f.context.producer_launches.keys().max().unwrap();
    assert!(f.context.terminal);
    assert_eq!(f.context.backend.submit_count, before);
    assert_eq!(
        f.context
            .versions
            .as_ref()
            .unwrap()
            .mixed_input_roots_for_test_v1(id),
        [(1, 0, false), (2, 0, false)]
    );
    assert_eq!(
        f.context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .retained_read_count(),
        3
    );
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(3));
    assert!(!f.context.submissions.contains_key(&id));
    for producer in &producers {
        assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    }
}

#[test]
fn producer_launch_admission_requires_live_exact_nonaliased_profile_producers() {
    let mut plain = Fixture::configured(4, false);
    validation(
        plain.launch(0, vec![], &[]),
        RuntimeValidationErrorV1::Unsupported,
    );
    assert_eq!(plain.context.backend.submit_count, 0);
    assert!(plain.context.cleanup().is_complete());
    for case in 0..5 {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let alias = f.context.record_event(&producers[0]).unwrap();
        let dependencies = match case {
            0 => vec![],
            1 => vec![events[1]],
            2 => vec![events[0], events[0]],
            3 => vec![events[0], alias],
            4 => {
                f.context.release_event(events[0]).unwrap();
                vec![events[0]]
            }
            _ => unreachable!(),
        };
        let before = f.snapshot();
        let result = f.launch(
            2,
            vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
            &dependencies,
        );
        if case == 0 || case == 1 {
            validation(result, RuntimeValidationErrorV1::ContextReserved);
        } else if case == 2 || case == 3 {
            validation(result, RuntimeValidationErrorV1::DuplicateDependency);
        } else {
            assert!(matches!(result, Err(RuntimeErrorV1::Validation(_))));
        }
        assert_eq!(f.snapshot(), before);
        validation(
            f.context.launch(
                f.streams[2],
                &f.kernel,
                &MixedArguments(vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)]),
                geometry(),
                &[alias],
            ),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert!(f.context.cleanup().is_complete());
    }
    let mut f = Fixture::new(4);
    let ordinary = f
        .context
        .launch(
            f.streams[0],
            &f.kernel,
            &MixedArguments(vec![region(f.allocations[0], RuntimeAccessV1::Write, 8)]),
            geometry(),
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&ordinary).unwrap();
    let before = f.snapshot();
    validation(
        f.launch(
            1,
            vec![region(f.allocations[0], RuntimeAccessV1::Read, 8)],
            &[event],
        ),
        RuntimeValidationErrorV1::Unsupported,
    );
    assert_eq!(f.snapshot(), before);
    assert!(f.context.cleanup().is_complete());
    for observation in [Observation::Failed, Observation::Quiescent] {
        let mut f = Fixture::new(4);
        let mut producer = f
            .launch(
                0,
                vec![region(f.allocations[0], RuntimeAccessV1::Write, 8)],
                &[],
            )
            .unwrap();
        let event = f.context.record_event(&producer).unwrap();
        f.context
            .backend
            .producer_launch
            .observations
            .insert(producer.backend_submission, observation);
        let _ = f.context.poll(&mut producer);
        let before = f.snapshot();
        validation(
            f.launch(
                1,
                vec![region(f.allocations[0], RuntimeAccessV1::Read, 8)],
                &[event],
            ),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.snapshot(), before);
        assert!(!f.context.terminal);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_public_event_release_and_fanout_keep_exact_producer_custody() {
    let mut f = Fixture::new(4);
    let producer = f
        .launch(
            0,
            vec![span(f.allocations[0], RuntimeAccessV1::Write, 8, 16)],
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&producer).unwrap();
    let mut first = f
        .launch(
            1,
            vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
            &[event],
        )
        .unwrap();
    let mut second = f
        .launch(
            2,
            vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
            &[event],
        )
        .unwrap();
    f.context.release_event(event).unwrap();
    assert!(
        !f.context
            .backend
            .producer_launch
            .events
            .values()
            .any(|id| *id == producer.backend_submission)
    );
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 2);
    assert!(f.context.unload_module(f.module).is_err());
    f.complete(&mut first);
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    assert!(f.context.release_submission_ref(&producer, None).is_err());
    f.context.poll(&mut first).unwrap();
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
    f.complete(&mut second);
    assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
    f.context.release_submission(producer).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn producer_launch_initial_failures_preserve_diagnostics_and_uncertain_mixed_roots() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        f.context.backend.launch_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.launch(2, f.mixed(true), &events)));
        match failure {
            MockMemoryFailure::Rejected => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendRejected(MockError(
                    "allocation rejected"
                )))
            )),
            MockMemoryFailure::Quiescent => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "allocation quiescent failure"
                )))
            )),
            MockMemoryFailure::Terminal => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(MockError(
                    "allocation terminal failure"
                )))
            )),
            MockMemoryFailure::Panic => assert!(result.is_err()),
            MockMemoryFailure::None => unreachable!(),
        }
        let retained = matches!(
            failure,
            MockMemoryFailure::Terminal | MockMemoryFailure::Panic
        );
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if retained { 3 } else { 0 })
        );
        for producer in &producers {
            assert_eq!(
                f.context.submissions[&producer.id].dependency_retains,
                usize::from(retained)
            );
        }
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        assert_eq!(f.context.terminal, retained);
        if !retained {
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_non_success_observations_release_only_quiescent_mixed_inputs() {
    for observation in [
        Observation::Pending,
        Observation::Failed,
        Observation::Rejected,
        Observation::Quiescent,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
        f.context
            .backend
            .producer_launch
            .observations
            .insert(consumer.backend_submission, observation);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut consumer)));
        match observation {
            Observation::Pending => assert!(matches!(result.unwrap(), Ok(RuntimePollV1::Pending))),
            Observation::Failed => assert!(matches!(
                result.unwrap(),
                Ok(RuntimePollV1::Failed { code: 7 })
            )),
            Observation::Rejected => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendRejected(MockError(
                    "producer launch rejected"
                )))
            )),
            Observation::Quiescent => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "producer launch quiescent"
                )))
            )),
            Observation::Terminal => assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(MockError(
                    "producer launch terminal"
                )))
            )),
            Observation::Panic => assert!(result.is_err()),
        }
        let released = matches!(observation, Observation::Failed | Observation::Quiescent);
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if released { 0 } else { 3 })
        );
        for producer in &producers {
            assert_eq!(
                f.context.submissions[&producer.id].dependency_retains,
                usize::from(!released)
            );
        }
        assert_eq!(f.context.backend.producer_launch.calls.len(), 1);
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        if !matches!(observation, Observation::Terminal | Observation::Panic) {
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_all_completion_ingresses_share_bounded_producer_first_reconciliation() {
    for ingress in 0..7 {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
        let completion = f.context.record_event(&consumer).unwrap();
        for event in events {
            f.context.release_event(event).unwrap();
        }
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        for submission in [&producers[0], &producers[1], &consumer] {
            let calls = callbacks.clone();
            let id = submission.id;
            f.context
                .on_completion(submission, move |status| {
                    calls.lock().unwrap().push((id, status))
                })
                .unwrap();
        }
        for step in 0..3 {
            match ingress {
                0 => {
                    f.context.poll(&mut consumer).unwrap();
                }
                1 => {
                    f.context.wait(&mut consumer, Duration::ZERO).unwrap();
                }
                2 => {
                    f.context.poll_event(completion).unwrap();
                }
                3 => {
                    f.context.wait_event(completion, Duration::ZERO).unwrap();
                }
                4 => {
                    f.context
                        .synchronize_stream(f.streams[2], Duration::ZERO)
                        .unwrap();
                }
                5 => {
                    f.context
                        .drain(&mut consumer, Instant::now() + Duration::from_secs(1))
                        .unwrap();
                }
                6 => {
                    f.context.poll_async_drain_v1(consumer.id).unwrap();
                }
                _ => unreachable!(),
            }
            assert_eq!(f.context.backend.producer_launch.calls.len(), step + 1);
            assert_eq!(
                f.context.backend.producer_launch.calls[step].1,
                if step == 0 {
                    consumer.backend_submission
                } else {
                    producers[step - 1].backend_submission
                }
            );
            assert_eq!(
                f.context.query_submission(&consumer).unwrap(),
                if step == 2 {
                    RuntimeCompletionStatusV1::Succeeded
                } else {
                    RuntimeCompletionStatusV1::Pending
                }
            );
        }
        assert_eq!(
            callbacks
                .lock()
                .unwrap()
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>(),
            vec![producers[0].id, producers[1].id, consumer.id]
        );
        f.assert_mixed_bytes(consumer.backend_submission, &producers);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.backend.flush_call_count, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_retained_consumer_success_cannot_override_producer_results() {
    for observation in [
        Observation::Pending,
        Observation::Failed,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        let mut consumer = f.launch(2, f.mixed(false), &events).unwrap();
        assert_eq!(
            f.context.poll(&mut consumer).unwrap(),
            RuntimePollV1::Pending
        );
        f.context
            .backend
            .producer_launch
            .observations
            .insert(producers[0].backend_submission, observation);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut consumer)));
        if matches!(observation, Observation::Panic) {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(f.context.terminal);
        assert_eq!(
            f.context.query_submission(&consumer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        for producer in &producers {
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
    }
}

#[test]
fn producer_launch_discarded_producer_result_preserves_quiescence_without_success() {
    for writable in [false, true] {
        for discarded in 0..2 {
            let mut f = Fixture::new(4);
            let (mut producers, events) = f.producers();
            let mut consumer = f.launch(2, f.mixed(writable), &events).unwrap();
            for event in events {
                f.context.release_event(event).unwrap();
            }
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            for submission in [&producers[0], &producers[1], &consumer] {
                let calls = callbacks.clone();
                let id = submission.id;
                f.context
                    .on_completion(submission, move |status| {
                        calls.lock().unwrap().push((id, status))
                    })
                    .unwrap();
            }
            assert_eq!(
                f.context.poll(&mut consumer).unwrap(),
                RuntimePollV1::Pending
            );
            for submission in [&producers[0], &producers[1], &consumer] {
                assert_eq!(
                    f.context.backend.producer_launch.completed[&submission.backend_submission],
                    BackendPollV1::Succeeded
                );
            }
            f.assert_mixed_bytes(consumer.backend_submission, &producers);
            for _ in 0..discarded {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            f.context.backend.producer_launch.observations.insert(
                producers[discarded].backend_submission,
                Observation::Quiescent,
            );
            assert!(matches!(
                f.context.poll(&mut consumer),
                Err(RuntimeErrorV1::BackendQuiescent(MockError(
                    "producer launch quiescent"
                )))
            ));
            let unknown = RuntimeCompletionStatusV1::QuiescentWithoutResult;
            assert_eq!(f.context.query_submission(&consumer).unwrap(), unknown);
            assert!(f.context.retained_directed_success_v1(consumer.id));
            assert_eq!(
                f.context.query_submission(&producers[discarded]).unwrap(),
                unknown
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.context.terminal);
            for producer in &producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
            }
            assert_eq!(
                writer_state(&f.context, f.allocations[discarded]),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
            if writable {
                assert_eq!(
                    writer_state(&f.context, f.allocations[3]),
                    ContextWriterStateV1::Unknown { member_count: 1 }
                );
            }
            let mut expected: Vec<_> = producers[..discarded]
                .iter()
                .map(|producer| (producer.id, RuntimeCompletionStatusV1::Succeeded))
                .collect();
            expected.extend([(producers[discarded].id, unknown), (consumer.id, unknown)]);
            assert_eq!(*callbacks.lock().unwrap(), expected);
            let calls = f.context.backend.producer_launch.calls.len();
            for submission in [&mut producers[discarded], &mut consumer] {
                assert_eq!(
                    f.context.poll(submission).unwrap(),
                    RuntimePollV1::Failed {
                        code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1
                    }
                );
            }
            assert_eq!(f.context.backend.producer_launch.calls.len(), calls);
            assert_eq!(*callbacks.lock().unwrap(), expected);
            for producer in &mut producers[discarded + 1..] {
                assert_eq!(
                    f.context.query_submission(producer).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
                f.complete(producer);
                expected.push((producer.id, RuntimeCompletionStatusV1::Succeeded));
            }
            assert_eq!(*callbacks.lock().unwrap(), expected);
            assert_eq!(
                f.context.version_journal_writer_records_v1(),
                Some(1 + usize::from(writable))
            );
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_resolved_reservation_survives_writer_slot_reuse() {
    for cancel in [false, true] {
        let mut f = Fixture::new(3);
        let mut producer = f
            .launch(
                0,
                vec![span(f.allocations[0], RuntimeAccessV1::Write, 8, 16)],
                &[],
            )
            .unwrap();
        let event = f.context.record_event(&producer).unwrap();
        let mut consumer = f
            .launch(
                1,
                vec![region(f.allocations[0], RuntimeAccessV1::Read, 12)],
                &[event],
            )
            .unwrap();
        let old = f.context.submissions[&producer.id].journal_writer.unwrap();
        let marker = f.context.submissions[&consumer.id]
            .journal_producer_read
            .unwrap();
        if cancel {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut producer).unwrap();
        } else {
            f.complete(&mut producer);
        }
        let unrelated = f
            .launch(
                2,
                vec![region(f.allocations[4], RuntimeAccessV1::Write, 0)],
                &[],
            )
            .unwrap();
        let fresh = f.context.submissions[&unrelated.id].journal_writer.unwrap();
        assert_eq!(old.slot, fresh.slot);
        assert_ne!(old.key, fresh.key);
        assert_eq!(
            f.context
                .versions
                .as_mut()
                .unwrap()
                .read_leases_for_test_v1()
                .producer_read_status(marker.first)
                .unwrap(),
            if cancel {
                ContextProducerReadStatusV1::NoEffect
            } else {
                ContextProducerReadStatusV1::Success
            }
        );
        if cancel {
            assert!(matches!(
                f.context.poll(&mut consumer).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            ));
            assert!(
                f.context
                    .backend
                    .observed_kernel_reads
                    .iter()
                    .all(|read| read.submission != consumer.backend_submission)
            );
        } else {
            f.complete(&mut consumer);
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn producer_launch_invalid_handles_retain_new_roots_and_every_mixed_input() {
    for duplicate in [false, true] {
        let mut f = Fixture::new(4);
        let (producers, events) = f.producers();
        for producer in &producers {
            f.context
                .on_completion(producer, |_| panic!("ambiguous handle must not notify"))
                .unwrap();
        }
        let handle = if duplicate {
            producers[0].backend_submission
        } else {
            0
        };
        let local = f.context.next_identity;
        f.context.backend.handle_override = Some((MockHandleKind::Submission, handle));
        assert!(matches!(
            f.launch(2, f.mixed(true), &events),
            Err(RuntimeErrorV1::BackendProtocol(_))
        ));
        let id = RuntimeSubmissionIdV1::new(f.context.context_generation, local);
        assert!(f.context.terminal);
        assert!(f.context.producer_launches.contains_key(&id));
        let record = f.context.submissions[&id];
        assert!(record.producer_launch);
        assert_eq!(record.journal_read.unwrap().count, 1);
        assert_eq!(record.journal_producer_read.unwrap().count, 2);
        assert!(record.journal_writer.is_some());
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        for producer in &producers {
            assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
        }
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        let pending = f.context.backend.pending_kernel_reads.clone();
        for _ in 0..2 {
            let report = f.context.cleanup();
            assert!(!report.is_complete());
            assert_eq!(report.reader_journal_records_v1(), 3);
            assert_eq!(f.context.backend.pending_kernel_reads, pending);
        }
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert!(f.context.backend.cleanup_log.is_empty());
    }
}

#[test]
fn producer_launch_cancellation_keeps_or_releases_the_complete_mixed_roster() {
    for writable in [false, true] {
        for case in 0..5 {
            let mut f = Fixture::new(4);
            let (producers, events) = f.producers();
            let mut consumer = f.launch(2, f.mixed(writable), &events).unwrap();
            assert_eq!(
                f.context.submissions[&consumer.id].journal_writer.is_some(),
                writable
            );
            if case == 0 {
                f.context
                    .backend
                    .producer_launch
                    .observations
                    .insert(consumer.backend_submission, Observation::Pending);
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            if case == 3 {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            f.context.backend.cancel_before_publication = case == 2 || case == 3;
            if case == 1 {
                f.context.backend.cancel_failure = MockMemoryFailure::Rejected;
            }
            if case == 4 {
                f.context.backend.cancel_failure = MockMemoryFailure::Quiescent;
            }
            let result = f.context.cancel(&mut consumer);
            match case {
                0 | 3 => assert!(matches!(result, Ok(RuntimeCancellationV1::TooLate))),
                1 => assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::BackendRejected(MockError(
                        "allocation rejected"
                    )))
                )),
                2 => assert!(matches!(result, Ok(RuntimeCancellationV1::Cancelled))),
                4 => assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::BackendQuiescent(MockError(
                        "allocation quiescent failure"
                    )))
                )),
                _ => unreachable!(),
            }
            let released = case == 2 || case == 4;
            assert_eq!(f.context.backend.cancel_call_count, usize::from(case != 3));
            assert_eq!(
                f.context.version_journal_read_records_v1(),
                Some(if released { 0 } else { 3 })
            );
            for producer in &producers {
                assert_eq!(
                    f.context.submissions[&producer.id].dependency_retains,
                    usize::from(!released)
                );
            }
            if case != 3 {
                assert!(f.context.backend.observed_kernel_reads.is_empty());
            }
            assert!(!f.context.terminal);
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn producer_launch_root_flag_dependency_and_late_marker_loss_seal_before_observation() {
    for late in [false, true] {
        for corruption in 0..4 {
            let mut f = Fixture::new(4);
            let (producers, events) = f.producers();
            let mut consumer = f.launch(2, f.mixed(true), &events).unwrap();
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            for submission in [&producers[0], &producers[1], &consumer] {
                let calls = callbacks.clone();
                f.context
                    .on_completion(submission, move |status| calls.lock().unwrap().push(status))
                    .unwrap();
            }
            if late {
                assert_eq!(
                    f.context.poll(&mut consumer).unwrap(),
                    RuntimePollV1::Pending
                );
            }
            match corruption {
                0 => {
                    f.context.producer_launches.remove(&consumer.id);
                }
                1 => {
                    f.context
                        .submissions
                        .get_mut(&consumer.id)
                        .unwrap()
                        .producer_launch = false;
                }
                2 => {
                    f.context
                        .producer_launches
                        .get_mut(&consumer.id)
                        .unwrap()
                        .dependencies
                        .pop();
                }
                3 => {
                    f.context
                        .submissions
                        .get_mut(&consumer.id)
                        .unwrap()
                        .journal_producer_read
                        .as_mut()
                        .unwrap()
                        .count += 1;
                }
                _ => unreachable!(),
            }
            let calls = f.context.backend.producer_launch.calls.len();
            assert!(f.context.poll(&mut consumer).is_err());
            assert_eq!(f.context.backend.producer_launch.calls.len(), calls);
            assert!(f.context.terminal);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
            assert!(f.context.submissions[&consumer.id].journal_read.is_some());
            assert!(
                f.context.submissions[&consumer.id]
                    .journal_producer_read
                    .is_some()
            );
            for producer in &producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
            }
            assert!(callbacks.lock().unwrap().is_empty());
        }
    }
}

#[test]
fn producer_launch_discarded_observer_keeps_inputs_until_consumer_stream_quiesces() {
    for writable in [false, true] {
        for quiescent_error in [false, true] {
            let mut f = Fixture::new(4);
            let (mut producers, events) = f.producers();
            let callbacks = Arc::new(Mutex::new(Vec::new()));
            let (id, backend_id) = {
                let consumer = f.launch(2, f.mixed(writable), &events).unwrap();
                let calls = callbacks.clone();
                f.context
                    .on_completion(&consumer, move |status| calls.lock().unwrap().push(status))
                    .unwrap();
                (consumer.id, consumer.backend_submission)
            };
            for event in events {
                f.context.release_event(event).unwrap();
            }
            assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
            for producer in &producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 1);
            }
            assert!(f.context.unload_module(f.module).is_err());
            assert!(f.context.backend.observed_kernel_reads.is_empty());
            if quiescent_error {
                f.context.backend.cleanup_failure = MockCleanupFailure::QuiescentStreamOnce;
            }
            let result = f.context.destroy_stream(f.streams[2]);
            if quiescent_error {
                assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::BackendQuiescent(MockError(
                        "destroy failed after quiescence"
                    )))
                ));
            } else {
                result.unwrap();
            }
            let record = f.context.submissions[&id];
            assert_eq!(
                record.status,
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert!(record.quiescent);
            assert!(record.journal_read.is_none());
            assert!(record.journal_producer_read.is_none());
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.context.producer_launches[&id].dependencies_held);
            assert_eq!(
                callbacks.lock().unwrap().as_slice(),
                &[RuntimeCompletionStatusV1::QuiescentWithoutResult]
            );
            assert_eq!(
                f.context.backend.producer_launch.completed[&backend_id],
                BackendPollV1::Succeeded
            );
            f.assert_mixed_bytes(backend_id, &producers);
            for producer in &mut producers {
                assert_eq!(f.context.submissions[&producer.id].dependency_retains, 0);
                f.complete(producer);
            }
            assert_eq!(
                f.context.version_journal_writer_records_v1(),
                Some(usize::from(writable))
            );
            f.context.unload_module(f.module).unwrap();
            assert!(!f.context.modules.contains_key(&f.module));
            assert!(f.context.cleanup().is_complete());
            assert_eq!(callbacks.lock().unwrap().len(), 1);
            assert!(!f.context.terminal);
        }
    }
}
