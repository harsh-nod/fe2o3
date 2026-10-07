//! Real router/child progress with scripted owners, not native DMA or arithmetic.

use super::*;
use crate::{
    BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    RuntimeDirectedScalarPeerCopyBackendV1,
};

struct Chain {
    fixture: Fixture,
    incoming: u64,
    producer: u64,
    route: RoutedHandleV1,
    outgoing: u64,
    original_owners: [Option<u64>; 3],
}

impl Chain {
    fn new(failure: Option<Stage>, unwind: bool) -> Self {
        let mut f = Fixture::with_peer_input_promotion(false, true);
        f.backend.compute_xgmi_routes.insert(
            (1, 0),
            Route::Scripted {
                failure,
                unwind,
                pending_samples: 3,
            },
        );
        let original_owners = [
            f.allocations[1][4],
            f.allocations[0][2],
            f.allocations[1][3],
        ]
        .map(|allocation| {
            let route = f.backend.allocations[&allocation];
            let KfdRuntimeSdmaStorageV1::H2dReady(ready) =
                &f.backend.children[route.child].allocations[&route.local].sdma_storage
            else {
                panic!("fixture owns initialized persistent inputs");
            };
            ready.owner.scripted_owner_id()
        });
        assert!(original_owners.iter().all(Option::is_some));
        let incoming_stream = f.backend.create_stream_v1(7).unwrap();
        let incoming = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: BackendDirectedPeerRouteV1 {
                    stream: incoming_stream,
                    source_device: 8,
                    destination_device: 7,
                    source: region(f.allocations[1][4], RuntimeAccessV1::Read),
                    destination: region(f.allocations[0][0], RuntimeAccessV1::Write),
                },
                dependencies: &[],
            })
            .unwrap();
        assert!(f.copy(incoming).directed.is_some());
        assert!(f.copy(incoming).compute_xgmi.is_some());
        let event = f
            .backend
            .record_event_v1(incoming_stream, incoming)
            .unwrap();
        let bindings = std::array::from_fn::<_, 3, _>(|index| BackendBindingV1 {
            region: region(
                f.allocations[0][index],
                if index == 2 {
                    RuntimeAccessV1::Write
                } else {
                    RuntimeAccessV1::Read
                },
            ),
            kernarg_byte_offset: index as u32 * 8,
        });
        let mut kernarg = [0; 32];
        kernarg[24..].copy_from_slice(&16_u64.to_le_bytes());
        let producer = f
            .backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream: f.compute_streams[0],
                kernel: f.kernels[0],
                explicit_kernarg: &kernarg,
                bindings: &bindings,
                dependencies: &[BackendLaunchProducerV1 {
                    event,
                    producer_submission: incoming,
                }],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
            .unwrap();
        f.backend.release_event_v1(event).unwrap();
        let route = f.native(producer);
        let pending = &f.backend.children[route.child].pending_compute[&route.local];
        assert!(pending.peer_gate.unwrap().owns(producer, route.local));
        assert_eq!(pending.dependency_depth, 2);
        let event = f.event(0, producer);
        let outgoing = f.peer(&[event]);
        f.backend.release_event_v1(event).unwrap();
        assert!(f.copy(outgoing).compute_producer.is_some());
        assert!(f.copy(outgoing).compute_xgmi.is_some());
        assert_eq!(f.copy(outgoing).dependency_depth, 3);
        Self {
            fixture: f,
            incoming,
            producer,
            route,
            outgoing,
            original_owners,
        }
    }

    fn assert_occupied_prefix(&self) {
        let f = &self.fixture;
        assert_eq!(
            f.backend.compute_xgmi_children,
            [Some(self.incoming), Some(self.incoming)]
        );
        assert!(
            !f.copy(self.incoming)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .is_quiescent()
        );
        assert!(
            f.copy(self.outgoing)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .is_quiescent()
        );
        assert!(
            f.backend.children[0]
                .pending_compute
                .contains_key(&self.route.local)
        );
        assert!(!f.backend.children[0].any_compute_active_v1());
        assert!(f.backend.peer_launch_retains.retains(self.incoming));
        assert!(f.backend.submission_retained_as_dependency(self.producer));
    }

    fn publish_from_tail(&mut self) {
        for _ in 0..8 {
            let stream = self.fixture.peer_stream;
            self.fixture.backend.flush_stream_v1(stream).unwrap();
            if self.fixture.backend.compute_xgmi_children[0] == Some(self.incoming) {
                self.assert_occupied_prefix();
                return;
            }
        }
        panic!("tail stream did not publish its retained directed prefix");
    }
}

#[test]
fn compute_peer_directed_prefix_tail_only_progress_restores_exact_owners() {
    for drain in [false, true] {
        let mut chain = Chain::new(None, false);
        // This later FIFO node is not publication authority granted to the peer.
        let trailing = chain.fixture.launch(0, 4, false, false);
        let trailing_route = chain.fixture.native(trailing);
        chain.publish_from_tail();
        let f = &mut chain.fixture;
        let before = f.backend.cooperative_progress_generation;
        let steps: Vec<_> = f
            .backend
            .children
            .iter()
            .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
            .collect();
        for _ in 0..3 {
            assert_eq!(
                f.backend.poll_v1(chain.outgoing).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                f.backend.wait_v1(chain.outgoing, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                f.backend.drain_v1(chain.outgoing, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(f.backend.cooperative_progress_generation, before);
        assert_eq!(
            f.backend
                .children
                .iter()
                .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
                .collect::<Vec<_>>(),
            steps
        );
        assert!(f.backend.release_submission_v1(chain.incoming).is_err());
        assert!(f.backend.release_submission_v1(chain.producer).is_err());
        if drain {
            assert_eq!(
                f.backend
                    .drain_v1(chain.outgoing, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
        } else {
            f.drive(f.peer_stream, chain.outgoing);
        }
        for id in [chain.incoming, chain.producer, chain.outgoing] {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert!(f.backend.children[0].exact_submission_quiescent_v1(chain.route.local));
        assert!(
            f.backend.children[0]
                .pending_compute
                .contains_key(&trailing_route.local)
        );
        assert!(!f.backend.children[0].any_compute_active_v1());
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        for (allocation, original) in [
            f.allocations[1][4],
            f.allocations[0][2],
            f.allocations[1][3],
        ]
        .into_iter()
        .zip(chain.original_owners)
        {
            let route = f.backend.allocations[&allocation];
            let KfdRuntimeSdmaStorageV1::Device(owner) =
                &f.backend.children[route.child].allocations[&route.local].sdma_storage
            else {
                panic!("original persistent owner must be restored");
            };
            assert_eq!(owner.scripted_owner_id(), original);
        }
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        assert_eq!(
            f.backend.cancel_v1(trailing).unwrap(),
            BackendCancellationV1::Cancelled
        );
        chain.fixture.clean();
    }
}

#[test]
fn compute_peer_directed_prefix_retirement_error_and_unwind_keep_all_roots() {
    for unwind in [false, true] {
        let mut chain = Chain::new(Some(Stage::Retire), unwind);
        chain.publish_from_tail();
        let f = &mut chain.fixture;
        let result = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..32 {
                f.backend.flush_stream_v1(f.peer_stream)?;
            }
            Ok::<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>(())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(f.backend.terminal);
        assert!(f.backend.children.iter().all(|child| child.terminal));
        assert_eq!(f.copy(chain.outgoing).status(), BackendPollV1::Pending);
        assert!(f.copy(chain.outgoing).compute_producer.is_some());
        assert!(
            f.backend
                .producer_aware_native
                .contains_key(&chain.producer)
        );
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        for child in &f.backend.children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.live_owner_count(), ALLOCATIONS);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        chain.assert_occupied_prefix();
        // Terminal fixtures intentionally retain every owner until process exit.
    }
}
