use super::retirement_tests::{bytes, chain, domain, snapshot};
use super::*;

// Freeze both old traversal and lookup, independent of the shared helpers.
fn reference(nodes: &[Option<Node>], profile: usize, leaf: Key) -> Option<([Key; 4], usize)> {
    if profile != 3 && profile != 4 {
        return None;
    }
    let mut path = [ROOT; 4];
    let mut next = Some(leaf);
    let mut depth = 0;
    while let Some(key) = next {
        if depth == path.len() || depth == profile {
            return None;
        }
        path[depth] = key;
        depth += 1;
        next = nodes
            .get(key.slot)
            .and_then(Option::as_ref)
            .filter(|node| node.key == key)?
            .parent;
    }
    (path[depth - 1] == ROOT).then_some((path, depth))
}

fn key(slot: usize) -> Key {
    Key {
        slot,
        generation: slot as u64 + 1,
    }
}

fn node(slot: usize, parent: Option<Key>) -> Node {
    Node {
        key: key(slot),
        parent,
        capacity: bytes(100),
        used: bytes(slot as u64),
        counts: [slot, slot + 1, slot + 2],
        record_limit: 100,
        handles: slot + 3,
        children: slot + 4,
    }
}

#[test]
fn domain_arena_exhaustive_small_graphs_match_original_traversal() {
    let choices = [
        None,
        Some(key(0)),
        Some(key(1)),
        Some(key(2)),
        Some(key(3)),
        Some(Key {
            slot: 1,
            generation: u64::MAX,
        }),
        Some(Key {
            slot: usize::MAX,
            generation: 1,
        }),
    ];
    for shape in 0..choices.len().pow(4) {
        let nodes = core::array::from_fn::<_, 4, _>(|slot| {
            Some(node(
                slot,
                choices[shape / choices.len().pow(slot as u32) % choices.len()],
            ))
        });
        for vacant in 0..=4 {
            let mut selected: Vec<_> = nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (i != vacant).then(|| node(i, n.as_ref().unwrap().parent)))
                .collect();
            for profile in [0, 2, 3, 4, 5, usize::MAX] {
                for leaf in [
                    key(0),
                    key(1),
                    key(2),
                    key(3),
                    key(4),
                    Key {
                        slot: 3,
                        generation: 0,
                    },
                ] {
                    let result = domain_path_v1(&selected, profile, leaf);
                    assert_eq!(result, reference(&selected, profile, leaf));
                    if let Some((path, depth)) = result {
                        assert_eq!(path[0], leaf);
                        assert_eq!(path[depth - 1], ROOT);
                        assert!(path[depth..].iter().all(|&key| key == ROOT));
                        for (i, key) in path[..depth].iter().enumerate() {
                            assert!(path[..i].iter().all(|prior| prior.slot != key.slot));
                        }
                    }
                }
            }
            selected.clear();
            assert_eq!(domain_path_v1(&selected, 4, ROOT), None);
        }
    }
}

#[test]
fn domain_arena_facts_are_exact_and_ignore_inactive_padding() {
    use ResourceKindV1 as K;
    let kinds = [
        K::LogicalPayloadBytes,
        K::RequestedAllocationBytes,
        K::ResidentHostAllocationBytes,
        K::ResidentDeviceAllocationBytes,
        K::ExecutableHostImageBytes,
        K::ExecutableDeviceBytes,
        K::ControlResidentBytes,
        K::QueueResidentBytes,
        K::SignalResidentBytes,
        K::KernargResidentBytes,
        K::QueueSlots,
        K::SignalSlots,
        K::KernargSlots,
        K::OperationSlots,
        K::ReplyBytes,
        K::ReplyCells,
        K::TerminalRecordBytes,
        K::QuarantineBookkeepingBytes,
        K::AllocationRecords,
    ];
    let nodes = core::array::from_fn::<_, 4, _>(|slot| {
        let mut n = node(slot, slot.checked_sub(1).map(key));
        for (d, kind) in kinds.iter().enumerate() {
            n.used = n.used.with(*kind, (100 * slot + d) as u64);
            n.capacity = n.capacity.with(*kind, (1000 * slot + d) as u64);
        }
        Some(n)
    });
    for depth in 1..=4 {
        let (mut path, actual) = domain_path_v1(&nodes, 4, key(depth - 1)).unwrap();
        assert_eq!(actual, depth);
        path[depth..].fill(Key {
            slot: usize::MAX,
            generation: 0,
        });
        let facts = domain_facts_v1(&nodes, &path, depth).unwrap();
        for (i, fact) in facts.iter().enumerate() {
            if i < depth {
                let expected = nodes[depth - 1 - i].as_ref().unwrap();
                assert_eq!(fact.used, expected.used);
                assert_eq!(fact.capacity, expected.capacity);
                assert_eq!(fact.counts, expected.counts);
                assert_eq!(fact.record_limit, expected.record_limit);
            } else {
                assert_eq!(*fact, R75ResourceDomainFactsV1::EMPTY);
            }
        }
        path[depth - 1].generation = 0;
        assert!(domain_facts_v1(&nodes, &path, depth).is_none());
    }
    for depth in [0, 5, usize::MAX] {
        assert!(domain_facts_v1(&nodes, &[ROOT; 4], depth).is_none());
    }
    assert_eq!(domain_path_v1(&nodes, 3, key(3)), None);
}

#[test]
fn domain_arena_actual_mixed_ancestor_charges_preserve_selected_deltas() {
    let chain = chain(4);
    let a = chain[0].reserve(bytes(1)).unwrap().retain();
    let b = chain[1].reserve(bytes(2)).unwrap();
    let c = chain[2].reserve(bytes(3)).unwrap().retain();
    let root = domain(&chain[0]).root.clone();
    let before = chain
        .iter()
        .map(|account| account.usage())
        .collect::<Vec<_>>();
    let batch = chain[3].reserve(bytes(5)).unwrap().retain();
    for (account, old) in chain.iter().zip(before) {
        let now = account.usage();
        assert_eq!(
            now.used,
            r67_resource_reserve_v1(old.used, bytes(5), old.capacity).unwrap()
        );
        assert_eq!(now.reserved_records, old.reserved_records);
        assert_eq!(now.retained_records, old.retained_records + 1);
    }
    batch.release_after_disposal().unwrap();
    c.release_after_disposal().unwrap();
    drop(b);
    a.release_after_disposal().unwrap();
    assert_eq!(root.lock().occupied_records(ROOT), Ok(0));
}

#[test]
fn domain_arena_bad_path_is_atomic_across_production_entry_points() {
    for mode in 0..7 {
        let mut chain = chain(4);
        let root = domain(&chain[0]).root.clone();
        let leaf = domain(&chain[3]).key;
        let mut reservation = Some(chain[3].reserve(bytes(1)).unwrap());
        let token = reservation.as_ref().unwrap().token.as_ref().unwrap();
        let (slot, owner) = (token.slot, token.owner);
        let retained = (mode >= 5).then(|| reservation.take().unwrap().retain());
        root.lock().node_mut(leaf).unwrap().parent = Some(Key {
            slot: 0,
            generation: 0,
        });
        let before = snapshot(&root);
        match mode {
            0 => assert!(matches!(
                chain[3].reserve(bytes(1)),
                Err(ResourceCreditErrorV1::Invariant)
            )),
            1 => assert_eq!(
                domain(&chain[3]).transition(slot, owner, Action::Retain),
                Err(ResourceCreditErrorV1::Invariant)
            ),
            2 => assert_eq!(
                domain(&chain[3]).transition(slot, owner, Action::CancelUnissued),
                Err(ResourceCreditErrorV1::Invariant)
            ),
            3 => assert!(matches!(
                chain[3].new_child(bytes(1), 1),
                Err(ResourceCreditErrorV1::Invariant)
            )),
            4 => drop(chain.pop().unwrap()),
            5 | 6 => assert_eq!(
                domain(&chain[3]).transition(
                    slot,
                    owner,
                    if mode == 5 {
                        Action::ReleaseDisposed
                    } else {
                        Action::Quarantine
                    }
                ),
                Err(ResourceCreditErrorV1::Invariant)
            ),
            _ => unreachable!(),
        }
        assert_eq!(snapshot(&root), before, "mode={mode}");
        assert!(root.lock().poisoned);
        drop(reservation);
        drop(retained);
        root.lock().quarantine_anchor = None;
    }
}
