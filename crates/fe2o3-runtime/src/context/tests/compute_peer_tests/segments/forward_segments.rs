//! Pending frame -> ordered list -> full readback, with real mock byte execution.

use super::*;

fn segments() -> Vec<RuntimePeerCopySegmentV1> {
    [(0, 5, 17), (32, 1, 8), (3, 14, 9), (3, 14, 9)]
        .map(
            |(source_offset, destination_offset, byte_len)| RuntimePeerCopySegmentV1 {
                source_offset,
                destination_offset,
                byte_len,
            },
        )
        .to_vec()
}

fn fixture() -> Forward {
    let mut f = Forward::new();
    f.chain.context.backend.pending_segment_frame_peer_copy = false;
    f.chain.context.backend.pending_segment_frame_peer_segments = true;
    f
}

fn submit(f: &mut Forward, events: &[RuntimeEventIdV1]) -> Result<List, RuntimeErrorV1<MockError>> {
    f.chain.context.peer_copy_segments(
        f.stream,
        span(f.chain.destination, RuntimeAccessV1::Read, 1, 47),
        span(f.output, RuntimeAccessV1::Write, 9, 80),
        &segments(),
        events,
    )
}

fn readback(f: &mut Forward, list: &List) -> RuntimeSubmissionV1<RuntimeCopyV1> {
    let event = f.chain.context.record_event(list).unwrap();
    let copy = f
        .chain
        .context
        .copy_async(
            f.readback,
            span(f.output, RuntimeAccessV1::Read, 0, 96),
            span(f.host, RuntimeAccessV1::Write, 4, 96),
            &[event],
        )
        .unwrap();
    f.chain.context.release_event(event).unwrap();
    copy
}

fn expected(frame: &[u8]) -> Vec<u8> {
    let mut output = vec![11; 96];
    for segment in segments() {
        let src = (1 + segment.source_offset) as usize;
        let dst = (9 + segment.destination_offset) as usize;
        let len = segment.byte_len as usize;
        output[dst..dst + len].copy_from_slice(&frame[src..src + len]);
    }
    output
}

#[test]
fn frame_segments_latest_tail_full_readback_reconciles_original_receipts_and_guards() {
    for physical_done in [false, true] {
        let mut f = fixture();
        let (first, second, event) = f.chain.pair();
        let frame = f.chain.expected([8, 16]);
        if physical_done {
            f.chain.finish(first.backend_submission);
            f.chain.finish(second.backend_submission);
        }
        let mut descriptors = segments();
        let list = f
            .chain
            .context
            .peer_copy_segments(
                f.stream,
                span(f.chain.destination, RuntimeAccessV1::Read, 1, 47),
                span(f.output, RuntimeAccessV1::Write, 9, 80),
                &descriptors,
                &[event],
            )
            .unwrap();
        descriptors.fill(RuntimePeerCopySegmentV1 {
            source_offset: u64::MAX,
            destination_offset: u64::MAX,
            byte_len: 0,
        });
        f.chain.context.release_event(event).unwrap();
        let root = &f.chain.context.segmented_peer_copies[&list.id];
        assert!(root.is_frame_source_v1());
        assert!(root.compute_producer_v1().is_none());
        assert_eq!(root.source_dependency_v1().unwrap().submission, second.id);
        assert_eq!(root.state.depth, 3);
        let mut copy = readback(&mut f, &list);
        assert_eq!(f.chain.context.same_device_copies[&copy.id].state.depth, 4);
        let calls = Arc::new(Mutex::new(Vec::new()));
        for submission in [&first, &second, &list] {
            callback(&mut f.chain.context, submission, &calls);
            assert_eq!(
                f.chain.context.submissions[&submission.id].dependency_retains,
                1
            );
        }
        callback(&mut f.chain.context, &copy, &calls);
        assert!(
            f.chain
                .context
                .release_allocation(f.chain.destination)
                .is_err()
        );
        assert!(
            f.chain
                .context
                .write_allocation(f.chain.destination, 0, &[1])
                .is_err()
        );
        let before = state(&f.chain.context, f.output);
        if !physical_done {
            f.chain.finish(first.backend_submission);
            f.chain.finish(second.backend_submission);
        }
        f.chain.finish(list.backend_submission);
        f.chain.finish(copy.backend_submission);
        assert_eq!(
            f.chain.context.poll(&mut copy).unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(state(&f.chain.context, f.output), before);
        assert!(calls.lock().unwrap().is_empty());
        f.chain.context.flush_stream(f.readback).unwrap();
        f.chain.observe(&mut copy);
        let output = expected(&frame);
        let mut host = vec![13; 104];
        host[4..100].copy_from_slice(&output);
        assert_eq!(f.bytes(f.chain.destination), frame);
        assert_eq!(f.bytes(f.output), output);
        assert_eq!(f.bytes(f.host), host);
        assert_eq!(
            *calls.lock().unwrap(),
            [first.id, second.id, list.id, copy.id]
                .map(|id| (id, RuntimeCompletionStatusV1::Succeeded))
        );
        assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
        for submission in [&first, &second, &list] {
            assert_eq!(
                f.chain.context.submissions[&submission.id].dependency_retains,
                0
            );
        }
        f.chain.context.release_submission(copy).unwrap();
        f.chain.context.release_submission(list).unwrap();
        f.chain.context.release_submission(second).unwrap();
        f.chain.context.release_submission(first).unwrap();
        assert!(f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn frame_segments_require_independent_opt_in_latest_event_and_valid_envelopes() {
    assert!(!MockBackend::default().supports_pending_segment_frame_peer_copy_segments_v1());
    for mode in 0..9 {
        let mut f = fixture();
        let (first, mut second, event) = f.chain.pair();
        let stale = f.chain.context.record_event(&first).unwrap();
        let mut source = span(f.chain.destination, RuntimeAccessV1::Read, 1, 47);
        let mut destination = span(f.output, RuntimeAccessV1::Write, 9, 80);
        let mut events = vec![event];
        let mut windows = segments();
        match mode {
            0 => {
                f.chain.context.backend.pending_segment_frame_peer_segments = false;
                f.chain.context.backend.pending_segment_frame_peer_copy = true;
                f.chain.context.backend.pending_compute_segments = true;
            }
            1 => events.clear(),
            2 => events[0] = stale,
            3 => source.access = RuntimeAccessV1::ReadWrite,
            4 => destination.access = RuntimeAccessV1::ReadWrite,
            5 => source.byte_offset = 32,
            6 => destination.byte_offset = 32,
            7 => windows[0].byte_len = u64::MAX,
            8 => destination.allocation = source.allocation,
            _ => unreachable!(),
        }
        let calls = f.chain.context.backend.copy_call_count;
        let readers = f.chain.context.version_journal_read_records_v1();
        let before = state(&f.chain.context, f.output);
        assert!(
            f.chain
                .context
                .peer_copy_segments(f.stream, source, destination, &windows, &events)
                .is_err(),
            "mode {mode}"
        );
        assert_eq!(f.chain.context.backend.copy_call_count, calls);
        assert_eq!(f.chain.context.version_journal_read_records_v1(), readers);
        assert_eq!(state(&f.chain.context, f.output), before);
        assert_eq!(f.chain.context.segmented_peer_copies.len(), 2);
        f.chain.context.release_event(event).unwrap();
        f.chain.context.release_event(stale).unwrap();
        f.chain.finish(first.backend_submission);
        f.chain.finish(second.backend_submission);
        f.chain.observe(&mut second);
        assert!(f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn frame_segments_reject_pending_destination_and_pending_compute_consumer() {
    let mut f = fixture();
    let parent = f.chain.list(0, 8, &[]).unwrap();
    let event = f.chain.context.record_event(&parent).unwrap();
    let mut list = submit(&mut f, &[event]).unwrap();
    let next = f.chain.context.record_event(&list).unwrap();
    let calls = f.chain.context.backend.copy_call_count;
    assert!(submit(&mut f, &[event, next]).is_err());
    assert_eq!(f.chain.context.backend.copy_call_count, calls);
    let device = f.chain.context.devices()[1].id();
    let module = f
        .chain
        .context
        .load_module(device, b"frame-list-consumer")
        .unwrap();
    let kernel = f
        .chain
        .context
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    let next_id = f.chain.context.backend.next;
    let readers = f.chain.context.version_journal_read_records_v1();
    let launches = f.chain.context.producer_launches.len();
    assert!(
        f.chain
            .context
            .launch_producer_aware_v1(
                f.stream,
                &kernel,
                &MixedArguments(vec![span(f.output, RuntimeAccessV1::Read, 0, 96)]),
                geometry(),
                &[next],
            )
            .is_err()
    );
    assert_eq!(f.chain.context.backend.next, next_id);
    assert_eq!(f.chain.context.version_journal_read_records_v1(), readers);
    assert_eq!(f.chain.context.producer_launches.len(), launches);
    f.chain.context.release_event(next).unwrap();
    f.chain.context.release_event(event).unwrap();
    f.chain.finish(parent.backend_submission);
    f.chain.finish(list.backend_submission);
    f.chain.observe(&mut list);
    assert!(f.chain.context.cleanup().is_complete());
}

#[test]
fn frame_segments_cancel_and_failed_parent_never_promote_output_lineage() {
    for mode in 0..3 {
        let mut f = fixture();
        let mut parent = f.chain.list(0, 8, &[]).unwrap();
        let event = f.chain.context.record_event(&parent).unwrap();
        let mut list = submit(&mut f, &[event]).unwrap();
        f.chain.context.release_event(event).unwrap();
        let lineage = state(&f.chain.context, f.output).content_lineage;
        f.chain.context.backend.cancel_before_publication = true;
        if mode == 0 {
            assert_eq!(
                f.chain.context.cancel(&mut list).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            assert_eq!(
                f.chain.context.query_submission(&parent).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            f.chain.finish(parent.backend_submission);
            f.chain.observe(&mut parent);
        } else {
            if mode == 1 {
                assert_eq!(
                    f.chain.context.cancel(&mut parent).unwrap(),
                    RuntimeCancellationV1::Cancelled
                );
            } else {
                f.chain
                    .context
                    .backend
                    .producer_launch
                    .observations
                    .insert(parent.backend_submission, Observation::Failed);
                assert_eq!(
                    f.chain.context.poll(&mut parent).unwrap(),
                    RuntimePollV1::Failed { code: 7 }
                );
            }
            f.chain
                .context
                .backend
                .producer_launch
                .observations
                .insert(list.backend_submission, Observation::Failed);
            assert_eq!(
                f.chain.context.poll(&mut list).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            );
        }
        assert_eq!(f.bytes(f.output), &[11; 96]);
        assert_eq!(state(&f.chain.context, f.output).content_lineage, lineage);
        assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(
            f.chain.context.submissions[&parent.id].dependency_retains,
            0
        );
        assert!(!f.chain.context.is_terminal());
        assert!(f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn frame_segments_plan_rank_and_queued_reader_drift_fail_before_poll() {
    for mode in 0..5 {
        let mut f = fixture();
        let (_, parent, event) = f.chain.pair();
        let mut list = submit(&mut f, &[event]).unwrap();
        match mode {
            0 => f
                .chain
                .context
                .segmented_peer_copies
                .get_mut(&parent.id)
                .unwrap()
                .replace_plan_identity_for_test_v1(),
            1 => {
                f.chain
                    .context
                    .segmented_peer_copies
                    .get_mut(&list.id)
                    .unwrap()
                    .state
                    .depth = 1
            }
            2 => {
                f.chain
                    .context
                    .segmented_peer_copies
                    .get_mut(&list.id)
                    .unwrap()
                    .source
                    .region
                    .byte_offset += 1
            }
            3 => f
                .chain
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_queued_read_for_test_v1(list.id, 0),
            4 => f.chain.context.backend.pending_segment_frame_peer_segments = false,
            _ => unreachable!(),
        }
        let before = f.chain.context.backend.poll_call_count;
        assert!(f.chain.context.poll(&mut list).is_err(), "mode {mode}");
        assert_eq!(f.chain.context.backend.poll_call_count, before);
        assert_eq!(f.bytes(f.output), &[11; 96]);
    }
}

#[test]
fn frame_segments_without_journal_reject_matching_pending_source_only() {
    let mut context = Context::open(MockBackend {
        next: 100,
        third_device: true,
        deferred_copies: true,
        peer_segments_frame: true,
        pending_segment_frame_peer_segments: true,
        ..MockBackend::default()
    })
    .unwrap();
    let devices = [
        context.devices()[0].id(),
        context.devices()[1].id(),
        context.devices()[2].id(),
    ];
    let [source, frame, target] = devices.map(|device| {
        context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap()
    });
    let other = context
        .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    for (allocation, byte) in [(source, 23), (frame, 5), (target, 11), (other, 17)] {
        context
            .write_allocation(allocation, 0, &[byte; 64])
            .unwrap();
    }
    let first_stream = context.create_stream(devices[1]).unwrap();
    let stream = context.create_stream(devices[2]).unwrap();
    let mut first = context
        .peer_copy_segments(
            first_stream,
            span(source, RuntimeAccessV1::Read, 0, 64),
            span(frame, RuntimeAccessV1::Write, 0, 64),
            &segments(),
            &[],
        )
        .unwrap();
    let event = context.record_event(&first).unwrap();
    let calls = context.backend.copy_call_count;
    validation(
        context.peer_copy_segments(
            stream,
            span(frame, RuntimeAccessV1::Read, 0, 64),
            span(target, RuntimeAccessV1::Write, 0, 64),
            &segments(),
            &[event],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(context.backend.copy_call_count, calls);
    let mut control = context
        .peer_copy_segments(
            stream,
            span(other, RuntimeAccessV1::Read, 0, 64),
            span(target, RuntimeAccessV1::Write, 0, 64),
            &segments(),
            &[event],
        )
        .unwrap();
    for submission in [&mut first, &mut control] {
        context
            .backend
            .finish_submission(submission.backend_submission, true);
        context
            .backend
            .polls
            .insert(submission.backend_submission, 1);
        assert_eq!(context.poll(submission).unwrap(), RuntimePollV1::Succeeded);
    }
    let mut settled = context
        .peer_copy_segments(
            stream,
            span(frame, RuntimeAccessV1::Read, 0, 64),
            span(target, RuntimeAccessV1::Write, 0, 64),
            &segments(),
            &[event],
        )
        .unwrap();
    context
        .backend
        .finish_submission(settled.backend_submission, true);
    context.backend.polls.insert(settled.backend_submission, 1);
    assert_eq!(
        context.poll(&mut settled).unwrap(),
        RuntimePollV1::Succeeded
    );
    context.release_event(event).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn frame_segments_source_ancestry_validation_visits_each_control_once() {
    let mut f = Forward::new();
    f.chain.context.backend.pending_segment_frame_peer_segments = true;
    let parent = f.chain.list(0, 8, &[]).unwrap();
    let event = f.chain.context.record_event(&parent).unwrap();
    let mut sentinel = f.peer(&[event], 0, 64).unwrap();
    f.chain.context.release_event(event).unwrap();
    f.chain.finish(parent.backend_submission);
    f.chain.finish(sentinel.backend_submission);
    f.chain.observe(&mut sentinel);
    let control = f.chain.context.record_event(&sentinel).unwrap();
    let mut source = f.chain.sources[0];
    let source_bytes = f.bytes(source).to_vec();
    let mut previous = None;
    let mut lists = Vec::new();
    for hop in 0..16 {
        let device = f.chain.context.devices()[(hop + 1) % 2].id();
        let target = f
            .chain
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        f.chain
            .context
            .write_allocation(target, 0, &[29; 64])
            .unwrap();
        let stream = f.chain.context.create_stream(device).unwrap();
        let mut events = vec![control];
        events.extend(previous);
        let list = f
            .chain
            .context
            .peer_copy_segments(
                stream,
                span(source, RuntimeAccessV1::Read, 0, 64),
                span(target, RuntimeAccessV1::Write, 0, 64),
                &[RuntimePeerCopySegmentV1 {
                    source_offset: 0,
                    destination_offset: 0,
                    byte_len: 64,
                }],
                &events,
            )
            .unwrap();
        if let Some(event) = previous {
            f.chain.context.release_event(event).unwrap();
        }
        previous = Some(f.chain.context.record_event(&list).unwrap());
        lists.push(list);
        source = target;
    }
    let tail = lists.last().unwrap().id;
    let (result, visits) = f
        .chain
        .context
        .count_compute_peer_validations_for_test_v1(|context| {
            context.validate_segmented_peer_custody_v1(tail)
        });
    result.unwrap();
    assert_eq!(visits, 16);
    for list in &lists {
        f.chain.finish(list.backend_submission);
    }
    f.chain.observe(lists.last_mut().unwrap());
    assert_eq!(f.bytes(source), source_bytes);
    f.chain.context.release_event(previous.unwrap()).unwrap();
    f.chain.context.release_event(control).unwrap();
    for list in lists.into_iter().rev() {
        f.chain.context.release_submission(list).unwrap();
    }
    assert!(f.chain.context.cleanup().is_complete());
}

#[test]
fn frame_segments_rejected_submission_refunds_queued_reader_and_latest_parent_retain() {
    let mut f = fixture();
    let (first, mut second, event) = f.chain.pair();
    let readers = f.chain.context.version_journal_read_records_v1();
    let lineage = state(&f.chain.context, f.output).content_lineage;
    f.chain.context.backend.copy_failure = MockMemoryFailure::Rejected;
    assert!(matches!(
        submit(&mut f, &[event]),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert!(!f.chain.context.is_terminal());
    assert_eq!(f.chain.context.segmented_peer_copies.len(), 2);
    assert_eq!(f.chain.context.version_journal_read_records_v1(), readers);
    assert_eq!(f.chain.context.submissions[&first.id].dependency_retains, 1);
    assert_eq!(
        f.chain.context.submissions[&second.id].dependency_retains,
        0
    );
    assert_eq!(state(&f.chain.context, f.output).content_lineage, lineage);
    assert_eq!(f.bytes(f.output), &[11; 96]);
    f.chain.context.release_event(event).unwrap();
    f.chain.finish(first.backend_submission);
    f.chain.finish(second.backend_submission);
    f.chain.observe(&mut second);
    assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
    assert!(f.chain.context.cleanup().is_complete());
}

#[test]
fn frame_segments_unknown_queued_parent_releases_readers_without_promoting_success() {
    let mut f = fixture();
    let (mut first, second, event) = f.chain.pair();
    let mut list = submit(&mut f, &[event]).unwrap();
    f.chain.context.release_event(event).unwrap();
    f.chain.finish(first.backend_submission);
    f.chain.observe(&mut first);
    f.chain.finish(second.backend_submission);
    f.chain.finish(list.backend_submission);
    assert_eq!(
        f.chain.context.poll(&mut list).unwrap(),
        RuntimePollV1::Pending
    );
    let lineage = state(&f.chain.context, f.output).content_lineage;
    f.chain
        .context
        .backend
        .producer_launch
        .observations
        .insert(second.backend_submission, Observation::Quiescent);
    assert!(matches!(
        f.chain.context.poll(&mut list),
        Err(RuntimeErrorV1::BackendQuiescent(_))
    ));
    assert_eq!(
        f.chain.context.query_submission(&list).unwrap(),
        RuntimeCompletionStatusV1::QuiescentWithoutResult
    );
    assert_eq!(state(&f.chain.context, f.output).content_lineage, lineage);
    assert_eq!(
        writer_state(&f.chain.context, f.output),
        ContextWriterStateV1::Unknown { member_count: 1 }
    );
    assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(
        f.chain.context.submissions[&second.id].dependency_retains,
        0
    );
    assert!(!f.chain.context.is_terminal());
    assert!(f.chain.context.cleanup().is_complete());
}
