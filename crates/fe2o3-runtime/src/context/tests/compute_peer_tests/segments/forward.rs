//! Journal/frame custody with real mock copies, not native publication evidence.

use super::*;

struct Forward {
    chain: Chain,
    output: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    stream: RuntimeStreamIdV1,
    readback: RuntimeStreamIdV1,
}

impl Forward {
    fn new() -> Self {
        let mut chain = Chain::new(true);
        chain.context.backend.pending_segment_frame_peer_copy = true;
        let device = chain.context.devices()[1].id();
        let output = chain
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 96, 16)
            .unwrap();
        let host = chain
            .context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 104, 16)
            .unwrap();
        chain
            .context
            .write_allocation(output, 0, &[11; 96])
            .unwrap();
        chain.context.write_allocation(host, 0, &[13; 104]).unwrap();
        let stream = chain.context.create_stream(device).unwrap();
        let readback = chain.context.create_stream(device).unwrap();
        Self {
            chain,
            output,
            host,
            stream,
            readback,
        }
    }

    fn peer(
        &mut self,
        events: &[RuntimeEventIdV1],
        offset: u64,
        bytes: u64,
    ) -> Result<Peer, RuntimeErrorV1<MockError>> {
        self.chain.context.peer_copy(
            self.stream,
            span(self.chain.destination, RuntimeAccessV1::Read, offset, bytes),
            span(self.output, RuntimeAccessV1::Write, 9, bytes),
            events,
        )
    }

    fn readback(&mut self, peer: &Peer) -> RuntimeSubmissionV1<RuntimeCopyV1> {
        let event = self.chain.context.record_event(peer).unwrap();
        let result = self
            .chain
            .context
            .copy_async(
                self.readback,
                span(self.output, RuntimeAccessV1::Read, 0, 96),
                span(self.host, RuntimeAccessV1::Write, 4, 96),
                &[event],
            )
            .unwrap();
        self.chain.context.release_event(event).unwrap();
        result
    }

    fn bytes(&self, allocation: RuntimeAllocationIdV1) -> &[u8] {
        &self.chain.context.backend.memory
            [&self.chain.context.allocations[&allocation].backend_allocation]
    }
}

#[test]
fn segment_frame_peer_latest_tail_and_full_target_readback_reconcile_original_ids() {
    for (offset, bytes) in [(0, 64), (49, 8)] {
        for physical_done in [false, true] {
            let mut f = Forward::new();
            let (first, second, event) = f.chain.pair();
            let frame = f.chain.expected([8, 16]);
            if physical_done {
                f.chain.finish(first.backend_submission);
                f.chain.finish(second.backend_submission);
            }
            let peer = f.peer(&[event], offset, bytes).unwrap();
            f.chain.context.release_event(event).unwrap();
            let input = f.chain.context.scalar_peer_copies[&peer.id]
                .compute
                .as_ref()
                .unwrap();
            assert!(input.is_segmented_frame_v1());
            assert_eq!(input.producer.submission, second.id);
            assert_eq!(input.state.depth, 3);
            assert!(
                f.chain.context.submissions[&peer.id]
                    .journal_producer_read
                    .is_some()
            );
            let mut readback = f.readback(&peer);
            assert_eq!(
                f.chain.context.same_device_copies[&readback.id].state.depth,
                4
            );
            let calls = Arc::new(Mutex::new(Vec::new()));
            callback(&mut f.chain.context, &first, &calls);
            callback(&mut f.chain.context, &second, &calls);
            callback(&mut f.chain.context, &peer, &calls);
            callback(&mut f.chain.context, &readback, &calls);
            assert_eq!(f.chain.context.submissions[&first.id].dependency_retains, 1);
            assert_eq!(
                f.chain.context.submissions[&second.id].dependency_retains,
                1
            );
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
            // Execute the mock's actual ordered descriptors and copies. Only the
            // leaf is then observed; ancestor logical success is never assigned.
            if !physical_done {
                f.chain.finish(first.backend_submission);
                f.chain.finish(second.backend_submission);
            }
            f.chain.finish(peer.backend_submission);
            f.chain.finish(readback.backend_submission);
            assert_eq!(
                f.chain.context.poll(&mut readback).unwrap(),
                RuntimePollV1::Pending
            );
            assert_eq!(state(&f.chain.context, f.output), before);
            assert!(calls.lock().unwrap().is_empty());
            f.chain.context.flush_stream(f.readback).unwrap();
            f.chain.observe(&mut readback);
            assert_eq!(
                f.chain.context.backend.last_flushed_stream,
                Some(f.chain.context.streams[&f.readback].backend_stream)
            );
            let mut output = vec![11; 96];
            output[9..9 + bytes as usize]
                .copy_from_slice(&frame[offset as usize..(offset + bytes) as usize]);
            let mut host = vec![13; 104];
            host[4..100].copy_from_slice(&output);
            assert_eq!(f.bytes(f.chain.destination), frame);
            assert_eq!(f.bytes(f.output), output);
            assert_eq!(f.bytes(f.host), host);
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    (first.id, RuntimeCompletionStatusV1::Succeeded),
                    (second.id, RuntimeCompletionStatusV1::Succeeded),
                    (peer.id, RuntimeCompletionStatusV1::Succeeded),
                    (readback.id, RuntimeCompletionStatusV1::Succeeded),
                ]
            );
            assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
            f.chain.context.release_submission(readback).unwrap();
            f.chain.context.release_submission(peer).unwrap();
            f.chain.context.release_submission(second).unwrap();
            f.chain.context.release_submission(first).unwrap();
            assert!(f.chain.context.cleanup().is_complete());
        }
    }
}

#[test]
fn segment_frame_peer_accepts_compute_backed_parent_without_faking_compute_origin() {
    let mut f = Forward::new();
    f.chain.context.backend.pending_compute_segments = true;
    let device = f.chain.context.devices()[0].id();
    let stream = f.chain.context.create_stream(device).unwrap();
    let module = f
        .chain
        .context
        .load_module(device, b"forward-source")
        .unwrap();
    let kernel = f
        .chain
        .context
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    let producer = f
        .chain
        .context
        .launch_producer_aware_v1(
            stream,
            &kernel,
            &MixedArguments(vec![span(
                f.chain.sources[0],
                RuntimeAccessV1::Write,
                0,
                64,
            )]),
            geometry(),
            &[],
        )
        .unwrap();
    let event = f.chain.context.record_event(&producer).unwrap();
    let list = f.chain.list(0, 8, &[event]).unwrap();
    f.chain.context.release_event(event).unwrap();
    let event = f.chain.context.record_event(&list).unwrap();
    let mut peer = f.peer(&[event], 0, 64).unwrap();
    f.chain.context.release_event(event).unwrap();
    assert_eq!(
        f.chain.context.scalar_peer_copies[&peer.id]
            .compute
            .as_ref()
            .unwrap()
            .state
            .depth,
        3
    );
    assert!(!f.chain.context.producer_launches.contains_key(&list.id));
    f.chain.finish(producer.backend_submission);
    f.chain.finish(list.backend_submission);
    f.chain.finish(peer.backend_submission);
    f.chain.observe(&mut peer);
    let mut frame = vec![5; 64];
    for segment in windows() {
        let start = 8 + segment.destination_offset as usize;
        frame[start..start + segment.byte_len as usize]
            .fill((producer.backend_submission as u8).wrapping_add(37));
    }
    assert_eq!(f.bytes(f.chain.destination), frame);
    assert_eq!(&f.bytes(f.output)[9..73], frame);
    assert_eq!(
        f.chain.context.query_submission(&producer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        f.chain.context.query_submission(&list).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    f.chain.context.release_submission(peer).unwrap();
    f.chain.context.release_submission(list).unwrap();
    f.chain.context.release_submission(producer).unwrap();
    assert!(f.chain.context.cleanup().is_complete());
}

#[test]
fn segment_frame_peer_requires_capability_exact_latest_event_and_checked_read_write_windows() {
    for mode in 0..10 {
        let mut f = Forward::new();
        let (first, second, event) = f.chain.pair();
        let stale = f.chain.context.record_event(&first).unwrap();
        let mut source = span(f.chain.destination, RuntimeAccessV1::Read, 0, 64);
        let mut destination = span(f.output, RuntimeAccessV1::Write, 9, 64);
        let mut events = vec![event];
        match mode {
            0 => f.chain.context.backend.pending_segment_frame_peer_copy = false,
            1 => events.clear(),
            2 => events[0] = stale,
            3 => events[0].context_generation += 1,
            4 => source.access = RuntimeAccessV1::ReadWrite,
            5 => destination.access = RuntimeAccessV1::ReadWrite,
            6 => source.byte_offset = 1,
            7 => destination.byte_offset = 33,
            8 => source.byte_offset = u64::MAX,
            9 => {
                source.byte_len = 0;
                destination.byte_len = 0;
            }
            _ => unreachable!(),
        }
        let calls = f.chain.context.backend.copy_call_count;
        let before = state(&f.chain.context, f.output);
        let readers = f.chain.context.version_journal_read_records_v1();
        assert!(
            f.chain
                .context
                .peer_copy(f.stream, source, destination, &events)
                .is_err(),
            "mode {mode}"
        );
        assert_eq!(f.chain.context.backend.copy_call_count, calls);
        assert_eq!(f.chain.context.version_journal_read_records_v1(), readers);
        assert_eq!(state(&f.chain.context, f.output), before);
        assert!(f.chain.context.scalar_peer_copies.is_empty());
        f.chain.context.release_event(event).unwrap();
        f.chain.context.release_event(stale).unwrap();
        f.chain.finish(first.backend_submission);
        f.chain.finish(second.backend_submission);
        let mut second = second;
        f.chain.observe(&mut second);
        assert!(f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn segment_frame_peer_without_journal_rejects_only_matching_pending_list_source() {
    let mut context = Context::open(MockBackend {
        next: 100,
        third_device: true,
        deferred_copies: true,
        peer_segments_frame: true,
        pending_segment_frame_peer_copy: true,
        ..MockBackend::default()
    })
    .unwrap();
    let devices = [
        context.devices()[0].id(),
        context.devices()[1].id(),
        context.devices()[2].id(),
    ];
    let allocations = devices.map(|device| {
        context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap()
    });
    let [source, frame, target] = allocations;
    for (allocation, byte) in [(source, 23), (frame, 5), (target, 11)] {
        context
            .write_allocation(allocation, 0, &[byte; 64])
            .unwrap();
    }
    let other = context
        .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    context.write_allocation(other, 0, &[17; 64]).unwrap();
    let list_stream = context.create_stream(devices[1]).unwrap();
    let peer_stream = context.create_stream(devices[2]).unwrap();
    let mut list = context
        .peer_copy_segments(
            list_stream,
            span(source, RuntimeAccessV1::Read, 3, 32),
            span(frame, RuntimeAccessV1::Write, 8, 40),
            &windows(),
            &[],
        )
        .unwrap();
    assert!(context.versions.is_none());
    assert!(context.segmented_peer_copies.is_empty());
    assert_eq!(
        context.submissions[&list.id].segmented_destination,
        Some(frame)
    );
    let event = context.record_event(&list).unwrap();
    let calls = context.backend.copy_call_count;
    validation(
        context.peer_copy(
            peer_stream,
            span(frame, RuntimeAccessV1::Read, 0, 64),
            span(target, RuntimeAccessV1::Write, 0, 64),
            &[event],
        ),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(context.backend.copy_call_count, calls);
    assert!(context.scalar_peer_copies.is_empty());
    assert_eq!(
        context.query_submission(&list).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );

    // A pending list used only as a control must not prohibit another source.
    let mut ordinary = context
        .peer_copy(
            peer_stream,
            span(other, RuntimeAccessV1::Read, 0, 64),
            span(target, RuntimeAccessV1::Write, 0, 64),
            &[event],
        )
        .unwrap();
    assert_eq!(
        context.submissions[&ordinary.id].segmented_destination,
        None
    );
    context
        .backend
        .finish_submission(list.backend_submission, true);
    context
        .backend
        .finish_submission(ordinary.backend_submission, true);
    context.backend.polls.insert(list.backend_submission, 1);
    context.backend.polls.insert(ordinary.backend_submission, 1);
    assert_eq!(context.poll(&mut list).unwrap(), RuntimePollV1::Succeeded);
    assert_eq!(
        context.poll(&mut ordinary).unwrap(),
        RuntimePollV1::Succeeded
    );
    assert_eq!(
        context.backend.memory[&context.allocations[&target].backend_allocation],
        [17; 64]
    );
    context.release_submission(ordinary).unwrap();

    let mut settled = context
        .peer_copy(
            peer_stream,
            span(frame, RuntimeAccessV1::Read, 0, 64),
            span(target, RuntimeAccessV1::Write, 0, 64),
            &[event],
        )
        .unwrap();
    context.release_event(event).unwrap();
    context
        .backend
        .finish_submission(settled.backend_submission, true);
    context.backend.polls.insert(settled.backend_submission, 1);
    assert_eq!(
        context.poll(&mut settled).unwrap(),
        RuntimePollV1::Succeeded
    );
    let mut expected = [5; 64];
    for segment in windows() {
        let start = (8 + segment.destination_offset) as usize;
        expected[start..start + segment.byte_len as usize].fill(23);
    }
    assert_eq!(
        context.backend.memory[&context.allocations[&target].backend_allocation],
        expected
    );
    context.release_submission(settled).unwrap();
    context.release_submission(list).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn segment_frame_peer_does_not_authorize_a_pending_destination_writer() {
    let mut f = Forward::new();
    let list = f.chain.list(0, 8, &[]).unwrap();
    let event = f.chain.context.record_event(&list).unwrap();
    let mut first = f.peer(&[event], 0, 32).unwrap();
    let predecessor = f.chain.context.record_event(&first).unwrap();
    let calls = f.chain.context.backend.copy_call_count;
    assert!(f.peer(&[event, predecessor], 16, 32).is_err());
    assert_eq!(f.chain.context.backend.copy_call_count, calls);
    assert_eq!(f.chain.context.scalar_peer_copies.len(), 1);
    f.chain.context.release_event(predecessor).unwrap();
    f.chain.context.release_event(event).unwrap();
    f.chain.finish(list.backend_submission);
    f.chain.finish(first.backend_submission);
    f.chain.observe(&mut first);
    assert!(f.chain.context.cleanup().is_complete());
}

#[test]
fn segment_frame_peer_releases_only_consumer_custody_on_cancel_or_certified_rejection() {
    let mut f = Forward::new();
    let mut list = f.chain.list(0, 8, &[]).unwrap();
    let event = f.chain.context.record_event(&list).unwrap();
    let before = state(&f.chain.context, f.chain.destination);
    let mut peer = f.peer(&[event], 0, 64).unwrap();
    f.chain.context.release_event(event).unwrap();
    f.chain.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.chain.context.cancel(&mut peer).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(state(&f.chain.context, f.chain.destination), before);
    assert_eq!(f.chain.context.submissions[&list.id].dependency_retains, 0);
    assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        f.chain.context.query_submission(&list).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    f.chain.context.release_submission(peer).unwrap();
    f.chain.finish(list.backend_submission);
    f.chain.observe(&mut list);
    assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
    f.chain.context.release_submission(list).unwrap();
    assert!(f.chain.context.cleanup().is_complete());

    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Forward::new();
        let mut list = f.chain.list(0, 8, &[]).unwrap();
        let event = f.chain.context.record_event(&list).unwrap();
        if failure == MockMemoryFailure::Quiescent {
            f.chain.finish(list.backend_submission);
        }
        f.chain.context.backend.copy_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.peer(&[event], 0, 64)));
        assert!(matches!(result, Err(_) | Ok(Err(_))));
        if matches!(
            failure,
            MockMemoryFailure::Rejected | MockMemoryFailure::Quiescent
        ) {
            assert!(!f.chain.context.is_terminal());
            assert!(f.chain.context.scalar_peer_copies.is_empty());
            assert_eq!(f.chain.context.submissions[&list.id].dependency_retains, 0);
            assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(1));
            f.chain.context.release_event(event).unwrap();
            if failure != MockMemoryFailure::Quiescent {
                f.chain.finish(list.backend_submission);
            }
            f.chain.observe(&mut list);
            assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
            f.chain.context.release_submission(list).unwrap();
            assert!(f.chain.context.cleanup().is_complete());
        } else {
            assert!(f.chain.context.is_terminal());
            assert_eq!(f.chain.context.scalar_peer_copies.len(), 1);
            assert_eq!(f.chain.context.submissions[&list.id].dependency_retains, 1);
            assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(2));
            assert!(!f.chain.context.cleanup().is_complete());
        }
    }
}

#[test]
fn segment_frame_peer_failed_or_cancelled_list_does_not_publish_success_or_restore_lineage() {
    for cancelled in [false, true] {
        let mut f = Forward::new();
        let mut list = f.chain.list(0, 8, &[]).unwrap();
        let event = f.chain.context.record_event(&list).unwrap();
        let mut peer = f.peer(&[event], 0, 64).unwrap();
        let before = state(&f.chain.context, f.output).content_lineage;
        if cancelled {
            f.chain.context.backend.cancel_before_publication = true;
            f.chain.context.cancel(&mut list).unwrap();
        } else {
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
        assert!(
            f.chain
                .context
                .validate_scalar_peer_custody_v1(peer.id)
                .is_ok()
        );
        f.chain
            .context
            .backend
            .producer_launch
            .observations
            .insert(peer.backend_submission, Observation::Failed);
        assert_eq!(
            f.chain.context.poll(&mut peer).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(f.bytes(f.output), &[11; 96]);
        assert_eq!(state(&f.chain.context, f.output).content_lineage, before);
        assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.chain.context.submissions[&list.id].dependency_retains, 0);
        assert!(!f.chain.context.is_terminal());
        if !cancelled {
            let calls = f.chain.context.backend.copy_call_count;
            assert!(f.peer(&[event], 0, 64).is_err());
            assert_eq!(f.chain.context.backend.copy_call_count, calls);
        }
        f.chain.context.release_event(event).unwrap();
        assert!(f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn segment_frame_peer_descended_unknown_or_uncertain_result_keeps_correct_custody() {
    for fault in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Forward::new();
        let list = f.chain.list(0, 8, &[]).unwrap();
        let event = f.chain.context.record_event(&list).unwrap();
        let mut peer = f.peer(&[event], 0, 64).unwrap();
        f.chain.context.release_event(event).unwrap();
        f.chain.finish(list.backend_submission);
        f.chain.finish(peer.backend_submission);
        assert_eq!(
            f.chain.context.poll(&mut peer).unwrap(),
            RuntimePollV1::Pending
        );
        let before = state(&f.chain.context, f.output).content_lineage;
        f.chain
            .context
            .backend
            .producer_launch
            .observations
            .insert(list.backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.chain.context.poll(&mut peer)));
        assert_eq!(state(&f.chain.context, f.output).content_lineage, before);
        if matches!(fault, Observation::Quiescent) {
            assert!(matches!(
                result,
                Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))
            ));
            assert_eq!(
                f.chain.context.query_submission(&peer).unwrap(),
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert_eq!(
                writer_state(&f.chain.context, f.output),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
            assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.chain.context.is_terminal());
            assert!(f.chain.context.cleanup().is_complete());
        } else {
            assert!(matches!(result, Err(_) | Ok(Err(_))));
            assert!(f.chain.context.is_terminal());
            assert_eq!(f.chain.context.submissions[&list.id].dependency_retains, 1);
            assert_eq!(f.chain.context.version_journal_read_records_v1(), Some(2));
            assert!(!f.chain.context.cleanup().is_complete());
        }
    }
}

#[test]
fn segment_frame_peer_retained_plan_window_rank_and_reader_drift_fail_before_backend_poll() {
    for (mode, queued) in (0..8)
        .flat_map(|mode| [false, true].map(|queued| (mode, queued)))
        .chain([(8, true)])
    {
        let mut f = Forward::new();
        let (list, event) = if queued {
            let (_, second, event) = f.chain.pair();
            (second, event)
        } else {
            let list = f.chain.list(0, 8, &[]).unwrap();
            let event = f.chain.context.record_event(&list).unwrap();
            (list, event)
        };
        let mut peer = f.peer(&[event], 0, 64).unwrap();
        match mode {
            0 => {
                f.chain
                    .context
                    .segmented_peer_copies
                    .get_mut(&list.id)
                    .unwrap()
                    .destination
                    .region
                    .byte_offset += 1
            }
            1 => {
                f.chain
                    .context
                    .segmented_peer_copies
                    .get_mut(&list.id)
                    .unwrap()
                    .source
                    .region
                    .byte_offset += 1
            }
            2 => {
                f.chain
                    .context
                    .scalar_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .source
                    .region
                    .byte_offset = 1
            }
            3 => {
                f.chain
                    .context
                    .scalar_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .destination
                    .region
                    .byte_offset += 1
            }
            4 => {
                f.chain
                    .context
                    .scalar_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .compute
                    .as_mut()
                    .unwrap()
                    .state
                    .depth = 1
            }
            5 if queued => f
                .chain
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_queued_read_for_test_v1(peer.id, 0),
            5 => f
                .chain
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_producer_read_reference_for_test_v1(peer.id, 0),
            6 => f.chain.context.backend.pending_segment_frame_peer_copy = false,
            7 => f
                .chain
                .context
                .segmented_peer_copies
                .get_mut(&list.id)
                .unwrap()
                .replace_plan_identity_for_test_v1(),
            8 => f
                .chain
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_queued_read_for_test_v1(peer.id, 3),
            _ => unreachable!(),
        }
        let calls = f.chain.context.backend.poll_call_count;
        assert!(f.chain.context.poll(&mut peer).is_err());
        assert_eq!(f.chain.context.backend.poll_call_count, calls);
        assert!(f.chain.context.is_terminal());
        assert_eq!(f.chain.context.submissions[&list.id].dependency_retains, 1);
        assert!(!f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn segment_frame_peer_does_not_add_pending_compute_consumer_authority() {
    for (offset, bytes) in [(0, 96), (9, 32)] {
        let mut f = Forward::new();
        let list = f.chain.list(0, 8, &[]).unwrap();
        let event = f.chain.context.record_event(&list).unwrap();
        let mut peer = f.peer(&[event], 0, 32).unwrap();
        f.chain.context.release_event(event).unwrap();
        let event = f.chain.context.record_event(&peer).unwrap();
        let device = f.chain.context.devices()[1].id();
        let output = f
            .chain
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 96, 16)
            .unwrap();
        let module = f
            .chain
            .context
            .load_module(device, b"excluded-frame-consumer")
            .unwrap();
        let kernel = f
            .chain
            .context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
        let arguments = MixedArguments(vec![
            span(f.output, RuntimeAccessV1::Read, offset, bytes),
            span(output, RuntimeAccessV1::Write, 0, 96),
        ]);
        let next = f.chain.context.backend.next;
        let before = state(&f.chain.context, output);
        assert!(
            f.chain
                .context
                .launch_producer_aware_v1(f.stream, &kernel, &arguments, geometry(), &[event],)
                .is_err()
        );
        assert_eq!(f.chain.context.backend.next, next);
        assert_eq!(state(&f.chain.context, output), before);
        assert!(f.chain.context.producer_launches.is_empty());
        f.chain.context.release_event(event).unwrap();
        f.chain.finish(list.backend_submission);
        f.chain.finish(peer.backend_submission);
        f.chain.observe(&mut peer);
        assert!(f.chain.context.cleanup().is_complete());
    }
}

#[test]
fn segment_frame_peer_survives_settled_predecessor_source_disposal() {
    let mut f = Forward::new();
    let (mut first, second, event) = f.chain.pair();
    let mut peer = f.peer(&[event], 0, 64).unwrap();
    f.chain.context.release_event(event).unwrap();
    let expected = f.chain.expected([8, 16]);
    f.chain.finish(first.backend_submission);
    f.chain.observe(&mut first);
    f.chain
        .context
        .release_allocation(f.chain.sources[0])
        .unwrap();
    assert!(
        f.chain
            .context
            .validate_scalar_peer_custody_v1(peer.id)
            .is_ok()
    );
    f.chain.finish(second.backend_submission);
    f.chain.finish(peer.backend_submission);
    f.chain.observe(&mut peer);
    assert_eq!(&f.bytes(f.output)[9..73], expected);
    assert!(
        f.chain
            .context
            .validate_scalar_peer_custody_v1(peer.id)
            .is_ok()
    );
    f.chain.context.release_submission(second).unwrap();
    f.chain.context.release_submission(first).unwrap();
    // A settled scalar result retains only immutable identity, not live ancestors.
    assert!(
        f.chain
            .context
            .validate_scalar_peer_custody_v1(peer.id)
            .is_ok()
    );
    f.chain.context.release_submission(peer).unwrap();
    assert!(f.chain.context.cleanup().is_complete());
}
