use super::*;

#[allow(dead_code)]
enum PreviousKind {
    Leaf {
        group: u32,
        whole: u64,
        partial: Option<Rc<PartialPaths>>,
    },
    Branch {
        key: u32,
        bit: u32,
        zero: Rc<Node>,
        one: Rc<Node>,
    },
}
#[allow(dead_code)]
struct PreviousNode {
    kind: PreviousKind,
    storage: Storage,
}

#[test]
fn removed_fields_reduce_real_node_layout_by_exact_charge_delta() {
    assert_eq!(size_of::<NodeStorage>(), size_of::<usize>());
    assert_eq!(size_of::<Storage>(), 2 * size_of::<usize>());
    assert_eq!(
        size_of::<PreviousNode>() - size_of::<Node>(),
        2 * size_of::<usize>()
    );
    assert_eq!(12 - NODE_WORDS, 2);
    let rc_header = 2 * size_of::<usize>();
    assert_eq!(
        NODE_WORDS * size_of::<usize>() - size_of::<Node>() - rc_header,
        12 * size_of::<usize>() - size_of::<PreviousNode>() - rc_header,
    );
}

#[test]
fn branch_prefix_retains_every_group_split_and_ancestor_bit() {
    let budget = Budget::new(0, 0, 2_097_152, 67_108_864).unwrap();
    for shift in 0..26 {
        let bit = 1u32 << shift;
        let all_groups = u32::MAX >> GROUP_SHIFT;
        for prefix in [0, all_groups & !(bit | (bit - 1))] {
            let mut left = State::default();
            let mut right = State::default();
            left.mark((prefix | (bit - 1)) << GROUP_SHIFT, vec![], &budget)
                .unwrap();
            right
                .mark((prefix | bit) << GROUP_SHIFT, vec![], &budget)
                .unwrap();
            let node = branch(
                bit,
                left.root.clone().unwrap(),
                right.root.clone().unwrap(),
                &budget,
            )
            .unwrap();
            assert_eq!(node.bit(), bit);
            assert_eq!(node.key(), prefix);
            for sample in [prefix, prefix | bit, prefix | (bit - 1), all_groups] {
                let old_difference = (prefix | (bit - 1)) ^ sample;
                let new_difference = node.key() ^ sample;
                let split = |difference: u32| {
                    if difference == 0 {
                        0
                    } else {
                        1u32 << (31 - difference.leading_zeros())
                    }
                };
                assert_eq!(split(old_difference) > bit, split(new_difference) > bit);
                if split(old_difference) > bit {
                    assert_eq!(split(old_difference), split(new_difference));
                }
            }
        }
    }
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn fixed_charge_transfer_and_last_alias_drop_release_exactly_once() {
    let budget = Budget::new(17, 0, 17 + NODE_WORDS, 10).unwrap();
    let storage = NodeStorage::reserve(&budget).unwrap();
    assert_eq!(budget.0.live.get(), NODE_WORDS);
    assert_eq!(budget.peak(), NODE_WORDS);
    let node = Rc::new(Node {
        kind: Kind::Leaf {
            group: 0,
            whole: 1,
            partial: None,
        },
        _storage: storage,
    });
    let saved = node.clone();
    let accounting = budget.0.clone();
    drop(node);
    drop(budget);
    assert_eq!(accounting.live.get(), NODE_WORDS);
    drop(saved);
    assert_eq!(accounting.live.get(), 0);
}

#[test]
fn failure_after_fixed_reservation_preserves_prior_root_and_releases_charge() {
    let budget = Budget::new(0, 0, 2 * NODE_WORDS, 1).unwrap();
    let mut state = State::default();
    assert!(matches!(
        state.mark(0, vec![], &budget),
        Err(Error::Limit {
            resource: Resource::Work,
            ..
        })
    ));
    assert_eq!(budget.0.live.get(), 0);
    assert_eq!(budget.peak(), NODE_WORDS);
    assert!(state.root.is_none());
    let exact = Budget::new(0, 0, NODE_WORDS, 100).unwrap();
    let mut state = State::default();
    state.mark(0, vec![], &exact).unwrap();
    let saved = state.root.clone().unwrap();
    assert!(matches!(
        state.mark(1, vec![], &exact),
        Err(Error::Limit {
            resource: Resource::Storage,
            storage: Some(StorageFailure {
                live: NODE_WORDS,
                requested: NODE_WORDS,
                ..
            }),
            ..
        })
    ));
    assert!(Rc::ptr_eq(state.root.as_ref().unwrap(), &saved));
    assert_eq!(exact.0.live.get(), NODE_WORDS);
    drop((state, saved));
    assert_eq!(exact.0.live.get(), 0);
}

#[test]
fn retained_dense_snapshots_charge_each_unique_node_once() {
    fn visit(node: &Rc<Node>, seen: &mut BTreeSet<*const Node>) {
        if !seen.insert(Rc::as_ptr(node)) {
            return;
        }
        match &node.kind {
            Kind::Branch { zero, one, .. } => {
                visit(zero, seen);
                visit(one, seen);
            }
            Kind::Leaf { partial, .. } => assert!(partial.is_none()),
        }
    }
    let budget = Budget::new(0, 0, 2_097_152, 67_108_864).unwrap();
    let mut state = State::default();
    let mut snapshots = Vec::new();
    for local in 0..4096 {
        state.mark(local, vec![], &budget).unwrap();
        snapshots.push(state.clone());
    }
    let mut nodes = BTreeSet::new();
    for saved in snapshots.iter().chain([&state]) {
        visit(saved.root.as_ref().unwrap(), &mut nodes);
    }
    assert_eq!(budget.0.live.get(), nodes.len() * NODE_WORDS);
    for local in 0..4096 {
        state.clear(local, &budget).unwrap();
    }
    assert_eq!(budget.0.live.get(), nodes.len() * NODE_WORDS);
    assert!(
        !snapshots
            .last()
            .unwrap()
            .readable(4095, &[], &budget)
            .unwrap()
    );
    drop((snapshots, state));
    assert_eq!(budget.0.live.get(), 0);
}
