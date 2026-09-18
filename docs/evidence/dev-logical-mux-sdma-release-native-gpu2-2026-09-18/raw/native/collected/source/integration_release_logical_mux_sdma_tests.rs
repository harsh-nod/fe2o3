use super::*;
use crate::sdma::retained_release::{
    fixture as sdma_fixture, supports_retained_sdma_composition_v1,
};

const LANES: [u8; 5] = [2, 4, 8, 14, 16];

fn with_logical_mux(
    lanes: u8,
    cursor: u8,
    dispatch: bool,
) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    sdma_cases::with_sdma_profile(dispatch, |memory, key| {
        let mut expected = memory.observation().host.unwrap();
        let set = sdma_fixture::logical_mux(memory, key, lanes, cursor);
        expected.used_backing_bytes += 2 * 4096;
        expected.used_allocation_records += 2;
        expected.retained_records += 2;
        assert_eq!(memory.observation().host, Some(expected));
        set
    })
}

#[test]
fn constructed_logical_mux_release_rejects_malformed_rosters_and_metadata_without_effects() {
    for (count, lanes, cursor) in [
        (0, 4, 0),
        (1, 4, 0),
        (3, 4, 0),
        (16, 4, 0),
        (2, 0, 0),
        (2, 1, 0),
        (2, 3, 0),
        (2, 6, 0),
        (2, 15, 0),
        (2, 17, 0),
        (2, 255, 0),
    ]
    .into_iter()
    .chain(
        LANES
            .into_iter()
            .flat_map(|lanes| [(2, lanes, lanes), (2, lanes, 255)]),
    ) {
        let (mut parent, t, gate) =
            sdma_cases::with_unchecked_sdma_profile(false, |memory, key| {
                let Gfx942SdmaQueueSetV1::Striped { owners, .. } =
                    sdma_fixture::striped(memory, key, count, 0)
                else {
                    unreachable!()
                };
                Gfx942SdmaQueueSetV1::LogicalMuxV2 {
                    owners,
                    logical_lane_count: lanes,
                    next_logical_lane: cursor,
                }
            });
        let before = sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap());
        assert!(matches!(
            supports_retained_sdma_composition_v1(parent.sdma.as_ref(), None),
            Err(Gfx942SdmaErrorV1::Contract("logical mux SDMA owner roster"))
        ));
        assert_eq!(
            sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap()),
            before
        );
        striped_sdma_cases::assert_preflight_rejected(&parent, &t, &gate);
        assert!(matches!(
            parent.preflight_release(),
            Err(ComputeAqlQueueSessionErrorV1::Sdma(
                Gfx942SdmaErrorV1::Contract("logical mux SDMA owner roster")
            ))
        ));
        sdma_fixture::cleanup_set(parent.sdma.as_mut().unwrap());
    }
}

#[test]
fn constructed_logical_mux_release_checks_every_field_on_both_physical_owners() {
    let (mut parent, t, gate) = with_logical_mux(16, 15, false);
    let original = sdma_fixture::creation_observation(parent.sdma.as_ref().unwrap());
    for index in 0..2 {
        for case in 0..22 {
            let set = parent.sdma.take().unwrap();
            parent.sdma = Some(sdma_fixture::with_owner_corruption(
                set,
                index,
                case,
                parent.queue_id,
                |set| {
                    parent.sdma = Some(set);
                    striped_sdma_cases::assert_preflight_rejected(&parent, &t, &gate);
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
    }
    sdma_fixture::cleanup_set(parent.sdma.as_mut().unwrap());
}

#[test]
fn constructed_logical_mux_release_orders_two_physical_owners_for_all_lane_counts() {
    for lanes in LANES {
        for cursor in [0, lanes - 1] {
            for dispatch in [false, true] {
                let (mut parent, t, gate) = with_logical_mux(lanes, cursor, dispatch);
                let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
                assert!(supports_retained_sdma_composition_v1(parent.sdma.as_ref(), None).unwrap());
                assert_eq!(
                    sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap()),
                    before
                );
                let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                let result = state.release_in_place(&mut parent).unwrap();
                assert_eq!(result.released_resources, 11);
                assert!(parent.sdma.is_none() && parent.striped_sdma.is_none() && state.complete);
                let sdma = state.sdma.as_ref().unwrap();
                assert!(sdma.is_complete());
                let observed = sdma.observation();
                observed.assert_original_owners(&before);
                assert_eq!(observed.state, (true, true, false, 2, 2));
                assert_eq!(t.borrow().sdma_destroy_ids, [100, 101]);
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
                let count = |name| calls.iter().filter(|&&c| c == name).count();
                assert_eq!(count("sdma-topology"), 2);
                assert_eq!(count("sdma-currentness"), 4);
                assert_eq!(count("sdma-doorbell"), 2);
                assert_eq!(count("sdma-release-resources"), 2);
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
}

#[test]
fn constructed_logical_mux_release_retains_every_callback_failure_prefix() {
    for lanes in LANES {
        striped_sdma_cases::check_callback_failure_prefixes(2, || {
            with_logical_mux(lanes, lanes - 1, false)
        });
    }
}

#[test]
fn constructed_logical_mux_release_retains_mutated_destroy_inputs_on_both_owners() {
    for lanes in LANES {
        striped_sdma_cases::check_mutated_destroy_inputs(&[0, 1], || {
            with_logical_mux(lanes, lanes - 1, false)
        });
    }
}

#[test]
fn constructed_logical_mux_release_keeps_real_late_cleanup_prefix_for_both_owners() {
    for lanes in LANES {
        striped_sdma_cases::check_late_cleanup_prefixes(2, || {
            with_logical_mux(lanes, lanes - 1, false)
        });
    }
}

#[test]
fn constructed_logical_mux_release_poison_retains_completed_siblings_on_primary_failure() {
    for name in [
        "destroy",
        "release-resources",
        "complete-shadows",
        "release-signals",
    ] {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_logical_mux(16, 15, false);
            let before = sdma_fixture::unreleased_observation(parent.sdma.as_ref().unwrap());
            if name == "destroy" {
                t.borrow_mut().destroy = Some(if panic { 3 } else { 2 });
            } else {
                t.borrow_mut().fault = Some((name, 1, panic));
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
                    Some(&(name, 1))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(parent.poisoned && !state.complete && parent.sdma.is_none());
            assert_eq!(gate.observation(), (false, true));
            let sdma = state.sdma.as_ref().unwrap();
            let observed = sdma.observation();
            observed.assert_original_owners(&before);
            let resources_done = name == "release-signals";
            assert_eq!(
                observed.state,
                (
                    true,
                    resources_done,
                    true,
                    2,
                    if resources_done { 2 } else { 0 }
                )
            );
            assert!(!sdma.is_complete());
            assert_eq!(t.borrow().sdma_destroy_ids, [100, 101]);
            assert_eq!(
                t.borrow().sdma_resource_ids.len(),
                if resources_done { 2 } else { 0 }
            );
            assert!(observed.owners.iter().all(|owner| owner.poisoned
                && owner.destroyed
                && owner.attempted
                && owner.result == Some(Ok(()))
                && owner.resources.is_some() == resources_done));
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}
