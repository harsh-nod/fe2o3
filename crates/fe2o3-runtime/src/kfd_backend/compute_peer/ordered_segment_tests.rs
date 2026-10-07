//! Ordered native list custody and scripted bytes, not compute arithmetic.

use super::*;

fn append(p: &mut PendingList, source: u64, event: u64) -> Result<u64, Failure> {
    let [_, destination] = regions(&p.f);
    p.f.backend.peer_copy_segments_v1(
        p.f.peer_stream,
        BackendMemoryRegionV1 {
            allocation: source,
            byte_offset: 3,
            byte_len: 53,
            access: RuntimeAccessV1::Read,
        },
        destination,
        &descriptors(),
        &[event],
    )
}

fn append_and_select(p: &mut PendingList, index: usize) -> (u64, Vec<u8>) {
    let source = p.f.allocations[0][index];
    let (_, bytes) = initialize_bytes(&mut p.f, source, 0x71 + index as u8);
    let previous = p.list;
    let event = p.event;
    let next = append(p, source, event).unwrap();
    assert_eq!(p.f.copy(next).dependencies, [previous]);
    assert!(p.f.copy(next).compute_producer.is_none());
    p.f.backend.release_event_v1(p.event).unwrap();
    p.list = next;
    p.event = p.f.backend.record_event_v1(p.f.peer_stream, next).unwrap();
    (previous, bytes)
}

fn expected(p: &PendingList, source: &[u8]) -> Vec<u8> {
    let mut result = p.expected_input(descriptors().len());
    for segment in descriptors() {
        let from = 3 + segment.source_offset as usize;
        let to = 5 + segment.destination_offset as usize;
        let bytes = segment.byte_len as usize;
        result[to..to + bytes].copy_from_slice(&source[from..from + bytes]);
    }
    result
}

fn drive(p: &mut PendingList, target: u64) {
    let stream = p.f.readback_stream;
    for _ in 0..512 {
        p.f.backend.progress_stream_v1(stream).unwrap();
        if p.f.backend.poll_v1(target).unwrap() != BackendPollV1::Pending {
            return;
        }
    }
    panic!("ordered list did not settle under final-stream progress");
}

#[test]
fn ordered_segments_latest_frame_readback_serializes_overlap_and_released_events() {
    for late in [false, true] {
        let mut p = PendingList::with_source_profile(None, true, true, true);
        let source = p.f.allocations[0][4];
        let (source_id, bytes) = initialize_bytes(&mut p.f, source, 0x91);
        let third_source = p.f.allocations[0][5];
        let (_, third_bytes) = initialize_bytes(&mut p.f, third_source, 0x92);
        let first = p.list;
        if late {
            p.publish();
        }
        let event = p.event;
        let second = append(&mut p, source, event).unwrap();
        p.f.backend.release_event_v1(p.event).unwrap();
        p.list = second;
        p.event =
            p.f.backend
                .record_event_v1(p.f.peer_stream, second)
                .unwrap();
        let event = p.event;
        let third = append(&mut p, third_source, event).unwrap();
        p.f.backend.release_event_v1(event).unwrap();
        p.list = third;
        p.event = p.f.backend.record_event_v1(p.f.peer_stream, third).unwrap();
        let last = p.list;
        let readback = p.direct_readback().unwrap();
        p.release_events();
        let generation = p.f.backend.cooperative_progress_generation;
        for id in [first, second, last, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                p.f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert!(p.f.backend.release_submission_v1(id).is_err());
        }
        assert_eq!(p.f.backend.cooperative_progress_generation, generation);
        drive(&mut p, readback);
        for id in [first, second, last, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(owner(&p.f, source), (source_id, bytes.as_slice()));
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], expected(&p, &third_bytes).as_slice())
        );
        let host = p.f.backend.allocations[&p.f.host.unwrap()];
        let KfdRuntimeSdmaStorageV1::Host(host) =
            &p.f.backend.children[1].allocations[&host.local].sdma_storage
        else {
            unreachable!()
        };
        assert_eq!(host.scripted_bytes().unwrap(), expected(&p, &third_bytes));
        assert!(
            p.f.backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        p.f.clean();
    }
}

#[test]
fn ordered_segments_latest_compute_return_readback_waits_for_published_ancestor() {
    for late in [false, true] {
        let mut p = PendingList::with_source_profile(None, false, true, true);
        let source = p.f.allocations[0][4];
        let (_, bytes) = initialize_bytes(&mut p.f, source, 0x82);
        let first = p.list;
        let event = p.event;
        let second = append(&mut p, source, event).unwrap();
        p.f.backend.release_event_v1(p.event).unwrap();
        let second_event =
            p.f.backend
                .record_event_v1(p.f.peer_stream, second)
                .unwrap();
        if late {
            // Publish exactly the oldest retained list, never infer from step counts.
            for _ in 0..64 {
                p.f.backend.progress_cooperative_copy(first).unwrap();
                if !p
                    .f
                    .copy(first)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .is_quiescent()
                {
                    break;
                }
            }
            assert_eq!(
                p.f.backend.compute_xgmi_children,
                [Some(first), Some(first)]
            );
        }
        p.list = second;
        p.event = second_event;
        let consumer = p.consumer();
        p.assert_deferred(consumer);
        let (returned, readback) = super::settled::return_and_readback(&mut p, consumer);
        p.release_events();
        drive(&mut p, readback);
        for id in [first, second, consumer, returned, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], expected(&p, &bytes).as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][2]),
            (p.identities[2], p.output.as_slice())
        );
        assert!(p.f.backend.deferred_compute_retains.is_empty());
        p.f.clean();
    }
}

#[test]
fn ordered_segments_completed_ancestor_source_can_be_disposed_before_tail() {
    let mut p = PendingList::with_source_profile(None, true, true, true);
    let (first, bytes) = append_and_select(&mut p, 4);
    let tail = p.list;
    let readback = p.direct_readback().unwrap();
    p.release_events();
    for _ in 0..128 {
        p.f.backend.progress_cooperative_copy(first).unwrap();
        if p.f.copy(first).is_quiescent() {
            break;
        }
    }
    assert_eq!(p.f.copy(first).status(), BackendPollV1::Succeeded);
    assert_eq!(p.f.copy(tail).status(), BackendPollV1::Pending);
    assert!(
        p.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    let released = p.f.allocations[0][2];
    assert_eq!(
        owner(&p.f, released),
        (p.identities[0], p.source.as_slice())
    );
    p.f.backend.release_allocation_v1(released).unwrap();
    assert!(p.f.backend.release_submission_v1(first).is_err());
    drive(&mut p, readback);
    assert_eq!(
        p.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        owner(&p.f, p.f.allocations[1][3]),
        (p.identities[1], expected(&p, &bytes).as_slice())
    );
    p.f.clean_except(&[released]);
}

#[test]
fn ordered_segments_missing_latest_event_or_wrong_stream_never_mints_frame() {
    for wrong_stream in [false, true] {
        let mut p = PendingList::with_source_profile(None, true, false, false);
        let source = p.f.allocations[0][4];
        initialize_bytes(&mut p.f, source, 0x61);
        let stale = p.event;
        let first = p.list;
        let second = append(&mut p, source, stale).unwrap();
        let original_stream = p.f.peer_stream;
        if wrong_stream {
            p.f.peer_stream = p.f.backend.create_stream_v1(8).unwrap();
        }
        let result = append(&mut p, source, stale);
        if let Ok(id) = result {
            assert!(
                p.f.copy(id)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .segment_frame_v1()
                    .is_none()
            );
            assert_eq!(
                p.f.backend.cancel_v1(id).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        p.f.peer_stream = original_stream;
        p.f.backend.release_event_v1(stale).unwrap();
        for id in [second, first] {
            assert_eq!(
                p.f.backend.cancel_v1(id).unwrap(),
                BackendCancellationV1::Cancelled
            );
        }
        p.f.clean();
    }
}

#[test]
fn ordered_segments_cancelled_ancestor_fails_tail_without_native_extraction() {
    let mut p = PendingList::with_source_profile(None, true, false, false);
    let (first, _) = append_and_select(&mut p, 4);
    p.release_events();
    assert_eq!(
        p.f.backend.cancel_v1(first).unwrap(),
        BackendCancellationV1::Cancelled
    );
    for _ in 0..64 {
        match p.f.backend.progress_stream_v1(p.f.peer_stream) {
            Ok(()) => {}
            Err(Failure::Quiescent(_)) => {
                assert!(p.f.copy(p.list).is_quiescent());
            }
            other => panic!("unexpected cancellation progress: {other:?}"),
        }
        if p.f.copy(p.list).is_quiescent() {
            break;
        }
    }
    for id in [first, p.list] {
        assert!(matches!(
            p.f.copy(id).status(),
            BackendPollV1::Failed { .. }
        ));
        assert!(p.f.copy(id).is_quiescent());
        assert!(
            p.f.copy(id)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
    }
    assert!(
        p.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    p.f.clean();
}

#[test]
fn ordered_segments_predecessor_rank_plan_and_route_corruption_fail_closed() {
    for case in 0..4 {
        let mut p = PendingList::with_source_profile(None, true, false, false);
        let (first, _) = append_and_select(&mut p, 4);
        p.release_events();
        let copy = match p.f.backend.submissions.get_mut(&p.list).unwrap() {
            RoutedSubmissionV1::CooperativeCopy(copy) => copy,
            _ => unreachable!(),
        };
        match case {
            0 => copy.dependency_depth += 1,
            1 => copy.destination_region.byte_offset += 1,
            2 => copy.dependencies.clear(),
            3 => {
                let root = copy.compute_xgmi.as_mut().unwrap();
                let old = root.segments_for_test_v1().unwrap();
                let changed = Arc::new(
                    Gfx942ComputeXgmiSegmentsPlanV1::new(
                        old.source_logical_bytes(),
                        old.destination_logical_bytes(),
                        old.source_offset(),
                        old.source_len(),
                        old.destination_offset(),
                        old.destination_len(),
                        &descriptors(),
                    )
                    .unwrap(),
                );
                root.replace_segments_for_test_v1(changed);
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            p.f.backend.progress_stream_v1(p.f.peer_stream),
            Err(Failure::Terminal(_))
        ));
        assert!(p.f.backend.terminal);
        assert!(p.f.backend.submissions.contains_key(&first));
        assert!(p.f.backend.submissions.contains_key(&p.list));
        assert!(p.f.backend.submission_retained_as_dependency(first));
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        // Corrupt, terminal fixtures deliberately retain their modeled native owners.
    }
}

#[test]
fn ordered_segments_depth_bound_rejects_before_effect_and_refunds_all_retains() {
    let mut p = PendingList::with_source_profile(None, true, false, false);
    let source = p.f.allocations[0][4];
    initialize_bytes(&mut p.f, source, 0x61);
    let mut ids = vec![p.list];
    for depth in 2..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        let event = p.event;
        let next = append(&mut p, source, event).unwrap();
        assert_eq!(p.f.copy(next).dependency_depth, depth);
        assert_eq!(
            p.f.copy(next)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .segment_frame_v1()
                .unwrap()
                .depth(),
            depth
        );
        p.f.backend.release_event_v1(event).unwrap();
        p.list = next;
        p.event = p.f.backend.record_event_v1(p.f.peer_stream, next).unwrap();
        ids.push(next);
    }
    let before = (
        p.f.backend.next_handle,
        p.f.backend.submissions.len(),
        p.f.backend.cooperative_dependency_retain_counts.clone(),
    );
    let event = p.event;
    assert!(
        matches!(append(&mut p, source, event), Err(Failure::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!(
        (
            p.f.backend.next_handle,
            p.f.backend.submissions.len(),
            p.f.backend.cooperative_dependency_retain_counts.clone()
        ),
        before
    );
    p.release_events();
    for id in ids.into_iter().rev() {
        assert_eq!(
            p.f.backend.cancel_v1(id).unwrap(),
            BackendCancellationV1::Cancelled
        );
        p.f.backend.release_submission_v1(id).unwrap();
    }
    assert!(p.f.backend.cooperative_dependency_retain_counts.is_empty());
    p.f.clean();
}

#[test]
fn ordered_segments_published_ancestor_fault_retains_tail_frame_and_both_owners() {
    for unwind in [false, true] {
        let mut p = PendingList::with_source_profile(None, true, false, false);
        let (first, _) = append_and_select(&mut p, 4);
        p.release_events();
        for _ in 0..64 {
            p.f.backend.progress_cooperative_copy(first).unwrap();
            if p.f
                .copy(first)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .contains(&Stage::Copy)
            {
                break;
            }
        }
        assert_eq!(
            p.f.backend.compute_xgmi_children,
            [Some(first), Some(first)]
        );
        assert_eq!(
            p.f.backend.cancel_v1(first).unwrap(),
            BackendCancellationV1::TooLate
        );
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            p.f.backend.submissions.get_mut(&first).unwrap()
        else {
            unreachable!()
        };
        copy.compute_xgmi
            .as_mut()
            .unwrap()
            .inject_failure_for_test_v1(Stage::NextSegment, unwind);
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Failure> {
            for _ in 0..128 {
                p.f.backend.progress_stream_v1(p.f.peer_stream)?;
            }
            Ok(())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result.unwrap(), Err(Failure::Terminal(_))));
        }
        assert!(p.f.backend.terminal);
        assert_eq!(
            p.f.backend.compute_xgmi_children,
            [Some(first), Some(first)]
        );
        assert!(p.f.backend.submission_retained_as_dependency(first));
        assert_eq!(p.f.copy(p.list).status(), BackendPollV1::Pending);
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert!(
            p.f.copy(first)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .scripted_owners_for_test_v1()
                .iter()
                .all(Option::is_some)
        );
        assert_eq!(p.f.backend.cooperative_progress_quantum, None);
    }
}
