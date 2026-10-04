use super::*;
use crate::ShellLimits;

fn session() -> PlironSession {
    PlironSession::new(ShellLimits::default(), []).expect("fresh session")
}

fn dead_constant(session: &mut PlironSession) -> OperationHandle {
    session
        .import_operation_text_v1(
            r#"builtin.module @custody {
                ^entry():
                dead = builtin.constant <builtin.integer <7: i64>> : builtin.integer i64
            }"#,
        )
        .expect("verified module")
}

#[test]
fn changed_pass_invalidates_old_custody_and_noop_preserves_new_analysis() {
    let mut session = session();
    let root = dead_constant(&mut session);
    let before = session.analyze_operation_graph_v1(&root).unwrap();
    let report = session
        .execute_optimization_v1(&root, &PlironOptimizationPlanV1::dead_code_elimination())
        .unwrap();
    let pass = report.passes()[0];
    assert!(pass.changed());
    assert_eq!(pass.input_epoch(), before.snapshot().epoch());
    assert_eq!(
        pass.output_epoch().sequence(),
        pass.input_epoch().sequence() + 1
    );
    assert_eq!(pass.invalidated_analysis_count(), 1);
    assert_eq!(pass.preserved_analysis_count(), 0);
    assert_eq!(
        session.require_operation_graph_snapshot_v1(&root, before.snapshot()),
        Err(OperationHandleError::OperationGraphSnapshotMismatch)
    );
    let after = session.analyze_operation_graph_v1(&root).unwrap();
    assert_eq!(report.final_graph_identity(), after.replay_identity());
    assert_eq!(after.tree_work(), report.final_graph_work());
    assert_eq!(after.operation_count(), 1);

    let noop = session
        .execute_optimization_v1(&root, &PlironOptimizationPlanV1::dead_code_elimination())
        .unwrap();
    let pass = noop.passes()[0];
    assert!(!pass.changed());
    assert_eq!(pass.input_epoch(), after.snapshot().epoch());
    assert_eq!(pass.output_epoch(), pass.input_epoch());
    assert_eq!(pass.invalidated_analysis_count(), 0);
    assert_eq!(pass.preserved_analysis_count(), 1);
    assert_eq!(noop.final_graph_identity(), report.final_graph_identity());
    session
        .require_operation_graph_snapshot_v1(&root, after.snapshot())
        .unwrap();
    assert!(!session.is_poisoned());
}

#[test]
fn repeated_passes_chain_epochs_without_claiming_a_missing_cache_was_preserved() {
    let mut session = session();
    let root = dead_constant(&mut session);
    let plan = PlironOptimizationPlanV1::new(
        vec![
            PlironOptimizationPassV1::DeadCodeElimination,
            PlironOptimizationPassV1::DeadCodeElimination,
        ],
        PlironOptimizationLimitsV1::default(),
    )
    .unwrap();
    let report = session.execute_optimization_v1(&root, &plan).unwrap();
    let first = report.passes()[0];
    let second = report.passes()[1];
    assert!(first.changed());
    assert!(!second.changed());
    assert_eq!(first.invalidated_analysis_count(), 1);
    assert_eq!(second.input_epoch(), first.output_epoch());
    assert_eq!(second.output_epoch(), second.input_epoch());
    assert_eq!(second.invalidated_analysis_count(), 0);
    assert_eq!(second.preserved_analysis_count(), 0);
    assert_eq!(report.final_graph_identity().epoch(), second.output_epoch());
    assert_eq!(
        report.final_graph_identity(),
        session
            .analyze_operation_graph_v1(&root)
            .unwrap()
            .replay_identity()
    );
}

#[test]
fn optimization_preflight_rejection_preserves_epoch_and_cached_analysis() {
    let mut session = session();
    let root = dead_constant(&mut session);
    let before = session.analyze_operation_graph_v1(&root).unwrap();
    for (graph_work, work_units) in [(1, 1_000), (16, 1)] {
        let plan = PlironOptimizationPlanV1::new(
            vec![PlironOptimizationPassV1::DeadCodeElimination],
            PlironOptimizationLimitsV1::new(1, graph_work, work_units).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            session.execute_optimization_v1(&root, &plan),
            Err(PlironOptimizationErrorV1::GraphWorkLimitExceeded { .. })
                | Err(PlironOptimizationErrorV1::WorkLimitExceeded { .. })
        ));
        assert_eq!(session.operation_graph_analysis_cache.len(), 1);
        assert_eq!(session.analyze_operation_graph_v1(&root).unwrap(), before);
        assert_eq!(session.operation_children(&root).unwrap().len(), 1);
        assert!(!session.is_poisoned());
    }
}
