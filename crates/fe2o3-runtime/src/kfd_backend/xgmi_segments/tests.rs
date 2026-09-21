use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    Submit,
    Wait,
    Close,
}

#[derive(Clone, Copy, Debug)]
enum Inject {
    Recover,
    SubmitRetained,
    SubmitPair,
    Timeout,
    WaitRetained,
    WaitPair,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Submit(usize, RuntimePeerCopySegmentV1),
    Wait(usize, Instant),
    Close(bool),
}

#[derive(Default, Debug, Eq, PartialEq)]
struct Log {
    submits: Vec<RuntimePeerCopySegmentV1>,
    waits: Vec<(usize, Instant)>,
    closes: Vec<bool>,
    calls: Vec<Call>,
}

struct Ticket {
    ordinal: usize,
    pair: Box<u64>,
}
struct Script {
    log: Rc<RefCell<Log>>,
    inject: Option<(usize, Inject)>,
    close_failure: bool,
}

impl Script {
    fn new(log: &Rc<RefCell<Log>>) -> Self {
        Self {
            log: Rc::clone(log),
            inject: None,
            close_failure: false,
        }
    }
}

impl Scope for Script {
    type Pair = Box<u64>;
    type Ticket = Ticket;
    type Error = Fault;

    fn submit(
        &mut self,
        pair: Box<u64>,
        segment: RuntimePeerCopySegmentV1,
    ) -> Result<Ticket, Observation<Box<u64>, Ticket, Fault>> {
        let ordinal = self.log.borrow().submits.len();
        self.log.borrow_mut().submits.push(segment);
        self.log
            .borrow_mut()
            .calls
            .push(Call::Submit(ordinal, segment));
        if let Some((at, inject)) = self.inject
            && ordinal == at
        {
            match inject {
                Inject::Recover | Inject::SubmitPair => {
                    return Err(Observation::Failed {
                        error: Fault::Submit,
                        custody: Custody::Pair(pair),
                        terminal: matches!(inject, Inject::SubmitPair),
                    });
                }
                Inject::SubmitRetained => {
                    return Err(Observation::Failed {
                        error: Fault::Submit,
                        custody: Custody::Ticket(Ticket { ordinal, pair }),
                        terminal: true,
                    });
                }
                _ => {}
            }
        }
        Ok(Ticket { ordinal, pair })
    }

    fn wait(
        &mut self,
        ticket: Ticket,
        bytes: u64,
        deadline: Instant,
    ) -> Observation<Box<u64>, Ticket, Fault> {
        assert_eq!(bytes, self.log.borrow().submits[ticket.ordinal].byte_len);
        self.log.borrow_mut().waits.push((ticket.ordinal, deadline));
        self.log
            .borrow_mut()
            .calls
            .push(Call::Wait(ticket.ordinal, deadline));
        if let Some((at, inject)) = self.inject
            && ticket.ordinal == at
        {
            match inject {
                Inject::Timeout => return Observation::Pending(ticket),
                Inject::WaitRetained => {
                    return Observation::Failed {
                        error: Fault::Wait,
                        custody: Custody::Ticket(ticket),
                        terminal: true,
                    };
                }
                Inject::WaitPair => {
                    return Observation::Failed {
                        error: Fault::Wait,
                        custody: Custody::Pair(ticket.pair),
                        terminal: true,
                    };
                }
                _ => {}
            }
        }
        Observation::Completed(ticket.pair)
    }

    fn finish(self, terminal: bool) -> Result<(), Fault> {
        self.log.borrow_mut().closes.push(terminal);
        self.log.borrow_mut().calls.push(Call::Close(terminal));
        if self.close_failure {
            Err(Fault::Close)
        } else {
            Ok(())
        }
    }
}

fn sequence(count: usize) -> Sequence {
    Sequence {
        segments: (0..count as u64)
            .map(|i| RuntimePeerCopySegmentV1 {
                source_offset: i * 8,
                destination_offset: i * 16,
                byte_len: i % 7 + 1,
            })
            .collect(),
        cursor: OrderedPeerCopyCursorV1::new(count).unwrap(),
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(600)
}

#[test]
fn whole_sequence_reuses_one_pair_and_one_scope_beyond_ring_capacity() {
    for count in [1, 2, 63, 64, 65, 4096] {
        let mut sequence = sequence(count);
        let log = Rc::new(RefCell::new(Log::default()));
        let pair = Box::new(42);
        let address = &*pair as *const u64;
        let deadline = deadline();
        let execution = execute(
            Script::new(&log),
            &mut sequence,
            Custody::Pair(pair),
            deadline,
            false,
        );
        assert!(!execution.terminal && execution.error.is_none());
        assert_eq!(execution.closing, Ok(()));
        let Custody::Pair(pair) = execution.custody else {
            panic!("pair restored")
        };
        assert_eq!(&*pair as *const u64, address);
        assert_eq!(sequence.cursor.completed(), count as u32);
        assert!(!sequence.cursor.is_open());
        assert!(sequence.cursor.transition(Action::Cancel).is_none());
        assert!(sequence.cursor.transition(Action::Succeed).is_some());
        let log = log.borrow();
        assert_eq!(log.submits, sequence.segments);
        assert_eq!(
            log.waits,
            (0..count).map(|i| (i, deadline)).collect::<Vec<_>>()
        );
        assert_eq!(log.closes, [false]);
    }
}

#[test]
fn timeout_retry_waits_the_retained_ticket_without_republishing_any_prefix() {
    for at in 0..4 {
        let mut sequence = sequence(4);
        let log = Rc::new(RefCell::new(Log::default()));
        let mut scope = Script::new(&log);
        scope.inject = Some((at, Inject::Timeout));
        let execution = execute(
            scope,
            &mut sequence,
            Custody::Pair(Box::new(42)),
            deadline(),
            false,
        );
        assert!(!execution.terminal && execution.error.is_none());
        assert_eq!(sequence.cursor.completed(), at as u32);
        assert!(sequence.cursor.has_ticket());
        assert_eq!(log.borrow().submits.len(), at + 1);
        let execution = execute(
            Script::new(&log),
            &mut sequence,
            execution.custody,
            deadline(),
            false,
        );
        assert!(!execution.terminal && execution.error.is_none());
        assert_eq!(sequence.cursor.completed(), 4);
        assert_eq!(log.borrow().submits, sequence.segments);
        assert_eq!(
            log.borrow()
                .waits
                .iter()
                .filter(|(ordinal, _)| *ordinal == at)
                .count(),
            2
        );
        assert_eq!(log.borrow().closes, [false, false]);
    }
}

#[test]
fn expired_deadline_and_poll_budget_stop_between_segments_with_irreversible_history() {
    for one_step in [false, true] {
        let mut sequence = sequence(3);
        let log = Rc::new(RefCell::new(Log::default()));
        let mut custody = Custody::Pair(Box::new(42));
        for prefix in 1..=3 {
            let deadline = if one_step { deadline() } else { Instant::now() };
            let execution = execute(
                Script::new(&log),
                &mut sequence,
                custody,
                deadline,
                one_step,
            );
            custody = execution.custody;
            assert_eq!(sequence.cursor.completed(), prefix);
            assert!(!sequence.cursor.has_ticket());
            assert!(sequence.ever_published());
            assert!(sequence.cursor.transition(Action::Cancel).is_none());
            assert_eq!(log.borrow().submits.len(), prefix as usize);
        }
        assert_eq!(log.borrow().closes, [false, false, false]);
    }
}

#[test]
fn every_native_failure_retains_exact_custody_and_cannot_resume_or_succeed() {
    for at in 0..4 {
        for inject in [
            Inject::Recover,
            Inject::SubmitRetained,
            Inject::SubmitPair,
            Inject::WaitRetained,
            Inject::WaitPair,
        ] {
            for close_failure in [false, true] {
                let mut sequence = sequence(4);
                let log = Rc::new(RefCell::new(Log::default()));
                let mut scope = Script::new(&log);
                scope.inject = Some((at, inject));
                scope.close_failure = close_failure;
                let execution = execute(
                    scope,
                    &mut sequence,
                    Custody::Pair(Box::new(42)),
                    deadline(),
                    false,
                );
                let fault_terminal = !matches!(inject, Inject::Recover);
                assert_eq!(execution.terminal, fault_terminal || close_failure);
                assert!(execution.error.is_some());
                assert_eq!(sequence.cursor.completed(), at as u32);
                assert!(sequence.ever_published());
                assert!(!sequence.cursor.is_open());
                for action in [
                    Action::Open,
                    Action::Publish,
                    Action::Succeed,
                    Action::Cancel,
                ] {
                    assert!(sequence.cursor.transition(action).is_none());
                }
                assert_eq!(log.borrow().closes, [fault_terminal]);
                assert_eq!(log.borrow().submits.len(), at + 1);
                let pair = match execution.custody {
                    Custody::Pair(pair) => pair,
                    Custody::Ticket(ticket) => ticket.pair,
                };
                assert_eq!(*pair, 42);
            }
        }
    }
}

#[test]
fn final_close_failure_never_authorizes_success() {
    let mut sequence = sequence(3);
    let log = Rc::new(RefCell::new(Log::default()));
    let mut scope = Script::new(&log);
    scope.close_failure = true;
    let execution = execute(
        scope,
        &mut sequence,
        Custody::Pair(Box::new(42)),
        deadline(),
        false,
    );
    assert_eq!(sequence.cursor.completed(), 3);
    assert!(execution.terminal);
    assert_eq!(execution.closing, Err(Fault::Close));
    assert!(sequence.cursor.transition(Action::Succeed).is_none());
}

#[test]
fn native_wait_forwards_the_absolute_deadline_without_singleton_rosters() {
    let source = include_str!("../xgmi_segments.rs");
    let native = source
        .split("impl Scope for NativeScope<'_>")
        .nth(1)
        .unwrap();
    let wait = native
        .split("fn wait(")
        .nth(1)
        .unwrap()
        .split("fn finish(")
        .next()
        .unwrap();
    assert!(wait.contains("self.0.wait_until(ticket, deadline)"));
    for repack in ["vec!", "Vec", "wait_batch", "saturating_duration_since"] {
        assert!(!wait.contains(repack));
    }
    assert!(wait.contains("retained != ticket"));
    assert!(wait.contains("u64::from(completed.copy_bytes()) != bytes"));
    assert!(wait.contains("Gfx942XgmiWaitFailureV1::CompletedCurrentnessIndeterminate"));
}

mod profile_equivalence;
