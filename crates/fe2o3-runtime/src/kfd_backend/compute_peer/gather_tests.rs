//! Ordered byte/custody composition with actual scripted owners and child compute
//! settlement. Scripted compute preserves initial bytes; it does not calculate them.

use super::window::{initialize_bytes, restored};
use super::*;

const DESTINATION_BYTES: usize = 96;

struct Gather {
    f: Fixture,
    producers: [u64; 2],
    producer_events: [u64; 2],
    peers: Vec<u64>,
    peer_events: Vec<u64>,
    source_ids: [u64; 2],
    source_bytes: [Vec<u8>; 2],
    destination_id: u64,
    expected: Vec<u8>,
}

impl Gather {
    fn new(readback: bool) -> Self {
        let mut f = Fixture::with_layout(
            [BYTES, DESTINATION_BYTES],
            readback.then_some((0, 0, DESTINATION_BYTES)),
            false,
        );
        let a = f.allocations[0][2];
        let b = f.allocations[0][6];
        let destination = f.allocations[1][3];
        let (a_id, a_bytes) = initialize_bytes(&mut f, a, 0x31);
        let (b_id, b_bytes) = initialize_bytes(&mut f, b, 0x75);
        let (destination_id, expected) = initialize_bytes(&mut f, destination, 0xc3);
        let producers = [f.launch(0, 0, true, true), f.launch(0, 4, true, true)];
        let producer_events = producers.map(|id| f.event(0, id));
        Self {
            f,
            producers,
            producer_events,
            peers: Vec::new(),
            peer_events: Vec::new(),
            source_ids: [a_id, b_id],
            source_bytes: [a_bytes, b_bytes],
            destination_id,
            expected,
        }
    }

    fn regions(
        &self,
        producer: usize,
        source: u64,
        destination: u64,
        bytes: u64,
    ) -> [BackendMemoryRegionV1; 2] {
        [
            BackendMemoryRegionV1 {
                byte_offset: source,
                byte_len: bytes,
                ..region(
                    self.f.allocations[0][[2, 6][producer]],
                    RuntimeAccessV1::Read,
                )
            },
            BackendMemoryRegionV1 {
                byte_offset: destination,
                byte_len: bytes,
                ..region(self.f.allocations[1][3], RuntimeAccessV1::Write)
            },
        ]
    }

    fn admit(&mut self, producer: usize, source: u64, destination: u64, bytes: u64) {
        let regions = self.regions(producer, source, destination, bytes);
        let mut dependencies = vec![self.producer_events[producer]];
        dependencies.extend(self.peer_events.last().copied());
        let peer = self
            .f
            .backend
            .peer_copy_v1(self.f.peer_stream, regions[0], regions[1], &dependencies)
            .unwrap();
        assert!(self.f.copy(peer).compute_producer.is_some());
        assert!(self.f.copy(peer).compute_xgmi.is_some());
        assert!(self.f.copy(peer).staging.is_empty());
        self.peers.push(peer);
        self.peer_events.push(
            self.f
                .backend
                .record_event_v1(self.f.peer_stream, peer)
                .unwrap(),
        );
        self.expected[destination as usize..(destination + bytes) as usize].copy_from_slice(
            &self.source_bytes[producer][source as usize..(source + bytes) as usize],
        );
    }

    fn chain(&mut self, overlap: bool) {
        self.admit(0, 3, 8, 32);
        self.admit(1, 7, 48, 32);
        self.admit(
            1,
            17,
            if overlap { 20 } else { 80 },
            if overlap { 24 } else { 16 },
        );
    }

    fn release_events(&mut self) {
        for event in self.peer_events.drain(..).rev().chain(self.producer_events) {
            self.f.backend.release_event_v1(event).unwrap();
        }
    }

    fn assert_ordered_custody(&self) {
        let owners: Vec<_> = self
            .f
            .backend
            .compute_xgmi_children
            .iter()
            .flatten()
            .copied()
            .collect();
        if let Some(first) = owners.first() {
            assert!(owners.iter().all(|id| id == first));
        }
        for (index, id) in self.peers.iter().enumerate() {
            let copy = self.f.copy(*id);
            if !copy.compute_xgmi.as_ref().unwrap().is_quiescent()
                || copy.status() == BackendPollV1::Succeeded
            {
                for earlier in &self.peers[..index] {
                    assert_eq!(self.f.copy(*earlier).status(), BackendPollV1::Succeeded);
                    assert!(self.f.copy(*earlier).is_quiescent());
                }
            }
        }
    }

    fn drive(&mut self, stream: u64, target: u64) {
        for _ in 0..192 {
            self.f.backend.flush_stream_v1(stream).unwrap();
            self.assert_ordered_custody();
            if self.f.backend.poll_v1(target).unwrap() != BackendPollV1::Pending {
                return;
            }
        }
        panic!("ordered gather did not finish under bounded explicit progress");
    }
}

#[test]
fn compute_peer_gather_latest_only_chain_preserves_frame_and_ordered_overlap() {
    for overlap in [false, true] {
        let mut g = Gather::new(false);
        g.chain(overlap);
        let last = *g.peers.last().unwrap();
        let destination = g.f.backend.allocations[&g.f.allocations[1][3]];
        assert_eq!(
            g.f.backend.cooperative_allocation_owners[&destination],
            g.peers
        );
        assert!(!g.f.copy(last).dependencies.contains(&g.peers[0]));
        g.release_events();
        let generation = g.f.backend.cooperative_progress_generation;
        for id in g.peers.clone() {
            assert_eq!(g.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                g.f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                g.f.backend.drain_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(g.f.backend.cooperative_progress_generation, generation);
        assert!(
            g.f.backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        let stream = g.f.peer_stream;
        g.drive(stream, last);
        assert_eq!(
            restored(&g.f, g.f.allocations[1][3]),
            (g.destination_id, g.expected.as_slice())
        );
        for (index, allocation) in [g.f.allocations[0][2], g.f.allocations[0][6]]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                restored(&g.f, allocation),
                (g.source_ids[index], g.source_bytes[index].as_slice())
            );
        }
        assert!(g.f.backend.cooperative_dependency_retain_counts.is_empty());
        assert!(g.f.backend.cooperative_allocation_owners.is_empty());
        assert_eq!(g.f.backend.completed_compute_xgmi_copies, 0);
        g.f.clean();
    }
}

#[test]
fn compute_peer_gather_full_frame_readback_preadmitted_then_drives_only_final_stream() {
    let mut g = Gather::new(true);
    g.chain(true);
    let host = g.f.host.unwrap();
    let readback =
        g.f.backend
            .copy_async_v1(
                g.f.readback_stream,
                BackendMemoryRegionV1 {
                    byte_len: DESTINATION_BYTES as u64,
                    ..region(g.f.allocations[1][3], RuntimeAccessV1::Read)
                },
                BackendMemoryRegionV1 {
                    byte_len: DESTINATION_BYTES as u64,
                    ..region(host, RuntimeAccessV1::Write)
                },
                &[*g.peer_events.last().unwrap()],
            )
            .unwrap();
    assert!(g.f.copy(readback).compute_xgmi.is_none());
    assert_eq!(
        g.f.copy(readback).dependencies,
        vec![*g.peers.last().unwrap()]
    );
    g.release_events();
    assert!(
        g.f.backend
            .release_allocation_v1(g.f.allocations[1][3])
            .is_err()
    );
    assert_eq!(
        g.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        g.f.backend.drain_v1(readback, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert!(
        g.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    let stream = g.f.readback_stream;
    g.drive(stream, readback);
    assert_eq!(
        g.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Succeeded
    );
    let host_route = g.f.backend.allocations[&host];
    let KfdRuntimeSdmaStorageV1::Host(owner) =
        &g.f.backend.children[1].allocations[&host_route.local].sdma_storage
    else {
        panic!("actual initialized host owner must be restored");
    };
    assert_eq!(owner.scripted_bytes().unwrap(), g.expected.as_slice());
    assert_eq!(
        restored(&g.f, g.f.allocations[1][3]),
        (g.destination_id, g.expected.as_slice())
    );
    assert!(g.f.backend.cooperative_dependency_retain_counts.is_empty());
    g.f.clean();
}

#[test]
fn compute_peer_gather_rejects_stale_missing_and_cross_stream_predecessors_without_effects() {
    let mut g = Gather::new(false);
    g.admit(0, 3, 8, 32);
    g.admit(1, 7, 48, 32);
    let other_stream = g.f.backend.create_stream_v1(8).unwrap();
    let request = g.regions(1, 17, 20, 24);
    for (stream, previous) in [
        (g.f.peer_stream, None),
        (g.f.peer_stream, Some(g.peer_events[0])),
        (other_stream, Some(g.peer_events[1])),
    ] {
        let before = (
            g.f.backend.next_handle,
            g.f.backend.submissions.len(),
            g.f.backend.cooperative_staging_bytes,
            g.f.backend.cooperative_dependency_retain_counts.clone(),
            g.f.backend.cooperative_allocation_owners.clone(),
        );
        let mut dependencies = vec![g.producer_events[1]];
        dependencies.extend(previous);
        assert!(matches!(
            g.f.backend
                .peer_copy_v1(stream, request[0], request[1], &dependencies),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(
            (
                g.f.backend.next_handle,
                g.f.backend.submissions.len(),
                g.f.backend.cooperative_staging_bytes,
                g.f.backend.cooperative_dependency_retain_counts.clone(),
                g.f.backend.cooperative_allocation_owners.clone()
            ),
            before
        );
        assert!(!g.f.backend.terminal);
    }
    g.release_events();
    let stream = g.f.peer_stream;
    let last = *g.peers.last().unwrap();
    g.drive(stream, last);
    g.f.clean();
}

#[test]
fn compute_peer_gather_settled_predecessor_does_not_retain_disposed_source() {
    let mut g = Gather::new(false);
    g.chain(true);
    g.release_events();
    let stream = g.f.peer_stream;
    // A stream flush can advance the tail into the next native transfer. Stop
    // at this exact predecessor's restoration before testing source disposal.
    assert_eq!(
        g.f.backend
            .drain_v1(
                g.peers[0],
                Instant::now() + std::time::Duration::from_secs(1),
            )
            .unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(g.f.copy(g.peers[0]).status(), BackendPollV1::Succeeded);
    assert!(g.f.copy(g.peers[0]).is_quiescent());
    assert!(
        g.f.copy(g.peers[0])
            .compute_xgmi
            .as_ref()
            .unwrap()
            .is_quiescent()
    );
    assert_eq!(g.f.copy(g.peers[1]).status(), BackendPollV1::Pending);
    assert_eq!(g.f.copy(g.peers[2]).status(), BackendPollV1::Pending);
    assert!(
        g.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    assert!(g.f.copy(g.peers[0]).compute_producer.is_none());
    assert!(g.f.backend.submission_retained_as_dependency(g.peers[0]));
    let disposed = g.f.allocations[0][2];
    g.f.backend.release_allocation_v1(disposed).unwrap();
    assert!(!g.f.backend.allocations.contains_key(&disposed));
    let last = *g.peers.last().unwrap();
    g.f.backend.validate_compute_peer_v1(last).unwrap();
    g.drive(stream, last);
    assert_eq!(
        restored(&g.f, g.f.allocations[1][3]),
        (g.destination_id, g.expected.as_slice())
    );
    g.f.clean_except(&[disposed]);
}

#[test]
fn compute_peer_gather_cancelled_middle_fails_descendants_without_native_extraction() {
    let mut g = Gather::new(false);
    g.chain(true);
    g.release_events();
    assert_eq!(
        g.f.backend.cancel_v1(g.peers[1]).unwrap(),
        BackendCancellationV1::Cancelled
    );
    for producer in g.producers.into_iter().rev() {
        assert_eq!(
            g.f.backend.cancel_v1(producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
    }
    let last = *g.peers.last().unwrap();
    for _ in 0..32 {
        let result = g.f.backend.flush_stream_v1(g.f.peer_stream);
        assert!(matches!(
            result,
            Ok(_) | Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        if g.f.copy(last).is_quiescent() {
            break;
        }
    }
    assert_eq!(
        g.f.copy(g.peers[1]).phase,
        CooperativeCopyPhaseV1::Cancelled
    );
    assert_eq!(g.f.copy(last).phase, CooperativeCopyPhaseV1::Failed);
    assert!(g.f.copy(g.peers[1]).is_quiescent());
    assert!(g.f.copy(last).is_quiescent());
    // Failing the tail on its canceled producer does not observe an unrelated
    // earlier root. Its own canceled producer must still be reconciled exactly.
    assert_eq!(g.f.copy(g.peers[0]).status(), BackendPollV1::Pending);
    assert!(!g.f.copy(g.peers[0]).is_quiescent());
    assert!(g.f.copy(g.peers[0]).compute_producer.is_some());
    assert!(
        g.f.backend
            .submission_retained_as_dependency(g.producers[0])
    );
    assert!(matches!(
        g.f.backend
            .drain_v1(
                g.peers[0],
                Instant::now() + std::time::Duration::from_secs(1),
            )
            .unwrap(),
        BackendPollV1::Failed { .. }
    ));
    for id in &g.peers {
        assert!(matches!(
            g.f.copy(*id).status(),
            BackendPollV1::Failed { .. }
        ));
        assert!(g.f.copy(*id).is_quiescent());
        assert!(g.f.copy(*id).compute_xgmi.as_ref().unwrap().is_quiescent());
    }
    assert!(!g.f.backend.terminal);
    assert!(g.f.backend.cooperative_dependency_retain_counts.is_empty());
    assert!(
        g.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    g.f.clean();
}

#[test]
fn compute_peer_gather_unknown_rank_and_window_corruption_fail_closed_before_traversal() {
    for case in 0..3 {
        let mut g = Gather::new(false);
        g.chain(true);
        g.release_events();
        let first = g.peers[0];
        let mut _orphan = None;
        if case == 0 {
            _orphan = Some(ManuallyDrop::new(
                g.f.backend.submissions.remove(&first).unwrap(),
            ));
        } else {
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                g.f.backend.submissions.get_mut(&first).unwrap()
            else {
                unreachable!()
            };
            if case == 1 {
                copy.dependency_depth = MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1;
            } else {
                copy.destination_region.byte_offset += 1;
            }
        }
        let last = *g.peers.last().unwrap();
        assert!(matches!(
            g.f.backend
                .drain_v1(last, Instant::now() + std::time::Duration::from_secs(1)),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(g.f.backend.terminal);
        assert!(g.f.backend.children.iter().all(|child| child.terminal));
        assert_eq!(g.f.copy(last).status(), BackendPollV1::Pending);
        assert!(g.f.copy(last).compute_producer.is_some());
        assert!(
            g.f.backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        assert!(
            g.producers
                .iter()
                .all(|id| g.f.backend.submission_retained_as_dependency(*id))
        );
        assert!(
            g.f.backend.children.iter().all(|child| child
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count()
                == ALLOCATIONS)
        );
    }
}

#[test]
fn compute_peer_gather_native_ancestor_failure_retains_every_pending_descendant() {
    for unwind in [false, true] {
        let mut g = Gather::new(false);
        g.f.backend.compute_xgmi_routes.insert(
            (0, 1),
            Route::Scripted {
                failure: Some(Stage::Poll),
                unwind,
                pending_samples: 0,
            },
        );
        g.chain(true);
        g.release_events();
        let last = *g.peers.last().unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..96 {
                g.f.backend.flush_stream_v1(g.f.peer_stream)?;
            }
            Ok::<_, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>(())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(g.f.backend.terminal);
        assert!(g.f.backend.children.iter().all(|child| child.terminal));
        assert_eq!(g.f.copy(last).status(), BackendPollV1::Pending);
        assert!(
            g.peers
                .iter()
                .all(|id| g.f.copy(*id).compute_producer.is_some())
        );
        assert!(
            g.f.backend
                .compute_xgmi_children
                .iter()
                .all(|owner| *owner == Some(g.peers[0]))
        );
        assert!(
            g.producers
                .iter()
                .all(|id| g.f.backend.submission_retained_as_dependency(*id))
        );
        assert!(
            g.f.backend.children.iter().all(|child| child
                .scripted_sdma
                .as_ref()
                .unwrap()
                .live_owner_count()
                == ALLOCATIONS)
        );
    }
}
