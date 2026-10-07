use super::*;

#[test]
fn hostile_quarantine_failure_preserves_each_exact_persistent_phase() {
    let owner = |id| {
        Gfx942PersistentDeviceAllocationV1::from_local_mapping(
            crate::shared_memory::local_mapping_for_persistent_sdma_test(id),
        )
    };
    let request = |byte_len| {
        Gfx942PersistentUseRequestV1::new(
            Gfx942PersistentOperationV1::ComputeReadWrite,
            0,
            byte_len,
        )
        .unwrap()
    };
    let mut foreign = owner(0x5220);

    let mut prepared_owner = owner(0x5221);
    let reserved = prepared_owner
        .reserve(request(prepared_owner.byte_len()), None)
        .unwrap();
    let prepared = prepared_owner.prepare(reserved).unwrap();
    assert!(matches!(
        quarantine_persistent_compute_prepared_v1(
            &mut foreign,
            prepared,
            Gfx942PersistentQuarantineReasonV1::CallerReportedPublicationIndeterminate,
        ),
        PersistentComputeUseStateV1::Prepared(_)
    ));

    let mut published_owner = owner(0x5222);
    let reserved = published_owner
        .reserve(request(published_owner.byte_len()), None)
        .unwrap();
    let prepared = published_owner.prepare(reserved).unwrap();
    let published = published_owner.publish(prepared).unwrap();
    assert!(matches!(
        quarantine_persistent_compute_published_v1(
            &mut foreign,
            published,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        ),
        PersistentComputeUseStateV1::Published(_)
    ));

    let mut completed_owner = owner(0x5223);
    let reserved = completed_owner
        .reserve(request(completed_owner.byte_len()), None)
        .unwrap();
    let prepared = completed_owner.prepare(reserved).unwrap();
    let published = completed_owner.publish(prepared).unwrap();
    let completed = completed_owner.complete(published).unwrap();
    assert!(matches!(
        quarantine_persistent_compute_completed_v1(
            &mut foreign,
            completed,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        ),
        PersistentComputeUseStateV1::Completed(_)
    ));

    let mut recycled_owner = owner(0x5224);
    let reserved = recycled_owner
        .reserve(request(recycled_owner.byte_len()), None)
        .unwrap();
    let prepared = recycled_owner.prepare(reserved).unwrap();
    let published = recycled_owner.publish(prepared).unwrap();
    let completed = recycled_owner.complete(published).unwrap();
    assert!(matches!(
        quarantine_persistent_compute_recycled_v1(
            &mut foreign,
            completed,
            Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
        ),
        PersistentComputeUseStateV1::Recycled(_)
    ));
}

#[test]
fn shared_persistent_ledger_core_is_equivalent_for_one_and_three_entries() {
    let mut single = prepared_test_persistent_ledger_entry_v1(0x5310);
    let [mut a, mut b, mut c] =
        [0x5311, 0x5312, 0x5313].map(prepared_test_persistent_ledger_entry_v1);

    assert!(publish_persistent_compute_entries_v1([&mut single]));
    assert!(publish_persistent_compute_entries_v1([
        &mut a, &mut b, &mut c
    ]));
    assert!(matches!(
        single.state,
        PersistentComputeUseStateV1::Published(_)
    ));
    assert!(
        [&a, &b, &c]
            .into_iter()
            .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
    );

    assert!(complete_persistent_compute_entries_v1([&mut single]));
    assert!(complete_persistent_compute_entries_v1([
        &mut a, &mut b, &mut c
    ]));
    assert!(recycle_persistent_compute_entries_v1([&mut single]));
    assert!(recycle_persistent_compute_entries_v1([
        &mut a, &mut b, &mut c
    ]));
    quarantine_persistent_compute_entries_v1(
        [&mut single],
        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
    );
    quarantine_persistent_compute_entries_v1(
        [&mut a, &mut b, &mut c],
        Gfx942PersistentQuarantineReasonV1::CallerReportedCompletionIndeterminate,
    );
    assert!(matches!(
        single.state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(
        [&a, &b, &c]
            .into_iter()
            .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Quarantined))
    );
    let quarantined_live_uses = single.owner.live_use_count();
    assert!(
        [&a, &b, &c]
            .into_iter()
            .all(|entry| entry.owner.live_use_count() == quarantined_live_uses)
    );

    let mut single_cancel = prepared_test_persistent_ledger_entry_v1(0x5314);
    let [mut cancel_a, mut cancel_b, mut cancel_c] =
        [0x5315, 0x5316, 0x5317].map(prepared_test_persistent_ledger_entry_v1);
    assert!(cancel_persistent_compute_prepublication_entries_v1([
        &mut single_cancel
    ]));
    assert!(cancel_persistent_compute_prepublication_entries_v1([
        &mut cancel_a,
        &mut cancel_b,
        &mut cancel_c,
    ]));
    assert_eq!(single_cancel.owner.live_use_count(), 0);
    assert!(
        [&cancel_a, &cancel_b, &cancel_c]
            .into_iter()
            .all(|entry| entry.owner.live_use_count() == 0)
    );
}

#[test]
fn mocked_facade_retains_and_recycles_a_b_c_on_one_immutable_wait_for_prior_recipe() {
    let queue = test_queue_key(172, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
    let (a, b, c) = {
        let mut submit = |packet_id| {
            session
                .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                    &mut dispatch,
                    |generation| {
                        let template = test_completion_template(queue, generation);
                        assert_eq!(
                            template.ordering_for_test(),
                            fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior
                        );
                        template
                    },
                    |_, packets| {
                        assert_eq!(packets.packet_count(), 1);
                        Ok(packet_id)
                    },
                )
                .unwrap()
        };
        (submit(41), submit(42), submit(43))
    };
    assert_eq!(dispatch.live_epoch_count(), 3);
    let b = session
        .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, b)
        .unwrap();
    session
        .recycle_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, b)
        .unwrap();
    assert_eq!(dispatch.live_epoch_count(), 2);
    let c = session
        .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, c)
        .unwrap();
    session
        .recycle_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, c)
        .unwrap();
    let a = session
        .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, a)
        .unwrap();
    session
        .recycle_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, a)
        .unwrap();
    assert_eq!(dispatch.live_epoch_count(), 0);
    assert!(!session.terminal_poisoned);
}

#[test]
fn public_auxiliary_lane_unwind_restores_lane_then_terminalizes_session() {
    assert!(!take_lane_unwind_process_gate_record_v1());
    let primary = test_queue_key(175, 1);
    let auxiliary = test_queue_key(176, 1);
    let mut session = persistent_compute_cancellation_test_session(primary, None, None);
    session
        .auxiliary_compute_lanes
        .push(AuxiliaryComputeLaneSlotV1 {
            generation: 7,
            state: Some(compute_lane_state_for_multi_inflight_test(auxiliary)),
        });
    let lane = ComputeAqlQueueLaneV1 {
        session: primary,
        ordinal: 1,
        generation: 7,
    };
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = session.with_compute_lane_v1(lane, |lane_dispatch| {
            let _published = lane_dispatch
                .session
                .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
                    &mut dispatch,
                    |generation| test_completion_template(auxiliary, generation),
                    |_, _| Ok(51),
                )
                .unwrap();
            std::panic::panic_any("r52-lane-after-publication")
        });
    }))
    .expect_err("public lane callback panic must be resumed");
    assert_eq!(
        panic.downcast_ref::<&'static str>(),
        Some(&"r52-lane-after-publication")
    );
    assert!(take_lane_unwind_process_gate_record_v1());
    assert!(session.terminal_poisoned);
    assert_eq!(session.key, primary);
    assert_eq!(
        session.auxiliary_compute_lanes[0]
            .state
            .as_ref()
            .expect("auxiliary lane is restored before unwind")
            .key,
        auxiliary
    );
    assert_eq!(dispatch.live_epoch_count(), 1);
    assert!(matches!(
        session.with_compute_lane_v1(lane, |_| ()),
        Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::Poisoned
        ))
    ));
    assert!(session.destroy().is_err());
}

#[test]
fn typed_native_callback_panic_terminalizes_without_rust_unwind_payload() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(177, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
    let failure = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, _| {
                Err(NativeAqlSubmissionFailureV1::Terminal(
                    NativeAqlSubmissionErrorV1::CallbackPanic,
                ))
            },
        )
        .expect_err("typed lower callback panic is terminal, not a Rust unwind");
    assert!(matches!(
        failure,
        FixedDispatchSubmissionFailureV1::Terminal(ComputeAqlQueueSessionErrorV1::Native(
            "submission callback panic"
        ))
    ));
    assert!(take_dispatch_terminal_process_gate_record_v1());
    assert!(session.terminal_poisoned);
    assert_eq!(dispatch.live_epoch_count(), 1);
    assert!(matches!(
        session.submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, _| panic!("terminal session must not call native submission"),
        ),
        Err(FixedDispatchSubmissionFailureV1::Terminal(
            ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
        ))
    ));
}

#[test]
fn completion_signal_full_is_retryable_without_native_effect_and_retries_after_recycle() {
    let queue = test_queue_key(173, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let full = session
        .completion_owner
        .fill_all_signals_for_test(test_completion_template(queue, 9));
    let full_snapshot = session.completion_owner.state_snapshot_for_test();
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
    let native_called = std::cell::Cell::new(false);

    let failure = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, _| {
                native_called.set(true);
                Ok(9000)
            },
        )
        .expect_err("a full completion arena must reject before native submission");
    assert!(matches!(
        failure,
        FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
            ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::InsufficientSignals)
        )
    ));
    assert!(!native_called.get());
    assert_eq!(
        session.completion_owner.state_snapshot_for_test(),
        full_snapshot
    );
    assert_eq!(dispatch.next_generation(), 2);
    assert_eq!(dispatch.live_epoch_count(), 0);
    assert!(!session.terminal_poisoned);

    let source_failure = session
        .submit_with_dependency_events_classified_v1(
            Box::new([test_completion_template(queue, 2)]),
            1,
            1,
            |_, _| panic!("full signals must refuse before native submit"),
        )
        .expect_err("dependency source capacity failure is retryable");
    assert!(matches!(
        source_failure,
        FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
            ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::InsufficientSignals)
        )
    ));
    assert_eq!(
        session.completion_owner.state_snapshot_for_test(),
        full_snapshot
    );
    assert!(!session.terminal_poisoned);

    session
        .completion_owner
        .complete_and_recycle_all_for_test(full);
    let _retry = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, packets| {
                assert_eq!(packets.packet_count(), 1);
                Ok(9001)
            },
        )
        .expect("recycled completion capacity accepts the burned successor identity");
    assert_eq!(dispatch.live_epoch_count(), 1);
    assert_eq!(dispatch.next_generation(), 3);

    let production = crate::queue::live_production_source_for_tests_v1()
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let target_capacity = production
        .split("fn submit_fixed_dispatch_with_dependencies_operation_v1")
        .nth(1)
        .unwrap()
        .split("let target = match")
        .next()
        .unwrap();
    assert!(target_capacity.contains("Err(Gfx942CompletionErrorV1::InsufficientSignals)"));
    assert!(target_capacity.contains("cancel_dependency_dispatch_generation_v1(identity)"));
    assert!(target_capacity.contains("return Err(retry("));
}

#[test]
fn prepared_persistent_compute_cancellation_restores_initialized_rebind_input() {
    let queue = test_queue_key(161, 1);
    let digest = [0xa5; 32];
    let (mut session, prepared, storage_identity) =
        prepared_persistent_compute_cancellation_fixture(queue, 6161, Some(digest), Some(7));

    let input = session
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("exact prepared cancellation must restore persistent input");
    let Gfx942PersistentComputeInputV1::Initialized(ready) = input else {
        panic!("initialized attachment must cancel back to initialized input")
    };
    assert_eq!(ready.authenticated_sha256(), digest);
    let allocation = ready.into_allocation();
    assert_eq!(
        allocation
            .owner
            .local_native_for_sdma()
            .expect("cancellation restored local native custody")
            .storage_identity(),
        storage_identity
    );
    assert!(allocation.owner.local_native_is_attached_for_sdma());
    assert_eq!(allocation.owner.live_use_count(), 0);
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    assert!(session.persistent_compute.is_none());
    assert!(session.dispatch.is_none());
    assert_eq!(session.detached_dispatch_generation, Some(7));
    assert_eq!(session.detached_next_insertion_index, Some(0));
    assert_eq!(session.detached_data_count, 0);
    assert!(session.detached_data_identities.is_empty());
    let input = Gfx942PersistentComputeInputV1::from_parts(
        allocation,
        PersistentComputeInitializationV1::AuthenticatedH2d(digest),
    );
    assert!(preserve_persistent_compute_bind_input_for_sdma_quiescence_v1(input, true).is_ok());
}

#[test]
fn single_uninitialized_write_bind_cancellation_preserves_uninitialized_custody() {
    let queue = test_queue_key(189, 1);
    let (mut session, prepared, storage_identity) =
        prepared_persistent_compute_cancellation_fixture_with_initialization_v1(
            queue, 8989, None, None, false,
        );
    let attachment = session
        .single_persistent_compute_attachment_v1()
        .expect("fixture retains one prepared attachment");
    assert!(
        !attachment
            .single_entry()
            .unwrap()
            .initialization
            .is_fully_initialized()
    );
    assert_eq!(
        attachment.single_entry().unwrap().effect,
        Gfx942PersistentComputeEffectV1::Write
    );

    let input = session
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("prepublication cancellation restores exact write-only input");
    let Gfx942PersistentComputeInputV1::Uninitialized(allocation) = input else {
        panic!("uninitialized write custody must not be promoted during cancellation")
    };
    assert_eq!(
        allocation
            .owner
            .local_native_for_sdma()
            .expect("cancellation reattached native storage")
            .storage_identity(),
        storage_identity
    );
    assert_eq!(allocation.owner.live_use_count(), 0);
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
    assert!(session.persistent_compute.is_none());
    assert!(session.dispatch.is_none());
    assert_eq!(session.detached_dispatch_generation, Some(0));
    assert_eq!(session.detached_data_count, 0);
}

#[test]
fn prepared_replay_cancellation_preserves_initialized_after_dispatch_without_digest() {
    let queue = test_queue_key(165, 1);
    let (mut session, prepared, storage_identity) =
        prepared_persistent_compute_cancellation_fixture(queue, 6565, None, Some(7));

    let input = session
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("exact replay cancellation must restore initialized input");
    let Gfx942PersistentComputeInputV1::InitializedAfterDispatch(allocation) = input else {
        panic!("digest-free initialized replay must remain fully initialized")
    };
    let allocation = allocation.into_allocation();
    assert_eq!(
        allocation
            .owner
            .local_native_for_sdma()
            .expect("cancellation restored local native custody")
            .storage_identity(),
        storage_identity
    );
    assert_eq!(session.detached_dispatch_generation, Some(7));
    assert_eq!(session.detached_next_insertion_index, Some(0));
}

#[test]
fn prepared_cancellation_preserves_initialized_storage_origin() {
    for predecessor in [None, Some(0), Some(7)] {
        let queue = test_queue_key(165, 1);
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 6565, None, predecessor);
        session.persistent_compute.as_mut().unwrap().entries[0].initialization =
            PersistentComputeInitializationV1::InitializedStorage;
        let input = session
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .expect("cancellation preserves digest-free storage authority");
        assert!(matches!(
            input,
            Gfx942PersistentComputeInputV1::InitializedStorage(_)
        ));
    }
}

#[test]
fn prepared_initial_cancellation_records_never_published_detached_generation() {
    let queue = test_queue_key(166, 1);
    let digest = [0xd8; 32];
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 6666, Some(digest), None);

    let input = session
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("initial prepared cancellation must restore its input");
    assert!(matches!(
        input,
        Gfx942PersistentComputeInputV1::Initialized(_)
    ));
    assert_eq!(session.detached_dispatch_generation, Some(0));
    assert_eq!(session.detached_next_insertion_index, Some(0));
    assert_eq!(session.detached_data_count, 0);
    assert!(session.detached_data_identities.is_empty());
}

#[test]
fn exact_prepared_persistent_submit_structural_failure_is_terminal_and_opaque() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(167, 1);
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 6767, Some([0xe9; 32]), Some(7));

    let failure = session
        .submit_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("missing retained dispatch owner is a terminal structural failure");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let (_, retryable) = failure.into_parts();
    assert!(retryable.is_none());
    assert!(session.terminal_poisoned);
    assert!(take_dispatch_terminal_process_gate_record_v1());
    assert_eq!(session.persistent_compute_terminal_stage_v1(), None);
    let attachment = session
        .single_persistent_compute_attachment_v1()
        .expect("terminal submit retains the exact attachment");
    assert!(matches!(
        attachment.single_entry().unwrap().state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(attachment.terminal_custody.is_none());
    assert!(matches!(
        session.release_retained_persistent_fixed_dispatch_control_v1(),
        Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::Poisoned
        ))
    ));
}

#[test]
fn foreign_prepared_persistent_submit_receipt_remains_recoverable() {
    let producer_key = test_queue_key(168, 1);
    let receiver_key = test_queue_key(169, 1);
    let (mut producer, prepared, _) = prepared_persistent_compute_cancellation_fixture(
        producer_key,
        6868,
        Some([0xfa; 32]),
        Some(7),
    );
    let mut receiver = persistent_compute_cancellation_test_session(receiver_key, None, None);

    let failure = receiver
        .submit_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("foreign receipt must be rejected before local attachment consumption");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let (_, retryable) = failure.into_parts();
    let prepared = retryable.expect("foreign receipt remains owned by its producer");
    assert!(!receiver.terminal_poisoned);
    assert!(receiver.persistent_compute.is_none());
    receiver.terminal_poisoned = true;
    let failure = receiver
        .submit_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("terminal foreign receiver cannot absorb another queue's receipt");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
    ));
    let (_, retryable) = failure.into_parts();
    let prepared = retryable.expect("terminal foreign receiver returns exact custody");
    assert!(
        producer
            .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
            .is_ok()
    );
}

#[test]
fn foreign_prepared_cancellation_receipt_retries_on_its_producer() {
    let producer_key = test_queue_key(162, 1);
    let receiver_key = test_queue_key(163, 1);
    let digest = [0xb6; 32];
    let (mut producer, prepared, storage_identity) =
        prepared_persistent_compute_cancellation_fixture(producer_key, 6262, Some(digest), Some(7));
    let mut receiver = persistent_compute_cancellation_test_session(receiver_key, None, None);

    let failure = receiver
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("a live foreign queue must reject the exact prepared receipt");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let (_, custody) = failure.into_parts();
    let crate::Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(prepared) = custody
    else {
        panic!("live foreign rejection must return the exact prepared receipt")
    };
    receiver.terminal_poisoned = true;
    let failure = receiver
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("a terminal foreign queue must still return the exact receipt");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
    ));
    let (_, custody) = failure.into_parts();
    let crate::Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(prepared) = custody
    else {
        panic!("terminal foreign rejection must return the exact prepared receipt")
    };
    let input = producer
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("the producer must accept the unchanged prepared receipt");
    let (allocation, initialization) = input.into_parts();
    let observed_digest = initialization.authenticated_sha256();
    let initialized = initialization.is_fully_initialized();
    assert!(initialized);
    assert_eq!(observed_digest, Some(digest));
    assert_eq!(
        allocation
            .owner
            .local_native_for_sdma()
            .expect("producer cancellation restored local native custody")
            .storage_identity(),
        storage_identity
    );
    assert!(allocation.owner.local_native_is_attached_for_sdma());
}

#[test]
fn terminal_self_owned_prepared_cancellation_is_absorbed_opaquely() {
    let queue = test_queue_key(164, 1);
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 6464, Some([0xc7; 32]), Some(7));
    session.terminal_poisoned = true;

    let failure = session
        .cancel_prepared_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("terminal producer must absorb its prepared receipt");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::Poisoned)
    ));
    assert_eq!(
        session.persistent_compute_terminal_stage_v1(),
        Some(crate::Gfx942PersistentComputeTerminalStageV1::Attached)
    );
    let (_, custody) = failure.into_parts();
    let crate::Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(terminal) =
        custody
    else {
        panic!("self-owned terminal receipt must not return retryable authority")
    };
    assert_eq!(terminal.stage(), None);
    let attachment = session
        .single_persistent_compute_attachment_v1()
        .expect("terminal queue retains the exact attachment");
    assert!(matches!(
        attachment.single_entry().unwrap().state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(matches!(
        attachment.terminal_custody,
        Some(PersistentComputeTerminalNativeCustodyV1::Attached)
    ));
}

#[test]
fn legacy_detached_only_gate_blocks_every_sdma_publication_and_both_directions() {
    let modes = [
        SdmaPublicationModeV1::Persistent,
        SdmaPublicationModeV1::DirectionalCopy(Gfx942PersistentSdmaDirectionV1::HostToDevice),
        SdmaPublicationModeV1::DirectionalCopy(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
        SdmaPublicationModeV1::DirectionalWindow(Gfx942PersistentSdmaDirectionV1::HostToDevice),
        SdmaPublicationModeV1::DirectionalWindow(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
        SdmaPublicationModeV1::SameDeviceWindow,
        SdmaPublicationModeV1::Ordinary,
        SdmaPublicationModeV1::OrdinaryBatch,
        SdmaPublicationModeV1::StripedBatch,
        SdmaPublicationModeV1::ExecuteBatch,
    ];
    for mode in modes {
        assert!(matches!(
            admit_sdma_publication_while_compute_detached(false, true, mode),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert!(matches!(
            admit_sdma_publication_while_compute_detached(false, false, mode),
            Ok(observed) if observed == mode
        ));
        assert!(matches!(
            admit_sdma_publication_while_compute_detached(true, true, mode),
            Err(Gfx942DispatchBindingErrorV1::Poisoned)
        ));
    }
}

#[test]
fn ordinary_sdma_publication_gate_returns_exact_buffer_custody() {
    let queue = test_queue_key(151, 1);
    let (source, destination) = crate::sdma::persistent_sdma_buffers_for_test(queue, 5151);
    let source_identity = source.storage_identity();
    let destination_identity = destination.storage_identity();
    let failure = preserve_ordinary_sdma_publication_custody_v1(true, source, destination)
        .expect_err("persistent compute must block ordinary SDMA publication");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let (_, recovered) = failure.into_parts();
    let (source, destination) = recovered.expect("preflight returns both exact buffers");
    assert_eq!(source.storage_identity(), source_identity);
    assert_eq!(destination.storage_identity(), destination_identity);
}

#[test]
fn combined_sdma_finish_recovers_only_timeout_after_closing_currentness() {
    let timeout = ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout);
    let terminal = ComputeAqlQueueSessionErrorV1::Contract("SDMA wait failure");

    assert_eq!(
        classify_sdma_batch_execution_finish(None, true),
        Gfx942SdmaBatchExecutionFinishV1::Success
    );
    assert_eq!(
        classify_sdma_batch_execution_finish(Some(&timeout), true),
        Gfx942SdmaBatchExecutionFinishV1::RecoverableTimeout
    );
    assert_eq!(
        classify_sdma_batch_execution_finish(Some(&timeout), false),
        Gfx942SdmaBatchExecutionFinishV1::Terminal
    );
    assert_eq!(
        classify_sdma_batch_execution_finish(None, false),
        Gfx942SdmaBatchExecutionFinishV1::Terminal
    );
    assert_eq!(
        classify_sdma_batch_execution_finish(Some(&terminal), true),
        Gfx942SdmaBatchExecutionFinishV1::Terminal
    );
}

#[test]
fn barrier_probe_backings_have_stable_distinct_contracts() {
    let logical_bytes = 64 * 1024;
    assert_eq!(
        QueueRingBackingV1::AqlSpecial.observation(),
        Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX
    );
    assert_eq!(
        QueueRingBackingV1::ExecutableProbe.observation(),
        Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX
    );
    assert_eq!(
        QueueRingBackingV1::UserptrProbe.observation(),
        Gfx942BarrierProbeRingBackingV1::UserptrOneX
    );
    assert_eq!(QueueRingBackingV1::AqlSpecial.digest_tag(), 1);
    assert_eq!(QueueRingBackingV1::ExecutableProbe.digest_tag(), 2);
    assert_eq!(QueueRingBackingV1::UserptrProbe.digest_tag(), 3);
    assert_eq!(
        QueueRingBackingV1::AqlSpecial.gpu_va_bytes(logical_bytes),
        u64::from(logical_bytes)
    );
    assert_eq!(
        QueueRingBackingV1::ExecutableProbe.gpu_va_bytes(logical_bytes),
        u64::from(logical_bytes)
    );
    assert_eq!(
        QueueRingBackingV1::UserptrProbe.gpu_va_bytes(logical_bytes),
        u64::from(logical_bytes)
    );
}

#[test]
fn queue_identity_digest_binds_backing_and_exact_ring_span() {
    let vm = fe2o3_runtime_model::VmKeyV1 {
        device: fe2o3_runtime_model::DeviceKeyV1 {
            physical: fe2o3_runtime_model::PhysicalDeviceIdV1(7),
            generation: fe2o3_runtime_model::DeviceGenerationV1(11),
        },
        id: fe2o3_runtime_model::VmIdV1(13),
    };
    let queue = QueueKeyV1 {
        vm,
        id: QueueInstanceIdV1(17),
        generation: QueueGenerationV1(19),
    };
    let mappings = core::array::from_fn(|index| fe2o3_runtime_model::MemoryMappingKeyV1 {
        allocation: fe2o3_runtime_model::MemoryAllocationKeyV1 {
            vm,
            id: fe2o3_runtime_model::AllocationIdV1(index as u64 + 1),
            generation: fe2o3_runtime_model::AllocationGenerationV1(1),
        },
        id: fe2o3_runtime_model::MappingIdV1(index as u64 + 21),
    });
    let special = digest_id(
        b"plan",
        queue,
        &mappings,
        QueueRingBackingV1::AqlSpecial,
        65_536,
        131_072,
    );
    let executable = digest_id(
        b"plan",
        queue,
        &mappings,
        QueueRingBackingV1::ExecutableProbe,
        65_536,
        65_536,
    );
    let hostile_span = digest_id(
        b"plan",
        queue,
        &mappings,
        QueueRingBackingV1::ExecutableProbe,
        65_536,
        131_072,
    );
    let userptr = digest_id(
        b"plan",
        queue,
        &mappings,
        QueueRingBackingV1::UserptrProbe,
        65_536,
        65_536,
    );
    assert_ne!(special, executable);
    assert_ne!(special, userptr);
    assert_ne!(executable, userptr);
    assert_ne!(executable, hostile_span);
}

#[test]
fn create_result_is_nonterminal_only_when_no_effect_is_explicit() {
    let no_effect = map_create(NativeQueueAdapterErrorV1::BackendFailedNoEffect(
        NativeQueueOperationV1::Create,
    ));
    assert!(!no_effect.is_terminal_creation());

    for error in [
        NativeQueueAdapterErrorV1::ProcessChanged,
        NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Create),
        NativeQueueAdapterErrorV1::MalformedKernelResult(
            NativeQueueOperationV1::Create,
            "hostile output",
        ),
        NativeQueueAdapterErrorV1::ModelProjection,
    ] {
        assert!(map_create(error).is_terminal_creation());
    }
}

#[test]
fn probe_creation_phase_preserves_backing_and_terminal_classification() {
    for backing in [
        Gfx942BarrierProbeRingBackingV1::Gfx942ExecutableOneX,
        Gfx942BarrierProbeRingBackingV1::ExecutableGttOneX,
    ] {
        let ordinary = barrier_probe_creation_failure(
            ComputeAqlQueueSessionErrorV1::Contract("pre-create"),
            backing,
        );
        assert!(matches!(
            ordinary,
            Gfx942BarrierProbeFailureV1::Creation { .. }
        ));
        assert_eq!(ordinary.backing(), backing);

        let terminal = barrier_probe_creation_failure(
            terminal_creation(
                "post-create",
                ComputeAqlQueueSessionErrorV1::Contract("fault injection"),
            ),
            backing,
        );
        assert!(matches!(
            terminal,
            Gfx942BarrierProbeFailureV1::TerminalCreation { .. }
        ));
        assert_eq!(terminal.backing(), backing);
    }
}
