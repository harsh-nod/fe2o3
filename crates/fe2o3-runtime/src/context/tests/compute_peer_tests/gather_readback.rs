//! Queued writer/read leases over real mock operations, not native DMA evidence.

use super::gather::Gather;
use super::*;
use fe2o3_runtime_model::ContextProducerReadStatusV1;

type Readback = RuntimeSubmissionV1<RuntimeCopyV1>;

fn readback(f: &mut Gather, dependencies: &[RuntimeEventIdV1]) -> Readback {
    f.context
        .copy_async(
            f.readback_stream,
            span(f.destination, RuntimeAccessV1::Read, 0, 257),
            span(f.host, RuntimeAccessV1::Write, 0, 257),
            dependencies,
        )
        .unwrap()
}

fn cancel(f: &mut Gather, readback: Option<&mut Readback>) {
    f.release_events();
    f.context.backend.cancel_before_publication = true;
    if let Some(readback) = readback {
        assert_eq!(
            f.context.cancel(readback).unwrap(),
            RuntimeCancellationV1::Cancelled
        );
    }
    for peer in f.peers.iter_mut().rev() {
        f.context.cancel(peer).unwrap();
    }
    for producer in f.producers.iter_mut().rev() {
        f.context.cancel(producer).unwrap();
    }
}

#[test]
fn compute_peer_gather_full_readback_retains_frame_and_queued_writer_until_actual_reconciliation() {
    for count in [1, 2, 3, 5] {
        for overlap in [false, true] {
            let mut f = Gather::new(count);
            let before = state(&f.context, f.destination);
            let host_before = state(&f.context, f.host);
            for index in 0..count {
                f.enqueue(index, 11 + index as u64 * if overlap { 7 } else { 43 }, 31);
            }
            let event = *f.peer_events.last().unwrap();
            let mut copied = readback(&mut f, &[event]);
            let read_source = f.context.same_device_copies[&copied.id].source;
            let tail = f.peers.last().unwrap().id;
            let root = &f.context.scalar_peer_copies[&tail];
            assert!(!root.covers_input_v1(read_source));
            assert!(root.preserves_destination_frame_v1(read_source));
            assert_eq!(
                f.context.completion_parent_depth_v1(copied.id).unwrap(),
                count + 2
            );
            let marker = f.context.submissions[&copied.id]
                .journal_producer_read
                .unwrap();
            assert_eq!(marker.active_first_for_test().is_some(), count == 1);
            assert_eq!(
                f.context.directed_input_status_v1(copied.id).unwrap(),
                Some(ContextProducerReadStatusV1::Pending)
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(count + 1));
            f.release_events();
            assert!(f.context.events.is_empty());
            assert_eq!(f.context.submissions[&tail].dependency_retains, 1);
            assert!(f.context.release_allocation(f.destination).is_err());
            assert!(f.context.write_allocation(f.destination, 0, &[0]).is_err());
            let pending = state(&f.context, f.destination);
            let host_pending = state(&f.context, f.host);
            f.finish_backend();
            f.context
                .backend
                .finish_submission(copied.backend_submission, true);
            f.context.backend.polls.insert(copied.backend_submission, 1);
            // The retained readback result cannot commit while its actual
            // Context producer chain still has unobserved results.
            assert_eq!(f.context.poll(&mut copied).unwrap(), RuntimePollV1::Pending);
            assert_eq!(state(&f.context, f.destination), pending);
            assert_eq!(state(&f.context, f.host), host_pending);
            for _ in 0..count * 4 + 8 {
                if f.context.poll(&mut copied).unwrap() == RuntimePollV1::Succeeded {
                    break;
                }
            }
            assert_eq!(
                f.context.query_submission(&copied).unwrap(),
                RuntimeCompletionStatusV1::Succeeded
            );
            for id in f
                .producers
                .iter()
                .map(|value| value.id)
                .chain(f.peers.iter().map(|value| value.id))
                .chain([copied.id])
            {
                let record = f.context.submissions[&id];
                assert_eq!(record.status, RuntimeCompletionStatusV1::Succeeded);
                assert!(record.quiescent);
                assert_eq!(record.dependency_retains, 0);
            }
            let mut actual = vec![0; 257];
            f.context.read_allocation(f.host, 0, &mut actual).unwrap();
            assert_eq!(actual, f.expected);
            f.context
                .read_allocation(f.destination, 0, &mut actual)
                .unwrap();
            assert_eq!(actual, f.expected);
            assert_eq!(
                state(&f.context, f.destination).content_lineage,
                before.content_lineage + count as u64
            );
            assert_eq!(
                state(&f.context, f.host).content_lineage,
                host_before.content_lineage + 1
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
            f.context.release_submission(copied).unwrap();
            f.release_results();
            assert!(f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn compute_peer_gather_full_readback_requires_frame_opt_in_and_exact_tail_event() {
    for mode in 0..5 {
        let mut f = Gather::new(2);
        if mode == 0 {
            f.context.backend.ordered_compute_peer = false;
        }
        f.enqueue(0, 11, 31);
        if mode != 0 {
            f.enqueue(1, 61, 31);
        }
        let tail = *f.peer_events.last().unwrap();
        let dependencies = match mode {
            1 => vec![f.peer_events[0]],
            2 => vec![],
            3 => vec![tail, tail],
            _ => vec![tail],
        };
        let source_offset = if mode == 4 { 1 } else { 0 };
        let calls = f.context.backend.copy_call_count;
        let reads = f.context.version_journal_read_records_v1();
        let writers = f.context.version_journal_writer_records_v1();
        validation(
            f.context.copy_async(
                f.readback_stream,
                span(f.destination, RuntimeAccessV1::Read, source_offset, 257),
                span(f.host, RuntimeAccessV1::Write, 0, 257),
                &dependencies,
            ),
            match mode {
                3 => RuntimeValidationErrorV1::DuplicateDependency,
                4 => RuntimeValidationErrorV1::InvalidRange,
                _ => RuntimeValidationErrorV1::ContextReserved,
            },
        );
        assert_eq!(f.context.backend.copy_call_count, calls);
        assert_eq!(f.context.version_journal_read_records_v1(), reads);
        assert_eq!(f.context.version_journal_writer_records_v1(), writers);
        cancel(&mut f, None);
        f.release_results();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_gather_readback_cancellation_and_rejection_refund_only_its_queued_lease() {
    for reject in [false, true] {
        let mut f = Gather::new(3);
        for index in 0..3 {
            f.enqueue(index, 11 + index as u64 * 43, 31);
        }
        let event = *f.peer_events.last().unwrap();
        let before = state(&f.context, f.destination);
        let host_before = state(&f.context, f.host);
        if reject {
            f.context.backend.copy_failure = MockMemoryFailure::Rejected;
            assert!(matches!(
                f.context.copy_async(
                    f.readback_stream,
                    span(f.destination, RuntimeAccessV1::Read, 0, 257),
                    span(f.host, RuntimeAccessV1::Write, 0, 257),
                    &[event],
                ),
                Err(RuntimeErrorV1::BackendRejected(_))
            ));
        } else {
            let mut copied = readback(&mut f, &[event]);
            f.context.backend.cancel_before_publication = true;
            assert_eq!(
                f.context.cancel(&mut copied).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            f.context.release_submission(copied).unwrap();
        }
        assert_eq!(state(&f.context, f.destination), before);
        assert_eq!(
            state(&f.context, f.host).content_lineage,
            host_before.content_lineage
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(3));
        assert_eq!(f.context.submissions[&f.peers[2].id].dependency_retains, 0);
        assert_eq!(
            f.context.query_submission(&f.peers[2]).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
        cancel(&mut f, None);
        f.release_results();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn compute_peer_gather_readback_queued_reference_and_active_epoch_drift_fail_before_poll() {
    for queued in [false, true] {
        for corruption in 0..4 {
            let mut f = Gather::new(if queued { 2 } else { 1 });
            for index in 0..f.producers.len() {
                f.enqueue(index, 11 + index as u64 * 43, 31);
            }
            let event = *f.peer_events.last().unwrap();
            let mut copied = readback(&mut f, &[event]);
            f.release_events();
            let versions = f.context.versions.as_mut().unwrap();
            if queued {
                versions.corrupt_queued_read_for_test_v1(copied.id, corruption);
            } else {
                versions.corrupt_active_input_request_for_test_v1(copied.id, corruption);
            }
            let polls = f.context.backend.poll_call_count;
            let lineage = state(&f.context, f.host).content_lineage;
            validation(
                f.context.poll(&mut copied),
                RuntimeValidationErrorV1::InvalidBackendDescription,
            );
            assert_eq!(f.context.backend.poll_call_count, polls);
            assert_eq!(state(&f.context, f.host).content_lineage, lineage);
            assert!(f.context.is_terminal());
            assert_eq!(
                f.context.submissions[&f.peers.last().unwrap().id].dependency_retains,
                1
            );
            assert!(!f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn compute_peer_gather_cancelled_predecessor_cannot_authorize_full_readback_success() {
    let mut f = Gather::new(3);
    for index in 0..3 {
        f.enqueue(index, 11 + index as u64 * 43, 31);
    }
    let event = *f.peer_events.last().unwrap();
    let mut copied = readback(&mut f, &[event]);
    f.release_events();
    let destination_before = state(&f.context, f.destination).content_lineage;
    let host_before = state(&f.context, f.host).content_lineage;
    f.context.backend.cancel_before_publication = true;
    f.context.cancel(&mut f.peers[0]).unwrap();
    for peer in &mut f.peers[1..] {
        f.context
            .backend
            .producer_launch
            .observations
            .insert(peer.backend_submission, Observation::Failed);
        assert_eq!(
            f.context.poll(peer).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
    }
    f.context
        .backend
        .producer_launch
        .observations
        .insert(copied.backend_submission, Observation::Failed);
    assert_eq!(
        f.context.poll(&mut copied).unwrap(),
        RuntimePollV1::Failed { code: 7 }
    );
    assert_eq!(
        state(&f.context, f.destination).content_lineage,
        destination_before
    );
    assert_eq!(state(&f.context, f.host).content_lineage, host_before);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(!f.context.is_terminal());
    for producer in &mut f.producers {
        f.context.cancel(producer).unwrap();
    }
    f.context.release_submission(copied).unwrap();
    f.release_results();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn compute_peer_gather_descended_unknown_or_terminal_preserves_no_success_claim() {
    for fault in [
        Observation::Quiescent,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Gather::new(2);
        for index in 0..2 {
            f.enqueue(index, 11 + index as u64 * 43, 31);
        }
        let event = *f.peer_events.last().unwrap();
        let mut copied = readback(&mut f, &[event]);
        f.release_events();
        let before = state(&f.context, f.host).content_lineage;
        f.finish_backend();
        f.context
            .backend
            .finish_submission(copied.backend_submission, true);
        f.context.backend.polls.insert(copied.backend_submission, 1);
        f.context
            .backend
            .producer_launch
            .observations
            .insert(f.peers[0].backend_submission, fault);
        let mut reached = false;
        for _ in 0..16 {
            let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut copied)));
            match (fault, result) {
                (_, Ok(Ok(RuntimePollV1::Pending))) => continue,
                (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                    assert_eq!(
                        f.context.query_submission(&copied).unwrap(),
                        RuntimeCompletionStatusV1::QuiescentWithoutResult
                    );
                    assert_eq!(
                        writer_state(&f.context, f.host),
                        ContextWriterStateV1::Unknown { member_count: 1 }
                    );
                    assert!(!f.context.is_terminal());
                }
                (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
                | (Observation::Panic, Err(_)) => {
                    assert!(f.context.is_terminal());
                    assert_eq!(f.context.submissions[&f.peers[1].id].dependency_retains, 1);
                    assert!(!f.context.cleanup().is_complete());
                }
                _ => panic!("descended gather uncertainty lost its classification"),
            }
            reached = true;
            break;
        }
        assert!(reached);
        assert_eq!(state(&f.context, f.host).content_lineage, before);
        assert_ne!(
            f.context.submissions[&copied.id].status,
            RuntimeCompletionStatusV1::Succeeded
        );
    }
}
