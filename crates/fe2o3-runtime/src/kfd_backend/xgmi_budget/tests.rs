use super::*;
use std::os::unix::process::ExitStatusExt;

#[test]
fn compound_backing_xgmi_admits_both_move_only_endpoints_before_vm_acquisition() {
    use std::{cell::RefCell, rc::Rc};
    struct Admission(u64, Rc<RefCell<Vec<u64>>>);
    impl Drop for Admission {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    for failure in ["none", "second_admit", "first_acquire", "admit_panic"] {
        let dropped = Rc::new(RefCell::new(Vec::new()));
        let calls = RefCell::new(Vec::new());
        let result = catch_unwind(AssertUnwindSafe(|| {
            let admissions =
                admit_endpoints([&19, &7], [Box::new(11), Box::new(22)], |device, budget| {
                    calls.borrow_mut().push(("admit", *device, *budget));
                    if *device == 7 && failure == "second_admit" {
                        return Err("admission");
                    }
                    if *device == 7 && failure == "admit_panic" {
                        std::panic::panic_any("admission panic");
                    }
                    Ok(Admission(*device, dropped.clone()))
                })?;
            acquire_sessions([19, 7], admissions, |device, admission| {
                assert_eq!(device, admission.0);
                calls.borrow_mut().push(("acquire", device, 0));
                if failure == "first_acquire" {
                    return Err("acquisition");
                }
                Ok(admission)
            })
            .map(drop)
        }));
        let calls = calls.into_inner();
        assert_eq!(&calls[..2], &[("admit", 19, 11), ("admit", 7, 22)]);
        match failure {
            "none" => {
                assert_eq!(result.unwrap(), Ok(()));
                assert_eq!(&calls[2..], &[("acquire", 19, 0), ("acquire", 7, 0)]);
            }
            "second_admit" => {
                assert_eq!(result.unwrap(), Err("admission"));
                assert_eq!(calls.len(), 2);
            }
            "first_acquire" => {
                assert_eq!(result.unwrap(), Err("acquisition"));
                assert_eq!(&calls[2..], &[("acquire", 19, 0)]);
            }
            "admit_panic" => {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"admission panic")
                );
                assert_eq!(calls.len(), 2);
            }
            _ => unreachable!(),
        }
        let mut dropped = dropped.borrow().clone();
        dropped.sort_unstable();
        assert_eq!(
            dropped,
            if matches!(failure, "second_admit" | "admit_panic") {
                vec![19]
            } else {
                vec![7, 19]
            }
        );
    }
}

#[test]
fn xgmi_budget_sessions_preserve_argument_order_and_independent_limits() {
    let budgets = [
        KfdNativeXgmiBackingBudgetV1 {
            device: Some(Gfx942DeviceBackingBudgetV1::new(4096, 1).unwrap()),
            host_visible: None,
        },
        KfdNativeXgmiBackingBudgetV1 {
            device: Some(Gfx942DeviceBackingBudgetV1::new(8192, 2).unwrap()),
            host_visible: Some(Gfx942HostVisibleBackingBudgetV1::new(4096, 1).unwrap()),
        },
    ];
    for budgets in [
        budgets,
        [budgets[1], budgets[0]],
        [KfdNativeXgmiBackingBudgetV1::default(); 2],
    ] {
        let mut calls = Vec::new();
        let sessions = acquire_sessions([19, 7], budgets, |device, budget| {
            calls.push((device, budget));
            Ok::<_, ()>(Box::new(device))
        })
        .unwrap();
        assert_eq!(calls, [(19, budgets[0]), (7, budgets[1])]);
        assert_eq!(sessions.map(|owner| *owner), [19, 7]);
    }
}

#[test]
fn xgmi_budget_first_acquisition_error_never_opens_peer() {
    let mut calls = 0;
    let mut error = Some(Box::new(73));
    let pointer = &**error.as_ref().unwrap() as *const _;
    let result = acquire_sessions(
        [19, 7],
        [KfdNativeXgmiBackingBudgetV1::default(); 2],
        |_, _| {
            calls += 1;
            Err::<(), _>(error.take().unwrap())
        },
    );
    assert_eq!(calls, 1);
    let returned = result.unwrap_err();
    assert_eq!(*returned, 73);
    assert_eq!(&*returned as *const _, pointer);
}

#[test]
fn xgmi_budget_second_acquisition_failure_cannot_drop_first_session() {
    const CHILD: &str = "FE2O3_XGMI_BUDGET_ACQUISITION_CHILD";
    const TEST: &str = "kfd_backend::xgmi_budget::tests::xgmi_budget_second_acquisition_failure_cannot_drop_first_session";
    if let Ok(mode) = std::env::var(CHILD) {
        struct Session;
        impl Drop for Session {
            fn drop(&mut self) {
                eprintln!("unexpected first session Drop");
            }
        }
        let _ = acquire_sessions(
            [19, 7],
            [KfdNativeXgmiBackingBudgetV1::default(); 2],
            |device, _| {
                if device == 19 {
                    return Ok(Session);
                }
                eprintln!("second acquisition reached");
                if mode == "panic" {
                    std::panic::panic_any("second acquisition panic");
                }
                Err(())
            },
        );
        panic!("second acquisition returned");
    }
    for mode in ["error", "panic"] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, mode)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "{stderr}");
        assert!(stderr.contains("second acquisition reached"));
        assert!(!stderr.contains("unexpected first session Drop"));
        assert!(!stderr.contains("second acquisition returned"));
    }
}

#[test]
fn xgmi_budget_allocation_disposition_preserves_rejection_and_terminal_policy() {
    for disposition in [
        Gfx942XgmiAllocationDispositionV1::RejectedCapacity,
        Gfx942XgmiAllocationDispositionV1::ProcessTeardown,
    ] {
        let mut terminal = false;
        let result = allocate::<(), _>(&mut terminal, || Err("original error"), |_| disposition);
        let error = match result.unwrap_err() {
            RuntimeBackendFailureV1::Rejected(error) => {
                assert_eq!(
                    disposition,
                    Gfx942XgmiAllocationDispositionV1::RejectedCapacity
                );
                assert!(!terminal);
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
                error
            }
            RuntimeBackendFailureV1::Terminal(error) => {
                assert_eq!(
                    disposition,
                    Gfx942XgmiAllocationDispositionV1::ProcessTeardown
                );
                assert!(terminal);
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Terminal);
                error
            }
            _ => panic!("unexpected settlement"),
        };
        assert!(error.to_string().contains("original error"));
    }
    let mut terminal = false;
    let owner = Box::new(9);
    let pointer = &*owner as *const _;
    let returned = allocate(
        &mut terminal,
        || Ok::<_, &str>(owner),
        |_| panic!("success classified"),
    )
    .unwrap();
    assert_eq!(&*returned as *const _, pointer);
    assert!(!terminal);
}

#[test]
fn xgmi_budget_allocation_and_diagnostic_panics_latch_terminal() {
    struct Error(bool);
    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if self.0 {
                std::panic::panic_any("diagnostic panic");
            }
            f.write_str("error")
        }
    }
    for stage in ["allocation", "classifier", "diagnostic"] {
        for disposition in [
            Gfx942XgmiAllocationDispositionV1::RejectedCapacity,
            Gfx942XgmiAllocationDispositionV1::ProcessTeardown,
        ] {
            let mut terminal = false;
            let payload = catch_unwind(AssertUnwindSafe(|| {
                allocate::<(), _>(
                    &mut terminal,
                    || {
                        if stage == "allocation" {
                            std::panic::panic_any("allocation panic");
                        }
                        Err(Error(stage == "diagnostic"))
                    },
                    |_| {
                        if stage == "classifier" {
                            std::panic::panic_any("classifier panic");
                        }
                        disposition
                    },
                )
            }))
            .unwrap_err();
            assert!(terminal);
            let expected = match stage {
                "allocation" => "allocation panic",
                "classifier" => "classifier panic",
                _ => "diagnostic panic",
            };
            assert_eq!(payload.downcast_ref::<&str>(), Some(&expected));
        }
    }
}

#[test]
fn xgmi_budget_queue_completion_capacity_keeps_creation_root_terminal() {
    for direction in 0..2 {
        let mut roots = [None, None];
        let mut queues = [None, None];
        let earlier_ring_control = Box::new(31);
        let pointer = &*earlier_ring_control as *const _;
        queues[1 - direction] = Some(Box::new(47));
        let peer_pointer = &**queues[1 - direction].as_ref().unwrap() as *const _;
        let mut terminal = false;
        let result =
            settle_xgmi_queue_creation(&mut roots, &mut queues, &mut terminal, direction, |root| {
                *root = Some(earlier_ring_control);
                Err(fe2o3_kfd::MemorySessionError::HostVisibleBackingCredits(
                    fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity,
                ))
            });
        assert!(matches!(
            result,
            Err(fe2o3_kfd::MemorySessionError::HostVisibleBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            ))
        ));
        assert!(terminal);
        assert_eq!(&**roots[direction].as_ref().unwrap() as *const _, pointer);
        assert!(roots[1 - direction].is_none());
        assert!(queues[direction].is_none());
        assert_eq!(
            &**queues[1 - direction].as_ref().unwrap() as *const _,
            peer_pointer
        );
    }
}

#[test]
fn xgmi_budget_production_wiring_preserves_defaults_order_and_classified_entry() {
    let source = include_str!("../../kfd_backend.rs");
    let constructor = source
        .split("impl KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("    fn rejected(")
        .next()
        .unwrap();
    assert_eq!(
        constructor
            .matches("[KfdNativeXgmiBackingBudgetV1::default(); 2]")
            .count(),
        2
    );
    assert!(constructor.contains("Self::open_default_with_backing_budgets_v1("));
    assert!(
        constructor
            .contains("Self::from_checked_pair_with_backing_budgets_v1(first, second, budgets)")
    );
    let compact = constructor.split_whitespace().collect::<String>();
    let prepare = compact
        .find("letadmissions=prepare([&first,&second])?")
        .unwrap();
    let acquire = compact
        .find("xgmi_budget::acquire_sessions([first,second],admissions,")
        .unwrap();
    assert!(prepare < acquire);
    assert!(constructor.contains("xgmi_budget::admit_endpoints("));
    assert!(
        constructor.contains(
            ".acquire_shared_gtt_memory_session_with_rooted_native_backing_v1(admission)"
        )
    );
    assert!(compact.contains("device.acquire_shared_gtt_memory_session_with_backing_budgets_v1("));
    assert!(constructor.contains("budget.device,"));
    assert!(constructor.contains("budget.host_visible,"));
    assert!(!constructor.contains(".acquire_shared_gtt_memory_session()"));
    let implementation = source
        .split("impl RuntimeBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap();
    let allocation = implementation
        .split("    fn allocate_v1(")
        .nth(1)
        .unwrap()
        .split("    fn release_allocation_v1(")
        .next()
        .unwrap();
    assert!(allocation.contains("xgmi_budget::allocate("));
    assert!(allocation.contains("allocate_gfx942_xgmi_device_memory_classified_v1("));
    assert!(allocation.contains("fe2o3_kfd::Gfx942XgmiAllocationFailureV1::disposition"));
    assert!(!allocation.contains(".allocate_gfx942_xgmi_device_memory("));
    let queue = source
        .split("    fn ensure_queue(")
        .nth(1)
        .unwrap()
        .split("    fn restore_unmapped(")
        .next()
        .unwrap();
    assert!(queue.contains("settle_xgmi_queue_creation("));
    assert!(queue.contains("&mut self.queue_creation_roots"));
    assert!(
        queue.contains("Gfx942NativeXgmiSdmaQueueV1::create(source, destination, route, root)")
    );
    let usage = include_str!("../xgmi_budget.rs")
        .split("    pub fn backing_usage_v1(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn acquire_sessions")
        .next()
        .unwrap();
    assert!(usage.contains("backend_device: self.descriptions[index].backend_device"));
    assert!(usage.contains("device: self.sessions[index].device_backing_usage_v1()"));
    assert!(usage.contains("host_visible: self.sessions[index].host_visible_backing_usage_v1()"));
    assert!(!usage.contains("require_live"));
    let lower = include_str!("../../../../fe2o3-kfd/src/shared_memory.rs");
    let allocation = lower
        .split("    pub fn allocate_gfx942_xgmi_device_memory(")
        .nth(1)
        .unwrap()
        .split("    /// Writes the complete logical extent")
        .next()
        .unwrap();
    assert!(allocation.contains("self.allocate_gfx942_xgmi_device_memory_classified_v1("));
    assert!(allocation.contains("self.engine.allocate_xgmi_device_memory_classified_v1("));
}
