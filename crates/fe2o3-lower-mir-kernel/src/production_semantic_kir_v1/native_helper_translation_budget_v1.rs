/// Compatibility boundary for callers that do not supply a canonical ledger.
/// The helper cache still has explicit finite storage/work limits; the checked
/// production routes use the additive caller-budgeted entry below instead.
#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_translation_with_semantic_v1(
    semantic: Option<&AdmittedInertSemanticMirV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    lowering: &ProductionRankedKernelLoweringInputV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
        DEFAULT_ARGUMENT_CORRESPONDENCE_WORK_V1,
    );
    let mut budget = ArgumentBudgetV1::new(&mut work, DEFAULT_ARGUMENT_CORRESPONDENCE_STORAGE_V1);
    validate_mir_pliron_translation_with_semantic_and_budget_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        lowering,
        sources,
        executable_effect_sources,
        max_operations,
        &mut budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_translation_with_semantic_and_budget_v1(
    semantic: Option<&AdmittedInertSemanticMirV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    lowering: &ProductionRankedKernelLoweringInputV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    native_helper_value_expansion_v1::with_native_value_expansion_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        budget,
        |expansion| {
            validate_mir_pliron_translation_inner_v1(
                semantic,
                module,
                correspondence,
                kernel_id,
                lowering,
                sources,
                executable_effect_sources,
                max_operations,
                expansion,
            )
        },
    )
}
