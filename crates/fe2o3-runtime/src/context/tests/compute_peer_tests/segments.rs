//! Scripted Context provenance tests, not native DMA or kernel-arithmetic evidence.

use super::*;

#[path = "segments/settled.rs"]
mod settled;

#[path = "segments/ordered.rs"]
mod ordered;

type List = RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>;
type Calls = Arc<Mutex<Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>>>;

fn descriptors() -> Vec<RuntimePeerCopySegmentV1> {
    vec![
        RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 0,
            byte_len: 12,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 13,
            destination_offset: 7,
            byte_len: 9,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 2,
            destination_offset: 31,
            byte_len: 4,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 13,
            destination_offset: 7,
            byte_len: 9,
        },
    ]
}

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.context.backend.pending_compute_segments = true;
    f
}

fn submit(
    f: &mut Fixture,
    segments: &[RuntimePeerCopySegmentV1],
    events: &[RuntimeEventIdV1],
) -> Result<List, RuntimeErrorV1<MockError>> {
    f.context.peer_copy_segments(
        f.peer_stream,
        span(f.source, RuntimeAccessV1::Read, 3, 32),
        span(f.destination, RuntimeAccessV1::Write, 8, 40),
        segments,
        events,
    )
}

fn list(f: &mut Fixture) -> List {
    let event = f.event;
    submit(f, &descriptors(), &[event]).unwrap()
}

fn frame(f: &Fixture) -> Vec<u8> {
    let byte = (f.producer.backend_submission as u8).wrapping_add(37);
    let mut expected = vec![5; 64];
    for segment in descriptors() {
        let start = 8 + segment.destination_offset as usize;
        expected[start..start + segment.byte_len as usize].fill(byte);
    }
    expected
}

fn observe<M>(f: &mut Fixture, submission: &mut RuntimeSubmissionV1<M>) {
    for _ in 0..10 {
        if f.context.poll(submission).unwrap() == RuntimePollV1::Succeeded {
            return;
        }
    }
    panic!("bounded logical chain did not reconcile");
}

fn callback<M>(context: &mut Context, submission: &RuntimeSubmissionV1<M>, calls: &Calls) {
    let calls = calls.clone();
    let id = submission.id;
    context
        .on_completion(submission, move |status| {
            calls.lock().unwrap().push((id, status))
        })
        .unwrap();
}

#[test]
fn segments_keep_exact_writer_list_snapshot_and_full_readback_frame() {
    for backend_already_done in [false, true] {
        let mut f = fixture();
        if backend_already_done {
            f.finish_backend(f.producer.backend_submission);
        }
        let event = f.event;
        let mut supplied = descriptors();
        let peer = submit(&mut f, &supplied, &[event]).unwrap();
        let identity = f.context.segmented_peer_copies[&peer.id].identity;
        supplied.reverse();
        supplied[0].byte_len = u64::MAX;
        assert_eq!(f.context.segmented_peer_copies[&peer.id].identity, identity);
        assert!(!f.context.submissions[&peer.id].scalar_peer_copy);
        assert_eq!(f.context.segmented_peer_copies[&peer.id].state.depth, 2);
        let event = f.context.record_event(&peer).unwrap();
        let mut readback = f
            .context
            .copy_async(
                f.readback_stream,
                span(f.destination, RuntimeAccessV1::Read, 0, 64),
                span(f.host, RuntimeAccessV1::Write, 0, 64),
                &[event],
            )
            .unwrap();
        assert_eq!(f.context.same_device_copies[&readback.id].state.depth, 3);
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        callback(&mut f.context, &f.producer, &callbacks);
        callback(&mut f.context, &peer, &callbacks);
        callback(&mut f.context, &readback, &callbacks);
        f.context.release_event(f.event).unwrap();
        f.context.release_event(event).unwrap();
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
        assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
        let before = state(&f.context, f.destination);
        f.finish_backend(f.producer.backend_submission);
        f.finish_backend(peer.backend_submission);
        f.finish_backend(readback.backend_submission);
        assert_eq!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(state(&f.context, f.destination), before);
        assert!(callbacks.lock().unwrap().is_empty());
        observe(&mut f, &mut readback);
        assert_eq!(
            f.context.query_submission(&f.producer).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(
            f.context.query_submission(&peer).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(
            f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
            frame(&f)
        );
        assert_eq!(callbacks.lock().unwrap().len(), 3);
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        f.context.destroy_stream(f.peer_stream).unwrap();
        assert!(
            f.context
                .validate_segmented_peer_custody_v1(peer.id)
                .is_ok()
        );
        f.context.release_submission(readback).unwrap();
        f.context.release_submission(peer).unwrap();
        f.context.release_submission(f.producer).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn segments_feed_full_frame_read_only_compute_aliases_with_final_only_observation() {
    let mut f = fixture();
    let second = f.context.devices()[1].id();
    let output = f
        .context
        .allocate(second, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    f.context.write_allocation(output, 0, &[17; 64]).unwrap();
    let module = f.context.load_module(second, b"segment-consumer").unwrap();
    let kernel = f
        .context
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    let peer = list(&mut f);
    let event = f.context.record_event(&peer).unwrap();
    let mut consumer = f
        .context
        .launch_producer_aware_v1(
            f.peer_stream,
            &kernel,
            &MixedArguments(vec![
                span(f.destination, RuntimeAccessV1::Read, 0, 64),
                span(f.destination, RuntimeAccessV1::Read, 49, 8),
                span(output, RuntimeAccessV1::Write, 0, 64),
            ]),
            geometry(),
            &[event],
        )
        .unwrap();
    assert_eq!(f.context.producer_launches[&consumer.id].state.depth, 3);
    f.context.release_event(f.event).unwrap();
    f.context.release_event(event).unwrap();
    f.finish_backend(f.producer.backend_submission);
    f.finish_backend(peer.backend_submission);
    f.finish_backend(consumer.backend_submission);
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    observe(&mut f, &mut consumer);
    let observed: Vec<_> = f
        .context
        .backend
        .observed_kernel_reads
        .iter()
        .filter(|read| read.submission == consumer.backend_submission)
        .collect();
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[0].bytes, frame(&f));
    assert_eq!(observed[1].bytes, vec![5; 8]);
    f.context.release_submission(consumer).unwrap();
    f.context.release_submission(peer).unwrap();
    f.context.release_submission(f.producer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn segments_require_opt_in_exact_full_writer_and_event_before_backend() {
    for mode in 0..6 {
        let mut f = if mode >= 3 {
            Fixture::with_bindings(|source| match mode {
                3 => vec![span(source, RuntimeAccessV1::Write, 0, 32)],
                4 => vec![span(source, RuntimeAccessV1::ReadWrite, 0, 64)],
                _ => vec![
                    span(source, RuntimeAccessV1::Write, 0, 64),
                    span(source, RuntimeAccessV1::Read, 0, 64),
                ],
            })
        } else {
            fixture()
        };
        f.context.backend.pending_compute_segments = mode != 0;
        let events = match mode {
            1 => vec![],
            2 => vec![f.event, f.event],
            _ => vec![f.event],
        };
        let before = f.context.backend.copy_call_count;
        assert!(submit(&mut f, &descriptors(), &events).is_err());
        assert_eq!(f.context.backend.copy_call_count, before);
        assert!(f.context.segmented_peer_copies.is_empty());
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
        assert!(!f.context.is_terminal());
        f.cancel_producer();
    }
}

#[test]
fn segments_reject_invalid_complete_roster_and_writable_frame_alias_before_effect() {
    let mut f = fixture();
    for mode in 0..4 {
        let mut segments = descriptors();
        match mode {
            0 => segments.clear(),
            1 => segments[3].byte_len = 0,
            2 => segments[3].source_offset = 31,
            _ => segments[3].destination_offset = u64::MAX,
        }
        let event = f.event;
        assert!(submit(&mut f, &segments, &[event]).is_err());
        assert_eq!(f.context.backend.copy_call_count, 0);
    }
    let second = f.context.devices()[1].id();
    let module = f
        .context
        .load_module(second, b"invalid-frame-alias")
        .unwrap();
    let kernel = f
        .context
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    let mut peer = list(&mut f);
    let event = f.context.record_event(&peer).unwrap();
    for access in [RuntimeAccessV1::Write, RuntimeAccessV1::ReadWrite] {
        let before = f.context.backend.submit_count;
        assert!(
            f.context
                .launch_producer_aware_v1(
                    f.peer_stream,
                    &kernel,
                    &MixedArguments(vec![
                        span(f.destination, RuntimeAccessV1::Read, 0, 64),
                        span(f.destination, access, 50, 4),
                    ]),
                    geometry(),
                    &[event]
                )
                .is_err()
        );
        assert_eq!(f.context.backend.submit_count, before);
    }
    f.finish_backend(f.producer.backend_submission);
    f.finish_backend(peer.backend_submission);
    observe(&mut f, &mut peer);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn segments_refund_cancellation_and_rejection_but_retain_terminal_admission() {
    let mut f = fixture();
    let before = state(&f.context, f.destination);
    let mut peer = list(&mut f);
    f.context.release_event(f.event).unwrap();
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(
        state(&f.context, f.destination).content_lineage,
        before.content_lineage
    );
    assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
    f.context.release_submission(peer).unwrap();
    f.cancel_producer();
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = fixture();
        f.context.backend.copy_failure = failure;
        let event = f.event;
        let result = catch_unwind(AssertUnwindSafe(|| {
            submit(&mut f, &descriptors(), &[event])
        }));
        match failure {
            MockMemoryFailure::Rejected | MockMemoryFailure::Quiescent => {
                assert!(matches!(result, Ok(Err(_))));
                assert!(f.context.segmented_peer_copies.is_empty());
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
                assert!(!f.context.is_terminal());
                f.cancel_producer();
            }
            _ => {
                assert!(result.is_err() || matches!(result, Ok(Err(_))));
                assert!(f.context.is_terminal());
                assert_eq!(f.context.segmented_peer_copies.len(), 1);
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
                assert!(!f.context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn segments_failed_parent_retains_identity_without_promoting_frame() {
    for cancelled in [false, true] {
        let mut f = fixture();
        let mut peer = list(&mut f);
        let before = state(&f.context, f.destination).content_lineage;
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
        assert!(
            f.context
                .validate_segmented_peer_custody_v1(peer.id)
                .is_ok()
        );
        f.context
            .backend
            .producer_launch
            .observations
            .insert(peer.backend_submission, Observation::Failed);
        assert_eq!(
            f.context.poll(&mut peer).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(state(&f.context, f.destination).content_lineage, before);
        assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
        assert!(!f.context.is_terminal());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn segments_retained_success_does_not_hide_descended_producer_fault() {
    for fault in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = fixture();
        let mut peer = list(&mut f);
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        callback(&mut f.context, &f.producer, &callbacks);
        callback(&mut f.context, &peer, &callbacks);
        f.context.release_event(f.event).unwrap();
        f.finish_backend(f.producer.backend_submission);
        f.finish_backend(peer.backend_submission);
        assert_eq!(f.context.poll(&mut peer).unwrap(), RuntimePollV1::Pending);
        let before = state(&f.context, f.destination).content_lineage;
        f.context
            .backend
            .producer_launch
            .observations
            .insert(f.producer.backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut peer)));
        assert_eq!(state(&f.context, f.destination).content_lineage, before);
        assert!(f.context.segmented_peer_copies.contains_key(&peer.id));
        match (fault, result) {
            (Observation::Quiescent, Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))) => {
                // Both physical operations are quiescent, but discarding the
                // source result cannot certify either output's logical success.
                for id in [f.producer.id, peer.id] {
                    assert_eq!(
                        f.context.submissions[&id].status,
                        RuntimeCompletionStatusV1::QuiescentWithoutResult
                    );
                    assert!(f.context.submissions[&id].quiescent);
                }
                assert_eq!(
                    *callbacks.lock().unwrap(),
                    [
                        (
                            f.producer.id,
                            RuntimeCompletionStatusV1::QuiescentWithoutResult
                        ),
                        (peer.id, RuntimeCompletionStatusV1::QuiescentWithoutResult),
                    ]
                );
                for allocation in [f.source, f.destination] {
                    assert_eq!(
                        writer_state(&f.context, allocation),
                        ContextWriterStateV1::Unknown { member_count: 1 }
                    );
                }
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 0);
                assert!(!f.context.segmented_peer_copies[&peer.id].dependencies_held);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
                assert!(!f.context.is_terminal());
                f.context.release_submission(peer).unwrap();
                f.context.release_submission(f.producer).unwrap();
                assert!(f.context.cleanup().is_complete());
            }
            (Observation::Rejected, Ok(Err(RuntimeErrorV1::BackendRejected(_))))
            | (Observation::Terminal, Ok(Err(RuntimeErrorV1::BackendTerminal(_))))
            | (Observation::Panic, Err(_)) => {
                assert!(f.context.is_terminal());
                for id in [f.producer.id, peer.id] {
                    assert_eq!(
                        f.context.submissions[&id].status,
                        RuntimeCompletionStatusV1::Pending
                    );
                    assert!(!f.context.submissions[&id].quiescent);
                }
                assert!(callbacks.lock().unwrap().is_empty());
                assert_eq!(f.context.completion_callback_count, 2);
                assert_eq!(f.context.submissions[&f.producer.id].dependency_retains, 1);
                assert!(f.context.segmented_peer_copies[&peer.id].dependencies_held);
                assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
                assert!(!f.context.cleanup().is_complete());
            }
            _ => panic!("descended segment producer fault lost its classification"),
        }
    }
}

#[test]
fn segments_reject_pending_destination_and_cancelled_list_never_promotes_readback() {
    let mut f = fixture();
    let mut peer = list(&mut f);
    let before = f.context.backend.copy_call_count;
    let source_event = f.event;
    validation(
        submit(&mut f, &descriptors(), &[source_event]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(f.context.backend.copy_call_count, before);
    assert_eq!(f.context.segmented_peer_copies.len(), 1);
    let event = f.context.record_event(&peer).unwrap();
    let mut readback = f
        .context
        .copy_async(
            f.readback_stream,
            span(f.destination, RuntimeAccessV1::Read, 0, 64),
            span(f.host, RuntimeAccessV1::Write, 0, 64),
            &[event],
        )
        .unwrap();
    f.context.release_event(event).unwrap();
    f.context.release_event(f.event).unwrap();
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    f.context
        .backend
        .producer_launch
        .observations
        .insert(readback.backend_submission, Observation::Failed);
    assert_eq!(
        f.context.poll(&mut readback).unwrap(),
        RuntimePollV1::Failed { code: 7 }
    );
    assert_eq!(
        f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
        vec![7; 64]
    );
    assert_eq!(
        f.context.query_submission(&f.producer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
    f.context.release_submission(readback).unwrap();
    f.context.release_submission(peer).unwrap();
    f.cancel_producer();
}

#[test]
fn segments_backend_completion_keeps_logical_source_reserved_and_stale_event_is_only_control() {
    let mut f = fixture();
    f.finish_backend(f.producer.backend_submission);
    assert_eq!(
        f.context.query_submission(&f.producer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    validation(
        f.context.write_allocation(f.source, 0, &[99; 64]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    let mut peer = list(&mut f);
    validation(
        f.context.write_allocation(f.source, 0, &[99; 64]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    f.finish_backend(peer.backend_submission);
    observe(&mut f, &mut peer);
    assert_eq!(
        f.context.query_submission(&f.producer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    f.context.release_submission(peer).unwrap();
    f.context.write_allocation(f.source, 0, &[99; 64]).unwrap();

    // A completed event remains usable as an ordinary control, never as the
    // current writer of rewritten bytes or as pending segmented frame custody.
    let stale_event = f.event;
    let mut legacy = submit(&mut f, &descriptors(), &[stale_event]).unwrap();
    assert!(!f.context.submissions[&legacy.id].segmented_peer_copy);
    assert!(f.context.segmented_peer_copies.is_empty());
    let event = f.context.record_event(&legacy).unwrap();
    validation(
        f.context.copy_async(
            f.readback_stream,
            span(f.destination, RuntimeAccessV1::Read, 0, 64),
            span(f.host, RuntimeAccessV1::Write, 0, 64),
            &[event],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    f.finish_backend(legacy.backend_submission);
    observe(&mut f, &mut legacy);
    let mut expected = frame(&f);
    for segment in descriptors() {
        let start = 8 + segment.destination_offset as usize;
        expected[start..start + segment.byte_len as usize].fill(99);
    }
    assert_eq!(
        f.context.backend.memory[&f.context.allocations[&f.destination].backend_allocation],
        expected
    );
    f.context.release_event(event).unwrap();
    f.context.release_event(f.event).unwrap();
    f.context.release_submission(legacy).unwrap();
    f.context.release_submission(f.producer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn segments_validate_immutable_identity_rank_and_nested_profile_before_observation() {
    for mode in 0..8 {
        let mut f = fixture();
        let peer = list(&mut f);
        let event = f.context.record_event(&peer).unwrap();
        let mut readback = f
            .context
            .copy_async(
                f.readback_stream,
                span(f.destination, RuntimeAccessV1::Read, 0, 64),
                span(f.host, RuntimeAccessV1::Write, 0, 64),
                &[event],
            )
            .unwrap();
        match mode {
            0 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .source
                    .region
                    .byte_offset += 1
            }
            1 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .destination
                    .region
                    .byte_len -= 1
            }
            2 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .state
                    .depth = 1
            }
            3 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .dependencies[0]
                    .submission = peer.id
            }
            4 => {
                f.context.segmented_peer_copies.remove(&peer.id);
            }
            5 => {
                let root = f.context.segmented_peer_copies.get_mut(&peer.id).unwrap();
                let mut producer = root.compute_producer_v1().unwrap();
                producer.backend_submission += 1;
                root.origin = peer_segments::SegmentedPeerSourceV1::Compute(producer);
            }
            6 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .backend_stream += 1;
            }
            _ => {
                f.context
                    .streams
                    .get_mut(&f.peer_stream)
                    .unwrap()
                    .backend_stream += 1;
            }
        }
        let before = f
            .context
            .backend
            .producer_launch_observation_count_for_test_v1();
        validation(
            f.context.poll(&mut readback),
            RuntimeValidationErrorV1::InvalidBackendDescription,
        );
        assert_eq!(
            f.context
                .backend
                .producer_launch_observation_count_for_test_v1(),
            before
        );
        assert!(f.context.is_terminal());
        assert!(!f.context.cleanup().is_complete());
    }
}
