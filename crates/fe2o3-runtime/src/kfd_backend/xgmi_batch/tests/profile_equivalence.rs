use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

#[derive(Debug, Eq, PartialEq)]
struct ProfileError {
    timeout: bool,
    message: String,
}

impl ProfileError {
    fn operation(case: usize) -> Self {
        Self {
            timeout: matches!(case, 2 | 3),
            message: format!("original operation error {case}"),
        }
    }

    fn close() -> Self {
        Self {
            timeout: false,
            message: "original close error".to_owned(),
        }
    }
}

#[derive(Default)]
struct Transcript {
    calls: Vec<Call>,
    custody: Vec<Vec<*const u64>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PanicStage {
    Submit,
    Wait,
    Close,
}

struct PanicPayload {
    message: String,
    token: Box<u64>,
}

struct ProfileScript {
    transcript: Rc<RefCell<Transcript>>,
    case: usize,
    close_error: bool,
    panic_at: Option<PanicStage>,
    panic_payload: Option<Box<PanicPayload>>,
}

impl ProfileScript {
    fn new(transcript: &Rc<RefCell<Transcript>>, case: usize, close_error: bool) -> Self {
        Self {
            transcript: Rc::clone(transcript),
            case,
            close_error,
            panic_at: None,
            panic_payload: None,
        }
    }

    fn observe(&self, call: Call, custody: &[Box<u64>]) {
        let mut transcript = self.transcript.borrow_mut();
        transcript.calls.push(call);
        transcript.custody.push(addresses(custody));
    }

    fn maybe_panic(&mut self, stage: PanicStage) {
        if self.panic_at == Some(stage) {
            panic_any(self.panic_payload.take().expect("original panic payload"));
        }
    }
}

impl Scope for ProfileScript {
    type Request = Box<u64>;
    type Ticket = Box<u64>;
    type Completed = Box<u64>;
    type Error = ProfileError;

    fn submit(&mut self, requests: Vec<Box<u64>>) -> Result<Vec<Box<u64>>, ScopedOperation<Self>> {
        self.observe(Call::Submit(values(&requests)), &requests);
        self.maybe_panic(PanicStage::Submit);
        match self.case {
            0 => Err(Operation::Unpublished {
                error: ProfileError::operation(self.case),
                requests,
            }),
            1 | 2 => Err(Operation::PublicationIndeterminate {
                error: ProfileError::operation(self.case),
                tickets: requests,
            }),
            _ => Ok(requests),
        }
    }

    fn wait(&mut self, tickets: Vec<Box<u64>>, deadline: Instant) -> ScopedOperation<Self> {
        self.observe(Call::Wait(values(&tickets), deadline), &tickets);
        self.maybe_panic(PanicStage::Wait);
        match self.case {
            3 | 4 => Operation::Retained {
                error: ProfileError::operation(self.case),
                tickets,
            },
            5 => Operation::Indeterminate {
                error: ProfileError::operation(self.case),
                completed: tickets,
            },
            _ => Operation::Completed(tickets),
        }
    }

    fn is_timeout(error: &ProfileError) -> bool {
        error.timeout
    }

    fn finish(mut self, terminal: bool) -> Result<(), ProfileError> {
        self.transcript
            .borrow_mut()
            .calls
            .push(Call::Close(terminal));
        self.maybe_panic(PanicStage::Close);
        if self.close_error {
            Err(ProfileError::close())
        } else {
            Ok(())
        }
    }
}

fn values(custody: &[Box<u64>]) -> Vec<u64> {
    custody.iter().map(|value| **value).collect()
}

fn addresses(custody: &[Box<u64>]) -> Vec<*const u64> {
    custody.iter().map(|value| &**value as *const u64).collect()
}

#[derive(Debug, Eq, PartialEq)]
struct Outcome {
    classification: &'static str,
    error: Option<ProfileError>,
    closing: Result<(), ProfileError>,
    values: Vec<u64>,
    calls: Vec<Call>,
}

fn run_case<const PROFILE: bool>(
    case: usize,
    close_error: bool,
    published: bool,
    count: u64,
    deadline: Instant,
) -> Outcome {
    let transcript = Rc::new(RefCell::new(Transcript::default()));
    let custody: Vec<_> = (1..=count).rev().map(Box::new).collect();
    let expected_addresses = addresses(&custody);
    let expected_values = values(&custody);
    let input = if published {
        Input::Published(custody)
    } else {
        Input::Ready(custody)
    };
    let (operation, closing) = execute_profiled::<_, PROFILE>(
        ProfileScript::new(&transcript, case, close_error),
        input,
        deadline,
        &mut CallTimer::<PROFILE>::new(),
    );
    let (classification, error, custody) = match operation {
        Operation::Unpublished { error, requests } => ("unpublished", Some(error), requests),
        Operation::PublicationIndeterminate { error, tickets } => {
            ("publication-indeterminate", Some(error), tickets)
        }
        Operation::Retained { error, tickets } => ("retained", Some(error), tickets),
        Operation::Indeterminate { error, completed } => ("indeterminate", Some(error), completed),
        Operation::Completed(completed) => ("completed", None, completed),
    };
    let effective_case = if published && case < 3 { 6 } else { case };
    let expected_classification = match effective_case {
        0 => "unpublished",
        1 | 2 => "publication-indeterminate",
        3 | 4 => "retained",
        5 => "indeterminate",
        _ => "completed",
    };
    assert_eq!(classification, expected_classification);
    assert_eq!(
        error,
        (effective_case != 6).then(|| ProfileError::operation(effective_case))
    );
    assert_eq!(
        closing,
        if close_error {
            Err(ProfileError::close())
        } else {
            Ok(())
        }
    );
    assert_eq!(addresses(&custody), expected_addresses);
    assert_eq!(values(&custody), expected_values);
    let mut expected_calls = Vec::new();
    if !published {
        expected_calls.push(Call::Submit(expected_values.clone()));
    }
    if published || case >= 3 {
        expected_calls.push(Call::Wait(expected_values, deadline));
    }
    expected_calls.push(Call::Close(matches!(effective_case, 1 | 2 | 4 | 5)));
    let transcript = transcript.borrow();
    assert_eq!(transcript.calls, expected_calls);
    assert!(
        transcript
            .custody
            .iter()
            .all(|pointers| *pointers == expected_addresses)
    );
    Outcome {
        classification,
        error,
        closing,
        values: values(&custody),
        calls: transcript.calls.clone(),
    }
}

#[test]
fn profiling_preserves_every_typed_outcome_close_error_and_custody_order() {
    let deadline = Instant::now();
    for count in [1, 2, 63] {
        for case in 0..7 {
            for close_error in [false, true] {
                for published in [false, true] {
                    assert_eq!(
                        run_case::<false>(case, close_error, published, count, deadline),
                        run_case::<true>(case, close_error, published, count, deadline),
                        "case={case} close_error={close_error} published={published} count={count}"
                    );
                }
            }
        }
    }
}

fn retry<const PROFILE: bool>(deadline: Instant) -> Vec<Call> {
    let transcript = Rc::new(RefCell::new(Transcript::default()));
    let requests: Vec<_> = [9, 4, 7].into_iter().map(Box::new).collect();
    let expected_addresses = addresses(&requests);
    let (operation, closing) = execute_profiled::<_, PROFILE>(
        ProfileScript::new(&transcript, 3, false),
        Input::Ready(requests),
        deadline,
        &mut CallTimer::<PROFILE>::new(),
    );
    assert_eq!(closing, Ok(()));
    let Operation::Retained { error, tickets } = operation else {
        panic!("timeout retains all published tickets")
    };
    assert_eq!(error, ProfileError::operation(3));
    assert_eq!(addresses(&tickets), expected_addresses);
    let mut scope = ProfileScript::new(&transcript, 6, false);
    scope.panic_at = Some(PanicStage::Submit);
    scope.panic_payload = Some(Box::new(PanicPayload {
        message: "retry must never submit".to_owned(),
        token: Box::new(99),
    }));
    let (operation, closing) = execute_profiled::<_, PROFILE>(
        scope,
        Input::Published(tickets),
        deadline,
        &mut CallTimer::<PROFILE>::new(),
    );
    assert_eq!(closing, Ok(()));
    let Operation::Completed(completed) = operation else {
        panic!("retry completes all original tickets")
    };
    assert_eq!(addresses(&completed), expected_addresses);
    assert_eq!(values(&completed), [9, 4, 7]);
    let transcript = transcript.borrow();
    assert!(
        transcript
            .custody
            .iter()
            .all(|pointers| *pointers == expected_addresses)
    );
    transcript.calls.clone()
}

#[test]
fn profiling_preserves_timeout_retry_without_resubmission_or_deadline_extension() {
    let deadline = Instant::now();
    let expected = vec![
        Call::Submit(vec![9, 4, 7]),
        Call::Wait(vec![9, 4, 7], deadline),
        Call::Close(false),
        Call::Wait(vec![9, 4, 7], deadline),
        Call::Close(false),
    ];
    assert_eq!(retry::<false>(deadline), expected);
    assert_eq!(retry::<true>(deadline), expected);
}

fn panic_trace<const PROFILE: bool>(
    stage: PanicStage,
    published: bool,
    deadline: Instant,
) -> Vec<Call> {
    let transcript = Rc::new(RefCell::new(Transcript::default()));
    let custody: Vec<_> = [9, 4, 7].into_iter().map(Box::new).collect();
    let expected_addresses = addresses(&custody);
    let payload = Box::new(PanicPayload {
        message: format!("original {stage:?} panic"),
        token: Box::new(123),
    });
    let payload_address = &*payload as *const PanicPayload;
    let token_address = &*payload.token as *const u64;
    let mut scope = ProfileScript::new(&transcript, 6, false);
    scope.panic_at = Some(stage);
    scope.panic_payload = Some(payload);
    let input = if published {
        Input::Published(custody)
    } else {
        Input::Ready(custody)
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        execute_profiled::<_, PROFILE>(scope, input, deadline, &mut CallTimer::<PROFILE>::new())
    }));
    let Err(payload) = result else {
        panic!("original scope panic must propagate")
    };
    let Ok(payload) = payload.downcast::<Box<PanicPayload>>() else {
        panic!("profiling must not replace the panic payload type")
    };
    assert_eq!(&**payload as *const PanicPayload, payload_address);
    assert_eq!(&*payload.token as *const u64, token_address);
    assert_eq!(payload.message, format!("original {stage:?} panic"));
    let mut expected = Vec::new();
    if !published {
        expected.push(Call::Submit(vec![9, 4, 7]));
    }
    if stage != PanicStage::Submit {
        expected.push(Call::Wait(vec![9, 4, 7], deadline));
    }
    if stage == PanicStage::Close {
        expected.push(Call::Close(false));
    }
    let transcript = transcript.borrow();
    assert_eq!(transcript.calls, expected);
    assert!(
        transcript
            .custody
            .iter()
            .all(|pointers| *pointers == expected_addresses)
    );
    transcript.calls.clone()
}

#[test]
fn profiling_preserves_submit_wait_and_close_panic_payload_identity() {
    let deadline = Instant::now();
    for stage in [PanicStage::Submit, PanicStage::Wait, PanicStage::Close] {
        for published in [false, true] {
            if published && stage == PanicStage::Submit {
                continue;
            }
            assert_eq!(
                panic_trace::<false>(stage, published, deadline),
                panic_trace::<true>(stage, published, deadline)
            );
        }
    }
}
