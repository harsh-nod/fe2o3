/// Compatibility boundary for callers that do not supply a canonical ledger.
/// The helper cache still has explicit finite storage/work limits; the checked
/// production routes use the additive caller-budgeted entry below instead.
#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_translation_with_semantic_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
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
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    lowering: &ProductionRankedKernelLoweringInputV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    validate_mir_pliron_recipe_translation_with_semantic_and_budget_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        lowering.kernel(),
        sources,
        executable_effect_sources,
        max_operations,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_recipe_translation_with_semantic_and_budget_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    validate_mir_pliron_recipe_translation_with_allowance_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        recipe,
        sources,
        executable_effect_sources,
        max_operations,
        budget,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_recipe_translation_with_allowance_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut native_helper_value_expansion_v1::TranslationAllowanceV1>,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    validate_mir_pliron_recipe_translation_with_allowance_resources_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        recipe,
        sources,
        executable_effect_sources,
        max_operations,
        budget,
        allowance,
    )
    .map_err(native_helper_value_expansion_v1::NativeTranslationErrorV1::into_translation)
}

#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_recipe_translation_with_allowance_resources_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut native_helper_value_expansion_v1::TranslationAllowanceV1>,
) -> Result<
    ProductionMirPlironTranslationValidationV1,
    native_helper_value_expansion_v1::NativeTranslationErrorV1,
> {
    native_helper_value_expansion_v1::with_native_value_expansion_and_allowance_resources_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        budget,
        allowance,
        |expansion| {
            validate_mir_pliron_translation_inner_v1(
                semantic.map(ProductionSemanticSsaOwnerV1::source_semantic),
                module,
                correspondence,
                kernel_id,
                recipe,
                sources,
                executable_effect_sources,
                max_operations,
                expansion,
            )
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_mir_pliron_translation_with_allowance_resources_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel_id: &str,
    lowering: &ProductionRankedKernelLoweringInputV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    max_operations: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut native_helper_value_expansion_v1::TranslationAllowanceV1>,
) -> Result<
    ProductionMirPlironTranslationValidationV1,
    native_helper_value_expansion_v1::NativeTranslationErrorV1,
> {
    validate_mir_pliron_recipe_translation_with_allowance_resources_v1(
        semantic,
        module,
        correspondence,
        kernel_id,
        lowering.kernel(),
        sources,
        executable_effect_sources,
        max_operations,
        budget,
        allowance,
    )
}

/// Candidate-only check while the exact materialized owner is still intact.
/// No receipt/attachment/ordinary admission is created by this private helper.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
fn validate_bf16_nominal_recipe_translation_with_allowance_resources_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    kernel_id: &str,
    recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    sources: &[ProductionRankedAccessSourceV1],
    executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut native_helper_value_expansion_v1::TranslationAllowanceV1>,
) -> Result<
    ProductionMirPlironTranslationValidationV1,
    native_helper_value_expansion_v1::NativeTranslationErrorV1,
> {
    native_helper_value_expansion_v1::with_nominal_value_expansion_and_allowance_resources_v1(
        owner,
        kernel_id,
        budget,
        allowance,
        |expansion| {
            validate_mir_pliron_translation_inner_v1(
                Some(owner.semantic_ssa().source_semantic()),
                owner.executable().module(),
                &owner.correspondence,
                kernel_id,
                recipe,
                sources,
                executable_effect_sources,
                owner.limits.max_operations,
                expansion,
            )
        },
    )
}

/// Fixed owner/root/floor preflight only; no detached observation is authority.
fn require_private_bf16_nominal_translation_owner_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    selected_root: SemanticFunctionIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    budget.charge_work(1)?;
    if owner.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::Bf16Nominal {
        return Err(
            ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                consumer: "private nominal translation",
            },
        );
    }
    budget.charge_work(1)?;
    let emission = owner
        .bf16_call_instance_emission_v1()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if !std::ptr::eq(emission.owner(), owner) || emission.root() != selected_root {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    budget.charge_work(1)?;
    if budget.storage() < owner.unit_local_source_storage_floor_v1()? {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(())
}

/// Scoped fixed phase-header accounting, not an owner/receipt constructor.
/// All helper queries share this one allowance; global and local caps are unchanged.
fn with_private_nominal_translation_phase_v1(
    budget: &mut ArgumentBudgetV1<'_>,
    work_limit: usize,
    storage_limit: usize,
    action: impl FnOnce(
        &mut ArgumentBudgetV1<'_>,
        &mut native_helper_value_expansion_v1::TranslationAllowanceV1,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionSemanticKirErrorV1,
    >,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionSemanticKirErrorV1> {
    use native_helper_value_expansion_v1::TranslationAllowanceV1;
    budget.check_prior_denials_v1()?;
    let header = std::mem::size_of::<TranslationAllowanceV1>();
    // Local-first, including the header, before any original debit/allocation.
    if header > work_limit || header > storage_limit {
        return Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::ResourceLimit,
        ));
    }
    let floor = budget.storage();
    let protected = floor
        .checked_add(header)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let slot = budget as *const _ as usize;
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(header)?;
    if let Err(error) = budget.charge_work(header) {
        budget.release_storage(header)?;
        return Err(error.into());
    }
    let result = {
        match TranslationAllowanceV1::with_prepaid_header_v1(
            budget,
            floor,
            work_limit,
            storage_limit,
        ) {
            Err(error) => Err(ProductionSemanticKirErrorV1::MirPlironTranslation(error)),
            Ok(mut allowance) => {
                // The nested nominal validator catches its callback panics.
                // Also clean this header if a future caller panics before entry.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    action(budget, &mut allowance)
                }));
                let refused = allowance.refused_v1();
                drop(allowance);
                let result = match result {
                    Ok(result) => result,
                    Err(payload) => {
                        drop(payload);
                        Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                            ProductionMirPlironTranslationErrorV1::KernelShape,
                        ))
                    }
                };
                // Do not hide a swallowed local denial, but preserve the
                // nested bridge's more precise original-resource diagnostic.
                if refused
                    && !matches!(
                        &result,
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
                    )
                {
                    Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
                        ProductionMirPlironTranslationErrorV1::ResourceLimit,
                    ))
                } else {
                    result
                }
            }
        }
    };
    if budget as *const _ as usize != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() != protected
    {
        drop(result);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    // An error carries no allocated successful report. On success, only the
    // fixed existing report leaves this scope; it grants no source authority.
    budget.release_storage(header)?;
    // Keep the nested validator's typed local-first refusal. Other outcomes
    // cannot conceal an original sticky denial.
    if !matches!(
        &result,
        Err(ProductionSemanticKirErrorV1::MirPlironTranslation(
            ProductionMirPlironTranslationErrorV1::ResourceLimit,
        ))
    ) {
        budget.check_prior_denials_v1()?;
    }
    result
}

impl ProductionPreRankedKirOwnerV1 {
    /// Checks one private nominal candidate against this intact source owner.
    ///
    /// This cross-crate checking entry creates no ranked receipt, attachment,
    /// ordinary admission or launch authority. The caller must retain the same
    /// materialization owner/account and actual ranked objects through postflight.
    /// Helper work/scratch uses the original caller Budget and one unchanged
    /// owner-local allowance; historical correlation checks keep their old limits.
    #[doc(hidden)]
    pub fn verify_private_bf16_nominal_candidate_translation_with_budget_v1(
        &self,
        selected_root: SemanticFunctionIdV1,
        lowering: &ProductionRankedKernelLoweringInputV1,
        sources: &[ProductionRankedAccessSourceV1],
        executable_effect_sources: &[ProductionRankedExecutableEffectSourceV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionMirPlironTranslationValidationV1, ProductionSemanticKirErrorV1> {
        require_private_bf16_nominal_translation_owner_v1(self, selected_root, budget)?;
        budget.charge_work(1)?;
        if !mandatory_generic_checks_are_clean(lowering) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        with_private_nominal_translation_phase_v1(
            budget,
            self.limits.max_argument_correspondence_work,
            self.limits.max_argument_correspondence_storage,
            |budget, allowance| {
                validate_bf16_nominal_recipe_translation_with_allowance_resources_v1(
                    self,
                    lowering.kernel().function_name(),
                    lowering.kernel(),
                    sources,
                    executable_effect_sources,
                    budget,
                    Some(allowance),
                )
                .map_err(native_helper_value_expansion_v1::NativeTranslationErrorV1::into_semantic)
            },
        )
    }
}
