use super::*;
use global_enum_transport_v1::tests::build_owner;

#[test]
fn private_flow_failure_observation_preserves_successful_owner_read_roster() {
    let owner = build_owner();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let reads = analysis.run().unwrap();
    assert!(!reads.is_empty());
    assert!(analysis.observation.completed);
    assert_eq!(analysis.observation.failure, None);
    assert_eq!(analysis.observation.phase, Phase::Complete);
    let published = PrivateScalarReads::for_root(&owner, root).unwrap();
    assert_eq!(published.reads, reads);
    for statement in view
        .body()
        .blocks()
        .iter()
        .flat_map(|block| block.statements())
    {
        assert_eq!(
            published.contains(view.body(), statement),
            reads.contains_key(&(statement as *const _ as usize))
        );
    }
}

#[test]
fn private_flow_failure_observation_distinguishes_conditions_from_work() {
    let owner = build_owner();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut missing = Analysis::new(owner.source_semantic().types(), view);
    missing.conditions = None;
    assert!(missing.run().is_err());
    assert_eq!(
        missing.observation.failure,
        Some(Failure::ConditionsUnavailable)
    );
    assert_eq!(missing.observation.phase, Phase::Conditions);
    assert_eq!(missing.observation.admitted_reads, 0);
    assert!(!missing.observation.work_exhausted);
    let mut work = Analysis::new(owner.source_semantic().types(), view);
    work.budget = Budget::new(0);
    assert!(work.run().is_err());
    assert!(matches!(
        work.observation.failure,
        Some(Failure::Resource(flow_failure::ResourceFailure::Work {
            remaining: 0,
            ..
        }))
    ));
    assert_eq!(work.observation.admitted_reads, 0);
    assert!(work.observation.work_exhausted);
}

#[test]
fn private_flow_failure_observation_storage_preflight_does_not_publish_reads() {
    let owner = build_owner();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let retained = analysis.budget.reserve(MAX_STORAGE).unwrap();
    assert!(analysis.run().is_err());
    assert!(!analysis.observation.completed);
    assert!(!analysis.observation.work_exhausted);
    assert_eq!(analysis.observation.admitted_reads, 0);
    assert_eq!(analysis.observation.phase, Phase::CfgPreflight);
    let expected = Some(Failure::Resource(flow_failure::ResourceFailure::Storage {
        operation: flow_failure::StorageOperation::Check,
        retained: MAX_STORAGE,
        replaced: 0,
        requested: Some(view.body().blocks().len() * 5),
        attempted: Some(MAX_STORAGE + view.body().blocks().len() * 5),
        limit: MAX_STORAGE,
    }));
    assert_eq!(analysis.observation.failure, expected);
    drop(retained);
    assert_eq!(analysis.observation.failure, expected);
}

#[test]
fn private_flow_failure_observation_records_retained_call_return_edge_not_source_guess() {
    let owner = global_enum_transport_v1::tests::scalar_enum_result_tests::scalar_result_owner();
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    analysis.observation.block(2);
    let edges = analysis.terminator(2, &mut Flow::default()).unwrap();
    assert_eq!(edges, BTreeSet::from([(3, Some(7))]));
    assert_eq!(analysis.observation.phase, Phase::TerminatorEdges);
    assert_eq!(analysis.observation.terminator, Some("Call"));
    let mut expected = flow_observation::Observation::default();
    expected.edge(3, SemanticEdgeRoleV1::CallReturn, Some(7));
    assert_eq!(analysis.observation.edges, expected.edges);
    assert!(!analysis.observation.edges_truncated);
}
