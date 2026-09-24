//! Synthetic transport uses the actual metadata retirement engine, no native owner.
use super::super::{NOTIFICATION_SIDE_EFFECT, abi};
use super::*;
use core::sync::atomic::Ordering;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Default)]
struct Fake {
    calls: Vec<&'static str>,
    fail: Option<usize>,
    panic_at: Option<usize>,
}
impl Fake {
    fn call(&mut self, op: &'static str) -> Result<(), E> {
        self.calls.push(op);
        if self.panic_at == Some(self.calls.len()) {
            panic!("injected");
        }
        if self.fail == Some(self.calls.len()) {
            Err(E::Currentness)
        } else {
            Ok(())
        }
    }
}
impl DebugEmptyRetirementTransportV1 for Fake {
    fn check_currentness(&mut self) -> Result<(), E> {
        self.call("currentness")
    }
    fn disable_runtime(&mut self) -> Result<(), E> {
        self.call("disable")
    }
    fn clear_trap(&mut self) -> Result<(), E> {
        self.call("clear")
    }
}
fn active() -> MetadataStorageV1 {
    let mut s = MetadataStorageV1::prepare(b"inert, not loader admitted", 0).unwrap();
    s.activate_for_test();
    s.publish_link().unwrap();
    s
}
fn reclaim(s: &mut MetadataStorageV1) {
    drop(s.take_storage_to_retain());
}
#[test]
fn actual_local_order_withdraws_before_disable_before_clear() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut s = active();
    let before = NOTIFICATION_SIDE_EFFECT.load(Ordering::Relaxed);
    let mut t = Fake::default();
    s.withdraw_empty_queue(&mut t).unwrap();
    assert_eq!(s.phase, Phase::ActiveAbsent);
    assert_eq!(s.record[0].root.map, 0);
    assert_eq!(s.record[0].root.state, abi::RT_CONSISTENT_V1);
    assert_eq!(
        NOTIFICATION_SIDE_EFFECT
            .load(Ordering::Relaxed)
            .wrapping_sub(before),
        2
    );
    s.disable_empty_runtime(&mut t).unwrap();
    assert_eq!(s.phase, Phase::LocalRuntimeDisabled);
    assert!(!s.empty_local_trap_cleared());
    s.clear_empty_trap(&mut t).unwrap();
    assert!(s.empty_local_trap_cleared());
    assert_eq!(
        t.calls,
        [
            "currentness",
            "currentness",
            "currentness",
            "currentness",
            "currentness",
            "currentness",
            "disable",
            "currentness",
            "currentness",
            "clear",
            "currentness",
        ]
    );
    assert!(s.take_storage_to_retain().is_none());
}
#[test]
fn disable_or_clear_cannot_skip_metadata_withdrawal() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut s = active();
    let mut t = Fake::default();
    assert_eq!(s.disable_empty_runtime(&mut t), Err(E::Transition));
    assert_eq!(s.clear_empty_trap(&mut t), Err(E::Transition));
    assert!(t.calls.is_empty());
    reclaim(&mut s);
}
#[test]
fn every_withdraw_failure_keeps_all_pointees_retained() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    for fail in 1..=5 {
        let mut s = active();
        let mut t = Fake {
            fail: Some(fail),
            ..Fake::default()
        };
        assert!(s.withdraw_empty_queue(&mut t).is_err());
        assert!(!s.empty_local_trap_cleared());
        assert!(s.take_storage_to_retain().is_some());
    }
}
#[test]
fn every_disable_or_clear_failure_retains_metadata_and_no_later_call() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    for clear in [false, true] {
        for fail in 1..=3 {
            let mut s = active();
            s.withdraw_empty_queue(&mut Fake::default()).unwrap();
            if clear {
                s.disable_empty_runtime(&mut Fake::default()).unwrap();
            }
            let mut t = Fake {
                fail: Some(fail),
                ..Fake::default()
            };
            let result = if clear {
                s.clear_empty_trap(&mut t)
            } else {
                s.disable_empty_runtime(&mut t)
            };
            assert!(result.is_err());
            assert_eq!(t.calls.len(), fail);
            assert!(!s.empty_local_trap_cleared());
            assert!(s.take_storage_to_retain().is_some());
        }
    }
}
#[test]
fn panic_during_native_disable_or_clear_retains_published_storage() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    for clear in [false, true] {
        let mut s = active();
        s.withdraw_empty_queue(&mut Fake::default()).unwrap();
        if clear {
            s.disable_empty_runtime(&mut Fake::default()).unwrap();
        }
        let mut t = Fake {
            panic_at: Some(2),
            ..Fake::default()
        };
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if clear {
                    s.clear_empty_trap(&mut t)
                } else {
                    s.disable_empty_runtime(&mut t)
                }
            }))
            .is_err()
        );
        assert_eq!(s.phase, Phase::Poisoned);
        assert!(s.take_storage_to_retain().is_some());
    }
}
#[test]
fn process_drift_before_disable_prevents_transport_and_retains_storage() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut s = active();
    s.withdraw_empty_queue(&mut Fake::default()).unwrap();
    s.opener_pid ^= 1;
    let mut t = Fake::default();
    assert_eq!(s.disable_empty_runtime(&mut t), Err(E::ProcessChanged));
    assert!(t.calls.is_empty());
    assert!(s.take_storage_to_retain().is_some());
}
#[test]
fn local_clear_is_terminal_and_never_reactivates_metadata() {
    let _lock = super::super::tests::NOTIFICATION_TEST_LOCK.lock().unwrap();
    let mut s = active();
    let mut t = Fake::default();
    s.withdraw_empty_queue(&mut t).unwrap();
    s.disable_empty_runtime(&mut t).unwrap();
    s.clear_empty_trap(&mut t).unwrap();
    let n = t.calls.len();
    assert_eq!(s.disable_empty_runtime(&mut t), Err(E::Transition));
    assert_eq!(s.clear_empty_trap(&mut t), Err(E::Transition));
    assert_eq!(t.calls.len(), n);
    assert!(!s.is_prepared());
}
