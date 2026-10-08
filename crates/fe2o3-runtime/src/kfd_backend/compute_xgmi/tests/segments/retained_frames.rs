use super::*;

#[test]
fn unified_segments_pending_compute_writer_is_not_bypassed_by_an_exact_event() {
    let mut f = Native::new(&[64, 96], 0);
    let source_stream = f.layout.global_streams[0];
    let destination_stream = f.layout.global_streams[1];
    let source_allocation = f.layout.global_allocations[0];
    let source_route = f.layout.routes[0];
    let [source, destination] = f.layout.backend_regions(0);
    let backend = f.context.backend_mut_for_test_v1();
    let gate_stream = backend.create_stream_v1(7).unwrap();
    let gate_stream_route = backend.streams[&gate_stream];
    // A synthetic pending control keeps a genuinely admitted compute launch
    // queued. It does not manufacture compute output or complete the writer.
    let gate_local = backend.children[0].next_id().unwrap();
    backend.children[0].submissions.insert(
        gate_local,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream: gate_stream_route.local,
            status: BackendPollV1::Pending,
            dependency_depth: 1,
            profile_dispatch_published: false,
        },
    );
    backend
        .reserve_native_stream_submission_v1(gate_stream)
        .unwrap();
    let gate = backend.next_id().unwrap();
    backend.submissions.insert(
        gate,
        RoutedSubmissionV1::Native {
            route: RoutedHandleV1 {
                child: 0,
                local: gate_local,
            },
            stream: gate_stream,
        },
    );
    backend.retain_native_stream_submission_v1(gate_stream);
    let gate_event = backend.record_event_v1(gate_stream, gate).unwrap();
    let module = backend
        .load_module_v1(7, &crate::synthetic_cov6::module())
        .unwrap();
    let kernel = backend
        .resolve_kernel_v1(module, "vecadd", [7; 32])
        .unwrap();
    let mut kernarg = [0; 16];
    kernarg[8..].copy_from_slice(&16_u64.to_le_bytes());
    let compute = backend
        .submit_v1(BackendLaunchV1 {
            stream: source_stream,
            kernel,
            explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: source_allocation,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 64,
                },
                kernarg_byte_offset: 0,
            }],
            dependencies: &[gate_event],
            geometry: RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        })
        .unwrap();
    let RoutedSubmissionV1::Native { route, .. } = backend.submissions[&compute] else {
        unreachable!()
    };
    assert!(
        backend.children[0]
            .pending_compute
            .contains_key(&route.local)
    );
    assert!(backend.children[0].allocation_is_active(source_route.local));
    assert!(!backend.children[0].any_compute_active_v1());
    let event = backend.record_event_v1(source_stream, compute).unwrap();
    let before = backend.next_handle;
    assert!(matches!(
        backend.peer_copy_segments_v1(
            destination_stream,
            source,
            destination,
            &descriptors(),
            &[event]
        ),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert_eq!(backend.next_handle, before);
    assert_eq!(backend.submissions.len(), 2);
    assert!(backend.compute_xgmi_children.iter().all(Option::is_none));
    assert!(
        backend.children[0]
            .pending_compute
            .contains_key(&route.local)
    );
    assert_eq!(
        backend.cancel_v1(compute).unwrap(),
        BackendCancellationV1::Cancelled
    );
    backend.release_event_v1(event).unwrap();
    backend.release_submission_v1(compute).unwrap();
    backend.release_event_v1(gate_event).unwrap();
    backend.children[0]
        .submissions
        .get_mut(&gate_local)
        .unwrap()
        .status = BackendPollV1::Failed { code: -17 };
    backend.release_submission_v1(gate).unwrap();
    backend.destroy_stream_v1(gate_stream).unwrap();
    backend.unload_module_v1(module).unwrap();
    let expected = f.layout.initial.clone();
    f.clean(vec![], expected);
}

#[test]
fn unified_segments_missing_frame_cannot_promote_envelope_to_consumer_coverage() {
    for published in [false, true] {
        let mut f = Native::new(&[64, 96], 0);
        let stream = f.layout.global_streams[1];
        let destination = f.layout.global_allocations[1];
        let backend = f.context.backend_mut_for_test_v1();
        // Add inert host metadata before any publication. Both requests below
        // must reject before touching host storage or admitting a native child.
        backend.children[1].native_available = false;
        backend.children[1].sdma_enabled = false;
        let host = backend
            .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 96, 8)
            .unwrap();
        backend.children[1].native_available = true;
        backend.children[1].sdma_enabled = true;
        let module = backend
            .load_module_v1(8, &crate::synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let list = [segment(0, 0, 1), segment(8, 12, 3)];
        let mut submission = f.submit(0, &list);
        let id = f.id(&submission);
        if published {
            for _ in 0..8 {
                f.context.progress_stream_v1(f.layout.streams[1]).unwrap();
                if root(&f.context, id).phase == Phase::Published {
                    break;
                }
            }
            assert_eq!(root(&f.context, id).phase, Phase::Published);
            assert_pair_held(&f, id, 0);
        }
        let gap = BackendMemoryRegionV1 {
            allocation: destination,
            access: RuntimeAccessV1::Read,
            byte_offset: 32,
            byte_len: 16,
        };
        let trace = root(&f.context, id).trace.clone();
        let backend = f.context.backend_mut_for_test_v1();
        // Isolate the legacy transfer-only profile: an envelope is never a
        // substitute for the newly retained whole-destination frame receipt.
        let RoutedSubmissionV1::CooperativeCopy(copy) = backend.submissions.get_mut(&id).unwrap()
        else {
            unreachable!()
        };
        let frame = copy
            .compute_xgmi
            .as_mut()
            .unwrap()
            .segment_frame
            .take()
            .unwrap();
        let event = backend.record_event_v1(stream, id).unwrap();
        let before = backend.next_handle;
        let retained = backend.cooperative_dependency_retain_counts.clone();
        let mut kernarg = [0; 16];
        kernarg[8..].copy_from_slice(&4_u64.to_le_bytes());
        assert!(
            matches!(backend.submit_producer_aware_launch_v1(BackendProducerAwareLaunchV1 {
            stream, kernel, explicit_kernarg: &kernarg,
            bindings: &[BackendBindingV1 { region: gap, kernarg_byte_offset: 0 }],
            dependencies: &[BackendLaunchProducerV1 { event, producer_submission: id }],
            geometry: RuntimeLaunchGeometryV1 { grid: [64, 1, 1], workgroup: [64, 1, 1], dynamic_shared_bytes: 0 },
        }), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported)
        );
        assert!(
            matches!(backend.copy_async_v1(stream, gap, BackendMemoryRegionV1 {
            allocation: host, access: RuntimeAccessV1::Write, byte_offset: 0, byte_len: 16,
        }, &[event]), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported)
        );
        assert_eq!(backend.next_handle, before);
        assert_eq!(backend.submissions.len(), 1);
        assert_eq!(backend.cooperative_dependency_retain_counts, retained);
        assert!(!backend.terminal && backend.children.iter().all(|child| !child.terminal));
        assert!(
            backend
                .children
                .iter()
                .all(|child| child.pending_compute.is_empty() && !child.any_compute_active_v1())
        );
        backend.release_event_v1(event).unwrap();
        let RoutedSubmissionV1::CooperativeCopy(copy) = backend.submissions.get_mut(&id).unwrap()
        else {
            unreachable!()
        };
        copy.compute_xgmi.as_mut().unwrap().segment_frame = Some(frame);
        assert_eq!(root(&f.context, id).trace, trace);
        if published {
            assert_pair_held(&f, id, 0);
        }
        drive(&mut f, &mut submission, 0);
        f.context
            .backend_mut_for_test_v1()
            .release_allocation_v1(host)
            .unwrap();
        f.context
            .backend_mut_for_test_v1()
            .unload_module_v1(module)
            .unwrap();
        let expected = f.layout.expected(&[&list]);
        f.clean(vec![submission], expected);
    }
}

#[test]
fn unified_segments_owned_futures_yield_to_a_disjoint_pair_and_resume_original_replies() {
    let f = Native::new(&[81, 117, 91, 127], 0);
    let layout = f.layout;
    let first = vec![segment(1, 3, 17); 16];
    let second = vec![segment(2, 4, 13); 2];
    let expected = layout.expected(&[&first, &second]);
    let (mut engine, handle) = Engine::new_with_progress(
        || Ok::<_, ()>(ManuallyDrop::into_inner(f.context)),
        RuntimeAsyncEngineConfigV1::new(8, 8, 8, 8, Duration::from_micros(1)).unwrap(),
        RuntimeAsyncProgressConfigV1::new(4, 1).unwrap(),
    )
    .unwrap();
    let mut futures = [first, second]
        .into_iter()
        .enumerate()
        .map(|(pair, list)| {
            let [source, destination] = layout.regions(pair);
            Box::pin(
                handle
                    .peer_copy_segments(
                        layout.streams[2 * pair + 1],
                        source,
                        destination,
                        list,
                        vec![],
                    )
                    .unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        engine.drive_until_ready(futures[0].as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    ));
    let mut previous = 0;
    let mut saw_second_finish_first = false;
    for _ in 0..96 {
        let mut observation = Box::pin(
            handle
                .observer()
                .enqueue_with_context(|context| {
                    let backend = context.backend();
                    let mut states: Vec<_> = backend
                        .submissions
                        .keys()
                        .map(|id| {
                            let copy = copy(context, *id);
                            (
                                copy.source.child,
                                copy.status(),
                                leaves(&root(context, *id).trace),
                            )
                        })
                        .collect();
                    states.sort_unstable_by_key(|entry| entry.0);
                    assert_eq!(backend.cooperative_progress_quantum, None);
                    states
                })
                .unwrap(),
        );
        engine.tick().unwrap();
        let states = ready(observation.as_mut()).unwrap();
        drop(observation);
        if states.len() != 2 {
            continue;
        }
        let current = states.iter().map(|(_, _, count)| count).sum::<usize>();
        assert!(current >= previous && current - previous <= 1);
        previous = current;
        saw_second_finish_first |=
            states[1].1 == BackendPollV1::Succeeded && states[0].1 == BackendPollV1::Pending;
        if states
            .iter()
            .all(|(_, status, _)| *status == BackendPollV1::Succeeded)
        {
            break;
        }
    }
    assert!(
        saw_second_finish_first,
        "one always-ready segment list monopolized the owner"
    );
    let mut submissions = Vec::new();
    for future in &mut futures {
        let result = engine
            .drive_until_ready(future.as_mut(), Instant::now() + Duration::from_secs(2))
            .unwrap()
            .unwrap();
        assert_eq!(
            result.observation.unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        assert_eq!(result.rejected_observations, 0);
        submissions.push(result.submission.unwrap());
    }
    drop(futures);
    assert_eq!(handle.observer().snapshot_bytes_in_use(), 0);
    let mut release = Box::pin(
        handle
            .observer()
            .enqueue_with_context(move |context| {
                layout.assert_storage(context, &expected);
                for submission in submissions {
                    context.release_submission(submission).unwrap();
                }
                cleanup(context);
            })
            .unwrap(),
    );
    engine
        .drive_until_ready(release.as_mut(), Instant::now() + Duration::from_secs(2))
        .unwrap()
        .unwrap();
    drop(release);
    let report = engine.shutdown();
    assert_eq!(report.disposition, RuntimeAsyncOwnedDispositionV1::Released);
    assert!(!report.worker_panicked);
    assert!(report.native_failure.is_none());
}
