use super::*;
use crate::sdma::retained_release::{RetainedSdmaReleaseCustodyV1, fixture as sdma_fixture};

fn with_striped(count: u32, dispatch: bool) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    sdma_cases::with_sdma_profile(dispatch, |memory, key| {
        let before = memory.observation().host.unwrap();
        let set = sdma_fixture::striped(memory, key, count, count as usize - 1);
        let mut expected = before;
        expected.used_backing_bytes += 4096 * u64::from(count);
        expected.used_allocation_records += u64::from(count);
        expected.retained_records += count as usize;
        assert_eq!(memory.observation().host, Some(expected));
        set
    })
}

fn assert_preflight_rejected(parent: &Parent, t: &Rc<RefCell<Trace>>, gate: &LocalGateV1) {
    let set = parent.sdma.as_ref().unwrap();
    let owners = sdma_fixture::creation_observation(set);
    let pending = sdma_fixture::generic_pending_observation(set);
    let Gfx942SdmaQueueSetV1::Striped { next_owner, .. } = set else {
        panic!("striped fixture")
    };
    let cursor = *next_owner;
    let before = parent
        .engine
        .backend
        .session
        .primary_release_memory_snapshot_v1(&parent.engine.foundation);
    assert!(parent.preflight_release().is_err());
    assert!(t.borrow().calls.is_empty());
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
    assert!(!parent.poisoned);
    assert_eq!(sdma_fixture::creation_observation(set), owners);
    assert_eq!(sdma_fixture::generic_pending_observation(set), pending);
    assert_eq!(*next_owner, cursor);
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
fn constructed_striped_release_rejects_malformed_rosters_without_effects() {
    for (count, cursor) in [
        (0, 0),
        (1, 0),
        (3, 0),
        (17, 0),
        (18, 0),
        (2, 2),
        (16, usize::MAX),
    ] {
        let (mut parent, t, gate) =
            sdma_cases::with_unchecked_sdma_profile(false, |memory, key| {
                sdma_fixture::striped(memory, key, count, cursor)
            });
        assert!(matches!(
            parent
                .sdma
                .as_ref()
                .unwrap()
                .supports_retained_sdma_release_v1(),
            Err(Gfx942SdmaErrorV1::Contract("striped SDMA owner roster"))
        ));
        assert_preflight_rejected(&parent, &t, &gate);
        sdma_fixture::cleanup_set(parent.sdma.as_mut().unwrap());
    }
}

#[test]
fn constructed_striped_release_checks_every_last_owner_field_before_effects() {
    let (mut parent, t, gate) = with_striped(16, false);
    let original = sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap());
    for case in 0..22 {
        let set = parent.sdma.take().unwrap();
        parent.sdma = Some(sdma_fixture::with_owner_corruption(
            set,
            15,
            case,
            parent.queue_id,
            |set| {
                parent.sdma = Some(set);
                assert_preflight_rejected(&parent, &t, &gate);
                let error = parent.preflight_release().unwrap_err();
                if (14..=18).contains(&case) {
                    assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Pending)
                    ));
                } else {
                    assert!(matches!(
                        error,
                        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract(_))
                    ));
                }
                parent.sdma.take().unwrap()
            },
        ));
        assert_eq!(
            sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap()),
            original
        );
        parent.preflight_release().unwrap();
        t.borrow_mut().calls.clear();
    }
    sdma_fixture::cleanup_set(parent.sdma.as_mut().unwrap());
}

#[test]
fn constructed_striped_release_orders_all_balanced_counts_and_refunds_backing() {
    for count in (2..=16).step_by(2) {
        for dispatch in [false, true] {
            let (mut parent, t, gate) = with_striped(count, dispatch);
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            assert!(
                parent
                    .sdma
                    .as_ref()
                    .unwrap()
                    .supports_retained_sdma_release_v1()
                    .unwrap()
            );
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = state.release_in_place(&mut parent).unwrap();
            assert_eq!(result.released_resources, 5 + 3 * count as u8);
            assert!(parent.sdma.is_none() && state.complete);
            let sdma = state.sdma.as_ref().unwrap();
            assert!(sdma.is_complete());
            let observed = sdma.observation();
            observed.assert_original_owners(&before);
            assert_eq!(
                observed.state,
                (true, true, false, count as usize, count as usize)
            );
            assert_eq!(
                t.borrow().sdma_destroy_ids,
                (100..100 + count).collect::<Vec<_>>()
            );
            assert_eq!(
                t.borrow().sdma_resource_ids,
                before
                    .owners
                    .iter()
                    .rev()
                    .map(|owner| owner.identities[0].unwrap())
                    .collect::<Vec<_>>()
            );
            parent
                .engine
                .backend
                .session
                .primary_assert_all_released_v1();
            let calls = t.borrow().calls.clone();
            let count_calls = |name| calls.iter().filter(|&&c| c == name).count();
            assert_eq!(count_calls("sdma-topology"), 2);
            assert_eq!(count_calls("sdma-currentness"), 2 * count as usize);
            assert_eq!(count_calls("sdma-doorbell"), count as usize);
            assert_eq!(count_calls("sdma-release-resources"), count as usize);
            let first = |name| calls.iter().position(|&c| c == name).unwrap();
            let last = |name| calls.iter().rposition(|&c| c == name).unwrap();
            assert!(last("sdma-destroy") < last("sdma-topology"));
            assert!(last("sdma-topology") < first("destroy"));
            assert!(first("release-resources") < first("sdma-release-resources"));
            assert!(last("sdma-release-resources") < first("release-signals"));
            assert_eq!(gate.observation(), (false, false));
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}

#[test]
fn constructed_striped_release_retains_every_callback_failure_prefix() {
    const COUNT: usize = 16;
    for (name, occurrences) in [
        ("sdma-topology", 2),
        ("sdma-currentness", 2 * COUNT),
        ("sdma-destroy", COUNT),
        ("sdma-doorbell", COUNT),
        ("sdma-release-resources", COUNT),
    ] {
        for occurrence in 1..=occurrences {
            for panic in [false, true] {
                let (mut parent, t, gate) = with_striped(COUNT as u32, false);
                let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
                t.borrow_mut().fault = Some((name, occurrence, panic));
                let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, usize)>(),
                        Some(&(name, occurrence))
                    );
                } else {
                    let error = result.unwrap().unwrap_err();
                    match name {
                        "sdma-destroy" => assert!(matches!(
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
                let resources_started = name == "sdma-release-resources";
                let (completed, attempted, destroyed) = match name {
                    "sdma-topology" if occurrence == 1 => (0, 0, 0),
                    "sdma-topology" | "sdma-release-resources" => (COUNT, COUNT, COUNT),
                    "sdma-currentness" => ((occurrence - 1) / 2, occurrence / 2, occurrence / 2),
                    _ => (occurrence - 1, occurrence, occurrence - 1),
                };
                let released = if resources_started { occurrence - 1 } else { 0 };
                let sdma = state.sdma.as_ref().unwrap();
                let observed = sdma.observation();
                observed.assert_original_owners(&before);
                assert_eq!(
                    observed.state,
                    (true, resources_started, true, completed, released)
                );
                assert!(!sdma.is_complete() && parent.poisoned && !state.complete);
                assert!(parent.engine.backend.session.primary_is_quarantined_v1());
                assert!(parent.sdma.is_none() && parent.signals.is_some());
                assert_eq!(
                    t.borrow().sdma_destroy_ids,
                    (100..100 + attempted as u32).collect::<Vec<_>>()
                );
                for (index, owner) in observed.owners.iter().enumerate() {
                    let attempted = index < attempted;
                    let failed_destroy = name == "sdma-destroy" && index == occurrence - 1;
                    let doorbell_attempted = attempted && !failed_destroy;
                    let failed_doorbell = name == "sdma-doorbell" && index == occurrence - 1;
                    assert!(owner.poisoned);
                    assert_eq!(owner.attempted, attempted);
                    assert_eq!(
                        owner.request,
                        attempted.then(|| fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs::new(
                            100 + index as u32
                        ))
                    );
                    assert_eq!(
                        owner.result,
                        if !attempted || (failed_destroy && panic) {
                            None
                        } else if failed_destroy {
                            Some(Err(rustix::io::Errno::IO))
                        } else {
                            Some(Ok(()))
                        }
                    );
                    assert_eq!(owner.destroyed, index < destroyed);
                    assert_eq!(owner.doorbell.attempted, doorbell_attempted);
                    assert_eq!(
                        owner.doorbell.result,
                        if !doorbell_attempted || (failed_doorbell && panic) {
                            None
                        } else if failed_doorbell {
                            Some(Err(rustix::io::Errno::IO))
                        } else {
                            Some(Ok(()))
                        }
                    );
                    assert_eq!(
                        owner.resources.is_some(),
                        resources_started && index >= COUNT - occurrence
                    );
                }
                if resources_started {
                    assert!(state.resources.as_ref().unwrap().is_complete());
                    assert_eq!(
                        t.borrow().sdma_resource_ids,
                        before
                            .owners
                            .iter()
                            .rev()
                            .take(occurrence)
                            .map(|owner| owner.identities[0].unwrap())
                            .collect::<Vec<_>>()
                    );
                } else {
                    assert!(!state.destroy.attempted && state.resources.is_none());
                }
                assert_eq!(gate.observation(), (false, true));
                assert_no_retry(&mut parent, &mut state, &t);
                state.sdma.as_mut().unwrap().cleanup_local_mappings();
            }
        }
    }
}

#[test]
fn constructed_striped_release_retains_mutated_destroy_inputs() {
    for index in [0, 7, 15] {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_striped(16, false);
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            let id = 100 + index as u32;
            t.borrow_mut().sdma_destroy_mutation = Some((id, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, u32)>(),
                    Some(&("mutated SDMA destroy", id))
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
            assert_eq!(observed.state, (true, false, true, index, 0));
            assert!(observed.owners.iter().all(|owner| owner.poisoned));
            let owner = &observed.owners[index];
            assert_eq!(
                owner.request,
                Some(fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs {
                    queue_id: id,
                    pad: 17
                })
            );
            assert_eq!(owner.result, (!panic).then_some(Ok(())));
            assert!(owner.attempted && !owner.destroyed && !owner.doorbell.started);
            assert!(!state.destroy.attempted);
            assert!(parent.engine.backend.session.primary_is_quarantined_v1());
            assert_eq!(gate.observation(), (false, true));
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}

#[test]
fn constructed_striped_release_keeps_real_late_cleanup_prefix_for_every_owner() {
    for occurrence in 1..=16 {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_striped(16, false);
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            t.borrow_mut().sdma_resource_native_fault = Some((occurrence, panic));
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
                occurrence,
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

#[test]
fn retained_striped_release_custody_has_bounded_inline_size() {
    let bytes = std::mem::size_of::<RetainedSdmaReleaseCustodyV1>();
    assert!(bytes <= 32 * 1024, "retained SDMA custody: {bytes} bytes");
    let root_bytes = std::mem::size_of::<crate::PrimaryQueueReleaseCustodyV1>();
    assert!(
        root_bytes <= 128 * 1024,
        "public release root: {root_bytes} bytes"
    );
}
