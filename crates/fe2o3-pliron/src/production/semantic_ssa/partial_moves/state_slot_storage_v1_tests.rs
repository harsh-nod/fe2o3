use super::*;

#[test]
fn packed_rows_preserve_direct_tag_vs_payload_read_facets() {
    let budget = budget();
    let mut state = State::default();
    let variant = PathElement::Downcast(1);
    let field = PathElement::Field(3);
    let payload = [variant, field];
    state.mark(63, payload.to_vec(), &budget).unwrap();
    let original = state.clone();
    // These are lattice queries; the caller still authenticates a direct-tag enum.
    for prefix in [&[][..], &[variant][..]] {
        assert!(!state.readable(63, prefix, &budget).unwrap());
        assert!(state.direct_tag_readable(63, prefix, &budget).unwrap());
    }
    for blocked in [&payload[..], &[variant, field, PathElement::Field(0)][..]] {
        assert!(!state.direct_tag_readable(63, blocked, &budget).unwrap());
    }
    assert!(state.initialize(63, &payload, &budget).unwrap());
    assert!(state.readable(63, &[], &budget).unwrap());
    assert!(original.direct_tag_readable(63, &[], &budget).unwrap());
    assert!(!original.direct_tag_readable(63, &payload, &budget).unwrap());
    state.mark(63, vec![], &budget).unwrap();
    assert!(!state.direct_tag_readable(63, &[], &budget).unwrap());
    assert!(!state.direct_tag_readable(63, &payload, &budget).unwrap());
    drop((state, original));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn direct_tag_queries_keep_all_slot_ranks_and_snapshot_payload_holes() {
    let budget = budget();
    let mut state = State::default();
    for slot in 0..64 {
        state
            .mark(slot, vec![PathElement::Field(slot)], &budget)
            .unwrap();
    }
    let original = state.clone();
    for slot in (0..64).rev() {
        let path = [PathElement::Field(slot)];
        assert!(state.direct_tag_readable(slot, &[], &budget).unwrap());
        assert!(!state.direct_tag_readable(slot, &path, &budget).unwrap());
        assert!(state.initialize(slot, &path, &budget).unwrap());
        assert!(state.readable(slot, &[], &budget).unwrap());
        assert!(original.direct_tag_readable(slot, &[], &budget).unwrap());
        assert!(!original.direct_tag_readable(slot, &path, &budget).unwrap());
    }
    drop((state, original));
    assert_eq!(budget.0.live.get(), 0);
}

fn budget() -> Budget {
    Budget::new(0, 0, 2_097_152, 67_108_864).unwrap()
}

fn partial(state: &State) -> &Rc<PartialPaths> {
    let Kind::Leaf {
        partial: Some(partial),
        ..
    } = &state.root.as_ref().unwrap().kind
    else {
        panic!("one sparse group");
    };
    assert_eq!(partial.slots.count_ones() as usize, partial.entries.len());
    partial
}

#[test]
#[cfg(target_pointer_width = "64")]
fn packed_slots_remove_real_pointer_padding_with_no_singleton_regression() {
    #[allow(dead_code)]
    struct Previous {
        entries: Box<[(u8, Rc<PathSet>)]>,
        storage: Storage,
    }
    let word = size_of::<usize>();
    assert_eq!(size_of::<PartialPaths>(), size_of::<Previous>() + word);
    assert_eq!(
        size_of::<(u8, Rc<PathSet>)>(),
        size_of::<Rc<PathSet>>() + word
    );
    for count in 1..=64 {
        let old_bytes = size_of::<Previous>() + 2 * word + count * size_of::<(u8, Rc<PathSet>)>();
        let new_bytes = size_of::<PartialPaths>() + 2 * word + count * size_of::<Rc<PathSet>>();
        let budget = budget();
        let storage = partial_storage(count, &budget).unwrap();
        assert_eq!(storage.words * word, new_bytes);
        assert_eq!(old_bytes - new_bytes, (count - 1) * word);
        drop(storage);
        assert_eq!(budget.0.live.get(), 0);
    }
}

#[test]
fn all_slot_ranks_and_endpoints_survive_insert_remove_and_retained_snapshots() {
    let budget = budget();
    let mut state = State::default();
    let group_base = u32::MAX & !63;
    for index in 0..64 {
        let slot = (index * 17 + 63) % 64;
        state
            .mark(group_base + slot, vec![PathElement::Field(slot)], &budget)
            .unwrap();
        let row = partial(&state);
        let expected = row.slots.count_ones() as usize;
        assert_eq!(entries(Some(row)).len(), expected);
        for (slot, paths) in entries(Some(row)) {
            let looked_up = partial_at(Some(row), slot, &budget).unwrap().unwrap();
            assert!(Rc::ptr_eq(paths, looked_up));
            assert_eq!(
                paths.paths,
                BTreeSet::from([vec![PathElement::Field(u32::from(slot))]])
            );
        }
    }
    assert_eq!(partial(&state).slots, u64::MAX);
    let original = state.clone();
    for slot in (0..64).rev() {
        state.clear(group_base + slot, &budget).unwrap();
        assert_eq!(partial(&original).slots, u64::MAX);
        if slot == 0 {
            assert!(state.root.is_none());
        } else {
            let row = partial(&state);
            assert_eq!(row.slots, (1u64 << slot) - 1);
            for retained in 0..slot as u8 {
                assert!(Rc::ptr_eq(
                    partial_at(Some(row), retained, &budget).unwrap().unwrap(),
                    partial_at(Some(partial(&original)), retained, &budget)
                        .unwrap()
                        .unwrap(),
                ));
            }
        }
    }
    drop((original, state));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn whole_slot_filtering_and_ordered_partial_union_keep_original_facts() {
    let budget = budget();
    let mut left = State::default();
    let mut right = State::default();
    for slot in 0..64 {
        left.mark(slot, vec![PathElement::Field(0)], &budget)
            .unwrap();
        right
            .mark(
                slot,
                if slot % 3 == 0 {
                    vec![]
                } else {
                    vec![PathElement::Field(1)]
                },
                &budget,
            )
            .unwrap();
    }
    let original = left.clone();
    assert!(left.merge(&right, &budget).unwrap());
    let row = partial(&left);
    let expected = (0..64)
        .filter(|slot| slot % 3 != 0)
        .fold(0, |mask, slot| mask | (1u64 << slot));
    assert_eq!(row.slots, expected);
    for (_, paths) in entries(Some(row)) {
        assert_eq!(
            paths.paths,
            BTreeSet::from([vec![PathElement::Field(0)], vec![PathElement::Field(1)],])
        );
    }
    assert!(!left.merge(&right, &budget).unwrap());
    for slot in 0..64 {
        assert!(
            !left
                .readable(slot, &[PathElement::Field(0)], &budget)
                .unwrap()
        );
        assert!(
            !left
                .readable(slot, &[PathElement::Field(1)], &budget)
                .unwrap()
        );
        assert!(
            original
                .readable(slot, &[PathElement::Field(1)], &budget)
                .unwrap()
        );
    }
    drop((left, right, original));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn each_sparse_row_capacity_reserves_before_allocation_and_releases_exactly_once() {
    for count in 1..=64 {
        let words = PARTIAL_WORDS + count * PARTIAL_ENTRY_WORDS;
        let budget = Budget::new(17, 23, 17 + words, 100).unwrap();
        let exact = partial_storage(count, &budget).unwrap();
        assert_eq!(budget.0.live.get(), words);
        let Err(Error::Limit {
            resource: Resource::Storage,
            required,
            limit,
            storage: Some(failure),
        }) = partial_storage(count, &budget)
        else {
            panic!("the second retained row must exceed the exact cap");
        };
        assert_eq!((required, limit), (17 + words * 2, 17 + words));
        assert_eq!(
            failure,
            StorageFailure {
                live: words,
                peak: words,
                requested: words
            }
        );
        assert_eq!(budget.work_units(), 0);
        drop(exact);
        assert_eq!(budget.0.live.get(), 0);
        assert_eq!(budget.peak(), words);
        assert!(partial_storage(count, &budget).is_ok());
        assert_eq!(budget.0.live.get(), 0);
    }
}
