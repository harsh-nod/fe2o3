use std::{cell::RefCell, rc::Rc};

use super::*;

#[derive(Default)]
struct Owner {
    trace: Rc<RefCell<Vec<&'static str>>>,
    value: u64,
    checks: usize,
    fail_at: Option<usize>,
    panic_at: Option<usize>,
    poisoned: bool,
}

impl RetainedDeviceScopeOwnerV1 for Owner {
    type Subject = u64;
    type Error = &'static str;

    fn check_scope(&mut self) -> Result<(), Self::Error> {
        if self.poisoned {
            return Err("poisoned");
        }
        self.checks += 1;
        self.trace.borrow_mut().push("check");
        if self.panic_at == Some(self.checks) {
            panic!("currentness panic");
        }
        if self.fail_at == Some(self.checks) {
            return Err("currentness error");
        }
        Ok(())
    }

    fn subject(&self) -> &u64 {
        self.trace.borrow_mut().push("borrow");
        &self.value
    }

    fn poison_scope(&mut self) {
        self.poisoned = true;
        self.trace.borrow_mut().push("poison");
    }
}

struct Candidate(Rc<RefCell<Vec<&'static str>>>);

impl Drop for Candidate {
    fn drop(&mut self) {
        self.0.borrow_mut().push("drop candidate");
    }
}

#[test]
fn retained_scope_is_reusable_and_keeps_error_results_inside_the_envelope() {
    let mut owner = Owner {
        value: 7,
        ..Owner::default()
    };
    for expected in [Ok(7), Err("callback error")] {
        let result = with_retained_device_scope_v1(&mut owner, |device| {
            assert_eq!(*device, 7);
            expected
        })
        .unwrap();
        assert_eq!(result, expected);
        assert!(!owner.poisoned);
    }
    assert_eq!(
        *owner.trace.borrow(),
        ["check", "borrow", "check", "check", "borrow", "check"]
    );
}

#[test]
fn retained_scope_opening_failure_never_calls_back_or_retries_observation() {
    let mut owner = Owner {
        fail_at: Some(1),
        ..Owner::default()
    };
    assert_eq!(
        with_retained_device_scope_v1(&mut owner, |_| panic!("must not run")),
        Err("currentness error")
    );
    assert_eq!(
        with_retained_device_scope_v1(&mut owner, |_| panic!("must not retry")),
        Err("poisoned")
    );
    assert_eq!(owner.checks, 1);
    assert_eq!(*owner.trace.borrow(), ["check", "poison", "poison"]);
}

#[test]
fn retained_scope_closing_failure_poisons_before_candidate_disposal() {
    let mut owner = Owner {
        fail_at: Some(2),
        ..Owner::default()
    };
    let trace = Rc::clone(&owner.trace);
    assert!(matches!(
        with_retained_device_scope_v1(&mut owner, |_| Candidate(trace)),
        Err("currentness error")
    ));
    assert_eq!(
        *owner.trace.borrow(),
        ["check", "borrow", "check", "poison", "drop candidate"]
    );
    assert_eq!(
        with_retained_device_scope_v1(&mut owner, |_| 0),
        Err("poisoned")
    );
    assert_eq!(owner.checks, 2);
}

#[test]
fn retained_scope_currentness_panics_poison_before_unwind_disposal() {
    for panic_at in [1, 2] {
        let mut owner = Owner {
            panic_at: Some(panic_at),
            ..Owner::default()
        };
        let trace = Rc::clone(&owner.trace);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            with_retained_device_scope_v1(&mut owner, |_| Candidate(trace))
        }))
        .err()
        .unwrap();
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"currentness panic"));
        assert!(owner.poisoned);
        assert_eq!(owner.checks, panic_at);
        if panic_at == 1 {
            assert_eq!(*owner.trace.borrow(), ["check", "poison"]);
        } else {
            assert_eq!(
                *owner.trace.borrow(),
                ["check", "borrow", "check", "poison", "drop candidate"]
            );
        }
    }
}

#[test]
fn retained_scope_callback_panic_skips_closing_check_and_preserves_payload() {
    let mut owner = Owner {
        panic_at: Some(2),
        ..Owner::default()
    };
    let panic = catch_unwind(AssertUnwindSafe(|| {
        with_retained_device_scope_v1(&mut owner, |_| panic!("callback panic"))
    }))
    .err()
    .unwrap();
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"callback panic"));
    assert_eq!(owner.checks, 1);
    assert!(owner.poisoned);
    assert_eq!(*owner.trace.borrow(), ["check", "borrow", "poison"]);
}

#[test]
fn retained_scope_success_keeps_candidate_alive_until_caller_disposal() {
    let mut owner = Owner::default();
    let trace = Rc::clone(&owner.trace);
    let candidate = with_retained_device_scope_v1(&mut owner, |_| Candidate(trace)).unwrap();
    assert_eq!(*owner.trace.borrow(), ["check", "borrow", "check"]);
    drop(candidate);
    assert_eq!(
        *owner.trace.borrow(),
        ["check", "borrow", "check", "drop candidate"]
    );
    assert!(!owner.poisoned);
}
