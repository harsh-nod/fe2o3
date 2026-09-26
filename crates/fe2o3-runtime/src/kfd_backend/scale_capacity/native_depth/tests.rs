//! Synthetic checker inputs are deliberately not native receipt evidence.

use super::*;
use fe2o3_profiler_protocol::ProfileTruthOriginV1;

fn digest(id: u64) -> [u8; 32] {
    Sha256::digest(id.to_le_bytes()).into()
}
fn identity(id: u64) -> ProfileIdentityV1 {
    ProfileIdentityV1::new(digest(id)).unwrap()
}

fn fixture() -> (Expected, DepthSnapshot) {
    let expected = Expected {
        ids: std::array::from_fn(|lane| {
            (0..DEPTH)
                .map(|index| 100 + (index * LANES + lane) as u64)
                .collect()
        }),
        streams: [7, 8],
        allocations: [[11, 12, 13], [14, 15, 16]],
        kernel: 20,
        module: 21,
    };
    let rows: Vec<_> = expected
        .ids
        .iter()
        .enumerate()
        .flat_map(|(lane, ids)| {
            let expected = &expected;
            ids.iter()
                .enumerate()
                .map(move |(ordinal, &id)| ReceiptRow {
                    id,
                    lane,
                    stream: expected.streams[lane],
                    kernel: expected.kernel,
                    allocations: expected.allocations[lane],
                    predecessor: ordinal.checked_sub(1).map(|index| ids[index]),
                    phase: (ordinal != 0).then_some(RuntimeComputePipelinePhaseV1::Published),
                    native: digest(id),
                    shape: digest(lane as u64),
                })
        })
        .collect();
    let custody = expected
        .allocations
        .iter()
        .enumerate()
        .flat_map(|(lane, allocations)| {
            let expected = &expected;
            allocations.iter().map(move |&allocation| {
                (
                    allocation,
                    Custody {
                        owners: expected.ids[lane]
                            .iter()
                            .map(|&submission| RuntimeAllocationCustodyOwnerV1 {
                                submission,
                                stream: expected.streams[lane],
                                kind: RuntimeAllocationCustodyKindV1::Compute,
                            })
                            .collect(),
                        sole_stream: Some(expected.streams[lane]),
                        counts: [DEPTH, 0],
                        capacity: DEPTH,
                    },
                )
            })
        })
        .collect();
    let cut = DepthSnapshot {
        membership: checker::membership(&rows),
        rows,
        pipeline_lengths: [DEPTH - 1; LANES],
        custody,
        leases: HashMap::from([(7, 0), (8, 1)]),
        tails: HashMap::from([
            (7, expected.ids[0][DEPTH - 1]),
            (8, expected.ids[1][DEPTH - 1]),
        ]),
        modules: HashMap::from([(21, LANES * DEPTH)]),
        dependencies: expected
            .ids
            .iter()
            .flat_map(|ids| ids[..DEPTH - 1].iter().map(|&id| (id, 1)))
            .collect(),
        reservations: LANES * DEPTH,
        usage: ResourceCreditUsageV1 {
            capacity: ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 200),
            used: ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 100),
            retained_records: 10,
            reserved_records: 0,
            quarantined_records: 0,
            record_capacity: 32,
            poisoned: false,
        },
    };
    (expected, cut)
}

#[test]
fn depth_checker_accepts_all_2048_rows_with_exact_runtime_joins() {
    let (expected, cut) = fixture();
    assert_eq!(checker::check_depth(&cut, &expected), Ok(()));
    assert_eq!(cut.rows[255].id, expected.ids[0][255]);
    assert_eq!(cut.rows[256].id, expected.ids[0][256]);
    assert_eq!(cut.rows[1023].id, expected.ids[0][1023]);
}

#[test]
fn depth_checker_rejects_frontier_only_missing_duplicate_and_cross_lane_rows() {
    let (expected, cut) = fixture();
    for index in [0, 255, 256, 1023, 1024, 2047] {
        let mut missing = cut.clone();
        missing.rows.remove(index);
        missing.membership = checker::membership(&missing.rows);
        assert!(checker::check_depth(&missing, &expected).is_err());
        let mut duplicate = cut.clone();
        duplicate.rows[index] = cut.rows[(index + 1) % cut.rows.len()].clone();
        duplicate.membership = checker::membership(&duplicate.rows);
        assert!(checker::check_depth(&duplicate, &expected).is_err());
    }
    let mut frontier = cut.clone();
    frontier.rows = vec![cut.rows[0].clone(), cut.rows[DEPTH].clone()];
    frontier.membership = checker::membership(&frontier.rows);
    assert!(checker::check_depth(&frontier, &expected).is_err());
    let mut swapped = cut.clone();
    swapped.rows.swap(1023, 1024);
    swapped.membership = checker::membership(&swapped.rows);
    assert!(checker::check_depth(&swapped, &expected).is_err());
}

#[test]
fn depth_checker_rejects_bad_coordinates_phases_and_native_digest_aliases() {
    let (expected, cut) = fixture();
    for case in 0..12 {
        let mut changed = cut.clone();
        let row = &mut changed.rows[256];
        match case {
            0 => row.stream += 1,
            1 => row.kernel += 1,
            2 => row.lane = 1,
            3 => row.allocations[0] = expected.allocations[1][0],
            4 => row.predecessor = None,
            5 => row.native = cut.rows[0].native,
            6 => row.native = [0; 32],
            7 => row.phase = None,
            8 => row.phase = Some(RuntimeComputePipelinePhaseV1::Completed),
            9 => row.phase = Some(RuntimeComputePipelinePhaseV1::PhysicallyRetired),
            10 => row.phase = Some(RuntimeComputePipelinePhaseV1::Quarantined),
            _ => row.shape = [0; 32],
        }
        changed.membership = checker::membership(&changed.rows);
        assert!(
            checker::check_depth(&changed, &expected).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn depth_membership_binds_each_opaque_receipt_to_its_owner_coordinates() {
    let (expected, mut cut) = fixture();
    let first = cut.rows[255].native;
    cut.rows[255].native = cut.rows[256].native;
    cut.rows[256].native = first;
    assert_eq!(
        checker::check_depth(&cut, &expected),
        Err("receipt membership digest")
    );
}

#[test]
fn depth_checker_rejects_corrupt_custody_and_runtime_indices() {
    let (expected, cut) = fixture();
    for case in 0..13 {
        let mut changed = cut.clone();
        let custody = changed.custody.get_mut(&11).unwrap();
        match case {
            0 => {
                custody.owners.pop_back();
            }
            1 => custody.owners[256].submission = 0,
            2 => custody.owners[256].stream = 8,
            3 => custody.owners[256].kind = RuntimeAllocationCustodyKindV1::Sdma,
            4 => custody.counts = [DEPTH - 1, 1],
            5 => custody.capacity = 256,
            6 => custody.sole_stream = None,
            7 => {
                changed.leases.insert(7, 1);
            }
            8 => {
                changed.tails.insert(7, expected.ids[0][DEPTH - 2]);
            }
            9 => {
                changed.modules.insert(21, LANES * DEPTH - 1);
            }
            10 => {
                changed.dependencies.insert(expected.ids[0][255], 2);
            }
            11 => changed.reservations -= 1,
            _ => changed.pipeline_lengths[1] -= 1,
        }
        assert!(
            checker::check_depth(&changed, &expected).is_err(),
            "case {case}"
        );
    }
    let mut foreign = expected.clone();
    foreign.ids[1][0] = foreign.ids[0][0];
    assert!(checker::check_depth(&cut, &foreign).is_err());
    foreign = expected.clone();
    foreign.allocations[1][0] = foreign.allocations[0][0];
    assert!(checker::check_depth(&cut, &foreign).is_err());
}

#[test]
fn depth_checker_keeps_peak_runtime_only_and_disposed_accounting_distinct() {
    let (expected, cut) = fixture();
    for case in 0..5 {
        let mut changed = cut.clone();
        match case {
            0 => changed.usage.retained_records = 2,
            1 => changed.usage.used = ResourceVectorV1::ZERO,
            2 => changed.usage.reserved_records = 1,
            3 => changed.usage.quarantined_records = 1,
            _ => changed.usage.poisoned = true,
        }
        assert!(checker::check_depth(&changed, &expected).is_err());
        assert!(checker::check_disposed(changed.usage).is_err());
    }
    let mut disposed = cut.usage;
    disposed.used = ResourceVectorV1::ZERO;
    disposed.retained_records = 0;
    assert_eq!(checker::check_disposed(disposed), Ok(()));
}

struct Trace {
    events: Vec<KfdRuntimeProfileEventV1>,
    ids: [Vec<ProfileIdentityV1>; LANES],
    queues: [ProfileIdentityV1; LANES],
    streams: [ProfileIdentityV1; LANES],
    publications: Vec<KfdRuntimeProfileEventKindV1>,
}

impl Trace {
    fn check(&self, events: &[KfdRuntimeProfileEventV1]) -> Result<(), &'static str> {
        checker::check_profile(
            events,
            &self.ids,
            self.queues,
            self.streams,
            &self.publications,
        )
    }
}

fn trace(reversed_lanes: bool) -> Trace {
    use KfdRuntimeProfileEventKindV1 as Event;
    let ids = std::array::from_fn(|lane| {
        (0..DEPTH)
            .map(|ordinal| identity(100 + (ordinal * LANES + lane) as u64))
            .collect::<Vec<_>>()
    });
    let queues = [identity(1), identity(2)];
    let streams = [identity(3), identity(4)];
    let mut events = Vec::new();
    for queue in queues {
        events.push(Event::NativeQueueCreated { queue });
    }
    for lane in 0..LANES {
        for &dispatch in &ids[lane] {
            events.push(Event::DispatchPublished {
                dispatch,
                queue: queues[lane],
                stream: streams[lane],
                kernel: identity(5),
                dispatch_shape: ProfileContentIdentityV1::observed(&[1]).unwrap(),
                launch: KfdProfileLaunchV1 {
                    grid: [256, 1, 1],
                    workgroup: [256, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                bindings: [
                    KfdProfileAccessV1::Read,
                    KfdProfileAccessV1::Read,
                    KfdProfileAccessV1::Write,
                ]
                .into_iter()
                .enumerate()
                .map(|(index, access)| KfdProfileBindingV1 {
                    allocation: identity(11 + (lane * 3 + index) as u64),
                    access,
                    byte_offset: 0,
                    byte_len: 4096,
                    kernarg_byte_offset: (index * 8) as u32,
                })
                .collect(),
            });
        }
    }
    let publications = events[LANES..].to_vec();
    for (&first, &second) in ids[0].iter().zip(&ids[1]) {
        for dispatch in if reversed_lanes {
            [second, first]
        } else {
            [first, second]
        } {
            events.push(Event::DispatchCompleted {
                dispatch,
                host_timing: KfdProfileHostTimingV1::default(),
            });
        }
    }
    for lane_ids in &ids {
        for &dispatch in lane_ids.iter().rev() {
            events.push(Event::SubmissionReleased { dispatch });
        }
    }
    for queue in queues {
        events.push(Event::NativeQueueDestroyed { queue });
    }
    let events = events
        .into_iter()
        .enumerate()
        .map(|(index, event)| KfdRuntimeProfileEventV1 {
            sequence: index as u64,
            identity: identity(index as u64 + 10000),
            origin: ProfileTruthOriginV1::Observed,
            event,
        })
        .collect();
    Trace {
        events,
        ids,
        queues,
        streams,
        publications,
    }
}

#[test]
fn depth_profile_checker_accepts_independent_cross_lane_completion_order() {
    for reversed in [false, true] {
        let trace = trace(reversed);
        assert_eq!(trace.check(&trace.events), Ok(()));
    }
}

#[test]
fn depth_profile_checker_rejects_wrong_queue_prefix_order_and_early_cleanup() {
    let trace = trace(false);
    for case in 0..8 {
        let mut changed = trace.events.clone();
        match case {
            0 => {
                changed.remove(256);
            }
            1 => changed[256].event = changed[255].event.clone(),
            2 => {
                let KfdRuntimeProfileEventKindV1::DispatchPublished { queue, .. } =
                    &mut changed[256].event
                else {
                    unreachable!()
                };
                *queue = trace.queues[1];
            }
            3 => changed[2].event = changed[2 + LANES * DEPTH].event.clone(),
            4 => {
                let index = 2 + LANES * DEPTH;
                let first = changed[index].event.clone();
                changed[index].event = changed[index + 2].event.clone();
                changed[index + 2].event = first;
            }
            5 => {
                changed[2 + LANES * DEPTH].event =
                    KfdRuntimeProfileEventKindV1::NativeQueueDestroyed {
                        queue: trace.queues[0],
                    }
            }
            6 => {
                changed.pop();
            }
            _ => changed[256].sequence = changed[255].sequence,
        }
        assert!(trace.check(&changed).is_err(), "case {case}");
    }
}

#[test]
fn depth_profile_checker_binds_each_publication_to_the_retained_recipe() {
    let trace = trace(false);
    for lane in 0..LANES {
        for ordinal in [0, 255, 256, 1023] {
            for case in 0..13 {
                let mut changed = trace.events.clone();
                let KfdRuntimeProfileEventKindV1::DispatchPublished {
                    kernel,
                    dispatch_shape,
                    launch,
                    bindings,
                    ..
                } = &mut changed[LANES + lane * DEPTH + ordinal].event
                else {
                    unreachable!()
                };
                match case {
                    0 => *kernel = identity(6),
                    1 => {
                        *dispatch_shape = ProfileContentIdentityV1::observed(b"different").unwrap()
                    }
                    2 => dispatch_shape.byte_len += 1,
                    3 => launch.grid[0] += 256,
                    4 => launch.workgroup[0] /= 2,
                    5 => launch.dynamic_shared_bytes = 256,
                    6 => bindings.clear(),
                    7 => bindings.swap(0, 1),
                    8 => bindings[0].allocation = identity(11 + ((1 - lane) * 3) as u64),
                    9 => bindings[0].access = KfdProfileAccessV1::Write,
                    10 => bindings[0].byte_offset += 4,
                    11 => bindings[0].byte_len -= 4,
                    _ => bindings[0].kernarg_byte_offset += 8,
                }
                assert_eq!(
                    trace.check(&changed),
                    Err("publication differs from retained recipe"),
                    "lane {lane}, ordinal {ordinal}, case {case}"
                );
            }
        }
    }
}
