use super::*;

fn superset_pair(budget: &Budget) -> (State, State) {
    let mut destination = State::default();
    destination.mark(7, field(0), budget).unwrap();
    let mut source = destination.clone();
    source.mark(7, field(1), budget).unwrap();
    (destination, source)
}

fn row(state: &State) -> &Rc<PartialPaths> {
    let Kind::Leaf { partial, .. } = &state.root.as_ref().unwrap().kind else {
        panic!("single-group fixture");
    };
    partial.as_ref().unwrap()
}

fn assert_live(budget: &Budget, states: &[&State]) {
    assert_eq!(
        allocation_profile(states.iter().copied()).total(),
        budget.0.live.get()
    );
}

#[test]
fn exact_superset_paths_table_and_leaf_reuse_incoming_without_allocation() {
    let budget = budget();
    let (mut destination, source) = superset_pair(&budget);
    let saved = destination.clone();
    let live = budget.0.live.get();
    let peak = budget.peak();
    let mut expected = snapshot(&saved);
    assert!(reference_merge(&mut expected, &snapshot(&source)));
    assert!(destination.merge(&source, &budget).unwrap());
    assert_eq!(snapshot(&destination), expected);
    assert!(same_root(&destination.root, &source.root));
    assert!(Rc::ptr_eq(row(&destination), row(&source)));
    assert_eq!(budget.0.live.get(), live);
    assert_eq!(budget.peak(), peak);
    assert!(!destination.merge(&source, &budget).unwrap());
    assert!(saved.readable(7, &field(1), &budget).unwrap());
    assert_live(&budget, &[&destination, &source, &saved]);
    drop((destination, source, saved));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn incomparable_slots_need_new_table_but_share_the_incoming_path_union() {
    let budget = budget();
    let (mut destination, mut source) = superset_pair(&budget);
    destination.mark(8, field(2), &budget).unwrap();
    source.mark(9, field(3), &budget).unwrap();
    let saved = destination.clone();
    let mut expected = snapshot(&saved);
    reference_merge(&mut expected, &snapshot(&source));
    assert!(destination.merge(&source, &budget).unwrap());
    assert_eq!(snapshot(&destination), expected);
    assert!(!Rc::ptr_eq(row(&destination), row(&source)));
    assert!(!Rc::ptr_eq(row(&destination), row(&saved)));
    assert_eq!(row(&destination).slots, (1 << 7) | (1 << 8) | (1 << 9));
    assert!(Rc::ptr_eq(
        partial_at(Some(row(&destination)), 7, &budget)
            .unwrap()
            .unwrap(),
        partial_at(Some(row(&source)), 7, &budget).unwrap().unwrap(),
    ));
    assert!(source.readable(8, &field(2), &budget).unwrap());
    assert!(saved.readable(9, &field(3), &budget).unwrap());
    assert_live(&budget, &[&destination, &source, &saved]);
    drop((destination, source, saved));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn whole_filter_cannot_reuse_an_incoming_table_with_an_extra_slot() {
    let budget = budget();
    let mut destination = State::default();
    destination.mark(7, vec![], &budget).unwrap();
    let mut source = State::default();
    source
        .mark(
            7,
            vec![PathElement::Downcast(1), PathElement::Field(0)],
            &budget,
        )
        .unwrap();
    source.mark(8, field(1), &budget).unwrap();
    let saved = destination.clone();
    let mut expected = snapshot(&saved);
    reference_merge(&mut expected, &snapshot(&source));
    assert!(destination.merge(&source, &budget).unwrap());
    assert_eq!(snapshot(&destination), expected);
    assert_eq!(row(&destination).slots, 1 << 8);
    assert!(!Rc::ptr_eq(row(&destination), row(&source)));
    assert!(!destination.direct_tag_readable(7, &[], &budget).unwrap());
    assert!(source.direct_tag_readable(7, &[], &budget).unwrap());
    assert_live(&budget, &[&destination, &source, &saved]);
    drop((destination, source, saved));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn prefix_downcast_and_index_direction_are_not_exact_path_containment() {
    let pairs = [
        (vec![PathElement::Field(0), PathElement::Field(1)], field(0)),
        (
            vec![PathElement::Downcast(0), PathElement::Field(1)],
            vec![PathElement::Downcast(1), PathElement::Field(1)],
        ),
        (
            vec![PathElement::ConstantIndex {
                offset: 3,
                from_end: false,
            }],
            vec![PathElement::ConstantIndex {
                offset: 3,
                from_end: true,
            }],
        ),
    ];
    for (old, new) in pairs {
        let budget = budget();
        let mut destination = State::default();
        let mut source = State::default();
        destination.mark(7, old.clone(), &budget).unwrap();
        source.mark(7, new.clone(), &budget).unwrap();
        let saved = destination.clone();
        let mut expected = snapshot(&saved);
        reference_merge(&mut expected, &snapshot(&source));
        assert!(destination.merge(&source, &budget).unwrap());
        assert_eq!(snapshot(&destination), expected);
        assert_eq!(expected[&7].len(), 2);
        assert!(!Rc::ptr_eq(
            &row(&destination).entries[0],
            &row(&source).entries[0]
        ));
        assert!(!Rc::ptr_eq(
            &row(&destination).entries[0],
            &row(&saved).entries[0]
        ));
        assert_eq!(
            destination.initialize(7, &old, &budget).unwrap(),
            reference_initialize(&mut expected, 7, &old)
        );
        assert_eq!(snapshot(&destination), expected);
        assert_live(&budget, &[&destination, &source, &saved]);
        drop((destination, source, saved));
        assert_eq!(budget.0.live.get(), 0);
    }
}

#[test]
fn charged_containment_errors_keep_owners_and_release_all_temporaries() {
    let broad = budget();
    let (mut left, right) = superset_pair(&broad);
    let setup = broad.work_units();
    left.merge(&right, &broad).unwrap();
    let needed = broad.work_units() - setup;
    assert!(needed > 5);
    drop((left, right));
    for remaining in 0..=needed {
        let limit = 13 + setup + remaining;
        let budget = Budget::new(0, 13, usize::MAX, limit).unwrap();
        let (mut left, right) = superset_pair(&budget);
        let saved = left.clone();
        let result = left.merge(&right, &budget);
        if remaining == needed {
            assert_eq!(result, Ok(true));
            assert!(same_root(&left.root, &right.root));
        } else {
            let Err(Error::Limit {
                resource: Resource::Work,
                required,
                limit: actual,
                storage: None,
            }) = result
            else {
                panic!("every incomplete charged search must fail: {result:?}");
            };
            assert_eq!(actual, limit);
            assert!(required > limit);
            assert!(same_root(&left.root, &saved.root));
        }
        assert_live(&budget, &[&left, &right, &saved]);
        drop((left, right, saved));
        assert_eq!(budget.0.live.get(), 0);
    }
}

#[test]
fn incomparable_path_union_keeps_exact_storage_rejection_and_snapshots() {
    let broad = budget();
    let mut a = State::default();
    let mut b = State::default();
    a.mark(7, field(0), &broad).unwrap();
    b.mark(7, field(1), &broad).unwrap();
    let live = broad.0.live.get();
    assert_eq!(live, broad.peak());
    drop((a, b));
    let budget = Budget::new(17, 0, 17 + live, usize::MAX).unwrap();
    let mut left = State::default();
    let mut right = State::default();
    left.mark(7, field(0), &budget).unwrap();
    right.mark(7, field(1), &budget).unwrap();
    let saved = left.clone();
    let request = PARTIAL_WORDS + PARTIAL_ENTRY_WORDS;
    assert_eq!(
        left.merge(&right, &budget),
        Err(Error::Limit {
            resource: Resource::Storage,
            required: 17 + live + request,
            limit: 17 + live,
            storage: Some(StorageFailure {
                live,
                peak: live,
                requested: request
            }),
        })
    );
    assert!(same_root(&left.root, &saved.root));
    assert_live(&budget, &[&left, &right, &saved]);
    drop((left, right, saved));
    assert_eq!(budget.0.live.get(), 0);
}
