use super::*;

#[derive(Clone, Copy, Debug)]
enum Fault {
    Rejected,
    Quiescent,
    Terminal,
    Panic,
    WrongHandle,
    MissingRecord,
}

struct Rig {
    streams: HashMap<u64, usize>,
    allocations: HashMap<u64, XgmiRuntimeAllocationV1>,
    events: HashMap<u64, EventRecordV1>,
    active: HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: HashMap<u64, SubmissionRecordV1>,
    roots: HashMap<u64, Root>,
    depths: HashMap<u64, usize>,
    event_retains: HashSet<u64>,
    dependency_retains: HashSet<u64>,
    next: u64,
    sealed: bool,
    submits: usize,
    progress: Vec<u64>,
    fault: Option<Fault>,
    status: BackendPollV1,
}

fn route() -> BackendDirectedPeerRouteV1 {
    BackendDirectedPeerRouteV1 {
        stream: 11,
        source_device: 7,
        destination_device: 9,
        source: BackendMemoryRegionV1 {
            allocation: 20,
            access: RuntimeAccessV1::Read,
            byte_offset: 8,
            byte_len: 32,
        },
        destination: BackendMemoryRegionV1 {
            allocation: 21,
            access: RuntimeAccessV1::Write,
            byte_offset: 16,
            byte_len: 32,
        },
    }
}

fn dependency(event: u64, producer_submission: u64) -> BackendDirectedPeerDependencyV1 {
    BackendDirectedPeerDependencyV1 {
        event,
        producer_submission,
    }
}

fn observation(id: u64, producers: &[u64]) -> BackendDirectedScalarProgressV1<'_> {
    BackendDirectedScalarProgressV1 {
        submission: id,
        route: route(),
        producer_submissions: producers,
    }
}

fn error() -> KfdRuntimeBackendErrorV1 {
    KfdRuntimeBackendErrorV1::new(
        KfdRuntimeBackendErrorKindV1::Native,
        "original scripted failure",
    )
}

impl Rig {
    fn new() -> Self {
        Self {
            streams: HashMap::from([(10, 0), (11, 1)]),
            allocations: [20, 21]
                .into_iter()
                .enumerate()
                .map(|(device, id)| {
                    (
                        id,
                        XgmiRuntimeAllocationV1 {
                            device,
                            byte_len: 1024,
                            alignment: 8,
                            authority: None,
                        },
                    )
                })
                .collect(),
            events: HashMap::new(),
            active: HashMap::new(),
            completed: HashMap::new(),
            roots: HashMap::new(),
            depths: HashMap::new(),
            event_retains: HashSet::new(),
            dependency_retains: HashSet::new(),
            next: 100,
            sealed: false,
            submits: 0,
            progress: Vec::new(),
            fault: None,
            status: BackendPollV1::Pending,
        }
    }

    fn submit(&mut self, dependencies: &[BackendDirectedPeerDependencyV1]) -> u64 {
        submit(
            self,
            BackendDirectedScalarPeerCopyV1 {
                route: route(),
                dependencies,
            },
        )
        .unwrap()
    }

    fn settle(&mut self, id: u64, status: BackendPollV1) {
        assert_ne!(status, BackendPollV1::Pending);
        let record = self.active.remove(&id).unwrap();
        self.completed.insert(
            id,
            SubmissionRecordV1 {
                stream: record.stream,
                status,
                profile_dispatch_published: false,
            },
        );
    }

    fn seed(&mut self, event: u64) -> u64 {
        let id = self.submit(&[]);
        self.events.insert(event, EventRecordV1 { submission: id });
        id
    }

    fn inject(&self) -> Result<(), Failure> {
        match self.fault {
            Some(Fault::Rejected) => Err(RuntimeBackendFailureV1::Rejected(error())),
            Some(Fault::Quiescent) => Err(RuntimeBackendFailureV1::Quiescent(error())),
            Some(Fault::Terminal) => Err(RuntimeBackendFailureV1::Terminal(error())),
            Some(Fault::Panic) => std::panic::panic_any(0x25_u64),
            _ => Ok(()),
        }
    }
}

impl Driver for Rig {
    fn require_live(&self) -> Result<(), Failure> {
        if self.sealed {
            Err(RuntimeBackendFailureV1::Terminal(error()))
        } else {
            Ok(())
        }
    }

    fn view(&self) -> View<'_> {
        View {
            devices: [7, 9],
            streams: &self.streams,
            allocations: &self.allocations,
            events: &self.events,
            active: &self.active,
            completed: &self.completed,
            roots: &self.roots,
        }
    }

    fn roots_mut(&mut self) -> &mut HashMap<u64, Root> {
        &mut self.roots
    }
    fn next_handle(&self) -> u64 {
        self.next
    }
    fn seal(&mut self) {
        self.sealed = true;
    }

    fn submit_scalar(
        &mut self,
        route: BackendDirectedPeerRouteV1,
        events: &[u64],
    ) -> Result<u64, Failure> {
        let root = &self.roots[&self.next];
        assert!(!root.admitted);
        assert_eq!(root.route, route);
        assert!(
            root.dependencies
                .iter()
                .map(|entry| entry.event)
                .eq(events.iter().copied())
        );
        self.submits += 1;
        self.inject()?;
        let id = self.next;
        self.next += 1;
        self.depths.insert(id, 1);
        if !matches!(self.fault, Some(Fault::MissingRecord)) {
            self.active.insert(
                id,
                XgmiRuntimeSubmissionV1 {
                    id,
                    stream: route.stream,
                    direction: usize::from(route.source_device == 9),
                    source: route.source.allocation,
                    destination: route.destination.allocation,
                    source_offset: route.source.byte_offset,
                    destination_offset: route.destination.byte_offset,
                    byte_len: route.source.byte_len as u32,
                    dependencies: collect_xgmi_dependencies_v1(&self.events, events).unwrap(),
                    dependency_cursor: 0,
                    ready_indexed: false,
                    ticket: None,
                    sequence: None,
                },
            );
        }
        Ok(if matches!(self.fault, Some(Fault::WrongHandle)) {
            id + 1
        } else {
            id
        })
    }

    fn progress_scalar(&mut self, id: u64) -> Result<BackendPollV1, Failure> {
        self.progress.push(id);
        self.inject()?;
        if let Some(record) = self.completed.get(&id) {
            return Ok(record.status);
        }
        if self.status != BackendPollV1::Pending {
            self.settle(id, self.status);
        }
        Ok(self.status)
    }

    fn retained_for_release(&self, id: u64) -> bool {
        self.event_retains.contains(&id) || self.dependency_retains.contains(&id)
    }

    fn depth_retained(&self, id: u64) -> bool {
        self.depths.contains_key(&id)
    }

    fn remove_completion(&mut self, id: u64) {
        remove_completion(&mut self.completed, &mut self.depths, &mut self.roots, id);
    }
}

fn rejected<T: std::fmt::Debug>(result: Result<T, Failure>, kind: KfdRuntimeBackendErrorKindV1) {
    assert!(
        matches!(result, Err(RuntimeBackendFailureV1::Rejected(ref error)) if error.kind() == kind),
        "{result:?}"
    );
}

#[test]
fn native_backend_explicitly_implements_directed_profile() {
    fn supports<T: RuntimeDirectedScalarPeerCopyBackendV1>() {}
    supports::<KfdNativeXgmiRuntimeBackendV1>();
}

#[test]
fn both_directions_root_exact_route_before_shared_admission() {
    for reverse in [false, true] {
        let mut rig = Rig::new();
        let mut request = route();
        if reverse {
            request.stream = 10;
            std::mem::swap(&mut request.source_device, &mut request.destination_device);
            std::mem::swap(
                &mut request.source.allocation,
                &mut request.destination.allocation,
            );
        }
        let id = submit(
            &mut rig,
            BackendDirectedScalarPeerCopyV1 {
                route: request,
                dependencies: &[],
            },
        )
        .unwrap();
        assert_eq!(id, 100);
        assert_eq!(rig.roots[&id].route, request);
        assert!(rig.roots[&id].admitted);
        assert_eq!(rig.submits, 1);
        assert_eq!(
            progress(
                &mut rig,
                BackendDirectedScalarProgressV1 {
                    submission: id,
                    route: request,
                    producer_submissions: &[],
                }
            )
            .unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(rig.progress, [id]);
    }
}

#[test]
fn active_and_completed_producers_keep_original_roster_order() {
    let mut rig = Rig::new();
    let first = rig.seed(50);
    let second = rig.seed(51);
    rig.settle(second, BackendPollV1::Succeeded);
    let dependencies = [dependency(51, second), dependency(50, first)];
    let id = rig.submit(&dependencies);
    assert_eq!(rig.roots[&id].dependencies, dependencies);
    assert_eq!(rig.active[&id].dependencies, [second, first]);
    assert_eq!(
        progress(&mut rig, observation(id, &[second, first])).unwrap(),
        BackendPollV1::Pending
    );
    rejected(
        progress(&mut rig, observation(id, &[first, second])),
        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
    );
    assert_eq!(rig.progress, [id]);
}

#[test]
fn route_substitutions_reject_without_roots_or_adapter_entry() {
    for mutation in 0..20 {
        let mut rig = Rig::new();
        let mut request = route();
        match mutation {
            0 => request.stream = 0,
            1 => request.stream = 10,
            2 => request.source_device = 0,
            3 => request.source_device = 9,
            4 => request.destination_device = 7,
            5 => request.source.allocation = 99,
            6 => request.destination.allocation = 99,
            7 => request.source.allocation = 21,
            8 => request.destination.allocation = 20,
            9 => request.source.access = RuntimeAccessV1::Write,
            10 => request.destination.access = RuntimeAccessV1::Read,
            11 => request.source.byte_offset = u64::MAX,
            12 => request.destination.byte_offset = u64::MAX,
            13 => request.source.byte_len = 0,
            14 => request.destination.byte_len = 31,
            15 => request.source.byte_offset = 1000,
            16 => request.destination.byte_offset = 1000,
            17 => {
                request.source.byte_len = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1) + 1;
                request.destination.byte_len = request.source.byte_len;
                rig.allocations.get_mut(&20).unwrap().byte_len = u64::MAX;
                rig.allocations.get_mut(&21).unwrap().byte_len = u64::MAX;
            }
            18 => rig.allocations.get_mut(&20).unwrap().device = 1,
            19 => rig.allocations.get_mut(&21).unwrap().device = 0,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                submit(
                    &mut rig,
                    BackendDirectedScalarPeerCopyV1 {
                        route: request,
                        dependencies: &[]
                    }
                ),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ),
            "mutation {mutation}"
        );
        assert!(rig.roots.is_empty());
        assert!(rig.active.is_empty());
        assert_eq!(rig.next, 100);
        assert_eq!(rig.submits, 0);
        assert!(!rig.sealed);
    }
}

#[test]
fn event_binding_alias_and_cohort_rejections_precede_adapter_entry() {
    for mutation in 0..8 {
        let mut rig = Rig::new();
        let producer = rig.seed(50);
        let other = rig.seed(51);
        let mut dependencies = vec![dependency(50, producer)];
        match mutation {
            0 => dependencies[0].event = 99,
            1 => dependencies[0].producer_submission = other,
            2 => dependencies[0].producer_submission = 0,
            3 => dependencies[0].producer_submission = rig.next,
            4 => dependencies.push(dependencies[0]),
            5 => {
                rig.events.insert(
                    52,
                    EventRecordV1 {
                        submission: producer,
                    },
                );
                dependencies.push(dependency(52, producer));
            }
            6 => {
                rig.roots.remove(&producer);
            }
            7 => {
                rig.settle(producer, BackendPollV1::Succeeded);
                rig.roots.remove(&producer);
            }
            _ => unreachable!(),
        }
        let roots = rig.roots.len();
        assert!(matches!(
            submit(
                &mut rig,
                BackendDirectedScalarPeerCopyV1 {
                    route: route(),
                    dependencies: &dependencies
                }
            ),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert_eq!(rig.submits, 2);
        assert_eq!(rig.roots.len(), roots);
        assert_eq!(rig.next, 102);
        assert!(!rig.sealed);
    }
}

#[test]
fn missing_event_producer_is_terminal_not_unsupported() {
    let mut rig = Rig::new();
    rig.events.insert(50, EventRecordV1 { submission: 1 });
    assert!(matches!(
        submit(
            &mut rig,
            BackendDirectedScalarPeerCopyV1 {
                route: route(),
                dependencies: &[dependency(50, 1)]
            }
        ),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(rig.sealed);
    assert_eq!(rig.submits, 0);
    assert!(rig.roots.is_empty());
}

#[test]
fn full_roster_bound_and_late_duplicates_are_checked() {
    let mut rig = Rig::new();
    let dependencies: Vec<_> = (0..MAX_RUNTIME_DEPENDENCIES_V1)
        .map(|index| {
            let event = 1000 + index as u64;
            dependency(event, rig.seed(event))
        })
        .collect();
    let id = rig.submit(&dependencies);
    assert_eq!(
        rig.roots[&id].dependencies.len(),
        MAX_RUNTIME_DEPENDENCIES_V1
    );
    let mut too_many = dependencies.clone();
    too_many.push(dependencies[0]);
    rejected(
        submit(
            &mut rig,
            BackendDirectedScalarPeerCopyV1 {
                route: route(),
                dependencies: &too_many,
            },
        ),
        KfdRuntimeBackendErrorKindV1::Capacity,
    );
    let mut duplicate = dependencies.clone();
    *duplicate.last_mut().unwrap() = dependencies[0];
    rejected(
        submit(
            &mut rig,
            BackendDirectedScalarPeerCopyV1 {
                route: route(),
                dependencies: &duplicate,
            },
        ),
        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
    );
    assert_eq!(rig.submits, MAX_RUNTIME_DEPENDENCIES_V1 + 1);
    assert_eq!(rig.roots.len(), MAX_RUNTIME_DEPENDENCIES_V1 + 1);
}

#[test]
fn rejected_admission_removes_only_provisional_root_and_can_retry() {
    let mut rig = Rig::new();
    let producer = rig.seed(50);
    rig.fault = Some(Fault::Rejected);
    assert!(
        matches!(submit(&mut rig, BackendDirectedScalarPeerCopyV1 { route: route(), dependencies: &[dependency(50, producer)] }), Err(RuntimeBackendFailureV1::Rejected(original)) if original == error())
    );
    assert_eq!(rig.roots.len(), 1);
    assert!(rig.roots[&producer].admitted);
    assert!(!rig.sealed);
    assert_eq!(rig.next, 101);
    rig.fault = None;
    assert_eq!(rig.submit(&[dependency(50, producer)]), 101);
}

#[test]
fn ambiguous_or_unreturned_admission_preserves_provisional_custody() {
    for fault in [
        Fault::Terminal,
        Fault::Quiescent,
        Fault::WrongHandle,
        Fault::MissingRecord,
    ] {
        let mut rig = Rig::new();
        let producer = rig.seed(50);
        rig.fault = Some(fault);
        assert!(matches!(
            submit(
                &mut rig,
                BackendDirectedScalarPeerCopyV1 {
                    route: route(),
                    dependencies: &[dependency(50, producer)]
                }
            ),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(rig.sealed);
        assert_eq!(rig.roots.len(), 2);
        assert_eq!(rig.roots[&101].dependencies, [dependency(50, producer)]);
        assert_eq!(rig.submits, 2);
        assert!(
            submit(
                &mut rig,
                BackendDirectedScalarPeerCopyV1 {
                    route: route(),
                    dependencies: &[]
                }
            )
            .is_err()
        );
        assert_eq!(rig.submits, 2);
    }
}

#[test]
fn admission_and_progress_unwind_preserve_original_payload_and_roots() {
    for during_progress in [false, true] {
        let mut rig = Rig::new();
        let producer = rig.seed(50);
        let id = if during_progress {
            rig.submit(&[dependency(50, producer)])
        } else {
            101
        };
        rig.fault = Some(Fault::Panic);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            if during_progress {
                let _ = progress(&mut rig, observation(id, &[producer]));
            } else {
                let _ = submit(
                    &mut rig,
                    BackendDirectedScalarPeerCopyV1 {
                        route: route(),
                        dependencies: &[dependency(50, producer)],
                    },
                );
            }
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u64>(), Some(&0x25));
        assert!(rig.sealed);
        assert_eq!(rig.roots.len(), 2);
        assert_eq!(rig.roots[&id].admitted, during_progress);
    }
}

#[test]
fn exact_progress_rejects_every_route_coordinate_and_roster_mismatch() {
    let mut rig = Rig::new();
    let first = rig.seed(50);
    let second = rig.seed(51);
    let id = rig.submit(&[dependency(50, first), dependency(51, second)]);
    let producers = [first, second];
    let first_only = [first];
    for mutation in 0..13 {
        let mut request = observation(id, &producers);
        match mutation {
            0 => request.route.stream += 1,
            1 => request.route.source_device += 1,
            2 => request.route.destination_device += 1,
            3 => request.route.source.allocation += 1,
            4 => request.route.destination.allocation += 1,
            5 => request.route.source.access = RuntimeAccessV1::ReadWrite,
            6 => request.route.destination.access = RuntimeAccessV1::ReadWrite,
            7 => request.route.source.byte_offset += 1,
            8 => request.route.destination.byte_offset += 1,
            9 => request.route.source.byte_len += 1,
            10 => request.route.destination.byte_len += 1,
            11 => request.producer_submissions = &first_only,
            12 => request.producer_submissions = &[],
            _ => unreachable!(),
        }
        rejected(
            progress(&mut rig, request),
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        );
    }
    for producers in [
        vec![second, first],
        vec![first, second, second],
        vec![first, 999],
    ] {
        rejected(
            progress(&mut rig, observation(id, &producers)),
            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        );
    }
    assert!(rig.progress.is_empty());
    assert_eq!(rig.roots.len(), 3);
    assert!(!rig.sealed);
}

#[test]
fn completed_provenance_outlives_events_allocations_streams_and_producers() {
    let mut rig = Rig::new();
    let producer = rig.seed(50);
    let id = rig.submit(&[dependency(50, producer)]);
    rig.settle(producer, BackendPollV1::Succeeded);
    rig.settle(id, BackendPollV1::Succeeded);
    rig.events.clear();
    rig.allocations.clear();
    rig.streams.clear();
    rig.completed.remove(&producer);
    rig.roots.remove(&producer);
    for _ in 0..3 {
        assert_eq!(
            progress(&mut rig, observation(id, &[producer])).unwrap(),
            BackendPollV1::Succeeded
        );
    }
    assert_eq!(rig.progress, [id; 3]);
    assert!(rig.roots.contains_key(&id));
    rejected(
        progress(&mut rig, observation(id, &[])),
        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
    );
}

#[test]
fn unknown_legacy_and_inconsistent_retained_records_are_distinct() {
    let mut rig = Rig::new();
    rejected(
        progress(&mut rig, observation(999, &[])),
        KfdRuntimeBackendErrorKindV1::UnknownHandle,
    );
    let id = rig.submit(&[]);
    let root = rig.roots.remove(&id).unwrap();
    rejected(
        progress(&mut rig, observation(id, &[])),
        KfdRuntimeBackendErrorKindV1::Unsupported,
    );
    rig.roots.insert(id, root);
    rig.completed.insert(
        id,
        SubmissionRecordV1 {
            stream: 11,
            status: BackendPollV1::Succeeded,
            profile_dispatch_published: false,
        },
    );
    assert!(matches!(
        progress(&mut rig, observation(id, &[])),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(rig.sealed);
    assert!(rig.progress.is_empty());
    assert!(rig.roots.contains_key(&id));
}

#[test]
fn malformed_active_or_completed_custody_never_reaches_progress_leaf() {
    for mutation in 0..13 {
        let mut rig = Rig::new();
        let id = rig.submit(&[]);
        match mutation {
            0 => rig.roots.get_mut(&id).unwrap().admitted = false,
            1 => rig.active.get_mut(&id).unwrap().id += 1,
            2 => rig.active.get_mut(&id).unwrap().stream += 1,
            3 => rig.active.get_mut(&id).unwrap().direction = 1,
            4 => rig.active.get_mut(&id).unwrap().source += 1,
            5 => rig.active.get_mut(&id).unwrap().destination += 1,
            6 => rig.active.get_mut(&id).unwrap().source_offset += 1,
            7 => rig.active.get_mut(&id).unwrap().destination_offset += 1,
            8 => rig.active.get_mut(&id).unwrap().byte_len += 1,
            9 => rig.active.get_mut(&id).unwrap().dependencies.push(1),
            10 => {
                rig.active.remove(&id);
            }
            11 => {
                rig.settle(id, BackendPollV1::Succeeded);
                rig.completed.get_mut(&id).unwrap().status = BackendPollV1::Pending;
            }
            12 => {
                rig.settle(id, BackendPollV1::Succeeded);
                rig.completed.get_mut(&id).unwrap().stream += 1;
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                progress(&mut rig, observation(id, &[])),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ),
            "mutation {mutation}"
        );
        assert!(rig.sealed);
        assert!(rig.progress.is_empty());
        assert!(rig.roots.contains_key(&id));
    }
}

#[test]
fn progress_preserves_rejection_and_terminalizes_unestablished_quiescence() {
    for fault in [Fault::Rejected, Fault::Quiescent, Fault::Terminal] {
        let mut rig = Rig::new();
        let id = rig.submit(&[]);
        rig.fault = Some(fault);
        let result = progress(&mut rig, observation(id, &[]));
        match fault {
            Fault::Rejected => assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Rejected(original)) if original == error())
            ),
            Fault::Quiescent => assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Terminal(original)) if original == error())
            ),
            Fault::Terminal => assert!(
                matches!(result, Err(RuntimeBackendFailureV1::Terminal(original)) if original == error())
            ),
            _ => unreachable!(),
        }
        assert_eq!(rig.sealed, !matches!(fault, Fault::Rejected));
        assert_eq!(rig.progress, [id]);
        assert!(rig.roots[&id].admitted);
    }
}

#[test]
fn native_readiness_accepts_only_success_and_retains_failed_provenance() {
    for status in [
        BackendPollV1::Pending,
        BackendPollV1::Succeeded,
        BackendPollV1::Failed { code: -2 },
        BackendPollV1::Failed { code: -3 },
        BackendPollV1::Failed { code: -4 },
    ] {
        let mut rig = Rig::new();
        let producer = rig.seed(50);
        let id = rig.submit(&[dependency(50, producer)]);
        if status != BackendPollV1::Pending {
            rig.settle(producer, status);
        }
        assert_eq!(
            xgmi_submission_is_ready_v1(&rig.active[&id], &rig.completed, 0),
            status == BackendPollV1::Succeeded
        );
        if matches!(status, BackendPollV1::Failed { .. }) {
            rig.settle(id, BackendPollV1::Failed { code: -1 });
            assert_eq!(
                progress(&mut rig, observation(id, &[producer])).unwrap(),
                BackendPollV1::Failed { code: -1 }
            );
            assert!(rig.roots.contains_key(&id));
        }
    }
}

#[test]
fn live_route_index_drift_is_terminal_before_progress() {
    for mutation in 0..8 {
        let mut rig = Rig::new();
        let id = rig.submit(&[]);
        match mutation {
            0 => {
                rig.streams.remove(&11);
            }
            1 => {
                rig.streams.insert(11, 0);
            }
            2 => {
                rig.allocations.remove(&20);
            }
            3 => {
                rig.allocations.remove(&21);
            }
            4 => rig.allocations.get_mut(&20).unwrap().device = 1,
            5 => rig.allocations.get_mut(&21).unwrap().device = 0,
            6 => rig.allocations.get_mut(&20).unwrap().byte_len += 1,
            7 => rig.allocations.get_mut(&21).unwrap().byte_len += 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            progress(&mut rig, observation(id, &[])),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(rig.sealed);
        assert!(rig.progress.is_empty());
        assert!(rig.roots.contains_key(&id));
    }
}

#[test]
fn release_guards_preserve_metadata_until_successful_removal() {
    for status in [BackendPollV1::Succeeded, BackendPollV1::Failed { code: -2 }] {
        let mut rig = Rig::new();
        let id = rig.submit(&[]);
        let untouched = rig.submit(&[]);
        rejected(release(&mut rig, id), KfdRuntimeBackendErrorKindV1::Busy);
        assert!(rig.roots.contains_key(&id));
        rig.settle(id, status);
        rig.event_retains.insert(id);
        // Ordinary retention guards precede even malformed directed metadata.
        rig.roots.get_mut(&id).unwrap().admitted = false;
        rejected(release(&mut rig, id), KfdRuntimeBackendErrorKindV1::Busy);
        assert!(!rig.sealed);
        rig.roots.get_mut(&id).unwrap().admitted = true;
        rig.event_retains.remove(&id);
        rig.dependency_retains.insert(id);
        rejected(release(&mut rig, id), KfdRuntimeBackendErrorKindV1::Busy);
        assert!(rig.roots.contains_key(&id));
        assert!(rig.completed.contains_key(&id));
        assert!(rig.depths.contains_key(&id));
        rig.dependency_retains.remove(&id);
        release(&mut rig, id).unwrap();
        assert!(!rig.roots.contains_key(&id));
        assert!(!rig.completed.contains_key(&id));
        assert!(!rig.depths.contains_key(&id));
        assert!(rig.roots.contains_key(&untouched));
        assert!(rig.active.contains_key(&untouched));
        assert!(rig.depths.contains_key(&untouched));
        rejected(
            release(&mut rig, id),
            KfdRuntimeBackendErrorKindV1::UnknownHandle,
        );
        assert!(!rig.sealed);
    }
}

#[test]
fn release_validation_failure_never_partially_removes_records() {
    for mutation in 0..4 {
        let mut rig = Rig::new();
        let id = rig.submit(&[]);
        rig.settle(id, BackendPollV1::Succeeded);
        match mutation {
            0 => {
                rig.depths.remove(&id);
            }
            1 => rig.roots.get_mut(&id).unwrap().admitted = false,
            2 => rig.completed.get_mut(&id).unwrap().stream += 1,
            3 => rig.completed.get_mut(&id).unwrap().status = BackendPollV1::Pending,
            _ => unreachable!(),
        }
        assert!(matches!(
            release(&mut rig, id),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(rig.sealed);
        assert!(rig.roots.contains_key(&id));
        assert!(rig.completed.contains_key(&id));
        assert_eq!(rig.depths.contains_key(&id), mutation != 0);
        assert!(release(&mut rig, id).is_err());
        assert!(rig.roots.contains_key(&id));
    }
}

#[test]
fn legacy_completion_release_does_not_require_directed_metadata() {
    let mut rig = Rig::new();
    let id = rig.submit(&[]);
    rig.roots.remove(&id);
    rig.settle(id, BackendPollV1::Succeeded);
    release(&mut rig, id).unwrap();
    assert!(!rig.completed.contains_key(&id));
    assert!(!rig.depths.contains_key(&id));
    assert!(!rig.sealed);
}
