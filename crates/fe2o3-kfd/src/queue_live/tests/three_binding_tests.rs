use super::*;

#[test]
#[allow(clippy::result_large_err)]
fn three_binding_bounded_wait_timeout_returns_exact_published_custody() {
    let queue = test_queue_key(197, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let mut dispatch_owner =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
    let batch = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch_owner,
            |generation| test_completion_template(queue, generation),
            |_, packets| {
                assert_eq!(packets.packet_count(), 1);
                Ok(197)
            },
        )
        .unwrap();
    let completion_before = session.completion_owner.state_snapshot_for_test();
    let binding = PersistentComputeBindingKeyV1 {
        queue,
        attachment_generation: 1,
    };
    session.persistent_compute = Some(BoundedPersistentComputeAttachmentV1::from_three(
        ThreeBindingPersistentComputeAttachmentV1 {
            entries: [
                published_three_binding_test_entry_v1(
                    queue,
                    0x5b00,
                    Gfx942PersistentComputeEffectV1::Read,
                ),
                published_three_binding_test_entry_v1(
                    queue,
                    0x5b01,
                    Gfx942PersistentComputeEffectV1::Read,
                ),
                published_three_binding_test_entry_v1(
                    queue,
                    0x5b02,
                    Gfx942PersistentComputeEffectV1::Write,
                ),
            ],
            binding,
            predecessor_dispatch_generation: None,
            terminal_custody: None,
        },
    ));
    let receipt = Gfx942ThreeBindingPersistentComputeDispatchV1 {
        binding,
        batch,
        thread_affinity: PhantomData,
    };
    let timeout =
        session
            .wait_and_recycle_three_binding_directional_persistent_fixed_dispatch_until_v1_using(
                receipt,
                Instant::now(),
                |session, dispatch| {
                    session
                    .poll_and_recycle_three_binding_directional_persistent_fixed_dispatch_v1_using(
                        dispatch,
                        |_, identity, completion| {
                            dispatch_owner.validate_published(identity, completion).is_ok()
                        },
                        |session, completion| {
                            session
                                .completion_owner
                                .observe_one_pending_with_current_closing_for_test(completion)
                                .map_err(|(error, completion)| (error.into(), completion))
                        },
                    )
                },
            )
            .expect("one conclusive Pending observation reaches the expired deadline");
    let Gfx942ThreeBindingPersistentComputeWaitAndRecycleV1::Timeout {
        dispatch,
        observations,
    } = timeout
    else {
        panic!("pending completion must return timeout custody")
    };
    assert_eq!(observations, 1);
    assert_eq!(dispatch.binding, binding);
    assert_eq!(dispatch_owner.live_epoch_count(), 1);
    assert_eq!(
        session.completion_owner.state_snapshot_for_test(),
        completion_before
    );
    assert!(!session.terminal_poisoned);
    assert!(
        session
            .three_binding_persistent_compute_attachment_v1()
            .unwrap()
            .entries
            .iter()
            .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
    );
}

#[test]
fn three_binding_prepublication_cancel_restores_all_exact_initialized_inputs() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(194, 1);
    let (mut session, prepared, identities, digests) =
        prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue);
    let inputs = session
        .cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(prepared)
        .expect("clean prepublication cancellation restores all three inputs")
        .into_inputs();
    assert!(session.persistent_compute.is_none());
    assert_eq!(session.detached_dispatch_generation, Some(7));
    assert_eq!(session.detached_next_insertion_index, Some(0));
    assert!(!session.terminal_poisoned);
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    for (index, input) in inputs.into_iter().enumerate() {
        let (allocation, initialization) = input.into_parts();
        let digest = initialization.authenticated_sha256();
        let initialized = initialization.is_fully_initialized();
        assert_eq!(digest, Some(digests[index]));
        assert!(initialized);
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .expect("cancelled input regains native storage")
                .storage_identity(),
            identities[index]
        );
        assert_eq!(allocation.owner.live_use_count(), 0);
        assert_eq!(allocation.owner.retained_settled_use_count(), 0);
        assert_eq!(allocation.owner.quarantine_reason(), None);
    }
}

#[test]
fn three_binding_cancel_identity_preflight_prevents_partial_restoration() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(195, 1);
    let (mut session, prepared, identities, _) =
        prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue);
    session
        .persistent_compute
        .as_mut()
        .expect("fixture retains three entries")
        .entries[1]
        .storage_identity = Some(identities[0]);
    let failure = session
        .cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(prepared)
        .expect_err("substituted identity must fail before any owner restoration");
    assert!(failure.into_parts().1.is_none());
    assert!(session.terminal_poisoned);
    assert!(take_dispatch_terminal_process_gate_record_v1());
    let attachment = session
        .three_binding_persistent_compute_attachment_v1()
        .expect("terminal cancellation retains all owners");
    assert!(attachment.entries.iter().all(|entry| {
        entry.allocation.owner.local_native_for_sdma().is_none()
            && matches!(entry.state, PersistentComputeUseStateV1::Quarantined)
            && entry.allocation.owner.quarantine_reason()
                == Some(Gfx942PersistentQuarantineReasonV1::CallerReportedCurrentnessLoss)
    }));
    assert!(matches!(
        attachment.terminal_custody,
        Some(PersistentComputeTerminalNativeCustodyV1::Cancellation(_))
    ));
    assert_eq!(
        session.persistent_compute_terminal_stage_v1(),
        Some(crate::Gfx942PersistentComputeTerminalStageV1::DataDetached)
    );
}

#[test]
fn production_three_binding_bind_validates_real_vecadd_metadata_and_exact_storage() {
    use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
    use fe2o3_hsaco::ArgumentAccess;

    let queue = test_queue_key(196, 1);
    let signature = [0x57; 32];
    let image =
        include_bytes!("../../../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    let program = validate(image, AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .reconcile_dispatch_abi(
            signature,
            &[
                KernelGlobalBufferAbiV1::new(0, "arg0.data", 0, 1, ArgumentAccess::ReadOnly),
                KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 1, ArgumentAccess::ReadOnly),
                KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 1, ArgumentAccess::WriteOnly),
            ],
        )
        .unwrap();
    let byte_len = 4096_u64;
    let mut kernarg = [0_u8; 48];
    for offset in [8, 24, 40] {
        kernarg[offset..offset + 8].copy_from_slice(&(byte_len / 4).to_le_bytes());
    }
    let packet = Gfx942FixedDispatchPacketV1::new(
        0,
        fe2o3_aql::AqlDispatchGeometryV1::new([1024, 1, 1], [256, 1, 1]).unwrap(),
        0,
        kernarg.into(),
        [
            super::super::super::dispatch_binding::Gfx942DispatchBufferBindingV1::new(
                0, 0, 0, byte_len,
            ),
            super::super::super::dispatch_binding::Gfx942DispatchBufferBindingV1::new(
                2, 1, 0, byte_len,
            ),
            super::super::super::dispatch_binding::Gfx942DispatchBufferBindingV1::new(
                4, 2, 0, byte_len,
            ),
        ]
        .into(),
    );
    assert_eq!(
        packet.ordering(),
        fe2o3_aql::AqlDispatchOrderingV1::WaitForPrior
    );
    let make = |index: u64| {
        let allocation = persistent_compute_gate_test_allocation_v1(
            queue,
            0x5a00_u64.checked_add(index).unwrap(),
        );
        let identity = allocation
            .owner
            .local_native_for_sdma()
            .unwrap()
            .storage_identity();
        (
            Gfx942PersistentComputeInputV1::from_parts(
                allocation,
                PersistentComputeInitializationV1::AfterDispatch,
            ),
            identity,
        )
    };
    let [
        (input_a, identity_a),
        (input_b, identity_b),
        (input_c, identity_c),
    ] = [0, 1, 2].map(make);
    let identities = [identity_a, identity_b, identity_c];
    assert!(identity_a != identity_b && identity_a != identity_c && identity_b != identity_c);
    THREE_BINDING_BIND_VALIDATION_EFFECTS_V1.set(None);
    THREE_BINDING_BIND_VALIDATION_ONLY_V1.set(true);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let failure = session
        .bind_three_binding_directional_persistent_fixed_dispatch_v1(
            vec![program],
            [packet],
            Gfx942ThreeBindingPersistentComputeInputsV1::new([input_a, input_b, input_c]),
            [0, 1, 2].map(|ordinal| Gfx942DeviceContentRoleV1::new(signature, ordinal).unwrap()),
        )
        .expect_err("validation-only boundary returns exact unmodified input custody");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Contract("test-only three-binding validation boundary")
    ));
    assert_eq!(
        THREE_BINDING_BIND_VALIDATION_EFFECTS_V1.replace(None),
        Some([
            DeviceDataEffectV1::ReadOnly,
            DeviceDataEffectV1::ReadOnly,
            DeviceDataEffectV1::WriteOnly,
        ])
    );
    let (_, custody) = failure.into_parts();
    let Gfx942ThreeBindingPersistentComputeBindFailureCustodyV1::Retryable(inputs) = custody else {
        panic!("validation-only exit must return all three inputs")
    };
    for (input, identity) in inputs.into_inputs().into_iter().zip(identities) {
        let (allocation, initialization) = input.into_parts();
        let _ = initialization.authenticated_sha256();
        let initialized = initialization.is_fully_initialized();
        assert!(initialized);
        assert_eq!(
            allocation
                .owner
                .local_native_for_sdma()
                .unwrap()
                .storage_identity(),
            identity
        );
        assert_eq!(allocation.owner.live_use_count(), 0);
    }
    assert!(!session.has_any_persistent_compute_attachment_v1());
    assert!(!session.terminal_poisoned);
}

#[test]
fn ordinary_template_validation_rejection_is_clean_and_next_binding_can_publish() {
    let queue = test_queue_key(170, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    for error in [
        Gfx942DispatchBindingErrorV1::ZeroPacketCount,
        Gfx942DispatchBindingErrorV1::PacketCountExceedsMaximum {
            requested: GFX942_MAX_FIXED_DISPATCH_PACKETS_V1 + 1,
            maximum: GFX942_MAX_FIXED_DISPATCH_PACKETS_V1,
        },
        Gfx942DispatchBindingErrorV1::WrongQueueGeneration,
    ] {
        let failure = session
            .classify_fixed_dispatch_binding::<[CompletionPacketTemplateV1; 1]>(
                FixedDispatchBindingModeV1::Ordinary,
                Err(error),
            )
            .expect_err("ordinary pre-mutation validation must reject");
        assert!(matches!(
            failure,
            FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(_)
        ));
        assert!(!session.terminal_poisoned);
    }

    let templates = session
        .classify_fixed_dispatch_binding(
            FixedDispatchBindingModeV1::Ordinary,
            Ok([test_completion_template(queue, 8)]),
        )
        .expect("a corrected batch remains admissible");
    let _published = session
        .submit_with_completions_classified_using(Box::new(templates), |_, packets| {
            assert_eq!(packets.packet_count(), 1);
            Ok(41)
        })
        .expect("clean validation rejection leaves completion submission usable");
    assert!(!session.terminal_poisoned);
}

#[test]
fn persistent_exact_occupancy_retry_restores_every_layer_then_succeeds() {
    for occupancy in [
        fe2o3_aql::AqlRingReservationError::Full,
        fe2o3_aql::AqlRingReservationError::InsufficientSpace {
            requested: 1,
            available: 0,
        },
    ] {
        let queue = test_queue_key(171, 1);
        let (mut session, prepared, _) =
            prepared_persistent_compute_cancellation_fixture(queue, 6969, None, Some(7));
        let expected_binding = prepared.binding;
        let mut dispatch =
            super::super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(
                7,
            )
            .expect("fixture admits a production-reachable recycled predecessor");

        let failure = session
            .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
                session.submit_fixed_dispatch_inner_classified_with_test_owner(
                    &mut dispatch,
                    |generation| test_completion_template(queue, generation),
                    |_, packets| {
                        assert_eq!(packets.packet_count(), 1);
                        Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                            NativeAqlSubmissionErrorV1::Ring(occupancy),
                        ))
                    },
                )
            })
            .expect_err("exact pre-side-effect occupancy must restore retry custody");
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::Native("submission ring occupancy")
        ));
        let (_, retryable) = failure.into_parts();
        let retryable = retryable.expect("outer persistent receipt is restored");
        assert_eq!(retryable.binding, expected_binding);
        assert_eq!(dispatch.predecessor_generation(), 7);
        assert_eq!(dispatch.last_cancelled_generation(), Some(8));
        assert!(matches!(
            dispatch.active_generation(),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        let attachment = session
            .single_persistent_compute_attachment_v1()
            .expect("persistent attachment is restored");
        assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Prepared(_)
        ));
        assert!(!session.terminal_poisoned);

        let _published = session
            .submit_directional_persistent_fixed_dispatch_v1_using(retryable, |session| {
                session.submit_fixed_dispatch_inner_classified_with_test_owner(
                    &mut dispatch,
                    |generation| test_completion_template(queue, generation),
                    |_, packets| {
                        assert_eq!(packets.packet_count(), 1);
                        Ok(64)
                    },
                )
            })
            .expect("the unchanged persistent receipt succeeds exactly once on retry");
        assert!(matches!(dispatch.active_generation(), Ok(9)));
        let attachment = session
            .single_persistent_compute_attachment_v1()
            .expect("successful retry retains published attachment custody");
        assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
        assert!(matches!(
            attachment.single_entry().unwrap().state,
            PersistentComputeUseStateV1::Published(_)
        ));
        assert!(!session.terminal_poisoned);
    }
}

#[test]
#[allow(clippy::result_large_err)]
fn three_binding_persistent_submit_retains_native_attachment_on_callback_failure() {
    for case in 0..4 {
        assert!(!take_persistent_unwind_process_gate_record_v1());
        assert!(!take_dispatch_terminal_process_gate_record_v1());
        let queue = test_queue_key(194, 1);
        let (mut session, prepared, identities, digests) =
            prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue);
        let binding = prepared.binding;
        let before: Vec<_> = session
            .three_binding_persistent_compute_attachment_v1()
            .unwrap()
            .entries
            .iter()
            .map(|entry| {
                (
                    prepared_publication_identity_v1(&entry.state),
                    entry.allocation.owner.ownership_snapshot_for_test_v1(),
                )
            })
            .collect();
        let roster = |session: &ComputeAqlQueueSessionV1| {
            let (generation, data) = session.persistent_compute_test_release.as_ref().unwrap();
            (
                *generation,
                data.as_ptr() as usize,
                data.iter()
                    .map(|data| data.storage_identity())
                    .collect::<Vec<_>>(),
            )
        };
        let native_roster = roster(&session);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
                prepared,
                |_| {
                    let error = ComputeAqlQueueSessionErrorV1::Native(
                        "three-binding injected submit failure",
                    );
                    match case {
                        0 => Err(FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(
                            error,
                        )),
                        1 => Err(FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(
                            error,
                        )),
                        2 => Err(FixedDispatchSubmissionFailureV1::Terminal(error)),
                        _ => std::panic::panic_any("three-binding submit unwind"),
                    }
                },
            )
        }));
        if case == 3 {
            assert_eq!(
                result.unwrap_err().downcast_ref::<&str>(),
                Some(&"three-binding submit unwind")
            );
        } else {
            let (_, retryable) = result.unwrap().unwrap_err().into_parts();
            assert_eq!(
                retryable.map(|receipt| receipt.binding),
                (case == 0).then_some(binding)
            );
        }
        assert_eq!(session.terminal_poisoned, case != 0);
        assert_eq!(take_persistent_unwind_process_gate_record_v1(), case == 3);
        assert_eq!(take_dispatch_terminal_process_gate_record_v1(), case == 2);
        assert_eq!(roster(&session), native_roster);
        let attachment = session
            .three_binding_persistent_compute_attachment_v1()
            .unwrap();
        assert_eq!(attachment.binding, binding);
        assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
        assert_eq!(attachment.entries.len(), 3);
        assert_eq!(
            matches!(
                attachment.terminal_custody,
                Some(PersistentComputeTerminalNativeCustodyV1::Attached)
            ),
            matches!(case, 1 | 2)
        );
        if matches!(case, 0 | 3) {
            assert!(attachment.terminal_custody.is_none());
        }
        for (index, entry) in attachment.entries.iter().enumerate() {
            assert!(
                before[index]
                    .1
                    .same_allocation(&entry.allocation.owner.ownership_snapshot_for_test_v1())
            );
            assert_eq!(entry.storage_identity, Some(identities[index]));
            assert_eq!(
                entry.initialization.authenticated_sha256(),
                Some(digests[index])
            );
            assert!(entry.initialization.is_fully_initialized());
            assert_eq!(
                entry.effect,
                if index == 2 {
                    Gfx942PersistentComputeEffectV1::Write
                } else {
                    Gfx942PersistentComputeEffectV1::Read
                }
            );
            if case == 0 {
                assert_eq!(
                    (
                        prepared_publication_identity_v1(&entry.state),
                        entry.allocation.owner.ownership_snapshot_for_test_v1()
                    ),
                    before[index]
                );
                assert!(matches!(
                    entry.state,
                    PersistentComputeUseStateV1::Prepared(_)
                ));
            } else {
                assert!(matches!(
                    entry.state,
                    PersistentComputeUseStateV1::Quarantined
                ));
            }
        }
    }
}

#[test]
fn three_binding_persistent_exact_occupancy_rearms_then_publishes() {
    for occupancy in [
        fe2o3_aql::AqlRingReservationError::Full,
        fe2o3_aql::AqlRingReservationError::InsufficientSpace {
            requested: 1,
            available: 0,
        },
    ] {
        let queue = test_queue_key(194, 1);
        let (mut session, prepared, identities, digests) =
            prepared_three_binding_persistent_compute_cancellation_fixture_v1(queue);
        let binding = prepared.binding;
        let before: Vec<_> = session
            .three_binding_persistent_compute_attachment_v1()
            .unwrap()
            .entries
            .iter()
            .map(|entry| {
                (
                    prepared_publication_identity_v1(&entry.state),
                    entry.allocation.owner.ownership_snapshot_for_test_v1(),
                )
            })
            .collect();
        let roster = |session: &ComputeAqlQueueSessionV1| {
            let (generation, data) = session.persistent_compute_test_release.as_ref().unwrap();
            (
                *generation,
                data.as_ptr() as usize,
                data.iter()
                    .map(|data| data.storage_identity())
                    .collect::<Vec<_>>(),
            )
        };
        let native_roster = roster(&session);
        let mut dispatch =
            super::super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(
                7,
            )
            .unwrap();
        let failure = session
            .submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
                prepared,
                |session| {
                    session.submit_fixed_dispatch_inner_classified_with_test_owner(
                        &mut dispatch,
                        |generation| test_completion_template(queue, generation),
                        |_, packets| {
                            assert_eq!(packets.packet_count(), 1);
                            Err(NativeAqlSubmissionFailureV1::RetryableBeforeSideEffect(
                                NativeAqlSubmissionErrorV1::Ring(occupancy),
                            ))
                        },
                    )
                },
            )
            .unwrap_err();
        let prepared = failure.into_parts().1.unwrap();
        assert_eq!(roster(&session), native_roster);
        assert_eq!(prepared.binding, binding);
        assert_eq!(dispatch.predecessor_generation(), 7);
        assert_eq!(dispatch.last_cancelled_generation(), Some(8));
        assert!(matches!(
            dispatch.active_generation(),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        let attachment = session
            .three_binding_persistent_compute_attachment_v1()
            .unwrap();
        for (index, entry) in attachment.entries.iter().enumerate() {
            assert_eq!(
                (
                    prepared_publication_identity_v1(&entry.state),
                    entry.allocation.owner.ownership_snapshot_for_test_v1()
                ),
                before[index]
            );
            assert_eq!(entry.storage_identity, Some(identities[index]));
            assert_eq!(
                entry.initialization.authenticated_sha256(),
                Some(digests[index])
            );
        }
        assert!(!session.terminal_poisoned);
        let _published = session
            .submit_three_binding_directional_persistent_fixed_dispatch_v1_using(
                prepared,
                |session| {
                    session.submit_fixed_dispatch_inner_classified_with_test_owner(
                        &mut dispatch,
                        |generation| test_completion_template(queue, generation),
                        |_, packets| {
                            assert_eq!(packets.packet_count(), 1);
                            Ok(64)
                        },
                    )
                },
            )
            .unwrap();
        assert!(matches!(dispatch.active_generation(), Ok(9)));
        let attachment = session
            .three_binding_persistent_compute_attachment_v1()
            .unwrap();
        assert_eq!(attachment.binding, binding);
        assert_eq!(attachment.predecessor_dispatch_generation, Some(7));
        assert!(
            attachment
                .entries
                .iter()
                .all(|entry| matches!(entry.state, PersistentComputeUseStateV1::Published(_)))
        );
        assert!(!session.terminal_poisoned);
    }
}

#[test]
fn persistent_submit_panic_resumes_payload_and_retains_terminal_phase() {
    assert!(!take_persistent_unwind_process_gate_record_v1());
    let queue = test_queue_key(174, 1);
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 0x5210, None, Some(7));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = session.submit_directional_persistent_fixed_dispatch_v1_using(
            prepared,
            |_| -> Result<Gfx942DispatchBatchV1<1>, FixedDispatchSubmissionFailureV1> {
                std::panic::panic_any("r52-persistent-submit-panic")
            },
        );
    }))
    .expect_err("persistent submit envelope must resume the original payload");
    assert_eq!(
        panic.downcast_ref::<&'static str>(),
        Some(&"r52-persistent-submit-panic")
    );
    assert!(take_persistent_unwind_process_gate_record_v1());
    assert!(session.terminal_poisoned);
    let attachment = session
        .single_persistent_compute_attachment_v1()
        .expect("panic retains the attachment");
    assert!(matches!(
        attachment.single_entry().unwrap().state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(attachment.terminal_custody.is_none());
}

#[test]
fn persistent_typed_native_callback_panic_has_no_claimed_native_stage() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(178, 1);
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 0x5211, None, Some(7));
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(7)
            .unwrap();
    let failure = session
        .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
            session.submit_fixed_dispatch_inner_classified_with_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| {
                    Err(NativeAqlSubmissionFailureV1::Terminal(
                        NativeAqlSubmissionErrorV1::CallbackPanic,
                    ))
                },
            )
        })
        .expect_err("typed native callback panic is a terminal failure");
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Native("submission callback panic")
    ));
    let (_, retryable) = failure.into_parts();
    assert!(retryable.is_none());
    assert!(take_dispatch_terminal_process_gate_record_v1());
    assert!(session.terminal_poisoned);
    let attachment = session.single_persistent_compute_attachment_v1().unwrap();
    assert!(matches!(
        attachment.single_entry().unwrap().state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(attachment.terminal_custody.is_none());
}

#[test]
fn persistent_normal_poll_identity_failure_process_gates_after_custody_consumption() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(182, 1);
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 0x5212, None, Some(7));
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(7)
            .unwrap();
    let published = session
        .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
            session.submit_fixed_dispatch_inner_classified_with_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| Ok(65),
            )
        })
        .unwrap();

    let failure = match session.poll_directional_persistent_fixed_dispatch_v1(published) {
        Err(failure) => failure,
        Ok(_) => panic!("the foreign dispatch owner identity must fail after consumption"),
    };
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
        )
    ));
    assert_eq!(failure.retained_stage(), None);
    let (_, custody) = failure.into_parts();
    assert!(matches!(
        custody,
        crate::Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(_)
    ));
    assert!(session.terminal_poisoned);
    assert!(take_dispatch_terminal_process_gate_record_v1());
    let attachment = session.single_persistent_compute_attachment_v1().unwrap();
    assert!(matches!(
        attachment.single_entry().unwrap().state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(matches!(
        attachment.terminal_custody,
        Some(PersistentComputeTerminalNativeCustodyV1::Published(_))
    ));
}

#[test]
fn persistent_normal_recycle_identity_failure_process_gates_after_custody_consumption() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(183, 1);
    let (mut session, prepared, _) =
        prepared_persistent_compute_cancellation_fixture(queue, 0x5213, None, Some(7));
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyDispatchGenerationOwnerV1::after_recycled(7)
            .unwrap();
    let published = session
        .submit_directional_persistent_fixed_dispatch_v1_using(prepared, |session| {
            session.submit_fixed_dispatch_inner_classified_with_test_owner(
                &mut dispatch,
                |generation| test_completion_template(queue, generation),
                |_, _| Ok(66),
            )
        })
        .unwrap();
    let Gfx942PersistentComputeDispatchV1 { binding, batch, .. } = published;
    let (completion, identity) = unwrap_published(batch);
    let completion = session
        .completion_owner
        .complete_one_without_native_for_test(completion);
    let completed = wrap_completed(completion, identity);
    let attachment = session
        .single_persistent_compute_attachment_mut_v1()
        .unwrap();
    let entry = attachment.single_entry_mut().unwrap();
    let state = core::mem::replace(&mut entry.state, PersistentComputeUseStateV1::Quarantined);
    let PersistentComputeUseStateV1::Published(published_use) = state else {
        panic!("successful submit retains a published allocation lease")
    };
    entry.state = PersistentComputeUseStateV1::Completed(
        entry.allocation.owner.complete(published_use).unwrap(),
    );

    let failure = match session.recycle_directional_persistent_fixed_dispatch_v1(
        Gfx942CompletedPersistentComputeDispatchV1 {
            binding,
            completed,
            thread_affinity: PhantomData,
        },
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("the foreign dispatch owner identity must fail after consumption"),
    };
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::StaleDispatchGeneration
        )
    ));
    assert_eq!(failure.retained_stage(), None);
    let (_, custody) = failure.into_parts();
    assert!(matches!(
        custody,
        crate::Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(_)
    ));
    assert!(session.terminal_poisoned);
    assert!(take_dispatch_terminal_process_gate_record_v1());
    let attachment = session.single_persistent_compute_attachment_v1().unwrap();
    assert!(matches!(
        attachment.single_entry().unwrap().state,
        PersistentComputeUseStateV1::Quarantined
    ));
    assert!(matches!(
        attachment.terminal_custody,
        Some(PersistentComputeTerminalNativeCustodyV1::Completed(_))
    ));
}

#[test]
fn fixed_dispatch_observation_error_process_gates_with_a_live_sibling_epoch() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(179, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
    let first = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, _| Ok(61),
        )
        .unwrap();
    let sibling = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, _| Ok(62),
        )
        .unwrap();
    assert_eq!(dispatch.live_epoch_count(), 2);

    drop(first);
    dispatch.poison();
    let error = session
        .terminalize_fixed_dispatch_observation_result_v1::<()>(Err(
            Gfx942CompletionErrorV1::Observation.into(),
        ))
        .expect_err("an accepted observation failure is terminal");
    assert!(matches!(
        error,
        ComputeAqlQueueSessionErrorV1::Completion(Gfx942CompletionErrorV1::Observation)
    ));
    assert!(take_dispatch_terminal_process_gate_record_v1());
    assert!(session.terminal_poisoned);
    assert_eq!(dispatch.live_epoch_count(), 2);
    assert!(matches!(
        dispatch.reserve_one(queue, test_completion_template(queue, 3)),
        Err(Gfx942DispatchBindingErrorV1::Poisoned)
    ));
    assert!(matches!(
        dispatch.ensure_releasable(),
        Err(Gfx942DispatchBindingErrorV1::Poisoned)
    ));
    drop(sibling);
}

#[test]
fn fixed_dispatch_recycle_only_preserves_the_signal_pinned_retry_path() {
    assert!(!take_dispatch_terminal_process_gate_record_v1());
    let queue = test_queue_key(180, 1);
    let mut session = persistent_compute_cancellation_test_session(queue, None, None);
    let mut dispatch =
        super::super::super::dispatch_binding::TestOnlyMultiInflightDispatchOwnerV1::new();
    let published = session
        .submit_fixed_dispatch_inner_classified_with_multi_inflight_test_owner(
            &mut dispatch,
            |generation| test_completion_template(queue, generation),
            |_, _| Ok(63),
        )
        .unwrap();
    let completed = session
        .complete_fixed_dispatch_with_multi_inflight_test_owner(&mut dispatch, published)
        .unwrap();
    let retryable = session
        .terminalize_fixed_dispatch_recycle_result_v1::<1>(Err(
            Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942CompletionErrorV1::SignalPinned {
                    slot: 0,
                    event_pins: 1,
                    native_reader_pins: 0,
                }
                .into(),
                retryable_completed: Some(completed),
            },
        ))
        .expect_err("a pinned signal retains exact retry custody");
    assert!(retryable.retryable_completed.is_some());
    assert!(!session.terminal_poisoned);
    assert!(!take_dispatch_terminal_process_gate_record_v1());

    let mut terminal_session =
        persistent_compute_cancellation_test_session(test_queue_key(181, 1), None, None);
    let terminal = terminal_session
        .terminalize_fixed_dispatch_recycle_result_v1::<1>(Err(
            Gfx942FixedDispatchRecycleFailureV1 {
                error: Gfx942CompletionErrorV1::Recycle.into(),
                retryable_completed: None,
            },
        ))
        .expect_err("a consumed reset failure is terminal");
    assert!(terminal.retryable_completed.is_none());
    assert!(terminal_session.terminal_poisoned);
    assert!(take_dispatch_terminal_process_gate_record_v1());
}
