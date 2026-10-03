//! Scripted three-device journal/frame composition, not native DMA evidence.

use super::*;
use crate::context::peer_segments::SegmentedPeerSourceV1;

#[path = "ordered_compute.rs"]
mod compute;

fn windows() -> Vec<RuntimePeerCopySegmentV1> {
    vec![
        RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 13,
            destination_offset: 5,
            byte_len: 7,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 13,
            destination_offset: 5,
            byte_len: 7,
        },
    ]
}

struct Chain {
    context: Context,
    sources: [RuntimeAllocationIdV1; 2],
    destination: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
}

impl Chain {
    fn new(ordered: bool) -> Self {
        let mut context = Context::open_with_version_journal_members_v1(
            MockBackend {
                next: 100,
                third_device: true,
                deferred_copies: true,
                deferred_kernel_reads: true,
                pending_peer_readback: true,
                peer_segments_frame: true,
                ordered_peer_segments: ordered,
                ..MockBackend::default()
            },
            32,
            MAX_RUNTIME_DEPENDENCIES_V1 + 1,
            MAX_RUNTIME_DEPENDENCIES_V1 + 1,
        )
        .unwrap();
        let devices = [
            context.devices()[0].id(),
            context.devices()[1].id(),
            context.devices()[2].id(),
        ];
        let sources = [devices[0], devices[1]].map(|device| {
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap()
        });
        let destination = context
            .allocate(devices[2], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let host = context
            .allocate(devices[2], RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        for (index, source) in sources.iter().enumerate() {
            let bytes: Vec<_> = (0..64).map(|byte| byte as u8 + 71 * index as u8).collect();
            context.write_allocation(*source, 0, &bytes).unwrap();
        }
        context.write_allocation(destination, 0, &[5; 64]).unwrap();
        context.write_allocation(host, 0, &[7; 64]).unwrap();
        let stream = context.create_stream(devices[2]).unwrap();
        let readback_stream = context.create_stream(devices[2]).unwrap();
        Self {
            context,
            sources,
            destination,
            host,
            stream,
            readback_stream,
        }
    }

    fn list(
        &mut self,
        index: usize,
        offset: u64,
        events: &[RuntimeEventIdV1],
    ) -> Result<List, RuntimeErrorV1<MockError>> {
        self.context.peer_copy_segments(
            self.stream,
            span(self.sources[index], RuntimeAccessV1::Read, 3, 32),
            span(self.destination, RuntimeAccessV1::Write, offset, 40),
            &windows(),
            events,
        )
    }

    fn readback(
        &mut self,
        event: RuntimeEventIdV1,
    ) -> Result<RuntimeSubmissionV1<RuntimeCopyV1>, RuntimeErrorV1<MockError>> {
        self.context.copy_async(
            self.readback_stream,
            span(self.destination, RuntimeAccessV1::Read, 0, 64),
            span(self.host, RuntimeAccessV1::Write, 0, 64),
            &[event],
        )
    }

    fn finish(&mut self, submission: u64) {
        self.context.backend.finish_submission(submission, true);
        self.context.backend.polls.insert(submission, 1);
    }

    fn observe<M>(&mut self, submission: &mut RuntimeSubmissionV1<M>) {
        for _ in 0..4 * MAX_RUNTIME_DEPENDENCIES_V1 {
            if self.context.poll(submission).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("ordered list chain did not reconcile");
    }

    fn expected(&self, offsets: [u64; 2]) -> Vec<u8> {
        let mut output = vec![5; 64];
        for (index, base) in offsets.into_iter().enumerate() {
            let source = &self.context.backend.memory
                [&self.context.allocations[&self.sources[index]].backend_allocation];
            for segment in windows() {
                let src = (3 + segment.source_offset) as usize;
                let dst = (base + segment.destination_offset) as usize;
                let len = segment.byte_len as usize;
                output[dst..dst + len].copy_from_slice(&source[src..src + len]);
            }
        }
        output
    }

    fn pair(&mut self) -> (List, List, RuntimeEventIdV1) {
        let first = self.list(0, 8, &[]).unwrap();
        let event = self.context.record_event(&first).unwrap();
        let second = self.list(1, 16, &[event]).unwrap();
        self.context.release_event(event).unwrap();
        let event = self.context.record_event(&second).unwrap();
        (first, second, event)
    }
}

#[test]
fn ordered_segments_latest_frame_reconciles_both_lists_from_only_final_readback() {
    for second_offset in [8, 24] {
        let mut f = Chain::new(true);
        let first = f.list(0, 8, &[]).unwrap();
        let first_event = f.context.record_event(&first).unwrap();
        let second = f.list(1, second_offset, &[first_event]).unwrap();
        let second_event = f.context.record_event(&second).unwrap();
        let mut readback = f.readback(second_event).unwrap();
        for (id, depth) in [(first.id, 1), (second.id, 2)] {
            let root = &f.context.segmented_peer_copies[&id];
            assert_eq!(root.origin, SegmentedPeerSourceV1::Settled);
            assert_eq!(root.state.depth, depth);
            assert!(f.context.submissions[&id].journal_producer_read.is_none());
        }
        assert_eq!(f.context.same_device_copies[&readback.id].state.depth, 3);
        let callbacks = Arc::new(Mutex::new(Vec::new()));
        callback(&mut f.context, &first, &callbacks);
        callback(&mut f.context, &second, &callbacks);
        callback(&mut f.context, &readback, &callbacks);
        f.context.release_event(first_event).unwrap();
        f.context.release_event(second_event).unwrap();
        assert_eq!(f.context.submissions[&first.id].dependency_retains, 1);
        assert_eq!(f.context.submissions[&second.id].dependency_retains, 1);
        let expected = f.expected([8, second_offset]);
        // Script the backend's actual descriptor execution in order. The logical
        // Context observes only the final consumer, never assigned parent success.
        f.finish(first.backend_submission);
        f.finish(second.backend_submission);
        f.finish(readback.backend_submission);
        for submission in [&first, &second] {
            assert_eq!(
                f.context.query_submission(submission).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
        }
        f.context.flush_stream(f.readback_stream).unwrap();
        f.observe(&mut readback);
        assert_eq!(
            f.context.backend.last_flushed_stream,
            Some(f.context.streams[&f.readback_stream].backend_stream)
        );
        assert_eq!(
            f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
            expected
        );
        assert_eq!(
            *callbacks.lock().unwrap(),
            [
                (first.id, RuntimeCompletionStatusV1::Succeeded),
                (second.id, RuntimeCompletionStatusV1::Succeeded),
                (readback.id, RuntimeCompletionStatusV1::Succeeded),
            ]
        );
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert_eq!(f.context.submissions[&first.id].dependency_retains, 0);
        assert_eq!(f.context.submissions[&second.id].dependency_retains, 0);
        f.context.release_submission(readback).unwrap();
        f.context.release_submission(second).unwrap();
        f.context.release_submission(first).unwrap();
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_segments_latest_frame_feeds_all_read_only_compute_aliases() {
    let mut f = Chain::new(true);
    let device = f.context.devices()[2].id();
    let output = f
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    f.context.write_allocation(output, 0, &[19; 64]).unwrap();
    let module = f
        .context
        .load_module(device, b"ordered-segments-consumer")
        .unwrap();
    let kernel: TypedRuntimeKernelV1<MixedArguments> =
        f.context.resolve_kernel(module, "mixed").unwrap();
    let (first, second, event) = f.pair();
    let args = MixedArguments(vec![
        span(f.destination, RuntimeAccessV1::Read, 0, 64),
        span(f.destination, RuntimeAccessV1::Read, 0, 4),
        span(output, RuntimeAccessV1::Write, 0, 64),
    ]);
    let mut consumer = f
        .context
        .launch_producer_aware_v1(f.stream, &kernel, &args, geometry(), &[event])
        .unwrap();
    assert_eq!(f.context.producer_launches[&consumer.id].state.depth, 3);
    f.context.release_event(event).unwrap();
    let expected = f.expected([8, 16]);
    f.finish(first.backend_submission);
    f.finish(second.backend_submission);
    f.finish(consumer.backend_submission);
    f.observe(&mut consumer);
    let reads: Vec<_> = f
        .context
        .backend
        .observed_kernel_reads
        .iter()
        .filter(|read| read.submission == consumer.backend_submission)
        .collect();
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[0].bytes, expected);
    assert_eq!(reads[1].bytes, [5; 4]);
    f.context.release_submission(consumer).unwrap();
    f.context.release_submission(second).unwrap();
    f.context.release_submission(first).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordered_segments_require_enabled_exact_latest_same_stream_predecessor() {
    for case in 0..5 {
        let mut f = Chain::new(case != 0);
        let first = f.list(0, 8, &[]).unwrap();
        let event = f.context.record_event(&first).unwrap();
        let mut second = None;
        let events = match case {
            1 => vec![],
            3 => {
                second = Some(f.list(1, 16, &[event]).unwrap());
                vec![event]
            }
            4 => vec![event, event],
            _ => vec![event],
        };
        if case == 2 {
            f.stream = f
                .context
                .create_stream(f.context.devices()[2].id())
                .unwrap();
        }
        let before = (
            f.context.next_identity,
            f.context.backend.copy_call_count,
            f.context.version_journal_usage_v1(),
            f.context.submissions[&first.id].dependency_retains,
        );
        validation(
            f.list(1, 16, &events),
            if case == 4 {
                RuntimeValidationErrorV1::DuplicateDependency
            } else {
                RuntimeValidationErrorV1::ContextReserved
            },
        );
        assert_eq!(
            (
                f.context.next_identity,
                f.context.backend.copy_call_count,
                f.context.version_journal_usage_v1(),
                f.context.submissions[&first.id].dependency_retains
            ),
            before
        );
        f.context.release_event(event).unwrap();
        f.finish(first.backend_submission);
        if let Some(second) = &mut second {
            f.finish(second.backend_submission);
            f.observe(second);
        } else {
            let mut first = first;
            f.observe(&mut first);
        }
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_segments_retained_successor_outlives_disposed_ancestor_source() {
    let mut f = Chain::new(true);
    let (mut first, second, event) = f.pair();
    for source in f.sources {
        validation(
            f.context.write_allocation(source, 0, &[9; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(
            f.context.release_allocation(source),
            RuntimeValidationErrorV1::ContextReserved,
        );
    }
    let expected = f.expected([8, 16]);
    f.finish(first.backend_submission);
    f.observe(&mut first);
    f.context.release_allocation(f.sources[0]).unwrap();
    assert_eq!(f.context.submissions[&first.id].dependency_retains, 1);
    f.context
        .validate_pending_segmented_peer_roots_v1(second.id)
        .unwrap();
    let mut readback = f.readback(event).unwrap();
    f.context.release_event(event).unwrap();
    f.finish(second.backend_submission);
    f.finish(readback.backend_submission);
    f.observe(&mut readback);
    assert_eq!(
        f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
        expected
    );
    f.context.release_submission(readback).unwrap();
    f.context.release_submission(second).unwrap();
    f.context.release_submission(first).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordered_segments_cancelled_or_failed_predecessor_never_promotes_latest_frame() {
    for cancelled in [false, true] {
        let mut f = Chain::new(true);
        let (mut first, mut second, event) = f.pair();
        let mut readback = f.readback(event).unwrap();
        f.context.release_event(event).unwrap();
        let lineage = state(&f.context, f.destination).content_lineage;
        if cancelled {
            f.context.backend.cancel_before_publication = true;
            assert_eq!(
                f.context.cancel(&mut first).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
        } else {
            f.context
                .backend
                .producer_launch
                .observations
                .insert(first.backend_submission, Observation::Failed);
            assert_eq!(
                f.context.poll(&mut first).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            );
        }
        f.context
            .validate_pending_segmented_peer_roots_v1(second.id)
            .unwrap();
        for submission in [second.backend_submission, readback.backend_submission] {
            f.context
                .backend
                .producer_launch
                .observations
                .insert(submission, Observation::Failed);
        }
        assert_eq!(
            f.context.poll(&mut second).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(state(&f.context, f.destination).content_lineage, lineage);
        assert_eq!(
            f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
            [7; 64]
        );
        validation(
            f.context.write_allocation(f.destination, 0, &[11; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        validation(f.list(0, 8, &[]), RuntimeValidationErrorV1::ContextReserved);
        assert_eq!(f.context.submissions[&first.id].dependency_retains, 0);
        assert_eq!(f.context.submissions[&second.id].dependency_retains, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_segments_cancel_successor_refunds_only_its_source_and_predecessor_hold() {
    let mut f = Chain::new(true);
    let (mut first, mut second, event) = f.pair();
    f.context.release_event(event).unwrap();
    f.context.backend.cancel_before_publication = true;
    assert_eq!(
        f.context.cancel(&mut second).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(f.context.submissions[&first.id].dependency_retains, 0);
    f.context
        .write_allocation(f.sources[1], 0, &[31; 64])
        .unwrap();
    validation(
        f.context.write_allocation(f.sources[0], 0, &[29; 64]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    f.finish(first.backend_submission);
    f.observe(&mut first);
    f.context.release_submission(second).unwrap();
    f.context.release_submission(first).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordered_segments_immutable_predecessor_and_rank_drift_fail_before_observation() {
    for mutation in 0..5 {
        let mut f = Chain::new(true);
        let (first, mut second, event) = f.pair();
        f.context.release_event(event).unwrap();
        let mut f = core::mem::ManuallyDrop::new(f);
        let root = f.context.segmented_peer_copies.get_mut(&second.id).unwrap();
        match mutation {
            0 => root.predecessor = None,
            1 => root.predecessor.as_mut().unwrap().backend_event += 1,
            2 => root.state.depth = 1,
            3 => root.backend_stream += 1,
            _ => root.destination.region.byte_offset += 1,
        }
        let calls = f.context.backend.poll_call_count;
        assert!(f.context.poll(&mut second).is_err());
        assert!(f.context.is_terminal());
        assert_eq!(f.context.backend.poll_call_count, calls);
        assert_eq!(f.context.submissions[&first.id].dependency_retains, 1);
        assert!(f.context.segmented_peer_copies[&second.id].dependencies_held);
    }
}

#[test]
fn ordered_segments_depth_bound_rejects_extra_pending_writer_without_effects() {
    let mut f = Chain::new(true);
    let mut lists = Vec::new();
    let mut previous = None;
    for depth in 1..=MAX_RUNTIME_DEPENDENCIES_V1 {
        let events: Vec<_> = previous.into_iter().collect();
        let list = f.list(depth % 2, 8, &events).unwrap();
        assert_eq!(f.context.segmented_peer_copies[&list.id].state.depth, depth);
        if let Some(event) = previous {
            f.context.release_event(event).unwrap();
        }
        previous = Some(f.context.record_event(&list).unwrap());
        lists.push(list);
    }
    let event = previous.unwrap();
    let before = (
        f.context.next_identity,
        f.context.backend.copy_call_count,
        f.context.version_journal_usage_v1(),
    );
    validation(
        f.list(0, 8, &[event]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(
        (
            f.context.next_identity,
            f.context.backend.copy_call_count,
            f.context.version_journal_usage_v1()
        ),
        before
    );
    f.context.release_event(event).unwrap();
    for list in &lists {
        f.finish(list.backend_submission);
    }
    f.observe(lists.last_mut().unwrap());
    for list in lists.into_iter().rev() {
        f.context.release_submission(list).unwrap();
    }
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordered_segments_foreign_and_released_events_reject_before_backend_admission() {
    let mut f = Chain::new(true);
    let mut foreign = Chain::new(true);
    let mut first = f.list(0, 8, &[]).unwrap();
    let mut other = foreign.list(0, 8, &[]).unwrap();
    let foreign_event = foreign.context.record_event(&other).unwrap();
    let released = f.context.record_event(&first).unwrap();
    f.context.release_event(released).unwrap();
    let before = (
        f.context.next_identity,
        f.context.backend.copy_call_count,
        f.context.version_journal_usage_v1(),
    );
    for event in [foreign_event, released] {
        validation(
            f.list(1, 16, &[event]),
            RuntimeValidationErrorV1::UnknownEvent,
        );
        assert_eq!(
            (
                f.context.next_identity,
                f.context.backend.copy_call_count,
                f.context.version_journal_usage_v1()
            ),
            before
        );
    }
    foreign.context.release_event(foreign_event).unwrap();
    f.finish(first.backend_submission);
    f.observe(&mut first);
    foreign.finish(other.backend_submission);
    foreign.observe(&mut other);
    assert!(f.context.cleanup().is_complete());
    assert!(foreign.context.cleanup().is_complete());
}

#[test]
fn ordered_segments_completed_event_is_only_control_after_destination_rewrite() {
    let mut f = Chain::new(true);
    let mut first = f.list(0, 8, &[]).unwrap();
    let event = f.context.record_event(&first).unwrap();
    f.finish(first.backend_submission);
    f.observe(&mut first);
    f.context
        .write_allocation(f.destination, 0, &[91; 64])
        .unwrap();
    let before = state(&f.context, f.destination);
    let second = f.list(1, 16, &[event]).unwrap();
    assert!(
        f.context.segmented_peer_copies[&second.id]
            .predecessor
            .is_none()
    );
    let writer = f.context.submissions[&second.id].journal_writer.unwrap();
    assert_eq!(
        state(&f.context, f.destination).pending_writer,
        Some(writer)
    );
    assert_eq!(
        state(&f.context, f.destination).content_lineage,
        before.content_lineage
    );
    f.context.release_event(event).unwrap();
    let event = f.context.record_event(&second).unwrap();
    let mut readback = f.readback(event).unwrap();
    f.context.release_event(event).unwrap();
    f.finish(second.backend_submission);
    f.finish(readback.backend_submission);
    f.observe(&mut readback);
    let bytes = &f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation];
    assert_eq!(&bytes[..16], &[91; 16]);
    assert_eq!(&bytes[28..], &[91; 36]);
    f.context.release_submission(readback).unwrap();
    f.context.release_submission(second).unwrap();
    f.context.release_submission(first).unwrap();
    assert!(f.context.cleanup().is_complete());
}
