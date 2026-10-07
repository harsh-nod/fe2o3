use super::*;

#[test]
fn cooperative_sdma_partial_copy_is_resumable_and_observers_do_not_drive_dma() {
    let mut fixture = fixture(
        16,
        phase_steps(true, 2, 4, true),
        phase_steps(false, 7, 4, true),
    );
    fixture.source.byte_offset = 2;
    fixture.source.byte_len = 4;
    fixture.destination.byte_offset = 7;
    fixture.destination.byte_len = 4;
    let submission = fixture.submit();
    assert_eq!(fixture.backend.cooperative_staging_bytes, 8);
    fixture.assert_observation_only(submission);
    for reading in [true, false] {
        fixture.backend.flush_stream_v1(fixture.stream).unwrap();
        fixture.assert_observation_only(submission);
        assert_eq!(
            fixture.backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::TooLate
        );
        for allocation in [fixture.source.allocation, fixture.destination.allocation] {
            assert!(
                matches!(fixture.backend.release_allocation_v1(allocation), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
            );
        }
        if reading {
            assert_eq!(fixture.bytes(false), [0x17; 16]);
        }
    }
    fixture.backend.flush_stream_v1(fixture.stream).unwrap();
    assert_eq!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut expected = [0x17; 16];
    expected[7..11].fill(0x53);
    assert_eq!(fixture.bytes(false), expected);
    assert_eq!(fixture.bytes(true), [0x53; 16]);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_reuses_one_scratch_per_phase_and_reads_all_before_writing() {
    let len = COOPERATIVE_COPY_CHUNK_BYTES_V1 + 3;
    let mut fixture = fixture(
        len + 8,
        phase_steps(true, 2, len, true),
        phase_steps(false, 4, len, true),
    );
    fixture.source.byte_offset = 2;
    fixture.source.byte_len = len as u64;
    fixture.destination.byte_offset = 4;
    fixture.destination.byte_len = len as u64;
    let submission = fixture.submit();
    for _ in 0..2 {
        fixture.backend.flush_stream_v1(fixture.stream).unwrap();
        assert_eq!(fixture.bytes(false), vec![0x17; len + 8]);
        fixture.assert_observation_only(submission);
    }
    for _ in 0..3 {
        fixture.backend.flush_stream_v1(fixture.stream).unwrap();
    }
    assert_eq!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut expected = vec![0x17; len + 8];
    expected[4..4 + len].fill(0x53);
    assert_eq!(fixture.bytes(false), expected);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_live_leaf_preserves_ordered_admission_and_rejects_unrelated_custody() {
    for explicit in [false, true] {
        let mut source_steps = phase_steps(true, 0, 8, true);
        source_steps.extend(phase_steps(true, 0, 8, false));
        let mut destination_steps = phase_steps(false, 0, 8, true);
        destination_steps.extend(phase_steps(false, 0, 8, false));
        let mut fixture = fixture(16, source_steps, destination_steps);
        fixture.source.byte_len = 8;
        fixture.destination.byte_len = 8;
        let producer = fixture.submit();
        fixture.backend.flush_stream_v1(fixture.stream).unwrap();
        let other = fixture.backend.create_stream_v1(8).unwrap();
        let before = fixture.steps();
        let next_handle = fixture.backend.next_handle;
        assert!(
            matches!(fixture.backend.peer_copy_v1(other, fixture.source, fixture.destination, &[]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert_eq!(fixture.backend.next_handle, next_handle);
        assert_eq!(fixture.steps(), before);
        let event = fixture
            .backend
            .record_event_v1(fixture.stream, producer)
            .unwrap();
        let stream = if explicit { other } else { fixture.stream };
        let consumer = fixture
            .backend
            .peer_copy_v1(
                stream,
                fixture.source,
                fixture.destination,
                if explicit {
                    core::slice::from_ref(&event)
                } else {
                    &[]
                },
            )
            .unwrap();
        fixture.backend.release_event_v1(event).unwrap();
        fixture.backend.assert_cooperative_indexes_consistent();
        fixture.backend.flush_stream_v1(stream).unwrap();
        fixture.assert_observation_only(consumer);
        fixture.backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            fixture.backend.poll_v1(consumer).unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(&fixture.bytes(false)[..8], [0x53; 8]);
        fixture.backend.release_submission_v1(consumer).unwrap();
        fixture.backend.release_submission_v1(producer).unwrap();
        fixture.backend.destroy_stream_v1(other).unwrap();
        fixture.clean(&[]);
    }
}

#[test]
fn cooperative_sdma_cleanup_failure_freezes_copy_and_retains_private_owner_until_release() {
    let mut source_steps = phase_steps(true, 0, 8, false);
    source_steps.insert(
        source_steps.len() - 1,
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
    );
    let mut fixture = fixture(16, source_steps, vec![]);
    fixture.source.byte_len = 8;
    fixture.destination.byte_len = 8;
    let submission = fixture.submit();
    assert!(matches!(
        fixture.backend.flush_stream_v1(fixture.stream),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(matches!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    fixture.backend.assert_cooperative_indexes_consistent();
    assert_eq!(fixture.backend.children[0].allocations.len(), 2);
    assert_eq!(
        fixture.backend.children[0]
            .scripted_sdma
            .as_ref()
            .unwrap()
            .live_owner_count(),
        2
    );
    assert_eq!(fixture.backend.cooperative_staging_bytes, 8);
    let before = fixture.steps();
    assert!(matches!(
        fixture
            .backend
            .drain_v1(submission, Instant::now() + Duration::from_secs(1))
            .unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(fixture.steps(), before);
    assert_eq!(fixture.bytes(false), [0x17; 16]);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_cancel_before_publication_cleans_scratch_and_rewinds_tail() {
    let mut source_steps = allocation_steps(8);
    source_steps.push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    let mut fixture = fixture(16, source_steps, vec![]);
    fixture.source.byte_len = 8;
    fixture.destination.byte_len = 8;
    let submission = fixture.submit();
    // Dependency transition, rooted descriptor, allocation, private stream.
    for _ in 0..4 {
        assert_eq!(
            fixture
                .backend
                .progress_cooperative_copy(submission)
                .unwrap(),
            BackendPollV1::Pending
        );
    }
    assert_eq!(
        fixture.backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(
        !fixture
            .backend
            .cooperative_stream_tails
            .contains_key(&fixture.stream)
    );
    assert_eq!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed { code: -2 }
    );
    assert_eq!(fixture.bytes(false), [0x17; 16]);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_context_cleanup_failure_is_terminal_for_copy_but_disposal_is_retryable() {
    for cancel in [false, true] {
        let mut source_steps = if cancel {
            allocation_steps(8)
        } else {
            phase_steps(true, 0, 8, false)
        };
        if !cancel {
            source_steps.pop();
        }
        source_steps.extend([
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let mut context = crate::RuntimeContextV1::open(backend).unwrap();
        let left = context.devices()[0].id();
        let right = context.devices()[1].id();
        let stream = context.create_stream(right).unwrap();
        let source = context
            .allocate(left, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let destination = context
            .allocate(right, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        let (source_handle, destination_handle) = {
            let backend = context.backend_mut_for_test_v1();
            let source = *backend
                .allocations
                .iter()
                .find(|(_, route)| route.child == 0)
                .unwrap()
                .0;
            let destination = *backend
                .allocations
                .iter()
                .find(|(_, route)| route.child == 1)
                .unwrap()
                .0;
            install_scripted_devices(backend, source, destination, 16, source_steps, vec![]);
            (source, destination)
        };
        let mut submission = context
            .peer_copy(
                stream,
                crate::RuntimeMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 8,
                },
                crate::RuntimeMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 8,
                },
                &[],
            )
            .unwrap();
        if cancel {
            let backend = context.backend_mut_for_test_v1();
            let id = *backend.submissions.keys().next().unwrap();
            for _ in 0..4 {
                backend.progress_cooperative_copy(id).unwrap();
            }
            assert!(matches!(
                context.cancel(&mut submission),
                Err(crate::RuntimeErrorV1::BackendQuiescent(_))
            ));
        } else {
            assert!(matches!(
                context.drain(&mut submission, Instant::now() + Duration::from_secs(1)),
                Err(crate::RuntimeErrorV1::BackendQuiescent(_))
            ));
        }
        let before: Vec<_> = context
            .backend()
            .children
            .iter()
            .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
            .collect();
        for _ in 0..2 {
            let _ = context
                .drain(&mut submission, Instant::now() + Duration::from_secs(1))
                .unwrap();
        }
        assert_eq!(
            before,
            context
                .backend()
                .children
                .iter()
                .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
                .collect::<Vec<_>>()
        );
        let failure = context.release_submission(submission).unwrap_err();
        assert_eq!(context.backend().cooperative_staging_bytes, 8);
        context.backend().assert_cooperative_indexes_consistent();
        let (submission, _) = failure.into_parts();
        context.release_submission(submission).unwrap();
        assert_eq!(context.backend().cooperative_staging_bytes, 0);
        {
            let backend = context.backend_mut_for_test_v1();
            for handle in [source_handle, destination_handle] {
                let route = backend.allocations[&handle];
                backend.children[route.child]
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .sdma_backed = false;
            }
        }
        context.release_allocation(source).unwrap();
        context.release_allocation(destination).unwrap();
        context.destroy_stream(stream).unwrap();
        let mut backend = context.shutdown().unwrap();
        for child in &backend.children {
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn cooperative_sdma_ancestor_cleanup_failure_settles_requested_dependent_too() {
    for depth in [3, MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1] {
        let mut steps = phase_steps(true, 0, 8, false);
        steps.insert(
            steps.len() - 1,
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
        );
        let mut fixture = fixture(16, steps, vec![]);
        fixture.source.byte_len = 8;
        fixture.destination.byte_len = 8;
        let submissions: Vec<_> = (0..depth).map(|_| fixture.submit()).collect();
        assert!(matches!(
            fixture.backend.drain_v1(
                *submissions.last().unwrap(),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        for id in &submissions {
            assert!(matches!(
                fixture.backend.poll_v1(*id).unwrap(),
                BackendPollV1::Failed { .. }
            ));
        }
        assert_eq!(fixture.bytes(false), [0x17; 16]);
        fixture.backend.assert_cooperative_indexes_consistent();
        assert!(
            fixture
                .backend
                .cooperative_dependency_retain_counts
                .is_empty()
        );
        fixture.clean(&submissions);
    }
}

#[test]
fn cooperative_sdma_budget_reserves_scratch_before_any_native_effect() {
    let mut fixture = fixture(16, vec![], vec![]);
    fixture.source.byte_len = 8;
    fixture.destination.byte_len = 8;
    fixture.backend.cooperative_staging_limit_bytes = 15;
    let next = fixture.backend.next_handle;
    let before = fixture.steps();
    assert!(
        matches!(fixture.backend.peer_copy_v1(fixture.stream, fixture.source, fixture.destination, &[]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!(fixture.backend.next_handle, next);
    assert_eq!(fixture.steps(), before);
    fixture.backend.cooperative_staging_limit_bytes = 16;
    let submission = fixture.submit();
    assert_eq!(
        fixture.backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_terminal_and_unwind_preserve_dma_custody() {
    for panic in [false, true] {
        let mut steps = allocation_steps(8);
        steps.push(scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ));
        if panic {
            steps.push(completed());
            steps.push(ScriptedSdmaStepV1::RetirePanic);
        } else {
            steps.push(ScriptedSdmaStepV1::Poll(
                ScriptedExecutionOutcomeV1::ProcessTeardown,
            ));
        }
        let mut fixture = fixture(16, steps, vec![]);
        fixture.source.byte_len = 8;
        fixture.destination.byte_len = 8;
        let submission = fixture.submit();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fixture.backend.flush_stream_v1(fixture.stream)
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(fixture.backend.terminal);
        assert!(fixture.backend.children[0].terminal);
        assert!(!fixture.backend.children[1].terminal);
        assert_eq!(fixture.backend.cooperative_staging_bytes, 16);
        fixture.backend.assert_cooperative_indexes_consistent();
        let before = fixture.steps();
        assert!(matches!(
            fixture.backend.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(fixture.steps(), before);
        for child in &mut fixture.backend.children {
            assert_eq!(child.scripted_sdma.as_ref().unwrap().unexpected_drops(), 0);
            assert!(child.admitted_device.is_none());
            assert!(child.queue.is_none());
            disarm_scripted_drop_after_inspection_v1(child);
        }
    }
}

#[test]
fn cooperative_sdma_allocation_rejection_and_settled_empty_failure_leave_no_private_owners() {
    for warm in [false, true] {
        let steps = vec![super::sdma_allocation_tests::reject(
            RuntimeMemoryKindV1::HostVisible,
            8,
            false,
        )];
        let mut fixture = fixture(16, steps, vec![]);
        fixture.backend.children[0].sdma_enabled = warm;
        fixture.source.byte_len = 8;
        fixture.destination.byte_len = 8;
        let submission = fixture.submit();
        assert!(matches!(
            fixture.backend.flush_stream_v1(fixture.stream),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        assert!(!fixture.backend.terminal);
        assert_eq!(fixture.backend.children[0].allocations.len(), 1);
        assert_eq!(fixture.backend.children[0].staged_context_bytes, 16);
        assert_eq!(fixture.backend.cooperative_staging_bytes, 0);
        assert_eq!(fixture.bytes(false), [0x17; 16]);
        fixture.clean(&[submission]);
    }
}

#[test]
fn cooperative_sdma_cancel_after_prepublication_failure_does_not_claim_publication() {
    let mut fixture = fixture(16, vec![], vec![]);
    fixture.backend.children[0]
        .staging_budgets
        .max_context_bytes = 16;
    fixture.source.byte_len = 8;
    fixture.destination.byte_len = 8;
    let submission = fixture.submit();
    for _ in 0..3 {
        fixture
            .backend
            .progress_cooperative_copy(submission)
            .unwrap();
    }
    assert!(matches!(
        fixture.backend.cancel_v1(submission),
        Err(RuntimeBackendFailureV1::Quiescent(_))
    ));
    assert!(matches!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(fixture.backend.children[0].allocations.len(), 1);
    assert_eq!(fixture.bytes(false), [0x17; 16]);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_full_page_preserves_authenticated_compute_ready_content() {
    let len = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize;
    let mut destination_steps = phase_steps(false, 0, len, true);
    // Authenticated full-H2D promotion consumes its completed frontier directly.
    destination_steps.retain(|step| !matches!(step, ScriptedSdmaStepV1::Retire(_)));
    let mut fixture = fixture(len, phase_steps(true, 0, len, true), destination_steps);
    let submission = fixture.submit();
    for _ in 0..3 {
        fixture.backend.flush_stream_v1(fixture.stream).unwrap();
    }
    assert_eq!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let route = fixture.backend.allocations[&fixture.destination.allocation];
    let record = &fixture.backend.children[route.child].allocations[&route.local];
    let KfdRuntimeSdmaStorageV1::H2dReady(ready) = &record.sdma_storage else {
        panic!("full copied content must remain compute-ready")
    };
    let expected = vec![0x53; len];
    assert_eq!(ready.owner.scripted_bytes().unwrap(), expected);
    assert_eq!(&*record.bytes, expected);
    assert_eq!(
        ready.owner.authenticated_sha256(),
        <[u8; 32]>::from(Sha256::digest(&expected))
    );
    assert_eq!(
        record.content_sha256,
        Some(ready.owner.authenticated_sha256())
    );
    assert!(!record.sdma_shadow_dirty);
    fixture.clean(&[submission]);
}
