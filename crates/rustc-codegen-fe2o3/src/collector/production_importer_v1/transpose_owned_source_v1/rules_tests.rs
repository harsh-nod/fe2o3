use super::*;

// Synthetic component facts only. None of these tests constructs a live source
// receipt, authenticates a provider or substitutes for actual AMD callbacks.
#[test]
fn three_owned_relations_and_complete_shared_roster_are_required() {
    let mut work = 64;
    let mut uses = Uses::default();
    uses.observe(BindingRole::IssuedTile, SourceUse::StageTile, &mut work)
        .unwrap();
    uses.observe(BindingRole::StagedTile, SourceUse::PublishTile, &mut work)
        .unwrap();
    uses.observe(
        BindingRole::Workgroup,
        SourceUse::SharedWorkgroup,
        &mut work,
    )
    .unwrap();
    uses.observe(
        BindingRole::Workgroup,
        SourceUse::PublishWorkgroup,
        &mut work,
    )
    .unwrap();
    uses.finish(1).unwrap();
    assert_eq!(work, 60);
    assert_eq!(
        Uses::default().finish(0),
        Err(Error::Source("owned source-use roster is incomplete"))
    );
}

#[test]
fn substitutions_borrows_and_other_binding_uses_are_not_owned_consumption() {
    for role in [
        BindingRole::IssuedTile,
        BindingRole::StagedTile,
        BindingRole::Workgroup,
    ] {
        let mut work = 8;
        assert_eq!(
            Uses::default().observe(role, SourceUse::Other, &mut work),
            Err(Error::Source("owned binding has an unmodeled source use"))
        );
    }
    assert_eq!(
        Uses::default().observe(BindingRole::IssuedTile, SourceUse::PublishTile, &mut 8),
        Err(Error::Source("owned binding has an unmodeled source use"))
    );
}

#[test]
fn duplicate_owned_consumption_is_rejected_before_publication() {
    let mut uses = Uses::default();
    let mut work = 8;
    uses.observe(BindingRole::StagedTile, SourceUse::PublishTile, &mut work)
        .unwrap();
    assert_eq!(
        uses.observe(BindingRole::StagedTile, SourceUse::PublishTile, &mut work),
        Err(Error::Source("owned binding is consumed more than once"))
    );
}

#[test]
fn normal_return_is_exact_and_does_not_ignore_dead_bypass_edges() {
    immediate_normal_edge(5, 6, 6, 7, 0, [5], &mut 8).unwrap();
    for predecessors in [vec![5, 9], vec![5, 6], vec![], vec![5, 5]] {
        let error =
            immediate_normal_edge(5, 6, 6, 7, 0, predecessors.clone(), &mut 16).unwrap_err();
        assert_eq!(
            error,
            if predecessors == [5, 9] || predecessors == [5, 6] {
                Error::Source("Publish has a bypass or reentry predecessor")
            } else {
                Error::Source("Publish normal predecessor is not unique")
            }
        );
    }
}

#[test]
fn changed_targets_and_intervening_statement_do_not_commute() {
    for (next, after, statements) in [(7, 8, 0), (6, 5, 0), (6, 6, 0), (6, 7, 1)] {
        assert_eq!(
            immediate_normal_edge(5, 6, next, after, statements, [5], &mut 8),
            Err(Error::Source(
                "Publish is not the exact immediate normal-return use"
            ))
        );
    }
}

#[test]
fn source_work_boundary_is_exact_and_no_failure_refunds_it() {
    let mut work = 2;
    immediate_normal_edge(5, 6, 6, 7, 0, [5], &mut work).unwrap();
    assert_eq!(work, 0);
    assert_eq!(
        immediate_normal_edge(5, 6, 6, 7, 0, [5], &mut work),
        Err(Error::Work)
    );
    assert_eq!(work, 0);
    let mut one = 1;
    assert_eq!(
        immediate_normal_edge(5, 6, 6, 7, 0, [5], &mut one),
        Err(Error::Work)
    );
    assert_eq!(one, 0);
}

#[test]
fn local_audit_requires_actual_definition_and_exact_use_multiplicity() {
    let expected = [(3, 0, LocalAction::Define), (5, 2, LocalAction::Move)];
    exact_local_actions(expected, &expected, &mut 128).unwrap();
    assert_eq!(
        exact_local_actions([expected[0]], &expected, &mut 128),
        Err(Error::Source("retained owned local lost a required use"))
    );
    for extra in [
        expected[1],
        (5, 3, LocalAction::Copy),
        (4, 0, LocalAction::SharedBorrow),
        (9, 1, LocalAction::Other),
    ] {
        assert_eq!(
            exact_local_actions([expected[0], expected[1], extra], &expected, &mut 128),
            Err(Error::Source(
                "retained owned local has an extra or changed use"
            ))
        );
    }
}

#[test]
fn retained_local_role_or_projection_change_is_not_an_equivalent_move() {
    let expected = [(3, 0, LocalAction::Define), (5, 2, LocalAction::Move)];
    for action in [
        LocalAction::Copy,
        LocalAction::Other,
        LocalAction::SharedBorrow,
    ] {
        assert_eq!(
            exact_local_actions([expected[0], (5, 2, action)], &expected, &mut 128),
            Err(Error::Source(
                "retained owned local has an extra or changed use"
            ))
        );
    }
}
