use super::*;

#[test]
fn cooperative_sdma_native_dirty_chunks_pin_authority_and_preserve_unaffected_backing() {
    let mut steps = allocation_steps(4);
    steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 4,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Write {
            offset: 8,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Write {
            offset: 12,
            byte_len: 1,
        },
        ScriptedSdmaStepV1::Read {
            offset: 7,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let mut fixture = native_host_fixture(steps, phase_steps(false, 14, 4, true));
    let submission = fixture.submit();
    assert_eq!(fixture.backend.cooperative_staging_bytes, 8);
    // Producer effects can become visible after copy admission, before Read.
    attach_native_dirty(&mut fixture, 4, (0..9).collect());
    let mut observed = 0;
    for _ in 0..64 {
        fixture
            .backend
            .progress_cooperative_copy(submission)
            .unwrap();
        if fixture.backend.poll_v1(submission).unwrap() == BackendPollV1::Succeeded {
            break;
        }
        fixture.assert_observation_only(submission);
        let child = &mut fixture.backend.children[0];
        if child.native_reconciliations.iter().any(Option::is_some) {
            observed += 1;
            assert_eq!(child.native_dirty_extents, 1);
            assert_ne!(child.free_compute_lane_v1(), Some(0));
            assert!(
                matches!(child.detach_recycled_dispatch(), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
            );
            let allocation = child.recycled_dispatch.as_ref().unwrap().descriptors[0].allocation;
            assert!(matches!(
                child.write_allocation_v1(allocation, 0, &[1]),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
            assert!(matches!(
                child.release_allocation_v1(allocation),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
        }
    }
    assert_eq!(observed, 3);
    assert_eq!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut source = vec![0x53; 24];
    source[4..13].copy_from_slice(&(0..9).collect::<Vec<_>>());
    let mut destination = vec![0x17; 24];
    destination[14..18].copy_from_slice(&source[7..11]);
    assert_eq!(fixture.bytes(true), source);
    assert_eq!(fixture.bytes(false), destination);
    let child = &fixture.backend.children[0];
    assert_eq!(child.native_dirty_extents, 0);
    assert_eq!(
        child.scripted_native_reconcile.as_ref().unwrap().reads,
        [(41, 0, 2, 4), (41, 0, 6, 4), (41, 0, 10, 1)]
    );
    forget_scripted_recycled(&mut fixture);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_native_dirty_generation_and_descriptor_changes_seal_without_writes() {
    for generation_changed in [true, false] {
        let mut steps = allocation_steps(4);
        steps.push(ScriptedSdmaStepV1::Write {
            offset: 4,
            byte_len: 4,
        });
        let mut fixture = native_host_fixture(steps, vec![]);
        attach_native_dirty(&mut fixture, 4, vec![0x42; 9]);
        let submission = fixture.submit();
        while fixture.backend.children[0]
            .scripted_native_reconcile
            .as_ref()
            .unwrap()
            .reads
            .is_empty()
        {
            fixture
                .backend
                .progress_cooperative_copy(submission)
                .unwrap();
        }
        let child = &mut fixture.backend.children[0];
        assert!(child.native_reconciliations[0].is_some());
        if generation_changed {
            child.scripted_native_reconcile.as_mut().unwrap().generation += 1;
        } else {
            child.recycled_dispatch.as_mut().unwrap().descriptors[0].allocation_offset += 1;
        }
        let before = fixture.steps();
        assert!(matches!(
            fixture.backend.progress_cooperative_copy(submission),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(fixture.steps(), before);
        assert!(fixture.backend.terminal);
        assert!(fixture.backend.children[0].native_reconciliations[0].is_some());
        assert_eq!(fixture.backend.children[0].native_dirty_extents, 1);
        assert!(matches!(
            fixture.backend.release_submission_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        for child in &mut fixture.backend.children {
            assert!(child.admitted_device.is_none() && child.queue.is_none());
            disarm_scripted_drop_after_inspection_v1(child);
        }
    }
}

#[test]
fn cooperative_sdma_native_dirty_cancel_retains_complete_extent_after_partial_reconciliation() {
    let mut steps = allocation_steps(4);
    steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 4,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    steps.extend(allocation_steps(4));
    steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 4,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Write {
            offset: 8,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Write {
            offset: 12,
            byte_len: 1,
        },
        ScriptedSdmaStepV1::Read {
            offset: 7,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let mut fixture = native_host_fixture(steps, phase_steps(false, 14, 4, false));
    attach_native_dirty(&mut fixture, 4, vec![0x42; 9]);
    let submission = fixture.submit();
    while fixture.backend.children[0]
        .scripted_native_reconcile
        .as_ref()
        .unwrap()
        .reads
        .is_empty()
    {
        fixture
            .backend
            .progress_cooperative_copy(submission)
            .unwrap();
    }
    assert_eq!(
        fixture.backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(fixture.backend.children[0].native_dirty_extents, 1);
    assert!(
        fixture.backend.children[0]
            .native_reconciliations
            .iter()
            .all(Option::is_none)
    );
    assert_eq!(fixture.bytes(false), [0x17; 24]);
    let mut expected = vec![0x53; 24];
    expected[4..8].fill(0x42);
    assert_eq!(fixture.bytes(true), expected);
    let retry = fixture.submit();
    for _ in 0..64 {
        fixture.backend.progress_cooperative_copy(retry).unwrap();
        if fixture.backend.poll_v1(retry).unwrap() == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        fixture.backend.poll_v1(retry).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(fixture.backend.children[0].native_dirty_extents, 0);
    assert_eq!(&fixture.bytes(false)[14..18], &[0x42; 4]);
    forget_scripted_recycled(&mut fixture);
    fixture.clean(&[submission, retry]);
}

#[test]
fn cooperative_sdma_native_dirty_destination_write_is_exact_range_without_shadow_refresh() {
    let mut steps = allocation_steps(4);
    steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 4,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Write {
            offset: 8,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Write {
            offset: 12,
            byte_len: 1,
        },
        ScriptedSdmaStepV1::Write {
            offset: 7,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let mut fixture = native_host_fixture(steps, phase_steps(true, 14, 4, true));
    attach_native_dirty(&mut fixture, 4, (0..9).collect());
    core::mem::swap(&mut fixture.source, &mut fixture.destination);
    fixture.source.access = RuntimeAccessV1::Read;
    fixture.destination.access = RuntimeAccessV1::Write;
    fixture.backend.destroy_stream_v1(fixture.stream).unwrap();
    fixture.stream = fixture.backend.create_stream_v1(7).unwrap();
    let submission = fixture.submit();
    for _ in 0..64 {
        fixture
            .backend
            .progress_cooperative_copy(submission)
            .unwrap();
        if fixture.backend.poll_v1(submission).unwrap() == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        fixture.backend.poll_v1(submission).unwrap(),
        BackendPollV1::Succeeded
    );
    let mut expected = vec![0x53; 24];
    expected[4..13].copy_from_slice(&(0..9).collect::<Vec<_>>());
    expected[7..11].fill(0x17);
    assert_eq!(fixture.bytes(false), expected);
    let child = &fixture.backend.children[0];
    let descriptor = child.recycled_dispatch.as_ref().unwrap().descriptors[0];
    let record = &child.allocations[&descriptor.allocation];
    assert!(record.sdma_shadow_dirty && record.content_sha256.is_none());
    assert!(compute_dispatch::resident_data_needs_host_overwrite_v1(
        &descriptor,
        record.content_sha256
    ));
    forget_scripted_recycled(&mut fixture);
    fixture.clean(&[submission]);
}

#[test]
fn cooperative_sdma_native_dirty_later_host_fault_retains_generation_extent_and_scratch() {
    for panic in [false, true] {
        let mut steps = allocation_steps(4);
        steps.extend([
            ScriptedSdmaStepV1::Write {
                offset: 4,
                byte_len: 4,
            },
            ScriptedSdmaStepV1::WriteFault {
                offset: 8,
                byte_len: 4,
                written_prefix: 2,
                panic,
            },
        ]);
        let mut fixture = native_host_fixture(steps, vec![]);
        attach_native_dirty(&mut fixture, 4, vec![0x42; 9]);
        let submission = fixture.submit();
        while fixture.backend.children[0]
            .scripted_native_reconcile
            .as_ref()
            .unwrap()
            .reads
            .is_empty()
        {
            fixture
                .backend
                .progress_cooperative_copy(submission)
                .unwrap();
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fixture.backend.progress_cooperative_copy(submission)
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
        assert_eq!(fixture.backend.children[0].native_dirty_extents, 1);
        assert!(fixture.backend.children[0].native_reconciliations[0].is_some());
        assert_eq!(fixture.backend.cooperative_staging_bytes, 8);
        assert_eq!(&fixture.bytes(true)[4..10], &[0x42; 6]);
        assert_eq!(fixture.bytes(false), [0x17; 24]);
        for child in &mut fixture.backend.children {
            assert!(child.admitted_device.is_none() && child.queue.is_none());
            assert_eq!(child.scripted_sdma.as_ref().unwrap().unexpected_drops(), 0);
            disarm_scripted_drop_after_inspection_v1(child);
        }
    }
}

#[test]
fn cooperative_sdma_native_dirty_pins_defer_compute_without_failing_admitted_work() {
    let mut child = KfdRuntimeBackendV1::mock();
    let stream = child.create_stream_v1(7).unwrap();
    let target = child
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let other = child
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let scratch = child
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 8)
        .unwrap();
    child
        .allocations
        .get_mut(&target)
        .unwrap()
        .native_dirty
        .push(NativeDirtyExtentV1 {
            compute_lane: 0,
            data_index: 0,
            allocation_offset: 0,
            data_offset: 0,
            byte_len: 8,
        });
    child.native_dirty_extents = 1;
    child.recycled_dispatch = Some(RecycledDispatchV1 {
        kernel: 9,
        dispatch_shape_sha256: [0; 32],
        descriptors: [target, other]
            .map(|allocation| ResidentDataDescriptorV1 {
                allocation,
                kind: RuntimeMemoryKindV1::HostVisible,
                alignment: 8,
                allocation_offset: 0,
                byte_len: 8,
                host_content_sha256: None,
                device_may_have_modified: true,
            })
            .to_vec(),
    });
    child.scripted_native_reconcile = Some(native_reconcile::ScriptedNativeReconcileV1 {
        generation: 41,
        data: vec![vec![1; 8]],
        reads: vec![],
    });
    let root = child
        .begin_native_reconciliation_v1(target, scratch)
        .unwrap()
        .unwrap();
    assert_eq!(child.free_compute_lane_v1(), Some(1));
    for allocation in [target, other] {
        let id = child.next_id().unwrap();
        let pending = pending_compute_for_test_v1(id, stream, allocation, vec![]);
        child
            .pending_compute_streams
            .insert(stream, VecDeque::from([id]));
        assert_eq!(
            child.progress_pending_compute_v1(pending).unwrap(),
            BackendPollV1::Pending
        );
        assert!(child.pending_compute.contains_key(&id));
        assert!(!child.submissions.contains_key(&id));
        assert_eq!(child.native_dirty_extents, 1);
        assert!(
            child
                .scripted_native_reconcile
                .as_ref()
                .unwrap()
                .reads
                .is_empty()
        );
        child.pending_compute.remove(&id);
        child.pending_compute_streams.remove(&stream);
    }
    child.release_native_reconciliation_v1(root);
    child.recycled_dispatch = None;
    child
        .allocations
        .get_mut(&target)
        .unwrap()
        .native_dirty
        .clear();
    child.native_dirty_extents = 0;
    for allocation in [target, other, scratch] {
        child.release_allocation_v1(allocation).unwrap();
    }
    child.destroy_stream_v1(stream).unwrap();
    child.shutdown_native_v1().unwrap();
}
