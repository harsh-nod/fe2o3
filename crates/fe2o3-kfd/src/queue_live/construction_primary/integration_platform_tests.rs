use super::*;

#[test]
fn local_linux_shadow_and_gate_helpers_compose_with_primary_custody_and_dispose_owned_mappings() {
    for external_runtime in [false, true] {
        for (boundary, panic) in [
            (None, false),
            (Some("shadow-init"), false),
            (Some("shadow-init"), true),
            (Some("shadow-restore"), false),
            (Some("doorbell"), false),
            (Some("doorbell"), true),
            (Some("gate-finish"), false),
            (Some("local-gate-poison"), false),
        ] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            let gate = LocalGateV1::new();
            trace.borrow_mut().local_gate = Some(gate.clone());
            let resources = trace.borrow().local_resources.clone();
            trace.borrow_mut().local_finish_poison = boundary == Some("local-gate-poison");
            trace.borrow_mut().fault = boundary
                .filter(|&b| b != "local-gate-poison")
                .map(|b| (b, 1, panic));
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, external_runtime);
            assert_eq!(result.is_ok(), boundary.is_none());
            if panic {
                assert_eq!(
                    result.as_ref().unwrap_err().downcast_ref::<(&str, usize)>(),
                    Some(&(boundary.unwrap(), 1))
                );
            }
            assert_root(&root, address, &trace);
            assert_eq!(
                resources.live(),
                (1, usize::from(root.completed.is_some()), 1)
            );
            assert_eq!(gate.observation(), (boundary.is_some(), boundary.is_some()));
            if let Some(c) = &root.completed {
                assert!(c.event.local_event.is_some() && c.shadows.local_published.is_some());
                assert!(c.shadows.local_unpublished.is_none());
                assert_eq!(
                    c.shadows.local_published.as_ref().unwrap().state(),
                    (true, true)
                );
            } else {
                assert!(root.event.as_ref().unwrap().local_event.is_some());
                assert!(
                    root.unpublished
                        .as_ref()
                        .unwrap()
                        .local_unpublished
                        .is_some()
                );
                assert_eq!(
                    root.unpublished
                        .as_ref()
                        .unwrap()
                        .local_unpublished
                        .as_ref()
                        .unwrap()
                        .state(),
                    (true, false, false)
                );
            }
            drop(root);
            assert_eq!(
                resources.live(),
                (0, 0, 0),
                "owned reservation, payload and event/file disposed"
            );
            assert_eq!(gate.observation(), (false, boundary.is_some()));
        }
    }
}

#[test]
fn observation_panics_keep_dependency_and_returned_doorbell_before_assembly_updates() {
    for boundary in ["event-id", "doorbell-observe"] {
        for external in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            trace.borrow_mut().fault = Some((boundary, 1, true));
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, external);
            assert_eq!(
                result.unwrap_err().downcast_ref::<(&str, usize)>(),
                Some(&(boundary, 1))
            );
            assert_root(&root, address, &trace);
            assert!(root.outputs.is_some());
            if boundary == "event-id" {
                assert!(root.completed.is_none() && root.dependency_owner.is_some());
                assert!(root.engine.is_some() && root.published.is_some());
                assert!(!trace.borrow().calls.contains(&"doorbell"));
            } else {
                let c = root.completed.as_ref().unwrap();
                assert!(c.doorbell.is_some());
                assert_eq!(
                    (
                        c.observation.doorbell_slice_bytes,
                        c.observation.doorbell_byte_offset
                    ),
                    (0, 0)
                );
            }
            assert!(!trace.borrow().calls.contains(&"gate-finish"));
        }
    }
}

fn external() -> ExternalSlots {
    ExternalSlots {
        runtime: Some(Owner::new(Role::Runtime)),
        control: Some(Owner::new(Role::Control)),
    }
}

fn identities(slots: &ExternalSlots) -> (Option<OwnerIdentity>, Option<OwnerIdentity>) {
    (
        slots.runtime.as_ref().map(|o| o.identity),
        slots.control.as_ref().map(|o| o.identity),
    )
}

fn assert_error(payload: &(dyn std::any::Any + Send), expected_stage: &str, expected_inner: &str) {
    let error = payload
        .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
        .unwrap();
    let ComputeAqlQueueSessionErrorV1::TerminalCreation { stage, source } = error else {
        panic!("expected terminal creation: {error:?}");
    };
    assert_eq!(*stage, expected_stage);
    assert!(
        matches!(&**source, ComputeAqlQueueSessionErrorV1::Contract(inner) if *inner == expected_inner),
        "{error:?}"
    );
}

#[test]
fn external_runtime_handoff_retains_exact_caller_slots_until_closing_currentness() {
    for (boundary, occurrence, transferred) in [
        ("runtime-validate", 1, false),
        ("runtime-validate", 2, false),
        ("currentness", 2, false),
        ("gate-arm", 1, true),
        ("event", 1, true),
        ("runtime-validate", 3, true),
        ("doorbell", 1, true),
        ("gate-finish", 1, true),
    ] {
        for panic in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            let mut slots = external();
            let before = identities(&slots);
            trace.borrow_mut().fault = Some((boundary, occurrence, panic));
            let (root, result) = run_with(
                root,
                QueueRingBackingV1::AqlSpecial,
                Some(&mut slots),
                prepare_fixed,
            );
            let payload = result.expect_err("injected external-runtime failure");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(boundary, occurrence))
                );
            } else {
                assert!(payload.is::<ComputeAqlQueueSessionErrorV1>());
            }
            assert_root_with_external(&root, address, &trace, Some(&slots));
            if transferred {
                assert_eq!(identities(&slots), (None, None));
                let (runtime, control) = root
                    .completed
                    .as_ref()
                    .map(|c| (&c.runtime, c.runtime_control.as_ref().unwrap()))
                    .unwrap_or_else(|| {
                        (
                            root.runtime.as_ref().unwrap(),
                            root.runtime_control.as_ref().unwrap(),
                        )
                    });
                assert_eq!((Some(runtime.identity), Some(control.identity)), before);
            } else {
                assert_eq!(identities(&slots), before);
                assert!(root.runtime.is_none() && root.runtime_control.is_none());
                assert!(root.creation_arm.is_none() && root.event.is_none());
                assert!(!trace.borrow().calls.contains(&"gate-arm"));
            }
            assert!(!trace.borrow().calls.contains(&"runtime-enable"));
        }
    }
}

#[test]
fn external_missing_or_foreign_authority_never_consumes_present_caller_slots() {
    for case in 0..6 {
        let (root, trace) = setup();
        let address = &*root as *const Root as usize;
        let mut slots = match case {
            0 => ExternalSlots {
                runtime: None,
                control: Some(Owner::new(Role::Control)),
            },
            1 => ExternalSlots {
                runtime: Some(Owner::new(Role::Runtime)),
                control: None,
            },
            2 => ExternalSlots {
                runtime: Some(Owner::new(Role::Control)),
                control: Some(Owner::new(Role::Control)),
            },
            3 => ExternalSlots {
                runtime: Some(Owner::new(Role::Runtime)),
                control: Some(Owner::new(Role::Runtime)),
            },
            _ => {
                let foreign = Rc::new(RefCell::new(Trace {
                    session: trace.borrow().session,
                    ..Trace::default()
                }));
                ACTIVE.with(|a| *a.borrow_mut() = Some(foreign));
                let owner = Owner::new(if case == 4 {
                    Role::Runtime
                } else {
                    Role::Control
                });
                ACTIVE.with(|a| *a.borrow_mut() = Some(trace.clone()));
                if case == 4 {
                    ExternalSlots {
                        runtime: Some(owner),
                        control: Some(Owner::new(Role::Control)),
                    }
                } else {
                    ExternalSlots {
                        runtime: Some(Owner::new(Role::Runtime)),
                        control: Some(owner),
                    }
                }
            }
        };
        let before = identities(&slots);
        let anchors = (
            slots.runtime.as_ref().map(|o| o.trace.clone()),
            slots.control.as_ref().map(|o| o.trace.clone()),
        );
        let (root, result) = run_with(
            root,
            QueueRingBackingV1::AqlSpecial,
            Some(&mut slots),
            prepare_fixed,
        );
        let payload = result.unwrap_err();
        assert_error(
            &*payload,
            "USERPTR queue-control creation",
            match case {
                0 => "missing debug runtime authority",
                1 => "missing debug runtime control descriptor",
                _ => "platform owner binding",
            },
        );
        assert_eq!(identities(&slots), before);
        for (owner, anchor) in [(&slots.runtime, anchors.0), (&slots.control, anchors.1)] {
            if let Some(owner) = owner {
                assert!(Rc::ptr_eq(&owner.trace, anchor.as_ref().unwrap()));
                assert_eq!(owner.trace.borrow().drops, 0);
            }
        }
        assert_root_with_external(&root, address, &trace, Some(&slots));
        assert!(
            root.runtime.is_none() && root.runtime_control.is_none() && root.creation_arm.is_none()
        );
        assert!(!trace.borrow().calls.contains(&"event"));
    }
}

#[test]
fn late_create_recovery_preserves_exact_engine_outputs_and_unassembled_owners() {
    for (boundary, stage, inner, invalid) in [
        (
            "recover-outputs",
            "CREATE_QUEUE output recovery",
            "missing CREATE outputs",
            false,
        ),
        (
            "recover-id",
            "CREATE_QUEUE identity recovery",
            "missing queue id",
            false,
        ),
        (
            "dependency",
            "compute dependency session owner",
            "dependency custody allocation",
            false,
        ),
        (
            "dependency",
            "compute dependency session owner",
            "dependency session occurrence",
            true,
        ),
    ] {
        for panic in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            trace.borrow_mut().fault = Some((boundary, 1, panic));
            trace.borrow_mut().dependency_invalid = invalid;
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, true);
            let payload = result.unwrap_err();
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(boundary, 1))
                );
            } else {
                assert_error(&*payload, stage, inner);
            }
            assert_root(&root, address, &trace);
            assert!(root.completed.is_none() && root.dependency_owner.is_none());
            assert!(
                root.runtime.is_some()
                    && root.runtime_control.is_some()
                    && root.event.is_some()
                    && root.published.is_some()
            );
            assert!(
                root.dispatch.is_some()
                    && root.submission.is_some()
                    && root.completion_owner.is_some()
                    && root.completion.retained.is_some()
            );
            let engine = root.engine.as_ref().unwrap();
            assert_eq!(engine.resources.len(), 1);
            let key = engine.resources[0].key;
            let outputs = engine.create_outputs(key).unwrap();
            assert_eq!(engine.native_queue_id(key), Some(7));
            assert_eq!(
                trace.borrow().create_return,
                Some((outputs.queue_id().value(), outputs.doorbell_offset().raw()))
            );
            assert_eq!(
                root.outputs,
                (boundary != "recover-outputs").then_some(outputs)
            );
            assert!(!trace.borrow().calls.contains(&"doorbell"));
            assert!(!trace.borrow().calls.contains(&"gate-finish"));
            assert_eq!(trace.borrow().cleanup, 0);
        }
    }
}

#[test]
fn platform_success_and_shadow_settlement_keep_original_bindings_and_phase() {
    for boundary in [
        None,
        Some("shadow-init"),
        Some("shadow-restore"),
        Some("doorbell"),
        Some("gate-finish"),
    ] {
        let (root, trace) = setup();
        let address = &*root as *const Root as usize;
        let mut slots = external();
        let original = identities(&slots);
        trace.borrow_mut().fault = boundary.map(|s| (s, 1, false));
        let (root, result) = run_with(
            root,
            QueueRingBackingV1::AqlSpecial,
            Some(&mut slots),
            prepare_fixed,
        );
        assert_eq!(result.is_ok(), boundary.is_none());
        assert_root_with_external(&root, address, &trace, Some(&slots));
        assert_eq!(identities(&slots), (None, None));
        let t = trace.borrow();
        if let Some(c) = &root.completed {
            assert_eq!(
                (
                    Some(c.runtime.identity),
                    c.runtime_control.as_ref().map(|o| o.identity)
                ),
                original
            );
            assert_eq!(c.shadows.shadow.unwrap().0, ShadowPhase::Published);
            assert_eq!(t.cleanup, 0);
            if let Some(doorbell) = &c.doorbell {
                let outputs = doorbell.doorbell.unwrap();
                assert_eq!(Some(outputs), root.outputs);
                assert_eq!(c.observation.queue_id, outputs.queue_id().value());
                assert_eq!(
                    c.observation.doorbell_byte_offset,
                    outputs.doorbell_offset().in_process_byte_offset()
                );
                assert_eq!(
                    c.observation.doorbell_slice_bytes,
                    KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize
                );
                assert_eq!(doorbell.identity.pid, memory(&root).opener_pid());
            } else {
                assert_eq!(
                    (
                        c.observation.doorbell_slice_bytes,
                        c.observation.doorbell_byte_offset
                    ),
                    (0, 0)
                );
            }
        } else {
            assert_eq!(
                root.unpublished.as_ref().unwrap().shadow.unwrap().0,
                ShadowPhase::Disposed
            );
            let poison = t.calls.iter().position(|&s| s == "poison").unwrap();
            let cleanup = t.calls.iter().position(|&s| s == "cleanup").unwrap();
            let retain = t.calls.iter().position(|&s| s == "retain-root").unwrap();
            assert!(poison < cleanup && cleanup < retain);
        }
    }
}
