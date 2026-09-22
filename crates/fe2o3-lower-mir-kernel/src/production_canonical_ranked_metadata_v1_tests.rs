use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as CrBudget,
    CanonicalKernelIrWorkBudgetV1 as CrWork,
};

const CR_WORK: usize = 1 << 40;
const CR_STORAGE: usize = 128 * 1024 * 1024;

fn cr_materialize_arguments(
    expanded: bool,
    shape: ArgumentTupleShape,
) -> ProductionPreRankedKirOwnerV1 {
    let ssa = argument_owner_shape(expanded, shape, true, true);
    cr_materialize_ssa(ssa)
}
fn cr_materialize_ssa(ssa: ProductionSemanticSsaOwnerV1) -> ProductionPreRankedKirOwnerV1 {
    let launch = argument_launch_roster(&ssa);
    let mut work = CrWork::new(CR_WORK);
    let mut budget = CrBudget::new(&mut work, CR_STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}
fn cr_test_budget<'w>(owner: &ProductionPreRankedKirOwnerV1, work: &'w mut CrWork) -> CrBudget<'w> {
    let mut budget = CrBudget::new(work, CR_STORAGE);
    budget
        .reserve_storage(owner.unit_local_source_storage_floor_v1().unwrap() + 19)
        .unwrap();
    budget
}
fn cr_inspect_complete(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut CrBudget<'_>,
) -> CrResultV1<()> {
    assert!(!source.grants_artifact_or_launch_authority());
    assert!(!source.ranked_verification_is_complete());
    assert!(source.external_refinement_is_pending());
    assert!(
        source
            .inventory(budget)?
            .belongs_to(source.owner.executable())
    );
    assert_eq!(source.function_count(budget)?, 4);
    assert_eq!(source.inventory(budget)?.functions().len(), 3);
    assert_eq!(source.launches(budget)?.len(), 2);
    let mut aliases = Vec::new();
    for i in 0..4 {
        let function = source.function(i, budget)?;
        if function.source().role == SemanticKirFunctionRoleV1::InternalHelper {
            aliases.push((
                function.source().correspondence_owner,
                function.canonical().coordinate,
            ));
        }
    }
    assert_eq!(aliases.len(), 2);
    assert_ne!(aliases[0].0, aliases[1].0);
    assert_eq!(aliases[0].1, aliases[1].1);
    let inventory = source.inventory(budget)?;
    let operations = inventory.operations().len();
    for operation in 0..operations {
        let expected = source
            .calls
            .groups
            .iter()
            .filter(|group| {
                group.function.canonical.coordinate
                    == source.inventory.operations()[operation]
                        .coordinate
                        .block
                        .function
            })
            .count();
        assert_eq!(source.source.operation_origins[operation].len(), expected);
        for alias in 0..expected {
            let span = source.operation_source(operation, alias, budget)?;
            assert!(span.operations().contains(&operation));
        }
    }
    let actual = source.spans(budget)?;
    assert_eq!(
        actual.len(),
        source.owner.correspondence.statement_operation_spans.len()
            + source.owner.correspondence.terminator_operation_spans.len()
            + source.owner.correspondence.synthetic_operation_spans.len()
    );
    for row in actual {
        if let ProductionCanonicalRankedSourceSiteV1::Statement {
            span,
            source: statement,
        } = row.site()
        {
            let expected = &source.owner.semantic_ssa.source_semantic().functions()
                [span.semantic_function.index() as usize]
                .blocks()[span.semantic_block.index() as usize]
                .statements()[span.statement_ordinal as usize];
            assert!(std::ptr::eq(statement, expected));
        }
    }
    Ok(())
}
#[test]
fn canonical_ranked_source_keeps_complete_packed_expanded_zero_and_shared_helper_metadata() {
    for expanded in [false, true] {
        for shape in [
            ArgumentTupleShape::Mixed,
            ArgumentTupleShape::AllZero,
            ArgumentTupleShape::EmptyUnit,
            ArgumentTupleShape::EmptyTuple,
        ] {
            let owner = cr_materialize_arguments(expanded, shape);
            let original = owner.executable().canonical().canonical_bytes().to_vec();
            let mut work = CrWork::new(CR_WORK);
            let mut budget = cr_test_budget(&owner, &mut work);
            let floor = budget.storage();
            owner
                .with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
                    cr_inspect_complete(source, budget)?;
                    let arguments = source.arguments(budget)?;
                    assert!(!arguments.is_empty());
                    assert!(
                        arguments.iter().any(|row| row.coverage()
                            == ProductionCanonicalRankedArgumentCoverageV1::Zero)
                    );
                    let helper = source
                        .calls
                        .groups
                        .iter()
                        .position(|g| {
                            g.function.source.role == SemanticKirFunctionRoleV1::InternalHelper
                        })
                        .unwrap();
                    let range = source.arguments.associations[helper].clone();
                    let nodes = &source.arguments.rows[range];
                    assert!(
                        nodes.iter().any(|row| row.adjusted_argument().is_none()),
                        "RustCall source envelope survives"
                    );
                    for (ordinal, row) in source.arguments.rows.iter().enumerate() {
                        let path = source.argument_path(ordinal, false, budget)?;
                        assert_eq!(path, &source.arguments.paths[row.path.clone()]);
                        if row.local().is_some() {
                            assert_eq!(
                                source.argument_path(ordinal, true, budget)?,
                                &path[row.local_offset..]
                            );
                        }
                    }
                    Ok(())
                })
                .unwrap();
            owner
                .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
                    assert!(!view.ranked_verification_is_complete());
                    assert!(!view.grants_artifact_or_launch_authority());
                    let count = view.row_count(budget)?;
                    assert!(count > owner.executable().module().functions.len());
                    let mut metadata_seen = false;
                    for ordinal in 0..count {
                        let row = view.row(ordinal, budget)?;
                        if matches!(row.subject, CrSubjectV1::Metadata(_)) {
                            metadata_seen = true;
                            assert!(row.obligations.contains(
                                fe2o3_kernel_analysis::CanonicalRankedObligationV1::SourceMetadata
                            ));
                        }
                    }
                    assert!(metadata_seen);
                    cr_inspect_complete(view.metadata(budget)?, budget)
                })
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(owner.executable().canonical().canonical_bytes(), original);
        }
    }
}
#[test]
fn canonical_ranked_source_span_and_operation_completeness_are_independent() {
    let owner = cr_materialize_arguments(true, ArgumentTupleShape::Mixed);
    let donor = cr_materialize_arguments(true, ArgumentTupleShape::Mixed);
    let mut work = CrWork::new(CR_WORK);
    let mut budget = cr_test_budget(&owner, &mut work);
    owner
        .with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
            for hostile in 0..6 {
                cr_protected_v1(budget, |budget| {
                    budget.reserve_storage(std::mem::size_of::<CrSourceRowsV1<'_>>())?;
                    let mut rows =
                        cr_build_source_rows_v1(&owner, source.inventory, source.calls, budget)?;
                    match hostile {
                        0 => {
                            rows.spans.pop();
                        }
                        1 => {
                            let index = rows
                                .spans
                                .iter()
                                .position(|r| {
                                    matches!(
                                        r.site,
                                        ProductionCanonicalRankedSourceSiteV1::Statement { .. }
                                    )
                                })
                                .unwrap();
                            let ProductionCanonicalRankedSourceSiteV1::Statement {
                                source: statement,
                                ..
                            } = rows.spans[index].site
                            else {
                                unreachable!()
                            };
                            rows.spans[index].site =
                                ProductionCanonicalRankedSourceSiteV1::Statement {
                                    span: &donor.correspondence.statement_operation_spans[index],
                                    source: statement,
                                };
                        }
                        2 => {
                            rows.origins[0].span = rows.spans.len();
                        }
                        3 => {
                            rows.operation_origins[0].end = rows.operation_origins[0].start;
                        }
                        4 => {
                            rows.associations.swap(0, 1);
                        }
                        5 => {
                            rows.origins[0].association = source.calls.groups.len();
                        }
                        _ => unreachable!(),
                    }
                    assert!(
                        cr_check_source_rows_v1(
                            &owner,
                            source.inventory,
                            source.calls,
                            &rows,
                            budget
                        )
                        .is_err()
                    );
                    Ok(())
                })?;
            }
            Ok(())
        })
        .unwrap();
}
#[test]
fn canonical_ranked_source_rejects_same_count_argument_path_type_coverage_and_owner_drift() {
    let owner = cr_materialize_arguments(true, ArgumentTupleShape::Mixed);
    let mut work = CrWork::new(CR_WORK);
    let mut budget = cr_test_budget(&owner, &mut work);
    owner.with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
        for hostile in 0..6 {
            cr_protected_v1(budget, |budget| {
                budget.reserve_storage(std::mem::size_of::<CrArgumentRowsV1>())?;
                let mut rows = cr_build_arguments_v1(&owner, source.calls, budget)?;
                match hostile {
                    0 => { rows.rows[0].association = source.calls.groups.len(); }
                    1 => { rows.rows[0].ty = SemanticTypeIdV1::from_index(u32::MAX); }
                    2 => { rows.paths[0] = ProductionArgumentProjectionV1::Field(u32::MAX); }
                    3 => { rows.rows[0].path.end = rows.paths.len() + 1; }
                    4 => { rows.rows[0].coverage = ProductionCanonicalRankedArgumentCoverageV1::WithinAtomicParameter { slot: usize::MAX, value: ValueId(u32::MAX) }; }
                    5 => { rows.associations[0].end += 1; }
                    _ => unreachable!(),
                }
                assert!(cr_check_arguments_v1(&owner, source.calls, &rows, budget).is_err());
                Ok(())
            })?;
        }
        Ok(())
    }).unwrap();
}
#[test]
fn canonical_ranked_source_projection_checks_layout_root_order_and_all_aliases() {
    let owner = cr_materialize_arguments(false, ArgumentTupleShape::AllZero);
    let mut work = CrWork::new(CR_WORK);
    let mut budget = cr_test_budget(&owner, &mut work);
    owner
        .with_canonical_ranked_metadata_v1(&mut budget, |source, budget| {
            for hostile in 0..5 {
                cr_protected_v1(budget, |budget| {
                    let mut rows = cr_build_projection_v1(source, budget)?;
                    match hostile {
                        0 => rows.facts[6] = CrFactV1::Unsigned(1),
                        1 => rows.keys.swap(1, 2),
                        2 => rows.facts[9] = CrFactV1::Unsigned(u64::MAX),
                        3 => {
                            rows.facts.pop();
                        }
                        4 => {
                            rows.keys.pop();
                        }
                        _ => unreachable!(),
                    }
                    assert!(cr_check_projection_v1(source, &rows, budget).is_err());
                    Ok(())
                })?;
            }
            Ok(())
        })
        .unwrap();
}

#[test]
fn canonical_ranked_source_keeps_scalar_projected_retained_and_zero_call_results() {
    for mode in [
        ArgumentCallResult::Zero,
        ArgumentCallResult::Scalar,
        ArgumentCallResult::Retained,
        ArgumentCallResult::Projected,
    ] {
        let owner = cr_materialize_ssa(argument_call_owner(
            true,
            ArgumentTupleShape::Mixed,
            true,
            true,
            true,
            mode,
            true,
        ));
        let mut work = CrWork::new(CR_WORK);
        let mut budget = cr_test_budget(&owner, &mut work);
        let floor = budget.storage();
        owner
            .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
                let source = view.metadata(budget)?;
                let count = source.call_transport_count(budget)?;
                assert_eq!(count, owner.correspondence.call_returns.len());
                assert!(count > 2);
                let mut calls = 0;
                let mut results = 0;
                for ordinal in 0..count {
                    let row = source.call_transport(ordinal, budget)?;
                    assert!(std::ptr::eq(
                        row.row,
                        &owner.correspondence.call_returns[ordinal]
                    ));
                    if row.call_operation_span().is_some() {
                        calls += 1;
                        assert!(row.destination().is_some());
                    }
                    for component in 0..row.component_count() {
                        assert!(row.component(component).is_some());
                        results += 1;
                    }
                    assert!(row.component(row.component_count()).is_none());
                }
                assert!(calls >= 4);
                if mode == ArgumentCallResult::Zero {
                    assert_eq!(results, 0);
                } else {
                    assert!(results > 0);
                }
                if matches!(
                    mode,
                    ArgumentCallResult::Retained | ArgumentCallResult::Projected
                ) {
                    assert!(!source.effects(budget)?.is_empty());
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn canonical_ranked_source_keeps_atomic_carrier_children_without_pointee_expansion() {
    use ProductionCanonicalRankedArgumentCoverageV1::{Parameter, WithinAtomicParameter, Zero};
    for length in [2, 257] {
        let owner = super::complete_view_tests::atomic_view_tests::atomic_argument_owner(length);
        let mut work = CrWork::new(CR_WORK);
        let mut budget = cr_test_budget(&owner, &mut work);
        let floor = budget.storage();
        owner
            .with_checked_canonical_ranked_source_v1(&mut budget, |view, budget| {
                let source = view.metadata(budget)?;
                assert_eq!(source.function_count(budget)?, 1);
                let arguments = source.arguments(budget)?;
                let carrier = arguments
                    .iter()
                    .find(|row| {
                        row.source_argument() == 1 && matches!(row.coverage(), Parameter { .. })
                    })
                    .unwrap();
                let Parameter { slot, value } = carrier.coverage() else {
                    unreachable!()
                };
                assert_eq!(slot, 1);
                assert_eq!(carrier.local(), Some(SemanticLocalIdV1::from_index(2)));
                let mut inside = 0;
                let mut zeros = 0;
                let mut all = 0;
                for (ordinal, row) in arguments.iter().enumerate() {
                    if row.source_argument() != 1 {
                        continue;
                    }
                    all += 1;
                    assert_eq!(row.adjusted_argument(), Some(1));
                    match row.coverage() {
                        WithinAtomicParameter {
                            slot: actual_slot,
                            value: actual_value,
                        } => {
                            inside += 1;
                            assert_eq!((actual_slot, actual_value), (slot, value));
                            assert_eq!(
                                source.argument_path(ordinal, false, budget)?,
                                &[ProductionArgumentProjectionV1::Field(1)]
                            );
                        }
                        Zero => zeros += 1,
                        Parameter { .. } => {
                            assert!(source.argument_path(ordinal, false, budget)?.is_empty())
                        }
                        _ => panic!("atomic carrier must not expand into ABI components"),
                    }
                }
                assert_eq!(inside, 1, "one pointer marker, no pointee walk");
                assert_eq!(zeros, length as usize + 7);
                assert_eq!(all, length as usize + 9);
                assert!(!view.ranked_verification_is_complete());
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
