use super::async_journal_tests::{MixedArguments, region, state};
use super::*;

type Context = RuntimeContextV1<MockBackend>;

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    allocations: [RuntimeAllocationIdV1; 3],
    kernel: TypedRuntimeKernelV1<MixedArguments>,
}

fn pattern(buffer: usize) -> Vec<u8> {
    (0..64).map(|index| (buffer * 71 + index) as u8).collect()
}

impl Fixture {
    fn new(readers: usize) -> Self {
        Self::with_journal(readers, true)
    }

    fn with_journal(readers: usize, journal: bool) -> Self {
        Self::configured(readers, journal, true)
    }

    fn configured(readers: usize, journal: bool, credits: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_kernel_reads: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, 16, readers).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let device = context.devices()[0].id();
        if credits {
            context
                .configure_allocation_admission_v1(device, 1024, 16)
                .unwrap();
        }
        let stream = context.create_stream(device).unwrap();
        let allocations = core::array::from_fn(|index| {
            let allocation = context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            context
                .write_allocation(allocation, 0, &pattern(index))
                .unwrap();
            allocation
        });
        let module = context.load_module(device, b"read-inputs").unwrap();
        let kernel = context
            .resolve_kernel::<MixedArguments>(module, "read")
            .unwrap();
        Self {
            context,
            stream,
            allocations,
            kernel,
        }
    }

    fn launch(
        &mut self,
        regions: Vec<RuntimeMemoryRegionV1>,
    ) -> RuntimeSubmissionV1<MixedArguments> {
        self.context
            .launch(
                self.stream,
                &self.kernel,
                &MixedArguments(regions),
                geometry(),
                &[],
            )
            .unwrap()
    }

    fn reads(&self) -> MixedArguments {
        MixedArguments(
            self.allocations[..2]
                .iter()
                .map(|&id| region(id, RuntimeAccessV1::Read, 8))
                .collect(),
        )
    }

    fn backend_bindings(&self, args: &MixedArguments) -> Vec<BackendBindingV1> {
        args.bindings_v1()
            .iter()
            .map(|binding| BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: self.context.allocations[&binding.region.allocation]
                        .backend_allocation,
                    access: binding.region.access,
                    byte_offset: binding.region.byte_offset,
                    byte_len: binding.region.byte_len,
                },
                kernarg_byte_offset: binding.kernarg_byte_offset,
            })
            .collect()
    }

    fn assert_observed(&self, submission: u64, args: &MixedArguments) {
        let bindings = self.backend_bindings(args);
        let expected: Vec<_> = args
            .0
            .iter()
            .enumerate()
            .filter_map(|(ordinal, region)| {
                if region.access == RuntimeAccessV1::Write {
                    return None;
                }
                let buffer = self
                    .allocations
                    .iter()
                    .position(|&id| id == region.allocation)
                    .unwrap();
                let start = region.byte_offset as usize;
                let end = start + region.byte_len as usize;
                Some(MockObservedKernelRead {
                    submission,
                    ordinal,
                    binding: bindings[ordinal],
                    bytes: pattern(buffer)[start..end].to_vec(),
                })
            })
            .collect();
        let actual: Vec<_> = self
            .context
            .backend
            .observed_kernel_reads
            .iter()
            .filter(|read| read.submission == submission)
            .collect();
        assert_eq!(actual, expected.iter().collect::<Vec<_>>());
    }
}

fn reserved<T>(result: Result<T, RuntimeErrorV1<MockError>>) {
    assert!(matches!(
        result,
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
}

#[test]
fn pure_read_batch_preserves_ordered_bindings_and_excludes_whole_allocation_writes() {
    let mut f = Fixture::new(4);
    let [a, b, c] = f.allocations;
    let args = MixedArguments(vec![
        region(b, RuntimeAccessV1::Read, 24),
        region(a, RuntimeAccessV1::Read, 0),
        region(a, RuntimeAccessV1::Read, 4),
        region(a, RuntimeAccessV1::Read, 32),
        region(b, RuntimeAccessV1::Read, 24),
    ]);
    let bindings = f.backend_bindings(&args);
    let mut submission = f.launch(args.0.clone());
    assert!(
        f.context.submissions[&submission.id]
            .journal_writer
            .is_none()
    );
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert_eq!(
        f.context.backend.pending_kernel_reads[&submission.backend_submission].bindings,
        bindings
    );
    assert!(f.context.backend.observed_kernel_reads.is_empty());
    let marker = f.context.submissions[&submission.id].journal_read.unwrap();
    assert_eq!(marker.count, 2);
    let lease = f
        .context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .lookup_read(marker.first)
        .unwrap();
    assert_eq!(lease.allocation.key.local, a.local);
    assert_eq!((lease.byte_offset, lease.byte_len), (0, 64));
    let before = (
        f.context.next_identity,
        f.context.backend.submit_count,
        f.context.backend.write_call_count,
        f.context.backend.cleanup_log.clone(),
    );
    for id in [a, b] {
        reserved(f.context.write_allocation(id, 56, &[9]));
        reserved(f.context.release_allocation(id));
        reserved(f.context.launch(
            f.stream,
            &f.kernel,
            &MixedArguments(vec![region(id, RuntimeAccessV1::Write, 0)]),
            geometry(),
            &[],
        ));
        reserved(f.context.copy_async(
            f.stream,
            region(c, RuntimeAccessV1::Read, 0),
            region(id, RuntimeAccessV1::Write, 0),
            &[],
        ));
    }
    assert_eq!(
        (
            f.context.next_identity,
            f.context.backend.submit_count,
            f.context.backend.write_call_count,
            f.context.backend.cleanup_log.clone()
        ),
        before
    );
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert_eq!(
        f.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert!(f.context.backend.observed_kernel_reads.is_empty());
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    f.assert_observed(submission.backend_submission, &args);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    f.context.write_allocation(a, 56, &[9]).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn mixed_aliases_use_one_writer_and_only_independent_readers() {
    for access in [RuntimeAccessV1::Write, RuntimeAccessV1::ReadWrite] {
        let mut f = Fixture::new(1);
        let [a, b, _] = f.allocations;
        let args = MixedArguments(vec![
            region(a, RuntimeAccessV1::Read, 0),
            region(b, RuntimeAccessV1::Read, 8),
            region(a, access, 16),
        ]);
        let mut submission = f.launch(args.0.clone());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        assert!(state(&f.context, a).pending_writer.is_some());
        assert!(state(&f.context, b).pending_writer.is_none());
        let reference = f.context.submissions[&submission.id]
            .journal_read
            .unwrap()
            .first;
        let lease = f
            .context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .lookup_read(reference)
            .unwrap();
        assert_eq!(lease.allocation.key.local, b.local);
        f.context.wait(&mut submission, Duration::ZERO).unwrap();
        f.assert_observed(submission.backend_submission, &args);
        assert_eq!(state(&f.context, a).content_lineage, 2);
        assert_eq!(state(&f.context, b).content_lineage, 1);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn reader_capacity_is_independent_and_batch_exhaustion_is_pre_issue() {
    let mut f = Fixture::new(2);
    let [a, b, c] = f.allocations;
    let mut writer = f.launch(vec![region(a, RuntimeAccessV1::Write, 0)]);
    let args = MixedArguments(vec![
        region(b, RuntimeAccessV1::Read, 0),
        region(c, RuntimeAccessV1::Read, 8),
    ]);
    let mut reader = f.launch(args.0.clone());
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    let device = f.context.devices()[0].id();
    let before = (
        f.context.next_identity,
        f.context.backend.submit_count,
        f.context.allocation_admission_usage_v1(device).unwrap(),
        state(&f.context, b),
    );
    assert!(matches!(
        f.context
            .launch(f.stream, &f.kernel, &args, geometry(), &[]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(
        (
            f.context.next_identity,
            f.context.backend.submit_count,
            f.context.allocation_admission_usage_v1(device).unwrap(),
            state(&f.context, b)
        ),
        before
    );
    f.context.wait(&mut reader, Duration::ZERO).unwrap();
    let mut retry = f.launch(args.0.clone());
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    f.context.wait(&mut retry, Duration::ZERO).unwrap();
    f.context.wait(&mut writer, Duration::ZERO).unwrap();
    f.assert_observed(reader.backend_submission, &args);
    f.assert_observed(retry.backend_submission, &args);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn out_of_order_shared_readers_survive_stream_destruction_and_old_observers() {
    let mut f = Fixture::new(3);
    let args = MixedArguments(vec![region(f.allocations[0], RuntimeAccessV1::Read, 8)]);
    let original_stream = f.stream;
    let mut first = f.launch(args.0.clone());
    f.stream = f
        .context
        .create_stream(f.context.devices()[0].id())
        .unwrap();
    let second = f.launch(args.0.clone());
    f.context.destroy_stream(f.stream).unwrap();
    f.assert_observed(second.backend_submission, &args);
    assert!(
        f.context
            .backend
            .pending_kernel_reads
            .contains_key(&first.backend_submission)
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    reserved(f.context.release_allocation(f.allocations[0]));
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    f.stream = original_stream;
    let mut third = f.launch(args.0.clone());
    let calls = f.context.backend.wait_call_count;
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    assert_eq!(f.context.backend.wait_call_count, calls);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    reserved(f.context.write_allocation(f.allocations[0], 0, &[9]));
    f.context.wait(&mut third, Duration::ZERO).unwrap();
    f.assert_observed(first.backend_submission, &args);
    f.assert_observed(third.backend_submission, &args);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn initial_read_only_failures_keep_original_diagnostics_and_exact_source_custody() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new(2);
        let args = f.reads();
        f.context.backend.launch_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.context
                .launch(f.stream, &f.kernel, &args, geometry(), &[])
        }));
        match (failure, result) {
            (MockMemoryFailure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(error)))) => {
                assert_eq!(error.0, "allocation rejected")
            }
            (MockMemoryFailure::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(error)))) => {
                assert_eq!(error.0, "allocation quiescent failure")
            }
            (MockMemoryFailure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                assert_eq!(error.0, "allocation terminal failure")
            }
            (MockMemoryFailure::Panic, Err(payload)) => assert_eq!(
                payload.downcast_ref::<&str>(),
                Some(&"scripted allocation adapter panic")
            ),
            _ => panic!("original launch failure replaced"),
        }
        let retained = matches!(
            failure,
            MockMemoryFailure::Terminal | MockMemoryFailure::Panic
        );
        assert_eq!(f.context.is_terminal(), retained);
        assert!(f.context.submissions.is_empty());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(if retained { 2 } else { 0 })
        );
        let usage = f
            .context
            .allocation_admission_usage_v1(f.context.devices()[0].id())
            .unwrap()
            .unwrap();
        assert_eq!(usage.quarantined_records, if retained { 2 } else { 0 });
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::AllocationRecords),
            3
        );
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
            192
        );
        if retained {
            let pending = f.context.backend.pending_kernel_reads.clone();
            assert_eq!(pending.len(), 1);
            assert_eq!(
                pending.values().next().unwrap().bindings,
                f.backend_bindings(&args)
            );
            assert!(f.context.backend.observed_kernel_reads.is_empty());
            for _ in 0..2 {
                let report = f.context.cleanup();
                assert!(!report.is_complete());
                assert_eq!(report.reader_journal_records_v1(), 2);
            }
            assert_eq!(f.context.backend.pending_kernel_reads, pending);
            assert!(f.context.backend.cleanup_log.is_empty());
        } else {
            assert!(f.context.backend.pending_kernel_reads.is_empty());
            if failure == MockMemoryFailure::Quiescent {
                f.assert_observed(f.context.backend.next, &args);
            } else {
                assert!(f.context.backend.observed_kernel_reads.is_empty());
                assert!(f.context.backend.polls.is_empty());
            }
            f.context
                .write_allocation(f.allocations[0], 0, &[9])
                .unwrap();
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn every_exact_completion_route_releases_read_only_batch() {
    for route in 0..7 {
        let mut f = Fixture::new(2);
        let args = f.reads();
        let mut submission = f.launch(args.0.clone());
        match route {
            0 => {
                f.context.poll(&mut submission).unwrap();
                f.context.poll(&mut submission).unwrap();
            }
            1 => {
                f.context.wait(&mut submission, Duration::ZERO).unwrap();
            }
            2 => {
                let event = f.context.record_event(&submission).unwrap();
                f.context.wait_event(event, Duration::ZERO).unwrap();
            }
            3 => {
                f.context
                    .drain(&mut submission, Instant::now() + Duration::from_secs(1))
                    .unwrap();
            }
            4 => {
                f.context
                    .synchronize_stream(f.stream, Duration::ZERO)
                    .unwrap();
            }
            5 => {
                f.context.destroy_stream(f.stream).unwrap();
            }
            _ => {
                f.context.backend.cleanup_failure = MockCleanupFailure::QuiescentStreamOnce;
                assert!(matches!(
                    f.context.destroy_stream(f.stream),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
        }
        f.assert_observed(submission.backend_submission, &args);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        f.context
            .write_allocation(f.allocations[0], 0, &[9])
            .unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn failed_quiescent_and_cancelled_readers_release_without_observing_inputs() {
    for route in 0..4 {
        let mut f = Fixture::new(2);
        let args = f.reads();
        let before = state(&f.context, f.allocations[0]);
        let mut submission = f.launch(args.0.clone());
        match route {
            0 => {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: -9 });
                assert_eq!(
                    f.context.wait(&mut submission, Duration::ZERO).unwrap(),
                    RuntimePollV1::Failed { code: -9 }
                );
            }
            1 => {
                f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    f.context.wait(&mut submission, Duration::ZERO),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            2 => {
                f.context.backend.cancel_failure = MockMemoryFailure::Quiescent;
                assert!(matches!(
                    f.context.cancel(&mut submission),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => {
                f.context.backend.cancel_before_publication = true;
                assert_eq!(
                    f.context.cancel(&mut submission).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
            }
        }
        assert_eq!(state(&f.context, f.allocations[0]), before);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        assert!(f.context.backend.pending_kernel_reads.is_empty());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn pending_rejected_and_too_late_observations_keep_read_only_inputs() {
    let mut f = Fixture::new(2);
    let args = f.reads();
    let mut submission = f.launch(args.0.clone());
    f.context.backend.first_wait_failure = MockWaitFailure::RejectFirst;
    assert!(matches!(
        f.context.wait(&mut submission, Duration::ZERO),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    f.context.backend.wait_observation = Some(BackendPollV1::Pending);
    assert_eq!(
        f.context.wait(&mut submission, Duration::ZERO).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        f.context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::TooLate
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert!(f.context.backend.observed_kernel_reads.is_empty());
    reserved(f.context.write_allocation(f.allocations[0], 0, &[9]));
    f.context.backend.wait_observation = None;
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    f.assert_observed(submission.backend_submission, &args);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn corrupted_batch_member_or_marker_retains_all_leases_atomically() {
    for corruption in 0..6 {
        let mut f = Fixture::new(3);
        let args = MixedArguments(
            f.allocations
                .iter()
                .map(|&id| region(id, RuntimeAccessV1::Read, 0))
                .collect(),
        );
        let mut submission = f.launch(args.0.clone());
        f.context
            .on_completion(&submission, |_| panic!("must not notify corrupt reader"))
            .unwrap();
        match corruption {
            0 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .remove_submission_readers_for_test_v1(submission.id),
            1 => {
                f.context
                    .submissions
                    .get_mut(&submission.id)
                    .unwrap()
                    .journal_read
                    .as_mut()
                    .unwrap()
                    .count -= 1
            }
            2 => {
                f.context
                    .submissions
                    .get_mut(&submission.id)
                    .unwrap()
                    .journal_read
                    .as_mut()
                    .unwrap()
                    .first
                    .consumer
                    .local += 1
            }
            index => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_submission_read_reference_for_test_v1(submission.id, index - 3),
        }
        assert!(matches!(
            f.context.wait(&mut submission, Duration::ZERO),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidBackendDescription
            ))
        ));
        assert!(f.context.is_terminal());
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        assert_eq!(
            f.context.submissions[&submission.id].status,
            RuntimeCompletionStatusV1::Pending
        );
        let report = f.context.cleanup();
        assert!(!report.is_complete());
        assert_eq!(report.reader_journal_records_v1(), 3);
    }
}

#[test]
fn prepared_reader_rejects_stale_identity_even_without_journal() {
    for journal in [false, true] {
        for writable_alias in [false, true] {
            let mut f = Fixture::with_journal(2, journal);
            let mut args = f.reads();
            if writable_alias {
                args.0
                    .push(region(f.allocations[0], RuntimeAccessV1::Write, 16));
            }
            let action = f
                .context
                .prepare_graph_launch_v1(
                    f.stream,
                    &f.kernel,
                    &args.encode_explicit_kernarg_v1(),
                    &args.bindings_v1(),
                    geometry(),
                )
                .unwrap();
            let old = f.allocations[0];
            let backend_id = f.context.allocations[&old].backend_allocation;
            f.context.release_allocation(old).unwrap();
            f.context.backend.handle_override = Some((MockHandleKind::Allocation, backend_id));
            let replacement = f
                .context
                .allocate(
                    f.context.devices()[0].id(),
                    RuntimeMemoryKindV1::DeviceLocal,
                    64,
                    16,
                )
                .unwrap();
            assert_ne!(old, replacement);
            let token = f.context.reserve_graph_v1(1).unwrap();
            let before = (f.context.next_identity, f.context.backend.submit_count);
            assert!(matches!(
                f.context.submit_graph_action_v1(token, action),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::UnknownAllocation
                ))
            ));
            assert_eq!(
                (f.context.next_identity, f.context.backend.submit_count),
                before
            );
            assert_eq!(
                f.context.version_journal_read_records_v1(),
                journal.then_some(0)
            );
            f.context.close_graph_issue_v1(token).unwrap();
            f.context.release_graph_v1(token).unwrap();
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn partial_reader_headroom_rejects_entire_batch_before_issue() {
    let mut f = Fixture::new(2);
    let [a, b, c] = f.allocations;
    let mut held = f.launch(vec![region(a, RuntimeAccessV1::Read, 0)]);
    let args = MixedArguments(vec![
        region(b, RuntimeAccessV1::Read, 0),
        region(c, RuntimeAccessV1::Read, 0),
    ]);
    let before = (
        f.context.next_identity,
        f.context.backend.submit_count,
        f.context.version_journal_usage_v1(),
    );
    assert!(matches!(
        f.context
            .launch(f.stream, &f.kernel, &args, geometry(), &[]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(
        (
            f.context.next_identity,
            f.context.backend.submit_count,
            f.context.version_journal_usage_v1()
        ),
        before
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(f.context.submissions.len(), 1);
    f.context.write_allocation(b, 0, &[9]).unwrap();
    f.context.write_allocation(c, 0, &[9]).unwrap();
    f.context.wait(&mut held, Duration::ZERO).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn busy_middle_or_last_reader_rejects_batch_without_partial_custody() {
    for index in [1, 2] {
        for unknown in [false, true] {
            let mut f = Fixture::new(4);
            let busy = f.allocations[index];
            let mut writer = f.launch(vec![region(busy, RuntimeAccessV1::Write, 0)]);
            if unknown {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: -1 });
                f.context.wait(&mut writer, Duration::ZERO).unwrap();
            }
            let args = MixedArguments(
                f.allocations
                    .iter()
                    .map(|&id| region(id, RuntimeAccessV1::Read, 0))
                    .collect(),
            );
            let before = (
                f.context.next_identity,
                f.context.backend.submit_count,
                f.context.version_journal_usage_v1(),
                state(&f.context, busy),
            );
            reserved(
                f.context
                    .launch(f.stream, &f.kernel, &args, geometry(), &[]),
            );
            assert_eq!(
                (
                    f.context.next_identity,
                    f.context.backend.submit_count,
                    f.context.version_journal_usage_v1(),
                    state(&f.context, busy)
                ),
                before
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
            for id in f.allocations {
                if id != busy {
                    f.context.write_allocation(id, 0, &[9]).unwrap();
                }
            }
            if !unknown {
                f.context.wait(&mut writer, Duration::ZERO).unwrap();
            }
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn unknown_writer_disposal_rejects_retained_reader_marker_without_root() {
    let mut f = Fixture::new(2);
    let [a, b, _] = f.allocations;
    let mut submission = f.launch(vec![
        region(a, RuntimeAccessV1::Write, 0),
        region(b, RuntimeAccessV1::Read, 0),
    ]);
    let original_marker = f.context.submissions[&submission.id].journal_read;
    f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: -1 });
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    f.context
        .submissions
        .get_mut(&submission.id)
        .unwrap()
        .journal_read = original_marker;
    let before = (
        f.context.backend.cleanup_log.clone(),
        f.context
            .allocation_admission_usage_v1(f.context.devices()[0].id())
            .unwrap()
            .unwrap()
            .used,
        state(&f.context, a),
    );
    assert!(matches!(
        f.context.release_allocation(a),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    assert_eq!(
        (
            f.context.backend.cleanup_log.clone(),
            f.context
                .allocation_admission_usage_v1(f.context.devices()[0].id())
                .unwrap()
                .unwrap()
                .used,
            state(&f.context, a)
        ),
        before
    );
    assert!(f.context.is_terminal());
    assert!(!f.context.cleanup().is_complete());
}

#[test]
fn graph_reads_capture_version_at_issue_after_predecessor_settlement() {
    let mut f = Fixture::new(2);
    let a = f.allocations[0];
    let args = MixedArguments(vec![region(a, RuntimeAccessV1::Read, 8)]);
    let writer = MixedArguments(vec![region(a, RuntimeAccessV1::Write, 0)]);
    let read_action = f
        .context
        .prepare_graph_launch_v1(
            f.stream,
            &f.kernel,
            &args.encode_explicit_kernarg_v1(),
            &args.bindings_v1(),
            geometry(),
        )
        .unwrap();
    let write_action = f
        .context
        .prepare_graph_launch_v1(
            f.stream,
            &f.kernel,
            &writer.encode_explicit_kernarg_v1(),
            &writer.bindings_v1(),
            geometry(),
        )
        .unwrap();
    let before = state(&f.context, a);
    let reservation = f.context.reserve_graph_v1(2).unwrap();
    let mut write = f
        .context
        .submit_graph_action_v1(reservation, write_action)
        .unwrap();
    assert_eq!(
        f.context
            .poll_with_graph_access_v1(&mut write, Some(reservation))
            .unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        f.context
            .poll_with_graph_access_v1(&mut write, Some(reservation))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    f.context
        .release_graph_submission_v1(reservation, &write)
        .unwrap();
    let after = state(&f.context, a);
    assert_eq!(after.attempt_epoch, before.attempt_epoch + 1);
    assert_eq!(after.content_lineage, before.content_lineage + 1);
    let mut read = f
        .context
        .submit_graph_action_v1(reservation, read_action)
        .unwrap();
    let marker = f.context.submissions[&read.id].journal_read.unwrap();
    let lease = f
        .context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .lookup_read(marker.first)
        .unwrap();
    assert_eq!(lease.attempt_epoch, after.attempt_epoch);
    assert_eq!(lease.content_lineage, after.content_lineage);
    assert_eq!(
        f.context
            .poll_with_graph_access_v1(&mut read, Some(reservation))
            .unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(
        f.context
            .poll_with_graph_access_v1(&mut read, Some(reservation))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    f.assert_observed(read.backend_submission, &args);
    f.context
        .release_graph_submission_v1(reservation, &read)
        .unwrap();
    f.context.close_graph_issue_v1(reservation).unwrap();
    f.context.release_graph_v1(reservation).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

impl RuntimeAtomicArgumentsV1 for MixedArguments {
    const OPERATION_V1: RuntimeAtomicOperationV1 = RuntimeAtomicOperationV1::Add;
    const SCOPE_V1: RuntimeMemoryScopeV1 = RuntimeMemoryScopeV1::Workgroup;
    const ORDER_V1: RuntimeMemoryOrderV1 = RuntimeMemoryOrderV1::Relaxed;
}

impl RuntimeCollectiveArgumentsV1 for MixedArguments {
    const OPERATION_V1: RuntimeCollectiveOperationV1 = RuntimeCollectiveOperationV1::ReduceSum;
    const SCOPE_V1: RuntimeMemoryScopeV1 = RuntimeMemoryScopeV1::Workgroup;
    const ORDER_V1: RuntimeMemoryOrderV1 = RuntimeMemoryOrderV1::AcquireRelease;
}

fn finish_forwarded<A>(
    mut f: Fixture,
    mut submission: RuntimeSubmissionV1<A>,
    args: MixedArguments,
) {
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert_eq!(
        f.context.backend.pending_kernel_reads[&submission.backend_submission].bindings,
        f.backend_bindings(&args)
    );
    reserved(f.context.write_allocation(f.allocations[0], 0, &[9]));
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    f.assert_observed(submission.backend_submission, &args);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn snapshot_atomic_and_collective_routes_forward_input_custody() {
    let mut f = Fixture::new(2);
    let args = f.reads();
    let submission = f
        .context
        .launch_snapshot_v1(
            f.stream,
            &f.kernel,
            &args.encode_explicit_kernarg_v1(),
            &args.bindings_v1(),
            geometry(),
            &[],
        )
        .unwrap();
    finish_forwarded(f, submission, args);

    let mut f = Fixture::new(2);
    f.context.backend.execution_capabilities.atomics = true;
    let args = f.reads();
    let contract = RuntimeAtomicLaunchContractV1 {
        operation: RuntimeAtomicOperationV1::Add,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::Relaxed,
        failure_order: None,
        weak: false,
        geometry: geometry(),
    };
    let submission = f
        .context
        .launch_atomic(f.stream, &f.kernel, &args, contract, &[])
        .unwrap();
    assert_eq!(f.context.backend.last_atomic_contract, Some(contract));
    finish_forwarded(f, submission, args);

    let mut f = Fixture::new(2);
    f.context.backend.execution_capabilities.collectives = true;
    let args = f.reads();
    let contract = RuntimeCollectiveLaunchContractV1 {
        operation: RuntimeCollectiveOperationV1::ReduceSum,
        scope: RuntimeMemoryScopeV1::Workgroup,
        order: RuntimeMemoryOrderV1::AcquireRelease,
        participants: 64,
        geometry: geometry(),
    };
    let submission = f
        .context
        .launch_collective(f.stream, &f.kernel, &args, contract, &[])
        .unwrap();
    assert_eq!(f.context.backend.last_collective_contract, Some(contract));
    finish_forwarded(f, submission, args);
}

#[test]
fn malformed_read_only_handles_retain_all_consumers_and_charge_each_source_once() {
    for duplicate in [false, true] {
        let mut f = Fixture::new(4);
        let args = f.reads();
        let handle = if duplicate {
            let first = f.launch(args.0.clone());
            f.context
                .on_completion(&first, |_| panic!("ambiguous handle must not notify"))
                .unwrap();
            first.backend_submission
        } else {
            0
        };
        f.context.backend.handle_override = Some((MockHandleKind::Submission, handle));
        assert!(matches!(
            f.context
                .launch(f.stream, &f.kernel, &args, geometry(), &[]),
            Err(RuntimeErrorV1::BackendProtocol(_))
        ));
        assert!(f.context.is_terminal());
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
        let retained = 2 * (1 + usize::from(duplicate));
        assert_eq!(f.context.version_journal_read_records_v1(), Some(retained));
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        let usage = f
            .context
            .allocation_admission_usage_v1(f.context.devices()[0].id())
            .unwrap()
            .unwrap();
        assert_eq!(usage.quarantined_records, 2);
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::AllocationRecords),
            3
        );
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
            192
        );
        let pending = f.context.backend.pending_kernel_reads.clone();
        assert_eq!(pending[&handle].bindings, f.backend_bindings(&args));
        for _ in 0..2 {
            let report = f.context.cleanup();
            assert!(!report.is_complete());
            assert_eq!(report.reader_journal_records_v1(), retained);
        }
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert_eq!(f.context.backend.pending_kernel_reads, pending);
        assert!(f.context.backend.cleanup_log.is_empty());
    }
}

#[test]
fn ambiguous_observation_keeps_pure_readers_with_or_without_admission_credits() {
    for credits in [false, true] {
        for route in 0..3 {
            let mut f = Fixture::configured(2, true, credits);
            let args = f.reads();
            let mut submission = f.launch(args.0.clone());
            let result = catch_unwind(AssertUnwindSafe(|| match route {
                0 => {
                    f.context.backend.first_wait_failure = MockWaitFailure::TerminalFirst;
                    f.context.wait(&mut submission, Duration::ZERO).map(|_| ())
                }
                _ => {
                    f.context.backend.cancel_failure = if route == 1 {
                        MockMemoryFailure::Terminal
                    } else {
                        MockMemoryFailure::Panic
                    };
                    f.context.cancel(&mut submission).map(|_| ())
                }
            }));
            match (route, result) {
                (0, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                    assert_eq!(error.0, "wait terminal")
                }
                (1, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                    assert_eq!(error.0, "allocation terminal failure")
                }
                (2, Err(payload)) => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"scripted allocation adapter panic")
                ),
                _ => panic!("original observation failure was replaced"),
            }
            assert!(f.context.is_terminal());
            assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
            assert!(f.context.backend.observed_kernel_reads.is_empty());
            let pending = f.context.backend.pending_kernel_reads.clone();
            assert_eq!(
                pending[&submission.backend_submission].bindings,
                f.backend_bindings(&args)
            );
            let usage = f
                .context
                .allocation_admission_usage_v1(f.context.devices()[0].id())
                .unwrap();
            if credits {
                let usage = usage.unwrap();
                assert_eq!(usage.quarantined_records, 2);
                assert_eq!(
                    usage
                        .used
                        .get(crate::RuntimeResourceKindV1::AllocationRecords),
                    3
                );
            } else {
                assert!(usage.is_none());
            }
            assert!(!f.context.cleanup().is_complete());
            assert!(!f.context.cleanup().is_complete());
            assert_eq!(f.context.backend.pending_kernel_reads, pending);
            assert!(f.context.backend.cleanup_log.is_empty());
        }
    }
}

#[test]
fn callback_panic_and_repeated_observation_cannot_release_a_later_read_batch() {
    let mut f = Fixture::new(2);
    let args = f.reads();
    let mut first = f.launch(args.0.clone());
    let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let callback_observed = observed.clone();
    f.context
        .on_completion(&first, move |status| {
            callback_observed.lock().unwrap().push(status);
            panic!("after kernel read completion");
        })
        .unwrap();
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    f.assert_observed(first.backend_submission, &args);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(f.context.completion_callback_panic_count(), 1);
    assert_eq!(
        *observed.lock().unwrap(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    let mut next = f.launch(args.0.clone());
    let calls = f.context.backend.wait_call_count;
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    assert_eq!(f.context.backend.wait_call_count, calls);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert_eq!(f.context.completion_callback_panic_count(), 1);
    assert_eq!(
        *observed.lock().unwrap(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    f.context.wait(&mut next, Duration::ZERO).unwrap();
    f.assert_observed(next.backend_submission, &args);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn dropped_reader_token_keeps_inputs_until_cleanup_quiesces_its_stream() {
    for credits in [false, true] {
        let mut f = Fixture::configured(2, true, credits);
        let args = f.reads();
        let backend_submission = {
            let submission = f.launch(args.0.clone());
            submission.backend_submission
        };
        assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
        assert!(f.context.backend.observed_kernel_reads.is_empty());
        reserved(f.context.release_allocation(f.allocations[0]));
        let bindings = f.backend_bindings(&args);
        let report = f.context.cleanup();
        assert!(report.is_complete());
        assert_eq!(report.reader_journal_records_v1(), 0);
        assert!(f.context.backend.pending_kernel_reads.is_empty());
        let expected: Vec<_> = bindings
            .into_iter()
            .enumerate()
            .map(|(ordinal, binding)| MockObservedKernelRead {
                submission: backend_submission,
                ordinal,
                binding,
                bytes: pattern(ordinal)[8..16].to_vec(),
            })
            .collect();
        assert_eq!(f.context.backend.observed_kernel_reads, expected);
        assert!(f.context.backend.memory.is_empty());
    }
}

#[test]
fn stale_internal_preparation_retains_provisional_root_before_any_backend_submission() {
    let mut f = Fixture::new(2);
    let sources: Vec<_> = f.allocations[..2]
        .iter()
        .map(|&id| ContextReadSourceV1 {
            region: RuntimeMemoryRegionV1 {
                allocation: id,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            record: f.context.allocations[&id],
        })
        .collect();
    let prepared = f.context.prepare_submission_readers_v1(&sources).unwrap();
    // Deliberately split the private preflight/Begin phases; no public race is claimed.
    f.context
        .write_allocation(f.allocations[0], 0, &[9])
        .unwrap();
    let neighbor = state(&f.context, f.allocations[2]);
    let id = RuntimeSubmissionIdV1::new(f.context.context_generation, f.context.next_id().unwrap());
    assert_eq!(
        f.context
            .begin_submission_readers_v1(id, prepared)
            .unwrap_err(),
        RuntimeValidationErrorV1::InvalidBackendDescription
    );
    assert!(f.context.is_terminal());
    assert!(f.context.submissions.is_empty());
    assert_eq!(f.context.backend.submit_count, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        f.context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .retained_read_count(),
        0
    );
    let originals = f
        .context
        .versions
        .as_ref()
        .unwrap()
        .submission_reader_sources_for_test_v1(id);
    assert_eq!(originals.len(), sources.len());
    for (actual, expected) in originals.iter().zip(&sources) {
        assert_eq!(actual.region, expected.region);
        assert_eq!(actual.record, expected.record);
    }
    let usage = f
        .context
        .allocation_admission_usage_v1(f.context.devices()[0].id())
        .unwrap()
        .unwrap();
    assert_eq!(usage.quarantined_records, 2);
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::AllocationRecords),
        3
    );
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
        192
    );
    assert_eq!(state(&f.context, f.allocations[2]), neighbor);
    for _ in 0..2 {
        let report = f.context.cleanup();
        assert!(!report.is_complete());
        assert_eq!(report.reader_journal_records_v1(), 1);
    }
    assert!(f.context.backend.cleanup_log.is_empty());
}
