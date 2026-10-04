fn replacement_allowance_manager(
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> PlironAnalysisManagerV1 {
    let setup = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::StructuralIdentity,
        3,
        7,
        3,
    )
    .unwrap();
    PlironAnalysisManagerV1::new_with_resource_contract(
        function,
        ProductionAnalysisInputCensusV1::default(),
        setup,
        5,
        limits,
    )
    .unwrap()
}

fn replacement_allowance_function(context: &mut Context) -> FuncOp {
    let function_type = FunctionType::get(context, vec![], vec![]);
    FuncOp::new(
        context,
        "replacement_allowance".try_into().unwrap(),
        function_type,
    )
}

#[test]
fn identity_replacement_allowance_is_read_only_exact_and_owner_scoped() {
    let mut context = Context::new();
    let function = replacement_allowance_function(&mut context);
    let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
    let mut manager =
        replacement_allowance_manager(&function, ProductionAnalysisResourceLimitsV1::new(24, 18));
    let before = manager.resource_upper_bound();
    // Setup work 3 plus the empty inventory's one visit; R=7 and peak=10.
    assert_eq!(before.work_upper_bound(), 4);
    assert_eq!(before.retained_storage_upper_bound(), 7);
    assert_eq!(before.peak_storage_upper_bound(), 10);
    for _ in 0..2 {
        assert_eq!(
            manager
                .remaining_identity_replacement_resource_limits_v1(5)
                .unwrap()
                .output,
            ProductionAnalysisResourceLimitsV1::new(20, 16),
        );
        assert_eq!(manager.resource_upper_bound(), before);
        assert_eq!(manager.lineage_identity_retained_storage, 5);
        assert_eq!(
            manager
                .remaining_identity_replacement_resource_limits_v1(5)
                .unwrap()
                .input,
            ProductionAnalysisResourceLimitsV1::new(20, 11),
        );
    }
    for other_phase in [phase, ProductionAnalysisResourcePhaseV1::ReportValidation] {
        assert_eq!(
            manager.remaining_resource_limits(other_phase).unwrap(),
            ProductionAnalysisResourceLimitsV1::new(20, 11),
        );
    }
    for wrong_owner in [4, 6] {
        assert_eq!(
            manager.remaining_identity_replacement_resource_limits_v1(wrong_owner),
            Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "replaced identity retained storage upper bound",
            }),
        );
        assert_eq!(manager.resource_upper_bound(), before);
    }
    // Pstage=16 includes outgoing5, incoming6 and transient5. The other
    // prefix2 remains live: cumulative peak18, retained8, and work24.
    let replacement =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 20, 6, 10).unwrap();
    manager
        .remaining_identity_replacement_resource_limits_v1(5)
        .unwrap()
        .output
        .require(phase, replacement)
        .unwrap();
    manager
        .resource_contract_replace_retained_v1(phase, 5, replacement, 4)
        .unwrap();
    assert_eq!(manager.resource_upper_bound().work_upper_bound(), 24);
    assert_eq!(
        manager
            .resource_upper_bound()
            .retained_storage_upper_bound(),
        8
    );
    assert_eq!(
        manager.resource_upper_bound().peak_storage_upper_bound(),
        18
    );
    assert_eq!(manager.lineage_identity_retained_storage, 4);
    assert!(
        manager
            .remaining_identity_replacement_resource_limits_v1(5)
            .is_err()
    );
    assert_eq!(
        manager
            .remaining_identity_replacement_resource_limits_v1(4)
            .unwrap()
            .output,
        ProductionAnalysisResourceLimitsV1::new(0, 14),
    );
}

#[test]
fn identity_replacement_one_under_denial_does_not_publish_or_release_owner() {
    let mut context = Context::new();
    let function = replacement_allowance_function(&mut context);
    let phase = ProductionAnalysisResourcePhaseV1::PassPreservation;
    let replacement =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 20, 6, 10).unwrap();
    for (work, peak, resource) in [
        (23, 18, "work upper bound"),
        (24, 17, "peak storage upper bound"),
    ] {
        let mut manager = replacement_allowance_manager(
            &function,
            ProductionAnalysisResourceLimitsV1::new(work, peak),
        );
        let before = manager.resource_upper_bound();
        assert_eq!(
            manager
                .remaining_identity_replacement_resource_limits_v1(5)
                .unwrap()
                .output
                .require(phase, replacement),
            Err(ProductionAnalysisResourceLimitV1 { phase, resource }),
        );
        assert_eq!(
            manager.resource_contract_replace_retained_v1(phase, 5, replacement, 4),
            Err(ProductionAnalysisResourceLimitV1 { phase, resource }),
        );
        assert_eq!(manager.resource_upper_bound(), before);
        assert_eq!(manager.lineage_identity_retained_storage, 5);
    }
}
