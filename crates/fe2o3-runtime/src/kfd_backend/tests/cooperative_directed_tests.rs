use super::*;
mod peer_ancestry_tests;
use crate::{
    BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    BackendDirectedScalarProgressV1, RuntimeDirectedScalarPeerCopyBackendV1,
};

struct Fixture {
    backend: KfdMultiDeviceRuntimeBackendV1,
    streams: [u64; 2],
    allocations: [u64; 4],
}

impl Fixture {
    fn new() -> Self {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let streams = [7, 8].map(|device| backend.create_stream_v1(device).unwrap());
        let allocations = [7, 8, 7, 8].map(|device| {
            backend
                .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap()
        });
        backend
            .write_allocation_v1(allocations[0], 0, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        Self {
            backend,
            streams,
            allocations,
        }
    }

    fn route(&self, source: usize, destination: usize) -> BackendDirectedPeerRouteV1 {
        BackendDirectedPeerRouteV1 {
            stream: self.streams[destination % 2],
            source_device: 7 + (source % 2) as u64,
            destination_device: 7 + (destination % 2) as u64,
            source: BackendMemoryRegionV1 {
                allocation: self.allocations[source],
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            destination: BackendMemoryRegionV1 {
                allocation: self.allocations[destination],
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 8,
            },
        }
    }

    fn submit(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        dependencies: &[BackendDirectedPeerDependencyV1],
    ) -> u64 {
        self.backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route,
                dependencies,
            })
            .unwrap()
    }

    fn event(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        submission: u64,
    ) -> BackendDirectedPeerDependencyV1 {
        BackendDirectedPeerDependencyV1 {
            event: self
                .backend
                .record_event_v1(route.stream, submission)
                .unwrap(),
            producer_submission: submission,
        }
    }

    fn progress(
        &mut self,
        submission: u64,
        route: BackendDirectedPeerRouteV1,
        producers: &[u64],
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.backend
            .progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission,
                route,
                producer_submissions: producers,
            })
    }

    fn drive(
        &mut self,
        submission: u64,
        route: BackendDirectedPeerRouteV1,
        producers: &[u64],
    ) -> BackendPollV1 {
        for _ in 0..8192 {
            let status = self.progress(submission, route, producers).unwrap();
            self.backend.assert_cooperative_indexes_consistent();
            if status != BackendPollV1::Pending {
                return status;
            }
        }
        panic!("directed operation did not settle");
    }

    fn clean(mut self, submissions: &[u64]) {
        for id in submissions.iter().rev() {
            self.backend.release_submission_v1(*id).unwrap();
        }
        for allocation in self.allocations {
            self.backend.release_allocation_v1(allocation).unwrap();
        }
        for stream in self.streams {
            self.backend.destroy_stream_v1(stream).unwrap();
        }
        self.backend.assert_cooperative_indexes_consistent();
        self.backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn directed_router_chain_progress_reports_requested_status_and_retains_exact_history() {
    let mut f = Fixture::new();
    let first_route = f.route(0, 1);
    let first = f.submit(first_route, &[]);
    let event = f.event(first_route, first);
    let second_route = f.route(1, 2);
    let second = f.submit(second_route, &[event]);
    f.backend.release_event_v1(event.event).unwrap();
    for _ in 0..4 {
        assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            f.backend.wait_v1(second, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert_eq!(f.backend.cooperative_progress_generation, 0);
    while f.backend.poll_v1(first).unwrap() == BackendPollV1::Pending {
        assert_eq!(
            f.progress(second, second_route, &[first]).unwrap(),
            BackendPollV1::Pending
        );
    }
    assert!(matches!(
        f.backend.release_submission_v1(first),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(
        f.drive(second, second_route, &[first]),
        BackendPollV1::Succeeded
    );
    let mut output = [0; 8];
    f.backend
        .read_allocation_v1(f.allocations[2], 0, &mut output)
        .unwrap();
    assert_eq!(output, [1, 2, 3, 4, 5, 6, 7, 8]);
    f.backend.release_submission_v1(first).unwrap();
    for allocation in f.allocations {
        f.backend.release_allocation_v1(allocation).unwrap();
    }
    assert_eq!(
        f.progress(second, second_route, &[first]).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(matches!(
        f.progress(second, second_route, &[]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    f.backend.destroy_stream_v1(second_route.stream).unwrap();
    assert!(f.backend.submissions.contains_key(&second));
    assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    f.backend.release_submission_v1(second).unwrap();
    for stream in f.streams {
        if stream != second_route.stream {
            f.backend.destroy_stream_v1(stream).unwrap();
        }
    }
    f.backend.assert_cooperative_indexes_consistent();
    f.backend.shutdown_native_v1().unwrap();
}

#[test]
fn directed_router_authenticates_route_event_aliases_profile_and_progress_order() {
    let mut f = Fixture::new();
    let first_route = f.route(0, 1);
    let first = f.submit(first_route, &[]);
    let event = f.event(first_route, first);
    let alias = f.event(first_route, first);
    let route = f.route(1, 2);
    let before = f.backend.next_handle;
    for dependencies in [
        vec![event, alias],
        vec![BackendDirectedPeerDependencyV1 {
            event: event.event,
            producer_submission: first + 1,
        }],
    ] {
        assert!(matches!(
            f.backend
                .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                    route,
                    dependencies: &dependencies
                }),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(f.backend.next_handle, before);
    }
    let mut wrong = route;
    wrong.source_device = 999;
    assert!(matches!(
        f.backend
            .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                route: wrong,
                dependencies: &[event]
            }),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    let second = f.submit(route, &[event]);
    for forged in [
        BackendDirectedPeerRouteV1 {
            source_device: 999,
            ..route
        },
        BackendDirectedPeerRouteV1 {
            source: BackendMemoryRegionV1 {
                access: RuntimeAccessV1::ReadWrite,
                ..route.source
            },
            ..route
        },
    ] {
        assert!(matches!(
            f.progress(second, forged, &[first]),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
    }
    assert!(matches!(
        f.progress(second, route, &[first, first]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.backend.cooperative_progress_generation, 0);
    f.backend.release_event_v1(alias.event).unwrap();
    f.backend.release_event_v1(event.event).unwrap();
    assert_eq!(f.drive(second, route, &[first]), BackendPollV1::Succeeded);
    f.clean(&[first, second]);

    let mut f = Fixture::new();
    let route = f.route(0, 1);
    let legacy = f
        .backend
        .peer_copy_v1(route.stream, route.source, route.destination, &[])
        .unwrap();
    let event = f.event(route, legacy);
    let consumer = f.route(1, 2);
    assert!(
        matches!(f.backend.submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 { route: consumer, dependencies: &[event] }), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported)
    );
    assert!(
        matches!(f.backend.submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 { route, dependencies: &[] }), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported)
    );
    f.backend.release_event_v1(event.event).unwrap();
    f.backend.cancel_v1(legacy).unwrap();
    f.clean(&[legacy]);
}

#[test]
fn directed_router_failed_chain_settles_one_dependent_per_quantum() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let event = f.event(a, first);
    let b = f.route(1, 2);
    let second = f.submit(b, &[event]);
    f.backend.release_event_v1(event.event).unwrap();
    let event = f.event(b, second);
    let c = f.route(2, 3);
    let third = f.submit(c, &[event]);
    f.backend.release_event_v1(event.event).unwrap();
    assert_eq!(
        f.backend.cancel_v1(first).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert_eq!(
        f.progress(third, c, &[second]).unwrap(),
        BackendPollV1::Pending
    );
    assert!(matches!(
        f.backend.poll_v1(second).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.backend.poll_v1(third).unwrap(), BackendPollV1::Pending);
    assert!(matches!(
        f.progress(third, c, &[second]).unwrap(),
        BackendPollV1::Failed { .. }
    ));
    assert_eq!(f.backend.cooperative_staging_bytes, 0);
    f.clean(&[first, second, third]);
}

#[test]
fn directed_router_preserves_completed_depth_and_exact_implicit_ordering() {
    let mut f = Fixture::new();
    let mut submissions = Vec::new();
    let mut event = None;
    for depth in 1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
        let route = f.route((depth - 1) % 2, depth % 2);
        let dependencies = event.into_iter().collect::<Vec<_>>();
        let id = f.submit(route, &dependencies);
        let producer = dependencies
            .iter()
            .map(|event| event.producer_submission)
            .collect::<Vec<_>>();
        if let Some(prior) = event {
            f.backend.release_event_v1(prior.event).unwrap();
        }
        assert_eq!(f.drive(id, route, &producer), BackendPollV1::Succeeded);
        let RoutedSubmissionV1::CooperativeCopy(copy) = &f.backend.submissions[&id] else {
            panic!()
        };
        assert_eq!(copy.dependency_depth, depth);
        event = Some(f.event(route, id));
        submissions.push(id);
    }
    let route = f.route(0, 1);
    let before = f.backend.next_handle;
    assert!(
        matches!(f.backend.submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 { route, dependencies: &[event.unwrap()] }), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!(f.backend.next_handle, before);
    f.backend.release_event_v1(event.unwrap().event).unwrap();
    f.clean(&submissions);
}

#[test]
fn directed_router_read_fanout_and_implicit_tail_keep_original_explicit_roster() {
    let mut f = Fixture::new();
    let route = f.route(0, 1);
    let first = f.submit(route, &[]);
    let second_route = f.route(0, 3);
    let second = f.submit(second_route, &[]);
    // Implicit FIFO producer is retained for execution but is not caller provenance.
    assert!(matches!(
        f.progress(second, second_route, &[first]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.drive(second, second_route, &[]), BackendPollV1::Succeeded);
    f.clean(&[first, second]);

    let mut f = Fixture::new();
    let first_route = f.route(0, 1);
    let first = f.submit(first_route, &[]);
    let stream = f.backend.create_stream_v1(8).unwrap();
    let second_route = BackendDirectedPeerRouteV1 {
        stream,
        ..f.route(0, 3)
    };
    let second = f.submit(second_route, &[]);
    assert_eq!(f.drive(second, second_route, &[]), BackendPollV1::Succeeded);
    assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Pending);
    assert_eq!(f.drive(first, first_route, &[]), BackendPollV1::Succeeded);
    f.backend.release_submission_v1(second).unwrap();
    f.backend.destroy_stream_v1(stream).unwrap();
    f.clean(&[first]);
}

#[test]
fn directed_router_shared_flush_rejects_corrupt_retained_route_before_action() {
    let mut f = Fixture::new();
    let route = f.route(0, 1);
    let id = f.submit(route, &[]);
    let RoutedSubmissionV1::CooperativeCopy(copy) = f.backend.submissions.get_mut(&id).unwrap()
    else {
        panic!()
    };
    copy.source_region.byte_offset = 1;
    assert!(matches!(
        f.backend.flush_stream_v1(route.stream),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(f.backend.terminal);
    assert_eq!(f.backend.cooperative_progress_generation, 0);
    assert!(
        f.backend
            .cooperative_allocation_owners
            .values()
            .all(|owners| owners.contains(&id))
    );
}

#[test]
fn directed_router_rejects_phase_and_consumed_prefix_corruption_without_actions() {
    for (phase, consumed, cancel) in [
        (CooperativeCopyPhaseV1::Read, 0, false),
        (CooperativeCopyPhaseV1::Write, 0, false),
        (CooperativeCopyPhaseV1::Read, 1, false),
        (CooperativeCopyPhaseV1::Dependencies, 1, true),
    ] {
        let mut f = Fixture::new();
        let a = f.route(0, 1);
        let first = f.submit(a, &[]);
        let event = f.event(a, first);
        let b = f.route(1, 2);
        let second = f.submit(b, &[event]);
        if cancel {
            f.backend.cancel_v1(first).unwrap();
        }
        let RoutedSubmissionV1::CooperativeCopy(copy) =
            f.backend.submissions.get_mut(&second).unwrap()
        else {
            panic!()
        };
        copy.phase = phase;
        copy.dependency_cursor = consumed;
        let before = f.backend.cooperative_progress_generation;
        assert!(matches!(
            f.progress(second, b, &[first]),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(f.backend.cooperative_progress_generation, before);
        assert!(f.backend.submissions.contains_key(&second));
        assert!(f.backend.terminal);
    }
}

#[test]
fn directed_router_retained_event_and_tail_corruption_seals_before_admission() {
    for corruption in 0..4 {
        let mut f = Fixture::new();
        let a = f.route(0, 1);
        let first = f.submit(a, &[]);
        let event = f.event(a, first);
        assert_eq!(f.drive(first, a, &[]), BackendPollV1::Succeeded);
        let b = f.route(1, 2);
        match corruption {
            0 => {
                f.backend.submissions.remove(&first);
            }
            1 => {
                f.backend.events.insert(
                    event.event,
                    RoutedEventV1::Native {
                        submission: first,
                        route: RoutedHandleV1 {
                            child: 1,
                            local: 99,
                        },
                    },
                );
            }
            2 => {
                f.backend.events.insert(
                    event.event,
                    RoutedEventV1::CooperativeCopy {
                        submission: first,
                        child: 0,
                    },
                );
            }
            _ => {
                f.backend
                    .cooperative_stream_tails
                    .insert(b.stream, u64::MAX);
            }
        }
        let before = (
            f.backend.next_handle,
            f.backend.cooperative_staging_bytes,
            f.backend.cooperative_progress_generation,
        );
        assert!(matches!(
            f.backend
                .submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                    route: b,
                    dependencies: &[event]
                }),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(
            (
                f.backend.next_handle,
                f.backend.cooperative_staging_bytes,
                f.backend.cooperative_progress_generation
            ),
            before
        );
        assert!(f.backend.terminal);
    }
}

#[test]
fn directed_router_two_parent_order_survives_event_and_parent_retirement() {
    let mut f = Fixture::new();
    let a = f.route(0, 1);
    let first = f.submit(a, &[]);
    let ea = f.event(a, first);
    let stream = f.backend.create_stream_v1(8).unwrap();
    let b = BackendDirectedPeerRouteV1 {
        stream,
        ..f.route(0, 3)
    };
    let second = f.submit(b, &[]);
    let eb = f.event(b, second);
    let c = f.route(1, 2);
    let third = f.submit(c, &[eb, ea]);
    for event in [ea, eb] {
        f.backend.release_event_v1(event.event).unwrap();
    }
    assert!(matches!(
        f.progress(third, c, &[first, second]),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(f.backend.cooperative_progress_generation, 0);
    for _ in 0..64 {
        assert_eq!(
            f.progress(third, c, &[second, first]).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(f.backend.poll_v1(first).unwrap(), BackendPollV1::Pending);
        if f.backend.poll_v1(second).unwrap() == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(f.backend.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
    assert_eq!(
        f.drive(third, c, &[second, first]),
        BackendPollV1::Succeeded
    );
    for id in [first, second] {
        f.backend.release_submission_v1(id).unwrap();
    }
    f.backend.destroy_stream_v1(stream).unwrap();
    assert_eq!(
        f.progress(third, c, &[second, first]).unwrap(),
        BackendPollV1::Succeeded
    );
    f.clean(&[third]);
}

#[test]
fn directed_router_owner_capacity_bounds_fanout_and_mixed_profile_admission() {
    let mut f = Fixture::new();
    let mut records = Vec::new();
    for _ in 0..MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 {
        let stream = f.backend.create_stream_v1(8).unwrap();
        let allocation = f
            .backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let route = BackendDirectedPeerRouteV1 {
            stream,
            destination: BackendMemoryRegionV1 {
                allocation,
                ..f.route(0, 1).destination
            },
            ..f.route(0, 1)
        };
        let id = f.submit(route, &[]);
        let event = f.event(route, id);
        records.push((route, id, event));
    }
    let route = f.route(0, 1);
    let before = (f.backend.next_handle, f.backend.cooperative_staging_bytes);
    assert!(
        matches!(f.backend.submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 { route, dependencies: &[] }), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    let events = records
        .iter()
        .map(|(_, _, event)| event.event)
        .collect::<Vec<_>>();
    assert!(
        matches!(f.backend.peer_copy_v1(route.stream, route.source, route.destination, &events), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    assert_eq!(
        (f.backend.next_handle, f.backend.cooperative_staging_bytes),
        before
    );
    assert!(!f.backend.terminal);
    for (route, id, event) in records {
        f.backend.release_event_v1(event.event).unwrap();
        f.backend.cancel_v1(id).unwrap();
        f.backend.release_submission_v1(id).unwrap();
        f.backend
            .release_allocation_v1(route.destination.allocation)
            .unwrap();
        f.backend.destroy_stream_v1(route.stream).unwrap();
    }
    f.clean(&[]);
}

#[test]
fn directed_router_context_reconciles_pending_chain_with_and_without_journal() {
    for journal in [false, true] {
        let left = KfdRuntimeBackendV1::mock();
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let backend = KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap();
        let mut context = if journal {
            crate::RuntimeContextV1::open_with_version_journal_v1(backend, 8, 8).unwrap()
        } else {
            crate::RuntimeContextV1::open(backend).unwrap()
        };
        let devices = [context.devices()[0].id(), context.devices()[1].id()];
        let streams = devices.map(|device| context.create_stream(device).unwrap());
        let source = context
            .allocate(devices[0], RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let middle = context
            .allocate(devices[1], RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = context
            .allocate(devices[0], RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        context
            .write_allocation(source, 0, &[1, 2, 3, 4, 5, 6, 7, 8])
            .unwrap();
        let region = |allocation, access| crate::RuntimeMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };
        let mut first = context
            .directed_peer_copy_v1(
                streams[1],
                region(source, RuntimeAccessV1::Read),
                region(middle, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let event = context.record_event(&first).unwrap();
        let mut second = context
            .directed_peer_copy_v1(
                streams[0],
                region(middle, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[event],
            )
            .unwrap();
        context.release_event(event).unwrap();
        assert_eq!(
            context.poll(&mut second).unwrap(),
            crate::RuntimePollV1::Pending
        );
        for _ in 0..128 {
            if context.progress_directed_peer_copy_v1(&mut second).unwrap()
                == crate::RuntimePollV1::Succeeded
            {
                break;
            }
        }
        assert_eq!(
            context.poll(&mut second).unwrap(),
            crate::RuntimePollV1::Succeeded
        );
        assert_eq!(
            context.poll(&mut first).unwrap(),
            crate::RuntimePollV1::Succeeded
        );
        let mut output = [0; 8];
        context
            .read_allocation(destination, 0, &mut output)
            .unwrap();
        assert_eq!(output, [1, 2, 3, 4, 5, 6, 7, 8]);
        context.release_submission(second).unwrap();
        context.release_submission(first).unwrap();
        for allocation in [source, middle, destination] {
            context.release_allocation(allocation).unwrap();
        }
        for stream in streams {
            context.destroy_stream(stream).unwrap();
        }
        context.shutdown().unwrap().shutdown_native_v1().unwrap();
    }
}
