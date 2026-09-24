use super::*;
use crate::synthetic_cov6;

fn geometry() -> crate::RuntimeLaunchGeometryV1 {
    crate::RuntimeLaunchGeometryV1 {
        grid: [1, 1, 1],
        workgroup: [1, 1, 1],
        dynamic_shared_bytes: 0,
    }
}

fn request(
    stream: u64,
    kernel: u64,
    dependencies: &[BackendLaunchProducerV1],
) -> BackendProducerAwareLaunchV1<'_> {
    BackendProducerAwareLaunchV1 {
        stream,
        kernel,
        explicit_kernarg: &[],
        bindings: &[],
        dependencies,
        geometry: geometry(),
    }
}

#[test]
fn exact_single_device_dependencies_reject_alias_mismatch_and_failure() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.submissions.insert(
        40,
        SubmissionRecordV1 {
            stream,
            status: BackendPollV1::Pending,
            profile_dispatch_published: false,
        },
    );
    let first = backend.record_event_v1(stream, 40).unwrap();
    let alias = backend.record_event_v1(stream, 40).unwrap();
    let exact = BackendLaunchProducerV1 {
        event: first,
        producer_submission: 40,
    };
    assert_eq!(
        backend
            .collect_exact_compute_dependencies_v1(&[exact])
            .unwrap(),
        vec![40].into_boxed_slice(),
    );
    assert!(matches!(
        backend.collect_exact_compute_dependencies_v1(&[
            exact,
            BackendLaunchProducerV1 {
                event: alias,
                producer_submission: 40,
            },
        ]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert!(matches!(
        backend.collect_exact_compute_dependencies_v1(&[BackendLaunchProducerV1 {
            event: first,
            producer_submission: 41,
        }]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    backend.submissions.get_mut(&40).unwrap().status = BackendPollV1::Failed { code: -7 };
    assert!(matches!(
        backend.collect_exact_compute_dependencies_v1(&[exact]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    let record = backend.submissions.remove(&40).unwrap();
    assert!(matches!(
        backend.collect_exact_compute_dependencies_v1(&[exact]),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle
    ));
    backend.submissions.insert(40, record);

    backend.release_event_v1(first).unwrap();
    backend.release_event_v1(alias).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();

    let mut terminal = KfdRuntimeBackendV1::mock();
    terminal.terminal = true;
    assert!(matches!(
        terminal.submit_producer_aware_launch_v1(request(1, 2, &[])),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    // A production terminal backend is process-teardown custody. This fixture
    // owns no native resource and intentionally bypasses its fail-stop Drop.
    std::mem::forget(terminal);
}

#[test]
fn producer_aware_single_device_rejection_precedes_launch_custody() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    backend.submissions.insert(
        40,
        SubmissionRecordV1 {
            stream,
            status: BackendPollV1::Succeeded,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, 40).unwrap();
    backend.native_available = true;
    let before_handle = backend.next_handle;
    let before_retains = backend.compute_dependency_retain_counts.clone();

    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            999,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: 41,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.next_handle, before_handle);
    assert_eq!(backend.compute_dependency_retain_counts, before_retains);
    assert!(backend.pending_compute.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);

    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            999,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: 40,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle
    ));
    assert_eq!(backend.next_handle, before_handle);
    assert_eq!(backend.compute_dependency_retain_counts, before_retains);
    assert!(backend.pending_compute.is_empty());

    backend.native_available = false;
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(40).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn accepted_pending_launch_retains_exact_producer_and_launch_custody() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    let module = backend
        .load_module_v1(7, &synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let allocation = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    backend.submissions.insert(
        40,
        SubmissionRecordV1 {
            stream,
            status: BackendPollV1::Pending,
            profile_dispatch_published: false,
        },
    );
    let event = backend.record_event_v1(stream, 40).unwrap();
    let binding = BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 8,
        },
        kernarg_byte_offset: 0,
    };
    backend.native_available = true;
    let bindings = [binding];
    let dependencies = [BackendLaunchProducerV1 {
        event,
        producer_submission: 40,
    }];
    let launch = BackendProducerAwareLaunchV1 {
        stream,
        kernel,
        explicit_kernarg: &[],
        bindings: &bindings,
        dependencies: &dependencies,
        geometry: geometry(),
    };
    let before_handle = backend.next_handle;
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(launch),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.next_handle, before_handle);
    assert!(backend.pending_compute.is_empty());
    assert!(backend.compute_dependency_retain_counts.is_empty());
    assert!(backend.compute_module_retain_counts.is_empty());
    assert!(backend.allocation_custody.is_empty());
    assert_eq!(backend.compute_completion_reservations, 0);

    // Model retained backing only: the pending producer prevents native
    // materialization, and cancellation must refund the real pending ledger.
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .sdma_backed = true;
    let submission = backend.submit_producer_aware_launch_v1(launch).unwrap();

    assert_eq!(
        backend.pending_compute[&submission]
            .explicit_success_dependencies
            .as_ref(),
        &[40],
    );
    assert_eq!(backend.compute_dependency_retain_counts[&40], 1);
    assert_eq!(backend.compute_module_retain_counts[&module], 1);
    assert!(backend.allocation_custody.contains_key(&allocation));
    assert_eq!(backend.compute_completion_reservations, 1);

    backend.release_event_v1(event).unwrap();
    assert!(!backend.event_submission_retain_counts.contains_key(&40));
    assert!(matches!(
        backend.release_submission_v1(40),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
    ));
    assert_eq!(
        backend.cancel_v1(submission).unwrap(),
        crate::BackendCancellationV1::Cancelled,
    );
    assert!(!backend.compute_dependency_retain_counts.contains_key(&40));
    assert!(!backend.compute_module_retain_counts.contains_key(&module));
    assert!(!backend.allocation_custody.contains_key(&allocation));
    assert_eq!(backend.compute_completion_reservations, 0);

    backend.release_submission_v1(40).unwrap();
    backend.release_submission_v1(submission).unwrap();
    backend.native_available = false;
    backend
        .allocations
        .get_mut(&allocation)
        .unwrap()
        .sdma_backed = false;
    backend.release_allocation_v1(allocation).unwrap();
    backend.unload_module_v1(module).unwrap();
    backend.destroy_stream_v1(stream).unwrap();
    backend.shutdown_native_v1().unwrap();
}

fn routed_backend() -> KfdMultiDeviceRuntimeBackendV1 {
    let left = KfdRuntimeBackendV1::mock();
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    KfdMultiDeviceRuntimeBackendV1::from_backends(vec![left, right]).unwrap()
}

#[test]
fn accepted_routed_producer_launch_retains_and_refunds_only_its_child() {
    for device in [7, 8] {
        let mut backend = routed_backend();
        let stream = backend.create_stream_v1(device).unwrap();
        let module = backend
            .load_module_v1(device, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let allocation = backend
            .allocate_v1(device, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let stream_route = backend.streams[&stream];
        let allocation_route = backend.allocations[&allocation];
        let module_route = backend.modules[&module];
        let child = stream_route.child;
        let producer = backend.next_id().unwrap();
        let local_producer = backend.children[child].next_id().unwrap();
        // A pending scripted producer keeps the consumer in its real ledger,
        // so this tests routing/custody without claiming native execution.
        backend.children[child].submissions.insert(
            local_producer,
            SubmissionRecordV1 {
                stream: stream_route.local,
                status: BackendPollV1::Pending,
                profile_dispatch_published: false,
            },
        );
        backend.reserve_native_stream_submission_v1(stream).unwrap();
        backend.submissions.insert(
            producer,
            RoutedSubmissionV1::Native {
                route: RoutedHandleV1 {
                    child,
                    local: local_producer,
                },
                stream,
            },
        );
        backend.retain_native_stream_submission_v1(stream);
        let event = backend.record_event_v1(stream, producer).unwrap();
        backend.children[child].native_available = true;
        backend.children[child]
            .allocations
            .get_mut(&allocation_route.local)
            .unwrap()
            .sdma_backed = true;
        let submission = backend
            .submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &[],
                bindings: &[BackendBindingV1 {
                    region: BackendMemoryRegionV1 {
                        allocation,
                        access: RuntimeAccessV1::Read,
                        byte_offset: 0,
                        byte_len: 8,
                    },
                    kernarg_byte_offset: 0,
                }],
                dependencies: &[BackendLaunchProducerV1 {
                    event,
                    producer_submission: producer,
                }],
                geometry: geometry(),
            })
            .unwrap();
        let RoutedSubmissionV1::Native { route, .. } = backend.submissions[&submission] else {
            panic!("producer launch must keep its native route");
        };
        assert_eq!(route.child, child);
        assert_eq!(backend.native_stream_submission_counts[&stream], 2);
        let native = &backend.children[child];
        let pending = &native.pending_compute[&route.local];
        assert_eq!(
            pending.explicit_success_dependencies.as_ref(),
            &[local_producer]
        );
        assert_eq!(pending.launch.stream, stream_route.local);
        assert_eq!(
            pending.launch.bindings[0].region.allocation,
            allocation_route.local
        );
        assert_eq!(native.compute_dependency_retain_counts[&local_producer], 1);
        assert_eq!(native.compute_module_retain_counts[&module_route.local], 1);
        assert!(
            native
                .allocation_custody
                .contains_key(&allocation_route.local)
        );
        assert_eq!(native.compute_completion_reservations, 1);
        assert!(backend.children[1 - child].submissions.is_empty());
        assert!(backend.children[1 - child].pending_compute.is_empty());

        backend.release_event_v1(event).unwrap();
        assert!(
            !backend
                .event_submission_retain_counts
                .contains_key(&producer)
        );
        assert!(matches!(
            backend.release_submission_v1(producer),
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy
        ));
        assert_eq!(backend.native_stream_submission_counts[&stream], 2);
        assert_eq!(
            backend.cancel_v1(submission).unwrap(),
            crate::BackendCancellationV1::Cancelled
        );
        let native = &backend.children[child];
        assert!(native.compute_dependency_retain_counts.is_empty());
        assert!(native.compute_module_retain_counts.is_empty());
        assert!(native.allocation_custody.is_empty());
        assert_eq!(native.compute_completion_reservations, 0);
        backend.release_submission_v1(submission).unwrap();
        backend.release_submission_v1(producer).unwrap();
        assert!(backend.native_stream_submission_counts.is_empty());

        backend.children[child].native_available = false;
        backend.children[child]
            .allocations
            .get_mut(&allocation_route.local)
            .unwrap()
            .sdma_backed = false;
        backend.release_allocation_v1(allocation).unwrap();
        backend.unload_module_v1(module).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn routed_exact_dependency_requires_native_same_child_identity() {
    let mut backend = routed_backend();
    let stream = 10;
    let kernel = 20;
    let producer = 40;
    let event = 50;
    backend.streams.insert(
        stream,
        RoutedHandleV1 {
            child: 0,
            local: 11,
        },
    );
    backend.kernels.insert(
        kernel,
        RoutedHandleV1 {
            child: 0,
            local: 21,
        },
    );
    backend.submissions.insert(
        producer,
        RoutedSubmissionV1::Native {
            route: RoutedHandleV1 {
                child: 0,
                local: 41,
            },
            stream,
        },
    );
    backend.events.insert(
        event,
        RoutedEventV1::Native {
            route: RoutedHandleV1 {
                child: 0,
                local: 51,
            },
            submission: producer,
        },
    );
    assert_eq!(
        backend
            .exact_launch_dependency_for_child(
                BackendLaunchProducerV1 {
                    event,
                    producer_submission: producer,
                },
                0,
            )
            .unwrap(),
        BackendLaunchProducerV1 {
            event: 51,
            producer_submission: 41,
        },
    );

    let before_handle = backend.next_handle;
    let oversized_binding = BackendBindingV1 {
        region: BackendMemoryRegionV1 {
            allocation: 999,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 1,
        },
        kernarg_byte_offset: 0,
    };
    let oversized_bindings = vec![oversized_binding; fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 + 1];
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &[],
            bindings: &oversized_bindings,
            dependencies: &[],
            geometry: geometry(),
        }),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
    ));
    assert_eq!(backend.next_handle, before_handle);

    backend.children[0].native_available = true;
    backend.children[0]
        .events
        .insert(51, EventRecordV1 { submission: 41 });
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle
    ));
    assert_eq!(backend.next_handle, before_handle);
    backend.children[0].submissions.insert(
        41,
        SubmissionRecordV1 {
            stream: 11,
            status: BackendPollV1::Failed { code: -3 },
            profile_dispatch_published: false,
        },
    );
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.next_handle, before_handle);
    backend.children[0].submissions.remove(&41);
    backend.children[0].events.remove(&51);
    backend.children[0].native_available = false;

    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer + 1,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.next_handle, before_handle);

    let alias_event = event + 1;
    backend.events.insert(
        alias_event,
        RoutedEventV1::Native {
            route: RoutedHandleV1 {
                child: 0,
                local: 52,
            },
            submission: producer,
        },
    );
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[
                BackendLaunchProducerV1 {
                    event,
                    producer_submission: producer,
                },
                BackendLaunchProducerV1 {
                    event: alias_event,
                    producer_submission: producer,
                },
            ],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    assert_eq!(backend.next_handle, before_handle);
    backend.events.remove(&alias_event);

    backend.events.insert(
        event,
        RoutedEventV1::Native {
            route: RoutedHandleV1 {
                child: 1,
                local: 51,
            },
            submission: producer,
        },
    );
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::WrongDevice
    ));
    assert_eq!(backend.next_handle, before_handle);

    backend.events.insert(
        event,
        RoutedEventV1::CooperativeCopy {
            submission: producer,
            child: 0,
        },
    );
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer,
            }],
        )),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
    ));
    assert_eq!(backend.next_handle, before_handle);

    backend.events.insert(
        event,
        RoutedEventV1::Native {
            route: RoutedHandleV1 {
                child: 0,
                local: 51,
            },
            submission: producer,
        },
    );
    backend.children[0].native_available = true;
    backend.children[0].terminal = true;
    assert!(matches!(
        backend.submit_producer_aware_launch_v1(request(
            stream,
            kernel,
            &[BackendLaunchProducerV1 {
                event,
                producer_submission: producer,
            }],
        )),
        Err(RuntimeBackendFailureV1::Terminal(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Terminal
    ));
    assert!(backend.terminal);
    assert_eq!(backend.next_handle, before_handle);
    // Synthetic backends own no native resource; clear the injected latch so
    // their normal empty shutdown assertions can run.
    backend.children[0].terminal = false;
    backend.children[0].native_available = false;
    backend.terminal = false;

    backend.events.clear();
    backend.submissions.clear();
    backend.streams.clear();
    backend.kernels.clear();
    backend.shutdown_native_v1().unwrap();
}
