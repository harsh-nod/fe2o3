//! Panic settlement for the single live-model loan, not native output custody.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type CustodyResult<R, E> = Result<(R, Result<(), E>), E>;

pub(in crate::queue) fn execute_live_model_custody_v1<C, L, R, E>(
    context: &mut C,
    open: impl FnOnce(&mut C) -> Result<L, E>,
    operation: impl FnOnce(&mut C) -> R,
    retake: impl FnOnce(&mut C, L) -> Result<(), E>,
    poison: impl FnOnce(&mut C),
) -> CustodyResult<R, E> {
    let loan = match catch_unwind(AssertUnwindSafe(|| open(context))) {
        Ok(loan) => loan?,
        Err(payload) => {
            poison(context);
            resume_unwind(payload)
        }
    };
    let result = catch_unwind(AssertUnwindSafe(|| operation(context)));
    let closing = catch_unwind(AssertUnwindSafe(|| retake(context, loan)));
    if result.is_err() || !matches!(closing, Ok(Ok(()))) {
        poison(context);
    }
    match result {
        Err(payload) => {
            // Terminal-only retention: secondary panic/error destructors must
            // not replace the original panic. No retry authority escapes.
            core::mem::forget(closing);
            resume_unwind(payload)
        }
        Ok(result) => match closing {
            Ok(closing) => Ok((result, closing)),
            Err(payload) => resume_unwind(payload),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Outcome {
        Success,
        Error,
        Panic,
    }

    fn outcome(value: Outcome, message: &'static str) -> Result<(), &'static str> {
        match value {
            Outcome::Success => Ok(()),
            Outcome::Error => Err(message),
            Outcome::Panic => std::panic::panic_any(message),
        }
    }

    #[test]
    fn live_model_custody_operation_retake_matrix_preserves_order_and_first_panic() {
        for operation in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            for retake in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                let mut trace = Vec::new();
                let caught = catch_unwind(AssertUnwindSafe(|| {
                    execute_live_model_custody_v1(
                        &mut trace,
                        |trace| {
                            trace.push("open");
                            Ok(17)
                        },
                        |trace| {
                            trace.push("operation");
                            outcome(operation, "operation")
                        },
                        |trace, loan| {
                            assert_eq!(loan, 17);
                            trace.push("retake");
                            outcome(retake, "retake")
                        },
                        |trace| trace.push("poison"),
                    )
                }));
                let poisoned = operation == Outcome::Panic || retake != Outcome::Success;
                assert_eq!(
                    trace,
                    if poisoned {
                        vec!["open", "operation", "retake", "poison"]
                    } else {
                        vec!["open", "operation", "retake"]
                    }
                );
                if operation == Outcome::Panic || retake == Outcome::Panic {
                    let payload = caught.expect_err("panic must escape after poison");
                    let first = if operation == Outcome::Panic {
                        "operation"
                    } else {
                        "retake"
                    };
                    assert_eq!(payload.downcast_ref::<&str>(), Some(&first));
                } else {
                    let (actual_operation, actual_retake) = caught.unwrap().unwrap();
                    assert_eq!(actual_operation, outcome(operation, "operation"));
                    assert_eq!(actual_retake, outcome(retake, "retake"));
                }
            }
        }
    }

    #[test]
    fn live_model_custody_opening_rejection_and_panic_never_retake() {
        for open in [Outcome::Error, Outcome::Panic] {
            let mut trace = Vec::new();
            let caught = catch_unwind(AssertUnwindSafe(|| {
                execute_live_model_custody_v1(
                    &mut trace,
                    |trace| {
                        trace.push("open");
                        outcome(open, "open")
                    },
                    |_| -> () { panic!("rejected opening must not execute") },
                    |_, ()| panic!("no loan to retake"),
                    |trace| trace.push("poison"),
                )
            }));
            if open == Outcome::Error {
                assert_eq!(caught.unwrap(), Err("open"));
                assert_eq!(trace, ["open"]);
            } else {
                assert_eq!(caught.unwrap_err().downcast_ref::<&str>(), Some(&"open"));
                assert_eq!(trace, ["open", "poison"]);
            }
        }
    }

    struct PanickingDrop(Arc<AtomicUsize>);
    impl Drop for PanickingDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("secondary destructor must never run");
        }
    }

    #[test]
    fn live_model_custody_preserves_primary_without_destroying_secondary_payload() {
        for secondary_panic in [false, true] {
            let drops = Arc::new(AtomicUsize::new(0));
            let mut poisoned = false;
            let caught = catch_unwind(AssertUnwindSafe(|| {
                execute_live_model_custody_v1(
                    &mut poisoned,
                    |_| Ok(()),
                    |_| -> () { std::panic::panic_any("primary") },
                    |_, ()| {
                        let secondary = PanickingDrop(drops.clone());
                        if secondary_panic {
                            std::panic::panic_any(secondary)
                        }
                        Err(secondary)
                    },
                    |poisoned| *poisoned = true,
                )
            }));
            let Err(payload) = caught else {
                panic!("primary panic must escape")
            };
            assert!(poisoned);
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"primary"));
            assert_eq!(drops.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn live_model_custody_external_owner_survives_closing_failure() {
        for retake in [Outcome::Error, Outcome::Panic] {
            let mut owner = None;
            let mut poisoned = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                execute_live_model_custody_v1(
                    &mut poisoned,
                    |_| Ok(()),
                    |_| owner = Some(Box::new([3, 5, 7])),
                    |_, ()| outcome(retake, "retake"),
                    |poisoned| *poisoned = true,
                )
            }));
            assert!(poisoned);
            assert_eq!(owner.as_deref(), Some(&[3, 5, 7]));
            if retake == Outcome::Error {
                assert_eq!(result.unwrap(), Ok(((), Err("retake"))));
            } else {
                assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&"retake"));
            }
        }
    }
}
