use super::*;
use crate::{
    RuntimeAccessV1, RuntimeMemoryRegionV1, RuntimePeerCopySegmentV1,
    RuntimePeerCopySegmentsBackendV1, RuntimeValidationErrorV1,
};

impl RuntimePeerCopySegmentsBackendV1 for MockBackend {
    fn peer_copy_segments_v1(
        &mut self,
        _stream: u64,
        _source: BackendMemoryRegionV1,
        _destination: BackendMemoryRegionV1,
        segments: &[RuntimePeerCopySegmentV1],
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        let handle = self.next();
        let mut state = self.state.lock().unwrap();
        state
            .peer_segment_issues
            .push((handle, segments.to_vec(), dependencies.to_vec()));
        state.statuses.insert(handle, BackendPollV1::Pending);
        Ok(handle)
    }
}

const SEGMENT: RuntimePeerCopySegmentV1 = RuntimePeerCopySegmentV1 {
    source_offset: 0,
    destination_offset: 0,
    byte_len: 4,
};

struct PeerFixture {
    h: Harness,
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
}

impl PeerFixture {
    fn new(bytes: usize, capacity: usize) -> Self {
        Self::from_harness(Harness::new_with_peers(bytes, capacity, true))
    }

    fn from_harness(mut h: Harness) -> Self {
        let devices = [h.context.devices()[0].id(), h.context.devices()[1].id()];
        let source = h
            .context
            .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 64, 8)
            .unwrap();
        let destination = h
            .context
            .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 8)
            .unwrap();
        let stream = h.context.create_stream(devices[1]).unwrap();
        Self {
            h,
            stream,
            source: RuntimeMemoryRegionV1 {
                allocation: source,
                byte_offset: 4,
                byte_len: 32,
                access: RuntimeAccessV1::Read,
            },
            destination: RuntimeMemoryRegionV1 {
                allocation: destination,
                byte_offset: 8,
                byte_len: 48,
                access: RuntimeAccessV1::Write,
            },
        }
    }

    fn dependency(&mut self) -> RuntimeEventIdV1 {
        let submission = self
            .h
            .context
            .launch(self.h.stream, &self.h.kernel, &self.h.args, geometry(), &[])
            .unwrap();
        let event = self.h.context.record_event(&submission).unwrap();
        self.h
            .state
            .lock()
            .unwrap()
            .statuses
            .values_mut()
            .for_each(|s| *s = BackendPollV1::Succeeded);
        event
    }

    fn submit(
        &self,
        tracked: bool,
        segments: Vec<RuntimePeerCopySegmentV1>,
        dependencies: Vec<RuntimeEventIdV1>,
    ) -> Result<
        RuntimeAsyncOperationFutureV1<crate::RuntimePeerCopySegmentsV1, MockError>,
        RuntimeAsyncEngineCallErrorV1,
    > {
        if tracked {
            self.h
                .handle
                .peer_copy_segments_tracked(
                    self.stream,
                    self.source,
                    self.destination,
                    segments,
                    dependencies,
                )
                .map(|tracked| tracked.future)
        } else {
            self.h.handle.peer_copy_segments(
                self.stream,
                self.source,
                self.destination,
                segments,
                dependencies,
            )
        }
    }
}

#[test]
fn versioned_pending_input_refusal_is_an_observed_error_with_no_second_submission() {
    let mut f = PeerFixture::from_harness(Harness::with_journal(4096, 2, true, true));
    let devices = [f.h.context.devices()[0].id(), f.h.context.devices()[1].id()];
    let seed =
        f.h.context
            .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 8)
            .unwrap();
    let producer_stream = f.h.context.create_stream(devices[0]).unwrap();
    let producer =
        f.h.context
            .peer_copy_segments(
                producer_stream,
                RuntimeMemoryRegionV1 {
                    allocation: seed,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 64,
                },
                RuntimeMemoryRegionV1 {
                    access: RuntimeAccessV1::Write,
                    ..f.source
                },
                &[SEGMENT],
                &[],
            )
            .unwrap();
    let event = f.h.context.record_event(&producer).unwrap();
    let future =
        f.h.handle
            .peer_copy_segments_tracked(
                f.stream,
                f.source,
                f.destination,
                vec![SEGMENT],
                vec![event],
            )
            .unwrap();
    let control = future.control();
    assert!(f.h.used() > 0);
    let mut driver = f.h.pop();
    assert!(driver.advance(&mut f.h.context));
    assert_eq!(f.h.used(), 0);
    let result = join_command(future.future).unwrap();
    assert!(result.submission.is_none());
    assert!(matches!(
        result.observation,
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(result.rejected_observations, 0);
    assert!(result.last_rejected_observation.is_none());
    assert_eq!(
        control.phase(),
        RuntimeAsyncOperationPhaseV1::ObservationFinished
    );
    assert_eq!(f.h.state.lock().unwrap().peer_segment_issues.len(), 1);
    assert_eq!(
        f.h.context
            .query_stream(f.stream)
            .unwrap()
            .total_submissions,
        0
    );
    assert_eq!(f.h.context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(f.h.context.version_journal_read_records_v1(), Some(1));
    f.h.state
        .lock()
        .unwrap()
        .statuses
        .values_mut()
        .for_each(|s| *s = BackendPollV1::Succeeded);
    assert!(f.h.context.cleanup().is_complete());
}

#[test]
fn ordered_peer_snapshot_preserves_order_duplicates_dependencies_and_whole_list_completion() {
    for tracked in [false, true] {
        let mut f = PeerFixture::new(4096, 2);
        let event = f.dependency();
        let expected = [
            SEGMENT,
            RuntimePeerCopySegmentV1 {
                source_offset: 12,
                ..SEGMENT
            },
            SEGMENT,
        ];
        let mut segments = Vec::with_capacity(65_536);
        segments.extend(expected);
        let mut dependencies = Vec::with_capacity(65_536);
        dependencies.push(event);
        let future = f.submit(tracked, segments, dependencies).unwrap();
        assert_eq!(
            f.h.used(),
            3 * size_of::<RuntimePeerCopySegmentV1>() + size_of::<RuntimeEventIdV1>()
        );
        let mut operation = f.h.pop();
        assert!(!operation.advance(&mut f.h.context));
        assert_eq!(f.h.used(), 0);
        let mut state = f.h.state.lock().unwrap();
        let (id, observed, dependencies) = &state.peer_segment_issues[0];
        assert_eq!(observed, &expected);
        assert_eq!(dependencies.len(), 1);
        assert!(state.event_sources.contains_key(&dependencies[0]));
        let id = *id;
        assert_eq!(state.release_calls, 0);
        state.statuses.insert(id, BackendPollV1::Succeeded);
        drop(state);
        assert!(operation.advance(&mut f.h.context));
        let result = join_command(future).unwrap();
        assert!(result.submission.is_some());
        assert_eq!(
            result.observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(f.h.state.lock().unwrap().peer_segment_issues.len(), 1);
        assert_eq!(f.h.state.lock().unwrap().release_calls, 0);
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_peer_snapshots_preflight_both_methods_before_enqueue() {
    for tracked in [false, true] {
        let mut f = PeerFixture::new(1024 * 1024, 2);
        let event = f.dependency();
        for (segments, dependencies, error) in [
            (
                vec![SEGMENT],
                vec![event; 2],
                RuntimeAsyncSnapshotErrorV1::DuplicateDependency,
            ),
            (
                vec![SEGMENT],
                vec![event; crate::MAX_RUNTIME_DEPENDENCIES_V1 + 1],
                RuntimeAsyncSnapshotErrorV1::TooManyDependencies,
            ),
            (
                vec![SEGMENT; crate::MAX_RUNTIME_PEER_COPY_SEGMENTS_V1 + 1],
                vec![],
                RuntimeAsyncSnapshotErrorV1::TooManyPeerCopySegments,
            ),
            (
                vec![SEGMENT; crate::MAX_RUNTIME_PEER_COPY_SEGMENTS_V1 + 1],
                vec![event; 2],
                RuntimeAsyncSnapshotErrorV1::DuplicateDependency,
            ),
        ] {
            assert_eq!(
                f.submit(tracked, segments, dependencies).err(),
                Some(RuntimeAsyncEngineCallErrorV1::InvalidSnapshot(error))
            );
            assert_eq!(f.h.used(), 0);
            assert!(f.h.receiver.try_recv().is_err());
            assert!(f.h.state.lock().unwrap().peer_segment_issues.is_empty());
        }
        let segments = vec![SEGMENT; crate::MAX_RUNTIME_PEER_COPY_SEGMENTS_V1];
        let future = f.submit(tracked, segments, vec![]).unwrap();
        assert_eq!(f.h.used(), 4096 * size_of::<RuntimePeerCopySegmentV1>());
        drop(future);
        assert_ne!(f.h.used(), 0);
        drop(f.h.pop_factory());
        assert_eq!(f.h.used(), 0);
    }
}

#[test]
fn ordered_peer_snapshot_charges_combined_exact_budget_and_refunds_failed_enqueue() {
    let bytes = size_of::<RuntimePeerCopySegmentV1>() + size_of::<RuntimeEventIdV1>();
    for tracked in [false, true] {
        let mut f = PeerFixture::new(bytes - 1, 1);
        let event = f.dependency();
        assert_eq!(
            f.submit(tracked, vec![SEGMENT], vec![event]).err(),
            Some(RuntimeAsyncEngineCallErrorV1::SnapshotCapacity)
        );
        assert_eq!(f.h.used(), 0);
        let mut f = PeerFixture::new(bytes, 1);
        let event = f.dependency();
        let future = f.submit(tracked, vec![SEGMENT], vec![event]).unwrap();
        assert_eq!(f.h.used(), bytes);
        drop(future);
        assert_eq!(f.h.used(), bytes);
        drop(f.h.pop_factory());
        assert_eq!(f.h.used(), 0);
        f.h.handle
            .observer
            .sender
            .try_send(RuntimeAsyncEngineCommandV1::Stop)
            .unwrap_or_else(|_| panic!("empty channel"));
        assert_eq!(
            f.submit(tracked, vec![SEGMENT], vec![event]).err(),
            Some(RuntimeAsyncEngineCallErrorV1::CommandQueueFull)
        );
        assert_eq!(f.h.used(), 0);
        drop(f.h.receiver.try_recv().unwrap());
        let future = f.submit(tracked, vec![SEGMENT], vec![event]).unwrap();
        drop(f.h.receiver);
        assert_eq!(f.h.handle.observer().snapshot_bytes_in_use(), 0);
        assert!(matches!(
            join_command(future),
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        ));
    }
}

#[test]
fn ordered_peer_tracked_cancellation_retains_credit_until_owner_disposal() {
    let mut f = PeerFixture::new(4096, 1);
    let tracked =
        f.h.handle
            .peer_copy_segments_tracked(f.stream, f.source, f.destination, vec![SEGMENT], vec![])
            .unwrap();
    tracked.control().cancel_before_submission();
    assert_eq!(f.h.used(), size_of::<RuntimePeerCopySegmentV1>());
    assert!(f.h.pop().advance(&mut f.h.context));
    assert_eq!(f.h.used(), 0);
    assert!(f.h.state.lock().unwrap().peer_segment_issues.is_empty());
    assert!(matches!(
        join_command(tracked.future),
        Err(RuntimeAsyncEngineCallErrorV1::CancelledBeforeSubmission)
    ));
}

#[test]
fn ordered_peer_invalid_lists_are_revalidated_by_owner_and_refund_credit() {
    for tracked in [false, true] {
        for (segments, expected) in [
            (vec![], RuntimeValidationErrorV1::Capacity),
            (
                vec![RuntimePeerCopySegmentV1 {
                    byte_len: 0,
                    ..SEGMENT
                }],
                RuntimeValidationErrorV1::InvalidRange,
            ),
            (
                vec![RuntimePeerCopySegmentV1 {
                    source_offset: u64::MAX,
                    ..SEGMENT
                }],
                RuntimeValidationErrorV1::InvalidRange,
            ),
        ] {
            let mut f = PeerFixture::new(4096, 1);
            let future = f.submit(tracked, segments, vec![]).unwrap();
            assert!(f.h.pop().advance(&mut f.h.context));
            assert_eq!(f.h.used(), 0);
            let result = join_command(future).unwrap();
            assert!(result.submission.is_none());
            assert!(
                matches!(result.observation, Err(RuntimeErrorV1::Validation(e)) if e == expected)
            );
            assert!(f.h.state.lock().unwrap().peer_segment_issues.is_empty());
        }
        let mut f = PeerFixture::new(4096, 1);
        let future = f.submit(tracked, vec![], vec![]).unwrap();
        f.h.context.destroy_stream(f.stream).unwrap();
        assert!(f.h.pop().advance(&mut f.h.context));
        assert!(matches!(
            join_command(future).unwrap().observation,
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownStream
            ))
        ));
        assert_eq!(f.h.used(), 0);
    }
}

#[test]
fn ordered_peer_snapshot_reentrant_rejection_precedes_payload_checks() {
    let f = PeerFixture::new(1, 1);
    f.h.handle
        .observer
        .worker_thread
        .set(thread::current().id())
        .unwrap();
    for tracked in [false, true] {
        assert_eq!(
            f.submit(tracked, vec![SEGMENT; 4097], vec![]).err(),
            Some(RuntimeAsyncEngineCallErrorV1::ReentrantCall)
        );
        assert_eq!(f.h.used(), 0);
        assert!(f.h.receiver.try_recv().is_err());
    }
}

#[test]
fn dropping_ordered_peer_future_does_not_cancel_owner_progress() {
    for tracked in [false, true] {
        let mut f = PeerFixture::new(4096, 1);
        drop(f.submit(tracked, vec![SEGMENT], vec![]).unwrap());
        assert_eq!(f.h.used(), size_of::<RuntimePeerCopySegmentV1>());
        let mut operation = f.h.pop();
        assert!(!operation.advance(&mut f.h.context));
        assert_eq!(f.h.used(), 0);
        let mut state = f.h.state.lock().unwrap();
        assert_eq!(state.peer_segment_issues.len(), 1);
        let id = state.peer_segment_issues[0].0;
        state.statuses.insert(id, BackendPollV1::Succeeded);
        drop(state);
        assert!(operation.advance(&mut f.h.context));
        assert!(f.h.context.cleanup().is_complete());
    }
}

#[test]
fn ordered_peer_copy_uses_background_owner_progress_for_both_methods() {
    for tracked in [false, true] {
        let f = PeerFixture::new(4096, 1);
        let state = f.h.state.clone();
        let (engine, handle) = RuntimeAsyncEngineV1::spawn_with_progress(
            f.h.context,
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        )
        .unwrap();
        let future = if tracked {
            handle
                .peer_copy_segments_tracked(
                    f.stream,
                    f.source,
                    f.destination,
                    vec![SEGMENT; 3],
                    vec![],
                )
                .unwrap()
                .future
        } else {
            handle
                .peer_copy_segments(f.stream, f.source, f.destination, vec![SEGMENT; 3], vec![])
                .unwrap()
        };
        wait_until(|| !state.lock().unwrap().peer_segment_issues.is_empty());
        {
            let mut state = state.lock().unwrap();
            let id = state.peer_segment_issues[0].0;
            state.statuses.insert(id, BackendPollV1::Succeeded);
        }
        assert_eq!(
            join_command(future).unwrap().observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
        let mut context = engine.into_context().unwrap();
        let state = state.lock().unwrap();
        assert_eq!(state.peer_segment_issues.len(), 1);
        assert_eq!(state.poll_threads.len(), 1);
        assert!(!state.poll_threads.contains(&thread::current().id()));
        drop(state);
        assert!(context.cleanup().is_complete());
    }
}
