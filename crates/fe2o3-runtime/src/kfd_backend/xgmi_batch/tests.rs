use super::*;
use std::cell::RefCell;
use std::rc::Rc;

mod admission_scaling;
mod dependency_scaling;
mod profile_equivalence;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    Timeout,
    Native,
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Submit(Vec<u64>),
    Wait(Vec<u64>, Instant),
    Close(bool),
}

struct Script {
    calls: Rc<RefCell<Vec<Call>>>,
    submit_error: Option<(Fault, bool)>,
    wait_error: Option<(Fault, bool)>,
    close_error: bool,
}

impl Script {
    fn new(calls: &Rc<RefCell<Vec<Call>>>) -> Self {
        Self {
            calls: Rc::clone(calls),
            submit_error: None,
            wait_error: None,
            close_error: false,
        }
    }
}

impl Scope for Script {
    type Request = Box<u64>;
    type Ticket = Box<u64>;
    type Completed = Box<u64>;
    type Error = Fault;

    fn submit(&mut self, requests: Vec<Box<u64>>) -> Result<Vec<Box<u64>>, ScopedOperation<Self>> {
        self.calls
            .borrow_mut()
            .push(Call::Submit(requests.iter().map(|id| **id).collect()));
        match self.submit_error {
            None => Ok(requests),
            Some((error, false)) => Err(Operation::Unpublished { error, requests }),
            Some((error, true)) => Err(Operation::PublicationIndeterminate {
                error,
                tickets: requests,
            }),
        }
    }

    fn wait(&mut self, tickets: Vec<Box<u64>>, deadline: Instant) -> ScopedOperation<Self> {
        self.calls.borrow_mut().push(Call::Wait(
            tickets.iter().map(|id| **id).collect(),
            deadline,
        ));
        match self.wait_error {
            None => Operation::Completed(tickets),
            Some((error, false)) => Operation::Retained { error, tickets },
            Some((error, true)) => Operation::Indeterminate {
                error,
                completed: tickets,
            },
        }
    }

    fn is_timeout(error: &Fault) -> bool {
        *error == Fault::Timeout
    }

    fn finish(self, terminal: bool) -> Result<(), Fault> {
        self.calls.borrow_mut().push(Call::Close(terminal));
        if self.close_error {
            Err(Fault::Close)
        } else {
            Ok(())
        }
    }
}

#[test]
fn ready_submits_once_and_retry_only_waits_with_the_original_deadline() {
    for count in [1, 2, 63] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut scope = Script::new(&calls);
        scope.wait_error = Some((Fault::Timeout, false));
        let requests: Vec<_> = (0..count).map(Box::new).collect();
        let addresses: Vec<_> = requests
            .iter()
            .map(|value| &**value as *const u64)
            .collect();
        let deadline = Instant::now();
        let (operation, closing) = execute(scope, Input::Ready(requests), deadline);
        assert_eq!(closing, Ok(()));
        let Operation::Retained {
            error: Fault::Timeout,
            tickets,
        } = operation
        else {
            panic!("exact timeout custody")
        };
        assert_eq!(
            tickets
                .iter()
                .map(|value| &**value as *const u64)
                .collect::<Vec<_>>(),
            addresses
        );
        let (operation, closing) =
            execute(Script::new(&calls), Input::Published(tickets), deadline);
        assert_eq!(closing, Ok(()));
        let Operation::Completed(completed) = operation else {
            panic!("all members completed")
        };
        assert_eq!(
            completed
                .iter()
                .map(|value| &**value as *const u64)
                .collect::<Vec<_>>(),
            addresses
        );
        let ids: Vec<_> = (0..count).collect();
        assert_eq!(
            *calls.borrow(),
            [
                Call::Submit(ids.clone()),
                Call::Wait(ids.clone(), deadline),
                Call::Close(false),
                Call::Wait(ids, deadline),
                Call::Close(false)
            ]
        );
    }
}

#[test]
fn every_typed_operation_outcome_closes_before_returning_custody() {
    for case in 0..7 {
        for close_error in [false, true] {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let mut scope = Script::new(&calls);
            scope.close_error = close_error;
            match case {
                0 => scope.submit_error = Some((Fault::Native, false)),
                1 => scope.submit_error = Some((Fault::Native, true)),
                2 => scope.submit_error = Some((Fault::Timeout, true)),
                3 => scope.wait_error = Some((Fault::Timeout, false)),
                4 => scope.wait_error = Some((Fault::Native, false)),
                5 => scope.wait_error = Some((Fault::Native, true)),
                _ => (),
            }
            let request = Box::new(7);
            let address = &*request as *const u64;
            let (operation, closing) = execute(scope, Input::Ready(vec![request]), Instant::now());
            assert_eq!(
                closing,
                if close_error {
                    Err(Fault::Close)
                } else {
                    Ok(())
                }
            );
            let custody = match (case, operation) {
                (
                    0,
                    Operation::Unpublished {
                        error: Fault::Native,
                        requests,
                    },
                ) => requests,
                (
                    1,
                    Operation::PublicationIndeterminate {
                        error: Fault::Native,
                        tickets,
                    },
                ) => tickets,
                (
                    2,
                    Operation::PublicationIndeterminate {
                        error: Fault::Timeout,
                        tickets,
                    },
                ) => tickets,
                (
                    3,
                    Operation::Retained {
                        error: Fault::Timeout,
                        tickets,
                    },
                ) => tickets,
                (
                    4,
                    Operation::Retained {
                        error: Fault::Native,
                        tickets,
                    },
                ) => tickets,
                (
                    5,
                    Operation::Indeterminate {
                        error: Fault::Native,
                        completed,
                    },
                ) => completed,
                (6, Operation::Completed(completed)) => completed,
                _ => panic!("failure must not change custody classification"),
            };
            assert_eq!(custody.len(), 1);
            assert_eq!(&*custody[0] as *const u64, address);
            let calls = calls.borrow();
            assert_eq!(
                calls.last(),
                Some(&Call::Close(matches!(case, 1 | 2 | 4 | 5)))
            );
            assert_eq!(
                calls
                    .iter()
                    .filter(|call| matches!(call, Call::Wait(..)))
                    .count(),
                usize::from(case >= 3)
            );
        }
    }
}

fn record(id: u64, direction: usize) -> XgmiRuntimeSubmissionV1 {
    XgmiRuntimeSubmissionV1 {
        id,
        stream: id + 100,
        direction,
        source: id * 2,
        destination: id * 2 + 1,
        source_offset: 0,
        destination_offset: 0,
        byte_len: 4096,
        dependencies: Vec::new(),
        dependency_cursor: 0,
        ticket: None,
    }
}

#[test]
fn admission_requires_an_exact_set_but_preserves_native_fifo_order() {
    for count in [1, 2, 63] {
        let active: HashMap<_, _> = (1..=count).map(|id| (id, record(id, 0))).collect();
        let ready = [(1..=count).rev().collect(), VecDeque::new()];
        let requested: Vec<_> = (1..=count).collect();
        assert_eq!(
            admit(
                &requested,
                &active,
                &ready,
                &[vec![], vec![]],
                &HashMap::new()
            ),
            Ok(Admission {
                direction: 0,
                published: false
            })
        );
        assert_eq!(
            ready[0].iter().copied().collect::<Vec<_>>(),
            requested.iter().rev().copied().collect::<Vec<_>>()
        );
    }
}

#[test]
fn admission_separates_caller_errors_from_corrupt_indexes() {
    for case in 0..14 {
        let mut active = HashMap::from([(1, record(1, 0)), (2, record(2, 0))]);
        let mut ready = [VecDeque::from([2, 1]), VecDeque::new()];
        let mut in_flight = [vec![], vec![]];
        let mut requested = vec![1, 2];
        let expected = match case {
            0 => {
                requested.clear();
                AdmissionError::Invalid
            }
            1 => {
                requested = vec![1; 64];
                AdmissionError::Invalid
            }
            2 => {
                requested = vec![1, 1];
                AdmissionError::Invalid
            }
            3 => {
                requested = vec![999];
                AdmissionError::Invalid
            }
            4 => {
                requested = vec![1];
                AdmissionError::Busy
            }
            5 => {
                ready[0] = VecDeque::from([1, 1]);
                AdmissionError::Corrupt
            }
            6 => {
                ready[0].pop_front();
                AdmissionError::Corrupt
            }
            7 => {
                ready[1].push_back(1);
                AdmissionError::Corrupt
            }
            8 => {
                active.get_mut(&2).unwrap().direction = 1;
                AdmissionError::Corrupt
            }
            9 => {
                in_flight[0].push(2);
                AdmissionError::Corrupt
            }
            10 => {
                active.get_mut(&2).unwrap().dependencies.push(1);
                AdmissionError::Corrupt
            }
            11 => {
                active.get_mut(&2).unwrap().source = 2;
                AdmissionError::Corrupt
            }
            12 => {
                ready[0] = VecDeque::from([1]);
                requested = vec![1];
                AdmissionError::Corrupt
            }
            _ => {
                active.remove(&2);
                AdmissionError::Corrupt
            }
        };
        assert_eq!(
            admit(&requested, &active, &ready, &in_flight, &HashMap::new()),
            Err(expected),
            "case {case}"
        );
    }
}

#[test]
fn admission_leaves_dependency_blocked_successors_outside_the_batch() {
    let mut successor = record(3, 1);
    successor.dependencies.push(1);
    successor.source = 3;
    let active = HashMap::from([(1, record(1, 0)), (2, record(2, 0)), (3, successor)]);
    let ready = [VecDeque::from([2, 1]), VecDeque::new()];
    assert_eq!(
        admit(&[1, 2], &active, &ready, &[vec![], vec![]], &HashMap::new()),
        Ok(Admission {
            direction: 0,
            published: false
        })
    );
    assert_eq!(
        admit(&[3], &active, &ready, &[vec![], vec![]], &HashMap::new()),
        Err(AdmissionError::Busy)
    );
}

#[cfg(unix)]
#[test]
fn native_attempt_unwind_aborts_without_dropping_the_panic_payload() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_XGMI_BATCH_ABORT_CHILD";
    if std::env::var_os(CHILD).is_some() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                std::process::exit(71);
            }
        }
        let result = std::panic::catch_unwind(|| std::panic::panic_any(Payload));
        let mut terminal = false;
        finish_native_attempt(result, &mut terminal);
        std::process::exit(72);
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "kfd_backend::xgmi_batch::tests::native_attempt_unwind_aborts_without_dropping_the_panic_payload", "--nocapture"])
        .env(CHILD, "1").output().unwrap();
    assert_eq!(
        output.status.signal(),
        Some(6),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut terminal = false;
    assert_eq!(finish_native_attempt(Ok(7), &mut terminal), 7);
    assert!(!terminal);
}

#[test]
fn dependency_indexes_retain_and_wake_blocked_successors_exactly() {
    for fault in 0..7 {
        let mut predecessor = record(2, 0);
        predecessor.dependencies.push(1);
        let mut successor = record(3, 1);
        successor.dependencies.push(2);
        let active = HashMap::from([(2, predecessor), (3, successor)]);
        let completed = HashMap::from([(
            1,
            SubmissionRecordV1 {
                stream: 1,
                status: BackendPollV1::Succeeded,
                profile_dispatch_published: false,
            },
        )]);
        let mut waiters = HashMap::from([(2, vec![3])]);
        let mut counts = HashMap::from([(1, 1), (2, 1)]);
        match fault {
            0 => (),
            1 => {
                waiters.remove(&2);
            }
            2 => {
                waiters.insert(2, vec![3, 3]);
            }
            3 => {
                counts.remove(&2);
            }
            4 => {
                counts.insert(1, 0);
            }
            5 => {
                counts.insert(1, 2);
            }
            _ => {
                waiters.insert(1, vec![2]);
            }
        }
        assert_eq!(
            valid_dependency_indexes(&[2], &active, &completed, &waiters, &counts),
            Ok(fault == 0),
            "fault {fault}"
        );
    }
}
