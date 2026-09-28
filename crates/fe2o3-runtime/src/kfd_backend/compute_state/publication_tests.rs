use super::super::tests::pipelined_active_for_test_v1;
use super::*;
use fe2o3_resource_accounting::{
    ResourceKindV1, ResourceVectorV1, host_metadata_table_payload_bytes_v1,
};

type Identity = RuntimeComputePipelineIdentityV1;
type Phase = RuntimeComputePipelinePhaseV1;

fn pipeline(capacity: usize) -> (RuntimeComputePipelineV1, ResourceCreditAccountV1) {
    let bytes =
        host_metadata_table_payload_bytes_v1::<RuntimeComputePipelineSlotV1>(capacity).unwrap();
    let account = ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        4,
    )
    .unwrap();
    let profile = match capacity {
        64 => Gfx942FixedDispatchCapacityProfileV1::Default64,
        1024 => Gfx942FixedDispatchCapacityProfileV1::Qualification1024,
        _ => unreachable!(),
    };
    (
        RuntimeComputePipelineV1::try_vacant(profile, Some(&account)).unwrap(),
        account,
    )
}

fn active(id: u64) -> ActiveSubmissionV1 {
    let mut owner = pipelined_active_for_test_v1(id);
    owner.allocations.insert(id + 100);
    owner.writebacks.push(WritebackV1 {
        allocation: id + 100,
        allocation_offset: 3,
        data_index: 2,
        data_offset: 8,
        byte_len: 16,
    });
    owner.ordinary_recipe = Some(Arc::new(OwnedComputeLaunchV1 {
        stream: owner.stream,
        kernel: owner.kernel,
        explicit_kernarg: vec![1, 2, 3].into_boxed_slice(),
        bindings: Box::new([]),
        geometry: crate::RuntimeLaunchGeometryV1 {
            grid: [1, 1, 1],
            workgroup: [1, 1, 1],
            dynamic_shared_bytes: 0,
        },
        semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
    }));
    owner.execution = Some(ActiveComputeExecutionV1::ScriptedMaterialized);
    owner
}

// Snapshot every field populated by these CPU fixtures, including stable payload
// addresses. This is not an observation of a native GPU receipt's internals.
#[derive(Clone, Debug, PartialEq)]
struct Owner {
    id: u64,
    stream: u64,
    predecessor: Option<u64>,
    deferred: bool,
    kernel: u64,
    depth: usize,
    allocations: Vec<u64>,
    allocation_capacity: usize,
    writebacks: Vec<(u64, usize, usize, u64, u64)>,
    writeback_address: usize,
    writeback_capacity: usize,
    residents: Vec<ResidentDataDescriptorV1>,
    resident_address: usize,
    resident_capacity: usize,
    recipe: Option<Arc<OwnedComputeLaunchV1>>,
    recipe_address: Option<usize>,
    shape: [u8; 32],
    published_at: Instant,
    performance: KfdRuntimeLaunchPerformanceV1,
    execution: Option<core::mem::Discriminant<ActiveComputeExecutionV1>>,
}

impl Owner {
    fn of(owner: &ActiveSubmissionV1) -> Self {
        let mut allocations: Vec<_> = owner.allocations.iter().copied().collect();
        allocations.sort_unstable();
        Self {
            id: owner.id,
            stream: owner.stream,
            predecessor: owner.ordered_predecessor,
            deferred: owner.deferred_ordered_predecessor_retain,
            kernel: owner.kernel,
            depth: owner.dependency_depth,
            allocations,
            allocation_capacity: owner.allocations.capacity(),
            writebacks: owner
                .writebacks
                .iter()
                .map(|v| {
                    (
                        v.allocation,
                        v.allocation_offset,
                        v.data_index,
                        v.data_offset,
                        v.byte_len,
                    )
                })
                .collect(),
            writeback_address: owner.writebacks.as_ptr() as usize,
            writeback_capacity: owner.writebacks.capacity(),
            residents: owner.resident_descriptors.clone(),
            resident_address: owner.resident_descriptors.as_ptr() as usize,
            resident_capacity: owner.resident_descriptors.capacity(),
            recipe: owner.ordinary_recipe.clone(),
            recipe_address: owner
                .ordinary_recipe
                .as_ref()
                .map(|v| Arc::as_ptr(v) as usize),
            shape: owner.dispatch_shape_sha256,
            published_at: owner.published_at,
            performance: owner.performance,
            execution: owner.execution.as_ref().map(core::mem::discriminant),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    identity: Identity,
    phase: Phase,
    owner: Owner,
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    slots: Vec<(u64, Option<Entry>)>,
    address: usize,
    live: usize,
    next: Option<u64>,
    frontier: Option<u64>,
    staged: Option<Identity>,
}

impl Snapshot {
    fn of(p: &RuntimeComputePipelineV1) -> Self {
        Self {
            slots: p
                .slots
                .iter()
                .map(|s| {
                    (
                        s.generation,
                        s.entry.as_ref().map(|e| Entry {
                            identity: e.identity,
                            phase: e.phase,
                            owner: Owner::of(&e.active),
                        }),
                    )
                })
                .collect(),
            address: p.slots.as_ptr() as usize,
            live: p.live,
            next: p.next_logical_epoch,
            frontier: p.commit_frontier,
            staged: p.staged,
        }
    }

    // Frozen pre-extraction predicates over snapshots, independent of all shared
    // helpers. Keep their intentionally narrow corruption checks unchanged.
    fn vacancy(&self) -> Option<(usize, u64)> {
        self.slots
            .iter()
            .enumerate()
            .find_map(|(i, (generation, entry))| {
                if entry.is_some() {
                    return None;
                }
                generation
                    .checked_add(1)
                    .filter(|g| *g != 0)
                    .map(|g| (i, g))
            })
    }

    fn capacity(&self) -> bool {
        self.staged.is_none()
            && self.live < self.slots.len() - 1
            && self.next.is_some()
            && self.vacancy().is_some()
    }

    fn entry(&self, id: Identity) -> Option<&Entry> {
        let (generation, entry) = self.slots.get(id.slot as usize)?;
        entry.as_ref().filter(|e| {
            *generation == id.slot_generation && e.identity == id && e.owner.id == id.submission
        })
    }

    fn frontier(&self) -> Result<Option<Identity>, ()> {
        if self.staged.is_some() {
            return Err(());
        }
        let entries: Vec<_> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(i, (g, e))| e.as_ref().map(|e| (i, g, e)))
            .collect();
        if entries.iter().any(|(i, g, e)| {
            e.identity.slot as usize != *i
                || e.identity.slot_generation != **g
                || e.identity.submission != e.owner.id
                || e.identity.submission == 0
        }) || entries.len() != self.live
        {
            return Err(());
        }
        if self.live == 0 {
            return self.frontier.is_none().then_some(None).ok_or(());
        }
        let found: Vec<_> = entries
            .iter()
            .filter(|(_, _, e)| Some(e.identity.logical_epoch) == self.frontier)
            .collect();
        let successor = self.frontier.and_then(|e| e.checked_add(1));
        if found.len() != 1
            || (self.live > 1
                && entries
                    .iter()
                    .filter(|(_, _, e)| Some(e.identity.logical_epoch) == successor)
                    .count()
                    != 1)
        {
            return Err(());
        }
        Ok(Some(found[0].2.identity))
    }

    fn stage(&self, id: u64) -> Option<Identity> {
        if id == 0
            || !self.capacity()
            || self
                .slots
                .iter()
                .any(|(_, e)| e.as_ref().is_some_and(|e| e.owner.id == id))
            || self.frontier().is_err()
            || self.next.is_none_or(|n| {
                n == 0
                    || self
                        .slots
                        .iter()
                        .any(|(_, e)| e.as_ref().is_some_and(|e| e.identity.logical_epoch >= n))
            })
        {
            return None;
        }
        let (slot, slot_generation) = self.vacancy().unwrap();
        Some(Identity {
            slot: slot as u16,
            slot_generation,
            logical_epoch: self.next.unwrap(),
            submission: id,
        })
    }

    fn intact(&self, id: Identity) -> bool {
        self.staged == Some(id)
            && id.slot_generation != 0
            && id.logical_epoch != 0
            && id.submission != 0
            && self.next == Some(id.logical_epoch)
            && self.entry(id).is_some_and(|e| e.phase == Phase::Publishing)
            && self.live != 0
            && self.slots.iter().filter(|(_, e)| e.is_some()).count() == self.live
            && self
                .slots
                .iter()
                .filter_map(|(_, e)| e.as_ref())
                .all(|e| e.identity == id || e.identity.logical_epoch < id.logical_epoch)
            && if self.live == 1 {
                self.frontier.is_none()
            } else {
                self.frontier.is_some_and(|f| {
                    f < id.logical_epoch
                        && self
                            .slots
                            .iter()
                            .filter_map(|(_, e)| e.as_ref())
                            .filter(|e| e.identity.logical_epoch == f)
                            .count()
                            == 1
                })
            }
    }
}

fn check_stage(p: &mut RuntimeComputePipelineV1, owner: ActiveSubmissionV1) {
    let mut expected = Snapshot::of(p);
    assert_eq!(p.has_successor_capacity(), expected.capacity());
    assert_eq!(
        p.checked_frontier_v1().map(|e| e.map(|e| e.identity)),
        expected.frontier()
    );
    let incoming = Owner::of(&owner);
    match (expected.stage(owner.id), p.stage_publication_v1(owner)) {
        (None, Err(returned)) => assert_eq!(Owner::of(&returned), incoming),
        (Some(identity), Ok(actual)) => {
            assert_eq!(actual, identity);
            expected.slots[identity.slot as usize] = (
                identity.slot_generation,
                Some(Entry {
                    identity,
                    phase: Phase::Publishing,
                    owner: incoming,
                }),
            );
            expected.live += 1;
            expected.staged = Some(identity);
        }
        _ => panic!("stage differs from frozen reference"),
    }
    assert_eq!(Snapshot::of(p), expected);
}

fn check_settlement(p: &mut RuntimeComputePipelineV1, id: Identity, confirm: bool) {
    let mut expected = Snapshot::of(p);
    let accepted = expected.intact(id);
    assert_eq!(
        p.entry_v1(id).map(|e| e.identity),
        expected.entry(id).map(|e| e.identity)
    );
    if confirm {
        assert_eq!(
            p.confirm_publication_v1(id),
            if accepted { Ok(()) } else { Err(()) }
        );
        if accepted {
            expected.slots[id.slot as usize].1.as_mut().unwrap().phase = Phase::Published;
            expected.staged = None;
            expected.next = id.logical_epoch.checked_add(1);
            expected.frontier = expected.frontier.or(Some(id.logical_epoch));
        }
    } else {
        let returned = p.withdraw_publication_v1(id);
        assert_eq!(returned.is_some(), accepted);
        if accepted {
            assert_eq!(
                Owner::of(&returned.unwrap()),
                expected.slots[id.slot as usize].1.take().unwrap().owner
            );
            expected.live -= 1;
            expected.staged = None;
        }
    }
    assert_eq!(Snapshot::of(p), expected);
}

fn relocate(p: &mut RuntimeComputePipelineV1, from: usize, to: usize) {
    let mut entry = p.slots[from].entry.take().unwrap();
    assert!(p.slots[to].entry.is_none());
    entry.identity.slot = to as u16;
    p.slots[to].generation = entry.identity.slot_generation;
    p.slots[to].entry = Some(entry);
}

#[test]
fn publication_stage_matches_frozen_predicate_and_exact_owner_frames() {
    for capacity in [64, 1024] {
        for case in 0..22 {
            let (mut p, account) = pipeline(capacity);
            p.insert_published(active(2)).unwrap();
            p.insert_published(active(3)).unwrap();
            relocate(&mut p, 1, capacity - 1);
            let mut incoming = active(4);
            match case {
                0 => {}
                1 => incoming.id = 0,
                2 => incoming.id = 3,
                3 => p.next_logical_epoch = Some(2),
                4 => p.next_logical_epoch = Some(0),
                5 => p.next_logical_epoch = None,
                6 => p.live = 0,
                7 => p.live = capacity - 1,
                8 => p.live = usize::MAX,
                9 => p.commit_frontier = None,
                10 => p.commit_frontier = Some(2),
                11 => p.staged = Some(p.slots[0].entry.as_ref().unwrap().identity),
                12 => p.slots[capacity - 1].generation += 1,
                13 => p.slots[capacity - 1].entry.as_mut().unwrap().identity.slot = 0,
                14 => p.slots[capacity - 1].entry.as_mut().unwrap().active.id = 99,
                15 => {
                    p.slots[capacity - 1]
                        .entry
                        .as_mut()
                        .unwrap()
                        .identity
                        .logical_epoch = 1
                }
                16 => {
                    p.slots[capacity - 1]
                        .entry
                        .as_mut()
                        .unwrap()
                        .identity
                        .logical_epoch = 4
                }
                17 => p.slots[1].generation = u64::MAX,
                18 => p.slots[1].generation = u64::MAX - 1,
                19 => {
                    for s in p.slots.iter_mut().filter(|s| s.entry.is_none()) {
                        s.generation = u64::MAX;
                    }
                }
                20 => p.quarantine_all(),
                21 => p.next_logical_epoch = Some(u64::MAX),
                _ => unreachable!(),
            }
            let usage = account.usage();
            check_stage(&mut p, incoming);
            assert_eq!(account.usage(), usage, "case {case}, capacity {capacity}");
        }
    }
}

#[test]
fn publication_settlement_matches_frozen_predicate_and_all_slot_frames() {
    for capacity in [64, 1024] {
        for confirm in [false, true] {
            for case in 0..25 {
                let (mut p, account) = pipeline(capacity);
                for id in 2..=4 {
                    p.insert_published(active(id)).unwrap();
                }
                let mut id = p.stage_publication_v1(active(5)).unwrap();
                relocate(&mut p, 3, capacity - 1);
                id.slot = (capacity - 1) as u16;
                p.staged = Some(id);
                match case {
                    0 => {}
                    1 => p.staged = None,
                    2 => p.next_logical_epoch = None,
                    3 => p.live = 0,
                    4 => p.live += 1,
                    5 => p.slots[capacity - 1].generation += 1,
                    6 => p.slots[capacity - 1].entry.as_mut().unwrap().active.id += 1,
                    7 => p.slots[capacity - 1].entry.as_mut().unwrap().phase = Phase::Published,
                    8 => p.quarantine_all(),
                    9 => p.commit_frontier = None,
                    10 => p.next_logical_epoch = Some(id.logical_epoch + 1),
                    11 => p.commit_frontier = Some(id.logical_epoch),
                    12 => id.slot = u16::MAX,
                    13 => id.slot_generation += 1,
                    14 => id.logical_epoch += 1,
                    15 => id.submission += 1,
                    16 => {
                        p.slots[1].entry.as_mut().unwrap().identity.logical_epoch = id.logical_epoch
                    }
                    17 => p.slots[1].entry.as_mut().unwrap().identity.logical_epoch = 1,
                    // These corrupt siblings are accepted by the old narrow
                    // predicate. They are not healthy-state endorsements.
                    18 => p.slots[1].entry.as_mut().unwrap().identity.slot = u16::MAX,
                    19 => p.slots[1].entry.as_mut().unwrap().active.id = 0,
                    20 => p.slots[1].entry.as_mut().unwrap().phase = Phase::Quarantined,
                    21 => p.slots[2].entry.as_mut().unwrap().identity.logical_epoch = 2,
                    22 => p.slots[1].entry.as_mut().unwrap().identity = id,
                    23 => p.slots[1].generation = u64::MAX,
                    24 => p.live = usize::MAX,
                    _ => unreachable!(),
                }
                let usage = account.usage();
                check_settlement(&mut p, id, confirm);
                assert_eq!(account.usage(), usage, "case {case}, capacity {capacity}");
            }
        }
    }
}

#[test]
fn publication_boundary_retries_use_last_slot_and_never_wrap() {
    for capacity in [64, 1024] {
        let (mut p, account) = pipeline(capacity);
        for slot in p.slots.iter_mut().take(capacity - 2) {
            slot.generation = u64::MAX;
        }
        p.slots[capacity - 2].generation = u64::MAX - 1;
        p.next_logical_epoch = Some(u64::MAX);
        let usage = account.usage();
        check_stage(&mut p, active(2));
        let id = p.staged.unwrap();
        assert_eq!(id.slot as usize, capacity - 2);
        assert_eq!(id.slot_generation, u64::MAX);
        check_settlement(&mut p, id, false);
        check_stage(&mut p, active(2));
        let id = p.staged.unwrap();
        assert_eq!(id.slot as usize, capacity - 1);
        assert_eq!(id.logical_epoch, u64::MAX);
        check_settlement(&mut p, id, false);
        check_stage(&mut p, active(2));
        let id = p.staged.unwrap();
        check_settlement(&mut p, id, true);
        assert_eq!(p.next_logical_epoch, None);
        check_stage(&mut p, active(3));
        assert_eq!(account.usage(), usage);
    }
}

#[test]
fn publication_successor_bound_and_metadata_paths_do_not_allocate() {
    for capacity in [64, 1024] {
        for confirm in [false, true] {
            let (mut p, account) = pipeline(capacity);
            p.slots[0].generation = u64::MAX;
            for id in 2..capacity as u64 {
                p.insert_published(active(id)).unwrap();
            }
            let incoming = active(capacity as u64);
            let refused = active(capacity as u64 + 1);
            let usage = account.usage();
            let address = p.slots.as_ptr();
            let ((), allocations) = super::super::drain_capture::tests::counted(|| {
                let id = p.stage_publication_v1(incoming).unwrap();
                assert_eq!(id.slot as usize, capacity - 1);
                assert!(p.stage_publication_v1(refused).is_err());
                if confirm {
                    p.confirm_publication_v1(id).unwrap();
                } else {
                    assert_eq!(p.withdraw_publication_v1(id).unwrap().id, capacity as u64);
                }
            });
            assert_eq!(allocations, 0);
            assert_eq!(p.len(), capacity - 2 + usize::from(confirm));
            assert_eq!(p.slots.as_ptr(), address);
            assert_eq!(account.usage(), usage);
            if confirm {
                check_stage(&mut p, active(capacity as u64 + 1));
            }
        }
    }
}

#[test]
fn publication_mutable_projection_is_exact_and_frames_neighbors() {
    for capacity in [64, 1024] {
        let (mut p, _) = pipeline(capacity);
        p.insert_published(active(2)).unwrap();
        let mut id = p.insert_published(active(3)).unwrap();
        relocate(&mut p, 1, capacity - 1);
        id.slot = (capacity - 1) as u16;
        let mut before = Snapshot::of(&p);
        let mut stale = id;
        stale.slot_generation += 1;
        assert!(p.entry_mut_v1(stale).is_none());
        assert_eq!(Snapshot::of(&p), before);
        p.entry_mut_v1(id).unwrap().phase = Phase::Completed;
        before.slots[capacity - 1].1.as_mut().unwrap().phase = Phase::Completed;
        assert_eq!(Snapshot::of(&p), before);
    }
}

#[test]
fn publication_preserves_narrow_acceptance_for_representable_corruption() {
    for capacity in [64, 1024] {
        for zero in [false, true] {
            let (mut p, _) = pipeline(capacity);
            for id in 2..=5 {
                p.insert_published(active(id)).unwrap();
            }
            if zero {
                for (i, slot) in p.slots.iter_mut().take(4).enumerate() {
                    slot.generation = 0;
                    let e = slot.entry.as_mut().unwrap();
                    e.identity.slot_generation = 0;
                    e.identity.logical_epoch = i as u64;
                }
                p.commit_frontier = Some(0);
                p.next_logical_epoch = Some(4);
            } else {
                p.slots[2].entry.as_mut().unwrap().identity.logical_epoch = 7;
                p.slots[3].entry.as_mut().unwrap().identity.logical_epoch = 7;
                p.next_logical_epoch = Some(8);
            }
            assert!(Snapshot::of(&p).stage(6).is_some());
            check_stage(&mut p, active(6));
            let id = p.staged.unwrap();
            assert!(Snapshot::of(&p).intact(id));
            check_settlement(&mut p, id, zero);
        }
        let (mut p, _) = pipeline(capacity);
        p.insert_published(active(2)).unwrap();
        let id = Identity {
            slot: 0,
            slot_generation: 0,
            logical_epoch: 0,
            submission: 0,
        };
        p.slots[0].generation = 0;
        p.slots[0].entry.as_mut().unwrap().identity = id;
        p.slots[0].entry.as_mut().unwrap().active.id = 0;
        assert_eq!(p.entry_v1(id).unwrap().identity, id);
        let mut before = Snapshot::of(&p);
        p.entry_mut_v1(id).unwrap().phase = Phase::Completed;
        before.slots[0].1.as_mut().unwrap().phase = Phase::Completed;
        assert_eq!(Snapshot::of(&p), before);
    }
}
