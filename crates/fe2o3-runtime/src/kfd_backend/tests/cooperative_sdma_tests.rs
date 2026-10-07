mod copy_lifecycle;
mod dirty_authority;
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
