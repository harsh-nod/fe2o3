//! Scripted physical results and real Context reconciliation, not native evidence.

use super::*;

struct PendingChain {
    f: Chain,
    producers: Vec<RuntimeSubmissionV1<MixedArguments>>,
    events: [Option<RuntimeEventIdV1>; 2],
}

impl PendingChain {
    fn new(pending: [bool; 2]) -> Self {
        let mut f = Chain::new(true);
        f.context.backend.pending_compute_segments = true;
        f.context.backend.ordered_pending_compute_segments = true;
        let mut producers = Vec::new();
        let mut events = [None; 2];
        for (index, pending) in pending.into_iter().enumerate() {
            if pending {
                let producer = launch_source(&mut f, index, 64);
                events[index] = Some(f.context.record_event(&producer).unwrap());
                producers.push(producer);
            }
        }
        Self {
            f,
            producers,
            events,
        }
    }

    fn first(&mut self) -> List {
        let events: Vec<_> = self.events[0].into_iter().collect();
        self.f.list(0, 8, &events).unwrap()
    }

    fn successor(
        &mut self,
        predecessor: RuntimeEventIdV1,
        offset: u64,
    ) -> Result<List, RuntimeErrorV1<MockError>> {
        let mut events = vec![predecessor];
        events.extend(self.events[1]);
        self.f.list(1, offset, &events)
    }

    fn pair(&mut self, offset: u64) -> (List, List, RuntimeEventIdV1) {
        let first = self.first();
        let event = self.f.context.record_event(&first).unwrap();
        let second = self.successor(event, offset).unwrap();
        self.f.context.release_event(event).unwrap();
        for event in self.events.iter_mut().filter_map(Option::take) {
            self.f.context.release_event(event).unwrap();
        }
        let event = self.f.context.record_event(&second).unwrap();
        (first, second, event)
    }

    fn finish_sources(&mut self) {
        for producer in &self.producers {
            self.f.finish(producer.backend_submission);
        }
    }

    fn release_sources(&mut self) {
        for producer in self.producers.drain(..) {
            self.f.context.release_submission(producer).unwrap();
        }
    }
}

fn launch_source(f: &mut Chain, index: usize, bytes: u64) -> RuntimeSubmissionV1<MixedArguments> {
    let device = f.context.allocations[&f.sources[index]].device;
    let stream = f.context.create_stream(device).unwrap();
    let module = f
        .context
        .load_module(device, b"ordered-pending-source")
        .unwrap();
    let kernel = f
        .context
        .resolve_kernel::<MixedArguments>(module, "mixed")
        .unwrap();
    f.context
        .launch_producer_aware_v1(
            stream,
            &kernel,
            &MixedArguments(vec![span(
                f.sources[index],
                RuntimeAccessV1::Write,
                0,
                bytes,
            )]),
            geometry(),
            &[],
        )
        .unwrap()
}

#[test]
fn ordered_pending_segments_mixed_origins_reconcile_only_from_final_readback() {
    for pending in [[true, false], [false, true], [true, true]] {
        for offset in [8, 24] {
            let mut p = PendingChain::new(pending);
            let (first, second, event) = p.pair(offset);
            let mut readback = p.f.readback(event).unwrap();
            p.f.context.release_event(event).unwrap();
            let calls = Arc::new(Mutex::new(Vec::new()));
            for producer in &p.producers {
                callback(&mut p.f.context, producer, &calls);
                assert_eq!(p.f.context.submissions[&producer.id].dependency_retains, 1);
            }
            callback(&mut p.f.context, &first, &calls);
            callback(&mut p.f.context, &second, &calls);
            callback(&mut p.f.context, &readback, &calls);
            for (list, compute) in [(&first, pending[0]), (&second, pending[1])] {
                let root = &p.f.context.segmented_peer_copies[&list.id];
                assert_eq!(root.compute_producer_v1().is_some(), compute);
                assert_eq!(
                    p.f.context.submissions[&list.id]
                        .journal_producer_read
                        .is_some(),
                    compute
                );
                assert!(!p.f.context.submissions[&list.id].scalar_peer_copy);
            }
            let first_depth = if pending[0] { 2 } else { 1 };
            assert_eq!(
                p.f.context.segmented_peer_copies[&first.id].state.depth,
                first_depth
            );
            assert_eq!(
                p.f.context.segmented_peer_copies[&second.id].state.depth,
                first_depth + 1
            );
            let lineage = state(&p.f.context, p.f.destination).content_lineage;
            p.finish_sources();
            let expected = p.f.expected([8, offset]);
            p.f.finish(first.backend_submission);
            p.f.finish(second.backend_submission);
            p.f.finish(readback.backend_submission);
            for source in p.f.sources {
                validation(
                    p.f.context.write_allocation(source, 0, &[11; 64]),
                    RuntimeValidationErrorV1::ContextReserved,
                );
            }
            for producer in &p.producers {
                assert_eq!(
                    p.f.context.query_submission(producer).unwrap(),
                    RuntimeCompletionStatusV1::Pending
                );
            }
            assert_eq!(
                p.f.context.query_submission(&first).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(
                p.f.context.query_submission(&second).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
            assert_eq!(
                state(&p.f.context, p.f.destination).content_lineage,
                lineage
            );
            assert!(calls.lock().unwrap().is_empty());
            p.f.context.flush_stream(p.f.readback_stream).unwrap();
            p.f.observe(&mut readback);
            assert_eq!(
                p.f.context.backend.memory[&p.f.context.allocations[&p.f.host].backend_allocation],
                expected
            );
            let calls = calls.lock().unwrap();
            assert_eq!(calls.len(), p.producers.len() + 3);
            assert!(
                calls
                    .iter()
                    .all(|(_, status)| *status == RuntimeCompletionStatusV1::Succeeded)
            );
            assert_eq!(calls.last().unwrap().0, readback.id);
            let first_position = calls.iter().position(|(id, _)| *id == first.id).unwrap();
            let second_position = calls.iter().position(|(id, _)| *id == second.id).unwrap();
            assert!(first_position < second_position);
            drop(calls);
            assert_eq!(p.f.context.version_journal_read_records_v1(), Some(0));
            p.f.context.release_submission(readback).unwrap();
            p.f.context.release_submission(second).unwrap();
            p.f.context.release_submission(first).unwrap();
            p.release_sources();
            assert!(p.f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn ordered_pending_segments_full_frame_compute_reads_gaps_and_preserved_bytes() {
    let mut p = PendingChain::new([true, true]);
    let device = p.f.context.devices()[2].id();
    let output =
        p.f.context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
    p.f.context.write_allocation(output, 0, &[19; 64]).unwrap();
    let module =
        p.f.context
            .load_module(device, b"ordered-pending-consumer")
            .unwrap();
    let kernel =
        p.f.context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
    let (first, second, event) = p.pair(24);
    let mut consumer =
        p.f.context
            .launch_producer_aware_v1(
                p.f.stream,
                &kernel,
                &MixedArguments(vec![
                    span(p.f.destination, RuntimeAccessV1::Read, 0, 64),
                    span(p.f.destination, RuntimeAccessV1::Read, 48, 8),
                    span(output, RuntimeAccessV1::Write, 0, 64),
                ]),
                geometry(),
                &[event],
            )
            .unwrap();
    assert_eq!(p.f.context.producer_launches[&consumer.id].state.depth, 4);
    p.f.context.release_event(event).unwrap();
    p.finish_sources();
    let expected = p.f.expected([8, 24]);
    p.f.finish(first.backend_submission);
    p.f.finish(second.backend_submission);
    p.f.finish(consumer.backend_submission);
    p.f.observe(&mut consumer);
    let observed: Vec<_> =
        p.f.context
            .backend
            .observed_kernel_reads
            .iter()
            .filter(|read| read.submission == consumer.backend_submission)
            .collect();
    assert_eq!(observed.len(), 2);
    assert_eq!(observed[0].bytes, expected);
    assert_eq!(observed[1].bytes, expected[48..56]);
    p.f.context.release_submission(consumer).unwrap();
    p.f.context.release_submission(second).unwrap();
    p.f.context.release_submission(first).unwrap();
    p.release_sources();
    assert!(p.f.context.cleanup().is_complete());
}

#[test]
fn ordered_pending_segments_independent_capability_is_required_for_every_mixed_edge() {
    for pending in [[true, false], [false, true], [true, true]] {
        let mut p = PendingChain::new(pending);
        p.f.context.backend.ordered_pending_compute_segments = false;
        let mut first = p.first();
        let event = p.f.context.record_event(&first).unwrap();
        let before = (
            p.f.context.next_identity,
            p.f.context.backend.copy_call_count,
            p.f.context.version_journal_usage_v1(),
        );
        validation(
            p.successor(event, 24),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(
            (
                p.f.context.next_identity,
                p.f.context.backend.copy_call_count,
                p.f.context.version_journal_usage_v1()
            ),
            before
        );
        assert_eq!(p.f.context.submissions[&first.id].dependency_retains, 0);
        p.f.context.release_event(event).unwrap();
        for event in p.events.iter_mut().filter_map(Option::take) {
            p.f.context.release_event(event).unwrap();
        }
        p.finish_sources();
        p.f.finish(first.backend_submission);
        p.f.observe(&mut first);
        for producer in &mut p.producers {
            p.f.observe(producer);
        }
        p.f.context.release_submission(first).unwrap();
        p.release_sources();
        assert!(p.f.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_pending_segments_exact_source_and_latest_destination_events_are_independent() {
    for case in 0..5 {
        let mut p = PendingChain::new([true, true]);
        let mut first = p.first();
        let event = p.f.context.record_event(&first).unwrap();
        let source = p.events[1].unwrap();
        let events = match case {
            0 => vec![event],
            1 => vec![source],
            2 => {
                let foreign = p.events[0].unwrap();
                let device = p.f.context.events[&foreign].device;
                assert_ne!(device, p.f.context.allocations[&p.f.sources[1]].device);
                assert_ne!(device, p.f.context.allocations[&p.f.destination].device);
                vec![event, foreign]
            }
            3 => vec![source, event, source],
            _ => {
                p.f.stream =
                    p.f.context
                        .create_stream(p.f.context.devices()[2].id())
                        .unwrap();
                vec![source, event]
            }
        };
        let before = (
            p.f.context.next_identity,
            p.f.context.backend.copy_call_count,
            p.f.context.version_journal_usage_v1(),
        );
        validation(
            p.f.list(1, 24, &events),
            match case {
                // Endpoint validation rejects this event before journal custody.
                2 => RuntimeValidationErrorV1::WrongDevice,
                3 => RuntimeValidationErrorV1::DuplicateDependency,
                _ => RuntimeValidationErrorV1::ContextReserved,
            },
        );
        assert_eq!(
            (
                p.f.context.next_identity,
                p.f.context.backend.copy_call_count,
                p.f.context.version_journal_usage_v1()
            ),
            before
        );
        p.f.context.release_event(event).unwrap();
        for event in p.events.iter_mut().filter_map(Option::take) {
            p.f.context.release_event(event).unwrap();
        }
        p.finish_sources();
        p.f.finish(first.backend_submission);
        p.f.observe(&mut first);
        for producer in &mut p.producers {
            p.f.observe(producer);
        }
        p.f.context.release_submission(first).unwrap();
        p.release_sources();
        assert!(p.f.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_pending_segments_partial_source_writer_remains_rejected() {
    let mut p = PendingChain::new([false, false]);
    let mut producer = launch_source(&mut p.f, 1, 32);
    let source_event = p.f.context.record_event(&producer).unwrap();
    let mut first = p.first();
    let event = p.f.context.record_event(&first).unwrap();
    let before = (
        p.f.context.next_identity,
        p.f.context.backend.copy_call_count,
        p.f.context.version_journal_usage_v1(),
    );
    validation(
        p.f.list(1, 24, &[source_event, event]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(
        (
            p.f.context.next_identity,
            p.f.context.backend.copy_call_count,
            p.f.context.version_journal_usage_v1()
        ),
        before
    );
    p.f.context.release_event(source_event).unwrap();
    p.f.context.release_event(event).unwrap();
    p.f.finish(producer.backend_submission);
    p.f.finish(first.backend_submission);
    p.f.observe(&mut producer);
    p.f.observe(&mut first);
    p.f.context.release_submission(first).unwrap();
    p.f.context.release_submission(producer).unwrap();
    assert!(p.f.context.cleanup().is_complete());
}

#[test]
fn ordered_pending_segments_stale_destination_event_cannot_skip_a_mixed_successor() {
    let mut p = PendingChain::new([true, true]);
    let first = p.first();
    let stale = p.f.context.record_event(&first).unwrap();
    let second = p.successor(stale, 24).unwrap();
    let latest = p.f.context.record_event(&second).unwrap();
    let before = (
        p.f.context.next_identity,
        p.f.context.backend.copy_call_count,
        p.f.context.version_journal_usage_v1(),
    );
    validation(
        p.successor(stale, 16),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(
        (
            p.f.context.next_identity,
            p.f.context.backend.copy_call_count,
            p.f.context.version_journal_usage_v1(),
        ),
        before
    );
    let mut third = p.successor(latest, 16).unwrap();
    assert_eq!(p.f.context.segmented_peer_copies[&third.id].state.depth, 4);
    assert_eq!(
        p.f.context.submissions[&p.producers[1].id].dependency_retains,
        2
    );
    for event in [stale, latest] {
        p.f.context.release_event(event).unwrap();
    }
    for event in p.events.iter_mut().filter_map(Option::take) {
        p.f.context.release_event(event).unwrap();
    }
    p.finish_sources();
    for id in [
        first.backend_submission,
        second.backend_submission,
        third.backend_submission,
    ] {
        p.f.finish(id);
    }
    p.f.observe(&mut third);
    p.f.context.release_submission(third).unwrap();
    p.f.context.release_submission(second).unwrap();
    p.f.context.release_submission(first).unwrap();
    p.release_sources();
    assert!(p.f.context.cleanup().is_complete());
}

#[test]
fn ordered_pending_segments_cancel_successor_refunds_both_paths_without_settling_parents() {
    let mut p = PendingChain::new([true, true]);
    let (mut first, mut second, event) = p.pair(24);
    p.f.context.release_event(event).unwrap();
    p.f.context.backend.cancel_before_publication = true;
    assert_eq!(
        p.f.context.cancel(&mut second).unwrap(),
        RuntimeCancellationV1::Cancelled
    );
    assert_eq!(p.f.context.submissions[&first.id].dependency_retains, 0);
    assert_eq!(
        p.f.context.submissions[&p.producers[0].id].dependency_retains,
        1
    );
    assert_eq!(
        p.f.context.submissions[&p.producers[1].id].dependency_retains,
        0
    );
    for producer in &p.producers {
        assert_eq!(
            p.f.context.query_submission(producer).unwrap(),
            RuntimeCompletionStatusV1::Pending
        );
    }
    validation(
        p.f.context.write_allocation(p.f.sources[1], 0, &[31; 64]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    p.finish_sources();
    p.f.finish(first.backend_submission);
    p.f.observe(&mut first);
    p.f.observe(&mut p.producers[1]);
    p.f.context
        .write_allocation(p.f.sources[1], 0, &[31; 64])
        .unwrap();
    p.f.context.release_submission(second).unwrap();
    p.f.context.release_submission(first).unwrap();
    p.release_sources();
    assert!(p.f.context.cleanup().is_complete());
}

#[test]
fn ordered_pending_segments_failed_source_or_destination_never_promotes_frame() {
    for failed_source in [false, true] {
        let mut p = PendingChain::new([true, true]);
        let (mut first, mut second, event) = p.pair(24);
        let mut readback = p.f.readback(event).unwrap();
        p.f.context.release_event(event).unwrap();
        let lineage = state(&p.f.context, p.f.destination).content_lineage;
        if failed_source {
            p.f.context
                .backend
                .producer_launch
                .observations
                .insert(p.producers[1].backend_submission, Observation::Failed);
            assert_eq!(
                p.f.context.poll(&mut p.producers[1]).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            );
            p.f.finish(p.producers[0].backend_submission);
            p.f.finish(first.backend_submission);
            p.f.observe(&mut first);
        } else {
            p.finish_sources();
            for producer in &mut p.producers {
                p.f.observe(producer);
            }
            p.f.context
                .backend
                .producer_launch
                .observations
                .insert(first.backend_submission, Observation::Failed);
            assert_eq!(
                p.f.context.poll(&mut first).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            );
        }
        p.f.context
            .validate_pending_segmented_peer_roots_v1(second.id)
            .unwrap();
        for id in [second.backend_submission, readback.backend_submission] {
            p.f.context
                .backend
                .producer_launch
                .observations
                .insert(id, Observation::Failed);
        }
        assert_eq!(
            p.f.context.poll(&mut second).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(
            p.f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Failed { code: 7 }
        );
        assert_eq!(
            p.f.context.backend.memory[&p.f.context.allocations[&p.f.host].backend_allocation],
            [7; 64]
        );
        if !failed_source {
            assert_eq!(
                state(&p.f.context, p.f.destination).content_lineage,
                lineage
            );
        }
        validation(
            p.f.list(0, 8, &[]),
            RuntimeValidationErrorV1::ContextReserved,
        );
        assert_eq!(p.f.context.version_journal_read_records_v1(), Some(0));
        assert!(p.f.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_pending_segments_terminal_uncertainty_retains_both_dependency_paths() {
    let mut p = core::mem::ManuallyDrop::new(PendingChain::new([true, true]));
    let (first, mut second, event) = p.pair(24);
    p.f.context.release_event(event).unwrap();
    p.f.context
        .backend
        .producer_launch
        .observations
        .insert(second.backend_submission, Observation::Terminal);
    assert!(p.f.context.poll(&mut second).is_err());
    assert!(p.f.context.is_terminal());
    assert!(p.f.context.segmented_peer_copies[&second.id].dependencies_held);
    assert_eq!(p.f.context.submissions[&first.id].dependency_retains, 1);
    for producer in &p.producers {
        assert_eq!(p.f.context.submissions[&producer.id].dependency_retains, 1);
    }
}

#[test]
fn ordered_pending_segments_independent_immutable_origins_and_ranks_reject_drift() {
    for mutation in 0..6 {
        let mut p = core::mem::ManuallyDrop::new(PendingChain::new([true, true]));
        let (first, mut second, event) = p.pair(24);
        p.f.context.release_event(event).unwrap();
        let root =
            p.f.context
                .segmented_peer_copies
                .get_mut(&second.id)
                .unwrap();
        match mutation {
            0 => root.origin = SegmentedPeerSourceV1::Settled,
            1 => root.predecessor = None,
            2 => root.state.depth = 2,
            3 => {
                let SegmentedPeerSourceV1::Compute(mut producer) = root.origin else {
                    panic!("compute origin");
                };
                producer.backend_event += 1;
                root.origin = SegmentedPeerSourceV1::Compute(producer);
            }
            4 => root.predecessor.as_mut().unwrap().submission = second.id,
            _ => root.source.region.byte_offset += 1,
        }
        let calls = p.f.context.backend.poll_call_count;
        assert!(p.f.context.poll(&mut second).is_err());
        assert!(p.f.context.is_terminal());
        assert_eq!(p.f.context.backend.poll_call_count, calls);
        assert_eq!(p.f.context.submissions[&first.id].dependency_retains, 1);
        assert!(p.f.context.segmented_peer_copies[&second.id].dependencies_held);
    }
}

#[test]
fn ordered_pending_segments_successor_outlives_disposed_compute_source_and_result() {
    let mut p = PendingChain::new([true, true]);
    let (mut first, second, event) = p.pair(24);
    p.finish_sources();
    let expected = p.f.expected([8, 24]);
    p.f.finish(first.backend_submission);
    p.f.observe(&mut first);
    p.f.context.release_allocation(p.f.sources[0]).unwrap();
    let producer = p.producers.remove(0);
    p.f.context.release_submission(producer).unwrap();
    p.f.context
        .validate_pending_segmented_peer_roots_v1(second.id)
        .unwrap();
    let mut readback = p.f.readback(event).unwrap();
    p.f.context.release_event(event).unwrap();
    p.f.finish(second.backend_submission);
    p.f.finish(readback.backend_submission);
    p.f.observe(&mut readback);
    assert_eq!(
        p.f.context.backend.memory[&p.f.context.allocations[&p.f.host].backend_allocation],
        expected
    );
    p.f.context.release_submission(readback).unwrap();
    p.f.context.release_submission(second).unwrap();
    p.f.context.release_submission(first).unwrap();
    p.release_sources();
    assert!(p.f.context.cleanup().is_complete());
}

#[test]
fn ordered_pending_segments_unknown_source_cannot_become_a_settled_origin() {
    let mut p = PendingChain::new([false, true]);
    let mut first = p.first();
    let event = p.f.context.record_event(&first).unwrap();
    p.f.context
        .backend
        .producer_launch
        .observations
        .insert(p.producers[0].backend_submission, Observation::Failed);
    assert_eq!(
        p.f.context.poll(&mut p.producers[0]).unwrap(),
        RuntimePollV1::Failed { code: 7 }
    );
    let before = (
        p.f.context.next_identity,
        p.f.context.backend.copy_call_count,
        p.f.context.version_journal_usage_v1(),
    );
    validation(
        p.successor(event, 24),
        RuntimeValidationErrorV1::ContextReserved,
    );
    validation(
        p.f.list(1, 24, &[event]),
        RuntimeValidationErrorV1::ContextReserved,
    );
    assert_eq!(
        (
            p.f.context.next_identity,
            p.f.context.backend.copy_call_count,
            p.f.context.version_journal_usage_v1()
        ),
        before
    );
    p.f.context.release_event(event).unwrap();
    p.f.context
        .release_event(p.events[1].take().unwrap())
        .unwrap();
    p.f.finish(first.backend_submission);
    p.f.observe(&mut first);
    p.f.context.release_submission(first).unwrap();
    p.release_sources();
    assert!(p.f.context.cleanup().is_complete());
}

#[test]
fn ordered_pending_segments_depth_256_admits_then_rejects_both_path_extension() {
    let mut p = PendingChain::new([false, true]);
    let mut lists = Vec::new();
    let mut previous = None;
    for depth in 1..MAX_RUNTIME_DEPENDENCIES_V1 {
        let events: Vec<_> = previous.into_iter().collect();
        let list = p.f.list(0, 8, &events).unwrap();
        assert_eq!(
            p.f.context.segmented_peer_copies[&list.id].state.depth,
            depth
        );
        if let Some(event) = previous {
            p.f.context.release_event(event).unwrap();
        }
        previous = Some(p.f.context.record_event(&list).unwrap());
        lists.push(list);
    }
    let event = previous.unwrap();
    let last = p.successor(event, 24).unwrap();
    assert_eq!(
        p.f.context.segmented_peer_copies[&last.id].state.depth,
        MAX_RUNTIME_DEPENDENCIES_V1
    );
    p.f.context.release_event(event).unwrap();
    let event = p.f.context.record_event(&last).unwrap();
    let before = (
        p.f.context.next_identity,
        p.f.context.backend.copy_call_count,
        p.f.context.version_journal_usage_v1(),
    );
    validation(
        p.successor(event, 24),
        RuntimeValidationErrorV1::TooManyDependencies,
    );
    assert_eq!(
        (
            p.f.context.next_identity,
            p.f.context.backend.copy_call_count,
            p.f.context.version_journal_usage_v1()
        ),
        before
    );
    p.f.context.release_event(event).unwrap();
    p.f.context
        .release_event(p.events[1].take().unwrap())
        .unwrap();
    lists.push(last);
    p.finish_sources();
    for list in &lists {
        p.f.finish(list.backend_submission);
    }
    p.f.observe(lists.last_mut().unwrap());
    for list in lists.into_iter().rev() {
        p.f.context.release_submission(list).unwrap();
    }
    p.release_sources();
    assert!(p.f.context.cleanup().is_complete());
}
