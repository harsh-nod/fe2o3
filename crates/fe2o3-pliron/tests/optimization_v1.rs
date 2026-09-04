use fe2o3_pliron::{
    HARD_MAX_PLIRON_OPTIMIZATION_GRAPH_WORK_V1, HARD_MAX_PLIRON_OPTIMIZATION_PASSES_V1,
    HARD_MAX_PLIRON_OPTIMIZATION_WORK_UNITS_V1, OperationHandleError, PlironOptimizationErrorV1,
    PlironOptimizationLimitErrorV1, PlironOptimizationLimitsV1, PlironOptimizationPassV1,
    PlironOptimizationPlanErrorV1, PlironOptimizationPlanV1, PlironOptimizationResourceV1,
    PlironSession, ShellLimits,
};

fn session() -> PlironSession {
    PlironSession::new(ShellLimits::default(), []).expect("fresh session")
}

#[test]
fn plans_are_closed_bounded_and_deterministic() {
    assert_eq!(
        PlironOptimizationPlanV1::standard().passes(),
        &[
            PlironOptimizationPassV1::SparseConditionalConstantPropagation,
            PlironOptimizationPassV1::SimplifyControlFlow,
            PlironOptimizationPassV1::SelectSameValueCanonicalization,
            PlironOptimizationPassV1::DeadCodeElimination,
            PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination,
            PlironOptimizationPassV1::DeadCodeElimination,
            PlironOptimizationPassV1::SimplifyControlFlow,
        ]
    );

    assert_eq!(
        PlironOptimizationLimitsV1::new(HARD_MAX_PLIRON_OPTIMIZATION_PASSES_V1 + 1, 1, 1,),
        Err(PlironOptimizationLimitErrorV1::AboveHardCap(
            PlironOptimizationResourceV1::Passes
        ))
    );
    assert_eq!(
        PlironOptimizationLimitsV1::new(1, HARD_MAX_PLIRON_OPTIMIZATION_GRAPH_WORK_V1 + 1, 1,),
        Err(PlironOptimizationLimitErrorV1::AboveHardCap(
            PlironOptimizationResourceV1::GraphWork
        ))
    );
    assert_eq!(
        PlironOptimizationLimitsV1::new(1, 1, HARD_MAX_PLIRON_OPTIMIZATION_WORK_UNITS_V1 + 1,),
        Err(PlironOptimizationLimitErrorV1::AboveHardCap(
            PlironOptimizationResourceV1::WorkUnits
        ))
    );

    let limits = PlironOptimizationLimitsV1::new(1, 64, 1_000).unwrap();
    assert_eq!(
        PlironOptimizationPlanV1::new(
            vec![
                PlironOptimizationPassV1::DeadCodeElimination,
                PlironOptimizationPassV1::SimplifyControlFlow,
            ],
            limits,
        ),
        Err(PlironOptimizationPlanErrorV1::TooManyPasses {
            required: 2,
            limit: 1,
        })
    );
}

#[test]
fn dce_invalidates_registered_descendant_handles() {
    let mut session = session();
    let root = session
        .import_operation_text_v1(
            r#"builtin.module @m {
                ^entry():
                dead0 = builtin.constant <builtin.integer <1: i64>> : builtin.integer i64;
                dead1 = builtin.constant <builtin.integer <2: i64>> : builtin.integer i64
            }"#,
        )
        .expect("verified module");
    let dead = session
        .operation_children(&root)
        .expect("registered children");

    let report = session
        .execute_optimization_v1(&root, &PlironOptimizationPlanV1::dead_code_elimination())
        .expect("DCE succeeds");

    assert_eq!(report.passes().len(), 1);
    assert_eq!(
        report.passes()[0].pass(),
        PlironOptimizationPassV1::DeadCodeElimination
    );
    assert!(report.passes()[0].changed());
    assert!(report.final_graph_work() < report.initial_graph_work());
    assert_eq!(report.invalidated_handle_count(), 2);
    assert!(session.operation_children(&root).unwrap().is_empty());
    for handle in dead {
        assert_eq!(
            session.operation_shape(&handle),
            Err(OperationHandleError::StaleHandle)
        );
    }
    assert!(!session.is_poisoned());
}

#[test]
fn standard_plan_executes_all_closed_passes() {
    let mut session = session();
    let root = session.create_module("m").expect("module");
    let report = session
        .execute_optimization_v1(&root, &PlironOptimizationPlanV1::standard())
        .expect("standard plan succeeds");

    assert_eq!(report.passes().len(), 7);
    assert_eq!(report.initial_graph_work(), report.final_graph_work());
    assert!(!session.is_poisoned());
}

#[test]
fn foreign_and_stale_roots_are_rejected_without_running_a_pass() {
    let mut owner = session();
    let mut foreign = session();
    let root = owner.create_module("owner").expect("module");
    assert_eq!(
        foreign.execute_optimization_v1(&root, &PlironOptimizationPlanV1::dead_code_elimination()),
        Err(PlironOptimizationErrorV1::Operation(
            OperationHandleError::ForeignSession
        ))
    );
    assert!(!foreign.is_poisoned());

    let stale = root.clone();
    owner.erase_operation(&root).expect("erase module");
    assert_eq!(
        owner.execute_optimization_v1(&stale, &PlironOptimizationPlanV1::dead_code_elimination()),
        Err(PlironOptimizationErrorV1::Operation(
            OperationHandleError::StaleHandle
        ))
    );
    assert!(!owner.is_poisoned());
}

#[test]
fn graph_and_work_limits_fail_before_mutation() {
    let mut session = session();
    let root = session.create_module("m").expect("module");
    let graph_limited = PlironOptimizationPlanV1::new(
        vec![PlironOptimizationPassV1::DeadCodeElimination],
        PlironOptimizationLimitsV1::new(1, 1, 1_000).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        session.execute_optimization_v1(&root, &graph_limited),
        Err(PlironOptimizationErrorV1::GraphWorkLimitExceeded { .. })
    ));
    assert!(!session.is_poisoned());

    let work_limited = PlironOptimizationPlanV1::new(
        vec![PlironOptimizationPassV1::DeadCodeElimination],
        PlironOptimizationLimitsV1::new(1, 16, 1).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        session.execute_optimization_v1(&root, &work_limited),
        Err(PlironOptimizationErrorV1::WorkLimitExceeded { .. })
    ));
    assert!(!session.is_poisoned());
    assert_eq!(session.operation_children(&root).unwrap().len(), 0);
}
