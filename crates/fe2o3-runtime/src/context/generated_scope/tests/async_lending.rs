use super::*;
use std::{future::Future, pin::Pin, task::Poll};

fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    Pin::new(future).poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

#[test]
fn unpolled_and_empty_cancelled_async_scopes_leave_context_reusable() {
    let mut context = context();
    let entered = Cell::new(0);
    let deadline = Instant::now() + Duration::from_secs(30);
    let future = context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, (), _, _>(
        1,
        deadline,
        |_| std::future::ready(()),
        async |_| {
            entered.set(entered.get() + 1);
            std::future::pending::<()>().await;
        },
    );
    drop(future);
    assert_eq!(entered.get(), 0);
    assert!(!context.scope_epoch.active());
    {
        let mut future = Box::pin(
            context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, (), _, _>(
                1,
                deadline,
                |_| std::future::ready(()),
                async |scope| {
                    entered.set(entered.get() + 1);
                    assert!(scope.context.scope_epoch.active());
                    assert!(scope.context.scope_epoch.require_access().is_err());
                    assert!(scope.context.cleanup().is_generated_scope_reserved_v1());
                    std::future::pending::<()>().await;
                },
            ),
        );
        assert!(poll(&mut future).is_pending());
        assert_eq!(entered.get(), 1);
    }
    assert!(!context.scope_epoch.active());
    futures_executor::LocalPool::new()
        .run_until(
            context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, (), _, _>(
                1,
                deadline,
                |_| std::future::ready(()),
                async |_| {},
            ),
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn async_early_callback_return_drains_original_carriers_with_caller_wakes() {
    let mut context = context();
    let device = context.devices()[0].id();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let order = RefCell::new(Vec::new());
    let inputs: Vec<_> = [5, 0, 2]
        .into_iter()
        .enumerate()
        .map(|(index, ticks)| {
            let stream = context.create_stream(device).unwrap();
            let prepared = context.bound_preparation_for_test_v1(Borrowed {
                ticks: Cell::new(ticks),
                decoded: &decoded,
                dropped: &dropped,
                domain: std::sync::Arc::new(()),
                completion_order: Some((&order, index)),
            });
            (prepared, stream)
        })
        .collect();
    let waits = Cell::new(0);
    let result =
        futures_executor::LocalPool::new().run_until(context.with_generated_gfx942_scope_async_v1(
            3,
            Instant::now() + Duration::from_secs(30),
            |_| {
                waits.set(waits.get() + 1);
                std::future::ready(())
            },
            async |scope| {
                scope.hooks = hooks();
                for (prepared, stream) in inputs {
                    std::mem::forget(scope.admit(prepared, stream).unwrap());
                }
                assert_eq!(scope.pending_v1(), 3);
                // An ordinary callback error is data, not a custody release.
                Err::<(), &'static str>("caller finished")
            },
        ));
    assert_eq!(result.unwrap(), Err("caller finished"));
    assert!(waits.get() > 0);
    assert_eq!(*order.borrow(), [1, 2, 0]);
    assert_eq!((decoded.get(), dropped.get()), (3, 3));
    assert!(!context.scope_epoch.active());
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn public_async_lending_is_awaited_inside_an_existing_tokio_current_thread_task() {
    let executor = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&executor, async {
        tokio::task::spawn_local(async {
            let mut context = context();
            let device = context.devices()[0].id();
            let stream = context.create_stream(device).unwrap();
            let decoded = Cell::new(0);
            let dropped = Cell::new(0);
            let prepared = context.bound_preparation_for_test_v1(Borrowed {
                ticks: Cell::new(5),
                decoded: &decoded,
                dropped: &dropped,
                domain: std::sync::Arc::new(()),
                completion_order: None,
            });
            let sibling_progress = Rc::new(Cell::new(false));
            let sibling_observation = Rc::clone(&sibling_progress);
            let sibling = tokio::task::spawn_local(async move {
                tokio::task::yield_now().await;
                sibling_progress.set(true);
            });
            let waits = Cell::new(0);
            context
                .with_generated_gfx942_scope_async_v1(
                    1,
                    Instant::now() + Duration::from_secs(30),
                    |deadline| {
                        waits.set(waits.get() + 1);
                        tokio::time::sleep_until(
                            std::cmp::min(deadline, Instant::now() + Duration::from_millis(1))
                                .into(),
                        )
                    },
                    async |scope| {
                        scope.hooks = hooks();
                        let _ticket = scope.admit(prepared, stream).unwrap();
                        tokio::task::yield_now().await;
                        assert_eq!(scope.pending_v1(), 1);
                        assert!(scope.context.scope_epoch.require_access().is_err());
                    },
                )
                .await
                .unwrap();
            sibling.await.unwrap();
            assert!(sibling_observation.get());
            assert!(waits.get() > 0);
            assert_eq!((decoded.get(), dropped.get()), (1, 1));
            assert!(!context.scope_epoch.active());
            assert!(context.cleanup().is_complete());
        })
        .await
        .unwrap();
    });
}

#[test]
fn forgotten_empty_async_scope_refuses_backend_mutation_cleanup_and_shutdown() {
    let mut context = context();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let before = (context.streams.len(), context.allocations.len());
    let mut future = Box::pin(
        context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, (), _, _>(
            1,
            Instant::now() + Duration::from_secs(30),
            |_| std::future::ready(()),
            async |_| std::future::pending::<()>().await,
        ),
    );
    assert!(poll(&mut future).is_pending());
    std::mem::forget(future);
    let report = context.cleanup();
    assert!(report.is_generated_scope_reserved_v1());
    assert!(!report.is_complete());
    assert!(context.create_stream(device).is_err());
    assert!(context.destroy_stream(stream).is_err());
    assert!(
        context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .is_err()
    );
    assert!(context.release_allocation(allocation).is_err());
    assert!(context.write_allocation(allocation, 0, &[0; 8]).is_err());
    assert!(context.read_allocation(allocation, 0, &mut [0; 8]).is_err());
    assert!(context.progress_stream_v1(stream).is_err());
    assert!(context.flush_stream(stream).is_err());
    assert!(context.reserve_graph_v1(1).is_err());
    assert!(context.execution_capabilities(device).is_err());
    assert!(
        context
            .with_gfx942_preparation_device_v1(device, |_| Ok::<(), ()>(()))
            .is_err()
    );
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| context.backend())).is_err());
    assert_eq!((context.streams.len(), context.allocations.len()), before);
    let failure = context.shutdown().unwrap_err();
    assert!(failure.report().is_generated_scope_reserved_v1());
    assert!(!failure.report().is_complete());
    assert_eq!(
        (
            failure.context().streams.len(),
            failure.context().allocations.len()
        ),
        before
    );
    // The death test below checks destruction. This case intentionally retains
    // the small CPU-only fixture so all refusals can be checked in one process.
    std::mem::forget(failure);
}

#[test]
fn forgotten_or_cancelled_async_native_custody_fails_stop_before_owner_release() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_ASYNC_SCOPE_CUSTODY";
    const TEST: &str = "context::generated_scope::tests::async_lending::forgotten_or_cancelled_async_native_custody_fails_stop_before_owner_release";
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
        let stream = context.create_stream(device).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let pending = mode != "forget-empty";
        let prepared = pending.then(|| {
            context.bound_preparation_for_test_v1(Borrowed {
                ticks: Cell::new(100),
                decoded: &decoded,
                dropped: &dropped,
                domain: std::sync::Arc::new(()),
                completion_order: None,
            })
        });
        let unwind = mode == "unwind-pending";
        let mut future = Box::pin(
            context.with_generated_gfx942_scope_async_v1::<Borrowed<'_>, (), _, _>(
                1,
                Instant::now() + Duration::from_secs(30),
                |_| std::future::pending::<()>(),
                async |scope| {
                    scope.hooks = hooks();
                    if let Some(prepared) = prepared {
                        let _ticket = scope.admit(prepared, stream).unwrap();
                    }
                    if unwind {
                        panic!("injected callback unwind with original carrier retained");
                    }
                    std::future::pending::<()>().await;
                },
            ),
        );
        if unwind {
            eprintln!("original custody retained; fail-stop required");
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| poll(&mut future)));
            panic!("unsettled callback unwind returned");
        }
        assert!(poll(&mut future).is_pending());
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        eprintln!("original custody retained; fail-stop required");
        if mode == "drop-pending" {
            drop(future);
        } else {
            assert!(mode == "forget-pending" || mode == "forget-empty");
            std::mem::forget(future);
            assert!(context.cleanup().is_generated_scope_reserved_v1());
            assert_eq!((decoded.get(), dropped.get()), (0, 0));
            drop(context);
        }
        panic!("unsettled async custody destruction returned");
    }
    for mode in [
        "forget-empty",
        "forget-pending",
        "drop-pending",
        "unwind-pending",
    ] {
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
            stderr.contains("original custody retained; fail-stop required"),
            "{mode}: {stderr}"
        );
        assert!(!stderr.contains("destruction returned"), "{mode}: {stderr}");
    }
}
