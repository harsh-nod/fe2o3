use super::async_journal_tests::{MixedArguments, state, writer_state};
use super::peer_directed_tests::Observation;
use super::*;
use fe2o3_runtime_model::ContextWriterStateV1;
use std::sync::{Arc, Mutex};

#[path = "compute_peer_tests/deferred_chain.rs"]
mod deferred_chain;
#[path = "compute_peer_tests/gather.rs"]
mod gather;
#[path = "compute_peer_tests/gather_readback.rs"]
mod gather_readback;
#[path = "compute_peer_tests/repeated.rs"]
mod repeated;
#[path = "compute_peer_tests/windows.rs"]
mod windows;

type Context = RuntimeContextV1<MockBackend>;
type Peer = RuntimeSubmissionV1<RuntimePeerCopyV1>;

fn span(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: u64,
    bytes: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset,
        byte_len: bytes,
    }
}

struct Fixture {
    context: Context,
    compute_stream: RuntimeStreamIdV1,
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    kernel: TypedRuntimeKernelV1<MixedArguments>,
    producer: RuntimeSubmissionV1<MixedArguments>,
    event: RuntimeEventIdV1,
}

impl Fixture {
    fn new() -> Self {
        Self::with_bindings(|source| vec![span(source, RuntimeAccessV1::Write, 0, 64)])
    }

    fn with_bindings(
        bindings: impl FnOnce(RuntimeAllocationIdV1) -> Vec<RuntimeMemoryRegionV1>,
    ) -> Self {
        Self::with_sizes([64; 3], bindings)
    }

    fn with_sizes(
        sizes: [u64; 3],
        bindings: impl FnOnce(RuntimeAllocationIdV1) -> Vec<RuntimeMemoryRegionV1>,
    ) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            deferred_kernel_reads: true,
            pending_compute_peer: true,
            pending_peer_readback: true,
            ..MockBackend::default()
        };
        let mut context =
            Context::open_with_version_journal_members_v1(backend, 32, 16, 64).unwrap();
        let first = context.devices()[0].id();
        let second = context.devices()[1].id();
        let compute_stream = context.create_stream(first).unwrap();
        let peer_stream = context.create_stream(second).unwrap();
        let readback_stream = context.create_stream(second).unwrap();
        let source = context
            .allocate(first, RuntimeMemoryKindV1::DeviceLocal, sizes[0], 16)
            .unwrap();
        let destination = context
            .allocate(second, RuntimeMemoryKindV1::DeviceLocal, sizes[1], 16)
            .unwrap();
        let host = context
            .allocate(second, RuntimeMemoryKindV1::HostVisible, sizes[2], 16)
            .unwrap();
        for ((allocation, byte), size) in [(source, 3), (destination, 5), (host, 7)]
            .into_iter()
            .zip(sizes)
        {
            context
                .write_allocation(allocation, 0, &vec![byte; size as usize])
                .unwrap();
        }
        let module = context.load_module(first, b"compute-peer-tests").unwrap();
        let kernel = context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
        let producer = context
            .launch_producer_aware_v1(
                compute_stream,
                &kernel,
                &MixedArguments(bindings(source)),
                geometry(),
                &[],
            )
            .unwrap();
        let event = context.record_event(&producer).unwrap();
        Self {
            context,
            compute_stream,
            peer_stream,
            readback_stream,
            source,
            destination,
            host,
            kernel,
            producer,
            event,
        }
    }

    fn peer(&mut self) -> Result<Peer, RuntimeErrorV1<MockError>> {
        self.context.peer_copy(
            self.peer_stream,
            span(self.source, RuntimeAccessV1::Read, 0, 64),
            span(self.destination, RuntimeAccessV1::Write, 0, 64),
            &[self.event],
        )
    }

    fn finish_backend(&mut self, id: u64) {
        // Execute the existing mock operation, never assign Context success.
        self.context.backend.finish_submission(id, true);
        self.context.backend.polls.insert(id, 1);
    }

    fn finish_peer(&mut self, peer: &mut Peer) {
        self.finish_backend(self.producer.backend_submission);
        self.finish_backend(peer.backend_submission);
        for _ in 0..4 {
            if self.context.poll(peer).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("compute-backed peer did not reconcile");
    }

    fn cancel_producer(&mut self) {
        self.context.backend.cancel_before_publication = true;
        self.context.cancel(&mut self.producer).unwrap();
        assert!(self.context.cleanup().is_complete());
    }
}

#[track_caller]
fn validation<T>(result: Result<T, RuntimeErrorV1<MockError>>, expected: RuntimeValidationErrorV1) {
    match result {
        Err(RuntimeErrorV1::Validation(actual)) => assert_eq!(actual, expected),
        Err(actual) => panic!("expected validation rejection {expected:?}, got {actual:?}"),
        Ok(_) => panic!("expected validation rejection {expected:?}, got success"),
    }
}

#[test]
fn compute_peer_exact_writer_lease_and_actual_parent_observation_precede_commit() {
    for backend_already_done in [false, true] {
        let mut f = Fixture::new();
        let before = state(&f.context, f.source);
        if backend_already_done {
            f.finish_backend(f.producer.backend_submission);
        }
        let mut peer = f.peer().unwrap();
        let record = f.context.submissions[&peer.id];
        assert!(record.scalar_peer_copy && !record.directed_peer_copy && !record.producer_launch);
        let root = &f.context.scalar_peer_copies[&peer.id];
        assert!(root.directed.is_none());
        assert_eq!(root.compute.as_ref().unwrap().state.depth, 2);
        assert_eq!(
            root.compute.as_ref().unwrap().producer.submission,
            f.producer.id
        );
        let marker = record.journal_producer_read.unwrap();
        let lease = f
            .context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .lookup_producer_read(marker.active_first_for_test().unwrap())
            .unwrap();
        assert_eq!(
            Some(lease.producer),
            f.context.submissions[&f.producer.id].journal_writer
        );
        assert_eq!(lease.read.attempt_epoch, before.attempt_epoch);
        assert_eq!(lease.read.content_lineage, before.content_lineage);
        assert_eq!((lease.read.byte_offset, lease.read.byte_len), (0, 64));
        f.context.release_event(f.event).unwrap();
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
        let failure = f.context.release_submission(f.producer).unwrap_err();
        assert!(matches!(
            failure.error(),
            RuntimeErrorV1::Validation(RuntimeValidationErrorV1::SubmissionPending)
        ));
        (f.producer, _) = failure.into_parts();
        assert!(f.context.release_allocation(f.source).is_err());
        let output_before = state(&f.context, f.destination);
        let calls = Arc::new(Mutex::new(Vec::new()));
        let parent_calls = Arc::clone(&calls);
        let parent_id = f.producer.id;
        f.context
            .on_completion(&f.producer, move |status| {
                parent_calls.lock().unwrap().push((parent_id, status))
            })
            .unwrap();
        let peer_calls = Arc::clone(&calls);
        let peer_id = peer.id;
        f.context
            .on_completion(&peer, move |status| {
                peer_calls.lock().unwrap().push((peer_id, status))
            })
            .unwrap();
        f.finish_backend(f.producer.backend_submission);
        f.finish_backend(peer.backend_submission);
        assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Pending);
        assert_eq!(
            f.context.query_submission(&f.producer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(state(&f.context, f.source), before);
        assert_eq!(state(&f.context, f.destination), output_before);
        assert!(calls.lock().unwrap().is_empty());
        f.finish_peer(&mut peer);
        assert_eq!(
            *calls.lock().unwrap(),
            [
                (parent_id, RuntimeCompletionStatusV1::Succeeded),
                (peer_id, RuntimeCompletionStatusV1::Succeeded)
            ]
        );
        assert_eq!(
            state(&f.context, f.destination).content_lineage,
            output_before.content_lineage + 1
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        let mut bytes = [0; 64];
        f.context
            .read_allocation(f.destination, 0, &mut bytes)
            .unwrap();
        assert_eq!(
            bytes,
            [(f.producer.backend_submission as u8).wrapping_add(37); 64]
        );
        f.context.release_submission(f.producer).unwrap();
        f.context.release_submission(peer).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_nested_readback_reconciles_all_three_original_roots() {
    let mut f = Fixture::new();
    let peer = f.peer().unwrap();
    let event = f.context.record_event(&peer).unwrap();
    let mut copy = f
        .context
        .copy_async(
            f.readback_stream,
            span(f.destination, RuntimeAccessV1::Read, 0, 64),
            span(f.host, RuntimeAccessV1::Write, 0, 64),
            &[event],
        )
        .unwrap();
    assert_eq!(f.context.same_device_copies[&copy.id].state.depth, 3);
    f.context.release_event(f.event).unwrap();
    f.context.release_event(event).unwrap();
    for id in [
        f.producer.backend_submission,
        peer.backend_submission,
        copy.backend_submission,
    ] {
        f.finish_backend(id);
    }
    assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
    assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
    assert_eq!(
        f.context.query_submission(&f.producer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    for _ in 0..4 {
        if f.context.poll(&mut copy).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
    }
    for id in [f.producer.id, peer.id, copy.id] {
        assert_eq!(
            f.context.submissions[&id].status,
            RuntimeCompletionStatusV1::Succeeded
        );
        assert!(f.context.submissions[&id].quiescent);
        assert_eq!(f.context.submissions[&id].dependency_retains, 0);
    }
    let mut bytes = [0; 64];
    f.context.read_allocation(f.host, 0, &mut bytes).unwrap();
    assert_eq!(
        bytes,
        [(f.producer.backend_submission as u8).wrapping_add(37); 64]
    );
    assert_eq!(f.context.completion_parent_depth_v1(copy.id).unwrap(), 3);
    let completed_control = f.context.record_event(&copy).unwrap();
    let mut next = f
        .context
        .launch_producer_aware_v1(
            f.compute_stream,
            &f.kernel,
            &MixedArguments(vec![span(f.source, RuntimeAccessV1::Write, 0, 64)]),
            geometry(),
            &[],
        )
        .unwrap();
    let next_event = f.context.record_event(&next).unwrap();
    let mut controlled = f
        .context
        .peer_copy(
            f.peer_stream,
            span(f.source, RuntimeAccessV1::Read, 0, 64),
            span(f.destination, RuntimeAccessV1::Write, 0, 64),
            &[next_event, completed_control],
        )
        .unwrap();
    assert_eq!(
        f.context.scalar_peer_copies[&controlled.id]
            .compute
            .as_ref()
            .unwrap()
            .state
            .depth,
        4
    );
    f.context.backend.cancel_before_publication = true;
    f.context.cancel(&mut controlled).unwrap();
    f.context.cancel(&mut next).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn compute_peer_opt_in_full_write_coverage_and_exact_events_are_required() {
    let mut f = Fixture::new();
    let before = state(&f.context, f.destination);
    let calls = f.context.backend.copy_call_count;
    f.context.backend.pending_compute_peer = false;
    validation(f.peer(), RuntimeValidationErrorV1::ContextReserved);
    f.context.backend.pending_compute_peer = true;
    let alias = f.context.record_event(&f.producer).unwrap();
    for (offset, bytes, dependencies, expected) in [
        (0, 64, vec![], RuntimeValidationErrorV1::ContextReserved),
        (
            0,
            64,
            vec![f.event, alias],
            RuntimeValidationErrorV1::DuplicateDependency,
        ),
    ] {
        validation(
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, RuntimeAccessV1::Read, offset, bytes),
                span(f.destination, RuntimeAccessV1::Write, 0, bytes),
                &dependencies,
            ),
            expected,
        );
    }
    validation(
        f.context.peer_copy(
            f.compute_stream,
            span(f.source, RuntimeAccessV1::Read, 0, 64),
            span(f.destination, RuntimeAccessV1::Write, 0, 64),
            &[f.event],
        ),
        RuntimeValidationErrorV1::WrongDevice,
    );
    assert_eq!(f.context.backend.copy_call_count, calls);
    assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
    assert_eq!(state(&f.context, f.destination), before);
    assert!(f.context.scalar_peer_copies.is_empty());
    assert!(!f.context.is_terminal());
    let submissions = f.context.backend.submit_count;
    validation(
        f.context.launch(
            f.compute_stream,
            &f.kernel,
            &MixedArguments(vec![span(f.source, RuntimeAccessV1::Read, 0, 64)]),
            geometry(),
            &[],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.context.backend.submit_count, submissions);
    f.cancel_producer();
    for mode in 0..3 {
        let mut f = Fixture::with_bindings(|source| match mode {
            0 => vec![span(source, RuntimeAccessV1::Write, 0, 32)],
            1 => vec![span(source, RuntimeAccessV1::ReadWrite, 0, 64)],
            _ => vec![
                span(source, RuntimeAccessV1::Write, 0, 64),
                span(source, RuntimeAccessV1::Read, 0, 64),
            ],
        });
        validation(f.peer(), RuntimeValidationErrorV1::ContextReserved);
        assert_eq!(f.context.backend.copy_call_count, 0);
        f.cancel_producer();
    }
}

#[test]
fn compute_peer_cancellation_and_preissue_errors_refund_only_consumer_custody() {
    let mut f = Fixture::new();
    let before = state(&f.context, f.source);
    let mut peer = f.peer().unwrap();
    f.context.release_event(f.event).unwrap();
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(state(&f.context, f.source), before);
    assert_eq!(
        f.context.query_submission(&f.producer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    f.context.release_submission(peer).unwrap();
    f.cancel_producer();
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new();
        if failure == MockMemoryFailure::Quiescent {
            f.finish_backend(f.producer.backend_submission);
        }
        f.context.backend.copy_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.peer()));
        match (failure, result) {
            (MockMemoryFailure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
            | (MockMemoryFailure::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                assert!(f.context.scalar_peer_copies.is_empty());
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
                assert!(!f.context.is_terminal());
                f.context.wait(&mut f.producer, Duration::ZERO).unwrap();
                assert!(f.context.cleanup().is_complete());
            }
            (MockMemoryFailure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (MockMemoryFailure::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                assert_eq!(f.context.scalar_peer_copies.len(), 1);
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
                assert!(!f.context.cleanup().is_complete());
            }
            _ => panic!("compute-peer admission fault lost classification"),
        }
    }
}

#[test]
fn compute_peer_failed_or_cancelled_parent_does_not_invalidate_retained_identity() {
    for cancelled in [false, true] {
        let mut f = Fixture::new();
        let mut peer = f.peer().unwrap();
        f.context.release_event(f.event).unwrap();
        if cancelled {
            f.context.backend.cancel_before_publication = true;
            f.context.cancel(&mut f.producer).unwrap();
        } else {
            f.context
                .backend
                .producer_launch
                .observations
                .insert(f.producer.backend_submission, Observation::Failed);
            f.context.poll(&mut f.producer).unwrap();
        }
        assert!(f.context.validate_scalar_peer_custody_v1(peer.id).is_ok());
        f.context
            .backend
            .producer_launch
            .observations
            .insert(peer.backend_submission, Observation::Failed);
        assert_eq!(
            f.context.poll(&mut peer).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(!f.context.is_terminal());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_descended_compute_fault_preserves_unknown_or_terminal_custody() {
    for fault in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new();
        let mut peer = f.peer().unwrap();
        f.context.release_event(f.event).unwrap();
        f.finish_backend(f.producer.backend_submission);
        f.finish_backend(peer.backend_submission);
        assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Pending);
        let before = state(&f.context, f.destination);
        f.context
            .backend
            .producer_launch
            .observations
            .insert(f.producer.backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut peer)));
        match (fault, result) {
            (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                assert_eq!(
                    f.context.query_submission(&peer).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert_eq!(
                    f.context.query_submission(&f.producer).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert_eq!(
                    writer_state(&f.context, f.destination),
                    ContextWriterStateV1::Unknown { member_count: 1 }
                );
                assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
                assert!(!f.context.is_terminal());
            }
            (Observation::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
            | (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (Observation::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
                assert!(!f.context.cleanup().is_complete());
            }
            _ => panic!("descended compute fault lost classification"),
        }
        assert_eq!(
            state(&f.context, f.destination).content_lineage,
            before.content_lineage
        );
        if !f.context.is_terminal() {
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn compute_peer_nested_pending_root_drift_is_rejected_before_backend_observation() {
    for corruption in 0..10 {
        let mut f = Fixture::new();
        let peer = f.peer().unwrap();
        let event = f.context.record_event(&peer).unwrap();
        let mut copy = f
            .context
            .copy_async(
                f.readback_stream,
                span(f.destination, RuntimeAccessV1::Read, 0, 64),
                span(f.host, RuntimeAccessV1::Write, 0, 64),
                &[event],
            )
            .unwrap();
        for id in [
            f.producer.backend_submission,
            peer.backend_submission,
            copy.backend_submission,
        ] {
            f.finish_backend(id);
        }
        assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
        match corruption {
            0 => {
                f.context
                    .submissions
                    .get_mut(&peer.id)
                    .unwrap()
                    .journal_producer_read = None
            }
            1 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_producer_read_reference_for_test_v1(peer.id, 0),
            2 => {
                f.context
                    .submissions
                    .get_mut(&peer.id)
                    .unwrap()
                    .journal_writer = None
            }
            3 => {
                f.context
                    .scalar_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .compute
                    .as_mut()
                    .unwrap()
                    .producer
                    .backend_submission += 1
            }
            4 => {
                f.context
                    .scalar_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .compute
                    .as_mut()
                    .unwrap()
                    .state
                    .depth += 1
            }
            5..=9 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_active_input_request_for_test_v1(peer.id, corruption - 5),
            _ => unreachable!(),
        }
        let calls = f.context.backend.poll_call_count;
        assert!(f.context.poll(&mut copy).is_err());
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert!(f.context.is_terminal());
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_missing_snapshot_rejects_direct_and_depth_masked_observation() {
    for nested in [false, true] {
        let mut f = Fixture::new();
        let control = if nested {
            let first = f.context.devices()[0].id();
            let second = f.context.devices()[1].id();
            let source = f
                .context
                .allocate(first, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            let destination = f
                .context
                .allocate(second, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            let host = f
                .context
                .allocate(second, RuntimeMemoryKindV1::HostVisible, 64, 16)
                .unwrap();
            for allocation in [source, destination, host] {
                f.context
                    .write_allocation(allocation, 0, &[19; 64])
                    .unwrap();
            }
            let predecessor = f
                .context
                .peer_copy(
                    f.peer_stream,
                    span(source, RuntimeAccessV1::Read, 0, 64),
                    span(destination, RuntimeAccessV1::Write, 0, 64),
                    &[],
                )
                .unwrap();
            let event = f.context.record_event(&predecessor).unwrap();
            let mut control = f
                .context
                .copy_async(
                    f.readback_stream,
                    span(destination, RuntimeAccessV1::Read, 0, 64),
                    span(host, RuntimeAccessV1::Write, 0, 64),
                    &[event],
                )
                .unwrap();
            f.context.release_event(event).unwrap();
            f.finish_backend(predecessor.backend_submission);
            f.finish_backend(control.backend_submission);
            for _ in 0..4 {
                if f.context.poll(&mut control).unwrap() == RuntimePollV1::Succeeded {
                    break;
                }
            }
            assert_eq!(
                f.context.query_submission(&control).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            assert_eq!(f.context.same_device_copies[&control.id].state.depth, 2);
            Some(f.context.record_event(&control).unwrap())
        } else {
            None
        };
        let mut peer = f.peer().unwrap();
        let mut copy = control.map(|control| {
            let event = f.context.record_event(&peer).unwrap();
            let copy = f
                .context
                .copy_async(
                    f.readback_stream,
                    span(f.destination, RuntimeAccessV1::Read, 0, 64),
                    span(f.host, RuntimeAccessV1::Write, 0, 64),
                    &[event, control],
                )
                .unwrap();
            f.context.release_event(event).unwrap();
            f.context.release_event(control).unwrap();
            f.finish_backend(copy.backend_submission);
            copy
        });
        if let Some(copy) = copy.as_mut() {
            assert_eq!(f.context.same_device_copies[&copy.id].state.depth, 3);
            assert_eq!(f.context.poll(copy).unwrap(), RuntimePollV1::Pending);
        }
        f.context
            .scalar_peer_copies
            .get_mut(&peer.id)
            .unwrap()
            .compute = None;
        if let Some(copy) = copy.as_ref() {
            // The completed control preserves depth three despite the missing snapshot.
            f.context
                .validate_same_device_copy_custody_v1(copy.id)
                .unwrap();
        }
        let calls = f.context.backend.poll_call_count;
        let result = match copy.as_mut() {
            Some(copy) => f.context.poll(copy),
            None => f.context.poll(&mut peer),
        };
        assert!(result.is_err());
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert!(f.context.is_terminal());
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
        assert!(
            f.context
                .versions
                .as_ref()
                .unwrap()
                .retains_compute_peer_input_v1(peer.id)
        );
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_forward_and_self_edges_reject_before_recursive_validation() {
    for corruption in 0..3 {
        let mut f = Fixture::new();
        let peer = f.peer().unwrap();
        let mut dependency = f.context.scalar_peer_copies[&peer.id]
            .compute
            .as_ref()
            .unwrap()
            .producer;
        dependency.submission = match corruption {
            0 => peer.id,
            1 => f.producer.id,
            2 => RuntimeSubmissionIdV1 {
                context_generation: f.producer.id.context_generation + 1,
                local: f.producer.id.local,
            },
            _ => unreachable!(),
        };
        let root = f.context.producer_launches.get_mut(&f.producer.id).unwrap();
        root.dependencies.push(dependency);
        root.state.depth = 3;
        let calls = f.context.backend.poll_call_count;
        validation(
            f.context.poll(&mut f.producer),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert!(f.context.is_terminal());
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_inconsistent_backward_ranks_reject_before_recursive_validation() {
    for depth in [0, 2, MAX_RUNTIME_DEPENDENCIES_V1] {
        let mut f = Fixture::new();
        let peer = f.peer().unwrap();
        let event = f.context.record_event(&peer).unwrap();
        let mut copy = f
            .context
            .copy_async(
                f.readback_stream,
                span(f.destination, RuntimeAccessV1::Read, 0, 64),
                span(f.host, RuntimeAccessV1::Write, 0, 64),
                &[event],
            )
            .unwrap();
        assert!(f.producer.id.local < peer.id.local && peer.id.local < copy.id.local);
        f.context
            .producer_launches
            .get_mut(&f.producer.id)
            .unwrap()
            .state
            .depth = depth;
        let calls = f.context.backend.poll_call_count;
        validation(
            f.context.poll(&mut copy),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert!(f.context.is_terminal());
        assert!(!f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_journal_and_current_nonqueued_producer_are_required() {
    assert!(!MockBackend::default().supports_pending_compute_peer_copy_v1());
    let mut plain = Context::open(MockBackend {
        pending_compute_peer: true,
        ..MockBackend::default()
    })
    .unwrap();
    let device = plain.devices()[0].id();
    let stream = plain.create_stream(device).unwrap();
    let source = plain
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = plain.load_module(device, b"plain").unwrap();
    let kernel = plain
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    validation(
        plain.launch_producer_aware_v1(
            stream,
            &kernel,
            &MixedArguments(vec![span(source, RuntimeAccessV1::Write, 0, 64)]),
            geometry(),
            &[],
        ),
        RuntimeValidationErrorV1::Unsupported,
    );
    assert_eq!(plain.backend.submit_count, 0);
    assert!(plain.cleanup().is_complete());
    let mut f = Fixture::new();
    let mut queued = f
        .context
        .launch_producer_aware_v1(
            f.compute_stream,
            &f.kernel,
            &MixedArguments(vec![span(f.source, RuntimeAccessV1::Write, 0, 64)]),
            geometry(),
            &[f.event],
        )
        .unwrap();
    let queued_event = f.context.record_event(&queued).unwrap();
    validation(f.peer(), RuntimeValidationErrorV1::ContextReserved);
    validation(
        f.context.peer_copy(
            f.peer_stream,
            span(f.source, RuntimeAccessV1::Read, 0, 64),
            span(f.destination, RuntimeAccessV1::Write, 0, 64),
            &[queued_event],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.context.backend.copy_call_count, 0);
    f.context.wait(&mut queued, Duration::ZERO).unwrap();
    for _ in 0..4 {
        if f.context.poll(&mut queued).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
    }
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn compute_peer_ordinary_writer_and_retain_overflow_reject_before_issue() {
    let mut f = Fixture::new();
    f.context
        .submissions
        .get_mut(&f.producer.id)
        .unwrap()
        .dependency_retains = usize::MAX;
    validation(f.peer(), RuntimeValidationErrorV1::Capacity);
    f.context
        .submissions
        .get_mut(&f.producer.id)
        .unwrap()
        .dependency_retains = 0;
    assert_eq!(f.context.backend.copy_call_count, 0);
    f.context.wait(&mut f.producer, Duration::ZERO).unwrap();
    f.context.release_event(f.event).unwrap();
    f.producer = f
        .context
        .launch(
            f.compute_stream,
            &f.kernel,
            &MixedArguments(vec![span(f.source, RuntimeAccessV1::Write, 0, 64)]),
            geometry(),
            &[],
        )
        .unwrap();
    f.event = f.context.record_event(&f.producer).unwrap();
    assert!(!f.context.producer_launches.contains_key(&f.producer.id));
    validation(f.peer(), RuntimeValidationErrorV1::ContextReserved);
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert!(!f.context.is_terminal());
    f.cancel_producer();
}
