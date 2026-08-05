#![forbid(unsafe_code)]

//! Completion policy for safe wrappers around asynchronous backend calls.
//!
//! This crate deliberately has no HIP dependency so failure and cleanup policy
//! can be exercised on CPU-only CI runners.

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

struct Retained<R>(Option<R>);

impl<R> Retained<R> {
    fn new(resources: R) -> Self {
        Self(Some(resources))
    }

    fn take(&mut self) -> R {
        self.0.take().expect("retained resources are present")
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
    use super::{CompletionError, complete_borrowed, complete_owned};
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
                _ => panic!("unknown abort case"),
            }
            std::process::exit(99);
        }

        for case in ["operation-success", "operation-failure", "operation-panic"] {
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
