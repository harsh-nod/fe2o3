use fe2o3_completion::{
    CancellationCodeV1, CompletionGraphErrorV1, CompletionGraphV1, CompletionNodeIdV1,
    CompletionNodeStateV1, CompletionNodeV1, CompletionTransitionErrorV1, ContextIdentityV1,
    DeviceIdentityV1, EventIdentityV1, FailureCodeV1, FutureIdentityV1, StreamIdentityV1,
};

fn bytes(value: u8) -> [u8; 32] {
    [value; 32]
}

fn node(value: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(value).unwrap()
}

fn context(device: u8, context: u8) -> ContextIdentityV1 {
    ContextIdentityV1::new(DeviceIdentityV1::from_bytes(bytes(device)), bytes(context))
}

fn fixture_parts() -> (
    ContextIdentityV1,
    Vec<StreamIdentityV1>,
    Vec<CompletionNodeV1>,
) {
    let context = context(1, 2);
    let stream_a = StreamIdentityV1::new(context, bytes(3));
    let stream_b = StreamIdentityV1::new(context, bytes(4));
    let event = EventIdentityV1::new(context, bytes(5));
    let nodes = vec![
        CompletionNodeV1::future(node(1), FutureIdentityV1::new(stream_a, bytes(11)), None),
        CompletionNodeV1::record_event(node(2), stream_a, event, Some(node(1))),
        CompletionNodeV1::future(node(3), FutureIdentityV1::new(stream_b, bytes(13)), None),
        CompletionNodeV1::wait_event(node(4), stream_b, event, node(2), Some(node(3))),
        CompletionNodeV1::future(
            node(5),
            FutureIdentityV1::new(stream_b, bytes(15)),
            Some(node(4)),
        ),
    ];
    (context, vec![stream_a, stream_b], nodes)
}

fn fixture() -> CompletionGraphV1 {
    let (context, streams, nodes) = fixture_parts();
    CompletionGraphV1::new(context, streams, nodes).unwrap()
}

#[test]
fn validates_exact_stream_and_event_dependency_graph() {
    let graph = fixture();
    assert_eq!(graph.device(), DeviceIdentityV1::from_bytes(bytes(1)));
    assert_eq!(graph.context(), context(1, 2));
    assert_eq!(graph.streams().len(), 2);
    assert_eq!(graph.nodes().len(), 5);
    assert_eq!(
        graph.topological_order(),
        &[node(1), node(2), node(3), node(4), node(5)]
    );
    assert!(!graph.authenticates_backend_identity());
    assert!(!graph.grants_hardware_execution_authority());
}

#[test]
fn construction_is_deterministic_under_input_permutation() {
    let (context, mut streams, mut nodes) = fixture_parts();
    streams.reverse();
    nodes.reverse();
    let graph = CompletionGraphV1::new(context, streams, nodes).unwrap();
    assert_eq!(
        graph
            .nodes()
            .iter()
            .map(|node| node.id())
            .collect::<Vec<_>>(),
        vec![node(1), node(2), node(3), node(4), node(5)]
    );
    assert_eq!(
        graph.topological_order(),
        &[node(1), node(2), node(3), node(4), node(5)]
    );
}

#[test]
fn rejects_device_and_context_substitution() {
    let (graph_context, streams, mut nodes) = fixture_parts();
    let foreign_context = context(9, 2);
    let foreign_stream = StreamIdentityV1::new(foreign_context, bytes(3));
    nodes[0] = CompletionNodeV1::future(
        node(1),
        FutureIdentityV1::new(foreign_stream, bytes(11)),
        None,
    );
    assert_eq!(
        CompletionGraphV1::new(graph_context, streams, nodes).unwrap_err(),
        CompletionGraphErrorV1::ForeignStreamContext(foreign_stream)
    );

    let (graph_context, streams, mut nodes) = fixture_parts();
    let foreign_event = EventIdentityV1::new(context(1, 99), bytes(5));
    nodes[1] = CompletionNodeV1::record_event(node(2), streams[0], foreign_event, Some(node(1)));
    assert_eq!(
        CompletionGraphV1::new(graph_context, streams, nodes).unwrap_err(),
        CompletionGraphErrorV1::ForeignEventContext(foreign_event)
    );
}

#[test]
fn rejects_undeclared_duplicate_and_unused_streams() {
    let (context, streams, mut nodes) = fixture_parts();
    let undeclared = StreamIdentityV1::new(context, bytes(77));
    nodes[0] =
        CompletionNodeV1::future(node(1), FutureIdentityV1::new(undeclared, bytes(11)), None);
    assert_eq!(
        CompletionGraphV1::new(context, streams.clone(), nodes).unwrap_err(),
        CompletionGraphErrorV1::UndeclaredStream(undeclared)
    );

    let (_, _, nodes) = fixture_parts();
    assert_eq!(
        CompletionGraphV1::new(context, vec![streams[0], streams[0]], nodes).unwrap_err(),
        CompletionGraphErrorV1::DuplicateStream(streams[0])
    );

    let (_, _, nodes) = fixture_parts();
    let unused = StreamIdentityV1::new(context, bytes(88));
    let mut with_unused = streams;
    with_unused.push(unused);
    assert_eq!(
        CompletionGraphV1::new(context, with_unused, nodes).unwrap_err(),
        CompletionGraphErrorV1::UnusedStream(unused)
    );
}

#[test]
fn rejects_malformed_per_stream_chains() {
    let (context, streams, mut nodes) = fixture_parts();
    nodes[4] = CompletionNodeV1::future(
        node(5),
        FutureIdentityV1::new(streams[1], bytes(15)),
        Some(node(2)),
    );
    assert_eq!(
        CompletionGraphV1::new(context, streams.clone(), nodes).unwrap_err(),
        CompletionGraphErrorV1::CrossStreamPredecessor {
            node: node(5),
            predecessor: node(2),
        }
    );

    let (_, _, mut nodes) = fixture_parts();
    nodes[4] = CompletionNodeV1::future(
        node(5),
        FutureIdentityV1::new(streams[1], bytes(15)),
        Some(node(3)),
    );
    assert_eq!(
        CompletionGraphV1::new(context, streams.clone(), nodes).unwrap_err(),
        CompletionGraphErrorV1::StreamFork {
            predecessor: node(3),
            first: node(4),
            second: node(5),
        }
    );

    let (_, _, mut nodes) = fixture_parts();
    nodes[4] =
        CompletionNodeV1::future(node(5), FutureIdentityV1::new(streams[1], bytes(15)), None);
    assert_eq!(
        CompletionGraphV1::new(context, streams, nodes).unwrap_err(),
        CompletionGraphErrorV1::InvalidStreamHeadCount {
            stream: StreamIdentityV1::new(context, bytes(4)),
            actual: 2,
        }
    );
}

#[test]
fn rejects_event_record_substitution_and_reuse() {
    let (context, streams, mut nodes) = fixture_parts();
    nodes[3] = CompletionNodeV1::wait_event(
        node(4),
        streams[1],
        EventIdentityV1::new(context, bytes(99)),
        node(2),
        Some(node(3)),
    );
    assert_eq!(
        CompletionGraphV1::new(context, streams.clone(), nodes).unwrap_err(),
        CompletionGraphErrorV1::EventRecordMismatch {
            wait: node(4),
            recorded_by: node(2),
        }
    );

    let (_, _, mut nodes) = fixture_parts();
    nodes[3] = CompletionNodeV1::wait_event(
        node(4),
        streams[1],
        EventIdentityV1::new(context, bytes(5)),
        node(1),
        Some(node(3)),
    );
    assert_eq!(
        CompletionGraphV1::new(context, streams.clone(), nodes).unwrap_err(),
        CompletionGraphErrorV1::EventRecordMismatch {
            wait: node(4),
            recorded_by: node(1),
        }
    );

    let (_, _, mut nodes) = fixture_parts();
    nodes[4] = CompletionNodeV1::record_event(
        node(5),
        streams[1],
        EventIdentityV1::new(context, bytes(5)),
        Some(node(4)),
    );
    assert!(matches!(
        CompletionGraphV1::new(context, streams, nodes),
        Err(CompletionGraphErrorV1::DuplicateEventRecord { .. })
    ));
}

#[test]
fn rejects_duplicate_future_and_node_identities() {
    let (context, streams, mut nodes) = fixture_parts();
    nodes[4] = CompletionNodeV1::future(
        node(5),
        FutureIdentityV1::new(streams[1], bytes(13)),
        Some(node(4)),
    );
    assert_eq!(
        CompletionGraphV1::new(context, streams.clone(), nodes).unwrap_err(),
        CompletionGraphErrorV1::DuplicateFuture {
            first: node(3),
            duplicate: node(5),
        }
    );

    let (_, _, mut nodes) = fixture_parts();
    nodes[4] = CompletionNodeV1::future(
        node(4),
        FutureIdentityV1::new(streams[1], bytes(15)),
        Some(node(4)),
    );
    assert_eq!(
        CompletionGraphV1::new(context, streams, nodes).unwrap_err(),
        CompletionGraphErrorV1::DuplicateNode(node(4))
    );
}

#[test]
fn rejects_cross_stream_event_cycle() {
    let context = context(1, 2);
    let stream_a = StreamIdentityV1::new(context, bytes(3));
    let stream_b = StreamIdentityV1::new(context, bytes(4));
    let event_a = EventIdentityV1::new(context, bytes(5));
    let event_b = EventIdentityV1::new(context, bytes(6));
    let nodes = vec![
        CompletionNodeV1::wait_event(node(1), stream_a, event_b, node(4), None),
        CompletionNodeV1::record_event(node(2), stream_a, event_a, Some(node(1))),
        CompletionNodeV1::wait_event(node(3), stream_b, event_a, node(2), None),
        CompletionNodeV1::record_event(node(4), stream_b, event_b, Some(node(3))),
    ];
    assert_eq!(
        CompletionGraphV1::new(context, vec![stream_a, stream_b], nodes).unwrap_err(),
        CompletionGraphErrorV1::Cycle
    );
}

#[test]
fn success_only_unblocks_exact_dependency_successors() {
    let mut authority = fixture().into_completion_authority();
    assert_eq!(
        authority.state(node(1)).unwrap(),
        CompletionNodeStateV1::Ready
    );
    assert_eq!(
        authority.state(node(3)).unwrap(),
        CompletionNodeStateV1::Ready
    );
    assert_eq!(
        authority.state(node(2)).unwrap(),
        CompletionNodeStateV1::Blocked
    );
    assert_eq!(
        authority.state(node(4)).unwrap(),
        CompletionNodeStateV1::Blocked
    );

    // SAFETY: This model-only test stands in for exact quiescent backend observations.
    unsafe { authority.mark_succeeded(node(1)) }.unwrap();
    assert_eq!(
        authority.state(node(2)).unwrap(),
        CompletionNodeStateV1::Ready
    );
    assert_eq!(
        authority.state(node(4)).unwrap(),
        CompletionNodeStateV1::Blocked
    );
    // SAFETY: This model-only test stands in for exact quiescent backend observations.
    unsafe { authority.mark_succeeded(node(2)) }.unwrap();
    assert_eq!(
        authority.state(node(4)).unwrap(),
        CompletionNodeStateV1::Blocked
    );
    // SAFETY: This model-only test stands in for exact quiescent backend observations.
    unsafe { authority.mark_succeeded(node(3)) }.unwrap();
    assert_eq!(
        authority.state(node(4)).unwrap(),
        CompletionNodeStateV1::Ready
    );
    // SAFETY: This model-only test stands in for exact quiescent backend observations.
    unsafe { authority.mark_succeeded(node(4)) }.unwrap();
    assert_eq!(
        authority.state(node(5)).unwrap(),
        CompletionNodeStateV1::Ready
    );
    // SAFETY: This model-only test stands in for exact quiescent backend observations.
    unsafe { authority.mark_succeeded(node(5)) }.unwrap();

    let report = authority.try_into_report().unwrap();
    assert!(
        report
            .entries()
            .iter()
            .all(|entry| entry.state() == CompletionNodeStateV1::Succeeded)
    );
    assert!(!report.authenticates_backend_observations());
    assert!(!report.grants_resource_reclamation_authority());
}

#[test]
fn failures_propagate_exact_origin_and_code() {
    let mut authority = fixture().into_completion_authority();
    let error = FailureCodeV1::new(7).unwrap();
    // SAFETY: This model-only test stands in for an exact quiescent backend failure.
    unsafe { authority.mark_failed(node(1), error) }.unwrap();

    assert_eq!(
        authority.state(node(1)).unwrap(),
        CompletionNodeStateV1::Failed {
            origin: node(1),
            error,
        }
    );
    for dependent in [node(2), node(4), node(5)] {
        assert_eq!(
            authority.state(dependent).unwrap(),
            CompletionNodeStateV1::DependencyFailed {
                origin: node(1),
                error,
            }
        );
    }
    assert_eq!(
        authority.state(node(3)).unwrap(),
        CompletionNodeStateV1::Ready
    );
    // SAFETY: This model-only test stands in for exact quiescent backend completion.
    unsafe { authority.mark_succeeded(node(3)) }.unwrap();
    assert!(authority.is_terminal());
}

#[test]
fn cancellation_is_nonterminal_until_confirmed_then_propagates() {
    let mut authority = fixture().into_completion_authority();
    let first = CancellationCodeV1::new(8).unwrap();
    let substituted = CancellationCodeV1::new(9).unwrap();
    assert_eq!(authority.request_cancel(node(1), first), Ok(true));
    assert_eq!(authority.request_cancel(node(1), first), Ok(false));
    assert_eq!(
        authority.request_cancel(node(1), substituted),
        Err(
            CompletionTransitionErrorV1::CancellationReasonSubstitution {
                node: node(1),
                expected: first,
                actual: substituted,
            }
        )
    );
    assert_eq!(
        authority.state(node(1)).unwrap(),
        CompletionNodeStateV1::CancelRequested(first)
    );
    assert_eq!(
        authority.state(node(2)).unwrap(),
        CompletionNodeStateV1::Blocked
    );

    // SAFETY: This model-only test stands in for exact quiescent cancellation.
    unsafe { authority.mark_cancelled(node(1)) }.unwrap();
    assert_eq!(
        authority.state(node(1)).unwrap(),
        CompletionNodeStateV1::Cancelled {
            origin: node(1),
            reason: first,
        }
    );
    for dependent in [node(2), node(4), node(5)] {
        assert_eq!(
            authority.state(dependent).unwrap(),
            CompletionNodeStateV1::DependencyCancelled {
                origin: node(1),
                reason: first,
            }
        );
    }
}

#[test]
fn cancellation_may_lose_to_success_and_blocked_nodes_cancel_without_observation() {
    let reason = CancellationCodeV1::new(10).unwrap();
    let mut authority = fixture().into_completion_authority();
    authority.request_cancel(node(1), reason).unwrap();
    // SAFETY: This models a successful completion racing ahead of cancellation.
    unsafe { authority.mark_succeeded(node(1)) }.unwrap();
    assert_eq!(
        authority.state(node(1)).unwrap(),
        CompletionNodeStateV1::Succeeded
    );

    authority.cancel_blocked(node(4), reason).unwrap();
    assert_eq!(
        authority.state(node(4)).unwrap(),
        CompletionNodeStateV1::Cancelled {
            origin: node(4),
            reason,
        }
    );
    assert_eq!(
        authority.state(node(5)).unwrap(),
        CompletionNodeStateV1::DependencyCancelled {
            origin: node(4),
            reason,
        }
    );
}

#[test]
fn illegal_transitions_fail_closed_without_state_changes() {
    let mut authority = fixture().into_completion_authority();
    let reason = CancellationCodeV1::new(11).unwrap();
    assert!(matches!(
        authority.request_cancel(node(2), reason),
        Err(CompletionTransitionErrorV1::NotReady { .. })
    ));
    // SAFETY: The method must reject a blocked node before consuming any observation.
    assert!(matches!(
        unsafe { authority.mark_succeeded(node(2)) },
        Err(CompletionTransitionErrorV1::NotReady { .. })
    ));
    // SAFETY: No cancellation was requested, so this must fail closed.
    assert!(matches!(
        unsafe { authority.mark_cancelled(node(1)) },
        Err(CompletionTransitionErrorV1::CancellationNotRequested { .. })
    ));
    assert_eq!(
        authority.state(node(2)).unwrap(),
        CompletionNodeStateV1::Blocked
    );
    assert_eq!(
        authority.state(node(99)),
        Err(CompletionTransitionErrorV1::UnknownNode(node(99)))
    );
    assert!(authority.try_into_report().is_err());
}
