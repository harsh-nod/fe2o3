// Complete ordinary recipe preparation, separated from verification without
// changing its preparation order or ending its lexical scratch lifetime early.
// This payload is UNVERIFIED. It grants no nominal, memory, or launch authority.
struct PreparedRankedRootRecipeV1<'s> {
    root_function: &'s SemanticFunctionDeclV1,
    kernel_binding: [u8; 32],
    semantic_u32_induction: fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1,
    reserved_reference_values: Option<Vec<ProductionRankedValueIdV1>>,
    kernel: ProductionRankedKernelV1,
    sources: Vec<ProjectedAccessSourceV1>,
    reference_writes: Vec<crate::production_reference_effect_join_v2::RankedGpuWriteV2>,
    helper_expression_storage: usize,
    access_sources: Vec<ProductionRankedAccessSourceV1>,
    executable_effect_sources: Vec<ProductionRankedExecutableEffectSourceV1>,
    system_coherent_allocations: Vec<u64>,
}

#[allow(clippy::too_many_arguments)]
fn with_prepared_ranked_root_recipe_v1<'s, T, F: ProjectedAssertionFactsV1>(
    semantic: &'s AdmittedInertSemanticMirV1,
    callable_effects: &DefinedCallableEmptyEffectSummariesV1,
    selection: SemanticKernelBodySelectionV1,
    input: &ProductionRankedRootInputV1,
    source_root: ProductionSourceLaunchRootV1,
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    assertion_facts: &mut F,
    singletons: &[u8],
    borrows: Option<&scalar_borrow_projection_v1::ScalarPrivateBorrowsV1<'_>>,
    induction_scope: &mut multi_entry_induction_v1::Scope,
    continue_with_recipe: impl FnOnce(
        PreparedRankedRootRecipeV1<'s>,
        &mut F,
    ) -> Result<T, ProductionRankedProjectionErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    let source_launch = &input.source_launch;
    let semantic_u32_induction =
        fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(semantic, selection.body())
            .map_err(ProductionRankedProjectionErrorV1::SemanticU32Induction)?;
    let root_function = semantic
        .functions()
        .get(selection.root().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an out-of-range semantic kernel root",
        ))?;
    let function = semantic
        .functions()
        .get(selection.body().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an out-of-range semantic kernel body",
        ))?;
    if root_function.role() != SemanticFunctionRoleV1::KernelRoot {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a root without the KernelRoot role",
        ));
    }
    let kernel_binding = *root_function
        .kernel_entry()
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic KernelRoot without an authenticated kernel entry",
        ))?
        .kernel_binding_identity()
        .as_bytes();

    if source_root.selected_root() != selection.root()
        || source_root.semantic_root_identity() != root_function.identity()
        || source_root.kernel_binding() != kernel_binding
        || source_root.source_launch() != source_launch_input_v1(source_launch)
    {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "source launch roster root changed before ranked projection",
        ));
    }
    let constants = constant_locals(function)?;
    let mut entry_operations = vec![ranked_execution_layout_v1(source_root.layout())];
    let mut next_value = 0_u32;
    let reserved_reference_values = if reference_bindings.as_slice().is_empty() {
        None
    } else {
        let output_ranks =
            crate::production_reference_effect_join_v2::reserved_reference_output_ranks_v2(
                reference_bindings,
            )?;
        let count = crate::production_reference_effect_join_v2::reserved_reference_value_count_v2(
            reference_bindings,
        )?;
        let mut values = Vec::with_capacity(count);
        for rank in output_ranks {
            for _ in 0..3 {
                let result = next_value_id(&mut next_value)?;
                values.push(result);
                entry_operations
                    .push(ProductionRankedOperationV1::SemanticConstant { result, value: 0 });
            }
            for axis in 0..rank {
                let symbol = u32::try_from(axis).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "reference-effect logical point rank does not fit the semantic symbol domain",
                    )
                })?;
                let result = next_value_id(&mut next_value)?;
                values.push(result);
                entry_operations
                    .push(ProductionRankedOperationV1::SemanticSymbol { result, symbol });
            }
        }
        debug_assert_eq!(values.len(), count);
        Some(values)
    };
    let mut incomplete = None;
    let mut projected_views = ProjectedViewsV1::new(function.locals().len(), Some(assertion_facts))
        .with_scalar_private_singletons(singletons)
        .with_scalar_private_borrows(borrows, semantic.target());
    let mut discarded_ir = String::new();
    // Fixed-size extent provenance construction visits at most one checked
    // receiver per semantic terminator; use the existing source-phase ledger.
    let extent_work = function.blocks().len().checked_mul(16).ok_or_else(|| {
        ranked_projection_source_v1::resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
        )
    })?;
    projected_views.charge_private_array_work(extent_work)?;
    let (intrinsic, bounds_checks) = projected_views.with_assertion_facts_v1(|facts| {
        slice_extent_projection_v1::with_scope(function.locals().len(), facts, |scope| {
            let mut intrinsic = project_intrinsic_contracts_with_multi_entry_v1(
                semantic.callables(),
                callable_effects,
                semantic.types(),
                function,
                bounded_linear_launch_extent_v1(source_launch),
                &constants,
                &mut entry_operations,
                &mut next_value,
                &mut discarded_ir,
                Some(&mut multi_entry_induction_v1::Context {
                    scope: induction_scope,
                    facts: scope.facts(),
                }),
            )?;
            let mut scratch = intrinsic.slice_extent_scratch.take().ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "slice extent scratch was not retained",
                ),
            )?;
            scope.retain(&scratch)?;
            let bounds = project_rust_bounds_checks_with_ordinary_v1(
                semantic.types(),
                function,
                intrinsic.extent_argument_count,
                &intrinsic.index_values,
                &intrinsic.ordinary_index_values,
                Some(
                    &intrinsic
                        .local_contracts
                        .checked_references
                        .enum_payload_dominance,
                ),
                &mut entry_operations,
                &mut next_value,
                Some(&mut slice_extent_projection_v1::Context {
                    scratch: &mut scratch,
                    facts: scope.facts(),
                }),
            )?;
            Ok((intrinsic, bounds))
        })
    })?;
    let switch_predicates = switch_predicates(
        function,
        &intrinsic.option_predicates,
        &intrinsic.direct_switch_predicates,
    )?;
    let mut projected_blocks = Vec::new();
    let mut projected_effect_count = 0_usize;
    for (block_index, block) in function.blocks().iter().enumerate() {
        let mut operations = Vec::new();
        let mut guarded_sites = Vec::new();
        let mut local_sources = Vec::new();
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let source_start = local_sources.len();
            let guarded_start = guarded_sites.len();
            projected_views.begin_site(
                ProjectedSemanticAccessSiteV1 {
                    block: block_index,
                    statement: Some(statement_index),
                },
                source_start,
                guarded_start,
            );
            let initializer = project_private_array_initializer_v1(
                semantic.types(),
                function,
                statement,
                block_index,
                statement_index,
                &mut projected_views,
                &mut operations,
                &mut local_sources,
                &mut next_value,
            )?;
            if !initializer {
                retain_incomplete(
                    project_statement_accesses(
                        semantic.types(),
                        function,
                        block_index,
                        &bounds_checks.checks,
                        statement,
                        &constants,
                        &intrinsic.local_contracts,
                        &intrinsic.guarded_accesses,
                        &mut guarded_sites,
                        &mut projected_views,
                        &mut operations,
                        &mut local_sources,
                        &mut next_value,
                        &mut discarded_ir,
                    ),
                    &mut incomplete,
                )?;
            }
            bind_projected_access_site(
                &mut local_sources[source_start..],
                &mut guarded_sites[guarded_start..],
                ProjectedSemanticAccessSiteV1 {
                    block: block_index,
                    statement: Some(statement_index),
                },
            )?;
        }
        let source_start = local_sources.len();
        let guarded_start = guarded_sites.len();
        projected_views.begin_site(
            ProjectedSemanticAccessSiteV1 {
                block: block_index,
                statement: None,
            },
            source_start,
            guarded_start,
        );
        retain_incomplete(
            project_terminator_accesses(
                semantic.callables(),
                callable_effects,
                semantic.types(),
                function,
                block_index,
                &bounds_checks.checks,
                block.terminator().kind(),
                block.terminator().source(),
                &constants,
                &intrinsic.local_contracts,
                &intrinsic.guarded_accesses,
                &mut guarded_sites,
                &mut projected_views,
                &mut operations,
                &mut local_sources,
                &mut next_value,
                &mut discarded_ir,
            ),
            &mut incomplete,
        )?;
        if let Some(tensor_layout) = intrinsic.tensor_layouts.get(block_index).cloned().flatten() {
            reserve_operation(&mut operations)?;
            // This records contract consistency for the mandatory verifier. It
            // carries no load-refinement or artifact authority by itself; that
            // authority is joined later with the exact semantic owner and KIR.
            operations.push(tensor_layout);
        }
        if let Some(effect) = intrinsic
            .capability_read_effects
            .get(block_index)
            .copied()
            .flatten()
        {
            let operation = operations.len();
            reserve_operation(&mut operations)?;
            operations.push(ProductionRankedOperationV1::AllocationEffect {
                kind: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: effect.allocation.allocation_origin,
                noalias_class: effect.allocation.noalias_class,
            });
            local_sources.push(ProjectedAccessSourceV1 {
                block: block_index,
                operation,
                access: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                source: effect.source,
                output_extent: None,
                semantic_site: None,
            });
        }
        if let Some(effect) = intrinsic
            .transpose_workgroup_effects
            .get(block_index)
            .copied()
            .flatten()
        {
            let (allocation_origin, noalias_class) =
                gfx950_transpose_workgroup_identity_v1(effect.format);
            let operation = operations.len();
            reserve_operation(&mut operations)?;
            operations.push(ProductionRankedOperationV1::AllocationEffect {
                kind: effect.access,
                memory_space: MemorySpaceAttr::Workgroup,
                allocation_origin,
                noalias_class,
            });
            local_sources.push(ProjectedAccessSourceV1 {
                block: block_index,
                operation,
                access: effect.access,
                memory_space: MemorySpaceAttr::Workgroup,
                source: block.terminator().source(),
                output_extent: None,
                semantic_site: None,
            });
        }
        if let Some(access) = intrinsic
            .read_view_effects
            .get(block_index)
            .cloned()
            .flatten()
        {
            guarded_sites.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "strided read access-site storage cannot be reserved",
                )
            })?;
            guarded_sites.push(GuardedAccessSiteV1 {
                insertion_operation: operations.len(),
                access,
            });
        }
        if let Some(access) = intrinsic
            .direct_read_effects
            .get(block_index)
            .cloned()
            .flatten()
        {
            guarded_sites.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "volatile read access-site storage cannot be reserved",
                )
            })?;
            guarded_sites.push(GuardedAccessSiteV1 {
                insertion_operation: operations.len(),
                access,
            });
        }
        if let Some(access) = intrinsic
            .direct_write_effects
            .get(block_index)
            .cloned()
            .flatten()
        {
            guarded_sites.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "write-only access-site storage cannot be reserved",
                )
            })?;
            guarded_sites.push(GuardedAccessSiteV1 {
                insertion_operation: operations.len(),
                access,
            });
        }
        bind_projected_access_site(
            &mut local_sources[source_start..],
            &mut guarded_sites[guarded_start..],
            ProjectedSemanticAccessSiteV1 {
                block: block_index,
                statement: None,
            },
        )?;
        let mut projected = order_projected_block_effects(
            operations,
            guarded_sites,
            local_sources,
            &mut entry_operations,
        )?;
        if let Some(effect) = intrinsic
            .pipeline_effects
            .get(block_index)
            .cloned()
            .flatten()
        {
            projected.items.push(ProjectedBlockItemV1::Pipeline(effect));
        }
        if let Some(effects) = intrinsic
            .generated_terminator_effects
            .get(block_index)
            .cloned()
            .flatten()
        {
            projected.items.extend(
                effects
                    .into_iter()
                    .map(ProjectedBlockItemV1::GeneratedFromSemanticTerminator),
            );
        }
        projected_effect_count = projected_effect_count
            .checked_add(projected.items.len())
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection operation count overflow",
            ))?;
        if projected_effect_count
            .checked_add(entry_operations.len())
            .is_none_or(|count| count > MAX_RANKED_BOUNDS_OPERATIONS)
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection exceeds the ranked operation limit",
            ));
        }
        projected_blocks.push(projected);
    }
    let slice_queries = projected_views.finish();
    if bounds_checks.checks.iter().any(|check| {
        check.must_authorize_access
            && projected_blocks
                .get(check.access_block)
                .is_none_or(|block| !projected_block_uses_bounds_check(block, *check))
    }) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a Rust bounds assertion does not authorize one matching projected access",
        ));
    }
    if !projected_blocks
        .iter()
        .any(ProjectedSemanticBlockV1::has_memory_access)
    {
        if let Some(error) = incomplete.take() {
            return Err(error);
        }
    }
    // The launch layout defines the domain, including complete memory-free kernels.
    if entry_operations.first() != Some(&ranked_execution_layout_v1(source_root.layout())) {
        incomplete.get_or_insert(ProductionRankedProjectionErrorV1::Incomplete(
            "ranked execution domain differs from the authenticated source launch",
        ));
    }
    let (blocks, sources, executable_effect_sources) = build_ranked_cfg(
        semantic.types(),
        function,
        semantic.callables(),
        &switch_predicates,
        &intrinsic.deterministic_switches,
        &intrinsic.uniform_inductions,
        entry_operations,
        projected_blocks,
        assertion_facts,
        Some(induction_scope),
    )?;
    let (reference_writes, helper_expression_storage) = if reference_bindings.as_slice().is_empty()
    {
        (
            projected_reference_gpu_writes_v2(
                semantic.types(),
                function,
                semantic.callables(),
                &blocks,
                &sources,
            )?,
            0,
        )
    } else {
        defined_helper_expression_v1::projected_reference_gpu_writes_with_helpers_v1(
            semantic,
            function,
            &blocks,
            &sources,
            assertion_facts,
        )?
    };
    let access_sources = production_access_sources(
        semantic.types(),
        function,
        &blocks,
        &sources,
        assertion_facts,
    )?;
    slice_queries.validate(&blocks, &access_sources)?;
    let system_coherent_allocations = intrinsic
        .local_contracts
        .allocations
        .iter()
        .flatten()
        .filter(|contract| contract.singleton_object)
        .map(|contract| contract.allocation_origin)
        .collect::<Vec<_>>();
    let kernel = ProductionRankedKernelV1::new(
        function_name(root_function)?,
        bounds_checks.argument_count,
        blocks,
    )
    .map_err(ProductionRankedProjectionErrorV1::Recipe)?;
    if let Some(error) = incomplete {
        return Err(error);
    }
    continue_with_recipe(
        PreparedRankedRootRecipeV1 {
            root_function,
            kernel_binding,
            semantic_u32_induction,
            reserved_reference_values,
            kernel,
            sources,
            reference_writes,
            helper_expression_storage,
            access_sources,
            executable_effect_sources,
            system_coherent_allocations,
        },
        assertion_facts,
    )
}

// Private source-bound preparation for the shared CFG emitter. The sole
// production constructor performs the existing analyzer. It does not accept
// caller-authored masks or convert the nominal observer into ordinary readiness.
struct PreparedRootCfgAssertionsV1<'s> {
    types: &'s [SemanticTypeDeclV1],
    function: &'s SemanticFunctionDeclV1,
    decisions: Vec<bool>,
}

impl<'s> PreparedRootCfgAssertionsV1<'s> {
    fn analyze(
        types: &'s [SemanticTypeDeclV1],
        function: &'s SemanticFunctionDeclV1,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let decisions = SemanticAssertProofsV1::analyze(types, function)?;
        Ok(Self {
            types,
            function,
            decisions,
        })
    }

    fn into_decisions_for(
        self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Result<Vec<bool>, ProductionRankedProjectionErrorV1> {
        if !std::ptr::eq(self.function, function)
            || self.types.as_ptr() != types.as_ptr()
            || self.types.len() != types.len()
            || self.decisions.len() != function.blocks().len()
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "prepared root assertion source binding differs",
            ));
        }
        Ok(self.decisions)
    }
}

// Ordinary continuation only. The nominal driver must not call this legacy
// resource route or treat the unverified recipe payload as admission.
fn verify_prepared_ranked_root_recipe_v1(
    recipe: PreparedRankedRootRecipeV1<'_>,
    selection: SemanticKernelBodySelectionV1,
    input: &ProductionRankedRootInputV1,
    source_root: ProductionSourceLaunchRootV1,
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    assertion_facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<ProductionRankedRootProgramV1, ProductionRankedProjectionErrorV1> {
    let logical_name = input.logical_name.as_str();
    let source_launch = &input.source_launch;
    let PreparedRankedRootRecipeV1 {
        root_function,
        kernel_binding,
        semantic_u32_induction,
        reserved_reference_values,
        kernel,
        sources,
        reference_writes,
        helper_expression_storage,
        access_sources,
        executable_effect_sources,
        system_coherent_allocations,
    } = recipe;
    let verification = if reference_bindings.as_slice().is_empty() {
        #[cfg(test)]
        conditional_output_observation_v1_tests::reject_unannotated()?;
        #[cfg(test)]
        conditional_bound_observation_v1_tests::reject_unannotated()?;
        let ranked_ir = format_ranked_cfg(function_name(root_function)?, kernel.blocks())?;
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
        crate::production_reference_effect_join_v2::conditional::ReferenceRootV1::Ordinary {
            lowering,
            receipts: Vec::new(),
        }
    } else {
        let reserved_reference_values =
            reserved_reference_values.ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "reference-effect scalar reservations were not retained",
            ))?;
        let request =
            crate::production_reference_effect_join_v2::prepare_reference_effect_request_v2(
                kernel,
                reference_bindings,
                &reference_writes,
                reserved_reference_values,
            )
            .map_err(ProductionRankedProjectionErrorV1::ReferenceEffectJoin)?;
        #[cfg(test)]
        if conditional_output_observation_v1_tests::is_active() {
            let ranked_ir = format_ranked_cfg(
                function_name(root_function)?,
                request.kernel_for_test_v1().blocks(),
            )?;
            assertion_facts.observe_conditional_prepared_for_test_v1(
                fe2o3_lower_mir_kernel::NativeRankedSourceCandidateV1::from_untrusted_parts(
                    selection.root().index(),
                    source_root.source_rank(),
                    request.kernel_for_test_v1(),
                    &access_sources,
                    &executable_effect_sources,
                    &ranked_ir,
                ),
            )?;
        }
        #[cfg(test)]
        if conditional_bound_observation_v1_tests::is_active() {
            let bound = request
                .prove_and_bind()
                .map_err(ProductionRankedProjectionErrorV1::ReferenceEffectJoin)?;
            assertion_facts.observe_conditional_bound_for_test_v1(
                bound,
                conditional_bound_observation_v1_tests::BoundSourceV1 {
                    root: selection.root().index(),
                    rank: source_root.source_rank(),
                    access: &access_sources,
                    effects: &executable_effect_sources,
                    references: reference_bindings,
                    logical_name,
                },
            )?;
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                conditional_bound_observation_v1_tests::STOP,
            ));
        }
        crate::production_reference_effect_join_v2::conditional::ReferenceRootV1::Pending(request)
    };
    let ranked_ir = format_ranked_cfg(
        function_name(root_function)?,
        verification.kernel().blocks(),
    )?;
    // The test-only source qualifier retains this observation. Shipping builds
    // release the temporary expression forest before returning the owner.
    #[cfg(not(test))]
    drop(reference_writes);
    if helper_expression_storage != 0 {
        assertion_facts.release_scalar_private_storage_v1(helper_expression_storage)?;
    }
    let export_symbol = root_function
        .kernel_entry()
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic KernelRoot without an authenticated kernel entry",
        ))?
        .export_symbol()
        .as_bytes()
        .to_vec()
        .into_boxed_slice();
    Ok(ProductionRankedRootProgramV1 {
        logical_name: logical_name.to_owned(),
        export_symbol,
        semantic_root: selection.root(),
        semantic_root_identity: root_function.identity(),
        kernel_binding,
        source_rank: source_launch.rank(),
        semantic_u32_induction,
        verification,
        ranked_ir,
        access_sources,
        executable_effect_sources,
        #[cfg(test)]
        observed_reference_writes: reference_writes,
    })
}
