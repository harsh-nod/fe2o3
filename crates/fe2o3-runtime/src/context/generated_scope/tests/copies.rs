use super::*;
use std::{future::Future, pin::Pin, task::Poll};

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Scope<'a> =
    RuntimeGfx942GeneratedScopeV1<'a, 'a, KfdMultiDeviceRuntimeBackendV1, Borrowed<'a>>;

fn context() -> Context {
    Context::open_with_version_journal_v1(
        KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1(),
        16,
        16,
    )
    .unwrap()
}

fn scope<'a>(context: &'a mut Context, capacity: usize) -> Scope<'a> {
    RuntimeGfx942GeneratedScopeV1 {
        epoch: context.scope_epoch.begin().unwrap(),
        context,
        slots: Vec::with_capacity(capacity),
        copies: Vec::with_capacity(capacity),
        graph: None,
        capacity,
        deadline: Instant::now() + Duration::from_secs(10),
        identity: Rc::new(()),
        invariant: PhantomData,
        hooks: hooks(),
    }
}

struct Endpoints {
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
}

fn endpoints(context: &mut Context, source_device: usize, destination_device: usize) -> Endpoints {
    let source_device = context.devices()[source_device].id();
    let destination_device = context.devices()[destination_device].id();
    let source = context
        .allocate(source_device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let destination = context
        .allocate(destination_device, RuntimeMemoryKindV1::HostVisible, 96, 8)
        .unwrap();
    context.write_allocation(source, 0, &[0x57; 64]).unwrap();
    context
        .write_allocation(destination, 0, &[0xa3; 96])
        .unwrap();
    Endpoints {
        stream: context.create_stream(destination_device).unwrap(),
        source: RuntimeMemoryRegionV1 {
            allocation: source,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 64,
        },
        destination: RuntimeMemoryRegionV1 {
            allocation: destination,
            access: RuntimeAccessV1::Write,
            byte_offset: 16,
            byte_len: 64,
        },
    }
}

fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    Pin::new(future).poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

fn check_bytes(context: &mut Context, endpoints: &Endpoints) {
    let mut source = [0; 64];
    context
        .read_allocation(endpoints.source.allocation, 0, &mut source)
        .unwrap();
    assert_eq!(source, [0x57; 64]);
    let mut destination = [0; 96];
    context
        .read_allocation(endpoints.destination.allocation, 0, &mut destination)
        .unwrap();
    let mut expected = [0xa3; 96];
    expected[16..80].fill(0x57);
    assert_eq!(destination, expected);
}

#[test]
fn async_scope_keeps_epoch_until_both_copy_and_compute_rosters_settle() {
    let mut context = context();
    let local = endpoints(&mut context, 0, 0);
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let prepared = context.bound_multi_preparation_for_test_v1(
        device,
        Borrowed {
            ticks: Cell::new(100),
            decoded: &decoded,
            dropped: &dropped,
            domain: std::sync::Arc::new(()),
            completion_order: None,
        },
    );
    futures_executor::LocalPool::new()
        .run_until(context.with_generated_gfx942_scope_async_v1(
            2,
            Instant::now() + Duration::from_secs(30),
            |_| std::future::ready(()),
            async |scope| {
                scope.hooks = hooks();
                let compute = scope.admit(prepared, stream).unwrap();
                let copy = scope
                    .copy_async_v1(local.stream, local.source, local.destination)
                    .unwrap();
                for _ in 0..30 {
                    scope.progress_v1().unwrap();
                    if scope.copy_completion_v1(&copy).unwrap().is_some() {
                        break;
                    }
                }
                assert_eq!(
                    scope.copy_completion_v1(&copy).unwrap(),
                    Some(RuntimePollV1::Succeeded)
                );
                assert!(scope.completion_v1(&compute).unwrap().is_none());
                assert_eq!(
                    (decoded.get(), dropped.get(), scope.pending_v1()),
                    (0, 0, 1)
                );
                assert!(scope.context.cleanup().is_generated_scope_reserved_v1());
                assert!(scope.context.scope_epoch.require_access().is_err());
                // Caller returns with compute still pending. Async closing retains
                // the same Context epoch and drains it before returning to caller.
            },
        ))
        .unwrap();
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    assert!(!context.scope_epoch.active());
    check_bytes(&mut context, &local);
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_copies_and_borrowed_compute_progress_independently_with_exact_release() {
    let mut context = context();
    let local = endpoints(&mut context, 0, 0);
    let peer = endpoints(&mut context, 0, 1);
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let prepared = context.bound_multi_preparation_for_test_v1(
        device,
        Borrowed {
            ticks: Cell::new(100),
            decoded: &decoded,
            dropped: &dropped,
            domain: std::sync::Arc::new(()),
            completion_order: None,
        },
    );
    let mut scope = scope(&mut context, 3);
    let compute = scope.admit(prepared, stream).unwrap();
    let local_ticket = scope
        .copy_async_v1(local.stream, local.source, local.destination)
        .unwrap();
    let peer_ticket = scope
        .peer_copy_v1(peer.stream, peer.source, peer.destination)
        .unwrap();
    let mut local_future = scope.copy_completion_future_v1(&local_ticket).unwrap();
    let mut peer_future = scope.copy_completion_future_v1(&peer_ticket).unwrap();
    assert!(matches!(
        scope.copy_completion_future_v1(&local_ticket),
        Err(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)
    ));
    assert_eq!(scope.pending_v1(), 3);
    assert_eq!(scope.context.submissions.len(), 2);
    for _ in 0..3 {
        assert!(poll(&mut local_future).is_pending());
        assert!(poll(&mut peer_future).is_pending());
    }
    assert!(
        scope
            .context
            .write_allocation(local.source.allocation, 0, &[0; 64])
            .is_err()
    );
    assert!(
        scope
            .context
            .release_allocation(peer.destination.allocation)
            .is_err()
    );
    assert!(matches!(
        scope.check_submission(),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
    for _ in 0..30 {
        scope.progress_v1().unwrap();
        if scope.copy_completion_v1(&local_ticket).unwrap().is_some()
            && scope.copy_completion_v1(&peer_ticket).unwrap().is_some()
        {
            break;
        }
    }
    assert_eq!(
        scope.copy_completion_v1(&local_ticket).unwrap(),
        Some(RuntimePollV1::Succeeded)
    );
    assert_eq!(
        scope.copy_completion_v1(&peer_ticket).unwrap(),
        Some(RuntimePollV1::Succeeded)
    );
    assert!(scope.completion_v1(&compute).unwrap().is_none());
    assert_eq!(
        (decoded.get(), dropped.get(), scope.pending_v1()),
        (0, 0, 1)
    );
    assert!(
        scope.context.submissions.is_empty(),
        "copy observers require original release"
    );
    assert!(matches!(poll(&mut local_future), Poll::Ready(Ok(()))));
    assert!(matches!(poll(&mut peer_future), Poll::Ready(Ok(()))));
    scope.drain_v1().unwrap();
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    drop((
        scope,
        compute,
        local_ticket,
        peer_ticket,
        local_future,
        peer_future,
    ));
    check_bytes(&mut context, &local);
    check_bytes(&mut context, &peer);
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_copy_observer_abandonment_and_foreign_tickets_do_not_detach_originals() {
    let mut context = context();
    let local = endpoints(&mut context, 0, 0);
    let mut scope = scope(&mut context, 1);
    let ticket = scope
        .copy_async_v1(local.stream, local.source, local.destination)
        .unwrap();
    let foreign = RuntimeGfx942ScopedCopyTicketV1 {
        scope: Rc::new(()),
        index: 0,
        invariant: PhantomData,
    };
    assert!(matches!(
        scope.copy_completion_v1(&foreign),
        Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    ));
    assert!(matches!(
        scope.copy_completion_future_v1(&foreign),
        Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    ));
    drop(scope.copy_completion_future_v1(&ticket).unwrap());
    // Losing the only public observers cannot remove the scope's original submission.
    std::mem::forget(ticket);
    assert_eq!(scope.pending_v1(), 1);
    assert_eq!(scope.context.submissions.len(), 1);
    scope.drain_v1().unwrap();
    assert_eq!(scope.pending_v1(), 0);
    assert!(scope.context.submissions.is_empty());
    drop(scope);
    check_bytes(&mut context, &local);
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_copy_refuses_graph_access_invalid_ranges_aliases_and_shared_pending_endpoints() {
    let mut context = context();
    let local = endpoints(&mut context, 0, 0);
    let other = endpoints(&mut context, 0, 0);
    let graph = context.reserve_graph_v1(1).unwrap();
    {
        let mut scope = scope(&mut context, 2);
        assert!(
            scope
                .copy_async_v1(local.stream, local.source, local.destination)
                .is_err()
        );
        assert!(scope.copies.is_empty());
    }
    context.close_graph_issue_v1(graph).unwrap();
    context.release_graph_v1(graph).unwrap();
    {
        let mut scope = scope(&mut context, 2);
        for source in [
            RuntimeMemoryRegionV1 {
                byte_len: 0,
                ..local.source
            },
            RuntimeMemoryRegionV1 {
                byte_len: 63,
                ..local.source
            },
            RuntimeMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..local.source
            },
            RuntimeMemoryRegionV1 {
                allocation: local.destination.allocation,
                ..local.source
            },
        ] {
            assert!(
                scope
                    .copy_async_v1(local.stream, source, local.destination)
                    .is_err()
            );
            assert!(scope.copies.is_empty());
            assert!(scope.context.submissions.is_empty());
        }
        let ticket = scope
            .copy_async_v1(local.stream, local.source, local.destination)
            .unwrap();
        assert!(
            scope
                .copy_async_v1(local.stream, other.source, other.destination)
                .is_err()
        );
        assert!(
            scope
                .copy_async_v1(other.stream, local.source, other.destination)
                .is_err()
        );
        assert_eq!(scope.copies.len(), 1);
        scope.drain_v1().unwrap();
        assert_eq!(
            scope.copy_completion_v1(&ticket).unwrap(),
            Some(RuntimePollV1::Succeeded)
        );
    }
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_copy_requires_original_version_journal_and_held_compute_stream_is_not_reused() {
    let mut context = Context::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let local = endpoints(&mut context, 0, 0);
    {
        let mut scope = scope(&mut context, 1);
        assert!(matches!(
            scope.copy_async_v1(local.stream, local.source, local.destination),
            Err(RuntimeGfx942ScopeErrorV1::Context(
                RuntimeErrorV1::Validation(RuntimeValidationErrorV1::Unsupported)
            ))
        ));
        assert!(scope.copies.is_empty());
    }
    assert!(context.cleanup().is_complete());
    let mut context = self::context();
    let local = endpoints(&mut context, 0, 0);
    let hold = context.hold_unpublished_stream_v1(local.stream).unwrap();
    {
        let mut scope = scope(&mut context, 1);
        assert!(
            scope
                .copy_async_v1(local.stream, local.source, local.destination)
                .is_err()
        );
        assert!(scope.copies.is_empty());
    }
    context.release_unpublished_hold_v1(&hold).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_copy_failure_notifies_observers_but_retains_originals_until_fail_stop() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_SCOPED_COPY_FAILURE";
    const TEST: &str = "context::generated_scope::tests::copies::scoped_copy_failure_notifies_observers_but_retains_originals_until_fail_stop";
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
        let local = endpoints(&mut context, 0, 0);
        let device = context.devices()[0].id();
        let compute_stream = context.create_stream(device).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_multi_preparation_for_test_v1(
            device,
            Borrowed {
                ticks: Cell::new(100),
                decoded: &decoded,
                dropped: &dropped,
                domain: std::sync::Arc::new(()),
                completion_order: None,
            },
        );
        let mut scope = scope(&mut context, 2);
        let compute = scope.admit(prepared, compute_stream).unwrap();
        let copy = scope
            .copy_async_v1(local.stream, local.source, local.destination)
            .unwrap();
        let copy_observer = scope.copy_completion_future_v1(&copy).unwrap();
        let compute_observer = scope.completion_future_v1(&compute).unwrap();
        match mode.to_str().unwrap() {
            "deadline" => scope.deadline = Instant::now(),
            "progress-error" => {
                scope.hooks.copy_progress =
                    |_, _| Err(RuntimeValidationErrorV1::ContextTerminal.into())
            }
            "unwind" => scope.hooks.copy_progress = |_, _| panic!("injected copy progress unwind"),
            _ => panic!("unknown child mode"),
        }
        if mode == "unwind" {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { scope.progress_v1() }))
                    .is_err()
            );
        } else {
            let (driver, (copy_result, compute_result)) = futures_executor::LocalPool::new()
                .run_until(futures_util::future::join(
                    scope.drive_with_wake_v1(|_| async { panic!("terminal copy may not wait") }),
                    futures_util::future::join(copy_observer, compute_observer),
                ));
            assert!(driver.is_err());
            assert!(matches!(
                copy_result,
                Err(RuntimeGfx942ScopeErrorV1::Unknown)
            ));
            assert!(matches!(
                compute_result,
                Err(RuntimeGfx942ScopeErrorV1::Unknown)
            ));
        }
        assert_eq!(
            (scope.pending_v1(), decoded.get(), dropped.get()),
            (2, 0, 0)
        );
        assert!(scope.copy_completion_v1(&copy).unwrap().is_none());
        assert!(scope.completion_v1(&compute).unwrap().is_none());
        assert_eq!(scope.context.submissions.len(), 1);
        assert_eq!(scope.context.allocations.len(), 2);
        assert!(scope.context.has_unpublished_holds_v1());
        eprintln!("copy failure retained original submission, buffers, compute carrier and hold");
        drop(scope);
        panic!("unsettled copy scope returned");
    }
    for mode in ["deadline", "progress-error", "unwind"] {
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
        assert!(!timed_out, "{mode}: observers did not resolve; {stderr}");
        assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
        assert!(stderr.contains(
            "copy failure retained original submission, buffers, compute carrier and hold"
        ));
        assert!(!stderr.contains("unsettled copy scope returned"));
    }
}

#[test]
fn scoped_copy_native_backend_rejection_does_not_install_a_submission() {
    let mut context = RuntimeContextV1::open_with_version_journal_v1(
        KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1(),
        4,
        4,
    )
    .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let mut regions = Vec::new();
    for access in [RuntimeAccessV1::Read, RuntimeAccessV1::Write] {
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        context.write_allocation(allocation, 0, &[0; 16]).unwrap();
        regions.push(RuntimeMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 16,
        });
    }
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            1,
            Instant::now() + Duration::from_secs(10),
            |scope| {
                assert!(matches!(
                    scope.copy_async_v1(stream, regions[0], regions[1]),
                    Err(RuntimeGfx942ScopeErrorV1::Context(
                        RuntimeErrorV1::BackendRejected(_)
                    ))
                ));
                assert!(scope.copies.is_empty());
                assert_eq!(scope.pending_v1(), 0);
                assert!(!scope.context.is_terminal());
                assert!(scope.context.submissions.is_empty());
            },
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}
