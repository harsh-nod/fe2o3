#[derive(Debug)]
pub(crate) enum CheckedOutputModuleJoinErrorV1 {
    Projection(ProductionRankedProjectionErrorV1),
    Formal(fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1),
}

impl fmt::Display for CheckedOutputModuleJoinErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => error.fmt(f),
            Self::Formal(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CheckedOutputModuleJoinErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Projection(error) => Some(error),
            Self::Formal(error) => Some(error),
        }
    }
}

// Only this exact sentinel restores a typed error from the inner callback. A
// later surrounding resource/accounting failure must not be hidden by it.
const CHECKED_OUTPUT_MODULE_CALLBACK_ERROR_V1: &str = "checked-output module callback failed";

#[allow(clippy::too_many_arguments)]
pub(crate) fn with_projected_checked_output_module_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    root_inputs: &[ProductionRankedRootInputV1],
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'formal, 'scope, 'source, 'output> FnOnce(
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1,
    >,
) -> Result<T, CheckedOutputModuleJoinErrorV1> {
    use CheckedOutputModuleJoinErrorV1 as Error;
    let source = RankedProjectionSourceV1::from_legacy(materialized).map_err(Error::Projection)?;
    source.require_floor(budget).map_err(Error::Projection)?;
    if !reference_bindings.as_slice().is_empty() {
        return Err(Error::Projection(
            ProductionRankedProjectionErrorV1::Incomplete(
                "checked-output module requires separate authenticated reference discharge",
            ),
        ));
    }
    with_prepared_checked_output_module_v1(
        &source,
        materialized,
        bound,
        checked,
        profile,
        root_inputs,
        reference_bindings,
        budget,
        None,
        |_, formals, budget| next(formals, budget),
    )
}

pub(crate) fn with_source_checked_output_module_v1<T>(
    custody: &SourceRankedCustodyV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'formal, 'scope, 'source, 'output> FnOnce(
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1,
    >,
) -> Result<T, CheckedOutputModuleJoinErrorV1> {
    with_source_checked_output_module_view_v1(
        custody,
        bound,
        checked,
        profile,
        budget,
        |_, formals, budget| next(formals, budget),
    )
}

// The catalog is borrowed from the already constructed and retained J view.
// Historical adapters above do not invoke this paid getter.
pub(crate) fn with_source_checked_output_module_catalog_v1<T>(
    custody: &SourceRankedCustodyV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'formal, 'scope, 'source, 'output> FnOnce(
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'scope,
            'source,
            'output,
        >],
        &fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1,
    >,
) -> Result<T, CheckedOutputModuleJoinErrorV1> {
    with_source_checked_output_module_view_v1(
        custody,
        bound,
        checked,
        profile,
        budget,
        |view, formals, budget| {
            let catalog = view.output_pipeline_catalog(budget).map_err(
                fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1::SourceOutput,
            )?;
            next(formals, catalog, budget)
        },
    )
}

fn with_source_checked_output_module_view_v1<T>(
    custody: &SourceRankedCustodyV1<'_>,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'formal, 'scope, 'source, 'output> FnOnce(
        &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'_, '_>,
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1,
    >,
) -> Result<T, CheckedOutputModuleJoinErrorV1> {
    use CheckedOutputModuleJoinErrorV1 as Error;
    let source =
        RankedProjectionSourceV1::from_legacy(custody.materialized).map_err(Error::Projection)?;
    source.require_floor(budget).map_err(Error::Projection)?;
    budget
        .charge_work(5)
        .map_err(|error| Error::Projection(source_join_resource_v1(error)))?;
    if !std::ptr::eq(custody.original.materialized(), custody.materialized)
        || custody.original.root_count() != custody.inputs.len()
        || custody.partition.len() != custody.inputs.len()
        || custody.recorders.len() != custody.inputs.len()
    {
        return Err(Error::Projection(
            ProductionRankedProjectionErrorV1::Incomplete(
                "source-first borrowed source or complete root roster differs",
            ),
        ));
    }
    with_prepared_checked_output_module_v1(
        &source,
        custody.materialized,
        bound,
        checked,
        profile,
        custody.inputs,
        custody.references,
        budget,
        Some(custody),
        next,
    )
}

#[allow(clippy::too_many_arguments)]
fn with_prepared_checked_output_module_v1<T>(
    source: &RankedProjectionSourceV1<'_>,
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    root_inputs: &[ProductionRankedRootInputV1],
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    source_custody: Option<&SourceRankedCustodyV1<'_>>,
    next: impl for<'formal, 'scope, 'source, 'output> FnOnce(
        &fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1<'_, '_>,
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1,
    >,
) -> Result<T, CheckedOutputModuleJoinErrorV1> {
    use CheckedOutputModuleJoinErrorV1 as Error;
    use canonical_assertion_facts_v1::CanonicalAssertionErrorV1 as Assertion;
    use fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1 as Output;
    let resource =
        |error| ProductionRankedProjectionErrorV1::CanonicalAssertions(Assertion::Resource(error));
    let floor = budget.storage();
    let mut formal_error = None;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let run = |effects: &DefinedCallableEmptyEffectSummariesV1,
                   references: &[crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1]| {
                let (result, coordinate_storage) = {
                    let (coordinates, coordinate_storage) =
                        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                            materialized.executable(),
                            bound,
                            profile,
                            budget,
                        )
                        .map_err(|e| {
                            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                Assertion::Target(e),
                            )
                        })?;
                    budget
                        .reserve_storage(coordinate_storage.retained_storage())
                        .map_err(resource)?;
                    let (view, view_storage) =
                        fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                            materialized,
                            &coordinates,
                            checked,
                            budget,
                        )
                        .map_err(|e| {
                            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                                Assertion::Output(e),
                            )
                        })?;
                    budget
                        .reserve_storage(view_storage.retained_storage())
                        .map_err(resource)?;
                    let result =
                        checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                            &view,
                            budget,
                            |session| {
                                with_prepared_canonical_memory_source_session_v1(
                                    source,
                                    root_inputs,
                                    effects,
                                    references,
                                    session,
                                    source_custody,
                                    |analyses, budget| {
                                        if let Some(source) = source_custody {
                                            budget.charge_work(2).map_err(Output::Resource)?;
                                            if analyses.len() != source.inputs.len() {
                                                return Err(Output::Invalid("source-first actual-O memory roster differs"));
                                            }
                                            for (ordinal, analysis) in analyses.iter().enumerate() {
                                                budget.charge_work(2).map_err(Output::Resource)?;
                                                let recorder = source.recorders.get(ordinal).ok_or(Output::Invalid("source-first full recorder absent"))?;
                                                analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                                                    relation.check_borrowed_ranked_addresses_v1(source.original, ordinal, recorder.candidate(), budget)
                                                })?;
                                            }
                                        }
                                        match fe2o3_lower_mir_kernel::with_complete_formal_memory_module_v1(
                                analyses, budget, |formals, budget| next(&view, formals, budget),
                            ) {
                                Ok(value) => Ok(value),
                                Err(error) => {
                                    formal_error = Some(error);
                                    Err(Output::Invalid(CHECKED_OUTPUT_MODULE_CALLBACK_ERROR_V1))
                                }
                            }
                                    },
                                )
                            },
                        );
                    drop(view);
                    budget
                        .release_storage(view_storage.retained_storage())
                        .map_err(resource)?;
                    (result, coordinate_storage)
                };
                budget
                    .release_storage(coordinate_storage.retained_storage())
                    .map_err(resource)?;
                result
            };
        if let Some(custody) = source_custody {
            run(custody.effects, custody.partition)
        } else {
            with_ranked_root_preparation_v1(source, root_inputs, reference_bindings, run)
        }
    }));
    let cleanup = budget
        .storage()
        .checked_sub(floor)
        .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    match outcome {
        Err(payload) => {
            cleanup.map_err(|e| Error::Projection(resource(e)))?;
            std::panic::resume_unwind(payload)
        }
        Ok(result) => {
            cleanup.map_err(|e| Error::Projection(resource(e)))?;
            match result {
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(Assertion::Output(
                    Output::Invalid(CHECKED_OUTPUT_MODULE_CALLBACK_ERROR_V1),
                ))) => match formal_error {
                    Some(error) => Err(Error::Formal(error)),
                    None => Err(Error::Projection(
                        ProductionRankedProjectionErrorV1::Incomplete(
                            "checked-output module callback error custody absent",
                        ),
                    )),
                },
                other => other.map_err(Error::Projection),
            }
        }
    }
}
