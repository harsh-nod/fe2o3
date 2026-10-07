//! Late joined failures retain earlier native receipts and the original suffix.

use super::*;
use crate::shared_memory::CleanupStageV1 as Stage;

fn assert_currentness_failure(
    result: std::thread::Result<Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1>>,
    panic: bool,
) {
    if panic {
        assert_eq!(
            result.unwrap_err().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "currentness"))
        );
    } else {
        assert!(matches!(
            result.unwrap(),
            Err(ComputeAqlQueueSessionErrorV1::Memory(
                MemorySessionError::Injected("currentness")
            ))
        ));
    }
}

#[test]
fn constructed_primary_release_late_resource_currentness_keeps_all_original_suffixes() {
    for point in 1..=24 {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            let dispatch =
                RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
            let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
            t.borrow_mut().queue_release_currentness_fault = Some((point, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            assert_currentness_failure(result, panic);
            assert!(state.destroy.attempted && state.destroy.returned.is_some());
            assert!(state.authority.is_none() && !parent.engine.backend.foundation_in_engine);
            parent
                .engine
                .backend
                .session
                .primary_assert_permanently_restored_v1(&parent.engine.foundation);
            t.borrow_mut()
                .release_snapshot
                .take()
                .unwrap()
                .assert_constructed_queue_currentness_v1(
                    &parent.engine.backend.session,
                    state.resources.as_ref().unwrap(),
                    point,
                );
            dispatch.assert_restored_v1(
                RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap()),
                true,
            );
            assert_eq!(
                Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
                signal_id
            );
            assert!(
                state.dispatch.is_none()
                    && state.signals.is_none()
                    && !state.complete
                    && parent.poisoned
            );
            assert!(!t.borrow().calls.contains(&"complete-shadows"));
            assert!(state.gate.is_some());
            assert_eq!(gate.teardown_count(), 1);
            assert!(gate.observation().1);
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}

fn assert_signal_suffix(
    parent: &Parent,
    state: &PrimaryReleaseStateV1<Fixture>,
    gate: &LocalGateV1,
) {
    assert!(state.resources.as_ref().unwrap().is_complete());
    assert!(state.dispatch.as_ref().unwrap().is_complete());
    assert!(
        parent.dispatch.is_none() && parent.signals.is_none() && !state.complete && parent.poisoned
    );
    assert!(state.gate.is_some() && state.signals.is_some());
    assert_eq!(gate.teardown_count(), 1);
    assert!(gate.observation().1);
    assert!(!parent.engine.backend.foundation_in_engine);
    parent
        .engine
        .backend
        .session
        .primary_assert_permanently_restored_v1(&parent.engine.foundation);
}

#[test]
fn constructed_primary_release_signal_currentness_preserves_unsettled_disposal_receipts() {
    for point in 1..=6 {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
            t.borrow_mut().signal_currentness_fault = Some((point, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            assert_currentness_failure(result, panic);
            assert_signal_suffix(&parent, &state, &gate);
            assert_eq!(
                state.signals.as_ref().unwrap().observation().identity,
                signal_id
            );
            t.borrow_mut()
                .signal_snapshot
                .take()
                .unwrap()
                .assert_constructed_signal_currentness_v1(
                    &parent.engine.backend.session,
                    state.signals.as_ref().unwrap(),
                    point,
                );
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}

#[test]
fn constructed_primary_release_signal_model_boundaries_keep_refunds_and_model_commits_separate() {
    for stage in [
        Stage::UnmapPreflight,
        Stage::UnmapEvidence,
        Stage::UnmapProjection,
        Stage::UnmapCommit,
        Stage::ReleasePreflight,
        Stage::ReleaseEvidence,
        Stage::ReleaseProjection,
        Stage::ReleaseCommit,
    ] {
        for panic in [false, true] {
            let (mut parent, t, gate) = constructed(true);
            let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
            t.borrow_mut().signal_projection_fault = Some((stage, panic));
            let mut state = PrimaryReleaseStateV1::<Fixture>::new();
            let result = catch_unwind(AssertUnwindSafe(|| state.release_in_place(&mut parent)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                    Some(&("control cleanup projection", stage))
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(ComputeAqlQueueSessionErrorV1::Memory(
                        MemorySessionError::Injected("control cleanup projection")
                    ))
                ));
            }
            assert_signal_suffix(&parent, &state, &gate);
            assert_eq!(
                state.signals.as_ref().unwrap().observation().identity,
                signal_id
            );
            t.borrow_mut()
                .signal_snapshot
                .take()
                .unwrap()
                .assert_constructed_signal_projection_v1(
                    &parent.engine.backend.session,
                    state.signals.as_ref().unwrap(),
                    stage,
                    panic,
                );
            assert_no_retry(&mut parent, &mut state, &t);
        }
    }
}
