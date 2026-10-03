//! Settled native owners and real routed custody; scripted compute does not emulate arithmetic.

use super::*;

fn settled(direct: bool, readback: bool, readback_will_run: bool) -> PendingList {
    let p = PendingList::with_source_profile(None, direct, readback, readback_will_run);
    assert!(p.producer.is_none() && p.producer_event.is_none());
    assert!(p.f.copy(p.list).compute_producer.is_none());
    assert!(p.f.copy(p.list).dependencies.is_empty());
    assert_eq!(p.f.copy(p.list).dependency_depth, 1);
    assert!(
        p.f.copy(p.list)
            .compute_xgmi
            .as_ref()
            .unwrap()
            .segment_frame_v1()
            .unwrap()
            .is_settled_source_for_test_v1()
    );
    p
}

fn return_and_readback(p: &mut PendingList, consumer: u64) -> (u64, u64) {
    let event = p.f.event(1, consumer);
    p.f.backend.compute_xgmi_routes.insert(
        (1, 0),
        Route::Scripted {
            failure: None,
            unwind: false,
            pending_samples: 2,
        },
    );
    let stream = p.f.backend.create_stream_v1(7).unwrap();
    let returned =
        p.f.backend
            .peer_copy_v1(
                stream,
                BackendMemoryRegionV1 {
                    byte_offset: 17,
                    byte_len: 47,
                    ..region(p.f.allocations[1][2], RuntimeAccessV1::Read)
                },
                BackendMemoryRegionV1 {
                    byte_offset: 7,
                    byte_len: 47,
                    ..region(p.f.allocations[0][3], RuntimeAccessV1::Write)
                },
                &[event],
            )
            .unwrap();
    p.f.backend.release_event_v1(event).unwrap();
    let event = p.f.backend.record_event_v1(stream, returned).unwrap();
    let readback =
        p.f.backend
            .copy_async_v1(
                p.f.readback_stream,
                BackendMemoryRegionV1 {
                    byte_offset: 7,
                    byte_len: 47,
                    ..region(p.f.allocations[0][3], RuntimeAccessV1::Read)
                },
                BackendMemoryRegionV1 {
                    byte_offset: 9,
                    byte_len: 47,
                    ..region(p.f.host.unwrap(), RuntimeAccessV1::Write)
                },
                &[event],
            )
            .unwrap();
    p.f.backend.release_event_v1(event).unwrap();
    (returned, readback)
}

#[test]
fn settled_segments_full_frame_readback_prequeued_and_late_preserves_guards() {
    for late in [false, true] {
        let mut p = settled(true, true, true);
        if late {
            p.publish();
        }
        let readback = p.direct_readback().unwrap();
        p.release_events();
        assert!(p.f.backend.release_submission_v1(p.list).is_err());
        let generation = p.f.backend.cooperative_progress_generation;
        assert_eq!(
            p.f.backend.poll_v1(readback).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(
            p.f.backend.wait_v1(readback, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(p.f.backend.cooperative_progress_generation, generation);
        p.drive(p.f.readback_stream, readback, None);
        assert_eq!(
            p.f.backend.poll_v1(readback).unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[0][2]),
            (p.identities[0], p.source.as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], p.expected_input(4).as_slice())
        );
        let route = p.f.backend.allocations[&p.f.host.unwrap()];
        let KfdRuntimeSdmaStorageV1::Host(host) =
            &p.f.backend.children[1].allocations[&route.local].sdma_storage
        else {
            unreachable!()
        };
        assert_eq!(host.scripted_bytes().unwrap(), p.expected_input(4));
        assert_eq!(p.f.backend.completed_compute_xgmi_copies, 0);
        p.f.clean();
    }
}

#[test]
fn settled_segments_compute_return_readback_uses_only_final_stream_and_exact_owners() {
    for late in [false, true] {
        let mut p = settled(false, true, true);
        if late {
            p.publish();
        }
        let consumer = p.consumer();
        p.assert_deferred(consumer);
        let (returned, readback) = return_and_readback(&mut p, consumer);
        p.release_events();
        let generation = p.f.backend.cooperative_progress_generation;
        for id in [p.list, consumer, returned, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert!(p.f.backend.release_submission_v1(id).is_err());
        }
        assert_eq!(p.f.backend.cooperative_progress_generation, generation);
        p.drive(p.f.readback_stream, readback, Some(consumer));
        for id in [p.list, consumer, returned, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(
            owner(&p.f, p.f.allocations[0][2]),
            (p.identities[0], p.source.as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], p.expected_input(4).as_slice())
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][2]),
            (p.identities[2], p.output.as_slice())
        );
        p.returned[7..54].copy_from_slice(&p.output[17..64]);
        assert_eq!(
            owner(&p.f, p.f.allocations[0][3]),
            (p.identities[3], p.returned.as_slice())
        );
        let route = p.f.backend.allocations[&p.f.host.unwrap()];
        let KfdRuntimeSdmaStorageV1::Host(host) =
            &p.f.backend.children[0].allocations[&route.local].sdma_storage
        else {
            unreachable!()
        };
        let mut expected = vec![0; BYTES];
        expected[9..56].copy_from_slice(&p.output[17..64]);
        assert_eq!(host.scripted_bytes().unwrap(), expected);
        let trace =
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1();
        for stage in [Stage::Create, Stage::Finish, Stage::Retire, Stage::Restore] {
            assert_eq!(trace.iter().filter(|seen| **seen == stage).count(), 1);
        }
        assert_eq!(
            trace
                .iter()
                .filter(|seen| **seen == Stage::NextSegment)
                .count(),
            3
        );
        assert!(p.f.backend.deferred_compute_retains.is_empty());
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
fn settled_segments_frame_aliases_and_rejections_refund_without_native_effects() {
    for case in 0..7 {
        // No promotion is scripted: every accepted consumer is canceled before handoff.
        let mut p = settled(true, false, false);
        let mut bindings = p.bindings();
        let mut dependency = BackendLaunchProducerV1 {
            event: p.event,
            producer_submission: p.list,
        };
        match case {
            0 => {}
            1 => {
                bindings[0].region.byte_offset = 32;
                bindings[0].region.byte_len = 16;
            }
            2 => {
                bindings[0].region.byte_offset = 32;
                bindings[0].region.byte_len = 16;
                bindings[1].region = BackendMemoryRegionV1 {
                    byte_offset: 1,
                    byte_len: 3,
                    ..bindings[0].region
                };
            }
            3 => bindings[0].region.access = RuntimeAccessV1::ReadWrite,
            4 => {
                bindings[1].region = BackendMemoryRegionV1 {
                    access: RuntimeAccessV1::Write,
                    ..bindings[0].region
                }
            }
            5 => bindings[0].region.byte_len += 1,
            6 => dependency.producer_submission += 1,
            _ => unreachable!(),
        }
        let before = (p.f.backend.next_handle, p.f.backend.submissions.len());
        let result = p.submit_consumer(&bindings, dependency);
        if case < 3 {
            let consumer = result.unwrap();
            p.assert_deferred(consumer);
            p.release_events();
            assert_eq!(
                p.f.backend.cancel_v1(consumer).unwrap(),
                BackendCancellationV1::Cancelled
            );
            assert!(p.f.backend.deferred_compute_retains.is_empty());
            p.f.backend.release_submission_v1(consumer).unwrap();
        } else {
            assert!(matches!(result, Err(RuntimeBackendFailureV1::Rejected(_))));
            assert_eq!(
                (p.f.backend.next_handle, p.f.backend.submissions.len()),
                before
            );
            p.release_events();
        }
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert_eq!(
            p.f.backend.cancel_v1(p.list).unwrap(),
            BackendCancellationV1::Cancelled
        );
        p.f.clean();
    }
}

#[test]
fn settled_segments_cancelled_parent_never_hands_off_compute_or_readback() {
    for direct in [false, true] {
        let mut p = settled(true, direct, false);
        let target = if direct {
            p.direct_readback().unwrap()
        } else {
            p.consumer()
        };
        p.release_events();
        assert_eq!(
            p.f.backend.cancel_v1(p.list).unwrap(),
            BackendCancellationV1::Cancelled
        );
        let stream = if direct {
            p.f.readback_stream
        } else {
            p.f.compute_streams[1]
        };
        for _ in 0..32 {
            match p.f.backend.progress_stream_v1(stream) {
                Ok(()) => {}
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    assert!(matches!(
                        p.f.backend.poll_v1(target).unwrap(),
                        BackendPollV1::Failed { .. }
                    ));
                }
                other => panic!("unexpected cancellation disposition: {other:?}"),
            }
            if matches!(
                p.f.backend.poll_v1(target).unwrap(),
                BackendPollV1::Failed { .. }
            ) {
                break;
            }
        }
        assert!(matches!(
            p.f.backend.poll_v1(target).unwrap(),
            BackendPollV1::Failed { .. }
        ));
        if direct {
            assert!(p.f.copy(target).is_quiescent());
        } else {
            assert!(
                p.f.backend
                    .deferred_compute_v1(target)
                    .unwrap()
                    .route
                    .is_none()
            );
            assert!(p.f.backend.deferred_compute_retains.is_empty());
        }
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
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
fn settled_segments_exact_frame_rejects_plan_rank_and_endpoint_drift() {
    for late in [false, true] {
        for case in 0..4 {
            let mut p = settled(true, false, false);
            let consumer = p.consumer();
            if late {
                p.publish();
            }
            p.release_events();
            let trace =
                p.f.copy(p.list)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .trace_for_test_v1()
                    .to_vec();
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                p.f.backend.submissions.get_mut(&p.list).unwrap()
            else {
                unreachable!()
            };
            match case {
                0 | 1 => {
                    let mut list = descriptors();
                    if case == 1 {
                        list[3].destination_offset += 1;
                    }
                    let plan = Arc::new(
                        Gfx942ComputeXgmiSegmentsPlanV1::new(
                            BYTES as u64,
                            BYTES as u64,
                            3,
                            53,
                            5,
                            51,
                            &list,
                        )
                        .unwrap(),
                    );
                    copy.compute_xgmi
                        .as_mut()
                        .unwrap()
                        .replace_segments_for_test_v1(plan);
                }
                2 => copy.dependency_depth += 1,
                3 => copy.destination_region.byte_offset += 1,
                _ => unreachable!(),
            }
            assert!(
                p.f.backend
                    .progress_stream_v1(p.f.compute_streams[1])
                    .is_err()
            );
            assert_eq!(
                p.f.copy(p.list)
                    .compute_xgmi
                    .as_ref()
                    .unwrap()
                    .trace_for_test_v1(),
                trace
            );
            p.assert_deferred(consumer);
            if late {
                p.assert_pair();
            }
            // Corrupted uncertain roots remain owned; no synthetic retirement.
        }
    }
}

#[test]
fn settled_segments_native_fault_keeps_independent_frame_and_both_owners() {
    for unwind in [false, true] {
        let mut p = settled(true, false, false);
        let consumer = p.consumer();
        p.publish();
        p.release_events();
        assert_eq!(
            p.f.backend.cancel_v1(p.list).unwrap(),
            BackendCancellationV1::TooLate
        );
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            p.f.backend.submissions.get_mut(&p.list).unwrap()
        else {
            unreachable!()
        };
        copy.compute_xgmi
            .as_mut()
            .unwrap()
            .inject_failure_for_test_v1(Stage::NextSegment, unwind);
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), Failure> {
            for _ in 0..64 {
                p.f.backend.progress_stream_v1(p.f.compute_streams[1])?;
            }
            Ok(())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        p.assert_pair();
        p.assert_deferred(consumer);
        assert!(p.f.backend.terminal && p.f.backend.children.iter().all(|child| child.terminal));
        assert!(p.f.backend.peer_launch_retains.retains(p.list));
        assert_eq!(p.f.backend.cooperative_progress_quantum, None);
    }
}

#[test]
fn settled_segments_restored_frame_outlives_source_release_before_compute_handoff() {
    let mut p = settled(false, false, false);
    let consumer = p.consumer();
    p.release_events();
    assert!(
        p.f.backend
            .release_allocation_v1(p.f.allocations[0][2])
            .is_err()
    );
    assert!(
        p.f.backend
            .release_allocation_v1(p.f.allocations[1][3])
            .is_err()
    );
    for _ in 0..128 {
        let status =
            p.f.backend
                .drain_v1(p.list, Instant::now() + Duration::from_millis(10))
                .unwrap();
        if status == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        p.f.backend.poll_v1(p.list).unwrap(),
        BackendPollV1::Succeeded
    );
    p.assert_deferred(consumer);
    assert!(
        p.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    let source = p.f.allocations[0][2];
    p.f.backend.release_allocation_v1(source).unwrap();
    assert!(
        p.f.backend
            .release_allocation_v1(p.f.allocations[1][3])
            .is_err()
    );
    for _ in 0..64 {
        p.f.backend
            .progress_stream_v1(p.f.compute_streams[1])
            .unwrap();
        if p.f.backend.poll_v1(consumer).unwrap() == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(
        p.f.backend.poll_v1(consumer).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        owner(&p.f, p.f.allocations[1][3]),
        (p.identities[1], p.expected_input(4).as_slice())
    );
    p.f.clean_except(&[source]);
}

fn discard_unpublished_list(p: &mut PendingList) {
    p.release_events();
    assert_eq!(
        p.f.backend.cancel_v1(p.list).unwrap(),
        BackendCancellationV1::Cancelled
    );
    p.f.backend.release_submission_v1(p.list).unwrap();
}

fn admit_list_after_controls(p: &mut PendingList, controls: &[u64]) {
    let [source, destination] = regions(&p.f);
    p.list =
        p.f.backend
            .peer_copy_segments_v1(
                p.f.peer_stream,
                source,
                destination,
                &descriptors(),
                controls,
            )
            .unwrap();
    p.event =
        p.f.backend
            .record_event_v1(p.f.peer_stream, p.list)
            .unwrap();
    assert!(p.f.copy(p.list).compute_producer.is_none());
}

#[test]
fn settled_segments_pending_controls_gate_frame_without_changing_endpoint_admission() {
    for explicit in [false, true] {
        let mut p = settled(true, true, true);
        discard_unpublished_list(&mut p);
        let control =
            p.f.backend
                .peer_copy_v1(
                    p.f.peer_stream,
                    region(p.f.allocations[0][8], RuntimeAccessV1::Read),
                    region(p.f.allocations[1][8], RuntimeAccessV1::Write),
                    &[],
                )
                .unwrap();
        let event =
            p.f.backend
                .record_event_v1(p.f.peer_stream, control)
                .unwrap();
        admit_list_after_controls(
            &mut p,
            if explicit {
                std::slice::from_ref(&event)
            } else {
                &[]
            },
        );
        assert_eq!(p.f.copy(p.list).dependencies, [control]);
        assert_eq!(p.f.copy(p.list).dependency_depth, 2);
        assert!(
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .segment_frame_v1()
                .is_some()
        );
        let readback = p.direct_readback().unwrap();
        p.f.backend.release_event_v1(event).unwrap();
        p.release_events();
        assert!(p.f.backend.release_submission_v1(control).is_err());
        p.drive(p.f.readback_stream, readback, None);
        for id in [control, p.list, readback] {
            assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        }
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], p.expected_input(4).as_slice())
        );
        p.f.clean();
    }
}

#[test]
fn settled_segments_completed_control_depth_preserves_legacy_transfer_limit() {
    for depth in [
        MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 - 1,
        MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1,
    ] {
        let mut p = settled(true, false, false);
        discard_unpublished_list(&mut p);
        let mut event = None;
        let mut control = 0;
        for expected in 1..=depth {
            control =
                p.f.backend
                    .peer_copy_v1(
                        p.f.peer_stream,
                        region(p.f.allocations[0][8], RuntimeAccessV1::Read),
                        region(p.f.allocations[1][8], RuntimeAccessV1::Write),
                        event.as_slice(),
                    )
                    .unwrap();
            assert_eq!(p.f.copy(control).dependency_depth, expected);
            if let Some(prior) = event {
                p.f.backend.release_event_v1(prior).unwrap();
            }
            event = Some(
                p.f.backend
                    .record_event_v1(p.f.peer_stream, control)
                    .unwrap(),
            );
        }
        for _ in 0..16 {
            if p.f
                .backend
                .drain_v1(control, Instant::now() + Duration::from_millis(100))
                .unwrap()
                == BackendPollV1::Succeeded
            {
                break;
            }
        }
        assert_eq!(
            p.f.backend.poll_v1(control).unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(p.f.copy(control).is_quiescent());
        admit_list_after_controls(&mut p, event.as_slice());
        let frame =
            p.f.copy(p.list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .segment_frame_v1();
        assert_eq!(
            frame.is_some(),
            depth < MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
        );
        assert_eq!(p.f.copy(p.list).dependency_depth, 1);
        assert_eq!(
            frame.map(|frame| frame.depth()),
            (depth < MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1).then_some(depth + 1)
        );
        // Frame provenance must not promote the old transfer-ordering rank or
        // reject a subsequent legacy tail that was previously admissible.
        let successor =
            p.f.backend
                .peer_copy_v1(
                    p.f.peer_stream,
                    region(p.f.allocations[0][9], RuntimeAccessV1::Read),
                    region(p.f.allocations[1][9], RuntimeAccessV1::Write),
                    &[p.event],
                )
                .unwrap();
        assert_eq!(p.f.copy(successor).dependency_depth, 2);
        p.f.backend.release_event_v1(event.unwrap()).unwrap();
        p.release_events();
        p.drive(p.f.peer_stream, successor, None);
        assert_eq!(
            p.f.backend.poll_v1(p.list).unwrap(),
            BackendPollV1::Succeeded
        );
        assert_eq!(
            owner(&p.f, p.f.allocations[1][3]),
            (p.identities[1], p.expected_input(4).as_slice())
        );
        if depth == MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 - 1 {
            assert_eq!(
                p.f.backend.compute_peer_dependency_depth_v1(p.list),
                Some(1)
            );
            assert_eq!(
                p.f.backend.segment_frame_dependency_depth_v1(p.list),
                Some(depth + 1)
            );
            let producer = p.f.launch(0, 3, false, true);
            let producer_event = p.f.event(0, producer);
            let list_event =
                p.f.backend
                    .record_event_v1(p.f.peer_stream, p.list)
                    .unwrap();
            let stream = p.f.backend.create_stream_v1(8).unwrap();
            let peer =
                p.f.backend
                    .peer_copy_v1(
                        stream,
                        region(p.f.allocations[0][5], RuntimeAccessV1::Read),
                        region(p.f.allocations[1][5], RuntimeAccessV1::Write),
                        &[producer_event, list_event],
                    )
                    .unwrap();
            assert_eq!(p.f.copy(peer).dependency_depth, 2);
            assert_eq!(
                p.f.backend.cancel_v1(peer).unwrap(),
                BackendCancellationV1::Cancelled
            );
            p.f.backend.release_event_v1(list_event).unwrap();
            p.f.backend.release_event_v1(producer_event).unwrap();
            p.f.drive(p.f.compute_streams[0], producer);
        }
        p.f.clean();
    }
}

#[test]
fn settled_segments_completed_native_copy_control_uses_its_retained_rank() {
    let steps = vec![
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::DeviceToHost,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: BYTES as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    let mut f = Fixture::with_layout_driver_prefixes_and_readback_progress(
        [BYTES; 2],
        Some((1, 0, 0, BYTES)),
        false,
        [vec![], steps],
        false,
    );
    let control = f
        .backend
        .copy_async_v1(
            f.readback_stream,
            region(f.allocations[1][8], RuntimeAccessV1::Read),
            region(f.host.unwrap(), RuntimeAccessV1::Write),
            &[],
        )
        .unwrap();
    f.drive(f.readback_stream, control);
    let RoutedSubmissionV1::Native { route, .. } = f.backend.submissions[&control] else {
        unreachable!()
    };
    assert_eq!(
        f.backend.poll_v1(control).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(f.backend.children[route.child].exact_submission_quiescent_v1(route.local));
    assert!(!f.backend.producer_aware_native.contains_key(&control));
    assert_eq!(f.backend.compute_peer_dependency_depth_v1(control), Some(1));
    assert_eq!(
        f.backend.segment_frame_dependency_depth_v1(control),
        Some(1)
    );
    // A rank projection is not synthesized from an event or a global route if
    // the actual retained completion record disappears. Restore the genuine
    // completed record after this metadata-corruption probe.
    let record = f.backend.children[route.child]
        .submissions
        .remove(&route.local)
        .unwrap();
    assert_eq!(f.backend.segment_frame_dependency_depth_v1(control), None);
    f.backend.children[route.child]
        .submissions
        .insert(route.local, record);
    let event = f
        .backend
        .record_event_v1(f.readback_stream, control)
        .unwrap();
    let [source, destination] = regions(&f);
    let list = f
        .backend
        .peer_copy_segments_v1(f.peer_stream, source, destination, &descriptors(), &[event])
        .unwrap();
    let frame = f
        .copy(list)
        .compute_xgmi
        .as_ref()
        .unwrap()
        .segment_frame_v1()
        .unwrap();
    assert!(frame.is_settled_source_for_test_v1());
    assert_eq!(frame.depth(), 2);
    assert_eq!(f.copy(list).dependency_depth, 1);
    assert!(f.copy(list).compute_producer.is_none());
    f.backend.release_event_v1(event).unwrap();
    f.drive(f.peer_stream, list);
    assert_eq!(f.backend.poll_v1(list).unwrap(), BackendPollV1::Succeeded);
    f.clean();
}
