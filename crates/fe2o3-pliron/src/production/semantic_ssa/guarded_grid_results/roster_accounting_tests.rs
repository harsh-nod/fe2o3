use super::*;

fn semantic() -> AdmittedInertSemanticMirV1 {
    super::super::tests::admitted_helper_semantic()
}

fn work_limit(work: usize) -> ProductionSemanticSsaLimitsV1 {
    let defaults = SsaPlannerLimitsV1::default();
    ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            defaults.max_variables(),
            defaults.max_blocks(),
            defaults.max_edges(),
            defaults.max_events(),
            defaults.max_edge_definitions(),
            defaults.max_output_items(),
            defaults.max_storage_words(),
            work,
        )
        .unwrap(),
    )
}

#[test]
fn guarded_grid_absent_family_retains_exact_roster_work_without_storage_or_events() {
    let semantic = semantic();
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = expansion.root(SemanticFunctionIdV1::from_index(0)).unwrap();
    assert!(view.has_expanded_calls());
    assert_eq!(semantic.functions().len(), 2);
    let result = GuardedGridResultsV1::derive(
        &semantic,
        &expansion,
        view,
        work_limit(semantic.functions().len()),
    )
    .unwrap();
    assert_eq!(result.resources().work_units, semantic.functions().len());
    assert_eq!(result.resources().storage_words, 0);
    assert!(result.entries().is_empty());
    assert!(result.events.is_empty());
    assert!(result.view.is_none());
    result.verify_view(view).unwrap();
    let mut actual = Sha256::new();
    let mut empty = Sha256::new();
    actual.update(b"unchanged-absent-grid-events");
    empty.update(b"unchanged-absent-grid-events");
    result.hash_into(&mut actual);
    GuardedGridResultsV1::default().hash_into(&mut empty);
    assert_eq!(actual.finalize(), empty.finalize());
}

#[test]
fn guarded_grid_absent_family_rejects_one_below_roster_work() {
    let semantic = semantic();
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let view = expansion.root(root).unwrap();
    let count = semantic.functions().len();
    assert!(matches!(
        GuardedGridResultsV1::derive(&semantic, &expansion, view, work_limit(count - 1)),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            function,
            resource: SsaPlannerResourceV1::WorkUnits,
            required,
            limit,
        }) if function == root && required == count && limit == count - 1
    ));
}

#[test]
fn guarded_grid_absent_family_still_requires_the_exact_expansion_owner() {
    let semantic = semantic();
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let other =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let other_view = other.root(root).unwrap();
    assert_eq!(
        expansion.root(root).unwrap().identity(),
        other_view.identity()
    );
    assert!(matches!(
        GuardedGridResultsV1::derive(
            &semantic,
            &expansion,
            other_view,
            work_limit(semantic.functions().len()),
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn guarded_grid_absent_family_work_reaches_the_execution_and_module_ledgers() {
    let semantic = semantic();
    let count = semantic.functions().len();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            semantic,
            crate::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let plan = owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let resources = plan.guarded_grid_results.resources();
    assert_eq!(resources.work_units, count);
    assert_eq!(resources.storage_words, 0);
    assert!(plan.auxiliary_resources.work_units >= count);

    // Removing only this retained auxiliary cost from an otherwise identical
    // plan must remove precisely that cost from the aggregate ledger as well.
    let variables = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body()
        .locals()
        .len();
    let mut without_scan = plan.clone();
    without_scan.auxiliary_resources.work_units -= count;
    let mut actual = ProductionSemanticSsaSummaryV1::default();
    let mut without = ProductionSemanticSsaSummaryV1::default();
    let limits = ProductionSemanticSsaLimitsV1::default();
    accumulate_summary_v1(&mut actual, plan, variables, limits).unwrap();
    accumulate_summary_v1(&mut without, &without_scan, variables, limits).unwrap();
    assert_eq!(actual.work_units(), without.work_units() + count);
    assert_eq!(actual.storage_words(), without.storage_words());
}
