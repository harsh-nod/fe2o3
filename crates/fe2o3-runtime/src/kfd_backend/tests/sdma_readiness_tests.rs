use super::*;

fn backing_bytes(backend: &KfdRuntimeBackendV1, allocation: u64) -> &[u8] {
    match &backend.allocations[&allocation].sdma_storage {
        KfdRuntimeSdmaStorageV1::Host(SdmaBufferOwnerV1::Scripted(host)) => host.observation().1,
        KfdRuntimeSdmaStorageV1::Device(device) => device.scripted_bytes().unwrap(),
        _ => unreachable!("quiescent copy restores backing owners"),
    }
}

#[test]
fn dirty_shadow_failed_or_cancelled_producer_never_publishes_consumer() {
    for reverse in [false, true] {
        for explicit in [false, true] {
            for cancel in [false, true] {
                let direction = if reverse {
                    Gfx942PersistentSdmaDirectionV1::DeviceToHost
                } else {
                    Gfx942PersistentSdmaDirectionV1::HostToDevice
                };
                let mut steps = Vec::new();
                if !cancel {
                    steps.push(scripted_submit_step_v1(
                        direction,
                        0,
                        0,
                        4,
                        ScriptedFailureModeV1::Retryable,
                    ));
                }
                steps.extend(scripted_release_steps_v1());
                let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
                let consumer_stream = if explicit {
                    backend.create_stream_v1(7).unwrap()
                } else {
                    stream
                };
                for allocation in [host, device] {
                    backend
                        .allocations
                        .get_mut(&allocation)
                        .unwrap()
                        .sdma_shadow_dirty = true;
                }
                // A scripted preparation blocker leaves the real producer ledger
                // unpublished, allowing both cancellation and recovered failure.
                backend
                    .allocations
                    .get_mut(&device)
                    .unwrap()
                    .native_dirty
                    .push(NativeDirtyExtentV1 {
                        compute_lane: 0,
                        data_index: 0,
                        allocation_offset: 4,
                        data_offset: 4,
                        byte_len: 4,
                    });
                backend.native_dirty_extents = 1;
                let (source, destination) = if reverse {
                    scripted_copy_regions_v1(device, host, 4)
                } else {
                    scripted_copy_regions_v1(host, device, 4)
                };
                let producer = backend
                    .copy_async_v1(stream, source, destination, &[])
                    .unwrap();
                let event = backend.record_event_v1(stream, producer).unwrap();
                let (source, destination) =
                    scripted_copy_regions_v1(destination.allocation, source.allocation, 4);
                let dependencies = if explicit {
                    core::slice::from_ref(&event)
                } else {
                    &[]
                };
                let consumer = backend
                    .copy_async_v1(consumer_stream, source, destination, dependencies)
                    .unwrap();
                assert_eq!(backend.sdma_dependency_retain_counts[&producer], 1);
                assert!(backend.published_sdma_submissions.is_empty());
                backend
                    .allocations
                    .get_mut(&device)
                    .unwrap()
                    .native_dirty
                    .clear();
                backend.native_dirty_extents = 0;
                if cancel {
                    assert_eq!(
                        backend.cancel_v1(producer).unwrap(),
                        crate::BackendCancellationV1::Cancelled
                    );
                } else {
                    assert!(matches!(
                        backend.flush_stream_v1(stream),
                        Err(RuntimeBackendFailureV1::Quiescent(_))
                    ));
                }
                assert!(matches!(
                    backend.poll_v1(producer).unwrap(),
                    BackendPollV1::Failed { .. }
                ));
                assert!(matches!(
                    backend.poll_v1(consumer).unwrap(),
                    BackendPollV1::Failed { .. }
                ));
                assert!(backend.published_sdma_submissions.is_empty());
                assert!(backend.active_sdma.is_empty());
                assert!(backend.allocation_custody.is_empty());
                assert!(backend.sdma_dependency_retain_counts.is_empty());
                assert_eq!(backend.sdma_completion_reservations, 0);
                let next_handle = backend.next_handle;
                assert!(matches!(
                    backend.copy_async_v1(consumer_stream, source, destination, &[event]),
                    Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                ));
                assert_eq!(backend.next_handle, next_handle);
                assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 3);
                for allocation in [host, device] {
                    assert_eq!(backing_bytes(&backend, allocation), [0; 8]);
                    assert!(backend.allocations[&allocation].sdma_shadow_dirty);
                }
                backend.release_event_v1(event).unwrap();
                backend.release_submission_v1(producer).unwrap();
                backend.release_submission_v1(consumer).unwrap();
                if explicit {
                    backend.destroy_stream_v1(consumer_stream).unwrap();
                }
                clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
            }
        }
    }
}

#[test]
fn dirty_shadow_deferred_copy_observes_dependencies_without_publishing() {
    for explicit in [false, true] {
        let mut steps = Vec::new();
        for direction in [
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
        ] {
            steps.push(scripted_submit_step_v1(
                direction,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ));
            if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
                steps.push(ScriptedSdmaStepV1::Poll(
                    ScriptedExecutionOutcomeV1::Pending,
                ));
            }
            steps.extend([
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                }),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ]);
        }
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
        let consumer_stream = if explicit {
            backend.create_stream_v1(7).unwrap()
        } else {
            stream
        };
        for allocation in [host, device] {
            backend
                .allocations
                .get_mut(&allocation)
                .unwrap()
                .sdma_shadow_dirty = true;
        }
        let (source, destination) = scripted_copy_regions_v1(device, host, 8);
        let producer = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        let event = explicit.then(|| backend.record_event_v1(stream, producer).unwrap());
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let consumer = backend
            .copy_async_v1(consumer_stream, source, destination, event.as_slice())
            .unwrap();
        if let Some(event) = event {
            backend.release_event_v1(event).unwrap();
        }
        assert_eq!(backend.published_sdma_submissions, [producer]);
        assert_eq!(backend.sdma_dependency_retain_counts[&producer], 1);
        assert!(matches!(
            backend.active_sdma[&consumer].phase,
            ActiveSdmaPhaseV1::Ready
        ));
        assert_eq!(backend.poll_v1(consumer).unwrap(), BackendPollV1::Pending);
        assert_eq!(backend.published_sdma_submissions, [producer]);
        assert_eq!(backend.poll_v1(consumer).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            backend.submissions[&producer].status,
            BackendPollV1::Succeeded
        );
        assert!(backend.published_sdma_submissions.is_empty());
        assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 6);
        assert!(
            matches!(backend.release_submission_v1(producer), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        // An already deferred copy still needs explicit progress after readiness.
        backend.flush_stream_v1(consumer_stream).unwrap();
        assert_eq!(backend.published_sdma_submissions, [consumer]);
        assert_eq!(backend.poll_v1(consumer).unwrap(), BackendPollV1::Succeeded);
        assert!(backend.sdma_dependency_retain_counts.is_empty());
        backend.release_submission_v1(producer).unwrap();
        backend.release_submission_v1(consumer).unwrap();
        if explicit {
            backend.destroy_stream_v1(consumer_stream).unwrap();
        }
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    }
}

#[test]
fn dirty_shadow_initial_publication_failure_preserves_exact_backing_and_custody() {
    for direction in [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
    ] {
        for terminal in [false, true] {
            let mut steps = vec![scripted_submit_step_v1(
                direction,
                0,
                0,
                8,
                if terminal {
                    ScriptedFailureModeV1::ProcessTeardown
                } else {
                    ScriptedFailureModeV1::Retryable
                },
            )];
            if !terminal {
                steps.extend(scripted_release_steps_v1());
            }
            let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
            for allocation in [host, device] {
                backend
                    .allocations
                    .get_mut(&allocation)
                    .unwrap()
                    .sdma_shadow_dirty = true;
            }
            let (source, destination) =
                if direction == Gfx942PersistentSdmaDirectionV1::HostToDevice {
                    scripted_copy_regions_v1(host, device, 8)
                } else {
                    scripted_copy_regions_v1(device, host, 8)
                };
            let result = backend.copy_async_v1(stream, source, destination, &[]);
            assert!(backend.published_sdma_submissions.is_empty());
            assert!(backend.published_sdma_index_is_consistent_v1());
            for allocation in [host, device] {
                assert!(backend.allocations[&allocation].sdma_shadow_dirty);
            }
            if terminal {
                assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
                assert!(backend.terminal);
                assert!(backend.terminal_sdma_custody.is_some());
                assert!(backend.allocation_is_active(host));
                assert!(backend.allocation_is_active(device));
                let driver = backend.scripted_sdma.as_ref().unwrap();
                assert!(driver.is_exhausted());
                assert_eq!(driver.live_owner_count(), 2);
                assert_eq!(driver.unexpected_drops(), 0);
                disarm_scripted_drop_after_inspection_v1(&mut backend);
            } else {
                let submission = result.unwrap();
                assert!(matches!(
                    backend.poll_v1(submission).unwrap(),
                    BackendPollV1::Failed { .. }
                ));
                assert!(backend.allocation_custody.is_empty());
                for allocation in [host, device] {
                    assert_eq!(backing_bytes(&backend, allocation), [0; 8]);
                }
                clean_scripted_direct_backend_v1(
                    &mut backend,
                    stream,
                    host,
                    device,
                    Some(submission),
                );
            }
        }
    }
}

#[test]
fn directional_round_trip_publishes_with_authoritative_dirty_shadows_without_flush() {
    let directions = [
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
        Gfx942PersistentSdmaDirectionV1::DeviceToHost,
        Gfx942PersistentSdmaDirectionV1::HostToDevice,
    ];
    let mut steps = vec![ScriptedSdmaStepV1::Write {
        offset: 1,
        byte_len: 6,
    }];
    for direction in directions {
        steps.extend([
            scripted_submit_step_v1(direction, 0, 0, 8, ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ]);
    }
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    backend
        .write_allocation_v1(host, 1, &[1, 3, 5, 7, 9, 11])
        .unwrap();
    let expected = [0, 1, 3, 5, 7, 9, 11, 0];
    let mut submissions = Vec::new();
    let mut required_flushes = 0;
    for (index, direction) in directions.into_iter().enumerate() {
        let (source, destination) = match direction {
            Gfx942PersistentSdmaDirectionV1::HostToDevice => {
                scripted_copy_regions_v1(host, device, 8)
            }
            Gfx942PersistentSdmaDirectionV1::DeviceToHost => {
                scripted_copy_regions_v1(device, host, 8)
            }
        };
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        if backend.published_sdma_submissions != [submission] {
            // Recover the old implementation for clean teardown before the
            // final regression assertion; a passing run never enters here.
            required_flushes += 1;
            backend.flush_stream_v1(stream).unwrap();
        }
        assert_eq!(backend.published_sdma_submissions, [submission]);
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            backend.write_allocation_v1(destination.allocation, 0, &[0]),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(backend.published_sdma_submissions.is_empty());
        assert!(backend.published_sdma_index_is_consistent_v1());
        assert!(backend.allocations[&destination.allocation].sdma_shadow_dirty);
        assert_eq!(backing_bytes(&backend, destination.allocation), expected);
        if index == 1 {
            // A dirty shadow is not data authority for the following H2D.
            Arc::make_mut(&mut backend.allocations.get_mut(&host).unwrap().bytes).fill(0xa5);
        }
        submissions.push(submission);
    }
    assert!(
        backend.allocations[&host]
            .bytes
            .iter()
            .all(|byte| *byte == 0xa5)
    );
    assert!(
        backend.allocations[&device]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );
    for submission in submissions {
        backend.release_submission_v1(submission).unwrap();
    }
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, None);
    assert_eq!(
        required_flushes, 0,
        "authoritative DMA backing needs no shadow preparation"
    );
}

#[test]
fn dirty_shadow_does_not_bypass_native_dirty_reconciliation_on_either_endpoint() {
    for reverse in [false, true] {
        for dirty_source in [false, true] {
            let (mut backend, stream, host, device) =
                scripted_direct_backend_v1(8, scripted_release_steps_v1());
            let (source, destination) = if reverse {
                scripted_copy_regions_v1(device, host, 4)
            } else {
                scripted_copy_regions_v1(host, device, 4)
            };
            for allocation in [host, device] {
                backend
                    .allocations
                    .get_mut(&allocation)
                    .unwrap()
                    .sdma_shadow_dirty = true;
            }
            let dirty = if dirty_source {
                source.allocation
            } else {
                destination.allocation
            };
            // Reconciliation remains allocation-wide even outside the copied range.
            let extent = NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 0,
                allocation_offset: 4,
                data_offset: 4,
                byte_len: 4,
            };
            backend
                .allocations
                .get_mut(&dirty)
                .unwrap()
                .native_dirty
                .push(extent);
            backend.native_dirty_extents = 1;
            let submission = backend
                .copy_async_v1(stream, source, destination, &[])
                .unwrap();
            assert!(matches!(
                backend.active_sdma[&submission].phase,
                ActiveSdmaPhaseV1::Ready
            ));
            assert!(backend.published_sdma_submissions.is_empty());
            assert_eq!(backend.poll_v1(submission).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                backend.wait_v1(submission, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(backend.allocations[&dirty].native_dirty, [extent]);
            assert_eq!(backend.native_dirty_extents, 1);
            assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 3);
            for allocation in [host, device] {
                assert_eq!(backing_bytes(&backend, allocation), [0; 8]);
                assert!(backend.allocation_is_active(allocation));
            }
            assert_eq!(
                backend.cancel_v1(submission).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            assert!(backend.allocation_custody.is_empty());
            backend
                .allocations
                .get_mut(&dirty)
                .unwrap()
                .native_dirty
                .clear();
            backend.native_dirty_extents = 0;
            clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
        }
    }
}

#[test]
fn dirty_shadow_partial_round_trip_uses_backing_and_preserves_surrounding_bytes() {
    let mut steps = vec![ScriptedSdmaStepV1::Write {
        offset: 0,
        byte_len: 8,
    }];
    for (direction, host_offset) in [
        (Gfx942PersistentSdmaDirectionV1::HostToDevice, 1),
        (Gfx942PersistentSdmaDirectionV1::DeviceToHost, 2),
    ] {
        steps.extend([
            scripted_submit_step_v1(direction, host_offset, 3, 3, ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ]);
    }
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    backend
        .write_allocation_v1(host, 0, &[10, 20, 30, 40, 50, 60, 70, 80])
        .unwrap();
    let KfdRuntimeSdmaStorageV1::Device(owner) =
        &mut backend.allocations.get_mut(&device).unwrap().sdma_storage
    else {
        unreachable!()
    };
    owner
        .scripted_bytes_mut()
        .unwrap()
        .copy_from_slice(&[91, 92, 93, 94, 95, 96, 97, 98]);
    for allocation in [host, device] {
        let record = backend.allocations.get_mut(&allocation).unwrap();
        record.sdma_shadow_dirty = true;
        Arc::make_mut(&mut record.bytes).fill(0xa5);
    }
    let (mut source, mut destination) = scripted_copy_regions_v1(host, device, 3);
    source.byte_offset = 1;
    destination.byte_offset = 3;
    let first = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.published_sdma_submissions, [first]);
    assert_eq!(backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(
        backing_bytes(&backend, device),
        [91, 92, 93, 20, 30, 40, 97, 98]
    );
    let (mut source, mut destination) = scripted_copy_regions_v1(device, host, 3);
    source.byte_offset = 3;
    destination.byte_offset = 2;
    let second = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(backend.published_sdma_submissions, [second]);
    assert_eq!(backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(
        backing_bytes(&backend, host),
        [10, 20, 20, 30, 40, 60, 70, 80]
    );
    for allocation in [host, device] {
        let record = &backend.allocations[&allocation];
        assert!(record.sdma_shadow_dirty);
        assert!(record.bytes.iter().all(|byte| *byte == 0xa5));
    }
    backend.release_submission_v1(first).unwrap();
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(second));
}

#[test]
fn dirty_shadow_publication_preserves_full_h2d_promotion_authority() {
    let byte_len = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize;
    for dirty_host in [false, true] {
        let mut steps = vec![
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len,
            },
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                byte_len as u32,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
        ];
        if dirty_host {
            steps.push(ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success));
        }
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) = scripted_direct_backend_v1(byte_len, steps);
        let expected = vec![0x5a; byte_len];
        backend.write_allocation_v1(host, 0, &expected).unwrap();
        backend
            .allocations
            .get_mut(&device)
            .unwrap()
            .sdma_shadow_dirty = true;
        if dirty_host {
            let record = backend.allocations.get_mut(&host).unwrap();
            record.sdma_shadow_dirty = true;
            Arc::make_mut(&mut record.bytes).fill(0xa5);
        }
        let (source, destination) = scripted_copy_regions_v1(host, device, byte_len as u64);
        let submission = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        assert_eq!(backend.published_sdma_submissions, [submission]);
        assert_eq!(
            backend.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        let record = &backend.allocations[&device];
        assert_eq!(
            record
                .sdma_storage
                .persistent_compute_ready_facts_v1()
                .is_some(),
            !dirty_host
        );
        if dirty_host {
            assert_eq!(backing_bytes(&backend, device), expected);
            assert!(record.sdma_shadow_dirty);
            assert!(record.content_sha256.is_none());
        } else {
            assert_eq!(&*record.bytes, expected);
            assert!(!record.sdma_shadow_dirty);
            assert_eq!(
                record.content_sha256,
                Some(Sha256::digest(&expected).into())
            );
        }
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    }
}
