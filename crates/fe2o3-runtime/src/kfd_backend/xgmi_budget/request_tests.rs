use super::*;
use std::{cell::RefCell, rc::Rc};

#[test]
fn xgmi_request_pair_binding_precedes_vm_and_reclaims_unused_owners() {
    struct Owner(u64, Rc<RefCell<Vec<u64>>>);
    impl Drop for Owner {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    for failure in ["none", "second_binding", "binding_panic", "first_vm"] {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let calls = RefCell::new(Vec::new());
        let result = catch_unwind(AssertUnwindSafe(|| {
            bind_before_acquire(
                [19, 7],
                [Owner(19, dropped.clone()), Owner(7, dropped.clone())],
                |devices, admissions| {
                    let bindings =
                        admit_endpoints(devices, admissions.each_ref(), |device, admission| {
                            calls.borrow_mut().push(("bind", *device));
                            assert_eq!(*device, admission.0);
                            if *device == 7 {
                                if failure == "second_binding" {
                                    return Err("binding");
                                }
                                if failure == "binding_panic" {
                                    std::panic::panic_any("binding panic");
                                }
                            }
                            Ok(Box::new(*device))
                        })?;
                    Ok(bindings)
                },
                |device, admission| {
                    calls.borrow_mut().push(("vm", device));
                    if failure == "first_vm" {
                        return Err("vm");
                    }
                    Ok(admission)
                },
            )
        }));
        match failure {
            "none" => {
                let (policy, sessions) = result.ok().unwrap().ok().unwrap();
                assert_eq!(policy.map(|entry| *entry), [19, 7]);
                assert_eq!(sessions.each_ref().map(|owner| owner.0), [19, 7]);
                assert_eq!(
                    *calls.borrow(),
                    [("bind", 19), ("bind", 7), ("vm", 19), ("vm", 7)]
                );
                drop(sessions);
            }
            "second_binding" => assert!(matches!(result, Ok(Err("binding")))),
            "first_vm" => assert!(matches!(result, Ok(Err("vm")))),
            _ => assert_eq!(
                result.err().unwrap().downcast_ref::<&str>(),
                Some(&"binding panic")
            ),
        }
        if failure.starts_with("binding") || failure == "second_binding" {
            assert_eq!(*calls.borrow(), [("bind", 19), ("bind", 7)]);
        }
        let mut dropped = dropped.borrow().clone();
        dropped.sort_unstable();
        assert_eq!(dropped, [7, 19]);
    }
}

#[test]
fn xgmi_request_record_success_roots_move_only_owner_without_replacing_neighbors() {
    let mut terminal = false;
    let mut next = 7;
    let mut records = HashMap::from([(3, Box::new(13))]);
    let neighbor = core::ptr::from_ref(records[&3].as_ref());
    let owner = Box::new(19);
    let pointer = core::ptr::from_ref(owner.as_ref());
    let id = allocate_record(
        &mut terminal,
        &mut next,
        &mut records,
        || Ok::<_, &str>(owner),
        |_| panic!("success classified"),
    )
    .unwrap();
    assert_eq!((id, next, terminal), (7, 8, false));
    assert_eq!(core::ptr::from_ref(records[&7].as_ref()), pointer);
    assert_eq!(core::ptr::from_ref(records[&3].as_ref()), neighbor);
}

#[test]
fn xgmi_request_record_preflight_never_enters_native_operation() {
    for mode in ["terminal", "zero", "occupied", "exhausted"] {
        let mut terminal = mode == "terminal";
        let mut next = match mode {
            "zero" => 0,
            "exhausted" => u64::MAX,
            _ => 7,
        };
        let mut records = HashMap::from([(7, Box::new(19))]);
        if mode == "terminal" {
            next = 8;
        }
        let pointer = core::ptr::from_ref(records[&7].as_ref());
        let before = next;
        let result = allocate_record(
            &mut terminal,
            &mut next,
            &mut records,
            || -> Result<Box<u64>, &str> { panic!("invalid state entered allocation") },
            |_| panic!("invalid state entered classifier"),
        );
        match result.unwrap_err() {
            RuntimeBackendFailureV1::Rejected(error) if mode == "exhausted" => {
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
                assert!(!terminal);
            }
            RuntimeBackendFailureV1::Terminal(_) if mode != "exhausted" => assert!(terminal),
            _ => panic!("wrong preflight classification"),
        }
        assert_eq!(next, before);
        assert_eq!(records.len(), 1);
        assert_eq!(core::ptr::from_ref(records[&7].as_ref()), pointer);
    }
}

#[test]
fn xgmi_request_record_capacity_burns_only_id_and_remains_retryable() {
    let mut terminal = false;
    let mut next = 7;
    let mut records = HashMap::new();
    assert!(
        matches!(allocate_record(&mut terminal, &mut next, &mut records,
        || Err::<Box<u64>, _>("capacity detail"), |_| Gfx942XgmiAllocationDispositionV1::RejectedCapacity),
        Err(RuntimeBackendFailureV1::Rejected(error)) if error.detail() == "native XGMI allocation: capacity detail")
    );
    assert!(!terminal && records.is_empty());
    assert_eq!(next, 8);
    assert_eq!(
        allocate_record(
            &mut terminal,
            &mut next,
            &mut records,
            || Ok::<_, &str>(Box::new(19)),
            |_| panic!("success classified")
        )
        .unwrap(),
        8
    );
    assert_eq!(next, 9);
    assert_eq!(*records[&8], 19);
}

#[test]
fn xgmi_request_record_terminal_and_panic_preserve_existing_custody() {
    for panic in [false, true] {
        let mut terminal = false;
        let mut next = 7;
        let mut records = HashMap::from([(3, Box::new(13))]);
        let pointer = core::ptr::from_ref(records[&3].as_ref());
        let payload = Box::new(927_u64);
        let payload_pointer = core::ptr::from_ref(payload.as_ref());
        let result = catch_unwind(AssertUnwindSafe(|| {
            allocate_record(
                &mut terminal,
                &mut next,
                &mut records,
                || {
                    if panic {
                        resume_unwind(payload);
                    }
                    Err("terminal detail")
                },
                |_| Gfx942XgmiAllocationDispositionV1::ProcessTeardown,
            )
        }));
        if panic {
            let payload = result.err().unwrap().downcast::<u64>().unwrap();
            assert_eq!(core::ptr::from_ref(payload.as_ref()), payload_pointer);
        } else {
            assert!(
                matches!(result, Ok(Err(RuntimeBackendFailureV1::Terminal(error)))
                if error.detail() == "native XGMI allocation: terminal detail")
            );
        }
        assert!(terminal);
        assert_eq!(next, 8);
        assert_eq!(records.len(), 1);
        assert_eq!(core::ptr::from_ref(records[&3].as_ref()), pointer);
        assert!(matches!(
            allocate_record(
                &mut terminal,
                &mut next,
                &mut records,
                || -> Result<Box<u64>, &str> { panic!("terminal retry entered allocation") },
                |_| panic!("terminal retry entered classifier")
            ),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
    }
}
