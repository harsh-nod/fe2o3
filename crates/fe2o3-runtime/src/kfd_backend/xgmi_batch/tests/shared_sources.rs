use super::*;
use std::cell::Cell;

#[test]
fn complete_shared_roster_is_healthy_but_busy_after_custody_validation() {
    let left = record(1, 0);
    let source = left.source;
    let mut right = record(2, 0);
    right.source = source;
    let active = HashMap::from([(1, left), (2, right)]);
    let ready = [VecDeque::from([1, 2]), VecDeque::new()];
    let checks = Cell::new(0);
    let selected = admit_with_sharing(
        &[2, 1],
        &active,
        &ready,
        &[vec![], vec![]],
        &HashMap::new(),
        Vec::try_reserve_exact,
        |left, right, allocation| {
            assert_eq!((left, right, allocation), (2, 1, source));
            checks.set(checks.get() + 1);
            Ok(true)
        },
    )
    .unwrap();
    assert_eq!(checks.get(), 1);
    assert!(selected.shared_source);
    assert_eq!(
        qualify_selection(selected, false),
        Err(AdmissionError::Corrupt)
    );
    assert_eq!(qualify_selection(selected, true), Err(AdmissionError::Busy));
    assert_eq!(ready[0], [1, 2]);
    assert!(active.values().all(|record| record.ticket.is_none()));
}

#[test]
fn healthy_sharing_does_not_hide_a_later_writer_or_unauthenticated_root() {
    for mutation in 0..4 {
        let left = record(1, 0);
        let source = left.source;
        let mut right = record(2, 0);
        right.source = source;
        let mut third = record(3, 0);
        if mutation == 0 {
            third.destination = source;
        }
        if mutation == 1 {
            third.stream = left.stream;
        }
        let active = HashMap::from([(1, left), (2, right), (3, third)]);
        let result = admit_with_sharing(
            &[1, 2, 3],
            &active,
            &[VecDeque::from([1, 2, 3]), VecDeque::new()],
            &[vec![], vec![]],
            &HashMap::new(),
            Vec::try_reserve_exact,
            |_, _, _| match mutation {
                2 => Ok(false),
                3 => Err(xgmi_directed::OwnerError::Corrupt),
                _ => Ok(true),
            },
        );
        assert_eq!(result, Err(AdmissionError::Corrupt), "mutation {mutation}");
    }
}

#[test]
fn frontier_custody_allows_only_unpublished_siblings_or_dependent_later_owners() {
    let mut other = record(2, 0);
    for selected in [1, 3] {
        assert!(compatible_owner(
            selected,
            &other,
            other.source,
            |_, _, _| Ok(true)
        ));
        assert!(!compatible_owner(
            selected,
            &other,
            other.source,
            |_, _, _| Ok(false)
        ));
        assert!(!compatible_owner(
            selected,
            &other,
            other.source,
            |_, _, _| Err(xgmi_directed::OwnerError::Corrupt)
        ));
    }
    other.dependencies.push(1);
    assert!(compatible_owner(1, &other, other.source, |_, _, _| panic!(
        "dependency suffices"
    )));
    assert!(!compatible_owner(3, &other, other.source, |_, _, _| Ok(
        false
    )));
    assert!(compatible_owner(2, &other, other.source, |_, _, _| panic!(
        "own retain"
    )));
}

#[test]
fn production_owner_roster_checks_both_sibling_orientations_and_corrupt_indexes() {
    for selected in [1, 2] {
        for mutation in 0..8 {
            let left = record(1, 0);
            let source = left.source;
            let mut right = record(2, 0);
            right.source = source;
            let mut active = HashMap::from([(1, left), (2, right)]);
            let mut owners = vec![1, 2];
            match mutation {
                0 | 6 | 7 => (),
                1 => owners.push(1),
                2 => owners.push(3),
                3 => owners.retain(|id| *id != selected),
                4 => active.get_mut(&2).unwrap().id = 3,
                5 => active.get_mut(&2).unwrap().source = 99,
                _ => unreachable!(),
            }
            let valid =
                valid_owner_roster(
                    selected,
                    source,
                    &owners,
                    &active,
                    |_, _, _| match mutation {
                        6 => Err(xgmi_directed::OwnerError::Corrupt),
                        7 => Ok(false),
                        _ => Ok(true),
                    },
                );
            assert_eq!(
                valid,
                mutation == 0,
                "selected {selected}, mutation {mutation}"
            );
        }
    }
}
