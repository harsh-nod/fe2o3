//! Pure host custody tests. No device, native mapping, runtime or trap exists.
use super::*;
use std::cell::Cell;
use std::rc::Rc;

struct CountDrop(Rc<Cell<u32>>);
impl Drop for CountDrop {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn counted() -> (RetainNativeOnDropV1<CountDrop>, Rc<Cell<u32>>) {
    let count = Rc::new(Cell::new(0));
    (
        RetainNativeOnDropV1::new(CountDrop(Rc::clone(&count))),
        count,
    )
}

#[test]
fn clean_inert_drop_releases_host_owner() {
    let (owner, count) = counted();
    drop(owner);
    assert_eq!(count.get(), 1);
}

#[test]
fn possible_effects_retain_owner_on_drop() {
    let (mut owner, count) = counted();
    owner.retain_before_native_effect();
    drop(owner);
    assert_eq!(count.get(), 0);
}

#[test]
fn arm_is_sticky_and_owner_remains_accessible() {
    let (mut owner, count) = counted();
    owner.retain_before_native_effect();
    owner.retain_before_native_effect();
    assert!(Rc::ptr_eq(&owner.get().0, &count));
    assert!(Rc::ptr_eq(&owner.get_mut().0, &count));
    drop(owner);
    assert_eq!(count.get(), 0);
}

#[test]
fn error_after_arm_retains() {
    let count = Rc::new(Cell::new(0));
    let result: Result<(), ()> = {
        let mut owner = RetainNativeOnDropV1::new(CountDrop(Rc::clone(&count)));
        owner.retain_before_native_effect();
        Err(())
    };
    assert_eq!(result, Err(()));
    assert_eq!(count.get(), 0);
}

#[test]
fn unwind_after_arm_retains() {
    let count = Rc::new(Cell::new(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut owner = RetainNativeOnDropV1::new(CountDrop(Rc::clone(&count)));
        owner.retain_before_native_effect();
        panic!("synthetic owner unwind");
    }));
    assert!(result.is_err());
    assert_eq!(count.get(), 0);
}

#[test]
fn unwind_before_arm_releases_only_host_owner() {
    let count = Rc::new(Cell::new(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _owner = RetainNativeOnDropV1::new(CountDrop(Rc::clone(&count)));
        panic!("synthetic inert unwind");
    }));
    assert!(result.is_err());
    assert_eq!(count.get(), 1);
}

#[test]
fn foreign_pid_retains_without_native_cleanup() {
    let (mut owner, count) = counted();
    owner.opener_pid = std::process::id().wrapping_add(1);
    drop(owner);
    assert_eq!(count.get(), 0);
}

#[test]
fn policy_never_reclaims_possible_effects_or_unknown_process() {
    assert!(!must_retain(false, 1, 1));
    for (effect, owner, current) in [
        (true, 1, 1),
        (true, 1, 2),
        (false, 1, 2),
        (false, 0, 0),
        (false, 0, 1),
        (true, 0, 0),
    ] {
        assert!(must_retain(effect, owner, current));
    }
}
