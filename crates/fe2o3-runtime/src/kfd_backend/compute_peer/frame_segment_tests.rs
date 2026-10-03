//! Scripted native ownership and descriptor execution for frame-backed lists.

use super::*;

fn segments() -> Vec<RuntimePeerCopySegmentV1> {
    [(0, 5, 17), (32, 1, 8), (3, 14, 9), (3, 14, 9)]
        .map(
            |(source_offset, destination_offset, byte_len)| RuntimePeerCopySegmentV1 {
                source_offset,
                destination_offset,
                byte_len,
            },
        )
        .to_vec()
}

fn envelopes(p: &Forward) -> [BackendMemoryRegionV1; 2] {
    let [source, mut destination] = p.regions();
    destination.byte_len = BYTES as u64 - destination.byte_offset;
    [source, destination]
}

fn submit(p: &mut Forward) -> Result<u64, Failure> {
    let [source, destination] = envelopes(p);
    p.f.backend.peer_copy_segments_v1(
        p.stream,
        source,
        destination,
        &segments(),
        &[*p.events.last().unwrap()],
    )
}

fn expected(p: &Forward) -> Vec<u8> {
    let mut output = p.destination.clone();
    for segment in segments() {
        let src = (1 + segment.source_offset) as usize;
        let dst = (7 + segment.destination_offset) as usize;
        let len = segment.byte_len as usize;
        output[dst..dst + len].copy_from_slice(&p.frame[src..src + len]);
    }
    output
}

#[test]
fn frame_segments_prequeued_late_and_settled_full_readback_preserve_native_owners() {
    for timing in 0..4 {
        let mut p = Forward::new(true, true);
        let tail = *p.lists.last().unwrap();
        match timing {
            1 => p.publish(p.lists[0]),
            2 => p.publish(tail),
            3 => p.drive(p.f.peer_stream, tail),
            _ => {}
        }
        let list = submit(&mut p).unwrap();
        assert!(p.f.copy(list).compute_producer.is_none());
        assert_eq!(p.f.copy(list).frame_source.is_some(), timing != 3);
        assert!(p.f.copy(list).staging.is_empty());
        let readback = p.readback(list);
        p.release_events();
        let generation = p.f.backend.cooperative_progress_generation;
        for id in [list, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(
                p.f.backend.wait_v1(id, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert!(p.f.backend.release_submission_v1(id).is_err());
        }
        assert_eq!(p.f.backend.cooperative_progress_generation, generation);
        p.drive(p.f.readback_stream, readback);
        for id in p.lists.iter().copied().chain([list, readback]) {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
            assert!(p.f.copy(id).is_quiescent());
        }
        assert_eq!(
            owner(&p.f, p.f.allocations[0][2]),
            (p.ids[0], p.source.as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.ids[1], p.frame.as_slice())
        );
        let output = expected(&p);
        assert_eq!(
            owner(&p.f, p.f.allocations[0][3]),
            (p.ids[2], output.as_slice())
        );
        let route = p.f.backend.allocations[&p.f.host.unwrap()];
        let KfdRuntimeSdmaStorageV1::Host(host) =
            &p.f.backend.children[route.child].allocations[&route.local].sdma_storage
        else {
            unreachable!()
        };
        assert_eq!(host.scripted_bytes().unwrap(), output);
        assert!(p.f.backend.cooperative_dependency_retain_counts.is_empty());
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
fn frame_segments_cancel_or_cancelled_parent_do_not_publish_successor() {
    for cancel_parent in [false, true] {
        let mut p = Forward::new(false, false);
        let list = submit(&mut p).unwrap();
        p.release_events();
        let cancelled = if cancel_parent { p.lists[0] } else { list };
        assert_eq!(
            p.f.backend.cancel_v1(cancelled).unwrap(),
            BackendCancellationV1::Cancelled
        );
        if cancel_parent {
            for _ in 0..8 {
                if p.f.backend.progress_cooperative_copy(list).unwrap() != BackendPollV1::Pending {
                    break;
                }
            }
            assert!(matches!(
                p.f.copy(list).status(),
                BackendPollV1::Failed { .. }
            ));
        } else {
            assert!(matches!(
                p.f.copy(list).status(),
                BackendPollV1::Failed { .. }
            ));
            assert_eq!(p.f.copy(p.lists[0]).status(), BackendPollV1::Pending);
            p.drive(p.f.peer_stream, p.lists[0]);
        }
        assert!(p.f.copy(list).is_quiescent());
        assert!(
            p.f.copy(list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        p.assert_destination_untouched();
        assert!(p.f.backend.cooperative_dependency_retain_counts.is_empty());
        p.f.clean();
    }
}

#[test]
fn frame_segments_reject_stale_controls_uninitialized_and_occupied_destination() {
    for mode in 0..5 {
        let mut p = Forward::new(true, false);
        let [source, mut destination] = envelopes(&p);
        let mut events = vec![*p.events.last().unwrap()];
        match mode {
            0 => events[0] = p.events[0],
            1 => events.clear(),
            2 => events.push(p.events[0]),
            3 => destination.allocation = p.f.allocations[0][2],
            4 => {
                let route = p.f.backend.allocations[&destination.allocation];
                p.f.backend.children[route.child]
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .sdma_initialized = false;
            }
            _ => unreachable!(),
        }
        let before = p.f.backend.submissions.len();
        assert!(matches!(
            p.f.backend
                .peer_copy_segments_v1(p.stream, source, destination, &segments(), &events),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(p.f.backend.submissions.len(), before);
        assert!(!p.f.backend.terminal);
        if mode == 4 {
            let route = p.f.backend.allocations[&destination.allocation];
            p.f.backend.children[route.child]
                .allocations
                .get_mut(&route.local)
                .unwrap()
                .sdma_initialized = true;
        }
        p.release_events();
        p.drive(p.f.peer_stream, *p.lists.last().unwrap());
        p.f.clean();
    }
}

#[test]
fn frame_segments_plan_receipt_rank_and_pair_drift_fail_before_publication() {
    for mode in 0..6 {
        let mut p = Forward::new(true, false);
        let list = submit(&mut p).unwrap();
        let [source, destination] = envelopes(&p);
        if mode == 5 {
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                p.f.backend
                    .submissions
                    .get_mut(p.lists.last().unwrap())
                    .unwrap()
            else {
                unreachable!()
            };
            let root = copy.compute_xgmi.as_mut().unwrap();
            let replacement = Arc::new((**root.segment_frame_v1().unwrap()).clone());
            root.clear_segment_frame_for_test_v1();
            root.bind_segment_frame_v1(Some(replacement));
        } else if mode == 4 {
            p.publish(p.lists[0]);
            p.f.backend.compute_xgmi_children[0] = None;
        } else {
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                p.f.backend.submissions.get_mut(&list).unwrap()
            else {
                unreachable!()
            };
            match mode {
                0 => copy.dependency_depth = 1,
                1 => copy.source_region.byte_offset += 1,
                2 => {
                    let plan = Arc::new(
                        Gfx942ComputeXgmiSegmentsPlanV1::new(
                            BYTES as u64,
                            BYTES as u64,
                            source.byte_offset,
                            source.byte_len,
                            destination.byte_offset,
                            destination.byte_len,
                            &segments(),
                        )
                        .unwrap(),
                    );
                    copy.compute_xgmi
                        .as_mut()
                        .unwrap()
                        .replace_segments_for_test_v1(plan);
                }
                3 => copy
                    .compute_xgmi
                    .as_mut()
                    .unwrap()
                    .clear_segment_frame_for_test_v1(),
                _ => unreachable!(),
            }
        }
        assert!(
            matches!(
                p.f.backend.progress_cooperative_copy(list),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ),
            "mode {mode}"
        );
        assert!(p.f.backend.terminal);
        assert!(
            p.f.copy(list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert!(p.f.copy(list).frame_source.is_some());
    }
}

#[test]
fn frame_segments_completed_ancestor_source_can_be_disposed_before_forward_progress() {
    let mut p = Forward::new(false, false);
    let list = submit(&mut p).unwrap();
    p.release_events();
    p.drive(p.f.peer_stream, p.lists[0]);
    assert_eq!(p.f.copy(p.lists[0]).status(), BackendPollV1::Succeeded);
    let original = p.f.allocations[0][2];
    p.f.backend.release_allocation_v1(original).unwrap();
    p.drive(p.stream, list);
    assert_eq!(p.f.copy(list).status(), BackendPollV1::Succeeded);
    assert_eq!(
        owner(&p.f, p.f.allocations[0][3]),
        (p.ids[2], expected(&p).as_slice())
    );
    p.f.clean_except(&[original]);
}

#[test]
fn frame_segments_three_hops_authenticate_occupied_source_ancestors() {
    for timing in 0..5 {
        let mut p = Forward::new(false, false);
        let middle = submit(&mut p).unwrap();
        let destination = p.f.allocations[1][4];
        let (identity, mut output) = initialize_bytes(&mut p.f, destination, 0x79);
        let stream = p.f.backend.create_stream_v1(8).unwrap();
        let event = p.f.backend.record_event_v1(p.stream, middle).unwrap();
        if timing == 2 {
            p.publish(p.lists[0]);
        }
        let tail =
            p.f.backend
                .peer_copy_segments_v1(
                    stream,
                    region(p.f.allocations[0][3], RuntimeAccessV1::Read),
                    region(destination, RuntimeAccessV1::Write),
                    &segments(),
                    &[event],
                )
                .unwrap();
        p.f.backend.release_event_v1(event).unwrap();
        p.release_events();
        if timing == 1 || timing >= 3 {
            p.publish(p.lists[0]);
        }
        if timing >= 3 {
            p.f.backend.compute_xgmi_children[timing - 3] = None;
            assert!(matches!(
                p.f.backend.progress_cooperative_copy(tail),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
            assert!(p.f.backend.terminal);
            assert!(
                p.f.copy(tail)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .trace_for_test_v1()
                    .is_empty()
            );
            assert!(p.f.backend.submission_retained_as_dependency(middle));
            continue;
        }
        p.drive(stream, tail);
        let middle_bytes = expected(&p);
        for segment in segments() {
            let src = segment.source_offset as usize;
            let dst = segment.destination_offset as usize;
            let len = segment.byte_len as usize;
            output[dst..dst + len].copy_from_slice(&middle_bytes[src..src + len]);
        }
        for id in [p.lists[0], middle, tail] {
            assert_eq!(p.f.copy(id).status(), BackendPollV1::Succeeded);
            assert!(p.f.copy(id).is_quiescent());
        }
        assert_eq!(
            owner(&p.f, p.f.allocations[0][2]),
            (p.ids[0], p.source.as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.ids[1], p.frame.as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[0][3]),
            (p.ids[2], middle_bytes.as_slice())
        );
        assert_eq!(owner(&p.f, destination), (identity, output.as_slice()));
        assert!(p.f.backend.cooperative_dependency_retain_counts.is_empty());
        assert!(
            p.f.backend
                .compute_xgmi_children
                .iter()
                .all(Option::is_none)
        );
        p.f.clean();
    }
}
