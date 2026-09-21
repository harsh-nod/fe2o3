use super::async_journal_tests::{state, writer_state};
use super::*;
use fe2o3_runtime_model::ContextWriterStateV1;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug)]
pub(super) struct PendingSegments {
    pub(super) source: BackendMemoryRegionV1,
    pub(super) destination: BackendMemoryRegionV1,
    pub(super) segments: Vec<RuntimePeerCopySegmentV1>,
}

impl RuntimePeerCopySegmentsBackendV1 for MockBackend {
    fn peer_copy_segments_v1(
        &mut self,
        _stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        segments: &[RuntimePeerCopySegmentV1],
        dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.copy_call_count += 1;
        self.last_dependency_count = dependencies.len();
        let failure = core::mem::take(&mut self.copy_failure);
        mock_memory_failure_v1(failure)?;
        let id = self.handle(MockHandleKind::Submission);
        self.polls.insert(id, 0);
        self.pending_peer_segments.insert(
            id,
            PendingSegments {
                source,
                destination,
                segments: segments.to_vec(),
            },
        );
        Ok(id)
    }
}

struct Fixture {
    context: RuntimeContextV1<MockBackend>,
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
}

impl Fixture {
    fn new() -> Self {
        let mut context = RuntimeContextV1::open_with_version_journal_v1(
            MockBackend {
                next: 100,
                ..MockBackend::default()
            },
            16,
            8,
        )
        .unwrap();
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let source = context
            .allocate(devices[0], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let destination = context
            .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let stream = context.create_stream(devices[1]).unwrap();
        context
            .write_allocation(source, 0, &(0..64).collect::<Vec<u8>>())
            .unwrap();
        context
            .write_allocation(destination, 0, &[0xa5; 64])
            .unwrap();
        Self {
            context,
            stream,
            source: RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 4,
                byte_len: 16,
            },
            destination: RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 8,
                byte_len: 24,
            },
        }
    }

    fn submit(
        &mut self,
        segments: &[RuntimePeerCopySegmentV1],
    ) -> Result<RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>, RuntimeErrorV1<MockError>> {
        self.context
            .peer_copy_segments(self.stream, self.source, self.destination, segments, &[])
    }

    fn bytes(&self, region: RuntimeMemoryRegionV1) -> &[u8] {
        &self.context.backend.memory
            [&self.context.allocations[&region.allocation].backend_allocation]
    }
}

fn segments() -> Vec<RuntimePeerCopySegmentV1> {
    vec![
        RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 0,
            byte_len: 8,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 8,
            destination_offset: 4,
            byte_len: 8,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 1,
            destination_offset: 18,
            byte_len: 3,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 8,
            destination_offset: 4,
            byte_len: 8,
        },
    ]
}

#[test]
fn pending_versioned_producer_refuses_reader_before_backend_then_completed_event_admits_it() {
    let mut f = Fixture::new();
    f.context.backend.deferred_copies = true;
    let devices = [f.context.devices()[0].id(), f.context.devices()[1].id()];
    let seed = f
        .context
        .allocate(devices[1], RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let input = (0..64).map(|i| 64 - i).collect::<Vec<u8>>();
    f.context.write_allocation(seed, 0, &input).unwrap();
    let producer_stream = f.context.create_stream(devices[0]).unwrap();
    let mut producer = f
        .context
        .peer_copy(
            producer_stream,
            RuntimeMemoryRegionV1 {
                allocation: seed,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            RuntimeMemoryRegionV1 {
                allocation: f.source.allocation,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 64,
            },
            &[],
        )
        .unwrap();
    let event = f.context.record_event(&producer).unwrap();
    assert_eq!(f.context.backend.copy_call_count, 1);
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert!(matches!(
        f.context
            .peer_copy_segments(f.stream, f.source, f.destination, &segments(), &[event]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(f.context.backend.copy_call_count, 1);
    assert!(f.context.backend.pending_peer_segments.is_empty());
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        f.context.query_stream(f.stream).unwrap().total_submissions,
        0
    );
    assert_eq!(f.context.query_stream(producer_stream).unwrap().pending, 1);
    assert_eq!(f.bytes(f.destination), &[0xa5; 64]);

    assert_eq!(
        f.context
            .wait(&mut producer, Duration::from_secs(1))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    let mut consumer = f
        .context
        .peer_copy_segments(f.stream, f.source, f.destination, &segments(), &[event])
        .unwrap();
    assert_eq!(f.context.backend.copy_call_count, 2);
    assert_eq!(f.context.backend.last_dependency_count, 1);
    assert_eq!(
        f.context
            .wait(&mut consumer, Duration::from_secs(1))
            .unwrap(),
        RuntimePollV1::Succeeded
    );
    let mut expected = vec![0xa5; 64];
    for segment in segments() {
        let from = 4 + segment.source_offset as usize;
        let to = 8 + segment.destination_offset as usize;
        let bytes = segment.byte_len as usize;
        expected[to..to + bytes].copy_from_slice(&input[from..from + bytes]);
    }
    assert_eq!(f.bytes(f.source), input);
    assert_eq!(f.bytes(f.destination), expected);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn ordered_overlaps_duplicates_canaries_and_snapshot_settle_one_writer_once() {
    let mut f = Fixture::new();
    let mut descriptors = segments();
    let mut submission = f.submit(&descriptors).unwrap();
    descriptors.reverse();
    descriptors[0].byte_len = 0;
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        writer_state(&f.context, f.destination.allocation),
        ContextWriterStateV1::Pending { member_count: 1 }
    );
    assert_eq!(
        f.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Pending
    );
    assert_eq!(f.bytes(f.destination), [0xa5; 64]);
    for source in [true, false] {
        let allocation = if source {
            f.source.allocation
        } else {
            f.destination.allocation
        };
        assert!(matches!(
            f.context.write_allocation(allocation, 0, &[1]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert!(matches!(
            f.context.release_allocation(allocation),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
    }
    let callbacks = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&callbacks);
    f.context
        .on_completion(&submission, move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    let event = f.context.record_event(&submission).unwrap();
    assert_eq!(
        f.context.wait_event(event, Duration::ZERO).unwrap(),
        RuntimeCompletionStatusV1::Succeeded
    );
    assert_eq!(
        f.context.poll(&mut submission).unwrap(),
        RuntimePollV1::Succeeded
    );
    assert_eq!(callbacks.load(Ordering::SeqCst), 1);
    let mut expected = [0xa5; 64];
    expected[8..16].copy_from_slice(&(4..12).collect::<Vec<_>>());
    expected[12..20].copy_from_slice(&(12..20).collect::<Vec<_>>());
    expected[26..29].copy_from_slice(&[5, 6, 7]);
    assert_eq!(f.bytes(f.destination), expected);
    assert_eq!(f.bytes(f.source), (0..64).collect::<Vec<_>>());
    assert_eq!(
        state(&f.context, f.destination.allocation).content_lineage,
        2
    );
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn invalid_last_descriptor_is_rejected_before_backend_or_journal_effects() {
    let mut f = Fixture::new();
    let before_source = state(&f.context, f.source.allocation);
    let before_destination = state(&f.context, f.destination.allocation);
    for bad in [
        RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 0,
            byte_len: 0,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 16,
            destination_offset: 0,
            byte_len: 1,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: 24,
            byte_len: 1,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: u64::MAX,
            destination_offset: 0,
            byte_len: 1,
        },
        RuntimePeerCopySegmentV1 {
            source_offset: 0,
            destination_offset: u64::MAX,
            byte_len: 1,
        },
    ] {
        let mut descriptors = segments();
        descriptors.push(bad);
        assert!(matches!(
            f.submit(&descriptors),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidRange
            ))
        ));
    }
    for descriptors in [
        vec![],
        vec![segments()[0]; MAX_RUNTIME_PEER_COPY_SEGMENTS_V1 + 1],
    ] {
        assert!(matches!(
            f.submit(&descriptors),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::Capacity
            ))
        ));
    }
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert_eq!(state(&f.context, f.source.allocation), before_source);
    assert_eq!(
        state(&f.context, f.destination.allocation),
        before_destination
    );
    assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    assert_eq!(f.context.version_journal_writer_records_v1(), Some(0));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn descriptor_limit_is_not_the_aggregate_ring_limit() {
    for count in [1, 63, 64, 65, MAX_RUNTIME_PEER_COPY_SEGMENTS_V1] {
        let mut f = Fixture::new();
        let mut submission = f.submit(&vec![segments()[0]; count]).unwrap();
        assert_eq!(f.context.version_journal_writer_records_v1(), Some(1));
        assert_eq!(f.context.version_journal_read_records_v1(), Some(1));
        assert_eq!(
            f.context.wait(&mut submission, Duration::ZERO).unwrap(),
            RuntimePollV1::Succeeded
        );
        assert_eq!(f.context.backend.copy_call_count, 1);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn cancellation_before_publication_restores_lineage_but_partial_failure_does_not() {
    for cancelled in [false, true] {
        let mut f = Fixture::new();
        let before = state(&f.context, f.destination.allocation);
        let mut submission = f.submit(&segments()).unwrap();
        if cancelled {
            f.context.backend.cancel_before_publication = true;
            let _ = f.context.cancel(&mut submission).unwrap();
            let after = state(&f.context, f.destination.allocation);
            assert_eq!(after.content_lineage, before.content_lineage);
            assert_eq!(after.attempt_epoch, before.attempt_epoch + 1);
            assert!(after.pending_writer.is_none());
        } else {
            let source = f.context.allocations[&f.source.allocation].backend_allocation;
            let destination = f.context.allocations[&f.destination.allocation].backend_allocation;
            f.context.backend.apply_copy(
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 4,
                    byte_len: 8,
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 8,
                    byte_len: 8,
                },
            );
            f.context.backend.first_wait_failure = MockWaitFailure::QuiescentFirst;
            assert!(matches!(
                f.context.wait(&mut submission, Duration::ZERO),
                Err(RuntimeErrorV1::BackendQuiescent(_))
            ));
            assert_eq!(
                writer_state(&f.context, f.destination.allocation),
                ContextWriterStateV1::Unknown { member_count: 1 }
            );
            assert_ne!(f.bytes(f.destination), [0xa5; 64]);
        }
        assert_eq!(f.context.version_journal_read_records_v1(), Some(0));
    }
}

#[test]
fn identity_binds_order_count_envelopes_and_branded_stream() {
    let mut f = Fixture::new();
    let identity = |stream, source, destination, segments: &[RuntimePeerCopySegmentV1]| {
        super::super::peer_segments::contract_identity(stream, source, destination, segments)
    };
    let descriptors = segments();
    let original = identity(f.stream, f.source, f.destination, &descriptors);
    let mut reversed = descriptors.clone();
    reversed.reverse();
    assert_ne!(
        original,
        identity(f.stream, f.source, f.destination, &reversed)
    );
    assert_ne!(
        original,
        identity(f.stream, f.source, f.destination, &descriptors[..3])
    );
    let source = RuntimeMemoryRegionV1 {
        byte_len: 17,
        ..f.source
    };
    assert_ne!(
        original,
        identity(f.stream, source, f.destination, &descriptors)
    );
    let device = f.context.devices()[1].id();
    let stream = f.context.create_stream(device).unwrap();
    assert_ne!(
        original,
        identity(stream, f.source, f.destination, &descriptors)
    );
    assert_ne!(
        original,
        peer_copy_contract_identity(f.stream, f.source, f.destination)
    );
    assert!(f.context.cleanup().is_complete());
}
