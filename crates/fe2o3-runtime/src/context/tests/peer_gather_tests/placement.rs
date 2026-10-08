#![cfg(test)]

use super::*;

fn sources(f: &Fixture, count: usize) -> Vec<RuntimePeerGatherSourceV1> {
    f.shards(count)
        .into_iter()
        .map(|shard| RuntimePeerGatherSourceV1 {
            node: shard.node,
            source: shard.source,
            destination_offset: shard.destination.byte_offset,
        })
        .collect()
}

fn candidates(f: &Fixture) -> [RuntimePeerGatherDestinationV1; 2] {
    [f.destination, f.sink].map(|allocation| RuntimePeerGatherDestinationV1 {
        stream: f.stream,
        allocation,
    })
}

fn quote(
    f: &mut Fixture,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    value: Option<BackendPeerCopyPlacementV1>,
) {
    let source = f.context.allocations[&source].backend_allocation;
    let destination = f.context.allocations[&destination].backend_allocation;
    f.context
        .backend
        .peer_placement
        .insert((source, destination), value);
}

#[test]
fn placement_selects_complete_minimum_and_preserves_original_order_ties_without_effects() {
    let mut f = Fixture::new(true);
    let sources = sources(&f, 3);
    let candidates = candidates(&f);
    let before = (
        f.context.backend.next,
        f.context.backend.memory.clone(),
        f.context.allocations.len(),
        f.context.streams.len(),
        f.context.version_journal_usage_v1(),
    );
    let plan = f
        .context
        .select_peer_gather_destination_v1(&sources, &candidates)
        .unwrap();
    assert_eq!(
        (
            plan.selected_index(),
            plan.staged_bytes(),
            plan.peak_staging_bytes()
        ),
        (0, 24, 8)
    );
    for source in f.sources {
        quote(
            &mut f,
            source,
            candidates[1].allocation,
            Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate),
        );
    }
    let plan = f
        .context
        .select_peer_gather_destination_v1(&sources, &candidates)
        .unwrap();
    assert_eq!(
        (
            plan.selected_index(),
            plan.staged_bytes(),
            plan.peak_staging_bytes()
        ),
        (1, 0, 0)
    );
    assert_eq!(plan.shards().len(), 3);
    assert_eq!(
        before,
        (
            f.context.backend.next,
            f.context.backend.memory.clone(),
            f.context.allocations.len(),
            f.context.streams.len(),
            f.context.version_journal_usage_v1()
        )
    );
    assert!(f.context.submissions.is_empty() && f.context.scalar_peer_copies.is_empty());
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn placement_never_selects_a_partially_feasible_candidate() {
    let mut f = Fixture::new(true);
    let sources = sources(&f, 2);
    let candidates = candidates(&f);
    quote(
        &mut f,
        sources[0].source.allocation,
        candidates[0].allocation,
        Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate),
    );
    quote(
        &mut f,
        sources[1].source.allocation,
        candidates[0].allocation,
        None,
    );
    let plan = f
        .context
        .select_peer_gather_destination_v1(&sources, &candidates)
        .unwrap();
    assert_eq!((plan.selected_index(), plan.staged_bytes()), (1, 16));
    quote(
        &mut f,
        sources[1].source.allocation,
        candidates[1].allocation,
        None,
    );
    assert!(matches!(
        f.context
            .select_peer_gather_destination_v1(&sources, &candidates),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Unsupported
        ))
    ));
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn placement_actual_graph_admission_rechecks_changed_routes_and_original_allocations() {
    let mut f = Fixture::new(true);
    let sources = sources(&f, 2);
    let candidates = &candidates(&f)[..1];
    let plan = f
        .context
        .select_peer_gather_destination_v1(&sources, candidates)
        .unwrap();
    let mut request = f.request(2);
    request.bind_selected_peer_gather_v1(plan).unwrap();
    let mut request = Some(request);
    quote(
        &mut f,
        sources[1].source.allocation,
        candidates[0].allocation,
        None,
    );
    assert!(prepare(&mut f.context, &mut request).is_err());
    assert!(request.is_some());
    assert!(f.context.submissions.is_empty() && f.context.scalar_peer_copies.is_empty());
    quote(
        &mut f,
        sources[1].source.allocation,
        candidates[0].allocation,
        Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes: 8 }),
    );
    f.context
        .release_allocation(sources[0].source.allocation)
        .unwrap();
    assert!(prepare(&mut f.context, &mut request).is_err());
    assert!(request.is_some());
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn placement_selected_graph_uses_original_settlement_and_exact_output_canaries() {
    let mut f = Fixture::new(true);
    let sources = sources(&f, 3);
    let candidates = candidates(&f);
    for source in f.sources {
        quote(
            &mut f,
            source,
            candidates[1].allocation,
            Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate),
        );
    }
    let plan = f
        .context
        .select_peer_gather_destination_v1(&sources, &candidates)
        .unwrap();
    let mut request = f.request(3);
    request.bind_selected_peer_gather_v1(plan).unwrap();
    let (graph, actions) = f.admit(request);
    let report = settle(&mut f.context, graph, actions);
    assert_eq!(
        report
            .versions
            .iter()
            .filter(|version| version.version.producer().is_some()
                && version.state == RuntimeGraphVersionStateV1::Committed)
            .count(),
        3
    );
    let mut expected = vec![0xb8; 96];
    expected[8..16].fill(0x31);
    expected[16..24].fill(0x62);
    expected[24..32].fill(0x31);
    assert_eq!(f.read(f.sink), expected);
    assert_eq!(f.read(f.destination), vec![0xa7; 96]);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn placement_bounds_aliases_foreign_inputs_and_underquoted_staging_refuse() {
    let mut f = Fixture::new(true);
    let mut input = sources(&f, 2);
    let candidates = candidates(&f);
    assert!(
        f.context
            .select_peer_gather_destination_v1(&[], &candidates)
            .is_err()
    );
    assert!(
        f.context
            .select_peer_gather_destination_v1(&input, &[])
            .is_err()
    );
    assert!(
        f.context
            .select_peer_gather_destination_v1(&vec![input[0]; 9], &candidates)
            .is_err()
    );
    assert!(
        f.context
            .select_peer_gather_destination_v1(&input, &vec![candidates[0]; 9])
            .is_err()
    );
    assert!(
        f.context
            .select_peer_gather_destination_v1(&input, &[candidates[0]; 2])
            .is_err()
    );
    input[1].destination_offset = input[0].destination_offset;
    assert!(
        f.context
            .select_peer_gather_destination_v1(&input, &candidates)
            .is_err()
    );
    input = sources(&f, 2);
    input[0].source.byte_offset = u64::MAX;
    assert!(
        f.context
            .select_peer_gather_destination_v1(&input, &candidates)
            .is_err()
    );
    let mut foreign = Fixture::new(true);
    input = sources(&f, 2);
    input[0].source.allocation = foreign.sources[0];
    assert!(matches!(
        f.context
            .select_peer_gather_destination_v1(&input, &candidates),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::UnknownAllocation
        ))
    ));
    input = sources(&f, 2);
    for candidate in candidates {
        quote(
            &mut f,
            input[0].source.allocation,
            candidate.allocation,
            Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes: 7 }),
        );
    }
    assert!(
        f.context
            .select_peer_gather_destination_v1(&input, &candidates)
            .is_err()
    );
    assert!(foreign.context.cleanup().is_complete());
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn placement_eight_by_eight_has_stable_first_candidate_and_exact_cost() {
    let mut f = Fixture::new(true);
    let device = f.context.allocations[&f.destination].device;
    let mut destinations = Vec::new();
    for _ in 0..8 {
        destinations.push(RuntimePeerGatherDestinationV1 {
            stream: f.stream,
            allocation: f
                .context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 96, 16)
                .unwrap(),
        });
    }
    let input = sources(&f, 8);
    let plan = f
        .context
        .select_peer_gather_destination_v1(&input, &destinations)
        .unwrap();
    assert_eq!(
        (
            plan.selected_index(),
            plan.shards().len(),
            plan.staged_bytes()
        ),
        (0, 8, 64)
    );
    assert!(f.context.cleanup().is_complete());
}
