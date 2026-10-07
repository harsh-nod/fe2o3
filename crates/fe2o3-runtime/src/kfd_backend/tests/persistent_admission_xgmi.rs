use super::*;

#[test]
fn persistent_full_range_compute_admission_is_exact_and_address_free() {
    let byte_len = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1;
    let mut allocation = AllocationRecordV1 {
        device: 7,
        kind: RuntimeMemoryKindV1::DeviceLocal,
        alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
        bytes: vec![3_u8; byte_len as usize].into(),
        content_sha256: Some(Sha256::digest(vec![3_u8; byte_len as usize]).into()),
        last_full_host_write: None,
        native_dirty: Vec::new(),
        sdma_storage: KfdRuntimeSdmaStorageV1::Synthetic,
        sdma_backed: true,
        sdma_initialized: true,
        sdma_shadow_dirty: false,
        persistent_storage_restore: None,
        #[cfg(test)]
        scripted_three_binding_replay: false,
    };
    let binding = BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 11,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len,
        },
        kernarg_byte_offset: 8,
    };
    let ready = PersistentComputeReadyFactsV1 {
        logical_bytes: byte_len,
        physical_bytes: byte_len,
        authenticated_sha256: allocation.content_sha256.unwrap(),
    };
    assert_eq!(
        persistent_full_range_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &[binding],
            7,
            Some(&allocation),
            Some(ready),
        ),
        Some(PersistentFullRangeComputeAdmissionV1 {
            allocation: 11,
            access: RuntimeAccessV1::Read,
            source: PersistentFullRangeComputeSourceV1::AuthenticatedH2d,
        })
    );
    let dispatch_shape_sha256 = [0x42; 32];
    let authenticated = persistent_full_range_compute_admission_v1(
        KfdRuntimeSemanticLaunchV1::Ordinary,
        &[binding],
        7,
        Some(&allocation),
        Some(ready),
    );
    assert!(persistent_control_is_reused_v1(
        Some(RetainedPersistentDispatchV1 {
            allocation: binding.region.allocation,
            dispatch_shape_sha256,
        }),
        authenticated,
        dispatch_shape_sha256,
    ));
    assert!(!persistent_control_is_reused_v1(
        Some(RetainedPersistentDispatchV1 {
            allocation: binding.region.allocation,
            dispatch_shape_sha256: [0x43; 32],
        }),
        authenticated,
        dispatch_shape_sha256,
    ));
    assert!(
        persistent_full_range_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &[binding, binding],
            7,
            Some(&allocation),
            Some(ready),
        )
        .is_none()
    );
    assert!(
        persistent_full_range_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    byte_offset: 1,
                    byte_len: byte_len - 1,
                    ..binding.region
                },
                ..binding
            }],
            7,
            Some(&allocation),
            Some(ready),
        )
        .is_none()
    );
    assert!(
        persistent_full_range_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Atomic(atomic_contract_v1()),
            &[binding],
            7,
            Some(&allocation),
            Some(ready),
        )
        .is_none()
    );
    allocation.sdma_shadow_dirty = true;
    assert!(
        persistent_full_range_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &[binding],
            7,
            Some(&allocation),
            Some(ready),
        )
        .is_none()
    );
    allocation.sdma_shadow_dirty = false;
    allocation.content_sha256 = Some([0x11; 32]);
    assert!(
        persistent_full_range_compute_admission_v1(
            KfdRuntimeSemanticLaunchV1::Ordinary,
            &[binding],
            7,
            Some(&allocation),
            Some(ready),
        )
        .is_none()
    );

    assert_eq!(admitted_compute_lane_v1(Some(0), true), Some(0));
    assert_eq!(admitted_compute_lane_v1(Some(1), true), None);
    assert_eq!(admitted_compute_lane_v1(None, true), None);
    assert_eq!(admitted_compute_lane_v1(Some(1), false), Some(1));

    allocation.content_sha256 = Some(ready.authenticated_sha256);
    allocation.last_full_host_write =
        Some((Arc::clone(&allocation.bytes), ready.authenticated_sha256));
    apply_persistent_compute_effect_v1(&mut allocation, Gfx942PersistentComputeEffectV1::Read);
    assert_eq!(allocation.content_sha256, Some(ready.authenticated_sha256));
    assert!(!allocation.sdma_shadow_dirty);
    apply_persistent_compute_effect_v1(&mut allocation, Gfx942PersistentComputeEffectV1::Write);
    assert_eq!(allocation.content_sha256, None);
    assert!(allocation.last_full_host_write.is_none());
    assert!(allocation.sdma_shadow_dirty);
    assert!(allocation.native_dirty.is_empty());

    let observation = KfdRuntimeLaunchPerformanceV1 {
        data_path: KfdRuntimeLaunchDataPathV1::PersistentDeviceReused,
        user_data_materializations: 0,
        ..KfdRuntimeLaunchPerformanceV1::default()
    };
    assert_eq!(
        observation.data_path(),
        KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
    );
    assert_eq!(observation.user_data_materializations(), 0);
}

#[test]
fn unticketed_xgmi_dependency_failure_is_observable_without_publication() {
    let active = synthetic_xgmi_submission_v1(2, 3, 4, 5, vec![1]);
    let mut completed = HashMap::new();
    completed.insert(
        1,
        SubmissionRecordV1 {
            stream: 3,
            status: BackendPollV1::Failed { code: -7 },
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    assert!(xgmi_submission_has_failed_dependency_v1(
        &active, &completed
    ));
    assert!(!xgmi_submission_is_ready_v1(&active, &completed, 0));
}

#[test]
fn native_xgmi_pair_and_capability_admission_fail_closed() {
    assert_eq!(admit_xgmi_unique_id_pair_v1(11, 22), Ok(()));
    assert_eq!(
        admit_xgmi_unique_id_pair_v1(0, 22),
        Err(XgmiPairAdmissionErrorV1::ZeroUniqueId)
    );
    assert_eq!(
        admit_xgmi_unique_id_pair_v1(11, 0),
        Err(XgmiPairAdmissionErrorV1::ZeroUniqueId)
    );
    assert_eq!(
        admit_xgmi_unique_id_pair_v1(11, 11),
        Err(XgmiPairAdmissionErrorV1::DuplicateUniqueId)
    );

    fn assert_runtime_extensions<T>()
    where
        T: RuntimeBackendV1
            + RuntimeAsyncCopyBackendV1
            + RuntimeAtomicBackendV1
            + RuntimeCancellationBackendV1
            + RuntimeCollectiveBackendV1
            + RuntimeFlushBackendV1,
    {
    }
    assert_runtime_extensions::<KfdNativeXgmiRuntimeBackendV1>();
    let capabilities = native_xgmi_execution_capabilities_v1();
    assert!(capabilities.native_peer_copy);
    assert!(capabilities.cancellation);
    assert!(!capabilities.native_async_copy);
    assert!(!capabilities.concurrent_compute);
    assert!(!capabilities.compute_copy_overlap);
    assert!(!capabilities.memory_pool);
    assert!(!capabilities.profiling);
    assert!(!capabilities.atomics);
    assert!(!capabilities.collectives);

    for failure in [
        reject_native_xgmi_semantic_submission_v1(
            BackendSemanticLaunchV1::Atomic(atomic_contract_v1()),
            true,
        ),
        reject_native_xgmi_semantic_submission_v1(
            BackendSemanticLaunchV1::Collective(collective_contract_v1()),
            false,
        ),
    ] {
        let RuntimeBackendFailureV1::Rejected(error) = failure else {
            panic!("unsupported native XGMI semantics must reject before custody");
        };
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Unsupported);
    }
    let RuntimeBackendFailureV1::Rejected(error) = reject_native_xgmi_semantic_submission_v1(
        BackendSemanticLaunchV1::Collective(collective_contract_v1()),
        true,
    ) else {
        panic!("mismatched native XGMI semantic variant must reject");
    };
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
}

#[test]
fn native_xgmi_batch_selection_is_ready_directional_bounded_and_ordered() {
    let mut active = HashMap::new();
    active.insert(9, synthetic_xgmi_submission_v1(9, 1, 10, 11, vec![]));
    active.insert(3, synthetic_xgmi_submission_v1(3, 2, 12, 13, vec![70]));
    active.insert(5, synthetic_xgmi_submission_v1(5, 3, 14, 15, vec![71]));
    let mut reverse = synthetic_xgmi_submission_v1(4, 4, 16, 17, vec![]);
    reverse.direction = 1;
    active.insert(4, reverse);

    let mut completed = HashMap::new();
    completed.insert(
        70,
        SubmissionRecordV1 {
            stream: 8,
            status: BackendPollV1::Succeeded,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    completed.insert(
        71,
        SubmissionRecordV1 {
            stream: 8,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );

    assert_eq!(
        ready_xgmi_batch_ids_v1(&active, &completed, 0, 8).unwrap(),
        vec![3, 9]
    );
    assert_eq!(
        ready_xgmi_batch_ids_v1(&active, &completed, 0, 1).unwrap(),
        vec![3]
    );
    assert_eq!(
        ready_xgmi_batch_ids_v1(&active, &completed, 1, 8).unwrap(),
        vec![4]
    );
    assert!(
        ready_xgmi_batch_ids_v1(&active, &completed, 2, 8)
            .unwrap()
            .is_empty()
    );

    let mut oversized = HashMap::new();
    for id in 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 2 {
        oversized.insert(
            id,
            synthetic_xgmi_submission_v1(id, id, id * 2, id * 2 + 1, vec![]),
        );
    }
    let admitted =
        ready_xgmi_batch_ids_v1(&oversized, &HashMap::new(), 0, GFX942_SDMA_MAX_IN_FLIGHT_V1)
            .unwrap();
    assert_eq!(admitted.len(), GFX942_SDMA_MAX_IN_FLIGHT_V1);
    assert_eq!(admitted[0], 1);
    assert_eq!(
        admitted[GFX942_SDMA_MAX_IN_FLIGHT_V1 - 1],
        GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64
    );

    // A caller focused beyond the first admitted ring batch advances one
    // published ticket per poll instead of waiting on an unrelated handle
    // forever. Once that batch drains, the focus can enter the next batch.
    let focus = GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 2;
    for completed in 0..GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 {
        let in_flight = (completed + 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64).collect::<Vec<_>>();
        assert_eq!(
            indexed_xgmi_progress_id_v1(&in_flight, focus),
            Some(completed + 1)
        );
    }
    assert_eq!(indexed_xgmi_progress_id_v1(&[], focus), None);
}

#[test]
fn native_xgmi_recoverable_batch_failure_settles_every_logical_owner() {
    let first = synthetic_xgmi_submission_v1(40, 4, 10, 11, vec![70]);
    let second = synthetic_xgmi_submission_v1(41, 5, 12, 13, vec![70, 71]);
    let mut dependency_retains = HashMap::from([(70, 2), (71, 1)]);
    let mut submissions = HashMap::new();
    let mut completion_reservations = 0;
    reserve_xgmi_completion_slot_v1(&mut submissions, &mut completion_reservations).unwrap();
    reserve_xgmi_completion_slot_v1(&mut submissions, &mut completion_reservations).unwrap();
    let completion_capacity = submissions.capacity();

    finish_failed_xgmi_batch_records_v1(
        &mut dependency_retains,
        &mut submissions,
        &mut completion_reservations,
        [first, second],
    );

    assert!(dependency_retains.is_empty());
    assert_eq!(completion_reservations, 0);
    assert_eq!(submissions.capacity(), completion_capacity);
    let first = submissions.get(&40).expect("first failure record");
    assert_eq!(first.stream, 4);
    assert_eq!(
        first.status,
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
        }
    );
    let second = submissions.get(&41).expect("second failure record");
    assert_eq!(second.stream, 5);
    assert_eq!(
        second.status,
        BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
        }
    );
}

#[test]
fn native_xgmi_completion_slots_cover_every_outstanding_submission() {
    const OUTSTANDING: u64 = 1_024;

    let mut submissions = HashMap::new();
    let mut completion_reservations = 0;
    let active = (1..=OUTSTANDING)
        .map(|id| synthetic_xgmi_submission_v1(id, id, id * 2, id * 2 + 1, vec![]))
        .collect::<Vec<_>>();
    for expected in 1..=OUTSTANDING as usize {
        reserve_xgmi_completion_slot_v1(&mut submissions, &mut completion_reservations).unwrap();
        assert_eq!(completion_reservations, expected);
        assert!(
            submissions.capacity().saturating_sub(submissions.len()) >= completion_reservations
        );
    }

    let reserved_capacity = submissions.capacity();
    let mut dependency_retains = HashMap::new();
    for (settled, active) in active.into_iter().enumerate() {
        settle_xgmi_submission_record_v1(
            &mut dependency_retains,
            &mut submissions,
            &mut completion_reservations,
            active,
            BackendPollV1::Succeeded,
        );
        assert_eq!(completion_reservations, OUTSTANDING as usize - settled - 1);
        assert_eq!(submissions.capacity(), reserved_capacity);
        assert!(
            submissions.capacity().saturating_sub(submissions.len()) >= completion_reservations
        );
    }
    assert_eq!(submissions.len(), OUTSTANDING as usize);
    assert_eq!(completion_reservations, 0);
}

#[test]
fn native_xgmi_ready_and_in_flight_indexes_remain_bounded_under_stress() {
    const READY: u64 = 16_384;

    let mut ready = VecDeque::new();
    ready.try_reserve_exact(READY as usize).unwrap();
    for id in 1..=READY {
        enqueue_xgmi_ready_id_v1(&mut ready, id);
    }
    assert!(remove_xgmi_ready_id_v1(&mut ready, READY / 2));
    enqueue_xgmi_ready_id_v1(&mut ready, READY / 2);

    let mut observed = 0;
    while !ready.is_empty() {
        let batch_len = ready.len().min(GFX942_SDMA_MAX_IN_FLIGHT_V1);
        let mut in_flight = Vec::new();
        in_flight.try_reserve_exact(batch_len).unwrap();
        for _ in 0..batch_len {
            let id = ready.pop_front().unwrap();
            insert_ordered_xgmi_id_v1(&mut in_flight, id);
        }
        assert!(in_flight.len() <= GFX942_SDMA_MAX_IN_FLIGHT_V1);
        let focus = READY + 1;
        while let Some(id) = indexed_xgmi_progress_id_v1(&in_flight, focus) {
            assert!(remove_ordered_xgmi_id_v1(&mut in_flight, id));
            observed += 1;
        }
    }
    assert_eq!(observed, READY);
}

#[test]
fn native_xgmi_completed_ticket_bypasses_a_large_ready_backlog() {
    const READY: u64 = 131_072;
    const COMPLETED: u64 = READY / 2;

    let mut ready = VecDeque::new();
    ready.try_reserve_exact(READY as usize).unwrap();
    ready.extend(1..=READY);
    let expected_ready = ready.clone();
    let mut in_flight = Vec::with_capacity(GFX942_SDMA_MAX_IN_FLIGHT_V1);
    in_flight.push(COMPLETED);

    // Mirroring the marker in the hostile test backlog makes index-order
    // observable: an implementation that searches ready first removes it.
    assert_eq!(
        remove_xgmi_progress_index_v1(&mut ready, false, &mut in_flight, COMPLETED),
        XgmiProgressIndexPhaseV1::InFlight
    );
    assert!(in_flight.is_empty());
    assert_eq!(ready, expected_ready);
}

#[test]
fn native_xgmi_recoverable_prefix_restoration_preserves_fifo_order() {
    let mut ready = VecDeque::new();
    ready.try_reserve_exact(8).unwrap();
    ready.extend([40, 50, 60]);

    for id in [10, 20, 30].into_iter().rev() {
        prepend_xgmi_ready_id_v1(&mut ready, id);
    }

    assert_eq!(
        ready.into_iter().collect::<Vec<_>>(),
        [10, 20, 30, 40, 50, 60]
    );
}

#[test]
fn native_xgmi_partial_reverse_restoration_keeps_each_restored_owner_indexed() {
    let mut ready = VecDeque::new();
    ready.try_reserve_exact(8).unwrap();
    ready.extend([40, 50, 60]);

    // Reverse restoration has completed owners 30 and 20 when restoring
    // owner 10 fails. Both completed owners remain a FIFO prefix.
    prepend_xgmi_ready_id_v1(&mut ready, 30);
    prepend_xgmi_ready_id_v1(&mut ready, 20);

    assert_eq!(ready.into_iter().collect::<Vec<_>>(), [20, 30, 40, 50, 60]);
}

#[test]
fn native_xgmi_flush_admission_is_complete_bounded_and_nonmutating() {
    assert_eq!(xgmi_direction_for_destination_v1(0), Some(1));
    assert_eq!(xgmi_direction_for_destination_v1(1), Some(0));
    assert_eq!(xgmi_direction_for_destination_v1(2), None);
    assert_eq!(
        classify_xgmi_flush_v1(0, false, GFX942_SDMA_MAX_IN_FLIGHT_V1),
        XgmiFlushAdmissionV1::NoReadyWork
    );
    assert_eq!(
        classify_xgmi_flush_v1(1, true, GFX942_SDMA_MAX_IN_FLIGHT_V1),
        XgmiFlushAdmissionV1::InFlight
    );

    let mut active = HashMap::new();
    for id in 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 1 {
        active.insert(
            id,
            synthetic_xgmi_submission_v1(id, id, id * 2, id * 2 + 1, vec![]),
        );
    }
    let before: Vec<_> = {
        let mut ids: Vec<_> = active.keys().copied().collect();
        ids.sort_unstable();
        ids
    };
    assert_eq!(
        classify_xgmi_flush_v1(active.len(), false, GFX942_SDMA_MAX_IN_FLIGHT_V1),
        XgmiFlushAdmissionV1::Capacity
    );
    let mut after: Vec<_> = active.keys().copied().collect();
    after.sort_unstable();
    assert_eq!(after, before);
    assert!(
        active
            .values()
            .all(|submission| submission.ticket.is_none())
    );
    assert_eq!(
        classify_xgmi_flush_v1(1, false, 0),
        XgmiFlushAdmissionV1::Capacity
    );

    active.remove(&(GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64 + 1));
    assert_eq!(
        classify_xgmi_flush_v1(active.len(), false, GFX942_SDMA_MAX_IN_FLIGHT_V1),
        XgmiFlushAdmissionV1::Publish {
            ready: GFX942_SDMA_MAX_IN_FLIGHT_V1,
        }
    );
    assert_eq!(
        ready_xgmi_batch_ids_v1(&active, &HashMap::new(), 0, GFX942_SDMA_MAX_IN_FLIGHT_V1,)
            .unwrap(),
        (1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64).collect::<Vec<_>>()
    );
}

#[test]
fn native_xgmi_flush_never_enters_publication_when_the_ready_set_cannot_fit() {
    for ready in [GFX942_SDMA_MAX_IN_FLIGHT_V1 + 1, 4096, usize::MAX] {
        let mut publication_calls = 0;
        let result = publish_xgmi_flush_v1(ready, false, || {
            publication_calls += 1;
            Ok(XgmiBatchPublicationOutcomeV1::Published)
        });
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
        assert_eq!(publication_calls, 0);
    }
    for ready in [1, GFX942_SDMA_MAX_IN_FLIGHT_V1, usize::MAX] {
        let result = publish_xgmi_flush_v1(ready, true, || {
            panic!("flush must not enter a busy native publication window")
        });
        assert!(matches!(
            result,
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
    }
    for in_flight in [false, true] {
        assert_eq!(
            publish_xgmi_flush_v1(0, in_flight, || {
                panic!("empty flush must not publish or wait for in-flight work")
            })
            .unwrap(),
            XgmiBatchPublicationOutcomeV1::NoReadyWork
        );
    }
}

#[test]
fn native_xgmi_flush_publishes_one_complete_window_and_preserves_failures() {
    for ready in 1..=GFX942_SDMA_MAX_IN_FLIGHT_V1 {
        let mut publication_calls = 0;
        assert_eq!(
            publish_xgmi_flush_v1(ready, false, || {
                publication_calls += 1;
                Ok(XgmiBatchPublicationOutcomeV1::Published)
            })
            .unwrap(),
            XgmiBatchPublicationOutcomeV1::Published
        );
        assert_eq!(publication_calls, 1);
    }
    for class in 0..3 {
        let mut publication_calls = 0;
        let result = publish_xgmi_flush_v1(GFX942_SDMA_MAX_IN_FLIGHT_V1, false, || {
            publication_calls += 1;
            let error = KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Native,
                "injected native publication failure",
            );
            Err(match class {
                0 => RuntimeBackendFailureV1::Rejected(error),
                1 => RuntimeBackendFailureV1::Quiescent(error),
                _ => RuntimeBackendFailureV1::Terminal(error),
            })
        });
        let error = match (class, result) {
            (0, Err(RuntimeBackendFailureV1::Rejected(error)))
            | (1, Err(RuntimeBackendFailureV1::Quiescent(error)))
            | (2, Err(RuntimeBackendFailureV1::Terminal(error))) => error,
            other => panic!("flush changed the native failure class: {other:?}"),
        };
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Native);
        assert_eq!(error.detail(), "injected native publication failure");
        assert_eq!(publication_calls, 1);
    }
}

#[test]
fn native_xgmi_scalar_flush_has_no_completion_or_backoff_path() {
    let source = include_str!("../../kfd_backend.rs");
    let flush = source
        .split("impl RuntimeFlushBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("impl RuntimeCancellationBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
        .next()
        .unwrap();
    let scalar = flush.split("let ready_at_entry =").nth(1).unwrap();
    assert_eq!(scalar.matches("publish_xgmi_flush_v1(").count(), 1);
    assert_eq!(scalar.matches("self.publish_ready_peer_batch(").count(), 1);
    for forbidden in [
        "loop {",
        "while ",
        "progress_peer_copy(",
        "wait",
        "sleep",
        "backoff",
    ] {
        assert!(
            !scalar.contains(forbidden),
            "unexpected scalar flush path: {forbidden}"
        );
    }
}

#[test]
fn native_xgmi_peer_admission_binds_direction_and_rejects_hostile_ranges() {
    let forward = XgmiPeerCopyAdmissionV1 {
        stream_device: 1,
        source_device: 0,
        destination_device: 1,
        source_offset: 8,
        source_len: 16,
        source_allocation_len: 32,
        source_access: RuntimeAccessV1::Read,
        destination_offset: 4,
        destination_len: 16,
        destination_allocation_len: 32,
        destination_access: RuntimeAccessV1::Write,
    };
    assert_eq!(admit_xgmi_peer_copy_v1(forward), Ok(0));
    assert_eq!(
        admit_xgmi_peer_copy_v1(XgmiPeerCopyAdmissionV1 {
            stream_device: 0,
            source_device: 1,
            destination_device: 0,
            ..forward
        }),
        Ok(1)
    );

    let mutations = [
        (
            XgmiPeerCopyAdmissionV1 {
                source_device: 2,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::UnknownDevice,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                destination_device: 0,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::SameDevice,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                stream_device: 0,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::WrongDestinationStream,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                source_len: 0,
                destination_len: 0,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::ZeroLength,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                destination_len: 15,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::LengthMismatch,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                source_len: u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) + 1,
                destination_len: u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) + 1,
                source_allocation_len: u64::MAX,
                destination_allocation_len: u64::MAX,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::PacketTooLarge,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                source_offset: u64::MAX,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::SourceRange,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                destination_offset: 17,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::DestinationRange,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                source_access: RuntimeAccessV1::Write,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::SourceAccess,
        ),
        (
            XgmiPeerCopyAdmissionV1 {
                destination_access: RuntimeAccessV1::Read,
                ..forward
            },
            XgmiPeerCopyAdmissionErrorV1::DestinationAccess,
        ),
    ];
    for (request, expected) in mutations {
        assert_eq!(admit_xgmi_peer_copy_v1(request), Err(expected));
    }
}

#[test]
fn native_xgmi_dependency_and_pending_ownership_rules_are_bounded() {
    let events = HashMap::from([
        (10, EventRecordV1 { submission: 100 }),
        (11, EventRecordV1 { submission: 101 }),
        (12, EventRecordV1 { submission: 100 }),
    ]);
    assert_eq!(
        collect_xgmi_dependencies_v1(&events, &[10, 11]),
        Ok(vec![100, 101])
    );
    assert_eq!(
        collect_xgmi_dependencies_v1(&events, &[99]),
        Err(XgmiDependencyAdmissionErrorV1::Unknown)
    );
    assert_eq!(
        collect_xgmi_dependencies_v1(&events, &[10, 12]),
        Err(XgmiDependencyAdmissionErrorV1::Duplicate)
    );
    assert_eq!(
        collect_xgmi_dependencies_v1(&events, &vec![10; MAX_RUNTIME_DEPENDENCIES_V1 + 1]),
        Err(XgmiDependencyAdmissionErrorV1::TooMany)
    );

    let active = synthetic_xgmi_submission_v1(100, 7, 20, 21, Vec::new());
    assert!(xgmi_allocation_is_active_v1([&active].into_iter(), 20));
    assert!(xgmi_allocation_is_active_v1([&active].into_iter(), 21));
    assert!(!xgmi_allocation_is_active_v1([&active].into_iter(), 22));
    assert!(has_active_xgmi_stream_v1([&active].into_iter(), 7));
    assert!(!has_active_xgmi_stream_v1([&active].into_iter(), 8));
    let mut depths = HashMap::from([(100, 1), (101, 255)]);
    assert_eq!(next_xgmi_dependency_depth_v1(&depths, &[100]), Ok(2));
    assert_eq!(next_xgmi_dependency_depth_v1(&depths, &[101]), Ok(256));
    depths.insert(102, 256);
    assert_eq!(
        next_xgmi_dependency_depth_v1(&depths, &[102]),
        Err(XgmiDependencyAdmissionErrorV1::TooMany)
    );
    assert_eq!(
        next_xgmi_dependency_depth_v1(&depths, &[999]),
        Err(XgmiDependencyAdmissionErrorV1::Unknown)
    );
}

#[test]
fn native_xgmi_cancellation_and_shutdown_preserve_phase_custody() {
    assert_eq!(
        xgmi_cancellation_disposition_v1(Some(false), false),
        XgmiCancellationDispositionV1::CancelPrepublication
    );
    assert_eq!(
        xgmi_cancellation_disposition_v1(Some(true), false),
        XgmiCancellationDispositionV1::TooLate
    );
    assert_eq!(
        xgmi_cancellation_disposition_v1(None, true),
        XgmiCancellationDispositionV1::TooLate
    );
    assert_eq!(
        xgmi_cancellation_disposition_v1(None, false),
        XgmiCancellationDispositionV1::Unknown
    );

    assert!(XgmiLogicalResourceCountsV1::default().permits_shutdown());
    for occupied in 0..16 {
        let mut resources = XgmiLogicalResourceCountsV1::default();
        match occupied {
            0 => resources.streams = 1,
            1 => resources.allocations = 1,
            2 => resources.submissions = 1,
            3 => resources.active = 1,
            4 => resources.events = 1,
            5 => resources.event_retains = 1,
            6 => resources.dependency_retains = 1,
            7 => resources.dependency_depths = 1,
            8 => resources.dependency_waiters = 1,
            9 => resources.completion_reservations = 1,
            10 => resources.ready_index_entries = 1,
            11 => resources.in_flight_index_entries = 1,
            12 => resources.directional_active = 1,
            13 => resources.stream_owners = 1,
            14 => resources.allocation_owners = 1,
            15 => resources.directed_roots = 1,
            _ => unreachable!(),
        }
        assert!(!resources.permits_shutdown());
    }
}
