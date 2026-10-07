use super::*;
use crate::{RuntimeGraphErrorV1, RuntimeGraphRequestV1, RuntimeGraphVersionSourceV1};
use fe2o3_completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, EventIdentityV1, FutureIdentityV1,
};
use std::{future::Future, pin::Pin, task::Poll};

type Backend = KfdMultiDeviceRuntimeBackendV1;
type Context = RuntimeContextV1<Backend>;

fn id(n: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(n).unwrap()
}
fn context() -> Context {
    Context::open_with_version_journal_v1(Backend::mock_preparation_v1(), 32, 32).unwrap()
}
fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    Pin::new(future).poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}
fn diamond(context: &Context, streams: &[RuntimeStreamIdV1]) -> RuntimeGraphRequestV1<Backend> {
    let identities: Vec<_> = streams
        .iter()
        .map(|&s| context.completion_stream_identity_v1(s).unwrap())
        .collect();
    let context = identities[0].context();
    let root = EventIdentityV1::new(context, [1; 32]);
    let left = EventIdentityV1::new(context, [2; 32]);
    let right = EventIdentityV1::new(context, [3; 32]);
    let future = |n, stream, previous: Option<u32>| {
        CompletionNodeV1::future(
            id(n),
            FutureIdentityV1::new(stream, [n as u8; 32]),
            previous.map(id),
        )
    };
    let nodes = vec![
        future(1, identities[0], None),
        CompletionNodeV1::record_event(id(2), identities[0], root, Some(id(1))),
        CompletionNodeV1::wait_event(id(3), identities[1], root, id(2), None),
        future(4, identities[1], Some(3)),
        CompletionNodeV1::record_event(id(5), identities[1], left, Some(id(4))),
        CompletionNodeV1::wait_event(id(6), identities[2], root, id(2), None),
        future(7, identities[2], Some(6)),
        CompletionNodeV1::record_event(id(8), identities[2], right, Some(id(7))),
        CompletionNodeV1::wait_event(id(9), identities[3], left, id(5), None),
        CompletionNodeV1::wait_event(id(10), identities[3], right, id(8), Some(id(9))),
        future(11, identities[3], Some(10)),
    ];
    RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(context, identities.clone(), nodes).unwrap(),
        identities
            .into_iter()
            .zip(streams.iter().copied())
            .collect(),
    )
    .unwrap()
}
fn prepare<'a>(
    context: &Context,
    stream: RuntimeStreamIdV1,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
    order: &'a RefCell<Vec<usize>>,
    node: u32,
    ticks: usize,
) -> RuntimeGfx942PreparedV1<Borrowed<'a>> {
    let device = context.streams[&stream].device;
    context.bound_multi_preparation_for_test_v1(
        device,
        Borrowed {
            ticks: Cell::new(ticks),
            decoded,
            dropped,
            domain: std::sync::Arc::new(()),
            completion_order: Some((order, node as usize)),
        },
    )
}

#[test]
fn lexical_diamond_defers_original_holds_and_joins_both_generated_branch_orders() {
    for reverse in [false, true] {
        let mut context = context();
        let device = context.devices()[0].id();
        let streams: Vec<_> = (0..4)
            .map(|_| context.create_stream(device).unwrap())
            .collect();
        let request = diamond(&context, &streams);
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let order = RefCell::new(Vec::new());
        context
            .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
                11,
                Instant::now() + Duration::from_secs(30),
                |scope| {
                    scope.hooks = hooks();
                    scope.hooks.preflight = |context, _, _, _, token| {
                        assert_eq!(context.graph_reservation, token);
                        Ok(())
                    };
                    scope.hooks.adopt = |context, _, _, hold| {
                        assert_eq!(context.graph_reservation, hold.graph_access());
                        assert!(hold.is_graph_scoped_v1());
                        context.validate_unpublished_hold_v1(hold)?;
                        Ok(())
                    };
                    let ticket = scope
                        .admit_graph_with_v1::<()>(request, |context, node, stream| {
                            let n = [1, 4, 7, 11].into_iter().find(|&n| id(n) == node).unwrap();
                            let ticks = match (n, reverse) {
                                (4, false) | (7, true) => 40,
                                _ => 0,
                            };
                            Ok(prepare(
                                context, stream, &decoded, &dropped, &order, n, ticks,
                            ))
                        })
                        .unwrap();
                    assert_eq!((decoded.get(), dropped.get()), (0, 0));
                    assert!(scope.context.graph_reservation.is_some());
                    assert!(
                        !scope.context.has_unpublished_holds_v1(),
                        "no graph node was issued during admission"
                    );
                    assert!(scope.slots.is_empty());
                    assert!(
                        scope
                            .try_submit_v1(device, streams[0], |_| -> Result<Borrowed<'_>, ()> {
                                panic!("independent submission during graph");
                            })
                            .is_err()
                    );
                    let mut future = scope.graph_completion_future_v1(&ticket).unwrap();
                    assert!(scope.graph_completion_future_v1(&ticket).is_err());
                    assert!(poll(&mut future).is_pending());
                    let foreign = RuntimeGfx942ScopedGraphTicketV1 {
                        scope: Rc::new(()),
                        invariant: PhantomData,
                    };
                    assert!(scope.graph_report_v1(&foreign).is_err());
                    assert!(
                        scope
                            .graph_generated_ticket_v1(&ticket, id(11))
                            .unwrap()
                            .is_none()
                    );
                    scope.progress_v1().unwrap();
                    let original_graph_node = RuntimeGfx942ScopedTicketV1 {
                        scope: Rc::clone(&scope.identity),
                        index: 0,
                        invariant: PhantomData,
                    };
                    assert!(matches!(
                        scope.cancel_before_adoption_v1(&original_graph_node),
                        Err(RuntimeGfx942ScopeErrorV1::Graph(RuntimeGraphErrorV1::Busy))
                    ));
                    scope.drain_v1().unwrap();
                    assert!(matches!(poll(&mut future), Poll::Ready(Ok(()))));
                    assert_eq!(
                        *order.borrow(),
                        if reverse {
                            vec![1, 4, 7, 11]
                        } else {
                            vec![1, 7, 4, 11]
                        }
                    );
                    assert_eq!((decoded.get(), dropped.get()), (4, 4));
                    assert!(scope.context.graph_reservation.is_none());
                    assert!(!scope.context.has_unpublished_holds_v1());
                    let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
                    assert!(report.errors.is_empty());
                    assert_eq!(report.observations.len(), 4);
                    for n in [1, 4, 7, 11] {
                        let node_ticket = scope
                            .graph_generated_ticket_v1(&ticket, id(n))
                            .unwrap()
                            .unwrap();
                        assert!(matches!(
                            scope.completion_v1(&node_ticket).unwrap(),
                            Some(Ok(()))
                        ));
                    }
                },
            )
            .unwrap();
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn lexical_graph_copies_keep_existing_version_lineage_across_generated_ordering() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let identity = context.completion_stream_identity_v1(stream).unwrap();
    let nodes = (1..=3)
        .map(|n| {
            CompletionNodeV1::future(
                id(n),
                FutureIdentityV1::new(identity, [n as u8; 32]),
                (n > 1).then(|| id(n - 1)),
            )
        })
        .collect();
    let mut request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(identity.context(), vec![identity], nodes).unwrap(),
        vec![(identity, stream)],
    )
    .unwrap();
    let mut allocations = Vec::new();
    for fill in [0x57, 0xa3, 0x9b] {
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 96, 8)
            .unwrap();
        context
            .write_allocation(allocation, 0, &[fill; 96])
            .unwrap();
        allocations.push(allocation);
    }
    let region = |i, access| RuntimeMemoryRegionV1 {
        allocation: allocations[i],
        access,
        byte_offset: 16,
        byte_len: 64,
    };
    request
        .bind_copy(
            id(1),
            region(0, RuntimeAccessV1::Read),
            region(1, RuntimeAccessV1::Write),
        )
        .unwrap();
    request
        .bind_copy(
            id(3),
            region(1, RuntimeAccessV1::Read),
            region(2, RuntimeAccessV1::Write),
        )
        .unwrap();
    request
        .expect_input_version(
            id(3),
            region(1, RuntimeAccessV1::Read),
            RuntimeGraphVersionSourceV1::ProducedBy(id(1)),
        )
        .unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::new());
    futures_executor::LocalPool::new()
        .run_until(
            context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, _, _, _>(
                3,
                Instant::now() + Duration::from_secs(30),
                |_| std::future::ready(()),
                async |scope| {
                    scope.hooks = hooks();
                    let ticket = scope
                        .admit_graph_with_v1::<()>(request, |context, node, stream| {
                            assert_eq!(node, id(2));
                            Ok(prepare(context, stream, &decoded, &dropped, &order, 2, 3))
                        })
                        .unwrap();
                    let future = scope.graph_completion_future_v1(&ticket).unwrap();
                    let (driver, observation) = futures_util::future::join(
                        scope.drive_with_wake_v1(|_| std::future::ready(())),
                        future,
                    )
                    .await;
                    driver.unwrap();
                    observation.unwrap();
                    let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
                    assert_eq!(report.observations.len(), 3);
                    assert!(
                        report
                            .version_inputs
                            .iter()
                            .any(|input| input.consumer == id(3)
                                && input.version.producer() == Some(id(1))
                                && input.available_at_issue)
                    );
                    assert!(scope.context.submissions.is_empty());
                    assert!(scope.context.graph_reservation.is_none());
                },
            ),
        )
        .unwrap();
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    for (index, &allocation) in allocations.iter().enumerate() {
        let mut bytes = [0; 96];
        context.read_allocation(allocation, 0, &mut bytes).unwrap();
        let mut expected = [[0x57; 96], [0xa3; 96], [0x9b; 96]][index];
        expected[16..80].fill(0x57);
        assert_eq!(bytes, expected);
    }
    assert!(context.cleanup().is_complete());
}

#[test]
fn lexical_graph_capacity_and_unordered_hazards_refuse_before_reservation() {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams: Vec<_> = (0..4)
        .map(|_| context.create_stream(device).unwrap())
        .collect();
    let request = diamond(&context, &streams);
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::new());
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            4,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                assert!(
                    scope
                        .admit_graph_with_v1::<()>(request, |context, _, stream| Ok(prepare(
                            context, stream, &decoded, &dropped, &order, 0, 0
                        )))
                        .is_err()
                );
                assert_eq!((decoded.get(), dropped.get()), (0, 4));
                assert!(scope.graph.is_none());
                assert!(scope.context.graph_reservation.is_none());
                assert!(!scope.context.has_unpublished_holds_v1());
            },
        )
        .unwrap();
    let source = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let identities: Vec<_> = streams[..2]
        .iter()
        .map(|&s| context.completion_stream_identity_v1(s).unwrap())
        .collect();
    let nodes = identities
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            CompletionNodeV1::future(
                id(i as u32 + 1),
                FutureIdentityV1::new(s, [i as u8 + 1; 32]),
                None,
            )
        })
        .collect();
    let mut request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(identities[0].context(), identities.clone(), nodes).unwrap(),
        identities
            .into_iter()
            .zip(streams.iter().copied())
            .collect(),
    )
    .unwrap();
    for n in [1, 2] {
        request
            .bind_copy(
                id(n),
                RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 64,
                },
                RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 64,
                },
            )
            .unwrap();
    }
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            2,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                assert!(matches!(
                    scope.admit_graph_with_v1::<()>(request, |_, _, _| panic!(
                        "all nodes are already ordinary copies"
                    )),
                    Err(RuntimeGfx942ScopedGraphAdmissionErrorV1::Graph(
                        RuntimeGraphErrorV1::Invalid(
                            crate::RuntimeGraphValidationErrorV1::UnorderedMemoryConflict { .. }
                        )
                    ))
                ));
                assert!(scope.context.graph_reservation.is_none());
                assert!(scope.context.submissions.is_empty());
            },
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn lexical_graph_definite_activation_refusal_retires_only_unissued_descendants() {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams: Vec<_> = (0..4)
        .map(|_| context.create_stream(device).unwrap())
        .collect();
    let request = diamond(&context, &streams);
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::new());
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            11,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.hooks.preflight = |_, prepared, _, _, token| {
                    if token.is_some() && prepared.value().completion_order.as_ref().unwrap().1 == 4
                    {
                        Err(RuntimeValidationErrorV1::Capacity.into())
                    } else {
                        Ok(())
                    }
                };
                let ticket = scope
                    .admit_graph_with_v1::<()>(request, |context, node, stream| {
                        let n = [1, 4, 7, 11].into_iter().find(|&n| id(n) == node).unwrap();
                        Ok(prepare(context, stream, &decoded, &dropped, &order, n, 0))
                    })
                    .unwrap();
                scope.drain_v1().unwrap();
                assert_eq!(*order.borrow(), [1, 7]);
                assert_eq!((decoded.get(), dropped.get()), (2, 4));
                let report = scope.graph_report_v1(&ticket).unwrap().unwrap();
                assert_eq!(report.errors.len(), 1);
                assert_eq!(report.errors[0].0, id(4));
                assert!(
                    scope
                        .graph_generated_ticket_v1(&ticket, id(4))
                        .unwrap()
                        .is_none()
                );
                assert!(
                    scope
                        .graph_generated_ticket_v1(&ticket, id(11))
                        .unwrap()
                        .is_none()
                );
                assert!(scope.context.graph_reservation.is_none());
                assert!(!scope.context.has_unpublished_holds_v1());
            },
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn lexical_graph_public_preparation_does_not_promote_synthetic_checked_device() {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams: Vec<_> = (0..4)
        .map(|_| context.create_stream(device).unwrap())
        .collect();
    let request = diamond(&context, &streams);
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            11,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                assert!(matches!(
                    scope.try_admit_graph_v1(request, |_, _| -> Result<Borrowed<'_>, ()> {
                        panic!("synthetic device cannot gain native preparation authority");
                    }),
                    Err(RuntimeGfx942ScopedGraphAdmissionErrorV1::Preparation(_))
                ));
                assert!(scope.context.graph_reservation.is_none());
                assert!(scope.graph.is_none());
            },
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn lexical_graph_uses_one_original_copy_leaf_per_stream_per_progress_pass_not_flush() {
    let mut context =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let identity = context.completion_stream_identity_v1(stream).unwrap();
    let node = CompletionNodeV1::future(id(1), FutureIdentityV1::new(identity, [1; 32]), None);
    let mut request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(identity.context(), vec![identity], vec![node]).unwrap(),
        vec![(identity, stream)],
    )
    .unwrap();
    let source = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    context.write_allocation(source, 0, &[0x57; 64]).unwrap();
    request
        .bind_copy(
            id(1),
            RuntimeMemoryRegionV1 {
                allocation: source,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 64,
            },
            RuntimeMemoryRegionV1 {
                allocation: destination,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 64,
            },
        )
        .unwrap();
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                // Keep the actual production hooks. The router first advances its
                // dependency metadata, then separate Read and Write leaves. Flush
                // would execute all three transitions during the first pass.
                let ticket = scope
                    .admit_graph_with_v1::<()>(request, |_, _, _| panic!("ordinary copy only"))
                    .unwrap();
                scope.progress_v1().unwrap();
                let original = scope
                    .context
                    .submissions
                    .values()
                    .next()
                    .unwrap()
                    .backend_submission;
                // Raw inspection is private CPU-test instrumentation, not a public
                // escape from the epoch. The backend's stored poll never drives copies.
                assert!(
                    matches!(
                        scope.context.backend.poll_v1(original),
                        Ok(BackendPollV1::Pending)
                    ),
                    "the first pass advances dependency metadata, not a full flush"
                );
                assert!(scope.graph_report_v1(&ticket).unwrap().is_none());
                scope.progress_v1().unwrap();
                assert!(
                    matches!(
                        scope.context.backend.poll_v1(original),
                        Ok(BackendPollV1::Pending)
                    ),
                    "the second pass may read, but must not also write"
                );
                assert!(scope.graph_report_v1(&ticket).unwrap().is_none());
                scope.progress_v1().unwrap();
                assert!(matches!(
                    scope.context.backend.poll_v1(original),
                    Ok(BackendPollV1::Succeeded)
                ));
                assert!(
                    scope.graph_report_v1(&ticket).unwrap().is_none(),
                    "backend readiness is not Context release"
                );
                scope.drain_v1().unwrap();
                assert!(scope.graph_report_v1(&ticket).unwrap().is_some());
                assert!(scope.context.submissions.is_empty());
            },
        )
        .unwrap();
    let mut observed = [0; 64];
    context
        .read_allocation(destination, 0, &mut observed)
        .unwrap();
    assert_eq!(observed, [0x57; 64]);
    assert!(context.cleanup().is_complete());
}

#[test]
fn lexical_graph_deferred_owner_cancellation_and_forget_require_fail_stop() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_SCOPED_GRAPH_CUSTODY_CHILD";
    const TEST: &str = "context::generated_scope::tests::graph::lexical_graph_deferred_owner_cancellation_and_forget_require_fail_stop";
    if let Some(mode) = std::env::var_os(CHILD) {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let mut context = context();
        let device = context.devices()[0].id();
        let streams: Vec<_> = (0..4)
            .map(|_| context.create_stream(device).unwrap())
            .collect();
        let request = diamond(&context, &streams);
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let order = RefCell::new(Vec::new());
        let mut future = Box::pin(
            context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, (), _, _>(
                11,
                Instant::now() + Duration::from_secs(30),
                |_| std::future::pending::<()>(),
                async |scope| {
                    scope.hooks = hooks();
                    let ticket = scope
                        .admit_graph_with_v1::<()>(request, |context, _, stream| {
                            Ok(prepare(context, stream, &decoded, &dropped, &order, 0, 0))
                        })
                        .unwrap();
                    std::mem::forget(ticket);
                    assert!(scope.slots.is_empty());
                    assert!(!scope.context.has_unpublished_holds_v1());
                    std::future::pending::<()>().await;
                },
            ),
        );
        assert!(poll(&mut future).is_pending());
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        eprintln!("graph reservation and all four deferred original owners retained");
        if mode == "drop" {
            drop(future);
        } else {
            assert_eq!(mode, "forget");
            std::mem::forget(future);
            assert!(context.cleanup().is_generated_scope_reserved_v1());
            assert!(context.graph_reservation.is_some());
            assert_eq!((decoded.get(), dropped.get()), (0, 0));
            drop(context);
        }
        panic!("deferred graph owner destruction returned");
    }
    for mode in ["drop", "forget"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut timed_out = false;
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                timed_out = true;
                child.kill().unwrap();
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!timed_out, "{mode}: {stderr}");
        assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
        assert!(
            stderr.contains("graph reservation and all four deferred original owners retained"),
            "{mode}: {stderr}"
        );
        assert!(!stderr.contains("destruction returned"), "{mode}: {stderr}");
    }
}
