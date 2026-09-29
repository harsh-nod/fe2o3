// Shared arithmetic/iterator bodies, not the fallible allocation wrapper,
// ExactSizeIterator selector, mapped caller projection or batch commit.
use std::collections::HashMap;
use vstd::prelude::*;
use vstd::std_specs::iter::IteratorSpec;

include!("../../fe2o3-kfd/src/queue_completion/release_pin_budget_body.rs");

verus! {
broadcast use vstd::std_specs::hash::group_hash_axioms;

spec fn occurrences(rows: Seq<(u32, u32)>, key: u32) -> nat
    decreases rows.len(),
{
    if rows.len() == 0 { 0 } else {
        occurrences(rows.drop_first(), key) + if rows[0].0 == key { 1nat } else { 0nat }
    }
}

spec fn first_budget(rows: Seq<(u32, u32)>, key: u32) -> u32
    decreases rows.len(),
{
    if rows.len() == 0 { 0 } else if rows[0].0 == key { rows[0].1 }
    else { first_budget(rows.drop_first(), key) }
}

spec fn capacity(rows: Seq<(u32, u32)>, remaining: Map<u32, u32>, key: u32) -> u32 {
    if remaining.contains_key(key) { remaining[key] } else { first_budget(rows, key) }
}

spec fn sufficient(rows: Seq<(u32, u32)>, remaining: Map<u32, u32>) -> bool {
    forall|key: u32| #[trigger] occurrences(rows, key) <= capacity(rows, remaining, key)
}

broadcast proof fn debit_preserves_decision(rows: Seq<(u32, u32)>, remaining: Map<u32, u32>)
    requires rows.len() > 0, capacity(rows, remaining, rows[0].0) > 0,
    ensures #[trigger] sufficient(rows, remaining) == sufficient(rows.drop_first(),
        remaining.insert(rows[0].0, (capacity(rows, remaining, rows[0].0) - 1) as u32)),
{
    let key = rows[0].0;
    let tail = rows.drop_first();
    let next = remaining.insert(key, (capacity(rows, remaining, key) - 1) as u32);
    if sufficient(rows, remaining) {
        assert forall|other: u32| #[trigger] occurrences(tail, other) <= capacity(tail, next, other) by {
            assert(occurrences(rows, other) <= capacity(rows, remaining, other));
            if other == key { } else { }
        }
    }
    if sufficient(tail, next) {
        assert forall|other: u32| #[trigger] occurrences(rows, other) <= capacity(rows, remaining, other) by {
            assert(occurrences(tail, other) <= capacity(tail, next, other));
            if other == key { } else { }
        }
    }
}

fn validate_single_release_pin_budget<I: Iterator<Item = (u32, u32)>, E>(
    mut budgets: I, insufficient: E,
) -> (out: Result<(), E>)
    requires budgets.obeys_prophetic_iter_laws(),
    ensures out.is_ok() == (budgets.remaining().len() == 0 || budgets.remaining()[0].1 > 0),
        match out { Err(error) => error == insufficient, _ => true },
{
    completion_single_release_pin_budget_body!(verus_exec_expr, budgets, insufficient)
}

fn validate_reserved_release_pin_budgets<I: Iterator<Item = (u32, u32)>, E>(
    input: I, mut remaining: HashMap<u32, u32>, insufficient: E,
) -> (out: Result<(), E>)
    requires input.obeys_prophetic_iter_laws(), input.decrease().is_some(),
        remaining@ == Map::<u32, u32>::empty(),
    ensures out.is_ok() == sufficient(input.remaining(), Map::empty()),
        match out { Err(error) => error == insufficient, _ => true },
{
    broadcast use debit_preserves_decision;
    let mut budgets = input;
    completion_reserved_release_pin_budgets_body!(@annotated verus_exec_expr,
        budgets, remaining, insufficient, index, pins, available,
        [],
        [invariant budgets.obeys_prophetic_iter_laws(), budgets.decrease().is_some(),
            sufficient(input.remaining(), Map::empty()) == sufficient(budgets.remaining(), remaining@),
         decreases budgets.decrease().unwrap(),],
        [let ghost before_iter = budgets; let ghost before = remaining@;],
        [proof {
            assert(before_iter.remaining().len() > 0 && before_iter.remaining()[0] == (index, pins));
            assert(occurrences(before_iter.remaining(), index) > 0);
            assert(capacity(before_iter.remaining(), before, index) == 0 ==> !sufficient(before_iter.remaining(), before));
        }],
        [proof { assert(*available == capacity(before_iter.remaining(), before, index)); }],
        [proof {
            assert(remaining@ =~= before.insert(index, (capacity(before_iter.remaining(), before, index) - 1) as u32));
            debit_preserves_decision(before_iter.remaining(), before);
            assert(sufficient(before_iter.remaining(), before) == sufficient(budgets.remaining(), remaining@));
        }])
}

fn accepted_alias_witness<E>(insufficient: E) {
    let mut rows = Vec::new();
    rows.push((3u32, 2u32));
    rows.push((3u32, 0u32));
    proof {
        reveal_with_fuel(occurrences, 3);
        reveal_with_fuel(first_budget, 3);
        assert(sufficient(rows@, Map::empty()));
    }
    let out = validate_reserved_release_pin_budgets(rows.into_iter(), HashMap::new(), insufficient);
    assert(out.is_ok());
}

fn rejected_alias_witness<E>(insufficient: E) {
    let mut rows = Vec::new();
    rows.push((3u32, 1u32));
    rows.push((3u32, 9u32));
    proof {
        reveal_with_fuel(occurrences, 3);
        assert(occurrences(rows@, 3) == 2);
        assert(!sufficient(rows@, Map::empty()));
    }
    let out = validate_reserved_release_pin_budgets(rows.into_iter(), HashMap::new(), insufficient);
    assert(out.is_err());
}
}
