//! Late directed admission uses retained native identities, not cloned owners.
//! Scripted byte transitions are CPU regression coverage, not hardware evidence.

use super::*;
use crate::{
    BackendCancellationV1, BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1,
    BackendDirectedScalarPeerCopyV1, RuntimeDirectedScalarPeerCopyBackendV1,
};

struct Late {
    fixture: Fixture,
    allocations: [u64; 3],
    streams: [u64; 3],
    identities: [Option<u64>; 3],
}

impl Late {
    fn new(failure: Option<Stage>, unwind: bool) -> Self {
        let mut fixture = Fixture::configured(failure, unwind, 3, 3);
        fixture.backend.compute_xgmi_routes.remove(&(2, 3));
        for pair in [(0, 2), (1, 2)] {
            fixture.backend.compute_xgmi_routes.insert(
                pair,
                Route::Scripted {
                    failure: None,
                    unwind: false,
                    pending_samples: 3,
                },
            );
        }
        let allocations = std::array::from_fn(|child| {
            *fixture
                .backend
                .allocations
                .iter()
                .find(|(_, route)| route.child == child)
                .unwrap()
                .0
        });
        let streams = [
            fixture.backend.create_stream_v1(7).unwrap(),
            fixture.stream,
            fixture.backend.create_stream_v1(9).unwrap(),
        ];
        let identities = allocations.map(|allocation| {
            let route = fixture.backend.allocations[&allocation];
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &fixture.backend.children[route.child].allocations[&route.local].sdma_storage
            else {
                unreachable!()
            };
            owner.scripted_owner_id()
        });
        Self {
            fixture,
            allocations,
            streams,
            identities,
        }
    }

    fn route(&self, source: usize, destination: usize) -> BackendDirectedPeerRouteV1 {
        BackendDirectedPeerRouteV1 {
            stream: self.streams[destination],
            source_device: 7 + source as u64,
            destination_device: 7 + destination as u64,
            source: BackendMemoryRegionV1 {
                allocation: self.allocations[source],
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
            destination: BackendMemoryRegionV1 {
                allocation: self.allocations[destination],
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: BYTES as u64,
            },
        }
    }

    fn submit(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        dependencies: &[BackendDirectedPeerDependencyV1],
    ) -> u64 {
        self.fixture
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route,
                dependencies,
            })
            .unwrap()
    }

    fn event(&mut self, id: u64) -> BackendDirectedPeerDependencyV1 {
        let stream = self.fixture.copy(id).stream;
        BackendDirectedPeerDependencyV1 {
            event: self.fixture.backend.record_event_v1(stream, id).unwrap(),
            producer_submission: id,
        }
    }

    fn publish(&mut self, id: u64, ready: bool) {
        let target = if ready {
            Phase::Ready
        } else {
            Phase::Published
        };
        for _ in 0..16 {
            assert_eq!(
                self.fixture
                    .backend
                    .progress_retained_directed_peer_v1(id)
                    .unwrap(),
                BackendPollV1::Pending
            );
            if self.fixture.root(id).phase == target {
                return;
            }
        }
        panic!("directed peer did not reach the requested retained phase");
    }

    fn drive(&mut self, id: u64) -> BackendPollV1 {
        let deadline = Instant::now() + Duration::from_secs(1);
        for _ in 0..64 {
            let result = self.fixture.backend.drain_v1(id, deadline).unwrap();
            self.fixture.backend.assert_cooperative_indexes_consistent();
            if result != BackendPollV1::Pending {
                return result;
            }
        }
        panic!("late directed graph did not settle through its final request");
    }

    fn assert_original_owners(&self, values: [u8; 3]) {
        for ((allocation, identity), value) in self
            .allocations
            .into_iter()
            .zip(self.identities)
            .zip(values)
        {
            let route = self.fixture.backend.allocations[&allocation];
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &self.fixture.backend.children[route.child].allocations[&route.local].sdma_storage
            else {
                panic!("exact device owner was not restored")
            };
            assert_eq!(owner.scripted_owner_id(), identity);
            assert_eq!(owner.scripted_bytes().unwrap(), &[value; BYTES]);
        }
    }

    fn clean(self) {
        assert_eq!(self.fixture.backend.compute_xgmi_children, [None; 3]);
        assert_eq!(self.fixture.backend.retained_compute_xgmi_copies_v1(), 0);
        assert_eq!(self.fixture.backend.completed_compute_xgmi_copies_v1(), 0);
        self.fixture.clean();
    }
}

#[test]
fn late_directed_chain_and_shared_source_fanout_keep_native_owners_until_final_drive() {
    for fanout in [false, true] {
        for ready in [false, true] {
            let mut f = Late::new(None, false);
            let first_route = f.route(0, 1);
            let first = f.submit(first_route, &[]);
            assert_eq!(f.fixture.backend.retained_compute_xgmi_copies_v1(), 0);
            f.publish(first, ready);
            let event = (!fanout).then(|| f.event(first));
            let dependencies = event.as_slice();
            let next_route = f.route(usize::from(!fanout), 2);
            let trace = f.fixture.root(first).trace.clone();
            let owners = f
                .fixture
                .root(first)
                .scripted_owners
                .each_ref()
                .map(|owner| owner.as_ref().unwrap().scripted_owner_id());
            let next = f.submit(next_route, dependencies);
            if let Some(event) = event {
                f.fixture.backend.release_event_v1(event.event).unwrap();
            }
            let root = f.fixture.root(next);
            assert_eq!(root.phase, Phase::Prepared);
            assert!(root.is_quiescent());
            assert!(root.scripted_owners.iter().all(Option::is_none));
            assert!(root.owners.iter().all(Option::is_none));
            assert!(root.queue.is_none() && root.creation.is_vacant());
            assert!(root.trace.is_empty());
            assert!(f.fixture.copy(next).staging.is_empty());
            assert_eq!(f.fixture.copy(next).scratch_byte_len, 0);
            assert_eq!(f.fixture.backend.cooperative_staging_bytes, 0);
            assert_eq!(
                f.fixture.backend.compute_xgmi_children,
                [Some(first), Some(first), None]
            );
            for _ in 0..3 {
                assert_eq!(
                    f.fixture.backend.poll_v1(next).unwrap(),
                    BackendPollV1::Pending
                );
                assert_eq!(
                    f.fixture.backend.wait_v1(next, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
                assert_eq!(
                    f.fixture.backend.drain_v1(next, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
                assert_eq!(f.fixture.backend.retained_compute_xgmi_copies_v1(), 0);
                assert_eq!(f.fixture.root(first).trace, trace);
                assert!(f.fixture.root(next).trace.is_empty());
            }
            assert_eq!(
                f.fixture
                    .root(first)
                    .scripted_owners
                    .each_ref()
                    .map(|owner| owner.as_ref().unwrap().scripted_owner_id()),
                owners
            );
            assert_eq!(f.drive(next), BackendPollV1::Succeeded);
            assert_eq!(f.fixture.copy(first).status(), BackendPollV1::Succeeded);
            f.assert_original_owners([0x53; 3]);
            f.clean();
        }
    }
}

#[test]
fn late_directed_unordered_destination_reads_reject_without_touching_published_parent() {
    let mut f = Late::new(None, false);
    let first_route = f.route(0, 1);
    let first = f.submit(first_route, &[]);
    f.publish(first, false);
    let trace = f.fixture.root(first).trace.clone();
    let count = f.fixture.backend.submissions.len();
    let next_route = f.route(1, 2);
    assert!(matches!(
        f.fixture
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: next_route,
                dependencies: &[],
            }),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.fixture.backend.submissions.len(), count);
    assert_eq!(f.fixture.root(first).trace, trace);
    assert!(!f.fixture.backend.terminal);
    assert_eq!(f.drive(first), BackendPollV1::Succeeded);
    f.clean();
}

#[test]
fn late_directed_checked_ranges_preserve_absent_route_and_ordinary_fallback() {
    for mode in 0..3 {
        let mut f = Late::new(None, false);
        let first_route = f.route(0, 1);
        let first = f.submit(first_route, &[]);
        f.publish(first, false);
        let event = f.event(first);
        let mut next_route = f.route(1, 2);
        if mode == 0 {
            next_route.source.byte_len -= 1;
            next_route.destination.byte_len -= 1;
        } else if mode == 1 {
            f.fixture.backend.compute_xgmi_routes.remove(&(1, 2));
        }
        let next = if mode == 2 {
            f.fixture
                .backend
                .peer_copy_v1(
                    next_route.stream,
                    next_route.source,
                    next_route.destination,
                    &[event.event],
                )
                .unwrap()
        } else {
            f.submit(next_route, &[event])
        };
        assert_eq!(f.fixture.copy(next).compute_xgmi.is_none(), mode != 0);
        assert_eq!(
            f.fixture.copy(next).staging.len() as u64,
            if mode == 0 {
                0
            } else {
                next_route.source.byte_len
            }
        );
        assert_eq!(
            f.fixture.backend.cancel_v1(next).unwrap(),
            BackendCancellationV1::Cancelled
        );
        f.fixture.backend.release_event_v1(event.event).unwrap();
        assert_eq!(f.drive(first), BackendPollV1::Succeeded);
        f.clean();
    }
}

#[test]
fn late_directed_cancel_refunds_only_empty_successor_and_keeps_parent_too_late() {
    let mut f = Late::new(None, false);
    let first_route = f.route(0, 1);
    let first = f.submit(first_route, &[]);
    f.publish(first, false);
    let event = f.event(first);
    let next_route = f.route(1, 2);
    let next = f.submit(next_route, &[event]);
    f.fixture.backend.release_event_v1(event.event).unwrap();
    assert_eq!(
        f.fixture.backend.cancel_v1(first).unwrap(),
        BackendCancellationV1::TooLate
    );
    assert_eq!(
        f.fixture.backend.cancel_v1(next).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(f.fixture.root(next).is_quiescent());
    assert!(f.fixture.root(next).trace.is_empty());
    assert!(!f.fixture.backend.submission_retained_as_dependency(first));
    assert_eq!(
        f.fixture.backend.compute_xgmi_children,
        [Some(first), Some(first), None]
    );
    f.fixture.backend.release_submission_v1(next).unwrap();
    assert_eq!(f.drive(first), BackendPollV1::Succeeded);
    f.clean();
}

#[test]
fn late_directed_selector_rejects_corrupt_retained_owner_profile_and_pair() {
    for drift in 0..6 {
        let mut f = Late::new(None, false);
        let first_route = f.route(0, 1);
        let first = f.submit(first_route, &[]);
        f.publish(first, false);
        let source = f.fixture.backend.allocations[&f.allocations[0]];
        let destination = f.fixture.backend.allocations[&f.allocations[2]];
        match drift {
            0 => {
                f.fixture.backend.children[0]
                    .allocations
                    .get_mut(&source.local)
                    .unwrap()
                    .sdma_storage = KfdRuntimeSdmaStorageV1::InFlight(
                    KfdRuntimeSdmaInFlightV1::ComputeXgmi(u64::MAX),
                );
            }
            1 => f.fixture.backend.compute_xgmi_children[1] = None,
            2..=4 => {
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    f.fixture.backend.submissions.get_mut(&first).unwrap()
                else {
                    unreachable!()
                };
                match drift {
                    2 => copy.directed = None,
                    3 => copy.source = copy.destination,
                    _ => copy.compute_xgmi.as_mut().unwrap().phase = Phase::Prepared,
                }
            }
            _ => {
                let local = f.fixture.backend.allocations[&f.allocations[1]].local;
                f.fixture.backend.children[1]
                    .allocations
                    .get_mut(&local)
                    .unwrap()
                    .sdma_storage = KfdRuntimeSdmaStorageV1::InFlight(
                    KfdRuntimeSdmaInFlightV1::ComputeXgmi(u64::MAX),
                );
            }
        }
        let count = f.fixture.backend.submissions.len();
        let route = f.route(0, 2);
        assert!(matches!(
            f.fixture.backend.prepare_compute_xgmi_v1(
                source,
                route.source,
                destination,
                route.destination,
                true,
            ),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(f.fixture.backend.terminal);
        assert_eq!(f.fixture.backend.submissions.len(), count);
        assert!(
            f.fixture
                .root(first)
                .scripted_owners
                .iter()
                .all(Option::is_some)
        );
        // Corrupted roots retain the physical owners. No cleanup is claimed.
    }
}

#[test]
fn late_directed_parent_fault_and_unwind_keep_successor_unissued() {
    for unwind in [false, true] {
        let mut f = Late::new(Some(Stage::Poll), unwind);
        let first_route = f.route(0, 1);
        let first = f.submit(first_route, &[]);
        f.publish(first, false);
        let event = f.event(first);
        let next_route = f.route(1, 2);
        let next = f.submit(next_route, &[event]);
        f.fixture.backend.release_event_v1(event.event).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.fixture
                .backend
                .drain_v1(next, Instant::now() + Duration::from_secs(1))
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(f.fixture.backend.terminal);
        assert!(f.fixture.backend.children[0].terminal);
        assert!(f.fixture.backend.children[1].terminal);
        assert!(!f.fixture.backend.children[2].terminal);
        assert_eq!(
            f.fixture.backend.compute_xgmi_children,
            [Some(first), Some(first), None]
        );
        assert_eq!(f.fixture.copy(next).status(), BackendPollV1::Pending);
        assert!(f.fixture.root(next).is_quiescent());
        assert!(f.fixture.root(next).trace.is_empty());
        assert!(f.fixture.backend.submission_retained_as_dependency(first));
    }
}
