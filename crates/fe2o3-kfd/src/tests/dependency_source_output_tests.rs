use crate::topology::tests::{AllocationCounter, fail_allocation_for_test};

#[test]
fn source_output_allocation_refusal_precedes_identity_burn_and_can_retry() {
    for auxiliary in [false, true] {
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert!(!take_lane_unwind_process_gate_record_v1());
        let (mut session, lane) = fixture(auxiliary);
        let mut recipe = Recipe::new();
        let before = [
            owner(&session, 0).source_rollback_snapshot_for_test(),
            owner(&session, 1).source_rollback_snapshot_for_test(),
        ];
        let dependency = session.dependency_owner.custody_snapshot_for_test();
        let dispatch = recipe.owner.cpu_snapshot();
        let (result, failed) = session
            .with_compute_lane_v1(lane, |selected| {
                fail_allocation_for_test(1, || {
                    submit(selected, &mut recipe, |_, _| {
                        panic!("output reservation refusal must precede native submission")
                    })
                })
            })
            .unwrap();
        assert!(failed);
        assert!(matches!(
            result,
            Err(
                Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                    ComputeAqlQueueSessionErrorV1::Contract(
                        "dependency source event output allocation"
                    )
                )
            )
        ));
        assert_eq!(
            session.dependency_owner.custody_snapshot_for_test(),
            dependency
        );
        assert_eq!(recipe.owner.cpu_snapshot(), dispatch);
        assert!(recipe.bound.is_none() && recipe.cancellations.is_empty());
        assert!(!session.terminal_poisoned);
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert!(!take_lane_unwind_process_gate_record_v1());
        for (ordinal, snapshot) in before.into_iter().enumerate() {
            assert_eq!(
                owner(&session, ordinal).source_rollback_snapshot_for_test(),
                snapshot
            );
        }
        restored(&session);
        let source = publish(&mut session, lane, &mut recipe, 42);
        retire(&mut session, lane, &mut recipe, source, true);
        recipe.owner.ensure_releasable().unwrap();
        owner(&session, usize::from(auxiliary))
            .ensure_releasable()
            .unwrap();
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        assert!(!take_lane_unwind_process_gate_record_v1());
    }
}

#[test]
fn source_output_preallocation_has_no_allocations_after_native_success() {
    for auxiliary in [false, true] {
        let (mut session, lane) = fixture(auxiliary);
        let mut recipe = Recipe::new();
        let mut measured = None;
        let result = session
            .with_compute_lane_v1(lane, |selected| {
                let result = submit(selected, &mut recipe, |_, packets| {
                    assert_eq!(packets.packet_count(), 3);
                    measured = Some(AllocationCounter::begin());
                    Ok(42)
                });
                let allocations = measured.take().expect("native callback ran").finish();
                (result, allocations)
            })
            .unwrap();
        assert_eq!(result.1, 0);
        let source = result.0.unwrap();
        assert_eq!(source.events.len(), 3);
        let exact = identities(&session, &source);
        for (index, (event, identity)) in source.events.iter().zip(exact).enumerate() {
            assert_eq!(event.lane, lane);
            assert_eq!(identity.packet_id, Some(40 + index as u64));
            assert_eq!(identity.acceptance_epoch, 1);
        }
        retire(&mut session, lane, &mut recipe, source, true);
        recipe.owner.ensure_releasable().unwrap();
        owner(&session, usize::from(auxiliary))
            .ensure_releasable()
            .unwrap();
        restored(&session);
    }
}

#[test]
fn source_output_reservation_preserves_guard_precedence() {
    for auxiliary in [false, true] {
        for fault in 0..5 {
            assert!(!take_dispatch_terminal_process_gate_record_v1());
            assert!(!take_lane_unwind_process_gate_record_v1());
            let (mut session, lane) = fixture(auxiliary);
            let mut recipe = Recipe::new();
            let mut attachment = if fault == 4 {
                persistent_compute_gate_test_session_v1(session.key, 3)
                    .persistent_compute
                    .take()
            } else {
                None
            };
            // Exercise operation-level guards without AUX selection's earlier guard.
            let (result, failed) = session
                .with_compute_lane_v1(lane, |selected| {
                    if fault == 0 {
                        selected.session.terminal_poisoned = true;
                    }
                    if fault == 2 {
                        selected
                            .session
                            .dependency_owner
                            .advance_to_last_acceptance_epoch_for_test();
                        selected
                            .session
                            .dependency_owner
                            .reserve_acceptance_epoch()
                            .unwrap();
                    }
                    if fault == 3 {
                        selected.session.dependency_owner.poison();
                    }
                    if fault == 4 {
                        selected.session.persistent_compute = attachment.take();
                    }
                    fail_allocation_for_test(1, || {
                        if fault == 1 {
                            selected
                                .session
                                .submit_dependency_source_using_v1::<0>(
                                    selected.lane,
                                    &mut recipe,
                                    |_, _| panic!("zero packet source must not submit"),
                                )
                                .map(|_| ())
                        } else {
                            submit(selected, &mut recipe, |_, _| {
                                panic!("preflight must not submit")
                            })
                            .map(|_| ())
                        }
                    })
                })
                .unwrap();
            assert_eq!(failed, fault == 2 || fault == 3);
            match fault {
                0 => assert!(matches!(
                    result,
                    Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(
                        ComputeAqlQueueSessionErrorV1::DispatchBinding(
                            Gfx942DispatchBindingErrorV1::Poisoned
                        )
                    ))
                )),
                1 => assert!(matches!(
                    result,
                    Err(
                        Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                            ComputeAqlQueueSessionErrorV1::Contract(
                                "dependency source packet count must be 1 through 8192"
                            )
                        )
                    )
                )),
                4 => assert!(matches!(
                    result,
                    Err(
                        Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                                Gfx942DispatchBindingErrorV1::ResourcePhase
                            )
                        )
                    )
                )),
                _ => assert!(matches!(
                    result,
                    Err(
                        Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                            ComputeAqlQueueSessionErrorV1::Contract(
                                "dependency source event output allocation"
                            )
                        )
                    )
                )),
            }
            assert!(recipe.bound.is_none() && recipe.cancellations.is_empty());
            assert_eq!(recipe.owner.next_generation(), 1);
            assert_eq!(take_dispatch_terminal_process_gate_record_v1(), fault == 0);
            assert!(!take_lane_unwind_process_gate_record_v1());
        }
    }
}
