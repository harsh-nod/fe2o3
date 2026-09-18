use super::*;
use crate::sdma::retained_release::{
    fixture as sdma_fixture, supports_retained_sdma_composition_v1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn with_combined(count: u32, dispatch: bool) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    let mut striped = None;
    let (mut parent, t, gate) = sdma_cases::with_unchecked_sdma_profile(dispatch, |memory, key| {
        let directional = sdma_fixture::directional_with_ids(memory, key, 200);
        striped = Some(sdma_fixture::striped(
            memory,
            key,
            count,
            count as usize - 1,
        ));
        directional
    });
    parent.striped_sdma = striped;
    parent.preflight_release().unwrap();
    t.borrow_mut().calls.clear();
    (parent, t, gate)
}

fn cleanup_parent(parent: &mut Parent) {
    for set in [&mut parent.sdma, &mut parent.striped_sdma]
        .into_iter()
        .flatten()
    {
        sdma_fixture::cleanup_set(set);
    }
}

fn cleanup_state(state: &mut PrimaryReleaseStateV1<Fixture>) {
    for set in [&mut state.sdma, &mut state.striped_sdma]
        .into_iter()
        .flatten()
    {
        set.cleanup_local_mappings();
    }
}

fn assert_rejected(parent: &Parent, t: &Rc<RefCell<Trace>>, gate: &LocalGateV1) {
    let observe = || {
        [parent.sdma.as_ref(), parent.striped_sdma.as_ref()]
            .map(|set| set.map(sdma_fixture::creation_observation))
    };
    let before = observe();
    let memory = parent
        .engine
        .backend
        .session
        .primary_release_memory_snapshot_v1(&parent.engine.foundation);
    assert!(parent.preflight_release().is_err());
    assert_eq!(observe(), before);
    assert_eq!(
        parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation),
        memory
    );
    assert!(t.borrow().calls.is_empty() && !parent.poisoned);
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
}

#[test]
fn constructed_combined_release_orders_all_counts_and_refunds_both_sets() {
    for count in (2..=14).step_by(2) {
        for dispatch in [false, true] {
            let (mut parent, t, gate) = with_combined(count, dispatch);
            let before = [parent.striped_sdma.as_ref(), parent.sdma.as_ref()]
                .map(|set| sdma_fixture::unreleased_observation(set.unwrap()));
            assert!(
                supports_retained_sdma_composition_v1(
                    parent.sdma.as_ref(),
                    parent.striped_sdma.as_ref()
                )
                .unwrap()
            );
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = state.release_in_place(&mut parent).unwrap();
            assert_eq!(result.released_resources, 11 + 3 * count as u8);
            assert!(state.complete && parent.sdma.is_none() && parent.striped_sdma.is_none());
            for (set, original) in [state.striped_sdma.as_ref(), state.sdma.as_ref()]
                .into_iter()
                .zip(&before)
            {
                let set = set.unwrap();
                assert!(set.is_complete());
                set.observation().assert_original_owners(original);
            }
            assert_eq!(
                t.borrow().sdma_destroy_ids,
                (100..100 + count).chain([201, 200]).collect::<Vec<_>>()
            );
            assert_eq!(
                t.borrow().sdma_resource_ids,
                before
                    .iter()
                    .flat_map(|set| set.owners.iter().rev())
                    .map(|owner| owner.identities[0].unwrap())
                    .collect::<Vec<_>>()
            );
            let calls = t.borrow().calls.clone();
            let first = |name| calls.iter().position(|&c| c == name).unwrap();
            let last = |name| calls.iter().rposition(|&c| c == name).unwrap();
            assert_eq!(calls.iter().filter(|&&c| c == "sdma-topology").count(), 4);
            assert_eq!(
                calls.iter().filter(|&&c| c == "sdma-currentness").count(),
                2 * (count as usize + 2)
            );
            assert!(last("sdma-topology") < first("destroy"));
            assert!(first("release-resources") < first("sdma-release-resources"));
            assert!(last("sdma-release-resources") < first("release-signals"));
            parent
                .engine
                .backend
                .session
                .primary_assert_all_released_v1();
            assert_eq!(gate.observation(), (false, false));
            assert_no_retry(&mut parent, &mut state, &t);
            cleanup_state(&mut state);
        }
    }
}

#[test]
fn constructed_combined_release_rejects_wrong_profiles_and_cross_set_ids() {
    for case in 0..6 {
        let (mut parent, t, gate) = with_combined(14, false);
        match case {
            0 => {
                let removed = parent.sdma.take();
                assert_rejected(&parent, &t, &gate);
                parent.sdma = removed;
            }
            1 => {
                std::mem::swap(&mut parent.sdma, &mut parent.striped_sdma);
                assert_rejected(&parent, &t, &gate);
                std::mem::swap(&mut parent.sdma, &mut parent.striped_sdma);
            }
            2 => {
                parent.sdma = Some(sdma_fixture::directional_as_malformed_generic(
                    parent.sdma.take().unwrap(),
                ));
                assert_rejected(&parent, &t, &gate);
            }
            3 => {
                let Gfx942SdmaQueueSetV1::Striped { next_owner, .. } =
                    parent.striped_sdma.as_mut().unwrap()
                else {
                    unreachable!()
                };
                *next_owner = 14;
                assert_rejected(&parent, &t, &gate);
            }
            _ => {
                let set = parent.striped_sdma.take().unwrap();
                parent.striped_sdma = Some(sdma_fixture::with_owner_corruption(
                    set,
                    if case == 4 { 0 } else { 13 },
                    2,
                    if case == 4 { 200 } else { 201 },
                    |set| {
                        parent.striped_sdma = Some(set);
                        assert_rejected(&parent, &t, &gate);
                        assert!(matches!(
                            parent.preflight_release(),
                            Err(ComputeAqlQueueSessionErrorV1::Sdma(
                                Gfx942SdmaErrorV1::Contract("combined SDMA queue identity")
                            ))
                        ));
                        parent.striped_sdma.take().unwrap()
                    },
                ));
            }
        }
        cleanup_parent(&mut parent);
    }
    let mut secondary = None;
    let (mut parent, t, gate) = sdma_cases::with_unchecked_sdma_profile(false, |memory, key| {
        secondary = Some(sdma_fixture::striped(memory, key, 16, 15));
        sdma_fixture::directional_with_ids(memory, key, 200)
    });
    parent.striped_sdma = secondary;
    assert_rejected(&parent, &t, &gate);
    cleanup_parent(&mut parent);

    for count in [2, 14] {
        let mut secondary = None;
        let (mut parent, t, gate) =
            sdma_cases::with_unchecked_sdma_profile(false, |memory, key| {
                secondary = Some(sdma_fixture::striped(memory, key, count, 0));
                sdma_fixture::creation_profile(memory, key, 3)
            });
        assert!(supports_retained_sdma_composition_v1(parent.sdma.as_ref(), None).unwrap());
        parent.striped_sdma = secondary;
        for malformed in [false, true] {
            if malformed {
                let Gfx942SdmaQueueSetV1::LogicalMuxV2 {
                    logical_lane_count, ..
                } = parent.sdma.as_mut().unwrap()
                else {
                    unreachable!()
                };
                *logical_lane_count = 3;
            }
            let expected = if malformed {
                "logical mux SDMA owner roster"
            } else {
                "combined SDMA owner roster"
            };
            assert!(matches!(
                supports_retained_sdma_composition_v1(parent.sdma.as_ref(), parent.striped_sdma.as_ref()),
                Err(Gfx942SdmaErrorV1::Contract(got)) if got == expected
            ));
            assert_rejected(&parent, &t, &gate);
            assert!(matches!(
                parent.preflight_release(),
                Err(ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract(got))) if got == expected
            ));
        }
        cleanup_parent(&mut parent);
    }
    let mut secondary = None;
    let (mut parent, t, gate) = sdma_cases::with_unchecked_sdma_profile(false, |memory, key| {
        secondary = Some(sdma_fixture::logical_mux(memory, key, 16, 15));
        sdma_fixture::directional_with_ids(memory, key, 200)
    });
    parent.striped_sdma = secondary;
    assert!(matches!(
        supports_retained_sdma_composition_v1(parent.sdma.as_ref(), parent.striped_sdma.as_ref()),
        Err(Gfx942SdmaErrorV1::Contract("combined SDMA owner roster"))
    ));
    assert_rejected(&parent, &t, &gate);
    cleanup_parent(&mut parent);
}

#[test]
fn constructed_combined_release_preflights_every_field_in_both_sets() {
    let (mut parent, t, gate) = with_combined(14, false);
    for secondary in [false, true] {
        for case in 0..22 {
            let set = if secondary {
                parent.striped_sdma.take()
            } else {
                parent.sdma.take()
            }
            .unwrap();
            let set = sdma_fixture::with_owner_corruption(
                set,
                if secondary { 13 } else { 1 },
                case,
                parent.queue_id,
                |set| {
                    if secondary {
                        parent.striped_sdma = Some(set);
                    } else {
                        parent.sdma = Some(set);
                    }
                    let pending = sdma_fixture::generic_pending_observation(
                        if secondary {
                            parent.striped_sdma.as_ref()
                        } else {
                            parent.sdma.as_ref()
                        }
                        .unwrap(),
                    );
                    assert_rejected(&parent, &t, &gate);
                    if (14..=18).contains(&case) {
                        assert!(matches!(
                            parent.preflight_release(),
                            Err(ComputeAqlQueueSessionErrorV1::Sdma(
                                Gfx942SdmaErrorV1::Pending
                            ))
                        ));
                    }
                    let set = if secondary {
                        parent.striped_sdma.take()
                    } else {
                        parent.sdma.take()
                    }
                    .unwrap();
                    assert_eq!(sdma_fixture::generic_pending_observation(&set), pending);
                    set
                },
            );
            if secondary {
                parent.striped_sdma = Some(set);
            } else {
                parent.sdma = Some(set);
            }
            parent.preflight_release().unwrap();
            t.borrow_mut().calls.clear();
        }
    }
    cleanup_parent(&mut parent);
}

#[test]
fn constructed_combined_release_retains_both_sets_at_every_callback_failure() {
    const COUNT: usize = 14;
    for (name, occurrences) in [
        ("sdma-topology", 4),
        ("sdma-currentness", 2 * (COUNT + 2)),
        ("sdma-destroy", COUNT + 2),
        ("sdma-doorbell", COUNT + 2),
        ("sdma-release-resources", COUNT + 2),
        ("destroy", 1),
        ("release-resources", 1),
        ("complete-shadows", 1),
        ("release-signals", 1),
    ] {
        for occurrence in 1..=occurrences {
            for panic in [false, true] {
                let (mut parent, t, gate) = with_combined(COUNT as u32, false);
                let before = [parent.striped_sdma.as_ref(), parent.sdma.as_ref()]
                    .map(|set| sdma_fixture::unreleased_observation(set.unwrap()));
                if name == "destroy" {
                    t.borrow_mut().destroy = Some(if panic { 3 } else { 2 });
                } else {
                    t.borrow_mut().fault = Some((name, occurrence, panic));
                }
                let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
                if panic && name == "destroy" {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<&str>(),
                        Some(&"DESTROY panic")
                    );
                } else if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, usize)>(),
                        Some(&(name, occurrence))
                    );
                } else {
                    assert!(result.unwrap().is_err(), "{name} occurrence {occurrence}");
                }
                assert!(parent.poisoned && !state.complete);
                assert!(parent.sdma.is_none() && parent.striped_sdma.is_none());
                assert_eq!(gate.observation(), (false, true));
                let calls = t.borrow().calls.clone();
                assert_eq!(calls.iter().filter(|&&c| c == name).count(), occurrence);
                let destroys = t.borrow().sdma_destroy_ids.clone();
                assert_eq!(
                    destroys,
                    (100..100 + COUNT as u32)
                        .chain([201, 200])
                        .take(destroys.len())
                        .collect::<Vec<_>>()
                );
                let resources = t.borrow().sdma_resource_ids.clone();
                assert_eq!(
                    resources,
                    before
                        .iter()
                        .flat_map(|set| set.owners.iter().rev())
                        .map(|owner| owner.identities[0].unwrap())
                        .take(resources.len())
                        .collect::<Vec<_>>()
                );
                for (set, original) in [state.striped_sdma.as_ref(), state.sdma.as_ref()]
                    .into_iter()
                    .zip(&before)
                {
                    let set = set.unwrap();
                    let observed = set.observation();
                    observed.assert_original_owners(original);
                    assert!(observed.state.2 && !set.is_complete());
                    for owner in observed.owners {
                        assert!(owner.poisoned);
                        let attempted = destroys.contains(&owner.id);
                        let failed = name == "sdma-destroy" && destroys.last() == Some(&owner.id);
                        assert_eq!(owner.attempted, attempted);
                        assert_eq!(
                            owner.request,
                            attempted
                                .then(|| fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs::new(owner.id))
                        );
                        assert_eq!(
                            owner.result,
                            if !attempted || failed && panic {
                                None
                            } else if failed {
                                Some(Err(rustix::io::Errno::IO))
                            } else {
                                Some(Ok(()))
                            }
                        );
                        assert_eq!(
                            owner.resources.is_some(),
                            resources.contains(
                                &original
                                    .owners
                                    .iter()
                                    .find(|o| o.id == owner.id)
                                    .unwrap()
                                    .identities[0]
                                    .unwrap()
                            )
                        );
                    }
                }
                if name.starts_with("sdma-") && name != "sdma-release-resources" {
                    assert!(!state.destroy.attempted && state.resources.is_none());
                }
                assert_no_retry(&mut parent, &mut state, &t);
                cleanup_state(&mut state);
            }
        }
    }
}

#[test]
fn constructed_combined_release_retains_real_late_cleanup_at_set_boundary() {
    for occurrence in [1, 14, 15, 16] {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_combined(14, false);
            let before = [parent.striped_sdma.as_ref(), parent.sdma.as_ref()]
                .map(|set| sdma_fixture::unreleased_observation(set.unwrap()));
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
            let (set, local_index) = if occurrence <= 14 {
                (state.striped_sdma.as_ref().unwrap(), occurrence)
            } else {
                (state.sdma.as_ref().unwrap(), occurrence - 14)
            };
            set.assert_late_resource_failure(
                &parent.engine.backend.session,
                t.borrow().sdma_resource_snapshot.as_ref().unwrap(),
                local_index,
                panic,
            );
            for (set, original) in [state.striped_sdma.as_ref(), state.sdma.as_ref()]
                .into_iter()
                .zip(&before)
            {
                let observed = set.unwrap().observation();
                observed.assert_original_owners(original);
                assert!(observed.owners.iter().all(|owner| owner.poisoned));
            }
            assert!(state.resources.as_ref().unwrap().is_complete());
            assert!(
                state.signals.is_none()
                    && parent.signals.is_some()
                    && parent.poisoned
                    && !state.complete
            );
            assert_eq!(gate.observation(), (false, true));
            assert_no_retry(&mut parent, &mut state, &t);
            cleanup_state(&mut state);
        }
    }
}
