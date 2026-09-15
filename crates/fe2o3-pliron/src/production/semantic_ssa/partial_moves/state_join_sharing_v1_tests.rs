use super::*;

fn assert_live(budget: &Budget, states: &[&State]) {
    assert_eq!(
        budget.0.live.get(),
        allocation_profile(states.iter().copied()).total()
    );
    assert!(budget.peak() >= budget.0.live.get());
}

#[test]
fn whole_leaf_union_reuses_the_exact_incoming_owner() {
    let budget = budget();
    let mut destination = State::default();
    destination.mark(0, vec![], &budget).unwrap();
    let snapshot = destination.clone();
    let mut source = destination.clone();
    source.mark(1, vec![], &budget).unwrap();
    let retained = budget.0.live.get();
    let peak = budget.peak();
    let work = budget.work_units();
    assert!(destination.merge(&source, &budget).unwrap());
    assert!(same_root(&destination.root, &source.root));
    assert_eq!(budget.0.live.get(), retained);
    assert_eq!(budget.peak(), peak);
    assert_eq!(budget.work_units() - work, 5);
    assert!(snapshot.readable(1, &[], &budget).unwrap());
    assert!(!destination.readable(1, &[], &budget).unwrap());
    destination.clear(1, &budget).unwrap();
    assert!(!source.readable(1, &[], &budget).unwrap());
    assert_live(&budget, &[&destination, &snapshot, &source]);
    drop((destination, snapshot, source));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn shared_partial_row_and_whole_dominance_preserve_read_facets() {
    let budget = budget();
    let payload = vec![PathElement::Downcast(1), PathElement::Field(0)];
    let mut destination = State::default();
    destination.mark(7, payload, &budget).unwrap();
    let snapshot = destination.clone();
    let mut source = destination.clone();
    source.mark(0, vec![], &budget).unwrap();
    assert!(destination.merge(&source, &budget).unwrap());
    assert!(same_root(&destination.root, &source.root));
    assert!(destination.direct_tag_readable(7, &[], &budget).unwrap());
    assert!(!destination.readable(7, &[], &budget).unwrap());

    source.mark(7, vec![], &budget).unwrap();
    assert!(destination.merge(&source, &budget).unwrap());
    assert!(same_root(&destination.root, &source.root));
    assert!(!destination.direct_tag_readable(7, &[], &budget).unwrap());
    assert!(snapshot.direct_tag_readable(7, &[], &budget).unwrap());
    assert_live(&budget, &[&destination, &snapshot, &source]);
}

#[test]
fn unrelated_partial_owners_and_incomparable_bitmaps_require_a_union() {
    let budget = budget();
    let mut destination = State::default();
    let mut source = State::default();
    destination.mark(7, field(0), &budget).unwrap();
    destination.mark(0, vec![], &budget).unwrap();
    source.mark(7, field(1), &budget).unwrap();
    source.mark(1, vec![], &budget).unwrap();
    let saved = destination.clone();
    let mut expected = snapshot(&destination);
    assert!(reference_merge(&mut expected, &snapshot(&source)));
    assert!(destination.merge(&source, &budget).unwrap());
    assert_eq!(snapshot(&destination), expected);
    assert!(!same_root(&destination.root, &source.root));
    assert!(!same_root(&destination.root, &saved.root));
    assert!(source.readable(0, &[], &budget).unwrap());
    assert!(source.readable(7, &field(0), &budget).unwrap());
    assert!(saved.readable(7, &field(1), &budget).unwrap());
    assert_live(&budget, &[&destination, &saved, &source]);
}

#[test]
fn no_change_keeps_destination_owner_even_for_distinct_equal_leaves() {
    let budget = budget();
    let mut destination = State::default();
    let mut source = State::default();
    for local in [0, 63, 64, u32::MAX] {
        destination.mark(local, vec![], &budget).unwrap();
        source.mark(local, vec![], &budget).unwrap();
    }
    let saved = destination.clone();
    let peak = budget.peak();
    assert!(!destination.merge(&source, &budget).unwrap());
    assert!(same_root(&destination.root, &saved.root));
    assert_eq!(budget.peak(), peak);
    assert_live(&budget, &[&destination, &saved, &source]);
}

#[test]
fn normal_return_and_unwind_join_reuses_owner_without_killing_payload_holes() {
    let budget = budget();
    let payload = vec![PathElement::Downcast(1), PathElement::Field(0)];
    let mut after_arguments = State::default();
    after_arguments.mark(0, vec![], &budget).unwrap();
    after_arguments.mark(7, payload.clone(), &budget).unwrap();
    let unwind = after_arguments.clone();
    let mut normal_return = after_arguments.clone();
    assert!(normal_return.initialize(7, &payload, &budget).unwrap());
    assert!(normal_return.readable(7, &payload, &budget).unwrap());

    // Parallel normal/unwind edges to the same target must still union both
    // states; only the normal-return edge initializes the call destination.
    let mut incoming = normal_return.clone();
    assert!(incoming.merge(&unwind, &budget).unwrap());
    assert!(same_root(&incoming.root, &unwind.root));
    assert!(!incoming.readable(7, &payload, &budget).unwrap());
    assert!(incoming.direct_tag_readable(7, &[], &budget).unwrap());
    assert!(!incoming.readable(0, &[], &budget).unwrap());
    assert!(!incoming.initialize(0, &field(0), &budget).unwrap());
    assert!(!incoming.merge(&normal_return, &budget).unwrap());
    assert_live(
        &budget,
        &[&after_arguments, &unwind, &normal_return, &incoming],
    );
}

#[test]
fn branch_allocations_still_report_exact_live_peak_and_request() {
    // A disjoint source leaf requires a genuinely new connecting branch.
    let budget = Budget::new(17, 0, 17 + 4 * NODE_WORDS, usize::MAX).unwrap();
    let mut destination = State::default();
    destination.mark(0, vec![], &budget).unwrap();
    destination.mark(64, vec![], &budget).unwrap();
    let saved = destination.clone();
    let mut source = State::default();
    source.mark(128, vec![], &budget).unwrap();
    assert_live(&budget, &[&destination, &saved, &source]);
    let live = budget.0.live.get();
    let peak = budget.peak();
    assert_eq!(live, 4 * NODE_WORDS);
    assert_eq!(
        destination.merge(&source, &budget),
        Err(Error::Limit {
            resource: Resource::Storage,
            required: 17 + live + NODE_WORDS,
            limit: 17 + 4 * NODE_WORDS,
            storage: Some(StorageFailure {
                live,
                peak,
                requested: NODE_WORDS
            }),
        })
    );
    assert!(same_root(&destination.root, &saved.root));
    assert_live(&budget, &[&destination, &saved, &source]);
    drop((destination, saved, source));
    assert_eq!(budget.0.live.get(), 0);
}

#[test]
fn reused_leaf_keeps_update_work_gate_and_exact_snapshot_on_failure() {
    for remaining in 0..5 {
        // Six setup units and the same accounting owner for every snapshot.
        let limited = Budget::new(0, 13, usize::MAX, 19 + remaining).unwrap();
        let mut destination = State::default();
        destination.mark(0, vec![], &limited).unwrap();
        let mut source = destination.clone();
        source.mark(1, vec![], &limited).unwrap();
        assert_eq!(limited.work_units(), 6);
        let before = destination.clone();
        assert_eq!(
            destination.merge(&source, &limited),
            Err(Error::Limit {
                resource: Resource::Work,
                required: 20 + remaining,
                limit: 19 + remaining,
                storage: None,
            })
        );
        assert!(same_root(&destination.root, &before.root));
        assert_live(&limited, &[&destination, &before, &source]);
        drop((destination, before, source));
        assert_eq!(limited.0.live.get(), 0);
    }
}
