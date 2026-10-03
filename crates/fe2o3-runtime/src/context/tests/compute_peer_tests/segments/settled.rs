//! Scripted journal/frame contracts, not native transfer or arithmetic evidence.

use super::*;
use crate::context::peer_segments::SegmentedPeerSourceV1;

struct Settled {
    context: Context,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
}

impl Settled {
    fn new(journal: bool, capability: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            deferred_kernel_reads: true,
            pending_peer_readback: true,
            peer_segments_frame: capability,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_members_v1(backend, 32, 16, 64).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let source = context
            .allocate(
                context.devices()[0].id(),
                RuntimeMemoryKindV1::DeviceLocal,
                64,
                16,
            )
            .unwrap();
        let device = context.devices()[1].id();
        let destination = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let host = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        context
            .write_allocation(source, 0, &(0..64).collect::<Vec<u8>>())
            .unwrap();
        context.write_allocation(destination, 0, &[5; 64]).unwrap();
        context.write_allocation(host, 0, &[7; 64]).unwrap();
        let stream = context.create_stream(device).unwrap();
        let readback_stream = context.create_stream(device).unwrap();
        Self {
            context,
            source,
            destination,
            host,
            stream,
            readback_stream,
        }
    }

    fn list(&mut self, events: &[RuntimeEventIdV1]) -> Result<List, RuntimeErrorV1<MockError>> {
        self.context.peer_copy_segments(
            self.stream,
            span(self.source, RuntimeAccessV1::Read, 3, 32),
            span(self.destination, RuntimeAccessV1::Write, 8, 40),
            &descriptors(),
            events,
        )
    }

    fn readback(
        &mut self,
        events: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimeCopyV1>, RuntimeErrorV1<MockError>> {
        self.context.copy_async(
            self.readback_stream,
            span(self.destination, RuntimeAccessV1::Read, 0, 64),
            span(self.host, RuntimeAccessV1::Write, 0, 64),
            events,
        )
    }

    fn kernel(&mut self) -> TypedRuntimeKernelV1<MixedArguments> {
        let module = self
            .context
            .load_module(self.context.devices()[1].id(), b"settled-list-consumer")
            .unwrap();
        self.context.resolve_kernel(module, "mixed").unwrap()
    }

    fn finish(&mut self, submission: u64) {
        self.context.backend.finish_submission(submission, true);
        self.context.backend.polls.insert(submission, 1);
    }

    fn observe<M>(&mut self, submission: &mut RuntimeSubmissionV1<M>) {
        for _ in 0..8 {
            if self.context.poll(submission).unwrap() == RuntimePollV1::Succeeded {
                return;
            }
        }
        panic!("settled list chain did not reconcile");
    }

    fn expected(&self) -> Vec<u8> {
        let source = &self.context.backend.memory
            [&self.context.allocations[&self.source].backend_allocation];
        let mut output = vec![5; 64];
        for descriptor in descriptors() {
            let src = 3 + descriptor.source_offset as usize;
            let dst = 8 + descriptor.destination_offset as usize;
            let len = descriptor.byte_len as usize;
            output[dst..dst + len].copy_from_slice(&source[src..src + len]);
        }
        output
    }
}

#[test]
fn settled_segments_empty_dependencies_hold_current_read_and_publish_full_frame() {
    let mut f = Settled::new(true, true);
    assert!(f.context.submissions.is_empty());
    let before = state(&f.context, f.source);
    let peer = f.list(&[]).unwrap();
    let root = &f.context.segmented_peer_copies[&peer.id];
    assert_eq!(root.origin, SegmentedPeerSourceV1::Settled);
    assert!(root.dependencies.is_empty());
    assert_eq!(root.state.depth, 1);
    let record = f.context.submissions[&peer.id];
    assert!(record.segmented_peer_copy && !record.scalar_peer_copy);
    assert!(record.journal_producer_read.is_none());
    let marker = record.journal_read.unwrap();
    let read = f
        .context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .lookup_read(marker.first)
        .unwrap();
    assert_eq!(
        (read.attempt_epoch, read.content_lineage),
        (before.attempt_epoch, before.content_lineage)
    );
    assert_eq!((read.byte_offset, read.byte_len), (3, 32));
    let event = f.context.record_event(&peer).unwrap();
    let mut readback = f.readback(&[event]).unwrap();
    assert_eq!(f.context.same_device_copies[&readback.id].state.depth, 2);
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    callback(&mut f.context, &peer, &callbacks);
    callback(&mut f.context, &readback, &callbacks);
    f.context.release_event(event).unwrap();
    assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
    for completed in [false, true] {
        if completed {
            f.finish(peer.backend_submission);
        }
        validation(
            f.context.write_allocation(f.source, 0, &[99; 64]),
            RuntimeValidationErrorV1::ContextReserved,
        );
    }
    let expected = f.expected();
    f.finish(readback.backend_submission);
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    f.observe(&mut readback);
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
        expected
    );
    assert_eq!(callbacks.lock().unwrap().len(), 2);
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    f.context.write_allocation(f.source, 0, &[99; 64]).unwrap();
    f.context.release_submission(readback).unwrap();
    f.context.destroy_stream(f.stream).unwrap();
    f.context.release_allocation(f.source).unwrap();
    f.context
        .validate_segmented_peer_custody_v1(peer.id)
        .unwrap();
    f.context.release_submission(peer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn settled_segments_feed_read_only_compute_aliases_without_source_producer() {
    let mut f = Settled::new(true, true);
    let kernel = f.kernel();
    let output = f
        .context
        .allocate(
            f.context.devices()[1].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            64,
            16,
        )
        .unwrap();
    f.context.write_allocation(output, 0, &[17; 64]).unwrap();
    let peer = f.list(&[]).unwrap();
    let event = f.context.record_event(&peer).unwrap();
    let args = MixedArguments(vec![
        span(f.destination, RuntimeAccessV1::Read, 0, 64),
        span(f.destination, RuntimeAccessV1::Read, 49, 8),
        span(output, RuntimeAccessV1::Write, 0, 64),
    ]);
    let mut consumer = f
        .context
        .launch_producer_aware_v1(f.stream, &kernel, &args, geometry(), &[event])
        .unwrap();
    assert_eq!(f.context.producer_launches[&consumer.id].state.depth, 2);
    f.context.release_event(event).unwrap();
    f.finish(peer.backend_submission);
    f.finish(consumer.backend_submission);
    assert_eq!(
        f.context.query_submission(&peer).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    f.observe(&mut consumer);
    let reads: Vec<_> = f
        .context
        .backend
        .observed_kernel_reads
        .iter()
        .filter(|read| read.submission == consumer.backend_submission)
        .collect();
    assert_eq!(reads.len(), 2);
    assert_eq!(reads[0].bytes, f.expected());
    assert_eq!(reads[1].bytes, vec![5; 8]);
    f.context.release_submission(consumer).unwrap();
    f.context.release_submission(peer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn settled_segments_capability_and_journal_are_independent_of_pending_compute_opt_in() {
    for (journal, capability) in [(true, false), (false, true), (false, false)] {
        let mut f = Settled::new(journal, capability);
        f.context.backend.pending_compute_segments = true;
        let mut peer = f.list(&[]).unwrap();
        assert!(!f.context.submissions[&peer.id].segmented_peer_copy);
        assert!(f.context.segmented_peer_copies.is_empty());
        if journal {
            let event = f.context.record_event(&peer).unwrap();
            validation(
                f.readback(&[event]),
                RuntimeValidationErrorV1::ContextReserved,
            );
            f.context.release_event(event).unwrap();
        }
        f.finish(peer.backend_submission);
        f.observe(&mut peer);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn settled_segments_do_not_reclassify_pending_or_unknown_source_as_initialized() {
    for unknown in [false, true] {
        let mut f = Fixture::new();
        f.context.backend.peer_segments_frame = true;
        if unknown {
            f.context
                .backend
                .producer_launch
                .observations
                .insert(f.producer.backend_submission, Observation::Quiescent);
            assert!(matches!(
                f.context.poll(&mut f.producer),
                Err(RuntimeErrorV1::BackendQuiescent(_))
            ));
        }
        let before = f.context.backend.copy_call_count;
        for events in [vec![], vec![f.event]] {
            assert!(submit(&mut f, &descriptors(), &events).is_err());
            assert_eq!(f.context.backend.copy_call_count, before);
            assert!(f.context.segmented_peer_copies.is_empty());
        }
        if unknown {
            assert!(f.context.cleanup().is_complete());
        } else {
            f.cancel_producer();
        }
    }
}

#[test]
fn settled_segments_require_exact_pending_list_event_and_read_only_frame_aliases() {
    let mut f = Settled::new(true, true);
    let kernel = f.kernel();
    let mut peer = f.list(&[]).unwrap();
    let event = f.context.record_event(&peer).unwrap();
    for events in [vec![], vec![event, event]] {
        let before = f.context.backend.copy_call_count;
        assert!(f.readback(&events).is_err());
        assert_eq!(f.context.backend.copy_call_count, before);
    }
    for access in [RuntimeAccessV1::Write, RuntimeAccessV1::ReadWrite] {
        let before = f.context.backend.submit_count;
        assert!(
            f.context
                .launch_producer_aware_v1(
                    f.stream,
                    &kernel,
                    &MixedArguments(vec![
                        span(f.destination, RuntimeAccessV1::Read, 0, 64),
                        span(f.destination, access, 49, 8)
                    ]),
                    geometry(),
                    &[event]
                )
                .is_err()
        );
        assert_eq!(f.context.backend.submit_count, before);
    }
    f.finish(peer.backend_submission);
    f.observe(&mut peer);
    f.context
        .write_allocation(f.destination, 0, &[93; 64])
        .unwrap();
    let mut readback = f.readback(&[event]).unwrap();
    assert!(!f.context.same_device_copies.contains_key(&readback.id));
    assert!(
        f.context.submissions[&readback.id]
            .journal_producer_read
            .is_none()
    );
    f.finish(readback.backend_submission);
    f.observe(&mut readback);
    assert_eq!(
        f.context.backend.memory[&f.context.allocations[&f.host].backend_allocation],
        vec![93; 64]
    );
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn settled_segments_control_results_retain_real_depth_and_pending_controls_stay_legacy() {
    for completed in [false, true] {
        let mut f = Settled::new(true, true);
        let control_source = f
            .context
            .allocate(
                f.context.devices()[1].id(),
                RuntimeMemoryKindV1::HostVisible,
                8,
                8,
            )
            .unwrap();
        let control_dest = f
            .context
            .allocate(
                f.context.devices()[1].id(),
                RuntimeMemoryKindV1::HostVisible,
                8,
                8,
            )
            .unwrap();
        f.context
            .write_allocation(control_source, 0, &[11; 8])
            .unwrap();
        f.context
            .write_allocation(control_dest, 0, &[12; 8])
            .unwrap();
        let mut control = f
            .context
            .copy_async(
                f.stream,
                span(control_source, RuntimeAccessV1::Read, 0, 8),
                span(control_dest, RuntimeAccessV1::Write, 0, 8),
                &[],
            )
            .unwrap();
        let event = f.context.record_event(&control).unwrap();
        if completed {
            f.finish(control.backend_submission);
            f.observe(&mut control);
        }
        let mut peer = f.list(&[event]).unwrap();
        assert_eq!(
            f.context.submissions[&peer.id].segmented_peer_copy,
            completed
        );
        if completed {
            assert_eq!(f.context.segmented_peer_copies[&peer.id].state.depth, 2);
            assert_eq!(f.context.submissions[&control.id].dependency_retains, 1);
        }
        f.context.release_event(event).unwrap();
        f.finish(control.backend_submission);
        f.observe(&mut control);
        f.finish(peer.backend_submission);
        f.observe(&mut peer);
        assert_eq!(f.context.submissions[&control.id].dependency_retains, 0);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn settled_segments_exhausted_frame_depth_preserves_legacy_transfer() {
    let mut f = Settled::new(true, true);
    let mut previous: Option<(List, RuntimeEventIdV1)> = None;
    for depth in 1..=MAX_RUNTIME_DEPENDENCIES_V1 + 1 {
        let events = previous
            .as_ref()
            .map(|(_, event)| vec![*event])
            .unwrap_or_default();
        let mut peer = f.list(&events).unwrap();
        if depth <= MAX_RUNTIME_DEPENDENCIES_V1 {
            assert_eq!(f.context.segmented_peer_copies[&peer.id].state.depth, depth);
        } else {
            assert!(!f.context.segmented_peer_copies.contains_key(&peer.id));
            assert!(!f.context.submissions[&peer.id].segmented_peer_copy);
        }
        f.finish(peer.backend_submission);
        f.observe(&mut peer);
        if let Some((prior, event)) = previous.take() {
            f.context.release_event(event).unwrap();
            f.context.release_submission(prior).unwrap();
        }
        let event = f.context.record_event(&peer).unwrap();
        previous = Some((peer, event));
    }
    let (peer, event) = previous.unwrap();
    f.context.release_event(event).unwrap();
    f.context.release_submission(peer).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn settled_segments_cancelled_or_failed_list_never_promotes_queued_readback() {
    for cancelled in [false, true] {
        let mut f = Settled::new(true, true);
        let before = state(&f.context, f.destination).content_lineage;
        let mut peer = f.list(&[]).unwrap();
        let event = f.context.record_event(&peer).unwrap();
        let mut readback = f.readback(&[event]).unwrap();
        f.context.release_event(event).unwrap();
        if cancelled {
            f.context.backend.cancel_before_publication = true;
            assert_eq!(
                f.context.cancel(&mut peer).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
        } else {
            f.context
                .backend
                .producer_launch
                .observations
                .insert(peer.backend_submission, Observation::Failed);
            assert_eq!(
                f.context.poll(&mut peer).unwrap(),
                RuntimePollV1::Failed { code: 7 }
            );
        }
        assert_eq!(state(&f.context, f.destination).content_lineage, before);
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
        assert_eq!(f.context.submissions[&peer.id].dependency_retains, 0);
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn settled_segments_unknown_and_uncertain_faults_preserve_the_right_custody() {
    for fault in [
        Observation::Quiescent,
        Observation::Rejected,
        Observation::Terminal,
        Observation::Panic,
    ] {
        let mut f = Settled::new(true, true);
        let peer = f.list(&[]).unwrap();
        let event = f.context.record_event(&peer).unwrap();
        let mut readback = f.readback(&[event]).unwrap();
        f.context.release_event(event).unwrap();
        f.finish(peer.backend_submission);
        f.finish(readback.backend_submission);
        assert_eq!(
            f.context.poll(&mut readback).unwrap(),
            RuntimePollV1::Pending
        );
        f.context
            .backend
            .producer_launch
            .observations
            .insert(peer.backend_submission, fault);
        let result = catch_unwind(AssertUnwindSafe(|| f.context.poll(&mut readback)));
        if matches!(fault, Observation::Quiescent) {
            assert!(matches!(
                result,
                Ok(Err(RuntimeErrorV1::BackendQuiescent(_)))
            ));
            assert!(!f.context.is_terminal());
            assert_eq!(
                f.context.submissions[&peer.id].status,
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            );
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(f.context.cleanup().is_complete());
        } else {
            assert!(matches!(result, Err(_) | Ok(Err(_))));
            assert!(f.context.is_terminal());
            assert_eq!(f.context.submissions[&peer.id].dependency_retains, 1);
            assert!(f.context.segmented_peer_copies[&peer.id].dependencies_held);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(2));
            assert!(!f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn settled_segments_admission_refunds_only_certified_preeffect_failures() {
    for failure in [
        MockMemoryFailure::Rejected,
        MockMemoryFailure::Quiescent,
        MockMemoryFailure::Terminal,
        MockMemoryFailure::Panic,
    ] {
        let mut f = Settled::new(true, true);
        let lineage = state(&f.context, f.destination).content_lineage;
        f.context.backend.copy_failure = failure;
        let result = catch_unwind(AssertUnwindSafe(|| f.list(&[])));
        assert!(matches!(result, Err(_) | Ok(Err(_))));
        assert_eq!(state(&f.context, f.destination).content_lineage, lineage);
        if matches!(
            failure,
            MockMemoryFailure::Rejected | MockMemoryFailure::Quiescent
        ) {
            assert!(f.context.segmented_peer_copies.is_empty());
            assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
            assert!(!f.context.is_terminal());
            assert!(f.context.cleanup().is_complete());
        } else {
            assert_eq!(f.context.segmented_peer_copies.len(), 1);
            assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
            assert!(f.context.is_terminal());
            assert!(!f.context.cleanup().is_complete());
        }
    }
}

#[test]
fn settled_segments_validate_read_incarnation_origin_and_depth_before_observation() {
    for mode in 0..5 {
        let mut f = Settled::new(true, true);
        let mut peer = f.list(&[]).unwrap();
        match mode {
            0 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_submission_read_reference_for_test_v1(peer.id, 0),
            1 => f
                .context
                .versions
                .as_mut()
                .unwrap()
                .remove_submission_readers_for_test_v1(peer.id),
            2 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .state
                    .depth = 2
            }
            3 => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .source
                    .region
                    .byte_offset += 1
            }
            _ => {
                f.context
                    .segmented_peer_copies
                    .get_mut(&peer.id)
                    .unwrap()
                    .backend_stream += 1
            }
        }
        let before = f.context.backend.poll_call_count;
        assert!(f.context.poll(&mut peer).is_err());
        assert_eq!(f.context.backend.poll_call_count, before);
        assert!(f.context.is_terminal());
    }
    let mut f = fixture();
    f.context.backend.peer_segments_frame = true;
    let mut peer = list(&mut f);
    f.context
        .segmented_peer_copies
        .get_mut(&peer.id)
        .unwrap()
        .origin = SegmentedPeerSourceV1::Settled;
    let before = f.context.backend.poll_call_count;
    assert!(f.context.poll(&mut peer).is_err());
    assert_eq!(f.context.backend.poll_call_count, before);
    assert!(f.context.is_terminal());
}
