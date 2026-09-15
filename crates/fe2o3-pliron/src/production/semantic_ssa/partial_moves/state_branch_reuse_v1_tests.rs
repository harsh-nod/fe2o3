use super::super::branch_reuse::{SourceLeaves, scratch_bytes};
use super::*;

fn superset_pair(budget: &Budget) -> (State, State) {
    let mut destination = State::default();
    destination.mark(0, vec![], budget).unwrap();
    destination.mark(64, vec![], budget).unwrap();
    let mut source = destination.clone();
    source.mark(1, vec![], budget).unwrap();
    (destination, source)
}

#[test]
fn exact_incoming_branch_owner_needs_no_sixth_node() {
    let budget = Budget::new(17, 0, 17 + 5 * NODE_WORDS, usize::MAX).unwrap();
    let (mut destination, source) = superset_pair(&budget);
    let saved = destination.clone();
    let expected_saved = snapshot(&saved);
    let expected = snapshot(&source);
    let live = budget.0.live.get();
    let peak = budget.peak();
    assert_eq!(live, 5 * NODE_WORDS);
    assert!(destination.merge(&source, &budget).unwrap());
    assert!(same_root(&destination.root, &source.root));
    assert_eq!(snapshot(&destination), expected);
    assert_eq!(snapshot(&saved), expected_saved);
    assert_eq!(budget.0.live.get(), live);
    assert_eq!(budget.peak(), peak);
    assert_eq!(
        allocation_profile([&destination, &source, &saved]).total(),
        live
    );
    assert!(!destination.merge(&source, &budget).unwrap());
    drop((destination, source, saved));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn disjoint_prefix_union_shares_internal_source_tree_not_unrelated_root() {
    let budget = budget();
    let mut destination = State::default();
    for local in [0, 64] {
        destination.mark(local, vec![], &budget).unwrap();
    }
    let saved = destination.clone();
    let mut source = State::default();
    for local in [256, 320] {
        source.mark(local, vec![], &budget).unwrap();
    }
    let mut expected = snapshot(&saved);
    reference_merge(&mut expected, &snapshot(&source));
    assert!(destination.merge(&source, &budget).unwrap());
    assert_eq!(snapshot(&destination), expected);
    let Kind::Branch { zero, one, .. } = &destination.root.as_ref().unwrap().kind else {
        unreachable!()
    };
    assert!(Rc::ptr_eq(zero, saved.root.as_ref().unwrap()));
    assert!(Rc::ptr_eq(one, source.root.as_ref().unwrap()));
    assert!(!same_root(&destination.root, &source.root));
    assert_eq!(budget.0.live.get(), 7 * NODE_WORDS);
    assert_eq!(budget.peak(), 8 * NODE_WORDS);
    assert_eq!(
        allocation_profile([&destination, &source, &saved]).total(),
        budget.0.live.get()
    );
    drop((destination, source, saved));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn dense_retained_join_reuses_every_source_branch_with_bounded_frontier_peak() {
    const GROUPS: u32 = 128;
    let budget = budget();
    let mut destination = State::default();
    for group in 0..GROUPS {
        destination
            .mark(group << GROUP_SHIFT, vec![], &budget)
            .unwrap();
    }
    let saved = destination.clone();
    let mut source = destination.clone();
    for group in 0..GROUPS {
        source
            .mark((group << GROUP_SHIFT) | 1, vec![], &budget)
            .unwrap();
    }
    let before = budget.0.live.get();
    assert_eq!(before, (4 * GROUPS as usize - 2) * NODE_WORDS);
    assert!(destination.merge(&source, &budget).unwrap());
    assert!(same_root(&destination.root, &source.root));
    assert_eq!(snapshot(&destination), snapshot(&source));
    assert_eq!(snapshot(&saved).len(), GROUPS as usize);
    assert_eq!(budget.0.live.get(), before);
    let depth = GROUPS.ilog2() as usize;
    assert!(budget.peak() <= before + 2 * depth * NODE_WORDS);
    assert_eq!(
        allocation_profile([&destination, &source, &saved]).total(),
        before
    );
    drop((destination, source, saved));
    assert_eq!(budget.0.live.get(), 0);
}

fn mark_pair(pair: &mut (State, Reference), local: u32, path: Path, budget: &Budget) {
    pair.0.mark(local, path.clone(), budget).unwrap();
    reference_mark(&mut pair.1, local, path);
}

#[test]
fn mixed_paths_bitmaps_snapshots_and_tag_facets_match_independent_map() {
    let groups = [0, 1, 2, 63, 64, 1024, 1 << 25, u32::MAX >> GROUP_SHIFT];
    let probes = [
        vec![],
        field(0),
        field(1),
        vec![PathElement::Downcast(1), PathElement::Field(2)],
        vec![PathElement::ConstantIndex {
            offset: 2,
            from_end: false,
        }],
        vec![PathElement::ConstantIndex {
            offset: 2,
            from_end: true,
        }],
    ];
    for seed in 0..32u32 {
        let budget = budget();
        let mut left = (State::default(), Reference::new());
        for group in groups {
            mark_pair(&mut left, (group << GROUP_SHIFT) | seed, field(0), &budget);
        }
        let saved = left.clone();
        let mut right = left.clone();
        for (index, group) in groups.into_iter().enumerate() {
            let local = (group << GROUP_SHIFT) | seed;
            mark_pair(
                &mut right,
                local,
                probes[(index + seed as usize) % probes.len()].clone(),
                &budget,
            );
            mark_pair(&mut left, local | 32, field(1), &budget);
        }
        let saved_right = right.clone();
        let expected_changed = reference_merge(&mut left.1, &right.1);
        assert_eq!(left.0.merge(&right.0, &budget).unwrap(), expected_changed);
        assert_eq!(snapshot(&left.0), left.1);
        assert!(!left.0.merge(&right.0, &budget).unwrap());
        for group in groups {
            let local = (group << GROUP_SHIFT) | seed;
            for probe in &probes {
                assert_eq!(
                    left.0.readable(local, probe, &budget).unwrap(),
                    readable(&left.1, local, probe)
                );
                let tag_readable = !left
                    .1
                    .get(&local)
                    .is_some_and(|paths| paths.iter().any(|path| probe.starts_with(path)));
                assert_eq!(
                    left.0.direct_tag_readable(local, probe, &budget).unwrap(),
                    tag_readable
                );
            }
            let path = field(0);
            assert_eq!(
                left.0.initialize(local, &path, &budget).unwrap(),
                reference_initialize(&mut left.1, local, &path)
            );
            left.0.clear(local | 32, &budget).unwrap();
            left.1.remove(&(local | 32));
        }
        assert_eq!(snapshot(&left.0), left.1);
        assert_eq!(snapshot(&saved.0), saved.1);
        assert_eq!(snapshot(&right.0), right.1);
        assert_eq!(snapshot(&saved_right.0), saved_right.1);
        assert_eq!(
            allocation_profile([&left.0, &right.0, &saved.0, &saved_right.0]).total(),
            budget.0.live.get()
        );
        drop((left, right, saved, saved_right));
        assert_eq!(budget.0.live.get(), 0);
    }
}

#[test]
fn every_new_cursor_and_reuse_work_unit_is_subject_to_existing_budget() {
    let broad = budget();
    let (mut left, right) = superset_pair(&broad);
    let setup = broad.work_units();
    left.merge(&right, &broad).unwrap();
    let needed = broad.work_units() - setup;
    assert_eq!(needed, 15);
    drop((left, right));
    for remaining in 0..=needed {
        let limit = 11 + setup + remaining;
        let budget = Budget::new(0, 11, usize::MAX, limit).unwrap();
        let (mut left, right) = superset_pair(&budget);
        let saved = left.clone();
        let expected_old = snapshot(&saved);
        let expected_union = snapshot(&right);
        let result = left.merge(&right, &budget);
        if remaining == needed {
            assert_eq!(result, Ok(true));
            assert_eq!(snapshot(&left), expected_union);
        } else {
            assert_eq!(
                result,
                Err(Error::Limit {
                    resource: Resource::Work,
                    required: limit + 1,
                    limit,
                    storage: None,
                })
            );
            let observed = snapshot(&left);
            assert!(observed == expected_old || observed == expected_union);
        }
        assert_eq!(snapshot(&saved), expected_old);
        assert_eq!(snapshot(&right), expected_union);
        assert_eq!(
            allocation_profile([&left, &right, &saved]).total(),
            budget.0.live.get()
        );
        drop((left, right, saved));
        assert_eq!(budget.0.live.get(), 0);
    }
}

#[test]
fn ancestor_cursor_preserves_sorted_leaf_order_and_old_scratch_envelope() {
    assert!(scratch_bytes() <= size_of::<[Option<&Rc<Node>>; 33]>() + size_of::<usize>());
    let budget = budget();
    let mut state = State::default();
    let locals = [u32::MAX, 4096, 0, 64, 128, 1 << 31, 63, 129];
    for local in locals {
        state.mark(local, vec![], &budget).unwrap();
    }
    let before = budget.0.live.get();
    let mut cursor = SourceLeaves::new(state.root.as_ref().unwrap());
    let mut actual = Vec::new();
    while let Some(node) = cursor.next_leaf(&budget).unwrap() {
        let Kind::Leaf { group, .. } = node.kind else {
            unreachable!()
        };
        actual.push(group);
    }
    let expected = locals
        .into_iter()
        .map(|local| local >> GROUP_SHIFT)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.into_iter().collect::<Vec<_>>());
    assert!(cursor.next_leaf(&budget).unwrap().is_none());
    assert_eq!(budget.0.live.get(), before);
}
