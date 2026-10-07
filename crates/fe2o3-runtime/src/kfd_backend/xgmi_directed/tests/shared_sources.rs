use super::*;

type Owners = HashMap<u64, Vec<u64>>;

fn branch(destination: u64, stream: u64) -> BackendDirectedPeerRouteV1 {
    BackendDirectedPeerRouteV1 {
        stream,
        source_device: 9,
        destination_device: 7,
        source: BackendMemoryRegionV1 {
            allocation: 21,
            access: RuntimeAccessV1::Read,
            byte_offset: 16,
            byte_len: 32,
        },
        destination: BackendMemoryRegionV1 {
            allocation: destination,
            access: RuntimeAccessV1::Write,
            byte_offset: 24,
            byte_len: 32,
        },
    }
}

fn diamond() -> (Rig, Owners) {
    let mut rig = Rig::new();
    rig.streams.insert(12, 0);
    for id in [22, 23] {
        rig.allocations.insert(
            id,
            XgmiRuntimeAllocationV1 {
                device: 0,
                byte_len: 1024,
                alignment: 8,
                authority: None,
            },
        );
    }
    assert_eq!(rig.seed(200), 100);
    assert_eq!(
        submit(
            &mut rig,
            BackendDirectedScalarPeerCopyV1 {
                route: branch(22, 10),
                dependencies: &[dependency(200, 100)],
            }
        )
        .unwrap(),
        101
    );
    let root = rig
        .view()
        .prepare(
            102,
            BackendDirectedScalarPeerCopyV1 {
                route: branch(23, 12),
                dependencies: &[dependency(200, 100)],
            },
        )
        .unwrap();
    rig.roots.insert(102, root);
    (
        rig,
        HashMap::from([(20, vec![100]), (21, vec![100, 101]), (22, vec![101])]),
    )
}

fn admission(rig: &Rig, owners: &Owners) -> Result<(), OwnerError> {
    rig.view()
        .admit_owners(owners, 102, branch(23, 12), &[200], &[100])
}

fn ready_siblings() -> Rig {
    let (mut rig, owners) = diamond();
    admission(&rig, &owners).unwrap();
    assert_eq!(rig.submit_scalar(branch(23, 12), &[200]).unwrap(), 102);
    rig.roots.get_mut(&102).unwrap().admitted = true;
    rig.settle(100, BackendPollV1::Succeeded);
    rig.active
        .values_mut()
        .for_each(|record| record.ready_indexed = true);
    rig
}

#[test]
fn production_owner_gate_admits_the_unchanged_diamond_without_sibling_dependency() {
    let (rig, owners) = diamond();
    let before = format!("{:?}{:?}", rig.roots, owners);
    assert_eq!(admission(&rig, &owners), Ok(()));
    assert_eq!(format!("{:?}{:?}", rig.roots, owners), before);
    assert_eq!(rig.roots[&102].dependencies, [dependency(200, 100)]);
}

#[test]
fn producer_write_still_requires_an_explicit_dependency() {
    let (mut rig, owners) = diamond();
    rig.roots.get_mut(&102).unwrap().dependencies.clear();
    assert_eq!(
        rig.view()
            .admit_owners(&owners, 102, branch(23, 12), &[], &[]),
        Err(OwnerError::Busy)
    );
}

#[test]
fn legacy_and_readwrite_roots_do_not_gain_the_shared_read_exception() {
    for mutation in 0..4 {
        let (mut rig, owners) = diamond();
        let mut request = branch(23, 12);
        match mutation {
            0 => {
                rig.roots.remove(&102);
            }
            1 => {
                rig.roots.remove(&101);
            }
            2 => {
                rig.roots.get_mut(&101).unwrap().route.source.access = RuntimeAccessV1::ReadWrite;
            }
            3 => {
                request.source.access = RuntimeAccessV1::ReadWrite;
                rig.roots.get_mut(&102).unwrap().route = request;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            rig.view()
                .admit_owners(&owners, 102, request, &[200], &[100]),
            Err(OwnerError::Busy)
        );
    }
}

#[test]
fn independent_destination_writes_still_reject() {
    let (mut rig, owners) = diamond();
    let request = branch(22, 12);
    rig.roots.get_mut(&102).unwrap().route = request;
    assert_eq!(
        rig.view()
            .admit_owners(&owners, 102, request, &[200], &[100]),
        Err(OwnerError::Busy)
    );
}

#[test]
fn owner_and_root_corruption_are_not_busy_or_permission() {
    for mutation in 0..12 {
        let (mut rig, mut owners) = diamond();
        match mutation {
            0 => owners.get_mut(&21).unwrap().push(101),
            1 => owners.get_mut(&21).unwrap().push(99),
            2 => {
                owners.insert(23, vec![101]);
            }
            3 => rig.active.get_mut(&101).unwrap().id = 99,
            4 => rig.roots.get_mut(&101).unwrap().admitted = false,
            5 => rig.roots.get_mut(&101).unwrap().route.source.byte_offset += 1,
            6 => rig.roots.get_mut(&102).unwrap().admitted = true,
            7 => rig.roots.get_mut(&102).unwrap().dependencies[0].event += 1,
            8 => rig.roots.get_mut(&102).unwrap().dependencies[0].producer_submission = 101,
            9 => rig.roots.get_mut(&102).unwrap().source_extent += 1,
            10 => owners.get_mut(&21).unwrap().clear(),
            11 => rig.active.get_mut(&101).unwrap().direction = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            admission(&rig, &owners),
            Err(OwnerError::Corrupt),
            "mutation {mutation}"
        );
    }
}

#[test]
fn owner_capacity_is_bounded_and_hazards_keep_precedence() {
    let (mut rig, _) = diamond();
    rig.roots.clear();
    rig.active.clear();
    let mut ids = Vec::new();
    for id in 100..100 + MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 as u64 {
        rig.active.insert(
            id,
            XgmiRuntimeSubmissionV1 {
                id,
                stream: id + 1000,
                direction: 1,
                source: 21,
                destination: 22,
                source_offset: 16,
                destination_offset: 24,
                byte_len: 32,
                dependencies: vec![],
                dependency_cursor: 0,
                ready_indexed: true,
                ticket: None,
                sequence: None,
            },
        );
        ids.push(id);
    }
    let owners = HashMap::from([(21, ids.clone()), (22, ids.clone())]);
    assert_eq!(
        rig.view()
            .admit_owners(&owners, 1000, branch(23, 12), &[], &ids),
        Err(OwnerError::Capacity)
    );
    assert_eq!(
        rig.view()
            .admit_owners(&owners, 1000, branch(23, 12), &[], &ids[1..]),
        Err(OwnerError::Busy)
    );
}

#[test]
fn exact_siblings_form_a_single_mapping_prefix_and_flush_rejects_before_publication() {
    let rig = ready_siblings();
    let ready = VecDeque::from([101, 102]);
    let prefix = |complete| {
        xgmi_progress::publication_len(
            1,
            &ready,
            &rig.active,
            &rig.completed,
            complete,
            |left, right, allocation| rig.view().shared_read(left, right, allocation),
        )
    };
    assert_eq!(prefix(false), Ok(1));
    let mut publications = 0;
    let result = publish_xgmi_flush_v1(2, false, || {
        prefix(true).map_err(|error| {
            assert_eq!(error, xgmi_progress::PrefixError::Shared);
            KfdNativeXgmiRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "shared mapping",
            )
        })?;
        publications += 1;
        Ok(XgmiBatchPublicationOutcomeV1::Published)
    });
    assert!(
        matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
    assert_eq!(publications, 0);
    assert_eq!(ready, [101, 102]);
    assert_eq!(rig.active.len(), 2);
}

#[test]
fn publication_never_skips_a_reader_or_masks_later_corruption() {
    let mut rig = ready_siblings();
    let mut other = XgmiRuntimeSubmissionV1 {
        id: 103,
        stream: 13,
        direction: 1,
        source: 30,
        destination: 31,
        source_offset: 0,
        destination_offset: 0,
        byte_len: 32,
        dependencies: vec![],
        dependency_cursor: 0,
        ready_indexed: true,
        ticket: None,
        sequence: None,
    };
    rig.active.insert(103, other);
    let select = |rig: &Rig, ids: &[u64]| {
        xgmi_progress::publication_len(
            1,
            &ids.iter().copied().collect(),
            &rig.active,
            &rig.completed,
            false,
            |left, right, allocation| rig.view().shared_read(left, right, allocation),
        )
    };
    assert_eq!(select(&rig, &[101, 103, 102]), Ok(2));
    assert_eq!(select(&rig, &[101, 102, 103]), Ok(1));
    other = rig.active.remove(&103).unwrap();
    other.destination = 21;
    rig.active.insert(103, other);
    assert_eq!(
        select(&rig, &[101, 102, 103]),
        Err(xgmi_progress::PrefixError::Corrupt)
    );
    assert_eq!(
        select(&rig, &[101, 102, 102]),
        Err(xgmi_progress::PrefixError::Corrupt)
    );
    rig.roots.remove(&102);
    assert_eq!(
        select(&rig, &[101, 102]),
        Err(xgmi_progress::PrefixError::Corrupt)
    );
}

#[test]
fn restored_mapping_allows_the_next_reader_without_a_new_dependency() {
    let mut rig = ready_siblings();
    let mut ready = VecDeque::from([101, 102]);
    let mut mapped = HashSet::new();
    let mut batches = Vec::new();
    while !ready.is_empty() {
        let count = xgmi_progress::publication_len(
            1,
            &ready,
            &rig.active,
            &rig.completed,
            false,
            |left, right, allocation| rig.view().shared_read(left, right, allocation),
        )
        .unwrap();
        let mut batch = Vec::new();
        for _ in 0..count {
            let id = ready.pop_front().unwrap();
            let record = &rig.active[&id];
            assert!(mapped.insert(record.source));
            assert!(mapped.insert(record.destination));
            batch.push(id);
        }
        for id in &batch {
            let record = &rig.active[id];
            assert!(mapped.remove(&record.source));
            assert!(mapped.remove(&record.destination));
            rig.settle(*id, BackendPollV1::Succeeded);
        }
        batches.push(batch);
    }
    assert_eq!(batches, [vec![101], vec![102]]);
    assert!(mapped.is_empty());
    assert!(rig.active.is_empty());
}

#[test]
fn shared_source_provenance_and_prefix_work_in_both_directions() {
    for mirror in [false, true] {
        let mut rig = ready_siblings();
        if mirror {
            rig.streams
                .values_mut()
                .for_each(|device| *device = 1 - *device);
            rig.allocations
                .values_mut()
                .for_each(|record| record.device = 1 - record.device);
            rig.active
                .values_mut()
                .for_each(|record| record.direction = 1 - record.direction);
            for root in rig.roots.values_mut() {
                root.route.source_device = 16 - root.route.source_device;
                root.route.destination_device = 16 - root.route.destination_device;
            }
        }
        assert_eq!(
            xgmi_progress::publication_len(
                usize::from(!mirror),
                &VecDeque::from([101, 102]),
                &rig.active,
                &rig.completed,
                false,
                |left, right, allocation| rig.view().shared_read(left, right, allocation)
            ),
            Ok(1)
        );
    }
}

#[test]
fn publication_window_stays_bounded_and_flush_capacity_precedes_selection() {
    let mut active = HashMap::new();
    for id in 1..=64 {
        active.insert(
            id,
            XgmiRuntimeSubmissionV1 {
                id,
                stream: id + 100,
                direction: 0,
                source: id * 2,
                destination: id * 2 + 1,
                source_offset: 0,
                destination_offset: 0,
                byte_len: 32,
                dependencies: vec![],
                dependency_cursor: 0,
                ready_indexed: true,
                ticket: None,
                sequence: None,
            },
        );
    }
    let ready: VecDeque<_> = (1..=64).collect();
    active.get_mut(&64).unwrap().direction = 9;
    assert_eq!(
        xgmi_progress::publication_len(
            0,
            &ready,
            &active,
            &HashMap::new(),
            false,
            |_, _, _| panic!("disjoint allocations")
        ),
        Ok(63)
    );
    let result = publish_xgmi_flush_v1(ready.len(), false, || panic!("capacity before selection"));
    assert!(
        matches!(result, Err(RuntimeBackendFailureV1::Rejected(error))
        if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity)
    );
    let result = publish_xgmi_flush_v1(ready.len(), true, || panic!("in-flight before selection"));
    assert!(
        matches!(result, Err(RuntimeBackendFailureV1::Rejected(error))
        if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
    );
}
