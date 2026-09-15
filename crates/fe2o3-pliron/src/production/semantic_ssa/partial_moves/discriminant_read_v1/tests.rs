use super::super::state::{Budget, Error, PathElement::*, Resource, State};

fn exercise(storage: usize, work: usize) -> Result<(usize, usize), Error> {
    let budget = Budget::new(0, 0, storage, work)?;
    let mut state = State::default();
    state.mark(7, vec![Field(2), Downcast(1), Field(0)], &budget)?;
    let peak = budget.peak();
    assert!(state.direct_tag_readable(7, &[Field(2)], &budget)?);
    assert!(!state.readable(7, &[Field(2)], &budget)?);
    assert!(!state.direct_tag_readable(7, &[Field(2), Downcast(1), Field(0)], &budget)?);
    assert_eq!(
        budget.peak(),
        peak,
        "tag query creates no persistent state or temporary path set"
    );
    Ok((peak, budget.work_units()))
}

#[test]
fn direct_tag_queries_use_existing_storage_and_inclusive_work_budget() {
    let (storage, work) = exercise(usize::MAX, usize::MAX).unwrap();
    assert_eq!(exercise(storage, work).unwrap(), (storage, work));
    assert!(matches!(
        exercise(storage - 1, work),
        Err(Error::Limit {
            resource: Resource::Storage,
            ..
        })
    ));
    assert!(matches!(
        exercise(storage, work - 1),
        Err(Error::Limit {
            resource: Resource::Work,
            ..
        })
    ));
}

#[test]
fn direct_tag_state_join_retains_every_equal_or_ancestor_hole() {
    for hole in [
        vec![],
        vec![Field(2)],
        vec![Field(2), Downcast(1), Field(0)],
    ] {
        let budget = Budget::new(0, 0, usize::MAX, usize::MAX).unwrap();
        let mut left = State::default();
        left.mark(7, vec![Field(2), Downcast(1), Field(1)], &budget)
            .unwrap();
        let mut right = State::default();
        right.mark(7, hole.clone(), &budget).unwrap();
        left.merge(&right, &budget).unwrap();
        assert_eq!(
            left.direct_tag_readable(7, &[Field(2)], &budget).unwrap(),
            hole.len() > 1
        );
        assert!(!left.readable(7, &[Field(2)], &budget).unwrap());
    }
}
