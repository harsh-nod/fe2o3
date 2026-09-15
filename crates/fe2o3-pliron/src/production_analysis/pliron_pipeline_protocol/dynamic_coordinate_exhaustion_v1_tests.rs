use super::*;

fn counters(meter: &EquivalenceResourceMeterV1) -> (usize, usize, usize, usize) {
    (
        meter.queries,
        meter.expanded_pairs,
        meter.cursor_steps,
        meter.memo.len(),
    )
}

#[test]
fn already_exhausted_never_accepts_empty_or_zero_arity_membership() {
    let (context, [one_a, zero_a, _, _]) = constants();
    for (left, right) in [
        (vec![], vec![]),
        (vec![], vec![vec![]]),
        (vec![vec![]], vec![]),
        (vec![vec![]], vec![vec![]]),
        (vec![vec![zero_a]], vec![vec![zero_a]]),
    ] {
        let mut meter = EquivalenceResourceMeterV1::new(0, 2).unwrap();
        assert!(!index_values_equivalent(
            &context, one_a, zero_a, &mut meter
        ));
        assert!(meter.exhausted());
        assert_eq!(counters(&meter), (1, 0, 0, 0));
        assert!(!coordinate_sets_equivalent_v1(
            &context, &left, &right, &mut meter
        ));
        assert_eq!(counters(&meter), (1, 0, 0, 0));
    }
}

#[test]
fn nonexhausted_empty_and_zero_arity_membership_keeps_existing_semantics() {
    let (context, _) = constants();
    for (left, right, expected) in [
        (vec![], vec![], true),
        (vec![vec![]], vec![vec![], vec![]], true),
        (vec![], vec![vec![]], false),
        (vec![vec![]], vec![], false),
    ] {
        let mut meter = EquivalenceResourceMeterV1::new(0, 0).unwrap();
        assert_eq!(
            coordinate_sets_equivalent_v1(&context, &left, &right, &mut meter),
            expected
        );
        assert!(!meter.exhausted());
        assert_eq!(counters(&meter), (0, 0, 0, 0));
    }
}

#[test]
fn query_denial_stops_before_later_identical_candidate() {
    let (context, [one_a, zero_a, zero_b, one_b]) = constants();
    let left = vec![vec![zero_a], vec![one_b]];
    let right = vec![vec![one_a], vec![zero_b], vec![zero_a]];
    // The first unequal pair consumes the sole admitted query and expansion.
    // The second query is denied before its cursor/expansion; the third
    // candidate is the needle itself but must never be queried after denial.
    let mut meter = EquivalenceResourceMeterV1::new(1, 8).unwrap();
    assert!(!coordinate_sets_equivalent_v1(
        &context, &left, &right, &mut meter
    ));
    assert!(meter.exhausted());
    assert_eq!(counters(&meter), (2, 1, 1, 1));
    assert!(!coordinate_sets_equivalent_v1(
        &context, &left, &right, &mut meter
    ));
    assert_eq!(counters(&meter), (2, 1, 1, 1));
}

#[test]
fn pair_denial_stops_before_later_identical_candidate() {
    let (context, [one_a, zero_a, zero_b, one_b]) = constants();
    let left = vec![vec![zero_a], vec![one_b]];
    let right = vec![vec![one_a], vec![zero_b], vec![zero_a]];
    // The first unequal pair is memoized. The second admitted query attempts
    // the second unique pair and fails before memo insertion. Unlike a query
    // denial, the denied expansion still follows its single cursor charge.
    let mut meter = EquivalenceResourceMeterV1::new(16, 1).unwrap();
    assert!(!coordinate_sets_equivalent_v1(
        &context, &left, &right, &mut meter
    ));
    assert!(meter.exhausted());
    assert_eq!(counters(&meter), (2, 2, 2, 1));
    assert!(!coordinate_sets_equivalent_v1(
        &context, &left, &right, &mut meter
    ));
    assert_eq!(counters(&meter), (2, 2, 2, 1));
}
