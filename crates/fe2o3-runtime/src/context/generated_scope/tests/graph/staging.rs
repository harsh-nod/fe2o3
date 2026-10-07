//! CPU transport and original synthetic decoder custody, never a GPU proof.

use super::*;
use crate::{RuntimeGfx942ScopedGraphStagingErrorV1, RuntimeGraphValidationErrorV1};
use std::sync::Arc;

mod faults;

fn chain<B: RuntimeBackendV1>(
    context: &RuntimeContextV1<B>,
    stream: RuntimeStreamIdV1,
) -> RuntimeGraphRequestV1<B> {
    let identity = context.completion_stream_identity_v1(stream).unwrap();
    RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(
            identity.context(),
            vec![identity],
            (1..=3)
                .map(|n| {
                    CompletionNodeV1::future(
                        id(n),
                        FutureIdentityV1::new(identity, [n as u8; 32]),
                        (n > 1).then(|| id(n - 1)),
                    )
                })
                .collect(),
        )
        .unwrap(),
        vec![(identity, stream)],
    )
    .unwrap()
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 32,
    }
}

#[test]
fn staging_authenticates_original_decoder_then_commits_distinct_copy_input_version() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let staging = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    context.write_allocation(staging, 0, &[0xa5; 32]).unwrap();
    context
        .write_allocation(destination, 0, &[0x3c; 32])
        .unwrap();
    let mut request = chain(&context, stream);
    request
        .bind_host_staging_v1(id(2), id(1), region(staging, RuntimeAccessV1::Write))
        .unwrap();
    request
        .bind_copy(
            id(3),
            region(staging, RuntimeAccessV1::Read),
            region(destination, RuntimeAccessV1::Write),
        )
        .unwrap();
    request
        .expect_input_version(
            id(3),
            region(staging, RuntimeAccessV1::Read),
            RuntimeGraphVersionSourceV1::ProducedBy(id(2)),
        )
        .unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let owner = Arc::new(());
    let mut scratch = [0; 32];
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            3,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                let ticket = scope
                    .admit_graph_with_v1::<()>(request, |context, node, _| {
                        assert_eq!(node, id(1));
                        Ok(context.bound_multi_preparation_for_test_v1(
                            device,
                            Borrowed {
                                ticks: Cell::new(0),
                                decoded: &decoded,
                                dropped: &dropped,
                                domain: Arc::clone(&owner),
                                completion_order: None,
                            },
                        ))
                    })
                    .unwrap();
                assert_eq!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |_, _| -> Result<Option<()>, ()> {
                                panic!("decoder has not completed")
                            }
                        )
                        .unwrap(),
                    None
                );
                for _ in 0..32 {
                    scope.progress_v1().unwrap();
                }
                assert_eq!((decoded.get(), dropped.get()), (1, 1));
                let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    scope.try_stage_graph_host_write_v1(
                        &ticket,
                        id(2),
                        id(1),
                        staging,
                        &mut scratch,
                        |domain, bytes| -> Result<Option<()>, ()> {
                            assert!(domain.matches_owner(&owner));
                            bytes[..2].fill(0x17);
                            panic!("caught encoding prefix, before graph begin or host effects")
                        },
                    )
                }));
                assert!(unwind.is_err());
                assert_eq!(&scratch[..2], &[0x17; 2]);
                assert!(!scope.context.is_terminal());
                assert!(scope.context.submissions.is_empty());
                let foreign = RuntimeGfx942ScopedGraphTicketV1 {
                    scope: Rc::new(()),
                    invariant: PhantomData,
                };
                for (ticket, node, producer, allocation) in [
                    (&foreign, id(2), id(1), staging),
                    (&ticket, id(3), id(1), staging),
                    (&ticket, id(2), id(3), staging),
                    (&ticket, id(2), id(1), destination),
                ] {
                    assert!(matches!(
                        scope.try_stage_graph_host_write_v1(
                            ticket,
                            node,
                            producer,
                            allocation,
                            &mut scratch,
                            |_, _| -> Result<Option<()>, ()> { panic!("invalid binding") }
                        ),
                        Err(RuntimeGfx942ScopedGraphStagingErrorV1::Scope(
                            RuntimeGfx942ScopeErrorV1::InvalidTicket
                        ))
                    ));
                }
                assert!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch[..31],
                            |_, _| -> Result<Option<()>, ()> { panic!("wrong extent") }
                        )
                        .is_err()
                );
                let foreign_owner = Arc::new(());
                assert!(matches!(
                    scope.try_stage_graph_host_write_v1(
                        &ticket,
                        id(2),
                        id(1),
                        staging,
                        &mut scratch,
                        |domain, _| {
                            assert!(!domain.matches_owner(&foreign_owner));
                            Err::<Option<()>, _>(17)
                        }
                    ),
                    Err(RuntimeGfx942ScopedGraphStagingErrorV1::Encoding(17))
                ));
                assert_eq!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |_, _| Ok::<_, ()>(None)
                        )
                        .unwrap(),
                    None
                );
                assert!(
                    scope.context.submissions.is_empty(),
                    "copy stays blocked on refused encodes"
                );
                let original = scope.context.graph_reservation.take().unwrap();
                assert!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |_, _| -> Result<Option<()>, ()> { panic!("stale reservation") }
                        )
                        .is_err()
                );
                scope.context.graph_reservation = Some(original);
                // Inject one hostile private observation; no production reopening
                // operation exists, and this fixture restores its original value.
                scope.context.graph_issue_closed = true;
                assert!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |_, _| -> Result<Option<()>, ()> { panic!("closed graph issue") }
                        )
                        .is_err()
                );
                scope.context.graph_issue_closed = false;
                assert_eq!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |domain, bytes| {
                                assert!(domain.matches_owner(&owner));
                                bytes.fill(0x5a);
                                Ok::<_, ()>(Some(()))
                            }
                        )
                        .unwrap(),
                    Some(())
                );
                assert!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |_, _| -> Result<Option<()>, ()> { panic!("duplicate write") }
                        )
                        .is_err()
                );
                scope.drain_v1().unwrap();
                let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
                assert!(report.errors.is_empty());
                assert_eq!(report.observations.len(), 3);
                assert!(
                    report
                        .versions
                        .iter()
                        .all(|v| v.version.producer() != Some(id(1))),
                    "generated effect set unchanged"
                );
                assert!(
                    report
                        .version_inputs
                        .iter()
                        .any(|input| input.consumer == id(3)
                            && input.version.producer() == Some(id(2))
                            && input.available_at_issue)
                );
                assert!(
                    scope
                        .try_stage_graph_host_write_v1(
                            &ticket,
                            id(2),
                            id(1),
                            staging,
                            &mut scratch,
                            |_, _| -> Result<Option<()>, ()> { panic!("retired generation") }
                        )
                        .is_err()
                );
            },
        )
        .unwrap();
    let mut observed = [0; 32];
    context
        .read_allocation(destination, 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [0x5a; 32]);
    assert!(context.cleanup().is_complete());
}

#[test]
fn staging_refuses_non_generated_producer_and_wrong_allocation_shape_precommit() {
    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        let mut context = context();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context.allocate(device, kind, 64, 8).unwrap();
        let mut request = chain(&context, stream);
        request
            .bind_host_staging_v1(id(2), id(1), region(allocation, RuntimeAccessV1::Write))
            .unwrap();
        let mut original = Some(request);
        let result = crate::async_engine::PreparedGraphAdmissionV1::prepare_scoped_v1(
            &mut context,
            &mut original,
            |_, _, _| Ok::<_, RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>>(()),
        );
        assert!(matches!(
            result,
            Err(RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidRange | RuntimeValidationErrorV1::Unsupported
            )))
        ));
        assert!(original.is_some());
        assert!(context.graph_reservation.is_none());
        assert!(context.cleanup().is_complete());
    }
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let mut request = chain(&context, stream);
    request
        .bind_host_staging_v1(id(2), id(1), region(allocation, RuntimeAccessV1::Write))
        .unwrap();
    request
        .bind_host_staging_v1(id(1), id(3), region(allocation, RuntimeAccessV1::Write))
        .unwrap();
    let mut original = Some(request);
    assert!(matches!(
        crate::async_engine::PreparedGraphAdmissionV1::prepare_scoped_v1(
            &mut context,
            &mut original,
            |_, _, _| Ok::<_, RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>>(())
        ),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::InvalidHostStaging
        ))
    ));
    assert!(original.is_some());
    assert!(context.graph_reservation.is_none());
    assert!(context.cleanup().is_complete());
}

#[test]
fn staging_requires_an_actual_predecessor_edge_and_bounded_whole_write() {
    let mut context = context();
    let device = context.devices()[0].id();
    let first = context.create_stream(device).unwrap();
    let second = context.create_stream(device).unwrap();
    let a = context.completion_stream_identity_v1(first).unwrap();
    let b = context.completion_stream_identity_v1(second).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let make = || {
        RuntimeGraphRequestV1::new(
            CompletionGraphV1::new(
                a.context(),
                vec![a, b],
                vec![
                    CompletionNodeV1::future(id(1), FutureIdentityV1::new(a, [1; 32]), None),
                    CompletionNodeV1::future(id(2), FutureIdentityV1::new(b, [2; 32]), None),
                ],
            )
            .unwrap(),
            vec![(a, first), (b, second)],
        )
        .unwrap()
    };
    for invalid in [
        RuntimeMemoryRegionV1 {
            byte_offset: 1,
            ..region(allocation, RuntimeAccessV1::Write)
        },
        RuntimeMemoryRegionV1 {
            byte_len: 0,
            ..region(allocation, RuntimeAccessV1::Write)
        },
        RuntimeMemoryRegionV1 {
            byte_len: crate::MAX_RUNTIME_GRAPH_HOST_STAGING_BYTES_V1 + 1,
            ..region(allocation, RuntimeAccessV1::Write)
        },
        region(allocation, RuntimeAccessV1::ReadWrite),
    ] {
        assert!(matches!(
            make().bind_host_staging_v1(id(2), id(1), invalid),
            Err(RuntimeGraphValidationErrorV1::InvalidHostStaging)
        ));
    }
    let mut request = make();
    request
        .bind_host_staging_v1(id(2), id(1), region(allocation, RuntimeAccessV1::Write))
        .unwrap();
    let mut original = Some(request);
    let result = crate::async_engine::PreparedGraphAdmissionV1::prepare_scoped_v1(
        &mut context,
        &mut original,
        |_, _, _| Ok::<_, RuntimeGraphErrorV1<KfdRuntimeBackendErrorV1>>(()),
    );
    assert!(matches!(
        result,
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::InvalidHostStaging
        ))
    ));
    assert!(original.is_some());
    assert!(context.graph_reservation.is_none());
    assert!(context.cleanup().is_complete());
}
