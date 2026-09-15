use super::*;

#[test]
fn unchanged_private_join_checked_owner_keeps_complete_rosters_and_late_backedge_rejection() {
    for (scalars, diamonds, killed) in [
        (0, 4, false),
        (16, 6, false),
        (64, 24, false),
        (16, 6, true),
    ] {
        let owner = owner(scalars, diamonds, killed);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let before = view.body().clone();
        let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
        assert!(
            reads.observation.completed_without_exhaustion(),
            "{:?}",
            reads.observation
        );
        let expected = if killed { vec![] } else { vec![(4, 2)] };
        assert_eq!(roster(&reads, view.body()), expected);
        assert_eq!(view.body(), &before);
        println!(
            "equal_join68_owner scalars={scalars} diamonds={diamonds} killed={killed} work={} roster={expected:?}",
            MAX_WORK - reads.observation.remaining_work
        );
    }
}

#[test]
fn unchanged_private_join_checked_owner_keeps_final_publication_and_same_budget_exhaustion() {
    let owner = owner(16, 6, false);
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let types = owner.source_semantic().types();
    let mut complete = Analysis::new(types, view);
    let expected = complete.run().unwrap();
    let used = MAX_WORK - complete.observation.remaining_work;
    for available in [0, 1, used / 2, used - 1, used] {
        let mut bounded = Analysis::new(types, view);
        bounded.budget = Budget::new(available);
        let result = bounded.run();
        if available == used {
            assert_eq!(result.unwrap(), expected);
            assert!(bounded.observation.completed_without_exhaustion());
        } else {
            assert!(result.is_err());
            assert!(bounded.observation.work_exhausted);
            assert!(!bounded.observation.completed);
            assert_eq!(bounded.observation.admitted_reads, 0);
        }
    }
}
