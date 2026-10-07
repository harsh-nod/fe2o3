//! Directed provenance over the production native-route orchestration.
//! Scripted owner/byte transitions here are not native hardware evidence.

use super::*;
use crate::{
    BackendCancellationV1, BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1,
    BackendDirectedScalarPeerCopyV1, BackendDirectedScalarProgressV1,
    RuntimeDirectedScalarPeerCopyBackendV1,
};

struct Three {
    fixture: Fixture,
    allocations: [u64; 3],
    streams: [u64; 3],
    owner_ids: [Option<u64>; 3],
}

impl Three {
    fn new(failure: Option<Stage>, unwind: bool) -> Self {
        let mut fixture = Fixture::configured(failure, unwind, 2, 3);
        // The parent fixture supplies disjoint pairs; this graph instead uses
        // exactly three children and the two directed chain/fanout routes.
        fixture.backend.compute_xgmi_routes.remove(&(2, 3));
        for pair in [(1, 2), (0, 2)] {
            fixture.backend.compute_xgmi_routes.insert(
                pair,
                Route::Scripted {
                    failure: None,
                    unwind: false,
                    pending_samples: 2,
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
        for (index, allocation) in allocations.iter().copied().enumerate() {
            let route = fixture.backend.allocations[&allocation];
            let record = fixture.backend.children[index]
                .allocations
                .get_mut(&route.local)
                .unwrap();
            let KfdRuntimeSdmaStorageV1::Device(owner) = &mut record.sdma_storage else {
                unreachable!()
            };
            owner
                .scripted_bytes_mut()
                .unwrap()
                .fill([0x53, 0x17, 0x29][index]);
            record.content_sha256 = None;
        }
        let owner_ids = allocations.map(|allocation| {
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
            owner_ids,
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

    fn event(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        id: u64,
    ) -> BackendDirectedPeerDependencyV1 {
        BackendDirectedPeerDependencyV1 {
            event: self
                .fixture
                .backend
                .record_event_v1(route.stream, id)
                .unwrap(),
            producer_submission: id,
        }
    }

    fn progress(
        &mut self,
        id: u64,
        route: BackendDirectedPeerRouteV1,
        parents: &[u64],
    ) -> Result<BackendPollV1, Failure> {
        self.fixture.backend.progress_directed_scalar_peer_copy_v1(
            BackendDirectedScalarProgressV1 {
                submission: id,
                route,
                producer_submissions: parents,
            },
        )
    }

    fn drive(
        &mut self,
        id: u64,
        route: BackendDirectedPeerRouteV1,
        parents: &[u64],
    ) -> BackendPollV1 {
        for _ in 0..128 {
            let status = self.progress(id, route, parents).unwrap();
            self.fixture.backend.assert_cooperative_indexes_consistent();
            if status != BackendPollV1::Pending {
                return status;
            }
        }
        panic!("directed native graph did not settle through its requested tail");
    }

    fn start(&mut self, id: u64, route: BackendDirectedPeerRouteV1) {
        for _ in 0..8 {
            assert_eq!(
                self.progress(id, route, &[]).unwrap(),
                BackendPollV1::Pending
            );
            if !self.fixture.root(id).is_quiescent() {
                return;
            }
        }
        panic!("directed native root did not acquire its endpoint owners");
    }

    fn assert_native(&self, ids: &[u64]) {
        for id in ids {
            let copy = self.fixture.copy(*id);
            assert!(copy.directed.is_some());
            assert!(copy.compute_xgmi.is_some());
            assert!(copy.compute_producer.is_none());
            assert!(copy.staging.is_empty());
            assert_eq!(copy.scratch_byte_len, 0);
            assert!(copy.sdma_leaf.is_none());
            assert!(self.fixture.backend.directed_identity_is_intact_v1(*id));
        }
        assert_eq!(self.fixture.backend.cooperative_staging_bytes, 0);
    }

    fn assert_storage(&self, values: [u8; 3]) {
        for (index, allocation) in self.allocations.iter().enumerate() {
            let route = self.fixture.backend.allocations[allocation];
            let record = &self.fixture.backend.children[index].allocations[&route.local];
            let KfdRuntimeSdmaStorageV1::Device(owner) = &record.sdma_storage else {
                panic!("exact device owner must be restored")
            };
            assert_eq!(owner.scripted_owner_id(), self.owner_ids[index]);
            assert_eq!(owner.scripted_bytes().unwrap(), &[values[index]; BYTES]);
        }
    }

    fn clean(self) {
        assert!(
            self.fixture
                .backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(
            self.fixture.backend.completed_compute_xgmi_copies, 0,
            "scripted execution is not a hardware completion"
        );
        self.fixture.clean();
    }
}

#[test]
fn directed_native_three_child_chain_progresses_only_the_final_request() {
    let mut f = Three::new(None, false);
    let first_route = f.route(0, 1);
    let first = f.submit(first_route, &[]);
    let event = f.event(first_route, first);
    let last_route = f.route(1, 2);
    let last = f.submit(last_route, &[event]);
    f.fixture.backend.release_event_v1(event.event).unwrap();
    f.assert_native(&[first, last]);
    assert_eq!(f.fixture.copy(first).dependency_depth, 1);
    assert_eq!(f.fixture.copy(last).dependency_depth, 2);
    for _ in 0..3 {
        assert_eq!(
            f.fixture.backend.poll_v1(last).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.fixture.backend.wait_v1(last, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.fixture.backend.drain_v1(last, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert!(f.fixture.root(first).trace.is_empty());
        assert!(f.fixture.root(last).trace.is_empty());
    }
    assert!(f.fixture.backend.release_submission_v1(first).is_err());
    assert_eq!(
        f.drive(last, last_route, &[first]),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        f.fixture.backend.poll_v1(first).unwrap(),
        BackendPollV1::Succeeded
    );
    f.assert_storage([0x53; 3]);
    for id in [first, last] {
        let trace = &f.fixture.root(id).trace;
        assert_eq!(
            trace.iter().filter(|stage| **stage == Stage::Copy).count(),
            1
        );
        assert_eq!(
            trace
                .iter()
                .filter(|stage| **stage == Stage::Restore)
                .count(),
            1
        );
        assert!(f.fixture.root(id).is_quiescent());
    }
    f.fixture.backend.release_submission_v1(first).unwrap();
    // Settled requested provenance outlives the released parent result/event.
    assert_eq!(
        f.progress(last, last_route, &[first]).unwrap(),
        BackendPollV1::Succeeded
    );
    f.clean();
}

#[test]
fn directed_native_shared_source_sibling_progress_retires_only_the_actual_blocker() {
    let mut f = Three::new(None, false);
    let a = f.route(0, 1);
    let b = f.route(0, 2);
    let first = f.submit(a, &[]);
    let sibling = f.submit(b, &[]);
    f.assert_native(&[first, sibling]);
    f.start(first, a);
    assert_eq!(
        f.fixture.backend.compute_xgmi_children,
        [Some(first), Some(first), None]
    );
    assert_eq!(f.fixture.copy(sibling).dependencies, Vec::<u64>::new());
    assert!(matches!(
        f.progress(sibling, b, &[first]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.drive(sibling, b, &[]), BackendPollV1::Succeeded);
    assert_eq!(
        f.fixture.backend.poll_v1(first).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(
        f.fixture
            .copy(sibling)
            .directed
            .as_ref()
            .unwrap()
            .dependencies()
            .is_empty()
    );
    f.assert_storage([0x53; 3]);
    f.clean();
}

#[test]
fn directed_native_cancelled_parent_fails_its_chain_but_not_an_independent_sibling() {
    for dependent in [false, true] {
        let mut f = Three::new(None, false);
        let a = f.route(0, 1);
        let first = f.submit(a, &[]);
        let event = f.event(a, first);
        let b = f.route(usize::from(dependent), 2);
        let last = f.submit(
            b,
            if dependent {
                std::slice::from_ref(&event)
            } else {
                &[]
            },
        );
        f.fixture.backend.release_event_v1(event.event).unwrap();
        f.assert_native(&[first, last]);
        assert_eq!(
            f.fixture.backend.cancel_v1(first).unwrap(),
            BackendCancellationV1::Cancelled
        );
        let status = f.drive(
            last,
            b,
            if dependent {
                std::slice::from_ref(&first)
            } else {
                &[]
            },
        );
        if dependent {
            assert!(matches!(status, BackendPollV1::Failed { .. }));
            assert!(f.fixture.root(last).trace.is_empty());
            f.assert_storage([0x53, 0x17, 0x29]);
        } else {
            assert_eq!(status, BackendPollV1::Succeeded);
            f.assert_storage([0x53, 0x17, 0x53]);
        }
        assert!(!f.fixture.backend.terminal);
        assert!(
            f.fixture
                .backend
                .cooperative_dependency_retain_counts
                .is_empty()
        );
        f.clean();
    }
}

#[test]
fn directed_native_cancel_is_too_late_after_ownership_but_refunds_unstarted_sibling() {
    let mut f = Three::new(None, false);
    let a = f.route(0, 1);
    let b = f.route(0, 2);
    let first = f.submit(a, &[]);
    let sibling = f.submit(b, &[]);
    f.start(first, a);
    assert_eq!(
        f.fixture.backend.cancel_v1(first).unwrap(),
        BackendCancellationV1::TooLate
    );
    assert_eq!(
        f.fixture.backend.cancel_v1(sibling).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert_eq!(
        f.fixture.backend.compute_xgmi_children,
        [Some(first), Some(first), None]
    );
    assert_eq!(f.drive(first, a, &[]), BackendPollV1::Succeeded);
    f.assert_storage([0x53, 0x53, 0x29]);
    f.clean();
}

#[test]
fn directed_native_checked_ranges_and_absent_routes_select_exact_transport() {
    for variant in 0..3 {
        let mut f = Three::new(None, false);
        let mut route = f.route(0, 1);
        match variant {
            0 => {
                route.source.byte_len -= 1;
                route.destination.byte_len -= 1;
            }
            1 => {
                route.source.byte_offset = 1;
                route.destination.byte_offset = 2;
                route.source.byte_len = BYTES as u64 - 2;
                route.destination.byte_len = BYTES as u64 - 2;
            }
            2 => {
                f.fixture.backend.compute_xgmi_routes.remove(&(0, 1));
            }
            _ => unreachable!(),
        }
        let id = f.submit(route, &[]);
        let copy = f.fixture.copy(id);
        assert!(copy.directed.is_some());
        assert_eq!(copy.compute_xgmi.is_none(), variant == 2);
        assert_eq!(
            copy.staging.len() as u64,
            if variant == 2 {
                route.source.byte_len
            } else {
                0
            }
        );
        assert!(f.fixture.backend.directed_identity_is_intact_v1(id));
        assert_eq!(
            f.fixture.backend.cancel_v1(id).unwrap(),
            BackendCancellationV1::Cancelled
        );
        f.assert_storage([0x53, 0x17, 0x29]);
        f.clean();
    }
}

#[test]
fn directed_native_corrupt_history_or_transport_shape_is_terminal_before_effects() {
    for transport in [false, true] {
        let mut f = Three::new(None, false);
        let a = f.route(0, 1);
        let first = f.submit(a, &[]);
        let event = f.event(a, first);
        let b = f.route(1, 2);
        let last = f.submit(b, &[event]);
        f.fixture.backend.release_event_v1(event.event).unwrap();
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            f.fixture.backend.submissions.get_mut(&last).unwrap()
        else {
            unreachable!()
        };
        if transport {
            copy.byte_cursor = 1;
        } else {
            copy.directed.as_mut().unwrap().dependencies_mut()[0].producer_submission = last;
        }
        assert!(matches!(
            f.progress(last, b, &[first]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(f.fixture.backend.terminal);
        assert!(f.fixture.root(first).trace.is_empty());
        assert!(f.fixture.root(last).trace.is_empty());
        assert!(f.fixture.backend.submission_retained_as_dependency(first));
        assert!(
            f.fixture
                .backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        f.assert_storage([0x53, 0x17, 0x29]);
        // ManuallyDrop retains the intentionally corrupted graph.
    }
}

#[test]
fn directed_native_fault_and_unwind_preserve_exact_pair_and_dependent_roots() {
    for unwind in [false, true] {
        for stage in STAGES {
            let mut f = Three::new(Some(stage), unwind);
            let a = f.route(0, 1);
            let first = f.submit(a, &[]);
            let event = f.event(a, first);
            let b = f.route(1, 2);
            let last = f.submit(b, &[event]);
            f.fixture.backend.release_event_v1(event.event).unwrap();
            f.assert_native(&[first, last]);
            let result = catch_unwind(AssertUnwindSafe(|| {
                for _ in 0..64 {
                    f.progress(last, b, &[first])?;
                }
                Ok::<_, Failure>(())
            }));
            if unwind {
                assert!(result.is_err(), "{stage:?}");
            } else {
                assert!(
                    matches!(result.unwrap(), Err(RuntimeBackendFailureV1::Terminal(_))),
                    "{stage:?}"
                );
            }
            assert!(f.fixture.backend.terminal);
            assert!(
                f.fixture.backend.children[0].terminal && f.fixture.backend.children[1].terminal
            );
            assert!(!f.fixture.backend.children[2].terminal);
            assert_eq!(
                f.fixture.backend.compute_xgmi_children,
                [Some(first), Some(first), None]
            );
            assert!(!f.fixture.root(first).is_quiescent());
            assert!(f.fixture.root(last).is_quiescent());
            assert!(f.fixture.root(last).trace.is_empty());
            for index in 0..2 {
                assert_eq!(
                    f.fixture.root(first).scripted_owners[index]
                        .as_ref()
                        .unwrap()
                        .scripted_owner_id(),
                    f.owner_ids[index]
                );
            }
            assert!(f.fixture.backend.submission_retained_as_dependency(first));
            assert_eq!(f.fixture.backend.completed_compute_xgmi_copies, 0);
            // Native ambiguity is retained; there is deliberately no cleanup claim.
        }
    }
}
