use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context as TaskContext, Poll, Wake, Waker},
};

type Scope<'a> = RuntimeGfx942GeneratedScopeV1<'a, 'a, KfdRuntimeBackendV1, Borrowed<'a>>;

mod cooperative;
mod retained_producer;
mod scale;

fn fixture<'a>(
    context: &'a mut RuntimeContextV1<KfdRuntimeBackendV1>,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
    ticks: &[usize],
) -> (Scope<'a>, Vec<RuntimeGfx942ScopedTicketV1<'a>>) {
    fixture_with_timeout(context, decoded, dropped, ticks, Duration::from_secs(10))
}

fn fixture_with_timeout<'a>(
    context: &'a mut RuntimeContextV1<KfdRuntimeBackendV1>,
    decoded: &'a Cell<usize>,
    dropped: &'a Cell<usize>,
    ticks: &[usize],
    timeout: Duration,
) -> (Scope<'a>, Vec<RuntimeGfx942ScopedTicketV1<'a>>) {
    let device = context.devices()[0].id();
    let mut inputs = Vec::new();
    for &ticks in ticks {
        let stream = context.create_stream(device).unwrap();
        let prepared = context.bound_preparation_for_test_v1(Borrowed {
            ticks: Cell::new(ticks),
            decoded,
            dropped,
            domain: Arc::new(()),
            completion_order: None,
        });
        inputs.push((prepared, stream));
    }
    let (slots, copies) = allocate_rosters(ticks.len()).unwrap();
    let mut scope = RuntimeGfx942GeneratedScopeV1 {
        epoch: context.scope_epoch.begin().unwrap(),
        context,
        slots,
        copies,
        graph: None,
        capacity: ticks.len(),
        deadline: Instant::now() + timeout,
        identity: Rc::new(()),
        invariant: PhantomData,
        hooks: hooks(),
    };
    let tickets = inputs
        .into_iter()
        .map(|(p, s)| scope.admit(p, s).unwrap())
        .collect();
    (scope, tickets)
}

struct Counter(AtomicUsize);
impl Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
fn poll<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
    Pin::new(future).poll(&mut TaskContext::from_waker(waker))
}

#[test]
fn scoped_futures_are_one_shot_and_polling_never_progresses_or_refunds() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[2, 0, 1]);
    let foreign = RuntimeGfx942ScopedTicketV1 {
        scope: Rc::new(()),
        index: 0,
        invariant: PhantomData,
    };
    assert!(matches!(
        scope.completion_future_v1(&foreign),
        Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    ));
    let mut slow = scope.completion_future_v1(&tickets[0]).unwrap();
    let mut fast = scope.completion_future_v1(&tickets[1]).unwrap();
    let abandoned = scope.completion_future_v1(&tickets[2]).unwrap();
    assert!(matches!(
        scope.completion_future_v1(&tickets[0]),
        Err(RuntimeGfx942ScopeErrorV1::CompletionObserverTaken)
    ));
    let old = Arc::new(Counter(AtomicUsize::new(0)));
    let latest = Arc::new(Counter(AtomicUsize::new(0)));
    assert!(poll(&mut fast, &Waker::from(old.clone())).is_pending());
    assert!(poll(&mut fast, &Waker::from(latest.clone())).is_pending());
    assert!(poll(&mut slow, Waker::noop()).is_pending());
    drop(abandoned);
    assert_eq!(
        (scope.pending_v1(), decoded.get(), dropped.get()),
        (3, 0, 0)
    );
    for _ in 0..3 {
        scope.progress_v1().unwrap();
    }
    assert_eq!(old.0.load(Ordering::SeqCst), 0);
    assert_eq!(latest.0.load(Ordering::SeqCst), 1);
    assert!(matches!(
        poll(&mut fast, Waker::noop()),
        Poll::Ready(Ok(()))
    ));
    assert!(poll(&mut slow, Waker::noop()).is_pending());
    scope.drain_v1().unwrap();
    assert!(matches!(
        poll(&mut slow, Waker::noop()),
        Poll::Ready(Ok(()))
    ));
    assert_eq!((decoded.get(), dropped.get()), (3, 3));
    drop((slow, fast, tickets, scope));
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_future_taken_after_settlement_is_immediately_ready() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[0]);
    scope.drain_v1().unwrap();
    let mut future = scope.completion_future_v1(&tickets[0]).unwrap();
    assert!(matches!(
        poll(&mut future, Waker::noop()),
        Poll::Ready(Ok(()))
    ));
    drop((future, tickets, scope));
    assert!(context.cleanup().is_complete());
}

#[test]
fn dropping_pending_driver_preserves_original_scope_custody() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[1]);
    let mut observer = scope.completion_future_v1(&tickets[0]).unwrap();
    let waits = Cell::new(0);
    {
        let mut driver = Box::pin(scope.drive_with_wake_v1(|_| {
            waits.set(waits.get() + 1);
            std::future::pending::<()>()
        }));
        for _ in 0..5 {
            assert!(poll(&mut driver, Waker::noop()).is_pending());
        }
    }
    assert_eq!(
        waits.get(),
        1,
        "polling without external notification must not scan again"
    );
    assert_eq!(
        (decoded.get(), dropped.get(), scope.pending_v1()),
        (0, 0, 1)
    );
    assert!(poll(&mut observer, Waker::noop()).is_pending());
    scope.drain_v1().unwrap();
    assert!(matches!(
        poll(&mut observer, Waker::noop()),
        Poll::Ready(Ok(()))
    ));
    drop((observer, tickets, scope));
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_borrowed_owner_future_composes_with_tokio_current_thread_timer() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[30, 0, 20]);
    let observers: Vec<_> = tickets
        .iter()
        .map(|t| scope.completion_future_v1(t).unwrap())
        .collect();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let (driver, results) = runtime.block_on(async {
        let driver = scope.drive_with_wake_v1(|deadline| {
            tokio::time::sleep_until(
                std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
            )
        });
        futures_util::future::join(driver, futures_util::future::join_all(observers)).await
    });
    driver.unwrap();
    assert!(results.into_iter().all(|r| r.is_ok()));
    assert_eq!(
        (decoded.get(), dropped.get(), scope.pending_v1()),
        (3, 3, 0)
    );
    drop((tickets, scope));
    assert!(context.cleanup().is_complete());
}

struct Pulse {
    state: Arc<Mutex<(usize, Option<Waker>)>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Pulse {
    fn new() -> Self {
        let state = Arc::new(Mutex::new((0, None::<Waker>)));
        let stop = Arc::new(AtomicBool::new(false));
        let captured = (state.clone(), stop.clone());
        // One test-only notification source, never a carrier/Work owner thread.
        let thread = std::thread::spawn(move || {
            while !captured.1.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(1));
                let wake = {
                    let mut state = captured.0.lock().unwrap();
                    state.0 += 1;
                    state.1.take()
                };
                if let Some(waker) = wake {
                    waker.wake();
                }
            }
        });
        Self {
            state,
            stop,
            thread: Some(thread),
        }
    }
    fn next(&self) -> impl Future<Output = ()> + '_ {
        let before = self.state.lock().unwrap().0;
        std::future::poll_fn(move |cx| {
            let mut state = self.state.lock().unwrap();
            if state.0 != before {
                Poll::Ready(())
            } else {
                state.1 = Some(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}
impl Drop for Pulse {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
    }
}

#[test]
fn scoped_borrowed_owner_future_composes_with_local_pool_external_notifications() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[20, 0, 10]);
    let observers: Vec<_> = tickets
        .iter()
        .map(|t| scope.completion_future_v1(t).unwrap())
        .collect();
    let pulses = Pulse::new();
    let (driver, results) =
        futures_executor::LocalPool::new().run_until(futures_util::future::join(
            scope.drive_with_wake_v1(|_| pulses.next()),
            futures_util::future::join_all(observers),
        ));
    driver.unwrap();
    assert!(results.into_iter().all(|r| r.is_ok()));
    assert_eq!(
        (decoded.get(), dropped.get(), scope.pending_v1()),
        (3, 3, 0)
    );
    drop((tickets, scope));
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_failed_driver_notifies_observers_before_fail_stop() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_TEST_SCOPED_FUTURE_FAILURE";
    const TEST: &str = "context::generated_scope::tests::futures::scoped_failed_driver_notifies_observers_before_fail_stop";
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
        let decoded = Cell::new(0);
        let dropped = Cell::new(0);
        let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[1, 0]);
        let mut observers: Vec<_> = tickets
            .iter()
            .map(|t| scope.completion_future_v1(t).unwrap())
            .collect();
        for observer in &mut observers {
            assert!(poll(observer, Waker::noop()).is_pending());
        }
        if mode == "deadline" {
            scope.deadline = Instant::now();
        } else {
            assert_eq!(mode, "native-error");
            scope.hooks.adopt = |_, _, _, _| Err(RuntimeValidationErrorV1::ContextTerminal.into());
        }
        let (driver, results) =
            futures_executor::LocalPool::new().run_until(futures_util::future::join(
                scope.drive_with_wake_v1(|_| async {
                    panic!("failed driver must not wait for another notification")
                }),
                futures_util::future::join_all(observers),
            ));
        if mode == "deadline" {
            assert!(matches!(driver, Err(RuntimeGfx942ScopeErrorV1::Deadline)));
        } else {
            assert!(matches!(driver, Err(RuntimeGfx942ScopeErrorV1::Context(_))));
            assert_eq!(scope.slots[0].lifecycle.phase, Phase::Unknown);
        }
        assert!(
            results
                .into_iter()
                .all(|r| matches!(r, Err(RuntimeGfx942ScopeErrorV1::Unknown)))
        );
        assert_eq!(
            (scope.pending_v1(), decoded.get(), dropped.get()),
            (2, 0, 0)
        );
        assert!(scope.context.has_unpublished_holds_v1());
        assert!(
            scope
                .slots
                .iter()
                .all(|s| s.lifecycle.value.is_some() && s.lifecycle.outcome.is_none())
        );
        eprintln!("failed observers joined; original carriers and holds retained; Drop must abort");
        drop(scope);
        panic!("unsettled scoped Drop returned");
    }
    for mode in ["deadline", "native-error"] {
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
        assert!(
            !timed_out,
            "{mode}: failed observers never resolved; {stderr}"
        );
        assert_eq!(output.status.signal(), Some(6), "{mode}: {stderr}");
        assert!(stderr.contains(
            "failed observers joined; original carriers and holds retained; Drop must abort"
        ));
        assert!(!stderr.contains("unsettled scoped Drop returned"));
    }
}
