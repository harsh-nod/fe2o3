use super::*;
use std::collections::BTreeMap;

#[path = "state_join_sharing_v1_tests.rs"]
mod join_sharing_tests;

#[path = "state_partial_owner_v1_tests.rs"]
mod partial_owner_tests;

#[path = "state_branch_reuse_v1_tests.rs"]
mod branch_reuse_tests;

type Reference = BTreeMap<u32, BTreeSet<Path>>;

fn budget() -> Budget {
    Budget::new(0, 0, usize::MAX, usize::MAX).unwrap()
}
fn field(index: u32) -> Path {
    vec![PathElement::Field(index)]
}

fn snapshot(state: &State) -> Reference {
    fn visit(node: &Node, result: &mut Reference) {
        match &node.kind {
            Kind::Leaf {
                group,
                whole,
                partial,
            } => {
                assert!(*whole != 0 || partial.is_some());
                for slot in 0..64 {
                    if whole & (1u64 << slot) != 0 {
                        result.insert((group << GROUP_SHIFT) | slot, BTreeSet::from([vec![]]));
                    }
                }
                let mut previous = None;
                for (slot, paths) in entries(partial.as_ref()) {
                    assert!(slot < 64 && whole & (1u64 << slot) == 0);
                    assert!(previous.is_none_or(|previous| previous < slot));
                    assert!(!paths.paths.is_empty() && !paths.paths.contains(&Vec::new()));
                    previous = Some(slot);
                    assert!(
                        result
                            .insert(
                                (group << GROUP_SHIFT) | u32::from(slot),
                                paths.paths.clone()
                            )
                            .is_none()
                    );
                }
            }
            Kind::Branch { zero, one, .. } => {
                let bit = node.bit();
                assert!(bit > zero.bit() && bit > one.bit());
                assert_eq!(zero.key() & bit, 0);
                assert_ne!(one.key() & bit, 0);
                visit(zero, result);
                visit(one, result);
            }
        }
    }
    let mut result = Reference::new();
    if let Some(root) = &state.root {
        visit(root, &mut result);
    }
    result
}

fn reference_mark(state: &mut Reference, local: u32, path: Path) {
    let paths = state.entry(local).or_default();
    if path.is_empty() {
        paths.clear();
    } else if paths.contains(&Vec::new()) {
        return;
    }
    paths.insert(path);
}

fn reference_initialize(state: &mut Reference, local: u32, path: &[PathElement]) -> bool {
    if path.is_empty() {
        state.remove(&local);
        return true;
    }
    if let Some(paths) = state.get_mut(&local) {
        for length in 0..path.len() {
            if paths.contains(&path[..length]) {
                return false;
            }
        }
        paths.retain(|candidate| !candidate.starts_with(path));
        if paths.is_empty() {
            state.remove(&local);
        }
    }
    true
}

fn reference_merge(destination: &mut Reference, source: &Reference) -> bool {
    let before = destination.clone();
    for (local, incoming) in source {
        let paths = destination.entry(*local).or_default();
        if paths.contains(&Vec::new()) {
            continue;
        }
        if incoming.contains(&Vec::new()) {
            paths.clear();
            paths.insert(Vec::new());
        } else {
            paths.extend(incoming.iter().cloned());
        }
    }
    *destination != before
}

fn readable(state: &Reference, local: u32, path: &[PathElement]) -> bool {
    !state.get(&local).is_some_and(|paths| {
        paths
            .iter()
            .any(|moved| moved.starts_with(path) || path.starts_with(moved))
    })
}

#[derive(Debug, Default)]
struct AllocationProfile {
    branch_words: usize,
    bitmap_leaf_words: usize,
    mixed_leaf_words: usize,
    sparse_index_words: usize,
    partial_path_words: usize,
}

impl AllocationProfile {
    fn total(&self) -> usize {
        self.branch_words
            + self.bitmap_leaf_words
            + self.mixed_leaf_words
            + self.sparse_index_words
            + self.partial_path_words
    }
}

fn allocation_profile<'a>(states: impl IntoIterator<Item = &'a State>) -> AllocationProfile {
    #[derive(Default)]
    struct Seen {
        nodes: BTreeSet<*const Node>,
        indices: BTreeSet<*const PartialPaths>,
        paths: BTreeSet<*const PathSet>,
        profile: AllocationProfile,
    }
    impl Seen {
        fn visit(&mut self, node: &Rc<Node>) {
            if !self.nodes.insert(Rc::as_ptr(node)) {
                return;
            }
            assert_eq!(node._storage.words(), NODE_WORDS);
            match &node.kind {
                Kind::Branch { zero, one, .. } => {
                    self.profile.branch_words += NODE_WORDS;
                    self.visit(zero);
                    self.visit(one);
                }
                Kind::Leaf { partial: None, .. } => {
                    self.profile.bitmap_leaf_words += NODE_WORDS;
                }
                Kind::Leaf {
                    partial: Some(partial),
                    ..
                } => {
                    self.profile.mixed_leaf_words += NODE_WORDS;
                    if !self.indices.insert(Rc::as_ptr(partial)) {
                        return;
                    }
                    assert_eq!(
                        partial._storage.words,
                        PARTIAL_WORDS + partial.entries.len() * PARTIAL_ENTRY_WORDS
                    );
                    self.profile.sparse_index_words += partial._storage.words;
                    assert_eq!(partial.slots.count_ones() as usize, partial.entries.len());
                    for paths in &partial.entries {
                        if self.paths.insert(Rc::as_ptr(paths)) {
                            assert_eq!(
                                paths._storage.words,
                                PATH_SET_WORDS
                                    + paths
                                        .paths
                                        .iter()
                                        .map(|path| path_words(path).unwrap())
                                        .sum::<usize>()
                            );
                            self.profile.partial_path_words += paths._storage.words;
                        }
                    }
                }
            }
        }
    }
    let mut seen = Seen::default();
    for state in states {
        if let Some(root) = &state.root {
            seen.visit(root);
        }
    }
    seen.profile
}

#[test]
fn differential_snapshots_reads_writes_and_joins_match_reference() {
    let budget = budget();
    let keys = [
        0,
        1,
        3,
        17,
        63,
        64,
        65,
        127,
        128,
        255,
        256,
        65535,
        1 << 31,
        u32::MAX,
    ];
    let paths = [
        vec![],
        field(0),
        field(1),
        vec![PathElement::Field(0), PathElement::Field(2)],
        vec![PathElement::Downcast(1), PathElement::Field(0)],
        vec![PathElement::ConstantIndex {
            offset: 3,
            from_end: true,
        }],
    ];
    let mut states = vec![(State::default(), Reference::new()); 8];
    let mut random = 0x8948_aac3_412f_239du64;
    for step in 0..12000 {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        let index = random as usize % states.len();
        let source = (random >> 8) as usize % states.len();
        let key = keys[(random >> 16) as usize % keys.len()];
        let path = &paths[(random >> 24) as usize % paths.len()];
        match (random >> 32) % 6 {
            0 => {
                states[index].0.mark(key, path.clone(), &budget).unwrap();
                reference_mark(&mut states[index].1, key, path.clone());
            }
            1 => {
                assert_eq!(
                    states[index].0.initialize(key, path, &budget).unwrap(),
                    reference_initialize(&mut states[index].1, key, path)
                );
            }
            2 => {
                states[index].0.clear(key, &budget).unwrap();
                states[index].1.remove(&key);
            }
            3 => {
                let incoming = states[source].clone();
                assert_eq!(
                    states[index].0.merge(&incoming.0, &budget).unwrap(),
                    reference_merge(&mut states[index].1, &incoming.1)
                );
            }
            4 => {
                states[index] = states[source].clone();
            }
            _ => {
                assert_eq!(
                    states[index].0.readable(key, path, &budget).unwrap(),
                    readable(&states[index].1, key, path)
                );
            }
        }
        for (actual, expected) in &states {
            assert_eq!(&snapshot(actual), expected, "step {step}");
            for key in keys {
                for path in &paths {
                    assert_eq!(
                        actual.readable(key, path, &budget).unwrap(),
                        readable(expected, key, path)
                    );
                }
            }
        }
    }
    drop(states);
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn whole_local_and_strict_ancestor_facts_are_not_cleared_by_field_writes() {
    let budget = budget();
    let mut state = State::default();
    state.mark(7, vec![], &budget).unwrap();
    assert!(!state.initialize(7, &field(0), &budget).unwrap());
    state.clear(7, &budget).unwrap();
    state.mark(7, field(0), &budget).unwrap();
    assert!(
        !state
            .initialize(7, &[PathElement::Field(0), PathElement::Field(1)], &budget)
            .unwrap()
    );
    assert!(state.initialize(7, &field(1), &budget).unwrap());
    assert!(!state.readable(7, &field(0), &budget).unwrap());
    assert!(state.initialize(7, &field(0), &budget).unwrap());
    assert!(state.readable(7, &[], &budget).unwrap());
}

#[test]
fn call_return_initialization_does_not_change_cleanup_snapshot() {
    let budget = budget();
    let mut incoming = State::default();
    incoming.mark(1, field(0), &budget).unwrap();
    let cleanup = incoming.clone();
    let mut returned = incoming.clone();
    assert!(returned.initialize(1, &field(0), &budget).unwrap());
    assert!(returned.readable(1, &[], &budget).unwrap());
    assert!(!cleanup.readable(1, &field(0), &budget).unwrap());
    assert!(!incoming.readable(1, &field(0), &budget).unwrap());
}

#[test]
fn loop_join_converges_without_erasing_maybe_moved_paths() {
    let budget = budget();
    let mut header = State::default();
    let mut backedge = header.clone();
    backedge.mark(4, field(0), &budget).unwrap();
    assert!(header.merge(&backedge, &budget).unwrap());
    assert!(!header.merge(&backedge, &budget).unwrap());
    assert!(!header.readable(4, &field(0), &budget).unwrap());
    let mut repaired = header.clone();
    repaired.initialize(4, &field(0), &budget).unwrap();
    assert!(!header.merge(&repaired, &budget).unwrap());
    assert!(!header.readable(4, &field(0), &budget).unwrap());
}

#[test]
fn unchanged_block_snapshots_share_storage_and_single_local_updates_copy_one_branch() {
    let budget = budget();
    let mut state = State::default();
    for local in 0..2048 {
        state.mark(local, field(0), &budget).unwrap();
    }
    let live = budget.0.live.get();
    let snapshots = vec![state.clone(); 4096];
    assert_eq!(budget.0.live.get(), live);
    state.mark(1700, field(1), &budget).unwrap();
    eprintln!(
        "partial-move sharing: 2048 locals, 4096 snapshots; live before={live}, after={}, peak={}",
        budget.0.live.get(),
        budget.peak()
    );
    assert!(budget.0.live.get() - live < 32 * NODE_WORDS + 2 * path_words(&field(0)).unwrap());
    assert!(snapshots[0].readable(1700, &field(1), &budget).unwrap());
    assert!(!state.readable(1700, &field(1), &budget).unwrap());
    drop(state);
    drop(snapshots);
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn released_facts_do_not_accumulate_as_storage_but_work_remains_cumulative() {
    let budget = Budget::new(0, 0, 128, usize::MAX).unwrap();
    let mut state = State::default();
    for _ in 0..10000 {
        state.mark(1, field(0), &budget).unwrap();
        state.clear(1, &budget).unwrap();
    }
    assert!(budget.peak() < 128);
    assert_eq!(budget.0.live.get(), 0);
    assert!(budget.work_units() >= 20000);
}

#[test]
fn exact_storage_and_work_limits_are_inclusive_and_release_failed_allocations() {
    fn run(budget: &Budget) -> Result<()> {
        let mut state = State::default();
        state.mark(1, field(0), budget)?;
        let saved = state.clone();
        state.mark(u32::MAX, field(1), budget)?;
        state.initialize(1, &field(0), budget)?;
        state.merge(&saved, budget)?;
        Ok(())
    }
    let broad = budget();
    run(&broad).unwrap();
    let exact = Budget::new(17, 23, 17 + broad.peak(), 23 + broad.work_units()).unwrap();
    run(&exact).unwrap();
    for (storage, work, resource) in [
        (broad.peak() - 1, broad.work_units(), Resource::Storage),
        (broad.peak(), broad.work_units() - 1, Resource::Work),
    ] {
        let tight = Budget::new(17, 23, 17 + storage, 23 + work).unwrap();
        assert!(
            matches!(run(&tight), Err(Error::Limit { resource: actual, .. }) if actual == resource)
        );
        assert_eq!(tight.0.live.get(), 0);
    }
}

#[test]
fn high_bit_keys_and_long_paths_remain_bounded_and_preserve_snapshots() {
    let budget = budget();
    let mut state = State::default();
    for bit in 0..32 {
        state
            .mark(1 << bit, vec![PathElement::Field(bit); 64], &budget)
            .unwrap();
    }
    let before = snapshot(&state);
    let saved = state.clone();
    for bit in (0..32).rev() {
        state.clear(1 << bit, &budget).unwrap();
    }
    assert!(state.root.is_none());
    assert_eq!(snapshot(&saved), before);
    let tight = Budget::new(
        0,
        0,
        NODE_WORDS + PATH_HEADER_WORDS + ELEMENT_WORDS * 64 - 1,
        usize::MAX,
    )
    .unwrap();
    assert!(matches!(
        state.mark(0, vec![PathElement::Field(0); 64], &tight),
        Err(Error::Limit {
            resource: Resource::Storage,
            ..
        })
    ));
    assert!(state.root.is_none());
}

#[test]
fn dense_whole_local_snapshots_fit_fixed_budget_without_path_allocations() {
    const LOCALS: u32 = 16384;
    let budget = Budget::new(0, 0, 65536, usize::MAX).unwrap();
    let mut state = State::default();
    let mut snapshots = Vec::new();
    for local in 0..LOCALS {
        state.mark(local, vec![], &budget).unwrap();
        if local % 64 == 63 {
            snapshots.push(state.clone());
        }
    }
    let live = budget.0.live.get();
    let aliases = vec![state.clone(); 4096];
    assert_eq!(budget.0.live.get(), live);
    let profile = allocation_profile(snapshots.iter().chain([&state]));
    assert_eq!(profile.total(), live);
    assert_eq!(profile.mixed_leaf_words, 0);
    assert_eq!(profile.sparse_index_words, 0);
    assert_eq!(profile.partial_path_words, 0);
    eprintln!(
        "compact dense whole-local snapshots: {profile:?}; peak={}",
        budget.peak()
    );
    for (index, saved) in snapshots.iter().enumerate() {
        let end = (index as u32 + 1) * 64;
        for local in end - 64..end {
            assert!(!saved.readable(local, &[], &budget).unwrap());
            assert!(!saved.readable(local, &field(0), &budget).unwrap());
        }
        assert!(!saved.readable(0, &[], &budget).unwrap());
        assert!(saved.readable(end, &[], &budget).unwrap());
    }
    for local in 0..LOCALS {
        assert!(!state.readable(local, &[], &budget).unwrap());
        assert!(state.initialize(local, &[], &budget).unwrap());
        assert!(state.readable(local, &[], &budget).unwrap());
        assert!(!aliases[0].readable(local, &[], &budget).unwrap());
    }
    assert!(state.root.is_none());
    assert_eq!(
        allocation_profile(snapshots.iter().chain(aliases.iter())).total(),
        budget.0.live.get()
    );
    drop(aliases);
    drop(snapshots);
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn bitmap_updates_share_sparse_index_and_partial_updates_share_other_sets() {
    let budget = budget();
    {
        let mut state = State::default();
        for local in 0..63 {
            state.mark(local, field(0), &budget).unwrap();
        }
        let original = state.clone();
        let Kind::Leaf {
            partial: Some(original_index),
            ..
        } = &original.root.as_ref().unwrap().kind
        else {
            panic!("one sparse group");
        };
        state.mark(63, vec![], &budget).unwrap();
        let with_bitmap = state.clone();
        let Kind::Leaf {
            whole,
            partial: Some(bitmap_index),
            ..
        } = &with_bitmap.root.as_ref().unwrap().kind
        else {
            panic!("mixed group");
        };
        assert_eq!(*whole, 1u64 << 63);
        assert!(Rc::ptr_eq(original_index, bitmap_index));
        state.mark(1, field(1), &budget).unwrap();
        let with_partial = state.clone();
        let Kind::Leaf {
            partial: Some(changed_index),
            ..
        } = &with_partial.root.as_ref().unwrap().kind
        else {
            panic!("changed partial group");
        };
        assert!(!Rc::ptr_eq(original_index, changed_index));
        for slot in 0..63 {
            let old = partial_at(Some(original_index), slot, &budget)
                .unwrap()
                .unwrap();
            let new = partial_at(Some(changed_index), slot, &budget)
                .unwrap()
                .unwrap();
            assert_eq!(Rc::ptr_eq(old, new), slot != 1);
        }
        state.mark(1, vec![], &budget).unwrap();
        assert!(!state.initialize(1, &field(0), &budget).unwrap());
        state.clear(1, &budget).unwrap();
        assert!(state.readable(1, &[], &budget).unwrap());
        assert!(!state.readable(63, &[], &budget).unwrap());
        assert!(!with_partial.readable(1, &field(1), &budget).unwrap());
        assert!(original.readable(63, &[], &budget).unwrap());
        let profile = allocation_profile([&original, &with_bitmap, &with_partial, &state]);
        assert_eq!(profile.total(), budget.0.live.get());
    }
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn dense_mixed_group_joins_and_initialization_match_original_map_sets() {
    let budget = budget();
    let mut left = (State::default(), Reference::new());
    let mut right = (State::default(), Reference::new());
    for local in 0..192 {
        for path in [field(0), vec![PathElement::Field(0), PathElement::Field(1)]] {
            left.0.mark(local, path.clone(), &budget).unwrap();
            reference_mark(&mut left.1, local, path);
        }
        let path = if local % 3 == 0 { vec![] } else { field(1) };
        right.0.mark(local, path.clone(), &budget).unwrap();
        reference_mark(&mut right.1, local, path);
    }
    let saved_left = left.clone();
    let saved_right = right.clone();
    for (destination, source) in [(&mut left, &saved_right), (&mut right, &saved_left)] {
        assert_eq!(
            destination.0.merge(&source.0, &budget).unwrap(),
            reference_merge(&mut destination.1, &source.1)
        );
        assert!(!destination.0.merge(&source.0, &budget).unwrap());
        assert_eq!(snapshot(&destination.0), destination.1);
        for local in (0..192).rev() {
            let path = if local % 5 == 0 { vec![] } else { field(0) };
            assert_eq!(
                destination.0.initialize(local, &path, &budget).unwrap(),
                reference_initialize(&mut destination.1, local, &path)
            );
            assert_eq!(snapshot(&destination.0), destination.1);
            for probe in [
                vec![],
                field(0),
                field(1),
                vec![PathElement::Field(0), PathElement::Field(1)],
            ] {
                assert_eq!(
                    destination.0.readable(local, &probe, &budget).unwrap(),
                    readable(&destination.1, local, &probe)
                );
            }
        }
    }
    assert_eq!(snapshot(&saved_left.0), saved_left.1);
    assert_eq!(snapshot(&saved_right.0), saved_right.1);
    assert_eq!(
        allocation_profile([&left.0, &right.0, &saved_left.0, &saved_right.0]).total(),
        budget.0.live.get()
    );
    drop((left, right, saved_left, saved_right));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn failed_partial_update_keeps_snapshots_and_releases_new_allocations() {
    fn prepare(budget: &Budget) -> State {
        let mut state = State::default();
        state.mark(63, field(0), budget).unwrap();
        state.mark(64, vec![], budget).unwrap();
        state
    }
    let broad = budget();
    let mut state = prepare(&broad);
    let preparation_peak = broad.peak();
    let saved = state.clone();
    let added = vec![PathElement::Field(1); 64];
    state.mark(63, added.clone(), &broad).unwrap();
    let peak = broad.peak();
    assert!(peak > preparation_peak);
    drop((state, saved));
    for limit in preparation_peak..peak {
        let tight = Budget::new(0, 0, limit, usize::MAX).unwrap();
        let mut state = prepare(&tight);
        let saved = state.clone();
        let expected = snapshot(&state);
        let live = tight.0.live.get();
        assert!(matches!(
            state.mark(63, added.clone(), &tight),
            Err(Error::Limit {
                resource: Resource::Storage,
                ..
            })
        ));
        assert!(same_root(&state.root, &saved.root));
        assert_eq!(snapshot(&state), expected);
        assert_eq!(tight.0.live.get(), live);
        drop((state, saved));
        assert_eq!(tight.0.live.get(), 0);
    }
}

#[test]
fn allocation_header_charges_cover_rc_nodes_and_sparse_storage() {
    assert!(NODE_WORDS >= size_of::<Node>().div_ceil(size_of::<usize>()) + 2);
    assert!(PATH_SET_WORDS >= size_of::<PathSet>().div_ceil(size_of::<usize>()) + 2);
    assert_eq!(
        PARTIAL_WORDS,
        size_of::<PartialPaths>().div_ceil(size_of::<usize>()) + 2
    );
    assert_eq!(
        PARTIAL_ENTRY_WORDS,
        size_of::<Rc<PathSet>>().div_ceil(size_of::<usize>())
    );
}
