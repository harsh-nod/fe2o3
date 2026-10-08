use super::root::{Operations, Owners, PendingCopy, Poll, Progress, Refusal};
use std::cell::Cell;
use std::rc::Rc;

struct Token(Rc<Cell<usize>>, usize);
impl Drop for Token {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Types;
impl Owners for Types {
    type Source = Token;
    type Destination = Token;
    type Buffer = Token;
    type Submission = (Token, Token);
    type Completed = (Token, Token, Token);
    type Frontier = Token;
    type TransferFailure = Token;
    type SubmitFailure = (Token, Token);
    type PollFailure = (Token, Token);
    type RetireFailure = (Token, Token);
    type ReleaseFailure = Token;
}

#[derive(Clone, Copy, PartialEq)]
enum Fault {
    None,
    Transfer,
    Submit,
    Poll,
    Retire,
    Release,
    TransferPanic,
    ReleasePanic,
    RetirePanic,
}
struct Ops {
    fault: Fault,
    pending: bool,
    retained: Option<Token>,
    retained_retirement: Option<(Token, Token)>,
    frontier_drops: Rc<Cell<usize>>,
    trace: Vec<&'static str>,
}
impl Operations<Types> for Ops {
    fn transfer(&mut self, source: Token) -> Result<Token, Token> {
        self.trace.push("transfer");
        if self.fault == Fault::TransferPanic {
            self.retained = Some(source);
            panic!("scripted rooted lower transfer");
        }
        if self.fault == Fault::Transfer {
            Err(source)
        } else {
            Ok(source)
        }
    }
    fn submit(
        &mut self,
        buffer: Token,
        destination: Token,
    ) -> Result<(Token, Token), (Token, Token)> {
        self.trace.push("submit");
        if self.fault == Fault::Submit {
            Err((buffer, destination))
        } else {
            Ok((buffer, destination))
        }
    }
    fn poll(
        &mut self,
        submission: (Token, Token),
    ) -> Result<Poll<(Token, Token), (Token, Token, Token)>, (Token, Token)> {
        self.trace.push("poll");
        if self.fault == Fault::Poll {
            return Err(submission);
        }
        if self.pending {
            self.pending = false;
            Ok(Poll::Pending(submission))
        } else {
            Ok(Poll::Completed((
                submission.0,
                submission.1,
                Token(self.frontier_drops.clone(), 31),
            )))
        }
    }
    fn split_completed(completed: (Token, Token, Token)) -> (Token, Token, Token) {
        completed
    }
    fn retire(&mut self, destination: Token, frontier: Token) -> Result<Token, (Token, Token)> {
        self.trace.push("retire");
        if self.fault == Fault::RetirePanic {
            self.retained_retirement = Some((destination, frontier));
            panic!("scripted rooted lower retirement");
        }
        if self.fault == Fault::Retire {
            Err((destination, frontier))
        } else {
            drop(frontier);
            Ok(destination)
        }
    }
    fn release(&mut self, buffer: Token) -> Result<(), Token> {
        self.trace.push("release");
        if self.fault == Fault::ReleasePanic {
            self.retained = Some(buffer);
            panic!("scripted rooted lower release");
        }
        if self.fault == Fault::Release {
            Err(buffer)
        } else {
            drop(buffer);
            Ok(())
        }
    }
}
fn fixture(fault: Fault) -> (PendingCopy<Types>, Ops, Rc<Cell<usize>>) {
    let drops = Rc::new(Cell::new(0));
    let mut root = PendingCopy::empty();
    assert!(
        root.install(Token(drops.clone(), 11), Token(drops.clone(), 17))
            .is_ok()
    );
    (
        root,
        Ops {
            fault,
            pending: true,
            retained: None,
            retained_retirement: None,
            frontier_drops: Rc::new(Cell::new(0)),
            trace: Vec::new(),
        },
        drops,
    )
}

#[test]
fn transfer_is_not_release_and_destination_is_not_ready() {
    let (mut root, mut ops, drops) = fixture(Fault::None);
    assert_eq!(root.advance(&mut ops), Ok(Progress::Transferred));
    assert!(!root.is_released());
    assert!(root.take_released().is_none());
    assert!(root.cancel_unentered().is_none());
    assert_eq!(drops.get(), 0);
    assert_eq!(root.advance(&mut ops), Ok(Progress::Submitted));
    assert_eq!(root.advance(&mut ops), Ok(Progress::Pending));
    assert_eq!(root.advance(&mut ops), Ok(Progress::Completed));
    assert!(root.take_released().is_none());
    assert_eq!(root.advance(&mut ops), Ok(Progress::Retired));
    assert_eq!(ops.frontier_drops.get(), 1);
    assert!(root.take_released().is_none());
    assert_eq!(drops.get(), 0);
    assert_eq!(root.advance(&mut ops), Ok(Progress::Released));
    assert_eq!(drops.get(), 1);
    let destination = root.take_released().expect("original released destination");
    assert_eq!(destination.1, 17);
    assert!(root.take_released().is_none());
    assert_eq!(root.advance(&mut ops), Err(Refusal::Phase));
    assert_eq!(
        ops.trace,
        ["transfer", "submit", "poll", "poll", "retire", "release"]
    );
    drop(destination);
    assert_eq!(drops.get(), 2);
}

#[test]
fn unentered_cancel_returns_both_originals_without_operations() {
    let (mut root, ops, drops) = fixture(Fault::None);
    let (source, destination) = root.cancel_unentered().expect("unentered originals");
    assert_eq!((source.1, destination.1), (11, 17));
    assert!(ops.trace.is_empty());
    assert!(root.cancel_unentered().is_none());
    assert_eq!(drops.get(), 0);
    drop((source, destination));
    assert_eq!(drops.get(), 2);
}

#[test]
fn occupied_install_returns_new_pair_without_changing_originals() {
    let (mut root, _, drops) = fixture(Fault::None);
    let recovered = root.install(Token(drops.clone(), 23), Token(drops.clone(), 29));
    let (source, destination) = match recovered {
        Err(originals) => originals,
        Ok(()) => panic!("occupied root accepted replacement"),
    };
    assert_eq!((source.1, destination.1), (23, 29));
    drop((source, destination));
    let originals = root.cancel_unentered().unwrap();
    assert_eq!((originals.0.1, originals.1.1), (11, 17));
    drop(originals);
    assert_eq!(drops.get(), 4);
}

#[test]
fn every_failure_retains_custody_and_refuses_retry_or_cancellation() {
    for (fault, refusal) in [
        (Fault::Transfer, Refusal::Transfer),
        (Fault::Submit, Refusal::Submit),
        (Fault::Poll, Refusal::Poll),
        (Fault::Retire, Refusal::Retire),
        (Fault::Release, Refusal::Release),
    ] {
        let (mut root, mut ops, drops) = fixture(fault);
        let result = loop {
            match root.advance(&mut ops) {
                Ok(_) => (),
                Err(error) => break error,
            }
        };
        assert_eq!(result, refusal);
        assert_eq!(drops.get(), 0);
        if fault == Fault::Retire {
            assert_eq!(
                root.retained_retirement_buffer().map(|owner| owner.1),
                Some(11)
            );
            let originals = root
                .retained_retirement_failure()
                .expect("original retirement refusal");
            assert_eq!((originals.0.1, originals.1.1), (17, 31));
            assert_eq!(ops.frontier_drops.get(), 0);
        }
        let count = ops.trace.len();
        assert!(root.take_released().is_none());
        assert!(root.cancel_unentered().is_none());
        assert_eq!(root.advance(&mut ops), Err(Refusal::Phase));
        assert_eq!(ops.trace.len(), count);
        // Failure custody is intentionally not destructible or retryable.
        std::mem::forget(root);
        assert_eq!(drops.get(), 0);
    }
}

#[test]
fn retirement_unwind_keeps_buffer_rooted_and_lower_destination_frontier_retained() {
    let (mut root, mut ops, drops) = fixture(Fault::RetirePanic);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        loop {
            root.advance(&mut ops)
                .expect("scripted retirement progress");
        }
    }));
    assert!(result.is_err());
    assert_eq!(
        root.retained_retirement_buffer().map(|owner| owner.1),
        Some(11)
    );
    let originals = ops
        .retained_retirement
        .as_ref()
        .expect("rooted lower owners");
    assert_eq!((originals.0.1, originals.1.1), (17, 31));
    assert_eq!(drops.get(), 0);
    assert_eq!(ops.frontier_drops.get(), 0);
    let count = ops.trace.len();
    assert!(root.take_released().is_none());
    assert!(root.cancel_unentered().is_none());
    assert_eq!(root.advance(&mut ops), Err(Refusal::Phase));
    assert_eq!(ops.trace.len(), count);
    std::mem::forget(root);
    std::mem::forget(ops);
    assert_eq!(drops.get(), 0);
}

#[test]
fn lower_unwind_keeps_destination_and_original_lower_owner() {
    for fault in [Fault::TransferPanic, Fault::ReleasePanic] {
        let (mut root, mut ops, drops) = fixture(fault);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            loop {
                root.advance(&mut ops).expect("scripted progress");
            }
        }));
        assert!(result.is_err());
        assert!(ops.retained.is_some());
        assert_eq!(drops.get(), 0);
        assert!(root.cancel_unentered().is_none());
        assert!(root.take_released().is_none());
        assert_eq!(root.advance(&mut ops), Err(Refusal::Phase));
        std::mem::forget(root);
        std::mem::forget(ops);
        assert_eq!(drops.get(), 0);
    }
}
