#![cfg(test)]

use super::*;

pub(super) fn simple(f: &mut Fixture, legacy: bool) -> Request {
    let group = f
        .context
        .create_graph_group_v1(&f.streams[..2], RuntimeGraphDeviceCoverageV1::Selected)
        .unwrap();
    let a = group.stream_identity(f.streams[0]).unwrap();
    let b = group.stream_identity(f.streams[1]).unwrap();
    let event = EventIdentityV1::new(group.context_identity(), [0x52; 32]);
    let graph = CompletionGraphV1::new(
        group.context_identity(),
        vec![a, b],
        vec![
            CompletionNodeV1::record_event(node(1), a, event, None),
            CompletionNodeV1::wait_event(node(2), b, event, node(1), None),
            CompletionNodeV1::future(node(3), FutureIdentityV1::new(b, [3; 32]), Some(node(2))),
        ],
    )
    .unwrap();
    if legacy {
        Request::new(graph, group.bindings()).unwrap()
    } else {
        Request::new_group_v1(graph, group).unwrap()
    }
}

#[test]
fn replica_group_exact_roster_refuses_foreign_duplicates_missing_and_overbound() {
    let mut f = Fixture::new(2);
    let foreign = Fixture::new(1);
    for streams in [
        vec![],
        vec![f.streams[0]],
        vec![f.streams[0], f.streams[0]],
        vec![f.streams[0], foreign.streams[1]],
        vec![f.streams[0]; 257],
    ] {
        assert!(
            f.context
                .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::Selected)
                .is_err()
        );
    }
    assert!(
        f.context
            .create_graph_group_v1(&f.streams[..2], RuntimeGraphDeviceCoverageV1::AllAdmitted)
            .is_err()
    );
    let selected = f
        .context
        .create_graph_group_v1(&f.streams[..2], RuntimeGraphDeviceCoverageV1::Selected)
        .unwrap();
    assert_eq!(selected.devices().len(), 2);
    assert_eq!(selected.coverage(), RuntimeGraphDeviceCoverageV1::Selected);
    assert!(selected.stream_identity(f.streams[2]).is_err());
    assert!(selected.revalidate(&foreign.context).is_err());
    let next = f
        .context
        .create_graph_group_v1(&f.streams[..2], RuntimeGraphDeviceCoverageV1::Selected)
        .unwrap();
    assert_ne!(selected.context_identity(), next.context_identity());
    f.context.destroy_stream(f.streams[1]).unwrap();
    let _replacement = f
        .context
        .create_stream(f.context.devices()[1].id())
        .unwrap();
    assert!(selected.revalidate(&f.context).is_err());
    assert!(next.revalidate(&f.context).is_err());
    foreign.finish();
    f.finish();
}

#[test]
fn replica_group_capacity_and_stale_stream_refuse_before_original_reservation() {
    let mut f = Fixture::new(1);
    let (request, _) = fanout(&mut f);
    let before = f.account.usage();
    let mut request = Some(request);
    assert!(matches!(
        prepare(&mut f.context, &mut request),
        Err(RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        )))
    ));
    assert!(request.is_some());
    assert!(f.context.graph_reservation.is_none());
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert_eq!(f.account.usage(), before);
    let mut request = simple(&mut f, false);
    request
        .bind_tracked_replica_copy_v1(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
        )
        .unwrap();
    f.context.destroy_stream(f.streams[1]).unwrap();
    let mut request = Some(request);
    assert!(prepare(&mut f.context, &mut request).is_err());
    assert!(request.is_some());
    assert!(f.context.graph_reservation.is_none());
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 0);
    f.finish();
}

#[test]
fn replica_group_cannot_relabel_legacy_graph_or_accept_unknown_producer() {
    let mut f = Fixture::new(2);
    let mut legacy = simple(&mut f, true);
    assert!(matches!(
        legacy.bind_tracked_replica_copy_v1(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write)
        ),
        Err(RuntimeGraphValidationErrorV1::InvalidReplicaCopy)
    ));
    legacy
        .bind_copy(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
        )
        .unwrap();
    assert!(matches!(
        prepare(&mut f.context, &mut Some(legacy)),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::StreamBindings
        ))
    ));
    let mut request = simple(&mut f, false);
    request
        .bind_tracked_replica_copy_v1(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
        )
        .unwrap();
    request
        .expect_input_version(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            RuntimeGraphVersionSourceV1::ProducedBy(node(99)),
        )
        .unwrap();
    assert!(prepare(&mut f.context, &mut Some(request)).is_err());
    assert_eq!(f.context.backend.copy_call_count, 0);
    f.finish();
}

#[test]
fn replica_group_refuses_partial_extent_or_source_outside_original_roster() {
    for outsider in [false, true] {
        let mut f = Fixture::new(2);
        let mut request = simple(&mut f, false);
        let mut source = region(
            f.allocations[if outsider { 2 } else { 0 }],
            RuntimeAccessV1::Read,
        );
        let mut destination = region(f.allocations[1], RuntimeAccessV1::Write);
        if !outsider {
            source.byte_len = 48;
            destination.byte_len = 48;
        }
        request
            .bind_tracked_replica_copy_v1(node(3), source, destination)
            .unwrap();
        assert!(prepare(&mut f.context, &mut Some(request)).is_err());
        assert_eq!(f.context.backend.copy_call_count, 0);
        assert!(f.context.graph_reservation.is_none());
        f.finish();
    }
}

#[test]
fn replica_group_rechecks_original_streams_at_commit_not_only_preparation() {
    let mut f = Fixture::new(2);
    let mut request = simple(&mut f, false);
    request
        .bind_tracked_replica_copy_v1(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
        )
        .unwrap();
    let plan = prepare(&mut f.context, &mut Some(request)).unwrap();
    f.context.destroy_stream(f.streams[1]).unwrap();
    let failure = match plan.commit(&mut f.context) {
        Ok(_) => panic!("stale original graph stream admitted"),
        Err(failure) => failure,
    };
    let mut recovered = failure.request;
    assert!(matches!(
        recovered.bind_tracked_replica_copy_v1(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write)
        ),
        Err(RuntimeGraphValidationErrorV1::DuplicateOperation)
    ));
    assert!(f.context.graph_reservation.is_none());
    assert_eq!(f.context.backend.copy_call_count, 0);
    f.finish();
}

#[test]
fn replica_group_refuses_unordered_whole_allocation_writers_even_for_disjoint_ranges() {
    let mut f = Fixture::new(2);
    let source = allocate(&mut f, 0, 0x92);
    let extra = f
        .context
        .create_stream(f.context.devices()[0].id())
        .unwrap();
    let streams = [f.streams[0], extra, f.streams[1]];
    let group = f
        .context
        .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::Selected)
        .unwrap();
    let ids = streams.map(|s| group.stream_identity(s).unwrap());
    let graph = CompletionGraphV1::new(
        group.context_identity(),
        ids.to_vec(),
        vec![
            CompletionNodeV1::future(node(1), FutureIdentityV1::new(ids[0], [1; 32]), None),
            CompletionNodeV1::future(node(2), FutureIdentityV1::new(ids[1], [2; 32]), None),
            CompletionNodeV1::record_event(
                node(3),
                ids[2],
                EventIdentityV1::new(group.context_identity(), [3; 32]),
                None,
            ),
        ],
    )
    .unwrap();
    let mut request = Request::new_group_v1(graph, group).unwrap();
    for (id, offset) in [(1, 0), (2, 32)] {
        request
            .bind_copy(
                node(id),
                RuntimeMemoryRegionV1 {
                    byte_offset: offset,
                    byte_len: 32,
                    ..region(source, RuntimeAccessV1::Read)
                },
                RuntimeMemoryRegionV1 {
                    byte_offset: offset,
                    byte_len: 32,
                    ..region(f.allocations[0], RuntimeAccessV1::Write)
                },
            )
            .unwrap();
    }
    assert!(matches!(
        prepare(&mut f.context, &mut Some(request)),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::UnorderedMemoryConflict { .. }
        ))
    ));
    assert!(f.context.graph_reservation.is_none());
    assert_eq!(f.context.backend.copy_call_count, 0);
    f.finish();
}
