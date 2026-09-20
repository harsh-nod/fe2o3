use super::*;
use dialect_kernel::AccessKindAttr;

fn session() -> ProductionPlironSessionV1 {
    ProductionPlironSessionV1::new(ProductionSessionLimitsV1::default(), [])
        .expect("fresh production session")
}

fn ranked_session() -> ProductionPlironSessionV1 {
    ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().expect("gpu registration"),
            dialect_kernel::dialect_registration().expect("kernel registration"),
        ],
    )
    .expect("fresh ranked production session")
}

fn ranked_construction(name: &str) -> ProductionConstructionV1 {
    let view = ProductionRankedValueIdV1::new(0);
    let index = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "checked",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                ProductionRankedOperationV1::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: index,
                    value: 0,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(index)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .expect("ranked recipe");
    ProductionConstructionV1::ranked_kernel(name, kernel).expect("ranked construction")
}

fn construct_ranked(
    session: &mut ProductionPlironSessionV1,
    name: &str,
) -> (
    ProductionStageHandleV1<ConstructedGraphStageV1>,
    ProductionRootHandleV1<ConstructedGraphStageV1>,
) {
    let registered = session
        .register_construction(ranked_construction(name))
        .expect("ranked registration");
    session
        .construct_registered(registered)
        .expect("ranked construction")
}

fn expect_injected_analysis_panic(
    session: &mut ProductionPlironSessionV1,
    stage: ProductionStageHandleV1<ConstructedGraphStageV1>,
    root: ProductionRootHandleV1<ConstructedGraphStageV1>,
) {
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::UpstreamPanicked
        ))
    ));
    assert!(session.is_poisoned());
}

#[test]
fn stale_private_operation_poisons_the_production_session() {
    let mut session = session();
    let registered = session
        .register_construction(
            ProductionConstructionV1::builtin_module("root").expect("valid recipe"),
        )
        .expect("registered recipe");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("constructed root");

    session
        .inner
        .erase_operation(&root.operation)
        .expect("simulate private registry corruption");
    assert_eq!(
        session.root_shape(&stage, &root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::StaleHandle
        ))
    );
    assert!(session.is_poisoned());
    assert!(matches!(
        session.register_construction(
            ProductionConstructionV1::builtin_module("later").expect("valid recipe")
        ),
        Err(ProductionSessionErrorV1::SessionPoisoned)
    ));
}

#[test]
fn equal_slot_foreign_analysis_handles_fail_before_raw_traversal() {
    let mut owner = ranked_session();
    let mut foreign = ranked_session();
    let (owner_stage, owner_root) = construct_ranked(&mut owner, "owner");
    let (foreign_stage, foreign_root) = construct_ranked(&mut foreign, "foreign");
    let owner_function = owner.constructed_roots[&owner_stage.identity].ranked_function;
    let foreign_function = foreign.constructed_roots[&foreign_stage.identity].ranked_function;
    assert_eq!(
        format!("{owner_function:?}"),
        format!("{foreign_function:?}"),
        "fresh contexts deliberately allocate the same upstream slot"
    );
    assert_ne!(
        owner_function, foreign_function,
        "owner-authenticated pointers must not compare equal across contexts"
    );

    crate::production_analysis::panic_next_production_analysis_for_test_v1();
    assert!(matches!(
        foreign.verify_production_ranked_kernel_pipeline(owner_stage, owner_root),
        Err(ProductionSessionErrorV1::ForeignSession)
    ));
    assert!(!foreign.is_poisoned());
    expect_injected_analysis_panic(&mut foreign, foreign_stage, foreign_root);
}

#[test]
fn stale_analysis_stage_fails_before_raw_traversal() {
    let mut session = ranked_session();
    let (stage, root) = construct_ranked(&mut session, "first");
    let stale_stage = ProductionStageHandleV1::<ConstructedGraphStageV1> {
        owner: stage.owner,
        identity: stage.identity,
        _stage: PhantomData,
    };
    let stale_root = ProductionRootHandleV1::<ConstructedGraphStageV1> {
        owner: root.owner,
        stage: root.stage,
        identity: root.identity,
        operation: root.operation.clone(),
        graph_snapshot: root.graph_snapshot,
        exact_graph_identity: root.exact_graph_identity,
        _stage: PhantomData,
    };
    let _verified = session
        .verify_production_ranked_kernel_pipeline(stage, root)
        .expect("first verification");
    let (live_stage, live_root) = construct_ranked(&mut session, "second");

    crate::production_analysis::panic_next_production_analysis_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stale_stage, stale_root),
        Err(ProductionSessionErrorV1::StaleStage)
    ));
    assert!(!session.is_poisoned());
    expect_injected_analysis_panic(&mut session, live_stage, live_root);
}

#[test]
fn transplanted_root_operation_fails_before_raw_traversal() {
    let mut session = ranked_session();
    let (first_stage, first_root) = construct_ranked(&mut session, "first");
    let (_second_stage, second_root) = construct_ranked(&mut session, "second");
    let transplanted = ProductionRootHandleV1::<ConstructedGraphStageV1> {
        owner: first_root.owner,
        stage: first_root.stage,
        identity: first_root.identity,
        operation: second_root.operation,
        graph_snapshot: first_root.graph_snapshot,
        exact_graph_identity: first_root.exact_graph_identity,
        _stage: PhantomData,
    };

    crate::production_analysis::panic_next_production_analysis_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(first_stage, transplanted),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationGraphSnapshotMismatch
        ))
    ));
    assert!(session.is_poisoned());

    let mut clean = ranked_session();
    let (stage, root) = construct_ranked(&mut clean, "clean");
    expect_injected_analysis_panic(&mut clean, stage, root);
}

#[test]
fn corrupt_context_marker_fails_before_raw_traversal() {
    let mut session = ranked_session();
    let (stage, root) = construct_ranked(&mut session, "corrupt");
    let key: pliron::identifier::Identifier = fe2o3_pliron_owner_core::CONTEXT_IDENTITY_MARKER_KEY
        .try_into()
        .expect("fixed marker key");
    session.inner.context.aux_data_map.remove(&key);

    crate::production_analysis::panic_next_production_analysis_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::ContextIdentity(
                fe2o3_pliron_owner_core::ContextIdentityError::CorruptMarker
            )
        ))
    ));
    assert!(session.is_poisoned());

    let mut clean = ranked_session();
    let (stage, root) = construct_ranked(&mut clean, "clean");
    expect_injected_analysis_panic(&mut clean, stage, root);
}

#[test]
fn raw_analysis_panic_is_contained_and_terminally_poisons_the_session() {
    let mut session = ranked_session();
    let (stage, root) = construct_ranked(&mut session, "panic");
    crate::production_analysis::panic_next_analysis_manager_prepare_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::UpstreamPanicked
        ))
    ));
    assert!(session.is_poisoned());
    assert!(matches!(
        session.register_construction(ranked_construction("later")),
        Err(ProductionSessionErrorV1::SessionPoisoned)
    ));
}

#[test]
fn transient_analysis_mutation_terminally_poisons_the_session() {
    let mut session = ranked_session();
    let (stage, root) = construct_ranked(&mut session, "transient_mutation");
    crate::production_analysis::transiently_mutate_next_production_analysis_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::RankedPassPreservation(
            crate::PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(crate::KernelCheckPassKindV1::TensorLayout),
                ..
            }
        ))
    ));
    assert!(session.is_poisoned());
    assert!(matches!(
        session.register_construction(ranked_construction("later")),
        Err(ProductionSessionErrorV1::SessionPoisoned)
    ));
}

#[test]
fn structural_analysis_drift_terminally_poisons_the_session() {
    let mut session = ranked_session();
    let (stage, root) = construct_ranked(&mut session, "structural_drift");
    crate::production_analysis::structurally_mutate_next_production_analysis_for_test_v1();
    assert!(matches!(
        session.verify_production_ranked_kernel_pipeline(stage, root),
        Err(ProductionSessionErrorV1::RankedPassPreservation(
            crate::PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass: crate::KernelCheckPassKindV1::TensorLayout,
                ..
            }
        ))
    ));
    assert!(session.is_poisoned());
    assert!(matches!(
        session.register_construction(ranked_construction("later")),
        Err(ProductionSessionErrorV1::SessionPoisoned)
    ));
}

#[test]
fn recipe_cross_check_failure_poisons_after_allocation() {
    let mut session = session();
    let operation = session
        .inner
        .create_module("actual")
        .expect("private typed construction");

    assert_eq!(
        session
            .inner
            .validate_production_module(&operation, "expected"),
        Err(OperationHandleError::ConstructionRecipeMismatch)
    );
    assert!(session.is_poisoned());
    assert!(matches!(
        session.register_construction(
            ProductionConstructionV1::builtin_module("later").expect("valid recipe")
        ),
        Err(ProductionSessionErrorV1::SessionPoisoned)
    ));
}

#[test]
fn exhausted_stage_identity_rejects_without_poisoning() {
    let mut session = session();
    session.next_stage = None;

    assert!(matches!(
        session.register_construction(
            ProductionConstructionV1::builtin_module("root").expect("valid recipe")
        ),
        Err(ProductionSessionErrorV1::StageIdentitySpaceExhausted)
    ));
    assert!(!session.is_poisoned());
    assert!(session.registered.is_empty());
    assert!(session.construction_names.is_empty());
    assert_eq!(session.registration_count, 0);
}

#[test]
fn exhausted_root_identity_rejects_before_pliron_allocation() {
    let mut session = session();
    let registered = session
        .register_construction(
            ProductionConstructionV1::builtin_module("root").expect("valid recipe"),
        )
        .expect("registered recipe");
    session.next_root = None;

    assert!(matches!(
        session.construct_registered(registered),
        Err(ProductionSessionErrorV1::RootIdentitySpaceExhausted)
    ));
    assert!(!session.is_poisoned());
    assert!(session.inner.operations.is_empty());
}

#[test]
fn stale_stage_identity_is_rejected_before_pliron_allocation() {
    let mut session = session();
    let stale = ProductionStageHandleV1 {
        owner: session.inner.identity,
        identity: StageIdentityV1(NonZeroU64::new(99).unwrap()),
        _stage: PhantomData,
    };

    assert!(matches!(
        session.construct_registered(stale),
        Err(ProductionSessionErrorV1::StaleStage)
    ));
    assert!(!session.is_poisoned());
    assert!(session.inner.operations.is_empty());
}

#[test]
fn forged_verified_typestate_cannot_skip_the_safety_transitions() {
    let mut session = ranked_session();
    let registered = session
        .register_construction(ranked_construction("root"))
        .expect("registration");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("construction");
    let forged_stage = ProductionStageHandleV1::<KernelChecksVerifiedGraphStageV1> {
        owner: stage.owner,
        identity: stage.identity,
        _stage: PhantomData,
    };
    let forged_root = ProductionRootHandleV1::<KernelChecksVerifiedGraphStageV1> {
        owner: root.owner,
        stage: root.stage,
        identity: root.identity,
        operation: root.operation,
        graph_snapshot: root.graph_snapshot,
        exact_graph_identity: root.exact_graph_identity,
        _stage: PhantomData,
    };

    assert!(matches!(
        session.prepare_ranked_lowering(forged_stage, forged_root),
        Err(ProductionSessionErrorV1::StageRootMismatch)
    ));
}

#[test]
fn private_graph_erasure_is_rechecked_before_lowering_release() {
    let mut session = ranked_session();
    let registered = session
        .register_construction(ranked_construction("root"))
        .expect("registration");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("construction");
    let (verified, root) = session
        .verify_production_ranked_kernel_pipeline(stage, root)
        .expect("generic kernel verification");
    session
        .inner
        .erase_operation(&root.operation)
        .expect("simulate private graph corruption");

    assert!(matches!(
        session.prepare_ranked_lowering(verified, root),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::StaleHandle
        ))
    ));
}

#[test]
fn ranked_analysis_binding_rejects_a_mismatched_exact_graph_identity() {
    let mut session = ranked_session();
    let registered = session
        .register_construction(ranked_construction("root"))
        .expect("registration");
    let (stage, root) = session
        .construct_registered(registered)
        .expect("construction");
    let (verified, mut root) = session
        .verify_production_ranked_kernel_pipeline(stage, root)
        .expect("generic kernel verification");
    let hostile = ProductionExactGraphIdentityV1([0xa5; 32]);
    session
        .constructed_roots
        .get_mut(&verified.identity)
        .expect("live record")
        .exact_graph_identity = Some(hostile);
    root.exact_graph_identity = Some(hostile);

    assert!(matches!(
        session.prepare_ranked_lowering(verified, root),
        Err(ProductionSessionErrorV1::RankedGraphChanged)
    ));
}

include!("ranked/custody_mismatch_v1_tests.rs");
include!("ranked/cache_release_replay_v1_tests.rs");

#[test]
fn ranked_tree_capacity_rejects_before_allocation_without_poisoning() {
    let mut session = ranked_session();
    let registered = session
        .register_construction(ranked_construction("root"))
        .expect("registration");
    session.inner.operation_tree_work = crate::HARD_MAX_SESSION_OPERATION_TREE_ITEMS - 1;

    assert!(matches!(
        session.construct_registered(registered),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::SessionOperationTreeLimitExceeded
        ))
    ));
    assert!(!session.is_poisoned());
    assert!(session.inner.operations.is_empty());
}
