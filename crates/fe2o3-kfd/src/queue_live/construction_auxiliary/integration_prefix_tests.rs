use super::*;

#[path = "integration_memory_tests.rs"]
mod memory_cases;

type PrefixCaseResult = (
    Box<Scope>,
    Result<(), Box<dyn std::any::Any + Send>>,
    Rc<RefCell<Trace>>,
);

fn prefix_case(configure: impl FnOnce(&mut Scope, &Rc<RefCell<Trace>>)) -> PrefixCaseResult {
    let (memory, trace) = setup_memory_with_host_budget(2 << 20);
    let (primary, trace) = setup_with_memory(memory, trace);
    let primary_address = &*primary as *const Root as usize;
    let (mut primary, result) = run(primary, QueueRingBackingV1::AqlSpecial, false);
    assert!(result.is_ok(), "primary fixture prerequisite");
    assert_root(&primary, primary_address, &trace);
    let complete = primary.completed.as_mut().unwrap();
    complete.completion_owner.bind_barrier_probe().unwrap();
    complete
        .dependency_owner
        .reserve_acceptance_epoch()
        .unwrap();
    let completion = complete.completion_owner.custody_snapshot_for_test();
    let dependency = complete.dependency_owner.custody_snapshot_for_test();
    let engine_address = &complete.engine as *const _ as usize;
    let before = complete.engine.backend.session.observation();
    let platform = platform::platform_identities(&primary, None);
    let call_prefix = trace.borrow().calls.clone();
    let (programs, packets) = recipe();
    let preparation = PrimaryPreparationSnapshotV1::packets(&packets);
    let mut scope = Box::new(Scope {
        parent: Parent {
            original: Some(Original {
                primary,
                lanes: Vec::with_capacity(1),
                data: Rc::new(RefCell::new(None)),
                preparation: Rc::new(RefCell::new(preparation)),
            }),
            poisoned: false,
            faults: Faults::default(),
        },
        construction: AuxiliaryConstructionV1::new(packets),
        terminal_parent: None,
    });
    configure(&mut scope, &trace);
    let primary = scope.primary.completed.as_ref().unwrap();
    let before_loan = primary
        .engine
        .backend
        .session
        .primary_loan_state_v1(&primary.engine.foundation);
    let scope_address = &*scope as *const Scope as usize;
    let (scope, result) = run_auxiliary(scope, programs);
    assert_eq!(&*scope as *const Scope as usize, scope_address);
    assert_eq!(&*scope.primary as *const Root as usize, primary_address);
    let primary = scope.primary.completed.as_ref().unwrap();
    assert_eq!(&primary.engine as *const _ as usize, engine_address);
    assert_eq!(
        primary.completion_owner.custody_snapshot_for_test(),
        completion
    );
    assert_eq!(
        primary.dependency_owner.custody_snapshot_for_test(),
        dependency
    );
    assert_eq!(
        platform::platform_identities(&scope.primary, None),
        platform
    );
    assert_eq!(&trace.borrow().calls[..call_prefix.len()], call_prefix);
    primary
        .engine
        .backend
        .session
        .assert_original_records_unchanged(&before);
    assert_parent_transport(&scope, result.is_err());
    let after_loan = primary
        .engine
        .backend
        .session
        .primary_loan_state_v1(&primary.engine.foundation);
    let loaned = trace.borrow().calls.contains(&"auxiliary-loan-complete");
    let reclaimed = trace.borrow().calls.contains(&"auxiliary-retake-complete");
    assert_eq!(after_loan.0, before_loan.0, "original foundation issuer");
    assert_eq!(
        after_loan.1,
        (loaned && !reclaimed).then_some(before_loan.2)
    );
    assert_eq!(
        after_loan.2,
        if loaned {
            before_loan.2.checked_add(1).unwrap()
        } else {
            before_loan.2
        }
    );
    assert_eq!(
        primary.engine.backend.foundation_in_engine,
        after_loan.1.is_none()
    );
    if result.is_err() {
        assert_early_pair(&scope);
        let t = trace.borrow();
        let retained = t
            .calls
            .iter()
            .position(|&c| c == "auxiliary-parent-retain")
            .unwrap();
        if let Some(cleanup) = t.calls.iter().position(|&c| c == "cleanup") {
            assert!(retained < cleanup);
        }
        assert_eq!(
            t.cleanup,
            usize::from(scope.construction.unpublished.is_some() && t.cleanup_panic != Some(false))
        );
    } else {
        assert_pair(&scope);
    }
    (scope, result, trace)
}

fn error_source(payload: &(dyn std::any::Any + Send)) -> &ComputeAqlQueueSessionErrorV1 {
    let error = payload
        .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
        .expect("typed queue error");
    match error {
        ComputeAqlQueueSessionErrorV1::TerminalCreation { source, .. } => source,
        _ => error,
    }
}

#[test]
fn same_engine_auxiliary_operation_reclaim_matrix_retains_actual_foundation_and_parent() {
    for operation in [Outcome::Success, Outcome::Error, Outcome::Panic] {
        for (before, after) in [
            (Outcome::Success, Outcome::Success),
            (Outcome::Error, Outcome::Success),
            (Outcome::Panic, Outcome::Success),
            (Outcome::Success, Outcome::Error),
            (Outcome::Success, Outcome::Panic),
        ] {
            let (scope, result, trace) = prefix_case(|scope, _| {
                scope.parent.faults.operation = operation;
                scope.parent.faults.reclaim_before = before;
                scope.parent.faults.reclaim_after = after;
            });
            if operation == Outcome::Success
                && before == Outcome::Success
                && after == Outcome::Success
            {
                assert!(result.is_ok());
                continue;
            }
            let payload = result.expect_err("operation or reclaim must fail");
            let (name, panic) = if operation == Outcome::Panic {
                ("auxiliary-operation-return", true)
            } else if before != Outcome::Success {
                ("auxiliary-retake", before == Outcome::Panic)
            } else if after != Outcome::Success {
                ("auxiliary-retake-complete", after == Outcome::Panic)
            } else {
                ("auxiliary-operation-return", false)
            };
            if panic {
                assert_eq!(payload.downcast_ref::<(&str, usize)>(), Some(&(name, 1)));
            } else {
                assert!(
                    matches!(error_source(&*payload), ComputeAqlQueueSessionErrorV1::Contract(actual) if *actual == name)
                );
            }
            assert!(scope.construction.dispatch.is_some());
            assert_eq!(
                trace
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == "auxiliary-retake")
                    .count(),
                1
            );
        }
    }
}

#[test]
fn same_engine_auxiliary_opening_and_loan_failures_never_enter_preparation_or_reclaim() {
    for opening in [false, true] {
        for panic in [false, true] {
            let (scope, result, trace) = prefix_case(|scope, trace| {
                if opening {
                    let nth = trace
                        .borrow()
                        .calls
                        .iter()
                        .filter(|&&c| c == "currentness")
                        .count()
                        + 1;
                    trace.borrow_mut().fault = Some(("currentness", nth, panic));
                } else {
                    scope.parent.faults.loan = if panic {
                        Outcome::Panic
                    } else {
                        Outcome::Error
                    };
                }
            });
            let payload = result.expect_err("opening or loan must fail");
            let name = if opening {
                "currentness"
            } else {
                "auxiliary-loan"
            };
            if panic {
                let nth = trace.borrow().calls.iter().filter(|&&c| c == name).count();
                assert_eq!(payload.downcast_ref::<(&str, usize)>(), Some(&(name, nth)));
            } else if opening {
                assert!(matches!(
                    error_source(&*payload),
                    ComputeAqlQueueSessionErrorV1::Native("queue currentness lost")
                ));
            } else {
                assert!(matches!(
                    error_source(&*payload),
                    ComputeAqlQueueSessionErrorV1::Contract("auxiliary-loan")
                ));
            }
            assert!(scope.data.borrow().is_none());
            assert!(scope.construction.preparation.is_none());
            for name in [
                "auxiliary-loan-complete",
                "plan-auxiliary-resources",
                "auxiliary-retake",
            ] {
                assert!(!trace.borrow().calls.contains(&name));
            }
        }
    }
}

#[test]
fn same_engine_auxiliary_real_loan_exhaustion_and_reclaim_revision_rejection_preserve_custody() {
    for reclaim in [false, true] {
        let (scope, result, trace) = prefix_case(|scope, _| {
            if reclaim {
                scope.parent.faults.regress_revision = true;
            } else {
                scope
                    .parent
                    .original
                    .as_mut()
                    .unwrap()
                    .primary
                    .completed
                    .as_mut()
                    .unwrap()
                    .engine
                    .backend
                    .session
                    .primary_expire_loan_generation_v1();
            }
        });
        let payload = result.expect_err("actual ownership/certificate validation must reject");
        assert!(
            matches!(error_source(&*payload), ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(actual))
            if *actual == if reclaim { "fixture live foundation reclaim" } else { "fixture live foundation loan" })
        );
        assert_eq!(scope.construction.dispatch.is_some(), reclaim);
        assert_eq!(trace.borrow().calls.contains(&"auxiliary-retake"), reclaim);
        assert!(!trace.borrow().calls.contains(&"auxiliary-retake-complete"));
    }
}

#[test]
fn same_engine_auxiliary_preparation_stage_failures_retain_every_returned_prefix() {
    let mut stages = vec![
        PreparationStageV1::Generation,
        PreparationStageV1::Plan,
        PreparationStageV1::Capacity,
        PreparationStageV1::DataRetention,
        PreparationStageV1::KernargAllocate,
        PreparationStageV1::KernargMaterialize,
        PreparationStageV1::KernargMap,
        PreparationStageV1::KernargRetain,
        PreparationStageV1::Commit,
        PreparationStageV1::Complete,
    ];
    for i in 0..3 {
        stages.extend([
            PreparationStageV1::CodeAllocate(i),
            PreparationStageV1::CodeMaterialize(i),
            PreparationStageV1::CodeSeal(i),
            PreparationStageV1::CodeMap(i),
            PreparationStageV1::CodeRetain(i),
            PreparationStageV1::CodeResolve(i),
            PreparationStageV1::PacketResolve(i),
        ]);
    }
    for stage in stages {
        for panic in [false, true] {
            let (scope, result, trace) = prefix_case(|scope, _| {
                scope.construction.preparation_fault = Some((stage, panic));
            });
            let payload = result.expect_err("preparation stage must fail");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, PreparationStageV1)>(),
                    Some(&("dispatch preparation", stage))
                );
            } else {
                assert!(matches!(
                    error_source(&*payload),
                    ComputeAqlQueueSessionErrorV1::DispatchBinding(
                        Gfx942DispatchBindingErrorV1::InvalidCode("injected preparation stage")
                    )
                ));
            }
            scope
                .construction
                .preparation
                .as_ref()
                .unwrap()
                .primary_assert_failed_stage_v1(stage);
            assert!(scope.construction.ring.is_none());
            assert_eq!(
                trace
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == "auxiliary-retake")
                    .count(),
                1
            );
            assert!(trace.borrow().calls.contains(&"auxiliary-retake-complete"));
        }
    }
}

#[test]
fn same_engine_auxiliary_cleanup_panic_preserves_first_panic_and_terminal_parent() {
    for after_disposal in [false, true] {
        for operation_panic in [false, true] {
            let (scope, result, trace) = prefix_case(|scope, trace| {
                scope.parent.faults.operation = if operation_panic {
                    Outcome::Panic
                } else {
                    Outcome::Error
                };
                trace.borrow_mut().cleanup_panic = Some(after_disposal);
            });
            let payload = result.expect_err("operation and cleanup fail");
            if operation_panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&("auxiliary-operation-return", 1))
                );
            } else {
                assert_eq!(
                    payload.downcast_ref::<(&str, bool)>(),
                    Some(&("auxiliary cleanup", after_disposal))
                );
            }
            assert!(scope.terminal_parent.is_some());
            assert_eq!(
                trace
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == "cleanup")
                    .count(),
                1
            );
        }
    }
}

#[test]
fn same_engine_auxiliary_control_and_platform_preflight_failures_keep_exact_returned_prefixes() {
    let (scope, result, successful) = prefix_case(|_, _| {});
    assert!(result.is_ok());
    let calls = successful.borrow().calls.clone();
    drop(scope);
    let start = calls
        .iter()
        .position(|&c| c == "auxiliary-loan-complete")
        .unwrap()
        + 1;
    let end = calls.iter().position(|&c| c == "auxiliary-retake").unwrap();
    let expected = [
        ("plan-auxiliary-resources", 1),
        ("allocate-ring", 1),
        ("allocate-control", 1),
        ("allocate-completion", 1),
        ("allocate-executable", 2),
        ("initialize-ring", 1),
        ("initialize-bytes", 4),
        ("currentness", 4),
        ("runtime-enable", 1),
        ("runtime-validate", 2),
        ("gate-arm", 1),
        ("event", 1),
        ("shadow-install", 1),
        ("shadow-init", 1),
        ("event-validate", 1),
        ("shadow-restore", 1),
        ("ring-preflight-cpu", 1),
        ("ring-preflight-mapped", 1),
        ("preflight-cpu", 4),
        ("preflight-mapped", 2),
        ("preflight-immutable", 2),
        ("preflight-executable", 2),
    ];
    for (name, count) in expected {
        assert_eq!(
            calls[start..end].iter().filter(|&&c| c == name).count(),
            count,
            "{name} occurrence coverage"
        );
    }
    let mut cases = 0;
    for index in start..end {
        let name = calls[index];
        if !expected.iter().any(|&(expected, _)| expected == name) {
            continue;
        }
        let nth = calls[..=index].iter().filter(|&&c| c == name).count();
        for panic in [false, true] {
            let (scope, result, trace) = prefix_case(|_, trace| {
                trace.borrow_mut().fault = Some((name, nth, panic));
            });
            let payload = result.expect_err("preflight/platform boundary must fail");
            if panic {
                assert_eq!(payload.downcast_ref::<(&str, usize)>(), Some(&(name, nth)));
            } else {
                match error_source(&*payload) {
                    ComputeAqlQueueSessionErrorV1::Contract(actual)
                    | ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(actual)) => {
                        assert_eq!(*actual, name)
                    }
                    error => panic!("wrong boundary error: {error:?}"),
                }
            }
            assert_eq!(&trace.borrow().calls[..=index], &calls[..=index]);
            assert!(trace.borrow().calls.contains(&"auxiliary-retake-complete"));
            for (owner, creation) in [
                (scope.construction.runtime.is_some(), "runtime-enable"),
                (scope.construction.creation_arm.is_some(), "gate-arm"),
                (scope.construction.event.is_some(), "event"),
                (scope.construction.unpublished.is_some(), "shadow-install"),
            ] {
                assert_eq!(
                    owner,
                    calls[start..index].contains(&creation),
                    "returned {creation} owner"
                );
            }
            cases += 1;
        }
    }
    assert_eq!(
        cases, 72,
        "every repeated preflight occurrence must be exercised"
    );
}

#[test]
fn same_engine_auxiliary_data_callback_failure_reclaims_without_returned_data_or_controls() {
    for panic in [false, true] {
        let (scope, result, trace) = prefix_case(|scope, _| {
            scope.parent.faults.prepare_data = if panic {
                Outcome::Panic
            } else {
                Outcome::Error
            };
        });
        let payload = result.expect_err("callback fails before allocating or returning owners");
        if panic {
            assert_eq!(
                payload.downcast_ref::<(&str, usize)>(),
                Some(&("auxiliary-data", 1))
            );
        } else {
            assert!(matches!(
                error_source(&*payload),
                ComputeAqlQueueSessionErrorV1::Contract("auxiliary-data")
            ));
        }
        assert!(scope.construction.data.is_none());
        assert!(scope.construction.preparation.is_none());
        assert!(scope.data.borrow().is_none());
        assert!(trace.borrow().calls.contains(&"auxiliary-retake-complete"));
    }
}

#[test]
fn same_engine_auxiliary_capacity_preflight_leaves_original_parent_and_native_calls_unchanged() {
    let (root, trace) = setup();
    let address = &*root as *const Root as usize;
    let (mut root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
    assert!(result.is_ok());
    let primary = root.completed.as_mut().unwrap();
    let engine = &mut primary.engine;
    // Populate model history, not native UPDATE calls, to reach the ingress bound.
    let mut configuration = 1_u8;
    while engine.preflight_operation().is_ok() {
        engine
            .begin(QueueTransitionV1::BeginUpdate {
                queue: primary.key,
                configuration: QueueConfigurationIdV1::from_untrusted_digest(
                    fe2o3_runtime_model::IdentityDigestV1::from_untrusted_bytes(
                        [configuration; 32],
                    ),
                ),
            })
            .unwrap();
        engine
            .observe(QueueTransitionV1::ObserveUpdate {
                queue: primary.key,
                status: QueueSyscallStatusV1::Succeeded,
            })
            .unwrap();
        configuration = configuration.checked_add(1).unwrap();
    }
    let before = engine.backend.session.observation();
    let journal = engine.journal_summary();
    let loan = engine
        .backend
        .session
        .primary_loan_state_v1(&engine.foundation);
    let calls = trace.borrow().calls.clone();
    let completion = primary.completion_owner.custody_snapshot_for_test();
    let dependency = primary.dependency_owner.custody_snapshot_for_test();
    for _ in 0..2 {
        assert_eq!(
            engine.preflight_operation(),
            Err(NativeQueueAdapterErrorV1::JournalCapacity)
        );
        assert_eq!(engine.journal_summary(), journal);
        assert_eq!(engine.backend.session.observation(), before);
        assert_eq!(
            engine
                .backend
                .session
                .primary_loan_state_v1(&engine.foundation),
            loan
        );
        assert_eq!(trace.borrow().calls, calls);
        assert!(!engine.authority_poisoned);
        assert_eq!(
            primary.completion_owner.custody_snapshot_for_test(),
            completion
        );
        assert_eq!(
            primary.dependency_owner.custody_snapshot_for_test(),
            dependency
        );
        assert!(!primary.completion_owner.is_poisoned_for_test());
        assert!(primary.dependency_owner.ensure_idle().is_ok());
    }
    assert_root(&root, address, &trace);
}
