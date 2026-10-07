use super::*;
use crate::shared_memory::{
    MutableGpuGttProfileV1, QueueConstructionFaultV1 as Fault,
    QueueConstructionMemoryFixtureV1 as Memory,
};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Debug)]
struct DropProbe(Rc<Cell<usize>>);

#[test]
fn constructor_error_keeps_public_auto_traits_without_native_custody() {
    fn error_traits<T: Send + Sync + 'static>() {}
    error_traits::<ComputeAqlQueueSessionErrorV1>();
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn rooted_settlement_preserves_original_owner_and_first_panic_through_cleanup() {
    for entered in [false, true] {
        for work_panics in [false, true] {
            for cleanup_panics in [false, true] {
                let drops = Rc::new(Cell::new(0));
                let root = Box::new(DropProbe(drops.clone()));
                let original = &*root as *const DropProbe;
                let retained = RefCell::new(None);
                let trace = RefCell::new(Vec::new());
                let result = catch_unwind(AssertUnwindSafe(|| {
                    run_rooted_construction_with_v1(
                        root,
                        |_, entry| {
                            if entered {
                                entry.enter("USERPTR queue-control creation");
                            }
                            trace.borrow_mut().push("work");
                            if work_panics {
                                std::panic::panic_any("original work panic");
                            }
                            Err(ComputeAqlQueueSessionErrorV1::Contract("work rejection"))
                        },
                        |_| {
                            trace.borrow_mut().push("cleanup");
                            assert_eq!(drops.get(), 0);
                            if cleanup_panics {
                                std::panic::panic_any("cleanup panic");
                            }
                        },
                        &|| trace.borrow_mut().push("poison"),
                        |root| {
                            trace.borrow_mut().push("retain");
                            *retained.borrow_mut() = Some(root);
                        },
                    )
                }));
                match result {
                    Err(payload) => assert_eq!(
                        payload.downcast_ref::<&str>(),
                        Some(&if work_panics {
                            "original work panic"
                        } else {
                            "cleanup panic"
                        })
                    ),
                    Ok(Err(error)) => {
                        assert!(!work_panics && !cleanup_panics);
                        assert_eq!(error.is_terminal_creation(), entered);
                    }
                    Ok(Ok(_)) => panic!("injected rejection returned success"),
                }
                let owner = retained.into_inner().expect("retained terminal allocation");
                assert_eq!(&*owner as *const DropProbe, original);
                assert_eq!(drops.get(), 0);
                assert_eq!(
                    *trace.borrow(),
                    if entered {
                        vec!["work", "poison", "cleanup", "retain"]
                    } else {
                        vec!["work", "cleanup", "retain"]
                    }
                );
                drop(owner);
                assert_eq!(drops.get(), 1);
            }
        }
    }
}

#[test]
fn rooted_settlement_does_not_destroy_secondary_panicking_payload() {
    struct PanicOnDrop(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
            panic!("secondary destructor must not run");
        }
    }
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let retained = RefCell::new(None);
    let root = Box::new(Rc::new(Cell::new(41)));
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_rooted_construction_with_v1(
            root,
            |_, entry| {
                entry.enter("USERPTR queue-control creation");
                std::panic::panic_any("first");
            },
            |_| std::panic::panic_any(PanicOnDrop(count.clone())),
            &|| {},
            |root| *retained.borrow_mut() = Some(root),
        )
    }));
    assert_eq!(result.err().unwrap().downcast_ref::<&str>(), Some(&"first"));
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert_eq!(retained.borrow().as_ref().unwrap().get(), 41);
}

#[test]
fn rooted_success_transfers_non_send_borrowed_preparation_without_cleanup() {
    let text = String::from("borrowed preparation");
    let drops = Rc::new(Cell::new(0));
    let root = Box::new((None, DropProbe(drops.clone())));
    let original = &*root as *const _;
    let mut root = run_rooted_construction_with_v1(
        root,
        |root, entry| {
            root.0 = Some((text.as_str(), Rc::new(41)));
            entry.enter("USERPTR queue-control creation");
            Ok(())
        },
        |_| panic!("successful cleanup"),
        &|| panic!("successful poison"),
        |_| panic!("successful terminal retention"),
    )
    .unwrap();
    assert_eq!(&*root as *const _, original);
    let output = root.0.take().unwrap();
    assert_eq!(output.0.as_ptr(), text.as_ptr());
    assert_eq!(*output.1, 41);
    assert!(root.0.take().is_none());
    assert_eq!(drops.get(), 0);
    drop(root);
    assert_eq!(drops.get(), 1);
}

fn mutable_failure<P: MutableGpuGttProfileV1>(fault: Fault, mapped_successor: bool) {
    let mut memory = Memory::new();
    let device = memory.charged_device();
    let completion = memory.allocate::<HostVisibleCoherentGttV1>(COMPLETION_SIGNAL_ARENA_BYTES_V1);
    let token = memory.allocate::<P>(4096);
    let identity = token.storage_identity();
    let layout = token.layout();
    let usage = (memory.host_usage(), memory.device_usage());
    memory.arm(fault);
    let prefix = MutablePrefixV1::<P, ()> {
        cpu: Some(token),
        mapped: None,
        retained: None,
        in_session: None,
    };
    let text = String::from("actual returned preparation");
    let root = Box::new((
        memory,
        prefix,
        device,
        completion,
        Some((text.as_str(), Rc::new(42))),
    ));
    let original = &*root as *const _;
    let retained = RefCell::new(None);
    let trace = RefCell::new(Vec::new());
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_rooted_construction_with_v1(
            root,
            |root, entry| {
                entry.enter("USERPTR queue-control creation");
                let p = &mut root.1;
                advance_token_v1(
                    &mut root.0,
                    &mut p.cpu,
                    &mut p.mapped,
                    &mut p.in_session,
                    |memory, token| {
                        memory.preflight_cpu(token)?;
                        Ok(token.storage_identity())
                    },
                    Memory::map,
                )?;
                panic!("injected transition succeeded");
            },
            |_| trace.borrow_mut().push("cleanup"),
            &|| trace.borrow_mut().push("poison"),
            |root| {
                trace.borrow_mut().push("retain");
                *retained.borrow_mut() = Some(root);
            },
        )
    }));
    match result {
        Err(payload) => Memory::assert_panic(fault, &*payload),
        Ok(Err(error)) => assert!(error.is_terminal_creation()),
        Ok(Ok(_)) => panic!("transition unexpectedly succeeded"),
    }
    let owner = retained.into_inner().unwrap();
    assert_eq!(&*owner as *const _, original);
    assert!(owner.1.cpu.is_none() && owner.1.mapped.is_none());
    assert_eq!(owner.1.in_session, Some(identity));
    owner
        .0
        .assert_terminal::<P>(identity, layout, mapped_successor);
    assert_eq!(
        owner.0.terminal_native_progress(),
        match fault {
            Fault::MapError => (true, Some(false), Some(1)),
            Fault::MapPanic => (true, None, None),
            Fault::CurrentnessError(1) | Fault::CurrentnessPanic(1) => (false, None, None),
            _ => (true, Some(true), Some(1)),
        }
    );
    assert_eq!((owner.0.host_usage(), owner.0.device_usage()), usage);
    assert_eq!(owner.4.as_ref().unwrap().0.as_ptr(), text.as_ptr());
    assert_eq!(*owner.4.as_ref().unwrap().1, 42);
    assert_eq!(*trace.borrow(), ["poison", "cleanup", "retain"]);
}

#[test]
fn captured_preparation_survives_later_rejection_and_panic_without_reinvocation() {
    for panics in [false, true] {
        let text = String::from("borrowed prepared value");
        let drops = Rc::new(Cell::new(0));
        let root = Box::new((Memory::new(), None));
        let original = &*root as *const _;
        let retained = RefCell::new(None);
        let identity = Cell::new(None);
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_rooted_construction_with_v1(
                root,
                |root, _| {
                    capture_returned_preparation_v1(&mut root.0, &mut root.1, |memory| {
                        let token = memory.allocate::<HostVisibleCoherentGttV1>(4096);
                        identity.set(Some(token.storage_identity()));
                        Ok((token, text.as_str(), DropProbe(drops.clone())))
                    })?;
                    assert!(
                        capture_returned_preparation_v1(&mut root.0, &mut root.1, |_| panic!(
                            "occupied preparation callback"
                        ))
                        .is_err()
                    );
                    if panics {
                        std::panic::panic_any("later failure");
                    }
                    Err(ComputeAqlQueueSessionErrorV1::Contract("later failure"))
                },
                |_| {},
                &|| panic!("pre-control poison"),
                |root| *retained.borrow_mut() = Some(root),
            )
        }));
        match result {
            Err(payload) => {
                assert!(panics);
                assert_eq!(payload.downcast_ref::<&str>(), Some(&"later failure"));
            }
            Ok(Err(error)) => assert!(!panics && !error.is_terminal_creation()),
            Ok(Ok(_)) => panic!("later failure succeeded"),
        }
        let owner = retained.into_inner().unwrap();
        assert_eq!(&*owner as *const _, original);
        let output = owner.1.as_ref().unwrap();
        assert_eq!(Some(output.0.storage_identity()), identity.get());
        assert_eq!(output.1.as_ptr(), text.as_ptr());
        assert_eq!(drops.get(), 0);
        assert_eq!(&owner.0.calls()[5..], &[0, 0, 0]);
        drop(owner);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn rooted_control_and_completion_handoffs_retain_actual_r88_tokens_and_accounts() {
    for (fault, successor) in [
        (Fault::MapError, false),
        (Fault::MapPanic, false),
        (Fault::CurrentnessError(1), false),
        (Fault::CurrentnessPanic(1), false),
        (Fault::CurrentnessError(2), false),
        (Fault::CurrentnessPanic(2), false),
        (Fault::ProjectionError, true),
        (Fault::ProjectionPanic, true),
        (Fault::CommitError, true),
        (Fault::CommitPanic, true),
    ] {
        mutable_failure::<UserptrAqlControlGttV1>(fault, successor);
        mutable_failure::<HostVisibleCoherentGttV1>(fault, successor);
    }
}

#[test]
fn construction_token_preflight_preserves_foreign_closed_and_occupied_inputs() {
    for fault in 0..4 {
        let mut memory = Memory::new();
        let mut foreign = Memory::new();
        let token = if fault == 0 {
            foreign.allocate::<UserptrAqlControlGttV1>(4096)
        } else {
            memory.allocate::<UserptrAqlControlGttV1>(4096)
        };
        let identity = token.storage_identity();
        let mut input = Some(token);
        let mut output = if fault == 2 {
            let token = memory.allocate::<UserptrAqlControlGttV1>(4096);
            Some(memory.map(token).unwrap())
        } else {
            None
        };
        let mut pending = if fault == 3 { Some(identity) } else { None };
        if fault == 1 {
            memory.close();
        }
        let before = memory.calls();
        assert!(
            advance_token_v1(
                &mut memory,
                &mut input,
                &mut output,
                &mut pending,
                |memory, token| {
                    memory.preflight_cpu(token)?;
                    Ok(token.storage_identity())
                },
                Memory::map
            )
            .is_err()
        );
        assert_eq!(input.as_ref().unwrap().storage_identity(), identity);
        assert_eq!(memory.calls(), before);
        assert_eq!(output.is_some(), fault == 2);
        assert_eq!(pending.is_some(), fault == 3);
    }
}

#[test]
fn construction_token_success_installs_exact_output_and_clears_marker_once() {
    let mut memory = Memory::new();
    let token = memory.allocate::<HostVisibleCoherentGttV1>(4096);
    let identity = token.storage_identity();
    let mut input = Some(token);
    let mut output = None;
    let mut pending = None;
    advance_token_v1(
        &mut memory,
        &mut input,
        &mut output,
        &mut pending,
        |memory, token| {
            memory.preflight_cpu(token)?;
            Ok(token.storage_identity())
        },
        Memory::map,
    )
    .unwrap();
    assert!(input.is_none() && pending.is_none());
    assert_eq!(output.as_ref().unwrap().storage_identity(), identity);
    let calls = memory.calls();
    assert!(
        advance_token_v1(
            &mut memory,
            &mut input,
            &mut output,
            &mut pending,
            |memory, token| {
                memory.preflight_cpu(token)?;
                Ok(token.storage_identity())
            },
            Memory::map
        )
        .is_err()
    );
    assert_eq!(memory.calls(), calls);
}
