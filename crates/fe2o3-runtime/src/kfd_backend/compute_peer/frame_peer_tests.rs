//! Real scripted list/window ownership and byte copies, not compute emulation.

use super::*;
use crate::BackendCancellationV1;

struct Forward {
    f: Fixture,
    lists: Vec<u64>,
    events: Vec<u64>,
    stream: u64,
    ids: [u64; 3],
    source: Vec<u8>,
    frame: Vec<u8>,
    destination: Vec<u8>,
}

impl Forward {
    fn new(ordered: bool, readback: bool) -> Self {
        Self::with_readback_route(ordered, readback, false)
    }

    fn with_readback_route(ordered: bool, readback: bool, direct: bool) -> Self {
        let direct_steps = if direct {
            vec![
                ScriptedSdmaStepV1::Submit {
                    direction: Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                    host_offset: 0,
                    device_offset: 0,
                    copy_bytes: BYTES as u32,
                    outcome: ScriptedFailureModeV1::Success,
                },
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Pending),
                ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
                    direction: None,
                    copy_bytes: None,
                }),
                ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ]
        } else {
            Vec::new()
        };
        let mut f = Fixture::with_layout_driver_prefixes_and_readback_progress(
            [BYTES; 2],
            readback.then_some((0, 0, 0, BYTES)),
            false,
            [direct_steps, Vec::new()],
            !direct,
        );
        let source_allocation = f.allocations[0][2];
        let frame_allocation = f.allocations[1][3];
        let destination_allocation = f.allocations[0][3];
        let (source_id, source) = initialize_bytes(&mut f, source_allocation, 0x31);
        let (frame_id, mut frame) = initialize_bytes(&mut f, frame_allocation, 0xc3);
        let (destination_id, destination) = initialize_bytes(&mut f, destination_allocation, 0xaf);
        let [source_region, destination_region] = regions(&f);
        let mut lists = Vec::new();
        let mut events = Vec::new();
        for round in 0..if ordered { 2 } else { 1 } {
            let mut segments = descriptors();
            if round == 1 {
                segments[0].source_offset = 19;
                segments[1].destination_offset = 19;
            }
            let list = f
                .backend
                .peer_copy_segments_v1(
                    f.peer_stream,
                    source_region,
                    destination_region,
                    &segments,
                    events.last().map(core::slice::from_ref).unwrap_or(&[]),
                )
                .unwrap();
            let event = f.backend.record_event_v1(f.peer_stream, list).unwrap();
            lists.push(list);
            events.push(event);
            for segment in segments {
                let src = (source_region.byte_offset + segment.source_offset) as usize;
                let dst = (destination_region.byte_offset + segment.destination_offset) as usize;
                let len = segment.byte_len as usize;
                frame[dst..dst + len].copy_from_slice(&source[src..src + len]);
            }
        }
        f.backend.compute_xgmi_routes.insert(
            (1, 0),
            Route::Scripted {
                failure: None,
                unwind: false,
                pending_samples: 2,
            },
        );
        let stream = f.backend.create_stream_v1(7).unwrap();
        Self {
            f,
            lists,
            events,
            stream,
            ids: [source_id, frame_id, destination_id],
            source,
            frame,
            destination,
        }
    }

    fn regions(&self) -> [BackendMemoryRegionV1; 2] {
        [
            BackendMemoryRegionV1 {
                byte_offset: 1,
                byte_len: 47,
                ..region(self.f.allocations[1][3], RuntimeAccessV1::Read)
            },
            BackendMemoryRegionV1 {
                byte_offset: 7,
                byte_len: 47,
                ..region(self.f.allocations[0][3], RuntimeAccessV1::Write)
            },
        ]
    }

    fn submit(&mut self) -> Result<u64, Failure> {
        let [source, destination] = self.regions();
        self.f.backend.peer_copy_v1(
            self.stream,
            source,
            destination,
            &[*self.events.last().unwrap()],
        )
    }

    fn readback(&mut self, peer: u64) -> u64 {
        let event = self.f.backend.record_event_v1(self.stream, peer).unwrap();
        let pending = self.f.copy(peer).status() == BackendPollV1::Pending;
        let source = region(self.f.allocations[0][3], RuntimeAccessV1::Read);
        let destination = region(self.f.host.unwrap(), RuntimeAccessV1::Write);
        let stream_route = self.f.backend.streams[&self.f.readback_stream];
        let source_route = self.f.backend.allocations[&source.allocation];
        let destination_route = self.f.backend.allocations[&destination.allocation];
        assert_eq!(
            self.f
                .backend
                .pending_native_peer_readback_v1(
                    stream_route,
                    source,
                    source_route,
                    destination,
                    destination_route,
                    &[event],
                )
                .unwrap(),
            pending
        );
        if !pending {
            assert_eq!(self.f.backend.dependency_for_child(event, 0).unwrap(), None);
        }
        let readback = self
            .f
            .backend
            .copy_async_v1(self.f.readback_stream, source, destination, &[event])
            .unwrap();
        self.f.backend.release_event_v1(event).unwrap();
        readback
    }

    fn release_events(&mut self) {
        for event in self.events.drain(..) {
            self.f.backend.release_event_v1(event).unwrap();
        }
    }

    fn publish(&mut self, list: u64) {
        for _ in 0..128 {
            self.f.backend.progress_cooperative_copy(list).unwrap();
            if self
                .f
                .copy(list)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .contains(&Stage::Copy)
            {
                assert_eq!(self.f.copy(list).status(), BackendPollV1::Pending);
                assert!(
                    self.f
                        .backend
                        .compute_xgmi_children
                        .iter()
                        .enumerate()
                        .all(|(child, owner)| *owner == (child < 2).then_some(list))
                );
                return;
            }
        }
        panic!("exact retained list must publish");
    }

    fn drive(&mut self, stream: u64, target: u64) {
        for _ in 0..256 {
            self.f.backend.progress_stream_v1(stream).unwrap();
            if self.f.backend.poll_v1(target).unwrap() != BackendPollV1::Pending {
                return;
            }
        }
        panic!("final stream must settle retained frame/window graph");
    }

    fn expected_destination(&self) -> Vec<u8> {
        let mut expected = self.destination.clone();
        expected[7..54].copy_from_slice(&self.frame[1..48]);
        expected
    }

    fn assert_destination_untouched(&self) {
        let route = self.f.backend.allocations[&self.f.allocations[0][3]];
        let KfdRuntimeSdmaStorageV1::H2dReady(ready) =
            &self.f.backend.children[route.child].allocations[&route.local].sdma_storage
        else {
            panic!("unextracted destination must retain its original ready owner");
        };
        assert_eq!(ready.owner.scripted_owner_id(), Some(self.ids[2]));
        assert_eq!(ready.owner.scripted_bytes().unwrap(), self.destination);
    }
}

#[test]
fn segment_frame_peer_prequeued_late_and_completed_tail_preserves_full_readback_guards() {
    for ordered in [false, true] {
        for timing in 0..4 {
            let mut p = Forward::new(ordered, true);
            let tail = *p.lists.last().unwrap();
            match timing {
                1 => p.publish(p.lists[0]),
                2 => p.publish(tail),
                3 => {
                    for _ in 0..128 {
                        if p.f.backend.progress_cooperative_copy(tail).unwrap()
                            == BackendPollV1::Succeeded
                        {
                            break;
                        }
                    }
                    assert_eq!(p.f.copy(tail).status(), BackendPollV1::Succeeded);
                    assert!(p.f.copy(tail).is_quiescent());
                }
                _ => {}
            }
            let peer = p.submit().unwrap();
            assert!(p.f.copy(peer).compute_producer.is_none());
            assert!(p.f.copy(peer).frame_source.is_some());
            assert!(p.f.copy(peer).staging.is_empty());
            let readback = p.readback(peer);
            p.release_events();
            let generation = p.f.backend.cooperative_progress_generation;
            for id in [peer, readback] {
                assert_eq!(p.f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
                assert_eq!(
                    p.f.backend.wait_v1(id, Instant::now()).unwrap(),
                    BackendPollV1::Pending
                );
                assert!(p.f.backend.release_submission_v1(id).is_err());
            }
            assert_eq!(p.f.backend.cooperative_progress_generation, generation);
            assert!(p.f.backend.release_submission_v1(tail).is_err());
            p.drive(p.f.readback_stream, readback);
            for id in p.lists.iter().copied().chain([peer, readback]) {
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
            let expected = p.expected_destination();
            assert_eq!(
                owner(&p.f, p.f.allocations[0][3]),
                (p.ids[2], expected.as_slice())
            );
            let host = p.f.backend.allocations[&p.f.host.unwrap()];
            let KfdRuntimeSdmaStorageV1::Host(host) =
                &p.f.backend.children[host.child].allocations[&host.local].sdma_storage
            else {
                unreachable!()
            };
            assert_eq!(host.scripted_bytes().unwrap(), expected);
            assert!(p.f.backend.cooperative_dependency_retain_counts.is_empty());
            assert!(
                p.f.backend
                    .compute_xgmi_children
                    .iter()
                    .all(Option::is_none)
            );
            assert_eq!(p.f.backend.completed_compute_xgmi_copies, 0);
            p.f.clean();
        }
    }
}

#[test]
fn segment_frame_peer_cancellation_releases_only_its_own_dependency_and_never_publishes() {
    for late in [false, true] {
        let mut p = Forward::new(true, false);
        if late {
            p.publish(p.lists[0]);
        }
        let peer = p.submit().unwrap();
        p.release_events();
        assert_eq!(
            p.f.backend.cancel_v1(peer).unwrap(),
            BackendCancellationV1::Cancelled
        );
        assert!(p.f.copy(peer).is_quiescent());
        assert!(p.f.copy(peer).frame_source.is_none());
        assert!(
            p.f.copy(peer)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        let tail = *p.lists.last().unwrap();
        assert_eq!(p.f.copy(tail).status(), BackendPollV1::Pending);
        assert!(!p.f.backend.submission_retained_as_dependency(tail));
        p.drive(p.f.peer_stream, tail);
        assert_eq!(p.f.copy(tail).status(), BackendPollV1::Succeeded);
        p.assert_destination_untouched();
        p.f.clean();
    }
}

#[test]
fn segment_frame_peer_failed_parent_never_extracts_destination() {
    let mut p = Forward::new(false, false);
    let peer = p.submit().unwrap();
    p.release_events();
    assert_eq!(
        p.f.backend.cancel_v1(p.lists[0]).unwrap(),
        BackendCancellationV1::Cancelled
    );
    for _ in 0..8 {
        if p.f.backend.progress_cooperative_copy(peer).unwrap() != BackendPollV1::Pending {
            break;
        }
    }
    assert!(matches!(
        p.f.copy(peer).status(),
        BackendPollV1::Failed { .. }
    ));
    assert!(p.f.copy(peer).is_quiescent());
    assert!(
        p.f.copy(peer)
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

#[test]
fn segment_frame_peer_rejects_stale_events_unrelated_controls_and_pending_destinations() {
    for case in 0..5 {
        let mut p = Forward::new(true, false);
        let [source, mut destination] = p.regions();
        let mut events = vec![*p.events.last().unwrap()];
        match case {
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
                .peer_copy_v1(p.stream, source, destination, &events),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(p.f.backend.submissions.len(), before);
        assert!(!p.f.backend.terminal);
        if case == 4 {
            let route = p.f.backend.allocations[&destination.allocation];
            p.f.backend.children[route.child]
                .allocations
                .get_mut(&route.local)
                .unwrap()
                .sdma_initialized = true;
        }
        p.release_events();
        let tail = *p.lists.last().unwrap();
        p.drive(p.f.peer_stream, tail);
        p.f.clean();
    }
}

#[test]
fn segment_frame_peer_late_admission_and_progress_authenticate_both_parent_markers() {
    for at_admission in [false, true] {
        for child in 0..2 {
            let mut p = Forward::new(true, false);
            p.publish(p.lists[0]);
            let peer = (!at_admission).then(|| p.submit().unwrap());
            p.f.backend.compute_xgmi_children[child] = None;
            if at_admission {
                assert!(matches!(
                    p.submit(),
                    Err(RuntimeBackendFailureV1::Rejected(_))
                ));
            } else {
                assert!(matches!(
                    p.f.backend.progress_cooperative_copy(peer.unwrap()),
                    Err(RuntimeBackendFailureV1::Terminal(_))
                ));
                assert!(p.f.backend.terminal);
                assert!(
                    p.f.backend
                        .submission_retained_as_dependency(*p.lists.last().unwrap())
                );
                assert!(
                    p.f.copy(peer.unwrap())
                        .compute_xgmi
                        .as_ref()
                        .unwrap()
                        .trace_for_test_v1()
                        .is_empty()
                );
            }
            // The fixture deliberately retains corrupted native owners until exit.
        }
    }
}

#[test]
fn segment_frame_peer_window_rank_plan_and_owner_drift_fail_before_native_handoff() {
    for case in 0..5 {
        let mut p = Forward::new(false, false);
        let peer = p.submit().unwrap();
        match case {
            0 | 1 => {
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    p.f.backend.submissions.get_mut(&peer).unwrap()
                else {
                    unreachable!()
                };
                if case == 0 {
                    copy.source_region.byte_offset += 1;
                } else {
                    copy.dependency_depth = 1;
                }
            }
            2 => {
                let [source, destination] = regions(&p.f);
                let mut changed = descriptors();
                changed[0].source_offset += 1;
                let plan = Arc::new(
                    Gfx942ComputeXgmiSegmentsPlanV1::new(
                        BYTES as u64,
                        BYTES as u64,
                        source.byte_offset,
                        source.byte_len,
                        destination.byte_offset,
                        destination.byte_len,
                        &changed,
                    )
                    .unwrap(),
                );
                let RoutedSubmissionV1::CooperativeCopy(copy) =
                    p.f.backend.submissions.get_mut(&p.lists[0]).unwrap()
                else {
                    unreachable!()
                };
                copy.compute_xgmi
                    .as_mut()
                    .unwrap()
                    .replace_segments_for_test_v1(plan);
            }
            3 => {
                let route = p.f.backend.allocations[&p.f.allocations[0][3]];
                p.f.backend.children[route.child]
                    .allocations
                    .get_mut(&route.local)
                    .unwrap()
                    .alignment *= 2;
            }
            4 => {
                p.f.backend
                    .cooperative_dependency_retain_counts
                    .remove(&p.lists[0]);
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            p.f.backend.progress_cooperative_copy(peer),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(p.f.backend.terminal);
        assert!(
            p.f.copy(peer)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        assert!(p.f.copy(peer).frame_source.is_some());
    }
}

#[test]
fn segment_frame_peer_restored_ancestor_source_can_be_disposed_before_tail_progress() {
    let mut p = Forward::new(false, false);
    let peer = p.submit().unwrap();
    p.release_events();
    for _ in 0..128 {
        if p.f.backend.progress_cooperative_copy(p.lists[0]).unwrap() == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(p.f.copy(p.lists[0]).status(), BackendPollV1::Succeeded);
    assert_eq!(p.f.copy(peer).status(), BackendPollV1::Pending);
    assert!(
        p.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    let original = p.f.allocations[0][2];
    p.f.backend.release_allocation_v1(original).unwrap();
    assert!(p.f.backend.release_submission_v1(p.lists[0]).is_err());
    p.drive(p.stream, peer);
    assert_eq!(p.f.copy(peer).status(), BackendPollV1::Succeeded);
    assert_eq!(
        owner(&p.f, p.f.allocations[0][3]),
        (p.ids[2], p.expected_destination().as_slice())
    );
    p.f.clean_except(&[original]);
}

#[test]
fn segment_frame_peer_successful_restored_target_admits_full_readback_after_source_receipt_release()
{
    let mut p = Forward::with_readback_route(true, true, true);
    let peer = p.submit().unwrap();
    p.release_events();
    p.drive(p.stream, peer);
    assert_eq!(p.f.copy(peer).status(), BackendPollV1::Succeeded);
    assert!(p.f.copy(peer).is_quiescent());
    assert!(p.f.copy(peer).frame_source.is_none());
    assert!(
        p.f.backend
            .compute_xgmi_children
            .iter()
            .all(Option::is_none)
    );
    for id in p.lists.iter().copied().rev() {
        p.f.backend.release_submission_v1(id).unwrap();
    }
    // Ordinary initialized-source D2H is sufficient once the exact successful
    // scalar receipt has restored its owners; no historical frame is fabricated.
    let readback = p.readback(peer);
    assert!(matches!(
        p.f.backend.submissions[&readback],
        RoutedSubmissionV1::Native { .. }
    ));
    p.drive(p.f.readback_stream, readback);
    assert_eq!(
        p.f.backend.poll_v1(readback).unwrap(),
        BackendPollV1::Succeeded
    );
    let expected = p.expected_destination();
    let host = p.f.backend.allocations[&p.f.host.unwrap()];
    let KfdRuntimeSdmaStorageV1::Host(host) =
        &p.f.backend.children[host.child].allocations[&host.local].sdma_storage
    else {
        unreachable!()
    };
    assert_eq!(host.scripted_bytes().unwrap(), expected);
    p.f.clean();
}

#[test]
fn segment_frame_peer_native_failure_and_unwind_keep_exact_parent_and_whole_owner_custody() {
    for unwind in [false, true] {
        let mut p = Forward::new(false, false);
        let peer = p.submit().unwrap();
        p.release_events();
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            p.f.backend.submissions.get_mut(&peer).unwrap()
        else {
            unreachable!()
        };
        copy.compute_xgmi
            .as_mut()
            .unwrap()
            .inject_failure_for_test_v1(Stage::Copy, unwind);
        let mut failed = false;
        for _ in 0..128 {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                p.f.backend.progress_cooperative_copy(peer)
            }));
            match result {
                Err(_) => {
                    assert!(unwind);
                    failed = true;
                    break;
                }
                Ok(Err(RuntimeBackendFailureV1::Terminal(_))) => {
                    assert!(!unwind);
                    failed = true;
                    break;
                }
                Ok(Ok(BackendPollV1::Pending)) => {}
                other => panic!("unexpected fault disposition: {other:?}"),
            }
        }
        assert!(failed && p.f.backend.terminal);
        assert_eq!(p.f.copy(p.lists[0]).status(), BackendPollV1::Succeeded);
        assert!(p.f.copy(peer).frame_source.is_some());
        assert!(p.f.backend.submission_retained_as_dependency(p.lists[0]));
        assert_eq!(p.f.backend.compute_xgmi_children, vec![Some(peer); 2]);
        assert!(
            p.f.copy(peer)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .scripted_owners_for_test_v1()
                .iter()
                .all(Option::is_some)
        );
    }
}

#[test]
fn segment_frame_peer_corrupt_parent_poisoning_includes_held_pair_outside_successor_pair() {
    for case in 0..5 {
        let mut p = Forward::new(true, false);
        let (mut child, local_stream, [local]) =
            scripted_persistent_backend_with_steps_v1::<1>(BYTES, []);
        child.description.backend_device = 9;
        child.peer_visible_device_allocations = true;
        for record in child.allocations.values_mut() {
            record.device = 9;
        }
        for device in child.streams.values_mut() {
            *device = 9;
        }
        p.f.backend.children.push(child);
        p.f.backend.compute_xgmi_children.push(None);
        let allocation = p.f.backend.next_id().unwrap();
        p.f.backend
            .allocations
            .insert(allocation, RoutedHandleV1 { child: 2, local });
        let stream = p.f.backend.next_id().unwrap();
        p.f.backend.streams.insert(
            stream,
            RoutedHandleV1 {
                child: 2,
                local: local_stream,
            },
        );
        p.f.backend.compute_xgmi_routes.insert(
            (1, 2),
            Route::Scripted {
                failure: None,
                unwind: false,
                pending_samples: 2,
            },
        );
        let [source, mut destination] = p.regions();
        destination.allocation = allocation;
        let peer =
            p.f.backend
                .peer_copy_v1(stream, source, destination, &[*p.events.last().unwrap()])
                .unwrap();
        p.publish(p.lists[0]);
        assert_eq!(
            p.f.backend.compute_xgmi_children,
            vec![Some(p.lists[0]), Some(p.lists[0]), None]
        );
        p.release_events();
        if case == 0 {
            p.f.backend.compute_xgmi_children[0] = None;
        } else {
            let RoutedSubmissionV1::CooperativeCopy(copy) =
                p.f.backend.submissions.get_mut(&peer).unwrap()
            else {
                unreachable!()
            };
            match case {
                1 => copy.source.child = 0,
                2 => copy.destination.child = 0,
                3 => copy.source.child = usize::MAX,
                4 => copy.destination.child = usize::MAX,
                _ => unreachable!(),
            }
        }
        assert!(matches!(
            p.f.backend.progress_cooperative_copy(peer),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(p.f.backend.children.iter().all(|child| child.terminal));
        assert!(
            p.f.backend
                .submission_retained_as_dependency(*p.lists.last().unwrap())
        );
        assert!(p.f.copy(peer).frame_source.is_some());
        assert!(
            p.f.copy(peer)
                .compute_xgmi
                .as_ref()
                .unwrap()
                .trace_for_test_v1()
                .is_empty()
        );
        let oldest = p.f.copy(p.lists[0]).compute_xgmi.as_ref().unwrap();
        assert!(
            oldest
                .scripted_owners_for_test_v1()
                .iter()
                .all(Option::is_some)
        );
        assert!(!oldest.trace_for_test_v1().contains(&Stage::Restore));
        assert!(matches!(
            p.f.backend.children[2].allocations[&local].sdma_storage,
            KfdRuntimeSdmaStorageV1::H2dReady(_)
        ));
    }
}
