use super::retirement_tests::{bytes, chain, domain, snapshot};
use super::*;

fn observer_fields(depth: usize) -> (Vec<Option<Node>>, Vec<Option<DomainRecord>>, Key) {
    let keys: Vec<_> = (0..depth)
        .map(|slot| {
            if slot == 0 {
                ROOT
            } else {
                Key {
                    slot,
                    generation: slot as u64 + 10,
                }
            }
        })
        .collect();
    let nodes = keys
        .iter()
        .enumerate()
        .map(|(slot, key)| {
            Some(Node {
                key: *key,
                parent: slot.checked_sub(1).map(|parent| keys[parent]),
                capacity: ResourceVectorV1::ZERO,
                used: ResourceVectorV1::ZERO,
                record_limit: 0,
                counts: [0; 3],
                handles: 0,
                children: 0,
            })
        })
        .collect();
    let leaf = *keys.last().unwrap();
    let record = DomainRecord {
        leaf,
        credit: Record {
            owner: 17,
            charge: bytes(8),
            phase: Phase::Retained,
        },
    };
    let wrong = DomainRecord {
        credit: Record {
            owner: 18,
            ..record.credit
        },
        ..record
    };
    (nodes, vec![Some(wrong), Some(record), None], leaf)
}

fn observer_snapshot(nodes: &[Option<Node>], records: &[Option<DomainRecord>]) -> String {
    let nodes: Vec<_> = nodes
        .iter()
        .map(|node| {
            node.as_ref().map(|node| {
                (
                    node.key,
                    node.parent,
                    node.capacity,
                    node.used,
                    node.record_limit,
                    node.counts,
                    node.handles,
                    node.children,
                )
            })
        })
        .collect();
    let records: Vec<_> = records
        .iter()
        .map(|record| {
            record.as_ref().map(|record| {
                (
                    record.leaf,
                    record.credit.owner,
                    record.credit.charge,
                    record.credit.phase,
                )
            })
        })
        .collect();
    format!("{nodes:?}/{records:?}")
}

#[test]
fn domain_retained_observation_scalar_matrix_is_exact_and_immutable() {
    for depth in 1..=4 {
        let (nodes, records, leaf) = observer_fields(depth);
        let before = observer_snapshot(&nodes, &records);
        let wrong = Key {
            generation: leaf.generation + 1,
            ..leaf
        };
        for profile in [0, 2, 3, 4, 5, usize::MAX] {
            for slot in [1, 2, records.len(), usize::MAX] {
                for owner in [0, 17, u64::MAX] {
                    for key in [leaf, wrong] {
                        for poisoned in [false, true] {
                            for expected in [bytes(8), bytes(9)] {
                                let accepts = !poisoned
                                    && [3, 4].contains(&profile)
                                    && depth <= profile
                                    && slot == 1
                                    && owner == 17
                                    && key == leaf
                                    && expected == bytes(8);
                                assert_eq!(
                                    domain_retained_observation_v1(
                                        &nodes, profile, &records, poisoned, key, slot, owner,
                                        expected,
                                    ),
                                    accepts
                                );
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(observer_snapshot(&nodes, &records), before);
    }
}

#[test]
fn domain_retained_observation_rejects_actual_ancestry_corruption_without_writes() {
    for mode in 0..10 {
        let (mut nodes, records, leaf) = observer_fields(4);
        match mode {
            0 => nodes[leaf.slot].as_mut().unwrap().key.generation += 1,
            1 => nodes[leaf.slot].as_mut().unwrap().key.slot = usize::MAX,
            2 => nodes[leaf.slot] = None,
            3 => nodes[0].as_mut().unwrap().key.generation += 1,
            4 => nodes[1] = None,
            5 => nodes[2].as_mut().unwrap().parent = Some(leaf),
            6 => nodes[2].as_mut().unwrap().parent = None,
            7 => nodes[0].as_mut().unwrap().parent = Some(leaf),
            8 => {
                nodes[2]
                    .as_mut()
                    .unwrap()
                    .parent
                    .as_mut()
                    .unwrap()
                    .generation += 1
            }
            9 => nodes[2].as_mut().unwrap().parent.as_mut().unwrap().slot = usize::MAX,
            _ => unreachable!(),
        }
        let before = observer_snapshot(&nodes, &records);
        assert!(
            !domain_retained_observation_v1(&nodes, 4, &records, false, leaf, 1, 17, bytes(8),),
            "mode={mode}"
        );
        assert_eq!(observer_snapshot(&nodes, &records), before);
    }
}

#[test]
fn domain_retained_observation_checks_actual_leaf_phase_owner_and_every_coordinate() {
    let (nodes, mut records, leaf) = observer_fields(4);
    let original = records[1].unwrap();
    for mode in 0..7 {
        records[1] = Some(original);
        match mode {
            0 => records[1] = None,
            1 => records[1].as_mut().unwrap().leaf.generation += 1,
            2 => records[1].as_mut().unwrap().leaf.slot = 0,
            3 => records[1].as_mut().unwrap().credit.owner = 0,
            4..=6 => {
                records[1].as_mut().unwrap().credit.phase =
                    [Phase::Reserved, Phase::Quarantined, Phase::Vacant][mode - 4]
            }
            _ => unreachable!(),
        }
        let before = observer_snapshot(&nodes, &records);
        assert!(
            !domain_retained_observation_v1(&nodes, 4, &records, false, leaf, 1, 17, bytes(8),),
            "mode={mode}"
        );
        assert_eq!(observer_snapshot(&nodes, &records), before);
    }
    for kind in crate::retained_tests::KINDS {
        records[1] = Some(original);
        let changed = bytes(8).with(kind, bytes(8).get(kind) + 1);
        let before = observer_snapshot(&nodes, &records);
        assert!(!domain_retained_observation_v1(
            &nodes, 4, &records, false, leaf, 1, 17, changed,
        ));
        assert_eq!(observer_snapshot(&nodes, &records), before);
        records[1].as_mut().unwrap().credit.charge = changed;
        let before = observer_snapshot(&nodes, &records);
        assert!(!domain_retained_observation_v1(
            &nodes,
            4,
            &records,
            false,
            leaf,
            1,
            17,
            bytes(8),
        ));
        assert_eq!(observer_snapshot(&nodes, &records), before);
    }
    records[1] = Some(DomainRecord {
        credit: Record {
            charge: ResourceVectorV1::ZERO,
            ..original.credit
        },
        ..original
    });
    let before = observer_snapshot(&nodes, &records);
    assert!(domain_retained_observation_v1(
        &nodes,
        4,
        &records,
        false,
        leaf,
        1,
        17,
        ResourceVectorV1::ZERO,
    ));
    assert_eq!(observer_snapshot(&nodes, &records), before);
}

#[test]
fn domain_retained_observation_uses_each_calls_current_locked_fields() {
    let chain = chain(4);
    let leaf = chain.last().unwrap();
    let root = &domain(leaf).root;
    let credit = leaf.reserve(bytes(8)).unwrap().retain();
    let before = snapshot(root);
    assert!(leaf.matches_retained_charge_v1(&credit, bytes(8)));
    root.state.lock().unwrap().poisoned = true;
    assert!(!leaf.matches_retained_charge_v1(&credit, bytes(8)));
    assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
    root.state.lock().unwrap().poisoned = false;
    assert!(leaf.matches_retained_charge_v1(&credit, bytes(8)));
    assert_eq!(snapshot(root), before);
    credit.release_after_disposal().unwrap();
}

#[test]
fn retained_charge_domain_requires_exact_leaf_root_and_all_coordinates() {
    for depth in [3, 4] {
        let chain = chain(depth);
        let leaf = chain.last().unwrap();
        let sibling = chain[depth - 2].new_child(bytes(100), 8).unwrap();
        let independent = ResourceCreditAccountV1::new(bytes(100), 8).unwrap();
        let other_chain = super::retirement_tests::chain(depth);
        let other = other_chain.last().unwrap();
        let cloned = leaf.clone();
        let credits = leaf.reserve(bytes(8)).unwrap().retain();
        let second = leaf.reserve(bytes(8)).unwrap().retain();
        let foreign = other.reserve(bytes(8)).unwrap().retain();
        assert_eq!(domain(leaf).key, domain(other).key);
        assert_eq!(
            credits.token.as_ref().unwrap().slot,
            foreign.token.as_ref().unwrap().slot
        );
        assert_eq!(
            credits.token.as_ref().unwrap().owner,
            foreign.token.as_ref().unwrap().owner
        );
        let independent_credit = independent.reserve(bytes(8)).unwrap().retain();
        let root = &domain(leaf).root;
        let before = snapshot(root);
        let references = Arc::strong_count(root);
        for _ in 0..16 {
            for credit in [&credits, &second] {
                assert!(leaf.matches_retained_charge_v1(credit, bytes(8)));
                assert!(cloned.matches_retained_charge_v1(credit, bytes(8)));
                for wrong in [&sibling, &chain[0], &chain[depth - 2], &independent, other] {
                    assert!(!wrong.matches_retained_charge_v1(credit, bytes(8)));
                }
                for kind in crate::retained_tests::KINDS {
                    assert!(!leaf.matches_retained_charge_v1(
                        credit,
                        bytes(8).with(kind, bytes(8).get(kind) + 1)
                    ));
                }
            }
        }
        assert_eq!(snapshot(root), before);
        assert_eq!(Arc::strong_count(root), references);
        assert!(!root.state.lock().unwrap().poisoned);
        assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
        assert!(!leaf.matches_retained_charge_v1(&foreign, bytes(8)));
        assert!(!leaf.matches_retained_charge_v1(&independent_credit, bytes(8)));
        independent_credit.release_after_disposal().unwrap();
        foreign.release_after_disposal().unwrap();
        credits.release_after_disposal().unwrap();
        second.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_domain_rejects_record_and_token_corruption_without_effects() {
    for mode in 0..11 {
        let chain = chain(4);
        let leaf = chain.last().unwrap();
        let root = &domain(leaf).root;
        let mut credits = leaf.reserve(bytes(8)).unwrap().retain();
        let token = credits.token.as_mut().unwrap();
        let (slot, owner) = (token.slot, token.owner);
        let original = root.state.lock().unwrap().records[slot];
        match mode {
            0 => token.slot = usize::MAX,
            1 => token.slot = 7,
            2 => token.owner = 0,
            3 => token.owner += 1,
            4 => root.state.lock().unwrap().records[slot] = None,
            5..=7 => {
                root.state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .credit
                    .phase = [Phase::Reserved, Phase::Vacant, Phase::Quarantined][mode - 5]
            }
            8 => root.state.lock().unwrap().poisoned = true,
            10 => {
                token.owner = 0;
                root.state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .credit
                    .owner = 0;
            }
            9 => {
                root.state.lock().unwrap().records[slot]
                    .as_mut()
                    .unwrap()
                    .leaf = ROOT
            }
            _ => unreachable!(),
        }
        let before = snapshot(root);
        assert!(
            !leaf.matches_retained_charge_v1(&credits, bytes(8)),
            "mode={mode}"
        );
        assert_eq!(snapshot(root), before);
        assert_eq!(root.state.lock().unwrap().poisoned, mode == 8);
        assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
        let token = credits.token.as_mut().unwrap();
        token.slot = slot;
        token.owner = owner;
        {
            let mut state = root.state.lock().unwrap();
            state.records[slot] = original;
            state.poisoned = false;
        }
        credits.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_domain_rejects_malformed_selected_ancestry_without_effects() {
    for mode in 0..8 {
        let chain = chain(4);
        let leaf = chain.last().unwrap();
        let root = &domain(leaf).root;
        let leaf_key = domain(leaf).key;
        let ancestor = domain(&chain[1]).key;
        let credits = leaf.reserve(bytes(8)).unwrap().retain();
        let saved = {
            let mut state = root.state.lock().unwrap();
            let key = if mode < 3 { leaf_key } else { ancestor };
            let saved = state.nodes[key.slot].take().unwrap();
            if mode != 2 {
                let mut node = Node { ..saved };
                match mode {
                    0 | 3 => node.key.generation += 1,
                    1 => node.key.slot = usize::MAX,
                    4 => node.parent = Some(leaf_key),
                    5 => node.parent = None,
                    6 => state.max_depth = 3,
                    7 => state.max_depth = 2,
                    _ => unreachable!(),
                }
                state.nodes[key.slot] = Some(node);
            }
            (key, saved)
        };
        let before = snapshot(root);
        assert!(
            !leaf.matches_retained_charge_v1(&credits, bytes(8)),
            "mode={mode}"
        );
        assert_eq!(snapshot(root), before);
        assert!(!root.state.lock().unwrap().poisoned);
        assert!(root.state.lock().unwrap().quarantine_anchor.is_none());
        {
            let mut state = root.state.lock().unwrap();
            state.nodes[saved.0.slot] = Some(saved.1);
            state.max_depth = 4;
        }
        credits.release_after_disposal().unwrap();
    }
}

#[test]
fn retained_charge_domain_mutex_poison_is_not_recovered_or_anchored() {
    let chain = chain(4);
    let leaf = chain.last().unwrap();
    let root = &domain(leaf).root;
    let credits = leaf.reserve(bytes(8)).unwrap().retain();
    let before = snapshot(root);
    assert!(
        std::panic::catch_unwind(|| {
            let _guard = root.state.lock().unwrap();
            panic!("poison CPU fixture");
        })
        .is_err()
    );
    assert!(!leaf.matches_retained_charge_v1(&credits, bytes(8)));
    match root.state.lock() {
        Err(error) => {
            let state = error.into_inner();
            assert!(!state.poisoned);
            assert!(state.quarantine_anchor.is_none());
        }
        Ok(_) => panic!("query cleared mutex poison"),
    }
    root.state.clear_poison();
    assert_eq!(snapshot(root), before);
    credits.release_after_disposal().unwrap();
}
