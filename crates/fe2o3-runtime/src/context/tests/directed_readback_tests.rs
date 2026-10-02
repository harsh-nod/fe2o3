use super::async_journal_tests::{state, writer_state};
use super::peer_directed_tests::Observation;
use super::*;
use fe2o3_runtime_model::ContextWriterStateV1;
use std::sync::{Arc, Mutex};

type Context = RuntimeContextV1<MockBackend>;
type Peer = RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>;
type Copy = RuntimeSubmissionV1<RuntimeCopyV1>;

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    allocations: Vec<RuntimeAllocationIdV1>,
    host: RuntimeAllocationIdV1,
    peers: Vec<Peer>,
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

fn validation<T>(result: Result<T, RuntimeErrorV1<MockError>>, expected: RuntimeValidationErrorV1) {
    match result {
        Err(RuntimeErrorV1::Validation(actual)) => assert_eq!(actual, expected),
        _ => panic!("expected validation rejection: {expected:?}"),
    }
}

impl Fixture {
    fn new(count: usize) -> Self {
        let backend = MockBackend {
            deferred_copies: true,
            pending_directed_peer_readback: true,
            ..MockBackend::default()
        };
        let mut context =
            Context::open_with_version_journal_members_v1(backend, 32, 16, 32).unwrap();
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let allocations = (0..=count)
            .map(|index| {
                context
                    .allocate(devices[index % 2], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let bytes = (0..64)
            .map(|index| (index * 3 + 1) as u8)
            .collect::<Vec<_>>();
        for (index, allocation) in allocations.iter().enumerate() {
            context
                .write_allocation(
                    *allocation,
                    0,
                    if index == 0 { &bytes } else { &[0x55; 64] },
                )
                .unwrap();
        }
        let host = context
            .allocate(devices[count % 2], RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        context.write_allocation(host, 0, &[0xaa; 64]).unwrap();
        let stream = context.create_stream(devices[count % 2]).unwrap();
        let mut peers = Vec::new();
        let mut event = None;
        for index in 0..count {
            let peer_stream = context.create_stream(devices[(index + 1) % 2]).unwrap();
            let peer = context
                .directed_peer_copy_v1(
                    peer_stream,
                    span(allocations[index], RuntimeAccessV1::Read, 8, 16),
                    span(allocations[index + 1], RuntimeAccessV1::Write, 8, 16),
                    event.as_slice(),
                )
                .unwrap();
            if let Some(previous) = event {
                context.release_event(previous).unwrap();
            }
            event = Some(context.record_event(&peer).unwrap());
            peers.push(peer);
        }
        Self {
            context,
            stream,
            allocations,
            host,
            peers,
            event: event.unwrap(),
            bytes,
        }
    }

    fn device(&self) -> RuntimeAllocationIdV1 {
        *self.allocations.last().unwrap()
    }

    fn copy(&mut self) -> Result<Copy, RuntimeErrorV1<MockError>> {
        self.context.copy_async(
            self.stream,
            span(self.device(), RuntimeAccessV1::Read, 12, 8),
            span(self.host, RuntimeAccessV1::Write, 4, 8),
            &[self.event],
        )
    }

    // Script backend completion order; only the leaf is subsequently observed through Context.
    fn complete_physical(&mut self, copy: &Copy) {
        for peer in &self.peers {
            self.context
                .backend
                .finish_submission(peer.backend_submission, true);
        }
        self.context
            .backend
            .finish_submission(copy.backend_submission, true);
        self.context
            .backend
            .polls
            .insert(copy.backend_submission, 1);
    }

    fn observe_leaf(&mut self, copy: &mut Copy) {
        for _ in 0..8 {
            if self.context.poll(copy).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("bounded directed readback reconciliation did not finish");
    }

    fn cancel_pending(&mut self) {
        self.context.backend.cancel_before_publication = true;
        for peer in self.peers.iter_mut().rev() {
            if self.context.query_submission(peer).unwrap() == RuntimeCompletionStatusV1::Pending {
                self.context.cancel(peer).unwrap();
            }
        }
        assert!(self.context.cleanup().is_complete());
    }
}

#[test]
fn directed_readback_leaf_observation_reconciles_released_event_ancestors() {
    for count in [1, 2] {
        let mut f = Fixture::new(count);
        assert!(!f.context.backend.pending_peer_readback);
        let device_before = state(&f.context, f.device());
        let host_before = state(&f.context, f.host);
        let mut copy = f.copy().unwrap();
        let host_pending = state(&f.context, f.host);
        assert_eq!(host_pending.attempt_epoch, host_before.attempt_epoch + 1);
        assert_eq!(host_pending.content_lineage, host_before.content_lineage);
        let root = &f.context.same_device_copies[&copy.id];
        assert_eq!(root.producer.submission, f.peers.last().unwrap().id);
        assert_eq!(root.state.depth, count + 1);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(count + 1));
        for peer in &f.peers {
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
        }
        let marker = f.context.submissions[&copy.id]
            .journal_producer_read
            .unwrap();
        let lease = f
            .context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .lookup_producer_read(marker.active_first_for_test().unwrap())
            .unwrap();
        assert_eq!((lease.read.byte_offset, lease.read.byte_len), (12, 8));
        assert_eq!(
            Some(lease.producer),
            f.context.submissions[&f.peers.last().unwrap().id].journal_writer
        );
        let calls = Arc::new(Mutex::new(Vec::new()));
        for peer in &f.peers {
            let calls = Arc::clone(&calls);
            let id = peer.id;
            f.context
                .on_completion(peer, move |status| calls.lock().unwrap().push((id, status)))
                .unwrap();
        }
        let copy_calls = Arc::clone(&calls);
        let id = copy.id;
        f.context
            .on_completion(&copy, move |status| {
                copy_calls.lock().unwrap().push((id, status))
            })
            .unwrap();
        f.context.release_event(f.event).unwrap();
        assert!(f.context.events.is_empty());
        assert!(f.context.release_allocation(f.host).is_err());
        f.context.backend.cleanup_failure = MockCleanupFailure::RejectStreamOnce;
        assert!(matches!(
            f.context.destroy_stream(f.stream),
            Err(RuntimeErrorV1::BackendRejected(_))
        ));
        assert!(f.context.streams.contains_key(&f.stream));
        assert_eq!(
            f.context.query_submission(&copy).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        assert!(
            f.context
                .backend
                .pending_copies
                .contains_key(&copy.backend_submission)
        );
        assert!(f.context.same_device_copies[&copy.id].dependencies_held);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(count + 1));
        for peer in &f.peers {
            assert_eq!(
                f.context.query_submission(peer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
        }
        assert_eq!(state(&f.context, f.device()), device_before);
        assert_eq!(state(&f.context, f.host), host_pending);
        f.complete_physical(&copy);
        assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
        assert_eq!(state(&f.context, f.device()), device_before);
        assert_eq!(state(&f.context, f.host), host_pending);
        assert!(calls.lock().unwrap().is_empty());
        f.observe_leaf(&mut copy);
        let mut expected = f
            .peers
            .iter()
            .map(|peer| (peer.id, RuntimeCompletionStatusV1::Succeeded))
            .collect::<Vec<_>>();
        expected.push((copy.id, RuntimeCompletionStatusV1::Succeeded));
        assert_eq!(*calls.lock().unwrap(), expected);
        for peer in &f.peers {
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
            assert!(
                f.context
                    .backend
                    .directed_calls
                    .contains(&("poll", peer.backend_submission))
            );
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(
            state(&f.context, f.host).content_lineage,
            host_before.content_lineage + 1
        );
        let mut observed = [0; 64];
        f.context.read_allocation(f.host, 0, &mut observed).unwrap();
        let mut expected_bytes = [0xaa; 64];
        expected_bytes[4..12].copy_from_slice(&f.bytes[12..20]);
        assert_eq!(observed, expected_bytes);
        f.context
            .read_allocation(f.allocations[0], 0, &mut observed)
            .unwrap();
        assert_eq!(observed.as_slice(), f.bytes);
        f.context.release_submission(copy).unwrap();
        for peer in f.peers.into_iter().rev() {
            f.context.release_submission(peer).unwrap();
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_readback_requires_distinct_opt_in_and_exact_dependency_range() {
    assert!(!MockBackend::default().supports_pending_directed_peer_readback_v1());
    let mut f = Fixture::new(1);
    let host_before = state(&f.context, f.host);
    let calls = f.context.backend.copy_call_count;
    f.context.backend.pending_directed_peer_readback = false;
    for ordinary in [false, true] {
        f.context.backend.pending_peer_readback = ordinary;
        validation(f.copy(), RuntimeValidationErrorV1::ContextReserved);
    }
    f.context.backend.pending_directed_peer_readback = true;
    f.context.backend.pending_peer_readback = false;
    let alias = f.context.record_event(&f.peers[0]).unwrap();
    for (offset, dependencies, expected) in [
        (12, vec![], RuntimeValidationErrorV1::ContextReserved),
        (7, vec![f.event], RuntimeValidationErrorV1::ContextReserved),
        (17, vec![f.event], RuntimeValidationErrorV1::ContextReserved),
        (63, vec![f.event], RuntimeValidationErrorV1::InvalidRange),
        (
            12,
            vec![f.event, alias],
            RuntimeValidationErrorV1::DuplicateDependency,
        ),
    ] {
        validation(
            f.context.copy_async(
                f.stream,
                span(f.device(), RuntimeAccessV1::Read, offset, 8),
                span(f.host, RuntimeAccessV1::Write, 4, 8),
                &dependencies,
            ),
            expected,
        );
    }
    validation(
        f.context.copy_async(
            f.stream,
            span(f.allocations[0], RuntimeAccessV1::Read, 12, 8),
            span(f.host, RuntimeAccessV1::Write, 4, 8),
            &[f.event],
        ),
        RuntimeValidationErrorV1::WrongDevice,
    );
    assert_eq!(f.context.backend.copy_call_count, calls);
    assert_eq!(state(&f.context, f.host), host_before);
    assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert!(f.context.same_device_copies.is_empty());
    assert!(!f.context.is_terminal());
    f.cancel_pending();
}

#[test]
fn directed_readback_requires_latest_writer_not_old_completed_event() {
    let mut f = Fixture::new(1);
    assert_eq!(
        f.context.wait(&mut f.peers[0], Duration::ZERO).unwrap(),
        RuntimePollV1::Succeeded
    );
    let peer = f
        .context
        .directed_peer_copy_v1(
            f.context.submissions[&f.peers[0].id].stream,
            span(f.allocations[0], RuntimeAccessV1::Read, 8, 16),
            span(f.device(), RuntimeAccessV1::Write, 8, 16),
            &[],
        )
        .unwrap();
    let calls = f.context.backend.copy_call_count;
    validation(f.copy(), RuntimeValidationErrorV1::ContextReserved);
    assert_eq!(f.context.backend.copy_call_count, calls);
    f.context.release_event(f.event).unwrap();
    f.event = f.context.record_event(&peer).unwrap();
    let old = std::mem::replace(&mut f.peers[0], peer);
    f.context.release_submission(old).unwrap();
    let mut copy = f.copy().unwrap();
    assert_eq!(
        f.context.same_device_copies[&copy.id].producer.submission,
        f.peers[0].id
    );
    f.context.release_event(f.event).unwrap();
    f.complete_physical(&copy);
    f.observe_leaf(&mut copy);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn directed_readback_cancellation_refunds_only_consumer_custody() {
    let mut f = Fixture::new(2);
    let before = state(&f.context, f.device());
    let mut copy = f.copy().unwrap();
    f.context.release_event(f.event).unwrap();
    assert!(f.context.write_allocation(f.device(), 12, &[9; 8]).is_err());
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut copy).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(state(&f.context, f.device()), before);
    assert_eq!(f.context.submissions[&f.peers[1].id].dependency_retains, 0);
    assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 1);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    for peer in &f.peers {
        assert_eq!(
            f.context.query_submission(peer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
    }
    assert!(!f.context.same_device_copies[&copy.id].dependencies_held);
    f.context.release_submission(copy).unwrap();
    f.context.release_allocation(f.host).unwrap();
    f.cancel_pending();
}

#[test]
fn directed_readback_failed_cancelled_unknown_parent_remains_authenticated() {
    for outcome in 0..3 {
        let mut f = Fixture::new(1);
        let mut copy = f.copy().unwrap();
        let before = state(&f.context, f.host);
        f.context.release_event(f.event).unwrap();
        match outcome {
            0 => {
                f.context
                    .backend
                    .directed_observations
                    .insert(f.peers[0].backend_submission, Observation::Failed);
                assert_eq!(
                    f.context.poll(&mut f.peers[0]).unwrap(),
                    RuntimePollV1::Failed { code: 7 }
                );
            }
            1 => {
                f.context.backend.cancel_before_publication = true;
                assert_eq!(
                    f.context.cancel(&mut f.peers[0]).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
            }
            2 => {
                f.context
                    .backend
                    .directed_observations
                    .insert(f.peers[0].backend_submission, Observation::Quiescent);
                assert!(matches!(
                    f.context.poll(&mut f.peers[0]),
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
        assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 1);
        f.context.backend.wait_observation = Some(BackendPollV1::Failed { code: 7 });
        assert_eq!(
            f.context.wait(&mut copy, Duration::ZERO).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(
            state(&f.context, f.host).content_lineage,
            before.content_lineage
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 0);
        assert!(!f.context.is_terminal());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn directed_readback_issue_faults_preserve_rejection_or_uncertain_custody() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Fixture::new(1);
        f.context.backend.copy_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.copy()));
        match (failure, result) {
            (MockMemoryFailure::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_)))) => {
                assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
                assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 0);
                assert!(f.context.same_device_copies.is_empty());
                assert!(!f.context.is_terminal());
                f.cancel_pending();
            }
            (MockMemoryFailure::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (MockMemoryFailure::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
                assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 1);
                assert_eq!(f.context.same_device_copies.len(), 1);
                assert!(
                    f.context
                        .same_device_copies
                        .values()
                        .all(|root| root.dependencies_held)
                );
                assert!(!f.context.cleanup().is_complete());
            }
            _ => panic!("directed readback issue fault lost classification"),
        }
    }
}

#[test]
fn directed_readback_descended_parent_fault_never_commits_output() {
    for fault in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Fixture::new(1);
        let mut copy = f.copy().unwrap();
        f.context.release_event(f.event).unwrap();
        let before = state(&f.context, f.host);
        f.complete_physical(&copy);
        assert_eq!(f.context.poll(&mut copy).unwrap(), RuntimePollV1::Pending);
        f.context
            .backend
            .directed_observations
            .insert(f.peers[0].backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut copy)));
        match (fault, result) {
            (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                assert_eq!(
                    f.context.query_submission(&copy).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert_eq!(
                    f.context.query_submission(&f.peers[0]).unwrap(),
                    RuntimeCompletionStatusV1::QuiescentWithoutResult
                );
                assert_eq!(
                    writer_state(&f.context, f.host),
                    ContextWriterStateV1::Unknown { member_count: 1 }
                );
                assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
                assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 0);
                assert!(!f.context.is_terminal());
            }
            (Observation::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
            | (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (Observation::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
                assert_eq!(f.context.submissions[&f.peers[0].id].dependency_retains, 1);
                assert!(f.context.same_device_copies[&copy.id].dependencies_held);
            }
            _ => panic!("directed readback parent fault lost classification"),
        }
        assert_eq!(
            state(&f.context, f.host).content_lineage,
            before.content_lineage
        );
        assert_eq!(f.context.cleanup().is_complete(), !f.context.is_terminal());
    }
}
