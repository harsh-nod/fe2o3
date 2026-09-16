#[derive(Clone, Copy)]
enum ProjectedGlobalWriteValuesV1 {
    Expression,
    CanonicalSourceUse,
}

// Shared pre-verification geometry, not a ranked root or evidence. The legacy
// caller still attaches every expression before constructing its old root.
struct PreparedProjectedRankedGeometryV1 {
    blocks: Vec<ProductionRankedBlockV1>,
    sources: Vec<ProjectedAccessSourceV1>,
    executable_effect_sources: Vec<ProductionRankedExecutableEffectSourceV1>,
    argument_count: usize,
    reserved_reference_values: Option<Vec<ProductionRankedValueIdV1>>,
    intrinsic: IntrinsicProjectionV1,
    semantic_u32_induction: fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1,
    kernel_binding: [u8; 32],
    incomplete: Option<ProductionRankedProjectionErrorV1>,
}

// Kept only inside the completing callback's enclosing scope. There is no
// conversion into ProductionRankedRootProgramV1 or any legacy receipt/roster.
struct CanonicalMemoryProjectedRootV1 {
    selected_root: SemanticFunctionIdV1,
    selected_function: SemanticFunctionIdV1,
    kernel_binding: [u8; 32],
    lowering: ProductionRankedKernelLoweringInputV1,
    access_sources: Vec<ProductionRankedAccessSourceV1>,
    executable_effect_sources: Vec<ProductionRankedExecutableEffectSourceV1>,
    control: canonical_memory_control_v1::CanonicalMemoryControlRecorderV1,
}

fn project_canonical_memory_root_v1(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    callable_effects: &DefinedCallableEmptyEffectSummariesV1,
    selection: SemanticKernelBodySelectionV1,
    input: &ProductionRankedRootInputV1,
    source_root: ProductionSourceLaunchRootV1,
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<CanonicalMemoryProjectedRootV1, ProductionRankedProjectionErrorV1> {
    if !references.as_slice().is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "canonical memory analysis does not discharge authenticated reference obligations",
        ));
    }
    let mut control = canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(facts)?;
    let PreparedProjectedRankedGeometryV1 {
        blocks,
        sources,
        executable_effect_sources,
        argument_count,
        reserved_reference_values,
        intrinsic,
        semantic_u32_induction: _,
        kernel_binding,
        incomplete,
    } = prepare_projected_ranked_geometry_v1(
        semantic_ssa,
        callable_effects,
        selection,
        input,
        source_root,
        references,
        facts,
        ProjectedGlobalWriteValuesV1::CanonicalSourceUse,
        Some(&mut control),
    )?;
    if reserved_reference_values.is_some() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "canonical memory analysis unexpectedly reserved reference values",
        ));
    }
    // This route never invokes the source expression resolver or converts an
    // unresolved ValueAccess into a memory access. It emits no value placeholder.
    let access_sources = production_access_sources(&blocks, &sources, facts)?;
    let semantic = semantic_ssa.source_semantic();
    let root = &semantic.functions()[selection.root().index() as usize];
    let system_coherent_allocations = intrinsic
        .local_contracts
        .allocations
        .iter()
        .flatten()
        .filter(|contract| contract.singleton_object)
        .map(|contract| contract.allocation_origin)
        .collect::<Vec<_>>();
    let kernel = ProductionRankedKernelV1::new(function_name(root)?, argument_count, blocks)
        .map_err(ProductionRankedProjectionErrorV1::Recipe)?;
    if let Some(error) = incomplete {
        return Err(error);
    }
    let ranked_ir = format_ranked_cfg(function_name(root)?, kernel.blocks())?;
    let construction = ProductionConstructionV1::ranked_kernel(ROOT_NAME_V1, kernel)
        .map_err(ProductionRankedProjectionErrorV1::Construction)?;
    let lowering = compile_ranked_kernel_for_gfx942_lowering_v1(
        construction,
        ProductionSessionLimitsV1::default(),
        system_coherent_allocations,
    )
    .map_err(|error| ProductionRankedProjectionErrorV1::Compile {
        error: Box::new(error),
        ranked_ir,
        access_sources: sources,
    })?;
    let ranked_ir = format_ranked_cfg(function_name(root)?, lowering.kernel().blocks())?;
    // Preserve the existing structural source-name/source-roster checks. This
    // API grants no translation receipt and does not pretend to prove values.
    fe2o3_lower_mir_kernel::validate_borrowed_ranked_semantic_projection_candidate_with_generated_effects_v1(
        semantic_ssa.source_owner(), selection.root(), &lowering, &ranked_ir,
        &access_sources, &executable_effect_sources,
    ).map_err(ProductionRankedProjectionErrorV1::StructuralValidation)?;
    Ok(CanonicalMemoryProjectedRootV1 {
        selected_root: selection.root(),
        selected_function: selection.body(),
        kernel_binding,
        lowering,
        access_sources,
        executable_effect_sources,
        control,
    })
}

/// Held graph-memory consumer, not a default compiler route. The only output is
/// the caller's value from a completing scoped callback; actual projections do
/// not escape. Source/projector/PLIRON allocations keep their existing bounded
/// phase domains, while inert control rows and all new relation scratch remain
/// prepaid on the caller's canonical ledger until this scope drops them.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "held for #271 exact-output final attachment")
)]
fn with_projected_canonical_memory_analysis_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    root_inputs: &[ProductionRankedRootInputV1],
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    body: impl for<'scope, 'source, 'output> FnOnce(
        &[fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1,
    >,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    let source = RankedProjectionSourceV1::from_legacy(materialized)?;
    source.require_floor(budget)?;
    if !reference_bindings.as_slice().is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "canonical memory analysis requires separate authenticated reference discharge",
        ));
    }
    with_ranked_root_preparation_v1(
        &source,
        root_inputs,
        reference_bindings,
        |effects, references| {
            checked_output_session_v1::with_checked_output_assertions_budget_v1(
                materialized,
                bound,
                checked,
                profile,
                budget,
                |session| {
                    with_prepared_canonical_memory_session_v1(
                        &source,
                        root_inputs,
                        effects,
                        references,
                        session,
                        body,
                    )
                },
            )
        },
    )
}

// Private common continuation for constructors that retain the SAME source,
// actual source/output view and caller ledger. Endpoint admission precedes this
// hook; it neither constructs an endpoint nor chooses a new optimization route.
fn with_prepared_canonical_memory_session_v1<T>(
    source: &RankedProjectionSourceV1<'_>,
    root_inputs: &[ProductionRankedRootInputV1],
    effects: &DefinedCallableEmptyEffectSummariesV1,
    references: &[crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1],
    session: &mut checked_output_session_v1::CheckedOutputAssertionSessionV1<'_, '_, '_, '_, '_>,
    body: impl for<'scope, 'source, 'output> FnOnce(
        &[fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        T,
        fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1,
    >,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    use canonical_assertion_facts_v1::CanonicalAssertionErrorV1;
    use fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1;
    session.with_canonical_memory_scope_v1(|session| {
        let semantic = source.semantic_ssa().source_semantic();
        let mut roots = Vec::with_capacity(root_inputs.len());
        for ((input, source_root), references) in root_inputs
            .iter()
            .zip(source.source_launch().roots())
            .zip(references)
        {
            let selection = semantic
                .select_kernel_body_for_root_v1(source_root.selected_root())
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "canonical memory root has no direct body or transparent wrapper",
                ))?;
            let mut facts = session.for_source(source_root.selected_root(), selection.body());
            let root = project_canonical_memory_root_v1(
                source.semantic_ssa(),
                effects,
                selection,
                input,
                *source_root,
                references,
                &mut facts,
            )
            .map_err(|error| {
                error.with_deterministic_root_context(
                    source_root.selected_root(),
                    selection.body(),
                    input.logical_name.as_bytes(),
                )
            })?;
            if root.kernel_binding != input.kernel_binding {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "canonical memory root kernel binding changed",
                ));
            }
            roots.push(root);
        }
        source
            .semantic_ssa()
            .verify_replay()
            .map_err(ProductionRankedProjectionErrorV1::SemanticSsa)?;
        session.with_output_occurrences_v1(|view, budget| {
            // These borrowed candidate rows are source/projector
            // descriptors, with the same finite root roster bound.
            // Their byte lifetime is additionally prepaid here.
            budget.charge_work(9).map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(error),
                )
            })?;
            let count = roots.len();
            let bytes = count
                .checked_mul(std::mem::size_of::<
                    ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
                >())
                .and_then(|bytes| {
                    bytes.checked_add(std::mem::size_of::<
                        Vec<ProductionCanonicalMemoryAnalysisCandidateV1<'_>>,
                    >())
                })
                .ok_or(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                    ),
                ))?;
            budget.reserve_storage(bytes).map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(error),
                )
            })?;
            let mut candidates = Vec::new();
            candidates.try_reserve_exact(count).map_err(|_| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
                    ),
                )
            })?;
            // Reconcile excess before the next fallible work charge.
            let extra = candidates
                .capacity()
                .checked_sub(count)
                .and_then(|extra| {
                    extra.checked_mul(std::mem::size_of::<
                        ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
                    >())
                })
                .ok_or(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                    ),
                ))?;
            if extra != 0 {
                budget.reserve_storage(extra).map_err(|error| {
                    ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Resource(error),
                    )
                })?;
            }
            for root in &roots {
                budget.charge_work(1).map_err(|error| {
                    ProductionRankedProjectionErrorV1::CanonicalAssertions(
                        CanonicalAssertionErrorV1::Resource(error),
                    )
                })?;
                candidates.push(ProductionCanonicalMemoryAnalysisCandidateV1 {
                    selected_root: root.selected_root,
                    selected_function: root.selected_function,
                    lowering: &root.lowering,
                    access_sources: &root.access_sources,
                    executable_effect_sources: &root.executable_effect_sources,
                    control: root.control.candidate(),
                });
            }
            view.with_conditional_memory_control_coverage_v1(
                &candidates,
                budget,
                |control, budget| {
                    view.with_canonical_store_analysis_v1(&candidates, control, budget, body)
                },
            )
            .map_err(|error| {
                ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Output(error),
                )
            })
        })
    })
}
