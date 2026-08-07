#![deny(unsafe_code)]

//! Completion policy for safe wrappers around asynchronous backend calls.
//!
//! This crate deliberately has no HIP dependency so failure and cleanup policy
//! can be exercised on CPU-only CI runners.

#[allow(unsafe_code)]
mod graph;
#[allow(unsafe_code)]
mod lifecycle;

pub use graph::{
    CancellationCodeV1, CompletionAuthorityV1, CompletionGraphErrorV1, CompletionGraphV1,
    CompletionNodeIdV1, CompletionNodeKindV1, CompletionNodeStateV1, CompletionNodeV1,
    CompletionReportEntryV1, CompletionReportV1, CompletionTransitionErrorV1, ContextIdentityV1,
    DeviceIdentityV1, EventIdentityV1, FailureCodeV1, FutureIdentityV1,
    MAX_COMPLETION_GRAPH_EDGES_V1, MAX_COMPLETION_GRAPH_NODES_V1, MAX_COMPLETION_GRAPH_STREAMS_V1,
    StreamIdentityV1,
};
pub use lifecycle::{
    BoundedQuarantine, CancelRequestError, ConcurrentOperationLifecycle, LeakReason,
    MAX_QUARANTINED_OPERATIONS, NotificationError, OperationLifecycle, OperationState,
    PoisonedLifecycle, QuarantineCapacityError, QuarantineTicket, ReclaimError, RetentionOutcome,
    SynchronizedLifecycleError, TerminalState, TransitionError,
};

/// A backend operation or completion failure.
#[derive(Debug, Eq, PartialEq)]
pub enum CompletionError<O, S> {
    /// The operation failed, but synchronization established completion.
    Operation(O),
    /// The operation succeeded, but synchronization could not establish completion.
    Synchronization(S),
    /// Both the operation and the recovery synchronization failed.
    OperationAndSynchronization { operation: O, synchronization: S },
}

/// A completion failure classified by whether backend work is quiescent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionFailure<E> {
    /// Completion was established, but the operation reported an error.
    Quiescent(E),
    /// Completion could not be established, so retained resources remain live.
    Ambiguous(E),
}

/// Backend completion operations used by an owned pending operation.
pub trait Completion {
    type Error;

    fn query(&self) -> Result<bool, Self::Error>;
    fn synchronize(&self) -> Result<(), CompletionFailure<Self::Error>>;
}

/// Owned resources retained until a backend operation has completed.
///
/// Dropping this state synchronizes before dropping its resources. A quiescent
/// execution error still permits cleanup. If completion remains ambiguous or
/// synchronization panics, the resources and completion object are leaked
/// because the backend may still refer to them.
#[derive(Debug)]
pub struct PendingOwned<R, C: Completion> {
    resources: Retained<R>,
    completion: Retained<C>,
    active: bool,
}

impl<R, C: Completion> PendingOwned<R, C> {
    pub fn new(resources: R, completion: C) -> Self {
        Self {
            resources: Retained::new(resources),
            completion: Retained::new(completion),
            active: true,
        }
    }

    pub fn query(&self) -> Result<bool, C::Error> {
        self.completion.get().query()
    }

    pub fn wait(mut self) -> Result<R, C::Error> {
        self.active = false;
        match self.completion.get().synchronize() {
            Ok(()) => {
                drop(self.completion.take());
                Ok(self.resources.take())
            }
            Err(CompletionFailure::Quiescent(error)) => {
                drop(self.completion.take());
                drop(self.resources.take());
                Err(error)
            }
            Err(CompletionFailure::Ambiguous(error)) => Err(error),
        }
    }
}

impl<R, C: Completion> Drop for PendingOwned<R, C> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        match self.completion.get().synchronize() {
            Ok(()) | Err(CompletionFailure::Quiescent(_)) => {
                drop(self.completion.take());
                drop(self.resources.take());
            }
            Err(CompletionFailure::Ambiguous(_)) => {}
        }
    }
}

/// Tries a primary synchronization method and then a stronger fallback.
pub fn synchronize_with_fallback<E>(
    primary: impl FnOnce() -> Result<(), E>,
    fallback: impl FnOnce() -> Result<(), E>,
    combine: impl FnOnce(E, E) -> E,
) -> Result<(), CompletionFailure<E>> {
    match primary() {
        Ok(()) => Ok(()),
        Err(primary) => match fallback() {
            Ok(()) => Err(CompletionFailure::Quiescent(primary)),
            Err(fallback) => Err(CompletionFailure::Ambiguous(combine(primary, fallback))),
        },
    }
}

/// Runs an operation over owned resources and establishes completion.
///
/// Synchronization is attempted even when `operation` reports an error. The
/// resources are returned or dropped only after successful synchronization. If
/// synchronization fails, they are leaked because backend work may still refer
/// to them.
pub fn complete_owned<R, O, S>(
    resources: R,
    operation: impl FnOnce() -> Result<(), O>,
    synchronize: impl FnOnce() -> Result<(), S>,
) -> Result<R, CompletionError<O, S>> {
    let mut resources = Retained::new(resources);
    let operation = operation();
    let synchronization = synchronize();

    match (operation, synchronization) {
        (Ok(()), Ok(())) => Ok(resources.take()),
        (Err(operation), Ok(())) => {
            drop(resources.take());
            Err(CompletionError::Operation(operation))
        }
        (Ok(()), Err(synchronization)) => Err(CompletionError::Synchronization(synchronization)),
        (Err(operation), Err(synchronization)) => {
            Err(CompletionError::OperationAndSynchronization {
                operation,
                synchronization,
            })
        }
    }
}

/// Runs an operation that may retain borrowed caller storage until completion.
///
/// Synchronization is attempted even when `operation` reports an error. A
/// synchronization failure aborts the process because returning or unwinding
/// would release caller borrows while backend work may still use them.
pub fn complete_borrowed<O, S>(
    operation: impl FnOnce() -> Result<(), O>,
    synchronize: impl FnOnce() -> Result<(), S>,
) -> Result<(), CompletionError<O, S>> {
    let mut abort_on_unwind = AbortOnDrop(true);
    let operation = operation();

    if synchronize().is_err() {
        std::process::abort();
    }

    abort_on_unwind.0 = false;
    operation.map_err(CompletionError::Operation)
}

/// Establishes completion before releasing resources borrowed by a backend.
///
/// A quiescent execution error is returned to the caller. Ambiguous completion
/// or a panic aborts the process because unwinding would release resources that
/// the backend may still access.
pub fn settle_borrowed<E>(
    synchronize: impl FnOnce() -> Result<(), CompletionFailure<E>>,
) -> Result<(), E> {
    let mut abort_on_unwind = AbortOnDrop(true);
    match synchronize() {
        Ok(()) => {
            abort_on_unwind.0 = false;
            Ok(())
        }
        Err(CompletionFailure::Quiescent(error)) => {
            abort_on_unwind.0 = false;
            Err(error)
        }
        Err(CompletionFailure::Ambiguous(_)) => std::process::abort(),
    }
}

#[derive(Debug)]
struct Retained<R>(Option<R>);

impl<R> Retained<R> {
    fn new(resources: R) -> Self {
        Self(Some(resources))
    }

    fn take(&mut self) -> R {
        self.0.take().expect("retained resources are present")
    }

    fn try_take(&mut self) -> Option<R> {
        self.0.take()
    }

    fn get(&self) -> &R {
        self.0.as_ref().expect("retained resources are present")
    }
}

impl<R> Drop for Retained<R> {
    fn drop(&mut self) {
        if let Some(resources) = self.0.take() {
            core::mem::forget(resources);
        }
    }
}

struct AbortOnDrop(bool);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if self.0 {
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Completion, CompletionError, CompletionFailure, PendingOwned, complete_borrowed,
        complete_owned, settle_borrowed, synchronize_with_fallback,
    };
    use std::cell::RefCell;
    use std::process::Command;
    use std::rc::Rc;

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    const ABORT_CASE: &str = "FE2O3_COMPLETION_ABORT_CASE";

    struct DropRecorder(Rc<RefCell<Vec<&'static str>>>);

    impl Drop for DropRecorder {
        fn drop(&mut self) {
            self.0.borrow_mut().push("resource-drop");
        }
    }

    struct FakeCompletion {
        events: Rc<RefCell<Vec<&'static str>>>,
        query: Result<bool, &'static str>,
        synchronize: Result<(), CompletionFailure<&'static str>>,
        panic_during_synchronize: bool,
        panic_during_drop: bool,
    }

    impl Completion for FakeCompletion {
        type Error = &'static str;

        fn query(&self) -> Result<bool, Self::Error> {
            self.events.borrow_mut().push("query");
            self.query
        }

        fn synchronize(&self) -> Result<(), CompletionFailure<Self::Error>> {
            self.events.borrow_mut().push("synchronize");
            assert!(!self.panic_during_synchronize, "synchronize panic");
            self.synchronize
        }
    }

    impl Drop for FakeCompletion {
        fn drop(&mut self) {
            self.events.borrow_mut().push("completion-drop");
            assert!(!self.panic_during_drop, "completion drop panic");
        }
    }

    fn pending(
        query: Result<bool, &'static str>,
        synchronize: Result<(), CompletionFailure<&'static str>>,
        panic_during_synchronize: bool,
    ) -> (
        PendingOwned<DropRecorder, FakeCompletion>,
        Rc<RefCell<Vec<&'static str>>>,
    ) {
        pending_with_drop(query, synchronize, panic_during_synchronize, false)
    }

    fn pending_with_drop(
        query: Result<bool, &'static str>,
        synchronize: Result<(), CompletionFailure<&'static str>>,
        panic_during_synchronize: bool,
        panic_during_drop: bool,
    ) -> (
        PendingOwned<DropRecorder, FakeCompletion>,
        Rc<RefCell<Vec<&'static str>>>,
    ) {
        let events = Rc::new(RefCell::new(Vec::new()));
        (
            PendingOwned::new(
                DropRecorder(events.clone()),
                FakeCompletion {
                    events: events.clone(),
                    query,
                    synchronize,
                    panic_during_synchronize,
                    panic_during_drop,
                },
            ),
            events,
        )
    }

    #[derive(Clone)]
    struct FakeBackend {
        events: Rc<RefCell<Vec<&'static str>>>,
        operation: Result<(), &'static str>,
        synchronization: Result<(), &'static str>,
    }

    impl FakeBackend {
        fn new(
            operation: Result<(), &'static str>,
            synchronization: Result<(), &'static str>,
        ) -> Self {
            Self {
                events: Rc::new(RefCell::new(Vec::new())),
                operation,
                synchronization,
            }
        }

        fn operation(&self) -> Result<(), &'static str> {
            self.events.borrow_mut().push("operation");
            self.operation
        }

        fn synchronize(&self) -> Result<(), &'static str> {
            self.events.borrow_mut().push("synchronize");
            self.synchronization
        }
    }

    fn run_owned(
        backend: &FakeBackend,
    ) -> Result<DropRecorder, CompletionError<&'static str, &'static str>> {
        let operation_backend = backend.clone();
        let synchronization_backend = backend.clone();
        complete_owned(
            DropRecorder(backend.events.clone()),
            move || operation_backend.operation(),
            move || synchronization_backend.synchronize(),
        )
    }

    #[test]
    fn owned_success_returns_resources_only_after_completion() {
        let backend = FakeBackend::new(Ok(()), Ok(()));

        let resources = run_owned(&backend).unwrap();
        assert_eq!(*backend.events.borrow(), ["operation", "synchronize"]);

        drop(resources);
        assert_eq!(
            *backend.events.borrow(),
            ["operation", "synchronize", "resource-drop"]
        );
    }

    #[test]
    fn owned_operation_error_synchronizes_before_cleanup() {
        let backend = FakeBackend::new(Err("enqueue"), Ok(()));

        assert!(matches!(
            run_owned(&backend),
            Err(CompletionError::Operation("enqueue"))
        ));
        assert_eq!(
            *backend.events.borrow(),
            ["operation", "synchronize", "resource-drop"]
        );
    }

    #[test]
    fn owned_synchronization_error_leaks_resources() {
        let backend = FakeBackend::new(Ok(()), Err("synchronize"));

        assert!(matches!(
            run_owned(&backend),
            Err(CompletionError::Synchronization("synchronize"))
        ));
        assert_eq!(*backend.events.borrow(), ["operation", "synchronize"]);
    }

    #[test]
    fn owned_dual_error_leaks_and_preserves_both_errors() {
        let backend = FakeBackend::new(Err("enqueue"), Err("synchronize"));

        assert!(matches!(
            run_owned(&backend),
            Err(CompletionError::OperationAndSynchronization {
                operation: "enqueue",
                synchronization: "synchronize"
            })
        ));
        assert_eq!(*backend.events.borrow(), ["operation", "synchronize"]);
    }

    #[test]
    fn owned_panics_leak_retained_resources() {
        let operation_events = Rc::new(RefCell::new(Vec::new()));
        let resource_events = operation_events.clone();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = complete_owned(
                DropRecorder(resource_events),
                || -> Result<(), ()> { panic!("operation panic") },
                || Ok::<(), ()>(()),
            );
        }));
        assert!(panic.is_err());
        assert!(operation_events.borrow().is_empty());

        let synchronization_events = Rc::new(RefCell::new(Vec::new()));
        let resource_events = synchronization_events.clone();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = complete_owned(
                DropRecorder(resource_events),
                || Ok::<(), ()>(()),
                || -> Result<(), ()> { panic!("synchronization panic") },
            );
        }));
        assert!(panic.is_err());
        assert!(synchronization_events.borrow().is_empty());
    }

    #[test]
    fn pending_owned_wait_returns_resources_after_completion() {
        let (pending, events) = pending(Ok(false), Ok(()), false);

        let resources = pending.wait().unwrap();
        assert_eq!(*events.borrow(), ["synchronize", "completion-drop"]);
        drop(resources);
        assert_eq!(
            *events.borrow(),
            ["synchronize", "completion-drop", "resource-drop"]
        );
    }

    #[test]
    fn pending_owned_drop_synchronizes_before_cleanup() {
        let (pending, events) = pending(Ok(false), Ok(()), false);

        drop(pending);
        assert_eq!(
            *events.borrow(),
            ["synchronize", "completion-drop", "resource-drop"]
        );
    }

    #[test]
    fn pending_owned_ambiguity_leaks_on_wait_and_drop() {
        let (wait_pending, wait_events) = pending(
            Ok(false),
            Err(CompletionFailure::Ambiguous("ambiguous")),
            false,
        );
        assert!(matches!(wait_pending.wait(), Err("ambiguous")));
        assert_eq!(*wait_events.borrow(), ["synchronize"]);

        let (drop_pending, drop_events) = pending(
            Ok(false),
            Err(CompletionFailure::Ambiguous("ambiguous")),
            false,
        );
        drop(drop_pending);
        assert_eq!(*drop_events.borrow(), ["synchronize"]);
    }

    #[test]
    fn pending_owned_quiescent_error_releases_on_wait_and_drop() {
        let (wait_pending, wait_events) =
            pending(Ok(false), Err(CompletionFailure::Quiescent("event")), false);
        assert!(matches!(wait_pending.wait(), Err("event")));
        assert_eq!(
            *wait_events.borrow(),
            ["synchronize", "completion-drop", "resource-drop"]
        );

        let (drop_pending, drop_events) =
            pending(Ok(false), Err(CompletionFailure::Quiescent("event")), false);
        drop(drop_pending);
        assert_eq!(
            *drop_events.borrow(),
            ["synchronize", "completion-drop", "resource-drop"]
        );
    }

    #[test]
    fn pending_owned_query_never_settles_or_releases_resources() {
        let (query_pending, events) = pending(Ok(false), Ok(()), false);

        assert!(!query_pending.query().unwrap());
        assert_eq!(*events.borrow(), ["query"]);
        drop(query_pending);
        assert_eq!(
            *events.borrow(),
            ["query", "synchronize", "completion-drop", "resource-drop"]
        );

        let (pending, events) = pending(Err("query"), Ok(()), false);
        assert!(matches!(pending.query(), Err("query")));
        assert_eq!(*events.borrow(), ["query"]);
        drop(pending);
        assert_eq!(
            *events.borrow(),
            ["query", "synchronize", "completion-drop", "resource-drop"]
        );
    }

    #[test]
    fn pending_owned_leaks_if_synchronization_panics() {
        let (pending, events) = pending(Ok(false), Ok(()), true);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(pending)));
        assert!(panic.is_err());
        assert_eq!(*events.borrow(), ["synchronize"]);
    }

    #[test]
    fn pending_owned_leaks_if_completion_destructor_panics() {
        let (pending, events) = pending_with_drop(Ok(false), Ok(()), false, true);

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(pending)));
        assert!(panic.is_err());
        assert_eq!(*events.borrow(), ["synchronize", "completion-drop"]);
    }

    #[test]
    fn synchronization_falls_back_and_preserves_both_errors() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let primary_events = events.clone();
        let fallback_events = events.clone();
        let combined = synchronize_with_fallback(
            move || {
                primary_events.borrow_mut().push("primary");
                Err("event".to_owned())
            },
            move || {
                fallback_events.borrow_mut().push("fallback");
                Err("stream".to_owned())
            },
            |primary, fallback| format!("{primary}+{fallback}"),
        );

        assert_eq!(
            combined,
            Err(CompletionFailure::Ambiguous("event+stream".to_owned()))
        );
        assert_eq!(*events.borrow(), ["primary", "fallback"]);

        let events = Rc::new(RefCell::new(Vec::new()));
        let primary_events = events.clone();
        let fallback_events = events.clone();
        assert_eq!(
            synchronize_with_fallback(
                move || {
                    primary_events.borrow_mut().push("primary");
                    Ok::<(), String>(())
                },
                move || {
                    fallback_events.borrow_mut().push("fallback");
                    Ok::<(), String>(())
                },
                |primary, fallback| format!("{primary}+{fallback}"),
            ),
            Ok(())
        );
        assert_eq!(*events.borrow(), ["primary"]);

        assert_eq!(
            synchronize_with_fallback(
                || Err("event".to_owned()),
                || Ok::<(), String>(()),
                |primary, fallback| format!("{primary}+{fallback}"),
            ),
            Err(CompletionFailure::Quiescent("event".to_owned()))
        );
    }

    #[test]
    fn borrowed_success_synchronizes_before_returning() {
        let backend = FakeBackend::new(Ok(()), Ok(()));
        let operation_backend = backend.clone();
        let synchronization_backend = backend.clone();

        complete_borrowed(
            move || operation_backend.operation(),
            move || synchronization_backend.synchronize(),
        )
        .unwrap();
        assert_eq!(*backend.events.borrow(), ["operation", "synchronize"]);
    }

    #[test]
    fn borrowed_operation_error_synchronizes_before_returning() {
        let backend = FakeBackend::new(Err("enqueue"), Ok(()));
        let operation_backend = backend.clone();
        let synchronization_backend = backend.clone();

        assert!(matches!(
            complete_borrowed(
                move || operation_backend.operation(),
                move || synchronization_backend.synchronize()
            ),
            Err(CompletionError::Operation("enqueue"))
        ));
        assert_eq!(*backend.events.borrow(), ["operation", "synchronize"]);
    }

    #[test]
    fn borrowed_settlement_returns_quiescent_execution_error() {
        assert_eq!(
            settle_borrowed(|| Err(CompletionFailure::Quiescent("event"))),
            Err("event")
        );
    }

    #[test]
    fn ambiguous_borrowed_completion_aborts() {
        if let Ok(case) = std::env::var(ABORT_CASE) {
            match case.as_str() {
                "operation-success" | "operation-failure" => {
                    let operation = if case == "operation-success" {
                        Ok(())
                    } else {
                        Err("enqueue")
                    };
                    let backend = FakeBackend::new(operation, Err("synchronize"));
                    let operation_backend = backend.clone();
                    let synchronization_backend = backend.clone();
                    let _ = complete_borrowed(
                        move || operation_backend.operation(),
                        move || synchronization_backend.synchronize(),
                    );
                }
                "operation-panic" => {
                    let _ = complete_borrowed::<(), ()>(
                        || panic!("operation panicked after it may have submitted work"),
                        || Ok(()),
                    );
                }
                "settlement-ambiguous" => {
                    let _ = settle_borrowed(|| Err(CompletionFailure::Ambiguous("stream")));
                }
                "settlement-panic" => {
                    let _ = settle_borrowed::<()>(|| panic!("completion panic"));
                }
                _ => panic!("unknown abort case"),
            }
            std::process::exit(99);
        }

        for case in [
            "operation-success",
            "operation-failure",
            "operation-panic",
            "settlement-ambiguous",
            "settlement-panic",
        ] {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::ambiguous_borrowed_completion_aborts",
                    "--nocapture",
                ])
                .env(ABORT_CASE, case)
                .output()
                .unwrap();
            assert_ne!(
                output.status.code(),
                Some(99),
                "borrowed case {case} returned instead of aborting"
            );
            #[cfg(unix)]
            assert_eq!(
                output.status.signal(),
                Some(6),
                "borrowed case {case} did not terminate with SIGABRT"
            );
        }
    }
}
