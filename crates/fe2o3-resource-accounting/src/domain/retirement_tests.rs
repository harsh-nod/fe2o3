use super::*;
use ResourceKindV1 as K;

#[derive(Debug, Eq, PartialEq)]
struct NodeSnapshot {
    key: Key,
    parent: Option<Key>,
    capacity: ResourceVectorV1,
    used: ResourceVectorV1,
    limit: usize,
    counts: [usize; 3],
    handles: usize,
    children: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    nodes: Vec<Option<NodeSnapshot>>,
    records: Vec<Option<(Key, u64, ResourceVectorV1, Phase)>>,
    free_nodes: Vec<usize>,
    free_records: Vec<usize>,
    next_node: u64,
    next_owner: u64,
    max_depth: usize,
    free_node_capacity: usize,
    free_record_capacity: usize,
    occupied: Vec<u64>,
}

pub(super) fn snapshot(root: &Arc<Root>) -> Snapshot {
    let state = root.lock();
    Snapshot {
        nodes: state
            .nodes
            .iter()
            .map(|node| {
                node.as_ref().map(|node| NodeSnapshot {
                    key: node.key,
                    parent: node.parent,
                    capacity: node.capacity,
                    used: node.used,
                    limit: node.record_limit,
                    counts: node.counts,
                    handles: node.handles,
                    children: node.children,
                })
            })
            .collect(),
        records: state
            .records
            .iter()
            .map(|record| {
                record.map(|record| {
                    (
                        record.leaf,
                        record.credit.owner,
                        record.credit.charge,
                        record.credit.phase,
                    )
                })
            })
            .collect(),
        free_nodes: state.free_nodes.clone(),
        free_records: state.free_records.clone(),
        next_node: state.next_node,
        next_owner: state.next_owner,
        max_depth: state.max_depth,
        free_node_capacity: state.free_nodes.capacity(),
        free_record_capacity: state.free_records.capacity(),
        occupied: state.occupied.clone(),
    }
}

fn corrupt_reap(root: &Arc<Root>, parent: Key, mode: usize, residual: u64) {
    let mut state = root.lock();
    match mode {
        0 => state.node_mut(parent).unwrap().used = bytes(residual),
        1 => state.node_mut(ROOT).unwrap().children = 0,
        2 => {
            let len = state.nodes.len() - 2;
            state.free_nodes.resize(len, usize::MAX);
        }
        3 => state.free_nodes = state.free_nodes.clone().into_boxed_slice().into_vec(),
        4 => state.free_records = state.free_records.clone().into_boxed_slice().into_vec(),
        _ => unreachable!(),
    }
}

pub(super) fn domain(account: &ResourceCreditAccountV1) -> &DomainAccount {
    let AccountHandle::Domain(domain) = &account.0 else {
        panic!("domain")
    };
    domain
}

pub(super) fn bytes(value: u64) -> ResourceVectorV1 {
    ResourceVectorV1::ZERO.with(K::RequestedAllocationBytes, value)
}

pub(super) fn chain(depth: usize) -> Vec<ResourceCreditAccountV1> {
    let capacity = bytes(100).with(K::ControlResidentBytes, 1 << 20);
    let root = if depth == 4 {
        ResourceCreditAccountV1::new_root_with_class_domains_v1(capacity, 8, 8)
    } else {
        ResourceCreditAccountV1::new_root(capacity, 8, 8)
    }
    .unwrap();
    let mut chain = vec![root];
    for _ in 1..depth {
        chain.push(chain.last().unwrap().new_child(bytes(100), 8).unwrap());
    }
    chain
}

#[test]
fn domain_retirement_late_reap_rejection_preserves_every_ancestor_and_record() {
    for (depth, mode) in [3, 4]
        .into_iter()
        .flat_map(|depth| (0..5).map(move |mode| (depth, mode)))
    {
        let chain = chain(depth);
        let root = domain(&chain[0]).root.clone();
        let parent = domain(&chain[1]).key;
        let retained = chain.last().unwrap().reserve(bytes(8)).unwrap().retain();
        drop(chain);
        corrupt_reap(&root, parent, mode, 9);
        let before = snapshot(&root);
        assert_eq!(
            retained.release_after_disposal(),
            Err(ResourceCreditErrorV1::Invariant)
        );
        assert!(root.lock().poisoned);
        assert_eq!(snapshot(&root), before);
        // CPU-only corrupt ledger: no native object was created.
        root.lock().quarantine_anchor = None;
    }
}

#[test]
fn domain_retirement_handle_drop_rejection_does_not_partially_recycle_children() {
    for (depth, mode) in [3, 4]
        .into_iter()
        .flat_map(|depth| (0..4).map(move |mode| (depth, mode)))
    {
        let mut chain = chain(depth);
        let root = domain(&chain[0]).root.clone();
        let parent = domain(&chain[1]).key;
        let leaf = chain.pop().unwrap();
        drop(chain);
        corrupt_reap(&root, parent, mode, 1);
        let before = snapshot(&root);
        drop(leaf);
        assert!(root.lock().poisoned);
        assert_eq!(snapshot(&root), before);
        root.lock().quarantine_anchor = None;
    }
}

#[test]
fn domain_retirement_drop_validates_ancestry_above_a_live_survivor_before_mutation() {
    for mode in 0..5 {
        let mut chain = chain(4);
        let root = domain(&chain[0]).root.clone();
        let leaf = chain.pop().unwrap();
        let survivor = chain.pop().unwrap();
        let survivor_key = domain(&survivor).key;
        drop(chain);
        {
            let mut state = root.lock();
            match mode {
                0 => state.max_depth = 3,
                1 => state.max_depth = 5,
                2 => state.node_mut(survivor_key).unwrap().parent = Some(survivor_key),
                3 => state.node_mut(survivor_key).unwrap().parent = Some(domain(&leaf).key),
                4 => {
                    state.node_mut(survivor_key).unwrap().parent = Some(Key {
                        slot: 0,
                        generation: 2,
                    })
                }
                _ => unreachable!(),
            }
        }
        let before = snapshot(&root);
        drop(leaf);
        assert!(root.lock().poisoned);
        assert_eq!(snapshot(&root), before);
        drop(survivor);
        assert_eq!(snapshot(&root), before);
        root.lock().quarantine_anchor = None;
    }
}

#[test]
fn domain_retirement_full_cascade_preserves_root_and_free_stack_order_at_every_depth() {
    for depth in 1..=4 {
        for action in [
            Action::CancelUnissued,
            Action::ReleaseRejected,
            Action::ReleaseDisposed,
            Action::Quarantine,
        ] {
            let chain = chain(depth);
            let root = domain(&chain[0]).root.clone();
            let initial_used = root.lock().node(ROOT).unwrap().used;
            let reservation = chain.last().unwrap().reserve(bytes(8)).unwrap();
            drop(chain);
            if action == Action::CancelUnissued {
                drop(reservation);
            } else {
                let retained = reservation.retain();
                for node in root.lock().nodes.iter().flatten() {
                    assert_eq!(node.counts, [0, 1, 0]);
                }
                match action {
                    Action::ReleaseRejected => retained.release_after_rejection().unwrap(),
                    Action::ReleaseDisposed => retained.release_after_disposal().unwrap(),
                    Action::Quarantine => retained.quarantine(),
                    _ => unreachable!(),
                }
            }
            let mut state = root.lock();
            assert!(!state.poisoned);
            assert_eq!(state.next_owner, 2);
            if action == Action::Quarantine {
                assert_eq!(state.nodes.iter().flatten().count(), depth);
                assert_eq!(
                    state
                        .node(ROOT)
                        .unwrap()
                        .used
                        .get(K::RequestedAllocationBytes),
                    8
                );
                for node in state.nodes.iter().flatten() {
                    assert_eq!(node.counts, [0, 0, 1]);
                }
                state.quarantine_anchor = None;
            } else {
                assert_eq!(state.nodes.iter().flatten().count(), 1);
                assert_eq!(state.node(ROOT).unwrap().used, initial_used);
                assert_eq!(state.node(ROOT).unwrap().children, 0);
                assert_eq!(state.node(ROOT).unwrap().counts, [0; 3]);
                assert_eq!(state.free_nodes, (1..8).rev().collect::<Vec<_>>());
                assert_eq!(state.free_records, (0..8).rev().collect::<Vec<_>>());
            }
        }
    }
}

#[test]
fn domain_retirement_stops_at_live_handles_siblings_and_remaining_records() {
    for stop in 0..3 {
        let mut chain = chain(4);
        let root = domain(&chain[0]).root.clone();
        let parent = domain(&chain[2]).key;
        let leaf = chain.pop().unwrap();
        let leaf_key = domain(&leaf).key;
        let retained = leaf.reserve(bytes(8)).unwrap().retain();
        let survivor = (stop == 0).then(|| chain[2].clone());
        let sibling = (stop == 1).then(|| chain[2].new_child(bytes(100), 8).unwrap());
        let second = (stop == 2).then(|| leaf.reserve(bytes(5)).unwrap().retain());
        drop(leaf);
        drop(chain);
        let before_free = root.lock().free_nodes.clone();
        retained.release_after_disposal().unwrap();
        {
            let state = root.lock();
            assert!(!state.poisoned);
            let parent = state.node(parent).unwrap();
            if stop == 2 {
                assert_eq!(state.free_nodes, before_free);
                assert!(state.node(leaf_key).is_ok());
                assert_eq!(parent.children, 1);
                assert_eq!(parent.used, bytes(5));
            } else {
                let mut expected = before_free;
                expected.push(leaf_key.slot);
                assert_eq!(state.free_nodes, expected);
                assert!(state.node(leaf_key).is_err());
                assert_eq!(parent.children, usize::from(stop == 1));
                assert_eq!(parent.used, bytes(0));
            }
        }
        drop(survivor);
        drop(sibling);
        if let Some(second) = second {
            second.release_after_disposal().unwrap();
        }
        let state = root.lock();
        assert!(!state.poisoned);
        assert_eq!(state.nodes.iter().flatten().count(), 1);
        assert_eq!(state.node(ROOT).unwrap().children, 0);
    }
}
