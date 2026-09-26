use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

fn pointers(queues: &[Option<Box<i32>>; 2]) -> [Option<*const i32>; 2] {
    queues
        .each_ref()
        .map(|queue| queue.as_ref().map(|value| &**value as *const _))
}

#[test]
fn native_xgmi_retirement_success_removes_only_selected_slot() {
    for direction in 0..2 {
        let mut queues = [Some(Box::new(11)), Some(Box::new(22))];
        let before = pointers(&queues);
        let mut terminal = false;
        settle_xgmi_queue_retirement(&mut queues, &mut terminal, direction, |queue| {
            assert_eq!(Some(&**queue as *const _), before[direction]);
            Ok::<(), ()>(())
        })
        .unwrap();
        assert!(!terminal);
        assert!(queues[direction].is_none());
        assert_eq!(pointers(&queues)[1 - direction], before[1 - direction]);
        settle_xgmi_queue_retirement(
            &mut queues,
            &mut terminal,
            direction,
            |_| -> Result<(), ()> { panic!("absent queue callback") },
        )
        .unwrap();
        assert!(!terminal);
    }
}

struct HostileDisplay;
impl fmt::Display for HostileDisplay {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        panic_any("retirement diagnostic panic")
    }
}

#[test]
fn native_xgmi_retirement_error_keeps_both_slots_and_latches_before_formatting() {
    for direction in 0..2 {
        let mut queues = [Some(Box::new(11)), Some(Box::new(22))];
        let before = pointers(&queues);
        let mut terminal = false;
        let result = settle_xgmi_queue_retirement(&mut queues, &mut terminal, direction, |queue| {
            **queue = 33;
            Err(HostileDisplay)
        });
        assert!(terminal);
        assert_eq!(pointers(&queues), before);
        assert_eq!(**queues[direction].as_ref().unwrap(), 33);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            result.map_err(|error| format!("XGMI queue teardown: {error}"))
        }))
        .unwrap_err();
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"retirement diagnostic panic")
        );
        assert!(terminal);
        assert_eq!(pointers(&queues), before);
    }
}

#[test]
fn native_xgmi_retirement_panic_keeps_both_slots_and_original_payload() {
    for direction in 0..2 {
        let mut queues = [Some(Box::new(11)), Some(Box::new(22))];
        let before = pointers(&queues);
        let mut terminal = false;
        let payload = Box::new(91);
        let address = &*payload as *const _;
        let panic = catch_unwind(AssertUnwindSafe(|| {
            settle_xgmi_queue_retirement(
                &mut queues,
                &mut terminal,
                direction,
                |queue| -> Result<(), ()> {
                    **queue = 33;
                    panic_any(payload);
                },
            )
        }))
        .unwrap_err();
        assert!(terminal);
        assert_eq!(pointers(&queues), before);
        assert_eq!(**queues[direction].as_ref().unwrap(), 33);
        assert_eq!(
            &**panic.downcast_ref::<Box<i32>>().unwrap() as *const _,
            address
        );
    }
}

#[test]
fn native_xgmi_retirement_reverse_prefix_preserves_failed_direction() {
    for failing in 0..2 {
        let mut queues = [Some(Box::new(11)), Some(Box::new(22))];
        let before = pointers(&queues);
        let mut terminal = false;
        let mut calls = Vec::new();
        for direction in (0..2).rev() {
            if settle_xgmi_queue_retirement(&mut queues, &mut terminal, direction, |_| {
                calls.push(direction);
                if direction == failing {
                    Err(())
                } else {
                    Ok(())
                }
            })
            .is_err()
            {
                break;
            }
        }
        assert!(terminal);
        assert_eq!(calls, if failing == 0 { vec![1, 0] } else { vec![1] });
        assert_eq!(pointers(&queues)[failing], before[failing]);
        if failing == 0 {
            assert!(queues[1].is_none());
        } else {
            assert_eq!(pointers(&queues)[0], before[0]);
        }
    }
}

#[test]
fn native_xgmi_retirement_runtime_wiring_borrows_slots_and_guards_terminal_roots() {
    let source = include_str!("../../kfd_backend.rs");
    let implementation = source
        .split("impl KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap();
    let require = implementation
        .split("    fn require_live(")
        .nth(1)
        .unwrap()
        .split("    fn next_id(")
        .next()
        .unwrap();
    assert!(require.contains("self.require_healthy_xgmi_v1()?"));
    let healthy = implementation
        .split("    fn require_healthy_xgmi_v1(")
        .nth(1)
        .unwrap()
        .split("    fn require_live(")
        .next()
        .unwrap();
    assert!(healthy.contains("Gfx942NativeXgmiSdmaQueueV1::has_terminal_retirement_v1"));
    let shutdown = implementation
        .split("    pub fn shutdown_native_v1(")
        .nth(1)
        .unwrap()
        .split("impl RuntimeBackendV1")
        .next()
        .unwrap();
    let drop = source
        .split("impl Drop for KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("impl RuntimeBackendV1")
        .next()
        .unwrap();
    assert!(
        shutdown.find("self.require_live()?").unwrap()
            < shutdown.find("settle_xgmi_queue_retirement").unwrap()
    );
    assert!(
        drop.find("has_terminal_retirement_v1").unwrap()
            < drop.find("settle_xgmi_queue_retirement").unwrap()
    );
    assert!(
        drop.find("std::panic::catch_unwind").unwrap()
            < drop.find("settle_xgmi_queue_retirement").unwrap()
    );
    assert!(
        drop.contains("if !matches!(result, Ok(Ok(()))) {\n                std::process::abort();")
    );
    for body in [shutdown, drop] {
        assert!(body.contains("for direction in (0..2).rev()"));
        assert!(body.contains("&mut self.queues"));
        assert!(body.contains("&mut self.terminal"));
        assert!(body.contains("queue.destroy_and_release(source, destination)"));
        assert!(!body.contains(".take()"));
    }
    let diagnostic = include_str!("../xgmi_diagnostic.rs");
    assert!(diagnostic.contains("Gfx942NativeXgmiSdmaQueueV1::has_terminal_retirement_v1"));
}
