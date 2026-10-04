use fe2o3_completion::{
    COMPLETION_GRAPH_WIRE_DOMAIN_V1, CancellationCodeV1, CompletionGraphDecodeErrorV1,
    CompletionGraphErrorV1, CompletionGraphV1, CompletionNodeIdV1, CompletionNodeStateV1,
    CompletionNodeV1, CompletionTransitionErrorV1, ContextIdentityV1, DeviceIdentityV1,
    EventIdentityV1, FailureCodeV1, FutureIdentityV1, MAX_COMPLETION_GRAPH_BYTES_V1,
    MAX_COMPLETION_GRAPH_NODES_V1, MAX_COMPLETION_GRAPH_STREAMS_V1, StreamIdentityV1,
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

fn indexed_bytes(value: u32) -> [u8; 32] {
    let mut identity = [0; 32];
    identity[..4].copy_from_slice(&value.to_le_bytes());
    identity
}

fn chain_fixture(node_count: usize) -> CompletionGraphV1 {
    let context = context(21, 22);
    let stream = StreamIdentityV1::new(context, bytes(23));
    let nodes = (1..=node_count as u32)
        .map(|value| {
            CompletionNodeV1::future(
                node(value),
                FutureIdentityV1::new(stream, indexed_bytes(value)),
                (value > 1).then(|| node(value - 1)),
            )
        })
        .collect();
    CompletionGraphV1::new(context, vec![stream], nodes).unwrap()
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
fn maximum_size_chain_completes_with_incremental_successor_updates() {
    let mut authority = chain_fixture(MAX_COMPLETION_GRAPH_NODES_V1).into_completion_authority();
    for value in 1..=MAX_COMPLETION_GRAPH_NODES_V1 as u32 {
        // SAFETY: This model-only test stands in for exact quiescent backend observations.
        unsafe { authority.mark_succeeded(node(value)) }.unwrap();
    }
    assert!(authority.is_terminal());
}

#[test]
fn maximum_size_chain_propagates_one_terminal_cause_to_its_tail() {
    let mut authority = chain_fixture(MAX_COMPLETION_GRAPH_NODES_V1).into_completion_authority();
    let error = FailureCodeV1::new(31).unwrap();
    // SAFETY: This model-only test stands in for an exact quiescent backend failure.
    unsafe { authority.mark_failed(node(1), error) }.unwrap();
    assert_eq!(
        authority
            .state(node(MAX_COMPLETION_GRAPH_NODES_V1 as u32))
            .unwrap(),
        CompletionNodeStateV1::DependencyFailed {
            origin: node(1),
            error,
        }
    );
    assert!(authority.is_terminal());
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

#[test]
fn canonical_wire_round_trips_and_binds_the_terminal_report() {
    let graph = fixture();
    let canonical = graph.canonical_bytes();
    let identity = graph.identity();
    assert_eq!(
        canonical.len(),
        COMPLETION_GRAPH_WIRE_DOMAIN_V1.len() + 80 + 2 * 32 + 5 * 80
    );
    assert_eq!(identity.byte_len(), canonical.len() as u64);
    assert!(identity.matches_canonical_bytes(&canonical));

    let decoded = CompletionGraphV1::decode_canonical(&canonical).unwrap();
    assert_eq!(decoded.canonical_bytes(), canonical);
    assert_eq!(decoded.identity(), identity);

    let expected_identity = identity;
    let mut authority = decoded.into_completion_authority();
    for id in [node(1), node(2), node(3), node(4), node(5)] {
        if authority.state(id).unwrap() == CompletionNodeStateV1::Ready {
            // SAFETY: This model-only test stands in for exact quiescent completion.
            unsafe { authority.mark_succeeded(id) }.unwrap();
        }
    }
    let report = authority.try_into_report().unwrap();
    assert_eq!(report.graph_identity(), expected_identity);
}

#[test]
fn canonical_wire_and_identity_ignore_constructor_permutation() {
    let canonical = fixture().canonical_bytes();
    let identity = fixture().identity();
    let (context, mut streams, mut nodes) = fixture_parts();
    streams.reverse();
    nodes.rotate_left(2);
    let permuted = CompletionGraphV1::new(context, streams, nodes).unwrap();
    assert_eq!(permuted.canonical_bytes(), canonical);
    assert_eq!(permuted.identity(), identity);
}

#[test]
fn graph_identity_is_domain_separated_and_mutation_sensitive() {
    let graph = fixture();
    let canonical = graph.canonical_bytes();
    let identity = graph.identity();
    assert_eq!(
        hex(identity.sha256()),
        "e35b4157ebae5535295305e926a2147ecfb6ff12224a9a85d36e0ca9b18a9fbb"
    );

    let node_start = wire_node_start(2);
    let mut mutated = canonical;
    mutated[node_start + 44] ^= 1;
    let changed = CompletionGraphV1::decode_canonical(&mutated).unwrap();
    assert_ne!(changed.identity(), identity);
    assert!(!identity.matches_canonical_bytes(&mutated));
}

#[test]
fn decoder_rejects_every_truncation_and_trailing_bytes() {
    let canonical = fixture().canonical_bytes();
    for length in 0..canonical.len() {
        assert!(
            CompletionGraphV1::decode_canonical(&canonical[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }
    let mut trailing = canonical;
    trailing.push(0);
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&trailing),
        Err(CompletionGraphDecodeErrorV1::DeclaredLengthMismatch { .. })
    ));
}

#[test]
fn decoder_rejects_header_and_allocation_attacks_before_graph_construction() {
    let canonical = fixture().canonical_bytes();
    let domain_len = COMPLETION_GRAPH_WIRE_DOMAIN_V1.len();

    let mut bad_domain = canonical.clone();
    bad_domain[0] ^= 1;
    assert_eq!(
        CompletionGraphV1::decode_canonical(&bad_domain).unwrap_err(),
        CompletionGraphDecodeErrorV1::InvalidDomain
    );

    let mut bad_length = canonical.clone();
    write_u32(&mut bad_length, domain_len, canonical.len() as u32 + 1);
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&bad_length),
        Err(CompletionGraphDecodeErrorV1::DeclaredLengthMismatch { .. })
    ));

    let mut flags = canonical.clone();
    write_u32(&mut flags, domain_len + 4, 1);
    assert_eq!(
        CompletionGraphV1::decode_canonical(&flags).unwrap_err(),
        CompletionGraphDecodeErrorV1::UnsupportedFlags(1)
    );

    let mut stream_count = canonical;
    write_u32(
        &mut stream_count,
        domain_len + 72,
        MAX_COMPLETION_GRAPH_STREAMS_V1 as u32 + 1,
    );
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&stream_count),
        Err(CompletionGraphDecodeErrorV1::CountBoundExceeded {
            field: "stream",
            ..
        })
    ));

    let oversized = vec![0; MAX_COMPLETION_GRAPH_BYTES_V1 + 1];
    assert_eq!(
        CompletionGraphV1::decode_canonical(&oversized).unwrap_err(),
        CompletionGraphDecodeErrorV1::TooLarge {
            actual: MAX_COMPLETION_GRAPH_BYTES_V1 + 1,
            maximum: MAX_COMPLETION_GRAPH_BYTES_V1,
        }
    );
}

#[test]
fn decoder_rejects_invalid_tags_reserved_fields_and_zero_node_ids() {
    let canonical = fixture().canonical_bytes();
    let node_start = wire_node_start(2);

    let mut zero_node = canonical.clone();
    write_u32(&mut zero_node, node_start, 0);
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&zero_node),
        Err(CompletionGraphDecodeErrorV1::InvalidNodeId {
            field: "node identity"
        })
    ));

    let mut bad_kind = canonical.clone();
    bad_kind[node_start + 4] = 99;
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&bad_kind),
        Err(CompletionGraphDecodeErrorV1::InvalidTag {
            field: "node kind",
            actual: 99,
        })
    ));

    let mut bad_predecessor_tag = canonical.clone();
    bad_predecessor_tag[node_start + 5] = 2;
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&bad_predecessor_tag),
        Err(CompletionGraphDecodeErrorV1::InvalidTag {
            field: "stream predecessor",
            actual: 2,
        })
    ));

    let mut flags = canonical.clone();
    flags[node_start + 6] = 1;
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&flags),
        Err(CompletionGraphDecodeErrorV1::NonzeroReserved {
            field: "node flags"
        })
    ));

    let mut absent_predecessor = canonical.clone();
    write_u32(&mut absent_predecessor, node_start + 8, 1);
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&absent_predecessor),
        Err(CompletionGraphDecodeErrorV1::NonzeroReserved {
            field: "absent stream predecessor"
        })
    ));

    let mut future_record = canonical;
    write_u32(&mut future_record, node_start + 76, 1);
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&future_record),
        Err(CompletionGraphDecodeErrorV1::NonzeroReserved {
            field: "non-wait event record"
        })
    ));
}

#[test]
fn decoder_rejects_noncanonical_order_and_mutated_event_edges() {
    let canonical = fixture().canonical_bytes();
    let stream_start = COMPLETION_GRAPH_WIRE_DOMAIN_V1.len() + 80;
    let node_start = wire_node_start(2);

    let mut reordered_streams = canonical.clone();
    swap_ranges(&mut reordered_streams, stream_start, stream_start + 32, 32);
    assert_eq!(
        CompletionGraphV1::decode_canonical(&reordered_streams).unwrap_err(),
        CompletionGraphDecodeErrorV1::NonCanonical
    );

    let mut reordered_nodes = canonical.clone();
    swap_ranges(&mut reordered_nodes, node_start, node_start + 80, 80);
    assert_eq!(
        CompletionGraphV1::decode_canonical(&reordered_nodes).unwrap_err(),
        CompletionGraphDecodeErrorV1::NonCanonical
    );

    let mut substituted_record = canonical;
    write_u32(&mut substituted_record, node_start + 3 * 80 + 76, 1);
    assert!(matches!(
        CompletionGraphV1::decode_canonical(&substituted_record),
        Err(CompletionGraphDecodeErrorV1::InvalidGraph(error))
            if matches!(*error, CompletionGraphErrorV1::EventRecordMismatch { .. })
    ));
}

fn wire_node_start(stream_count: usize) -> usize {
    COMPLETION_GRAPH_WIRE_DOMAIN_V1.len() + 80 + stream_count * 32
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn swap_ranges(bytes: &mut [u8], first: usize, second: usize, length: usize) {
    for index in 0..length {
        bytes.swap(first + index, second + index);
    }
}

fn hex(bytes: [u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
