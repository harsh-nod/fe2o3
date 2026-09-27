use super::*;

fn binding(f: &Fixture, index: usize) -> BackendBindingV1 {
    BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: f.backend.allocations[&f.allocations[index]].local,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    }
}

#[test]
fn peer_ancestry_completed_ordinary_copy_is_a_terminal_leaf() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f
        .backend
        .peer_copy_v1(a.stream, a.source, a.destination, &[])
        .unwrap();
    let b = f.route(0, 3);
    let second = f
        .backend
        .peer_copy_v1(b.stream, b.source, b.destination, &[])
        .unwrap();
    f.backend.flush_stream_v1(b.stream).unwrap();
    assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    let RoutedSubmissionV1::CooperativeCopy(copy) = &f.backend.submissions[&second] else {
        panic!()
    };
    assert_eq!(copy.dependency_depth, 2);
    let owner = f.backend.next_id().unwrap();
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_v1(owner, b.stream, &[second])
        .unwrap();
    assert_eq!(ancestry.retained_ids().unwrap(), [second]);
    assert_eq!(
        ancestry.depth(),
        2,
        "ordinary completed producer is a depth-one leaf"
    );
    f.clean(&[first, second]);
}

#[test]
fn peer_ancestry_transitive_source_and_released_events_keep_exact_retains() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let first_event = f.event(a, first);
    let b = f.route(1, 2);
    let second = f.submit(b, &[first_event]);
    let second_event = f.event(b, second);
    let c = f.route(2, 3);
    let third = f.submit(c, &[second_event]);
    f.backend.release_event_v1(first_event.event).unwrap();
    f.backend.release_event_v1(second_event.event).unwrap();
    let owner = f.backend.next_id().unwrap();
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_v1(owner, c.stream, &[third])
        .unwrap();
    assert_eq!(ancestry.retained_ids().unwrap(), [first, second, third]);
    assert_eq!(ancestry.depth(), 4);
    let source = binding(&f, 0);
    let middle = binding(&f, 2);
    f.backend
        .admit_peer_ancestry_bindings_v1(&ancestry, 0, &[source, middle])
        .unwrap();
    let consumer = RoutedHandleV1 {
        child: 0,
        local: f.backend.children[0].next_handle,
    };
    let permits = f
        .backend
        .prepare_peer_compute_access_v1(consumer, &ancestry, &[source, middle])
        .unwrap();
    assert!(permits.valid_for(
        Some(PeerComputeGateV1::waiting(owner, consumer.local, true)),
        consumer.local,
        &[source, middle]
    ));
    f.backend
        .with_peer_launch_ancestry_v1(owner, Some(ancestry), |backend| {
            for id in [first, second, third] {
                assert!(backend.peer_launch_retains.retains(id));
            }
            Ok(consumer.local)
        })
        .unwrap();
    assert_eq!(f.drive(third, c, &[second]), BackendPollV1::Succeeded);
    for id in [first, second, third] {
        assert!(
            matches!(f.backend.release_submission_v1(id), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
    }
    f.backend.peer_launch_retains.release(owner);
    f.backend.peer_launch_retains.release(owner);
    assert!(f.backend.peer_launch_retains.is_empty());
    f.clean(&[first, second, third]);
}

#[test]
fn peer_ancestry_completed_history_does_not_require_released_grandparents_or_allocations() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let event = f.event(a, first);
    let b = f.route(1, 2);
    let second = f.submit(b, &[event]);
    f.backend.release_event_v1(event.event).unwrap();
    assert_eq!(f.drive(second, b, &[first]), BackendPollV1::Succeeded);
    f.backend.release_submission_v1(first).unwrap();
    f.backend.release_allocation_v1(f.allocations[0]).unwrap();
    let owner = f.backend.next_id().unwrap();
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_v1(owner, b.stream, &[second])
        .unwrap();
    assert_eq!(ancestry.retained_ids().unwrap(), [second]);
    assert_eq!(ancestry.depth(), 3);
    f.backend
        .validate_peer_launch_ancestry_v1(&ancestry)
        .unwrap();
    f.backend.release_submission_v1(second).unwrap();
    for allocation in &f.allocations[1..] {
        f.backend.release_allocation_v1(*allocation).unwrap();
    }
    for stream in f.streams {
        f.backend.destroy_stream_v1(stream).unwrap();
    }
    f.backend.shutdown_native_v1().unwrap();
}

#[test]
fn peer_ancestry_captures_fifo_without_following_later_stream_tail() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let b = f.route(0, 3);
    let second = f.submit(b, &[]);
    let owner = f.backend.next_id().unwrap();
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_v1(owner, b.stream, &[])
        .unwrap();
    assert_eq!(ancestry.retained_ids().unwrap(), [first, second]);
    assert_eq!(
        ancestry.depth(),
        1,
        "FIFO alone is ordering, not a success dependency"
    );
    assert_eq!(f.drive(second, b, &[]), BackendPollV1::Succeeded);
    let third = f.submit(a, &[]);
    assert_eq!(f.backend.cooperative_stream_tails[&a.stream], third);
    f.backend
        .validate_peer_launch_ancestry_v1(&ancestry)
        .unwrap();
    assert!(!ancestry.contains(third));
    assert_eq!(f.drive(third, a, &[]), BackendPollV1::Succeeded);
    f.clean(&[first, second, third]);
}

#[test]
fn peer_ancestry_unrelated_read_sibling_is_not_a_predecessor() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let stream = f.backend.create_stream_v1(8).unwrap();
    let b = BackendDirectedPeerRouteV1 {
        stream,
        ..f.route(0, 3)
    };
    let sibling = f.submit(b, &[]);
    let owner = f.backend.next_id().unwrap();
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_v1(owner, a.stream, &[first])
        .unwrap();
    let source = binding(&f, 0);
    assert!(
        matches!(f.backend.admit_peer_ancestry_bindings_v1(&ancestry, 0, &[source]),
        Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert!(f.backend.peer_launch_retains.is_empty());
    assert_eq!(f.drive(sibling, b, &[]), BackendPollV1::Succeeded);
    assert_eq!(f.drive(first, a, &[]), BackendPollV1::Succeeded);
    f.backend.release_submission_v1(sibling).unwrap();
    f.backend.destroy_stream_v1(stream).unwrap();
    f.clean(&[first]);
}

#[test]
fn peer_ancestry_diamond_deduplicates_nodes_but_counts_every_edge_before_effects() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let first_event = f.event(a, first);
    let b = f.route(1, 2);
    let left = f.submit(b, &[first_event]);
    let left_event = f.event(b, left);
    let other_stream = f.backend.create_stream_v1(7).unwrap();
    let other_destination = f
        .backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let c = BackendDirectedPeerRouteV1 {
        stream: other_stream,
        destination: BackendMemoryRegionV1 {
            allocation: other_destination,
            ..b.destination
        },
        ..b
    };
    let right = f.submit(c, &[first_event]);
    let right_event = f.event(c, right);
    let d = f.route(2, 3);
    let join = f.submit(d, &[left_event, right_event]);
    for event in [first_event, left_event, right_event] {
        f.backend.release_event_v1(event.event).unwrap();
    }
    let owner = f.backend.next_id().unwrap();
    let before = f.backend.cooperative_progress_generation;
    for (nodes, edges) in [(3, 5), (4, 4)] {
        assert!(
            matches!(f.backend.capture_peer_launch_ancestry_with_limits_v1(owner, d.stream, &[join], nodes, edges),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
        );
        assert_eq!(f.backend.cooperative_progress_generation, before);
        assert!(f.backend.peer_launch_retains.is_empty());
    }
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_with_limits_v1(owner, d.stream, &[join], 4, 5)
        .unwrap();
    assert_eq!(ancestry.retained_ids().unwrap(), [first, left, right, join]);
    assert_eq!(ancestry.depth(), 4);
    assert_eq!(f.drive(join, d, &[left, right]), BackendPollV1::Succeeded);
    f.backend
        .validate_peer_launch_ancestry_v1(&ancestry)
        .unwrap();
    f.backend.release_submission_v1(join).unwrap();
    f.backend.release_submission_v1(right).unwrap();
    f.backend.destroy_stream_v1(other_stream).unwrap();
    f.backend.release_allocation_v1(other_destination).unwrap();
    f.clean(&[first, left]);
}

#[test]
fn peer_ancestry_transaction_retains_full_closure_only_on_uncertain_failure() {
    for mode in 0..4 {
        let mut f = Fixture::new();
        let a = f.route(0, 1);
        let first = f.submit(a, &[]);
        let b = f.route(0, 3);
        let second = f.submit(b, &[]);
        let owner = f.backend.next_id().unwrap();
        let ancestry = f
            .backend
            .capture_peer_launch_ancestry_v1(owner, b.stream, &[second])
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.backend
                .with_peer_launch_ancestry_v1(owner, Some(ancestry), |backend| {
                    assert!(backend.peer_launch_retains.retains(first));
                    assert!(backend.peer_launch_retains.retains(second));
                    let error = KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Native,
                        "ancestry test",
                    );
                    match mode {
                        0 => Err(RuntimeBackendFailureV1::Rejected(error)),
                        1 => Err(RuntimeBackendFailureV1::Quiescent(error)),
                        2 => Err(RuntimeBackendFailureV1::Terminal(error)),
                        _ => std::panic::panic_any(1234_u32),
                    }
                })
        }));
        if mode == 3 {
            assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 1234);
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(f.backend.peer_launch_retains.retains(first), mode >= 2);
        assert_eq!(f.backend.peer_launch_retains.retains(second), mode >= 2);
        // Synthetic bookkeeping test: no child entry or native custody occurred.
        f.backend.terminal = false;
        f.backend.peer_launch_retains.release(owner);
        assert!(f.backend.peer_launch_retains.is_empty());
        assert_eq!(f.drive(second, b, &[]), BackendPollV1::Succeeded);
        f.clean(&[first, second]);
    }
}

#[test]
fn peer_ancestry_keeps_original_event_roster_before_and_after_settlement() {
    for settle in 0..3 {
        let mut f = Fixture::new();
        let a = f.route(0, 1);
        let first = f.submit(a, &[]);
        let event = f.event(a, first);
        let b = f.route(1, 2);
        let second = f.submit(b, &[event]);
        f.backend.release_event_v1(event.event).unwrap();
        if settle == 2 {
            assert_eq!(f.drive(second, b, &[first]), BackendPollV1::Succeeded);
        }
        let owner = f.backend.next_id().unwrap();
        let ancestry = f
            .backend
            .capture_peer_launch_ancestry_v1(owner, b.stream, &[second])
            .unwrap();
        if settle == 1 {
            assert_eq!(f.drive(second, b, &[first]), BackendPollV1::Succeeded);
        }
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            f.backend.submissions.get_mut(&second).unwrap()
        else {
            panic!()
        };
        copy.directed.as_mut().unwrap().dependencies_mut()[0].event += 1;
        assert!(
            f.backend.directed_identity_is_intact_v1(second),
            "changed nonzero event passes the structural root check"
        );
        let before = f.backend.cooperative_progress_generation;
        assert!(matches!(
            f.backend.validate_peer_launch_ancestry_v1(&ancestry),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(f.backend.cooperative_progress_generation, before);
        assert!(f.backend.peer_launch_retains.is_empty());
        // Restore only the injected corruption so the fixture can retire normally.
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            f.backend.submissions.get_mut(&second).unwrap()
        else {
            panic!()
        };
        copy.directed.as_mut().unwrap().dependencies_mut()[0] = event;
        f.backend.terminal = false;
        assert_eq!(f.drive(second, b, &[first]), BackendPollV1::Succeeded);
        f.clean(&[first, second]);
    }
}

#[test]
fn peer_ancestry_terminal_history_still_consumes_edge_budget() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let event = f.event(a, first);
    let b = f.route(1, 2);
    let second = f.submit(b, &[event]);
    f.backend.release_event_v1(event.event).unwrap();
    assert_eq!(f.drive(second, b, &[first]), BackendPollV1::Succeeded);
    let owner = f.backend.next_id().unwrap();
    assert!(
        matches!(f.backend.capture_peer_launch_ancestry_with_limits_v1(owner, b.stream, &[second], 1, 0),
        Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    let ancestry = f
        .backend
        .capture_peer_launch_ancestry_with_limits_v1(owner, b.stream, &[second], 1, 1)
        .unwrap();
    assert_eq!(ancestry.retained_ids().unwrap(), [second]);
    f.clean(&[first, second]);
}

#[test]
fn peer_ancestry_success_depth_limit_survives_terminal_history() {
    let mut f = Fixture::new();
    let mut submissions = Vec::new();
    let mut event = None;
    for depth in 1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        let route = f.route((depth - 1) % 2, depth % 2);
        let dependencies = event.into_iter().collect::<Vec<_>>();
        let id = f.submit(route, &dependencies);
        let producers = dependencies
            .iter()
            .map(|event| event.producer_submission)
            .collect::<Vec<_>>();
        if let Some(prior) = event {
            f.backend.release_event_v1(prior.event).unwrap();
        }
        assert_eq!(f.drive(id, route, &producers), BackendPollV1::Succeeded);
        let owner = f.backend.next_id().unwrap();
        let capture = f
            .backend
            .capture_peer_launch_ancestry_v1(owner, route.stream, &[id]);
        if depth == MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            assert!(
                matches!(capture, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
            );
        } else {
            assert_eq!(capture.unwrap().depth(), depth + 1);
        }
        assert!(f.backend.peer_launch_retains.is_empty());
        event = Some(f.event(route, id));
        submissions.push(id);
    }
    f.backend.release_event_v1(event.unwrap().event).unwrap();
    f.clean(&submissions);
}
