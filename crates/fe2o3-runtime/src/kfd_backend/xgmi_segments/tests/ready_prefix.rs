use super::*;

fn flush<const PROFILE: bool>(
    scope: Script,
    sequence: &mut Sequence,
    custody: Custody<Box<u64>, Ticket>,
    deadline: Instant,
) -> Execution<Box<u64>, Ticket, Fault> {
    execute_profiled(
        scope,
        sequence,
        custody,
        deadline,
        Progress::Flush,
        &mut Timer::<PROFILE>::new(),
    )
}

#[test]
fn ready_flush_closes_each_bounded_prefix_and_preserves_pair_order_and_deadline() {
    assert_eq!(FLUSH_OBSERVATION_BUDGET, 8);
    for count in [1, 8, 9, 16, 17, 65, 4096] {
        let mut sequence = sequence(count);
        let log = Rc::new(RefCell::new(Log::default()));
        let pair = Box::new(42);
        let address = &*pair as *const u64;
        let mut custody = Custody::Pair(pair);
        let deadline = Instant::now() - Duration::from_secs(1);
        let mut completed = 0;
        let mut calls = 0;
        while completed < count {
            let execution = flush::<false>(Script::new(&log), &mut sequence, custody, deadline);
            completed = (completed + FLUSH_OBSERVATION_BUDGET).min(count);
            calls += 1;
            assert_eq!(sequence.cursor.completed() as usize, completed);
            assert!(!sequence.cursor.is_open() && !sequence.cursor.has_ticket());
            assert!(sequence.cursor.transition(Action::Cancel).is_none());
            assert_eq!(
                sequence.cursor.transition(Action::Succeed).is_some(),
                completed == count
            );
            assert!(!execution.terminal && execution.error.is_none());
            assert_eq!(execution.closing, Ok(()));
            let Custody::Pair(pair) = execution.custody else {
                panic!("completed prefix lost pair")
            };
            assert_eq!(&*pair as *const u64, address);
            custody = Custody::Pair(pair);
            assert_eq!(log.borrow().submits.len(), completed);
            assert_eq!(log.borrow().closes.len(), calls);
        }
        let log = log.borrow();
        assert_eq!(log.submits, sequence.segments);
        assert_eq!(
            log.waits,
            (0..count).map(|i| (i, deadline)).collect::<Vec<_>>()
        );
        assert_eq!(
            log.closes,
            vec![false; count.div_ceil(FLUSH_OBSERVATION_BUDGET)]
        );
    }
}

#[test]
fn pending_flush_stops_at_each_position_and_retry_counts_the_retained_ticket() {
    for at in 0..FLUSH_OBSERVATION_BUDGET {
        let mut sequence = sequence(17);
        let log = Rc::new(RefCell::new(Log::default()));
        let mut scope = Script::new(&log);
        scope.inject = Some((at, Inject::Timeout));
        let deadline = Instant::now() - Duration::from_secs(1);
        let execution = flush::<false>(scope, &mut sequence, Custody::Pair(Box::new(42)), deadline);
        assert_eq!(sequence.cursor.completed() as usize, at);
        assert!(sequence.cursor.has_ticket());
        assert_eq!(log.borrow().submits.len(), at + 1);
        assert!(!execution.terminal && execution.error.is_none());
        let calls_before = log.borrow().calls.len();
        let execution = flush::<false>(
            Script::new(&log),
            &mut sequence,
            execution.custody,
            deadline,
        );
        assert_eq!(
            sequence.cursor.completed() as usize,
            at + FLUSH_OBSERVATION_BUDGET
        );
        assert!(matches!(execution.custody, Custody::Pair(_)));
        let log = log.borrow();
        assert_eq!(log.calls[calls_before], Call::Wait(at, deadline));
        assert_eq!(
            log.submits,
            sequence.segments[..at + FLUSH_OBSERVATION_BUDGET]
        );
        assert_eq!(log.waits.len(), at + 1 + FLUSH_OBSERVATION_BUDGET);
        assert_eq!(log.closes, [false, false]);
    }
}

#[test]
fn repeated_pending_never_republishes_and_failed_close_quarantines_the_ticket() {
    for at in 0..FLUSH_OBSERVATION_BUDGET {
        for close_failure in [false, true] {
            let mut sequence = sequence(17);
            let log = Rc::new(RefCell::new(Log::default()));
            let mut custody = Custody::Pair(Box::new(42));
            let deadline = Instant::now() - Duration::from_secs(1);
            for retry in [false, true] {
                let mut scope = Script::new(&log);
                scope.inject = Some((at, Inject::Timeout));
                scope.close_failure = retry && close_failure;
                let execution = flush::<false>(scope, &mut sequence, custody, deadline);
                assert_eq!(execution.terminal, retry && close_failure);
                assert!(execution.error.is_none());
                assert_eq!(sequence.cursor.completed() as usize, at);
                assert!(sequence.cursor.has_ticket());
                custody = execution.custody;
            }
            assert_eq!(log.borrow().submits.len(), at + 1);
            assert_eq!(log.borrow().waits.len(), at + 2);
            assert_eq!(log.borrow().closes, [false, false]);
            assert_eq!(
                sequence.cursor.transition(Action::Open).is_some(),
                !close_failure
            );
            assert!(sequence.cursor.transition(Action::Succeed).is_none());
            let Custody::Ticket(ticket) = custody else {
                panic!("pending ticket lost")
            };
            assert_eq!(ticket.ordinal, at);
            assert_eq!(*ticket.pair, 42);
        }
    }
}

#[test]
fn native_poll_and_flush_supply_an_already_expired_observation_deadline() {
    let source = include_str!("../../../kfd_backend.rs")
        .split_whitespace()
        .collect::<String>()
        .replace(",)", ")");
    assert!(
        source.contains(
            "self.progress_peer_segments(id,Instant::now(),xgmi_segments::Progress::Flush)"
        )
    );
    assert!(source.contains(
        "self.progress_peer_segments(submission,Instant::now(),xgmi_segments::Progress::Poll)"
    ));
    assert!(source.contains(
        "self.progress_peer_segments(submission,deadline,xgmi_segments::Progress::Wait)"
    ));
}

#[test]
fn flush_faults_close_once_and_cannot_publish_a_later_descriptor() {
    for at in 0..FLUSH_OBSERVATION_BUDGET {
        for inject in [
            Inject::Recover,
            Inject::SubmitRetained,
            Inject::SubmitPair,
            Inject::WaitRetained,
            Inject::WaitPair,
        ] {
            for close_failure in [false, true] {
                let mut sequence = sequence(17);
                let log = Rc::new(RefCell::new(Log::default()));
                let mut scope = Script::new(&log);
                scope.inject = Some((at, inject));
                scope.close_failure = close_failure;
                let pair = Box::new(42);
                let address = &*pair as *const u64;
                let execution =
                    flush::<false>(scope, &mut sequence, Custody::Pair(pair), Instant::now());
                let terminal = !matches!(inject, Inject::Recover);
                assert_eq!(execution.terminal, terminal || close_failure);
                assert!(execution.error.is_some());
                assert_eq!(sequence.cursor.completed() as usize, at);
                assert_eq!(log.borrow().submits.len(), at + 1);
                assert_eq!(log.borrow().closes, [terminal]);
                for action in [
                    Action::Open,
                    Action::Publish,
                    Action::Succeed,
                    Action::Cancel,
                ] {
                    assert!(sequence.cursor.transition(action).is_none());
                }
                let pair = match execution.custody {
                    Custody::Pair(pair) => pair,
                    Custody::Ticket(ticket) => ticket.pair,
                };
                assert_eq!(&*pair as *const u64, address);
            }
        }
    }
}

#[test]
fn flush_close_failure_at_budget_or_list_end_never_authorizes_success() {
    for count in [1, 8, 9] {
        let mut sequence = sequence(count);
        let log = Rc::new(RefCell::new(Log::default()));
        let mut scope = Script::new(&log);
        scope.close_failure = true;
        let execution = flush::<false>(
            scope,
            &mut sequence,
            Custody::Pair(Box::new(42)),
            Instant::now(),
        );
        assert_eq!(sequence.cursor.completed() as usize, count.min(8));
        assert!(execution.terminal);
        assert_eq!(execution.closing, Err(Fault::Close));
        assert!(sequence.cursor.transition(Action::Succeed).is_none());
        assert_eq!(log.borrow().closes, [false]);
    }
}

#[test]
fn instrumentation_does_not_change_ready_prefix_progress() {
    for count in [1, 8, 9, 65] {
        for inject in [
            None,
            Some((3, Inject::Timeout)),
            Some((7, Inject::WaitPair)),
        ] {
            let deadline = Instant::now() - Duration::from_secs(1);
            let run = |profile| {
                let mut sequence = sequence(count);
                let log = Rc::new(RefCell::new(Log::default()));
                let mut scope = Script::new(&log);
                scope.inject = inject;
                let custody = Custody::Pair(Box::new(42));
                let result = if profile {
                    flush::<true>(scope, &mut sequence, custody, deadline)
                } else {
                    flush::<false>(scope, &mut sequence, custody, deadline)
                };
                let calls = log.borrow().calls.clone();
                (
                    sequence.cursor,
                    result.error,
                    result.closing,
                    result.terminal,
                    calls,
                )
            };
            assert_eq!(run(false), run(true));
        }
    }
}
