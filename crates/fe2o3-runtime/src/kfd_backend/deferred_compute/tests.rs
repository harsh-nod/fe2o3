use super::*;

mod completed_results;

struct Fixture {
    backend: KfdMultiDeviceRuntimeBackendV1,
    consumer: u64,
    producer: u64,
    producer_event: u64,
    producer_local: u64,
    producer_stream: u64,
    stream: u64,
    module: u64,
    allocation: u64,
}

impl Fixture {
    fn new() -> Self {
        let mut right = KfdRuntimeBackendV1::mock();
        right.description.backend_device = 8;
        let mut backend =
            KfdMultiDeviceRuntimeBackendV1::from_backends(vec![KfdRuntimeBackendV1::mock(), right])
                .unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let producer_stream = backend.create_stream_v1(7).unwrap();
        let module = backend
            .load_module_v1(7, &crate::synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let allocation_route = backend.allocations[&allocation];
        let stream_route = backend.streams[&stream];
        let producer = backend.next_id().unwrap();
        let producer_local = backend.children[0].next_id().unwrap();
        // This retained pending record prevents native publication. The tests
        // below exercise router handoff/custody, not GPU execution or completion.
        backend.children[0].submissions.insert(
            producer_local,
            SubmissionRecordV1 {
                stream: backend.streams[&producer_stream].local,
                status: BackendPollV1::Pending,
                dependency_depth: 1,
                profile_dispatch_published: false,
            },
        );
        backend
            .reserve_native_stream_submission_v1(producer_stream)
            .unwrap();
        backend.submissions.insert(
            producer,
            RoutedSubmissionV1::Native {
                route: RoutedHandleV1 {
                    child: 0,
                    local: producer_local,
                },
                stream: producer_stream,
            },
        );
        backend.retain_native_stream_submission_v1(producer_stream);
        let producer_event = backend.record_event_v1(producer_stream, producer).unwrap();
        let event_route = match backend.events[&producer_event] {
            RoutedEventV1::Native { route, .. } => route,
            _ => unreachable!(),
        };
        backend.children[0].native_available = true;
        backend.children[0]
            .allocations
            .get_mut(&allocation_route.local)
            .unwrap()
            .sdma_backed = true;
        let bindings = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: allocation_route.local,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            kernarg_byte_offset: 0,
        }];
        let launch = BackendLaunchV1 {
            stream: stream_route.local,
            kernel: backend.kernels[&kernel].local,
            explicit_kernarg: &[11; 8],
            bindings: &bindings,
            dependencies: &[],
            geometry: crate::RuntimeLaunchGeometryV1 {
                grid: [1; 3],
                workgroup: [1; 3],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        };
        let collected = backend.children[0]
            .preflight_compute_v1(
                launch,
                ComputeDependencyRosterV1::Exact(&[BackendLaunchProducerV1 {
                    event: event_route.local,
                    producer_submission: producer_local,
                }]),
            )
            .unwrap();
        let root = DeferredComputeV1 {
            stream,
            child: 0,
            route: None,
            status: BackendPollV1::Pending,
            quiescent: None,
            completed: None,
            launch: Some(RetainedComputeLaunchV1::copy_from(launch, None).unwrap()),
            collected: Some(collected),
            kernel: (kernel, backend.kernels[&kernel]),
            module: (module, backend.modules[&module]),
            allocations: vec![(allocation, allocation_route)],
            peers: Vec::new(),
        };
        // Start at the already-restored peer boundary; public admission and
        // actual native-copy restoration are covered by queued_consumer tests.
        backend
            .deferred_compute_retains
            .prepare(stream, &root.allocations, root.module.1)
            .unwrap();
        backend.peer_launch_retains.prepare(&[producer]).unwrap();
        backend.reserve_native_stream_submission_v1(stream).unwrap();
        let consumer = backend.next_id().unwrap();
        backend
            .with_peer_launch_custody_v1(consumer, vec![producer], |backend| {
                backend.deferred_compute_retains.acquire(consumer, &root);
                backend.submissions.insert(
                    consumer,
                    RoutedSubmissionV1::DeferredCompute(Box::new(root)),
                );
                backend.retain_native_stream_submission_v1(stream);
                Ok(consumer)
            })
            .unwrap();
        Self {
            backend,
            consumer,
            producer,
            producer_event,
            producer_local,
            producer_stream,
            stream,
            module,
            allocation,
        }
    }

    fn finish(mut self) {
        if self
            .backend
            .deferred_compute_v1(self.consumer)
            .unwrap()
            .status
            == BackendPollV1::Pending
        {
            assert_eq!(
                self.backend.cancel_v1(self.consumer).unwrap(),
                crate::BackendCancellationV1::Cancelled
            );
        }
        self.backend.release_submission_v1(self.consumer).unwrap();
        if self.backend.events.contains_key(&self.producer_event) {
            self.backend.release_event_v1(self.producer_event).unwrap();
        }
        self.backend.children[0]
            .submissions
            .get_mut(&self.producer_local)
            .unwrap()
            .status = BackendPollV1::Failed { code: -2 };
        self.backend.release_submission_v1(self.producer).unwrap();
        self.backend.children[0].native_available = false;
        let allocation = self.backend.allocations[&self.allocation].local;
        self.backend.children[0]
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_backed = false;
        self.backend.release_allocation_v1(self.allocation).unwrap();
        self.backend.unload_module_v1(self.module).unwrap();
        self.backend.destroy_stream_v1(self.stream).unwrap();
        self.backend
            .destroy_stream_v1(self.producer_stream)
            .unwrap();
        self.backend.assert_cooperative_indexes_consistent();
        self.backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn deferred_compute_indexes_retain_mutations_and_survive_event_release() {
    let mut fixture = Fixture::new();
    let backend = &mut fixture.backend;
    backend.assert_cooperative_indexes_consistent();
    assert!(
        backend
            .write_allocation_v1(fixture.allocation, 0, &[0; 8])
            .is_err()
    );
    assert!(
        backend
            .read_allocation_v1(fixture.allocation, 0, &mut [0; 8])
            .is_err()
    );
    assert!(backend.release_allocation_v1(fixture.allocation).is_err());
    assert!(backend.unload_module_v1(fixture.module).is_err());
    assert!(backend.destroy_stream_v1(fixture.stream).is_err());
    backend.release_event_v1(fixture.producer_event).unwrap();
    assert!(backend.peer_launch_retains.retains(fixture.producer));
    assert_eq!(
        backend
            .progress_deferred_compute_v1(fixture.consumer)
            .unwrap(),
        BackendPollV1::Pending
    );
    let route = backend
        .deferred_compute_v1(fixture.consumer)
        .unwrap()
        .route
        .unwrap();
    let pending = &backend.children[route.child].pending_compute[&route.local];
    assert_eq!(&*pending.launch.explicit_kernarg, &[11; 8]);
    assert_eq!(
        &*pending.explicit_success_dependencies,
        &[fixture.producer_local]
    );
    assert!(Arc::ptr_eq(
        &pending.launch,
        backend
            .deferred_compute_v1(fixture.consumer)
            .unwrap()
            .launch
            .as_ref()
            .unwrap()
    ));
    backend.assert_cooperative_indexes_consistent();
    assert_eq!(
        backend.cancel_v1(fixture.consumer).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(backend.deferred_compute_retains.is_empty());
    assert!(!backend.peer_launch_retains.retains(fixture.producer));
    fixture.finish();
}

#[test]
fn deferred_compute_handoff_rechecks_child_capacity_and_keeps_diagnostic() {
    let mut fixture = Fixture::new();
    fixture.backend.children[0].compute_completion_reservations = MAX_RUNTIME_SUBMISSIONS_V1;
    let error = fixture
        .backend
        .progress_deferred_compute_v1(fixture.consumer)
        .unwrap_err();
    let RuntimeBackendFailureV1::Quiescent(error) = error else {
        panic!("accepted deferred rejection must retain its quiescent diagnostic")
    };
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
    assert_eq!(error.detail(), "KFD submission capacity exceeded");
    assert!(
        matches!(fixture.backend.poll_v1(fixture.consumer), Err(RuntimeBackendFailureV1::Quiescent(ref repeated)) if *repeated == error)
    );
    let root = fixture
        .backend
        .deferred_compute_v1(fixture.consumer)
        .unwrap();
    assert!(root.route.is_none());
    assert!(root.launch.is_none());
    assert!(fixture.backend.deferred_compute_retains.is_empty());
    assert!(
        !fixture
            .backend
            .peer_launch_retains
            .retains(fixture.producer)
    );
    fixture.backend.children[0].compute_completion_reservations = 0;
    fixture.finish();
}

#[test]
fn deferred_compute_post_handoff_terminal_and_unwind_keep_real_child_custody() {
    for unwind in [false, true] {
        let mut fixture = Fixture::new();
        let id = fixture.consumer;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fixture.backend.with_deferred_root_v1(id, |backend| {
                assert_eq!(
                    backend.progress_deferred_compute_inner_v1(id)?,
                    BackendPollV1::Pending
                );
                if unwind {
                    panic!("post-handoff fault");
                }
                Err::<(), _>(RuntimeBackendFailureV1::Terminal(
                    KfdRuntimeBackendErrorV1::new(
                        KfdRuntimeBackendErrorKindV1::Terminal,
                        "post-handoff terminal",
                    ),
                ))
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ));
        }
        assert!(fixture.backend.terminal);
        assert!(fixture.backend.children[0].terminal);
        let root = fixture.backend.deferred_compute_v1(id).unwrap();
        let route = root.route.unwrap();
        assert!(root.launch.is_some() && root.collected.is_some());
        assert!(
            fixture.backend.children[0]
                .pending_compute
                .contains_key(&route.local)
        );
        assert!(!fixture.backend.children[0].allocation_custody.is_empty());
        assert!(
            !fixture.backend.children[0]
                .compute_module_retain_counts
                .is_empty()
        );
        assert!(
            fixture
                .backend
                .peer_launch_retains
                .retains(fixture.producer)
        );
        fixture.backend.assert_cooperative_indexes_consistent();
        std::mem::forget(fixture.backend);
    }
}

#[test]
fn deferred_compute_index_overflow_rejects_before_retaining() {
    let mut fixture = Fixture::new();
    let root = fixture
        .backend
        .deferred_compute_v1(fixture.consumer)
        .unwrap();
    let mut index = DeferredComputeRetainsV1::default();
    index.modules.insert(root.module.1, usize::MAX);
    assert!(matches!(
        index.prepare(root.stream, &root.allocations, root.module.1),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(index.streams.is_empty() && index.allocations.is_empty());
    index.modules.clear();
    index.allocations.insert(root.allocations[0].1, usize::MAX);
    assert!(
        index
            .prepare(root.stream, &root.allocations, root.module.1)
            .is_err()
    );
    assert!(index.streams.is_empty() && index.modules.is_empty());
    assert!(!fixture.backend.deferred_compute_retains.is_empty());
    assert_eq!(
        fixture.backend.cancel_v1(fixture.consumer).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    fixture.finish();
}

#[test]
fn deferred_compute_quiescence_is_attributed_only_to_the_exact_child_target() {
    let mut fixture = Fixture::new();
    let id = fixture.consumer;
    fixture.backend.progress_deferred_compute_v1(id).unwrap();
    let route = fixture
        .backend
        .deferred_compute_v1(id)
        .unwrap()
        .route
        .unwrap();
    let error = KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Native,
        "predecessor quiescent fault",
    );
    assert_eq!(
        fixture
            .backend
            .attribute_deferred_child_quiescence_v1(id, route, error.clone())
            .unwrap(),
        BackendPollV1::Pending
    );
    assert_eq!(
        fixture.backend.deferred_compute_v1(id).unwrap().status,
        BackendPollV1::Pending
    );
    assert!(
        fixture.backend.children[0]
            .pending_compute
            .contains_key(&route.local)
    );
    assert!(
        fixture
            .backend
            .peer_launch_retains
            .retains(fixture.producer)
    );
    fixture.backend.assert_cooperative_indexes_consistent();
    // Real child cancellation establishes exact quiescence; no completion is fabricated.
    assert_eq!(
        fixture.backend.children[0].cancel_v1(route.local).unwrap(),
        crate::BackendCancellationV1::Cancelled
    );
    assert!(
        matches!(fixture.backend.attribute_deferred_child_quiescence_v1(id, route, error.clone()),
        Err(RuntimeBackendFailureV1::Quiescent(ref actual)) if *actual == error)
    );
    assert!(fixture.backend.deferred_compute_retains.is_empty());
    fixture.finish();
}

#[test]
fn deferred_compute_predispatch_quiescence_preserves_parent_status_and_diagnostic() {
    let mut fixture = Fixture::new();
    let error = KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Native,
        "peer ancestor quiescent fault",
    );
    assert!(
        matches!(fixture.backend.quiesce_deferred_compute_v1(fixture.consumer, error.clone()),
        Err(RuntimeBackendFailureV1::Quiescent(ref actual)) if *actual == error)
    );
    assert_eq!(
        fixture.backend.children[0].submissions[&fixture.producer_local].status,
        BackendPollV1::Pending
    );
    assert!(
        fixture
            .backend
            .deferred_compute_v1(fixture.consumer)
            .unwrap()
            .route
            .is_none()
    );
    assert!(fixture.backend.deferred_compute_retains.is_empty());
    assert!(
        fixture
            .backend
            .release_submission_v1(fixture.producer)
            .is_err()
    );
    assert!(
        matches!(fixture.backend.poll_v1(fixture.consumer), Err(RuntimeBackendFailureV1::Quiescent(ref actual)) if *actual == error)
    );
    fixture.finish();
}
