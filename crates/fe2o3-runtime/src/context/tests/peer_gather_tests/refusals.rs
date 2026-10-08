#![cfg(test)]

use super::*;

#[test]
fn peer_gather_bind_shape_refusals_leave_nodes_available() {
    let mut f = Fixture::new(true);
    for case in 0..9 {
        let mut request = f.request(9);
        let mut shards = f.shards(2);
        match case {
            0 => shards.clear(),
            1 => shards = f.shards(9),
            2 => shards[1].node = shards[0].node,
            3 => shards[1].destination.byte_offset = shards[0].destination.byte_offset,
            4 => shards[0].source.access = RuntimeAccessV1::ReadWrite,
            5 => shards[0].destination.access = RuntimeAccessV1::ReadWrite,
            6 => shards[0].source.byte_len = 0,
            7 => shards[0].destination.byte_offset = u64::MAX,
            8 => shards[1].destination.allocation = f.sink,
            _ => unreachable!(),
        }
        assert_eq!(
            request.bind_peer_gather_v1(&shards),
            Err(RuntimeGraphValidationErrorV1::InvalidPeerGather)
        );
        // No earlier member was inserted by a late validation refusal.
        request.bind_peer_gather_v1(&f.shards(2)).unwrap();
        assert_eq!(
            request.bind_peer_gather_v1(&f.shards(2)),
            Err(RuntimeGraphValidationErrorV1::InvalidPeerGather)
        );
    }
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn peer_gather_different_streams_are_not_a_concurrent_writer_profile() {
    let mut f = Fixture::new(true);
    let second = f
        .context
        .create_stream(f.context.devices()[0].id())
        .unwrap();
    let a = f.context.completion_stream_identity_v1(f.stream).unwrap();
    let b = f.context.completion_stream_identity_v1(second).unwrap();
    let graph = CompletionGraphV1::new(
        a.context(),
        vec![a, b],
        vec![
            CompletionNodeV1::future(node(1), FutureIdentityV1::new(a, [1; 32]), None),
            CompletionNodeV1::future(node(2), FutureIdentityV1::new(b, [2; 32]), None),
        ],
    )
    .unwrap();
    let mut request = Request::new(graph, vec![(a, f.stream), (b, second)]).unwrap();
    assert_eq!(
        request.bind_peer_gather_v1(&f.shards(2)),
        Err(RuntimeGraphValidationErrorV1::InvalidPeerGather)
    );
    assert!(f.context.cleanup().is_complete());
}

fn refused_original(f: &mut Fixture, request: Request) -> RuntimeGraphErrorV1<MockError> {
    let mut original = Some(request);
    let error = match prepare(&mut f.context, &mut original) {
        Ok(_) => panic!("invalid gather admitted"),
        Err(error) => error,
    };
    assert!(original.is_some());
    assert!(f.context.graph_reservation.is_none());
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert!(f.context.submissions.is_empty());
    assert!(f.context.scalar_peer_copies.is_empty());
    error
}

#[test]
fn peer_gather_actual_context_rejects_foreign_stale_same_device_and_out_of_range_sources() {
    let mut foreign = Fixture::new(true);
    for case in 0..4 {
        let mut f = Fixture::new(true);
        let mut request = f.request(1);
        let mut shard = f.shards(1)[0];
        match case {
            0 => shard.source.allocation = foreign.sources[0],
            1 => f
                .context
                .release_allocation(shard.source.allocation)
                .unwrap(),
            2 => shard.source.allocation = f.sink,
            3 => shard.source.byte_offset = 96,
            _ => unreachable!(),
        }
        request.bind_peer_gather_v1(&[shard]).unwrap();
        let error = refused_original(&mut f, request);
        assert!(matches!(
            error,
            RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::UnknownAllocation
                    | RuntimeValidationErrorV1::WrongDevice
                    | RuntimeValidationErrorV1::InvalidRange
            ))
        ));
        assert!(f.context.cleanup().is_complete());
    }
    assert!(foreign.context.cleanup().is_complete());
}

#[test]
fn peer_gather_refuses_cross_device_producer_claims_and_other_writers_before_reservation() {
    for case in 0..3 {
        let mut f = Fixture::new(true);
        let mut request = f.request(if case == 0 { 2 } else { 3 });
        let shards = f.shards(2);
        request.bind_peer_gather_v1(&shards).unwrap();
        match case {
            0 => request
                .expect_input_version(
                    node(2),
                    shards[1].source,
                    RuntimeGraphVersionSourceV1::ProducedBy(node(1)),
                )
                .unwrap(),
            1 => request
                .bind_copy(
                    node(3),
                    region(f.sink, RuntimeAccessV1::Read, 0, 8),
                    region(f.sources[0], RuntimeAccessV1::Write, 80, 8),
                )
                .unwrap(),
            2 => request
                .bind_copy(
                    node(3),
                    region(f.sink, RuntimeAccessV1::Read, 0, 8),
                    region(f.destination, RuntimeAccessV1::Write, 80, 8),
                )
                .unwrap(),
            _ => unreachable!(),
        }
        assert!(matches!(
            refused_original(&mut f, request),
            RuntimeGraphErrorV1::Invalid(RuntimeGraphValidationErrorV1::InvalidPeerGather)
        ));
        assert_eq!(f.read(f.destination), vec![0xa7; 96]);
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn peer_gather_submission_requires_original_graph_reservation() {
    let mut f = Fixture::new(true);
    let shard = f.shards(1)[0];
    let stale = f.context.reserve_graph_v1(1).unwrap();
    f.context.close_graph_issue_v1(stale).unwrap();
    f.context.release_graph_v1(stale).unwrap();
    let prepared = f
        .context
        .prepare_graph_peer_copy_v1(f.stream, shard.source, shard.destination)
        .unwrap();
    let current = f.context.reserve_graph_v1(1).unwrap();
    assert!(matches!(
        f.context.submit_graph_action_v1(stale, prepared),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(f.context.graph_reservation, Some(current));
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert!(f.context.scalar_peer_copies.is_empty());
    f.context.close_graph_issue_v1(current).unwrap();
    f.context.release_graph_v1(current).unwrap();
    assert!(f.context.cleanup().is_complete());
}

#[test]
#[allow(unsafe_code)]
fn peer_gather_definite_backend_refusal_does_not_produce_versions_or_copy_bytes() {
    let mut f = Fixture::new(true);
    let mut request = f.request(2);
    request.bind_peer_gather_v1(&f.shards(2)).unwrap();
    let (mut graph, mut actions) = f.admit(request);
    let first = graph.pop_ready_notification().unwrap();
    assert!(graph.begin(first));
    f.context.backend.copy_failure = MockMemoryFailure::Rejected;
    assert!(matches!(
        f.context
            .submit_graph_action_v1(graph.token(), *actions[first].take().unwrap()),
        Err(RuntimeErrorV1::BackendRejected(_))
    ));
    assert!(!f.context.is_terminal());
    assert!(f.context.scalar_peer_copies.is_empty());
    assert!(f.context.submissions.is_empty());
    // SAFETY: the original backend reported definite pre-publication rejection.
    unsafe { graph.fail(first, 1) };
    assert_eq!(actions.len(), graph.len());
    for (index, action) in actions.iter_mut().enumerate() {
        if index != first {
            drop(action.take());
            // SAFETY: this exact remaining preparation was never submitted.
            unsafe { graph.cancel_unissued(index) };
        }
    }
    let report = match graph.finish(&mut f.context) {
        Ok(report) => report,
        Err(failure) => panic!("refused graph failed retirement: {:?}", failure.error),
    };
    assert!(
        report
            .versions
            .iter()
            .filter(|v| v.version.producer().is_some())
            .all(|v| v.state != RuntimeGraphVersionStateV1::Committed)
    );
    // A failed writer can invalidate the public journal; inspect the synthetic
    // backend's actual untouched bytes without claiming host-read authority.
    let native = f.context.allocations[&f.destination].backend_allocation;
    assert_eq!(f.context.backend.memory[&native], vec![0xa7; 96]);
    assert_eq!(f.context.backend.copy_call_count, 1);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn peer_gather_indeterminate_backend_failure_keeps_global_quarantine_and_original_custody() {
    for panic in [false, true] {
        let mut f = Fixture::new(true);
        let mut request = f.request(2);
        request.bind_peer_gather_v1(&f.shards(2)).unwrap();
        let (mut graph, mut actions) = f.admit(request);
        let first = graph.pop_ready_notification().unwrap();
        assert!(graph.begin(first));
        f.context.backend.copy_failure = if panic {
            MockMemoryFailure::Panic
        } else {
            MockMemoryFailure::Terminal
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.context
                .submit_graph_action_v1(graph.token(), *actions[first].take().unwrap())
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::BackendTerminal(_))
            ));
        }
        assert!(f.context.is_terminal());
        assert_eq!(f.context.scalar_peer_copies.len(), 1);
        assert!(f.context.submissions.is_empty());
        let root = f.context.scalar_peer_copies.values().next().unwrap();
        assert!(root.backend_submission.is_none());
        assert!(root.dependencies_held);
        assert_eq!(root.source.region, f.shards(2)[0].source);
        assert_eq!(root.destination.region, f.shards(2)[0].destination);
        assert_eq!(
            super::super::async_journal_tests::writer_state(&f.context, f.destination),
            fe2o3_runtime_model::ContextWriterStateV1::Unknown { member_count: 1 }
        );
        assert_eq!(f.context.backend.pending_copies.len(), 1);
        assert_eq!(f.context.backend.copy_call_count, 1);
        assert!(!graph.is_terminal());
        let failure = match graph.finish(&mut f.context) {
            Ok(_) => panic!("unknown graph cannot retire"),
            Err(failure) => failure,
        };
        assert_eq!(f.context.graph_reservation, Some(failure.graph.token()));
        assert!(!f.context.cleanup().is_complete());
        assert_eq!(f.context.scalar_peer_copies.len(), 1);
        assert_eq!(f.context.backend.copy_call_count, 1);
        // Only synthetic allocations exist in this test. No device quiescence
        // or per-device isolation is inferred from dropping the test fixture.
    }
}
