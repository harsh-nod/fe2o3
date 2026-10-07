use super::*;
use RuntimeGfx942ScopedCancelResultV1 as Cancel;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Poll, Wake, Waker},
};

fn poll<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
    Pin::new(future).poll(&mut std::task::Context::from_waker(waker))
}
struct Counter(AtomicUsize);
impl Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn cancelling_never_adopted_carrier_disposes_once_notifies_and_keeps_ticket_quota() {
    for late_observer in [false, true] {
        let mut context = context();
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_preparation_for_test_v1(Borrowed {
            ticks: Cell::new(5),
            decoded: &decoded,
            dropped: &dropped,
            domain: Arc::new(()),
            completion_order: None,
        });
        context
            .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
                1,
                Instant::now() + Duration::from_secs(30),
                |scope| {
                    scope.hooks = hooks();
                    let ticket = scope.admit(prepared, stream).unwrap();
                    let domain = Arc::clone(
                        &scope.slots[0]
                            .lifecycle
                            .value
                            .as_ref()
                            .unwrap()
                            .value()
                            .domain,
                    );
                    let mut future =
                        (!late_observer).then(|| scope.completion_future_v1(&ticket).unwrap());
                    let old = Arc::new(Counter(AtomicUsize::new(0)));
                    let latest = Arc::new(Counter(AtomicUsize::new(0)));
                    if let Some(future) = &mut future {
                        assert!(poll(future, &Waker::from(old.clone())).is_pending());
                        assert!(poll(future, &Waker::from(latest.clone())).is_pending());
                    }
                    let foreign = RuntimeGfx942ScopedTicketV1 {
                        scope: Rc::new(()),
                        index: 0,
                        invariant: PhantomData,
                    };
                    assert!(matches!(
                        scope.cancel_before_adoption_v1(&foreign),
                        Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
                    ));
                    assert_eq!(
                        scope.cancel_before_adoption_v1(&ticket).unwrap(),
                        Cancel::CancelledBeforeSubmission
                    );
                    assert_eq!(
                        scope.cancel_before_adoption_v1(&ticket).unwrap(),
                        Cancel::AlreadyCancelled
                    );
                    assert_eq!(
                        (decoded.get(), dropped.get(), scope.pending_v1()),
                        (0, 1, 0)
                    );
                    assert!(!scope.context.has_unpublished_holds_v1());
                    assert_eq!(scope.slots.len(), 1);
                    assert!(matches!(
                        scope.check_submission(),
                        Err(RuntimeGfx942ScopeErrorV1::Capacity)
                    ));
                    assert!(matches!(
                        scope.completion_v1(&ticket),
                        Err(RuntimeGfx942ScopeErrorV1::CancelledBeforeSubmission)
                    ));
                    assert!(matches!(
                        scope.completion_matches_owner_v1(&ticket, &domain),
                        Err(RuntimeGfx942ScopeErrorV1::CancelledBeforeSubmission)
                    ));
                    if late_observer {
                        future = Some(scope.completion_future_v1(&ticket).unwrap());
                    }
                    assert!(matches!(
                        poll(future.as_mut().unwrap(), Waker::noop()),
                        Poll::Ready(Err(RuntimeGfx942ScopeErrorV1::CancelledBeforeSubmission))
                    ));
                    assert_eq!(old.0.load(Ordering::SeqCst), 0);
                    assert_eq!(latest.0.load(Ordering::SeqCst), usize::from(!late_observer));
                    assert_eq!(scope.progress_v1().unwrap(), 0);
                    assert_eq!((decoded.get(), dropped.get()), (0, 1));
                },
            )
            .unwrap();
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn readiness_only_pending_can_cancel_but_every_post_adoption_phase_refuses() {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams = [
        context.create_stream(device).unwrap(),
        context.create_stream(device).unwrap(),
    ];
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let mut prepared: Vec<_> = (0..2)
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
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            2,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.hooks.ready = |_, _| Ok(false);
                let first = scope.admit(prepared.remove(0), streams[0]).unwrap();
                assert_eq!(scope.progress_v1().unwrap(), 0);
                assert_eq!(scope.slots[0].lifecycle.phase, Phase::Adopting);
                assert_eq!(
                    scope.cancel_before_adoption_v1(&first).unwrap(),
                    Cancel::CancelledBeforeSubmission
                );
                scope.hooks.ready = |_, _| Ok(true);
                let second = scope.admit(prepared.remove(0), streams[1]).unwrap();
                for phase in [Phase::Issuing, Phase::Completing, Phase::Settled] {
                    scope.progress_v1().unwrap();
                    assert_eq!(scope.slots[1].lifecycle.phase, phase);
                    assert_eq!(
                        scope.cancel_before_adoption_v1(&second).unwrap(),
                        Cancel::NotCancellable
                    );
                }
                assert_eq!((decoded.get(), dropped.get()), (1, 2));
                assert!(matches!(
                    scope.completion_v1(&second).unwrap(),
                    Some(Ok(()))
                ));
            },
        )
        .unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn unknown_adoption_cannot_be_cancelled_or_refunded_and_drop_fails_stop() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_SCOPED_CANCEL_UNKNOWN_CHILD";
    const TEST: &str = "context::generated_scope::tests::cancellation::unknown_adoption_cannot_be_cancelled_or_refunded_and_drop_fails_stop";
    if std::env::var_os(CHILD).is_some() {
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
        let stream = context.create_stream(context.devices()[0].id()).unwrap();
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let prepared = context.bound_preparation_for_test_v1(Borrowed {
            ticks: Cell::new(0),
            decoded: &decoded,
            dropped: &dropped,
            domain: Arc::new(()),
            completion_order: None,
        });
        let _ = context.with_generated_gfx942_scope_v1::<Borrowed<'_>, _>(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                scope.hooks = hooks();
                scope.hooks.adopt =
                    |_, _, _, _| Err(RuntimeValidationErrorV1::ContextReserved.into());
                let ticket = scope.admit(prepared, stream).unwrap();
                assert!(scope.progress_v1().is_err());
                assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
                assert_eq!(
                    scope.cancel_before_adoption_v1(&ticket).unwrap(),
                    Cancel::NotCancellable
                );
                assert_eq!((decoded.get(), dropped.get()), (0, 0));
                assert!(scope.slots[0].lifecycle.value.is_some());
                assert!(scope.context.has_unpublished_holds_v1());
                eprintln!("unknown original owner and hold retained after cancellation refusal");
            },
        );
        panic!("unknown scoped owner was released");
    }
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(CHILD, "1")
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
    assert!(!timed_out, "{stderr}");
    assert_eq!(output.status.signal(), Some(6), "{stderr}");
    assert!(
        stderr.contains("unknown original owner and hold retained after cancellation refusal"),
        "{stderr}"
    );
    assert!(!stderr.contains("owner was released"), "{stderr}");
}
