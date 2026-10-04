//! Source-backed conditional tests, not protected proof or launch evidence.

use super::*;

#[path = "production_conditional_source_ranked_fixture_v1_tests.rs"]
mod ranked_fixture;
#[path = "production_conditional_source_output_fixture_v1_tests.rs"]
mod source_fixture;
use ranked_fixture::*;
use source_fixture::output_source;

#[test]
fn source_output_has_checked_conditional_coverage() {
    let owner = output_source(7);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    let result = fe2o3_kernel_ir::derive_conditional_total_view_from_verified_v1(
        owner.executable.verified_module_ref_v1(),
        &owner.executable.module().kernels[0].id,
        &mut budget,
    )
    .unwrap();
    let fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Established(facts) = result else {
        panic!("source output coverage: {result:?}");
    };
    let binding = owner
        .bind_conditional_output_v1(facts, &mut budget)
        .unwrap();
    assert_eq!(binding.coverage().output_parameter_index(), 0);
    assert_eq!(binding.coverage().read_count(), 0);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_output_authenticates_effects_but_cannot_omit_the_reference_contract() {
    let source = output_source(7);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    budget.reserve_storage(floor).unwrap();
    let input = output_input(&source);
    let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
        input.semantic_root,
        input.launch_rank,
        input.pending.kernel().unwrap(),
        &input.access_sources,
        &[],
        "",
    );
    let translation = source
        .check_conditional_source_translation_v1(&input.pending, candidate, &mut budget)
        .unwrap();
    assert_eq!(translation.memory_effects(), 1);
    assert_eq!(translation.value_expressions(), 1);
    assert!(std::ptr::eq(translation.source(), &source));
    assert_eq!(budget.storage(), floor);
    let error = with_conditional_root_request_v1(&source, input, &mut budget, |_, _| {
        panic!("an unproved effect must not expose an aggregate request")
    })
    .err()
    .unwrap();
    assert!(
        matches!(
            error,
            ProductionConditionalContinuationErrorV1::Ranked(
                ProductionConditionalRankedOutputErrorV1::Contract
            )
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > 0);
}

#[test]
fn source_output_does_not_materialize_an_unproved_reference_request() {
    let source = output_source(7);
    let error = pending(output_recipe(&source, true)).err().unwrap();
    assert!(
        error
            .to_string()
            .contains("unbound functional-refinement request cannot be materialized"),
        "{error:?}"
    );
}

#[test]
fn source_output_joins_an_inert_ranked_request_without_granting_proof_authority() {
    let source = output_source(7);
    let recipe = output_recipe(&source, true);
    let rows = [output_source_row()];
    let candidate =
        crate::NativeRankedSourceCandidateV1::from_untrusted_parts(0, 1, &recipe, &rows, &[], "");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    budget.reserve_storage(floor).unwrap();
    let facts = fe2o3_kernel_ir::derive_conditional_total_view_from_verified_v1(
        source.executable.verified_module_ref_v1(),
        &source.executable.module().kernels[0].id,
        &mut budget,
    )
    .unwrap();
    let fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Established(facts) = facts else {
        panic!("source coverage")
    };
    let binding = source
        .bind_conditional_output_v1(facts, &mut budget)
        .unwrap();
    let coverage = binding
        .inspect_ranked_output_v1(candidate, &mut budget)
        .unwrap()
        .check_ranked_coverage_v1(&mut budget)
        .unwrap();
    assert_eq!(
        coverage.output().gpu_write_site(),
        fe2o3_pliron::ProductionGpuWriteSiteV2::new(1, 0)
    );
    assert!(matches!(
        coverage.output().dynamic_extent(),
        ProductionConditionalRankedExtentV1::CanonicalOutputLength {
            operand: ProductionRankedValueV1::Argument(0),
            ..
        }
    ));
    assert_eq!(budget.storage(), floor);
}

fn refused(
    source: &ProductionPreRankedKirOwnerV1,
    input: ProductionConditionalRootInputV1,
    work_limit: usize,
    storage_limit: usize,
) -> (ProductionConditionalContinuationErrorV1, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    budget.reserve_storage(floor).unwrap();
    let account = budget.work_ledger_identity_v1();
    let error = with_conditional_root_request_v1(source, input, &mut budget, |_, _| {
        panic!("no aggregate request without a proved reference contract")
    })
    .err()
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == account);
    (error, budget.work(), budget.peak_storage())
}

fn is_missing_contract(error: &ProductionConditionalContinuationErrorV1) -> bool {
    matches!(
        error,
        ProductionConditionalContinuationErrorV1::Ranked(
            ProductionConditionalRankedOutputErrorV1::Contract
        )
    )
}

#[test]
fn source_output_rejects_changed_scalar_semantics_before_the_reference_gate() {
    // Re-admit and lower a different source, without changing any retained owner.
    let source = output_source(8);
    let (error, _, _) = refused(&source, output_input(&source), WORK, STORAGE);
    assert!(
        matches!(
            error,
            ProductionConditionalContinuationErrorV1::Source(
                ProductionConditionalSourceTranslationErrorV1::Correspondence(
                    ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::ValueExpressionMismatch { .. }
                    )
                )
            )
        ),
        "{error:?}"
    );
}

#[test]
fn source_output_rejects_substituted_root_rank_and_access_rows() {
    let source = output_source(7);
    for mutation in 0..5 {
        let mut input = output_input(&source);
        match mutation {
            0 => input.semantic_root = 1,
            1 => input.launch_rank = 2,
            2 => input.access_sources.clear(),
            3 => input.access_sources.push(output_source_row()),
            4 => input.access_sources[0] = ProductionRankedAccessSourceV1::new(2, Some(1), 0, 1, 0),
            _ => unreachable!(),
        }
        let (error, _, _) = refused(&source, input, WORK, STORAGE);
        assert!(
            matches!(error, ProductionConditionalContinuationErrorV1::Source(_)),
            "mutation {mutation}: {error:?}"
        );
    }
}

#[test]
fn source_output_refusal_obeys_exact_original_budget_limits() {
    let source = output_source(7);
    let (error, work, peak) = refused(&source, output_input(&source), WORK, STORAGE);
    assert!(is_missing_contract(&error), "{error:?}");
    assert!(work > 0);
    assert!(peak > source.retained_analysis_storage_v1() + FLOOR);
    for (work_limit, storage_limit, reaches_contract) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let (error, spent, observed_peak) =
            refused(&source, output_input(&source), work_limit, storage_limit);
        assert_eq!(is_missing_contract(&error), reaches_contract, "{error:?}");
        assert!(spent <= work_limit);
        assert!(observed_peak <= storage_limit);
    }
}
