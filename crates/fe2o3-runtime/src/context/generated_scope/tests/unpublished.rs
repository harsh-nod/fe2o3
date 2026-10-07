//! CPU ownership composition only; synthetic hooks do not admit native DATA.
use super::*;
use RuntimeGfx942ScopedCancelResultV1 as Cancel;
use std::{future::Future, pin::Pin, sync::Arc, task::Poll};

fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
    Pin::new(future).poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
}

#[test]
fn unpublished_cancellation_keeps_other_child_running_and_never_decodes_cancelled_output() {
    let mut context =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let streams = devices.map(|device| context.create_stream(device).unwrap());
    let decoded = [Cell::new(0), Cell::new(0)];
    let dropped = [Cell::new(0), Cell::new(0)];
    let prepared: Vec<_> = (0..2)
        .map(|index| {
            context.bound_multi_preparation_for_test_v1(
                devices[index],
                Borrowed {
                    ticks: Cell::new(2),
                    decoded: &decoded[index],
                    dropped: &dropped[index],
                    domain: Arc::new(()),
                    completion_order: None,
                },
            )
        })
        .collect();
    context
        .with_generated_gfx942_scope_v1(2, Instant::now() + Duration::from_secs(30), |scope| {
            scope.hooks = hooks();
            let tickets: Vec<_> = prepared
                .into_iter()
                .zip(streams)
                .map(|(prepared, stream)| scope.admit(prepared, stream).unwrap())
                .collect();
            assert_eq!(
                scope.cancel_before_publication_v1(&tickets[0]).unwrap(),
                Cancel::NotCancellable
            );
            scope.progress_v1().unwrap();
            assert_eq!(scope.slots[0].lifecycle.phase, Phase::Issuing);
            assert_eq!(
                scope.cancel_before_adoption_v1(&tickets[0]).unwrap(),
                Cancel::NotCancellable
            );
            let mut cancelled = scope.completion_future_v1(&tickets[0]).unwrap();
            let mut sibling = scope.completion_future_v1(&tickets[1]).unwrap();
            assert!(poll(&mut cancelled).is_pending());
            assert!(poll(&mut sibling).is_pending());
            assert_eq!(
                scope.cancel_before_publication_v1(&tickets[0]).unwrap(),
                Cancel::CancelledBeforePublication
            );
            assert_eq!(
                scope.cancel_before_publication_v1(&tickets[0]).unwrap(),
                Cancel::AlreadyCancelled
            );
            assert_eq!(
                scope.cancel_before_adoption_v1(&tickets[0]).unwrap(),
                Cancel::AlreadyCancelled
            );
            assert_eq!(
                (decoded[0].get(), dropped[0].get(), dropped[1].get()),
                (0, 1, 0)
            );
            assert_eq!(scope.pending_v1(), 1);
            assert!(matches!(
                poll(&mut cancelled),
                Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::CancelledBeforePublication))
            ));
            assert!(matches!(
                scope.completion_v1(&tickets[0]),
                Err(RuntimeGfx942ScopeErrorV1::CancelledBeforePublication)
            ));
            assert_eq!(
                scope
                    .context
                    .validate_unpublished_hold_v1(&scope.slots[1].hold),
                Err(RuntimeValidationErrorV1::ContextReserved)
            );
            {
                let _permit = scope.epoch.enter().unwrap();
                assert!(
                    scope
                        .context
                        .validate_unpublished_hold_v1(&scope.slots[1].hold)
                        .is_ok()
                );
            }
            assert!(matches!(
                scope.check_submission(),
                Err(RuntimeGfx942ScopeErrorV1::Capacity)
            ));
            scope.drain_v1().unwrap();
            assert!(matches!(poll(&mut sibling), Poll::Ready(Ok(()))));
            assert_eq!(
                (decoded[0].get(), decoded[1].get(), dropped[1].get()),
                (0, 1, 1)
            );
            assert!(!scope.context.has_unpublished_holds_v1());
        })
        .unwrap();
    assert!(!context.is_terminal());
    assert!(context.cleanup().is_complete());
}

#[test]
fn nonpublication_refusal_does_not_dispose_retry_poll_or_change_original_phase() {
    let mut context = context();
    let stream = context.create_stream(context.devices()[0].id()).unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let prepared = context.bound_preparation_for_test_v1(Borrowed {
        ticks: Cell::new(1),
        decoded: &decoded,
        dropped: &dropped,
        domain: Arc::new(()),
        completion_order: None,
    });
    context
        .with_generated_gfx942_scope_v1(1, Instant::now() + Duration::from_secs(30), |scope| {
            scope.hooks = hooks();
            scope.hooks.unpublished = |_, _| Ok(false);
            scope.hooks.retire_unpublished = |_, _| panic!("refusal cannot retire");
            let ticket = scope.admit(prepared, stream).unwrap();
            scope.progress_v1().unwrap();
            for _ in 0..3 {
                assert_eq!(
                    scope.cancel_before_publication_v1(&ticket).unwrap(),
                    Cancel::NotCancellable
                );
                assert_eq!(scope.slots[0].lifecycle.phase, Phase::Issuing);
                assert_eq!(
                    scope.slots[0]
                        .lifecycle
                        .value
                        .as_ref()
                        .unwrap()
                        .value()
                        .ticks
                        .get(),
                    1
                );
                assert_eq!((decoded.get(), dropped.get()), (0, 0));
                assert_eq!(
                    scope
                        .context
                        .validate_unpublished_hold_v1(&scope.slots[0].hold),
                    Err(RuntimeValidationErrorV1::ContextReserved)
                );
                {
                    let _permit = scope.epoch.enter().unwrap();
                    assert!(
                        scope
                            .context
                            .validate_unpublished_hold_v1(&scope.slots[0].hold)
                            .is_ok()
                    );
                }
            }
            let foreign = RuntimeGfx942ScopedTicketV1 {
                scope: Rc::new(()),
                index: 0,
                invariant: PhantomData,
            };
            assert!(matches!(
                scope.cancel_before_publication_v1(&foreign),
                Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
            ));
            scope.drain_v1().unwrap();
            assert_eq!(
                scope.cancel_before_publication_v1(&ticket).unwrap(),
                Cancel::NotCancellable
            );
            assert_eq!((decoded.get(), dropped.get()), (1, 1));
        })
        .unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn unpublished_disposal_order_and_refusal_unwind_keep_original_custody() {
    struct Owner<'a>(&'a RefCell<Vec<&'static str>>);
    impl Drop for Owner<'_> {
        fn drop(&mut self) {
            self.0.borrow_mut().push("carrier");
        }
    }
    for failure in [0, 1, 2, 3] {
        let order = RefCell::new(Vec::new());
        let mut lifecycle = Lifecycle::<_, ()>::new(Owner(&order));
        lifecycle.phase = Phase::Issuing;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            lifecycle.cancel_unpublished(
                &mut (),
                |_| {
                    order.borrow_mut().push("native");
                    assert_ne!(failure, 3, "injected original native disposal unwind");
                    if failure == 1 { Err(1) } else { Ok(()) }
                },
                |_| {
                    order.borrow_mut().push("hold");
                    if failure == 2 { Err(2) } else { Ok(()) }
                },
            )
        }));
        if failure == 0 {
            assert_eq!(result.unwrap(), Ok(true));
            assert_eq!(lifecycle.phase, Phase::CancelledUnpublished);
            assert_eq!(*order.borrow(), ["native", "carrier", "hold"]);
            assert!(!lifecycle.unsettled());
        } else {
            if failure == 3 {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap(), Err(failure));
            }
            assert_eq!(lifecycle.phase, Phase::Unknown);
            assert!(lifecycle.unsettled());
            assert_eq!(lifecycle.value.is_some(), failure != 2);
            assert_eq!(
                lifecycle.cancel_unpublished(
                    &mut (),
                    |_| -> Result<(), ()> { panic!("no retry") },
                    |_| panic!("no release")
                ),
                Ok(false)
            );
        }
        assert!(lifecycle.outcome.is_none());
    }
}

#[test]
fn unpublished_disposal_failure_preserves_global_unknown_and_scope_fail_stop() {
    const CHILD: &str = "FE2O3_UNPUBLISHED_CANCEL_FAILURE";
    const TEST: &str = "context::generated_scope::tests::unpublished::unpublished_disposal_failure_preserves_global_unknown_and_scope_fail_stop";
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
        let streams = [
            context.create_stream(device).unwrap(),
            context.create_stream(device).unwrap(),
        ];
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared: Vec<_> = (0..2)
            .map(|_| {
                context.bound_preparation_for_test_v1(Borrowed {
                    ticks: Cell::new(0),
                    decoded: &decoded,
                    dropped: &dropped,
                    domain: Arc::new(()),
                    completion_order: None,
                })
            })
            .collect();
        let _ = context.with_generated_gfx942_scope_v1(
            2,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.hooks.retire_unpublished = if mode == "panic" {
                    |_, _| panic!("injected disposal unwind")
                } else {
                    |_, _| Err(RuntimeValidationErrorV1::ContextReserved.into())
                };
                let tickets: Vec<_> = prepared
                    .into_iter()
                    .zip(streams)
                    .map(|(p, s)| scope.admit(p, s).unwrap())
                    .collect();
                scope.progress_v1().unwrap();
                let mut observer = scope.completion_future_v1(&tickets[0]).unwrap();
                let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    scope.cancel_before_publication_v1(&tickets[0])
                }));
                if mode == "panic" {
                    assert!(failure.is_err());
                } else {
                    assert!(failure.unwrap().is_err());
                }
                assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
                assert!(
                    scope
                        .slots
                        .iter()
                        .all(|slot| slot.lifecycle.value.is_some())
                );
                assert_eq!(
                    (decoded.get(), dropped.get(), scope.pending_v1()),
                    (0, 0, 2)
                );
                assert!(matches!(
                    scope.cancel_before_publication_v1(&tickets[1]),
                    Err(RuntimeGfx942ScopeErrorV1::Unknown)
                ));
                assert!(matches!(
                    scope.progress_v1(),
                    Err(RuntimeGfx942ScopeErrorV1::Unknown)
                ));
                assert!(matches!(
                    poll(&mut observer),
                    Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::Unknown))
                ));
                assert!(scope.context.has_unpublished_holds_v1());
                eprintln!("UNPUBLISHED_CANCEL_ORIGINALS_RETAINED");
            },
        );
        panic!("unpublished failure returned after releasing owners");
    }
    for mode in ["error", "panic"] {
        abort_child(TEST, CHILD, mode, "UNPUBLISHED_CANCEL_ORIGINALS_RETAINED");
    }
}

pub(super) fn abort_child(test: &str, variable: &str, mode: &str, marker: &str) {
    use std::{
        io::Read,
        os::unix::process::ExitStatusExt,
        process::{Child, Command, Stdio},
    };
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = OwnedChild(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(variable, mode)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut stderr = child.0.stderr.take().unwrap();
    let flags = rustix::fs::fcntl_getfl(&stderr).unwrap();
    rustix::fs::fcntl_setfl(&stderr, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut bytes = Vec::new();
    let mut terminal = None;
    loop {
        let mut block = [0; 1024];
        let eof = match stderr.read(&mut block) {
            Ok(0) => true,
            Ok(n) => {
                bytes.extend_from_slice(&block[..n]);
                false
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(e) => panic!("bounded child stderr: {e}"),
        };
        assert!(bytes.len() <= 16 * 1024, "bounded child output");
        if terminal.is_none() {
            terminal = child.0.try_wait().unwrap();
        }
        if eof && terminal.is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "owned child deadline");
        std::thread::sleep(Duration::from_millis(5));
    }
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(terminal.unwrap().signal(), Some(6), "{mode}: {text}");
    assert!(text.contains(marker), "{mode}: {text}");
    assert!(
        !text.contains("returned after releasing owners"),
        "{mode}: {text}"
    );
}
