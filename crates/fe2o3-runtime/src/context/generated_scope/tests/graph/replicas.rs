#![cfg(test)]

//! CPU composition of the existing synthetic decoder and actual copy journal.
//! This is not a protected native source execution or hardware overlap result.

use super::*;
use crate::{RuntimeGraphDeviceCoverageV1, RuntimeReplicaStorageV1};
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 32,
    }
}

#[test]
fn lexical_original_decoder_stages_actual_version_then_ordered_replica_fanout_and_consumers() {
    let mut context = context();
    let devices: [_; 3] = core::array::from_fn(|i| context.devices()[i].id());
    let streams = devices.map(|device| context.create_stream(device).unwrap());
    let metadata = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(
            ResourceKindV1::ControlResidentBytes,
            RuntimeReplicaStorageV1::required_payload_bytes_v1(2).unwrap(),
        ),
        1,
    )
    .unwrap();
    context
        .configure_replica_registry_v1(RuntimeReplicaStorageV1::preallocate(&metadata, 2).unwrap())
        .unwrap();
    let allocations = devices.map(|device| {
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
            .unwrap();
        context
            .write_allocation(allocation, 0, &[0xa5; 32])
            .unwrap();
        allocation
    });
    let sinks = [devices[1], devices[2]].map(|device| {
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
            .unwrap();
        context
            .write_allocation(allocation, 0, &[0x3c; 32])
            .unwrap();
        allocation
    });
    let group = context
        .create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::AllAdmitted)
        .unwrap();
    let identities = streams.map(|stream| group.stream_identity(stream).unwrap());
    let event = EventIdentityV1::new(group.context_identity(), [0x61; 32]);
    let first_consumer_done = EventIdentityV1::new(group.context_identity(), [0x62; 32]);
    let future = |n, stream, previous: Option<u32>| {
        CompletionNodeV1::future(
            id(n),
            FutureIdentityV1::new(stream, [n as u8; 32]),
            previous.map(id),
        )
    };
    let graph = CompletionGraphV1::new(
        group.context_identity(),
        identities.to_vec(),
        vec![
            future(1, identities[0], None),
            future(2, identities[0], Some(1)),
            CompletionNodeV1::record_event(id(3), identities[0], event, Some(id(2))),
            CompletionNodeV1::wait_event(id(4), identities[1], event, id(3), None),
            future(5, identities[1], Some(4)),
            future(6, identities[1], Some(5)),
            // The ordinary lower copy path requires ordered shared-source fanout.
            CompletionNodeV1::record_event(id(10), identities[1], first_consumer_done, Some(id(6))),
            CompletionNodeV1::wait_event(id(7), identities[2], first_consumer_done, id(10), None),
            future(8, identities[2], Some(7)),
            future(9, identities[2], Some(8)),
        ],
    )
    .unwrap();
    let mut request = RuntimeGraphRequestV1::new_group_v1(graph, group).unwrap();
    request
        .bind_host_staging_v1(id(2), id(1), region(allocations[0], RuntimeAccessV1::Write))
        .unwrap();
    for (copy, consume, index) in [(5, 6, 1), (8, 9, 2)] {
        request
            .bind_tracked_replica_copy_v1(
                id(copy),
                region(allocations[0], RuntimeAccessV1::Read),
                region(allocations[index], RuntimeAccessV1::Write),
            )
            .unwrap();
        request
            .expect_input_version(
                id(copy),
                region(allocations[0], RuntimeAccessV1::Read),
                RuntimeGraphVersionSourceV1::ProducedBy(id(2)),
            )
            .unwrap();
        request
            .bind_copy(
                id(consume),
                region(allocations[index], RuntimeAccessV1::Read),
                region(sinks[index - 1], RuntimeAccessV1::Write),
            )
            .unwrap();
        request
            .expect_input_version(
                id(consume),
                region(allocations[index], RuntimeAccessV1::Read),
                RuntimeGraphVersionSourceV1::ProducedBy(id(copy)),
            )
            .unwrap();
    }
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let owner = std::sync::Arc::new(());
    let mut scratch = [0; 32];
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            10,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                let ticket = scope
                    .admit_graph_with_v1::<()>(request, |context, node, _| {
                        assert_eq!(node, id(1));
                        Ok(context.bound_multi_preparation_for_test_v1(
                            devices[0],
                            Borrowed {
                                ticks: Cell::new(2),
                                decoded: &decoded,
                                dropped: &dropped,
                                domain: std::sync::Arc::clone(&owner),
                                completion_order: None,
                            },
                        ))
                    })
                    .unwrap();
                let mut staged = false;
                for _ in 0..256 {
                    scope.progress_v1().unwrap();
                    if !staged {
                        staged = scope
                            .try_stage_graph_host_write_v1(
                                &ticket,
                                id(2),
                                id(1),
                                allocations[0],
                                &mut scratch,
                                |domain, bytes| {
                                    assert!(domain.matches_owner(&owner));
                                    assert_eq!((decoded.get(), dropped.get()), (1, 1));
                                    bytes.fill(0x6d);
                                    Ok::<_, ()>(Some(()))
                                },
                            )
                            .unwrap()
                            .is_some();
                    }
                    if scope.pending_v1() == 0 {
                        break;
                    }
                }
                assert!(staged);
                assert_eq!(scope.pending_v1(), 0);
                let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
                assert!(report.errors.is_empty(), "{:?}", report.errors);
                assert_eq!(
                    report
                        .version_inputs
                        .iter()
                        .filter(|input| input.version.producer().is_some())
                        .count(),
                    4
                );
            },
        )
        .unwrap();
    for index in 1..=2 {
        let reference = context
            .find_current_replica_v1(allocations[0], allocations[index])
            .unwrap()
            .unwrap();
        context.validate_replica_v1(reference).unwrap();
        let mut bytes = [0; 32];
        context
            .read_allocation(sinks[index - 1], 0, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0x6d; 32]);
    }
    assert_eq!(metadata.usage().retained_records, 1);
    assert!(context.cleanup().is_complete());
    drop(context);
    assert_eq!(metadata.usage().used, ResourceVectorV1::ZERO);
}
