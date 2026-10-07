//! Settle both model loans without moving operation-owned native custody.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

pub(super) struct Failure<E> {
    pub(super) error: E,
    pub(super) terminal: bool,
}

pub(super) trait Context {
    type Loan;
    type Error;

    fn open(&mut self, endpoint: usize) -> Result<Self::Loan, Self::Error>;
    fn retake(&mut self, endpoint: usize, loan: Self::Loan) -> Result<(), Self::Error>;
    fn poison_endpoint(&mut self, endpoint: usize);
    fn poison_process(&mut self);
}

struct Settlement<E> {
    error: Option<E>,
    panic: Option<Box<dyn core::any::Any + Send>>,
    terminal: bool,
}

impl<E> Settlement<E> {
    fn observe(&mut self, result: std::thread::Result<Result<(), Failure<E>>>) -> bool {
        match result {
            Ok(Ok(())) => true,
            Ok(Err(failure)) => {
                self.terminal |= failure.terminal;
                if self.error.is_none() {
                    self.error = Some(failure.error);
                } else {
                    // Error payload destructors must not interrupt settlement.
                    core::mem::forget(failure.error);
                }
                false
            }
            Err(payload) => {
                self.terminal = true;
                if self.panic.is_none() {
                    self.panic = Some(payload);
                } else {
                    core::mem::forget(payload);
                }
                false
            }
        }
    }
}

/// The operation returns diagnostics only. All created/recovered native owners
/// must be rooted outside this function before its operation callback returns.
pub(super) fn execute<C: Context>(
    context: &mut C,
    operation: impl FnOnce(&mut C) -> Result<(), Failure<C::Error>>,
) -> Result<(), Failure<C::Error>> {
    let mut settlement = Settlement {
        error: None,
        panic: None,
        terminal: false,
    };
    let mut loans = [None, None];
    let mut opened = true;
    for (endpoint, slot) in loans.iter_mut().enumerate() {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            *slot = Some(context.open(endpoint).map_err(|error| Failure {
                error,
                terminal: true,
            })?);
            Ok(())
        }));
        if !settlement.observe(outcome) {
            opened = false;
            break;
        }
    }
    if opened {
        settlement.observe(catch_unwind(AssertUnwindSafe(|| operation(context))));
    }
    // Never let one failed retake skip the other foundation's settlement.
    for (endpoint, loan) in loans.into_iter().enumerate().rev() {
        if let Some(loan) = loan {
            settlement.observe(catch_unwind(AssertUnwindSafe(|| {
                context.retake(endpoint, loan).map_err(|error| Failure {
                    error,
                    terminal: true,
                })
            })));
        }
    }
    if settlement.terminal {
        for endpoint in 0..2 {
            settlement.observe(catch_unwind(AssertUnwindSafe(|| {
                context.poison_endpoint(endpoint);
                Ok(())
            })));
        }
        settlement.observe(catch_unwind(AssertUnwindSafe(|| {
            context.poison_process();
            Ok(())
        })));
    }
    if let Some(payload) = settlement.panic {
        core::mem::forget(settlement.error);
        resume_unwind(payload);
    }
    match settlement.error {
        Some(error) => Err(Failure {
            error,
            terminal: settlement.terminal,
        }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Outcome {
        Success,
        Error,
        Panic,
    }

    fn outcome(value: Outcome, label: &'static str) -> Result<(), &'static str> {
        match value {
            Outcome::Success => Ok(()),
            Outcome::Error => Err(label),
            Outcome::Panic => std::panic::panic_any(label),
        }
    }

    struct Fixture {
        opens: [Outcome; 2],
        retakes: [Outcome; 2],
        poison_panics: [bool; 3],
        trace: Vec<&'static str>,
        rooted: Rc<Cell<bool>>,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                opens: [Outcome::Success; 2],
                retakes: [Outcome::Success; 2],
                poison_panics: [false; 3],
                trace: Vec::new(),
                rooted: Rc::new(Cell::new(false)),
            }
        }
    }

    impl Context for Fixture {
        type Loan = usize;
        type Error = &'static str;

        fn open(&mut self, endpoint: usize) -> Result<usize, &'static str> {
            let label = ["open0", "open1"][endpoint];
            self.trace.push(label);
            outcome(self.opens[endpoint], label)?;
            Ok(endpoint)
        }

        fn retake(&mut self, endpoint: usize, loan: usize) -> Result<(), &'static str> {
            assert_eq!(endpoint, loan);
            let label = ["retake0", "retake1"][endpoint];
            self.trace.push(label);
            if self.trace.contains(&"operation") {
                assert!(self.rooted.get());
            }
            outcome(self.retakes[endpoint], label)
        }

        fn poison_endpoint(&mut self, endpoint: usize) {
            let label = ["poison0", "poison1"][endpoint];
            self.trace.push(label);
            if self.poison_panics[endpoint] {
                std::panic::panic_any(label);
            }
        }

        fn poison_process(&mut self) {
            self.trace.push("process");
            if self.poison_panics[2] {
                std::panic::panic_any("process");
            }
        }
    }

    #[test]
    fn pair_retake_matrix_retains_output_and_attempts_both_reclaims() {
        for first in [Outcome::Success, Outcome::Error, Outcome::Panic] {
            for second in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                let mut fixture = Fixture {
                    retakes: [first, second],
                    ..Fixture::default()
                };
                let root = fixture.rooted.clone();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    execute(&mut fixture, |fixture| {
                        fixture.trace.push("operation");
                        root.set(true);
                        Ok(())
                    })
                }));
                assert!(root.get());
                let mut expected = vec!["open0", "open1", "operation", "retake1", "retake0"];
                if first != Outcome::Success || second != Outcome::Success {
                    expected.extend(["poison0", "poison1", "process"]);
                }
                assert_eq!(fixture.trace, expected);
                if second == Outcome::Panic || first == Outcome::Panic {
                    let payload = result.err().expect("retake panic must survive");
                    let first_panic = if second == Outcome::Panic {
                        "retake1"
                    } else {
                        "retake0"
                    };
                    assert_eq!(payload.downcast_ref::<&str>(), Some(&first_panic));
                } else if first != Outcome::Success || second != Outcome::Success {
                    assert!(result.unwrap().err().unwrap().terminal);
                } else {
                    assert!(result.unwrap().is_ok());
                }
            }
        }
    }

    #[test]
    fn pair_second_open_failure_reclaims_first_and_never_operates() {
        for opening in [Outcome::Error, Outcome::Panic] {
            for closing in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                let mut fixture = Fixture {
                    opens: [Outcome::Success, opening],
                    retakes: [closing, Outcome::Success],
                    ..Fixture::default()
                };
                let result = catch_unwind(AssertUnwindSafe(|| {
                    execute(&mut fixture, |_| panic!("must not execute"))
                }));
                assert_eq!(
                    fixture.trace,
                    ["open0", "open1", "retake0", "poison0", "poison1", "process"]
                );
                if opening == Outcome::Panic {
                    assert_eq!(result.err().unwrap().downcast_ref::<&str>(), Some(&"open1"));
                } else if closing != Outcome::Panic {
                    assert_eq!(result.unwrap().err().unwrap().error, "open1");
                }
            }
        }
    }

    #[test]
    fn pair_first_open_failure_never_opens_peer_or_retakes() {
        for opening in [Outcome::Error, Outcome::Panic] {
            let mut fixture = Fixture {
                opens: [opening, Outcome::Success],
                ..Fixture::default()
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                execute(&mut fixture, |_| panic!("must not execute"))
            }));
            assert_eq!(fixture.trace, ["open0", "poison0", "poison1", "process"]);
            if opening == Outcome::Panic {
                assert_eq!(result.err().unwrap().downcast_ref::<&str>(), Some(&"open0"));
            } else {
                assert!(result.unwrap().err().unwrap().terminal);
            }
        }
    }

    #[test]
    fn pair_external_native_owner_is_not_dropped_by_retake_failure() {
        struct Owner(Rc<Cell<usize>>);
        impl Drop for Owner {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        for endpoint in 0..2 {
            for closing in [Outcome::Error, Outcome::Panic] {
                let drops = Rc::new(Cell::new(0));
                let mut root = None;
                let mut fixture = Fixture::default();
                fixture.retakes[endpoint] = closing;
                let _ = catch_unwind(AssertUnwindSafe(|| {
                    execute(&mut fixture, |fixture| {
                        fixture.trace.push("operation");
                        root = Some(Owner(drops.clone()));
                        fixture.rooted.set(true);
                        Ok(())
                    })
                }));
                assert!(root.is_some());
                assert_eq!(drops.get(), 0);
                drop(root);
                assert_eq!(drops.get(), 1);
            }
        }
    }

    #[test]
    fn pair_secondary_diagnostic_destructors_cannot_replace_original_panic() {
        struct Hostile(Rc<Cell<usize>>);
        impl Drop for Hostile {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
                panic!("secondary diagnostic destructor");
            }
        }
        struct HostileContext {
            drops: Rc<Cell<usize>>,
            poison: [bool; 3],
        }
        impl Context for HostileContext {
            type Loan = ();
            type Error = Hostile;
            fn open(&mut self, _: usize) -> Result<(), Hostile> {
                Ok(())
            }
            fn retake(&mut self, endpoint: usize, _: ()) -> Result<(), Hostile> {
                if endpoint == 1 {
                    Err(Hostile(self.drops.clone()))
                } else {
                    std::panic::panic_any("original")
                }
            }
            fn poison_endpoint(&mut self, endpoint: usize) {
                self.poison[endpoint] = true;
                std::panic::panic_any("secondary poison");
            }
            fn poison_process(&mut self) {
                self.poison[2] = true;
            }
        }
        let drops = Rc::new(Cell::new(0));
        let mut context = HostileContext {
            drops: drops.clone(),
            poison: [false; 3],
        };
        let result = catch_unwind(AssertUnwindSafe(|| execute(&mut context, |_| Ok(()))));
        assert_eq!(
            result.err().unwrap().downcast_ref::<&str>(),
            Some(&"original")
        );
        assert_eq!(context.poison, [true; 3]);
        assert_eq!(drops.get(), 0);
    }

    #[test]
    fn pair_operation_panic_precedes_all_retake_and_poison_panics() {
        let mut fixture = Fixture {
            retakes: [Outcome::Panic; 2],
            poison_panics: [true; 3],
            ..Fixture::default()
        };
        let result = catch_unwind(AssertUnwindSafe(|| {
            execute(&mut fixture, |fixture| {
                fixture.trace.push("operation");
                fixture.rooted.set(true);
                std::panic::panic_any("original")
            })
        }));
        assert!(fixture.rooted.get());
        assert_eq!(
            fixture.trace,
            [
                "open0",
                "open1",
                "operation",
                "retake1",
                "retake0",
                "poison0",
                "poison1",
                "process"
            ]
        );
        assert_eq!(
            result.err().unwrap().downcast_ref::<&str>(),
            Some(&"original")
        );
    }

    #[test]
    fn pair_retryable_operation_error_does_not_poison_but_terminal_error_does() {
        for terminal in [false, true] {
            let mut fixture = Fixture::default();
            let failure = execute(&mut fixture, |_| {
                Err(Failure {
                    error: "operation",
                    terminal,
                })
            })
            .err()
            .unwrap();
            assert_eq!(failure.error, "operation");
            assert_eq!(failure.terminal, terminal);
            assert_eq!(fixture.trace.contains(&"process"), terminal);
            assert_eq!(
                &fixture.trace[..4],
                ["open0", "open1", "retake1", "retake0"]
            );
        }
    }
}
