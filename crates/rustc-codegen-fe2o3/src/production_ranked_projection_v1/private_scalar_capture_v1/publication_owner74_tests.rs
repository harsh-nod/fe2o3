use super::*;

#[test]
fn publication_owner74_exact_read_rosters_and_late_cycle_remain_source_bound() {
    for (scalars, diamonds, killed) in [
        (0, 4, false),
        (16, 6, false),
        (64, 24, false),
        (16, 6, true),
    ] {
        let owner = owner(scalars, diamonds, killed);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let body = view.body().clone();
        let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
        assert!(
            reads.observation.completed_without_exhaustion(),
            "{:?}",
            reads.observation
        );
        let expected = if killed { vec![] } else { vec![(4, 2)] };
        assert_eq!(roster(&reads, view.body()), expected);
        assert_eq!(view.body(), &body);
        assert!(!reads.contains(&body, &body.blocks()[4].statements()[0]));
        println!(
            "publication74 scalars={scalars} diamonds={diamonds} killed={killed} work={} roster={expected:?}",
            MAX_WORK - reads.observation.remaining_work
        );
    }
}

#[test]
fn publication_owner74_budget_failure_never_publishes_a_pre_fixed_point_read() {
    let owner = owner(16, 6, true);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut full = Analysis::new(owner.source_semantic().types(), view);
    let expected = full.run().unwrap();
    assert!(expected.is_empty());
    let used = MAX_WORK - full.budget.remaining;
    for available in [0, 1, used / 2, used - 1, used] {
        let mut analysis = Analysis::new(owner.source_semantic().types(), view);
        analysis.budget = Budget::new(available);
        let result = analysis.run();
        if available == used {
            assert_eq!(result.unwrap(), expected);
            assert!(analysis.observation.completed_without_exhaustion());
        } else {
            assert!(result.is_err());
            assert_eq!(analysis.observation.admitted_reads, 0);
            assert!(!analysis.observation.completed);
            assert!(analysis.observation.work_exhausted);
        }
        assert!(analysis.budget.reserve(MAX_STORAGE).is_ok());
    }
}
