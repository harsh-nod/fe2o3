use super::async_journal_tests::{state, writer_state};
use super::peer_directed_tests::Observation;
use super::*;
use fe2o3_runtime_model::ContextWriterStateV1;
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<MockBackend>;
type Peer = RuntimeSubmissionV1<RuntimePeerCopyV1>;
type Copy = RuntimeSubmissionV1<RuntimeCopyV1>;

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    source: RuntimeAllocationIdV1,
    device: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    peer: Peer,
    event: RuntimeEventIdV1,
    bytes: Vec<u8>,
}

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

impl Fixture {
    fn new() -> Self {
        let backend = MockBackend {
            deferred_copies: true,
            pending_peer_readback: true,
            ..MockBackend::default()
        };
        let mut context =
            Context::open_with_version_journal_members_v1(backend, 32, 8, 32).unwrap();
        let first = context.devices()[0].id();
        let second = context.devices()[1].id();
        let peer_stream = context.create_stream(second).unwrap();
        let stream = context.create_stream(second).unwrap();
        let source = context
            .allocate(first, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let device = context
            .allocate(second, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let host = context
            .allocate(second, RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        let bytes = (0..64)
            .map(|index| (index * 3 + 1) as u8)
            .collect::<Vec<_>>();
        context.write_allocation(source, 0, &bytes).unwrap();
        context.write_allocation(device, 0, &[0x55; 64]).unwrap();
        context.write_allocation(host, 0, &[0xaa; 64]).unwrap();
        let peer = context
            .peer_copy(
                peer_stream,
                span(source, RuntimeAccessV1::Read, 8, 16),
                span(device, RuntimeAccessV1::Write, 16, 16),
                &[],
            )
            .unwrap();
        let event = context.record_event(&peer).unwrap();
        Self {
            context,
            stream,
            source,
            device,
            host,
            peer,
            event,
            bytes,
        }
    }

    fn copy(&mut self) -> Result<Copy, RuntimeErrorV1<MockError>> {
        self.context.copy_async(
            self.stream,
            span(self.device, RuntimeAccessV1::Read, 20, 8),
            span(self.host, RuntimeAccessV1::Write, 4, 8),
            &[self.event],
        )
    }

    fn finish_backend(&mut self, id: u64) {
        self.context.backend.finish_submission(id, true);
        self.context.backend.polls.insert(id, 1);
    }

    fn complete(&mut self, copy: &mut Copy) {
        self.finish_backend(self.peer.backend_submission);
        self.finish_backend(copy.backend_submission);
        for _ in 0..4 {
            if self.context.poll(copy).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("bounded copy reconciliation did not finish");
    }

    fn cancel_pending(&mut self) {
        self.context.backend.cancel_before_publication = true;
        self.context.cancel(&mut self.peer).unwrap();
        assert!(self.context.cleanup().is_complete());
    }
}

fn validation<T>(result: Result<T, RuntimeErrorV1<MockError>>, expected: RuntimeValidationErrorV1) {
    match result {
        Err(RuntimeErrorV1::Validation(actual)) => assert_eq!(actual, expected),
        _ => panic!("expected validation rejection: {expected:?}"),
    }
}

#[test]
fn copy_custody_reconciles_actual_parent_before_readback_commit() {
    for backend_already_done in [false, true] {
        for event_observer in [false, true] {
            let mut f = Fixture::new();
            let before = state(&f.context, f.device);
            if backend_already_done {
                f.finish_backend(f.peer.backend_submission);
            }
            let mut copy = f.copy().unwrap();
            let record = f.context.submissions[&copy.id];
            assert!(record.same_device_copy);
            assert!(
                !record.scalar_peer_copy && !record.directed_peer_copy && !record.producer_launch
            );
            assert_eq!(
                f.context
                    .cleanup_report(Vec::new())
                    .same_device_copy_records_v1(),
                1
            );
            assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 1);
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
                f.context.submissions[&f.peer.id].journal_writer
            );
            assert_eq!(lease.read.attempt_epoch, before.attempt_epoch);
            assert_eq!(lease.read.content_lineage, before.content_lineage);
            assert_eq!((lease.read.byte_offset, lease.read.byte_len), (20, 8));
            let output_before = state(&f.context, f.host);
            let event = f.context.record_event(&copy).unwrap();
            f.context.release_event(f.event).unwrap();
            let calls = Arc::new(Mutex::new(Vec::new()));
            for (id, is_parent) in [(f.peer.id, true), (copy.id, false)] {
                let calls = Arc::clone(&calls);
                let callback = move |status| {
                    calls.lock().unwrap().push((id, status));
                };
                if is_parent {
                    f.context.on_completion(&f.peer, callback).unwrap();
                } else {
                    f.context.on_completion(&copy, callback).unwrap();
                }
            }
            f.finish_backend(f.peer.backend_submission);
            f.finish_backend(copy.backend_submission);
            if event_observer {
                f.context.poll_event(event).unwrap();
            } else {
                assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
            }
            assert_eq!(
                f.context.query_submission(&copy).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(
                f.context.query_submission(&f.peer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(state(&f.context, f.device), before);
            assert_eq!(state(&f.context, f.host), output_before);
            assert_eq!(
                f.context.same_device_copies[&copy.id].state.terminal,
                Some(BackendPollV1::Succeeded)
            );
            assert!(calls.lock().unwrap().is_empty());
            f.complete(&mut copy);
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    (f.peer.id, RuntimeCompletionStatusV1::Succeeded),
                    (copy.id, RuntimeCompletionStatusV1::Succeeded)
                ]
            );
            assert_eq!(
                state(&f.context, f.host).content_lineage,
                output_before.content_lineage + 1
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            let mut bytes = [0; 64];
            f.context.read_allocation(f.host, 0, &mut bytes).unwrap();
            assert_eq!(&bytes[4..12], &f.bytes[12..20]);
            assert!(
                bytes[..4]
                    .iter()
                    .chain(&bytes[12..])
                    .all(|byte| *byte == 0xaa)
            );
            f.context.release_submission(f.peer).unwrap();
            f.context.release_event(event).unwrap();
            f.context.release_submission(copy).unwrap();
            assert_eq!(
                f.context
                    .cleanup_report(Vec::new())
                    .same_device_copy_records_v1(),
                0
            );
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn copy_custody_opt_in_and_exact_event_range_are_required_before_issue() {
    let mut f = Fixture::new();
    let before = state(&f.context, f.host);
    let calls = f.context.backend.copy_call_count;
    f.context.backend.pending_peer_readback = false;
    validation(f.copy(), RuntimeValidationErrorV1::ContextReserved);
    f.context.backend.pending_peer_readback = true;
    let alias = f.context.record_event(&f.peer).unwrap();
    for (offset, dependencies, expected) in [
        (20, vec![], RuntimeValidationErrorV1::ContextReserved),
        (15, vec![f.event], RuntimeValidationErrorV1::ContextReserved),
        (31, vec![f.event], RuntimeValidationErrorV1::ContextReserved),
        (
            20,
            vec![f.event, alias],
            RuntimeValidationErrorV1::DuplicateDependency,
        ),
    ] {
        validation(
            f.context.copy_async(
                f.stream,
                span(f.device, RuntimeAccessV1::Read, offset, 8),
                span(f.host, RuntimeAccessV1::Write, 4, 8),
                &dependencies,
            ),
            expected,
        );
    }
    validation(
        f.context.copy_async(
            f.stream,
            span(f.source, RuntimeAccessV1::Read, 20, 8),
            span(f.host, RuntimeAccessV1::Write, 4, 8),
            &[f.event],
        ),
        RuntimeValidationErrorV1::WrongDevice,
    );
    assert_eq!(f.context.backend.copy_call_count, calls);
    assert!(f.context.same_device_copies.is_empty());
    assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 0);
    assert_eq!(state(&f.context, f.host), before);
    assert!(!f.context.is_terminal());
    f.cancel_pending();
}

#[test]
fn copy_custody_requires_current_writer_not_previous_completed_event() {
    let mut f = Fixture::new();
    assert_eq!(
        f.context.wait(&mut f.peer, Duration::ZERO).unwrap(),
        RuntimePollV1::Succeeded
    );
    let current = f
        .context
        .peer_copy(
            f.context.submissions[&f.peer.id].stream,
            span(f.source, RuntimeAccessV1::Read, 8, 16),
            span(f.device, RuntimeAccessV1::Write, 16, 16),
            &[],
        )
        .unwrap();
    let calls = f.context.backend.copy_call_count;
    validation(f.copy(), RuntimeValidationErrorV1::ContextReserved);
    assert_eq!(f.context.backend.copy_call_count, calls);
    f.peer = current;
    f.event = f.context.record_event(&f.peer).unwrap();
    let mut copy = f.copy().unwrap();
    assert_eq!(
        f.context.same_device_copies[&copy.id].producer.submission,
        f.peer.id
    );
    f.complete(&mut copy);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn copy_custody_cancellation_releases_only_consumer_leases_and_retains() {
    let mut f = Fixture::new();
    let before = state(&f.context, f.device);
    let mut copy = f.copy().unwrap();
    f.context.release_event(f.event).unwrap();
    assert!(f.context.release_allocation(f.host).is_err());
    assert!(f.context.write_allocation(f.device, 20, &[9; 8]).is_err());
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut copy).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(state(&f.context, f.device), before);
    assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 0);
    assert_eq!(
        f.context.query_submission(&f.peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert!(!f.context.same_device_copies[&copy.id].dependencies_held);
    f.context.release_submission(copy).unwrap();
    f.context.release_allocation(f.host).unwrap();
    f.cancel_pending();
}

#[test]
fn copy_custody_failed_cancelled_or_unknown_parent_keeps_valid_identity() {
    for outcome in 0..3 {
        let mut f = Fixture::new();
        let mut copy = f.copy().unwrap();
        f.context.release_event(f.event).unwrap();
        match outcome {
            0 => {
                f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: 7 });
                f.context.wait(&mut f.peer, Duration::ZERO).unwrap();
            }
            1 => {
                f.context.backend.cancel_before_publication = true;
                f.context.cancel(&mut f.peer).unwrap();
            }
            2 => {
                f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
                assert!(matches!(
                    f.context.wait(&mut f.peer, Duration::ZERO),
                    Err(RuntimeErrorV1::BackendQuiescent(_))
                ));
            }
            _ => unreachable!(),
        }
        assert!(
            f.context
                .validate_same_device_copy_custody_v1(copy.id)
                .is_ok()
        );
        f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: 7 });
        assert!(matches!(
            f.context.wait(&mut copy, Duration::ZERO).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        ));
        assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(!f.context.is_terminal());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn copy_custody_preissue_rejection_refunds_and_uncertain_issue_retains() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new();
        let reads = f.context.version_journal_read_records_v1();
        if failure == MockMemoryFailure::Quiescent {
            f.finish_backend(f.peer.backend_submission);
        }
        f.context.backend.copy_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.copy()));
        match (failure, result) {
            (MockMemoryFailure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
            | (MockMemoryFailure::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                assert!(f.context.same_device_copies.is_empty());
                assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 0);
                assert_eq!(f.context.version_journal_read_records_v1(), reads);
                assert!(!f.context.is_terminal());
                if failure == MockMemoryFailure::Quiescent {
                    assert_eq!(
                        f.context.wait(&mut f.peer, Duration::ZERO).unwrap(),
                        RuntimePollV1::Succeeded
                    );
                    assert!(f.context.cleanup().is_complete());
                } else {
                    f.cancel_pending();
                }
            }
            (MockMemoryFailure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (MockMemoryFailure::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                assert_eq!(f.context.same_device_copies.len(), 1);
                assert!(
                    f.context
                        .same_device_copies
                        .values()
                        .all(|root| root.dependencies_held)
                );
                assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 1);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
                assert!(!f.context.cleanup().is_complete());
            }
            _ => panic!("copy issue fault lost its classification"),
        }
    }
}

#[test]
fn copy_custody_descended_parent_fault_preserves_unknown_or_terminal_roots() {
    for fault in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new();
        let mut copy = f.copy().unwrap();
        f.context.release_event(f.event).unwrap();
        f.finish_backend(f.peer.backend_submission);
        f.finish_backend(copy.backend_submission);
        assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
        let before = state(&f.context, f.host);
        f.context
            .backend
            .producer_launch
            .observations
            .insert(f.peer.backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut copy)));
        match (fault, result) {
            (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                assert_eq!(
                    f.context.query_submission(&copy).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert_eq!(
                    f.context.query_submission(&f.peer).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
                assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 0);
                assert!(!f.context.is_terminal());
                assert_eq!(
                    writer_state(&f.context, f.host),
                    ContextWriterStateV1::Unknown { member_count: 1 }
                );
                assert!(f.context.cleanup().is_complete());
            }
            (Observation::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
            | (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (Observation::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
                assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 1);
                assert!(f.context.same_device_copies[&copy.id].dependencies_held);
                assert!(!f.context.cleanup().is_complete());
            }
            _ => panic!("descended copy fault lost its classification"),
        }
        // Neither an unknown nor an uncertain result commits the output version.
        if f.context.allocations.contains_key(&f.host) {
            assert_eq!(
                state(&f.context, f.host).content_lineage,
                before.content_lineage
            );
        }
    }
}

#[test]
fn copy_custody_corrupted_parent_or_consumer_fails_closed_before_observation() {
    for corruption in 0..5 {
        let mut f = Fixture::new();
        let mut copy = f.copy().unwrap();
        match corruption {
            0 => {
                f.context
                    .same_device_copies
                    .get_mut(&copy.id)
                    .unwrap()
                    .source
                    .record
                    .byte_len -= 1;
            }
            1 => {
                f.context
                    .same_device_copies
                    .get_mut(&copy.id)
                    .unwrap()
                    .producer
                    .backend_submission += 1000;
            }
            2 => {
                f.context
                    .submissions
                    .get_mut(&copy.id)
                    .unwrap()
                    .journal_writer = None;
            }
            3 => {
                f.context.submissions.get_mut(&f.peer.id).unwrap().status =
                    RuntimeCompletionStatusV1::Succeeded;
            }
            4 => {
                f.context
                    .submissions
                    .get_mut(&copy.id)
                    .unwrap()
                    .journal_producer_read = None;
            }
            _ => unreachable!(),
        }
        let calls = f.context.backend.poll_call_count;
        assert!(f.context.poll(&mut copy).is_err());
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert!(f.context.is_terminal());
        assert_eq!(f.context.submissions[&f.peer.id].dependency_retains, 1);
        assert!(!f.context.cleanup().is_complete());
    }
}
