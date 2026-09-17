use super::async_journal_tests::{region, state, writer_state};
use super::*;
use fe2o3_runtime_model::ContextWriterStateV1;

type Context = RuntimeContextV1<MockBackend>;

struct Fixture {
    context: Context,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    source_stream: RuntimeStreamIdV1,
    stream: RuntimeStreamIdV1,
    kernel: TypedRuntimeKernelV1<AddArguments>,
}

impl Fixture {
    fn new(peer: bool, writers: usize) -> Self {
        Self::with_journal(peer, writers, true)
    }

    fn with_journal(peer: bool, writers: usize, journal: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, 128, writers).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
        for device in devices {
            context
                .configure_allocation_admission_v1(device, 1024, 16)
                .unwrap();
        }
        let source_device = context.devices()[0].id();
        let source_stream = context.create_stream(source_device).unwrap();
        let source = context
            .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let module = context.load_module(source_device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        let device = context.devices()[usize::from(peer)].id();
        let destination = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let stream = if peer {
            context.create_stream(device).unwrap()
        } else {
            source_stream
        };
        context.write_allocation(source, 0, &[0x51; 64]).unwrap();
        context
            .write_allocation(destination, 0, &[0xa3; 64])
            .unwrap();
        Self {
            context,
            source,
            destination,
            source_stream,
            stream,
            kernel,
        }
    }

    fn local(&mut self) -> Result<RuntimeSubmissionV1<RuntimeCopyV1>, RuntimeErrorV1<MockError>> {
        self.context.copy_async(
            self.stream,
            region(self.source, RuntimeAccessV1::Read, 8),
            region(self.destination, RuntimeAccessV1::Write, 24),
            &[],
        )
    }

    fn peer(
        &mut self,
    ) -> Result<RuntimeSubmissionV1<RuntimePeerCopyV1>, RuntimeErrorV1<MockError>> {
        self.context.peer_copy(
            self.stream,
            region(self.source, RuntimeAccessV1::Read, 8),
            region(self.destination, RuntimeAccessV1::Write, 24),
            &[],
        )
    }

    fn bytes(&self, allocation: RuntimeAllocationIdV1) -> &[u8] {
        &self.context.backend.memory[&self.context.allocations[&allocation].backend_allocation]
    }

    fn assert_copied(&self) {
        let mut expected = [0xa3; 64];
        expected[24..32].fill(0x51);
        assert_eq!(self.bytes(self.destination), expected);
        assert_eq!(self.bytes(self.source), [0x51; 64]);
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

fn pending_conflicts<A>(mut f: Fixture, mut submission: RuntimeSubmissionV1<A>) {
    let source_before = state(&f.context, f.source);
    let before = (
        f.context.next_identity,
        f.context.backend.next,
        f.context.backend.copy_call_count,
        f.context.backend.submit_count,
        f.context.backend.write_call_count,
        f.context.backend.cleanup_log.clone(),
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(f.bytes(f.destination), [0xa3; 64]);
    assert_eq!(
        f.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    reserved(f.context.write_allocation(f.source, 0, &[7; 8]));
    reserved(f.context.release_allocation(f.source));
    reserved(f.context.launch(
        f.source_stream,
        &f.kernel,
        &AddArguments {
            allocation: f.source,
            scalar: 1,
        },
        geometry(),
        &[],
    ));
    // An unrelated source on the same device attempts to overwrite the pinned source.
    let destination = f.context.allocations[&f.destination];
    if destination.device == f.context.allocations[&f.source].device {
        // This source already has a Pending writer, so use another initialized buffer.
        let other = f
            .context
            .allocate(destination.device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let next = f.context.next_identity;
        reserved(f.context.copy_async(
            f.source_stream,
            region(other, RuntimeAccessV1::Read, 0),
            region(f.source, RuntimeAccessV1::Write, 0),
            &[],
        ));
        assert_eq!(f.context.next_identity, next);
        f.context.release_allocation(other).unwrap();
    } else {
        assert_eq!(
            (
                f.context.next_identity,
                f.context.backend.next,
                f.context.backend.copy_call_count,
                f.context.backend.submit_count,
                f.context.backend.write_call_count,
                f.context.backend.cleanup_log.clone()
            ),
            before
        );
    }
    assert_eq!(f.context.backend.copy_call_count, before.2);
    assert_eq!(f.context.backend.submit_count, before.3);
    assert_eq!(f.context.backend.write_call_count, before.4);
    assert_eq!(state(&f.context, f.source), source_before);
    assert_eq!(f.bytes(f.source), [0x51; 64]);
    assert_eq!(f.bytes(f.destination), [0xa3; 64]);
    assert!(!f.context.is_terminal());
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    f.assert_copied();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(state(&f.context, f.source), source_before);
    f.context.write_allocation(f.source, 0, &[7; 8]).unwrap();
    f.context.release_allocation(f.source).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn pending_local_and_peer_copies_exclude_source_mutation_before_backend_entry() {
    let mut f = Fixture::new(false, 4);
    let submission = f.local().unwrap();
    pending_conflicts(f, submission);
    let mut f = Fixture::new(true, 4);
    let submission = f.peer().unwrap();
    pending_conflicts(f, submission);
}

#[test]
fn shared_source_stays_pinned_until_last_out_of_order_completion() {
    let mut f = Fixture::new(false, 4);
    let mut first = f.local().unwrap();
    let device = f.context.devices()[0].id();
    let destination = f
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let mut second = f
        .context
        .copy_async(
            f.stream,
            region(f.source, RuntimeAccessV1::Read, 8),
            region(destination, RuntimeAccessV1::Write, 24),
            &[],
        )
        .unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    f.context.wait(&mut second, Duration::ZERO).unwrap();
    f.context.release_submission(second).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    reserved(f.context.write_allocation(f.source, 0, &[7]));
    reserved(f.context.release_allocation(f.source));
    assert_eq!(f.bytes(f.destination), [0xa3; 64]);
    assert_eq!(&f.bytes(destination)[24..32], &[0x51; 8]);
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    f.assert_copied();
    f.context.write_allocation(f.source, 0, &[7]).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn every_terminal_observation_releases_the_exact_copy_source() {
    for route in 0..8 {
        let mut f = Fixture::new(false, 2);
        let mut submission = f.local().unwrap();
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
            6 => {
                f.context.backend.cleanup_failure = MockCleanupFailure::QuiescentStreamOnce;
                assert!(matches!(
                    f.context.destroy_stream(f.stream),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => {
                // This observer token intentionally owns no cleanup authority.
                #[allow(clippy::drop_non_drop)]
                drop(submission);
                reserved(f.context.release_allocation(f.source));
                f.context
                    .synchronize_stream(f.stream, Duration::ZERO)
                    .unwrap();
            }
        }
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(0),
            "route {route}"
        );
        assert_eq!(
            state(&f.context, f.destination).content_lineage,
            if (5..=6).contains(&route) { 1 } else { 2 }
        );
        f.assert_copied();
        f.context.write_allocation(f.source, 0, &[7]).unwrap();
        assert!(f.context.cleanup().is_complete(), "route {route}");
    }
}

#[test]
fn cancellation_no_effect_is_backed_by_unpublished_copy_bytes() {
    let mut f = Fixture::new(false, 2);
    let mut submission = f.local().unwrap();
    f.context.backend.cancel_failure = MockMemoryFailure::Rejected;
    assert!(matches!(
        f.context.cancel(&mut submission),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(
        f.context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::TooLate
    );
    f.context.backend.wait_observation = Some(BackendPollV1::Pending);
    assert_eq!(
        f.context.wait(&mut submission, Duration::ZERO).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    reserved(f.context.write_allocation(f.source, 0, &[7]));
    f.context.backend.wait_observation = None;
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    f.assert_copied();
    assert!(f.context.cleanup().is_complete());

    let mut f = Fixture::new(false, 2);
    let mut submission = f.local().unwrap();
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut submission).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(f.bytes(f.destination), [0xa3; 64]);
    assert_eq!(state(&f.context, f.destination).content_lineage, 1);
    assert_eq!(state(&f.context, f.destination).attempt_epoch, 2);
    f.context.write_allocation(f.source, 0, &[7]).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn failed_or_quiescent_copy_releases_source_but_preserves_unknown_destination() {
    for route in 0..3 {
        let mut f = Fixture::new(false, 2);
        let mut submission = f.local().unwrap();
        match route {
            0 => {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: -2 });
                assert_eq!(
                    f.context.wait(&mut submission, Duration::ZERO).unwrap(),
                    RuntimePollV1::Failed { code: -2 }
                );
            }
            1 => {
                f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    f.context.wait(&mut submission, Duration::ZERO),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => {
                f.context.backend.cancel_failure = MockMemoryFailure::Quiescent;
                assert!(matches!(
                    f.context.cancel(&mut submission),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(
            writer_state(&f.context, f.destination),
            ContextWriterStateV1::Unknown { member_count: 1 }
        );
        assert_eq!(state(&f.context, f.destination).content_lineage, 1);
        assert_eq!(f.bytes(f.destination), [0xa3; 64]);
        assert!(
            !f.context
                .backend
                .pending_copies
                .contains_key(&submission.backend_submission)
        );
        f.context.release_submission(submission).unwrap();
        f.context.write_allocation(f.source, 0, &[7]).unwrap();
        f.context.release_allocation(f.destination).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn initial_copy_failure_retains_only_ambiguous_source_custody() {
    for peer in [false, true] {
        for failure in [
            MockMemoryFailure::Rejected,
            MockMemoryFailure::Quiescent,
            MockMemoryFailure::Terminal,
            MockMemoryFailure::Panic,
        ] {
            let mut f = Fixture::new(peer, 2);
            f.context.backend.copy_failure = failure;
            let result = catch_unwind(AssertUnwindSafe(|| {
                if peer {
                    f.peer().map(|_| ())
                } else {
                    f.local().map(|_| ())
                }
            }));
            match (failure, result) {
                (MockMemoryFailure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(error)))) => {
                    assert_eq!(error.0, "allocation rejected")
                }
                (
                    MockMemoryFailure::Quiescent,
                    Ok(Err(RuntimeErrorV1::BackendQuiescent(error))),
                ) => assert_eq!(error.0, "allocation quiescent failure"),
                (MockMemoryFailure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(error)))) => {
                    assert_eq!(error.0, "allocation terminal failure")
                }
                (MockMemoryFailure::Panic, Err(payload)) => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"scripted allocation adapter panic")
                ),
                _ => panic!("original copy failure was replaced"),
            }
            let retained = matches!(
                failure,
                MockMemoryFailure::Terminal | MockMemoryFailure::Panic
            );
            assert_eq!(
                f.context.version_journal_read_records_v1(),
                Some(usize::from(retained))
            );
            assert_eq!(f.context.is_terminal(), retained);
            assert!(f.context.submissions.is_empty());
            assert_eq!(f.context.backend.copy_call_count, 1);
            assert_eq!(state(&f.context, f.destination).content_lineage, 1);
            if retained {
                let pending = f.context.backend.pending_copies.clone();
                assert_eq!(pending.len(), 1);
                let &(stream, source, destination) = pending.values().next().unwrap();
                assert_eq!(stream, f.context.streams[&f.stream].backend_stream);
                assert_eq!(
                    source.allocation,
                    f.context.allocations[&f.source].backend_allocation
                );
                assert_eq!((source.byte_offset, source.byte_len), (8, 8));
                assert_eq!(
                    destination.allocation,
                    f.context.allocations[&f.destination].backend_allocation
                );
                assert_eq!((destination.byte_offset, destination.byte_len), (24, 8));
                assert_eq!(f.bytes(f.destination), [0xa3; 64]);
                assert!(!f.context.cleanup().is_complete());
                assert!(!f.context.cleanup().is_complete());
                assert_eq!(f.context.backend.pending_copies, pending);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            } else {
                assert!(f.context.backend.pending_copies.is_empty());
                if failure == MockMemoryFailure::Quiescent {
                    f.assert_copied();
                } else {
                    assert_eq!(f.bytes(f.destination), [0xa3; 64]);
                    assert!(f.context.backend.polls.is_empty());
                }
                f.context.write_allocation(f.source, 0, &[7]).unwrap();
                assert!(f.context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn pending_unknown_and_capacity_rejections_do_not_consume_identity_or_leases() {
    for unknown in [false, true] {
        let mut f = Fixture::new(false, 3);
        let mut writer = f
            .context
            .launch(
                f.source_stream,
                &f.kernel,
                &AddArguments {
                    allocation: f.source,
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap();
        if unknown {
            f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: -1 });
            f.context.wait(&mut writer, Duration::ZERO).unwrap();
        }
        let before = (
            f.context.next_identity,
            state(&f.context, f.source),
            state(&f.context, f.destination),
        );
        reserved(f.local());
        assert_eq!(
            (
                f.context.next_identity,
                state(&f.context, f.source),
                state(&f.context, f.destination)
            ),
            before
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.backend.copy_call_count, 0);
        assert!(!f.context.is_terminal());
        assert!(f.context.cleanup().is_complete());
    }
    let mut f = Fixture::new(false, 1);
    let mut first = f.local().unwrap();
    let before = (f.context.next_identity, f.context.backend.copy_call_count);
    assert!(matches!(
        f.local(),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(
        (f.context.next_identity, f.context.backend.copy_call_count),
        before
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    f.context.wait(&mut first, Duration::ZERO).unwrap();
    let mut next = f.local().unwrap();
    f.context.wait(&mut next, Duration::ZERO).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn prepared_copy_rejects_released_original_source_even_with_recycled_backend_handle() {
    for journal in [false, true] {
        let mut f = Fixture::with_journal(false, 2, journal);
        let action = f
            .context
            .prepare_graph_copy_v1(
                f.stream,
                region(f.source, RuntimeAccessV1::Read, 8),
                region(f.destination, RuntimeAccessV1::Write, 24),
            )
            .unwrap();
        let backend = f.context.allocations[&f.source].backend_allocation;
        f.context.release_allocation(f.source).unwrap();
        f.context.backend.handle_override = Some((MockHandleKind::Allocation, backend));
        let device = f.context.devices()[0].id();
        let replacement = f
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        assert_ne!(replacement, f.source);
        let reservation = f.context.reserve_graph_v1(1).unwrap();
        let next = f.context.next_identity;
        assert!(matches!(
            f.context.submit_graph_action_v1(reservation, action),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownAllocation
            ))
        ));
        assert_eq!(f.context.next_identity, next);
        assert_eq!(f.context.backend.copy_call_count, 0);
        f.context.close_graph_issue_v1(reservation).unwrap();
        f.context.release_graph_v1(reservation).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn missing_or_substituted_copy_marker_cannot_publish_completion() {
    for fault in 0..3 {
        let mut f = Fixture::new(false, 2);
        let mut submission = f.local().unwrap();
        f.context
            .on_completion(&submission, |_| panic!("must not run"))
            .unwrap();
        match fault {
            0 => {
                f.context
                    .submissions
                    .get_mut(&submission.id)
                    .unwrap()
                    .journal_read = None
            }
            1 => {
                f.context
                    .submissions
                    .get_mut(&submission.id)
                    .unwrap()
                    .journal_read
                    .as_mut()
                    .unwrap()
                    .incarnation += 1
            }
            _ => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .remove_submission_writer_root_for_test_v1(submission.id),
        }
        assert!(matches!(
            f.context.wait(&mut submission, Duration::ZERO),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidBackendDescription
            ))
        ));
        assert!(f.context.is_terminal());
        assert_eq!(
            f.context.query_submission(&submission).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(f.context.completion_callback_panic_count(), 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        assert_eq!(state(&f.context, f.destination).content_lineage, 1);
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn callback_panic_and_repeat_observation_cannot_repeat_reader_release() {
    let mut f = Fixture::new(false, 2);
    let mut submission = f.local().unwrap();
    f.context
        .on_completion(&submission, |_| panic!("after copy settlement"))
        .unwrap();
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(f.context.completion_callback_panic_count(), 1);
    let mut other = f.local().unwrap();
    f.context.wait(&mut submission, Duration::ZERO).unwrap();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(f.context.completion_callback_panic_count(), 1);
    f.context.wait(&mut other, Duration::ZERO).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn malformed_handles_quarantine_all_readers_including_shared_sources() {
    for duplicate in [false, true] {
        let mut f = Fixture::new(false, 2);
        let handle = if duplicate {
            let first = f.local().unwrap();
            let device = f.context.devices()[0].id();
            f.destination = f
                .context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            first.backend_submission
        } else {
            0
        };
        f.context.backend.handle_override = Some((MockHandleKind::Submission, handle));
        assert!(matches!(f.local(), Err(RuntimeErrorV1::BackendProtocol(_))));
        assert!(f.context.is_terminal());
        assert_eq!(
            f.context.version_journal_read_records_v1(),
            Some(1 + usize::from(duplicate))
        );
        let calls = f.context.backend.copy_call_count;
        assert!(!f.context.cleanup().is_complete());
        assert!(!f.context.cleanup().is_complete());
        assert_eq!(f.context.backend.copy_call_count, calls);
        assert!(f.context.backend.cleanup_log.is_empty());
        let device = f.context.devices()[0].id();
        let usage = f
            .context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap();
        assert_eq!(usage.quarantined_records, 2 + usize::from(duplicate));
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::AllocationRecords),
            2 + u64::from(duplicate)
        );
        assert_eq!(
            usage
                .used
                .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
            64 * (2 + u64::from(duplicate))
        );
    }
}

#[test]
fn stream_quiescence_does_not_release_another_streams_shared_source_lease() {
    let mut f = Fixture::new(false, 3);
    let first = f.local().unwrap();
    let device = f.context.devices()[0].id();
    let stream = f.context.create_stream(device).unwrap();
    let destination = f
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let mut second = f
        .context
        .copy_async(
            stream,
            region(f.source, RuntimeAccessV1::Read, 8),
            region(destination, RuntimeAccessV1::Write, 24),
            &[],
        )
        .unwrap();
    let usage = f.context.allocation_admission_usage_v1(device).unwrap();
    f.context.backend.cleanup_failure = MockCleanupFailure::RejectStreamOnce;
    assert!(matches!(
        f.context.destroy_stream(f.stream),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    assert_eq!(f.bytes(f.destination), [0xa3; 64]);
    f.context.destroy_stream(f.stream).unwrap();
    f.assert_copied();
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(f.bytes(destination), [0; 64]);
    assert!(
        f.context
            .backend
            .pending_copies
            .contains_key(&second.backend_submission)
    );
    f.context.release_submission(first).unwrap();
    reserved(f.context.release_allocation(f.source));
    assert_eq!(
        f.context.allocation_admission_usage_v1(device).unwrap(),
        usage
    );
    f.context.wait(&mut second, Duration::ZERO).unwrap();
    assert_eq!(&f.bytes(destination)[24..32], &[0x51; 8]);
    f.context.release_allocation(f.source).unwrap();
    let usage = f
        .context
        .allocation_admission_usage_v1(device)
        .unwrap()
        .unwrap();
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::AllocationRecords),
        2
    );
    assert_eq!(usage.quarantined_records, 0);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn rejected_wait_retains_exact_reader_and_terminal_wait_quarantines_both_owners() {
    for terminal in [false, true] {
        let mut f = Fixture::new(false, 2);
        let mut submission = f.local().unwrap();
        let device = f.context.devices()[0].id();
        let used = f
            .context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap()
            .used;
        f.context.backend.first_wait_failure = if terminal {
            MockWaitFailure::TerminalFirst
        } else {
            MockWaitFailure::RejectFirst
        };
        let result = f.context.wait(&mut submission, Duration::ZERO);
        assert!(if terminal {
            matches!(result, Err(RuntimeErrorV1::BackendTerminal(_)))
        } else {
            matches!(result, Err(RuntimeErrorV1::BackendRejected(_)))
        });
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        assert_eq!(f.bytes(f.destination), [0xa3; 64]);
        assert_eq!(f.context.backend.pending_copies.len(), 1);
        assert_eq!(f.context.is_terminal(), terminal);
        let usage = f
            .context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap();
        assert_eq!(usage.used, used);
        assert_eq!(usage.quarantined_records, if terminal { 2 } else { 0 });
        if terminal {
            assert!(!f.context.cleanup().is_complete());
        } else {
            reserved(f.context.release_allocation(f.source));
            f.context.wait(&mut submission, Duration::ZERO).unwrap();
            f.assert_copied();
            assert!(f.context.cleanup().is_complete());
        }
    }
}
