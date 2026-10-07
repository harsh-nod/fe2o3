use super::*;

#[test]
fn staged_allocations_are_bounded_and_round_trip() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
        .unwrap();
    backend
        .write_allocation_v1(allocation, 4, &[1, 2, 3])
        .unwrap();
    let mut bytes = [0_u8; 5];
    backend
        .read_allocation_v1(allocation, 2, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [0, 0, 1, 2, 3]);
    assert!(matches!(
        backend.write_allocation_v1(allocation, 15, &[1, 2]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
}

#[test]
fn profiler_unassigned_bootstrap_queue_teardown_does_not_drop_an_event() {
    for logical_lane in [None, Some(0), Some(1)] {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x60; 32], 16).unwrap())
            .unwrap();
        let queue = logical_lane.and_then(|lane| {
            backend.profile_resource_v1(
                KfdProfileResourceKindV1::NativeQueue,
                KFD_PROFILE_NATIVE_QUEUE_ORDINAL_V1 + lane as u64,
            )
        });
        if logical_lane.is_some() {
            backend.observe_profile_v1(
                queue.map(|queue| KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }),
            );
            backend.observe_destroyed_compute_lane_v1(logical_lane);
        }
        backend.observe_destroyed_compute_lane_v1(None);
        backend.shutdown_native_v1().unwrap();
        let capture = backend.finish_profiler_v1().unwrap();
        capture.validate().unwrap();
        assert!(capture.coverage.complete_runtime_operation_history);
        assert_eq!(capture.coverage.dropped_events, 0);
        if let Some(queue) = queue {
            assert_eq!(capture.events.len(), 2);
            assert!(matches!(
                capture.events[0].event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue: observed }
                    if observed == queue
            ));
            assert!(matches!(
                capture.events[1].event,
                KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue: observed }
                    if observed == queue
            ));
        } else {
            assert!(capture.events.is_empty());
        }
    }
}

#[test]
fn profiler_records_sdma_host_and_device_reads_with_exact_content_and_ranges() {
    for content_identity in [false, true] {
        for device_read in [false, true] {
            for (offset, byte_len) in [(0, 8), (3, 2), (8, 0)] {
                let mut steps = if byte_len == 0 {
                    Vec::new()
                } else if device_read {
                    scripted_sync_copy_steps_v1(
                        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                        offset,
                        byte_len,
                        ScriptedFailureModeV1::Success,
                    )
                } else {
                    vec![ScriptedSdmaStepV1::Read {
                        offset,
                        byte_len: u64::from(byte_len),
                    }]
                };
                steps.extend(scripted_release_steps_v1());
                let (mut backend, stream, host, device) =
                    scripted_direct_backend_configured_v1(8, steps, |backend| {
                        let config = KfdRuntimeProfilerConfigV1::new([0x78; 32], 32).unwrap();
                        backend
                            .enable_profiler_v1(if content_identity {
                                config.with_host_content_identities()
                            } else {
                                config
                            })
                            .unwrap();
                    });
                let allocation = if device_read { device } else { host };
                let identity = backend
                    .profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation)
                    .unwrap();
                // Neither a stale shadow nor its cached digest describes the SDMA bytes.
                let record = backend.allocations.get_mut(&allocation).unwrap();
                record.bytes = vec![0xa5; 8].into();
                record.content_sha256 = Some(Sha256::digest([0xa5; 8]).into());
                let mut destination = vec![0xff; byte_len as usize];
                backend
                    .read_allocation_v1(allocation, offset, &mut destination)
                    .unwrap();
                assert_eq!(destination, vec![0; byte_len as usize]);
                clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
                let capture = backend.finish_profiler_v1().unwrap();
                capture.validate().unwrap();
                assert!(capture.coverage.complete_runtime_operation_history);
                assert_eq!(capture.coverage.dropped_events, 0);
                let reads = capture
                    .events
                    .iter()
                    .filter_map(|entry| match entry.event {
                        KfdRuntimeProfileEventKindV1::HostRead {
                            allocation,
                            byte_offset,
                            content,
                        } => Some((allocation, byte_offset, content)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let expected = if content_identity {
                    KfdProfileHostContentV1::ContentIdentity {
                        content: ProfileContentIdentityV1::observed(&destination).unwrap(),
                    }
                } else {
                    KfdProfileHostContentV1::RangeOnly {
                        byte_len: u64::from(byte_len),
                    }
                };
                assert_eq!(reads, vec![(identity, offset, expected)]);
            }
        }
    }
}

#[test]
fn profiler_omits_successful_host_read_events_after_sdma_read_failures() {
    for after_host_mutation in [false, true] {
        let steps = if after_host_mutation {
            let mut steps = scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                0,
                8,
                ScriptedFailureModeV1::Success,
            );
            *steps.last_mut().unwrap() =
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered);
            steps
        } else {
            // A mismatched observation fails before writing the destination.
            vec![ScriptedSdmaStepV1::Read {
                offset: 1,
                byte_len: 8,
            }]
        };
        let (mut backend, _, host, device) =
            scripted_direct_backend_configured_v1(8, steps, |backend| {
                backend
                    .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x79; 32], 32).unwrap())
                    .unwrap();
            });
        let allocation = if after_host_mutation { device } else { host };
        let mut destination = [0xff; 8];
        assert!(matches!(
            backend.read_allocation_v1(allocation, 1, &mut destination),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(destination, [0xff; 8]);
        assert!(matches!(
            backend.read_allocation_v1(allocation, 0, &mut destination),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(destination, [if after_host_mutation { 0 } else { 0xff }; 8]);
        assert!(backend.terminal);
        assert_eq!(backend.terminal_sdma_custody.is_some(), after_host_mutation);
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert!(driver.is_exhausted());
        assert_eq!(
            driver.live_owner_count(),
            if after_host_mutation { 3 } else { 2 }
        );
        assert_eq!(driver.unexpected_drops(), 0);
        assert!(matches!(
            backend.finish_profiler_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        // Inspect the internal recorder only; a terminal backend cannot publish it.
        assert!(
            !backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1()
                .iter()
                .any(|entry| matches!(entry.event, KfdRuntimeProfileEventKindV1::HostRead { .. }))
        );
        disarm_scripted_drop_after_inspection_v1(&mut backend);
    }
}

#[test]
fn profiler_records_only_complete_sdma_chunked_reads() {
    let chunk = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let offset = 3;
    let byte_len = chunk + 8;
    for fail_second_chunk in [false, true] {
        let mut steps = vec![
            ScriptedSdmaStepV1::Read {
                offset,
                byte_len: chunk,
            },
            ScriptedSdmaStepV1::Read {
                offset: offset + chunk + u64::from(fail_second_chunk),
                byte_len: 8,
            },
        ];
        if !fail_second_chunk {
            steps.extend(scripted_release_steps_v1());
        }
        let (mut backend, stream, host, device) =
            scripted_direct_backend_configured_v1((offset + byte_len) as usize, steps, |backend| {
                backend
                    .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x7a; 32], 32).unwrap())
                    .unwrap();
            });
        let identity = backend
            .profile_resource_v1(KfdProfileResourceKindV1::Allocation, host)
            .unwrap();
        let mut destination = vec![0xff; byte_len as usize];
        let result = backend.read_allocation_v1(host, offset, &mut destination);
        assert!(destination[..chunk as usize].iter().all(|&byte| byte == 0));
        let events = if fail_second_chunk {
            assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
            assert_eq!(&destination[chunk as usize..], &[0xff; 8]);
            assert!(backend.terminal);
            let driver = backend.scripted_sdma.as_ref().unwrap();
            assert!(driver.is_exhausted());
            assert_eq!(driver.live_owner_count(), 2);
            assert_eq!(driver.unexpected_drops(), 0);
            assert!(matches!(
                backend.finish_profiler_v1(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            disarm_scripted_drop_after_inspection_v1(&mut backend);
            backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1()
                .to_vec()
        } else {
            result.unwrap();
            assert_eq!(&destination[chunk as usize..], &[0; 8]);
            clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
            backend.finish_profiler_v1().unwrap().events
        };
        let reads = events
            .iter()
            .filter_map(|entry| match entry.event {
                KfdRuntimeProfileEventKindV1::HostRead {
                    allocation,
                    byte_offset,
                    content,
                } => Some((allocation, byte_offset, content)),
                _ => None,
            })
            .collect::<Vec<_>>();
        if fail_second_chunk {
            assert!(reads.is_empty());
        } else {
            assert_eq!(
                reads,
                vec![(
                    identity,
                    offset,
                    KfdProfileHostContentV1::RangeOnly { byte_len }
                )]
            );
        }
    }
}

#[test]
fn profiler_records_complete_address_free_runtime_lifecycle() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([11; 32], 32).unwrap())
        .unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
        .unwrap();
    backend
        .write_allocation_v1(allocation, 4, &[1, 2, 3])
        .unwrap();
    let mut readback = [0_u8; 3];
    backend
        .read_allocation_v1(allocation, 4, &mut readback)
        .unwrap();
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    backend.unload_module_v1(module).unwrap();
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
    let capture = backend.finish_profiler_v1().unwrap();
    capture.validate().unwrap();
    assert_eq!(
        capture.host_content_mode,
        fe2o3_profiler_protocol::KfdProfileHostContentModeV1::RangeOnly
    );
    assert!(capture.coverage.complete_runtime_operation_history);
    assert_eq!(capture.coverage.dropped_events, 0);
    assert!(
        capture
            .events
            .iter()
            .any(|event| matches!(event.event, KfdRuntimeProfileEventKindV1::HostWrite { .. }))
    );
    assert!(
        capture
            .events
            .iter()
            .any(|event| matches!(event.event, KfdRuntimeProfileEventKindV1::HostRead { .. }))
    );
    let encoded = fe2o3_profiler_protocol::encode_kfd_runtime_profile_v1(&capture).unwrap();
    let encoded = String::from_utf8(encoded).unwrap();
    assert!(!encoded.contains("backend_handle"));
    assert!(!encoded.contains("device_address"));
    assert!(!encoded.contains("queue_id"));
}

#[test]
fn profiler_content_identity_mode_is_explicit_in_every_host_record() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .enable_profiler_v1(
            KfdRuntimeProfilerConfigV1::new([15; 32], 16)
                .unwrap()
                .with_host_content_identities(),
        )
        .unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend.write_allocation_v1(allocation, 0, &[1; 8]).unwrap();
    let mut readback = [0; 8];
    backend
        .read_allocation_v1(allocation, 0, &mut readback)
        .unwrap();
    backend.release_allocation_v1(allocation).unwrap();
    backend.shutdown_native_v1().unwrap();
    let capture = backend.finish_profiler_v1().unwrap();
    assert_eq!(
        capture.host_content_mode,
        fe2o3_profiler_protocol::KfdProfileHostContentModeV1::ContentIdentity
    );
    let host_records: Vec<_> = capture
        .events
        .iter()
        .filter_map(|event| match &event.event {
            KfdRuntimeProfileEventKindV1::HostWrite { content, .. }
            | KfdRuntimeProfileEventKindV1::HostRead { content, .. } => Some(*content),
            _ => None,
        })
        .collect();
    assert_eq!(host_records.len(), 2);
    assert!(host_records.iter().all(|content| matches!(
        content,
        KfdProfileHostContentV1::ContentIdentity { content }
            if content.byte_len == 8
    )));
}

#[test]
fn profiler_timestamp_retrieval_requires_cleanup_and_preserves_runtime_custody() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([24; 32], 16).unwrap())
        .unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    assert!(matches!(
        backend.finish_profiler_with_dispatch_timestamps_v1(),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
    let evidence = backend
        .finish_profiler_with_dispatch_timestamps_v1()
        .unwrap();
    assert!(
        evidence
            .runtime_profile()
            .coverage
            .complete_runtime_operation_history
    );
    assert!(
        evidence
            .dispatch_timestamps()
            .coverage()
            .complete_runtime_operation_history
    );
    assert!(evidence.dispatch_timestamps().records().is_empty());
}

#[test]
fn semantic_timestamp_v2_requires_explicit_sidecar_enablement() {
    let mut ordinary = KfdRuntimeBackendV1::mock();
    ordinary
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([25; 32], 16).unwrap())
        .unwrap();
    ordinary.shutdown_native_v1().unwrap();
    assert!(matches!(
        ordinary.finish_profiler_with_dispatch_timestamps_v2(),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    let ordinary_evidence = ordinary
        .finish_profiler_with_dispatch_timestamps_v1()
        .unwrap();
    assert!(ordinary_evidence.runtime_profile().events.is_empty());
    assert!(ordinary_evidence.dispatch_timestamps().records().is_empty());

    let mut semantic = KfdRuntimeBackendV1::mock();
    semantic
        .enable_profiler_with_semantic_profile_v1(
            KfdRuntimeProfilerConfigV1::new([26; 32], 16).unwrap(),
        )
        .unwrap();
    semantic.shutdown_native_v1().unwrap();
    let evidence = semantic
        .finish_profiler_with_dispatch_timestamps_v2()
        .unwrap();
    assert!(evidence.runtime_profile().events.is_empty());
    assert!(evidence.dispatch_timestamps().records().is_empty());
    assert!(evidence.semantic_profile().records().is_empty());
    assert!(
        evidence
            .semantic_profile()
            .coverage()
            .complete_retained_dispatch_classification
    );
}

#[test]
fn semantic_sidecar_finish_rejection_preserves_ordinary_v1_profiler() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([27; 32], 16).unwrap())
        .unwrap();
    backend.shutdown_native_v1().unwrap();
    assert!(matches!(
        backend.finish_profiler_with_semantic_profile_v1(),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    let capture = backend.finish_profiler_v1().unwrap();
    assert!(capture.events.is_empty());
    assert!(capture.coverage.complete_runtime_operation_history);
}

#[test]
fn profiler_loss_is_bounded_and_freezes_a_valid_prefix() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([12; 32], 2).unwrap())
        .unwrap();
    let stream = backend.create_stream_v1(7).unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend.write_allocation_v1(allocation, 0, &[1; 8]).unwrap();
    backend.release_allocation_v1(allocation).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
    let capture = backend.finish_profiler_v1().unwrap();
    capture.validate().unwrap();
    assert_eq!(capture.events.len(), 2);
    assert_eq!(capture.coverage.dropped_events, 3);
    assert!(!capture.coverage.complete_runtime_operation_history);
}

#[test]
fn profiler_enable_rejects_a_logically_clean_but_used_backend() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    assert!(matches!(
        backend.enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([13; 32], 8).unwrap()),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
}

#[test]
fn profiler_enable_rejects_a_shutdown_backend() {
    let mut backend = KfdRuntimeBackendV1::mock();
    backend.shutdown_native_v1().unwrap();
    assert!(matches!(
        backend.enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([14; 32], 8).unwrap()),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
}

#[test]
fn complete_writes_cache_content_evidence_and_partial_writes_invalidate_it() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let complete = [1_u8, 2, 3, 4, 5, 6, 7, 8];
    backend
        .write_allocation_v1(allocation, 0, &complete)
        .unwrap();
    assert_eq!(
        backend.allocations[&allocation].content_sha256,
        Some(Sha256::digest(complete).into())
    );
    let first_image = Arc::clone(&backend.allocations[&allocation].bytes);
    backend
        .write_allocation_v1(allocation, 0, &complete)
        .unwrap();
    assert!(Arc::ptr_eq(
        &first_image,
        &backend.allocations[&allocation].bytes
    ));

    let full = snapshot_bound_data_v1(
        &backend.allocations,
        &[BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            kernarg_byte_offset: 0,
        }],
        7,
    )
    .unwrap();
    assert_eq!(
        full.data[0].content_sha256,
        backend.allocations[&allocation].content_sha256
    );

    backend.write_allocation_v1(allocation, 3, &[9]).unwrap();
    assert_eq!(backend.allocations[&allocation].content_sha256, None);
}

#[test]
fn staging_budgets_reject_before_allocation_and_release_exact_accounting() {
    let mut backend = KfdRuntimeBackendV1::mock_with_staging_budgets(StagingBudgetsV1 {
        max_allocation_bytes: 8,
        max_context_bytes: 12,
    });
    let first = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let second = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 4)
        .unwrap();
    assert_eq!(backend.staged_context_bytes, 12);
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 1, 1),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 9, 1),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    backend.release_allocation_v1(first).unwrap();
    assert_eq!(backend.staged_context_bytes, 4);
    let replacement = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    assert_eq!(backend.staged_context_bytes, 12);
    backend.release_allocation_v1(second).unwrap();
    backend.release_allocation_v1(replacement).unwrap();
}

#[test]
fn staged_allocation_capacity_failure_is_fallible() {
    assert!(matches!(
        try_zeroed_staging_v1(usize::MAX),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
}
