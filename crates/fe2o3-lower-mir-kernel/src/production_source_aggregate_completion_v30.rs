struct AggregateRuntimeStateV30 {
    globals: slice_view_v1::AggregateGlobalTransportV30,
    roles: AggregateSourceRolesV30,
}
impl AggregateStageStateV30 for AggregateRuntimeStateV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            self.globals.retained_storage()?,
            self.roles.retained_storage()?,
        ])
    }
}

/// Source-owned conditional completion of the complete actual Policy12 chain.
/// The final native facts and runtime occurrences are not an executed refinement
/// receipt, concrete allocation admission, or artifact/launch authority.
#[must_use = "retain the actual Policy12 chain and its final runtime requirements together"]
pub struct ProductionConditionalAggregateOutputHandoffV30<'chain, 'view, 'source> {
    chain: &'chain ProductionAggregateSourceOutputHandoffV30<'view, 'source>,
    premises: Vec<ProductionMixedSliceRuntimePremiseV26>,
    occurrences: Vec<ProductionMixedRuntimeOccurrenceV26>,
    histories: Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
    launches: &'chain [fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    retained: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ProductionConditionalAggregateOutputHandoffV30<'_, '_, '_> {
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let source = self.chain.owned.source;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            source.cleanup.deny_refund();
            return source.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.chain
            .owned
            .observe_retained_storage_v18(self.required, budget)
    }
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.custody(budget)?;
        self.chain.owned.check(budget)
    }
    /// Borrows the complete nominal chain, including every scalar/aggregate stage.
    pub fn output(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_opt::OwnedAggregateFixedpointV18> {
        self.check(budget)?;
        self.chain.output(budget)
    }
    /// Returns the complete original slice roster with actual final parameters.
    pub fn runtime_premises(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedSliceRuntimePremiseV26]> {
        self.check(budget)?;
        Ok(&self.premises)
    }
    /// Returns authenticated original occurrences joined to fresh final facts.
    pub fn runtime_occurrences(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedRuntimeOccurrenceV26]> {
        self.check(budget)?;
        Ok(&self.occurrences)
    }
    /// Returns the actual nine-stage histories; declarations have no history.
    pub fn native_histories(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>]> {
        self.check(budget)?;
        Ok(&self.histories)
    }
    /// Returns original per-root launch conditions and the retained formal width.
    pub fn launch_context(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        &[fe2o3_kernel_ir::ExplicitLaunchExtent],
        fe2o3_kernel_ir::FormalIndexWidth,
    )> {
        self.check(budget)?;
        Ok((self.launches, self.width))
    }
    /// Rejoins the genuine original semantic/SSA owner under current custody.
    pub fn check_original_source(
        &self,
        original: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.chain.check_original_source(original, budget)
    }
    /// Rejoins the exact complete captured original argument ABI.
    pub fn check_original_argument_abi_v30(
        &self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.chain.check_original_argument_abi_v30(abi, budget)
    }
    /// Returns this owner's credit, excluding the borrowed actual chain.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }
    /// Checks original-account custody without making a refused query successful.
    pub fn observe_retained_storage_v30(
        &self,
        required: usize,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        if budget.storage() < required {
            self.chain.owned.source.cleanup.deny_refund();
            return self
                .chain
                .owned
                .source
                .retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.custody(budget)
    }
    /// Drops owned runtime rows before settling only intact original-account credit.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let selected = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            chain,
            premises,
            occurrences,
            histories,
            retained,
            ..
        } = self;
        drop((premises, occurrences, histories));
        let settled = custody.and_then(|()| {
            chain
                .owned
                .source
                .retain_query(budget.release_storage(retained).map_err(Into::into))
        });
        selected?;
        settled
    }
    /// All original source roles are retained or independently retired by a checked aggregate stage.
    pub const fn source_roles_are_complete(&self) -> bool {
        true
    }
    /// Fresh final-owner native checks completed inside this source-owned consumer.
    pub const fn final_native_completion_is_complete(&self) -> bool {
        true
    }
    /// Concrete allocation, initialization, alias and launch admission remain open.
    pub const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    /// Complete generated and executed scalar/aggregate refinement remains separate.
    pub const fn executed_source_refinement_is_complete(&self) -> bool {
        false
    }
    /// A conditional native owner grants neither artifact nor launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl<'view, 'source> ProductionAggregateSourceOutputHandoffV30<'view, 'source> {
    /// Completes original source roles, every actual checked stage, and fresh
    /// final native/runtime conditions without projecting a scalar-only map.
    pub fn complete_native_v30<'chain>(
        &'chain self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        launches: &'chain [fe2o3_kernel_ir::ExplicitLaunchExtent],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionConditionalAggregateOutputHandoffV30<'chain, 'view, 'source>,
        ProductionAggregateSourceErrorV30,
    > {
        self.complete_native_inner_v30(
            abi,
            launches,
            width,
            budget,
            #[cfg(test)]
            None,
        )
    }

    fn complete_native_inner_v30<'chain>(
        &'chain self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        launches: &'chain [fe2o3_kernel_ir::ExplicitLaunchExtent],
        width: fe2o3_kernel_ir::FormalIndexWidth,
        budget: &mut ArgumentBudgetV1<'_>,
        #[cfg(test)] fault: Option<u8>,
    ) -> Result<
        ProductionConditionalAggregateOutputHandoffV30<'chain, 'view, 'source>,
        ProductionAggregateSourceErrorV30,
    > {
        use ProductionAggregateSourceErrorV30 as E;
        use slice_view_v1::AggregateGlobalTransportV30 as Globals;
        self.owned.check(budget)?;
        let source = self.owned.source;
        let floor = budget.storage();
        let (premises, occurrences, histories, retained) = scoped_source_attempt_v29(
            source.cleanup,
            budget,
            floor,
            |budget| {
                let entry = budget.storage();
                let result = (|| {
                    let header = aggregate_completion_headers_v30()?;
                    budget.reserve_storage(header)?;
                    self.check_original_argument_abi_v30(abi, budget)?;
                    let chain = self.output(budget)?;
                    let (premises, occurrences, histories) = with_aggregate_initial_source_v30(
                        source,
                        chain,
                        budget,
                        |original, optimized, budget| {
                            let mut state = fold_aggregate_source_stages_v30(
                                self,
                                |stage, state: Option<AggregateRuntimeStateV30>, budget| {
                                    if let Some(mut state) = state {
                                        state.globals = state.globals.advance(stage, budget)?;
                                        state.roles = state.roles.advance(stage, budget)?;
                                        return Ok(state);
                                    }
                                    if stage.ordinal != 0 {
                                        return source
                                            .missing("aggregate native seed is not first stage")
                                            .map_err(Into::into);
                                    }
                                    let (mut premises, mut occurrences) =
                                        Globals::storage_vectors(stage.output, budget)?;
                                    let mut roles = AggregateSourceRolesV30::seed_storage(
                                        stage.output,
                                        budget,
                                    )?;
                                    with_mixed_source_completion_v26(
                                        original,
                                        optimized,
                                        launches,
                                        width,
                                        budget,
                                        &mut |native, budget| {
                                            roles.fill_initial(
                                                native, original, optimized, budget,
                                            )?;
                                            Globals::fill_initial(
                                                native.completed_globals_v30(
                                                    original, optimized, budget,
                                                )?,
                                                original,
                                                optimized,
                                                &mut premises,
                                                &mut occurrences,
                                                budget,
                                            )?;
                                            Ok(())
                                        },
                                    )
                                    .map_err(E::InitialCompletion)?;
                                    let globals =
                                        Globals::seed(stage, premises, occurrences, budget)?
                                            .advance(stage, budget)?;
                                    Ok(AggregateRuntimeStateV30 { globals, roles })
                                },
                                budget,
                            )?;
                            #[cfg(test)]
                            match fault {
                                Some(1) => {
                                    state.roles.next_stage -= 1;
                                }
                                Some(2) => {
                                    state.roles.owner = 0;
                                }
                                Some(3) => {
                                    state.globals.omit_last_occurrence_v30();
                                }
                                Some(4) => {
                                    state.globals.omit_first_definition_transport_v30(3);
                                }
                                Some(5) => {
                                    state.globals.omit_first_definition_transport_v30(6);
                                }
                                None => (),
                                _ => panic!("unknown aggregate native test fault"),
                            }
                            let (output, inventory_credit) =
                                fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(
                                    chain.owner(),
                                    budget,
                                )
                                .map_err(E::Inventory)?;
                            budget.reserve_storage(inventory_credit.retained_storage())?;
                            let (physical, physical_credit) =
                                fe2o3_kernel_analysis::check_canonical_kir_private_memory_v18(
                                    &output,
                                    fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1 {
                                        max_cells: output.definitions().len(),
                                    },
                                    budget,
                                )
                                .map_err(ProductionSourceOwnedViewErrorV18::PrivateMemory)?;
                            budget.reserve_storage(physical_credit.retained_storage())?;
                            let function_launches = state
                                .globals
                                .function_launches(original, chain, &output, launches, budget)?;
                            let mut seen = source_reference_emission_vec_v29(
                                output.operations().len(),
                                budget,
                            )
                            .map_err(source_argument_error_v18)?;
                            budget.charge_work(output.operations().len())?;
                            seen.resize(output.operations().len(), false);
                            let mut histories =
                                source_reference_emission_vec_v29(output.functions().len(), budget)
                                    .map_err(source_argument_error_v18)?;
                            let layouts = source.limits(budget)?.storage_layout_limits();
                            with_source_pending_native_v30(
                                &output,
                                layouts,
                                budget,
                                |message| {
                                    ProductionSourceOwnedViewErrorV18::Binding(message).into()
                                },
                                |error| source.deny_aggregate_accounting_v30(error),
                                |pending, budget| {
                                    let mut selected = None;
                                    let mut native_result = None;
                                    let family =
                                        fe2o3_kernel_ir::with_canonical_guarded_global_reads_v18(
                                            chain.owner(),
                                            Default::default(),
                                            budget,
                                            |reads, budget| {
                                                fe2o3_kernel_ir::with_canonical_guarded_global_stores_v24(
                                                    chain.owner(),
                                                    Default::default(),
                                                    budget,
                                                    |stores, budget| {
                                                        fe2o3_kernel_ir::with_canonical_conditional_slice_domains_v26(
                                                            reads, stores, &function_launches, width, budget,
                                                            |globals, budget| {
                                                                native_result = Some(pending.with_mixed_memory_observations_v26(
                                                                    &physical, globals, budget,
                                                                    |native, budget| {
                                                                        let result = (|| {
                                                                            state.roles.join_final(
                                                                                chain, &output, native,
                                                                                &mut seen, &mut histories, budget,
                                                                            )?;
                                                                            state.globals.finish_domains(
                                                                                original, chain, &output, native,
                                                                                reads, stores, budget,
                                                                            )?;
                                                                            Ok(())
                                                                        })();
                                                                        selected = Some(source.retain_aggregate_result_v30(result));
                                                                        Ok(())
                                                                    },
                                                                ));
                                                                Ok(())
                                                            },
                                                        )
                                                    },
                                                )
                                            },
                                        );
                                    let called = selected.is_some();
                                    if let Some(Err(error)) = selected {
                                        return Err(error);
                                    }
                                    if family.map_err(|error| E::Native(
                                        ProductionSourceNativeLifecycleErrorV18::Pending(
                                            fe2o3_pliron::CanonicalRankedPolicyFailureV1::ConditionalGlobalsV26(error),
                                        ),
                                    ))?.is_none() {
                                        return source
                                            .missing("aggregate final conditional global family incomplete")
                                            .map_err(Into::into);
                                    }
                                    native_result
                                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                            "aggregate final native callback absent",
                                        ))?
                                        .map_err(|error| {
                                            E::Native(
                                                ProductionSourceNativeLifecycleErrorV18::Native(
                                                    error,
                                                ),
                                            )
                                        })?;
                                    if !called {
                                        return source
                                            .missing("aggregate final source conjunction absent")
                                            .map_err(Into::into);
                                    }
                                    Ok(())
                                },
                            )?;
                            self.owned.check(budget)?;
                            let roles_credit = state.roles.retained_storage()?;
                            let scratch = argument_sum_v1(&[
                                aggregate_vector_credit_v30(&function_launches)?,
                                aggregate_vector_credit_v30(&seen)?,
                                roles_credit,
                                inventory_credit.retained_storage(),
                                physical_credit.retained_storage(),
                            ])?;
                            drop((state.roles, function_launches, seen));
                            drop(physical);
                            drop(output);
                            budget.release_storage(scratch)?;
                            let (premises, occurrences) = state.globals.into_runtime(budget)?;
                            Ok((premises, occurrences, histories))
                        },
                    )?;
                    let owner = aggregate_completion_owner_headers_v30()?;
                    budget.reserve_storage(owner)?;
                    budget.release_storage(header)?;
                    let retained = argument_sum_v1(&[
                        owner,
                        aggregate_vector_credit_v30(&premises)?,
                        aggregate_vector_credit_v30(&occurrences)?,
                        aggregate_vector_credit_v30(&histories)?,
                    ])?;
                    if entry.checked_add(retained) != Some(budget.storage()) {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    Ok((premises, occurrences, histories, retained))
                })();
                source.retain_aggregate_result_v30(result)
            },
        )?;
        Ok(ProductionConditionalAggregateOutputHandoffV30 {
            chain: self,
            premises,
            occurrences,
            histories,
            launches,
            width,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}

fn aggregate_completion_owner_headers_v30() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<ProductionConditionalAggregateOutputHandoffV30<'_, '_, '_>>(),
        std::mem::align_of::<ProductionConditionalAggregateOutputHandoffV30<'_, '_, '_>>(),
        size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>() * 4,
        size_of::<[&(); 4]>(),
        size_of::<[usize; 5]>(),
    ])
}

fn aggregate_completion_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Rows = (
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
        Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
    );
    type Frame<'a> = (
        AggregateRuntimeStateV30,
        Option<AggregateRuntimeStateV30>,
        Result<AggregateRuntimeStateV30, ProductionAggregateSourceErrorV30>,
        &'a ProductionAggregateSourceOutputHandoffV30<'a, 'a>,
        [&'a (); 32],
        [usize; 24],
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'a, 'a>,
        Vec<bool>,
        Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>,
        Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
        Option<Result<(), ProductionAggregateSourceErrorV30>>,
        Option<Result<(), fe2o3_pliron::CanonicalRankedPolicyChecksErrorV1>>,
        Result<Option<()>, fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        aggregate_source_headers_v30()?,
        mixed_source_completion_headers_v26()?,
        aggregate_completion_owner_headers_v30()?,
        aggregate_role_headers_v30()?,
        size_of::<
            Result<
                &slice_view_v1::CompletedGlobalSourcesV26<'_, '_>,
                ProductionSourceNativeLifecycleErrorV18,
            >,
        >(),
        slice_view_v1::aggregate_global_headers_v30()?,
        size_of::<Rows>(),
        size_of::<Result<Rows, ProductionAggregateSourceErrorV30>>(),
        size_of::<
            Result<
                (
                    Vec<ProductionMixedSliceRuntimePremiseV26>,
                    Vec<ProductionMixedRuntimeOccurrenceV26>,
                    Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
                    usize,
                ),
                ProductionAggregateSourceErrorV30,
            >,
        >(),
        size_of::<
            Result<
                ProductionConditionalAggregateOutputHandoffV30<'_, '_, '_>,
                ProductionAggregateSourceErrorV30,
            >,
        >(),
        size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
                    fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >(),
        size_of::<
            Result<
                (
                    fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                    fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
                ),
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1,
            >,
        >(),
    ])
}
