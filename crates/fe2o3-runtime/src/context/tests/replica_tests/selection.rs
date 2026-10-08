#![cfg(test)]

use super::*;

fn target(f: &mut Fixture, device: usize) -> RuntimeAllocationIdV1 {
    let allocation = f
        .context
        .allocate(
            f.context.devices()[device].id(),
            RuntimeMemoryKindV1::HostVisible,
            96,
            16,
        )
        .unwrap();
    f.context
        .write_allocation(allocation, 0, &[0xaa; 96])
        .unwrap();
    allocation
}

fn quote(
    f: &mut Fixture,
    source: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
    placement: Option<BackendPeerCopyPlacementV1>,
) {
    let source = f.context.allocations[&source].backend_allocation;
    let destination = f.context.allocations[&destination].backend_allocation;
    f.context
        .backend
        .peer_placement
        .insert((source, destination), placement);
}

#[test]
fn replica_selection_uses_actual_settled_versions_on_every_admitted_mock_device() {
    let mut f = Fixture::new(8);
    let mut originals = Vec::new();
    for device in 1..f.context.devices().len() {
        let copy = f.start(0, device);
        originals.push(f.settle(copy));
    }
    assert_eq!(originals.len() + 1, f.context.devices().len());
    for device in 0..f.context.devices().len() {
        let destination = target(&mut f, device);
        let candidates = [f.allocations[2], f.allocations[1], f.allocations[0]];
        let selection = f
            .context
            .select_replica_source_v1(
                region(f.allocations[0], RuntimeAccessV1::Read),
                f.streams[device],
                region(destination, RuntimeAccessV1::Write),
                &candidates,
            )
            .unwrap();
        assert_eq!(selection.source().allocation, f.allocations[device]);
        assert_eq!(selection.source_device(), f.context.devices()[device].id());
        assert_eq!(selection.peer_placement(), None);
        assert_eq!(selection.replica().is_some(), device != 0);
        let copy = f
            .context
            .submit_selected_replica_copy_v1(selection)
            .unwrap();
        let reference = f.settle(copy);
        f.context.validate_replica_v1(reference).unwrap();
        let mut bytes = [0; 96];
        f.context
            .read_allocation(destination, 0, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0x31; 96]);
        if device != 0 {
            // This new fact refers to the selected physical source, not a
            // caller-synthesized transitive relationship to allocation zero.
            let another_target = target(&mut f, 0);
            assert!(matches!(
                f.context.select_replica_source_v1(
                    region(f.allocations[0], RuntimeAccessV1::Read),
                    f.streams[0],
                    region(another_target, RuntimeAccessV1::Write),
                    &[destination]
                ),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Unsupported
                ))
            ));
        }
    }
    for reference in originals {
        f.context.validate_replica_v1(reference).unwrap();
    }
    f.finish();
}

#[test]
fn replica_selection_prefers_feasible_native_routes_then_staging_and_stable_roster_ties() {
    let mut f = Fixture::new(4);
    let copy = f.start(0, 1);
    f.settle(copy);
    let destination = target(&mut f, 2);
    let source = f.allocations[0];
    let replica = f.allocations[1];
    let candidates = [replica, source];
    let select = |f: &Fixture| {
        f.context
            .select_replica_source_v1(
                region(source, RuntimeAccessV1::Read),
                f.streams[2],
                region(destination, RuntimeAccessV1::Write),
                &candidates,
            )
            .unwrap()
    };
    assert_eq!(select(&f).source().allocation, source);
    quote(
        &mut f,
        replica,
        destination,
        Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate),
    );
    assert_eq!(select(&f).source().allocation, replica);
    quote(
        &mut f,
        replica,
        destination,
        Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes: 128 }),
    );
    assert_eq!(select(&f).source().allocation, source);
    quote(&mut f, source, destination, None);
    assert_eq!(select(&f).source().allocation, replica);
    quote(
        &mut f,
        replica,
        destination,
        Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes: 95 }),
    );
    assert!(
        f.context
            .select_replica_source_v1(
                region(source, RuntimeAccessV1::Read),
                f.streams[2],
                region(destination, RuntimeAccessV1::Write),
                &candidates
            )
            .is_err()
    );
    assert_eq!(f.context.backend.copy_call_count, 1);
    f.finish();
}

#[test]
fn replica_selection_revalidates_actual_origin_copy_destination_and_route_before_entry() {
    for changed in 0..4 {
        let mut f = Fixture::new(4);
        let copy = f.start(0, 1);
        f.settle(copy);
        let destination = target(&mut f, 2);
        let selection = f
            .context
            .select_replica_source_v1(
                region(f.allocations[0], RuntimeAccessV1::Read),
                f.streams[2],
                region(destination, RuntimeAccessV1::Write),
                &[f.allocations[1]],
            )
            .unwrap();
        if changed == 3 {
            let source = f.allocations[1];
            quote(
                &mut f,
                source,
                destination,
                Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate),
            );
        } else {
            let allocation = [f.allocations[0], f.allocations[1], destination][changed];
            f.context
                .write_allocation(allocation, 0, &[0x31; 96])
                .unwrap();
        }
        let calls = f.context.backend.copy_call_count;
        let account = f.account.usage();
        assert!(
            f.context
                .submit_selected_replica_copy_v1(selection)
                .is_err()
        );
        assert_eq!(f.context.backend.copy_call_count, calls);
        assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 0);
        assert_eq!(f.account.usage(), account);
        f.finish();
    }
}

#[test]
fn replica_selection_refuses_hash_like_equal_bytes_duplicates_and_foreign_allocations() {
    let mut f = Fixture::new(4);
    let foreign = Fixture::new(1);
    f.context
        .write_allocation(f.allocations[1], 0, &[0x31; 96])
        .unwrap();
    let destination = target(&mut f, 2);
    let origin = region(f.allocations[0], RuntimeAccessV1::Read);
    let output = region(destination, RuntimeAccessV1::Write);
    for candidates in [
        vec![],
        vec![f.allocations[1]],
        vec![f.allocations[0]; 2],
        vec![f.allocations[0]; MAX_RUNTIME_DEVICES_V1 + 1],
        vec![foreign.allocations[0]],
    ] {
        assert!(
            f.context
                .select_replica_source_v1(origin, f.streams[2], output, &candidates)
                .is_err()
        );
    }
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 0);
    foreign.finish();
    f.finish();
}
