//! Public runtime handoffs with opaque lower receipts and CPU signals, not GPU execution.

use super::super::ordinary_queue_io::CpuOrdinaryQueueV1;
use super::*;
use fe2o3_kfd::{CpuDispatchIdentityV1, CpuFixedDispatchFixtureV1};

#[path = "cpu_completion_receipt_tests.rs"]
mod completion;

#[path = "cpu_source_event_tests.rs"]
mod source_events;

struct Fixture {
    backend: KfdRuntimeBackendV1,
    streams: [u64; 2],
    hosts: [u64; 2],
    module: u64,
    kernel: u64,
    lane: usize,
    blocker: Option<u64>,
    predecessor: u64,
    event: u64,
    ids: Vec<u64>,
    primary_snapshot: Option<fe2o3_kfd::CpuLaneSnapshotV1>,
    primary_owner: Option<String>,
}

fn launch(
    backend: &mut KfdRuntimeBackendV1,
    stream: u64,
    kernel: u64,
    host: u64,
    dependencies: &[u64],
) -> u64 {
    try_launch(
        backend,
        stream,
        kernel,
        host,
        dependencies,
        RuntimeAccessV1::Read,
    )
    .unwrap()
}

fn try_launch(
    backend: &mut KfdRuntimeBackendV1,
    stream: u64,
    kernel: u64,
    host: u64,
    dependencies: &[u64],
    access: RuntimeAccessV1,
) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
    let mut kernarg = [0; 16];
    kernarg[8..].copy_from_slice(&13_u64.to_le_bytes());
    backend.submit_v1(BackendLaunchV1 {
        stream,
        kernel,
        explicit_kernarg: &kernarg,
        bindings: &[BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: host,
                access,
                byte_offset: 0,
                byte_len: 4096,
            },
            kernarg_byte_offset: 0,
        }],
        dependencies,
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    })
}

fn owner(backend: &KfdRuntimeBackendV1, lane: usize, id: u64) -> &ActiveSubmissionV1 {
    let (active, pipeline) = if lane == 0 {
        (&backend.active, &backend.compute_pipeline)
    } else {
        let lane = &backend.auxiliary_compute_lanes[lane - 1];
        (&lane.active, &lane.pipeline)
    };
    active
        .iter()
        .chain(pipeline.iter())
        .find(|active| active.id == id)
        .unwrap()
}

fn complete(backend: &mut KfdRuntimeBackendV1, lane: usize, id: u64) {
    backend.with_compute_lane_state_v1(lane, |backend| {
        let active = backend
            .active
            .iter()
            .chain(backend.compute_pipeline.iter())
            .find(|active| active.id == id)
            .unwrap();
        let Some(ActiveComputeExecutionV1::Materialized(
            MaterializedCompletionReceiptV1::Published(batch),
        )) = active.execution.as_ref()
        else {
            panic!("expected real published receipt");
        };
        let handle = backend.native_compute_lanes[lane].unwrap();
        backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |selected| {
                selected.complete_signal(batch).unwrap();
            })
            .unwrap();
    });
    assert_eq!(backend.selected_compute_lane, 0);
}

impl Fixture {
    fn new(lane: usize) -> Self {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend
            .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0xc4; 32], 256).unwrap())
            .unwrap();
        let streams = std::array::from_fn(|_| backend.create_stream_v1(7).unwrap());
        let hosts = std::array::from_fn(|_| {
            backend
                .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4096, 8)
                .unwrap()
        });
        for host in hosts {
            let record = backend.allocations.get_mut(&host).unwrap();
            assert!(matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Synthetic
            ));
            record.sdma_backed = true;
            record.sdma_initialized = true;
        }
        let module = backend
            .load_module_v1(7, &synthetic_cov6::module())
            .unwrap();
        let kernel = backend
            .resolve_kernel_v1(module, "vecadd", [7; 32])
            .unwrap();
        let fixture = CpuFixedDispatchFixtureV1::new().unwrap();
        backend.native_compute_lanes[0] = Some(fixture.primary_lane());
        backend.native_compute_lanes[1] = Some(fixture.auxiliary_lane());
        backend.cpu_queue = Some(Box::new(CpuOrdinaryQueueV1 {
            fixture,
            next_outer_fault: None,
            lane_control: Default::default(),
            before_outer_fault: None,
        }));
        backend.native_available = true;
        backend.require_cpu_provider_v1().unwrap();
        let predecessor = launch(&mut backend, streams[0], kernel, hosts[0], &[]);
        complete(&mut backend, 0, predecessor);
        assert_eq!(
            backend.poll_v1(predecessor).unwrap(),
            BackendPollV1::Succeeded
        );
        let event = backend.record_event_v1(streams[0], predecessor).unwrap();
        // Discard only the checked, clean CPU metadata cache before target setup.
        backend.release_compute_lane_cache_v1(0).unwrap();
        let blocker = (lane == 1).then(|| {
            let id = launch(&mut backend, streams[0], kernel, hosts[0], &[]);
            backend.flush_stream_v1(streams[0]).unwrap();
            assert_eq!(backend.active.as_ref().unwrap().id, id);
            id
        });
        let primary_snapshot = blocker.map(|_| {
            backend
                .cpu_queue
                .as_ref()
                .unwrap()
                .fixture
                .snapshots()
                .into_iter()
                .next()
                .unwrap()
        });
        let primary_owner = blocker.map(|_| format!("{:?}", backend.active));
        Self {
            backend,
            streams,
            hosts,
            module,
            kernel,
            lane,
            blocker,
            predecessor,
            event,
            ids: vec![predecessor],
            primary_snapshot,
            primary_owner,
        }
    }

    fn submit(&mut self) -> u64 {
        let id = launch(
            &mut self.backend,
            self.streams[self.lane],
            self.kernel,
            self.hosts[self.lane],
            &[self.event],
        );
        self.ids.push(id);
        id
    }

    fn flush(&mut self) {
        self.backend
            .flush_stream_v1(self.streams[self.lane])
            .unwrap();
        assert_eq!(self.backend.selected_compute_lane, 0);
    }

    fn identity(&mut self, id: u64) -> CpuDispatchIdentityV1 {
        self.backend
            .with_compute_lane_state_v1(self.lane, |backend| {
                let active = backend
                    .active
                    .iter()
                    .chain(backend.compute_pipeline.iter())
                    .find(|active| active.id == id)
                    .unwrap();
                let Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Published(batch),
                )) = active.execution.as_ref()
                else {
                    panic!("expected real published receipt");
                };
                backend
                    .cpu_queue
                    .as_mut()
                    .unwrap()
                    .fixture
                    .with_lane(
                        backend.native_compute_lanes[self.lane].unwrap(),
                        |selected| selected.identity(batch).unwrap(),
                    )
                    .unwrap()
            })
    }

    fn live_epochs(&mut self) -> usize {
        self.backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(
                self.backend.native_compute_lanes[self.lane].unwrap(),
                |lane| lane.live_epochs(),
            )
            .unwrap()
    }

    fn returned_identity(&self) -> CpuDispatchIdentityV1 {
        *self
            .backend
            .cpu_queue
            .as_ref()
            .unwrap()
            .lane_control
            .last_submitted_identity
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
    }

    fn complete(&mut self, id: u64) {
        complete(&mut self.backend, self.lane, id);
    }

    fn publication_count(&self, id: u64) -> usize {
        let identity = self
            .backend
            .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, id)
            .unwrap();
        self.backend.profiler.as_ref().unwrap().recorded_events_for_test_v1().iter().filter(|event| {
            matches!(event.event, KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch, .. } if dispatch == identity)
        }).count()
    }

    fn generation_and_capacity(&mut self) -> (u64, usize) {
        let handle = self.backend.native_compute_lanes[self.lane].unwrap();
        self.backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| {
                (lane.next_generation(), lane.available_signals())
            })
            .unwrap()
    }

    fn cleanup(mut self) {
        if let Some(blocker) = self.blocker {
            assert_eq!(
                self.primary_owner.as_ref().unwrap(),
                &format!("{:?}", self.backend.active)
            );
            assert_eq!(
                self.primary_snapshot.as_ref().unwrap(),
                &self.backend.cpu_queue.as_ref().unwrap().fixture.snapshots()[0]
            );
            complete(&mut self.backend, 0, blocker);
            assert_eq!(
                self.backend.poll_v1(blocker).unwrap(),
                BackendPollV1::Succeeded
            );
            self.ids.push(blocker);
        }
        self.backend
            .cpu_queue
            .as_ref()
            .unwrap()
            .fixture
            .ensure_clean()
            .unwrap();
        self.backend.release_event_v1(self.event).unwrap();
        for id in self.ids.into_iter().rev() {
            assert_eq!(self.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
            self.backend.release_submission_v1(id).unwrap();
        }
        for host in self.hosts {
            self.backend.release_allocation_v1(host).unwrap();
        }
        self.backend.unload_module_v1(self.module).unwrap();
        for stream in self.streams {
            self.backend.destroy_stream_v1(stream).unwrap();
        }
        self.backend.shutdown_native_v1().unwrap();
        self.backend.shutdown_native_v1().unwrap();
        assert!(self.backend.cpu_queue.is_none());
        assert!(
            self.backend
                .native_compute_lanes
                .iter()
                .all(Option::is_none)
        );
        assert!(!self.backend.scripted_drop_disarmed);
    }
}

#[test]
fn cpu_receipts_public_initial_handoff_and_pending_identity() {
    for lane in 0..2 {
        let mut f = Fixture::new(lane);
        let id = f.submit();
        let recipe = Arc::clone(&f.backend.pending_compute[&id].launch);
        let retained = f.backend.compute_module_retain_counts[&f.module];
        let reservations = f.backend.compute_completion_reservations;
        let allocations = format!("{:?}", f.backend.allocation_custody);
        f.flush();
        assert!(!f.backend.pending_compute.contains_key(&id));
        assert!(
            !f.backend
                .pending_compute_streams
                .contains_key(&f.streams[lane])
        );
        assert!(Arc::ptr_eq(
            owner(&f.backend, lane, id)
                .ordinary_recipe
                .as_ref()
                .unwrap(),
            &recipe
        ));
        assert_eq!(f.backend.compute_module_retain_counts[&f.module], retained);
        assert!(
            !f.backend
                .compute_dependency_retain_counts
                .contains_key(&f.predecessor)
        );
        assert_eq!(f.backend.event_submission_retain_counts[&f.predecessor], 1);
        assert_eq!(f.backend.compute_completion_reservations, reservations);
        assert_eq!(format!("{:?}", f.backend.allocation_custody), allocations);
        assert_eq!(f.publication_count(id), 1);
        let identity = f.identity(id);
        for _ in 0..3 {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            assert_eq!(f.identity(id), identity);
            assert_eq!(f.live_epochs(), 1);
            assert_eq!(f.publication_count(id), 1);
        }
        f.complete(id);
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        assert_eq!(f.live_epochs(), 0);
        f.cleanup();
    }
}

#[test]
fn cpu_receipts_initial_capacity_retry_keeps_actual_prepared_custody() {
    for lane in 0..2 {
        let mut f = Fixture::new(lane);
        let handle = f.backend.native_compute_lanes[lane].unwrap();
        f.backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| lane.saturate_signals().unwrap())
            .unwrap();
        let id = f.submit();
        let recipe = Arc::clone(&f.backend.pending_compute[&id].launch);
        let (generation, capacity) = f.generation_and_capacity();
        assert_eq!(capacity, 0);
        f.flush();
        assert_eq!(f.generation_and_capacity(), (generation + 1, 0));
        for retry in 0..3 {
            assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
            let active = owner(&f.backend, lane, id);
            assert!(matches!(
                active.execution,
                Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
            ));
            assert!(Arc::ptr_eq(
                active.ordinary_recipe.as_ref().unwrap(),
                &recipe
            ));
            assert!(!f.backend.pending_compute.contains_key(&id));
            assert_eq!(f.live_epochs(), 0);
            assert_eq!(f.generation_and_capacity(), (generation + retry + 2, 0));
            assert_eq!(f.publication_count(id), 0);
        }
        f.backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| lane.drain_saturation().unwrap())
            .unwrap();
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
        let _identity = f.identity(id);
        assert_eq!(f.live_epochs(), 1);
        assert_eq!(f.publication_count(id), 1);
        f.complete(id);
        assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
        f.cleanup();
    }
}

#[test]
fn cpu_receipts_ordered_physical_retirement_preserves_logical_frontier() {
    for lane in 0..2 {
        for retire_last in [false, true] {
            let mut f = Fixture::new(lane);
            let a = f.submit();
            f.flush();
            let a_receipt = f.returned_identity();
            assert_eq!(f.identity(a), a_receipt);
            let b = f.submit();
            let c = f.submit();
            assert_eq!(f.backend.pending_compute_streams[&f.streams[lane]], [b, c]);
            assert_eq!(
                f.backend.compute_dependency_retain_counts[&f.predecessor],
                2
            );
            f.flush();
            let b_receipt = f.returned_identity();
            assert_eq!(f.identity(b), b_receipt);
            assert_eq!(f.identity(a), a_receipt);
            assert_eq!(f.backend.pending_compute_streams[&f.streams[lane]], [c]);
            assert_eq!(
                f.backend.compute_dependency_retain_counts[&f.predecessor],
                1
            );
            f.flush();
            let c_receipt = f.returned_identity();
            assert_eq!(f.identity(c), c_receipt);
            assert_eq!(f.identity(a), a_receipt);
            assert_eq!(f.identity(b), b_receipt);
            assert!(
                !f.backend
                    .compute_dependency_retain_counts
                    .contains_key(&f.predecessor)
            );
            assert!(
                !f.backend
                    .pending_compute_streams
                    .contains_key(&f.streams[lane])
            );
            let identities = [f.identity(a), f.identity(b), f.identity(c)];
            assert_ne!(identities[0], identities[1]);
            assert_ne!(identities[1], identities[2]);
            assert_ne!(identities[0], identities[2]);
            assert_eq!(f.live_epochs(), 3);
            let reservations = f.backend.compute_completion_reservations;
            let retains = f.backend.compute_module_retain_counts[&f.module];
            let allocations = format!("{:?}", f.backend.allocation_custody);
            f.complete(b);
            assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Pending);
            assert!(matches!(
                owner(&f.backend, lane, b).execution,
                Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Retired(_)
                ))
            ));
            assert_eq!(f.live_epochs(), 2);
            if retire_last {
                f.complete(c);
                assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
                assert_eq!(f.live_epochs(), 1);
            }
            assert_eq!(f.backend.compute_completion_reservations, reservations);
            assert_eq!(f.backend.compute_module_retain_counts[&f.module], retains);
            assert_eq!(format!("{:?}", f.backend.allocation_custody), allocations);
            assert_eq!(f.backend.compute_dependency_retain_counts[&a], 1);
            assert_eq!(f.backend.compute_dependency_retain_counts[&b], 1);
            f.complete(a);
            assert_eq!(f.backend.poll_v1(a).unwrap(), BackendPollV1::Succeeded);
            assert_eq!(f.backend.poll_v1(b).unwrap(), BackendPollV1::Succeeded);
            if !retire_last {
                assert_eq!(f.identity(c), identities[2]);
                assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Pending);
                f.complete(c);
            }
            assert_eq!(f.backend.poll_v1(c).unwrap(), BackendPollV1::Succeeded);
            assert!(!f.backend.compute_dependency_retain_counts.contains_key(&a));
            assert!(!f.backend.compute_dependency_retain_counts.contains_key(&b));
            let completed: Vec<_> = f
                .backend
                .profiler
                .as_ref()
                .unwrap()
                .recorded_events_for_test_v1()
                .iter()
                .filter_map(|event| {
                    let KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } =
                        event.event
                    else {
                        return None;
                    };
                    [a, b, c].into_iter().find(|id| {
                        f.backend
                            .profile_resource_v1(KfdProfileResourceKindV1::Dispatch, *id)
                            == Some(dispatch)
                    })
                })
                .collect();
            assert_eq!(completed, [a, b, c]);
            f.cleanup();
        }
    }
}

#[test]
fn cpu_receipts_outer_fault_retains_exact_indexed_publication() {
    use super::super::materialized_submission_attempt::MaterializedSubmissionAttemptV1 as Attempt;
    use super::super::ordinary_queue_io::CpuOuterFaultV1 as Fault;
    use super::initial_publication_tests::pending_facts;
    use std::mem::ManuallyDrop;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const CHILD: &str = "FE2O3_RUNTIME_CPU_OUTER_CHILD";
    if let Ok(case) = std::env::var(CHILD) {
        let case: usize = case.parse().unwrap();
        let lane = case % 2;
        let ordered = case & 2 != 0;
        let fault = if case / 4 == 0 {
            Fault::Error
        } else {
            Fault::Unwind
        };
        let mut f = ManuallyDrop::new(Fixture::new(lane));
        let predecessor = ordered.then(|| {
            let id = f.submit();
            f.flush();
            (id, f.identity(id))
        });
        let target = f.submit();
        let recipe = Arc::clone(&f.backend.pending_compute[&target].launch);
        let trailing = f.submit();
        let trailing_facts = pending_facts(&f.backend.pending_compute[&trailing]);
        let reservations = f.backend.compute_completion_reservations;
        let retained = f.backend.compute_module_retain_counts[&f.module];
        let allocations = format!("{:?}", f.backend.allocation_custody);
        let heads_before = if lane == 0 {
            f.backend.compute_pipeline.publication_heads_v1()
        } else {
            f.backend.auxiliary_compute_lanes[lane - 1]
                .pipeline
                .publication_heads_v1()
        };
        let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
        f.backend.cpu_queue.as_mut().unwrap().next_outer_fault = Some((
            super::super::ordinary_queue_io::CpuIoOperationV1::Submit,
            fault,
        ));
        let stream = f.streams[lane];
        let outcome = catch_unwind(AssertUnwindSafe(|| f.backend.flush_stream_v1(stream)));
        match fault {
            Fault::Error => assert!(matches!(
                outcome,
                Ok(Err(RuntimeBackendFailureV1::Terminal(_)))
            )),
            Fault::Unwind => assert_eq!(
                *outcome.unwrap_err().downcast::<Fault>().unwrap(),
                Fault::Unwind
            ),
        }
        assert!(f.backend.terminal);
        assert_eq!(f.backend.selected_compute_lane, 0);
        assert!(!f.backend.pending_compute.contains_key(&target));
        assert_eq!(f.backend.pending_compute_streams[&stream], [trailing]);
        assert_eq!(
            pending_facts(&f.backend.pending_compute[&trailing]),
            trailing_facts
        );
        assert_eq!(f.backend.compute_completion_reservations, reservations);
        assert_eq!(f.backend.compute_module_retain_counts[&f.module], retained);
        assert_eq!(format!("{:?}", f.backend.allocation_custody), allocations);
        assert_eq!(
            f.backend.compute_dependency_retain_counts[&f.predecessor],
            1
        );
        assert_eq!(f.backend.compute_dependency_retain_counts[&target], 1);
        assert_eq!(f.publication_count(target), 0);
        assert!(!f.backend.submissions.contains_key(&target));
        let active = owner(&f.backend, lane, target);
        assert!(Arc::ptr_eq(
            active.ordinary_recipe.as_ref().unwrap(),
            &recipe
        ));
        let attempt = match active.execution.as_ref().unwrap() {
            ActiveComputeExecutionV1::MaterializedBinding(root) if !ordered => &root.submission,
            ActiveComputeExecutionV1::MaterializedSuccessorPublication(root) if ordered => {
                &root.attempt
            }
            _ => panic!("publication root must remain indexed"),
        };
        let Attempt::Published(batch) = attempt else {
            panic!("real batch must remain indexed");
        };
        let cpu = f.backend.cpu_queue.as_ref().unwrap();
        assert!(cpu.fixture.is_terminal());
        assert!(
            cpu.lane_control
                .last_submitted_identity
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .matches_published(batch)
        );
        let after = cpu.fixture.snapshots();
        assert!(after[lane].same_custody(cpu.before_outer_fault.as_ref().unwrap()));
        assert!(after[1 - lane].same_custody(&before[1 - lane]));
        if let Some((id, identity)) = predecessor {
            let Some(ActiveComputeExecutionV1::Materialized(
                MaterializedCompletionReceiptV1::Published(batch),
            )) = owner(&f.backend, lane, id).execution.as_ref()
            else {
                panic!("predecessor receipt");
            };
            assert!(identity.matches_published(batch));
            assert!(active.deferred_ordered_predecessor_retain);
            assert_eq!(f.backend.compute_dependency_retain_counts[&id], 1);
            let pipeline = if lane == 0 {
                &f.backend.compute_pipeline
            } else {
                &f.backend.auxiliary_compute_lanes[lane - 1].pipeline
            };
            let identity = pipeline.identity_for_submission_v1(target).unwrap();
            assert_eq!(
                pipeline.publication_heads_v1(),
                (heads_before.0, heads_before.1, Some(identity))
            );
            assert!(pipeline.checked_frontier_v1().is_err());
            assert_eq!(
                pipeline
                    .entry_v1(pipeline.identity_for_submission_v1(target).unwrap())
                    .unwrap()
                    .phase,
                RuntimeComputePipelinePhaseV1::Quarantined
            );
        }
        let active_before = format!("{:?}", owner(&f.backend, lane, target));
        assert!(matches!(
            f.backend.poll_v1(target),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            f.backend.cancel_v1(target),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(matches!(
            f.backend.shutdown_native_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        let handle = f.backend.native_compute_lanes[lane].unwrap();
        assert!(
            f.backend
                .cpu_queue
                .as_mut()
                .unwrap()
                .fixture
                .with_lane(handle, |_| panic!("terminal callback ran"))
                .is_err()
        );
        assert_eq!(
            f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
            after
        );
        assert_eq!(
            format!("{:?}", owner(&f.backend, lane, target)),
            active_before
        );
        assert_eq!(
            pending_facts(&f.backend.pending_compute[&trailing]),
            trailing_facts
        );
        assert_eq!(f.backend.compute_completion_reservations, reservations);
        assert_eq!(f.backend.compute_module_retain_counts[&f.module], retained);
        assert_eq!(format!("{:?}", f.backend.allocation_custody), allocations);
        if lane == 1 {
            assert_eq!(
                f.primary_owner.as_ref().unwrap(),
                &format!("{:?}", f.backend.active)
            );
        }
        println!("CPU_OUTER_CHILD_VERIFIED_{case}");
        return;
    }
    for case in 0..8 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "kfd_backend::tests::cpu_receipt_tests::cpu_receipts_outer_fault_retains_exact_indexed_publication", "--nocapture"])
            .env(CHILD, case.to_string()).output().unwrap();
        assert!(
            output.status.success(),
            "case {case}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout)
                .contains(&format!("CPU_OUTER_CHILD_VERIFIED_{case}"))
        );
    }
}

#[test]
fn cpu_receipts_cache_refusals_preserve_metadata_and_lower_custody() {
    let mut f = Fixture::new(0);
    let id = f.submit();
    f.flush();
    f.complete(id);
    assert_eq!(f.backend.poll_v1(id).unwrap(), BackendPollV1::Succeeded);
    for fault in 0..4 {
        match fault {
            0 => {
                f.backend.recycled_dispatch.as_mut().unwrap().descriptors[0]
                    .device_may_have_modified = true
            }
            1 => f.backend.native_dirty_extents = 1,
            2 => {
                f.backend.resident_data = Some(ResidentDataRosterV1 {
                    descriptors: Vec::new(),
                    data: Vec::new(),
                })
            }
            3 => f.backend.scripted_sdma = Some(ScriptedSdmaDriverV1::new([])),
            _ => unreachable!(),
        }
        let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
        let descriptors = format!(
            "{:?}",
            f.backend.recycled_dispatch.as_ref().unwrap().descriptors
        );
        assert!(f.backend.detach_cpu_recycled_dispatch_v1().is_err());
        assert_eq!(
            f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
            before
        );
        assert_eq!(
            format!(
                "{:?}",
                f.backend.recycled_dispatch.as_ref().unwrap().descriptors
            ),
            descriptors
        );
        assert!(!f.backend.terminal);
        // Restore only the deliberately corrupted test metadata, never lower ownership.
        match fault {
            0 => {
                f.backend.recycled_dispatch.as_mut().unwrap().descriptors[0]
                    .device_may_have_modified = false
            }
            1 => f.backend.native_dirty_extents = 0,
            2 => assert!(f.backend.resident_data.take().unwrap().data.is_empty()),
            3 => {
                f.backend.scripted_sdma = None;
            }
            _ => unreachable!(),
        }
    }
    let handle = f.backend.native_compute_lanes[0].unwrap();
    let (batch, identity) = f
        .backend
        .cpu_queue
        .as_mut()
        .unwrap()
        .fixture
        .with_lane(handle, |lane| {
            let batch = lane.submit_pinned().unwrap();
            let identity = lane.identity(&batch).unwrap();
            (batch, identity)
        })
        .unwrap();
    let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
    assert!(f.backend.detach_cpu_recycled_dispatch_v1().is_err());
    assert!(f.backend.recycled_dispatch.is_some());
    assert_eq!(
        f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
        before
    );
    f.backend
        .cpu_queue
        .as_mut()
        .unwrap()
        .fixture
        .with_lane(handle, |lane| {
            lane.complete_signal(&batch).unwrap();
            let Gfx942DispatchPollV1::Ready(completed) = lane.poll(batch).unwrap() else {
                panic!("completed CPU signal");
            };
            lane.release_pin(identity).unwrap();
            lane.recycle(completed).unwrap();
        })
        .unwrap();
    f.cleanup();
}

#[test]
fn cpu_receipts_drop_aborts_for_unindexed_lower_custody() {
    use std::os::unix::process::ExitStatusExt;
    const CHILD: &str = "FE2O3_RUNTIME_CPU_DROP_CHILD";
    if std::env::var_os(CHILD).is_some() {
        rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
            .unwrap();
        let mut f = Fixture::new(0);
        let handle = f.backend.native_compute_lanes[0].unwrap();
        let _batch = f
            .backend
            .cpu_queue
            .as_mut()
            .unwrap()
            .fixture
            .with_lane(handle, |lane| lane.submit().unwrap())
            .unwrap();
        assert!(!f.backend.any_compute_active_v1());
        assert!(f.backend.pending_compute.is_empty());
        println!("CPU_DROP_LIVE_LOWER");
        drop(f);
        panic!("Drop accepted live lower custody");
    }
    let output = std::process::Command::new("sh")
        .args(["-c", "ulimit -c 0; exec \"$@\"", "cpu-receipt-drop"])
        .arg(std::env::current_exe().unwrap())
        .args(["--exact", "kfd_backend::tests::cpu_receipt_tests::cpu_receipts_drop_aborts_for_unindexed_lower_custody", "--nocapture"])
        .env(CHILD, "1").output().unwrap();
    assert_eq!(output.status.signal(), Some(6));
    assert!(!output.status.core_dumped());
    assert!(String::from_utf8_lossy(&output.stdout).contains("CPU_DROP_LIVE_LOWER"));
}

#[test]
fn cpu_receipts_reject_writes_before_accepting_custody() {
    for lane in 0..2 {
        let mut f = Fixture::new(lane);
        for access in [RuntimeAccessV1::Write, RuntimeAccessV1::ReadWrite] {
            let before = f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots();
            let next = f.backend.next_handle;
            let allocations = format!("{:?}", f.backend.allocation_custody);
            let retains = f.backend.compute_module_retain_counts.clone();
            let dependencies = f.backend.compute_dependency_retain_counts.clone();
            let reservations = f.backend.compute_completion_reservations;
            assert!(matches!(
                try_launch(
                    &mut f.backend,
                    f.streams[lane],
                    f.kernel,
                    f.hosts[lane],
                    &[f.event],
                    access
                ),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
            assert_eq!(f.backend.next_handle, next);
            assert_eq!(
                f.backend.cpu_queue.as_ref().unwrap().fixture.snapshots(),
                before
            );
            assert_eq!(format!("{:?}", f.backend.allocation_custody), allocations);
            assert_eq!(f.backend.compute_module_retain_counts, retains);
            assert_eq!(f.backend.compute_dependency_retain_counts, dependencies);
            assert_eq!(f.backend.compute_completion_reservations, reservations);
            assert!(f.backend.pending_compute.is_empty());
            assert!(!f.backend.terminal);
        }
        f.cleanup();
    }
}
