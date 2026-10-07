//! Real MockBackend operations and journal reconciliation, not GPU execution.

use super::*;

type Calls = Arc<Mutex<Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>;

fn fixture() -> Fixture {
    Fixture::with_sizes([73, 91, 103], |source| {
        vec![span(source, RuntimeAccessV1::Write, 0, 73)]
    })
}

fn peer(f: &mut Fixture, source: u64, destination: u64, bytes: u64) -> Peer {
    f.context
        .peer_copy(
            f.peer_stream,
            span(f.source, RuntimeAccessV1::Read, source, bytes),
            span(f.destination, RuntimeAccessV1::Write, destination, bytes),
            &[f.event],
        )
        .unwrap()
}

fn bytes(f: &mut Fixture, allocation: RuntimeAllocationIdV1, size: usize) -> Vec<u8> {
    let mut result = vec![0; size];
    f.context
        .read_allocation(allocation, 0, &mut result)
        .unwrap();
    result
}

#[test]
fn compute_peer_windows_exact_active_read_bounds_preserve_full_writer_ownership() {
    for backend_done in [false, true] {
        for (source, destination, count) in [(0, 0, 1), (72, 90, 1), (9, 17, 37), (0, 7, 73)] {
            let mut f = fixture();
            let source_before = state(&f.context, f.source);
            if backend_done {
                f.finish_backend(f.producer.backend_submission);
            }
            let mut copied = peer(&mut f, source, destination, count);
            let pending_destination = state(&f.context, f.destination);
            let marker = f.context.submissions[&copied.id]
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
            assert_eq!(
                (
                    lease.read.byte_extent,
                    lease.read.byte_offset,
                    lease.read.byte_len
                ),
                (73, source, count)
            );
            assert_eq!(
                Some(lease.producer),
                f.context.submissions[&f.producer.id].journal_writer
            );
            assert_eq!(lease.read.attempt_epoch, source_before.attempt_epoch);
            assert_eq!(lease.read.content_lineage, source_before.content_lineage);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            f.context.release_event(f.event).unwrap();
            assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
            assert!(f.context.write_allocation(f.source, 0, &[99]).is_err());
            assert!(
                f.context
                    .write_allocation(f.destination, 90, &[99])
                    .is_err()
            );
            assert!(f.context.release_allocation(f.source).is_err());
            assert!(f.context.release_allocation(f.destination).is_err());
            f.finish_backend(f.producer.backend_submission);
            f.finish_backend(copied.backend_submission);
            assert_eq!(f.context.poll(&mut copied).unwrap(), RuntimePollV1::Pending);
            assert_eq!(state(&f.context, f.source), source_before);
            assert_eq!(state(&f.context, f.destination), pending_destination);
            f.finish_peer(&mut copied);
            let output = (f.producer.backend_submission as u8).wrapping_add(37);
            let mut expected = vec![5; 91];
            expected[destination as usize..(destination + count) as usize].fill(output);
            let source_id = f.source;
            let destination_id = f.destination;
            assert_eq!(bytes(&mut f, source_id, 73), vec![output; 73]);
            assert_eq!(bytes(&mut f, destination_id, 91), expected);
            assert_eq!(
                state(&f.context, f.source).content_lineage,
                source_before.content_lineage + 1
            );
            assert_eq!(
                state(&f.context, f.destination).content_lineage,
                pending_destination.content_lineage + 1
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            f.context.release_submission(copied).unwrap();
            f.context.release_submission(f.producer).unwrap();
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn compute_peer_windows_nested_readback_observes_original_parent_before_any_commit() {
    let mut f = fixture();
    let copied = peer(&mut f, 9, 17, 37);
    let event = f.context.record_event(&copied).unwrap();
    let mut readback = f
        .context
        .copy_async(
            f.readback_stream,
            span(f.destination, RuntimeAccessV1::Read, 21, 23),
            span(f.host, RuntimeAccessV1::Write, 29, 23),
            &[event],
        )
        .unwrap();
    f.context.release_event(f.event).unwrap();
    f.context.release_event(event).unwrap();
    assert!(f.context.events.is_empty());
    assert_eq!(
        f.context.completion_parent_depth_v1(readback.id).unwrap(),
        3
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
    let ids = [f.producer.id, copied.id, readback.id];
    let calls = Arc::new(Mutex::new(Vec::new()));
    fn attach<A>(context: &mut Context, submission: &RuntimeSubmissionV1<A>, calls: &Calls) {
        let id = submission.id;
        let calls = Arc::clone(calls);
        context
            .on_completion(submission, move |status| {
                calls.lock().unwrap().push((id, status))
            })
            .unwrap();
    }
    attach(&mut f.context, &f.producer, &calls);
    attach(&mut f.context, &copied, &calls);
    attach(&mut f.context, &readback, &calls);
    let pending = [f.source, f.destination, f.host].map(|id| state(&f.context, id));
    for id in [
        f.producer.backend_submission,
        copied.backend_submission,
        readback.backend_submission,
    ] {
        f.finish_backend(id);
    }
    // Physical mock operations are complete, but final-leaf observation must
    // still reconcile each original Context writer before committing its child.
    for _ in 0..2 {
        assert_eq!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(
            [f.source, f.destination, f.host].map(|id| state(&f.context, id)),
            pending
        );
        assert!(calls.lock().unwrap().is_empty());
    }
    for _ in 0..8 {
        if f.context.poll(&mut readback).unwrap() == RuntimePollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        *calls.lock().unwrap(),
        ids.map(|id| (id, RuntimeCompletionStatusV1::Succeeded))
    );
    for id in ids {
        let record = f.context.submissions[&id];
        assert_eq!(record.status, RuntimeCompletionStatusV1::Succeeded);
        assert!(record.quiescent);
        assert_eq!(record.dependency_retains, 0);
    }
    let output = (f.producer.backend_submission as u8).wrapping_add(37);
    let mut destination = vec![5; 91];
    destination[17..54].fill(output);
    let mut host = vec![7; 103];
    host[29..52].fill(output);
    for (allocation, expected) in [
        (f.source, vec![output; 73]),
        (f.destination, destination),
        (f.host, host),
    ] {
        assert_eq!(bytes(&mut f, allocation, expected.len()), expected);
    }
    for (id, before) in [f.source, f.destination, f.host].into_iter().zip(pending) {
        assert_eq!(
            state(&f.context, id).content_lineage,
            before.content_lineage + 1
        );
    }
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    f.context.release_submission(readback).unwrap();
    f.context.release_submission(copied).unwrap();
    f.context.release_submission(f.producer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn compute_peer_windows_reject_bounds_access_and_event_errors_before_effects() {
    let mut f = fixture();
    let alias = f.context.record_event(&f.producer).unwrap();
    let before = state(&f.context, f.destination);
    let calls = f.context.backend.copy_call_count;
    for (source, destination, source_len, destination_len, expected) in [
        (0, 0, 0, 0, RuntimeValidationErrorV1::InvalidRange),
        (73, 0, 1, 1, RuntimeValidationErrorV1::InvalidRange),
        (0, 91, 1, 1, RuntimeValidationErrorV1::InvalidRange),
        (72, 0, 2, 2, RuntimeValidationErrorV1::InvalidRange),
        (0, 90, 2, 2, RuntimeValidationErrorV1::InvalidRange),
        (u64::MAX, 0, 1, 1, RuntimeValidationErrorV1::InvalidRange),
        (0, u64::MAX, 1, 1, RuntimeValidationErrorV1::InvalidRange),
        (0, 0, 1, 2, RuntimeValidationErrorV1::WrongDevice),
    ] {
        validation(
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, RuntimeAccessV1::Read, source, source_len),
                span(
                    f.destination,
                    RuntimeAccessV1::Write,
                    destination,
                    destination_len,
                ),
                &[f.event],
            ),
            expected,
        );
    }
    for (source_access, destination_access) in [
        (RuntimeAccessV1::ReadWrite, RuntimeAccessV1::Write),
        (RuntimeAccessV1::Read, RuntimeAccessV1::ReadWrite),
    ] {
        validation(
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, source_access, 9, 37),
                span(f.destination, destination_access, 17, 37),
                &[f.event],
            ),
            RuntimeValidationErrorV1::ContextReserved,
        );
    }
    for (events, expected) in [
        (vec![], RuntimeValidationErrorV1::ContextReserved),
        (
            vec![f.event, alias],
            RuntimeValidationErrorV1::DuplicateDependency,
        ),
    ] {
        validation(
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, RuntimeAccessV1::Read, 9, 37),
                span(f.destination, RuntimeAccessV1::Write, 17, 37),
                &events,
            ),
            expected,
        );
    }
    let device = f.context.devices()[0].id();
    let other = f
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 16)
        .unwrap();
    f.context.write_allocation(other, 0, &[19; 8]).unwrap();
    let mut unrelated = f
        .context
        .launch_producer_aware_v1(
            f.compute_stream,
            &f.kernel,
            &MixedArguments(vec![span(other, RuntimeAccessV1::Write, 0, 8)]),
            geometry(),
            &[],
        )
        .unwrap();
    let unrelated_event = f.context.record_event(&unrelated).unwrap();
    for events in [vec![unrelated_event], vec![f.event, unrelated_event]] {
        validation(
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, RuntimeAccessV1::Read, 9, 37),
                span(f.destination, RuntimeAccessV1::Write, 17, 37),
                &events,
            ),
            RuntimeValidationErrorV1::ContextReserved,
        );
    }
    assert_eq!(f.context.submissions[&unrelated.id].dependency_retains, 0);
    f.context.release_event(unrelated_event).unwrap();
    f.context.backend.cancel_before_publication = true;
    f.context.cancel(&mut unrelated).unwrap();
    f.context.release_submission(unrelated).unwrap();
    f.context.release_event(alias).unwrap();
    validation(
        f.context.peer_copy(
            f.peer_stream,
            span(f.source, RuntimeAccessV1::Read, 9, 37),
            span(f.destination, RuntimeAccessV1::Write, 17, 37),
            &[alias],
        ),
        RuntimeValidationErrorV1::UnknownEvent,
    );
    f.context.backend.pending_compute_peer = false;
    validation(
        f.context.peer_copy(
            f.peer_stream,
            span(f.source, RuntimeAccessV1::Read, 9, 37),
            span(f.destination, RuntimeAccessV1::Write, 17, 37),
            &[f.event],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.context.backend.copy_call_count, calls);
    assert_eq!(state(&f.context, f.destination), before);
    assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.scalar_peer_copies.is_empty());
    assert!(!f.context.is_terminal());
    f.cancel_producer();
}

#[test]
fn compute_peer_windows_reject_partial_or_aliased_writer_even_when_it_covers_read() {
    for mode in 0..6 {
        let mut f = Fixture::with_bindings(|source| match mode {
            0 => vec![span(source, RuntimeAccessV1::Write, 0, 32)],
            1 => vec![span(source, RuntimeAccessV1::Write, 8, 32)],
            2 => vec![span(source, RuntimeAccessV1::ReadWrite, 0, 64)],
            3 => vec![
                span(source, RuntimeAccessV1::Write, 0, 64),
                span(source, RuntimeAccessV1::Read, 12, 8),
            ],
            4 => vec![
                span(source, RuntimeAccessV1::Write, 0, 32),
                span(source, RuntimeAccessV1::Write, 32, 32),
            ],
            _ => vec![
                span(source, RuntimeAccessV1::Write, 0, 64),
                span(source, RuntimeAccessV1::Write, 0, 64),
            ],
        });
        let before = state(&f.context, f.destination);
        validation(
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, RuntimeAccessV1::Read, 12, 8),
                span(f.destination, RuntimeAccessV1::Write, 19, 8),
                &[f.event],
            ),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(f.context.backend.copy_call_count, 0);
        assert_eq!(state(&f.context, f.destination), before);
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        f.cancel_producer();
    }
}

#[test]
fn compute_peer_windows_cancellation_refunds_only_the_exact_consumer() {
    for cancel_parent in [false, true] {
        let mut f = fixture();
        let before = state(&f.context, f.source);
        let mut copied = peer(&mut f, 9, 17, 37);
        f.context.release_event(f.event).unwrap();
        f.context.backend.cancel_before_publication = true;
        if cancel_parent {
            f.context.cancel(&mut f.producer).unwrap();
            assert!(f.context.validate_scalar_peer_custody_v1(copied.id).is_ok());
            f.context
                .backend
                .producer_launch
                .observations
                .insert(copied.backend_submission, Observation::Failed);
            assert_eq!(
                f.context.poll(&mut copied).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            );
        } else {
            assert_eq!(
                f.context.cancel(&mut copied).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            assert_eq!(state(&f.context, f.source), before);
            assert_eq!(
                f.context.query_submission(&f.producer).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
        }
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        f.context.release_submission(copied).unwrap();
        if !cancel_parent {
            f.context.cancel(&mut f.producer).unwrap();
        }
        f.context.release_submission(f.producer).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_windows_retained_endpoint_drift_fails_before_observation() {
    for backend_done in [false, true] {
        for corruption in 0..7 {
            let mut f = fixture();
            let mut copied = peer(&mut f, 9, 17, 37);
            f.context.release_event(f.event).unwrap();
            if backend_done {
                f.finish_backend(f.producer.backend_submission);
                f.finish_backend(copied.backend_submission);
            }
            let root = f.context.scalar_peer_copies.get_mut(&copied.id).unwrap();
            match corruption {
                0 => root.source.region.byte_offset += 1,
                1 => root.destination.region.byte_offset += 1,
                2 => {
                    root.source.region.byte_len -= 1;
                    root.destination.region.byte_len -= 1;
                }
                3 => root.source.record.byte_len += 1,
                4 => root.destination.record.byte_len += 1,
                5 => root.source.region.allocation = f.destination,
                _ => root.destination.region.allocation = f.source,
            }
            let calls = f.context.backend.poll_call_count;
            assert!(f.context.poll(&mut copied).is_err());
            assert_eq!(f.context.backend.poll_call_count, calls);
            assert!(f.context.is_terminal());
            assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            assert!(!f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn compute_peer_windows_preissue_errors_keep_disposition_specific_custody() {
    for fault in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = fixture();
        if fault == MockMemoryFailure::Quiescent {
            f.finish_backend(f.producer.backend_submission);
        }
        f.context.backend.copy_failure = fault;
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.context.peer_copy(
                f.peer_stream,
                span(f.source, RuntimeAccessV1::Read, 9, 37),
                span(f.destination, RuntimeAccessV1::Write, 17, 37),
                &[f.event],
            )
        }));
        match (fault, result) {
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
            _ => panic!("window admission fault lost classification"),
        }
    }
}
