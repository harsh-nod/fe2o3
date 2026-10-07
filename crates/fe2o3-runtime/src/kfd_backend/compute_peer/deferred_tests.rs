//! Actual router handoff and scripted compute/copy custody, not GPU arithmetic.

use super::*;

struct Chain {
    fixture: Fixture,
    incoming: u64,
    incoming_event: u64,
    producer: u64,
    producer_event: u64,
}

impl Chain {
    fn new(readback: bool, promote: bool) -> Self {
        Self::with_depth(readback, promote, 1)
    }

    fn with_depth(readback: bool, promote: bool, incoming_depth: usize) -> Self {
        Self::with_fixture(
            Fixture::with_peer_input_promotion(readback, promote),
            incoming_depth,
        )
    }

    fn with_fixture(mut f: Fixture, incoming_depth: usize) -> Self {
        f.backend.compute_xgmi_routes.insert(
            (1, 0),
            Route::Scripted {
                failure: None,
                unwind: false,
                pending_samples: 2,
            },
        );
        let stream = f.backend.create_stream_v1(7).unwrap();
        let incoming = f
            .backend
            .peer_copy_v1(
                stream,
                region(f.allocations[1][4], RuntimeAccessV1::Read),
                region(f.allocations[0][0], RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        if incoming_depth != 1 {
            // Exercise admission arithmetic using retained rank metadata; no
            // completion or native execution is invented by this depth fixture.
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                f.backend.submissions.get_mut(&incoming).unwrap()
            else {
                unreachable!()
            };
            copy.dependency_depth = incoming_depth;
        }
        let incoming_event = f.backend.record_event_v1(stream, incoming).unwrap();
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
                    event: incoming_event,
                    producer_submission: incoming,
                }],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
            })
            .unwrap();
        assert!(
            f.backend
                .deferred_compute_v1(producer)
                .unwrap()
                .route
                .is_none()
        );
        let producer_event = f.event(0, producer);
        Self {
            fixture: f,
            incoming,
            incoming_event,
            producer,
            producer_event,
        }
    }

    fn peer(&mut self) -> u64 {
        self.fixture.peer(&[self.producer_event])
    }

    fn release_events(&mut self) {
        self.fixture
            .backend
            .release_event_v1(self.producer_event)
            .unwrap();
        self.fixture
            .backend
            .release_event_v1(self.incoming_event)
            .unwrap();
    }

    fn handoff(&mut self) -> RoutedHandleV1 {
        for _ in 0..32 {
            assert_eq!(
                self.fixture
                    .backend
                    .progress_deferred_compute_v1(self.producer)
                    .unwrap(),
                BackendPollV1::Pending
            );
            if let Some(route) = self
                .fixture
                .backend
                .deferred_compute_v1(self.producer)
                .unwrap()
                .route
            {
                return route;
            }
        }
        panic!("scripted deferred handoff did not issue");
    }

    fn cancel_unissued(mut self) {
        assert_eq!(
            self.fixture.backend.cancel_v1(self.producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        if self.fixture.copy(self.incoming).status() == BackendPollV1::Pending {
            assert_eq!(
                self.fixture.backend.cancel_v1(self.incoming).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        self.fixture.clean();
    }
}

#[test]
fn deferred_compute_peer_window_final_readback_preserves_full_unequal_owner_guards() {
    for routed in [false, true] {
        let mut f = Fixture::with_layout([BYTES, 96], Some((11, 7, 47)), true);
        let source = f.allocations[0][2];
        let destination = f.allocations[1][3];
        let (source_id, original) = window::initialize_bytes(&mut f, source, 0x37);
        let (destination_id, mut expected) = window::initialize_bytes(&mut f, destination, 0xa1);
        let mut chain = Chain::with_fixture(f, 1);
        if routed {
            chain.handoff();
        }
        let f = &mut chain.fixture;
        let source_region = BackendMemoryRegionV1 {
            byte_offset: 17,
            byte_len: 47,
            ..region(source, RuntimeAccessV1::Read)
        };
        let destination_region = BackendMemoryRegionV1 {
            byte_offset: 11,
            byte_len: 47,
            ..region(destination, RuntimeAccessV1::Write)
        };
        let peer = f
            .backend
            .peer_copy_v1(
                f.peer_stream,
                source_region,
                destination_region,
                &[chain.producer_event],
            )
            .unwrap();
        let exact = fe2o3_kfd::Gfx942ComputeXgmiCopyWindowV1::new(64, 96, 17, 11, 47).unwrap();
        assert_eq!(
            f.copy(peer).compute_producer.as_ref().unwrap().window(),
            exact
        );
        assert!(
            f.copy(peer)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .matches_window(exact)
        );
        assert!(f.copy(peer).staging.is_empty());
        let peer_event = f.backend.record_event_v1(f.peer_stream, peer).unwrap();
        let readback = f
            .backend
            .copy_async_v1(
                f.readback_stream,
                BackendMemoryRegionV1 {
                    access: RuntimeAccessV1::Read,
                    ..destination_region
                },
                BackendMemoryRegionV1 {
                    allocation: f.host.unwrap(),
                    access: RuntimeAccessV1::Write,
                    byte_offset: 7,
                    byte_len: 47,
                },
                &[peer_event],
            )
            .unwrap();
        f.backend.release_event_v1(peer_event).unwrap();
        chain.release_events();
        let f = &mut chain.fixture;
        assert!(f.backend.release_submission_v1(chain.producer).is_err());
        assert_eq!(f.backend.poll_v1(readback).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(readback, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            f.backend.drain_v1(readback, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert!(f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent());
        f.drive(f.readback_stream, readback);
        for id in [chain.incoming, chain.producer, peer, readback] {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        expected[11..58].copy_from_slice(&original[17..64]);
        assert_eq!(
            window::restored(f, source),
            (source_id, original.as_slice())
        );
        assert_eq!(
            window::restored(f, destination),
            (destination_id, expected.as_slice())
        );
        let host = f.backend.allocations[&f.host.unwrap()];
        let KfdRuntimeSdmaStorageV1::Host(owner) =
            &f.backend.children[host.child].allocations[&host.local].sdma_storage
        else {
            panic!("host owner expected")
        };
        let mut expected_host = vec![0; 96];
        expected_host[7..54].copy_from_slice(&original[17..64]);
        assert_eq!(owner.scripted_bytes().unwrap(), expected_host);
        assert!(f.backend.deferred_compute_retains.is_empty());
        assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        chain.fixture.clean();
    }
}

#[test]
fn deferred_compute_peer_final_stream_drives_four_roots_after_event_release() {
    let mut chain = Chain::new(true, true);
    let peer = chain.peer();
    let f = &mut chain.fixture;
    let peer_event = f.backend.record_event_v1(f.peer_stream, peer).unwrap();
    let readback = f
        .backend
        .copy_async_v1(
            f.readback_stream,
            region(f.allocations[1][3], RuntimeAccessV1::Read),
            region(f.host.unwrap(), RuntimeAccessV1::Write),
            &[peer_event],
        )
        .unwrap();
    f.backend.release_event_v1(peer_event).unwrap();
    assert_eq!(f.copy(chain.incoming).dependency_depth, 1);
    assert_eq!(f.backend.deferred_peer_depth_v1(chain.producer), Some(2));
    assert_eq!(f.copy(peer).dependency_depth, 3);
    assert_eq!(f.copy(readback).dependency_depth, 4);
    assert!(f.copy(peer).compute_xgmi.is_some());
    assert!(f.copy(peer).staging.is_empty());
    chain.release_events();
    let f = &mut chain.fixture;
    assert!(f.backend.release_submission_v1(chain.producer).is_err());
    assert!(f.backend.release_submission_v1(chain.incoming).is_err());
    for _ in 0..3 {
        for id in [chain.incoming, chain.producer, peer, readback] {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(
            f.backend.drain_v1(readback, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert!(
        f.backend
            .deferred_compute_v1(chain.producer)
            .unwrap()
            .route
            .is_none()
    );
    assert!(f.backend.children[0].allocation_custody.is_empty());
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    f.drive(f.readback_stream, readback);
    for id in [chain.incoming, chain.producer, peer, readback] {
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
    }
    let route = f
        .backend
        .deferred_compute_v1(chain.producer)
        .unwrap()
        .route
        .unwrap();
    assert!(f.backend.children[0].exact_submission_quiescent_v1(route.local));
    assert!(f.backend.deferred_compute_retains.is_empty());
    assert!(f.backend.cooperative_dependency_retain_counts.is_empty());
    assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    assert_eq!(
        f.backend.completed_compute_xgmi_copies, 0,
        "scripted transfers are not hardware completions"
    );
    chain.fixture.clean();
}

#[test]
fn deferred_compute_peer_accepts_routed_producer_and_retains_completed_identity() {
    for routed in [false, true] {
        let mut chain = Chain::new(false, true);
        if routed {
            chain.handoff();
        }
        let peer = chain.peer();
        chain.release_events();
        let route = chain.handoff();
        let f = &mut chain.fixture;
        // Settle only the producer, leaving the downstream native plan untouched.
        f.drive(f.compute_streams[0], chain.producer);
        assert_eq!(
            f.backend.poll_v1(chain.producer).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(f.copy(peer).compute_producer.is_some());
        f.backend.validate_compute_peer_v1(peer).unwrap();
        assert!(f.backend.children[0].exact_submission_quiescent_v1(route.local));
        assert!(f.backend.release_submission_v1(chain.producer).is_err());
        // The private completion receipt must survive destruction of the old
        // producer stream; the outgoing copy is not its publication authority.
        f.backend.destroy_stream_v1(f.compute_streams[0]).unwrap();
        f.drive(f.peer_stream, peer);
        assert_eq!(f.backend.poll_v1(peer).unwrap(), BackendPollV1::Succeeded);
        chain.fixture.clean();
    }
}

#[test]
fn deferred_compute_peer_rejection_is_pre_effect_and_no_staged_fallback() {
    let mut chain = Chain::new(false, false);
    let f = &mut chain.fixture;
    let before = (
        f.backend.next_handle,
        f.backend.submissions.len(),
        f.backend.cooperative_staging_bytes,
    );
    for events in [
        vec![],
        vec![chain.incoming_event],
        vec![chain.producer_event, chain.producer_event],
    ] {
        assert!(
            f.backend
                .peer_copy_v1(
                    f.peer_stream,
                    region(f.allocations[0][2], RuntimeAccessV1::Read),
                    region(f.allocations[1][3], RuntimeAccessV1::Write),
                    &events,
                )
                .is_err()
        );
        assert_eq!(
            (
                f.backend.next_handle,
                f.backend.submissions.len(),
                f.backend.cooperative_staging_bytes
            ),
            before
        );
        assert!(!f.backend.terminal);
    }
    for (source, destination) in [
        (f.allocations[0][1], f.allocations[1][3]),
        (f.allocations[1][3], f.allocations[0][2]),
    ] {
        let stream = if source == f.allocations[0][1] {
            f.peer_stream
        } else {
            f.compute_streams[0]
        };
        assert!(
            f.backend
                .peer_copy_v1(
                    stream,
                    region(source, RuntimeAccessV1::Read),
                    region(destination, RuntimeAccessV1::Write),
                    &[chain.producer_event]
                )
                .is_err()
        );
        assert_eq!(
            (
                f.backend.next_handle,
                f.backend.submissions.len(),
                f.backend.cooperative_staging_bytes
            ),
            before
        );
    }
    let peer = chain.peer();
    assert_eq!(
        chain.fixture.backend.cancel_v1(peer).unwrap(),
        BackendCancellationV1::Cancelled
    );
    chain.cancel_unissued();
}

#[test]
fn deferred_compute_peer_cancel_and_failed_parent_refund_only_safe_roots() {
    for cancel_peer in [false, true] {
        let mut chain = Chain::new(false, false);
        let peer = chain.peer();
        chain.release_events();
        let f = &mut chain.fixture;
        if cancel_peer {
            assert_eq!(
                f.backend.cancel_v1(peer).unwrap(),
                BackendCancellationV1::Cancelled
            );
            assert!(!f.backend.submission_retained_as_dependency(chain.producer));
        }
        assert_eq!(
            f.backend.cancel_v1(chain.producer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        if !cancel_peer {
            assert!(matches!(
                f.backend.flush_stream_v1(f.peer_stream),
                Err(RuntimeBackendFailureV1::Quiescent(_))
            ));
            assert!(matches!(
                f.backend.poll_v1(peer).unwrap(),
                BackendPollV1::Failed { .. }
            ));
        }
        assert!(f.copy(peer).is_quiescent());
        assert!(f.backend.deferred_compute_retains.is_empty());
        assert!(!f.backend.submission_retained_as_dependency(chain.producer));
        assert!(!f.backend.peer_launch_retains.retains(chain.incoming));
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
        assert!(!f.backend.terminal);
        assert_eq!(
            f.backend.cancel_v1(chain.incoming).unwrap(),
            BackendCancellationV1::Cancelled
        );
        chain.fixture.clean();
    }
}

#[test]
fn deferred_compute_peer_incoming_cancellation_propagates_without_false_success() {
    let mut chain = Chain::new(false, false);
    let peer = chain.peer();
    chain.release_events();
    let f = &mut chain.fixture;
    assert_eq!(
        f.backend.cancel_v1(chain.incoming).unwrap(),
        BackendCancellationV1::Cancelled
    );
    assert!(matches!(
        f.backend.flush_stream_v1(f.peer_stream),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(matches!(
        f.backend.poll_v1(chain.producer).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert!(matches!(
        f.backend.poll_v1(peer).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert!(
        f.backend
            .deferred_compute_v1(chain.producer)
            .unwrap()
            .route
            .is_none()
    );
    assert!(f.backend.children[0].allocation_custody.is_empty());
    assert!(!f.backend.terminal);
    chain.fixture.clean();
}

#[test]
fn deferred_compute_peer_exact_depth_boundary_is_checked_before_retains() {
    for depth in [
        MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 - 2,
        MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 - 1,
    ] {
        let mut chain = Chain::with_depth(false, false, depth);
        let f = &mut chain.fixture;
        let before = (f.backend.next_handle, f.backend.submissions.len());
        let result = f.backend.peer_copy_v1(
            f.peer_stream,
            region(f.allocations[0][2], RuntimeAccessV1::Read),
            region(f.allocations[1][3], RuntimeAccessV1::Write),
            &[chain.producer_event],
        );
        if depth + 2 == MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let peer = result.unwrap();
            assert_eq!(
                f.copy(peer).dependency_depth,
                MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
            );
            assert_eq!(
                f.backend.cancel_v1(peer).unwrap(),
                BackendCancellationV1::Cancelled
            );
        } else {
            assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
            );
            assert_eq!((f.backend.next_handle, f.backend.submissions.len()), before);
            assert!(!f.backend.submission_retained_as_dependency(chain.producer));
        }
        chain.cancel_unissued();
    }
}

#[test]
fn deferred_compute_peer_forged_handoff_and_backward_rank_fail_closed() {
    for forge_route in [false, true] {
        let mut chain = Chain::new(false, forge_route);
        let peer = chain.peer();
        chain.release_events();
        if forge_route {
            let route = chain.handoff();
            let RoutedSubmissionV1::DeferredCompute(root) = chain
                .fixture
                .backend
                .submissions
                .get_mut(&chain.producer)
                .unwrap()
            else {
                unreachable!()
            };
            root.route = Some(RoutedHandleV1 {
                child: route.child,
                local: route.local + 1,
            });
        } else {
            let RoutedSubmissionV1::CooperativeCopy(copy) = chain
                .fixture
                .backend
                .submissions
                .get_mut(&chain.incoming)
                .unwrap()
            else {
                unreachable!()
            };
            copy.dependency_depth = 2;
        }
        let f = &mut chain.fixture;
        assert!(matches!(
            f.backend.progress_cooperative_copy(peer),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(f.backend.terminal);
        assert!(f.copy(peer).compute_producer.is_some());
        assert!(f.backend.submission_retained_as_dependency(chain.producer));
        assert!(f.backend.peer_launch_retains.retains(chain.incoming));
        assert_eq!(f.backend.completed_compute_xgmi_copies, 0);
        // Intentionally retain the inconsistent/uncertain graph in ManuallyDrop.
    }
}

#[test]
fn deferred_compute_peer_compute_uncertainty_retains_payload_and_all_ancestors() {
    for fault in [
        ScriptedThreeCompletionFaultV1::Poll,
        ScriptedThreeCompletionFaultV1::AfterRetirement,
    ] {
        let mut chain = Chain::new(false, true);
        let peer = chain.peer();
        chain.release_events();
        chain.handoff();
        let f = &mut chain.fixture;
        f.backend.children[0].scripted_three_completion_fault = Some(fault);
        let result = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..16 {
                f.backend.progress_cooperative_copy(peer)?;
            }
            Ok::<_, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>(())
        }));
        if fault == ScriptedThreeCompletionFaultV1::AfterRetirement {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(f.backend.terminal);
        assert_eq!(
            f.backend
                .deferred_compute_v1(chain.producer)
                .unwrap()
                .status,
            BackendPollV1::Pending
        );
        assert!(f.backend.submission_retained_as_dependency(chain.producer));
        assert!(f.backend.peer_launch_retains.retains(chain.incoming));
        assert!(!f.backend.deferred_compute_retains.is_empty());
        assert!(f.copy(peer).compute_producer.is_some());
        assert!(f.copy(peer).compute_xgmi.as_ref().unwrap().is_quiescent());
        assert!(f.backend.compute_xgmi_children.iter().all(Option::is_none));
    }
}
