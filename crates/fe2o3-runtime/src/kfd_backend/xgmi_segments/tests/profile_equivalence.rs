use super::*;

#[cfg(feature = "hardware-diagnostic")]
impl CurrentnessFinish for Script {
    type Detail = bool;

    fn finish_currentness(self, terminal: bool) -> Result<bool, Fault> {
        self.finish(terminal)?;
        Ok(terminal)
    }
}

fn run_scope<const PROFILE: bool>(
    scope: Script,
    sequence: &mut Sequence,
    custody: Custody<Box<u64>, Ticket>,
    deadline: Instant,
    one_step: bool,
) -> Execution<Box<u64>, Ticket, Fault> {
    let mut timer = Timer::<PROFILE>::new();
    #[cfg(feature = "hardware-diagnostic")]
    if PROFILE {
        let mut closing = None;
        let execution = execute_profiled(
            CurrentnessScope {
                inner: scope,
                closing: &mut closing,
            },
            sequence,
            custody,
            deadline,
            one_step,
            &mut timer,
        );
        assert_eq!(closing.is_some(), execution.closing.is_ok());
        return execution;
    }
    execute_profiled(scope, sequence, custody, deadline, one_step, &mut timer)
}

#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    cursor: OrderedPeerCopyCursorV1,
    ticket: Option<usize>,
    error: Option<Fault>,
    closing: Result<(), Fault>,
    terminal: bool,
    calls: Vec<Call>,
}

fn run<const PROFILE: bool>(
    count: usize,
    inject: Option<(usize, Inject)>,
    close_failure: bool,
    deadline: Instant,
    one_step: bool,
    retry: bool,
) -> Outcome {
    let mut sequence = sequence(count);
    let log = Rc::new(RefCell::new(Log::default()));
    let pair = Box::new(42);
    let address = &*pair as *const u64;
    let mut scope = Script::new(&log);
    scope.inject = inject;
    scope.close_failure = close_failure;
    let mut execution = run_scope::<PROFILE>(
        scope,
        &mut sequence,
        Custody::Pair(pair),
        deadline,
        one_step,
    );
    if retry {
        execution = run_scope::<PROFILE>(
            Script::new(&log),
            &mut sequence,
            execution.custody,
            deadline,
            one_step,
        );
    }
    let ticket = match &execution.custody {
        Custody::Pair(pair) => {
            assert_eq!(&**pair as *const u64, address);
            None
        }
        Custody::Ticket(ticket) => {
            assert_eq!(&*ticket.pair as *const u64, address);
            Some(ticket.ordinal)
        }
    };
    let calls = log.borrow().calls.clone();
    Outcome {
        cursor: sequence.cursor,
        ticket,
        error: execution.error,
        closing: execution.closing,
        terminal: execution.terminal,
        calls,
    }
}

#[test]
fn profiling_preserves_whole_list_one_step_and_expired_deadline_transcripts() {
    for count in [1, 2, 65, 4096] {
        for one_step in [false, true] {
            for deadline in [deadline(), Instant::now() - Duration::from_secs(1)] {
                assert_eq!(
                    run::<false>(count, None, false, deadline, one_step, false),
                    run::<true>(count, None, false, deadline, one_step, false)
                );
            }
        }
    }
}

#[test]
fn profiling_preserves_failure_custody_and_exact_retained_ticket_retry() {
    let deadline = deadline();
    for count in [1, 2, 65] {
        for at in [0, count / 2, count - 1] {
            for inject in [
                Inject::Recover,
                Inject::SubmitRetained,
                Inject::SubmitPair,
                Inject::Timeout,
                Inject::WaitRetained,
                Inject::WaitPair,
            ] {
                for close_failure in [false, true] {
                    assert_eq!(
                        run::<false>(
                            count,
                            Some((at, inject)),
                            close_failure,
                            deadline,
                            false,
                            false
                        ),
                        run::<true>(
                            count,
                            Some((at, inject)),
                            close_failure,
                            deadline,
                            false,
                            false
                        )
                    );
                }
            }
            let ordinary = run::<false>(
                count,
                Some((at, Inject::Timeout)),
                false,
                deadline,
                false,
                true,
            );
            let profiled = run::<true>(
                count,
                Some((at, Inject::Timeout)),
                false,
                deadline,
                false,
                true,
            );
            assert_eq!(ordinary, profiled);
            let first_close = profiled
                .calls
                .iter()
                .position(|call| matches!(call, Call::Close(false)))
                .unwrap();
            assert_eq!(profiled.calls[first_close + 1], Call::Wait(at, deadline));
            assert_eq!(
                profiled
                    .calls
                    .iter()
                    .filter(|call| matches!(call, Call::Submit(..)))
                    .count(),
                count
            );
            assert_eq!(profiled.cursor.completed(), count as u32);
        }
    }
}

struct Panicking {
    stage: Fault,
    payload: Box<u64>,
    calls: Rc<RefCell<Vec<Fault>>>,
}

impl Panicking {
    fn visit(&mut self, stage: Fault) {
        self.calls.borrow_mut().push(stage);
        if self.stage == stage {
            std::panic::panic_any(std::mem::take(&mut self.payload));
        }
    }
}

impl Scope for Panicking {
    type Pair = Box<u64>;
    type Ticket = Box<u64>;
    type Error = Fault;
    fn submit(
        &mut self,
        pair: Box<u64>,
        _: RuntimePeerCopySegmentV1,
    ) -> Result<Box<u64>, ScopedObservation<Self>> {
        self.visit(Fault::Submit);
        Ok(pair)
    }
    fn wait(&mut self, ticket: Box<u64>, _: u64, _: Instant) -> ScopedObservation<Self> {
        self.visit(Fault::Wait);
        Observation::Completed(ticket)
    }
    fn finish(mut self, _: bool) -> Result<(), Fault> {
        self.visit(Fault::Close);
        Ok(())
    }
}

fn panic_run<const PROFILE: bool>(stage: Fault) -> Vec<Fault> {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let payload = Box::new(73);
    let pointer = &*payload as *const u64;
    let scope = Panicking {
        stage,
        payload,
        calls: Rc::clone(&calls),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        execute_profiled(
            scope,
            &mut sequence(1),
            Custody::Pair(Box::new(42)),
            deadline(),
            false,
            &mut Timer::<PROFILE>::new(),
        )
    }));
    let Err(payload) = result else {
        panic!("injected panic must propagate")
    };
    let payload = payload.downcast::<Box<u64>>().unwrap();
    assert_eq!(&**payload as *const u64, pointer);
    calls.borrow().clone()
}

#[test]
fn generic_timer_preserves_panic_payload_and_never_attempts_extra_cleanup() {
    // This tests the generic executor only. Native authority-crossing unwinds
    // remain process-abort-only via finish_native_attempt.
    for stage in [Fault::Submit, Fault::Wait, Fault::Close] {
        assert_eq!(panic_run::<false>(stage), panic_run::<true>(stage));
    }
}
