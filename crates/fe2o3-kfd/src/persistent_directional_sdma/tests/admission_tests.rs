use super::*;

#[test]
fn manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_ADAPTER_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        rendered,
        GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_ADAPTER_MANIFEST_SHA256_V1
    );
}

#[test]
fn window_manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_WINDOW_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        rendered,
        GFX942_PERSISTENT_DIRECTIONAL_LOCAL_SDMA_WINDOW_MANIFEST_SHA256_V1
    );
}

#[test]
fn window_clean_recovery_restores_the_exact_owner_pair() {
    for (id, direction) in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ]
    .into_iter()
    .enumerate()
    {
        let (allocation, host) = promoted_fixture(100 + id as u64, 2048);
        let identity = allocation.attachment.storage_identity;
        let (prepared, request, _) = prepared_window_fixture(allocation, host, direction, 3);
        let DirectionalPersistentSdmaWindowPublicationTransitionV1::Retryable { allocation, host } =
            transition_directional_persistent_sdma_window_publication_v1(
                prepared,
                DirectionalPersistentSdmaWindowPublicationObservationV1::Recoverable(request),
                true,
                true,
            )
        else {
            panic!("clean window rejection must restore exact custody")
        };
        assert_eq!(allocation.attachment.storage_identity, identity);
        assert_eq!(host.kind(), Gfx942SdmaBufferKindV1::HostVisibleCoherent);
        assert!(allocation.owner.local_native_is_attached_for_sdma());
    }
}

#[test]
fn window_retained_and_substituted_publications_quarantine_the_whole_roster() {
    let (allocation, host) = promoted_fixture(110, 2048);
    let (prepared, _, tickets) = prepared_window_fixture(
        allocation,
        host,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        3,
    );
    let DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_window_publication_v1(
            prepared,
            DirectionalPersistentSdmaWindowPublicationObservationV1::Retained(tickets),
            true,
            true,
        )
    else {
        panic!("retained window publication must be terminal")
    };
    assert_eq!(custody.packet_count(), 3);
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PreparedQueueRetained
    );

    let (allocation, host) = promoted_fixture(111, 2048);
    let (prepared, _, mut tickets) = prepared_window_fixture(
        allocation,
        host,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        3,
    );
    tickets.swap(0, 1);
    let DirectionalPersistentSdmaWindowPublicationTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_window_publication_v1(
            prepared,
            DirectionalPersistentSdmaWindowPublicationObservationV1::Confirmed(tickets),
            true,
            true,
        )
    else {
        panic!("reordered window tickets must be terminal")
    };
    assert_eq!(custody.packet_count(), 3);
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::PublishedQueueRetained
    );
}

#[test]
fn window_pending_timeout_and_exact_completion_are_aggregate() {
    let (submission, request) =
        published_window_fixture(120, Gfx942PersistentSdmaDirectionV1::DeviceToHost, 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::Pending(submission) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Pending,
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(submission.packet_count(), 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::Timeout(submission) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Timeout,
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(submission.packet_count(), 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        )
    else {
        unreachable!()
    };
    assert_eq!(completed.packet_count(), 3);
    assert_eq!(completed.copy_bytes(), 32);
    assert_eq!(completed.host_offset(), 8);
    assert_eq!(completed.device_offset(), 16);
    assert_eq!(
        completed.direction(),
        Gfx942PersistentSdmaDirectionV1::DeviceToHost
    );
    let (allocation, _, frontier) = completed.into_parts();
    let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
    assert!(allocation.owner.local_native_is_attached_for_sdma());
}

#[test]
fn poisoned_queue_promotion_preflight_preserves_completed_h2d_frontier() {
    let (submission, request) =
        published_window_fixture(122, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        )
    else {
        unreachable!()
    };
    let failure = match crate::queue::preserve_persistent_compute_ready_preflight_custody_v1(
        completed,
        true,
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "terminal queue session requires process teardown",
        )),
    ) {
        Ok(_) => panic!("a poisoned queue must reject H2D-ready promotion"),
        Err(failure) => failure,
    };
    let (_, custody) = failure.into_parts();
    let crate::Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(terminal) = custody
    else {
        panic!("terminal preflight must return opaque process-teardown custody")
    };
    let (allocation, _host, frontier) = terminal.completed.into_parts();
    assert_eq!(allocation.owner.retained_settled_use_count(), 1);
    let allocation = allocation
        .retire_settled_frontier_v1(frontier)
        .expect("preflight must not retire the completed H2D frontier");
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
}

#[test]
fn currentness_hash_failure_returns_opaque_completed_h2d_custody() {
    let (submission, request) =
        published_window_fixture(124, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        )
    else {
        unreachable!()
    };
    let failure = crate::queue::terminal_persistent_compute_ready_hash_failure_v1(
        crate::MemorySessionError::ProcessChanged.into(),
        completed,
    );
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::Memory(crate::MemorySessionError::ProcessChanged)
    ));
    let (_, custody) = failure.into_parts();
    let crate::Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(terminal) = custody
    else {
        panic!("currentness loss must seal exact completed-window custody")
    };
    let (allocation, _host, frontier) = terminal.completed.into_parts();
    assert_eq!(allocation.owner.retained_settled_use_count(), 1);
    let allocation = allocation
        .retire_settled_frontier_v1(frontier)
        .expect("hash failure must not retire the completed H2D frontier");
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
}

#[test]
fn terminal_directional_admission_absorbs_self_owned_invalid_geometry() {
    let (allocation, host) = promoted_fixture(125, 2048);
    let allocation_identity = allocation.attachment.storage_identity;
    let host_identity = host.storage_identity();
    let failure = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
        queue_key(),
        true,
        allocation,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        host,
        u64::MAX,
        u64::MAX,
        0,
    )
    .expect_err("terminal custody must dominate invalid single-copy geometry");
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(terminal) = custody
    else {
        panic!("self-owned terminal input must not return retryable custody")
    };
    assert_eq!(
        terminal.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored
    );
    let Gfx942DirectionalPersistentSdmaTerminalStateV1::AdmissionRestored { allocation, host } =
        terminal.state
    else {
        unreachable!()
    };
    assert_eq!(allocation.attachment.storage_identity, allocation_identity);
    assert_eq!(host.storage_identity(), host_identity);

    let (allocation, host) = promoted_fixture(126, 2048);
    let allocation_identity = allocation.attachment.storage_identity;
    let host_identity = host.storage_identity();
    let failure = crate::queue::admit_directional_persistent_sdma_window_input_v1(
        queue_key(),
        true,
        allocation,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        host,
        u64::MAX,
        u64::MAX,
        0,
    )
    .expect_err("terminal custody must dominate invalid window geometry");
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(terminal) =
        custody
    else {
        panic!("self-owned terminal window must not return retryable custody")
    };
    assert_eq!(terminal.packet_count(), 0);
    assert_eq!(
        terminal.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::AdmissionRestored
    );
    let Gfx942DirectionalPersistentSdmaWindowTerminalStateV1::AdmissionRestored {
        allocation,
        host,
    } = terminal.state
    else {
        unreachable!()
    };
    assert_eq!(allocation.attachment.storage_identity, allocation_identity);
    assert_eq!(host.storage_identity(), host_identity);
}

#[test]
fn terminal_receiver_returns_foreign_directional_inputs_exactly() {
    let (allocation, host) = promoted_fixture(127, 2048);
    let allocation_identity = allocation.attachment.storage_identity;
    let host_identity = host.storage_identity();
    let failure = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
        queue_key_with_generation(2),
        true,
        allocation,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        host,
        0,
        0,
        1,
    )
    .expect_err("foreign input must be returned before terminal absorption");
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable { allocation, host } =
        custody
    else {
        panic!("foreign input must remain retryable on its producing queue")
    };
    assert_eq!(allocation.attachment.storage_identity, allocation_identity);
    assert_eq!(host.storage_identity(), host_identity);
    let retry = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
        queue_key(),
        false,
        allocation,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        host,
        0,
        0,
        1,
    );
    assert!(retry.is_ok());

    let (allocation, host) = promoted_fixture(128, 2048);
    let allocation_identity = allocation.attachment.storage_identity;
    let host_identity = host.storage_identity();
    let failure = crate::queue::admit_directional_persistent_sdma_window_input_v1(
        queue_key_with_generation(2),
        true,
        allocation,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        host,
        0,
        0,
        1,
    )
    .expect_err("foreign window input must be returned before terminal absorption");
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable { allocation, host } =
        custody
    else {
        panic!("foreign window input must remain retryable on its producing queue")
    };
    assert_eq!(allocation.attachment.storage_identity, allocation_identity);
    assert_eq!(host.storage_identity(), host_identity);
    let retry = crate::queue::admit_directional_persistent_sdma_window_input_v1(
        queue_key(),
        false,
        allocation,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        host,
        0,
        0,
        1,
    );
    assert!(retry.is_ok());
}

#[test]
fn foreign_queue_ready_promotion_returns_exact_retryable_completed_receipt() {
    let (submission, request) =
        published_window_fixture(123, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::Completed(completed) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        )
    else {
        unreachable!()
    };
    let failure = match crate::queue::preserve_persistent_compute_ready_affiliation_v1(
        completed,
        queue_key_with_generation(2),
        true,
    ) {
        Ok(_) => panic!("foreign queue must not consume completed H2D custody"),
        Err(failure) => failure,
    };
    assert!(matches!(
        failure.error(),
        ComputeAqlQueueSessionErrorV1::DispatchBinding(
            crate::Gfx942DispatchBindingErrorV1::Poisoned
        )
    ));
    let (_, custody) = failure.into_parts();
    let crate::Gfx942PersistentComputeReadyFailureCustodyV1::ForeignQueue(completed) = custody
    else {
        panic!("foreign queue must return the original completed receipt")
    };
    let completed = crate::queue::preserve_persistent_compute_ready_affiliation_v1(
        completed,
        queue_key(),
        false,
    )
    .expect("the producing queue must accept the unchanged receipt");
    assert_eq!(completed.packet_count(), 3);
    let (allocation, _host, frontier) = completed.into_parts();
    let allocation = allocation.retire_settled_frontier_v1(frontier).unwrap();
    assert_eq!(allocation.owner.retained_settled_use_count(), 0);
}

#[test]
fn legacy_detached_window_gate_returns_exact_custody_in_both_directions() {
    for (ordinal, direction) in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ]
    .into_iter()
    .enumerate()
    {
        let (allocation, host) = promoted_fixture(130 + ordinal as u64, 4096);
        let allocation_identity = allocation.attachment.storage_identity;
        let host_identity = host.storage_identity();
        let failure = match crate::queue::preserve_directional_window_sdma_publication_custody_v1(
            true, direction, allocation, host,
        ) {
            Ok(_) => panic!("persistent compute must block directional window publication"),
            Err(failure) => failure,
        };
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                crate::Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, custody) = failure.into_parts();
        let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
            allocation,
            host,
        } = custody
        else {
            panic!("pure publication rejection must be retryable")
        };
        assert_eq!(allocation.attachment.storage_identity, allocation_identity);
        assert_eq!(host.storage_identity(), host_identity);
        assert_eq!(allocation.owner.live_use_count(), 0);
    }
}

#[test]
fn legacy_quiescence_gate_returns_exact_compute_input() {
    for (ordinal, direction) in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ]
    .into_iter()
    .enumerate()
    {
        let (active, _request) = published_window_fixture(140 + ordinal as u64, direction, 3);
        let (allocation, _host) = promoted_fixture(150 + ordinal as u64, 4096);
        let identity = allocation.attachment.storage_identity;
        let input = crate::Gfx942PersistentComputeInputV1::Uninitialized(allocation);
        let failure =
            match crate::queue::preserve_persistent_compute_bind_input_for_sdma_quiescence_v1(
                input,
                active.packet_count() == 0,
            ) {
                Ok(_) => panic!("active directional SDMA must block persistent compute bind"),
                Err(failure) => failure,
            };
        assert_eq!(active.packet_count(), 3);
        assert!(matches!(
            failure.error(),
            ComputeAqlQueueSessionErrorV1::DispatchBinding(
                crate::Gfx942DispatchBindingErrorV1::ResourcePhase
            )
        ));
        let (_, custody) = failure.into_parts();
        let crate::Gfx942PersistentComputeBindFailureCustodyV1::Retryable(recovered) = custody
        else {
            panic!("quiescence rejection returns exact retryable compute input")
        };
        let (allocation, initialization) = recovered.into_parts();
        let digest = initialization.authenticated_sha256();
        let initialized = initialization.is_fully_initialized();
        assert_eq!(allocation.attachment.storage_identity, identity);
        assert_eq!(digest, None);
        assert!(!initialized);
        assert_eq!(allocation.owner.live_use_count(), 0);
    }
}

#[test]
fn window_completion_metadata_mismatch_is_terminal() {
    let (submission, request) =
        published_window_fixture(121, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 2,
                },
            ),
            true,
        )
    else {
        panic!("partial window completion must retain terminal custody")
    };
    assert_eq!(custody.packet_count(), 3);
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
    );
}

#[test]
fn window_completion_offset_substitution_is_terminal() {
    let (submission, mut request) =
        published_window_fixture(122, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
    request.source_offset += 1;
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        )
    else {
        panic!("host-offset substitution must retain terminal custody")
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
    );

    let (submission, mut request) =
        published_window_fixture(123, Gfx942PersistentSdmaDirectionV1::HostToDevice, 3);
    request.destination_offset += 1;
    let DirectionalPersistentSdmaWindowCompletionTransitionV1::ProcessTeardown(custody) =
        transition_directional_persistent_sdma_window_completion_v1(
            submission,
            DirectionalPersistentSdmaWindowCompletionObservationV1::Completed(
                CompletedPersistentSdmaWindowV1 {
                    request,
                    packet_count: 3,
                },
            ),
            true,
        )
    else {
        panic!("device-offset substitution must retain terminal custody")
    };
    assert_eq!(
        custody.stage(),
        Gfx942DirectionalPersistentSdmaTerminalStageV1::CompletedUnrestored
    );
}

#[test]
fn pair_admission_rejects_swapped_engines_and_duplicate_children() {
    let pair = admit_persistent_directional_sdma_pair_v1(pair_observation(17, 23)).unwrap();
    assert_eq!(pair.host_to_device_queue_id, 17);
    assert_eq!(pair.device_to_host_queue_id, 23);

    let mut swapped = pair_observation(17, 23);
    swapped.host_to_device.engine_index = Some(GFX942_SDMA_D2H_ENGINE_INDEX_V1);
    swapped.device_to_host.engine_index = Some(GFX942_SDMA_H2D_ENGINE_INDEX_V1);
    assert!(admit_persistent_directional_sdma_pair_v1(swapped).is_err());
    assert!(admit_persistent_directional_sdma_pair_v1(pair_observation(17, 17)).is_err());
    let mut wrong_inventory = pair_observation(17, 23);
    wrong_inventory.admitted_queues_per_engine = 7;
    assert!(admit_persistent_directional_sdma_pair_v1(wrong_inventory).is_err());
}

#[test]
fn pooled_logical_extent_preserves_physical_owner_and_buffer_debit() {
    let (allocation, _host) = promoted_fixture(10, 2048);
    assert_eq!(allocation.byte_len(), 2048);
    assert_eq!(allocation.physical_byte_len(), 4096);
    let original_generation = allocation.attachment.pool_generation;
    let (device, outstanding) =
        demote_directional_persistent_sdma_custody_v1(allocation, 2).unwrap();
    assert_eq!(outstanding, 2);
    assert_eq!(device.requested_bytes(), 2048);
    assert_eq!(device.physical_bytes(), 4096);
    assert_eq!(device.pool_generation(), original_generation + 1);
    assert!(!directional_persistent_sdma_queue_destroy_is_admitted_v1(
        outstanding
    ));
    assert!(directional_persistent_sdma_queue_destroy_is_admitted_v1(0));
}

#[test]
fn extent_admission_is_bounded_and_page_rounded_only_physically() {
    assert!(directional_persistent_sdma_extents_are_admitted_v1(
        1, 4096, 1
    ));
    assert!(directional_persistent_sdma_extents_are_admitted_v1(
        2048, 4096, 1
    ));
    assert!(!directional_persistent_sdma_extents_are_admitted_v1(
        4097, 4096, 1
    ));
    assert!(!directional_persistent_sdma_extents_are_admitted_v1(
        1, 4095, 1
    ));
    assert!(!directional_persistent_sdma_extents_are_admitted_v1(
        1,
        GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_ALLOCATION_BYTES_V1 + 4096,
        1,
    ));
}

#[test]
fn promotion_and_demotion_failures_preserve_explicit_owner_custody() {
    let (device, _) = persistent_sdma_buffers_for_test(queue_key(), 80);
    let identity = device.storage_identity();
    let failure = classify_directional_persistent_sdma_promotion_failure_v1(
        ComputeAqlQueueSessionErrorV1::Contract("retryable promotion"),
        device,
        false,
    );
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaPromotionCustodyV1::Retryable(device) = custody else {
        panic!("recoverable promotion must return retryable device custody")
    };
    assert_eq!(device.storage_identity(), identity);

    let (device, _) = persistent_sdma_buffers_for_test(queue_key(), 81);
    let identity = device.storage_identity();
    let failure = classify_directional_persistent_sdma_promotion_failure_v1(
        ComputeAqlQueueSessionErrorV1::Contract("terminal promotion"),
        device,
        true,
    );
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(terminal) = custody
    else {
        panic!("terminal promotion must retain opaque device custody")
    };
    assert_eq!(terminal.buffer.storage_identity(), identity);

    let (allocation, _) = promoted_fixture(82, 2048);
    let identity = allocation.attachment.storage_identity;
    let failure = classify_directional_persistent_sdma_demotion_failure_v1(
        ComputeAqlQueueSessionErrorV1::Contract("retryable demotion"),
        allocation,
        false,
    );
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaDemotionCustodyV1::Retryable(allocation) = custody else {
        panic!("recoverable demotion must return retryable allocation custody")
    };
    assert_eq!(allocation.attachment.storage_identity, identity);

    let (allocation, _) = promoted_fixture(83, 2048);
    let identity = allocation.attachment.storage_identity;
    let failure = classify_directional_persistent_sdma_demotion_failure_v1(
        ComputeAqlQueueSessionErrorV1::Contract("terminal demotion"),
        allocation,
        true,
    );
    let (_, custody) = failure.into_parts();
    let Gfx942DirectionalPersistentSdmaDemotionCustodyV1::ProcessTeardown(terminal) = custody
    else {
        panic!("terminal demotion must retain opaque allocation custody")
    };
    assert_eq!(terminal.allocation.attachment.storage_identity, identity);
}

#[test]
fn live_r19_path_uses_physical_promotion_and_operational_hot_path_checks() {
    let live = crate::queue::live_production_source_for_tests_v1();
    let promotion = live
        .split("pub fn promote_sdma_device_buffer_to_directional_persistent_allocation_v1")
        .nth(1)
        .unwrap()
        .split("pub fn demote_directional_persistent_allocation_to_sdma_device_buffer_v1")
        .next()
        .unwrap();
    assert!(promotion.contains("sdma_promotion::promote_in_place(self, buffer)"));
    let promotion_driver = include_str!("../../queue_live/sdma_promotion.rs");
    assert!(promotion_driver.contains("validate_physical_device_mapping"));
    assert!(!promotion_driver.contains("checked_gpu_subrange"));
    assert!(!promotion.contains("checked_gpu_subrange"));

    let synchronous = include_str!("../../queue_live/sdma_synchronous.rs");
    let opening_scope = synchronous
        .split("fn opening(&mut self)")
        .nth(2)
        .unwrap()
        .split("fn loan(&mut self)")
        .next()
        .unwrap();
    assert_eq!(
        opening_scope
            .matches("check_queue_operational_currentness")
            .count(),
        1
    );
    let admitted_preparation = live
        .split("fn prepare_admitted_directional_persistent_sdma_request_v1")
        .nth(1)
        .unwrap()
        .split("pub fn submit_directional_persistent_sdma_copy_v1")
        .next()
        .unwrap();
    assert!(admitted_preparation.contains("allocation.owner.reserve("));
    assert!(admitted_preparation.contains("allocation.owner.prepare("));
    assert!(admitted_preparation.contains("allocation.owner.detach_sdma_buffer("));
    assert!(!admitted_preparation.contains("with_sdma_owner_memory"));
    assert!(!admitted_preparation.contains("check_queue_operational_currentness"));

    let submission = live
        .split("pub fn submit_directional_persistent_sdma_copy_v1")
        .nth(1)
        .unwrap()
        .split("pub fn execute_synchronous_directional_persistent_sdma_copy_for_v1")
        .next()
        .unwrap();
    assert_eq!(submission.matches("with_sdma_owner_memory").count(), 1);
    assert!(!submission.contains("check_directional_persistent_sdma_operational_currentness"));
    let admission = submission
        .find("admit_directional_persistent_sdma_request_v1")
        .unwrap();
    let loan = submission.find("with_sdma_owner_memory").unwrap();
    let opening = submission
        .find("if let Err(error) = memory.check_queue_operational_currentness()")
        .unwrap();
    let request_preparation = submission
        .find("prepare_admitted_directional_persistent_sdma_request_v1")
        .unwrap();
    let lower_preparation = submission
        .find("prepare_directional_persistent_single_recoverable")
        .unwrap();
    let prepublication = submission[opening + 1..]
        .find("if let Err(error) = memory.check_queue_operational_currentness()")
        .map(|offset| opening + 1 + offset)
        .unwrap();
    let handoff = submission
        .find("DirectionalPersistentSdmaSinglePreparedHandoffV1")
        .unwrap();
    let publication = submission.find("handoff.publish(owner, memory)").unwrap();
    let final_close = submission
        .rfind("memory.check_queue_operational_currentness()")
        .unwrap();
    assert!(admission < loan);
    assert!(loan < opening);
    assert!(opening < request_preparation);
    assert!(request_preparation < lower_preparation);
    assert!(lower_preparation < prepublication);
    assert!(prepublication < handoff);
    assert!(handoff < publication);
    let handoff_to_publication = &submission[handoff..publication];
    assert!(!handoff_to_publication.contains("return Err"));
    assert!(!handoff_to_publication.contains('?'));
    assert!(!handoff_to_publication.contains("check_"));
    assert!(publication < final_close);
    assert!(submission.contains("OpeningCurrentnessLost"));
    assert!(submission.contains("RequestPreparationRejected"));
    assert!(submission.contains("LowerPreparationRejected"));
    assert!(submission.contains("finish_asynchronous_directional_persistent_sdma_single_v1"));
    let finish = live
        .split("fn finish_asynchronous_directional_persistent_sdma_single_v1")
        .nth(1)
        .unwrap()
        .split("fn finish_directional_persistent_sdma_publication_transition")
        .next()
        .unwrap();
    assert!(finish.contains("loan_error.is_none() && preparation_succeeded"));
    assert!(finish.contains("loan_error.is_none() && closing_currentness_succeeded"));
    assert!(!submission.contains("self.check_currentness()"));
    assert!(submission.contains("prepare_directional_persistent_single_recoverable"));
    assert!(!submission.contains("owner.prepare_single_recoverable"));
    assert!(!submission.contains("vec![request]"));

    let window = live
        .split("pub fn submit_directional_persistent_sdma_window_v1")
        .nth(1)
        .unwrap()
        .split("pub fn poll_directional_persistent_sdma_window_v1")
        .next()
        .unwrap();
    assert_eq!(
        window
            .matches("check_directional_persistent_sdma_operational_currentness")
            .count(),
        3
    );
    assert_eq!(
        window
            .matches("check_queue_operational_currentness")
            .count(),
        1
    );
    let roster_allocation = window.find("try_reserve_exact(packet_count)").unwrap();
    let preparation_open = window
        .find("check_directional_persistent_sdma_operational_currentness")
        .unwrap();
    let roster_count_guard = window
        .find("if prepared.tickets().len() != packet_count")
        .unwrap();
    let roster_population = window
        .find("planned_tickets.extend_from_slice(prepared.tickets())")
        .unwrap();
    let shared_close_open = window
        .find("if let Err(error) = memory.check_queue_operational_currentness()")
        .unwrap();
    let handoff = window
        .find("DirectionalPersistentSdmaWindowPreparedHandoffV1")
        .unwrap();
    let publication = window.find("handoff.publish(owner, memory)").unwrap();
    let failed_prepare = window.find("if !handoff_attempted").unwrap();
    let handoff_failure = window.find("let Some((handoff_direction").unwrap();
    let missing_publication = window
        .find("directional persistent SDMA window handoff did not publish")
        .unwrap();
    let final_close = window
        .rfind("check_directional_persistent_sdma_operational_currentness")
        .unwrap();
    assert!(roster_allocation < preparation_open);
    assert!(preparation_open < roster_count_guard);
    assert!(roster_count_guard < roster_population);
    assert!(roster_population < shared_close_open);
    assert!(shared_close_open < handoff);
    assert!(handoff < publication);
    let handoff_to_publication = &window[handoff..publication];
    assert!(!handoff_to_publication.contains("return Err"));
    assert!(!handoff_to_publication.contains('?'));
    assert!(!handoff_to_publication.contains("check_"));
    assert!(publication < failed_prepare);
    assert!(failed_prepare < handoff_failure);
    assert!(handoff_failure < missing_publication);
    assert!(missing_publication < final_close);
    let failed_prepare_path = &window[failed_prepare..handoff_failure];
    assert_eq!(
        failed_prepare_path
            .matches("check_directional_persistent_sdma_operational_currentness")
            .count(),
        1
    );
    assert!(failed_prepare_path.contains("preparation_contract_failed"));
    let handoff_failure_path = &window[handoff_failure..final_close];
    assert!(
        handoff_failure_path
            .contains("terminal_prepared_directional_persistent_sdma_window_failure")
    );
    assert!(handoff_failure_path.contains("prepared_without_handoff"));

    let same_device = live
        .split("pub fn submit_same_device_persistent_sdma_window_v1")
        .nth(1)
        .unwrap()
        .split("pub fn poll_same_device_persistent_sdma_window_v1")
        .next()
        .unwrap();
    assert!(!same_device.contains("PreparedHandoffV1"));

    let lower = include_str!("../../sdma.rs")
        .split("pub(crate) fn poll(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn wait_for(")
        .next()
        .unwrap();
    assert_eq!(
        lower.matches("check_queue_operational_currentness").count(),
        3
    );
    assert!(lower.contains("if observed == 0"));
    assert!(lower.contains("return Ok(Gfx942SdmaCopyPollV1::Pending)"));

    let pair = admit_persistent_directional_sdma_pair_v1(pair_observation(17, 23)).unwrap();
    assert_eq!(
        pair.queue_id(Gfx942PersistentSdmaDirectionV1::HostToDevice),
        17
    );
    assert_eq!(
        pair.queue_id(Gfx942PersistentSdmaDirectionV1::DeviceToHost),
        23
    );
}
