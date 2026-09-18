use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn with_sdma(dispatch: bool) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    with_sdma_profile(
        dispatch,
        crate::sdma::retained_release::fixture::directional,
    )
}

pub(super) fn with_sdma_profile(
    dispatch: bool,
    create: impl FnOnce(&mut Memory, QueueKeyV1) -> Gfx942SdmaQueueSetV1,
) -> (Parent, Rc<RefCell<Trace>>, LocalGateV1) {
    let (mut parent, t, gate) = constructed(dispatch);
    let engine = &mut parent.engine;
    let loan = engine
        .backend
        .session
        .primary_loan(&mut engine.foundation)
        .unwrap();
    parent.sdma = Some(create(&mut engine.backend.session, parent.key));
    engine
        .backend
        .session
        .primary_reclaim(&mut engine.foundation, loan)
        .unwrap();
    parent.preflight_release().unwrap();
    t.borrow_mut().calls.clear();
    (parent, t, gate)
}

#[test]
fn constructed_directional_release_orders_original_owners_and_refunds_all_backing() {
    for dispatch in [false, true] {
        let (mut parent, t, gate) = with_sdma(dispatch);
        let mut state = PrimaryReleaseStateV1::<Fixture>::new();
        let destroyed = state.release_in_place(&mut parent).unwrap();
        assert_eq!(destroyed.released_resources, 11);
        assert!(parent.sdma.is_none());
        assert!(state.sdma.as_ref().unwrap().is_complete());
        assert!(state.complete);
        parent
            .engine
            .backend
            .session
            .primary_assert_all_released_v1();
        let calls = t.borrow().calls.clone();
        let position = |name| calls.iter().position(|&c| c == name).unwrap();
        assert!(position("sdma-destroy-h2d") < position("sdma-destroy-d2h"));
        assert!(position("sdma-destroy-d2h") < position("destroy"));
        assert!(position("release-resources") < position("sdma-release-resources"));
        assert!(position("sdma-release-resources") < position("release-signals"));
        assert_eq!(
            calls
                .iter()
                .filter(|&&c| c == "sdma-release-resources")
                .count(),
            2
        );
        assert_eq!(gate.observation(), (false, false));
        assert_no_retry(&mut parent, &mut state, &t);
        state.sdma.as_mut().unwrap().cleanup_local_mappings();
    }
}

#[test]
fn constructed_directional_release_failures_keep_prefix_and_do_not_complete_primary() {
    for (name, count) in [
        ("sdma-topology", 2),
        ("sdma-currentness", 4),
        ("sdma-destroy-h2d", 1),
        ("sdma-destroy-d2h", 1),
        ("sdma-doorbell", 2),
        ("sdma-release-resources", 2),
    ] {
        for occurrence in 1..=count {
            for panic in [false, true] {
                let (mut parent, t, gate) = with_sdma(false);
                t.borrow_mut().fault = Some((name, occurrence, panic));
                let mut state = PrimaryReleaseStateV1::<Fixture>::new();
                let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, usize)>(),
                        Some(&(name, occurrence))
                    );
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert!(parent.poisoned && !state.complete);
                assert!(parent.engine.backend.session.primary_is_quarantined_v1());
                assert!(parent.sdma.is_none());
                assert!(state.sdma.is_some());
                assert!(parent.signals.is_some());
                let observed = state.sdma.as_ref().unwrap().observation();
                let completed = match (name, occurrence) {
                    ("sdma-topology", 1)
                    | ("sdma-doorbell", 1)
                    | ("sdma-currentness", 1 | 2)
                    | ("sdma-destroy-h2d", _) => 0,
                    ("sdma-currentness", 3 | 4)
                    | ("sdma-destroy-d2h", _)
                    | ("sdma-doorbell", 2) => 1,
                    _ => 2,
                };
                assert_eq!(
                    observed.state,
                    (
                        true,
                        name == "sdma-release-resources",
                        true,
                        completed,
                        if name == "sdma-release-resources" {
                            occurrence - 1
                        } else {
                            0
                        }
                    )
                );
                for (index, owner) in observed.owners.iter().enumerate() {
                    let call = if index == 1 {
                        "sdma-destroy-h2d"
                    } else {
                        "sdma-destroy-d2h"
                    };
                    let attempted = t.borrow().calls.contains(&call);
                    let failed = name == call;
                    assert_eq!(owner.attempted, attempted);
                    assert_eq!(
                        owner.request,
                        attempted.then(|| fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs::new(
                            100 + index as u32
                        ))
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
                    let doorbell_failed = name == "sdma-doorbell" && occurrence == 2 - index;
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
                }
                assert_eq!(gate.observation(), (false, true));
                if name != "sdma-release-resources" {
                    assert!(!state.destroy.attempted);
                    assert!(state.resources.is_none());
                }
                assert_no_retry(&mut parent, &mut state, &t);
                state.sdma.as_mut().unwrap().cleanup_local_mappings();
            }
        }
    }
}

#[test]
fn constructed_directional_release_retains_mutated_destroy_argument_and_raw_outcome() {
    for id in [101, 100] {
        for panic in [false, true] {
            let (mut parent, t, _gate) = with_sdma(false);
            t.borrow_mut().sdma_destroy_mutation = Some((id, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, u32)>(),
                    Some(&("mutated SDMA destroy", id))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            let observed = state.sdma.as_ref().unwrap().observation();
            assert_eq!(observed.state.3, usize::from(id == 100));
            let owner = &observed.owners[(id - 100) as usize];
            assert_eq!(
                owner.request,
                Some(fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs {
                    queue_id: id,
                    pad: 17
                })
            );
            assert!(owner.attempted && owner.poisoned);
            assert_eq!(owner.result, (!panic).then_some(Ok(())));
            assert!(!owner.destroyed && !owner.doorbell.started);
            assert!(!state.destroy.attempted);
            assert!(parent.engine.backend.session.primary_is_quarantined_v1());
            assert_no_retry(&mut parent, &mut state, &t);
            state.sdma.as_mut().unwrap().cleanup_local_mappings();
        }
    }
}

#[test]
fn constructed_directional_release_rejects_bad_second_owner_before_any_effect() {
    for case in 0..9 {
        let (mut parent, t, gate) = with_sdma(false);
        crate::sdma::retained_release::fixture::corrupt_preflight(
            parent.sdma.as_mut().unwrap(),
            case,
            parent.queue_id,
        );
        let before = parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation);
        assert!(parent.preflight_release().is_err());
        assert!(t.borrow().calls.is_empty());
        assert_eq!(gate.teardown_count(), 0);
        assert!(!parent.poisoned);
        assert_eq!(
            parent
                .engine
                .backend
                .session
                .primary_release_memory_snapshot_v1(&parent.engine.foundation),
            before
        );
        crate::sdma::retained_release::fixture::cleanup_set(parent.sdma.as_mut().unwrap());
    }
}

#[test]
fn constructed_directional_release_late_native_resource_failure_keeps_real_prefix() {
    for occurrence in 1..=2 {
        for panic in [false, true] {
            let (mut parent, t, gate) = with_sdma(false);
            t.borrow_mut().sdma_resource_native_fault = Some((occurrence, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "release_va_reservation"))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            state.sdma.as_ref().unwrap().assert_late_resource_failure(
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
