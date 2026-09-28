//! Scripted ordinary first submission, not native batch or hardware evidence.

use super::super::materialized_publication::{
    MaterializedBindingV1, MaterializedFirstSubmissionV1,
    ScriptedMaterializedPublicationFaultV1 as Fault, with_recycled_materialized_metadata_v1,
};
use super::initial_publication_tests::pending_facts;
use super::sdma_host_write_tests::discard_scripted_fixture;
use super::*;
use std::mem::ManuallyDrop;
use std::os::unix::process::ExitStatusExt;
use std::panic::{AssertUnwindSafe, catch_unwind};

const FAULTS: [Fault; 11] = [
    Fault::BindingRejected,
    Fault::BindingUnwind,
    Fault::InitialObserverUnwind,
    Fault::SubmitRejected,
    Fault::SubmitTerminal,
    Fault::SubmitUnwind,
    Fault::OuterErrorAfterRetry,
    Fault::OuterUnwindAfterRetry,
    Fault::OuterErrorAfterPublish,
    Fault::OuterUnwindAfterPublish,
    Fault::ProfileUnwind,
];

struct Fixture {
    backend: ManuallyDrop<KfdRuntimeBackendV1>,
    lane: usize,
    stream: u64,
    module: u64,
    host: u64,
    predecessor: u64,
    first: u64,
    trailing: u64,
    recipe: Arc<OwnedComputeLaunchV1>,
    trailing_recipe: String,
    primary: Option<String>,
}

impl Fixture {
    fn new(lane: usize, write: bool) -> Self {
        let (backend, stream, host, _) =
            scripted_direct_backend_configured_v1(4096, [], |backend| {
                backend
                    .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0xa7; 32], 256).unwrap())
                    .unwrap();
            });
        let mut backend = ManuallyDrop::new(backend);
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let (stream, host) = if lane == 0 {
            (stream, host)
        } else {
            let blocker = submit_scripted_read_v1(&mut backend, stream, kernel, host, 4096, &[]);
            backend.flush_stream_v1(stream).unwrap();
            assert_eq!(backend.active.as_ref().unwrap().id, blocker);
            let stream = backend.create_stream_v1(7).unwrap();
            let (host, _) = add_scripted_direct_pair_v1(&mut backend, 4096);
            (stream, host)
        };
        // A completed ordered predecessor and explicit event remain distinct from
        // the successor retain acquired for the first accepted launch below.
        let predecessor = submit_scripted_read_v1(&mut backend, stream, kernel, host, 4096, &[]);
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend.poll_v1(predecessor).unwrap(),
            BackendPollV1::Succeeded
        );
        let event = backend.record_event_v1(stream, predecessor).unwrap();
        let mut kernarg = [0; 16];
        kernarg[8..].copy_from_slice(&17_u64.to_le_bytes());
        let first = backend
            .submit_v1(BackendLaunchV1 {
                stream,
                kernel,
                explicit_kernarg: &kernarg,
                bindings: &[BackendBindingV1 {
                    region: BackendMemoryRegionV1 {
                        allocation: host,
                        access: if write {
                            RuntimeAccessV1::Write
                        } else {
                            RuntimeAccessV1::Read
                        },
                        byte_offset: 0,
                        byte_len: 4096,
                    },
                    kernarg_byte_offset: 0,
                }],
                dependencies: &[event],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            })
            .unwrap();
        let first_event = backend.record_event_v1(stream, first).unwrap();
        let trailing = submit_scripted_read_v1(
            &mut backend,
            stream,
            kernel,
            host,
            4096,
            &[event, first_event],
        );
        let recipe = Arc::clone(&backend.pending_compute[&first].launch);
        let trailing_recipe = pending_facts(&backend.pending_compute[&trailing]);
        let primary = (lane != 0).then(|| format!("{:?}", backend.active.as_ref().unwrap()));
        Self {
            backend,
            lane,
            stream,
            module,
            host,
            predecessor,
            first,
            trailing,
            recipe,
            trailing_recipe,
            primary,
        }
    }

    fn assert_handoff(&self) {
        let b = &self.backend;
        assert_eq!(b.selected_compute_lane, 0);
        assert_eq!(b.active_compute_lane_v1(self.first), Some(self.lane));
        let active = b.active_compute_submission_v1(self.first).unwrap();
        assert!(Arc::ptr_eq(
            active.ordinary_recipe.as_ref().unwrap(),
            &self.recipe
        ));
        assert_eq!(active.stream, self.stream);
        assert_eq!(active.kernel, self.recipe.kernel);
        assert_eq!(active.ordered_predecessor, Some(self.predecessor));
        assert!(!active.deferred_ordered_predecessor_retain);
        assert_eq!(active.allocations, HashSet::from([self.host]));
        assert_eq!(active.resident_descriptors.len(), 1);
        let descriptor = active.resident_descriptors[0];
        assert_eq!(descriptor.allocation, self.host);
        assert_eq!(descriptor.allocation_offset, 0);
        assert_eq!(descriptor.byte_len, 4096);
        assert!(!descriptor.device_may_have_modified);
        assert_eq!(
            active.writebacks.len(),
            usize::from(self.recipe.bindings[0].region.access != RuntimeAccessV1::Read)
        );
        assert!(!b.pending_compute.contains_key(&self.first));
        assert!(!b.submissions.contains_key(&self.first));
        assert_eq!(b.pending_compute_streams[&self.stream], [self.trailing]);
        assert_eq!(
            pending_facts(&b.pending_compute[&self.trailing]),
            self.trailing_recipe
        );
        assert_eq!(b.compute_dependency_retain_counts[&self.predecessor], 1);
        assert_eq!(b.compute_dependency_retain_counts[&self.first], 1);
        assert_eq!(b.event_submission_retain_counts[&self.predecessor], 1);
        assert_eq!(b.event_submission_retain_counts[&self.first], 1);
        assert_eq!(b.compute_module_retain_counts[&self.module], 2 + self.lane);
        assert_eq!(
            b.allocation_custody[&self.host].owner_counts
                [RuntimeAllocationCustodyKindV1::Compute.index()],
            2
        );
        assert_eq!(b.compute_completion_reservations, 2 + self.lane);
        assert_eq!(
            b.allocation_custody[&self.host]
                .owners
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![
                RuntimeAllocationCustodyOwnerV1 {
                    submission: self.first,
                    stream: self.stream,
                    kind: RuntimeAllocationCustodyKindV1::Compute
                },
                RuntimeAllocationCustodyOwnerV1 {
                    submission: self.trailing,
                    stream: self.stream,
                    kind: RuntimeAllocationCustodyKindV1::Compute
                },
            ]
        );
        assert_eq!(b.stream_compute_lanes[&self.stream], self.lane);
        assert_eq!(b.stream_submission_tails[&self.stream], self.trailing);
        if let Some(primary) = &self.primary {
            assert_eq!(format!("{:?}", b.active.as_ref().unwrap()), *primary);
        }
    }

    fn published_count(&self) -> usize {
        let dispatch = self
            .backend
            .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, self.first)
            .unwrap();
        self.backend.profiler.as_ref().unwrap().recorded_events_for_test_v1().iter()
            .filter(|event| matches!(event.event, KfdRuntimeProfileEventKindV1::DispatchPublished {dispatch: id, ..} if id == dispatch)).count()
    }
}

fn inspect_fault(lane: usize, recycled: bool, fault: Fault, drop_unrepaired: bool) {
    let mut f = Fixture::new(lane, true);
    let origin = if recycled {
        MaterializedPreparationOriginV1::RecycledAttachment { generation: 7 }
    } else {
        MaterializedPreparationOriginV1::NewBinding
    };
    f.backend.scripted_materialized_preparation = Some((origin, 1));
    f.backend.scripted_materialized_publication_fault = Some(fault);
    let before = Arc::clone(&f.backend.allocations[&f.host].bytes);
    let dirty = format!("{:?}", f.backend.allocations[&f.host].native_dirty);
    let result = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(f.stream)));
    let message = match fault {
        Fault::BindingUnwind => Some("scripted ordinary binding unwind"),
        Fault::InitialObserverUnwind => Some("scripted ordinary queue observer unwind"),
        Fault::SubmitUnwind => Some("scripted ordinary submit unwind"),
        Fault::OuterUnwindAfterRetry | Fault::OuterUnwindAfterPublish => {
            Some("scripted ordinary outer lane unwind")
        }
        Fault::ProfileUnwind => Some("scripted ordinary publication profile unwind"),
        _ => None,
    };
    if let Some(message) = message {
        assert_eq!(result.unwrap_err().downcast_ref::<&str>(), Some(&message));
    } else {
        assert!(
            matches!(result, Ok(Err(RuntimeBackendFailureV1::Terminal(_)))),
            "{result:?}"
        );
    }
    f.assert_handoff();
    assert!(f.backend.terminal);
    assert_eq!(f.published_count(), 0);
    assert!(f.backend.scripted_materialized_publication_fault.is_none());
    match f
        .backend
        .active_compute_submission_v1(f.first)
        .unwrap()
        .execution
        .as_ref()
        .unwrap()
    {
        ActiveComputeExecutionV1::MaterializedBinding(root) => {
            assert_eq!(root.origin, origin);
            assert!(matches!(root.profile.bindings, Some(Ok(_))));
            assert_eq!(
                root.scripted.as_ref().unwrap().0[0].bytes.as_ptr(),
                before.as_ptr()
            );
            assert!(match fault {
                Fault::BindingRejected | Fault::BindingUnwind | Fault::InitialObserverUnwind =>
                    matches!(root.submission, MaterializedFirstSubmissionV1::Unattempted),
                Fault::SubmitRejected | Fault::SubmitTerminal | Fault::SubmitUnwind =>
                    matches!(root.submission, MaterializedFirstSubmissionV1::NativeOwned),
                Fault::OuterErrorAfterRetry | Fault::OuterUnwindAfterRetry =>
                    matches!(root.submission, MaterializedFirstSubmissionV1::Retryable),
                Fault::OuterErrorAfterPublish | Fault::OuterUnwindAfterPublish => matches!(
                    root.submission,
                    MaterializedFirstSubmissionV1::ScriptedPublished
                ),
                Fault::ProfileUnwind => false,
            });
        }
        ActiveComputeExecutionV1::ScriptedMaterialized => assert_eq!(fault, Fault::ProfileUnwind),
        _ => panic!("no retry or completion may be invented after a first-submit failure"),
    }
    for result in [
        f.backend.poll_v1(f.first).map(|_| ()),
        f.backend.cancel_v1(f.first).map(|_| ()),
        f.backend.shutdown_native_v1(),
    ] {
        assert!(matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))));
    }
    f.assert_handoff();
    assert!(Arc::ptr_eq(&f.backend.allocations[&f.host].bytes, &before));
    assert_eq!(
        format!("{:?}", f.backend.allocations[&f.host].native_dirty),
        dirty
    );
    assert_eq!(
        f.backend.scripted_sdma.as_ref().unwrap().unexpected_drops(),
        0
    );
    if drop_unrepaired {
        eprintln!("ordinary first-submit custody inspected; dropping unrepaired backend");
        drop(ManuallyDrop::into_inner(f.backend));
        panic!("unrepaired ordinary backend Drop returned");
    }
    discard_scripted_fixture(f.backend);
}

#[test]
fn ordinary_initial_publication_faults_retain_exact_indexed_custody() {
    for lane in 0..2 {
        for recycled in [false, true] {
            for fault in FAULTS {
                inspect_fault(lane, recycled, fault, false);
            }
        }
    }
}

#[test]
fn ordinary_initial_publication_unrepaired_drop_aborts() {
    const CHILD: &str = "FE2O3_TEST_ORDINARY_BINDING_DROP";
    const TEST: &str = "kfd_backend::tests::materialized_publication_tests::ordinary_initial_publication_unrepaired_drop_aborts";
    if let Ok(case) = std::env::var(CHILD) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        let case: usize = case.parse().unwrap();
        inspect_fault(
            case / FAULTS.len(),
            case % 2 != 0,
            FAULTS[case % FAULTS.len()],
            true,
        );
        unreachable!();
    }
    for case in 0..2 * FAULTS.len() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD, case.to_string())
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.signal(), Some(6), "case {case}: {stderr}");
        assert!(
            stderr.contains("ordinary first-submit custody inspected; dropping unrepaired backend"),
            "case {case}: {stderr}"
        );
    }
}

#[test]
fn ordinary_initial_publication_success_and_retry_share_the_indexed_handoff() {
    for lane in 0..2 {
        for retry in [false, true] {
            let mut f = Fixture::new(lane, false);
            if retry {
                f.backend.scripted_materialized_preparation =
                    Some((MaterializedPreparationOriginV1::NewBinding, 1));
            }
            f.backend.flush_stream_v1(f.stream).unwrap();
            f.assert_handoff();
            assert!(!f.backend.terminal);
            match f
                .backend
                .active_compute_submission_v1(f.first)
                .unwrap()
                .execution
            {
                Some(ActiveComputeExecutionV1::MaterializedPrepared(_)) => assert!(retry),
                Some(ActiveComputeExecutionV1::ScriptedMaterialized) => assert!(!retry),
                _ => panic!("healthy first submit must install its exact confirmed outcome"),
            }
            assert_eq!(f.published_count(), usize::from(!retry));
            discard_scripted_fixture(f.backend);
        }
    }
}

#[test]
fn attached_metadata_is_retained_through_overwrite_and_outer_lane_failure() {
    for count in [1, 3] {
        for fail_after in 0..=count + 1 {
            for unwind in [false, true] {
                let descriptors: Vec<_> = (0..count)
                    .map(|index| ResidentDataDescriptorV1 {
                        allocation: 11 + index as u64,
                        kind: RuntimeMemoryKindV1::HostVisible,
                        alignment: 8,
                        allocation_offset: 0,
                        byte_len: 4096,
                        host_content_sha256: Some([index as u8; 32]),
                        device_may_have_modified: false,
                    })
                    .collect();
                let current: Vec<_> = descriptors
                    .iter()
                    .map(|prior| ResidentDataDescriptorV1 {
                        host_content_sha256: Some([0x7e; 32]),
                        ..*prior
                    })
                    .collect();
                let mut recycled = Some(RecycledDispatchV1 {
                    kernel: 17,
                    dispatch_shape_sha256: [0x31; 32],
                    descriptors: descriptors.clone(),
                });
                let old_pointer = recycled.as_ref().unwrap().descriptors.as_ptr();
                let current_pointer = current.as_ptr();
                let mut root = MaterializedBindingV1::new(PersistentPublicationProfileV1 {
                    launch: KfdProfileLaunchV1 {
                        grid: [64, 1, 1],
                        workgroup: [64, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    bindings: None,
                    semantic_contract: None,
                });
                let mut writes = 0;
                let result = catch_unwind(AssertUnwindSafe(|| {
                    with_recycled_materialized_metadata_v1(&mut recycled, |prior| {
                        assert_eq!(prior.descriptors.as_ptr(), old_pointer);
                        root.origin =
                            MaterializedPreparationOriginV1::RecycledAttachment { generation: 7 };
                        // Positions 0..count model overwrite prefixes; count models
                        // outer lane close. No native bytes or receipts are fabricated.
                        for position in 0..=count {
                            if position == fail_after {
                                if unwind {
                                    panic!("scripted recycled metadata unwind");
                                }
                                return Err(());
                            }
                            if position < count {
                                writes += 1;
                            }
                        }
                        Ok(())
                    })
                }));
                assert_eq!(writes, fail_after.min(count));
                assert_eq!(
                    root.origin,
                    MaterializedPreparationOriginV1::RecycledAttachment { generation: 7 }
                );
                assert!(matches!(
                    root.submission,
                    MaterializedFirstSubmissionV1::Unattempted
                ));
                assert_eq!(current.as_ptr(), current_pointer);
                assert!(
                    current
                        .iter()
                        .all(|descriptor| descriptor.host_content_sha256 == Some([0x7e; 32]))
                );
                if fail_after <= count {
                    if unwind {
                        assert!(result.is_err());
                    } else {
                        assert!(matches!(result, Ok(Err(()))));
                    }
                    let retained = recycled.as_ref().unwrap();
                    assert_eq!(retained.kernel, 17);
                    assert_eq!(retained.dispatch_shape_sha256, [0x31; 32]);
                    assert_eq!(retained.descriptors.as_ptr(), old_pointer);
                    assert_eq!(retained.descriptors, descriptors);
                } else {
                    assert!(matches!(result, Ok(Ok(()))));
                    assert!(recycled.is_none());
                }
            }
        }
    }
}
