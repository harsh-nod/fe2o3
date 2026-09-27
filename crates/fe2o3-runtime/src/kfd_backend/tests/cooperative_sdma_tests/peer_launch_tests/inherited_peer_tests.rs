use super::*;

fn fixture_for_launches(count: usize) -> ManuallyDrop<Fixture> {
    fixture_with_reporter(count, false)
}

fn fixture_with_reporter(count: usize, reporter: bool) -> ManuallyDrop<Fixture> {
    let mut steps = phase_steps(false, 0, 2048, true);
    for _ in 0..count {
        steps.push(ScriptedSdmaStepV1::PromoteInitializedStorage(
            ScriptedFailureModeV1::Success,
        ));
    }
    if reporter {
        steps.push(ScriptedSdmaStepV1::Recycle(
            ScriptedRecycleOutcomeV1::Success,
        ));
    }
    let mut f = ManuallyDrop::new(fixture(4096, phase_steps(true, 0, 2048, true), steps));
    f.source.byte_len = 2048;
    f.destination.byte_len = 2048;
    f
}

fn dependency(f: &mut Fixture, stream: u64, id: u64) -> BackendLaunchProducerV1 {
    BackendLaunchProducerV1 {
        event: f.backend.record_event_v1(stream, id).unwrap(),
        producer_submission: id,
    }
}

fn publish_peer(f: &mut Fixture, peer: u64) {
    for _ in 0..96 {
        f.backend.progress_retained_directed_peer_v1(peer).unwrap();
        if !f.backend.children[1].published_sdma_submissions.is_empty() {
            return;
        }
    }
    panic!("peer destination did not publish");
}

#[test]
fn inherited_peer_public_native_intermediate_keeps_independent_custody() {
    for published in [false, true] {
        for explicit in [false, true] {
            let mut f = fixture_for_launches(2);
            let p = directed(&mut f);
            if published {
                publish_peer(&mut f, p);
            }
            let peer_stream = f.stream;
            let p_dep = dependency(&mut f, peer_stream, p);
            let b_stream = f.backend.create_stream_v1(8).unwrap();
            let c_stream = if explicit {
                f.backend.create_stream_v1(8).unwrap()
            } else {
                b_stream
            };
            let (module, kernel) = load(&mut f);
            let b = launch(&mut f, b_stream, kernel, &[p_dep]);
            let b_dep = dependency(&mut f, b_stream, b);
            let before = f.steps();
            let c = launch(
                &mut f,
                c_stream,
                kernel,
                if explicit {
                    core::slice::from_ref(&b_dep)
                } else {
                    &[]
                },
            );
            assert_eq!(f.steps(), before);
            let b_route = routed(&f, b);
            let c_route = routed(&f, c);
            let pending = &f.backend.children[1].pending_compute[&c_route.local];
            assert_eq!(
                pending.explicit_success_dependencies.as_ref(),
                if explicit {
                    core::slice::from_ref(&b_route.local)
                } else {
                    &[]
                }
            );
            assert_eq!(
                pending.ordered_predecessor,
                (!explicit).then_some(b_route.local)
            );
            assert_eq!(pending.dependency_depth, if explicit { 3 } else { 1 });
            f.backend.release_event_v1(p_dep.event).unwrap();
            f.backend.release_event_v1(b_dep.event).unwrap();
            f.assert_observation_only(c);
            assert_eq!(
                f.backend.drain_v1(c, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(f.steps(), before);
            assert_eq!(
                f.backend
                    .drain_v1(c, Instant::now() + Duration::from_secs(1))
                    .unwrap(),
                BackendPollV1::Succeeded
            );
            assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Succeeded);
            assert!(f.backend.peer_launch_retains.is_empty());
            assert!(f.bytes(false)[..2048].iter().all(|byte| *byte == 0x53));
            assert!(f.bytes(false)[2048..].iter().all(|byte| *byte == 0x17));
            for id in [c, b] {
                f.backend.release_submission_v1(id).unwrap();
            }
            f.backend.unload_module_v1(module).unwrap();
            if explicit {
                f.backend.destroy_stream_v1(c_stream).unwrap();
            }
            f.backend.destroy_stream_v1(b_stream).unwrap();
            ManuallyDrop::into_inner(f).clean(&[p]);
        }
    }
}

#[test]
fn inherited_peer_public_cancellation_preserves_independent_custody() {
    for explicit in [false, true] {
        let mut f = fixture_for_launches(0);
        let p = directed(&mut f);
        publish_peer(&mut f, p);
        let peer_stream = f.stream;
        let p_dep = dependency(&mut f, peer_stream, p);
        let b_stream = f.backend.create_stream_v1(8).unwrap();
        let c_stream = if explicit {
            f.backend.create_stream_v1(8).unwrap()
        } else {
            b_stream
        };
        let (module, kernel) = load(&mut f);
        let b = launch(&mut f, b_stream, kernel, &[p_dep]);
        let b_dep = dependency(&mut f, b_stream, b);
        let c = launch(
            &mut f,
            c_stream,
            kernel,
            if explicit {
                core::slice::from_ref(&b_dep)
            } else {
                &[]
            },
        );
        let before = f.steps();
        assert_eq!(
            f.backend.cancel_v1(b).unwrap(),
            if explicit {
                crate::BackendCancellationV1::Cancelled
            } else {
                crate::BackendCancellationV1::TooLate
            }
        );
        f.backend.release_event_v1(b_dep.event).unwrap();
        f.backend.release_event_v1(p_dep.event).unwrap();
        assert!(f.backend.peer_launch_retains.retains(p));
        assert_eq!(f.steps(), before);
        if explicit {
            assert!(matches!(
                f.backend.poll_v1(c).unwrap(),
                BackendPollV1::Failed { .. }
            ));
            assert_eq!(
                f.steps(),
                before,
                "failed consumer must not drive its unrelated input"
            );
            assert!(f.backend.peer_launch_retains.is_empty());
        } else {
            f.assert_observation_only(c);
            assert_eq!(f.steps(), before);
            assert_eq!(
                f.backend.cancel_v1(c).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            assert!(f.backend.peer_launch_retains.retains(p));
            assert_eq!(
                f.backend.cancel_v1(b).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
        }
        assert_eq!(f.steps(), before);
        for _ in 0..96 {
            if f.backend.progress_retained_directed_peer_v1(p).unwrap() == BackendPollV1::Succeeded
            {
                break;
            }
        }
        for id in [c, b] {
            f.backend.release_submission_v1(id).unwrap();
        }
        assert!(f.backend.peer_launch_retains.is_empty());
        f.backend.unload_module_v1(module).unwrap();
        if explicit {
            f.backend.destroy_stream_v1(c_stream).unwrap();
        }
        f.backend.destroy_stream_v1(b_stream).unwrap();
        ManuallyDrop::into_inner(f).clean(&[p]);
    }
}

#[test]
fn inherited_peer_public_failed_root_does_not_discard_live_ancestor() {
    for late_admission in [false, true] {
        let mut f = fixture_with_reporter(1, late_admission);
        let x_target = synthetic_fanout_destination(&mut f, 4096);
        let q_target = synthetic_fanout_destination(&mut f, 4096);
        let mut x_route = directed_route(&f);
        x_route.stream = x_target.0;
        x_route.destination.allocation = x_target.1;
        let x = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: x_route,
                dependencies: &[],
            })
            .unwrap();
        let p = directed(&mut f);
        publish_peer(&mut f, p);
        let peer_stream = f.stream;
        let p_dep = dependency(&mut f, peer_stream, p);
        let x_dep = dependency(&mut f, x_target.0, x);
        let mut q_route = directed_route(&f);
        q_route.stream = q_target.0;
        q_route.destination.allocation = q_target.1;
        let q = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: q_route,
                dependencies: &[
                    BackendDirectedPeerDependencyV1 {
                        event: x_dep.event,
                        producer_submission: x,
                    },
                    BackendDirectedPeerDependencyV1 {
                        event: p_dep.event,
                        producer_submission: p,
                    },
                ],
            })
            .unwrap();
        let q_dep = dependency(&mut f, q_target.0, q);
        let stream = f.backend.create_stream_v1(8).unwrap();
        let (module, kernel) = load(&mut f);
        let b = launch(&mut f, stream, kernel, &[q_dep]);
        let c = (!late_admission).then(|| launch(&mut f, stream, kernel, &[]));
        let reporter = if late_admission {
            let extra = synthetic_fanout_destination(&mut f, 4096);
            let local = f.backend.allocations[&extra.1].local;
            let host = f.backend.children[1]
                .scripted_sdma
                .as_ref()
                .unwrap()
                .test_host_owner(4096);
            let record = f.backend.children[1].allocations.get_mut(&local).unwrap();
            record.sdma_storage = KfdRuntimeSdmaStorageV1::Host(host);
            record.sdma_backed = true;
            record.sdma_initialized = true;
            let b_dep = dependency(&mut f, stream, b);
            let destination = f.destination;
            f.destination.allocation = extra.1;
            let reporter = launch(&mut f, extra.0, kernel, &[b_dep]);
            f.destination = destination;
            f.backend.release_event_v1(b_dep.event).unwrap();
            Some((reporter, extra))
        } else {
            None
        };
        for event in [x_dep.event, p_dep.event, q_dep.event] {
            f.backend.release_event_v1(event).unwrap();
        }
        assert_eq!(
            f.backend.cancel_v1(x).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        assert!(matches!(
            f.backend.progress_retained_directed_peer_v1(q).unwrap(),
            BackendPollV1::Failed { .. }
        ));
        if let Some((reporter, _)) = reporter {
            assert!(matches!(
                f.backend.poll_v1(reporter).unwrap(),
                BackendPollV1::Failed { .. }
            ));
            let route = routed(&f, b);
            assert!(
                !f.backend.children[1]
                    .pending_compute
                    .contains_key(&route.local)
            );
        }
        let c = c.unwrap_or_else(|| launch(&mut f, stream, kernel, &[]));
        assert!(matches!(
            f.backend.poll_v1(b).unwrap(),
            BackendPollV1::Failed { .. }
        ));
        assert!(f.backend.peer_launch_retains.retains(p));
        let before = f.steps();
        f.assert_observation_only(c);
        assert_eq!(f.steps(), before);
        assert_eq!(
            f.backend
                .drain_v1(c, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        assert!(f.backend.peer_launch_retains.is_empty());
        assert!(f.bytes(false)[..2048].iter().all(|byte| *byte == 0x53));
        assert!(f.bytes(false)[2048..].iter().all(|byte| *byte == 0x17));
        if let Some((reporter, (stream, allocation))) = reporter {
            f.backend.release_submission_v1(reporter).unwrap();
            let local = f.backend.allocations[&allocation].local;
            f.backend.children[1]
                .allocations
                .get_mut(&local)
                .unwrap()
                .sdma_backed = false;
            f.backend.release_allocation_v1(allocation).unwrap();
            f.backend.destroy_stream_v1(stream).unwrap();
        }
        for id in [c, b, q, x] {
            f.backend.release_submission_v1(id).unwrap();
        }
        f.backend.unload_module_v1(module).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
        for (stream, allocation) in [x_target, q_target] {
            f.backend.release_allocation_v1(allocation).unwrap();
            f.backend.destroy_stream_v1(stream).unwrap();
        }
        ManuallyDrop::into_inner(f).clean(&[p]);
    }
}

#[test]
fn inherited_peer_public_admission_rejects_unrelated_and_corrupt_native_prefixes() {
    for corruption in 0..6 {
        let mut f = fixture_for_launches(0);
        let p = directed(&mut f);
        let peer_stream = f.stream;
        let p_dep = dependency(&mut f, peer_stream, p);
        let stream = f.backend.create_stream_v1(8).unwrap();
        let unrelated = f.backend.create_stream_v1(8).unwrap();
        let (module, kernel) = load(&mut f);
        let b = launch(&mut f, stream, kernel, &[p_dep]);
        let route = routed(&f, b);
        let gate = f.backend.children[1].pending_compute[&route.local].peer_gate;
        let before = f.steps();
        let count = f.backend.children[1].pending_compute.len();
        assert!(
            matches!(try_launch(&mut f, unrelated, kernel, &[]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        let ledger = (corruption >= 4).then(|| std::mem::take(&mut f.backend.peer_launch_retains));
        match corruption {
            0 => {
                f.backend.children[1]
                    .pending_compute
                    .get_mut(&route.local)
                    .unwrap()
                    .peer_gate = None
            }
            1 => {
                f.backend.children[1]
                    .pending_compute
                    .get_mut(&route.local)
                    .unwrap()
                    .peer_gate = Some(PeerComputeGateV1::waiting(b + 100, route.local, true))
            }
            2 => {
                f.backend.submissions.insert(
                    b,
                    RoutedSubmissionV1::Native {
                        route,
                        stream: unrelated,
                    },
                );
            }
            3 => {
                f.backend.submissions.insert(
                    b,
                    RoutedSubmissionV1::Native {
                        route: RoutedHandleV1 {
                            local: route.local + 100,
                            ..route
                        },
                        stream,
                    },
                );
            }
            _ => {}
        }
        if corruption == 5 {
            assert!(matches!(
                f.backend.poll_v1(b),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        } else {
            assert!(matches!(
                try_launch(&mut f, stream, kernel, &[]),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert_eq!(f.steps(), before);
        assert_eq!(f.backend.children[1].pending_compute.len(), count);
        if corruption < 4 {
            assert!(f.backend.peer_launch_retains.retains(p));
        }
        // Restore deliberately corrupted CPU-only metadata, not native recovery.
        f.backend.children[1]
            .pending_compute
            .get_mut(&route.local)
            .unwrap()
            .peer_gate = gate;
        f.backend
            .submissions
            .insert(b, RoutedSubmissionV1::Native { route, stream });
        f.backend.terminal = false;
        if let Some(ledger) = ledger {
            f.backend.peer_launch_retains = ledger;
        }
        assert_eq!(
            f.backend.cancel_v1(b).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        f.backend.release_event_v1(p_dep.event).unwrap();
        f.backend.release_submission_v1(b).unwrap();
        for _ in 0..96 {
            if f.backend.progress_retained_directed_peer_v1(p).unwrap() == BackendPollV1::Succeeded
            {
                break;
            }
        }
        f.backend.unload_module_v1(module).unwrap();
        f.backend.destroy_stream_v1(stream).unwrap();
        f.backend.destroy_stream_v1(unrelated).unwrap();
        ManuallyDrop::into_inner(f).clean(&[p]);
    }
}
