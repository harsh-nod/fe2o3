use super::*;
mod peer_compute_access_tests;
mod peer_launch_tests;
use crate::{
    BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    BackendDirectedScalarProgressV1, RuntimeDirectedScalarPeerCopyBackendV1,
};

fn directed_route(fixture: &Fixture) -> BackendDirectedPeerRouteV1 {
    let source = fixture.backend.allocations[&fixture.source.allocation];
    let destination = fixture.backend.allocations[&fixture.destination.allocation];
    BackendDirectedPeerRouteV1 {
        stream: fixture.stream,
        source_device: fixture.backend.children[source.child]
            .description
            .backend_device,
        destination_device: fixture.backend.children[destination.child]
            .description
            .backend_device,
        source: fixture.source,
        destination: fixture.destination,
    }
}

#[test]
fn directed_router_scripted_sdma_progress_keeps_one_selected_window_and_no_waits() {
    let mut f = fixture(
        16,
        phase_steps(true, 2, 4, true),
        phase_steps(false, 7, 4, true),
    );
    f.source.byte_offset = 2;
    f.destination.byte_offset = 7;
    f.source.byte_len = 4;
    f.destination.byte_len = 4;
    let route = directed_route(&f);
    let id = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route,
            dependencies: &[],
        })
        .unwrap();
    let mut observations = 0;
    for _ in 0..64 {
        let before: usize = f.steps().iter().sum();
        let status = f
            .backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: id,
                route,
                producer_submissions: &[],
            })
            .unwrap();
        let after: usize = f.steps().iter().sum();
        assert!(
            before - after <= 2,
            "one selected allocation, publication or poll/retire step"
        );
        if status == BackendPollV1::Succeeded {
            break;
        }
        if f.backend
            .children
            .iter()
            .any(|child| !child.published_sdma_submissions.is_empty())
        {
            observations += 1;
            assert_eq!(
                f.backend.cancel_v1(id).unwrap(),
                crate::BackendCancellationV1::TooLate
            );
        }
        f.assert_observation_only(id);
    }
    assert!(observations >= 2);
    assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
    let mut expected = [0x17; 16];
    expected[7..11].fill(0x53);
    assert_eq!(f.bytes(false), expected);
    f.clean(&[id]);
}

#[test]
fn directed_router_native_read_fanout_drives_only_exact_private_blocker() {
    let mut source_steps = phase_steps(true, 0, 8, true);
    let mut first_readback = source_steps.split_off(source_steps.len() - 2);
    let mut second_phase = phase_steps(true, 0, 8, false);
    second_phase.insert(3, first_readback.remove(0));
    source_steps.extend(second_phase);
    source_steps.extend(first_readback);
    let mut destination_steps = allocation_steps(8);
    destination_steps.push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    destination_steps.extend(phase_steps(false, 0, 8, false));
    let mut f = fixture(8, source_steps, destination_steps);
    let extra = synthetic_fanout_destination(&mut f, 8);
    let a = directed_route(&f);
    let first = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: a,
            dependencies: &[],
        })
        .unwrap();
    for _ in 0..16 {
        f.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: first,
                route: a,
                producer_submissions: &[],
            })
            .unwrap();
        if !f.backend.children[0].published_sdma_submissions.is_empty() {
            break;
        }
    }
    assert!(!f.backend.children[0].published_sdma_submissions.is_empty());
    let first_private = *f.backend.children[0].active_sdma.keys().next().unwrap();
    let b = BackendDirectedPeerRouteV1 {
        stream: extra.0,
        destination: BackendMemoryRegionV1 {
            allocation: extra.1,
            ..a.destination
        },
        ..a
    };
    let second = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: b,
            dependencies: &[],
        })
        .unwrap();
    let mut independent_readback = false;
    for _ in 0..128 {
        let before: usize = f.steps().iter().sum();
        let status = f
            .backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: second,
                route: b,
                producer_submissions: &[],
            })
            .unwrap();
        assert!(before - f.steps().iter().sum::<usize>() <= 2);
        if !independent_readback
            && !f.backend.children[0].published_sdma_submissions.is_empty()
            && !f.backend.children[0]
                .active_sdma
                .contains_key(&first_private)
        {
            let before = f.steps();
            f.backend
                .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                    submission: first,
                    route: a,
                    producer_submissions: &[],
                })
                .unwrap();
            assert_eq!(
                before[0] - f.steps()[0],
                1,
                "first readback must not poll the second DMA"
            );
            independent_readback = true;
        }
        if status == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    assert!(independent_readback);
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Pending);
    let mut bytes = [0; 8];
    f.backend
        .read_allocation_v1(extra.1, 0, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [0x53; 8]);
    for _ in 0..128 {
        if f.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: first,
                route: a,
                producer_submissions: &[],
            })
            .unwrap()
            == BackendPollV1::Succeeded
        {
            break;
        }
    }
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
    f.backend.release_submission_v1(second).unwrap();
    f.backend.release_allocation_v1(extra.1).unwrap();
    f.backend.destroy_stream_v1(extra.0).unwrap();
    f.clean(&[first]);
}

fn synthetic_fanout_destination(f: &mut Fixture, byte_len: u64) -> (u64, u64) {
    // This extra destination is CPU-backed; only the source contention is native-scripted.
    f.backend.children[1].native_available = false;
    let allocation = f
        .backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
        .unwrap();
    f.backend.children[1].native_available = true;
    let stream = f.backend.create_stream_v1(8).unwrap();
    (stream, allocation)
}

#[test]
fn directed_router_corrupt_private_dma_blocker_is_terminal_before_action() {
    for scratch in [false, true] {
        let mut f = fixture(8, phase_steps(true, 0, 8, false), vec![]);
        let extra = synthetic_fanout_destination(&mut f, 8);
        let a = directed_route(&f);
        let first = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: a,
                dependencies: &[],
            })
            .unwrap();
        for _ in 0..16 {
            f.backend
                .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                    submission: first,
                    route: a,
                    producer_submissions: &[],
                })
                .unwrap();
            if !f.backend.children[0].published_sdma_submissions.is_empty() {
                break;
            }
        }
        let b = BackendDirectedPeerRouteV1 {
            stream: extra.0,
            destination: BackendMemoryRegionV1 {
                allocation: extra.1,
                ..a.destination
            },
            ..a
        };
        let second = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: b,
                dependencies: &[],
            })
            .unwrap();
        let request = BackendDirectedScalarProgressV1 {
            submission: second,
            route: b,
            producer_submissions: &[],
        };
        f.backend
            .progress_directed_scalar_peer_copy_v1(request)
            .unwrap();
        let active = f.backend.children[0]
            .active_sdma
            .values_mut()
            .next()
            .unwrap();
        if scratch {
            active.destination = u64::MAX;
        } else {
            active.source_offset += 1;
        }
        let before = f.steps();
        assert!(matches!(
            f.backend.progress_directed_scalar_peer_copy_v1(request),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(f.steps(), before);
        assert!(f.backend.terminal);
        assert!(!f.backend.children[0].active_sdma.is_empty());
        for child in &mut f.backend.children {
            assert!(child.admitted_device.is_none() && child.queue.is_none());
            disarm_scripted_drop_after_inspection_v1(child);
        }
    }
}

#[test]
fn directed_router_native_reconciliation_fanout_drives_pinned_owner() {
    let mut source_steps = allocation_steps(4);
    source_steps.extend([
        ScriptedSdmaStepV1::Write {
            offset: 7,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Read {
            offset: 7,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Read {
            offset: 7,
            byte_len: 4,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let mut destination_steps = allocation_steps(4);
    destination_steps.push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    destination_steps.extend(phase_steps(false, 14, 4, false));
    let mut f = native_host_fixture(source_steps, destination_steps);
    let extra = synthetic_fanout_destination(&mut f, 24);
    attach_native_dirty(&mut f, 7, vec![0x62; 4]);
    let a = directed_route(&f);
    let first = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: a,
            dependencies: &[],
        })
        .unwrap();
    for _ in 0..16 {
        f.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: first,
                route: a,
                producer_submissions: &[],
            })
            .unwrap();
        if f.backend.children[0]
            .native_reconciliations
            .iter()
            .any(Option::is_some)
        {
            break;
        }
    }
    assert!(
        f.backend.children[0]
            .native_reconciliations
            .iter()
            .any(Option::is_some)
    );
    let b = BackendDirectedPeerRouteV1 {
        stream: extra.0,
        destination: BackendMemoryRegionV1 {
            allocation: extra.1,
            ..a.destination
        },
        ..a
    };
    let second = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: b,
            dependencies: &[],
        })
        .unwrap();
    for _ in 0..128 {
        if f.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: second,
                route: b,
                producer_submissions: &[],
            })
            .unwrap()
            == BackendPollV1::Succeeded
        {
            break;
        }
    }
    assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Pending);
    let mut bytes = [0; 4];
    f.backend
        .read_allocation_v1(extra.1, 14, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [0x62; 4]);
    for _ in 0..128 {
        if f.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: first,
                route: a,
                producer_submissions: &[],
            })
            .unwrap()
            == BackendPollV1::Succeeded
        {
            break;
        }
    }
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
    f.backend.release_submission_v1(second).unwrap();
    f.backend.release_allocation_v1(extra.1).unwrap();
    f.backend.destroy_stream_v1(extra.0).unwrap();
    forget_scripted_recycled(&mut f);
    f.clean(&[first]);
}

#[test]
fn directed_router_orphan_reconciliation_pin_is_terminal_before_action() {
    let mut f = native_host_fixture(allocation_steps(4), vec![]);
    let extra = synthetic_fanout_destination(&mut f, 24);
    attach_native_dirty(&mut f, 7, vec![0x62; 4]);
    let a = directed_route(&f);
    let first = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: a,
            dependencies: &[],
        })
        .unwrap();
    for _ in 0..16 {
        f.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: first,
                route: a,
                producer_submissions: &[],
            })
            .unwrap();
        if f.backend.children[0]
            .native_reconciliations
            .iter()
            .any(Option::is_some)
        {
            break;
        }
    }
    let b = BackendDirectedPeerRouteV1 {
        stream: extra.0,
        destination: BackendMemoryRegionV1 {
            allocation: extra.1,
            ..a.destination
        },
        ..a
    };
    let second = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: b,
            dependencies: &[],
        })
        .unwrap();
    let request = BackendDirectedScalarProgressV1 {
        submission: second,
        route: b,
        producer_submissions: &[],
    };
    f.backend
        .progress_directed_scalar_peer_copy_v1(request)
        .unwrap();
    let RoutedSubmissionV1::CooperativeCopy(copy) = f.backend.submissions.get_mut(&first).unwrap()
    else {
        panic!()
    };
    let retained_leaf = copy.sdma_leaf.take();
    let before = f.steps();
    assert!(matches!(
        f.backend.progress_directed_scalar_peer_copy_v1(request),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert_eq!(f.steps(), before);
    assert!(
        f.backend.children[0]
            .native_reconciliations
            .iter()
            .any(Option::is_some)
    );
    let RoutedSubmissionV1::CooperativeCopy(copy) = f.backend.submissions.get_mut(&first).unwrap()
    else {
        panic!()
    };
    copy.sdma_leaf = retained_leaf;
    for child in &mut f.backend.children {
        assert!(child.admitted_device.is_none() && child.queue.is_none());
        disarm_scripted_drop_after_inspection_v1(child);
    }
}

#[test]
fn directed_router_read_and_write_select_write_reconciliation_on_shared_lane() {
    for requester_writes in [false, true] {
        let mut steps = allocation_steps(4);
        steps.extend([
            ScriptedSdmaStepV1::Write {
                offset: 7,
                byte_len: 4,
            },
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let mut f = native_host_fixture(steps, vec![]);
        let (input_stream, input) = synthetic_fanout_destination(&mut f, 24);
        f.backend.children[0].native_available = false;
        let other = f
            .backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 24, 8)
            .unwrap();
        f.backend.children[0].native_available = true;
        attach_native_dirty(&mut f, 7, vec![0x62; 4]);
        let other_local = f.backend.allocations[&other].local;
        let child = &mut f.backend.children[0];
        let mut descriptor = child.recycled_dispatch.as_ref().unwrap().descriptors[0];
        descriptor.allocation = other_local;
        child
            .recycled_dispatch
            .as_mut()
            .unwrap()
            .descriptors
            .push(descriptor);
        child
            .allocations
            .get_mut(&other_local)
            .unwrap()
            .native_dirty
            .push(NativeDirtyExtentV1 {
                compute_lane: 0,
                data_index: 1,
                allocation_offset: 7,
                data_offset: 2,
                byte_len: 4,
            });
        child.native_dirty_extents += 1;
        child
            .scripted_native_reconcile
            .as_mut()
            .unwrap()
            .data
            .push(vec![0x33; 6]);
        let stream = f.backend.create_stream_v1(7).unwrap();
        let a = BackendDirectedPeerRouteV1 {
            stream,
            source_device: 8,
            destination_device: 7,
            source: BackendMemoryRegionV1 {
                allocation: input,
                byte_offset: 0,
                ..f.source
            },
            destination: BackendMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..f.source
            },
        };
        let first = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: a,
                dependencies: &[],
            })
            .unwrap();
        for _ in 0..16 {
            f.backend
                .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                    submission: first,
                    route: a,
                    producer_submissions: &[],
                })
                .unwrap();
            if f.backend.children[0]
                .native_reconciliations
                .iter()
                .any(Option::is_some)
            {
                break;
            }
        }
        assert!(
            f.backend.children[0]
                .native_reconciliations
                .iter()
                .any(Option::is_some)
        );
        let second_stream = f.backend.create_stream_v1(7).unwrap();
        let b = if requester_writes {
            BackendDirectedPeerRouteV1 {
                stream: second_stream,
                destination: BackendMemoryRegionV1 {
                    allocation: other,
                    ..a.destination
                },
                ..a
            }
        } else {
            BackendDirectedPeerRouteV1 {
                source: BackendMemoryRegionV1 {
                    allocation: other,
                    ..f.source
                },
                ..directed_route(&f)
            }
        };
        let second = f
            .backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: b,
                dependencies: &[],
            })
            .unwrap();
        let request = BackendDirectedScalarProgressV1 {
            submission: second,
            route: b,
            producer_submissions: &[],
        };
        f.backend
            .progress_directed_scalar_peer_copy_v1(request)
            .unwrap();
        assert_eq!(
            {
                if requester_writes {
                    f.backend
                        .progress_directed_scalar_peer_copy_v1(request)
                        .unwrap();
                }
                f.backend.directed_private_blocker_v1(second).unwrap()
            },
            Some(first)
        );
        assert_eq!(
            f.backend
                .progress_directed_scalar_peer_copy_v1(request)
                .unwrap(),
            BackendPollV1::Pending
        );
        assert!(
            f.backend.children[0]
                .native_reconciliations
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(&f.bytes(true)[7..11], &[0x62; 4]);
        for id in [second, first] {
            assert_eq!(
                f.backend.cancel_v1(id).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
            f.backend.release_submission_v1(id).unwrap();
        }
        forget_scripted_recycled(&mut f);
        for allocation in [input, other] {
            f.backend.release_allocation_v1(allocation).unwrap();
        }
        for stream in [input_stream, stream, second_stream] {
            f.backend.destroy_stream_v1(stream).unwrap();
        }
        f.clean(&[]);
    }
}

#[test]
fn directed_router_ancestor_cleanup_error_does_not_settle_requested_descendants() {
    let mut steps = phase_steps(true, 0, 8, false);
    steps.pop();
    steps.extend([
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Recovered),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let mut f = fixture(16, steps, vec![]);
    f.source.byte_len = 8;
    f.destination.byte_len = 8;
    let a = directed_route(&f);
    let first = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: a,
            dependencies: &[],
        })
        .unwrap();
    let event = f.backend.record_event_v1(a.stream, first).unwrap();
    let reverse_stream = f.backend.create_stream_v1(7).unwrap();
    let b = BackendDirectedPeerRouteV1 {
        stream: reverse_stream,
        source_device: 8,
        destination_device: 7,
        source: BackendMemoryRegionV1 {
            access: RuntimeAccessV1::Read,
            ..a.destination
        },
        destination: BackendMemoryRegionV1 {
            access: RuntimeAccessV1::Write,
            ..a.source
        },
    };
    let second = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: b,
            dependencies: &[BackendDirectedPeerDependencyV1 {
                event,
                producer_submission: first,
            }],
        })
        .unwrap();
    f.backend.release_event_v1(event).unwrap();
    let event = f.backend.record_event_v1(b.stream, second).unwrap();
    let third = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route: a,
            dependencies: &[BackendDirectedPeerDependencyV1 {
                event,
                producer_submission: second,
            }],
        })
        .unwrap();
    f.backend.release_event_v1(event).unwrap();
    let request = BackendDirectedScalarProgressV1 {
        submission: third,
        route: a,
        producer_submissions: &[second],
    };
    for _ in 0..32 {
        assert_eq!(
            f.backend
                .progress_directed_scalar_peer_copy_v1(request)
                .unwrap(),
            BackendPollV1::Pending
        );
        if matches!(
            f.backend.poll_v1(first).unwrap(),
            BackendPollV1::Failed { .. }
        ) {
            break;
        }
    }
    assert!(matches!(
        f.backend.poll_v1(first).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Pending);
    assert_eq!(f.backend.poll_v1(third).unwrap(), BackendPollV1::Pending);
    assert_eq!(
        f.backend
            .progress_directed_scalar_peer_copy_v1(request)
            .unwrap(),
        BackendPollV1::Pending
    );
    assert!(matches!(
        f.backend.poll_v1(second).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert!(matches!(
        f.backend
            .progress_directed_scalar_peer_copy_v1(request)
            .unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.bytes(false), [0x17; 16]);
    f.backend.release_submission_v1(third).unwrap();
    f.backend.release_submission_v1(second).unwrap();
    f.backend.destroy_stream_v1(reverse_stream).unwrap();
    f.clean(&[first]);
}

#[test]
fn directed_router_clean_host_source_panic_seals_router_and_retains_copy_custody() {
    let mut f = native_host_fixture(
        vec![ScriptedSdmaStepV1::ReadFault {
            offset: 7,
            byte_len: 4,
            panic: true,
        }],
        vec![],
    );
    let route = directed_route(&f);
    let id = f
        .backend
        .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
            route,
            dependencies: &[],
        })
        .unwrap();
    let request = BackendDirectedScalarProgressV1 {
        submission: id,
        route,
        producer_submissions: &[],
    };
    assert_eq!(
        f.backend
            .progress_directed_scalar_peer_copy_v1(request)
            .unwrap(),
        BackendPollV1::Pending
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.backend.progress_directed_scalar_peer_copy_v1(request)
    }));
    assert!(result.is_err());
    assert!(f.backend.terminal && f.backend.children[0].terminal);
    assert_eq!(f.backend.cooperative_staging_bytes, 8);
    assert!(matches!(
        f.backend.poll_v1(id),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    for child in &mut f.backend.children {
        assert!(child.admitted_device.is_none() && child.queue.is_none());
        assert_eq!(child.scripted_sdma.as_ref().unwrap().unexpected_drops(), 0);
        disarm_scripted_drop_after_inspection_v1(child);
    }
}

struct Fixture {
    backend: KfdMultiDeviceRuntimeBackendV1,
    stream: u64,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
}

fn allocation_steps(byte_len: usize) -> Vec<ScriptedSdmaStepV1> {
    vec![
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len,
        },
    ]
}

fn completed() -> ScriptedSdmaStepV1 {
    ScriptedSdmaStepV1::Poll(ScriptedExecutionOutcomeV1::Completed {
        direction: None,
        copy_bytes: None,
    })
}

fn attach_native_dirty(fixture: &mut Fixture, offset: usize, data: Vec<u8>) {
    let route = fixture.backend.allocations[&fixture.source.allocation];
    let child = &mut fixture.backend.children[route.child];
    let record = child.allocations.get_mut(&route.local).unwrap();
    let len = data.len() as u64;
    record.native_dirty.push(NativeDirtyExtentV1 {
        compute_lane: 0,
        data_index: 0,
        allocation_offset: offset,
        data_offset: 2,
        byte_len: len,
    });
    child.native_dirty_extents += 1;
    child.recycled_dispatch = Some(RecycledDispatchV1 {
        kernel: 99,
        dispatch_shape_sha256: [0; 32],
        descriptors: vec![ResidentDataDescriptorV1 {
            allocation: route.local,
            kind: RuntimeMemoryKindV1::HostVisible,
            alignment: 8,
            allocation_offset: (offset - 2) as u64,
            byte_len: len + 2,
            host_content_sha256: None,
            device_may_have_modified: true,
        }],
    });
    let mut bytes = vec![0xEE; 2];
    bytes.extend(data);
    child.scripted_native_reconcile = Some(native_reconcile::ScriptedNativeReconcileV1 {
        generation: 41,
        data: vec![bytes],
        reads: vec![],
    });
}

fn native_host_fixture(
    source_steps: Vec<ScriptedSdmaStepV1>,
    destination_steps: Vec<ScriptedSdmaStepV1>,
) -> Fixture {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(8).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 24, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, 24, 8)
        .unwrap();
    install_scripted_devices(
        &mut backend,
        source,
        destination,
        24,
        source_steps,
        destination_steps,
    );
    let (mut source, mut destination) = scripted_copy_regions_v1(source, destination, 4);
    source.byte_offset = 7;
    destination.byte_offset = 14;
    Fixture {
        backend,
        stream,
        source,
        destination,
    }
}

fn forget_scripted_recycled(fixture: &mut Fixture) {
    for child in &mut fixture.backend.children {
        assert!(child.native_reconciliations.iter().all(Option::is_none));
        // This CPU seam owns descriptor metadata, never a real native dispatch.
        child.recycled_dispatch = None;
        let allocations: Vec<_> = child
            .allocations
            .ordinary_iter()
            .map(|(id, _)| *id)
            .collect();
        for id in allocations {
            child.allocations.get_mut(&id).unwrap().native_dirty.clear();
        }
        child.native_dirty_extents = 0;
    }
}

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

fn phase_steps(
    reading: bool,
    offset: u64,
    byte_len: usize,
    pending: bool,
) -> Vec<ScriptedSdmaStepV1> {
    let scratch_len = byte_len.min(COOPERATIVE_COPY_CHUNK_BYTES_V1);
    let mut steps = allocation_steps(scratch_len);
    for start in (0..byte_len).step_by(COOPERATIVE_COPY_CHUNK_BYTES_V1) {
        let len = (byte_len - start).min(COOPERATIVE_COPY_CHUNK_BYTES_V1);
        if !reading {
            steps.push(ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: len,
            });
        }
        steps.push(scripted_submit_step_v1(
            if reading {
                Gfx942PersistentSdmaDirectionV1::DeviceToHost
            } else {
                Gfx942PersistentSdmaDirectionV1::HostToDevice
            },
            0,
            offset + start as u64,
            len as u32,
            ScriptedFailureModeV1::Success,
        ));
        if pending {
            steps.push(ScriptedSdmaStepV1::Poll(
                ScriptedExecutionOutcomeV1::Pending,
            ));
        }
        steps.push(completed());
        steps.push(ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success));
        if reading {
            steps.push(ScriptedSdmaStepV1::Read {
                offset: 0,
                byte_len: len as u64,
            });
        }
    }
    steps.push(ScriptedSdmaStepV1::Recycle(
        ScriptedRecycleOutcomeV1::Success,
    ));
    steps
}

fn fixture(
    byte_len: usize,
    source_steps: Vec<ScriptedSdmaStepV1>,
    destination_steps: Vec<ScriptedSdmaStepV1>,
) -> Fixture {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
    let stream = backend.create_stream_v1(8).unwrap();
    let source = backend
        .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, byte_len as u64, 8)
        .unwrap();
    let destination = backend
        .allocate_v1(8, RuntimeMemoryKindV1::DeviceLocal, byte_len as u64, 8)
        .unwrap();
    install_scripted_devices(
        &mut backend,
        source,
        destination,
        byte_len,
        source_steps,
        destination_steps,
    );
    let (source, destination) = scripted_copy_regions_v1(source, destination, byte_len as u64);
    Fixture {
        backend,
        stream,
        source,
        destination,
    }
}

fn install_scripted_devices(
    backend: &mut KfdMultiDeviceRuntimeBackendV1,
    source: u64,
    destination: u64,
    byte_len: usize,
    source_steps: Vec<ScriptedSdmaStepV1>,
    destination_steps: Vec<ScriptedSdmaStepV1>,
) {
    for (allocation, mut steps, fill) in [
        (source, source_steps, 0x53),
        (destination, destination_steps, 0x17),
    ] {
        let route = backend.allocations[&allocation];
        let host = backend.children[route.child].allocations[&route.local].kind
            == RuntimeMemoryKindV1::HostVisible;
        if !host {
            steps.push(ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success));
        }
        steps.push(ScriptedSdmaStepV1::Recycle(
            ScriptedRecycleOutcomeV1::Success,
        ));
        let driver = ScriptedSdmaDriverV1::new(steps);
        let storage = if host {
            let mut owner = driver.test_host_owner(byte_len);
            owner.scripted_bytes_mut().unwrap().fill(fill);
            KfdRuntimeSdmaStorageV1::Host(owner)
        } else {
            let mut device = driver.test_device_owner(byte_len);
            device.scripted_bytes_mut().unwrap().fill(fill);
            KfdRuntimeSdmaStorageV1::Device(Box::new(device))
        };
        let child = &mut backend.children[route.child];
        let record = child.allocations.get_mut(&route.local).unwrap();
        record.sdma_storage = storage;
        record.sdma_backed = true;
        record.sdma_initialized = true;
        record.sdma_shadow_dirty = true;
        child.native_available = true;
        child.sdma_enabled = true;
        child.scripted_sdma = Some(driver);
    }
}

impl Fixture {
    fn submit(&mut self) -> u64 {
        self.backend
            .peer_copy_v1(self.stream, self.source, self.destination, &[])
            .unwrap()
    }

    fn bytes(&self, source: bool) -> &[u8] {
        let allocation = if source {
            self.source.allocation
        } else {
            self.destination.allocation
        };
        let route = self.backend.allocations[&allocation];
        match &self.backend.children[route.child].allocations[&route.local].sdma_storage {
            KfdRuntimeSdmaStorageV1::Device(device) => device.scripted_bytes().unwrap(),
            KfdRuntimeSdmaStorageV1::Host(host) => host.scripted_bytes().unwrap(),
            _ => panic!("expected quiescent backing"),
        }
    }

    fn steps(&self) -> Vec<usize> {
        self.backend
            .children
            .iter()
            .map(|child| child.scripted_sdma.as_ref().unwrap().remaining_steps())
            .collect()
    }

    fn assert_observation_only(&mut self, submission: u64) {
        let before = self.steps();
        for _ in 0..3 {
            assert_eq!(
                self.backend.poll_v1(submission).unwrap(),
                BackendPollV1::Pending
            );
            assert_eq!(
                self.backend.wait_v1(submission, Instant::now()).unwrap(),
                BackendPollV1::Pending
            );
        }
        assert_eq!(self.steps(), before);
        self.backend.assert_cooperative_indexes_consistent();
    }

    fn clean(mut self, submissions: &[u64]) {
        for submission in submissions.iter().rev() {
            self.backend.release_submission_v1(*submission).unwrap();
        }
        assert_eq!(self.backend.cooperative_staging_bytes, 0);
        for allocation in [self.source.allocation, self.destination.allocation] {
            let route = self.backend.allocations[&allocation];
            // Scripted device teardown skips the unrelated native zero-fill path.
            self.backend.children[route.child]
                .allocations
                .get_mut(&route.local)
                .unwrap()
                .sdma_backed = false;
            self.backend.release_allocation_v1(allocation).unwrap();
        }
        self.backend.destroy_stream_v1(self.stream).unwrap();
        self.backend.assert_cooperative_indexes_consistent();
        for child in &self.backend.children {
            assert!(child.streams.is_empty());
            assert!(child.allocations.is_empty());
            assert!(child.submissions.is_empty());
            assert!(child.active_sdma.is_empty());
            assert!(child.allocation_custody.is_empty());
            assert_eq!(child.staged_context_bytes, 0);
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert_eq!(driver.remaining_steps(), 0);
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        self.backend.shutdown_native_v1().unwrap();
    }
}

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
