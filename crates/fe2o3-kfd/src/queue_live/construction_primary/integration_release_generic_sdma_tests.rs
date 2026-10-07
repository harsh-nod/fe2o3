use super::*;
use crate::sdma::retained_release::fixture as sdma_fixture;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn with_generic(engine: Option<u32>, dispatch: bool) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    sdma_cases::with_sdma_profile(dispatch, |memory, key| {
        sdma_fixture::generic(memory, key, engine)
    })
}

fn assert_preflight_rejection(
    parent: &Parent,
    t: &Rc<RefCell<Trace>>,
    gate: &LocalGateV1,
    case: u8,
) {
    let set = parent.sdma.as_ref().unwrap();
    assert!(matches!(set, Gfx942SdmaQueueSetV1::Generic(_)));
    let owners = sdma_fixture::creation_observation(set);
    let pending_owners = sdma_fixture::generic_pending_observation(set);
    let before = parent
        .engine
        .backend
        .session
        .primary_release_memory_snapshot_v1(&parent.engine.foundation);
    let error = parent.preflight_release().unwrap_err();
    if case >= 14 {
        assert!(matches!(
            error,
            ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Pending)
        ));
    } else {
        let expected = match case {
            0 | 1 => "generic SDMA owner roster",
            3 | 4 => "SDMA queue is not live",
            13 => "missing SDMA doorbell",
            _ => "retained SDMA release owners",
        };
        assert!(
            matches!(error, ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract(got)) if got == expected)
        );
    }
    assert!(
        t.borrow().calls.is_empty(),
        "case {case}: {:?}",
        t.borrow().calls
    );
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
    assert!(!parent.poisoned);
    assert_eq!(
        sdma_fixture::generic_pending_observation(parent.sdma.as_ref().unwrap()),
        pending_owners
    );
    assert_eq!(
        sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap()),
        owners
    );
    assert_eq!(
        parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation),
        before
    );
}

#[test]
fn constructed_generic_release_rejects_malformed_or_pending_owner_without_effects() {
    for engine in [None, Some(0), Some(1)] {
        let (mut parent, t, gate) = with_generic(engine, false);
        let original = sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap());
        for case in 0..19 {
            let set = parent.sdma.take().unwrap();
            let set = sdma_fixture::with_generic_corruption(set, case, parent.queue_id, |set| {
                if case <= 1 {
                    assert!(set.supports_retained_sdma_release_v1().is_err());
                }
                parent.sdma = Some(set);
                assert_preflight_rejection(&parent, &t, &gate, case);
                parent.sdma.take().unwrap()
            });
            parent.sdma = Some(set);
            assert_eq!(
                sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap()),
                original
            );
            parent.preflight_release().unwrap();
            t.borrow_mut().calls.clear();
        }
        sdma_fixture::cleanup_set(parent.sdma.as_mut().unwrap());
    }
    let (mut parent, t, gate) = sdma_cases::with_sdma(false);
    parent.sdma = Some(sdma_fixture::directional_as_malformed_generic(
        parent.sdma.take().unwrap(),
    ));
    assert!(
        parent
            .sdma
            .as_ref()
            .unwrap()
            .supports_retained_sdma_release_v1()
            .is_err()
    );
    assert_preflight_rejection(&parent, &t, &gate, 0);
    sdma_fixture::cleanup_set(parent.sdma.as_mut().unwrap());
}

#[test]
fn constructed_generic_release_orders_original_owner_and_refunds_all_backing() {
    for engine in [None, Some(0), Some(1)] {
        for dispatch in [false, true] {
            let (mut parent, t, gate) = sdma_cases::with_sdma_profile(dispatch, |memory, key| {
                let before = memory.observation().host.unwrap();
                assert!(before.used_backing_bytes > 0 && before.used_allocation_records > 0);
                assert_eq!(before.reserved_records, 0);
                assert_eq!(
                    before.retained_records as u64,
                    before.used_allocation_records
                );
                assert_eq!(before.quarantined_records, 0);
                assert!(!before.poisoned);
                let set = sdma_fixture::generic(memory, key, engine);
                let mut expected = before;
                expected.used_backing_bytes += 4096;
                expected.used_allocation_records += 1;
                expected.retained_records += 1;
                assert_eq!(memory.observation().host, Some(expected));
                set
            });
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            assert!(
                parent
                    .sdma
                    .as_ref()
                    .unwrap()
                    .supports_retained_sdma_release_v1()
                    .unwrap()
            );
            // An untargeted queue must not acquire a topology dependency.
            if engine.is_none() {
                t.borrow_mut().fault = Some(("sdma-topology", 1, true));
            }
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let destroyed = state.release_in_place(&mut parent).unwrap();
            assert_eq!(destroyed.released_resources, 8);
            assert!(parent.sdma.is_none() && state.complete);
            let sdma = state.sdma.as_ref().unwrap();
            assert!(sdma.is_complete());
            let observed = sdma.observation();
            observed.assert_original_owners(&before);
            assert_eq!(observed.state, (true, true, false, 1, 1));
            assert_eq!(observed.owners[0].engine, engine);
            assert_eq!(observed.owners[0].result, Some(Ok(())));
            parent
                .engine
                .backend
                .session
                .primary_assert_all_released_v1();
            let calls = t.borrow().calls.clone();
            let position = |name| calls.iter().position(|&c| c == name).unwrap();
            let count = |name| calls.iter().filter(|&&c| c == name).count();
            assert_eq!(count("sdma-topology"), if engine.is_some() { 2 } else { 0 });
            assert_eq!(count("sdma-currentness"), 2);
            assert_eq!(count("sdma-destroy-h2d"), 0);
            assert_eq!(count("sdma-destroy-d2h"), 1);
            assert_eq!(count("sdma-release-resources"), 1);
            assert!(position("sdma-destroy-d2h") < position("destroy"));
            assert!(position("release-resources") < position("sdma-release-resources"));
            assert!(position("sdma-release-resources") < position("release-signals"));
            assert_eq!(gate.observation(), (false, false));
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}

#[test]
fn constructed_generic_release_failures_keep_original_owner_and_completed_prefix() {
    for engine in [None, Some(0), Some(1)] {
        for (name, count) in [
            ("sdma-topology", if engine.is_some() { 2 } else { 0 }),
            ("sdma-currentness", 2),
            ("sdma-destroy-d2h", 1),
            ("sdma-doorbell", 1),
            ("sdma-release-resources", 1),
        ] {
            for occurrence in 1..=count {
                for panic in [false, true] {
                    let (mut parent, t, gate) = with_generic(engine, false);
                    let before =
                        sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
                    t.borrow_mut().fault = Some((name, occurrence, panic));
                    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                    let result =
                        catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, usize)>(),
                            Some(&(name, occurrence))
                        );
                    } else {
                        let error = result.unwrap().unwrap_err();
                        match name {
                            "sdma-destroy-d2h" => assert!(matches!(
                                error,
                                ComputeAqlQueueSessionErrorV1::Sdma(
                                    Gfx942SdmaErrorV1::QueueDestroyIndeterminate
                                )
                            )),
                            "sdma-doorbell" => assert!(matches!(
                                error,
                                ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract(
                                    "SDMA doorbell release"
                                ))
                            )),
                            _ => assert!(
                                matches!(error, ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Memory(MemorySessionError::Model(got))) if got == name)
                            ),
                        }
                    }
                    assert!(parent.poisoned && !state.complete);
                    assert!(parent.engine.backend.session.primary_is_quarantined_v1());
                    assert!(parent.sdma.is_none() && parent.signals.is_some());
                    let sdma = state.sdma.as_ref().unwrap();
                    assert!(!sdma.is_complete());
                    let observed = sdma.observation();
                    observed.assert_original_owners(&before);
                    let completed = usize::from(
                        name == "sdma-release-resources"
                            || (name == "sdma-topology" && occurrence == 2),
                    );
                    assert_eq!(
                        observed.state,
                        (true, name == "sdma-release-resources", true, completed, 0)
                    );
                    let owner = &observed.owners[0];
                    let attempted = t.borrow().calls.contains(&"sdma-destroy-d2h");
                    let failed = name == "sdma-destroy-d2h";
                    assert_eq!(owner.attempted, attempted);
                    assert_eq!(
                        owner.request,
                        attempted.then(|| fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs::new(100))
                    );
                    assert_eq!(
                        owner.result,
                        if !attempted || (failed && panic) {
                            None
                        } else if failed {
                            Some(Err(rustix::io::Errno::IO))
                        } else {
                            Some(Ok(()))
                        }
                    );
                    let doorbell_failed = name == "sdma-doorbell";
                    assert_eq!(owner.destroyed, attempted && !failed && !doorbell_failed);
                    assert_eq!(owner.doorbell.attempted, attempted && !failed);
                    assert_eq!(
                        owner.doorbell.result,
                        if !attempted || failed || (doorbell_failed && panic) {
                            None
                        } else if doorbell_failed {
                            Some(Err(rustix::io::Errno::IO))
                        } else {
                            Some(Ok(()))
                        }
                    );
                    assert!(owner.poisoned);
                    assert_eq!(gate.observation(), (false, true));
                    if name != "sdma-release-resources" {
                        assert!(!state.destroy.attempted && state.resources.is_none());
                    } else {
                        assert!(state.resources.as_ref().unwrap().is_complete());
                    }
                    assert_no_retry(&mut parent, &mut state, &t);
                    state.sdma.as_mut().unwrap().cleanup_local_mappings();
                }
            }
        }
    }
}

#[test]
fn constructed_generic_release_retains_mutated_destroy_argument_and_raw_outcome() {
    for engine in [None, Some(0), Some(1)] {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_generic(engine, false);
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            t.borrow_mut().sdma_destroy_mutation = Some((100, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, u32)>(),
                    Some(&("mutated SDMA destroy", 100))
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Sdma(
                        Gfx942SdmaErrorV1::Contract(
                            "kernel changed immutable SDMA DESTROY_QUEUE inputs"
                        )
                    ))
                ));
            }
            let observed = state.sdma.as_ref().unwrap().observation();
            observed.assert_original_owners(&before);
            assert_eq!(observed.state, (true, false, true, 0, 0));
            let owner = &observed.owners[0];
            assert_eq!(
                owner.request,
                Some(fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs {
                    queue_id: 100,
                    pad: 17
                })
            );
            assert!(owner.attempted && owner.poisoned);
            assert_eq!(owner.result, (!panic).then_some(Ok(())));
            assert!(!owner.destroyed && !owner.doorbell.started);
            assert!(!state.destroy.attempted);
            assert!(parent.engine.backend.session.primary_is_quarantined_v1());
            assert_eq!(gate.observation(), (false, true));
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}

#[test]
fn constructed_generic_release_late_native_resource_failure_keeps_real_prefix() {
    for engine in [None, Some(0), Some(1)] {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_generic(engine, false);
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            t.borrow_mut().sdma_resource_native_fault = Some((1, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "release_va_reservation"))
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Sdma(
                        Gfx942SdmaErrorV1::Memory(MemorySessionError::Injected(
                            "release_va_reservation"
                        ))
                    ))
                ));
            }
            let sdma = state.sdma.as_ref().unwrap();
            sdma.observation().assert_original_owners(&before);
            sdma.assert_late_resource_failure(
                &parent.engine.backend.session,
                t.borrow().sdma_resource_snapshot.as_ref().unwrap(),
                1,
                panic,
            );
            assert!(state.resources.as_ref().unwrap().is_complete());
            assert!(state.signals.is_none() && parent.signals.is_some());
            assert!(parent.poisoned && !state.complete);
            assert_eq!(gate.observation(), (false, true));
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}
