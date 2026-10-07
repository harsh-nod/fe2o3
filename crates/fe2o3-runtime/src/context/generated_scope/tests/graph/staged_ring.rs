#![cfg(test)]
//! Synthetic decoder plus actual mock Context journal/copy machinery, not GPU proof.
use super::*;
use crate::completion::CompletionNodeStateV1;
use crate::{RuntimeGraphDeviceCoverageV1, RuntimeReplicaStorageV1};
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

mod transfer_faults;

fn nodes(index: usize) -> [CompletionNodeIdV1; 5] {
    core::array::from_fn(|offset| id((index * 5 + offset + 1) as u32))
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 16,
    }
}

fn run(fail_first: bool) {
    let mut context = context();
    assert_eq!(context.devices().len(), 3);
    let devices: [_; 3] = core::array::from_fn(|index| context.devices()[index].id());
    let compute = devices.map(|device| context.create_stream(device).unwrap());
    let transfer = core::array::from_fn::<_, 3, _>(|index| {
        context.create_stream(devices[(index + 1) % 3]).unwrap()
    });
    let metadata = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(
            ResourceKindV1::ControlResidentBytes,
            RuntimeReplicaStorageV1::required_payload_bytes_v1(3).unwrap(),
        ),
        1,
    )
    .unwrap();
    context
        .configure_replica_registry_v1(RuntimeReplicaStorageV1::preallocate(&metadata, 3).unwrap())
        .unwrap();
    let sources = devices.map(|device| {
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        context
            .write_allocation(allocation, 0, &[0xa5; 16])
            .unwrap();
        allocation
    });
    let destinations = core::array::from_fn::<_, 3, _>(|index| {
        let allocation = context
            .allocate(
                devices[(index + 1) % 3],
                RuntimeMemoryKindV1::HostVisible,
                16,
                8,
            )
            .unwrap();
        context
            .write_allocation(allocation, 0, &[0x3c; 16])
            .unwrap();
        allocation
    });
    let streams: Vec<_> = compute.into_iter().chain(transfer).collect();
    let group = context
        .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::AllAdmitted)
        .unwrap();
    let identities = streams
        .iter()
        .map(|&stream| group.stream_identity(stream).unwrap())
        .collect::<Vec<_>>();
    let mut list = Vec::new();
    for index in 0..3 {
        let ids = nodes(index);
        let event = EventIdentityV1::new(group.context_identity(), [index as u8 + 1; 32]);
        list.extend([
            CompletionNodeV1::future(
                ids[0],
                FutureIdentityV1::new(identities[index], [ids[0].get() as u8; 32]),
                None,
            ),
            CompletionNodeV1::future(
                ids[1],
                FutureIdentityV1::new(identities[index], [ids[1].get() as u8; 32]),
                Some(ids[0]),
            ),
            CompletionNodeV1::record_event(ids[2], identities[index], event, Some(ids[1])),
            CompletionNodeV1::wait_event(ids[3], identities[3 + index], event, ids[2], None),
            CompletionNodeV1::future(
                ids[4],
                FutureIdentityV1::new(identities[3 + index], [ids[4].get() as u8; 32]),
                Some(ids[3]),
            ),
        ]);
    }
    let graph = CompletionGraphV1::new(group.context_identity(), identities, list).unwrap();
    let mut request = RuntimeGraphRequestV1::new_group_v1(graph, group).unwrap();
    for index in 0..3 {
        let ids = nodes(index);
        request
            .bind_host_staging_v1(
                ids[1],
                ids[0],
                region(sources[index], RuntimeAccessV1::Write),
            )
            .unwrap();
        request
            .bind_tracked_replica_copy_v1(
                ids[4],
                region(sources[index], RuntimeAccessV1::Read),
                region(destinations[index], RuntimeAccessV1::Write),
            )
            .unwrap();
        request
            .expect_input_version(
                ids[4],
                region(sources[index], RuntimeAccessV1::Read),
                RuntimeGraphVersionSourceV1::ProducedBy(ids[1]),
            )
            .unwrap();
    }
    let decoded = [const { Cell::new(0) }; 3];
    let dropped = [const { Cell::new(0) }; 3];
    let order = RefCell::new(Vec::new());
    let owners: [_; 3] = core::array::from_fn(|_| std::sync::Arc::new(()));
    let mut scratch = [[0; 16]; 3];
    let mut staged = [false; 3];
    let stale_identity = context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| Rc::clone(&scope.identity),
        )
        .unwrap();
    let closed = context.with_generated_gfx942_scope_settled_v1::<Borrowed<'_>, _>(15,
        Instant::now() + Duration::from_secs(30), |scope| {
            scope.hooks = hooks();
            if fail_first {
                scope.hooks.decode = |prepared| {
                    if prepared.value().completion_order.is_some_and(|(_, index)| index == 0) {
                        drop(prepared); Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
                    } else { prepared.complete_readback_v1() }
                };
            }
            let ticket = scope.admit_graph_with_v1::<()>(request, |context, node, stream| {
                let index = (0..3).find(|&index| nodes(index)[0] == node).unwrap();
                Ok(context.bound_multi_preparation_for_test_v1(context.streams[&stream].device,
                    Borrowed { ticks: Cell::new(index * 3), decoded: &decoded[index], dropped: &dropped[index],
                        domain: owners[index].clone(), completion_order: Some((&order, index)) }))
            }).unwrap();
            let foreign = RuntimeGfx942ScopedGraphTicketV1 { scope: Rc::new(()), invariant: PhantomData };
            let stale = RuntimeGfx942ScopedGraphTicketV1 { scope: stale_identity.clone(), invariant: PhantomData };
            assert!(matches!(scope.graph_node_state_v1(&foreign, id(1)), Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)));
            assert!(matches!(scope.graph_node_state_v1(&stale, id(1)), Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)));
            assert!(matches!(scope.graph_node_state_v1(&ticket, id(16)), Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)));
            let pending = scope.pending_v1();
            let before = scope.context.graph_reservation;
            for _ in 0..3 {
                assert_eq!(scope.graph_node_state_v1(&ticket, id(1)).unwrap(), CompletionNodeStateV1::Ready);
                assert_eq!(scope.graph_node_state_v1(&ticket, id(2)).unwrap(), CompletionNodeStateV1::Blocked);
            }
            assert_eq!(scope.pending_v1(), pending);
            assert_eq!(scope.context.graph_reservation, before);
            assert_eq!(dropped.each_ref().map(Cell::get), [0; 3]);
            assert!(scope.graph_generated_ticket_v1(&ticket, id(1)).unwrap().is_none());
            for _ in 0..256 {
                scope.progress_v1().unwrap();
                for index in 0..3 {
                    let ids = nodes(index);
                    if !staged[index] && scope.graph_node_state_v1(&ticket, ids[1]).unwrap() == CompletionNodeStateV1::Ready {
                        staged[index] = scope.try_stage_graph_host_write_v1(&ticket, ids[1], ids[0], sources[index],
                            &mut scratch[index], |domain, bytes| {
                                assert!(domain.matches_owner(&owners[index]));
                                assert_eq!(decoded[index].get(), 1);
                                assert_eq!(dropped[index].get(), 1);
                                bytes.fill(index as u8 + 0x60);
                                Ok::<_, ()>(Some(()))
                            }).unwrap().is_some();
                    }
                }
                if scope.pending_v1() == 0 { break; }
            }
            assert_eq!(scope.pending_v1(), 0);
            let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
            assert!(report.errors.is_empty(), "{:?}", report.errors);
            assert_eq!(report.versions.len(), 12);
            assert_eq!(report.version_inputs.len(), 3);
            for index in 0..3 {
                let ids = nodes(index);
                for &node in &ids {
                    let state = scope.graph_node_state_v1(&ticket, node).unwrap();
                    assert_eq!(state, report.completion.entries()[(node.get() - 1) as usize].state());
                    if fail_first && index == 0 {
                        assert!(matches!(state, CompletionNodeStateV1::Failed { origin, .. }
                            | CompletionNodeStateV1::DependencyFailed { origin, .. } if origin == ids[0]));
                    } else { assert_eq!(state, CompletionNodeStateV1::Succeeded); }
                }
            }
            assert!(matches!(scope.graph_node_state_v1(&foreign, id(1)), Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)));
            assert!(matches!(scope.graph_node_state_v1(&stale, id(1)), Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)));
        }).unwrap();
    assert_eq!(closed.completion_v1().is_err(), fail_first);
    assert_eq!(staged, [!fail_first, true, true]);
    assert_eq!(
        decoded.each_ref().map(Cell::get),
        [usize::from(!fail_first), 1, 1]
    );
    assert_eq!(dropped.each_ref().map(Cell::get), [1; 3]);
    for index in 0..3 {
        let reference = context
            .find_current_replica_v1(sources[index], destinations[index])
            .unwrap();
        let mut actual = [0; 16];
        context
            .read_allocation(destinations[index], 0, &mut actual)
            .unwrap();
        if fail_first && index == 0 {
            assert!(reference.is_none());
            assert_eq!(actual, [0x3c; 16]);
        } else {
            context.validate_replica_v1(reference.unwrap()).unwrap();
            assert_eq!(actual, [index as u8 + 0x60; 16]);
        }
    }
    assert!(!context.scope_epoch.active());
    assert!(context.graph_reservation.is_none());
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
    drop(context);
    assert_eq!(metadata.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn staged_ring_uses_all_original_devices_and_retires_actual_replica_versions() {
    run(false);
}

#[test]
fn failed_producer_cancels_only_its_ring_transfer_while_both_other_branches_settle() {
    run(true);
}
