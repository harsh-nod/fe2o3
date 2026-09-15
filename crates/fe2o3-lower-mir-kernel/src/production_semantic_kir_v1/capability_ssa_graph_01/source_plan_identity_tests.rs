use super::super::global_bf16_live_v1::GlobalBf16SourceQueryV1;

// Constructor-only context, not authenticated Matrix capture or launch evidence.
fn source_plan_context(root: SemanticFunctionIdV1) -> RootKernelContextLoweringV1 {
    RootKernelContextLoweringV1 {
        entry_transfer: None,
        selected_root: root,
        semantic_type: SemanticTypeIdV1::from_index(0),
        context_type: KernelContextTypeV1::new("source_plan", [1; 32], [2; 32], [3; 32]),
        source: KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]),
    }
}

fn source_plan_setup(body: &SemanticFunctionDeclV1) -> usize {
    query_reuse::header_words() + body.blocks().len() + body.locals().len()
}

#[test]
fn source_plan_identity_and_equal_copies_have_independent_exact_work_boundaries() {
    let owner = indexed_owner(indexed_blocks(1));
    let foreign = indexed_owner(indexed_blocks(1));
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let retained = owner.execution_plan_for_root(root).unwrap().plan();
    let copy = retained.clone();
    let foreign_plan = foreign.execution_plan_for_root(root).unwrap().plan();
    assert_eq!(retained, &copy);
    assert_eq!(retained, foreign_plan);
    let context = source_plan_context(root);
    let storage = retained.resources().storage_words();
    assert!(storage > 1);
    for (plan, same) in [(retained, true), (&copy, false), (foreign_plan, false)] {
        assert_eq!(std::ptr::eq(plan, retained), same);
        let cost = if same { 1 } else { storage + 1 };
        for available in 0..=storage + 2 {
            let limit = source_plan_setup(view.body()) + available;
            let mut graph = CapabilitySsaGraphV1::new(view.body(), plan, limit).unwrap();
            let result =
                GlobalBf16SourceQueryV1::new(&owner, root, &context, &mut graph).map(|_| ());
            if available >= cost {
                assert!(result.is_ok());
                assert_eq!(graph.remaining, available - cost);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual, limit: cap,
                }) if actual == limit + 1 && cap == limit)
                );
                assert_eq!(graph.remaining, available.saturating_sub(1));
            }
            assert!(
                graph.reuse.uses.is_empty()
                    && graph.reuse.definitions.is_empty()
                    && graph.reuse.reachability.is_empty()
                    && graph.reuse.loans.is_empty()
                    && graph.reuse.loan_regions.is_empty()
                    && graph.reuse.owner_invalidations.is_empty()
            );
        }
        let mut graph =
            CapabilitySsaGraphV1::new(view.body(), plan, source_plan_setup(view.body()) + 2 * cost)
                .unwrap();
        for left in [cost, 0] {
            GlobalBf16SourceQueryV1::new(&owner, root, &context, &mut graph).unwrap();
            assert_eq!(graph.remaining, left);
        }
        assert!(GlobalBf16SourceQueryV1::new(&owner, root, &context, &mut graph).is_err());
    }
}

#[test]
fn source_plan_identity_preserves_plan_body_root_and_missing_root_rejections() {
    let owner = indexed_owner(indexed_blocks(1));
    let foreign = indexed_owner(indexed_blocks(1));
    let unequal = indexed_owner(indexed_blocks(2));
    let root = SemanticFunctionIdV1::from_index(0);
    let view = owner.execution_view_for_root(root).unwrap();
    let retained = owner.execution_plan_for_root(root).unwrap().plan();
    let copy = retained.clone();
    let unequal_plan = unequal.execution_plan_for_root(root).unwrap().plan();
    assert_ne!(retained, unequal_plan);
    let cloned_body = view.body().clone();
    let foreign_body = foreign.execution_view_for_root(root).unwrap().body();
    let context = source_plan_context(root);
    let wrong_context = source_plan_context(SemanticFunctionIdV1::from_index(1));
    let storage = retained.resources().storage_words();
    for plan in [retained, &copy, unequal_plan] {
        let cost = if std::ptr::eq(plan, retained) {
            1
        } else {
            storage + 1
        };
        for (body, context) in [
            (&cloned_body, &context),
            (foreign_body, &context),
            (view.body(), &wrong_context),
        ] {
            let mut graph =
                CapabilitySsaGraphV1::new(body, plan, source_plan_setup(body) + cost).unwrap();
            assert!(matches!(
                GlobalBf16SourceQueryV1::new(&owner, root, context, &mut graph),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(graph.remaining, 0);
        }
        if !std::ptr::eq(plan, retained) {
            let mut graph = CapabilitySsaGraphV1::new(
                view.body(),
                plan,
                source_plan_setup(view.body()) + storage,
            )
            .unwrap();
            assert!(matches!(
                GlobalBf16SourceQueryV1::new(&owner, root, &wrong_context, &mut graph),
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
            assert_eq!(graph.remaining, storage - 1);
        }
    }
    let mut graph = CapabilitySsaGraphV1::new(
        view.body(),
        unequal_plan,
        source_plan_setup(view.body()) + storage + 1,
    )
    .unwrap();
    assert!(matches!(
        GlobalBf16SourceQueryV1::new(&owner, root, &context, &mut graph),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(graph.remaining, 0);
    let mut graph =
        CapabilitySsaGraphV1::new(view.body(), retained, source_plan_setup(view.body())).unwrap();
    assert!(matches!(
        GlobalBf16SourceQueryV1::new(
            &owner,
            SemanticFunctionIdV1::from_index(99),
            &context,
            &mut graph
        ),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(graph.remaining, 0);
}

#[test]
fn source_plan_identity_survives_replay_for_expanded_and_unexpanded_roots() {
    use super::super::resource_tests;
    for semantic in [
        resource_tests::noop_semantic_owner(&["first", "second"]),
        resource_tests::helper_closure_semantic_owner_with_calls(2),
    ] {
        let owner = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let plan = owner.execution_plan_for_root(root).unwrap().plan();
        owner.verify_replay().unwrap();
        assert!(std::ptr::eq(
            plan,
            owner.execution_plan_for_root(root).unwrap().plan()
        ));
        let context = source_plan_context(root);
        let mut graph =
            CapabilitySsaGraphV1::new(view.body(), plan, source_plan_setup(view.body()) + 1)
                .unwrap();
        GlobalBf16SourceQueryV1::new(&owner, root, &context, &mut graph).unwrap();
        assert_eq!(graph.remaining, 0);
        if let Some(other) = owner.execution_view_for_root(SemanticFunctionIdV1::from_index(1)) {
            let other_root = other.root();
            let other_plan = owner.execution_plan_for_root(other_root).unwrap().plan();
            let other_context = source_plan_context(other_root);
            let mut graph = CapabilitySsaGraphV1::new(
                view.body(),
                other_plan,
                source_plan_setup(view.body()) + 1,
            )
            .unwrap();
            assert!(matches!(
                GlobalBf16SourceQueryV1::new(&owner, other_root, &other_context, &mut graph),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            assert_eq!(graph.remaining, 0);
        }
    }
}
