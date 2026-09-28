//! Production receipt-cell code with lower typed CPU receipts, not GPU execution.

use super::super::runtime_materialized_submission_attempt::MaterializedSubmissionAttemptV1 as Attempt;
use super::*;

type DispatchOwner = super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1;

pub(super) fn fixture(auxiliary: bool) -> (ComputeAqlQueueSessionV1, ComputeAqlQueueLaneV1) {
    let primary = test_queue_key(186, 1);
    let mut session = persistent_compute_cancellation_test_session(primary, None, None);
    session
        .auxiliary_compute_lanes
        .push(AuxiliaryComputeLaneSlotV1 {
            generation: 7,
            state: Some(compute_lane_state_for_multi_inflight_test(test_queue_key(
                187, 1,
            ))),
        });
    let lane = if auxiliary {
        ComputeAqlQueueLaneV1 {
            session: primary,
            ordinal: 1,
            generation: 7,
        }
    } else {
        session.primary_compute_lane_v1()
    };
    (session, lane)
}

pub(super) fn owner(
    session: &ComputeAqlQueueSessionV1,
    ordinal: usize,
) -> &CompletionSignalArenaOwnerV1 {
    if ordinal == 0 {
        &session.completion_owner
    } else {
        &session.auxiliary_compute_lanes[0]
            .state
            .as_ref()
            .unwrap()
            .completion_owner
    }
}

pub(super) fn restored(session: &ComputeAqlQueueSessionV1) {
    assert_eq!(session.key, test_queue_key(186, 1));
    assert_eq!(session.auxiliary_compute_lanes[0].generation, 7);
    assert_eq!(
        session.auxiliary_compute_lanes[0]
            .state
            .as_ref()
            .unwrap()
            .key,
        test_queue_key(187, 1)
    );
}

fn submit(
    selected: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    dispatch: &mut DispatchOwner,
    cell: &mut Attempt,
    packet: u64,
) -> Result<(), Gfx942FixedDispatchSubmissionFailureV1> {
    let queue = selected.session.key;
    cell.submit_classified(|| {
        selected
            .session
            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                dispatch,
                |generation| test_completion_template(queue, generation),
                |_, packets| {
                    assert_eq!(packets.packet_count(), 1);
                    Ok(packet)
                },
            )
            .map_err(FixedDispatchSubmissionFailureV1::into_public)
    })
}

fn recycle(
    selected: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    dispatch: &mut DispatchOwner,
    cell: Attempt,
) {
    let Attempt::Published(batch) = cell else {
        panic!("real published receipt required")
    };
    let completed = selected
        .session
        .complete_fixed_dispatch_with_multi_inflight_test_owner(dispatch, batch)
        .unwrap();
    let observation = selected
        .session
        .recycle_fixed_dispatch_with_multi_inflight_test_owner(dispatch, completed)
        .unwrap();
    assert_eq!(observation.packet_count(), 1);
}

#[test]
fn runtime_publication_cell_retains_real_batches_through_out_of_order_recycle() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let available = owner(&session, ordinal).state_snapshot_for_test().1;
        let mut dispatch = DispatchOwner::new();
        let mut cells = [
            Attempt::Unattempted,
            Attempt::Unattempted,
            Attempt::Unattempted,
        ];
        for (index, cell) in cells.iter_mut().enumerate() {
            session
                .with_compute_lane_v1(lane, |selected| {
                    submit(selected, &mut dispatch, cell, 41 + index as u64)
                })
                .unwrap()
                .unwrap();
            restored(&session);
            assert!(matches!(cell, Attempt::Published(_)));
            assert_eq!(dispatch.live_epoch_count(), index + 1);
            assert_eq!(dispatch.next_generation(), index as u64 + 2);
            assert_eq!(
                owner(&session, ordinal).state_snapshot_for_test().1,
                available - index - 1
            );
            assert_eq!(
                owner(&session, 1 - ordinal).custody_snapshot_for_test(),
                untouched
            );
        }
        let [a, b, c] = cells;
        for (index, cell) in [b, c, a].into_iter().enumerate() {
            session
                .with_compute_lane_v1(lane, |selected| recycle(selected, &mut dispatch, cell))
                .unwrap();
            assert_eq!(dispatch.live_epoch_count(), 2 - index);
            assert_eq!(
                owner(&session, ordinal).state_snapshot_for_test().1,
                available - 2 + index
            );
        }
        dispatch.ensure_releasable().unwrap();
        assert!(!session.terminal_poisoned);
        restored(&session);
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
    }
}

#[test]
fn runtime_publication_cell_signal_capacity_retry_preserves_custody_then_reuses() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let available = owner(&session, ordinal).state_snapshot_for_test().1;
        let full = session
            .with_compute_lane_v1(lane, |selected| {
                let queue = selected.session.key;
                selected
                    .session
                    .completion_owner
                    .fill_all_signals_for_test(test_completion_template(queue, 9))
            })
            .unwrap();
        let snapshot = owner(&session, ordinal).custody_snapshot_for_test();
        let mut dispatch = DispatchOwner::new();
        let mut cell = Attempt::Unattempted;
        session
            .with_compute_lane_v1(lane, |selected| {
                let queue = selected.session.key;
                cell.submit_classified(|| {
                    selected
                        .session
                        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                            &mut dispatch,
                            |generation| test_completion_template(queue, generation),
                            |_, _| panic!("full signals must refuse before native submit"),
                        )
                        .map_err(FixedDispatchSubmissionFailureV1::into_public)
                })
            })
            .unwrap()
            .unwrap();
        assert!(matches!(cell, Attempt::Retryable));
        assert_eq!(dispatch.live_epoch_count(), 0);
        assert_eq!(dispatch.next_generation(), 2);
        assert_eq!(
            owner(&session, ordinal).custody_snapshot_for_test(),
            snapshot
        );
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        assert!(!session.terminal_poisoned);
        session
            .with_compute_lane_v1(lane, |selected| {
                selected
                    .session
                    .completion_owner
                    .complete_and_recycle_all_for_test(full)
            })
            .unwrap();
        session
            .with_compute_lane_v1(lane, |selected| {
                submit(selected, &mut dispatch, &mut cell, 51)
            })
            .unwrap()
            .unwrap();
        assert!(matches!(cell, Attempt::Published(_)));
        assert_eq!(dispatch.live_epoch_count(), 1);
        assert_eq!(dispatch.next_generation(), 3);
        session
            .with_compute_lane_v1(lane, |selected| recycle(selected, &mut dispatch, cell))
            .unwrap();
        dispatch.ensure_releasable().unwrap();
        assert_eq!(
            owner(&session, ordinal).state_snapshot_for_test().1,
            available
        );
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        assert!(!session.terminal_poisoned);
        restored(&session);
    }
}

#[test]
fn runtime_publication_cell_rejection_is_not_a_retry() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let before = [
            owner(&session, 0).custody_snapshot_for_test(),
            owner(&session, 1).custody_snapshot_for_test(),
        ];
        let mut cell = Attempt::Unattempted;
        let failure = session
            .with_compute_lane_v1(lane, |selected| {
                cell.submit_classified(|| selected.submit_fixed_dispatch_classified_v1::<1>())
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )
            )
        ));
        assert!(matches!(cell, Attempt::NativeOwned));
        assert_eq!(
            [
                owner(&session, 0).custody_snapshot_for_test(),
                owner(&session, 1).custody_snapshot_for_test()
            ],
            before
        );
        assert!(!session.terminal_poisoned);
        restored(&session);
    }
}

#[test]
fn runtime_publication_cell_typed_terminal_retains_consuming_state() {
    for auxiliary in [false, true] {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let (mut session, lane) = fixture(auxiliary);
        let ordinal = usize::from(auxiliary);
        let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
        let available = owner(&session, ordinal).state_snapshot_for_test().1;
        let mut dispatch = DispatchOwner::new();
        let mut cell = Attempt::Unattempted;
        let failure = session
            .with_compute_lane_v1(lane, |selected| {
                let queue = selected.session.key;
                cell.submit_classified(|| {
                    selected
                        .session
                        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                            &mut dispatch,
                            |generation| test_completion_template(queue, generation),
                            |_, _| {
                                Err(NativeAqlSubmissionFailureV1::Terminal(
                                    NativeAqlSubmissionErrorV1::CallbackPanic,
                                ))
                            },
                        )
                        .map_err(FixedDispatchSubmissionFailureV1::into_public)
                })
            })
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            failure,
            Gfx942FixedDispatchSubmissionFailureV1::Terminal(
                ComputeAqlQueueSessionErrorV1::Native("submission callback panic")
            )
        ));
        assert!(matches!(cell, Attempt::NativeOwned));
        assert!(take_dispatch_terminal_process_gate_record_v1());
        assert!(session.terminal_poisoned);
        assert!(owner(&session, ordinal).is_poisoned_for_test());
        assert_eq!(dispatch.live_epoch_count(), 1);
        assert_eq!(dispatch.next_generation(), 2);
        assert_eq!(
            owner(&session, ordinal).state_snapshot_for_test().1,
            available - 1
        );
        assert_eq!(
            owner(&session, 1 - ordinal).custody_snapshot_for_test(),
            untouched
        );
        let retained = owner(&session, ordinal).custody_snapshot_for_test();
        assert!(
            session
                .with_compute_lane_v1(lane, |_| panic!("terminal lane must not reenter"))
                .is_err()
        );
        assert_eq!(
            owner(&session, ordinal).custody_snapshot_for_test(),
            retained
        );
        restored(&session);
        // This CPU fixture owns no device resources. Do not manufacture recycle
        // or clear poison to claim healthy native cleanup after terminal failure.
    }
}

#[test]
fn runtime_publication_cell_unwind_preserves_exact_returned_or_consuming_state() {
    for auxiliary in [false, true] {
        for outcome in 0..3 {
            assert!(!take_lane_unwind_process_gate_record_v1());
            let (mut session, lane) = fixture(auxiliary);
            let ordinal = usize::from(auxiliary);
            let untouched = owner(&session, 1 - ordinal).custody_snapshot_for_test();
            let mut dispatch = DispatchOwner::new();
            let mut cell = Attempt::Unattempted;
            let mut retained = None;
            let full = if outcome == 1 {
                Some(
                    session
                        .with_compute_lane_v1(lane, |selected| {
                            let queue = selected.session.key;
                            selected
                                .session
                                .completion_owner
                                .fill_all_signals_for_test(test_completion_template(queue, 9))
                        })
                        .unwrap(),
                )
            } else {
                None
            };
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = session.with_compute_lane_v1(lane, |selected| {
                    let queue = selected.session.key;
                    cell.submit_classified(|| {
                        selected
                            .session
                            .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                                &mut dispatch,
                                |generation| test_completion_template(queue, generation),
                                |session, _| {
                                    assert_ne!(
                                        outcome, 1,
                                        "full arena cannot call native submission"
                                    );
                                    if outcome == 2 {
                                        retained = Some(
                                            session.completion_owner.custody_snapshot_for_test(),
                                        );
                                        std::panic::panic_any("runtime-cell-native-unwind");
                                    }
                                    Ok(61)
                                },
                            )
                            .map_err(FixedDispatchSubmissionFailureV1::into_public)
                    })
                    .unwrap();
                    retained = Some(
                        selected
                            .session
                            .completion_owner
                            .custody_snapshot_for_test(),
                    );
                    std::panic::panic_any("runtime-cell-returned-unwind");
                });
            }))
            .unwrap_err();
            assert_eq!(
                panic.downcast_ref::<&str>(),
                Some(&if outcome == 2 {
                    "runtime-cell-native-unwind"
                } else {
                    "runtime-cell-returned-unwind"
                })
            );
            match outcome {
                0 => assert!(matches!(cell, Attempt::Published(_))),
                1 => assert!(matches!(cell, Attempt::Retryable)),
                2 => assert!(matches!(cell, Attempt::NativeOwned)),
                _ => unreachable!(),
            }
            assert_eq!(dispatch.live_epoch_count(), usize::from(outcome != 1));
            assert_eq!(dispatch.next_generation(), 2);
            assert!(take_lane_unwind_process_gate_record_v1());
            assert!(session.terminal_poisoned);
            assert!(owner(&session, 0).is_poisoned_for_test());
            restored(&session);
            assert_eq!(
                owner(&session, ordinal).custody_snapshot_for_test(),
                retained.unwrap()
            );
            assert_eq!(
                owner(&session, 1 - ordinal).custody_snapshot_for_test(),
                untouched
            );
            assert!(
                session
                    .with_compute_lane_v1(lane, |_| panic!("terminal lane must not reenter"))
                    .is_err()
            );
            assert!(session.destroy().is_err());
            drop((cell, full));
        }
    }
}

#[test]
fn runtime_publication_cell_adapters_use_shared_classifier() {
    for source in [
        include_str!("../../../fe2o3-runtime/src/kfd_backend/materialized_publication.rs"),
        include_str!("../../../fe2o3-runtime/src/kfd_backend/ordered_publication.rs"),
    ] {
        assert!(
            source.contains(
                ".submit_classified(|| queue.submit_fixed_dispatch_classified_v1::<1>())"
            )
        );
        assert!(!source.contains("match queue.submit_fixed_dispatch_classified_v1::<1>()"));
    }
}
