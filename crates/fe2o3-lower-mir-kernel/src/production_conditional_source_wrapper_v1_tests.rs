#[test]
fn conditional_translation_distinguishes_the_root_from_its_selected_result_body() {
    use fe2o3_pliron::{
        ProductionConditionalOwnershipSiteV1, ProductionConstructionV1, ProductionPlironSessionV1,
        ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
        ProductionRankedValueIdV1, ProductionSessionLimitsV1,
    };
    let mut source = materialize(Fixture::default());
    let selected = source
        .semantic_ssa()
        .source_semantic()
        .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    assert_ne!(selected.root(), selected.body());
    let root = selected.root();
    let body = selected.body();
    let layout = source.source_launch().roots()[0].layout();
    let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    // No global effects: the pending contract exists only to exercise custody
    // and wrapper association. No output coverage is claimed by this fixture.
    let recipe = ProductionRankedKernelV1::new(
        "selected_wrapper_0",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: layout.grid_identity(),
                    global_extents: layout.global_extents(),
                    workgroup_extents: layout.workgroup_extents(),
                    subgroup_size: layout.subgroup_size(),
                    full_physical_workgroups: layout.full_physical_workgroups(),
                },
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::OwnershipContract {
                    view,
                    coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
                    partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    let registered = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("selected_body_pending", recipe).unwrap(),
        )
        .unwrap();
    let (stage, ranked_root) = session.construct_registered(registered).unwrap();
    let pending = session
        .prepare_conditional_ranked_analysis_v1(
            stage,
            ranked_root,
            &[ProductionConditionalOwnershipSiteV1 {
                block: 0,
                operation: 2,
                view,
            }],
        )
        .unwrap();
    let run = |source: &ProductionPreRankedKirOwnerV1, candidate_root: u32| {
        let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
            candidate_root,
            1,
            pending.kernel().unwrap(),
            &[],
            &[],
            "",
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let floor = source.retained_analysis_storage_v1() + FLOOR;
        budget.reserve_storage(floor).unwrap();
        let result = source
            .check_conditional_source_translation_v1(&pending, candidate, &mut budget)
            .map(|result| result.memory_effects());
        assert_eq!(budget.storage(), floor);
        result
    };
    assert_eq!(run(&source, root.index()).unwrap(), 0);
    assert!(matches!(
        run(&source, body.index()),
        Err(ProductionConditionalSourceTranslationErrorV1::SourceAssociation)
    ));
    let mut rows = source.correspondence.lowered_functions.to_vec();
    let entry = rows
        .iter_mut()
        .find(|row| row.role == SemanticKirFunctionRoleV1::KernelEntry)
        .unwrap();
    entry.semantic_function = root;
    source.correspondence.lowered_functions = rows.into_boxed_slice();
    assert!(matches!(
        run(&source, root.index()),
        Err(ProductionConditionalSourceTranslationErrorV1::SourceAssociation)
    ));
}
